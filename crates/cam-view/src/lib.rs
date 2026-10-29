//! Backend-agnostic 2D scene: **what** to paint, not how to paint it.
//!
//! The shell paints this list today with `egui`'s painter; a `wgpu` backend will consume the same
//! list when the 3D simulation needs the GPU (decision D-M1-2). Keeping the decision "what to draw"
//! here is what makes picking, bounds and grid testable without a display.
//!
//! Arcs stay arcs in the scene. Only the painter flattens them, so the picture keeps the same
//! geometry the toolpath engine will cut.
//!
//! Must not depend on `cam-ui`, `egui`, `wgpu` or any rendering crate.

#![warn(missing_docs)]

use cam_app::{Camera, Selection};
use cam_geom::{Arc, Bounds, LineSeg, Point2, Segment};
use cam_model::Document;

/// How an item should be drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
  /// Ordinary geometry.
  Normal,
  /// Geometry the user has selected.
  Selected,
  /// Reference geometry the user is drawing against.
  Construction,
  /// Grid and other background guides.
  Grid,
}

/// One drawable piece of the scene.
///
/// A contour becomes one item per segment, so a mixed contour keeps its straight parts straight and
/// its arcs curved, and picking can name the entity a click landed on.
#[derive(Clone, Debug, PartialEq)]
pub enum SceneItem {
  /// A straight run of points. Two points for a line segment.
  Polyline {
    /// The points, in world millimetres.
    points: Vec<Point2>,
    /// How to draw it.
    style: Style,
    /// The entity it belongs to, when it comes from the document.
    entity: Option<String>,
  },
  /// A circular arc, kept as parameters so the painter can flatten it at its own tolerance.
  Arc {
    /// Centre, in world millimetres.
    center: Point2,
    /// Radius in millimetres.
    radius: f64,
    /// Start angle in radians.
    start_angle: f64,
    /// Signed sweep in radians; positive is counter-clockwise.
    sweep: f64,
    /// How to draw it.
    style: Style,
    /// The entity it belongs to, when it comes from the document.
    entity: Option<String>,
  },
}

impl SceneItem {
  /// The style this item is drawn with.
  pub fn style(&self) -> Style {
    match self {
      Self::Polyline { style, .. } | Self::Arc { style, .. } => *style,
    }
  }

  /// The entity this item belongs to, if any.
  pub fn entity(&self) -> Option<&str> {
    match self {
      Self::Polyline { entity, .. } | Self::Arc { entity, .. } => entity.as_deref(),
    }
  }

  /// The geometry as a kernel segment, for bounds and distance queries.
  pub fn as_segment(&self) -> Option<Segment> {
    match self {
      Self::Polyline { points, .. } => {
        let (first, second) = (points.first()?, points.get(1)?);
        Some(Segment::Line(LineSeg::new(*first, *second)))
      }
      Self::Arc {
        center,
        radius,
        start_angle,
        sweep,
        ..
      } => Arc::new(*center, *radius, *start_angle, *sweep)
        .ok()
        .map(Segment::Arc),
    }
  }

  /// Bounds of this item.
  pub fn bounds(&self) -> Bounds {
    match self {
      Self::Polyline { points, .. } => Bounds::from_points(points),
      Self::Arc { .. } => self
        .as_segment()
        .map(|segment| segment.bounds())
        .unwrap_or_else(Bounds::empty),
    }
  }
}

/// Everything to draw, in order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scene {
  /// The items, back to front.
  pub items: Vec<SceneItem>,
}

impl Scene {
  /// Bounds of everything in the scene, empty when it holds nothing.
  pub fn bounds(&self) -> Bounds {
    self
      .items
      .iter()
      .fold(Bounds::empty(), |bounds, item| bounds.union(item.bounds()))
  }

  /// How many items the scene holds.
  pub fn len(&self) -> usize {
    self.items.len()
  }

  /// True when there is nothing to draw.
  pub fn is_empty(&self) -> bool {
    self.items.is_empty()
  }

  /// Add an item.
  pub fn push(&mut self, item: SceneItem) {
    self.items.push(item);
  }
}

