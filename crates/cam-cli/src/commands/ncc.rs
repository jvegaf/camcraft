//! Non-copper clearing: generate NCC G-code from a document's NonCopperClear toolpaths (F4).
//!
//! Finds every `Operation::NonCopperClear` toolpath, builds the non-copper envelope from the
//! referenced copper contours and the board outline, fills it with the selected method, and emits
//! G-code through the post-processor.

use std::path::PathBuf;

use cam_geom::Point2;
use cam_model::{Document, FillMethod, Operation};
use cam_move::{MoveStream, Segment, ToolShape};
use cam_post::{emit, parse_post, ProgramMeta};
use cam_units::Length;

use crate::{number, open, option_value, write_report, Failure};

struct Flags {
  document: Option<PathBuf>,
  post: Option<PathBuf>,
  out: Option<PathBuf>,
  tool_number: u32,
  diameter: Option<f64>,
  feed: f64,
  plunge: f64,
  spindle: u32,
}

fn parse_flags(rest: &[String]) -> Result<Flags, Failure> {
  let mut flags = Flags {
    document: None,
    post: None,
    out: None,
    tool_number: 1,
    diameter: None,
    feed: 100.0,
    plunge: 30.0,
    spindle: 18_000,
  };

  let mut index = 0;
  while index < rest.len() {
    let flag = rest[index].as_str();
    match flag {
      "--post" => {
        flags.post = Some(PathBuf::from(option_value(rest, index, "--post")?));
        index += 2;
      }
      "--out" => {
        flags.out = Some(PathBuf::from(option_value(rest, index, "--out")?));
        index += 2;
      }
      "--tool" => {
        let value = option_value(rest, index, "--tool")?;
        flags.tool_number = value
          .parse::<u32>()
          .map_err(|_| Failure::input(format!("--tool expects a tool number, got {value:?}")))?;
        index += 2;
      }
      "--diameter" => {
        flags.diameter = Some(number(rest, index, "--diameter")?);
        index += 2;
      }
      "--feed" => {
        flags.feed = number(rest, index, "--feed")?;
        index += 2;
      }
      "--plunge" => {
        flags.plunge = number(rest, index, "--plunge")?;
        index += 2;
      }
      "--spindle" => {
        let value = option_value(rest, index, "--spindle")?;
        flags.spindle = value.parse::<u32>().map_err(|_| {
          Failure::input(format!("--spindle expects a speed in rpm, got {value:?}"))
        })?;
        index += 2;
      }
      other if other.starts_with('-') => {
        return Err(Failure::input(format!("unknown ncc option {other:?}")));
      }
      other => {
        if flags.document.is_some() {
          return Err(Failure::input("ncc takes exactly one document"));
        }
        flags.document = Some(PathBuf::from(other));
        index += 1;
      }
    }
  }

  Ok(flags)
}

struct NccRun {
  name: String,
  contours: usize,
  moves: Vec<cam_move::Move>,
}

/// `cam-cli ncc <document>`: non-copper clearing for the document's NonCopperClear toolpaths.
pub fn run(rest: &[String]) -> Result<(), Failure> {
  let flags = parse_flags(rest)?;
  let path = flags
    .document
    .clone()
    .ok_or_else(|| Failure::input("ncc requires a document path"))?;
  let document = open(&path)?;
  let diameter = flags
    .diameter
    .ok_or_else(|| Failure::input("ncc requires --diameter <mm>"))?;
  if diameter <= 0.0 {
    return Err(Failure::input("--diameter must be greater than zero"));
  }

  let (runs, total_moves) = ncc_runs(&document, diameter)?;

  let Some(post_path) = flags.post.clone() else {
    if flags.out.is_some() {
      return Err(Failure::input(
        "ncc --out writes the program, so it needs --post: without a post-processor the run \
         prints a report and writes no file",
      ));
    }
    let mut report = String::new();
    for run in &runs {
      report.push_str(&format!(
        "ncc {}: {} contour(s), {} move(s)\n",
        run.name,
        run.contours,
        run.moves.len()
      ));
    }
    report.push_str(&format!(
      "ncc: {} toolpath(s), {} move(s)\n",
      runs.len(),
      total_moves
    ));
    return write_report(&mut std::io::stdout(), &report);
  };

  let mut stream = MoveStream::new(
    Length::from_mm(cam_model::DEFAULT_SAFE_Z_MM),
    Point2::new(0.0, 0.0),
  );
  for run in runs {
    stream.segments.push(Segment {
      name: run.name,
      tool_number: flags.tool_number,
      tool: ToolShape::cylinder(Length::from_mm(diameter)),
      feed_rate: flags.feed,
      plunge_rate: flags.plunge,
      spindle_speed: flags.spindle,
      moves: run.moves,
    });
  }
  emit_program(&stream, &post_path, flags.out.as_deref(), &path)
}

