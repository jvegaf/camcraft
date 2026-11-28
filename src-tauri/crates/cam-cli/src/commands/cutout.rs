//! Board cutout, and the complete board program: the drills, then the cutout, in one stream.
//!
//! `cutout` answers what the document's `Cutout` toolpaths ask for; `program` is E2.5 — one program
//! per board, with the drilling first and the cutout last, because the cutout is what frees the
//! board from the stock. Neither command re-implements a strategy: the cutout walk is
//! [`cam_ops::cutout`] and the drilling plan is [`cam_ops::plan_excellon_drill`], so a program this
//! command writes and the one `drill` writes cut the same holes in the same order.
//!
//! Both read geometry from the document and machining parameters from their flags, and both emit
//! through `cam-post`, which is the only place a dialect lives.

use std::io::Write;
use std::path::{Path, PathBuf};

use cam_excellon::parse as parse_excellon;
use cam_geom::Point2;
use cam_model::{Document, Operation};
use cam_move::{Move, MoveStream, Segment, ToolShape};
use cam_ops::excellon_drill::{
  plan_excellon_drill, ExcellonToolPlan, ExcellonToolSetup, SlotMode, ToolResolver,
};
use cam_ops::{cutout, CutoutRequest, DrillParams, RetractMode, ToolSpec, ToolpathError};
use cam_post::{emit, parse_post, ProgramMeta};
use cam_units::Length;

use crate::{number, open, option_value, write_report, Failure};

/// The flags both commands share, plus the drilling ones `program` adds.
struct Flags {
  document: Option<PathBuf>,
  post: Option<PathBuf>,
  out: Option<PathBuf>,
  drill: Option<PathBuf>,
  tool_number: u32,
  diameter: Option<f64>,
  feed: f64,
  plunge: f64,
  spindle: u32,
  drill_cut_depth: Option<f64>,
  peck_depth: f64,
  start_depth: f64,
  retract: RetractMode,
  slot_mode: SlotMode,
}

/// Read the flags. `program` accepts the drilling flags; `cutout` refuses them by name rather than
/// silently ignoring a flag the run then does not honour.
fn parse_flags(rest: &[String], program: bool) -> Result<Flags, Failure> {
  let command = if program { "program" } else { "cutout" };
  let mut flags = Flags {
    document: None,
    post: None,
    out: None,
    drill: None,
    tool_number: 1,
    diameter: None,
    feed: 100.0,
    plunge: 30.0,
    spindle: 18_000,
    drill_cut_depth: None,
    peck_depth: 0.0,
    start_depth: 0.0,
    retract: RetractMode::Full,
    slot_mode: SlotMode::Drill,
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
      "--drill" if program => {
        flags.drill = Some(PathBuf::from(option_value(rest, index, "--drill")?));
        index += 2;
      }
      "--cut-depth" if program => {
        flags.drill_cut_depth = Some(number(rest, index, "--cut-depth")?);
        index += 2;
      }
      "--peck-depth" if program => {
        flags.peck_depth = number(rest, index, "--peck-depth")?;
        index += 2;
      }
      "--start-depth" if program => {
        flags.start_depth = number(rest, index, "--start-depth")?;
        index += 2;
      }
      "--retract" if program => {
        let value = option_value(rest, index, "--retract")?;
        flags.retract = if value == "full" {
          RetractMode::Full
        } else {
          let amount = value.parse::<f64>().map_err(|_| {
            Failure::input(format!(
              "--retract expects full or an amount in mm, got {value:?}"
            ))
          })?;
          RetractMode::Amount {
            amount: Length::from_mm(amount),
          }
        };
        index += 2;
      }
      "--slot-mode" if program => {
        let value = option_value(rest, index, "--slot-mode")?;
        flags.slot_mode = match value {
          "drill" => SlotMode::Drill,
          "trajectory" => SlotMode::Trajectory,
          other => {
            return Err(Failure::input(format!(
              "unknown --slot-mode {other:?}; expected drill or trajectory"
            )))
          }
        };
        index += 2;
      }
      other if other.starts_with('-') => {
        return Err(Failure::input(format!(
          "unknown {command} option {other:?}"
        )))
      }
      other => {
        if flags.document.is_some() {
          return Err(Failure::input(format!(
            "{command} takes exactly one document"
          )));
        }
        flags.document = Some(PathBuf::from(other));
        index += 1;
      }
    }
  }

  Ok(flags)
}

