//! Raster tracing: PNG in, contours out (M3 A12, A13; D-M3-8).
//!
//! # Pipeline
//!
//! 1. **Decode** the PNG by hand (D-M3-1): signature, `IHDR`, `PLTE`, every `IDAT` concatenated and
//!    inflated with `flate2`, then all five scanline filters undone (None, Sub, Up, Average,
//!    Paeth). 8-bit greyscale, RGB, palette and RGBA are read; 16-bit, interlaced and
//!    greyscale-with-alpha files are refused by name. Chunk CRCs are required to be present but are
//!    not verified: the data is inflated and bounds-checked anyway.
//! 2. **Luminance** `0.2126 R + 0.7152 G + 0.0722 B` (D-M3-8), palette entries being RGB, then a
//!    global threshold (`--threshold`, default 128, flipped by `--invert`) turns it into a binary
//!    mask. There is no adaptive thresholding and no smoothing, so the same bytes and options give
//!    the same mask (A13).
//! 3. **Marching squares** on the mask, on **pixel corners**: a foreground pixel occupies the unit
//!    square from its index to index + 1, and every mask edge between a foreground and a background
//!    pixel becomes a directed boundary edge. Boundaries are walked with a fixed neighbour order
//!    (the right-hand turn when the corner offers a choice, otherwise the only remaining edge) and
//!    every edge is consumed once, so a boundary is extracted exactly once. Where two foreground
//!    pixels touch only diagonally, the right-hand turn keeps them one 8-connected region, and the
//!    closed polyline passes through the shared corner.
//! 4. **Holes are their own closed contours**, wound the opposite way round: after the flip that
//!    puts the image's top row at the highest `y`, the outer boundary of a foreground region is
//!    counter-clockwise and the boundary of a hole inside it is clockwise. `cam_import`'s kernel
//!    contract of "closed contours come out counter-clockwise" therefore holds for the solid
//!    regions, and the winding is the machine-readable record of which contours are holes.
//! 5. **Millimetres** come from `--dpi` (default 96): `pixel * 25.4 / dpi`, with `y` flipped so the
//!    image's top row is the highest `y`. `--min-area` drops traced regions whose enclosed area is
//!    below the given square millimetres, and `--simplify` runs Douglas–Peucker on each surviving
//!    contour; both are reported. `--min-area` compares the area of the **traced** contour, before
//!    simplification, so the two options do not interact.
//!
//! A trace is a polyline by nature — there is no arc to preserve in a binary mask — so nothing is
//! counted as "flattened"; the report says so in a warning instead.

use std::io::Read;

use cam_geom::{Contour, Point2};

use crate::{ImportError, ImportOptions, ImportReport, ImportedLayer};

/// Format name, as the report and the errors spell it.
const FORMAT: &str = "png";

/// The one layer a trace produces: a raster carries no layers of its own.
const LAYER_NAME: &str = "trace";

/// The eight bytes every PNG starts with.
const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

/// Millimetres in an inch: the whole point of `--dpi`.
const MILLIMETRES_PER_INCH: f64 = 25.4;

