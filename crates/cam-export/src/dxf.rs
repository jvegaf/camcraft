//! DXF export: the ASCII group-code stream that `cam-import`'s DXF reader reads back.
//!
//! # What is written, and why in this shape
//!
//! * A `HEADER` section declaring `$ACADVER` `AC1015` (R2000, the first version whose `LWPOLYLINE`
//!   carries bulges) and `$INSUNITS` `4`, millimetres. Code 4 is the code the reader turns into a
//!   scale of exactly `1.0`, so a coordinate leaves and comes back with no factor between them.
//! * A `TABLES` section with one `LAYER` table holding every document layer under its own `name`, in
//!   document order. The reader takes that order as the document's layer order
//!   (`crates/cam-import/tests/dxf.rs:404`), so writing it in any other order would reorder the
//!   document that comes back.
//! * An `ENTITIES` section with one `LWPOLYLINE` per contour: `8` is its layer, `90` the vertex
//!   count, `70` is `1` when the contour is closed and `0` when it is open, and every vertex is a
//!   `10`/`20` point followed by `42`, the bulge of the segment leaving it.
//!
//! Nothing is derived. A bulge leaves with the value the kernel gave it, so `sweep = 4·atan(bulge)`
//! is the same on both sides and a circle's two half turns stay two half turns. Closedness is the
//! `70` bit and never a repeated first vertex, because the model stores it explicitly so that a
//! contour carries no phantom zero-length segment (`crates/cam-geom/src/contour.rs:34`); repeating
//! the first vertex here would hand the reader that phantom segment back.
//!
//! # What a DXF cannot carry
//!
//! The document has one entity kind, so a contour leaves as a polyline and never as a `LINE`, `ARC`
//! or `CIRCLE`, an ellipse's axis ratio is already gone by the time geometry reaches the model, and
//! toolpaths are not geometry: they are not written, and writing them would make "the same thing
//! that went in" unprovable.

use cam_geom::Contour;
use cam_model::{Document, EntityKind};

use crate::ExportError;

/// The format name every error in this module carries.
const FORMAT: &str = "dxf";

/// The declared DXF version: R2000, the first one that knows `LWPOLYLINE`.
const ACAD_VERSION: &str = "AC1015";

/// The `$INSUNITS` code for millimetres, which the reader turns into a scale of exactly `1.0`.
const INSUNITS_MILLIMETRES: i32 = 4;

/// The ACI colour every layer is declared with: white, the default a drawing opens with.
const LAYER_COLOUR: i32 = 7;

/// The linetype every layer is declared with, so the table is complete for any other CAD reader.
const LAYER_LINETYPE: &str = "CONTINUOUS";

/// The `70` flag of a closed light polyline.
const FLAG_CLOSED: i32 = 1;

/// The `70` flag of an open light polyline.
const FLAG_OPEN: i32 = 0;

/// Export `document` as ASCII DXF.
///
/// The bytes depend on the document and on nothing else: layers in document order, entities in layer
/// order, and numbers in their shortest round-trip form, so two exports of one document are equal.
pub fn export(document: &Document) -> Result<Vec<u8>, ExportError> {
  refuse_unwritable(document)?;

  let mut out = String::new();
  write_header(&mut out);
  write_layer_table(&mut out, document);
  write_entities(&mut out, document);
  pair(&mut out, 0, "EOF");
  Ok(out.into_bytes())
}

