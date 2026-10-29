//! The data-driven G-code emitter.

use std::collections::BTreeMap;

use cam_move::{Move, MoveStream, Point3};
use cam_units::{Format, MM_PER_INCH};

use crate::{BlockKind, EmissionMode, PostProcessor, PostUnits, VarSpec};

/// Metadata a post-processor may reference in its text variables.
///
/// Nothing here is generated automatically: a timestamp in the output would break the guarantee
/// that identical input produces identical G-code (A9).
#[derive(Clone, Debug, Default)]
pub struct ProgramMeta {
  /// Name of the document being emitted.
  pub file_name: String,
  /// Full path of the document, when the caller wants it in the output.
  pub file_path: String,
}

/// What emitting produced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Emitted {
  /// The program text, line endings included.
  pub text: String,
  /// Number of output lines written.
  pub lines: usize,
  /// Number of blocks emitted.
  pub blocks: usize,
}

/// Why a program could not be emitted.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum EmitError {
  /// A block references a variable this build does not know.
  #[error("block {block:?} references unknown variable [{name}]")]
  UnknownVariable {
    /// The block being emitted.
    block: BlockKind,
    /// The variable name.
    name: String,
  },
  /// The program has arcs but the post-processor cannot express them.
  #[error(
    "post-processor {post_name:?} defines no arc block, so it cannot express a circular move; \
     refusing to emit an approximation"
  )]
  ArcsUnsupported {
    /// The post-processor's display name.
    post_name: String,
  },
  /// A block line has an unterminated variable reference.
  #[error("block {block:?} has an unterminated variable reference in {line:?}")]
  UnterminatedReference {
    /// The block being emitted.
    block: BlockKind,
    /// The offending template line.
    line: String,
  },
}

/// Emit a program.
pub fn emit(
  post: &PostProcessor,
  stream: &MoveStream,
  meta: &ProgramMeta,
) -> Result<Emitted, EmitError> {
  let mut writer = Writer::new(post, stream, meta);
  writer.run()?;
  Ok(writer.finish())
}

struct Writer<'a> {
  post: &'a PostProcessor,
  stream: &'a MoveStream,
  meta: &'a ProgramMeta,
  out: String,
  lines: usize,
  blocks: usize,
  line_number: i64,
  last_emitted: BTreeMap<String, String>,
  previous: Point3,
  /// Whether the line currently being expanded referenced the line-number field.
  line_number_referenced: bool,
  /// Formatting rules indexed by the field a block references.
  ///
  /// A block writes `[X]`, not `[X_POSITION]`: placeholders name the source field and the `VAR` name
  /// is the human label for that rule. Indexing by source is what makes a real post-processor work.
  by_source: BTreeMap<String, VarSpec>,
}

/// What the current block is being emitted for.
#[derive(Clone, Copy, Debug)]
struct Site {
  segment: Option<usize>,
  from: Point3,
  to: Point3,
  arc_center: Option<cam_geom::Point2>,
  arc_radius: Option<f64>,
}

impl<'a> Writer<'a> {
  fn new(post: &'a PostProcessor, stream: &'a MoveStream, meta: &'a ProgramMeta) -> Self {
    let home = Point3 {
      x: cam_units::Length::from_mm(stream.home.x),
      y: cam_units::Length::from_mm(stream.home.y),
      z: stream.safe_z,
    };
    Self {
      post,
      stream,
      meta,
      out: String::new(),
      lines: 0,
      blocks: 0,
      line_number: post.line_number_start.unwrap_or(0),
      last_emitted: BTreeMap::new(),
      previous: home,
      line_number_referenced: false,
      by_source: index_by_source(post),
    }
  }

