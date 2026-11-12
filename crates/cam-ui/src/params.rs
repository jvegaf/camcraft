//! The six commands whose action is built from numbers: one builder per command, and the editor each
//! number is typed into.
//!
//! Six commands in [`crate::help::Command`] take their payload from [`Shell`]'s own fields rather than
//! from a gesture — Move, Rotate, Scale, Mirror, Array, Offset — because a gesture cannot say "three
//! rows" or "rotate by fifteen degrees". Two surfaces reach them: a palette chip or a canvas-menu row
//! (`chrome::Shell::command_action`) and a labelled button in the Tools tab (`Shell::authoring`). Those
//! two used to build each payload themselves, which is the shape where a number typed once is one number
//! the operator can run two ways *if* both copies agree.
//!
//! This module is that one copy, and it holds three things so they cannot drift apart:
//!
//! - **The builders.** One function per command, `&Shell` plus the view centre, returning the
//!   [`UiAction`]. A caller passes the centre in rather than reading the camera, so the shell stays the
//!   only place that knows where the view is looking.
//! - **The range.** A count that has a legal span carries it here
//!   ([`array_grid_range`](crate::params::array_grid_range)), and the builder
//!   that emits the count clamps with the same range the widget refuses to leave. A chip and a form
//!   therefore cannot disagree about a grid: before this module the widget applied `1..=64` and the
//!   palette passed the raw field, so a field set by anything other than the widget ran unchecked.
//! - **The editor metadata.** The word before each number, its unit, how far one drag step moves it, and
//!   which of [`Shell`]'s fields it edits, in the order the Tools tab draws them. The form is rendered
//!   from the same table the builder reads.
//!
//! What it deliberately does **not** own is validation with a sentence attached. [`Shell::scale_factors`]
//! has no range here, so a typed `0.0` reaches [`cam_app::Transform::Scale`] and is refused downstream
//! with a message that names the number; a silent clamp would replace that refusal with a result nobody
//! asked for. Ranges belong here only where the shell has no message to give.

use std::ops::RangeInclusive;

use cam_app::Transform;
use cam_geom::{Point2, Vec2};

use crate::help::Command;
use crate::{Shell, UiAction, DEFAULT_TOLERANCE_MM};

/// The smallest count an array grid dimension can hold, in rows or columns.
pub const ARRAY_GRID_MIN: usize = 1;

/// The largest count an array grid dimension can hold, in rows or columns.
pub const ARRAY_GRID_MAX: usize = 64;

/// The range an array's row and column count lives in.
///
/// One range, read by the builder that clamps *and* by the widget that refuses to leave it: the palette
/// can never run a grid the Tools tab cannot type, and the form can never show one the builder will
/// change.
pub fn array_grid_range() -> RangeInclusive<usize> {
  ARRAY_GRID_MIN..=ARRAY_GRID_MAX
}

/// `count` brought inside [`array_grid_range`].
fn grid_count(count: usize) -> usize {
  let range = array_grid_range();
  count.clamp(*range.start(), *range.end())
}

/// The move command: a displacement in millimetres, which no point has to be named for.
pub fn translate(shell: &Shell, _centre: Point2) -> UiAction {
  UiAction::Transform(Transform::Translate {
    delta: Vec2::new(shell.move_delta.0, shell.move_delta.1),
  })
}

/// The rotate command: degrees on the field, radians on the action, turning about the view centre.
pub fn rotate(shell: &Shell, centre: Point2) -> UiAction {
  UiAction::Transform(Transform::Rotate {
    radians: shell.rotate_degrees.to_radians(),
    about: centre,
  })
}

/// The scale command: the two factors, about the view centre, at the shell's arc tolerance.
pub fn scale(shell: &Shell, centre: Point2) -> UiAction {
  UiAction::Transform(Transform::Scale {
    sx: shell.scale_factors.0,
    sy: shell.scale_factors.1,
    about: centre,
    tolerance: DEFAULT_TOLERANCE_MM,
  })
}

