use super::support::*;
use cam_geom::Point2;
use cam_move::{Move, MoveStream, Point3, Segment, ToolShape};
use cam_ui::view3d::{clock_text, seconds_for};
use cam_ui::Preview3d;
use cam_units::Length;

// ---------------------------------------------------------------------------------------------
// V3 — playback (A10): the program plays, and the stock disappears as it cuts.
// ---------------------------------------------------------------------------------------------

#[test]
fn a_fixed_dt_sequence_reaches_the_closed_form_cursor() {
  // Four moves lasting 0.2, 0.2, 0.5 and 0.1 s. The frames never land on a move boundary, so the
  // expectation is the arithmetic in the fixture's own doc comment and not a floating-point edge.
  let mut preview = Preview3d::new(untouched(), playback_program(), &job());
  // One second of wall clock per second of program time, so the closed form is in program seconds.
  preview.set_speed(1.0);
  preview.set_playing(true);

  let frames = [
    (0.1, 0usize, 0.5, 0.1),
    (0.15, 1, 0.25, 0.25),
    (0.25, 2, 0.2, 0.5),
    (0.2, 2, 0.6, 0.7),
    (0.15, 2, 0.9, 0.85),
    // The last frame overshoots the end, and the clock stops at the program's end rather than at the
    // delta that crossed it: 1.15 s of frames is 1 s of program.
    (0.3, 4, 1.0, 1.0),
  ];
  for (dt, cursor, fraction, elapsed) in frames {
    preview.advance(dt);
    assert_eq!(
      preview.cursor(),
      cursor,
      "after {elapsed} s of program time the cursor must be at move {cursor}"
    );
    assert!(
      (preview.fraction() - fraction).abs() < 1.0e-9,
      "after {elapsed} s the current move must be {fraction} consumed, got {}",
      preview.fraction()
    );
    assert!(
      (preview.elapsed() - elapsed).abs() < 1.0e-9,
      "the clock must read {elapsed} s, got {}",
      preview.elapsed()
    );
  }
  assert!(
    preview.finished(),
    "the sequence adds up to the whole program"
  );
  assert_eq!(preview.progress(), 1.0);
}

#[test]
fn a_fixed_dt_sequence_removes_the_closed_form_volume() {
  let mut preview = Preview3d::new(untouched(), playback_program(), &job());
  preview.set_speed(1.0);
  preview.set_playing(true);
  assert_eq!(
    preview.field().volumes(),
    closed_form_volumes(0),
    "playback starts on untouched stock"
  );

  // 0.45 s in: both the rapid and the plunge are whole moves behind the cursor.
  preview.advance(0.45);
  assert_eq!(preview.cursor(), 2);
  assert_eq!(preview.field().volumes(), closed_form_volumes(2));
  assert!(
    preview.field().volumes().removed > 0.0,
    "the plunge is a cutting move, so material is already gone"
  );

  // 0.75 s in: part of the feed is consumed, and a partly consumed move removes nothing yet.
  preview.advance(0.3);
  assert_eq!(preview.cursor(), 2);
  assert_eq!(
    preview.field().volumes(),
    closed_form_volumes(2),
    "a half-consumed move must not have been applied to the field"
  );

  // 0.95 s in: the feed is behind the cursor and the capsule it swept is gone.
  preview.advance(0.2);
  assert_eq!(preview.cursor(), 3);
  assert_eq!(preview.field().volumes(), closed_form_volumes(3));
  assert!(
    preview.field().volumes().removed > closed_form_volumes(2).removed,
    "the feed removes more material than the plunge alone"
  );

  // 1.05 s in: the retract ends the program, and its own cut is nothing.
  preview.advance(0.1);
  assert!(preview.finished());
  assert_eq!(preview.field().volumes(), closed_form_volumes(4));
}

#[test]
fn a_zero_dt_changes_nothing() {
  let mut preview = Preview3d::new(untouched(), playback_program(), &job());
  preview.set_speed(1.0);
  preview.set_playing(true);
  preview.advance(0.45);
  let cursor = preview.cursor();
  let fraction = preview.fraction();
  let volumes = preview.field().volumes();
  let elapsed = preview.elapsed();

  preview.advance(0.0);

  assert_eq!(preview.cursor(), cursor);
  assert_eq!(preview.fraction(), fraction);
  assert_eq!(preview.elapsed(), elapsed);
  assert_eq!(preview.field().volumes(), volumes);
  assert!(preview.playing(), "a zero frame must not stop playback");
}