  fn run(&mut self) -> Result<(), EmitError> {
    let home = self.previous;
    self.blocks += 1;
    self.emit_block(
      BlockKind::Header,
      &Site {
        segment: self.stream.segments.first().map(|_| 0),
        from: home,
        to: home,
        arc_center: None,
        arc_radius: None,
      },
    )?;

    let mut first_rapid_of_program = true;
    let mut previous_tool: Option<u32> = None;

    for (index, segment) in self.stream.segments.iter().enumerate() {
      if let Some(tool) = previous_tool {
        if tool != segment.tool_number {
          self.blocks += 1;
          let site = Site {
            segment: Some(index),
            from: self.previous,
            to: self.previous,
            arc_center: None,
            arc_radius: None,
          };
          self.emit_block(BlockKind::Toolchange, &site)?;
        }
      }
      if index > 0 {
        self.blocks += 1;
        let site = Site {
          segment: Some(index),
          from: self.previous,
          to: self.previous,
          arc_center: None,
          arc_radius: None,
        };
        self.emit_block(BlockKind::NewSegment, &site)?;
      }
      previous_tool = Some(segment.tool_number);

      let mut first_rapid_of_segment = true;
      let mut first_feed_of_segment = true;
      let mut first_cw_of_segment = true;
      let mut first_ccw_of_segment = true;

      for movement in &segment.moves {
        let from = self.previous;
        let to = movement.destination();
        let (center, radius) = match movement {
          Move::Arc { center, to, .. } => {
            let radius = from.xy().distance_to(*center);
            let _ = to;
            (Some(*center), Some(radius))
          }
          _ => (None, None),
        };

        let kind = match movement {
          Move::Retract { .. } => BlockKind::RetractMove,
          Move::Rapid { .. } => {
            if first_rapid_of_program {
              first_rapid_of_program = false;
              first_rapid_of_segment = false;
              BlockKind::InitialRapidMove
            } else if first_rapid_of_segment {
              first_rapid_of_segment = false;
              BlockKind::FirstRapidMove
            } else {
              BlockKind::RapidMove
            }
          }
          Move::Feed { .. } => {
            if is_plunge(from, to) {
              BlockKind::PlungeMove
            } else if first_feed_of_segment {
              first_feed_of_segment = false;
              BlockKind::FirstFeedMove
            } else {
              BlockKind::FeedMove
            }
          }
          Move::Arc { ccw, .. } => {
            if *ccw {
              if first_ccw_of_segment {
                first_ccw_of_segment = false;
                BlockKind::FirstCcwArcMove
              } else {
                BlockKind::CcwArcMove
              }
            } else if first_cw_of_segment {
              first_cw_of_segment = false;
              BlockKind::FirstCwArcMove
            } else {
              BlockKind::CwArcMove
            }
          }
        };

        let site = Site {
          segment: Some(index),
          from,
          to,
          arc_center: center,
          arc_radius: radius,
        };
        if matches!(movement, Move::Arc { .. }) && !self.supports_arcs() {
          return Err(EmitError::ArcsUnsupported {
            post_name: self.post.post_name.clone(),
          });
        }
        self.blocks += 1;
        self.emit_block(kind, &site)?;
        self.previous = to;
      }
    }

    self.blocks += 1;
    let site = Site {
      segment: self
        .stream
        .segments
        .last()
        .map(|_| self.stream.segments.len() - 1),
      from: self.previous,
      to: self.previous,
      arc_center: None,
      arc_radius: None,
    };
    self.emit_block(BlockKind::Footer, &site)?;
    Ok(())
  }

  /// True when the post-processor can express a circular move at all.
  fn supports_arcs(&self) -> bool {
    [
      BlockKind::CwArcMove,
      BlockKind::CcwArcMove,
      BlockKind::FirstCwArcMove,
      BlockKind::FirstCcwArcMove,
    ]
    .iter()
    .any(|kind| !self.post.block(*kind).is_empty())
  }

  fn emit_block(&mut self, kind: BlockKind, site: &Site) -> Result<(), EmitError> {
    let template: Vec<String> = self.post.block(kind).to_vec();
    for line in &template {
      self.line_number_referenced = false;
      let expanded = self.expand(line, kind, site)?;
      if expanded.trim().is_empty() {
        continue;
      }
      // Only a line that actually carries the block number consumes one: a comment line without
      // `[N]` must not advance the sequence.
      if self.line_number_referenced && self.post.line_number_increment.is_some() {
        self.advance_line_number();
      }
      self.out.push_str(&expanded);
      self.out.push_str(&self.line_ending());
      self.lines += 1;
    }
    Ok(())
  }

  fn advance_line_number(&mut self) {
    let increment = self.post.line_number_increment.unwrap_or(1);
    let maximum = self.post.line_number_maximum.unwrap_or(i64::MAX);
    if self.line_number > maximum - increment {
      self.line_number = self.post.line_number_start.unwrap_or(0);
    } else {
      self.line_number += increment;
    }
  }

