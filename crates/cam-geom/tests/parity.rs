//! The corpus is what the parity harness measures, so it has to be the corpus it says it is.
//!
//! `scripts/parity/run.sh` answers "does the tree still measure what the goldens pinned?". This test
//! answers the question underneath it: "is the pinned thing still a whole board?" — every board present
//! with the six roles a fabrication set has (top, bottom, mask, silk, outline, drill), with its
//! provenance, and with a golden that pins exactly the artefacts the board actually carries. A golden
//! that pins a file nobody has, or carries a number for a refusal, is a fixture that would let a real
//! regression through while looking green.
//!
//! It lives here because the corpus is judged against the kernel's own conventions (a refused layer
//! has no polygons and no area), and it needs no parser to run: that is what makes it the part of the
//! harness `cargo test --workspace` can execute on its own.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// The roles every board in the corpus promises. Inner copper is extra, and a board that has it must
/// pin it too: dropping inner copper would let a 4-layer board pass parity on half its layers.
const REQUIRED_ROLES: [&str; 8] = [
  "top",
  "bottom",
  "mask-top",
  "mask-bottom",
  "silk-top",
  "silk-bottom",
  "outline",
  "drill",
];

fn corpus_root() -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../../fixtures/corpus")
    .canonicalize()
    .expect("fixtures/corpus is part of the repository")
}

fn boards() -> Vec<PathBuf> {
  let root = corpus_root();
  let mut found: Vec<PathBuf> = std::fs::read_dir(&root)
    .unwrap_or_else(|error| panic!("cannot read {}: {error}", root.display()))
    .map(|entry| entry.expect("a readable corpus entry").path())
    .filter(|path| path.is_dir())
    .collect();
  found.sort();
  assert!(
    !found.is_empty(),
    "the corpus under {} is empty",
    root.display()
  );
  found
}

/// The artefacts a board directory carries, keyed by role: `top.gbr` is `top`, `drill.drl` is
/// `drill`, and an inner copper layer is its own stem (`inner1`).
fn artefacts(board: &Path) -> BTreeSet<String> {
  let mut roles = BTreeSet::new();
  for entry in std::fs::read_dir(board)
    .unwrap_or_else(|error| panic!("cannot read {}: {error}", board.display()))
  {
    let path = entry.expect("a readable board entry").path();
    if !path.is_file() {
      continue;
    }
    if let Some("gbr" | "drl") = path.extension().and_then(|extension| extension.to_str()) {
      let role = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .expect("a Unicode artefact name")
        .to_owned();
      roles.insert(role);
    }
  }
  roles
}

fn read_json(path: &Path) -> serde_json::Value {
  let text = std::fs::read_to_string(path)
    .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
  serde_json::from_str(&text)
    .unwrap_or_else(|error| panic!("{} is not JSON: {error}", path.display()))
}

#[test]
fn every_board_carries_its_six_roles_and_its_provenance() {
  for board in boards() {
    let slug = board
      .file_name()
      .and_then(|name| name.to_str())
      .expect("a Unicode board name")
      .to_owned();
    let present = artefacts(&board);

    for role in REQUIRED_ROLES {
      assert!(
        present.contains(role),
        "{slug}: the corpus promises {role} and {}/. has no {role}.gbr or {role}.drl",
        board.display()
      );
    }

    let provenance_path = board.join("provenance.json");
    let provenance = read_json(&provenance_path);
    for field in [
      "slug",
      "source",
      "source_sha256",
      "exported_with",
      "fills",
      "artefacts",
    ] {
      assert!(
        provenance.get(field).is_some(),
        "{}: provenance does not record {field}",
        provenance_path.display()
      );
    }
    assert_eq!(
      provenance["slug"].as_str(),
      Some(slug.as_str()),
      "{}: provenance names a different board than its directory",
      provenance_path.display()
    );
    let recorded = provenance["source_sha256"]
      .as_str()
      .expect("a recorded source hash");
    assert_eq!(
      recorded.len(),
      64,
      "{}: source_sha256 is not a SHA-256 digest",
      provenance_path.display()
    );
    assert!(
      provenance["exported_with"]
        .as_str()
        .is_some_and(|version| version.starts_with("kicad-cli ")),
      "{}: provenance does not say which plotter wrote the files",
      provenance_path.display()
    );
  }
}

#[test]
fn every_golden_pins_exactly_the_artefacts_the_board_has() {
  for board in boards() {
    let slug = board
      .file_name()
      .and_then(|name| name.to_str())
      .expect("a Unicode board name")
      .to_owned();
    let golden_path = board.join("parity.json");
    assert!(
      golden_path.is_file(),
      "{slug}: no parity.json; run scripts/parity/run.sh --update and commit it"
    );
    let golden = read_json(&golden_path);

    for field in ["flatten_tolerance_mm", "timeout_seconds"] {
      assert!(
        golden[field].as_f64().is_some_and(|value| value > 0.0),
        "{}: {field} is missing or not a positive number",
        golden_path.display()
      );
    }

    let pinned: BTreeSet<String> = golden["artefacts"]
      .as_object()
      .unwrap_or_else(|| panic!("{}: no artefacts object", golden_path.display()))
      .keys()
      .cloned()
      .collect();
    let present = artefacts(&board);
    assert_eq!(
      pinned, present,
      "{slug}: the golden and the board disagree about which artefacts exist"
    );
  }
}

