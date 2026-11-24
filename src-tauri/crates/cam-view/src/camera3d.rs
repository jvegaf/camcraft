//! The 3D preview camera: an orbit around a target, projected into a viewport (M5 A9).
//!
//! The projection is **pure math**. Nothing here paints: the shell takes [`Screen3`] coordinates and
//! draws them, which is what makes the 3D view testable headless (S2: "Testeable headless,
//! determinista") and keeps the `wgpu` decision open (D-M5-7).
//!
//! # The camera
//!
//! The eye sits on a sphere around [`Camera3d::target`]: `yaw` around the vertical axis, `pitch`
//! above the horizontal plane, `distance` outwards. A perspective projection is then built from the
//! basis `(right, up, forward)`; because that basis is derived from the world's vertical axis, there
//! is no up vector to invent when the camera looks straight down, which is exactly why `pitch` is
//! clamped short of vertical ([`PITCH_LIMIT`]).
//!
//! Screen space has its origin at the top-left with Y growing downwards, as the 2D camera in
//! `cam-app` does, so both projections agree about which way is up on the display.

use cam_app::Viewport;
use cam_move::Point3;
use cam_units::Length;

/// How far short of vertical `pitch` is held, in radians.
///
/// Roughly 0.06 degrees: visually indistinguishable from straight down, and enough that the camera's
/// right vector never collapses (the projection has no up vector to invent at the pole).
pub const PITCH_LIMIT: f64 = std::f64::consts::FRAC_PI_2 - 1.0e-3;

/// The nearest the eye may come to its target.
pub const MIN_DISTANCE: Length = Length::from_mm(1.0);

/// The farthest the eye may be pulled back: 100 m, enough for the largest sheet in millimetres.
pub const MAX_DISTANCE: Length = Length::from_mm(100_000.0);

/// The default orbit angle around the vertical axis, in radians.
///
/// About 43 degrees, the three-quarter view an operator expects: the front and the right side of the
/// stock both visible, so neither axis is foreshortened into invisibility.
pub const DEFAULT_YAW: f64 = 0.75;

/// The default elevation above the horizontal plane, in radians.
///
/// About 31 degrees: enough to read the heightfield's shape, shallow enough to keep it a side-on
/// drawing rather than a plan.
pub const DEFAULT_PITCH: f64 = 0.55;

/// The default orbit distance, in millimetres.
///
/// A starting point only: the caller frames its own stock by setting `distance` or calling
/// [`Camera3d::zoom`].
pub const DEFAULT_DISTANCE: Length = Length::from_mm(600.0);

/// The vertical field of view, in radians.
///
/// 45 degrees: a standard perspective, wide enough to frame a stock and narrow enough not to warp a
/// straight edge at the picture's border.
pub const FIELD_OF_VIEW: f64 = std::f64::consts::FRAC_PI_4;

/// Points closer than this to the eye, in millimetres, are not projected.
///
/// The perspective divide has no answer at the eye's own plane, so the near clip is where it starts
/// rather than an infinity or a sign flip.
pub const MIN_DEPTH: f64 = 1.0e-6;

/// A projected point in screen pixels, and how far in front of the eye it is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Screen3 {
  /// Pixels from the left edge of the viewport.
  pub x: f64,
  /// Pixels from the top edge of the viewport.
  pub y: f64,
  /// Distance along the view axis in millimetres, positive in front of the eye.
  ///
  /// Larger means farther away, so the painter can sort by it directly.
  pub depth: f64,
}

/// An orbiting 3D camera looking at a target through a viewport.
///
/// All three orbit fields are public because the shell binds them to a drag and a wheel, and a test
/// sets them without going through an event: the arithmetic is the contract, the gesture is not.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera3d {
  /// Angle around the vertical axis, in radians. Positive turns the eye counter-clockwise seen from
  /// above; it accumulates and is not wrapped.
  pub yaw: f64,
  /// Elevation above the horizontal plane, in radians, held within `±`[`PITCH_LIMIT`].
  pub pitch: f64,
  /// How far the eye is from the target.
  pub distance: Length,
  /// The point the camera looks at, in machine millimetres.
  pub target: Point3,
  /// Where the result is projected: a size in pixels, with no painter attached.
  pub viewport: Viewport,
}

impl Camera3d {
  /// A camera framing `target` in `viewport`, at the default three-quarter viewpoint.
  ///
  /// The distance is [`DEFAULT_DISTANCE`] and is not derived from the stock: `cam-view` does not know
  /// how big a job is, so framing is the caller's decision.
  pub fn new(target: Point3, viewport: Viewport) -> Self {
    Self {
      yaw: DEFAULT_YAW,
      pitch: DEFAULT_PITCH,
      distance: DEFAULT_DISTANCE,
      target,
      viewport,
    }
  }