  fn line_ending(&self) -> String {
    let mut result = String::new();
    let raw = self.post.line_ending.as_str();
    let bytes = raw.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
      if bytes[index] == b'[' {
        if let Some(close) = raw[index..].find(']') {
          let code = &raw[index + 1..index + close];
          if let Ok(value) = code.parse::<u8>() {
            result.push(value as char);
            index += close + 1;
            continue;
          }
        }
      }
      result.push(bytes[index] as char);
      index += 1;
    }
    if result.is_empty() {
      result.push('\n');
    }
    result
  }

  fn expand(&mut self, template: &str, kind: BlockKind, site: &Site) -> Result<String, EmitError> {
    let mut output = String::with_capacity(template.len() + 16);
    let mut rest = template;
    loop {
      let Some(open) = rest.find('[') else {
        output.push_str(rest);
        break;
      };
      output.push_str(&rest[..open]);
      let tail = &rest[open..];
      let Some(close) = tail.find(']') else {
        return Err(EmitError::UnterminatedReference {
          block: kind,
          line: template.to_owned(),
        });
      };
      let name = &tail[1..close];
      output.push_str(&self.resolve(name, kind, site)?);
      rest = &tail[close + 1..];
    }
    Ok(output)
  }

  /// Resolve one variable reference for one block.
  fn resolve(&mut self, name: &str, kind: BlockKind, site: &Site) -> Result<String, EmitError> {
    let key = name.trim().to_ascii_uppercase();
    if let Some(spec) = self.by_source.get(&key).cloned() {
      let value = self
        .numeric_source(&key, site)
        .ok_or_else(|| EmitError::UnknownVariable {
          block: kind,
          name: key.clone(),
        })?;
      if key == "N" {
        self.line_number_referenced = true;
      }
      let scaled = value * spec.scale;
      let text = format!(
        "{}{}",
        spec.prefix,
        Format::fixed(spec.decimals).apply(scaled)
      );
      return Ok(match spec.mode {
        EmissionMode::Always => text,
        EmissionMode::OnChange => {
          let previous = self.last_emitted.get(&key).cloned();
          if previous.as_deref() == Some(text.as_str()) {
            String::new()
          } else {
            self.last_emitted.insert(key, text.clone());
            text
          }
        }
      });
    }

    // Text variables are built in rather than declared, exactly as the dialect's own Lua host
    // exposes them (TOOLPATH_NAME, FILE_NAME, POST_NAME, UNITS, ...).
    if let Some(text) = self.text_source(name, site) {
      return Ok(text);
    }

    Err(EmitError::UnknownVariable {
      block: kind,
      name: name.to_owned(),
    })
  }

  /// Numeric value of a source field, already converted to the post-processor's units.
  fn numeric_source(&self, source: &str, site: &Site) -> Option<f64> {
    let upper = source.trim().to_ascii_uppercase();
    let to_output = |value: f64| -> f64 {
      match self.post.units {
        PostUnits::Millimetres => value,
        PostUnits::Inches => value / MM_PER_INCH,
      }
    };
    let segment = site
      .segment
      .and_then(|index| self.stream.segments.get(index));
    match upper.as_str() {
      "X" => Some(to_output(site.to.x.mm())),
      "Y" => Some(to_output(site.to.y.mm())),
      "Z" => Some(to_output(site.to.z.mm())),
      "XH" => Some(to_output(self.stream.home.x)),
      "YH" => Some(to_output(self.stream.home.y)),
      "ZH" => Some(to_output(self.stream.safe_z.mm())),
      "SAFEZ" => Some(to_output(self.stream.safe_z.mm())),
      "I" => site
        .arc_center
        .map(|center| to_output(center.x - site.from.x.mm())),
      "J" => site
        .arc_center
        .map(|center| to_output(center.y - site.from.y.mm())),
      "RADIUS" | "R" => site.arc_radius.map(to_output),
      "F" | "FC" => segment.map(|segment| segment.feed_rate),
      "FP" => segment.map(|segment| segment.plunge_rate),
      "S" => segment.map(|segment| f64::from(segment.spindle_speed)),
      "T" => segment.map(|segment| f64::from(segment.tool_number)),
      "N" => Some(self.line_number as f64),
      "WRAP_DIA" | "WRAPDIAMETER" => Some(0.0),
      _ => None,
    }
  }

  /// Text value of a built-in variable.
  fn text_source(&self, name: &str, site: &Site) -> Option<String> {
    let segment = site
      .segment
      .and_then(|index| self.stream.segments.get(index));
    match name.trim().to_ascii_uppercase().as_str() {
      "POST_NAME" => Some(self.post.post_name.clone()),
      "UNITS" => Some(match self.post.units {
        PostUnits::Millimetres => "MM".to_owned(),
        PostUnits::Inches => "INCHES".to_owned(),
      }),
      "TOOLPATH_NAME" | "PATHNAME" => segment.map(|segment| segment.name.clone()),
      "FILE_NAME" | "TP_FILENAME" => Some(self.meta.file_name.clone()),
      "FILE_PATHNAME" => Some(self.meta.file_path.clone()),
      "TOOLNAME" => segment.map(|segment| format!("T{}", segment.tool_number)),
      "DATE" | "TIME" => Some(String::new()),
      _ => None,
    }
  }

  fn finish(self) -> Emitted {
    Emitted {
      text: self.out,
      lines: self.lines,
      blocks: self.blocks,
    }
  }
}

/// Index the post-processor's `VAR` declarations by the field their blocks reference.
///
/// If a post declares the same source twice, the first declaration in name order wins: deterministic,
/// and equivalent for every post-processor that declares one rule per field.
fn index_by_source(post: &PostProcessor) -> BTreeMap<String, VarSpec> {
  let mut index: BTreeMap<String, VarSpec> = BTreeMap::new();
  for spec in post.variables.values() {
    index
      .entry(spec.source.trim().to_ascii_uppercase())
      .or_insert_with(|| spec.clone());
  }
  index
}

/// True when the move descends without changing XY, which the dialect calls a plunge.
fn is_plunge(from: Point3, to: Point3) -> bool {
  let same_xy = from.xy().is_near(to.xy(), 1.0e-9);
  to.z.mm() < from.z.mm() && same_xy
}

/// A `VarSpec`'s effective decimal count, exposed for documentation and tests.
impl VarSpec {
  /// Decimals actually applied, after the dialect's `1.N` spelling is decoded.
  pub fn effective_decimals(&self) -> u8 {
    self.decimals
  }
}
