use super::support::*;
use cam_move::MoveStream;
use cam_ui::theme::{Role, Theme};

#[test]
fn the_panel_offers_play_step_and_reset() {
  let mut harness = Harness::playing();
  harness.frame(Vec::new());

  // The transport is S8's painted `▶` rather than the word `Play`, so the three controls are read from the
  // report the bar publishes — the same surface a test presses them through (camui-prancheta S8).
  let words: Vec<&str> = harness
    .output
    .drawn
    .playback_hits
    .iter()
    .map(|(word, _)| word.as_str())
    .collect();
  for text in ["Play", "Step", "Reset"] {
    assert!(
      words.contains(&text),
      "the panel must offer a {text} control, reported: {words:?}"
    );
  }
  // `Step` and `Reset` are words on the readout's line, so they are painted as well as reported.
  for text in ["Step", "Reset"] {
    assert!(
      harness.texts.iter().any(|painted| painted == text),
      "the panel must paint {text}, painted texts: {:?}",
      harness.texts
    );
  }
}

// ---------------------------------------------------------------------------------------------
// T11 — the simulation's progress is a bar with a clock on both sides, not a text percentage.
// ---------------------------------------------------------------------------------------------

/// A program whose moves all take no time at all: every rate is zero, so the clock's total is `0.0`.
///
/// The fixture exists for the one case the readout must not lie about: a program the shell can still
/// play to its end while computing no time at all to spend.
fn timeless_program() -> MoveStream {
  let mut stream = playback_program();
  for segment in &mut stream.segments {
    segment.feed_rate = 0.0;
    segment.plunge_rate = 0.0;
  }
  stream
}

/// The two rectangles the progress bar painted in the last frame: its trough, then its fill.
///
/// Found by colour rather than by position, which is the point: the trough is the theme's deepest
/// surface and the fill is the cut signal, so a bar painted with a literal of its own would not be
/// found at all. The bar is not inside the surface's own clip rectangle — the playback controls sit
/// above it — so the panel is the clip rect that *contains* the surface mesh's viewport, and every
/// other panel's clip is disjoint from it.
fn bar_rects(harness: &Harness) -> (egui::Rect, egui::Rect) {
  let palette = Theme::Dark.palette();
  let surface_vertices = 3 * harness.output.drawn.painted_triangles;
  assert!(surface_vertices > 0, "the frame painted no surface");
  let viewport = harness
    .shapes
    .iter()
    .find(|clipped| {
      matches!(&clipped.shape, egui::Shape::Mesh(mesh) if mesh.vertices.len() == surface_vertices)
    })
    .map(|clipped| clipped.clip_rect)
    .expect("the surface reached the painter as a mesh");
  // The bar has a clip of its own since the T6 correction (it sits below the picture, and a painter clipped to
  // the picture erased it: R2-001/R3-001), so it is found by where it is — under the surface's viewport — rather
  // than by sharing the surface's own clip. The surface's viewport still proves the 3D half is the one asked.
  let rects_in_panel = || {
    harness
      .shapes
      .iter()
      .filter(|clipped| clipped.shape.visual_bounding_rect().max.y >= viewport.min.y)
      .filter_map(|clipped| match &clipped.shape {
        egui::Shape::Rect(rect) if rect.rect.max.y >= viewport.max.y => {
          Some((rect.fill, rect.rect))
        }
        _ => None,
      })
  };
  let fill = rects_in_panel()
    .find(|(colour, _)| *colour == palette.get(Role::Cut))
    .map(|(_, rect)| rect)
    .expect("the panel painted no progress bar fill in the cut signal");
  // The smallest `Role::Canvas` rectangle that contains the fill: the central panel's own ground is painted in the
  // same role now that the split shares one panel with the drawing (S8), so "the first one" would be the whole
  // canvas rather than the bar's track.
  let trough = rects_in_panel()
    .filter(|(colour, rect)| *colour == palette.get(Role::Canvas) && rect.contains_rect(fill))
    .map(|(_, rect)| rect)
    .min_by(|a, b| {
      (a.width() * a.height())
        .partial_cmp(&(b.width() * b.height()))
        .unwrap_or(std::cmp::Ordering::Equal)
    })
    .expect("the panel painted no progress bar trough under its fill");
  (trough, fill)
}