/// One cutout toolpath's result, kept named so the report can say which one it is.
struct CutoutRun {
  name: String,
  contours: usize,
  moves: Vec<Move>,
  mouse_bite_moves: Vec<Move>,
}

/// `cam-cli cutout <document>`: the document's own cutout, reported or emitted.
pub fn run(rest: &[String]) -> Result<(), Failure> {
  let flags = parse_flags(rest, false)?;
  let path = flags
    .document
    .clone()
    .ok_or_else(|| Failure::input("cutout requires a document path"))?;
  let document = open(&path)?;
  let (diameter, runs) = cutout_runs(&document, &flags)?;

  let Some(post_path) = flags.post.clone() else {
    // `--out` names the file the program is written to, so without `--post` there is no program and
    // nothing to write: accepting the flag and creating no file is a run that reports success and
    // leaves the caller with a stale program from some earlier attempt.
    if flags.out.is_some() {
      return Err(Failure::input(
        "cutout --out writes the program, so it needs --post: without a post-processor the run \
         prints a report and writes no file",
      ));
    }
    let mut report = String::new();
    for run in &runs {
      let bite_moves = run.mouse_bite_moves.len();
      if bite_moves > 0 {
        report.push_str(&format!(
          "cutout {}: {} contour(s), {} move(s), {} mouse-bite drill move(s)\n",
          run.name,
          run.contours,
          run.moves.len(),
          bite_moves
        ));
      } else {
        report.push_str(&format!(
          "cutout {}: {} contour(s), {} move(s)\n",
          run.name,
          run.contours,
          run.moves.len()
        ));
      }
    }
    let moves: usize = runs
      .iter()
      .map(|run| run.moves.len() + run.mouse_bite_moves.len())
      .sum();
    report.push_str(&format!(
      "cutout: {} toolpath(s), {} move(s)\n",
      runs.len(),
      moves
    ));
    return write_report(&mut std::io::stdout(), &report);
  };

  let mut stream = MoveStream::new(
    Length::from_mm(cam_model::DEFAULT_SAFE_Z_MM),
    Point2::new(0.0, 0.0),
  );
  for run in &runs {
    if !run.mouse_bite_moves.is_empty() {
      stream.segments.push(mouse_bite_segment(run, &flags));
    }
    stream.segments.push(cutout_segment(run, &flags, diameter));
  }
  let stream = cam_ops::route_stream_around_exc_areas(
    &stream,
    &document.exc_areas,
    cam_geom::TolerancePolicy::default(),
  )
  .map_err(|e| Failure::input(format!("{e}")))?;
  emit_program(&stream, &post_path, flags.out.as_deref(), &path)
}

