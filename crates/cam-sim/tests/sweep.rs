//! The sweep: what a program removes, and the rapid that rams the stock (M5 A2, A4).

use cam_geom::Point2;
use cam_model::{Job, OriginCorner};
use cam_move::{Move, MoveStream, Point3, Segment, ToolShape};
use cam_sim::{sweep, Heightfield};
use cam_units::{Angle, Length, UnitSystem};

fn job(extent: f64, thickness: f64) -> Job {
  Job {
    width: Length::from_mm(extent),
    height: Length::from_mm(extent),
    thickness: Length::from_mm(thickness),
    units: UnitSystem::Mm,
    origin: OriginCorner::BottomLeft,
    safe_z: Length::from_mm(5.0),
  }
}

fn field(extent: f64, cell: f64) -> Heightfield {
  Heightfield::new(&job(extent, 10.0), Length::from_mm(cell)).expect("a small field fits")
}

fn segment(name: &str, diameter: f64, moves: Vec<Move>) -> Segment {
  Segment {
    name: name.to_owned(),
    tool_number: 1,
    tool: ToolShape::cylinder(Length::from_mm(diameter)),
    feed_rate: 1000.0,
    plunge_rate: 300.0,
    spindle_speed: 12000,
    moves,
  }
}

fn program(segments: Vec<Segment>) -> MoveStream {
  MoveStream {
    segments,
    safe_z: Length::from_mm(5.0),
    home: Point2::new(0.0, 0.0),
  }
}

fn cut_cells(field: &Heightfield) -> usize {
  let grid = field.grid();
  let mut count = 0;
  for row in 0..grid.rows {
    for column in 0..grid.columns {
      if field.surface(column, row) < 0.0 {
        count += 1;
      }
    }
  }
  count
}

/// The distance from a point to a segment, computed the slow obvious way: the test's own opinion.
fn distance_to_segment(point: Point2, from: Point2, to: Point2) -> f64 {
  let dx = to.x - from.x;
  let dy = to.y - from.y;
  let length2 = dx * dx + dy * dy;
  let t = if length2 == 0.0 {
    0.0
  } else {
    (((point.x - from.x) * dx + (point.y - from.y) * dy) / length2).clamp(0.0, 1.0)
  };
  let foot = Point2::new(from.x + dx * t, from.y + dy * t);
  point.distance_to(foot)
}

/// Every cell the exact capsule covers, brute-forced: the sweep must agree with it cell for cell.
fn capsule_cells(field: &Heightfield, from: Point2, to: Point2, radius: f64) -> usize {
  let grid = field.grid();
  let mut count = 0;
  for row in 0..grid.rows {
    for column in 0..grid.columns {
      let centre = field.cell_centre(column, row);
      if distance_to_segment(centre, from, to) <= radius {
        count += 1;
      }
    }
  }
  count
}

#[test]
fn a_straight_feed_cuts_exactly_the_capsule_the_tool_sweeps() {
  // A feed at **one depth**, which is what this test is about: the footprint in XY. Its fixture used to
  // start at clearance and arrive at `-2` over real travel, which D-M6-12 made a ramping move — a graded
  // floor is the next test's question, so the fixture is now constant-Z. Nothing about the capsule
  // changed; only the fixture that was never the subject did.
  let mut field = field(40.0, 1.0);
  let from = Point2::new(5.0, 5.0);
  let to = Point2::new(15.0, 5.0);
  let radius = 3.0;
  let expected = capsule_cells(&field, from, to, radius);

  sweep::apply_move(
    &mut field,
    Point3::new(from.x, from.y, -2.0),
    Move::Feed {
      to: Point3::new(to.x, to.y, -2.0),
    },
    ToolShape::cylinder(Length::from_mm(2.0 * radius)),
  );

  assert_eq!(cut_cells(&field), expected, "the sweep's capsule");
  let volumes = field.volumes();
  assert!(
    (volumes.removed - expected as f64 * 2.0).abs() < 1e-9,
    "{} cells at 2 mm deep removed {}",
    expected,
    volumes.removed
  );
}