#[test]
fn a_refused_artefact_is_pinned_without_geometry() {
  for board in boards() {
    let slug = board
      .file_name()
      .and_then(|name| name.to_str())
      .expect("a Unicode board name")
      .to_owned();
    let golden_path = board.join("parity.json");
    let golden = read_json(&golden_path);
    let artefacts = golden["artefacts"]
      .as_object()
      .unwrap_or_else(|| panic!("{}: no artefacts object", golden_path.display()));

    for (role, measured) in artefacts {
      let where_ = format!("{slug}/{role}");
      let status = measured["status"]
        .as_str()
        .unwrap_or_else(|| panic!("{where_}: no status"));
      assert!(
        matches!(status, "parsed" | "refused" | "timeout"),
        "{where_}: status {status:?} is not one of parsed, refused or timeout"
      );

      for field in ["polygons", "points", "tools", "features"] {
        if let Some(value) = measured.get(field) {
          let number = value
            .as_u64()
            .unwrap_or_else(|| panic!("{where_}: {field} is not a count"));
          assert!(
            number <= u32::MAX as u64,
            "{where_}: {field} is not a plausible count"
          );
          if status != "parsed" {
            assert_eq!(
              number, 0,
              "{where_}: status is {status} but {field} reports geometry"
            );
          }
        }
      }

      if let Some(value) = measured.get("contour_area_mm2") {
        let area = value
          .as_f64()
          .unwrap_or_else(|| panic!("{where_}: contour_area_mm2 is not a number"));
        assert!(area.is_finite(), "{where_}: contour_area_mm2 is not finite");
        assert!(area >= 0.0, "{where_}: contour_area_mm2 is negative");
        if status != "parsed" {
          assert_eq!(
            area, 0.0,
            "{where_}: status is {status} but contour_area_mm2 is not zero"
          );
        }
      }
    }
  }
}

/// The tolerance a layer's image is evaluated at, matching `scripts/parity/measure/src/main.rs`.
const IMAGE_TOLERANCE_MM: f64 = 1e-6;

/// The relative epsilon E0.4 asks for, matching `scripts/parity/compare.py`.
const RELATIVE_EPSILON: f64 = 1e-9;

fn artefact_path(board: &Path, role: &str) -> PathBuf {
  let gerber = board.join(format!("{role}.gbr"));
  if gerber.is_file() {
    return gerber;
  }
  let drill = board.join(format!("{role}.drl"));
  assert!(
    drill.is_file(),
    "{role} is pinned for {} but neither {role}.gbr nor {role}.drl exists",
    board.display()
  );
  drill
}

fn close(left: f64, right: f64) -> bool {
  if left == right {
    return true;
  }
  let scale = left.abs().max(right.abs());
  (left - right).abs() <= RELATIVE_EPSILON * scale
}

/// Measure one artefact exactly as the harness does, and return its JSON object.
fn measure_artefact(artefact: &Path, flatten_tolerance_mm: f64) -> serde_json::Value {
  let text = std::fs::read_to_string(artefact)
    .unwrap_or_else(|error| panic!("cannot read {}: {error}", artefact.display()));
  let where_ = artefact.display();

  if artefact.extension().and_then(|ext| ext.to_str()) == Some("drl") {
    let drill = cam_excellon::parse(&text)
      .unwrap_or_else(|error| panic!("{where_} is pinned as parsed and is refused now: {error}"));
    return serde_json::json!({
      "status": "parsed",
      "tools": drill.tools.len(),
      "features": drill.features.len(),
    });
  }

  let gerber = cam_gerber::parse(&text)
    .unwrap_or_else(|error| panic!("{where_} is pinned as parsed and is refused now: {error}"));
  let image = gerber.image(IMAGE_TOLERANCE_MM).unwrap_or_else(|error| {
    panic!("{where_} is pinned as parsed and no longer evaluates: {error}")
  });
  let mut area = 0.0;
  let mut points = 0;
  for boundary in &image.boundaries {
    area += boundary.signed_area().abs();
    points += boundary.tessellate(flatten_tolerance_mm).len();
  }
  serde_json::json!({
    "status": "parsed",
    "polygons": image.boundaries.len(),
    "contour_area_mm2": area,
    "points": points,
  })
}

/// The half of the harness `cargo test` can run: every artefact the goldens pin as *parsed* is measured
/// here again, and compared with the same rules the script uses. A layer pinned as refused or timed out
/// is deliberately skipped — a test cannot bound a process, so those are `scripts/parity/run.sh`'s job,
/// where `timeout(1)` can stop one. Together the two cover the whole corpus; neither covers it alone.
#[test]
fn every_artefact_pinned_as_parsed_measures_the_same_in_the_workspace_suite() {
  for board in boards() {
    let slug = board
      .file_name()
      .and_then(|name| name.to_str())
      .expect("a Unicode board name")
      .to_owned();
    let golden = read_json(&board.join("parity.json"));
    let flatten = golden["flatten_tolerance_mm"]
      .as_f64()
      .expect("a recorded flatten tolerance");
    let artefacts = golden["artefacts"]
      .as_object()
      .unwrap_or_else(|| panic!("{slug}: no artefacts object"));

    for (role, pinned) in artefacts {
      if pinned["status"].as_str() != Some("parsed") {
        continue;
      }
      let measured = measure_artefact(&artefact_path(&board, role), flatten);
      let where_ = format!("{slug}/{role}");

      for field in ["polygons", "points", "tools", "features"] {
        if let Some(expected) = pinned.get(field) {
          assert_eq!(
            measured.get(field).and_then(|value| value.as_u64()),
            expected.as_u64(),
            "{where_}: {field} no longer matches the golden"
          );
        }
      }
      if let Some(expected) = pinned
        .get("contour_area_mm2")
        .and_then(|value| value.as_f64())
      {
        let actual = measured["contour_area_mm2"]
          .as_f64()
          .expect("a measured area");
        assert!(
          close(actual, expected),
          "{where_}: contour_area_mm2 is {actual} and the golden pins {expected}, past a relative {}",
          RELATIVE_EPSILON
        );
      }
    }
  }
}
