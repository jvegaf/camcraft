//! SVG export: the path data and the millimetre page that `cam-import`'s SVG reader reads back.
//!
//! # What is written, and why in this shape
//!
//! * One `<path>` per contour, in layer order and then in entity order. The path opens with `M` on
//!   the contour's first vertex, then carries one command per segment: `L x y` for a straight one
//!   and `A rx ry rot large-arc sweep x y` for an arc.
//! * `rx == ry` and `rot == 0`, always. The reader keeps an `A` exact only when the radii are equal
//!   and the rotation vanishes (`crates/cam-import/src/svg.rs:1674`); any other `A` is flattened
//!   into a chain of straight segments, which would lose the arc the document holds. The radius is
//!   the arc's own, `chord·(1 + bulge²)/(4·|bulge|)`, taken from the kernel rather than
//!   re-derived here.
//! * A closed contour keeps its closing segment: `Z` closes a subpath with a *straight* line, so
//!   when the closing segment is an arc it is written as an explicit `A` back to the first vertex
//!   and `Z` follows (drawing nothing) to say the subpath is closed. When the closing segment is
//!   straight the `Z` alone is the segment and the first vertex is not repeated — repeating it
//!   would hand the model a phantom zero-length segment
//!   (`crates/cam-geom/src/contour.rs:34`).
//! * A full circle is therefore one path with two `A` commands. That is the only faithful form: the
//!   model holds a circle as two vertices at bulge 1.0, closed (`crates/cam-geom/src/primitive.rs:17`),
//!   so both its segments are half turns, and a single `Z` would silently turn the second half into
//!   a chord.
//! * The page is the document's own bounds in millimetres: `width` and `height` are the bounds'
//!   width and height with an `mm` suffix, and the `viewBox` is the same box with the same numbers,
//!   so the reader's scale is `width / viewBox width` — exactly `1.0`, no factor of any kind. The
//!   bounds are composed from the exact per-segment bounds rather than from a tessellation, so an
//!   arc's extreme point is inside the page and not a fraction of a millimetre outside it.
//!
//! # The Y axis, and why the two reflections compose to the identity
//!
//! SVG user space is y-down and millimetres are y-up, so the file is written in a reflected frame:
//! a document point `(x, y)` is written at `(x, 2·cy - y)` where `cy` is the centre of the page
//! box, `(min_y + max_y) / 2`. That is a reflection about the page's own horizontal centre line, so
//! every point of the drawing stays inside the `viewBox` it declares.
//!
//! The reader reflects the other way, about the top edge of the page it derives from that same box
//! (`crates/cam-import/src/svg.rs:111`, `tests/svg.rs:440`): `y_out = top - y_user`, with
//! `top = min_y + height`. Substituting the writer's `y_user = min.y + top - y_doc` gives
//! `y_out = top - (top - y_doc) = y_doc`. The two reflections are the same reflection, so they
//! cancel. What is left is the reader's own placement — it always translates the drawing by the
//! `viewBox` origin — which is the translation S4 compares the round trip up to, never the writer's
//! doing.
//!
//! A reflection is an isometry that reverses orientation, so it also reverses every turn: the
//! reader negates each bulge it reads from a reflected file (`svg.rs:1010`). The path therefore
//! writes the *user-space* bulge, `-bulge_document`, and the sweep flag is the reader's own rule
//! that `sweep = 4·atan(bulge)` is positive exactly when the flag is `1`. A counter-clockwise
//! document arc (positive bulge) is clockwise in user space and writes sweep flag `0`; the reader
//! negates it once more on the way back and the bulge leaves and returns unchanged.
//!
//! # What an SVG cannot carry
//!
//! An SVG has no layers: the reader files every contour under one layer named `svg`
//! (`svg.rs:120-123`, `tests/svg.rs:26`), so no layer name is written and none is pretended to be
//! preserved. The document has one entity kind, so a contour leaves as a path and never as a
//! `<circle>` or `<rect>`, an ellipse's axis ratio is already gone by the time geometry reaches the
//! model, and toolpaths are not geometry: they are not written, exactly as in the DXF writer.

use std::f64::consts::PI;

