//! The post-processor engine: a parser for the `.pp` dialect and a data-driven G-code emitter.
//!
//! # The dialect
//!
//! The dialect is data, not code: a `.pp` file declares thirteen directives and twenty output blocks, and both
//! the parser and the emitter know all of them — `cam_post::parse_post` answers a summary the CLI prints and
//! `cam_post::emit` refuses a block it cannot fill. A post-processor is plain text:
//!
//! ```text
//! + a comment line
//! POST_NAME = "Mach2/3 Arcs (inch) (*.txt)"
//! FILE_EXTENSION = "txt"
//! UNITS = "INCHES"
//! LINE_ENDING = "[13][10]"
//! VAR FEED_RATE = [FC|C|F|1.1]
//! begin HEADER
//! "[N]G00G20G17G90G40G49G80"
//! ```
//!
//! Three details are easy to get wrong and are therefore pinned by tests:
//!
//! * The decimal count is written in the dialect's own form, `1.4` meaning *four* decimals, not
//!   `1.4` decimals.
//! * A `VAR` may carry a fifth field, a **scale factor** applied before formatting
//!   (`[FC|A||1.0|0.0166]` converts a per-minute rate to a per-second one).
//! * Mode `A` always emits the variable; mode `C` emits it only when it changed, which is what
//!   produces `G1X10.000Y20.000` instead of repeating `F` on every line.
//! * A block placeholder names the **source field**, not the `VAR`: blocks write `[X]` while the
//!   declaration is `VAR X_POSITION = [X|C|X|1.4]`. The `VAR` name is a human label for the rule.
//!
//! Must not depend on `cam-toolpath`, `cam-model`, `cam-store` or any UI crate.

#![warn(missing_docs)]

mod emit;
mod parser;

use std::collections::BTreeMap;

pub use emit::{emit, EmitError, Emitted, ProgramMeta};
pub use parser::parse_post;

/// The unit system a post-processor expects in its output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PostUnits {
  /// Millimetres.
  Millimetres,
  /// Inches.
  Inches,
}

impl PostUnits {
  /// Parse the `UNITS` directive's value.
  pub fn parse(value: &str) -> Option<Self> {
    match value.trim().to_ascii_uppercase().as_str() {
      "MM" | "MILLIMETRES" | "MILLIMETERS" => Some(Self::Millimetres),
      "INCH" | "INCHES" => Some(Self::Inches),
      _ => None,
    }
  }
}

/// How a variable appears in the output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmissionMode {
  /// Emit on every line that references the variable.
  Always,
  /// Emit only when the value changed since the last emitted line.
  OnChange,
}

/// One `VAR` declaration.
#[derive(Clone, Debug, PartialEq)]
pub struct VarSpec {
  /// Source field name, e.g. `X` or `FC`.
  pub source: String,
  /// Whether the value is emitted every time or only on change.
  pub mode: EmissionMode,
  /// Literal prefix placed before the number, e.g. `X`. May be empty.
  pub prefix: String,
  /// Digits after the decimal separator.
  pub decimals: u8,
  /// Multiplier applied to the source value before formatting. Usually `1.0`.
  pub scale: f64,
}

/// The output blocks a post-processor can define.
///
/// Twenty blocks: the sixteen the dialect's shipped post-processors carry, plus M6's four helical
/// kinds, which carry `[Z]` where the arc kinds cannot (D-M6-8).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BlockKind {
  /// Emitted once at the start of the program.
  Header,
  /// Emitted once at the end of the program.
  Footer,
  /// The first rapid of a segment.
  InitialRapidMove,
  /// A rapid move.
  RapidMove,
  /// The first rapid move of the program.
  FirstRapidMove,
  /// A move that plunges into the material.
  PlungeMove,
  /// The first feed move of a segment.
  FirstFeedMove,
  /// A feed move.
  FeedMove,
  /// A move that lifts to clearance.
  RetractMove,
  /// The first clockwise arc of a segment.
  FirstCwArcMove,
  /// A clockwise arc.
  CwArcMove,
  /// The first counter-clockwise arc of a segment.
  FirstCcwArcMove,
  /// A counter-clockwise arc.
  CcwArcMove,
  /// The first clockwise helical move of a segment.
  ///
  /// A helical block carries the Z as well as the centre offsets, because the move descends: an arc
  /// block's `[X][Y][I][J]` would emit the helix at one depth whatever it was doing (M6 A7).
  FirstCwHelixMove,
  /// A clockwise helical move.
  CwHelixMove,
  /// The first counter-clockwise helical move of a segment.
  FirstCcwHelixMove,
  /// A counter-clockwise helical move.
  CcwHelixMove,
  /// A feed rate change inside a segment.
  FeedRateChange,
  /// Start of a new toolpath segment.
  NewSegment,
  /// A tool change.
  Toolchange,
}

