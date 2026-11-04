//! The error contract: which [`ToolpathError`]s exist, and what each one says (A14).
//!
//! M3's L6 defect was a placeholder surviving as a contract: the strategies were stubs returning
//! "not implemented yet", and nothing failed when they stayed stubs. This file is the guard against
//! a repeat. The `match` below has no wildcard arm, so Rust's exhaustiveness check makes a new
//! variant a compile error until it is described here, and no refusal may tell the operator that a
//! strategy is missing: an unimplemented strategy is a build error or a red test, never a message.
//!
//! It pins the *text* as well as the set, because the text is observable: `cam-cli` prints it
//! verbatim to stderr and exits `2`, so a reworded refusal is a behaviour change and should be a
//! deliberate edit here.

use cam_toolpath::ToolpathError;

/// One value of every variant the strategies can produce.
///
/// The length assertion in the message test keeps this list honest: a variant that is described
/// below but missing from the list would otherwise never be checked.
fn every_variant() -> Vec<ToolpathError> {
  vec![
    ToolpathError::OpenContour,
    ToolpathError::InvalidToolDiameter,
    ToolpathError::InvalidPassDepth,
    ToolpathError::InvalidDepthRange {
      cut_depth: 6.0,
      start_depth: 0.0,
    },
    ToolpathError::OffsetCollapsed {
      reason: "the offset has no area".to_owned(),
    },
    ToolpathError::TabOutOfRange {
      index: 2,
      position: 1.5,
    },
    ToolpathError::InvalidStepover { stepover: 0.0 },
    ToolpathError::InvalidPeckDepth { peck_depth: -1.0 },
    ToolpathError::NotACircle {
      entity: "E7".to_owned(),
    },
    ToolpathError::NoHoles,
    ToolpathError::InvalidBank,
    ToolpathError::InvalidIncludedAngle { degrees: 0.0 },
    ToolpathError::InvalidRamp { ramp: -1.0 },
    ToolpathError::RampTooLong {
      ramp: 51.0,
      length: 100.0,
    },
    ToolpathError::InvalidPitch { pitch: 0.0 },
    ToolpathError::ThreadToolTooWide {
      diameter: 14.0,
      hole: 12.0,
    },
    ToolpathError::InvalidSpacing { spacing: 0.0 },
    ToolpathError::NoGroove {
      spacing: 500.0,
      angle: 0.0,
    },
    ToolpathError::InlayAllowanceCollapsed {
      allowance: -25.0,
      reason: "the region offset collapsed an enclosing boundary".to_owned(),
    },
    ToolpathError::InlayRegionOutsideJob {
      width: 100.0,
      height: 100.0,
    },
    ToolpathError::VCarveNeedsAngledTool { tool_number: 3 },
    ToolpathError::VCarveNeedsSharpBit {
      tool_number: 3,
      flat_diameter: 0.4,
    },
    ToolpathError::MedialAxis {
      reason: "the medial axis needs a positive finite tolerance, in millimetres".to_owned(),
    },
    ToolpathError::VCarveNothingToCarve {
      tolerance: 50.0,
      width: 20.0,
      height: 20.0,
    },
    ToolpathError::ToolNotInLibrary {
      tool_number: 7,
      toolpath: "TP1".to_owned(),
    },
    ToolpathError::ToolWithoutDiameter { tool_number: 3 },
    ToolpathError::ChamferNeedsAngle {
      tool_number: 3,
      toolpath: "TP1".to_owned(),
    },
    ToolpathError::ChamferNeedsSharpBit {
      tool_number: 3,
      toolpath: "TP1".to_owned(),
      flat_diameter: 0.4,
      chamfer_width: 2.0,
    },
    ToolpathError::VCarveNeedsAngle {
      tool_number: 3,
      toolpath: "TP1".to_owned(),
    },
    ToolpathError::VCarveNeedsSharpTip {
      tool_number: 3,
      toolpath: "TP1".to_owned(),
      flat_diameter: 0.4,
    },
    ToolpathError::UnknownEntity {
      toolpath: "TP1".to_owned(),
      entity: "E9".to_owned(),
    },
    ToolpathError::NotAContour {
      entity: "E9".to_owned(),
    },
    ToolpathError::UnknownIsland {
      toolpath: "TP1".to_owned(),
      island: "E9".to_owned(),
    },
    ToolpathError::IslandNotAContour {
      island: "E9".to_owned(),
    },
    ToolpathError::MouldingProfileTooShort { points: 1 },
    ToolpathError::MouldingProfileLateral {
      index: 2,
      previous: 6.0,
      lateral: 6.0,
    },
    ToolpathError::MouldingGeometry {
      reason: "a moulding rail needs a length to sweep along, got 0 mm".to_owned(),
    },
    ToolpathError::InToolpath {
      toolpath: "TP1".to_owned(),
      source: Box::new(ToolpathError::NoHoles),
    },
  ]
}