/// Trace a PNG into closed contours in millimetres.
pub fn import(
  bytes: &[u8],
  options: &ImportOptions,
  report: &mut ImportReport,
) -> Result<Vec<ImportedLayer>, ImportError> {
  let trace = options.trace;
  if !(trace.dpi.is_finite() && trace.dpi > 0.0) {
    return Err(ImportError::refused(
      FORMAT,
      format!(
        "--dpi must be a positive number of dots per inch, got {}",
        trace.dpi
      ),
    ));
  }

  let decoded = decode(bytes)?;
  let millimetres_per_pixel = MILLIMETRES_PER_INCH / trace.dpi;
  let threshold = trace.effective_threshold();

  if decoded.translucent > 0 {
    report.warn(format!(
      "{} of {} RGBA pixels are not fully opaque; alpha is ignored and only luminance decides the mask",
      decoded.translucent,
      decoded.width * decoded.height
    ));
  }

  let threshold_luminance = f64::from(threshold);
  let foreground: Vec<bool> = decoded
    .luminance
    .iter()
    .map(|luminance| (*luminance < threshold_luminance) != trace.invert)
    .collect();

  let pixel_box = foreground_box(&foreground, decoded.width, decoded.height);
  let box_text = match pixel_box {
    Some((left, top, right, bottom)) => {
      format!("foreground pixel box ({left}, {top})-({right}, {bottom})")
    }
    None => "no foreground pixels".to_string(),
  };
  report.warn(format!(
    "trace: {}x{} px at {} dpi ({:.6} mm/px), threshold {}, invert {}, {box_text}",
    decoded.width, decoded.height, trace.dpi, millimetres_per_pixel, threshold, trace.invert
  ));
  report.warn("traced contours are straight-segment polylines: a raster has no arcs to preserve");

  let min_area = trace.min_area.max(0.0);
  let simplify = trace.simplify.max(0.0);
  let mut contours = Vec::new();
  for (ordinal, path) in trace_boundaries(&foreground, decoded.width, decoded.height)?
    .into_iter()
    .enumerate()
  {
    let ordinal = ordinal + 1;
    let location = format!("region {ordinal} at pixel ({}, {})", path[0].0, path[0].1);
    let points: Vec<Point2> = path
      .iter()
      .map(|&(x, y)| corner_to_millimetres(x, y, decoded.height, millimetres_per_pixel))
      .collect();
    let contour = build_contour(&points, &location)?;

    if contour.signed_area().abs() < min_area {
      report.skip("region", "area below --min-area", Some(location));
      continue;
    }

    let contour = if simplify > 0.0 {
      let reduced = simplify_closed(&points, simplify);
      build_contour(&reduced, &location)?
    } else {
      contour
    };
    contours.push(contour);
  }

  report.count("region", contours.len());
  report.unit_scale = millimetres_per_pixel;
  if contours.is_empty() {
    return Ok(Vec::new());
  }
  Ok(vec![ImportedLayer {
    name: LAYER_NAME.to_string(),
    contours,
  }])
}

/// Build the closed contour for one traced loop, keeping the failure inside [`ImportError`].
fn build_contour(points: &[Point2], location: &str) -> Result<Contour, ImportError> {
  Contour::from_points(points, true).map_err(|error| {
    ImportError::parse(
      FORMAT,
      format!("the traced boundary is not a valid contour: {error}"),
      location.to_string(),
    )
  })
}

/// Pixel corner `(x, y)` in millimetres, with the image's top row at the highest `y`.
fn corner_to_millimetres(x: usize, y: usize, height: usize, millimetres_per_pixel: f64) -> Point2 {
  Point2::new(
    x as f64 * millimetres_per_pixel,
    (height - y) as f64 * millimetres_per_pixel,
  )
}

/// The inclusive pixel box `(left, top, right, bottom)` of the foreground, if there is any.
fn foreground_box(
  foreground: &[bool],
  width: usize,
  height: usize,
) -> Option<(usize, usize, usize, usize)> {
  let mut found: Option<(usize, usize, usize, usize)> = None;
  for y in 0..height {
    for x in 0..width {
      if !foreground[y * width + x] {
        continue;
      }
      found = Some(match found {
        None => (x, y, x, y),
        Some((left, top, right, bottom)) => (left.min(x), top.min(y), right.max(x), bottom.max(y)),
      });
    }
  }
  found
}

// ---------------------------------------------------------------- PNG decoding

/// How a PNG sample turns into colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Colour {
  /// Colour type 0: one grey sample.
  Grey,
  /// Colour type 2: red, green, blue.
  Rgb,
  /// Colour type 3: a `PLTE` index.
  Palette,
  /// Colour type 6: red, green, blue, alpha.
  Rgba,
}