  /// Project a machine-space point into the viewport, or `None` when it cannot be drawn.
  ///
  /// Returns `None` for a point behind the eye — a screen coordinate computed there would appear in
  /// front and be wrong — for a point at the eye's own plane (the perspective divide has no answer
  /// there), for non-finite input, and for a viewport with no area to project into.
  ///
  /// A `pitch` set past [`PITCH_LIMIT`] from outside is treated as the limit: the basis stays valid
  /// and the picture stays a picture.
  pub fn project(&self, point: Point3) -> Option<Screen3> {
    if self.viewport.is_empty() {
      return None;
    }
    let view = self.view()?;
    let (x, y, z) = (point.x.mm(), point.y.mm(), point.z.mm());
    if !x.is_finite() || !y.is_finite() || !z.is_finite() {
      return None;
    }

    let from_eye = [x - view.eye[0], y - view.eye[1], z - view.eye[2]];
    let depth = dot(from_eye, view.forward);
    if !depth.is_finite() || depth <= MIN_DEPTH {
      return None;
    }
    let focal = self.focal_pixels();
    let centre = self.viewport.center();
    let right = dot(from_eye, view.right);
    let up = dot(from_eye, view.up);
    Some(Screen3 {
      x: centre.x + focal * right / depth,
      y: centre.y - focal * up / depth,
      depth,
    })
  }

  /// Turn the eye around the target, and hold the pitch short of vertical.
  ///
  /// A non-finite delta is ignored rather than stored: one NaN would poison every later projection,
  /// and a drag gesture is not worth that risk.
  pub fn orbit(&mut self, delta_yaw: f64, delta_pitch: f64) {
    if !delta_yaw.is_finite() || !delta_pitch.is_finite() {
      return;
    }
    self.yaw += delta_yaw;
    self.pitch = (self.pitch + delta_pitch).clamp(-PITCH_LIMIT, PITCH_LIMIT);
  }

  /// Move the eye along its own axis by `factor`: above one moves closer, below one moves away.
  ///
  /// The distance is held within [`MIN_DISTANCE`]`..=`[`MAX_DISTANCE`], and a factor that is zero,
  /// negative or not finite is ignored, so a wheel event cannot invert or destroy the camera.
  pub fn zoom(&mut self, factor: f64) {
    if !factor.is_finite() || factor <= 0.0 {
      return;
    }
    let distance = self.distance.mm() / factor;
    self.distance = Length::from_mm(distance.max(MIN_DISTANCE.mm()).min(MAX_DISTANCE.mm()));
  }

  /// The length in pixels that one millimetre at the target's distance covers.
  ///
  /// Derived from the viewport's height and [`FIELD_OF_VIEW`]: the picture is vertical-field based, so
  /// a taller viewport shows more of the stock rather than stretching it.
  fn focal_pixels(&self) -> f64 {
    let half_height = self.viewport.height.max(0.0) / 2.0;
    half_height / (FIELD_OF_VIEW / 2.0).tan()
  }

  /// The camera's orthonormal basis and eye position, or `None` when the orbit is not finite.
  fn view(&self) -> Option<View> {
    let distance = self.distance.mm();
    if !self.yaw.is_finite() || !self.pitch.is_finite() || !distance.is_finite() {
      return None;
    }
    let pitch = self.pitch.clamp(-PITCH_LIMIT, PITCH_LIMIT);
    if distance <= 0.0 {
      return None;
    }
    let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
    let (sin_pitch, cos_pitch) = pitch.sin_cos();
    let (tx, ty, tz) = (self.target.x.mm(), self.target.y.mm(), self.target.z.mm());
    Some(View {
      eye: [
        tx + distance * cos_yaw * cos_pitch,
        ty + distance * sin_yaw * cos_pitch,
        tz + distance * sin_pitch,
      ],
      // World +Z is the reference up: right is forward crossed with it, up follows from right and
      // forward. Both stay unit length for every pitch inside the limit.
      right: [-sin_yaw, cos_yaw, 0.0],
      up: [-cos_yaw * sin_pitch, -sin_yaw * sin_pitch, cos_pitch],
      forward: [-cos_yaw * cos_pitch, -sin_yaw * cos_pitch, -sin_pitch],
    })
  }
}

/// The eye and its orthonormal basis, as plain arrays to keep the projection loop arithmetic.
struct View {
  eye: [f64; 3],
  right: [f64; 3],
  up: [f64; 3],
  forward: [f64; 3],
}

fn dot(left: [f64; 3], right: [f64; 3]) -> f64 {
  left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}