impl BlockKind {
  /// Every block kind, in a stable order.
  pub const ALL: [Self; 20] = [
    Self::Header,
    Self::Footer,
    Self::InitialRapidMove,
    Self::RapidMove,
    Self::FirstRapidMove,
    Self::PlungeMove,
    Self::FirstFeedMove,
    Self::FeedMove,
    Self::RetractMove,
    Self::FirstCwArcMove,
    Self::CwArcMove,
    Self::FirstCcwArcMove,
    Self::CcwArcMove,
    Self::FirstCwHelixMove,
    Self::CwHelixMove,
    Self::FirstCcwHelixMove,
    Self::CcwHelixMove,
    Self::FeedRateChange,
    Self::NewSegment,
    Self::Toolchange,
  ];

  /// The dialect's name for the block, as written after `begin`.
  pub fn as_str(self) -> &'static str {
    match self {
      Self::Header => "HEADER",
      Self::Footer => "FOOTER",
      Self::InitialRapidMove => "INITIAL_RAPID_MOVE",
      Self::RapidMove => "RAPID_MOVE",
      Self::FirstRapidMove => "FIRST_RAPID_MOVE",
      Self::PlungeMove => "PLUNGE_MOVE",
      Self::FirstFeedMove => "FIRST_FEED_MOVE",
      Self::FeedMove => "FEED_MOVE",
      Self::RetractMove => "RETRACT_MOVE",
      Self::FirstCwArcMove => "FIRST_CW_ARC_MOVE",
      Self::CwArcMove => "CW_ARC_MOVE",
      Self::FirstCcwArcMove => "FIRST_CCW_ARC_MOVE",
      Self::CcwArcMove => "CCW_ARC_MOVE",
      Self::FirstCwHelixMove => "FIRST_CW_HELIX_MOVE",
      Self::CwHelixMove => "CW_HELIX_MOVE",
      Self::FirstCcwHelixMove => "FIRST_CCW_HELIX_MOVE",
      Self::CcwHelixMove => "CCW_HELIX_MOVE",
      Self::FeedRateChange => "FEED_RATE_CHANGE",
      Self::NewSegment => "NEW_SEGMENT",
      Self::Toolchange => "TOOLCHANGE",
    }
  }

  /// Parse a block name as written after `begin`.
  pub fn parse(name: &str) -> Option<Self> {
    let name = name.trim();
    Self::ALL
      .into_iter()
      .find(|kind| kind.as_str().eq_ignore_ascii_case(name))
  }
}

/// A parsed post-processor.
#[derive(Clone, Debug, PartialEq)]
pub struct PostProcessor {
  /// `POST_NAME`, the display name shown next to the machine.
  pub post_name: String,
  /// `FILE_EXTENSION`, without the dot.
  pub file_extension: String,
  /// `UNITS`.
  pub units: PostUnits,
  /// `LINE_ENDING`, with `[13]`/`[10]` escapes resolved at emission time.
  pub line_ending: String,
  /// `LINE_NUMBER_START`, when line numbering is used.
  pub line_number_start: Option<i64>,
  /// `LINE_NUMBER_INCREMENT`.
  pub line_number_increment: Option<i64>,
  /// `LINE_NUMBER_MAXIMUM`, after which numbering wraps.
  pub line_number_maximum: Option<i64>,
  /// `DIRECT_OUTPUT`, an optional controller integration descriptor.
  pub direct_output: Option<String>,
  /// `SUBSTITUTE`, an optional literal-escape descriptor.
  pub substitute: Option<String>,
  /// `ROTARY_WRAP_X`.
  pub rotary_wrap_x: Option<String>,
  /// `ROTARY_WRAP_Y`.
  pub rotary_wrap_y: Option<String>,
  /// `RAPID_PLUNGE_TO_STARTZ`: plunge at rapid rate down to the start depth.
  pub rapid_plunge_to_start_z: bool,
  /// `PRINT_DIRECT`.
  pub print_direct: bool,
  /// `VAR` declarations, keyed by variable name in declaration order.
  pub variables: BTreeMap<String, VarSpec>,
  /// Output blocks, keyed by kind.
  pub blocks: BTreeMap<BlockKind, Vec<String>>,
}

impl Default for PostProcessor {
  fn default() -> Self {
    Self {
      post_name: String::new(),
      file_extension: String::new(),
      units: PostUnits::Millimetres,
      line_ending: String::from("\r\n"),
      line_number_start: None,
      line_number_increment: None,
      line_number_maximum: None,
      direct_output: None,
      substitute: None,
      rotary_wrap_x: None,
      rotary_wrap_y: None,
      rapid_plunge_to_start_z: false,
      print_direct: false,
      variables: BTreeMap::new(),
      blocks: BTreeMap::new(),
    }
  }
}

impl PostProcessor {
  /// The template lines of one block, or an empty slice when the post-processor omits it.
  pub fn block(&self, kind: BlockKind) -> &[String] {
    self
      .blocks
      .get(&kind)
      .map(Vec::as_slice)
      .unwrap_or_default()
  }
}

/// Where a post-processor could not be parsed.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("line {line}:{column}: {message}")]
pub struct PostError {
  /// 1-based line number.
  pub line: usize,
  /// 1-based column number.
  pub column: usize,
  /// What is wrong, including what was found and what was expected.
  pub message: String,
}

impl PostError {
  /// Build a diagnostic.
  pub fn new(line: usize, column: usize, message: impl Into<String>) -> Self {
    Self {
      line,
      column,
      message: message.into(),
    }
  }
}