impl Colour {
  /// Bytes per pixel, which is also the Paeth/Average "bytes per pixel" stride.
  fn channels(self) -> usize {
    match self {
      Self::Grey | Self::Palette => 1,
      Self::Rgb => 3,
      Self::Rgba => 4,
    }
  }
}

/// The parsed `IHDR` fields this decoder acts on.
#[derive(Clone, Copy, Debug)]
struct Header {
  width: usize,
  height: usize,
  colour: Colour,
}

/// A decoded image: one luminance sample per pixel, plus what was ignored on the way.
struct Decoded {
  width: usize,
  height: usize,
  luminance: Vec<f64>,
  translucent: usize,
}

/// Read a PNG into luminance.
fn decode(raw: &[u8]) -> Result<Decoded, ImportError> {
  if raw.len() < SIGNATURE.len() || raw[..SIGNATURE.len()] != SIGNATURE {
    return Err(ImportError::parse(
      FORMAT,
      "the file does not start with the PNG signature",
      "byte 0",
    ));
  }

  let mut header: Option<Header> = None;
  let mut palette: Option<Vec<[u8; 3]>> = None;
  let mut idat: Vec<u8> = Vec::new();
  let mut offset = SIGNATURE.len();

  loop {
    if offset + 8 > raw.len() {
      return Err(ImportError::parse(
        FORMAT,
        "the file ends in the middle of a chunk header",
        format!("byte {offset}"),
      ));
    }
    let length = u32::from_be_bytes([
      raw[offset],
      raw[offset + 1],
      raw[offset + 2],
      raw[offset + 3],
    ]) as usize;
    let kind = [
      raw[offset + 4],
      raw[offset + 5],
      raw[offset + 6],
      raw[offset + 7],
    ];
    let data_start = offset + 8;
    let data_end = data_start.checked_add(length).ok_or_else(|| {
      ImportError::parse(
        FORMAT,
        "a chunk length overflows the file",
        format!("byte {offset}"),
      )
    })?;
    // The CRC is not verified, but it must be there: a missing one means a truncated file.
    let chunk_end = data_end.saturating_add(4);
    if chunk_end > raw.len() {
      return Err(ImportError::parse(
        FORMAT,
        format!(
          "chunk {} claims {length} data bytes, which runs past the end of the file",
          chunk_name(&kind)
        ),
        format!("byte {offset}"),
      ));
    }
    let data = &raw[data_start..data_end];

    match &kind {
      b"IHDR" => header = Some(parse_header(data)?),
      b"PLTE" => palette = Some(parse_palette(data)?),
      b"IDAT" => idat.extend_from_slice(data),
      b"IEND" => break,
      _ => {}
    }
    offset = chunk_end;
  }

  let header = header.ok_or_else(|| {
    ImportError::parse(
      FORMAT,
      "the file has no IHDR chunk",
      format!("byte {offset}"),
    )
  })?;
  if header.colour == Colour::Palette && palette.is_none() {
    return Err(ImportError::parse(
      FORMAT,
      "a palette image has no PLTE chunk",
      "PLTE",
    ));
  }
  if idat.is_empty() {
    return Err(ImportError::parse(
      FORMAT,
      "the image has no IDAT chunk",
      "IDAT",
    ));
  }

  let bytes_per_pixel = header.colour.channels();
  let row_bytes = header
    .width
    .checked_mul(bytes_per_pixel)
    .ok_or_else(|| ImportError::parse(FORMAT, "the image width overflows", "IHDR"))?;
  let expected = header
    .height
    .checked_mul(row_bytes + 1)
    .ok_or_else(|| ImportError::parse(FORMAT, "the image height overflows", "IHDR"))?;

  let mut inflated = Vec::new();
  flate2::read::ZlibDecoder::new(&idat[..])
    .read_to_end(&mut inflated)
    .map_err(|error| {
      ImportError::parse(
        FORMAT,
        format!("the IDAT stream does not inflate: {error}"),
        "IDAT",
      )
    })?;
  if inflated.len() < expected {
    return Err(ImportError::parse(
      FORMAT,
      format!(
        "the image data is {} bytes, {expected} expected for {}x{}",
        inflated.len(),
        header.width,
        header.height
      ),
      "IDAT",
    ));
  }

  let mut luminance = Vec::with_capacity(header.width * header.height);
  let mut translucent = 0;
  let mut previous = vec![0u8; row_bytes];
  let mut current = vec![0u8; row_bytes];
  for y in 0..header.height {
    let base = y * (row_bytes + 1);
    let filter = inflated[base];
    uninflate_row(
      filter,
      &inflated[base + 1..base + 1 + row_bytes],
      &previous,
      bytes_per_pixel,
      &mut current,
      y,
    )?;
    for x in 0..header.width {
      let sample = &current[x * bytes_per_pixel..(x + 1) * bytes_per_pixel];
      luminance.push(sample_luminance(
        sample,
        header.colour,
        palette.as_deref(),
        x,
        y,
        &mut translucent,
      )?);
    }
    previous.copy_from_slice(&current);
  }

  Ok(Decoded {
    width: header.width,
    height: header.height,
    luminance,
    translucent,
  })
}

