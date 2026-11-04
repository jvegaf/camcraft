//! Moulding rail geometry: arc-length parametrisation, lateral offsets, and the certified band a
//! cross-section sweeps along a rail (M8 T2, T3; S1–S3).
//!
//! S3 fixes the reading this module is built on: the **rail** is an XY path (the selected entity), the
//! **profile** is authored data — a cross-section of `(lateral, depth)` points — and the **stepover**
//! spaces the overlapped lateral passes. Nothing here knows about Z or tools: the cross-section's depth
//! is carried as a number so the geometry can interpolate it, but the band is the cross-section's planar
//! shadow, and the depth becomes a Z only in `cam-toolpath`.
//!
//! # Arc length, and arcs
//!
//! [`Rail`] stores the contour's own segments — never a tessellation — and maps a distance along the
//! rail to the point, tangent and normal at that distance with the segment's exact equation. A point
//! asked for on an arc is on the arc to the last bit; a chord-based answer would miss it by the
//! sagitta of a step, which is what makes this module able to *certify* a band instead of approximating
//! one. Tessellation happens only in [`Rail::samples`], where the caller asks for a walk.
//!
//! Lateral is measured **to the right of travel** ([`Rail::normal_at`] is the tangent turned clockwise),
//! so a rail running along `+X` has its positive lateral side towards `-Y`, and a counter-clockwise
//! closed rail has its positive lateral side outside the loop. The sign is a decision, not a detail: it
//! is the same one [`CrossSection`]'s own point order uses, and the one `cam-toolpath` offsets each pass
//! by.
//!
//! # The stepover law's home
//!
//! T3 lives here rather than in `cam-toolpath` because it is lateral-offset geometry: the passes are
//! points on the cross-section's own lateral axis, and whether they cover the span is a property of
//! that axis alone. A strategy that repeated the law would be a second implementation of the same
//! certificate, and the two could disagree.

use std::f64::consts::FRAC_PI_2;

use crate::contour::Contour;
use crate::point::{Point2, Vec2};
use crate::segment::{LineSeg, Segment};
use crate::GeomError;

/// How far a sampled offset curve may sit from the true curve between two samples, in millimetres.
///
/// The same order as the offset kernel's chord tolerance: fine enough that a moulding's chords are
/// invisible against the machine's own resolution, coarse enough that a curved rail does not become a
/// program of thousands of moves.
pub const DEFAULT_CHORD_TOLERANCE: f64 = 0.001;

/// How far apart two offset points must be before the edge between them is a real edge, in millimetres.
///
/// A closed rail's offset walk ends on the point where the rail closes. At a seam in the middle of an
/// edge the two normals there are collinear, so the two offset points are the same point up to the noise
/// of the arithmetic that produced them; at a seam on a **corner** they are genuinely apart, and the
/// edge between them is the corner's diagonal, which is material the program removes. This is the only
/// number that tells the two cases apart, and [`Rail::offset_walk`] is the only place it is read.
const SEAM_EPSILON: f64 = 1.0e-9;

/// One point of a moulding cross-section, in millimetres.
///
/// The kernel's own form of `cam_model::MouldingProfilePoint`: `lateral` is the distance from the
/// rail, positive to the right of travel, and `depth` is the cut depth there, negative below the stock
/// surface. `cam-geom` must not depend on `cam-model`, so the strategy maps one onto the other.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CrossSectionPoint {
  /// Lateral distance from the rail, in millimetres. Negative is the other side of the rail.
  pub lateral: f64,
  /// Cut depth at this lateral distance, in millimetres: zero at the surface, negative below.
  pub depth: f64,
}

/// A moulding cross-section: one depth for each lateral distance from the rail.
///
/// The points are ordered from one side to the other with strictly increasing laterals, which is what
/// makes the cross-section a **function** a pass can read its depth from. A profile that doubles back
/// would be two depths for one lateral distance, and [`CrossSection::new`] refuses it by name instead
/// of choosing one.
#[derive(Clone, Debug, PartialEq)]
pub struct CrossSection {
  points: Vec<CrossSectionPoint>,
}

