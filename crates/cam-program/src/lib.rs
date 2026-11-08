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
use cam_store::tools::{
  stored_tool_for_number, CuttingData, StoredTool, ToolGeometry, ToolLibrary,
};
use cam_toolpath::{ProgramTool, ToolSpec};
use cam_units::Length;

/// Why a document's program could not be built.
///
/// Two of the three ways to fail are given no sentence of this crate's own: `Display` is the cause's
/// wording, verbatim. The text a person reads belongs to the failure that produced it — the library's
/// query or the emitter's refusal — and it is byte-for-byte the text `cam-cli` printed when this
/// mapping was private to that binary, where `tests/cli.rs` pins every one of those sentences.
///
/// The third is this crate's own, and it is the one M11 exists for: a material the library has no
/// cutting data for. It names the material and the tool instead of falling back to the tool's own
/// record, because a program that quietly cuts at a rate nobody chose is the defect the milestone
/// removes (S4).
#[derive(Debug, thiserror::Error)]
pub enum ProgramError {
  /// The tool library could not be read while resolving a tool number.
  #[error("{0}")]
  Library(#[from] cam_store::StoreError),
  /// The emitter refused one of the document's toolpaths.
  #[error("{0}")]
  Toolpath(#[from] cam_toolpath::ToolpathError),
  /// The library keys no cutting data for the material the job asked to cut (M11 S4).
  ///
  /// It carries both names rather than a finished sentence: which of the two the operator has to
  /// add to the library is the thing they cannot guess, and the wording a person reads belongs to the
  /// caller that prints it.
  #[error("tool T{tool_number} has no cutting data for material \"{material}\"")]
  NoCuttingDataForMaterial {
    /// The program number the document's toolpath asked for.
    tool_number: u32,
    /// The material the job names, exactly as it was written.
    material: String,
  },
}

/// The program a document asks for, with every tool resolved from `library`.
///
/// The rates and the shape come from the library and from nowhere else, and one number the library
/// does not hold is refused by name together with the toolpath that asked for it. A caller that
/// already holds the tools as [`ProgramTool`]s calls `cam-toolpath::program_for` directly; this is
/// the function for a caller that holds a database.
///
/// `material` is the selector the job names (M11 S2). When it is `Some`, each tool's rates come from
/// the record the library keys for that material instead of the tool's own, and a material the
/// library has nothing for is refused by name (S4). When it is `None` the program is the one this
/// function has always built, byte for byte (S3).
///
/// The lookup asks for the machine-less record — `machine_id` is `None` — because a job names no
/// machine yet: M11 selects a material and nothing else.
pub fn program_for(
  document: &Document,
  library: &ToolLibrary,
  material: Option<&str>,
) -> Result<MoveStream, ProgramError> {
  let tools = program_tools(
    document,
    |number| -> Result<Option<ProgramTool>, ProgramError> {
      let Some((geometry, matched)) = library.tool_for_number(number)? else {
        return Ok(None);
      };
      // With no material selected, the record a program cuts with is the one the library keys **as the
      // tool's own** — the row whose material is null. Asking `tool_for_number` alone would answer with
      // whichever row the tool number matched first, which is how a library that keys rates per material
      // would cut as one of them while nobody had selected any (M11 S3).
      let own = match material {
        None => library
          .cutting_data_for(&geometry.id, None, None)?
          .unwrap_or(matched),
        Some(_) => matched,
      };
      let in_material = match material {
        Some(material) => library.cutting_data_for(&geometry.id, Some(material), None)?,
        None => None,
      };
      program_tool(number, &geometry, &own, material, in_material).map(Some)
    },
  )?;
  Ok(cam_toolpath::program_for(document, &tools)?)
}

/// The program a document asks for, with every tool resolved from a library *snapshot*.
///
/// The shell holds no [`ToolLibrary`] — it owns a rusqlite connection and is not `Clone`, so a window
/// is given `Vec<StoredTool>` instead (S4) — which leaves [`program_for`] unreachable from it. This is
/// the same program for a caller that holds those records and does no I/O of its own: the numbers
/// resolve through [`cam_store::tools::stored_tool_for_number`], the one rule the current-tool card
/// already uses (S3), and each record becomes the builder's tool through the same mapping the library
/// path uses.
///
/// There is no query here, so nothing about the snapshot itself can fail: a number the snapshot does
/// not hold is left out exactly as it is in [`program_for`], so the builder names the missing tool and
/// the toolpath that asked for it.
///
/// `material` is the same selector [`program_for`] takes, and a snapshot cannot answer it: a
/// [`StoredTool`] is the tool's own record with no material attached, so there is nothing in the
/// snapshot to select from. `None` is therefore the snapshot's whole range, and a material is refused
/// by name exactly as a missing library record is (S4) — the alternative would be a program cutting
/// at a rate nobody chose.
pub fn program_for_snapshot(
  document: &Document,
  tools: &[StoredTool],
  material: Option<&str>,
) -> Result<MoveStream, ProgramError> {
  let resolved = program_tools(
    document,
    |number| -> Result<Option<ProgramTool>, ProgramError> {
      let Some(tool) = stored_tool_for_number(tools, number) else {
        return Ok(None);
      };
      program_tool(number, &tool.geometry, &tool.cutting, material, None).map(Some)
    },
  )?;
  Ok(cam_toolpath::program_for(document, &resolved)?)
}

/// Resolve the document's tools the way the program builder needs them (S6).
///
/// `cam-toolpath` gains no `cam-store` edge, so the mapping from a stored tool onto the builder's
/// own types happens here, where both are known. One entry per distinct tool number, in document
/// order — and a number the entry point cannot find is left *out* rather than refused here: the
/// builder names the missing tool and the toolpath that asked for it, so the sentence is the same
/// one whether the tool is missing from a list of specs or from a library.
///
/// The two entry points differ in one thing only — how a number becomes a record, `resolve` — so this
/// loop and [`program_tool`] are written once. The error type is the caller's: the library path's
/// query can fail, the snapshot path's cannot.
fn program_tools<E, F>(document: &Document, mut resolve: F) -> Result<Vec<ProgramTool>, E>
where
  F: FnMut(u32) -> Result<Option<ProgramTool>, E>,
{
  let mut tools: Vec<ProgramTool> = Vec::new();
  for toolpath in &document.toolpaths {
    let number = toolpath.tool_number;
    if tools.iter().any(|tool| tool.spec.number == number) {
      continue;
    }
    let Some(tool) = resolve(number)? else {
      continue;
    };
    tools.push(tool);
  }
  Ok(tools)
}

/// One stored tool record, in the shape the program builder reads it (S6).
///
/// The single mapping both entry points resolve through: the library path hands it what
/// `ToolLibrary::tool_for_number` returned and the snapshot path what `stored_tool_for_number` found,
/// so the two cannot describe the same tool differently.
///
/// `material` is the selector the job names and `in_material` what the library answered for it — the
/// record keyed for `(geometry, material, machine)`, when there is one. A selector with no answer is
/// refused here, naming the material and the tool, so this one function is also the one place that
/// decides which of the two records a program cuts with: the library's record for the material, or,
/// when nothing is selected, the tool's own (M11 S2, S4).
fn program_tool(
  number: u32,
  geometry: &ToolGeometry,
  own: &CuttingData,
  material: Option<&str>,
  in_material: Option<CuttingData>,
) -> Result<ProgramTool, ProgramError> {
  let cutting: &CuttingData = match (material, &in_material) {
    (None, _) => own,
    (Some(_), Some(cutting)) => cutting,
    (Some(material), None) => {
      return Err(ProgramError::NoCuttingDataForMaterial {
        tool_number: number,
        material: material.to_owned(),
      })
    }
  };
  Ok(ProgramTool {
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
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use cam_geom::{Contour, Point2};
  use cam_model::{
    Entity, EntityKind, Job, Layer, Operation, OriginCorner, ProfileParams, ProfileSide, Toolpath,
  };
  use cam_move::ToolShape;
  use cam_store::tools::{CuttingData, Material, ToolGeometry};
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

    let program = program_for(&document, &library, None).expect("a program");

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

    let error = program_for(&document, &library, None).expect_err("the tool is missing");

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

    let error = program_for(&document, &library, None).expect_err("no diameter, no offset");

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
    link_second_tool(&library);

    let program = program_for(&document, &library, None).expect("a program");

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

  /// A second tool, program number `2`, whose rates differ from the first one's so a segment's own
  /// numbers say which tool it resolved.
  fn link_second_tool(library: &ToolLibrary) {
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
  }

  #[test]
  fn the_snapshot_and_the_library_build_the_same_program() {
    let document = document_with(vec![
      outside_profile("TP1", "First", 1),
      outside_profile("TP2", "Second", 2),
      outside_profile("TP3", "Third", 1),
    ]);
    let library = library_with(1, Some(6.0), None, None, 100.0, 30.0, 18_000);
    link_second_tool(&library);
    let snapshot = library.tools().expect("the snapshot reads");

    let from_library = program_for(&document, &library, None).expect("a program");
    let from_snapshot = program_for_snapshot(&document, &snapshot, None).expect("a program");

    // One per-record mapping, two ways of finding the record: the same tool numbers become the same
    // program, field for field — the tool, its shape, its rates and the order the toolpaths keep.
    assert_eq!(from_snapshot, from_library);
  }

  #[test]
  fn a_number_the_snapshot_does_not_hold_is_named_with_its_toolpath() {
    let document = document_with(vec![outside_profile("TP1", "Outside", 7)]);
    let library = library_with(1, Some(6.0), None, None, 100.0, 30.0, 18_000);
    let snapshot = library.tools().expect("the snapshot reads");

    let error = program_for_snapshot(&document, &snapshot, None).expect_err("the tool is missing");

    // The snapshot path's own failure is the emitter's — no material is selected, so nothing here
    // can refuse on the library — and the sentence is the one the library path prints for the same
    // absence (L6).
    assert_eq!(
      error.to_string(),
      "tool T7 used by toolpath \"TP1\" is not in the library"
    );
    assert!(matches!(
      error,
      ProgramError::Toolpath(ToolpathError::ToolNotInLibrary { tool_number: 7, .. })
    ));
  }

  /// A library holding one 6 mm end mill whose cutting data is keyed per material: `120 / 40 /
  /// 18 000` in `M1` and `60 / 20 / 12 000` in `M2`, and no material key at all for the record the
  /// tool resolves by number.
  ///
  /// This is the store's own shape (M11 S2): the same tool carries different rates per material, and
  /// the material is the only thing that says which one a program cuts with.
  fn library_with_materials() -> ToolLibrary {
    let library = ToolLibrary::open_in_memory().expect("the library opens");
    library
      .put_geometry(&ToolGeometry {
        id: "G1".to_owned(),
        name_format: "{Diameter}".to_owned(),
        tool_type: 0,
        units: UnitSystem::Mm,
        diameter: Some(Length::from_mm(6.0)),
        included_angle: None,
        flat_diameter: None,
        num_flutes: Some(2),
        flute_length: None,
        tip_radius: None,
        custom_attributes: None,
      })
      .expect("geometry");
    for (id, name) in [("M1", "MDF"), ("M2", "Hardwood")] {
      library
        .put_material(&Material {
          id: id.to_owned(),
          name: name.to_owned(),
        })
        .expect("material");
    }
    for (id, feed, plunge, spindle, material) in [
      ("C1", 120.0, 40.0, 18_000, "M1"),
      ("C2", 60.0, 20.0, 12_000, "M2"),
    ] {
      library
        .put_cutting_data(&CuttingData {
          id: id.to_owned(),
          rate_units: UnitSystem::Mm,
          feed_rate: Some(feed),
          plunge_rate: Some(plunge),
          spindle_speed: Some(spindle),
          stepdown: Some(Length::from_mm(2.0)),
          stepover: None,
          clear_stepover: None,
          // The same program number in every material: a material selects *which* record the number
          // resolves to, it does not renumber the tool.
          tool_number: Some(1),
          notes: None,
        })
        .expect("cutting data");
      library
        .link(&format!("E-{id}"), "G1", id, Some(material), None)
        .expect("the tool is linked in its material");
    }
    library
  }

  #[test]
  fn a_material_selects_the_record_the_library_keys_for_it() {
    let document = document_with(vec![outside_profile("TP1", "Outside", 1)]);
    let library = library_with_materials();

    let mdf = program_for(&document, &library, Some("M1")).expect("a program");
    let hardwood = program_for(&document, &library, Some("M2")).expect("a program");

    // S2: the rates come from the record the library keys for (geometry, material, machine) and not
    // from the tool's own, so the same toolpath cuts at the rate its material was authored with.
    assert_eq!(
      (mdf.segments[0].feed_rate, mdf.segments[0].plunge_rate),
      (120.0, 40.0)
    );
    assert_eq!(mdf.segments[0].spindle_speed, 18_000);
    assert_eq!(
      (
        hardwood.segments[0].feed_rate,
        hardwood.segments[0].plunge_rate
      ),
      (60.0, 20.0)
    );
    assert_eq!(hardwood.segments[0].spindle_speed, 12_000);
    // The shape is still the tool's: a material selects rates, never geometry.
    assert_eq!(mdf.segments[0].tool, hardwood.segments[0].tool);
    assert_eq!(mdf.segments[0].tool_number, 1);
  }

  #[test]
  fn a_material_the_library_has_nothing_for_is_refused_by_name() {
    let document = document_with(vec![outside_profile("TP1", "Outside", 1)]);
    let library = library_with_materials();

    let error = program_for(&document, &library, Some("M-unknown")).expect_err("no such material");

    // S4: never the tool's own rate. The refusal names the material and the tool, so an operator can
    // see which of the two is missing from the library.
    assert!(
      matches!(
        error,
        ProgramError::NoCuttingDataForMaterial {
          tool_number: 1,
          ref material,
        } if material == "M-unknown"
      ),
      "{error:?}"
    );
    let sentence = error.to_string();
    assert!(sentence.contains("M-unknown"), "{sentence}");
    assert!(sentence.contains("T1"), "{sentence}");
  }

  #[test]
  fn a_snapshot_holds_no_material_key_and_refuses_one_by_name() {
    // The shell is given `Vec<StoredTool>`, and a stored tool carries the tool's own record with no
    // material attached (S4), so a snapshot can never answer for a selected material. Refusing by
    // name is the only alternative to quietly cutting at the tool's own rate.
    let document = document_with(vec![outside_profile("TP1", "Outside", 1)]);
    let library = library_with_materials();
    let snapshot = library.tools().expect("the snapshot reads");

    assert!(program_for_snapshot(&document, &snapshot, None).is_ok());
    let error =
      program_for_snapshot(&document, &snapshot, Some("M1")).expect_err("the snapshot has no key");
    assert!(
      matches!(
        error,
        ProgramError::NoCuttingDataForMaterial {
          tool_number: 1,
          ref material,
        } if material == "M1"
      ),
      "{error:?}"
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