/// A chunk type as text, for the error messages.
fn chunk_name(kind: &[u8; 4]) -> String {
  String::from_utf8_lossy(kind).into_owned()
}

/// Read the `IHDR` chunk, refusing what this decoder does not read.
fn parse_header(data: &[u8]) -> Result<Header, ImportError> {
  if data.len() != 13 {
    return Err(ImportError::parse(
      FORMAT,
      format!("IHDR is {} bytes, 13 expected", data.len()),
      "IHDR",
    ));
  }
  let width = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
  let height = u32::from_be_bytes([data[4], data[5], data[6], data[7]]) as usize;
  let bit_depth = data[8];
  let colour_type = data[9];
  let compression = data[10];
  let filter = data[11];
  let interlace = data[12];

  if width == 0 || height == 0 {
    return Err(ImportError::parse(
      FORMAT,
      format!("the image is {width}x{height} pixels"),
      "IHDR",
    ));
  }
  if compression != 0 {
    return Err(ImportError::refused(
      FORMAT,
      format!("compression method {compression} is not supported; only method 0 is read"),
    ));
  }
  if filter != 0 {
    return Err(ImportError::refused(
      FORMAT,
      format!("filter method {filter} is not supported; only method 0 is read"),
    ));
  }
  if interlace != 0 {
    return Err(ImportError::refused(
      FORMAT,
      format!(
        "interlace {interlace} is not supported; only non-interlaced (interlace 0) PNGs are read"
      ),
    ));
  }
  if bit_depth != 8 {
    return Err(ImportError::refused(
      FORMAT,
      format!("bit depth {bit_depth} is not supported; only 8-bit samples are read"),
    ));
  }
  let colour = match colour_type {
    0 => Colour::Grey,
    2 => Colour::Rgb,
    3 => Colour::Palette,
    4 => {
      return Err(ImportError::refused(
        FORMAT,
        "colour type 4 (greyscale with alpha) is not supported; greyscale, RGB, palette and RGBA are read",
      ));
    }
    6 => Colour::Rgba,
    other => {
      return Err(ImportError::refused(
        FORMAT,
        format!("colour type {other} is not supported; greyscale, RGB, palette and RGBA are read"),
      ));
    }
  };

  Ok(Header {
    width,
    height,
    colour,
  })
}