fn ncc_runs(document: &Document, diameter: f64) -> Result<(Vec<NccRun>, usize), Failure> {
  let tolerance = cam_geom::TolerancePolicy::uniform(1.0e-6);
  let mut runs = Vec::new();
  let mut total = 0usize;

  for toolpath in &document.toolpaths {
    let Operation::NonCopperClear(params) = &toolpath.operation else {
      continue;
    };

    let mut copper_contours = Vec::new();
    let mut outline_contours = Vec::new();
    for entity_id in &toolpath.entity_ids {
      let Some(entity) = document.entity(entity_id.as_str()) else {
        return Err(Failure::input(format!(
          "ncc toolpath {:?} references unknown entity {:?}",
          toolpath.name, entity_id
        )));
      };
      let Some(contour) = entity.kind.as_contour() else {
        return Err(Failure::input(format!(
          "ncc toolpath {:?} entity {:?} is not a contour",
          toolpath.name, entity_id
        )));
      };
      let role = document.layers.iter().find_map(|layer| {
        if layer.entities.iter().any(|e| e.id == *entity_id) {
          Some(layer.role)
        } else {
          None
        }
      });
      match role {
        Some(cam_model::LayerRole::Outline) => outline_contours.push(contour.clone()),
        _ => copper_contours.push(contour.clone()),
      }
    }

    let boundary = if outline_contours.is_empty() {
      // Fallback: use board rectangle as boundary.
      let w = document.board.width.mm();
      let h = document.board.height.mm();
      let rect = cam_geom::Contour::from_points(
        &[
          Point2::new(0.0, 0.0),
          Point2::new(w, 0.0),
          Point2::new(w, h),
          Point2::new(0.0, h),
        ],
        true,
      )
      .map_err(|err| Failure::input(format!("ncc {:?}: {err}", toolpath.name)))?;
      cam_geom::region::Region::new(vec![rect])
        .and_then(|r| r.normalize(tolerance).map(|n| n.region))
        .map_err(|err| Failure::input(format!("ncc {:?}: {err}", toolpath.name)))?
    } else {
      let outline_region = cam_geom::region::Region::new(outline_contours)
        .and_then(|r| r.normalize(tolerance).map(|n| n.region))
        .map_err(|err| Failure::input(format!("ncc {:?}: {err}", toolpath.name)))?;
      outline_region_to_board(&outline_region, tolerance)
        .map_err(|err| Failure::input(format!("ncc {:?}: {err}", toolpath.name)))?
    };

    let margin = params.margin.mm();
    let copper_offset = params.copper_offset.map(|l| l.mm()).unwrap_or(0.0);
    let clearing_limit = if margin == 0.0 {
      boundary.clone()
    } else {
      boundary
        .offset(margin, tolerance)
        .map_err(|err| Failure::input(format!("ncc {:?}: {err}", toolpath.name)))?
        .region
    };
    let united_copper = cam_ops::unite_copper(&copper_contours, tolerance)
      .map_err(|err| Failure::input(format!("ncc {:?}: {err}", toolpath.name)))?;
    let keep_out = if copper_offset == 0.0 {
      united_copper
    } else {
      united_copper
        .offset(-copper_offset, tolerance)
        .map_err(|err| Failure::input(format!("ncc {:?}: {err}", toolpath.name)))?
        .region
    };

    let raw_fill = match params.method {
      FillMethod::Standard => {
        cam_ops::ncc_fill_rings(&clearing_limit, diameter, params.overlap, tolerance)
      }
      FillMethod::Seed => {
        cam_ops::ncc_fill_seed(&clearing_limit, diameter, params.overlap, tolerance)
      }
      FillMethod::Lines => {
        cam_ops::ncc_fill_lines(&clearing_limit, diameter, params.overlap, tolerance)
      }
    }
    .map_err(|err| Failure::input(format!("ncc {:?}: {err}", toolpath.name)))?;

    let fill_region = cam_geom::region::Region::new(raw_fill.contours.clone())
      .and_then(|r| r.normalize(tolerance).map(|n| n.region))
      .map_err(|err| Failure::input(format!("ncc {:?}: {err}", toolpath.name)))?;
    let clip_keepout = if keep_out.boundaries.is_empty() {
      keep_out.clone()
    } else {
      let tool_radius = diameter / 2.0;
      keep_out
        .offset(-tool_radius, tolerance)
        .map_err(|err| Failure::input(format!("ncc {:?}: {err}", toolpath.name)))?
        .region
    };
    let clipped_region = if clip_keepout.boundaries.is_empty() {
      fill_region
    } else {
      match cam_geom::boolean::combine(
        &fill_region,
        &clip_keepout,
        cam_geom::boolean::BooleanOp::Difference,
        tolerance,
      )
      .map_err(|err| Failure::input(format!("ncc {:?}: {err}", toolpath.name)))?
      {
        cam_geom::boolean::Combined::Empty => cam_geom::region::Region::new(Vec::new())
          .map_err(|err| Failure::input(format!("ncc {:?}: {err}", toolpath.name)))?,
        cam_geom::boolean::Combined::Region { region, .. } => region,
      }
    };
    let fill = cam_ops::NccFill {
      contours: clipped_region.boundaries,
      flattened: raw_fill.flattened,
    };

    let moves = cam_ops::ncc_moves(&fill.contours, params.cut_depth.mm(), &document.board)
      .map_err(|err| Failure::input(format!("ncc {:?}: {err}", toolpath.name)))?;

    let contour_count = fill.contours.len();
    total += moves.len();
    runs.push(NccRun {
      name: toolpath.name.clone(),
      contours: contour_count,
      moves,
    });
  }

  if runs.is_empty() {
    return Err(Failure::input(
      "ncc found no NonCopperClear toolpaths in the document",
    ));
  }

  Ok((runs, total))
}

