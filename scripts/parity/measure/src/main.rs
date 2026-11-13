//! Measure one corpus artefact, for the parity harness in `scripts/parity/`.
//!
//! This is deliberately a separate, standalone crate rather than a test inside the workspace: the
//! parity harness has to read Gerber and Excellon, and `crates/cam-geom/Cargo.toml` — where those two
//! would have to be added as dev-dependencies — belongs to another task of the same wave. Keeping the
//! harness out of the workspace also keeps its cost out of `cargo test --workspace`.
//!
//! One artefact per process, on purpose. Some layers of the corpus take an unbounded time in the
//! geometry kernel today, and a process can be bounded by `timeout(1)` where a function call cannot;
//! the harness records that bound as `status: "timeout"` instead of hanging CI.
//!
//! Usage: `parity-measure <artefact>` where the artefact is a `.gbr` or a `.drl`.
//! Output: one JSON object, on stdout, in the shape the goldens store under `<board>.<role>`.

use std::path::Path;

use serde_json::{Map, Number, Value};

/// The tolerance the image of a layer is evaluated at, in millimetres.
///
/// It is the value the Gerber tests already use, and it is the one the goldens pin: the image of a
/// layer is a function of this tolerance, so a golden that does not say which tolerance produced it
/// cannot be compared against anything.
const IMAGE_TOLERANCE_MM: f64 = 1e-6;

/// The tolerance the contours are flattened at, in millimetres.
///
/// `points` counts points after flattening, so the count is a function of this number. It is a
/// separate constant from [`IMAGE_TOLERANCE_MM`] because they answer different questions: the first
/// decides how finely the boolean engine treats a curve while it works, the second how finely the
/// resulting contour is walked. Both are recorded in the golden.
const FLATTEN_TOLERANCE_MM: f64 = 1e-3;

fn main() {
  let path = std::env::args_os()
    .nth(1)
    .expect("usage: parity-measure <artefact>");
  let path = Path::new(&path);
  let name = path
    .file_name()
    .and_then(|name| name.to_str())
    .unwrap_or_default();

  let measured = if name.ends_with(".gbr") {
    measure_gerber(path)
  } else if name.ends_with(".drl") {
    measure_drill(path)
  } else {
    panic!("{name:?} is neither a .gbr nor a .drl");
  };

  // A file that cannot be read is not a file the reader refused, and the difference decides whether the
  // harness is measuring or pretending to. `refused` means "the reader answered and said no"; an I/O
  // failure means nobody asked. Collapsing the two would let an unreadable artefact — a permission
  // change, a vanished file — match a golden already pinned as `refused` and report a false green, so
  // this exits non-zero and `collect.py` records `error`, which `run.sh` refuses to compare or pin.
  let measured = match measured {
    Ok(measured) => measured,
    Err(error) => {
      eprintln!("{name}: {error}");
      std::process::exit(1);
    }
  };

  println!(
    "{}",
    serde_json::to_string(&Value::Object(measured)).expect("serialise the measurement")
  );
}

/// What a Gerber layer comes to, or that it was refused.
///
/// A refusal is a measurement, not an absence: a file this reader cannot honour has zero polygons,
/// zero area and zero points, and the golden records that so the day it starts reading, the change is
/// visible. The *reason* is deliberately not stored here — the parser's message is prose that changes
/// when someone rewords a diagnostic, and a golden that moves for that reason is noise. The reason is
/// printed by the harness' readable report instead.
fn measure_gerber(path: &Path) -> Result<Map<String, Value>, String> {
  let refused = || Ok(refusal(0.0));

  let text = std::fs::read_to_string(path)
    .map_err(|error| format!("the file could not be read: {error}"))?;
  let gerber = match cam_gerber::parse(&text) {
    Ok(gerber) => gerber,
    Err(_) => return refused(),
  };
  let image = match gerber.image(IMAGE_TOLERANCE_MM) {
    Ok(image) => image,
    Err(_) => return refused(),
  };

  // `contour_area_mm2` is the sum of the absolute area of every boundary, and it is named that way
  // because it is not the material area of the layer: a region with a hole has two boundaries, and
  // subtracting the hole needs the parity depth of each boundary, which `Region` does not hand out.
  // The rule here is the one this repository's own boolean corpus already uses for the same reason
  // (`crates/cam-geom/tests/boolean_corpus.rs`), and it is spelled out rather than assumed: a board
  // outline reads as roughly twice its enclosed area, which is the signature of a sum and not of an
  // area. When material area becomes a question (D2, and the hole handling of G0.2), it gets its own
  // field and its own golden line, instead of silently changing what this one has always meant.
  let mut area = 0.0;
  let mut points = 0usize;
  for boundary in &image.boundaries {
    area += boundary.signed_area().abs();
    points += boundary.tessellate(FLATTEN_TOLERANCE_MM).len();
  }

  let mut measured = Map::new();
  measured.insert("status".to_owned(), Value::String("parsed".to_owned()));
  measured.insert(
    "polygons".to_owned(),
    Value::Number((image.boundaries.len() as u64).into()),
  );
  measured.insert("contour_area_mm2".to_owned(), number(area));
  measured.insert("points".to_owned(), Value::Number((points as u64).into()));
  Ok(measured)
}

/// What an Excellon file comes to: how many tools it declares and how many features it asks for.
fn measure_drill(path: &Path) -> Result<Map<String, Value>, String> {
  let refused = || {
    let mut measured = Map::new();
    measured.insert("status".to_owned(), Value::String("refused".to_owned()));
    measured.insert("tools".to_owned(), Value::Number(0u64.into()));
    measured.insert("features".to_owned(), Value::Number(0u64.into()));
    Ok(measured)
  };

  let text = std::fs::read_to_string(path)
    .map_err(|error| format!("the file could not be read: {error}"))?;
  let drill = match cam_excellon::parse(&text) {
    Ok(drill) => drill,
    Err(_) => return refused(),
  };

  let mut measured = Map::new();
  measured.insert("status".to_owned(), Value::String("parsed".to_owned()));
  measured.insert(
    "tools".to_owned(),
    Value::Number((drill.tools.len() as u64).into()),
  );
  measured.insert(
    "features".to_owned(),
    Value::Number((drill.features.len() as u64).into()),
  );
  Ok(measured)
}

/// A refused Gerber layer: the three geometric metrics, all zero.
fn refusal(area: f64) -> Map<String, Value> {
  let mut measured = Map::new();
  measured.insert("status".to_owned(), Value::String("refused".to_owned()));
  measured.insert("polygons".to_owned(), Value::Number(0u64.into()));
  measured.insert("contour_area_mm2".to_owned(), number(area));
  measured.insert("points".to_owned(), Value::Number(0u64.into()));
  measured
}

/// A finite measurement as a JSON number that round-trips exactly.
///
/// `serde_json` with `float_roundtrip` writes the shortest representation that reads back as the same
/// bits, which is what makes the 1e-9 relative comparison in `compare.py` a comparison of the
/// measurement and not of its formatting.
fn number(value: f64) -> Value {
  Value::Number(Number::from_f64(value).expect("a finite measurement"))
}