/// Read the `PLTE` chunk as RGB triples.
fn parse_palette(data: &[u8]) -> Result<Vec<[u8; 3]>, ImportError> {
  if data.len() % 3 != 0 {
    return Err(ImportError::parse(
      FORMAT,
      format!(
        "PLTE is {} bytes, which is not a whole number of RGB triples",
        data.len()
      ),
      "PLTE",
    ));
  }
  Ok(
    data
      .chunks_exact(3)
      .map(|entry| [entry[0], entry[1], entry[2]])
      .collect(),
  )
}

/// Undo one scanline filter (PNG 9.2), leaving the reconstructed bytes in `out`.
fn uninflate_row(
  filter: u8,
  source: &[u8],
  previous: &[u8],
  bytes_per_pixel: usize,
  out: &mut [u8],
  row: usize,
) -> Result<(), ImportError> {
  match filter {
    0 => out.copy_from_slice(source),
    1 => {
      for (index, byte) in source.iter().enumerate() {
        let left = if index >= bytes_per_pixel {
          out[index - bytes_per_pixel]
        } else {
          0
        };
        out[index] = byte.wrapping_add(left);
      }
    }
    2 => {
      for (index, byte) in source.iter().enumerate() {
        out[index] = byte.wrapping_add(previous[index]);
      }
    }
    3 => {
      for (index, byte) in source.iter().enumerate() {
        let left = if index >= bytes_per_pixel {
          u16::from(out[index - bytes_per_pixel])
        } else {
          0
        };
        let above = u16::from(previous[index]);
        out[index] = byte.wrapping_add(((left + above) / 2) as u8);
      }
    }
    4 => {
      for (index, byte) in source.iter().enumerate() {
        let left = if index >= bytes_per_pixel {
          i16::from(out[index - bytes_per_pixel])
        } else {
          0
        };
        let above = i16::from(previous[index]);
        let above_left = if index >= bytes_per_pixel {
          i16::from(previous[index - bytes_per_pixel])
        } else {
          0
        };
        out[index] = byte.wrapping_add(paeth(left, above, above_left));
      }
    }
    other => {
      return Err(ImportError::parse(
        FORMAT,
        format!("scanline filter {other} is not one of None, Sub, Up, Average or Paeth"),
        format!("row {row}"),
      ));
    }
  }
  Ok(())
}

/// The Paeth predictor of PNG 9.4, on the three already-known neighbouring bytes.
fn paeth(left: i16, above: i16, above_left: i16) -> u8 {
  let estimate = left + above - above_left;
  let to_left = (estimate - left).abs();
  let to_above = (estimate - above).abs();
  let to_above_left = (estimate - above_left).abs();
  let predictor = if to_left <= to_above && to_left <= to_above_left {
    left
  } else if to_above <= to_above_left {
    above
  } else {
    above_left
  };
  predictor as u8
}

/// `0.2126 R + 0.7152 G + 0.0722 B` on the 0..255 scale (D-M3-8).
fn luminance(red: u8, green: u8, blue: u8) -> f64 {
  0.2126 * f64::from(red) + 0.7152 * f64::from(green) + 0.0722 * f64::from(blue)
}

/// The luminance of one pixel, counting the alpha that RGBA throws away.
fn sample_luminance(
  sample: &[u8],
  colour: Colour,
  palette: Option<&[[u8; 3]]>,
  x: usize,
  y: usize,
  translucent: &mut usize,
) -> Result<f64, ImportError> {
  Ok(match colour {
    Colour::Grey => f64::from(sample[0]),
    Colour::Rgb => luminance(sample[0], sample[1], sample[2]),
    Colour::Rgba => {
      if sample[3] != 0xff {
        *translucent += 1;
      }
      luminance(sample[0], sample[1], sample[2])
    }
    Colour::Palette => {
      let index = usize::from(sample[0]);
      let entry = palette.and_then(|table| table.get(index)).ok_or_else(|| {
        ImportError::parse(
          FORMAT,
          format!("palette index {index} is past the end of the PLTE table"),
          format!("row {y}, column {x}"),
        )
      })?;
      luminance(entry[0], entry[1], entry[2])
    }
  })
}

