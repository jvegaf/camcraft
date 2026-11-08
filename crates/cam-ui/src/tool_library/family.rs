use cam_store::tools::ToolGeometry;
use cam_units::{Format, Length};

/// A tool family: the shape a tool's own geometry proves.
///
/// Derived, never declared (see this module's header): `tool_type` is a raw code the library does not map, so
/// two tools that carry the same shape fields are the same family whatever their code says.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ToolFamily {
  /// A straight cutter with a square end: a diameter and nothing that shapes its tip.
  Flat,
  /// A hemisphere for a tip: a tip radius of exactly half the diameter.
  BallNose,
  /// A square corner with a radius on it: a tip radius that is not half the diameter.
  CornerRadius,
  /// A cone ending in a flat: an included angle.
  VBit,
}

impl ToolFamily {
  /// Every family, in the order the tree groups them: the plainest shape first, the V last.
  pub const ALL: [ToolFamily; 4] = [
    ToolFamily::Flat,
    ToolFamily::BallNose,
    ToolFamily::CornerRadius,
    ToolFamily::VBit,
  ];

  /// The family a tool's own geometry proves.
  ///
  /// An included angle decides first, because a V is the only shape with one. Then the tip radius: half the
  /// diameter is a hemisphere, anything else is a corner that was rounded. A tool with neither is straight,
  /// which is also the answer for a record whose diameter is missing — there is no radius to prove otherwise.
  pub fn of(geometry: &ToolGeometry) -> Self {
    if geometry.included_angle.is_some() {
      return Self::VBit;
    }
    match (geometry.diameter, geometry.tip_radius) {
      (Some(diameter), Some(radius)) if (radius.mm() * 2.0 - diameter.mm()).abs() <= 1.0e-6 => {
        Self::BallNose
      }
      (_, Some(_)) => Self::CornerRadius,
      _ => Self::Flat,
    }
  }

  /// The word the tree's heading carries.
  pub fn label(self) -> &'static str {
    match self {
      Self::Flat => "Flat end mill",
      Self::BallNose => "Ball nose",
      Self::CornerRadius => "Corner radius",
      Self::VBit => "V-bit",
    }
  }

  /// The `GEOMETRY` fields this family has, in the order the form draws them.
  pub fn fields(self) -> &'static [ShapeField] {
    match self {
      Self::Flat => &[
        ShapeField::Diameter,
        ShapeField::FluteLength,
        ShapeField::Flutes,
      ],
      Self::BallNose => &[
        ShapeField::Diameter,
        ShapeField::TipRadius,
        ShapeField::FluteLength,
        ShapeField::Flutes,
      ],
      Self::CornerRadius => &[
        ShapeField::Diameter,
        ShapeField::CornerRadius,
        ShapeField::FluteLength,
        ShapeField::Flutes,
      ],
      Self::VBit => &[
        ShapeField::Diameter,
        ShapeField::IncludedAngle,
        ShapeField::FlatTip,
        ShapeField::FluteLength,
        ShapeField::Flutes,
      ],
    }
  }
}

/// One field of the `GEOMETRY` group.
///
/// The family decides which of these a tool has ([`ToolFamily::fields`]), which is the whole of "the fields
/// change with the family": a flat end mill has no included angle to show, and showing one as `—` would
/// describe a shape it is not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeField {
  /// The cutting diameter.
  Diameter,
  /// A V's included angle.
  IncludedAngle,
  /// The flat at the bottom of a V.
  FlatTip,
  /// A ball's radius.
  TipRadius,
  /// A square corner's radius.
  CornerRadius,
  /// The usable length of the flutes.
  FluteLength,
  /// How many cutting edges the tool has.
  Flutes,
}

impl ShapeField {
  /// The label the form writes above the value.
  pub fn label(self) -> &'static str {
    match self {
      Self::Diameter => "Diameter",
      Self::IncludedAngle => "Included angle",
      Self::FlatTip => "Flat tip",
      Self::TipRadius => "Ball radius",
      Self::CornerRadius => "Corner radius",
      Self::FluteLength => "Flute length",
      Self::Flutes => "Flutes",
    }
  }

  /// The value the record holds, or a dash for a number the library does not have.
  ///
  /// A dash and never a zero: a tool with no flute length has no length, and printing `0.00 mm` would be a
  /// number nobody wrote — the same rule [`crate::chrome`]'s shape line follows.
  pub fn value(self, geometry: &ToolGeometry) -> String {
    let length = |value: Option<Length>| match value {
      Some(length) => format!("{} mm", Format::fixed(2).apply(length.mm())),
      None => "—".to_owned(),
    };
    match self {
      Self::Diameter => match geometry.diameter {
        Some(diameter) => format!("Ø {}", length(Some(diameter))),
        None => "—".to_owned(),
      },
      Self::IncludedAngle => match geometry.included_angle {
        Some(angle) => format!("{}°", Format::fixed(1).apply(angle.degrees())),
        None => "—".to_owned(),
      },
      Self::FlatTip => length(geometry.flat_diameter),
      Self::TipRadius | Self::CornerRadius => match geometry.tip_radius {
        Some(radius) => format!("R {}", length(Some(radius))),
        None => "—".to_owned(),
      },
      Self::FluteLength => length(geometry.flute_length),
      Self::Flutes => match geometry.num_flutes {
        Some(flutes) => flutes.to_string(),
        None => "—".to_owned(),
      },
    }
  }
}
