//! Behaviour tests for the PNG raster trace (M3 A12, A13; D-M3-8).
//!
//! Every fixture is built here, byte by byte, with `flate2` inflating what the decoder has to
//! inflate and real CRCs in the chunks: no file, no network, no absolute path.

use std::io::Write;

use cam_geom::{Contour, Point2};
use cam_import::{import_bytes, ImportError, ImportOptions, Imported, TraceOptions};

// ------------------------------------------------------------------- PNG writer

/// A PNG under construction. `bit_depth`, `interlace` and `truncated_idat` exist so a test can
/// build the files the decoder must refuse.
struct Png {
  width: u32,
  height: u32,
  bit_depth: u8,
  colour_type: u8,
  interlace: u8,
  palette: Vec<[u8; 3]>,
  samples: Vec<u8>,
  /// Per-row filter type; a short list is padded with filter 0 (None).
  filters: Vec<u8>,
  truncated_idat: bool,
}

impl Png {
  /// An 8-bit, non-interlaced, palette-free image of the given colour type.
  fn new(width: u32, height: u32, colour_type: u8, samples: &[u8]) -> Self {
    Self {
      width,
      height,
      bit_depth: 8,
      colour_type,
      interlace: 0,
      palette: Vec::new(),
      samples: samples.to_vec(),
      filters: Vec::new(),
      truncated_idat: false,
    }
  }

  /// 8-bit greyscale.
  fn grey(width: u32, height: u32, samples: &[u8]) -> Self {
    Self::new(width, height, 0, samples)
  }

  /// The channels this colour type stores per pixel.
  fn channels(&self) -> usize {
    match self.colour_type {
      2 => 3,
      6 => 4,
      _ => 1,
    }
  }

  /// The complete file: signature, `IHDR`, optional `PLTE`, one `IDAT`, `IEND`.
  fn bytes(&self) -> Vec<u8> {
    let channels = self.channels();
    let row_bytes = self.width as usize * channels;
    let mut scanlines = Vec::with_capacity(self.height as usize * (row_bytes + 1));
    let mut previous = vec![0u8; row_bytes];
    for row in 0..self.height as usize {
      let raw = &self.samples[row * row_bytes..(row + 1) * row_bytes];
      let filter = self.filters.get(row).copied().unwrap_or(0);
      scanlines.push(filter);
      scanlines.extend_from_slice(&apply_filter(filter, raw, &previous, channels));
      previous.copy_from_slice(raw);
    }
    let mut compressed = deflate(&scanlines);
    if self.truncated_idat {
      compressed.truncate(compressed.len() / 2);
    }

    let mut file = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut header = Vec::new();
    header.extend_from_slice(&self.width.to_be_bytes());
    header.extend_from_slice(&self.height.to_be_bytes());
    header.extend_from_slice(&[self.bit_depth, self.colour_type, 0, 0, self.interlace]);
    file.extend(chunk(b"IHDR", &header));
    if !self.palette.is_empty() {
      let mut table = Vec::new();
      for entry in &self.palette {
        table.extend_from_slice(entry);
      }
      file.extend(chunk(b"PLTE", &table));
    }
    file.extend(chunk(b"IDAT", &compressed));
    file.extend(chunk(b"IEND", &[]));
    file
  }
}

/// A PNG chunk: length, type, data, CRC over type and data.
fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
  let mut body = kind.to_vec();
  body.extend_from_slice(data);
  let mut out = Vec::with_capacity(body.len() + 8);
  out.extend_from_slice(&(data.len() as u32).to_be_bytes());
  out.extend_from_slice(&body);
  out.extend_from_slice(&crc32(&body).to_be_bytes());
  out
}

/// CRC-32 as PNG specifies it.
fn crc32(bytes: &[u8]) -> u32 {
  let mut crc = 0xffff_ffffu32;
  for byte in bytes {
    crc ^= u32::from(*byte);
    for _ in 0..8 {
      let mask = (crc & 1).wrapping_neg();
      crc = (crc >> 1) ^ (0xedb8_8320 & mask);
    }
  }
  !crc
}

/// zlib-compress a byte string.
fn deflate(bytes: &[u8]) -> Vec<u8> {
  let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
  encoder.write_all(bytes).expect("write");
  encoder.finish().expect("finish")
}