/// `cam-cli program <document> --drill <file.drl> --post <file.pp>`: E2.5's complete board program.
///
/// The drills come first and the cutout last: drilling a board that the cutout has already freed is
/// drilling a part the machine no longer holds. The two halves share one `Job`, so the holes and the
/// outline land on the same board.
pub fn program(rest: &[String]) -> Result<(), Failure> {
  let flags = parse_flags(rest, true)?;
  let path = flags
    .document
    .clone()
    .ok_or_else(|| Failure::input("program requires a document path"))?;
  let drill_path = flags
    .drill
    .clone()
    .ok_or_else(|| Failure::input("program requires --drill <file.drl>"))?;
  let post_path = flags
    .post
    .clone()
    .ok_or_else(|| Failure::input("program requires --post <file.pp>"))?;
  let cut_depth = flags
    .drill_cut_depth
    .ok_or_else(|| Failure::input("program requires --cut-depth <mm>, the drilling depth"))?;

  let document = open(&path)?;
  let job = document.board.clone();

  let text = std::fs::read_to_string(&drill_path)
    .map_err(|error| Failure::input(format!("{}: {error}", drill_path.display())))?;
  let excellon = parse_excellon(&text)
    .map_err(|error| Failure::input(format!("{}: {error}", drill_path.display())))?;
  let resolver = TableResolver {
    peck_depth: flags.peck_depth,
    start_depth: flags.start_depth,
    cut_depth,
    retract: flags.retract,
    feed: flags.feed,
    plunge: flags.plunge,
    spindle: flags.spindle,
  };
  let plan = plan_excellon_drill(&excellon, &resolver, &job, flags.slot_mode)
    .map_err(|error| Failure::input(format!("{}: {error}", drill_path.display())))?;

  // The cutout is a different tool from every drill, and the Excellon table already claims its
  // numbers. A program whose cutout reuses a drill's number emits no tool change there, so the
  // outline is cut with the drill bit while the run reports success. The mouse-bite perforation
  // tool (cutout number + 100) needs the same guarantee: two segments sharing one number with
  // different geometry make a tool-change-keyed controller skip a change and cut with the wrong
  // physical tool.
  let mouse_bite_tool_number = flags.tool_number.saturating_add(100);
  let colliding_drill_tool = plan.groups.iter().find_map(|group| {
    if group.excellon_tool == flags.tool_number {
      Some((group.excellon_tool, "cutout"))
    } else if group.excellon_tool == mouse_bite_tool_number {
      Some((group.excellon_tool, "mouse-bite"))
    } else {
      None
    }
  });
  if let Some((drill_tool, role)) = colliding_drill_tool {
    return Err(Failure::input(format!(
      "--tool {} collides with drill tool T{} in this Excellon file: the {} pass needs a tool \
       number the file does not use, because a program that cuts or drills with the wrong bit \
       reports success and ruins the board",
      flags.tool_number, drill_tool, role
    )));
  }

  let (diameter, runs) = cutout_runs(&document, &flags)?;

  let mut stream = MoveStream::new(
    Length::from_mm(cam_model::DEFAULT_SAFE_Z_MM),
    Point2::new(0.0, 0.0),
  );
  for group in &plan.groups {
    if let Some(segment) = drill_segment(group, &flags) {
      stream.segments.push(segment);
    }
  }
  // Everything from here on is the cutout: the two loops split the stream by position, and routing
  // rewrites each segment's moves in place without adding, dropping or reordering one, so the index
  // still marks the boundary after it runs.
  let drill_segments = stream.segments.len();
  for run in &runs {
    if !run.mouse_bite_moves.is_empty() {
      stream.segments.push(mouse_bite_segment(run, &flags));
    }
    stream.segments.push(cutout_segment(run, &flags, diameter));
  }
  let stream = cam_ops::route_stream_around_exc_areas(
    &stream,
    &document.exc_areas,
    cam_geom::TolerancePolicy::default(),
  )
  .map_err(|e| Failure::input(format!("{e}")))?;

  // Counted on the routed stream, because the router inserts its detour moves *inside* the segment
  // that travels: counting the cutout's moves before routing charged every detour it added to the
  // drilling half by subtraction, so a board whose cutout stepped around a keep-out reported drilled
  // moves no drill made (R3-002, R4-003).
  let moves_in = |segments: &[Segment]| -> usize {
    segments
      .iter()
      .map(|segment| segment.moves.len())
      .sum()
  };
  let drill_moves = moves_in(&stream.segments[..drill_segments]);
  let cutout_moves = moves_in(&stream.segments[drill_segments..]);
  let bite_hits: usize = runs
    .iter()
    .map(|run| {
      run
        .mouse_bite_moves
        .iter()
        .filter(|m| matches!(m, Move::Feed { .. }))
        .count()
    })
    .sum();
  let mut report = String::new();
  report.push_str(&format!(
    "program: {} drill hit(s) in {} tool(s), {} tool change(s)\n",
    plan.total_hits,
    plan.groups.len(),
    plan.tool_changes
  ));
  report.push_str(&format!(
    "program: {} cutout toolpath(s), {} move(s)\n  {} drill move(s), {} cutout move(s)\n",
    runs.len(),
    stream.move_count(),
    drill_moves,
    cutout_moves
  ));
  if bite_hits > 0 {
    report.push_str(&format!("  {} mouse-bite perforation(s)\n", bite_hits));
  }
  write_report(&mut std::io::stderr(), &report)?;

  emit_program(&stream, &post_path, flags.out.as_deref(), &path)
}