#[test]
fn pausing_holds_the_cursor_and_playing_resumes_from_it() {
  let mut preview = Preview3d::new(untouched(), playback_program(), &job());
  preview.set_speed(1.0);
  preview.set_playing(true);
  preview.advance(0.45);
  assert_eq!(preview.cursor(), 2);

  preview.set_playing(false);
  preview.advance(10.0);
  assert_eq!(preview.cursor(), 2, "a paused preview must not move");
  assert!(
    (preview.fraction() - 0.1).abs() < 1.0e-9,
    "a paused preview must not consume part of a move either, got {}",
    preview.fraction()
  );
  assert_eq!(preview.field().volumes(), closed_form_volumes(2));

  preview.set_playing(true);
  preview.advance(0.4);
  assert_eq!(
    preview.cursor(),
    2,
    "playing resumes where the pause left it"
  );
  assert!(
    (preview.fraction() - 0.9).abs() < 1.0e-9,
    "0.85 s into a 1 s program is 0.9 of its third move, got {}",
    preview.fraction()
  );
}

#[test]
fn a_step_advances_exactly_one_move() {
  let mut preview = Preview3d::new(untouched(), playback_program(), &job());
  assert!(!preview.playing(), "a preview opens paused");

  preview.step();
  assert_eq!(preview.cursor(), 1);
  assert_eq!(preview.fraction(), 0.0);
  assert!(!preview.playing(), "stepping is a paused advance");
  assert_eq!(
    preview.field().volumes(),
    closed_form_volumes(1),
    "one step applies exactly the first move"
  );

  preview.step();
  assert_eq!(preview.cursor(), 2);
  assert_eq!(preview.field().volumes(), closed_form_volumes(2));
  assert!(
    preview.field().volumes().removed > 0.0,
    "the second move is the plunge"
  );

  preview.step();
  preview.step();
  assert_eq!(preview.cursor(), 4);
  assert!(preview.finished());
  assert_eq!(preview.progress(), 1.0);

  preview.step();
  assert_eq!(preview.cursor(), 4, "a step past the end changes nothing");
}

#[test]
fn reset_restores_the_untouched_stock() {
  let mut preview = Preview3d::new(untouched(), playback_program(), &job());
  preview.set_playing(true);
  preview.advance(10.0);
  assert!(preview.finished());
  assert!(preview.field().volumes().removed > 0.0);

  preview.reset();

  assert_eq!(preview.cursor(), 0);
  assert_eq!(preview.fraction(), 0.0);
  assert_eq!(preview.progress(), 0.0);
  assert!(!preview.playing());
  assert_eq!(
    preview.field().volumes(),
    untouched().volumes(),
    "reset restores the whole stock, not just the cursor"
  );
  let fresh = untouched();
  let grid = fresh.grid();
  for row in 0..grid.rows {
    for column in 0..grid.columns {
      assert_eq!(
        preview.field().surface(column, row),
        fresh.surface(column, row),
        "cell ({column}, {row}) must be back at the stock top"
      );
    }
  }
}

#[test]
fn the_final_field_equals_the_whole_program_sweep_cell_for_cell() {
  let expected = whole_program_field();
  let grid = expected.grid();

  // One call and many frames must end in the same place: `advance` is a pure function of the deltas
  // it is given, and it applies whole moves as the cursor crosses them.
  for (how, deltas) in [
    ("one frame", vec![10.0_f64]),
    ("many frames", vec![0.03_f64; 40]),
  ] {
    let mut preview = Preview3d::new(untouched(), program(), &job());
    preview.set_playing(true);
    for dt in deltas {
      preview.advance(dt);
    }
    assert!(
      preview.finished(),
      "{how}: the whole program must have been played"
    );
    assert_eq!(preview.field().grid(), grid, "{how}");
    for row in 0..grid.rows {
      for column in 0..grid.columns {
        assert_eq!(
          preview.field().surface(column, row),
          expected.surface(column, row),
          "{how}: cell ({column}, {row}) must match `sweep::apply` over the whole program"
        );
      }
    }
    assert_eq!(
      preview.field().volumes(),
      expected.volumes(),
      "{how}: the animation and the report cannot disagree"
    );
  }
}

#[test]
fn playback_stops_at_the_end_of_the_program() {
  let mut preview = Preview3d::new(untouched(), playback_program(), &job());
  preview.set_playing(true);
  preview.advance(10.0);

  assert!(preview.finished());
  assert!(!preview.playing(), "playback stops at the end");
  assert_eq!(preview.progress(), 1.0);
  assert_eq!(preview.fraction(), 1.0);
  let volumes = preview.field().volumes();
  let elapsed = preview.elapsed();

  preview.set_playing(true);
  preview.advance(10.0);
  assert_eq!(preview.cursor(), 4);
  assert_eq!(preview.field().volumes(), volumes);
  assert_eq!(preview.elapsed(), elapsed);
  assert!(
    !preview.playing(),
    "playing a finished program stops immediately"
  );
}