#[test]
fn a_full_circle_arc_cuts_its_annulus_and_leaves_the_centre_standing() {
  // Constant-Z for the same reason as the line above: a sloped arc grades its annulus along the sweep.
  let mut field = field(40.0, 1.0);
  let centre = Point2::new(20.0, 20.0);
  let start = Point2::new(25.0, 20.0);
  sweep::apply_move(
    &mut field,
    Point3::new(start.x, start.y, -2.0),
    Move::Arc {
      to: Point3::new(start.x, start.y, -2.0),
      center: centre,
      ccw: true,
    },
    ToolShape::cylinder(Length::from_mm(2.0)),
  );

  let middle = field.cell_centre(20, 20);
  assert!(
    (middle.x - 20.5).abs() < 1e-12,
    "the centre cell's centre {middle:?}"
  );
  assert_eq!(
    field.surface(20, 20),
    0.0,
    "a circle of radius 5 cut by a 1 mm tool leaves its middle standing"
  );
  assert!(
    field.surface(25, 20) < 0.0,
    "the cell the arc passes through is cut"
  );
  assert!(
    field.surface(24, 20) < 0.0,
    "the annulus between 4 and 6 mm from the centre is cut: the cell at (24.5, 20.5) is 4.53 mm out"
  );
  assert_eq!(
    field.surface(21, 20),
    0.0,
    "the cell at (21.5, 20.5) is 1.58 mm from the centre, inside the circle the tool never enters"
  );
}

#[test]
fn a_rapid_at_clearance_cuts_nothing_and_is_not_a_crash() {
  let mut field = field(40.0, 1.0);
  let removal = sweep::apply(
    &program(vec![segment(
      "profile",
      6.0,
      vec![Move::Rapid {
        to: Point3::new(20.0, 20.0, 5.0),
      }],
    )]),
    &mut field,
  );
  assert!(removal.crashes.is_empty(), "{:?}", removal.crashes);
  assert_eq!(removal.moves, 1);
  assert_eq!(field.volumes().removed, 0.0);
}

#[test]
fn a_rapid_into_the_material_is_a_crash_and_cuts_nothing() {
  let mut field = field(40.0, 1.0);
  let removal = sweep::apply(
    &program(vec![segment(
      "pocket 1",
      6.0,
      vec![
        Move::Rapid {
          to: Point3::new(20.0, 20.0, 5.0),
        },
        Move::Rapid {
          to: Point3::new(20.5, 20.5, -3.0),
        },
      ],
    )]),
    &mut field,
  );

  assert_eq!(removal.moves, 2);
  assert_eq!(removal.crashes.len(), 1, "{:?}", removal.crashes);
  let crash = &removal.crashes[0];
  assert_eq!(crash.toolpath, "pocket 1");
  assert_eq!(crash.move_index, 1);
  assert_eq!(crash.at.z.mm(), -3.0);
  assert_eq!(
    field.volumes().removed,
    0.0,
    "a crash is reported, not simulated"
  );
}

#[test]
fn a_feed_above_the_surface_cuts_air() {
  let mut field = field(40.0, 1.0);
  let removal = sweep::apply(
    &program(vec![segment(
      "profile",
      6.0,
      vec![Move::Feed {
        to: Point3::new(20.0, 20.0, 2.0),
      }],
    )]),
    &mut field,
  );
  assert!(removal.crashes.is_empty(), "{:?}", removal.crashes);
  assert_eq!(field.volumes().removed, 0.0);
}