/// The forward direction of one scanline filter, so a test can build all five of them.
fn apply_filter(filter: u8, raw: &[u8], previous: &[u8], bytes_per_pixel: usize) -> Vec<u8> {
  let mut out = Vec::with_capacity(raw.len());
  for (index, byte) in raw.iter().enumerate() {
    let left = if index >= bytes_per_pixel {
      raw[index - bytes_per_pixel]
    } else {
      0
    };
    let above = previous[index];
    let above_left = if index >= bytes_per_pixel {
      previous[index - bytes_per_pixel]
    } else {
      0
    };
    let predictor = match filter {
      1 => left,
      2 => above,
      3 => ((u16::from(left) + u16::from(above)) / 2) as u8,
      4 => paeth(left, above, above_left),
      _ => 0,
    };
    out.push(byte.wrapping_sub(predictor));
  }
  out
}

/// The Paeth predictor, in the writer's direction.
fn paeth(left: u8, above: u8, above_left: u8) -> u8 {
  let left = i16::from(left);
  let above = i16::from(above);
  let above_left = i16::from(above_left);
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

// --------------------------------------------------------------------- helpers

const DPI: f64 = 96.0;

fn millimetres_per_pixel(dpi: f64) -> f64 {
  25.4 / dpi
}

fn options() -> ImportOptions {
  ImportOptions::default()
}

fn with_trace(
  threshold: Option<u8>,
  invert: bool,
  dpi: f64,
  min_area: f64,
  simplify: f64,
) -> ImportOptions {
  ImportOptions {
    tolerance: cam_import::DEFAULT_TOLERANCE,
    trace: TraceOptions {
      threshold,
      invert,
      dpi,
      min_area,
      simplify,
    },
  }
}

fn trace(png: &Png, options: &ImportOptions) -> Imported {
  import_bytes("art.png", &png.bytes(), options).expect("import")
}

/// The single contour of a trace that produced exactly one.
fn sole_contour(imported: &Imported) -> &Contour {
  assert_eq!(imported.layers.len(), 1, "one layer");
  assert_eq!(imported.layers[0].contours.len(), 1, "one contour");
  &imported.layers[0].contours[0]
}

fn close(actual: f64, expected: f64) {
  assert!(
    (actual - expected).abs() < 1e-12,
    "expected {expected}, got {actual}"
  );
}

fn distance_to_segment(point: Point2, start: Point2, end: Point2) -> f64 {
  let span = end - start;
  let length_squared = span.length_squared();
  if length_squared <= f64::EPSILON {
    return point.distance_to(start);
  }
  let t = ((point - start).dot(span) / length_squared).clamp(0.0, 1.0);
  point.distance_to(start.lerp(end, t))
}

fn distance_to_closed_polyline(point: Point2, vertices: &[Point2]) -> f64 {
  let mut best = f64::INFINITY;
  for index in 0..vertices.len() {
    let start = vertices[index];
    let end = vertices[(index + 1) % vertices.len()];
    best = best.min(distance_to_segment(point, start, end));
  }
  best
}

/// A 4x4 grey image with a 2x2 dark block at pixel (1, 1).
fn block_samples() -> Vec<u8> {
  vec![
    255, 255, 255, 255, //
    255, 0, 0, 255, //
    255, 0, 0, 255, //
    255, 255, 255, 255,
  ]
}

// ----------------------------------------------------------------------- tests

#[test]
fn a_dark_block_traces_to_its_millimetre_bounds_at_96_dpi() {
  let pixels = millimetres_per_pixel(DPI);
  let imported = trace(&Png::grey(4, 4, &block_samples()), &options());

  assert_eq!(imported.layers.len(), 1);
  assert_eq!(imported.layers[0].name, "trace");
  let contour = sole_contour(&imported);
  assert!(contour.is_closed(), "a traced boundary is closed");
  assert!(!contour.has_arcs(), "a mask has no arcs");

  // The block occupies pixels x 1..=2, y 1..=2, so its corners are (1,1) and (3,3); the image is
  // 4 rows tall, so image row 1 becomes the highest y.
  let bounds = contour.bounds();
  close(bounds.min.x, pixels);
  close(bounds.min.y, pixels);
  close(bounds.max.x, 3.0 * pixels);
  close(bounds.max.y, 3.0 * pixels);
  assert!(contour.is_ccw(), "an outer boundary is counter-clockwise");
  close(contour.signed_area(), 4.0 * pixels * pixels);
  close(contour.perimeter(), 8.0 * pixels);

  assert_eq!(imported.report.contours, 1);
  assert_eq!(imported.report.unit_scale, 25.4 / DPI);
  assert_eq!(imported.report.flattened_curves, 0);

  let expected = [
    "format: png",
    "tolerance: 0.010 mm",
    "unit scale: 0.264583",
    "contours: 1",
    "flattened curves: 0",
    "sources: region=1",
    "skipped: 0",
    "warnings:",
    "  - trace: 4x4 px at 96 dpi (0.264583 mm/px), threshold 128, invert false, foreground pixel box (1, 1)-(2, 2)",
    "  - traced contours are straight-segment polylines: a raster has no arcs to preserve",
    "",
  ]
  .join("\n");
  assert_eq!(imported.report.render(), expected);
}

#[test]
fn a_donut_traces_an_outer_contour_and_a_hole_of_the_opposite_winding() {
  let pixels = millimetres_per_pixel(DPI);
  // 5x5, every pixel dark except the light centre: a frame with a one-pixel hole.
  let mut samples = vec![0u8; 25];
  samples[2 * 5 + 2] = 255;
  let imported = trace(&Png::grey(5, 5, &samples), &options());

  let contours = &imported.layers[0].contours;
  assert_eq!(contours.len(), 2, "the frame and its hole");
  let outer = &contours[0];
  let hole = &contours[1];

  // The outer boundary of the solid region is counter-clockwise; the hole inside it is clockwise,
  // which is how a caller tells a hole from an island.
  assert!(outer.is_ccw());
  assert!(!hole.is_ccw());
  close(outer.signed_area(), 25.0 * pixels * pixels);
  close(hole.signed_area(), -pixels * pixels);

  let outer_bounds = outer.bounds();
  close(outer_bounds.min.x, 0.0);
  close(outer_bounds.min.y, 0.0);
  close(outer_bounds.max.x, 5.0 * pixels);
  close(outer_bounds.max.y, 5.0 * pixels);

  let hole_bounds = hole.bounds();
  close(hole_bounds.min.x, 2.0 * pixels);
  close(hole_bounds.min.y, 2.0 * pixels);
  close(hole_bounds.max.x, 3.0 * pixels);
  close(hole_bounds.max.y, 3.0 * pixels);

  assert_eq!(imported.report.contours, 2);
  assert_eq!(imported.report.sources.get("region"), Some(&2));
  assert_eq!(imported.report.skipped_count(), 0);
}

#[test]
fn invert_traces_the_complement_region() {
  let pixels = millimetres_per_pixel(DPI);
  // The same 4x4 image with the polarity swapped: a light block on a dark field.
  let samples = vec![
    0, 0, 0, 0, //
    0, 255, 255, 0, //
    0, 255, 255, 0, //
    0, 0, 0, 0,
  ];
  let png = Png::grey(4, 4, &samples);

  let normal = trace(&png, &options());
  assert_eq!(
    normal.layers[0].contours.len(),
    2,
    "the dark field is a frame with a hole"
  );
  close(
    normal.layers[0].contours[0].signed_area(),
    16.0 * pixels * pixels,
  );
  close(
    normal.layers[0].contours[1].signed_area(),
    -4.0 * pixels * pixels,
  );

  let inverted = trace(&png, &with_trace(None, true, DPI, 0.0, 0.0));
  let contour = sole_contour(&inverted);
  let bounds = contour.bounds();
  close(bounds.min.x, pixels);
  close(bounds.min.y, pixels);
  close(bounds.max.x, 3.0 * pixels);
  close(bounds.max.y, 3.0 * pixels);
  close(contour.signed_area(), 4.0 * pixels * pixels);
  assert!(inverted
    .report
    .warnings
    .iter()
    .any(|warning| warning.contains("invert true")));
}

#[test]
fn the_threshold_decides_where_the_mask_edge_falls() {
  let pixels = millimetres_per_pixel(DPI);
  // Four columns of increasing grey: 50, 100, 180, 220, repeated down four rows.
  let mut samples = Vec::new();
  for _ in 0..4 {
    samples.extend_from_slice(&[50u8, 100, 180, 220]);
  }
  let png = Png::grey(4, 4, &samples);

  // The default threshold of 128 keeps the two darkest columns.
  let default = trace(&png, &options());
  let contour = sole_contour(&default);
  close(contour.bounds().max.x, 2.0 * pixels);
  close(contour.bounds().max.y, 4.0 * pixels);
  close(contour.signed_area(), 8.0 * pixels * pixels);

  // A lower threshold keeps only the darkest column.
  let low = trace(&png, &with_trace(Some(60), false, DPI, 0.0, 0.0));
  let contour = sole_contour(&low);
  close(contour.bounds().max.x, pixels);
  close(contour.signed_area(), 4.0 * pixels * pixels);
  assert!(low
    .report
    .warnings
    .iter()
    .any(|warning| warning.contains("threshold 60")));

  // A higher threshold keeps three columns.
  let high = trace(&png, &with_trace(Some(200), false, DPI, 0.0, 0.0));
  let contour = sole_contour(&high);
  close(contour.bounds().max.x, 3.0 * pixels);
  close(contour.signed_area(), 12.0 * pixels * pixels);
  assert!(high
    .report
    .warnings
    .iter()
    .any(|warning| warning.contains("threshold 200")));
}

#[test]
fn min_area_drops_a_single_pixel_speck_and_reports_it() {
  let pixels = millimetres_per_pixel(DPI);
  // A 2x2 block worth 4 px^2 and one isolated speck worth 1 px^2: 0.2796 mm^2 against 0.0699 mm^2.
  let mut samples = vec![255u8; 36];
  for (x, y) in [(1, 1), (2, 1), (1, 2), (2, 2), (5, 5)] {
    samples[y * 6 + x] = 0;
  }
  let imported = trace(
    &Png::grey(6, 6, &samples),
    &with_trace(None, false, DPI, 0.1, 0.0),
  );

  let contour = sole_contour(&imported);
  close(contour.signed_area(), 4.0 * pixels * pixels);
  assert_eq!(imported.report.skipped_count(), 1);
  let skipped = &imported.report.skipped[0];
  assert_eq!(skipped.construct, "region");
  assert_eq!(skipped.reason, "area below --min-area");
  assert_eq!(
    skipped.location.as_deref(),
    Some("region 2 at pixel (5, 5)")
  );
  assert!(imported
    .report
    .render()
    .contains("  - region at region 2 at pixel (5, 5): area below --min-area\n"));
  assert!(imported.report.render().contains("sources: region=1\n"));
}

#[test]
fn simplify_reduces_a_staircase_without_moving_a_vertex_past_the_tolerance() {
  // The lower-right triangle of a 10x10 image: its diagonal is a ten-step staircase.
  let samples: Vec<u8> = (0..100)
    .map(|index| {
      let x = index % 10;
      let y = index / 10;
      if x + y >= 9 {
        0
      } else {
        255
      }
    })
    .collect();
  let png = Png::grey(10, 10, &samples);

  let raw = trace(&png, &options());
  let simplified = trace(&png, &with_trace(None, false, DPI, 0.0, 0.5));
  let raw_contour = sole_contour(&raw);
  let simplified_contour = sole_contour(&simplified);

  assert!(
    raw_contour.len() >= 20,
    "a staircase has many corners, got {}",
    raw_contour.len()
  );
  assert!(
    simplified_contour.len() < raw_contour.len(),
    "{} is not fewer than {}",
    simplified_contour.len(),
    raw_contour.len()
  );
  assert!(
    simplified_contour.len() >= 3,
    "a closed contour keeps three"
  );

  let raw_points: Vec<Point2> = raw_contour
    .vertices()
    .iter()
    .map(|vertex| vertex.point)
    .collect();
  let kept: Vec<Point2> = simplified_contour
    .vertices()
    .iter()
    .map(|vertex| vertex.point)
    .collect();

  // Simplification only drops vertices: every kept vertex is an original one, unmoved.
  for point in &kept {
    assert!(
      raw_points.iter().any(|original| original == point),
      "kept vertex {point:?} is not an original vertex"
    );
  }
  // And every dropped vertex is within the tolerance of the polyline that replaced it.
  for point in &raw_points {
    let distance = distance_to_closed_polyline(*point, &kept);
    assert!(
      distance <= 0.5 + 1e-9,
      "vertex {point:?} moved {distance} mm from the original polyline"
    );
  }

  let raw_bounds = raw_contour.bounds();
  let kept_bounds = simplified_contour.bounds();
  assert!((kept_bounds.min.x - raw_bounds.min.x).abs() <= 0.5 + 1e-9);
  assert!((kept_bounds.min.y - raw_bounds.min.y).abs() <= 0.5 + 1e-9);
  assert!((kept_bounds.max.x - raw_bounds.max.x).abs() <= 0.5 + 1e-9);
  assert!((kept_bounds.max.y - raw_bounds.max.y).abs() <= 0.5 + 1e-9);
}

#[test]
fn a_sixteen_bit_png_is_refused_naming_the_bit_depth() {
  let mut png = Png::grey(4, 4, &block_samples());
  png.bit_depth = 16;
  match import_bytes("deep.png", &png.bytes(), &options()) {
    Err(ImportError::Refused { format, reason }) => {
      assert_eq!(format, "png");
      assert!(reason.contains("bit depth 16"), "{reason}");
    }
    other => panic!("expected a refusal, got {other:?}"),
  }
}

#[test]
fn an_interlaced_png_is_refused_naming_the_interlace() {
  let mut png = Png::grey(4, 4, &block_samples());
  png.interlace = 1;
  match import_bytes("adam7.png", &png.bytes(), &options()) {
    Err(ImportError::Refused { format, reason }) => {
      assert_eq!(format, "png");
      assert!(reason.contains("interlace 1"), "{reason}");
    }
    other => panic!("expected a refusal, got {other:?}"),
  }
}

#[test]
fn a_truncated_idat_is_a_parse_error() {
  let mut png = Png::grey(4, 4, &block_samples());
  png.truncated_idat = true;
  match import_bytes("cut.png", &png.bytes(), &options()) {
    Err(ImportError::Parse { format, .. }) => assert_eq!(format, "png"),
    other => panic!("expected a parse error, got {other:?}"),
  }
}

#[test]
fn tracing_the_same_bytes_twice_yields_identical_contours() {
  let samples: Vec<u8> = (0..100)
    .map(|index| {
      let x = index % 10;
      let y = index / 10;
      if (x + y) % 7 < 3 || (x >= 6 && y <= 4) {
        40
      } else {
        220
      }
    })
    .collect();
  let png = Png::grey(10, 10, &samples);
  let options = with_trace(Some(140), false, 300.0, 0.2, 0.1);

  let first = trace(&png, &options);
  let second = trace(&png, &options);
  assert_eq!(first.layers, second.layers);
  assert_eq!(first.report, second.report);
  assert!(first.report.contours > 0);
}

#[test]
fn rgb_palette_and_rgba_samples_trace_the_same_mask_as_grey() {
  let grey = trace(&Png::grey(4, 4, &block_samples()), &options());

  // The same image as RGB: white field, black block.
  let mut rgb = Vec::new();
  for value in block_samples() {
    rgb.extend_from_slice(&[value, value, value]);
  }
  let rgb = trace(&Png::new(4, 4, 2, &rgb), &options());

  // The same image as a two-entry palette: index 0 white, index 1 black.
  let indices: Vec<u8> = block_samples()
    .into_iter()
    .map(|value| u8::from(value == 0))
    .collect();
  let mut palette = Png::new(4, 4, 3, &indices);
  palette.palette = vec![[255, 255, 255], [0, 0, 0]];
  let palette = trace(&palette, &options());

  // The same image as RGBA, fully opaque.
  let mut rgba = Vec::new();
  for value in block_samples() {
    rgba.extend_from_slice(&[value, value, value, 255]);
  }
  let rgba = trace(&Png::new(4, 4, 6, &rgba), &options());

  assert_eq!(grey.layers, rgb.layers);
  assert_eq!(grey.layers, palette.layers);
  assert_eq!(grey.layers, rgba.layers);
  assert_eq!(
    grey.report.warnings.len(),
    2,
    "no alpha warning when opaque"
  );
}

#[test]
fn a_translucent_rgba_pixel_is_reported_not_silently_ignored() {
  let mut rgba = Vec::new();
  for value in block_samples() {
    rgba.extend_from_slice(&[value, value, value, 255]);
  }
  // One pixel is half transparent; luminance still decides, and the report says so.
  rgba[3] = 128;
  let imported = trace(&Png::new(4, 4, 6, &rgba), &options());

  close(
    sole_contour(&imported).signed_area(),
    4.0 * (25.4 / DPI) * (25.4 / DPI),
  );
  assert_eq!(imported.report.warnings.len(), 3);
  assert_eq!(
    imported.report.warnings[0],
    "1 of 16 RGBA pixels are not fully opaque; alpha is ignored and only luminance decides the mask"
  );
}

#[test]
fn the_dpi_decides_the_pixel_to_millimetre_scale() {
  let png = Png::grey(4, 4, &block_samples());
  let coarse = trace(&png, &options());
  let fine = trace(&png, &with_trace(None, false, 192.0, 0.0, 0.0));

  let coarse_bounds = sole_contour(&coarse).bounds();
  let fine_bounds = sole_contour(&fine).bounds();
  assert!((fine_bounds.max.x - coarse_bounds.max.x / 2.0).abs() < 1e-12);
  assert!((fine_bounds.max.y - coarse_bounds.max.y / 2.0).abs() < 1e-12);
  assert_eq!(fine.report.unit_scale, 25.4 / 192.0);
  assert!(fine
    .report
    .warnings
    .iter()
    .any(|warning| warning.contains("192 dpi (0.132292 mm/px)")));
}

#[test]
fn every_scanline_filter_reconstructs_the_same_image() {
  let samples = block_samples();
  let baseline = trace(&Png::grey(4, 4, &samples), &options());
  assert_eq!(baseline.layers[0].contours.len(), 1);

  for filters in [
    vec![1u8, 1, 1, 1],
    vec![2, 2, 2, 2],
    vec![3, 3, 3, 3],
    vec![4, 4, 4, 4],
    vec![0, 3, 4, 1],
  ] {
    let mut png = Png::grey(4, 4, &samples);
    png.filters = filters.clone();
    let filtered = trace(&png, &options());
    assert_eq!(filtered.layers, baseline.layers, "filters {filters:?}");
    assert_eq!(filtered.report, baseline.report, "filters {filters:?}");
  }
}

#[test]
fn a_non_positive_dpi_is_refused() {
  let png = Png::grey(4, 4, &block_samples());
  match import_bytes(
    "art.png",
    &png.bytes(),
    &with_trace(None, false, 0.0, 0.0, 0.0),
  ) {
    Err(ImportError::Refused { format, reason }) => {
      assert_eq!(format, "png");
      assert!(reason.contains("--dpi"), "{reason}");
    }
    other => panic!("expected a refusal, got {other:?}"),
  }
}

#[test]
fn a_diagonal_pair_of_dark_pixels_is_one_eight_connected_region() {
  // Pixels (1, 1) and (2, 2) touch only at a corner. The walk takes the right-hand turn there, so
  // the pair is one region whose boundary passes through the shared corner exactly once per lobe.
  let pixels = millimetres_per_pixel(DPI);
  let mut samples = vec![255u8; 16];
  samples[4 + 1] = 0;
  samples[2 * 4 + 2] = 0;
  let imported = trace(&Png::grey(4, 4, &samples), &options());

  let contour = sole_contour(&imported);
  close(contour.signed_area(), 2.0 * pixels * pixels);
  let bounds = contour.bounds();
  close(bounds.min.x, pixels);
  close(bounds.min.y, pixels);
  close(bounds.max.x, 3.0 * pixels);
  close(bounds.max.y, 3.0 * pixels);
  assert_eq!(
    contour.vertices().len(),
    8,
    "one loop with the shared corner visited twice"
  );
  assert_eq!(imported.report.contours, 1);
}