impl CrossSection {
  /// Build a cross-section, validating it as a depth-per-lateral function.
  ///
  /// # Errors
  ///
  /// [`GeomError::MouldingProfileTooShort`] for fewer than two points,
  /// [`GeomError::MouldingProfilePointNotFinite`] for a coordinate that is not a finite number of
  /// millimetres, and [`GeomError::MouldingProfileLateralOrder`] for a lateral that does not exceed
  /// the previous one. Each refusal names the values that produced it.
  pub fn new(points: Vec<CrossSectionPoint>) -> Result<Self, GeomError> {
    if points.len() < 2 {
      return Err(GeomError::MouldingProfileTooShort {
        points: points.len(),
      });
    }
    let mut previous: Option<CrossSectionPoint> = None;
    for (index, point) in points.iter().enumerate() {
      if !point.lateral.is_finite() || !point.depth.is_finite() {
        return Err(GeomError::MouldingProfilePointNotFinite {
          lateral: point.lateral,
          depth: point.depth,
        });
      }
      if let Some(previous) = previous {
        if point.lateral <= previous.lateral {
          return Err(GeomError::MouldingProfileLateralOrder {
            index,
            previous: previous.lateral,
            lateral: point.lateral,
          });
        }
      }
      previous = Some(*point);
    }
    Ok(Self { points })
  }

  /// The cross-section's points, in lateral order.
  pub fn points(&self) -> &[CrossSectionPoint] {
    &self.points
  }

  /// The lateral distance of the first point.
  pub fn lateral_min(&self) -> f64 {
    self.points.first().map_or(0.0, |point| point.lateral)
  }

  /// The lateral distance of the last point.
  pub fn lateral_max(&self) -> f64 {
    self.points.last().map_or(0.0, |point| point.lateral)
  }

  /// The lateral span the cross-section covers, in millimetres.
  pub fn width(&self) -> f64 {
    self.lateral_max() - self.lateral_min()
  }

  /// The depth the cross-section authors at `lateral`, by linear interpolation between its own points.
  ///
  /// The profile's own points are exact; outside the authored span the edge depth is kept rather than
  /// extrapolating a slope the document never wrote.
  pub fn depth_at(&self, lateral: f64) -> f64 {
    if !lateral.is_finite() {
      return f64::NAN;
    }
    let first = match self.points.first() {
      Some(first) => *first,
      None => return f64::NAN,
    };
    if lateral <= first.lateral {
      return first.depth;
    }
    for window in self.points.windows(2) {
      let (from, to) = (window[0], window[1]);
      if lateral <= to.lateral {
        let span = to.lateral - from.lateral;
        let fraction = (lateral - from.lateral) / span;
        return from.depth + (to.depth - from.depth) * fraction;
      }
    }
    self.points.last().map_or(f64::NAN, |point| point.depth)
  }
}

/// One point of a walked rail: where it is, and which side is its positive lateral side.
///
/// The walk keeps one sample per segment end, so a corner appears twice — once arriving and once
/// leaving — because a corner really does have two normals. Averaging them would invent a direction the
/// rail never has and cut across the corner.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RailSample {
  /// Distance along the rail, in millimetres.
  pub distance: f64,
  /// The point on the rail at that distance.
  pub point: Point2,
  /// The unit normal on the positive lateral side of travel.
  pub normal: Vec2,
}

impl RailSample {
  /// The point `lateral` millimetres to the right of this sample.
  pub fn offset(&self, lateral: f64) -> Point2 {
    self.point + self.normal * lateral
  }
}

/// A rail: a contour walked by arc length, with its arcs kept as arcs.
#[derive(Clone, Debug, PartialEq)]
pub struct Rail {
  segments: Vec<Segment>,
  /// Arc length at the start of each segment.
  starts: Vec<f64>,
  length: f64,
  closed: bool,
}