// -------------------------------------------------------------------- tracing

/// One directed boundary edge: where it goes and whether the walk has used it.
struct Edge {
  to: usize,
  used: bool,
}

/// Extract the closed boundaries of every foreground region, in scan order.
///
/// Corners are numbered row-major over the `(width + 1) x (height + 1)` corner lattice, which is
/// what makes both the region order and each region's start point deterministic (A13).
fn trace_boundaries(
  foreground: &[bool],
  width: usize,
  height: usize,
) -> Result<Vec<Vec<(usize, usize)>>, ImportError> {
  let stride = width + 1;
  let mut outgoing: Vec<Vec<Edge>> = (0..stride * (height + 1)).map(|_| Vec::new()).collect();
  let is_foreground = |x: usize, y: usize| foreground[y * width + x];

  for y in 0..height {
    for x in 0..width {
      if !is_foreground(x, y) {
        continue;
      }
      // Top: leftwards along the pixel's top side, so the walk keeps the pixel on its right.
      if y == 0 || !is_foreground(x, y - 1) {
        outgoing[y * stride + x + 1].push(Edge {
          to: y * stride + x,
          used: false,
        });
      }
      // Right: upwards along the right side.
      if x + 1 == width || !is_foreground(x + 1, y) {
        outgoing[(y + 1) * stride + x + 1].push(Edge {
          to: y * stride + x + 1,
          used: false,
        });
      }
      // Bottom: rightwards along the bottom side.
      if y + 1 == height || !is_foreground(x, y + 1) {
        outgoing[(y + 1) * stride + x].push(Edge {
          to: (y + 1) * stride + x + 1,
          used: false,
        });
      }
      // Left: downwards along the left side.
      if x == 0 || !is_foreground(x - 1, y) {
        outgoing[y * stride + x].push(Edge {
          to: (y + 1) * stride + x,
          used: false,
        });
      }
    }
  }

  let mut loops = Vec::new();
  for start in 0..outgoing.len() {
    let mut next = outgoing[start].iter().position(|edge| !edge.used);
    while let Some(first) = next {
      loops.push(walk_boundary(start, first, &mut outgoing, stride)?);
      next = outgoing[start].iter().position(|edge| !edge.used);
    }
  }
  Ok(loops)
}

/// Walk one closed boundary from the unused edge `first` out of `start`, consuming every edge it
/// touches, and return the corners in walk order.
fn walk_boundary(
  start: usize,
  first: usize,
  outgoing: &mut [Vec<Edge>],
  stride: usize,
) -> Result<Vec<(usize, usize)>, ImportError> {
  outgoing[start][first].used = true;

  let mut path = vec![(start % stride, start / stride)];
  let mut corner = start;
  let mut edge = first;
  loop {
    let from_x = corner % stride;
    let from_y = corner / stride;
    let to = outgoing[corner][edge].to;
    if to == start {
      return Ok(path);
    }
    let to_x = to % stride;
    let to_y = to / stride;
    // Straight on is what a one-pixel-wide boundary normally does; when the boundary offers a
    // choice (two foreground pixels touching only diagonally) take the right-hand turn, so the
    // pair stays one region and the walk passes through the shared corner.
    let incoming = (
      to_x as isize - from_x as isize,
      to_y as isize - from_y as isize,
    );
    let wanted = (-incoming.1, incoming.0);
    let mut chosen = None;
    let mut fallback = None;
    for (index, candidate) in outgoing[to].iter().enumerate() {
      if candidate.used {
        continue;
      }
      let candidate_x = candidate.to % stride;
      let candidate_y = candidate.to / stride;
      let step = (
        candidate_x as isize - to_x as isize,
        candidate_y as isize - to_y as isize,
      );
      if step == wanted {
        chosen = Some(index);
        break;
      }
      if fallback.is_none() {
        fallback = Some(index);
      }
    }
    let Some(next_edge) = chosen.or(fallback) else {
      return Err(ImportError::parse(
        FORMAT,
        "the traced boundary does not close",
        format!("corner ({to_x}, {to_y})"),
      ));
    };
    outgoing[to][next_edge].used = true;
    path.push((to_x, to_y));
    corner = to;
    edge = next_edge;
  }
}

