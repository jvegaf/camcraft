//! Profile (follow) machining: cut along a contour with side offset, leads and tabs.

use cam_geom::{Contour, Point2, Segment};
use cam_model::{Job, ProfileParams, ProfileSide, Tab};
use cam_move::{Move, Point3};
use cam_units::Length;

use crate::shared::{
  machine_point, pass_depths, piece_endpoints, push_horizontal, sub_segment, ToolSpec,
  ToolpathError,
};

/// Everything a profile pass needs.
#[derive(Clone, Copy, Debug)]
pub struct ProfileRequest<'a> {
  /// The closed geometry to cut.
  pub contour: &'a Contour,
  /// Side, allowance, depths, leads and tabs.
  pub params: &'a ProfileParams,
  /// The tool doing the cutting.
  pub tool: &'a ToolSpec,
  /// Job setup, used to map document coordinates onto machine coordinates.
  pub job: &'a Job,
}

/// Compute the moves for a profile cut.
///
/// The moves always retract to the job's safe Z between passes: a wrong-depth pass is a ruined part,
/// and staying down between passes buys nothing on a profile cut.
pub fn profile(request: &ProfileRequest<'_>) -> Result<Vec<Move>, ToolpathError> {
  if !request.contour.is_closed() {
    return Err(ToolpathError::OpenContour);
  }
  if request.tool.diameter.mm() <= 0.0 {
    return Err(ToolpathError::InvalidToolDiameter);
  }
  let params = request.params;
  if params.pass_depth.mm() <= 0.0 {
    return Err(ToolpathError::InvalidPassDepth);
  }
  if params.cut_depth.mm() <= params.start_depth.mm() {
    return Err(ToolpathError::InvalidDepthRange {
      cut_depth: params.cut_depth.mm(),
      start_depth: params.start_depth.mm(),
    });
  }
  for (index, tab) in params.tabs.iter().enumerate() {
    if !(0.0..1.0).contains(&tab.position) {
      return Err(ToolpathError::TabOutOfRange {
        index,
        position: tab.position,
      });
    }
  }

  let lateral = lateral_offset(params, request.tool);
  let cut = if lateral.mm() == 0.0 {
    request.contour.clone()
  } else {
    cam_geom::offset::offset_contour(
      request.contour,
      lateral.mm(),
      &cam_geom::offset::OffsetOptions::default(),
    )
    .map_err(|error| ToolpathError::OffsetCollapsed {
      reason: error.to_string(),
    })?
    .contour
  };

  let safe_z = request.job.safe_z;
  let passes = pass_depths(
    params.start_depth.mm(),
    params.cut_depth.mm(),
    params.pass_depth.mm(),
  )?;
  let mut moves = Vec::new();

  for depth in passes {
    // Start point: the lead-in origin, so the plunge never happens on the finished edge.
    let start = cut.segments().first().map(|segment| segment.start_point());
    let Some(start) = start else {
      return Err(ToolpathError::OffsetCollapsed {
        reason: "the offset contour has no segments".to_owned(),
      });
    };
    let approach = approach_point(&cut, start, params.lead_in);
    let approach_machine = machine_point(request.job, approach);
    let surface_machine = machine_point(request.job, start);

    moves.push(Move::Rapid {
      to: Point3 {
        x: Length::from_mm(approach_machine.x),
        y: Length::from_mm(approach_machine.y),
        z: safe_z,
      },
    });
    moves.push(Move::Feed {
      to: Point3 {
        x: Length::from_mm(approach_machine.x),
        y: Length::from_mm(approach_machine.y),
        z: Length::from_mm(-depth),
      },
    });
    if params.lead_in.mm() > 0.0 {
      moves.push(Move::Feed {
        to: Point3 {
          x: Length::from_mm(surface_machine.x),
          y: Length::from_mm(surface_machine.y),
          z: Length::from_mm(-depth),
        },
      });
    }

    let lift_for = |tab: &Tab| -> Option<Length> {
      // A tab only matters once the pass is deeper than the material the tab keeps behind.
      let keep_level = params.cut_depth.mm() - tab.height.mm();
      if depth > keep_level + 1.0e-9 {
        Some(tab.height)
      } else {
        None
      }
    };
    moves.extend(walk_contour(
      &cut,
      request.job,
      -depth,
      &params.tabs,
      &lift_for,
    ));

    if params.lead_out.mm() > 0.0 {
      let end = cut
        .segments()
        .last()
        .map(|segment| segment.end_point())
        .unwrap_or(start);
      let departure = departure_point(&cut, end, params.lead_out);
      let departure_machine = machine_point(request.job, departure);
      moves.push(Move::Feed {
        to: Point3 {
          x: Length::from_mm(departure_machine.x),
          y: Length::from_mm(departure_machine.y),
          z: Length::from_mm(-depth),
        },
      });
    }

    let last = moves
      .last()
      .map(|movement| movement.destination())
      .unwrap_or(Point3::ORIGIN);
    moves.push(Move::Retract {
      to: Point3 {
        x: last.x,
        y: last.y,
        z: safe_z,
      },
    });
  }

  Ok(moves)
}

/// The lateral distance between the vector and the tool centre.
fn lateral_offset(params: &ProfileParams, tool: &ToolSpec) -> Length {
  let radius = tool.diameter.mm() / 2.0;
  match params.side {
    ProfileSide::On => Length::ZERO,
    ProfileSide::Outside => Length::from_mm(radius + params.allowance.mm()),
    // Growing the region is a positive offset, so cutting inside shrinks it.
    ProfileSide::Inside => Length::from_mm(-(radius + params.allowance.mm())),
  }
}