impl Rail {
  /// Build a rail from a contour, refusing one with no length to sweep along.
  ///
  /// # Errors
  ///
  /// [`GeomError::MouldingRailTooShort`] when the contour has no segments or a perimeter of zero: a
  /// rail with no length has no sweep behind it, and answering a point on it would be an invention.
  pub fn new(contour: &Contour) -> Result<Self, GeomError> {
    let segments = contour.segments();
    let mut starts = Vec::with_capacity(segments.len());
    let mut length = 0.0;
    for segment in &segments {
      starts.push(length);
      length += segment.length();
    }
    if segments.is_empty() || length <= 0.0 || !length.is_finite() {
      return Err(GeomError::MouldingRailTooShort { length });
    }
    Ok(Self {
      segments,
      starts,
      length,
      closed: contour.is_closed(),
    })
  }

  /// Total arc length of the rail, in millimetres.
  pub fn length(&self) -> f64 {
    self.length
  }

  /// True when the rail closes on itself.
  pub fn is_closed(&self) -> bool {
    self.closed
  }

  /// The point at `distance` along the rail, from the segment's exact equation.
  ///
  /// A closed rail wraps its distance modulo its own length; an open rail clamps to its ends.
  pub fn point_at(&self, distance: f64) -> Point2 {
    let (index, fraction) = self.locate(distance);
    match self.segments[index] {
      Segment::Line(line) => line.point_at(fraction),
      Segment::Arc(arc) => arc.point_at_fraction(fraction),
    }
  }

  /// The unit tangent at `distance`, in the direction of travel.
  ///
  /// At a vertex the outgoing segment's tangent is the answer: that is the direction the rail continues
  /// in, and it is the one a pass leaves the vertex along.
  pub fn tangent_at(&self, distance: f64) -> Vec2 {
    let (index, fraction) = self.locate(distance);
    self.segment_tangent(index, fraction)
  }

  /// The unit normal at `distance`, on the positive lateral side of travel.
  ///
  /// Positive lateral is the **right** of travel, so this is the tangent turned clockwise.
  pub fn normal_at(&self, distance: f64) -> Vec2 {
    self.tangent_at(distance).perp_cw()
  }

  /// The point `lateral` millimetres to the right of the rail at `distance`.
  pub fn offset_at(&self, distance: f64, lateral: f64) -> Point2 {
    self.point_at(distance) + self.normal_at(distance) * lateral
  }

  /// Walk the rail into samples, chording an arc to within `chord_tolerance` millimetres.
  ///
  /// Every segment end is a sample, so the walk is in arc-length order and a closed rail includes the
  /// point where it closes. The samples are the only place this module tessellates anything, and the
  /// tessellation is of the offset *walk*, never of an answer [`Rail::point_at`] gives.
  ///
  /// # Errors
  ///
  /// [`GeomError::MouldingToleranceNotPositive`] when `chord_tolerance` is not a positive finite number
  /// of millimetres.
  pub fn samples(&self, chord_tolerance: f64) -> Result<Vec<RailSample>, GeomError> {
    if !chord_tolerance.is_finite() || chord_tolerance <= 0.0 {
      return Err(GeomError::MouldingToleranceNotPositive {
        tolerance: chord_tolerance,
      });
    }
    let mut samples = Vec::new();
    for (index, segment) in self.segments.iter().enumerate() {
      let steps = match segment {
        Segment::Line(_) => 1,
        Segment::Arc(arc) => arc.step_count(chord_tolerance),
      };
      for step in 0..=steps {
        let fraction = step as f64 / steps as f64;
        let distance = self.starts[index] + segment.length() * fraction;
        samples.push(RailSample {
          distance,
          point: self.point_at_on(index, fraction),
          normal: self.segment_tangent(index, fraction).perp_cw(),
        });
      }
    }
    Ok(samples)
  }