// --------------------------------------------------------------- simplification

/// Douglas–Peucker on a **closed** ring, keeping at least three vertices.
///
/// The ring is cut at vertex 0 and at the vertex farthest from it, the two open chains between them
/// are simplified, and the results are joined back together. Every kept vertex is an original
/// vertex, so simplification never moves anything: it only drops vertices, each within `tolerance`
/// of the polyline that replaced it.
fn simplify_closed(points: &[Point2], tolerance: f64) -> Vec<Point2> {
  if tolerance <= 0.0 || points.len() <= 3 {
    return points.to_vec();
  }

  let anchor = points[0];
  let mut far = 0;
  let mut far_distance = -1.0;
  for (index, point) in points.iter().enumerate() {
    let distance = point.distance_to(anchor);
    if distance > far_distance {
      far_distance = distance;
      far = index;
    }
  }
  if far == 0 || far + 1 == points.len() {
    return points.to_vec();
  }

  let mut kept = simplify_open(&points[..=far], tolerance);
  let mut second = points[far..].to_vec();
  second.push(anchor);
  let reduced = simplify_open(&second, tolerance);
  // The first vertex of the second chain (`far`) and its last (`points[0]`) are already kept.
  kept.extend(
    reduced
      .iter()
      .skip(1)
      .take(reduced.len().saturating_sub(2))
      .copied(),
  );

  if kept.len() >= 3 {
    return kept;
  }

  // A tolerance large enough to flatten the whole ring leaves two vertices; keep the three most
  // spread out original vertices instead of a contour the kernel would refuse.
  let mut third = 0;
  let mut third_distance = -1.0;
  for (index, point) in points.iter().enumerate() {
    let distance = distance_to_segment(*point, anchor, points[far]);
    if distance > third_distance {
      third_distance = distance;
      third = index;
    }
  }
  let mut indices = [0, far, third];
  indices.sort_unstable();
  indices.iter().map(|&index| points[index]).collect()
}

/// Douglas–Peucker on an open chain, endpoints kept.
fn simplify_open(points: &[Point2], tolerance: f64) -> Vec<Point2> {
  let count = points.len();
  if count <= 2 {
    return points.to_vec();
  }
  let mut keep = vec![false; count];
  keep[0] = true;
  keep[count - 1] = true;
  let mut pending = vec![(0usize, count - 1)];
  while let Some((start, end)) = pending.pop() {
    if end <= start + 1 {
      continue;
    }
    let mut farthest = start;
    let mut farthest_distance = -1.0;
    for (index, point) in points.iter().enumerate().take(end).skip(start + 1) {
      let distance = distance_to_segment(*point, points[start], points[end]);
      if distance > farthest_distance {
        farthest_distance = distance;
        farthest = index;
      }
    }
    if farthest_distance > tolerance {
      keep[farthest] = true;
      pending.push((start, farthest));
      pending.push((farthest, end));
    }
  }
  points
    .iter()
    .zip(keep.iter().copied())
    .filter_map(|(point, kept)| kept.then_some(*point))
    .collect()
}

/// Distance from `point` to the segment `start`–`end`.
fn distance_to_segment(point: Point2, start: Point2, end: Point2) -> f64 {
  let span = end - start;
  let length_squared = span.length_squared();
  if length_squared <= f64::EPSILON {
    return point.distance_to(start);
  }
  let t = ((point - start).dot(span) / length_squared).clamp(0.0, 1.0);
  point.distance_to(start.lerp(end, t))
}