/// Build the scene for a document.
///
/// Entities on hidden layers are left out entirely: hiding a layer must hide it from picking too, or
/// the user would select geometry they cannot see.
pub fn build_scene(document: &Document, selection: &Selection) -> Scene {
  let mut scene = Scene::default();
  for layer in &document.layers {
    if !layer.visible {
      continue;
    }
    for entity in &layer.entities {
      let Some(contour) = entity.kind.as_contour() else {
        continue;
      };
      let style = if selection.contains(&entity.id) {
        Style::Selected
      } else {
        Style::Normal
      };
      for segment in contour.segments() {
        scene.push(item_for(segment, style, Some(entity.id.clone())));
      }
    }
  }
  scene
}

/// Turn a kernel segment into a scene item.
fn item_for(segment: Segment, style: Style, entity: Option<String>) -> SceneItem {
  match segment {
    Segment::Line(line) => SceneItem::Polyline {
      points: vec![line.start, line.end],
      style,
      entity,
    },
    Segment::Arc(arc) => SceneItem::Arc {
      center: arc.center(),
      radius: arc.radius(),
      start_angle: arc.start_angle(),
      sweep: arc.sweep(),
      style,
      entity,
    },
  }
}

/// The screen spacing the grid aims for, in pixels.
pub const TARGET_GRID_PIXELS: f64 = 40.0;

/// Beyond this many lines the grid stops subdividing, so a deep zoom cannot lock the painter up.
pub const MAX_GRID_LINES: usize = 400;

/// The largest 1/2/5-style step whose on-screen size is at least `target_px`.
///
/// A grid whose spacing is not a round number is unreadable, so the spacing is always
/// `1`, `2` or `5` times a power of ten.
pub fn nice_spacing(pixels_per_mm: f64, target_px: f64) -> f64 {
  if !pixels_per_mm.is_finite() || pixels_per_mm <= 0.0 {
    return 1.0;
  }
  let wanted_mm = (target_px / pixels_per_mm).max(f64::MIN_POSITIVE);
  let magnitude = 10f64.powf(wanted_mm.log10().floor());
  let normalised = wanted_mm / magnitude;
  let step = if normalised <= 1.0 {
    1.0
  } else if normalised <= 2.0 {
    2.0
  } else if normalised <= 5.0 {
    5.0
  } else {
    10.0
  };
  step * magnitude
}

/// Build the background grid covering what the camera can see.
pub fn build_grid(camera: &Camera, target_px: f64) -> Scene {
  let mut scene = Scene::default();
  let viewport = camera.viewport();
  if viewport.is_empty() {
    return scene;
  }
  let spacing = nice_spacing(camera.pixels_per_mm(), target_px.max(8.0));
  if spacing <= 0.0 || !spacing.is_finite() {
    return scene;
  }

  let top_left = camera.screen_to_world(cam_app::ScreenPoint::new(0.0, 0.0));
  let bottom_right =
    camera.screen_to_world(cam_app::ScreenPoint::new(viewport.width, viewport.height));
  let min_x = top_left.x.min(bottom_right.x);
  let max_x = top_left.x.max(bottom_right.x);
  let min_y = top_left.y.min(bottom_right.y);
  let max_y = top_left.y.max(bottom_right.y);

  let first_x = (min_x / spacing).ceil() * spacing;
  let first_y = (min_y / spacing).ceil() * spacing;

  let mut count = 0;
  let mut x = first_x;
  while x <= max_x + spacing * 1.0e-9 && count < MAX_GRID_LINES {
    scene.push(SceneItem::Polyline {
      points: vec![Point2::new(x, min_y), Point2::new(x, max_y)],
      style: Style::Grid,
      entity: None,
    });
    x += spacing;
    count += 1;
  }
  let mut y = first_y;
  while y <= max_y + spacing * 1.0e-9 && count < MAX_GRID_LINES {
    scene.push(SceneItem::Polyline {
      points: vec![Point2::new(min_x, y), Point2::new(max_x, y)],
      style: Style::Grid,
      entity: None,
    });
    y += spacing;
    count += 1;
  }
  scene
}