use cam_geom::{Bounds, Contour, Point2};
use cam_model::{Document, EntityKind};

use crate::ExportError;

/// The format name every error in this module carries.
const FORMAT: &str = "svg";

/// The namespace that makes the file an SVG rather than an XML document that happens to look like
/// one.
const NAMESPACE: &str = "http://www.w3.org/2000/svg";

/// The smallest page extent, in millimetres, along an axis the geometry does not span.
///
/// An SVG page needs a positive width and height — the reader refuses a `viewBox` whose width or
/// height is not greater than zero — so a straight line, which has no height, still needs a page to
/// be drawn on. The box is widened symmetrically, so the geometry stays in the middle of it.
const MINIMUM_PAGE_EXTENT: f64 = 1.0;

/// The `large-arc` flag of an arc wider than half a turn.
const LARGE_ARC: bool = true;

/// The `large-arc` flag of an arc no wider than half a turn.
const MINOR_ARC: bool = false;

/// Export `document` as SVG.
///
/// The bytes depend on the document and on nothing else: layers in document order, contours in
/// entity order, one `<path>` each, and numbers in their shortest round-trip form, so two exports of
/// one document are equal.
pub fn export(document: &Document) -> Result<Vec<u8>, ExportError> {
  refuse_unwritable(document)?;
  let page = Page::of(document)?;

  let mut out = String::new();
  out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
  out.push_str(&format!(
    "<svg xmlns=\"{NAMESPACE}\" width=\"{}mm\" height=\"{}mm\" viewBox=\"{} {} {} {}\">\n",
    number(page.width),
    number(page.height),
    number(page.min.x),
    number(page.min.y),
    number(page.width),
    number(page.height),
  ));
  for layer in &document.layers {
    for entity in &layer.entities {
      // The match has one arm and no wildcard on purpose: a second entity kind must be a decision
      // made here, not a contour silently written as something else.
      match &entity.kind {
        EntityKind::Contour { contour } => write_path(&mut out, contour, &page),
      }
    }
  }
  out.push_str("</svg>\n");
  Ok(out.into_bytes())
}

/// Refuse a document whose geometry the file could not carry back unchanged.
///
/// There is nothing to refuse about a layer name: an SVG has no layer record, so the name is
/// dropped by the format and named as a loss in this module's documentation rather than silently
/// mangled into a group. A non-finite coordinate has no SVG spelling at all, and it would take the
/// page's bounds with it, so it is refused by name.
fn refuse_unwritable(document: &Document) -> Result<(), ExportError> {
  for layer in &document.layers {
    for entity in &layer.entities {
      let EntityKind::Contour { contour } = &entity.kind;
      for vertex in contour.vertices() {
        let finite =
          vertex.point.x.is_finite() && vertex.point.y.is_finite() && vertex.bulge.is_finite();
        if !finite {
          return Err(refused(format!(
            "entity {:?} on layer {:?} carries a coordinate or a bulge that is not a finite \
             number, which an SVG cannot spell",
            entity.id, layer.id
          )));
        }
      }
    }
  }
  Ok(())
}

// ---------------------------------------------------------------------------------------------
// The page.
// ---------------------------------------------------------------------------------------------

/// The page an export draws on: the document's own bounds, in millimetres.
#[derive(Clone, Copy, Debug)]
struct Page {
  /// Lower-left corner of the page, in document millimetres.
  min: Point2,
  /// Extent along X, in millimetres. Never zero.
  width: f64,
  /// Extent along Y, in millimetres. Never zero.
  height: f64,
}

impl Page {
  /// The page a document needs: its bounds, widened where it has no extent along one axis.
  fn of(document: &Document) -> Result<Self, ExportError> {
    let bounds = geometry_bounds(document);
    if bounds.is_empty() {
      return Err(refused(
        "the document has no geometry, so an svg has no page: its width, height and viewBox are \
         the drawing's own bounds",
      ));
    }
    let mut min = bounds.min;
    let mut width = bounds.width();
    let mut height = bounds.height();
    if width <= 0.0 {
      min.x -= MINIMUM_PAGE_EXTENT / 2.0;
      width = MINIMUM_PAGE_EXTENT;
    }
    if height <= 0.0 {
      min.y -= MINIMUM_PAGE_EXTENT / 2.0;
      height = MINIMUM_PAGE_EXTENT;
    }
    Ok(Self { min, width, height })
  }