/// The document's cutout toolpaths, computed, with the diameter they were cut by.
///
/// The diameter comes back with the runs so the segment built for them cannot fall back to a
/// different one: the tool that planned the offset and the tool the program states are the same
/// number. A document with no cutout is refused by name rather than emitting an empty program that
/// looks like a run with nothing to do.
fn cutout_runs(document: &Document, flags: &Flags) -> Result<(f64, Vec<CutoutRun>), Failure> {
  let diameter = flags.diameter.ok_or_else(|| {
    Failure::input("cutout requires --diameter <mm>, the diameter of the tool that cuts it")
  })?;
  if diameter <= 0.0 || !diameter.is_finite() {
    return Err(Failure::input("--diameter must be a positive length in mm"));
  }
  let tool = ToolSpec {
    number: flags.tool_number,
    diameter: Length::from_mm(diameter),
    included_angle: None,
    feed_rate: flags.feed,
    plunge_rate: flags.plunge,
    spindle_speed: flags.spindle,
  };

  let mut runs = Vec::new();
  for toolpath in &document.toolpaths {
    let Operation::Cutout(params) = &toolpath.operation else {
      continue;
    };
    let mut contours = Vec::new();
    for id in &toolpath.entity_ids {
      let contour = document
        .entity(id)
        .and_then(|entity| entity.kind.as_contour())
        .ok_or_else(|| {
          Failure::input(format!(
            "cutout toolpath {:?} cites entity {id:?}, which is not a contour",
            toolpath.id
          ))
        })?;
      contours.push(contour.clone());
    }
    if contours.is_empty() {
      return Err(Failure::input(format!(
        "cutout toolpath {:?} cites no geometry to cut",
        toolpath.id
      )));
    }
    let request = CutoutRequest {
      contours: &contours,
      params,
      tool: &tool,
      job: &document.board,
    };
    let moves = cutout(&request)
      .map_err(|error| Failure::input(format!("cutout toolpath {:?}: {error}", toolpath.id)))?;
    let mouse_bite_moves = cam_ops::mouse_bite_drills(&request)
      .map_err(|error| Failure::input(format!("cutout toolpath {:?}: {error}", toolpath.id)))?;
    runs.push(CutoutRun {
      name: toolpath.name.clone(),
      contours: contours.len(),
      moves,
      mouse_bite_moves,
    });
  }

  if runs.is_empty() {
    return Err(Failure::input(
      "the document holds no cutout toolpath, so there is nothing to cut out",
    ));
  }
  Ok((diameter, runs))
}

/// The emitter's segment for one cutout toolpath, cut by a `diameter` tool.
fn cutout_segment(run: &CutoutRun, flags: &Flags, diameter: f64) -> Segment {
  Segment {
    name: run.name.clone(),
    tool_number: flags.tool_number,
    tool: ToolShape::cylinder(Length::from_mm(diameter)),
    feed_rate: flags.feed,
    plunge_rate: flags.plunge,
    spindle_speed: flags.spindle,
    moves: run.moves.clone(),
  }
}