#[test]
fn the_panel_states_the_elapsed_time_and_the_total_as_clocks() {
  let mut harness = Harness::playing();
  // The fixture lasts exactly one second: 0.2 s rapid, 0.2 s plunge, 0.5 s feed, 0.1 s retract.
  assert_eq!(harness.output.drawn.preview_progress, 0.0);
  assert_eq!(
    harness.output.drawn.preview_clock, "0:00 / 0:01",
    "a program that has not started reads as nothing spent of its total"
  );

  harness
    .shell
    .preview_mut()
    .expect("the harness loaded a preview")
    .set_speed(600.0);
  harness
    .shell
    .preview_mut()
    .expect("the harness loaded a preview")
    .set_playing(true);
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
    harness.output.drawn.preview_clock, "0:01 / 0:01",
    "a finished program reads as all of its time spent"
  );
  assert_eq!(harness.output.drawn.preview_bar, 1.0);
}

#[test]
fn the_progress_bar_is_painted_in_the_cut_signal_and_follows_the_clock() {
  let mut harness = Harness::playing();
  let (trough, start_fill) = bar_rects(&harness);
  assert_eq!(
    start_fill.width(),
    0.0,
    "a program that has not started fills nothing, not even a bead of progress"
  );
  assert!(
    trough.width() > 0.0 && trough.height() > 0.0,
    "the empty bar still draws its track, so the row does not jump when playback starts"
  );

  harness
    .shell
    .preview_mut()
    .expect("the harness loaded a preview")
    .set_speed(600.0);
  harness
    .shell
    .preview_mut()
    .expect("the harness loaded a preview")
    .set_playing(true);
  for _ in 0..10 {
    harness.frame(Vec::new());
    if harness.preview().finished() {
      break;
    }
  }
  let (end_trough, end_fill) = bar_rects(&harness);
  assert_eq!(
    (end_trough.width(), end_trough.height()),
    (trough.width(), trough.height()),
    "the bar cannot change size as it fills"
  );
  assert_eq!(
    end_fill.width(),
    end_trough.width(),
    "a finished program fills the track exactly"
  );
}

#[test]
fn a_program_with_no_time_paints_an_empty_bar_and_says_so() {
  let mut harness = Harness::bare();
  harness
    .shell
    .set_preview(untouched(), timeless_program(), &job());
  harness.frame(Vec::new());
  assert_eq!(
    harness.output.drawn.preview_clock, "0:00 / 0:00",
    "a program with no time to spend states that on both sides"
  );
  assert_eq!(
    harness.output.drawn.preview_bar, 0.0,
    "a clock with no total must not fill the bar"
  );

  harness
    .shell
    .preview_mut()
    .expect("the harness loaded a preview")
    .set_playing(true);
  harness.frame(Vec::new());
  assert!(
    harness.preview().finished(),
    "every move takes no time, so one frame plays the program to its end"
  );
  assert_eq!(
    harness.output.drawn.preview_progress, 1.0,
    "the clock's own answer is unchanged: the program really is played out"
  );
  assert_eq!(
    harness.output.drawn.preview_bar, 0.0,
    "and the bar still claims no time, because there is no total to spend"
  );
  assert_eq!(harness.output.drawn.preview_clock, "0:00 / 0:00");
}

#[test]
fn the_playback_bar_plays_pauses_and_names_the_move_the_playhead_is_on() {
  let mut harness = Harness::drilling();
  harness.frame(Vec::new());

  let words: Vec<&str> = harness
    .output
    .drawn
    .playback_hits
    .iter()
    .map(|(word, _)| word.as_str())
    .collect();
  for word in ["Play", "To the end", "Step", "Reset", "60x"] {
    assert!(
      words.contains(&word),
      "the bar must offer `{word}`, painted: {words:?}"
    );
  }
  assert_eq!(
    harness.output.drawn.playback_readout, "0:00 / 0:05  T22 · Drilling 1 · Z 5.000",
    "the readout states the two clocks and names the move under the playhead"
  );

  harness.click(hit(&harness.output.drawn.playback_hits, "Play"));
  assert!(
    harness.preview().playing(),
    "`Play` starts the program: {:?}",
    harness.output.drawn.playback_readout
  );
  assert!(
    harness
      .output
      .drawn
      .playback_hits
      .iter()
      .any(|(word, _)| word == "Pause"),
    "and the control names what pressing it again would do"
  );

  harness.click(hit(&harness.output.drawn.playback_hits, "Pause"));
  assert!(!harness.preview().playing(), "`Pause` stops it again");

  // `Step` and `Reset` are the two controls S8's list does not name and this shell keeps (M5 A10): the buttons
  // are wired to the playhead they describe, which is a claim about the bar rather than about the preview.
  harness.click(hit(&harness.output.drawn.playback_hits, "Reset"));
  assert_eq!(
    harness.preview().cursor(),
    0,
    "`Reset` rewinds to the first move"
  );
  harness.click(hit(&harness.output.drawn.playback_hits, "Step"));
  assert_eq!(
    harness.preview().cursor(),
    1,
    "`Step` advances exactly one move"
  );
  assert!(
    !harness.preview().playing(),
    "and leaves playback paused, so the next frame cannot undo the step"
  );
}