/// The mirror command: in the world's Y direction, through the view centre.
///
/// The direction is a constant and the point is the view centre, so this command has no number to type:
/// [`fields`] answers with an empty row, which is why the Tools tab draws a button and nothing else.
pub fn mirror(_shell: &Shell, centre: Point2) -> UiAction {
  UiAction::Transform(Transform::Mirror {
    point: centre,
    direction: Vec2::Y_AXIS,
  })
}

/// The array command: the grid, clamped to [`array_grid_range`], and the two spacings.
pub fn array(shell: &Shell, _centre: Point2) -> UiAction {
  UiAction::Array {
    rows: grid_count(shell.array_grid.0),
    columns: grid_count(shell.array_grid.1),
    dx: shell.array_spacing.0,
    dy: shell.array_spacing.1,
  }
}

/// The offset command: one distance in millimetres.
pub fn offset(shell: &Shell, _centre: Point2) -> UiAction {
  UiAction::Offset {
    distance: shell.offset_distance,
  }
}

/// The action `command` pushes when it is one of the six, or `None` for every other command.
///
/// The palette chip, the canvas-menu row and the Tools tab's button all end up in one of the builders
/// above; this is the dispatcher the two command surfaces share, and [`Command::action`] is what answers
/// for the commands that need no number.
pub fn action(command: Command, shell: &Shell, centre: Point2) -> Option<UiAction> {
  let action = match command {
    Command::Move => translate(shell, centre),
    Command::Rotate => rotate(shell, centre),
    Command::Scale => scale(shell, centre),
    Command::Mirror => mirror(shell, centre),
    Command::Array => array(shell, centre),
    Command::Offset => offset(shell, centre),
    _ => return None,
  };
  Some(action)
}

/// Which of [`Shell`]'s numbers a Tools-tab row edits.
///
/// The list is the numbers of the six parameterised commands, and nothing else: a field with no entry
/// here has no builder and no row, which is what keeps this table honest about its own reach.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Number {
  /// The move command's displacement along X, in millimetres.
  MoveX,
  /// The move command's displacement along Y, in millimetres.
  MoveY,
  /// The rotate command's angle, in degrees counter-clockwise.
  RotateDegrees,
  /// The scale command's X factor.
  ScaleX,
  /// The scale command's Y factor.
  ScaleY,
  /// The array command's row count.
  ArrayRows,
  /// The array command's column count.
  ArrayColumns,
  /// The array command's spacing between columns, in millimetres.
  ArrayDx,
  /// The array command's spacing between rows, in millimetres.
  ArrayDy,
  /// The offset command's distance, in millimetres.
  OffsetDistance,
}

impl Number {
  /// The range this number lives in, when it has one.
  ///
  /// The widget refuses to leave it and the builder clamps into it, so both read the same span: a form
  /// that types `1..=64` and an action that emits `1..=64`. The scale factors have no entry on purpose —
  /// see the module comment.
  pub fn range(self) -> Option<RangeInclusive<usize>> {
    match self {
      Number::ArrayRows | Number::ArrayColumns => Some(array_grid_range()),
      _ => None,
    }
  }

  /// The count's slot in `shell`, for the widget that edits it as a whole number.
  ///
  /// Only called for a number whose [`Number::range`] is `Some`; a count edited as an `f64` would let a
  /// half-typed `2.5` become two rows without anyone saying so.
  fn count_mut(self, shell: &mut Shell) -> &mut usize {
    match self {
      Number::ArrayRows => &mut shell.array_grid.0,
      Number::ArrayColumns => &mut shell.array_grid.1,
      other => unreachable!("{other:?} is not a count"),
    }
  }

  /// The number's slot in `shell`, for the widget that edits it as a real number.
  fn value_mut(self, shell: &mut Shell) -> &mut f64 {
    match self {
      Number::MoveX => &mut shell.move_delta.0,
      Number::MoveY => &mut shell.move_delta.1,
      Number::RotateDegrees => &mut shell.rotate_degrees,
      Number::ScaleX => &mut shell.scale_factors.0,
      Number::ScaleY => &mut shell.scale_factors.1,
      Number::ArrayDx => &mut shell.array_spacing.0,
      Number::ArrayDy => &mut shell.array_spacing.1,
      Number::OffsetDistance => &mut shell.offset_distance,
      other => unreachable!("{other:?} is a count, not a real number"),
    }
  }
}