/// Refuse a document whose layers or geometry a DXF could not carry back unchanged.
///
/// Every refusal here is a fact the reader would silently drop: it skips a `LAYER` record with no
/// name, trims every value line it reads, and files entities under a name it has already seen, so an
/// empty, padded or repeated name would come back as a different document. A non-finite number has no
/// ASCII DXF spelling at all, and writing one would produce a file that only this writer can read.
fn refuse_unwritable(document: &Document) -> Result<(), ExportError> {
  let mut seen: Vec<&str> = Vec::with_capacity(document.layers.len());
  for layer in &document.layers {
    let name = layer.name.as_str();
    if name.trim().is_empty() {
      return Err(refused(
        "a layer has an empty name and a DXF layer record needs one: the reader skips a nameless \
         record, so the layer would vanish from the file",
      ));
    }
    if name.trim() != name {
      return Err(refused(format!(
        "the layer name {name:?} has leading or trailing whitespace, which the reader trims, so the \
         name would come back changed"
      )));
    }
    if name.chars().any(char::is_control) {
      return Err(refused(format!(
        "the layer name {name:?} carries a control character, which would break the group-code \
         stream"
      )));
    }
    if seen.contains(&name) {
      return Err(refused(format!(
        "two layers are named {name:?}: the layer table holds one record per name and the reader \
         files every entity under the first match"
      )));
    }
    seen.push(name);
  }

  for layer in &document.layers {
    for entity in &layer.entities {
      let EntityKind::Contour { contour } = &entity.kind;
      for vertex in contour.vertices() {
        let finite =
          vertex.point.x.is_finite() && vertex.point.y.is_finite() && vertex.bulge.is_finite();
        if !finite {
          return Err(refused(format!(
            "entity {:?} on layer {:?} carries a coordinate or a bulge that is not a finite \
             number, which a DXF cannot spell",
            entity.id, layer.name
          )));
        }
      }
    }
  }
  Ok(())
}

/// The `HEADER` section: the version, and millimetres.
fn write_header(out: &mut String) {
  section(out, "HEADER");
  pair(out, 9, "$ACADVER");
  pair(out, 1, ACAD_VERSION);
  pair(out, 9, "$INSUNITS");
  pair(out, 70, &INSUNITS_MILLIMETRES.to_string());
  end_section(out);
}

/// The `TABLES` section: one `LAYER` table, in document order.
fn write_layer_table(out: &mut String, document: &Document) {
  section(out, "TABLES");
  pair(out, 0, "TABLE");
  pair(out, 2, "LAYER");
  pair(out, 70, &document.layers.len().to_string());
  for layer in &document.layers {
    pair(out, 0, "LAYER");
    pair(out, 2, &layer.name);
    pair(out, 70, "0");
    pair(out, 62, &LAYER_COLOUR.to_string());
    pair(out, 6, LAYER_LINETYPE);
  }
  pair(out, 0, "ENDTAB");
  end_section(out);
}

/// The `ENTITIES` section: one `LWPOLYLINE` per contour, in layer order.
fn write_entities(out: &mut String, document: &Document) {
  section(out, "ENTITIES");
  for layer in &document.layers {
    for entity in &layer.entities {
      // The match has one arm and no wildcard on purpose: a second entity kind must be a decision
      // made here, not a contour silently written as something else.
      match &entity.kind {
        EntityKind::Contour { contour } => write_polyline(out, &layer.name, contour),
      }
    }
  }
  end_section(out);
}

/// One contour as a light polyline: its vertices, each with the bulge leaving it, and its closedness.
fn write_polyline(out: &mut String, layer: &str, contour: &Contour) {
  pair(out, 0, "LWPOLYLINE");
  pair(out, 8, layer);
  pair(out, 90, &contour.len().to_string());
  let closed = if contour.is_closed() {
    FLAG_CLOSED
  } else {
    FLAG_OPEN
  };
  pair(out, 70, &closed.to_string());
  for vertex in contour.vertices() {
    number(out, 10, vertex.point.x);
    number(out, 20, vertex.point.y);
    number(out, 42, vertex.bulge);
  }
}

// ---------------------------------------------------------------------------------------------
// The group-code stream.
// ---------------------------------------------------------------------------------------------

/// `0 / SECTION`, `2 / <name>`: how every section opens.
fn section(out: &mut String, name: &str) {
  pair(out, 0, "SECTION");
  pair(out, 2, name);
}

/// `0 / ENDSEC`: how every section closes.
fn end_section(out: &mut String) {
  pair(out, 0, "ENDSEC");
}

/// One group-code pair: the code line and the value line, one pair per two lines.
fn pair(out: &mut String, code: i32, value: &str) {
  out.push_str(&code.to_string());
  out.push('\n');
  out.push_str(value);
  out.push('\n');
}

/// One numeric group-code pair, in the shortest form that reads back as the same `f64`.
fn number(out: &mut String, code: i32, value: f64) {
  pair(out, code, &value.to_string());
}

/// A refusal in this module's format.
fn refused(reason: impl Into<String>) -> ExportError {
  ExportError::refused(FORMAT, reason)
}
