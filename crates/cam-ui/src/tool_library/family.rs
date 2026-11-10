use cam_store::tools::{ToolGeometry, ToolKind};
use cam_units::{Format, Length};

/// The shape a tool's cutting end has, which is what the drawing can paint.
///
/// Two vocabularies, and the split is deliberate: [`ToolKind`] is what the operator *called* the tool and
/// what the library stores; this is what can be *drawn*. Six kinds collapse to four shapes because two
/// pairs paint identically — an engraving cutter ends flat like an end mill, and a drill ends in a cone to
/// a point like a V-bit whose flat is zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ToolFamily {
  /// A straight cutter with a square end.
  Flat,
  /// A hemisphere for a tip.
  BallNose,
  /// A square corner with a radius on it.
  CornerRadius,
  /// A cone ending in a flat or a point.
  VBit,
}

impl ToolFamily {
  /// The shape a kind paints as.
  pub fn of(kind: ToolKind) -> ToolFamily {
    match kind {
      ToolKind::FlatEndMill | ToolKind::Engraving => ToolFamily::Flat,
      ToolKind::BallNose => ToolFamily::BallNose,
      ToolKind::CornerRadius => ToolFamily::CornerRadius,
      ToolKind::VBit | ToolKind::Drill => ToolFamily::VBit,
    }
  }

  /// The shape a record paints as, read the way the form reads it.
  pub fn of_geometry(geometry: &ToolGeometry) -> ToolFamily {
    ToolFamily::of(ToolKind::of(geometry))
  }
}

/// One row of the `GEOMETRY` group.
///
/// [S5] fixes the order and the membership: a diameter every tool has, the shape numbers its kind needs —
/// one for most kinds, two for a V-bit (angle and tip width) and for a bull nose's radius — the flute length,
/// the shank, the overall length and the cuts. A field a kind does not have is
/// not drawn as a dash — it is not drawn at all, because a row saying `—` describes a shape the tool is
/// not.
///
/// [S5]: ../../../../odd/tasks/tool-library-editor.md
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeField {
  /// The cutting diameter.
  Diameter,
  /// A pointed tip's included angle: a V-bit's or a drill's.
  IncludedAngle,
  /// The flat at the end of a cone: a V-bit's or an engraving cutter's.
  TipWidth,
  /// A bull-nose's corner radius.
  CornerRadius,
  /// The usable length of the flutes.
  FluteLength,
  /// The diameter of the shank the cutter is held by.
  ShankDiameter,
  /// The whole tool's length.
  OverallLength,
  /// How many cutting edges the tool has.
  Flutes,
}

impl ShapeField {
  /// The `GEOMETRY` rows one kind draws, in S5's order.
  pub fn of(kind: ToolKind) -> &'static [ShapeField] {
    match kind {
      ToolKind::FlatEndMill => &[
        ShapeField::Diameter,
        ShapeField::FluteLength,
        ShapeField::ShankDiameter,
        ShapeField::OverallLength,
        ShapeField::Flutes,
      ],
      // An engraving cutter's cone ends flat, and the reference asks for that flat: the same row a V-bit has,
      // in the place its own frame puts it — right under the diameter.
      ToolKind::Engraving => &[
        ShapeField::Diameter,
        ShapeField::TipWidth,
        ShapeField::FluteLength,
        ShapeField::ShankDiameter,
        ShapeField::OverallLength,
        ShapeField::Flutes,
      ],
      ToolKind::BallNose => &[
        ShapeField::Diameter,
        ShapeField::FluteLength,
        ShapeField::ShankDiameter,
        ShapeField::OverallLength,
        ShapeField::Flutes,
      ],
      ToolKind::CornerRadius => &[
        ShapeField::Diameter,
        ShapeField::CornerRadius,
        ShapeField::FluteLength,
        ShapeField::ShankDiameter,
        ShapeField::OverallLength,
        ShapeField::Flutes,
      ],
      // A drill's tip is a point by definition, so it carries no tip width; a V-bit's does.
      ToolKind::VBit => &[
        ShapeField::Diameter,
        ShapeField::IncludedAngle,
        ShapeField::TipWidth,
        ShapeField::FluteLength,
        ShapeField::ShankDiameter,
        ShapeField::OverallLength,
        ShapeField::Flutes,
      ],
      ToolKind::Drill => &[
        ShapeField::Diameter,
        ShapeField::IncludedAngle,
        ShapeField::FluteLength,
        ShapeField::ShankDiameter,
        ShapeField::OverallLength,
        ShapeField::Flutes,
      ],
    }
  }

  /// The row's label, spelled the way the reference screen spells it.
  pub fn label(self) -> &'static str {
    match self {
      Self::Diameter => "Diameter",
      Self::IncludedAngle => "Included angle",
      Self::TipWidth => "Tip width",
      Self::CornerRadius => "Corner radius",
      Self::FluteLength => "Flute length",
      Self::ShankDiameter => "Shank diameter",
      Self::OverallLength => "Overall length",
      Self::Flutes => "Number of cuts",
    }
  }

  /// The unit the box's suffix shows, or `None` for a count that has none.
  pub fn unit(self) -> Option<&'static str> {
    match self {
      Self::Diameter
      | Self::TipWidth
      | Self::FluteLength
      | Self::ShankDiameter
      | Self::OverallLength
      | Self::CornerRadius => Some("mm"),
      Self::IncludedAngle => Some("°"),
      Self::Flutes => None,
    }
  }

  /// The number the record holds, rendered for its box, or `None` when the library does not hold one.
  ///
  /// `None` and never a zero: a tool with no flute length has no length, and seeding a box with `0.00`
  /// would be a number nobody wrote — the rule this module already followed about a dash.
  pub fn value(self, geometry: &ToolGeometry) -> Option<String> {
    match self {
      Self::Diameter => geometry.diameter.map(decimal),
      Self::IncludedAngle => geometry.included_angle.map(|angle| {
        let degrees = angle.degrees();
        if degrees.fract() == 0.0 {
          format!("{degrees:.0}")
        } else {
          Format::fixed(1).apply(degrees)
        }
      }),
      Self::CornerRadius => geometry.tip_radius.map(decimal),
      Self::TipWidth => geometry.flat_diameter.map(decimal),
      Self::FluteLength => geometry.flute_length.map(decimal),
      Self::ShankDiameter => geometry.shank_diameter.map(decimal),
      Self::OverallLength => geometry.overall_length.map(decimal),
      Self::Flutes => geometry.num_flutes.map(|flutes| flutes.to_string()),
    }
  }
}

/// A millimetre value with at most three decimals and no trailing zeroes: `6`, `3.175`, `12.7`.
fn decimal(length: Length) -> String {
  let text = Format::fixed(3).apply(length.mm());
  let trimmed = text.trim_end_matches('0').trim_end_matches('.');
  if trimmed.is_empty() {
    "0".to_owned()
  } else {
    trimmed.to_owned()
  }
}