#[test]
fn two_runs_over_the_same_program_give_the_same_field() {
  let build = || {
    program(vec![segment(
      "pocket",
      6.0,
      vec![
        Move::Rapid {
          to: Point3::new(5.0, 5.0, 5.0),
        },
        Move::Feed {
          to: Point3::new(5.0, 5.0, -3.0),
        },
        Move::Feed {
          to: Point3::new(30.0, 5.0, -3.0),
        },
        Move::Arc {
          to: Point3::new(30.0, 25.0, -3.0),
          center: Point2::new(30.0, 15.0),
          ccw: false,
        },
      ],
    )])
  };
  let mut first = field(40.0, 1.0);
  let mut second = field(40.0, 1.0);
  let a = sweep::apply(&build(), &mut first);
  let b = sweep::apply(&build(), &mut second);

  assert_eq!(a, b);
  assert_eq!(first.volumes(), second.volumes());
  let grid = first.grid();
  for row in 0..grid.rows {
    for column in 0..grid.columns {
      assert_eq!(
        first.surface(column, row),
        second.surface(column, row),
        "cell ({column}, {row})"
      );
    }
  }
  assert!(
    first.volumes().removed > 0.0,
    "the program did cut something"
  );
}

/// A V-bit: `degrees` across the tip, and a flat of `flat` millimetres at it.
fn v_bit(degrees: f64, flat: f64) -> ToolShape {
  ToolShape::v_bit(Angle::from_degrees(degrees), Length::from_mm(flat))
}

#[test]
fn a_v_bit_leaves_a_v_groove_whose_floor_follows_its_flank() {
  // A 90 degree V-bit with a sharp point, one pass 6 mm deep along y = 20.5 from x = 5 to x = 15. The
  // cone's radius where it meets the surface is `6 * tan(45) = 6`, so the groove is 12 mm wide and its
  // floor `x` millimetres to the side of the line is `6 - x` deep. A cylinder would leave a flat floor
  // across all 12 mm, which is material the machine never removed (M6 A8). The pass is constant-Z: a
  // sloped one grades its floor along the travel, which is D-M6-12's question and the next test's.
  let mut field = field(40.0, 1.0);
  sweep::apply_move(
    &mut field,
    Point3::new(5.0, 20.5, -6.0),
    Move::Feed {
      to: Point3::new(15.0, 20.5, -6.0),
    },
    v_bit(90.0, 0.0),
  );

  // One column inside the pass, so every distance below is measured to the line and not to an end.
  let column = 10;
  let at = |row: usize| field.surface(column, row);
  assert!(
    (at(20) + 6.0).abs() < 1e-9,
    "on the line the floor is the pass depth, got {}",
    at(20)
  );
  assert!(
    (at(23) + 3.0).abs() < 0.01,
    "3 mm off the line the flank is 3 mm above the tip, got {}",
    at(23)
  );
  assert!(
    (at(25) + 1.0).abs() < 0.01,
    "5 mm off the line only the last millimetre is cut, got {}",
    at(25)
  );
  assert_eq!(at(27), 0.0, "7 mm off the line is past the 6 mm reach");
}

#[test]
fn a_v_bit_with_a_flat_tip_holds_its_depth_across_the_flat_and_then_climbs() {
  // The same constant-Z pass with a 2 mm flat: the floor stays at -6 for the first millimetre either
  // side (the tip is a disc of radius 1), then climbs at 45 degrees, and reaches the surface at
  // 1 + 6 = 7 mm.
  let mut field = field(40.0, 1.0);
  sweep::apply_move(
    &mut field,
    Point3::new(5.0, 20.5, -6.0),
    Move::Feed {
      to: Point3::new(15.0, 20.5, -6.0),
    },
    v_bit(90.0, 2.0),
  );

  let column = 10;
  let at = |row: usize| field.surface(column, row);
  assert!((at(20) + 6.0).abs() < 1e-9, "under the flat: {}", at(20));
  assert!(
    (at(21) + 6.0).abs() < 1e-9,
    "at the flat's edge: {}",
    at(21)
  );
  assert!(
    (at(24) + 3.0).abs() < 0.01,
    "4 mm off the line, 3 mm up the flank: {}",
    at(24)
  );
  assert_eq!(at(28), 0.0, "8 mm off the line is past the 7 mm reach");
}

// --- the vertical interpolation (M6 A2, A8, D-M6-12) ------------------------------------------