/// The emitter's segment for mouse-bite perforations drilled along a cutout's tabs.
fn mouse_bite_segment(run: &CutoutRun, flags: &Flags) -> Segment {
  Segment {
    name: format!("{} mouse-bites", run.name),
    tool_number: flags.tool_number.saturating_add(100),
    tool: ToolShape::cylinder(Length::from_mm(0.8)),
    feed_rate: flags.feed,
    plunge_rate: flags.plunge,
    spindle_speed: flags.spindle,
    moves: run.mouse_bite_moves.clone(),
  }
}

/// The emitter's segment for one drilled tool, or `None` when the tool cut nothing.
///
/// A tool that contributed no move is dropped rather than emitted empty: a segment with no moves
/// still produces a tool-change block, and a change to a tool that cuts nothing is a machine stop
/// nobody asked for.
fn drill_segment(group: &ExcellonToolPlan, flags: &Flags) -> Option<Segment> {
  if group.moves.is_empty() {
    return None;
  }
  Some(Segment {
    name: format!("Drill T{}", group.excellon_tool),
    tool_number: group.excellon_tool,
    tool: ToolShape::cylinder(group.diameter.unwrap_or_else(|| Length::from_mm(1.0))),
    feed_rate: flags.feed,
    plunge_rate: flags.plunge,
    spindle_speed: flags.spindle,
    moves: group.moves.clone(),
  })
}

/// Drill parameters and tool data for one Excellon tool, taken from the table's own diameters.
struct TableResolver {
  peck_depth: f64,
  start_depth: f64,
  cut_depth: f64,
  retract: RetractMode,
  feed: f64,
  plunge: f64,
  spindle: u32,
}

impl ToolResolver for TableResolver {
  fn resolve(&self, tool: &cam_excellon::Tool) -> Result<ExcellonToolSetup, ToolpathError> {
    Ok(ExcellonToolSetup {
      excellon_tool: tool.number,
      tool: ToolSpec {
        number: tool.number,
        diameter: tool.diameter,
        included_angle: None,
        feed_rate: self.feed,
        plunge_rate: self.plunge,
        spindle_speed: self.spindle,
      },
      params: DrillParams {
        peck_depth: Length::from_mm(self.peck_depth),
        retract: self.retract,
        start_depth: Length::from_mm(self.start_depth),
        cut_depth: Length::from_mm(self.cut_depth),
        bank: None,
      },
    })
  }
}

/// Emit `stream` through the post-processor at `post_path`, to `out` or to stdout.
///
/// The file write is whole or not at all, the way `export` writes: a truncated `.nc` beside a live
/// job reads as a program that runs, and it would run short.
///
/// Reading the post is `cam_post::parse_post` and emitting is `cam_post::emit`, so nothing about a
/// dialect lives here. `cutout` and `program` call it, and the library exports it as
/// [`crate::emit_program`] so a window can emit the same program without an emitter of its own; its
/// refusals keep the crate's [`Failure`] codes, so a caller needs no error mapping.
///
/// It is not yet the crate's only emitter: `isolate` and `ncc` each still carry a private
/// `emit_program` copy, and `drill` reads its post and calls `cam_post::emit` inline. Those copies
/// also name their temporary file differently from this one, so folding them in is deferred work of
/// its own, not something this function has already done.
pub fn emit_program(
  stream: &MoveStream,
  post_path: &Path,
  out: Option<&Path>,
  document_path: &Path,
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
      let mut temporary_name = destination.as_os_str().to_owned();
      temporary_name.push(".tmp");
      let temporary = PathBuf::from(temporary_name);
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
      // `print!` panics on a closed pipe, which would turn `cutout … | head` into exit 101.
      let mut stdout = std::io::stdout().lock();
      stdout
        .write_all(emitted.text.as_bytes())
        .and_then(|()| stdout.flush())
        .map_err(|error| Failure::input(format!("cannot write the program: {error}")))
    }
  }
}
