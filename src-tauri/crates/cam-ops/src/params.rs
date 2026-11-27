//! The machining vocabulary `cam-ops` implements.
//!
//! These describe how a capability *cuts*: stepover, allowance, peck depth, retract mode, lead
//! lengths, tab layout. They live here and not in `cam-model` because they are not part of the PCB
//! document. The document records what the board needs — an `Isolation`, a `Cutout` — and an adapter
//! maps that onto the machining parameters of the capability that performs it.
//!
//! Keeping the two apart is what lets the rescued capabilities exist before the operations that will
//! drive them: the algorithms were copied out of the general-machining crate, and so was the language
//! they speak.

use cam_model::Tab;
use cam_units::Length;

/// Which clearing pattern a pocket uses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PocketStrategy {
  /// Parallel rows across the region.
  Raster,
  /// Concentric rings following the boundary.
  #[default]
  Offset,
}

/// Parameters of a pocket toolpath.
#[derive(Clone, Debug, PartialEq)]
pub struct PocketParams {
  /// How the region is cleared.
  pub strategy: PocketStrategy,
  /// Material left on the wall, measured from the vector.
  pub allowance: Length,
  /// Distance between adjacent rings or raster rows.
  pub stepover: Length,
  /// Depth of material removed per pass.
  pub pass_depth: Length,
  /// Depth at which cutting starts, for a part already partly machined.
  pub start_depth: Length,
  /// Total cut depth below the stock surface.
  pub cut_depth: Length,
}

/// How far the tool comes back up between pecks.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum RetractMode {
  /// Lift by a fixed distance above the current depth.
  Amount {
    /// How far above the bottom of the current peck the tool returns.
    amount: Length,
  },
  /// Lift all the way to the job's safe Z, clearing the hole completely.
  #[default]
  Full,
}

/// A gang of drills on one head, spaced a fixed pitch apart along X.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrillBank {
  /// How many drills the head carries.
  pub count: u32,
  /// Distance between neighbouring drills along X.
  pub pitch: Length,
}

/// Parameters of a drilling toolpath.
#[derive(Clone, Debug, PartialEq)]
pub struct DrillParams {
  /// Depth of material removed per peck. Zero cuts the hole in one plunge.
  pub peck_depth: Length,
  /// How far the tool retracts between pecks.
  pub retract: RetractMode,
  /// Depth at which cutting starts, for a part already partly machined.
  pub start_depth: Length,
  /// Total cut depth below the stock surface.
  pub cut_depth: Length,
  /// The head's drill gang, when the machine has one.
  pub bank: Option<DrillBank>,
}

/// Which side of the selected vectors the tool runs on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ProfileSide {
  /// Cut inside the selected closed vector.
  Inside,
  /// Cut outside the selected closed vector.
  #[default]
  Outside,
  /// Cut directly on the vector.
  On,
}

/// Parameters of a profile (contour) toolpath.
#[derive(Clone, Debug, PartialEq)]
pub struct ProfileParams {
  /// Which side of the vector the tool runs on.
  pub side: ProfileSide,
  /// Extra material left outside the tool (positive) or removed beyond it (negative).
  pub allowance: Length,
  /// Depth of material removed per pass.
  pub pass_depth: Length,
  /// Depth at which cutting starts, for a part already partly machined.
  pub start_depth: Length,
  /// Total cut depth below the stock surface.
  pub cut_depth: Length,
  /// Straight lead-in length before the tool reaches the contour.
  pub lead_in: Length,
  /// Straight lead-out length after the contour is complete.
  pub lead_out: Length,
  /// Holding tabs, empty for none.
  pub tabs: Vec<Tab>,
}