/// A point `lead` millimetres behind `start` along the contour's direction of travel.
fn approach_point(contour: &Contour, start: Point2, lead: Length) -> Point2 {
  if lead.mm() <= 0.0 {
    return start;
  }
  let tangent = start_tangent(contour);
  start - tangent * lead.mm()
}

/// A point `lead` millimetres beyond `end` along the contour's direction of travel.
fn departure_point(contour: &Contour, end: Point2, lead: Length) -> Point2 {
  if lead.mm() <= 0.0 {
    return end;
  }
  let tangent = end_tangent(contour);
  end + tangent * lead.mm()
}

fn start_tangent(contour: &Contour) -> cam_geom::Vec2 {
  contour
    .segments()
    .first()
    .map(|segment| tangent_of(*segment))
    .unwrap_or(cam_geom::Vec2::X_AXIS)
}

fn end_tangent(contour: &Contour) -> cam_geom::Vec2 {
  contour
    .segments()
    .last()
    .map(|segment| tangent_of(*segment))
    .unwrap_or(cam_geom::Vec2::X_AXIS)
}

/// Direction of travel at the start of a segment.
fn tangent_of(segment: Segment) -> cam_geom::Vec2 {
  match segment {
    Segment::Line(line) => line.direction().unwrap_or(cam_geom::Vec2::X_AXIS),
    Segment::Arc(arc) => arc.tangent_at_start(),
  }
}

/// Walk the contour once at `z`, splitting at tab boundaries and lifting where a tab applies.
///
/// Shared with `chamfer`, which has no tabs: with an empty `tabs` the split is the whole contour, so a
/// one-pass strategy gets the same arc-preserving walk a profile pass does instead of a second
/// implementation of it.
pub(crate) fn walk_contour(
  contour: &Contour,
  job: &Job,
  z: f64,
  tabs: &[Tab],
  lift_for: &dyn Fn(&Tab) -> Option<Length>,
) -> Vec<Move> {
  let segments = contour.segments();
  let lengths: Vec<f64> = segments.iter().map(|segment| segment.length()).collect();
  let total: f64 = lengths.iter().sum();
  let mut moves = Vec::new();
  let mut current_z = z;

  if tabs.is_empty() || total <= 0.0 {
    for segment in &segments {
      let (from, to) = piece_endpoints(*segment, 0.0, 1.0);
      push_horizontal(&mut moves, job, *segment, from, to, z);
    }
    return moves;
  }

  // Boundaries along the whole contour: 0, each tab edge, each tab start, and the total.
  let spans = tab_spans(tabs, total);
  for (index, segment) in segments.iter().enumerate() {
    let segment_start: f64 = lengths[..index].iter().sum();
    let segment_end = segment_start + lengths[index];
    let length = lengths[index];
    if length <= 0.0 {
      continue;
    }

    // Every boundary that falls inside this segment, in order, plus its own ends.
    let mut cuts: Vec<f64> = vec![segment_start];
    for span in &spans {
      for boundary in [span.start, span.end] {
        if boundary > segment_start + 1.0e-9 && boundary < segment_end - 1.0e-9 {
          cuts.push(boundary);
        }
      }
    }
    cuts.push(segment_end);
    cuts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    cuts.dedup_by(|a, b| (*a - *b).abs() < 1.0e-9);

    for window in cuts.windows(2) {
      let (from_along, to_along) = (window[0], window[1]);
      let from_fraction = (from_along - segment_start) / length;
      let to_fraction = (to_along - segment_start) / length;
      let (from, to) = piece_endpoints(*segment, from_fraction, to_fraction);

      let lift = spans
        .iter()
        .find(|span| span.start <= from_along + 1.0e-9 && span.end >= to_along - 1.0e-9)
        .and_then(|span| lift_for(&tabs[span.tab_index]))
        .map(Length::mm)
        .unwrap_or(0.0);
      let target_z = z + lift;

      if (target_z - current_z).abs() > 1.0e-9 {
        let boundary = machine_point(job, from);
        moves.push(Move::Feed {
          to: Point3 {
            x: Length::from_mm(boundary.x),
            y: Length::from_mm(boundary.y),
            z: Length::from_mm(target_z),
          },
        });
        current_z = target_z;
      }

      let piece = sub_segment(*segment, from_fraction, to_fraction);
      push_horizontal(&mut moves, job, piece, from, to, target_z);
    }
  }

  moves
}

/// A region of the contour covered by one tab.
struct Span {
  start: f64,
  end: f64,
  tab_index: usize,
}

/// The perimeter spans covered by tabs, with wrap-around applied.
fn tab_spans(tabs: &[Tab], total: f64) -> Vec<Span> {
  let mut spans = Vec::with_capacity(tabs.len());
  for (index, tab) in tabs.iter().enumerate() {
    let centre = tab.position * total;
    let half = tab.width.mm() / 2.0;
    let mut start = centre - half;
    let mut end = centre + half;
    if start < 0.0 {
      start += total;
      end += total;
    }
    if end > total {
      // A tab straddling the contour's start splits into two spans.
      spans.push(Span {
        start: 0.0,
        end: end - total,
        tab_index: index,
      });
      spans.push(Span {
        start,
        end: total,
        tab_index: index,
      });
    } else {
      spans.push(Span {
        start,
        end,
        tab_index: index,
      });
    }
  }
  spans.sort_by(|a, b| {
    a.start
      .partial_cmp(&b.start)
      .unwrap_or(std::cmp::Ordering::Equal)
  });
  spans
}
