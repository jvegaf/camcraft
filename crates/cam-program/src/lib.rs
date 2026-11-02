//! The document's program: the one place that joins a document's toolpaths to a tool library.
//!
//! `cam-toolpath::program_for` turns each toolpath into moves but refuses to know where its tools
//! come from, and `cam-store` knows the tools but may not depend on the emitter — the two edges S6
//! forbids. Neither crate can therefore see the other half, which is why the mapping from a stored
//! tool record onto the emitter's own tool type used to live in `cam-cli`'s binary, reachable by
//! nothing else (L8). This crate is that join, one level above both: it depends on `cam-store` and
//! on `cam-toolpath`, and nothing depends on it except the binary, so it closes no cycle.
//!
//! Must not depend on `cam-post` or any UI crate.

#![warn(missing_docs)]

use cam_model::Document;
use cam_move::MoveStream;
use cam_store::tools::ToolLibrary;
use cam_toolpath::{ProgramTool, ToolSpec};
use cam_units::Length;

/// Why a document's program could not be built.
///
/// There are exactly two ways to fail, and neither is given a sentence of this crate's own:
/// `Display` is the cause's wording, verbatim. The text a person reads belongs to the failure that
/// produced it — the library's query or the emitter's refusal — and it is byte-for-byte the text
/// `cam-cli` printed when this mapping was private to that binary, where `tests/cli.rs` pins every
/// one of those sentences.
#[derive(Debug, thiserror::Error)]
pub enum ProgramError {
  /// The tool library could not be read while resolving a tool number.
  #[error("{0}")]
  Library(#[from] cam_store::StoreError),
  /// The emitter refused one of the document's toolpaths.
  #[error("{0}")]
  Toolpath(#[from] cam_toolpath::ToolpathError),
}

/// The program a document asks for, with every tool resolved from `library`.
///
/// The rates and the shape come from the library and from nowhere else, and one number the library
/// does not hold is refused by name together with the toolpath that asked for it. A caller that
/// already holds the tools as [`ProgramTool`]s calls `cam-toolpath::program_for` directly; this is
/// the function for a caller that holds a database.
pub fn program_for(document: &Document, library: &ToolLibrary) -> Result<MoveStream, ProgramError> {
  let tools = program_tools(document, library)?;
  Ok(cam_toolpath::program_for(document, &tools)?)
}

/// Resolve the document's tools the way the program builder needs them (S6).
///
/// `cam-toolpath` gains no `cam-store` edge, so the mapping from a stored tool onto the builder's
/// own types happens here, where both are known. One entry per distinct tool number, in document
/// order — and a number the library does not hold is left *out* rather than refused here: the
/// builder names the missing tool and the toolpath that asked for it, so the sentence is the same
/// one whether the tool is missing from a list of specs or from a library.
fn program_tools(
  document: &Document,
  library: &ToolLibrary,
) -> Result<Vec<ProgramTool>, ProgramError> {
  let mut tools: Vec<ProgramTool> = Vec::new();
  for toolpath in &document.toolpaths {
    let number = toolpath.tool_number;
    if tools.iter().any(|tool| tool.spec.number == number) {
      continue;
    }
    let Some((geometry, cutting)) = library
      .tool_for_number(number)
      .map_err(ProgramError::Library)?
    else {
      continue;
    };
    tools.push(ProgramTool {
      spec: ToolSpec {
        number,
        // A tool whose diameter the library does not record keeps the builder's own refusal: zero is
        // the spec's way of saying "no diameter", and the builder names the tool when it sees one.
        diameter: geometry.diameter.unwrap_or(Length::ZERO),
        included_angle: geometry.included_angle,
        feed_rate: cutting.feed_rate.unwrap_or(0.0),
        plunge_rate: cutting.plunge_rate.unwrap_or(0.0),
        spindle_speed: cutting.spindle_speed.unwrap_or(0),
      },
      flat_diameter: geometry.flat_diameter.unwrap_or(Length::ZERO),
    });
  }
  Ok(tools)
}

#[cfg(test)]
mod tests {
  use super::*;
  use cam_geom::{Contour, Point2};
  use cam_model::{
    Entity, EntityKind, Job, Layer, Operation, OriginCorner, ProfileParams, ProfileSide, Toolpath,
  };
  use cam_move::ToolShape;
  use cam_store::tools::{CuttingData, ToolGeometry};
  use cam_toolpath::ToolpathError;
  use cam_units::{Angle, UnitSystem};

  fn profile_params() -> ProfileParams {
    ProfileParams {
      side: ProfileSide::Outside,
      allowance: Length::ZERO,
      pass_depth: Length::from_mm(6.0),
      start_depth: Length::ZERO,
      cut_depth: Length::from_mm(6.0),
      lead_in: Length::ZERO,
      lead_out: Length::ZERO,
      tabs: Vec::new(),
    }
  }

  fn outside_profile(id: &str, name: &str, tool_number: u32) -> Toolpath {
    Toolpath {
      id: id.to_owned(),
      name: name.to_owned(),
      entity_ids: vec!["E1".to_owned()],
      island_ids: Vec::new(),
      tool_number,
      operation: Operation::Profile(profile_params()),
    }
  }

  /// One layer holding one rectangle, `E1`, and the toolpaths given.
  fn document_with(toolpaths: Vec<Toolpath>) -> Document {
    let contour = Contour::from_points(
      &[
        Point2::new(0.0, 0.0),
        Point2::new(40.0, 0.0),
        Point2::new(40.0, 30.0),
        Point2::new(0.0, 30.0),
      ],
      true,
    )
    .expect("rectangle");
    let mut document = Document::new(Job {
      origin: OriginCorner::BottomLeft,
      width: Length::from_mm(100.0),
      height: Length::from_mm(100.0),
      ..Job::default_sheet()
    });
    document.layers.push(Layer {
      id: "L1".to_owned(),
      name: "Part".to_owned(),
      visible: true,
      entities: vec![Entity {
        id: "E1".to_owned(),
        kind: EntityKind::Contour { contour },
      }],
    });
    document.toolpaths = toolpaths;
    document
  }

  /// A library holding one tool: program number `number`, the geometry the caller passes, and a
  /// cutting record whose rates are the ones a program carries.
  fn library_with(
    number: u32,
    diameter: Option<f64>,
    flat_diameter: Option<f64>,
    included_angle: Option<f64>,
    feed_rate: f64,
    plunge_rate: f64,
    spindle_speed: u32,
  ) -> ToolLibrary {
    let library = ToolLibrary::open_in_memory().expect("the library opens");
    library
      .put_geometry(&ToolGeometry {
        id: "G1".to_owned(),
        name_format: "{Diameter}".to_owned(),
        tool_type: 0,
        units: UnitSystem::Mm,
        diameter: diameter.map(Length::from_mm),
        included_angle: included_angle.map(Angle::from_degrees),
        flat_diameter: flat_diameter.map(Length::from_mm),
        num_flutes: Some(2),
        flute_length: None,
        tip_radius: None,
        custom_attributes: None,
      })
      .expect("geometry");
    library
      .put_cutting_data(&CuttingData {
        id: "C1".to_owned(),
        rate_units: UnitSystem::Mm,
        feed_rate: Some(feed_rate),
        plunge_rate: Some(plunge_rate),
        spindle_speed: Some(spindle_speed),
        stepdown: Some(Length::from_mm(2.0)),
        stepover: None,
        clear_stepover: None,
        tool_number: Some(number),
        notes: None,
      })
      .expect("cutting data");
    library
      .link("T1", "G1", "C1", None, None)
      .expect("the tool is linked");
    library
  }

  #[test]
  fn the_rates_and_the_diameter_come_from_the_library() {
    let document = document_with(vec![outside_profile("TP1", "Outside", 1)]);
    let library = library_with(1, Some(6.35), None, None, 120.0, 40.0, 18_000);

    let program = program_for(&document, &library).expect("a program");

    // One toolpath, one segment, and every number on it is the library's: this function opens no
    // other door onto a tool.
    assert_eq!(program.segments.len(), 1);
    let segment = &program.segments[0];
    assert_eq!(segment.name, "Outside");
    assert_eq!(segment.tool_number, 1);
    assert_eq!(segment.feed_rate, 120.0);
    assert_eq!(segment.plunge_rate, 40.0);
    assert_eq!(segment.spindle_speed, 18_000);
    assert_eq!(segment.tool, ToolShape::cylinder(Length::from_mm(6.35)));
    assert_eq!(program.safe_z, document.job.safe_z);
  }

  #[test]
  fn a_number_the_library_does_not_hold_is_named_with_its_toolpath() {
    let document = document_with(vec![outside_profile("TP1", "Outside", 7)]);
    // The library holds a different tool, so the number is genuinely absent rather than the library
    // being empty by accident.
    let library = library_with(1, Some(6.0), None, None, 100.0, 30.0, 18_000);

    let error = program_for(&document, &library).expect_err("the tool is missing");

    // The sentence is the emitter's, unchanged: the tool is absent from the list this crate built,
    // and the builder is what names it (L6).
    assert_eq!(
      error.to_string(),
      "tool T7 used by toolpath \"TP1\" is not in the library"
    );
    assert!(matches!(
      error,
      ProgramError::Toolpath(ToolpathError::ToolNotInLibrary { tool_number: 7, .. })
    ));
  }

  #[test]
  fn a_tool_without_a_diameter_keeps_the_emitters_refusal() {
    let document = document_with(vec![outside_profile("TP1", "Outside", 1)]);
    let library = library_with(1, None, None, None, 100.0, 30.0, 18_000);

    let error = program_for(&document, &library).expect_err("no diameter, no offset");

    // The library's silence about the diameter reaches the emitter as `Length::ZERO`, which is the
    // spec's way of saying "none", so the builder's own sentence is what a person reads.
    assert_eq!(
      error.to_string(),
      "tool T1 has no diameter, so a profile offset cannot be computed"
    );
  }

  #[test]
  fn two_toolpaths_on_one_tool_resolve_it_once_and_keep_their_order() {
    let document = document_with(vec![
      outside_profile("TP1", "First", 1),
      outside_profile("TP2", "Second", 2),
      outside_profile("TP3", "Third", 1),
    ]);
    let library = library_with(1, Some(6.0), None, None, 100.0, 30.0, 18_000);
    library
      .put_geometry(&ToolGeometry {
        id: "G2".to_owned(),
        name_format: "{Diameter}".to_owned(),
        tool_type: 0,
        units: UnitSystem::Mm,
        diameter: Some(Length::from_mm(3.0)),
        included_angle: None,
        flat_diameter: None,
        num_flutes: None,
        flute_length: None,
        tip_radius: None,
        custom_attributes: None,
      })
      .expect("geometry");
    library
      .put_cutting_data(&CuttingData {
        id: "C2".to_owned(),
        rate_units: UnitSystem::Mm,
        feed_rate: Some(50.0),
        plunge_rate: Some(20.0),
        spindle_speed: Some(12_000),
        stepdown: None,
        stepover: None,
        clear_stepover: None,
        tool_number: Some(2),
        notes: None,
      })
      .expect("cutting data");
    library
      .link("T2", "G2", "C2", None, None)
      .expect("the second tool is linked");

    let program = program_for(&document, &library).expect("a program");

    // Every toolpath becomes its own segment, in document order, and a number used twice resolves to
    // the same tool both times.
    let seen: Vec<(&str, u32, f64)> = program
      .segments
      .iter()
      .map(|segment| {
        (
          segment.name.as_str(),
          segment.tool_number,
          segment.feed_rate,
        )
      })
      .collect();
    assert_eq!(
      seen,
      vec![
        ("First", 1, 100.0),
        ("Second", 2, 50.0),
        ("Third", 1, 100.0)
      ]
    );
  }

  #[test]
  fn the_wrapper_adds_no_words_of_its_own() {
    // The refusals `cam-cli` prints are pinned byte for byte by `tests/cli.rs`; this crate's job is to
    // carry them, so each one has to survive unchanged — a missing angle first, a flat tip second.
    let refusals = [
      ToolpathError::ChamferNeedsAngle {
        tool_number: 4,
        toolpath: "TP1".to_owned(),
      },
      ToolpathError::VCarveNeedsSharpTip {
        tool_number: 4,
        toolpath: "TP1".to_owned(),
        flat_diameter: 0.2,
      },
    ];
    for refusal in refusals {
      let sentence = refusal.to_string();
      assert_eq!(ProgramError::from(refusal).to_string(), sentence);
    }
    let wrapped = ProgramError::from(ToolpathError::VCarveNeedsAngle {
      tool_number: 4,
      toolpath: "TP1".to_owned(),
    });
    assert!(
      wrapped.to_string().contains("has no included angle"),
      "{wrapped}"
    );
  }
}