/// The program the graded-floor tests run: rapid over `from`, a plunge to the surface, one ramped feed
/// to `depth`, and then `after` millimetres of cutting at that depth.
fn ramped_feed(from: (f64, f64), to: (f64, f64), depth: f64, after: f64) -> Vec<Move> {
  let (x0, y0) = from;
  let (x1, y1) = to;
  let mut moves = vec![
    Move::Rapid {
      to: Point3::new(x0, y0, 5.0),
    },
    Move::Feed {
      to: Point3::new(x0, y0, 0.0),
    },
    Move::Feed {
      to: Point3::new(x1, y1, -depth),
    },
  ];
  if after > 0.0 {
    moves.push(Move::Feed {
      to: Point3::new(x1 + after, y1, -depth),
    });
  }
  moves.push(Move::Retract {
    to: Point3::new(x1 + after, y1, 5.0),
  });
  moves
}

#[test]
fn a_ramped_feed_leaves_a_floor_graded_from_its_start_depth_to_its_end_depth() {
  // D-M6-12: a cutting move removes, at each sampled point along it, the depth interpolated between the
  // `z` it started at and the `z` it ends at. A 0.2 mm cutter feeding from `(5, 20.5)` at the surface to
  // `(15, 20.5)` at `-6` therefore leaves a *slope*, not a step: the floor is `0.6 · (x − 5)` deep and
  // this grid samples it exactly, because the move's half-cell samples (every 0.5 mm from x = 5) land on
  // every cell centre along the line.
  //
  // The cutter is narrow on purpose. Its disc also reaches *along* the path — at 3 mm radius a cell
  // three millimetres up a ramp is cut by the deeper part of the tool rather than by the part over it —
  // and 0.2 mm of diameter keeps that reach a tenth of a millimetre, so the floor measured here is the
  // interpolation and not the footprint. The footprint's own effect is asserted separately, with a
  // V-bit, in `a_ramped_v_bit_applies_its_cone_law_at_each_points_own_depth`.
  //
  // Each number is asserted against the *continuous* ramp to within a sixteenth of a cell, which is the
  // residual the sampler promises (`sample_steps`), not to the last bit: the samples now follow the Z a
  // move covers as well as its travel, so they no longer coincide with the cell centres by luck.
  let mut ramped = field(40.0, 1.0);
  sweep::apply(
    &program(vec![segment(
      "fluting",
      0.2,
      ramped_feed((5.0, 20.5), (15.0, 20.5), 6.0, 0.0),
    )]),
    &mut ramped,
  );

  // The row the line runs through: cell `c` holds the centre `(c + 0.5, 20.5)`.
  let at = |column: usize| -ramped.surface(column, 20);
  let residual = 1.0 / 16.0;
  assert_eq!(at(4), 0.0, "before the ramp nothing is cut");
  for (column, expected) in [(5, 0.3), (10, 3.3), (14, 5.7)] {
    assert!(
      (at(column) - expected).abs() <= residual,
      "column {column} is {expected} mm down the continuous ramp and the sweep recorded {}: past the \
       sixteenth-of-a-cell residual",
      at(column)
    );
  }
  assert_eq!(
    at(15),
    0.0,
    "the ramp ends at its destination: the cutter leaves no disc a millimetre past it"
  );

  // The same feed without a ramp is one flat floor at the full depth: interpolation of a constant is
  // that constant, and this is the neutrality the change has to keep (D-M6-12).
  let mut flat = field(40.0, 1.0);
  sweep::apply(
    &program(vec![segment(
      "fluting",
      0.2,
      vec![
        Move::Rapid {
          to: Point3::new(5.0, 20.5, 5.0),
        },
        Move::Feed {
          to: Point3::new(5.0, 20.5, -6.0),
        },
        Move::Feed {
          to: Point3::new(15.0, 20.5, -6.0),
        },
        Move::Retract {
          to: Point3::new(15.0, 20.5, 5.0),
        },
      ],
    )]),
    &mut flat,
  );
  for column in 5..=14 {
    assert!(
      (flat.surface(column, 20) + 6.0).abs() < 1e-9,
      "column {column} of a constant-Z feed: {}",
      flat.surface(column, 20)
    );
  }
}