fn emit_program(
  stream: &MoveStream,
  post_path: &std::path::Path,
  out: Option<&std::path::Path>,
  document_path: &std::path::Path,
) -> Result<(), Failure> {
  let post_text = std::fs::read_to_string(post_path)
    .map_err(|error| Failure::input(format!("{}: {error}", post_path.display())))?;
  let post = parse_post(&post_text)
    .map_err(|error| Failure::post(format!("{}: {error}", post_path.display())))?;
  let meta = ProgramMeta {
    file_name: document_path
      .file_name()
      .map(|name| name.to_string_lossy().into_owned())
      .unwrap_or_else(|| document_path.display().to_string()),
    file_path: document_path.display().to_string(),
  };
  let emitted = emit(&post, stream, &meta)
    .map_err(|error| Failure::post(format!("{}: {error}", post_path.display())))?;

  match out {
    Some(destination) => {
      let unique = format!(
        "tmp.{}.{}",
        std::process::id(),
        std::time::SystemTime::now()
          .duration_since(std::time::UNIX_EPOCH)
          .map(|d| d.as_nanos())
          .unwrap_or(0)
      );
      let temporary = destination.with_extension(
        destination
          .extension()
          .and_then(|ext| ext.to_str())
          .map(|ext| format!("{ext}.{unique}"))
          .unwrap_or(unique),
      );
      std::fs::write(&temporary, emitted.text.as_bytes())
        .map_err(|error| Failure::input(format!("{}: {error}", temporary.display())))?;
      std::fs::rename(&temporary, destination).map_err(|error| {
        let _ = std::fs::remove_file(&temporary);
        Failure::input(format!("{}: {error}", destination.display()))
      })?;
      eprintln!(
        "wrote {} ({} line(s), {} block(s))",
        destination.display(),
        emitted.lines,
        emitted.blocks
      );
      Ok(())
    }
    None => {
      use std::io::Write;
      let mut stdout = std::io::stdout().lock();
      stdout
        .write_all(emitted.text.as_bytes())
        .and_then(|()| stdout.flush())
        .map_err(|error| Failure::input(format!("cannot write the program: {error}")))
    }
  }
}

/// Convert a stroked outline region into the filled board interior.
///
/// Gerber outline layers are often drawn as strokes (thin ring). For NCC we need the board
/// interior, not the stroke band. If the outline has inner holes, return the holes filled;
/// otherwise return the outline region unchanged.
fn outline_region_to_board(
  outline: &cam_geom::region::Region,
  tolerance: cam_geom::TolerancePolicy,
) -> Result<cam_geom::region::Region, cam_geom::GeomError> {
  if outline.boundaries.len() < 2 {
    return Ok(outline.clone());
  }
  // Outermost boundary = largest absolute area.
  let mut outer_idx = 0usize;
  let mut outer_area = 0.0f64;
  for (i, b) in outline.boundaries.iter().enumerate() {
    let a = b.signed_area().abs();
    if a > outer_area {
      outer_area = a;
      outer_idx = i;
    }
  }
  let outer_filled = cam_geom::region::Region::new(vec![outline.boundaries[outer_idx].clone()])?
    .normalize(tolerance)?
    .region;
  let stroke_area: f64 = outline
    .boundaries
    .iter()
    .map(|b| b.signed_area().abs())
    .sum::<f64>();
  // If stroke material is much smaller than the outer filled area, it's a stroked outline.
  let filled_area = outline.boundaries[outer_idx].signed_area().abs();
  if stroke_area >= filled_area * 0.8 {
    return Ok(outline.clone());
  }
  match cam_geom::boolean::combine(
    &outer_filled,
    outline,
    cam_geom::boolean::BooleanOp::Difference,
    tolerance,
  )? {
    cam_geom::boolean::Combined::Empty => Ok(outline.clone()),
    cam_geom::boolean::Combined::Region { region, .. } => Ok(region),
  }
}
