use std::io::Write;
use std::path::PathBuf;

use cam_excellon::parse;
use cam_model::Job;
use cam_move::{MoveStream, Segment, ToolShape};
use cam_ops::excellon_drill::{plan_excellon_drill, ExcellonToolSetup, SlotMode, ToolResolver};
use cam_ops::{DrillParams, RetractMode, ToolSpec, ToolpathError};
use cam_post::{emit, parse_post, ProgramMeta};
use cam_units::Length;

use crate::commands::export::write_atomically;
use crate::{open, option_value, write_report, Failure};

struct DiameterResolver {
  peck_depth: f64,
  retract: RetractMode,
  start_depth: f64,
  cut_depth: f64,
}

impl ToolResolver for DiameterResolver {
  fn resolve(&self, tool: &cam_excellon::Tool) -> Result<ExcellonToolSetup, ToolpathError> {
    Ok(ExcellonToolSetup {
      excellon_tool: tool.number,
      tool: ToolSpec {
        number: tool.number,
        diameter: tool.diameter,
        included_angle: None,
        feed_rate: 100.0,
        plunge_rate: 30.0,
        spindle_speed: 18_000,
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

pub fn run(rest: &[String]) -> Result<(), Failure> {
  let mut input: Option<PathBuf> = None;
  let mut post_path: Option<PathBuf> = None;
  let mut out: Option<PathBuf> = None;
  let mut job_path: Option<PathBuf> = None;
  let mut cut_depth: Option<f64> = None;
  let mut peck_depth: f64 = 0.0;
  let mut start_depth: f64 = 0.0;
  let mut retract = RetractMode::Full;
  let mut slot_mode = SlotMode::Drill;

  let mut index = 0;
  while index < rest.len() {
    match rest[index].as_str() {
      "--post" => {
        post_path = Some(PathBuf::from(option_value(rest, index, "--post")?));
        index += 2;
      }
      "--out" => {
        out = Some(PathBuf::from(option_value(rest, index, "--out")?));
        index += 2;
      }
      "--job" => {
        job_path = Some(PathBuf::from(option_value(rest, index, "--job")?));
        index += 2;
      }
      "--cut-depth" => {
        cut_depth = Some(parse_number(
          option_value(rest, index, "--cut-depth")?,
          "--cut-depth",
        )?);
        index += 2;
      }
      "--peck-depth" => {
        peck_depth = parse_number(option_value(rest, index, "--peck-depth")?, "--peck-depth")?;
        index += 2;
      }
      "--start-depth" => {
        start_depth = parse_number(option_value(rest, index, "--start-depth")?, "--start-depth")?;
        index += 2;
      }
      "--retract" => {
        let value = option_value(rest, index, "--retract")?;
        retract = if value == "full" {
          RetractMode::Full
        } else {
          RetractMode::Amount {
            amount: Length::from_mm(parse_number(value, "--retract")?),
          }
        };
        index += 2;
      }
      "--slot-mode" => {
        let value = option_value(rest, index, "--slot-mode")?;
        slot_mode = match value {
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
        return Err(Failure::input(format!("unknown drill option {other:?}")))
      }
      other => {
        if input.is_some() {
          return Err(Failure::input("drill takes exactly one Excellon file"));
        }
        input = Some(PathBuf::from(other));
        index += 1;
      }
    }
  }

  let input = input.ok_or_else(|| Failure::input("drill requires an Excellon file path"))?;
  let post_path = post_path.ok_or_else(|| Failure::input("drill requires --post <file.pp>"))?;
  let cut_depth = cut_depth.ok_or_else(|| Failure::input("drill requires --cut-depth <mm>"))?;

  let excellon_text = std::fs::read_to_string(&input)
    .map_err(|error| Failure::input(format!("{}: {error}", input.display())))?;
  let excellon = parse(&excellon_text)
    .map_err(|error| Failure::input(format!("{}: {error}", input.display())))?;

  let job = match job_path {
    Some(path) => {
      let document = open(&path)?;
      document.board
    }
    None => Job::default_sheet(),
  };

  let post_text = std::fs::read_to_string(&post_path)
    .map_err(|error| Failure::input(format!("{}: {error}", post_path.display())))?;
  let post = parse_post(&post_text)
    .map_err(|error| Failure::post(format!("{}: {error}", post_path.display())))?;

  let resolver = DiameterResolver {
    peck_depth,
    retract,
    start_depth,
    cut_depth,
  };
  let plan = plan_excellon_drill(&excellon, &resolver, &job, slot_mode)
    .map_err(|error| Failure::input(format!("{}: {error}", input.display())))?;

  let stream = build_stream(&plan, &job);
  let meta = ProgramMeta {
    file_name: input
      .file_name()
      .map(|name| name.to_string_lossy().into_owned())
      .unwrap_or_else(|| input.display().to_string()),
    file_path: input.display().to_string(),
  };
  let emitted = emit(&post, &stream, &meta)
    .map_err(|error| Failure::post(format!("{}: {error}", post_path.display())))?;

  let report = format_report(&plan);
  write_report(&mut std::io::stderr(), &report)?;

  match out {
    Some(path) => {
      // Whole or not at all: a truncated `.nc` runs short, and it runs short with no footer, no `M5`
      // and no `M30` — the one failure a controller cannot tell from a finished program
      // (R1-drill-non-atomic-write, R3-002, R4-drill-non-atomic-output-write).
      write_atomically(&path, emitted.text.as_bytes())?;
      eprintln!("wrote {} ({} bytes)", path.display(), emitted.text.len());
    }
    None => {
      let mut stdout = std::io::stdout().lock();
      stdout
        .write_all(emitted.text.as_bytes())
        .and_then(|()| stdout.flush())
        .map_err(|error| Failure::input(format!("cannot write program to stdout: {error}")))?;
    }
  }

  Ok(())
}

fn parse_number(value: &str, flag: &str) -> Result<f64, Failure> {
  let number = value
    .parse::<f64>()
    .map_err(|_| Failure::input(format!("{flag} expects a number, got {value:?}")))?;
  // `NaN` and `±inf` parse, and every depth guard downstream compares numbers: a comparison against
  // `NaN` is false, so the guard lets it through and the program carries `ZNaN` into the material.
  // The value is refused here, where the flag that carried it is still known by name (R3-003).
  if !number.is_finite() {
    return Err(Failure::input(format!(
      "{flag} expects a finite number, got {value:?}"
    )));
  }
  Ok(number)
}

fn build_stream(plan: &cam_ops::excellon_drill::ExcellonDrillPlan, _job: &Job) -> MoveStream {
  let mut stream = MoveStream::new(
    Length::from_mm(cam_model::DEFAULT_SAFE_Z_MM),
    cam_geom::Point2::new(0.0, 0.0),
  );
  for group in &plan.groups {
    if group.moves.is_empty() {
      continue;
    }
    let diameter = group.diameter.unwrap_or_else(|| Length::from_mm(1.0));
    stream.segments.push(Segment {
      name: format!("Drill T{}", group.excellon_tool),
      tool_number: group.excellon_tool,
      tool: ToolShape::cylinder(diameter),
      feed_rate: 100.0,
      plunge_rate: 30.0,
      spindle_speed: 18_000,
      moves: group.moves.clone(),
    });
  }
  stream
}

fn format_report(plan: &cam_ops::excellon_drill::ExcellonDrillPlan) -> String {
  let mut out = String::new();
  out.push_str(&format!(
    "drill: {} hit(s), {} tool(s), {} tool change(s)\n",
    plan.total_hits,
    plan.groups.len(),
    plan.tool_changes
  ));
  for group in &plan.groups {
    let diameter = group
      .diameter
      .map(|d| format!("{:.4} mm", d.mm()))
      .unwrap_or_else(|| "?".to_owned());
    out.push_str(&format!(
      "  T{} ({}): {} hit(s) ({} hole(s), {} slot(s)), {} move(s)\n",
      group.excellon_tool,
      diameter,
      group.hits,
      group.holes,
      group.slots,
      group.moves.len()
    ));
  }
  out
}