#[test]
fn a_ramped_v_bit_applies_its_cone_law_at_each_points_own_depth() {
  // A8's shape and D-M6-12's interpolation together. A 90 degree V-bit ramping from the surface at
  // `x = 5` to `-6` at `x = 15` and then holding 6 mm deep to `x = 25`. On the line the floor is the
  // interpolated depth of the point above it, because a sharp cone's flank climbs faster than the ramp
  // does (`rise` is 1 mm per millimetre of radius, the ramp is 0.6). Off the line the floor is the cone
  // law of whichever sample reaches deepest: in the ramp's first millimetre and 2 mm to the side,
  // nothing is cut at all, because no sample there is wide enough to reach; in the plateau it is the
  // familiar `depth − distance`.
  let mut field = field(40.0, 1.0);
  sweep::apply_move(
    &mut field,
    Point3::new(5.0, 20.5, 5.0),
    Move::Feed {
      to: Point3::new(5.0, 20.5, 0.0),
    },
    v_bit(90.0, 0.0),
  );
  sweep::apply_move(
    &mut field,
    Point3::new(5.0, 20.5, 0.0),
    Move::Feed {
      to: Point3::new(15.0, 20.5, -6.0),
    },
    v_bit(90.0, 0.0),
  );
  sweep::apply_move(
    &mut field,
    Point3::new(15.0, 20.5, -6.0),
    Move::Feed {
      to: Point3::new(25.0, 20.5, -6.0),
    },
    v_bit(90.0, 0.0),
  );

  // On the line, the tip's own interpolated depth to within the sampler's residual (a sixteenth of a
  // cell): cell `c` is the point `x = c + 0.5`.
  for (column, expected) in [(7, 1.5), (10, 3.3), (14, 5.7), (20, 6.0)] {
    let at = -field.surface(column, 20);
    assert!(
      (at - expected).abs() <= 1.0 / 16.0,
      "on the line at column {column} the floor is {expected} mm down, got {at}"
    );
  }
  // Two millimetres to the side, half a millimetre into the ramp: the cone there is 0.3 mm wide and
  // reaches nothing.
  assert_eq!(
    field.surface(5, 22),
    0.0,
    "a 0.3 mm deep cone cannot reach a cell 2 mm off the line"
  );
  // Two millimetres to the side in the plateau: 6 − 2, the cone's own flank.
  let plateau = field.surface(20, 22);
  assert!(
    (plateau + 4.0).abs() < 1e-9,
    "2 mm off the line in the plateau the floor is 4 mm down, got {plateau}"
  );
}

#[test]
fn a_plunge_is_ramped_but_removes_one_disc_at_its_destination_depth() {
  // The neutrality check inside the suite: a plunge's `z` changes, so it *is* a ramping move, but its
  // travel is zero, so both of its samples sit at the same XY and the deeper one is the destination.
  // A plunge therefore removes exactly one disc at its destination depth, as it did before D-M6-12.
  let mut field = field(40.0, 1.0);
  sweep::apply_move(
    &mut field,
    Point3::new(20.0, 20.0, 5.0),
    Move::Feed {
      to: Point3::new(20.0, 20.0, -4.0),
    },
    ToolShape::cylinder(Length::from_mm(6.0)),
  );

  let radius = 3.0;
  let expected = capsule_cells(
    &field,
    Point2::new(20.0, 20.0),
    Point2::new(20.0, 20.0),
    radius,
  );
  assert_eq!(cut_cells(&field), expected, "a plunge's footprint");
  assert!(
    (field.volumes().removed - expected as f64 * 4.0).abs() < 1e-9,
    "{} cells at 4 mm deep removed {}",
    expected,
    field.volumes().removed
  );
}