  /// The offset polyline that a pass at `lateral` cuts: one point per sample, and the closing edge a
  /// closed rail's seam needs when that seam sits on a corner.
  ///
  /// This is **the** walk of a pass. `cam-toolpath::moulding` emits it as its feeds and
  /// `cam_sim::target::claim_moulding` claims against it, and it is one function for exactly that
  /// reason: a claim measuring a different polyline from the program's would report a correct program as
  /// `gouge`. A closed rail's walk ends on the point where the rail closes, carrying the *closing*
  /// segment's normal ([`Rail::samples`]), so the last offset point and the first coincide — in the
  /// middle of an edge, because the two tangents there are collinear — or they do not, and then the edge
  /// between them is a cut the program makes and the claim must authorise. An open rail closes nothing.
  ///
  /// The polyline closes by itself where [`SEAM_EPSILON`] says the two ends are one point, so appending
  /// the first point again would add a zero-length edge rather than a cut.
  pub fn offset_walk(&self, samples: &[RailSample], lateral: f64) -> Vec<Point2> {
    let mut walk: Vec<Point2> = samples
      .iter()
      .map(|sample| sample.offset(lateral))
      .collect();
    if self.closed {
      if let (Some(first), Some(last)) = (walk.first().copied(), walk.last().copied()) {
        if last.distance_to(first) > SEAM_EPSILON {
          walk.push(first);
        }
      }
    }
    walk
  }

  /// The segment index and the fraction along it that `distance` names.
  fn locate(&self, distance: f64) -> (usize, f64) {
    let distance = if !distance.is_finite() {
      0.0
    } else if self.closed {
      distance.rem_euclid(self.length)
    } else {
      distance.clamp(0.0, self.length)
    };
    let index = match self.starts.binary_search_by(|start| {
      start
        .partial_cmp(&distance)
        .unwrap_or(std::cmp::Ordering::Equal)
    }) {
      Ok(index) => index,
      Err(0) => 0,
      Err(index) => index - 1,
    };
    let segment_length = self.segments[index].length();
    let fraction = if segment_length > 0.0 {
      ((distance - self.starts[index]) / segment_length).clamp(0.0, 1.0)
    } else {
      0.0
    };
    (index, fraction)
  }

  /// A point on one named segment, without locating a distance first.
  ///
  /// The walk needs this because the last sample of a closed rail sits at the rail's full length, which
  /// [`Rail::locate`] wraps back to the first segment: the point is the same, but the **normal** is the
  /// closing segment's, and that is the one a corner at the seam needs.
  fn point_at_on(&self, index: usize, fraction: f64) -> Point2 {
    match self.segments[index] {
      Segment::Line(line) => line.point_at(fraction),
      Segment::Arc(arc) => arc.point_at_fraction(fraction),
    }
  }

  /// The unit tangent of one named segment at a fraction along it.
  fn segment_tangent(&self, index: usize, fraction: f64) -> Vec2 {
    match self.segments[index] {
      Segment::Line(line) => line.direction().unwrap_or(Vec2::X_AXIS),
      Segment::Arc(arc) => {
        let angle = arc.start_angle() + arc.sweep() * fraction;
        let turn = if arc.is_ccw() { FRAC_PI_2 } else { -FRAC_PI_2 };
        Vec2::from_angle(angle + turn, 1.0)
      }
    }
  }
}

/// One cross-section of a swept band: the rail point, its normal, and the band's two edges there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BandSample {
  /// Distance along the rail, in millimetres.
  pub distance: f64,
  /// The point on the rail.
  pub point: Point2,
  /// The unit normal on the positive lateral side of travel.
  pub normal: Vec2,
  /// The band's edge at the cross-section's minimum lateral distance.
  pub inner: Point2,
  /// The band's edge at the cross-section's maximum lateral distance.
  pub outer: Point2,
}

impl BandSample {
  /// The point `lateral` millimetres to the right of the rail at this sample.
  ///
  /// This is the rail's own offset family at the sample, which is exactly what the band must contain:
  /// the certificate is that it lies on the segment from [`BandSample::inner`] to
  /// [`BandSample::outer`].
  pub fn offset(&self, lateral: f64) -> Point2 {
    self.point + self.normal * lateral
  }
}