#[test]
fn to_the_end_cuts_the_whole_program_and_reads_as_the_references_own_line() {
  let mut harness = Harness::new();
  harness.frame(Vec::new());
  harness.click(hit(&harness.output.drawn.playback_hits, "To the end"));

  assert!(
    harness.preview().finished(),
    "`To the end` plays the program out"
  );
  assert_eq!(
    harness.preview().field().volumes(),
    whole_program_field().volumes(),
    "the field it leaves is the whole-program sweep, so `To the end` is a cut and not a jump cut"
  );
  assert_eq!(
    harness.output.drawn.preview_bar, 1.0,
    "and the bar's fill is the clock's own answer"
  );

  // The reference's own line, from a program that runs exactly five seconds and ends at Z 5.000.
  let mut drilling = Harness::drilling();
  drilling.frame(Vec::new());
  drilling.click(hit(&drilling.output.drawn.playback_hits, "To the end"));
  assert_eq!(
    drilling.output.drawn.playback_readout, "0:05 / 0:05  T22 · Drilling 1 · Z 5.000",
    "S8's readout, in its own words"
  );
}

#[test]
fn the_scrubber_seeks_the_playhead_to_the_point_it_was_pressed() {
  let mut harness = Harness::new();
  harness.frame(Vec::new());
  harness.click(hit(&harness.output.drawn.playback_hits, "To the end"));
  assert!(harness.preview().finished());

  let (left, right, y) = harness
    .output
    .drawn
    .scrubber
    .expect("the bar must report the track it drew");
  assert!(right > left, "the scrubber has a length to seek along");

  // Back to the start: seeking is a re-cut, so nothing is removed at zero. The press is on the track's own left
  // end, because a pixel into a 129 px track is already a tenth of a second of program.
  harness.click((left, y));
  assert_eq!(
    harness.preview().cursor(),
    0,
    "the playhead is back at the first move"
  );
  assert!(
    harness.preview().elapsed().abs() < 1.0e-9,
    "and its clock reads nothing spent: {}",
    harness.preview().elapsed()
  );
  assert_eq!(
    harness.preview().field().volumes(),
    untouched().volumes(),
    "a playhead at zero must leave the stock untouched"
  );

  // Half way along the track is half of the program's own time.
  harness.click(((left + right) * 0.5, y));
  let half = harness.preview().duration() * 0.5;
  assert!(
    (harness.preview().elapsed() - half).abs() < 0.01 * harness.preview().duration(),
    "the middle of the track is half the program: {} against {half}",
    harness.preview().elapsed()
  );
}

#[test]
fn the_speed_control_names_the_speed_it_sets() {
  let mut harness = Harness::new();
  harness.frame(Vec::new());

  let speed = harness
    .output
    .drawn
    .playback_hits
    .iter()
    .find(|(word, _)| word.ends_with('x'))
    .map(|(word, at)| (word.clone(), *at))
    .expect("the bar must name the speed it is set to");
  assert_eq!(speed.0, "60x", "the default speed is the preview's own");
  assert_eq!(harness.preview().speed(), 60.0);

  harness.click(speed.1);
  assert_eq!(
    harness.preview().speed(),
    200.0,
    "the chip steps to the next speed"
  );
  assert!(
    harness
      .output
      .drawn
      .playback_hits
      .iter()
      .any(|(word, _)| word == "200x"),
    "and the frame that set it is the frame that says so: {:?}",
    harness.output.drawn.playback_hits
  );
}