#[test]
fn the_panel_reports_playback_progress_from_the_start_to_the_end() {
  let mut harness = Harness::playing();

  // Paused by default: two frames leave the program where it started.
  harness.frame(Vec::new());
  harness.frame(Vec::new());
  assert_eq!(
    harness.output.drawn.preview_progress, 0.0,
    "the readout starts at the first move"
  );
  assert_eq!(harness.preview().cursor(), 0);

  // Playing: the shell reads the clock and hands the delta in, so the readout moves on its own.
  harness
    .shell
    .preview_mut()
    .expect("the harness loaded a preview")
    .set_speed(0.5);
  harness
    .shell
    .preview_mut()
    .expect("the harness loaded a preview")
    .set_playing(true);
  harness.frame(Vec::new());
  let midway = harness.output.drawn.preview_progress;
  assert!(
    midway > 0.0 && midway < 1.0,
    "one frame of a playing program is part of the way through, not at either end: {midway}"
  );

  harness
    .shell
    .preview_mut()
    .expect("the harness loaded a preview")
    .set_speed(600.0);
  for _ in 0..10 {
    harness.frame(Vec::new());
    if harness.preview().finished() {
      break;
    }
  }
  assert!(
    harness.preview().finished(),
    "the panel must play the program to its end"
  );
  assert_eq!(
    harness.output.drawn.preview_progress, 1.0,
    "the readout reaches its end value when the program finishes"
  );
  assert_eq!(harness.preview().cursor(), playback_program().move_count());
}

// ---------------------------------------------------------------------------------------------
// M6 — a helical move (A7): the preview times it along its own spiral and draws it descending.
// ---------------------------------------------------------------------------------------------

#[test]
fn a_helical_move_is_timed_along_its_own_spiral_and_played_to_the_end() {
  // The preview's clock is a story about progress, and a helix travels the hypotenuse of a turn rather
  // than the turn itself: one full revolution of radius 10 descending 2 mm is
  // `sqrt((2π·10)² + 2²) = 62.8638` mm, which at the segment's 100 mm/s feed is 0.628638 s — where an
  // arc's own timing, the turn alone, would say 0.628319. The fixture's four moves are therefore a
  // rapid of 20 mm (0.2 s), a plunge of 10 mm at 50 mm/s (0.2 s), the helix, and a retract of 12 mm
  // (0.12 s): `1.148638` s in all, and the program must reach its end.
  let mut stream = MoveStream::new(Length::from_mm(5.0), Point2::new(10.0, 0.0));
  stream.segments.push(Segment {
    name: "thread".to_owned(),
    tool_number: 1,
    tool: ToolShape::cylinder(Length::from_mm(4.0)),
    feed_rate: 100.0,
    plunge_rate: 50.0,
    spindle_speed: 18_000,
    moves: vec![
      Move::Rapid {
        to: Point3::new(10.0, 20.0, 5.0),
      },
      Move::Feed {
        to: Point3::new(10.0, 20.0, -5.0),
      },
      Move::Helix {
        to: Point3::new(10.0, 20.0, -7.0),
        center: Point2::new(20.0, 20.0),
        ccw: true,
      },
      Move::Retract {
        to: Point3::new(10.0, 20.0, 5.0),
      },
    ],
  });

  let spiral = ((std::f64::consts::TAU * 10.0_f64).powi(2) + 4.0).sqrt();
  let expected = 0.2 + 0.2 + spiral / 100.0 + 12.0 / 100.0;

  let mut preview = Preview3d::new(untouched(), stream, &job());
  preview.set_speed(1.0);
  preview.set_playing(true);
  preview.advance(10.0);
  assert!(preview.finished(), "the program plays to its end");
  assert!(
    (preview.elapsed() - expected).abs() < 1.0e-9,
    "the program's own time is {expected} s, the preview says {} s",
    preview.elapsed()
  );
  assert_eq!(
    preview.cursor(),
    4,
    "four moves: the helix is one of them, not a staircase"
  );
}

#[test]
fn seconds_for_is_the_time_laws_total_and_not_a_second_opinion() {
  // `playback_program`'s four moves are 20 mm at 100 mm/s, a 10 mm plunge at 50 mm/s, a 50 mm feed at
  // 100 mm/s and a 10 mm retract at 100 mm/s: one second, exactly what the clock's own arithmetic
  // answers. The plunge is the case a second formula gets wrong — it is timed at the *plunge* rate.
  let seconds = seconds_for(&playback_program());
  assert!(
    (seconds - 1.0).abs() < 1e-12,
    "the four moves are one second, not {seconds}"
  );
}

#[test]
fn the_published_seconds_agree_with_the_playback_clock() {
  let program = playback_program();
  let mut preview = Preview3d::new(untouched(), program.clone(), &job());

  // The estimate and the playback bar are one number with two readers (S5): the published total is
  // the preview's own duration, not a parallel computation that happens to agree here.
  assert_eq!(seconds_for(&program), preview.duration());

  // And it is the number the clock really runs out on: played to its end, the readout has consumed
  // exactly the published seconds.
  preview.set_playing(true);
  preview.advance(10.0);
  assert!(
    preview.finished(),
    "ten seconds of wall clock play one second out"
  );
  assert_eq!(preview.elapsed(), seconds_for(&program));
  assert_eq!(clock_text(seconds_for(&program)), "0:01");
}