/// The planar shadow of a cross-section swept along a rail.
///
/// The band is bounded by the rail's offset curves at the cross-section's two extreme lateral
/// distances, sampled along the rail. Its width is the profile's width at every sample, and between
/// samples it chords the offset curves to the declared tolerance — which is what the coverage tests
/// certify.
#[derive(Clone, Debug, PartialEq)]
pub struct SweptBand {
  samples: Vec<BandSample>,
  width: f64,
  closed: bool,
}

impl SweptBand {
  /// Sweep `cross` along `rail`, sampling the band to `chord_tolerance` millimetres.
  ///
  /// # Errors
  ///
  /// [`GeomError::MouldingToleranceNotPositive`] when `chord_tolerance` is not a positive finite number
  /// of millimetres, and whatever [`Rail::samples`] refuses.
  pub fn sweep(rail: &Rail, cross: &CrossSection, chord_tolerance: f64) -> Result<Self, GeomError> {
    let (lateral_min, lateral_max) = (cross.lateral_min(), cross.lateral_max());
    let samples = rail
      .samples(chord_tolerance)?
      .into_iter()
      .map(|sample| BandSample {
        distance: sample.distance,
        point: sample.point,
        normal: sample.normal,
        inner: sample.offset(lateral_min),
        outer: sample.offset(lateral_max),
      })
      .collect();
    Ok(Self {
      samples,
      width: cross.width(),
      closed: rail.is_closed(),
    })
  }

  /// The band's samples, in arc-length order.
  pub fn samples(&self) -> &[BandSample] {
    &self.samples
  }

  /// The band's width in millimetres: the cross-section's lateral span.
  pub fn width(&self) -> f64 {
    self.width
  }

  /// True when the rail it was swept along is closed.
  pub fn is_closed(&self) -> bool {
    self.closed
  }

  /// True when `point` lies inside the band, or within `tolerance` millimetres of its boundary.
  ///
  /// The boundary is the band's two offset curves joined at the rail's ends — and at the seam of a
  /// closed rail — so this is the envelope's own membership test. It is what "the band covers the
  /// rail's offset family" means as a predicate: a point of that family a band rejects is a part of
  /// the profile's shadow the sweep does not cover, and the tolerance is the chord error the walk
  /// declared.
  pub fn contains(&self, point: Point2, tolerance: f64) -> bool {
    let boundary = self.boundary_points();
    if boundary.len() < 3 {
      return false;
    }
    if tolerance > 0.0 && distance_to_polyline(point, &boundary) <= tolerance {
      return true;
    }
    point_in_polygon(point, &boundary)
  }

  /// The band's boundary polygon: the inner offset curve, then the outer one walked back.
  fn boundary_points(&self) -> Vec<Point2> {
    let mut boundary = Vec::with_capacity(self.samples.len() * 2);
    for sample in &self.samples {
      boundary.push(sample.inner);
    }
    for sample in self.samples.iter().rev() {
      boundary.push(sample.outer);
    }
    boundary
  }
}

/// Distance from `point` to a closed polyline's edges.
fn distance_to_polyline(point: Point2, polyline: &[Point2]) -> f64 {
  let mut best = f64::INFINITY;
  for index in 0..polyline.len() {
    let edge = LineSeg::new(polyline[index], polyline[(index + 1) % polyline.len()]);
    best = best.min(edge.distance_to_point(point));
  }
  best
}

/// Even-odd point-in-polygon test over a closed polyline.
fn point_in_polygon(point: Point2, polygon: &[Point2]) -> bool {
  let mut inside = false;
  for index in 0..polygon.len() {
    let from = polygon[index];
    let to = polygon[(index + 1) % polygon.len()];
    if (from.y > point.y) != (to.y > point.y) {
      let crossing_x = from.x + (point.y - from.y) / (to.y - from.y) * (to.x - from.x);
      if point.x < crossing_x {
        inside = !inside;
      }
    }
  }
  inside
}