  /// The top edge of the page, in document millimetres.
  fn top(self) -> f64 {
    self.min.y + self.height
  }

  /// A document point in the file's y-down user space.
  ///
  /// The reflection is about the horizontal centre line of the page, which is the one the reader
  /// inverts (see the module documentation).
  fn place(self, point: Point2) -> Point2 {
    Point2::new(point.x, self.min.y + self.top() - point.y)
  }
}

/// Every segment's exact bounds, unioned: what the drawing occupies.
///
/// It is composed from [`cam_geom::Segment::bounds`], which is exact for an arc (endpoints plus the
/// axis extremes the sweep passes through), rather than from a tessellation of the contour, which
/// would leave an arc's outermost point a tolerance outside the page.
fn geometry_bounds(document: &Document) -> Bounds {
  let mut bounds = Bounds::empty();
  for layer in &document.layers {
    for entity in &layer.entities {
      let EntityKind::Contour { contour } = &entity.kind;
      for segment in contour.segments() {
        let segment = segment.bounds();
        bounds = bounds.include(segment.min).include(segment.max);
      }
    }
  }
  bounds
}

// ---------------------------------------------------------------------------------------------
// One contour, as path data.
// ---------------------------------------------------------------------------------------------

/// One contour as a `<path>`: a move, one command per segment, and `Z` when it closes.
///
/// Coordinates come from the contour's vertices and never from a recomputed arc endpoint: the
/// kernel derives an arc's endpoints from its centre and radius, and those differ from the authored
/// vertex in the last bits. The file carries the authored numbers.
fn write_path(out: &mut String, contour: &Contour, page: &Page) {
  let segments = contour.segments();
  if segments.is_empty() {
    // A default-constructed contour holds no vertex and therefore no segment: there is no geometry
    // to draw and none to lose.
    return;
  }
  let first = contour.vertices()[0].point;
  let start = page.place(first);
  out.push_str(&format!(
    "  <path d=\"M {} {}",
    number(start.x),
    number(start.y)
  ));

  for (index, segment) in segments.iter().enumerate() {
    let to = match contour.vertices().get(index + 1) {
      Some(vertex) => vertex.point,
      // Only a closed contour has a segment past its last vertex, and that segment ends where the
      // contour started.
      None => first,
    };
    let closing = contour.is_closed() && index + 1 == segments.len();
    match segment.as_arc() {
      // A straight closing segment is what `Z` draws, so writing `L` for it and then `Z` would
      // hand the reader the first vertex twice.
      None if closing => {}
      None => {
        let to = page.place(to);
        out.push_str(&format!(" L {} {}", number(to.x), number(to.y)));
      }
      Some(arc) => {
        let to = page.place(to);
        out.push_str(&format!(
          " A {radius} {radius} 0 {large} {sweep} {x} {y}",
          radius = number(arc.radius()),
          large = flag(if arc.sweep().abs() > PI {
            LARGE_ARC
          } else {
            MINOR_ARC
          }),
          // The file is reflected, so the arc turns the other way in user space; the reader takes a
          // positive `sweep = 4·atan(bulge)` exactly when this flag is 1.
          sweep = flag(arc.sweep() < 0.0),
          x = number(to.x),
          y = number(to.y),
        ));
      }
    }
  }

  if contour.is_closed() {
    out.push_str(" Z");
  }
  out.push_str("\"/>\n");
}

// ---------------------------------------------------------------------------------------------
// The text.
// ---------------------------------------------------------------------------------------------

/// One number, in the shortest form that reads back as the same `f64`.
fn number(value: f64) -> String {
  value.to_string()
}

/// An SVG flag: the single character `0` or `1`, which is all the grammar allows.
fn flag(value: bool) -> char {
  if value {
    '1'
  } else {
    '0'
  }
}

/// A refusal in this module's format.
fn refused(reason: impl Into<String>) -> ExportError {
  ExportError::refused(FORMAT, reason)
}
