//! Isolation routing: generate isolation G-code from a document's Isolation toolpaths.
//!
//! Finds every `Operation::Isolation` toolpath in the document, unites the copper contours of its
//! referenced entities, computes isolation passes (with `iso_type` and `keep_pours` selection), runs
//! the anti-gouge guard, and emits G-code through the post-processor.

use std::path::PathBuf;

use cam_geom::Point2;
use cam_model::{Document, Operation};
use cam_move::{MoveStream, Segment, ToolShape};
use cam_ops::vbit_isolation_depth_mm;
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
  vbit_angle: Option<f64>,
  vbit_tip: Option<f64>,
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
    vbit_angle: None,
    vbit_tip: None,
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
      "--vbit-angle" => {
        flags.vbit_angle = Some(number(rest, index, "--vbit-angle")?);
        index += 2;
      }
      "--vbit-tip" => {
        flags.vbit_tip = Some(number(rest, index, "--vbit-tip")?);
        index += 2;
      }
      other if other.starts_with('-') => {
        return Err(Failure::input(format!("unknown isolate option {other:?}")));
      }
      other => {
        if flags.document.is_some() {
          return Err(Failure::input("isolate takes exactly one document"));
        }
        flags.document = Some(PathBuf::from(other));
        index += 1;
      }
    }
  }

  Ok(flags)
}

/// `cam-cli isolate <document>`: isolation for the document's Isolation toolpaths.
pub fn run(rest: &[String]) -> Result<(), Failure> {
  let flags = parse_flags(rest)?;
  let path = flags
    .document
    .clone()
    .ok_or_else(|| Failure::input("isolate requires a document path"))?;
  let document = open(&path)?;
  let diameter = flags
    .diameter
    .ok_or_else(|| Failure::input("isolate requires --diameter <mm>"))?;
  if diameter <= 0.0 {
    return Err(Failure::input("--diameter must be greater than zero"));
  }

  let (runs, total_moves) = isolation_runs(&document, &flags, diameter)?;

  let Some(post_path) = flags.post.clone() else {
    if flags.out.is_some() {
      return Err(Failure::input(
        "isolate --out writes the program, so it needs --post: without a post-processor the run \
         prints a report and writes no file",
      ));
    }
    let mut report = String::new();
    for run in &runs {
      report.push_str(&format!(
        "isolate {}: {} contour(s), {} move(s)\n",
        run.name,
        run.contours,
        run.moves.len()
      ));
    }
    report.push_str(&format!(
      "isolate: {} toolpath(s), {} move(s)\n",
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

struct IsolationRun {
  name: String,
  contours: usize,
  moves: Vec<cam_move::Move>,
}

fn isolation_runs(
  document: &Document,
  flags: &Flags,
  diameter: f64,
) -> Result<(Vec<IsolationRun>, usize), Failure> {
  let tolerance = cam_geom::TolerancePolicy::uniform(1.0e-6);
  let mut runs = Vec::new();
  let mut total = 0usize;

  for toolpath in &document.toolpaths {
    let Operation::Isolation(params) = &toolpath.operation else {
      continue;
    };

    // Collect copper contours from referenced entities.
    let mut copper_contours = Vec::new();
    for entity_id in &toolpath.entity_ids {
      let Some(entity) = document.entity(entity_id.as_str()) else {
        return Err(Failure::input(format!(
          "isolation toolpath {:?} references unknown entity {:?}",
          toolpath.name, entity_id
        )));
      };
      let Some(contour) = entity.kind.as_contour() else {
        return Err(Failure::input(format!(
          "isolation toolpath {:?} entity {:?} is not a contour",
          toolpath.name, entity_id
        )));
      };
      copper_contours.push(contour.clone());
    }

    let copper = cam_ops::unite_copper(&copper_contours, tolerance)
      .map_err(|err| Failure::input(format!("isolation {:?}: {err}", toolpath.name)))?;

    // V-bit depth wiring (E3.3/E3.6): derive cut depth from V-bit geometry when provided.
    let cut_depth = match (flags.vbit_angle, flags.vbit_tip) {
      (Some(angle), Some(tip)) => vbit_isolation_depth_mm(diameter, angle, tip)
        .map_err(|err| Failure::input(format!("isolation {:?}: {err}", toolpath.name)))?,
      (Some(_), None) | (None, Some(_)) => {
        return Err(Failure::input(
          "V-bit depth derivation needs both --vbit-angle <deg> and --vbit-tip <mm>",
        ));
      }
      (None, None) => params.cut_depth.mm(),
    };

    // Anti-gouge guard runs inside isolation_moves against protected (united) copper.
    let moves = cam_ops::isolation_moves(
      &copper,
      diameter,
      params.passes,
      params.overlap,
      params.direction,
      params.kind,
      params.keep_pours,
      cut_depth,
      tolerance,
      &document.board,
      &copper,
    )
    .map_err(|err| Failure::input(format!("isolation {:?}: {err}", toolpath.name)))?;

    let contour_count = moves
      .iter()
      .filter(|m| matches!(m, cam_move::Move::Rapid { .. }))
      .count();
    total += moves.len();
    runs.push(IsolationRun {
      name: toolpath.name.clone(),
      contours: contour_count,
      moves,
    });
  }

  if runs.is_empty() {
    return Err(Failure::input(
      "isolate found no Isolation toolpaths in the document",
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