/// The lateral offsets of the overlapped passes a cross-section's width needs (M8 T3).
///
/// The law is: start at the cross-section's minimum lateral distance, step by the stepover while the
/// next pass stays inside the span, and always make the maximum lateral distance a pass. That makes
/// every gap at most one stepover and both authored edge depths cut, and the coverage radius — the
/// furthest any lateral position in the span can be from a pass — at most half a stepover.
///
/// Two cases are decided rather than clamped:
///
/// * A stepover **exactly** the profile's width is the two edge passes ([`StepoverLaw::new`]). The
///   law's own start-and-step produces them, and it does not collapse them into one pass.
/// * A stepover **wider** than the profile is one pass, centred on the profile. With that spacing no
///   second distinct pass exists, and a pass at one edge would cut one authored edge and leave the
///   other; the centre is the honest single answer. Its coverage radius is half the profile's width,
///   which is still less than half the requested stepover, so the certificate holds.
#[derive(Clone, Debug, PartialEq)]
pub struct StepoverLaw {
  passes: Vec<f64>,
  lateral_min: f64,
  lateral_max: f64,
  stepover: f64,
}

impl StepoverLaw {
  /// Build the pass family for `cross` at `stepover` millimetres.
  ///
  /// # Errors
  ///
  /// [`GeomError::MouldingStepoverNotPositive`] when `stepover` is zero, negative or not finite: a
  /// stepover with no distance behind it cannot space passes, and it is refused with its value rather
  /// than replaced by a default.
  pub fn new(cross: &CrossSection, stepover: f64) -> Result<Self, GeomError> {
    if !stepover.is_finite() || stepover <= 0.0 {
      return Err(GeomError::MouldingStepoverNotPositive { stepover });
    }
    let lateral_min = cross.lateral_min();
    let lateral_max = cross.lateral_max();
    let width = lateral_max - lateral_min;
    let passes = if stepover > width {
      // One pass, centred on the profile: see the type's own note on this decision.
      vec![lateral_min + width / 2.0]
    } else {
      let mut passes = Vec::new();
      let mut index = 0usize;
      loop {
        let lateral = lateral_min + index as f64 * stepover;
        if lateral >= lateral_max - 1.0e-12 {
          break;
        }
        passes.push(lateral);
        index += 1;
      }
      // The far edge is always a pass: dropping it would leave the profile's own boundary uncut.
      if passes.is_empty()
        || passes
          .last()
          .is_some_and(|last| lateral_max - last > 1.0e-12)
      {
        passes.push(lateral_max);
      }
      passes
    };
    Ok(Self {
      passes,
      lateral_min,
      lateral_max,
      stepover,
    })
  }

  /// The lateral offsets of the passes, in increasing order.
  pub fn passes(&self) -> &[f64] {
    &self.passes
  }

  /// The cross-section's minimum lateral distance.
  pub fn lateral_min(&self) -> f64 {
    self.lateral_min
  }

  /// The cross-section's maximum lateral distance.
  pub fn lateral_max(&self) -> f64 {
    self.lateral_max
  }

  /// The requested stepover, in millimetres.
  pub fn stepover(&self) -> f64 {
    self.stepover
  }

  /// The furthest any lateral position in the span can be from its nearest pass.
  ///
  /// With the two edges among the passes it is half the largest gap; with the single centred pass of a
  /// span narrower than the stepover it is half the profile's width.
  pub fn coverage_radius(&self) -> f64 {
    if self.passes.len() == 1 {
      return (self.lateral_max - self.lateral_min) / 2.0;
    }
    self
      .passes
      .windows(2)
      .map(|pair| (pair[1] - pair[0]) / 2.0)
      .fold(0.0, f64::max)
  }

  /// True when every lateral position of the profile is within half a stepover of a pass.
  ///
  /// This is the T3 certificate as one predicate, so a strategy can state the guarantee it relies on
  /// instead of restating the arithmetic.
  pub fn covers(&self) -> bool {
    self.coverage_radius() <= self.stepover / 2.0 + 1.0e-9
  }
}