/// One number in a Tools-tab row: what it edits, how it is written, and how it is dragged.
pub struct Field {
  /// The word the tab writes before the number, or `""` for a number that simply follows the one before
  /// it — an `x` and a `y` do not need to repeat the command's name.
  pub label: &'static str,
  /// Which of [`Shell`]'s numbers this row edits.
  pub number: Number,
  /// The unit drawn after the number; empty for a count or a factor, because a bare `2` is not a unit and
  /// the design forbids a numeric field without one.
  pub suffix: &'static str,
  /// How far one drag step moves the number.
  pub speed: f64,
}

impl Field {
  /// Draw this number into the Tools tab and let the user type or drag it.
  ///
  /// The widget writes straight into [`Shell`]; the action is built later from the same field, so there is
  /// no copy of the number between the form and the command.
  pub fn widget(&self, ui: &mut egui::Ui, shell: &mut Shell) {
    if !self.label.is_empty() {
      ui.label(self.label);
    }
    if let Some(range) = self.number.range() {
      ui.add(egui::DragValue::new(self.number.count_mut(shell)).range(range));
      return;
    }
    let mut drag = egui::DragValue::new(self.number.value_mut(shell)).speed(self.speed);
    if !self.suffix.is_empty() {
      drag = drag.suffix(self.suffix);
    }
    ui.add(drag);
  }
}

/// The move command's row: a displacement along X and Y.
const MOVE: [Field; 2] = [
  Field {
    label: "Move",
    number: Number::MoveX,
    suffix: " mm",
    speed: 1.0,
  },
  Field {
    label: "",
    number: Number::MoveY,
    suffix: " mm",
    speed: 1.0,
  },
];

/// The rotate command's row: one angle.
const ROTATE: [Field; 1] = [Field {
  label: "Rotate",
  number: Number::RotateDegrees,
  suffix: "°",
  speed: 1.0,
}];

/// The scale command's row: two factors. A factor is a ratio, so neither carries a unit.
const SCALE: [Field; 2] = [
  Field {
    label: "Scale",
    number: Number::ScaleX,
    suffix: "",
    speed: 0.05,
  },
  Field {
    label: "",
    number: Number::ScaleY,
    suffix: "",
    speed: 0.05,
  },
];

/// The array command's row: the grid, then the spacing between its copies.
const ARRAY: [Field; 4] = [
  Field {
    label: "Array",
    number: Number::ArrayRows,
    suffix: "",
    speed: 1.0,
  },
  Field {
    label: "",
    number: Number::ArrayColumns,
    suffix: "",
    speed: 1.0,
  },
  Field {
    label: "",
    number: Number::ArrayDx,
    suffix: " mm",
    speed: 1.0,
  },
  Field {
    label: "",
    number: Number::ArrayDy,
    suffix: " mm",
    speed: 1.0,
  },
];

/// The offset command's row: one distance.
const OFFSET: [Field; 1] = [Field {
  label: "Offset",
  number: Number::OffsetDistance,
  suffix: " mm",
  speed: 0.5,
}];

/// The numbers a command's row edits, in the order the Tools tab draws them.
///
/// Mirror answers with an empty row — its point is the view centre and its direction is the world's Y —
/// and so does every command that is not one of the six.
pub fn fields(command: Command) -> &'static [Field] {
  match command {
    Command::Move => &MOVE,
    Command::Rotate => &ROTATE,
    Command::Scale => &SCALE,
    Command::Array => &ARRAY,
    Command::Offset => &OFFSET,
    _ => &[],
  }
}

/// Draw every number one of the six commands reads, the way the Tools tab draws it.
pub fn edit(ui: &mut egui::Ui, shell: &mut Shell, command: Command) {
  for field in fields(command) {
    field.widget(ui, shell);
  }
}
