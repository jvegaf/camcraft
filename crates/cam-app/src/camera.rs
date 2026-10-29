//! The viewport camera: mapping between world millimetres and screen pixels.
//!
//! Screen space has its origin at the top-left with Y growing downwards, world space has Y growing
//! upwards. That flip lives here and nowhere else, so no other module has to remember it.
//!
//! The camera is pure math on purpose: pan, zoom and fit are the parts of the viewport that can be
//! wrong in subtle ways (zoom drifting under the cursor, fit clipping a job by a pixel), and this way
//! they are testable without a window.

use cam_geom::{Bounds, Point2};

/// Smallest allowed zoom: 0.01 pixels per millimetre, i.e. a 100 m job in a 1000 pixel window.
pub const MIN_PIXELS_PER_MM: f64 = 0.01;

/// Largest allowed zoom: 200 pixels per millimetre, i.e. 0.5 mm of detail across 100 pixels.
pub const MAX_PIXELS_PER_MM: f64 = 200.0;

/// A position in the window, in pixels, with the origin at the top-left.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScreenPoint {
  /// Pixels from the left edge.
  pub x: f64,
  /// Pixels from the top edge.
  pub y: f64,
}

impl ScreenPoint {
  /// Build a screen position.
  pub const fn new(x: f64, y: f64) -> Self {
    Self { x, y }
  }
}

/// Size of the drawing area, in pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
  /// Width in pixels.
  pub width: f64,
  /// Height in pixels.
  pub height: f64,
}

impl Viewport {
  /// Build a viewport size.
  pub const fn new(width: f64, height: f64) -> Self {
    Self { width, height }
  }

  /// The centre of the viewport in screen coordinates.
  pub fn center(&self) -> ScreenPoint {
    ScreenPoint::new(self.width / 2.0, self.height / 2.0)
  }

  /// True when the viewport has no area to draw into.
  pub fn is_empty(&self) -> bool {
    self.width <= 0.0 || self.height <= 0.0
  }
}

/// Where the viewport is looking, and how magnified.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
  center: Point2,
  pixels_per_mm: f64,
  viewport: Viewport,
}

impl Default for Camera {
  fn default() -> Self {
    Self {
      center: Point2::ORIGIN,
      pixels_per_mm: 1.0,
      viewport: Viewport::new(1024.0, 768.0),
    }
  }
}

impl Camera {
  /// Build a camera looking at `center` with the given magnification.
  ///
  /// The zoom is clamped to [`MIN_PIXELS_PER_MM`]..=[`MAX_PIXELS_PER_MM`]; a non-finite zoom falls
  /// back to 1 pixel per millimetre instead of poisoning every later transform.
  pub fn new(center: Point2, pixels_per_mm: f64, viewport: Viewport) -> Self {
    Self {
      center,
      pixels_per_mm: clamp_zoom(pixels_per_mm),
      viewport,
    }
  }

  /// The world point currently at the centre of the viewport.
  pub fn center(&self) -> Point2 {
    self.center
  }

  /// Current magnification.
  pub fn pixels_per_mm(&self) -> f64 {
    self.pixels_per_mm
  }

  /// Current viewport size.
  pub fn viewport(&self) -> Viewport {
    self.viewport
  }

  /// Resize the viewport, keeping the centre and the zoom.
  pub fn set_viewport(&mut self, viewport: Viewport) {
    if !viewport.is_empty() {
      self.viewport = viewport;
    }
  }

  /// World millimetres to screen pixels.
  pub fn world_to_screen(&self, world: Point2) -> ScreenPoint {
    let centre = self.viewport.center();
    ScreenPoint::new(
      (world.x - self.center.x) * self.pixels_per_mm + centre.x,
      centre.y - (world.y - self.center.y) * self.pixels_per_mm,
    )
  }

  /// Screen pixels to world millimetres.
  pub fn screen_to_world(&self, screen: ScreenPoint) -> Point2 {
    let centre = self.viewport.center();
    Point2::new(
      (screen.x - centre.x) / self.pixels_per_mm + self.center.x,
      (centre.y - screen.y) / self.pixels_per_mm + self.center.y,
    )
  }

  /// Move the view by a drag of `(dx, dy)` pixels: the content follows the cursor.
  pub fn pan_by_pixels(&mut self, dx: f64, dy: f64) {
    self.center = Point2::new(
      self.center.x - dx / self.pixels_per_mm,
      self.center.y + dy / self.pixels_per_mm,
    );
  }

  /// Multiply the zoom by `factor` while keeping the world point under `anchor` fixed.
  ///
  /// A zoom that ignores the anchor makes the drawing slide away from the cursor, which is the
  /// difference between a viewport that feels right and one that fights the user.
  pub fn zoom_at(&mut self, anchor: ScreenPoint, factor: f64) {
    if !factor.is_finite() || factor <= 0.0 {
      return;
    }
    let world_under_anchor = self.screen_to_world(anchor);
    let zoom = clamp_zoom(self.pixels_per_mm * factor);
    if zoom == self.pixels_per_mm {
      return;
    }
    self.pixels_per_mm = zoom;
    let centre = self.viewport.center();
    self.center = Point2::new(
      world_under_anchor.x - (anchor.x - centre.x) / zoom,
      world_under_anchor.y + (anchor.y - centre.y) / zoom,
    );
  }

  /// Frame `bounds` inside the viewport, leaving `margin_px` pixels of padding.
  pub fn zoom_to_fit(&mut self, bounds: Bounds, margin_px: f64) {
    if bounds.is_empty() || self.viewport.is_empty() {
      return;
    }
    let margin = margin_px.max(0.0);
    let available_width = (self.viewport.width - 2.0 * margin).max(1.0);
    let available_height = (self.viewport.height - 2.0 * margin).max(1.0);
    let width = bounds.width().max(f64::MIN_POSITIVE);
    let height = bounds.height().max(f64::MIN_POSITIVE);
    let zoom = clamp_zoom((available_width / width).min(available_height / height));
    self.pixels_per_mm = zoom;
    self.center = bounds.center();
  }

  /// Screen pixels covered by one world millimetre, clamped.
  pub fn clamp_zoom_value(pixels_per_mm: f64) -> f64 {
    clamp_zoom(pixels_per_mm)
  }
}

/// Clamp a zoom into the supported range, replacing non-finite input with a usable default.
fn clamp_zoom(pixels_per_mm: f64) -> f64 {
  if !pixels_per_mm.is_finite() || pixels_per_mm <= 0.0 {
    return 1.0;
  }
  pixels_per_mm.clamp(MIN_PIXELS_PER_MM, MAX_PIXELS_PER_MM)
}