#[test]
fn a_ramp_is_sampled_finely_enough_that_its_floor_is_within_a_sixteenth_of_a_cell() {
  // D-M6-12 interpolates the depth along a cutting move; the *sampling* has to resolve it. A cell's
  // recorded removal is the deepest **sample** inside its footprint, while the ramp it stands for is
  // continuous, so the two differ by up to one sample interval's worth of Z — `|Δz| / steps`. Sampling
  // only by travel at half a cell leaves that difference at `grade · step`: here a 6 mm cutter ramping
  // 6 mm over 10 mm is sampled every 0.5 mm, and this cell's footprint reaches 2.958 mm up the ramp,
  // where the ramp is 5.075 mm down while the deepest sample inside its footprint is at 5 + 0.5 · 17 =
  // 13.5 mm — 0.275 mm short of it. Sampling by the depth the move covers as well pins the residual to
  // `cell / 16` by construction — half the tolerance a simulation is judged at, so the bound is met with
  // headroom rather than grazing it.
  let cell = 1.0;
  let depth = 6.0;
  let ramp = 10.0;
  let radius = 3.0;
  let mut field = field(40.0, cell);
  sweep::apply(
    &program(vec![segment(
      "fluting",
      2.0 * radius,
      vec![
        Move::Rapid {
          to: Point3::new(5.0, 20.0, 5.0),
        },
        Move::Feed {
          to: Point3::new(5.0, 20.0, 0.0),
        },
        Move::Feed {
          to: Point3::new(5.0 + ramp, 20.0, -depth),
        },
        Move::Retract {
          to: Point3::new(5.0 + ramp, 20.0, 5.0),
        },
      ],
    )]),
    &mut field,
  );

  // The cell at `(10.5, 20.5)` is half a millimetre off the line, so its footprint reaches
  // `sqrt(3² − 0.5²) = 2.958` mm further along the ramp from its own position.
  let grade = depth / ramp;
  let reach = (radius * radius - 0.25_f64).sqrt();
  let sup = grade * (10.5 - 5.0 + reach);
  let removed = -field.surface(10, 20);
  assert!(
    removed <= sup + 1e-9,
    "the floor can never be deeper than the ramp it comes from: removed {removed}, ramp {sup}"
  );
  assert!(
    sup - removed <= cell / 16.0,
    "the sampled floor is {sup} mm down and the sweep recorded {removed}: a residual of {} mm, past \
     the cell / 16 = {} mm bound",
    sup - removed,
    cell / 16.0
  );
  // The same statement at the entry's first cell — the one before the ramp starts at `x = 5`, whose
  // footprint still reaches `reach` mm into it — where the residual has the opposite sign and the same
  // bound.
  let sup_near_end = grade * ((4.5 - 5.0) + reach);
  let removed_near_end = -field.surface(4, 20);
  assert!(
    removed_near_end <= sup_near_end + 1e-9 && sup_near_end - removed_near_end <= cell / 16.0,
    "the entry's first cell: ramp {sup_near_end}, removed {removed_near_end}"
  );
}

// --- helical moves (M6 A7, D-M6-8, D-M6-12) ----------------------------------------------------

/// The depth the sweep removed at the cell nearest `point`.
fn removal_at(field: &Heightfield, point: Point2) -> f64 {
  let grid = field.grid();
  let mut best: Option<(f64, f64)> = None;
  for row in 0..grid.rows {
    for column in 0..grid.columns {
      let centre = field.cell_centre(column, row);
      let distance = centre.distance_to(point);
      if best.map_or(true, |(_, nearest)| distance < nearest) {
        best = Some((-field.surface(column, row), distance));
      }
    }
  }
  best.expect("a grid with cells").0
}