#[test]
fn every_variant_keeps_its_message() {
  let variants = every_variant();
  assert_eq!(
    variants.len(),
    38,
    "every variant must be listed exactly once"
  );

  for error in variants {
    // Exhaustive on purpose (A14): a new variant does not compile until it is named here with the
    // sentence it shows the operator.
    let expected = match &error {
      ToolpathError::OpenContour => "cutting toolpaths need closed geometry".to_owned(),
      ToolpathError::InvalidToolDiameter => "tool diameter must be greater than zero".to_owned(),
      ToolpathError::InvalidPassDepth => "pass depth must be greater than zero".to_owned(),
      ToolpathError::InvalidDepthRange {
        cut_depth,
        start_depth,
      } => format!("cut depth {cut_depth} mm must be greater than start depth {start_depth} mm"),
      ToolpathError::OffsetCollapsed { reason } => format!("the tool does not fit: {reason}"),
      ToolpathError::TabOutOfRange { index, position } => {
        format!("tab {index} position {position} must be within [0, 1)")
      }
      ToolpathError::InvalidStepover { stepover } => {
        format!("stepover {stepover} mm must be greater than zero")
      }
      ToolpathError::InvalidPeckDepth { peck_depth } => {
        format!("peck depth {peck_depth} mm must not be negative")
      }
      ToolpathError::NotACircle { entity } => {
        format!("entity {entity:?} is not a circle, so it has no centre to work from")
      }
      ToolpathError::NoHoles => "drilling needs at least one hole".to_owned(),
      ToolpathError::InvalidBank => {
        "drill bank needs at least one drill and a positive pitch".to_owned()
      }
      ToolpathError::InvalidIncludedAngle { degrees } => {
        format!("chamfer included angle {degrees} degrees must be strictly between 0 and 180")
      }
      ToolpathError::InvalidRamp { ramp } => {
        format!("fluting ramp {ramp} mm must not be negative")
      }
      ToolpathError::RampTooLong { ramp, length } => {
        format!("fluting ramp {ramp} mm is longer than half of the vector's {length} mm length")
      }
      ToolpathError::InvalidPitch { pitch } => {
        format!("thread pitch {pitch} mm must be greater than zero")
      }
      ToolpathError::ThreadToolTooWide { diameter, hole } => {
        format!(
          "tool diameter {diameter} mm does not fit a {hole} mm hole: an internal thread's tool centre \
           orbits at (D - t)/2, which has no radius when the tool is as wide as the hole"
        )
      }
      ToolpathError::InvalidSpacing { spacing } => {
        format!("texture spacing {spacing} mm must be greater than zero")
      }
      ToolpathError::NoGroove { spacing, angle } => {
        format!(
          "texture spacing {spacing} mm at {angle} degrees leaves no groove across the region: the \
           pattern would cut nothing, so this is a spacing with no pattern behind it"
        )
      }
      ToolpathError::InlayAllowanceCollapsed { allowance, reason } => {
        format!("inlay allowance {allowance} mm leaves no region to cut: {reason}")
      }
      ToolpathError::InlayRegionOutsideJob { width, height } => {
        format!(
          "inlay region is not strictly inside the job's {width} x {height} mm rectangle: the male \
           clears the stock around the region, so a region on or across the stock's edge leaves nothing \
           for it to clear"
        )
      }
      ToolpathError::VCarveNeedsAngledTool { tool_number } => {
        format!(
          "tool T{tool_number} has no included angle in the library, so a V-carve's depth cannot be \
           derived: a V-carve needs a V-bit"
        )
      }
      ToolpathError::VCarveNeedsSharpBit {
        tool_number,
        flat_diameter,
      } => {
        format!(
          "tool T{tool_number} has a {flat_diameter} mm flat tip, so it cannot carve to a sharp edge: \
           a V-carve's depth law is the cone of a sharp point, so a V-carve needs a sharp V-bit"
        )
      }
      ToolpathError::MedialAxis { reason } => {
        format!("the V-carve's medial axis could not be computed: {reason}")
      }
      ToolpathError::VCarveNothingToCarve {
        tolerance,
        width,
        height,
      } => {
        format!(
          "the V-carve's {tolerance} mm tolerance leaves nothing to carve in the {width} x {height} \
           mm region: every point of the region is within that distance of its boundary, so the \
           medial axis is empty and there is no tip to plunge"
        )
      }
      ToolpathError::ToolNotInLibrary {
        tool_number,
        toolpath,
      } => format!("tool T{tool_number} used by toolpath {toolpath:?} is not in the library"),
      ToolpathError::ToolWithoutDiameter { tool_number } => {
        format!("tool T{tool_number} has no diameter, so a profile offset cannot be computed")
      }
      ToolpathError::ChamferNeedsAngle {
        tool_number,
        toolpath,
      } => {
        format!(
          "tool T{tool_number} has no included angle, so toolpath {toolpath:?} cannot cut a chamfer: \
           a V-bit strategy needs the tool's angle, and defaulting 90 degrees would cut a different \
           bevel"
        )
      }
      ToolpathError::ChamferNeedsSharpBit {
        tool_number,
        toolpath,
        flat_diameter,
        chamfer_width,
      } => {
        format!(
          "tool T{tool_number} has a {flat_diameter} mm flat tip, so toolpath {toolpath:?} would cut \
           a bevel wider than its {chamfer_width} mm chamfer width: a chamfer needs a sharp V-bit"
        )
      }
      ToolpathError::VCarveNeedsAngle {
        tool_number,
        toolpath,
      } => {
        format!(
          "tool T{tool_number} has no included angle, so toolpath {toolpath:?} cannot carve: a \
           V-carve's depth cannot be derived, and defaulting 90 degrees would cut a different carve"
        )
      }
      ToolpathError::VCarveNeedsSharpTip {
        tool_number,
        toolpath,
        flat_diameter,
      } => {
        format!(
          "tool T{tool_number} has a {flat_diameter} mm flat tip, so toolpath {toolpath:?} would not \
           carve to a sharp edge: a V-carve needs a sharp V-bit"
        )
      }
      ToolpathError::UnknownEntity { toolpath, entity } => {
        format!("toolpath {toolpath:?} cites unknown entity {entity:?}")
      }
      ToolpathError::NotAContour { entity } => format!("entity {entity:?} is not a contour"),
      ToolpathError::UnknownIsland { toolpath, island } => {
        format!("toolpath {toolpath:?} cites unknown island {island:?}")
      }
      ToolpathError::IslandNotAContour { island } => {
        format!("island {island:?} is not a contour")
      }
      ToolpathError::MouldingProfileTooShort { points } => {
        format!("a moulding profile needs at least two points, got {points}")
      }
      ToolpathError::MouldingProfileLateral {
        index,
        previous,
        lateral,
      } => {
        format!(
          "moulding profile point {index} at lateral {lateral} mm must be greater than the previous \
           {previous} mm: a cross-section is one depth per lateral distance"
        )
      }
      ToolpathError::MouldingGeometry { reason } => {
        format!("the moulding's geometry could not be built: {reason}")
      }
      ToolpathError::InToolpath { toolpath, source } => {
        format!("toolpath {toolpath:?}: {source}")
      }
    };
    assert_eq!(error.to_string(), expected, "{error:?}");
  }
}

#[test]
fn no_refusal_says_a_strategy_is_not_implemented() {
  for error in every_variant() {
    let message = error.to_string().to_ascii_lowercase();
    assert!(
      !message.contains("not implemented"),
      "{error:?} still tells the operator the strategy is missing: {message}"
    );
  }
}