#[test]
fn a_helical_move_descends_along_its_arc() {
  // Half a turn of a 10 mm circle, from the surface to `-4`, cut by a 2 mm tool. D-M6-12's
  // interpolation is what makes this a *descending* path rather than a step: at each sampled point the
  // sweep removes the depth the move has reached **there**, so the floor is graded by the angle and the
  // deepest cell is at the end of the turn.
  let mut field = field(40.0, 1.0);
  let centre = Point2::new(20.0, 20.0);
  let radius = 10.0;
  let depth = 4.0;
  let start = Point2::new(centre.x + radius, centre.y);

  sweep::apply_move(
    &mut field,
    Point3::new(start.x, start.y, 0.0),
    Move::Helix {
      to: Point3::new(centre.x - radius, centre.y, -depth),
      center: centre,
      ccw: true,
    },
    ToolShape::cylinder(Length::from_mm(2.0)),
  );

  for (fraction, degrees) in [
    (0.0, 0.0),
    (0.25, 45.0),
    (0.5, 90.0),
    (0.75, 135.0),
    (1.0, 180.0),
  ] {
    let angle = fraction * std::f64::consts::PI;
    let point = Point2::new(
      centre.x + radius * angle.cos(),
      centre.y + radius * angle.sin(),
    );
    let expected = depth * fraction;
    let removed = removal_at(&field, point);
    assert!(
      (removed - expected).abs() < 0.2,
      "at {degrees} degrees of the turn the tool has descended {expected} mm, and the sweep recorded \
       {removed} mm"
    );
  }
  // The middle of the circle is never touched: a helical move is an arc that descends, not a disc.
  assert_eq!(removal_at(&field, centre), 0.0, "the arc's interior stands");
}

#[test]
fn a_full_turn_helix_is_one_revolution_and_grades_one_pitch_around_it() {
  // A full turn's destination is its own start point, which is one revolution — the same reading a
  // full-circle arc has — and the one revolution descends the whole of its pitch around it. So the floor
  // is a spiral: the depth at each angle is that angle's share of the descent, and after one turn every
  // angle has been passed exactly once by this move.
  //
  // The fraction the probe is *behind* the tool is not tested, and the reason is the geometry rather
  // than convenience: a 2 mm tool on a 10 mm circle covers a cell half a hundredth of a turn ahead of its
  // own centre, so the start point itself is passed once more at the very end of the revolution — a full
  // turn ends where it began — and reads the final depth. Every other angle reads its own share.
  let mut field = field(40.0, 1.0);
  let centre = Point2::new(20.0, 20.0);
  let radius = 10.0;
  let depth = 2.0;
  let point = Point2::new(centre.x + radius, centre.y);

  sweep::apply_move(
    &mut field,
    Point3::new(point.x, point.y, 0.0),
    Move::Helix {
      to: Point3::new(point.x, point.y, -depth),
      center: centre,
      ccw: true,
    },
    ToolShape::cylinder(Length::from_mm(2.0)),
  );

  let probe_at = |field: &Heightfield, fraction: f64| {
    let angle = fraction * std::f64::consts::TAU;
    removal_at(
      field,
      Point2::new(
        centre.x + radius * angle.cos(),
        centre.y + radius * angle.sin(),
      ),
    )
  };
  for fraction in [0.25_f64, 0.5, 0.75] {
    let expected = depth * fraction;
    let removed = probe_at(&field, fraction);
    assert!(
      (removed - expected).abs() < 0.2,
      "at {fraction} of the turn the tool has descended {expected} mm, and the sweep recorded {removed}"
    );
  }
  assert!(
    (probe_at(&field, 0.0) - depth).abs() < 0.2,
    "the start point is passed again where the revolution ends, at the final depth"
  );

  // A second revolution of the same pitch passes every angle again, a pitch deeper: the deepest pass at
  // any angle is the later one, which is why a thread's descent is several moves and not one.
  sweep::apply_move(
    &mut field,
    Point3::new(point.x, point.y, -depth),
    Move::Helix {
      to: Point3::new(point.x, point.y, -2.0 * depth),
      center: centre,
      ccw: true,
    },
    ToolShape::cylinder(Length::from_mm(2.0)),
  );
  for fraction in [0.25_f64, 0.5, 0.75] {
    let expected = depth + depth * fraction;
    let removed = probe_at(&field, fraction);
    assert!(
      (removed - expected).abs() < 0.2,
      "after the second revolution {fraction} of the turn is at {expected} mm, recorded {removed}"
    );
  }
  assert_eq!(
    removal_at(&field, centre),
    0.0,
    "and the middle is still standing"
  );
}
