use super::support::*;
use cam_app::AppState;
use cam_geom::Point2;
use cam_ui::{handle_action, DrawShape, Shell, Tool, UiAction};

#[test]
fn the_status_bar_chips_are_the_references_five_and_the_grid_names_its_step() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  // Two frames: the canvas measures the camera after the panels, so the first frame's bar cannot know the
  // grid step yet, exactly like the zoom it reports beside it.
  let _ = frame(&mut shell, &mut app);
  let (output, texts) = frame(&mut shell, &mut app);
  let labels: Vec<String> = output
    .drawn
    .status_chips
    .iter()
    .map(|(label, _)| label.clone())
    .collect();
  assert_eq!(
    labels.len(),
    5,
    "the reference draws five chips, this bar drew {labels:?}"
  );
  assert_eq!(
    &labels[..4],
    ["Snaps", "Point", "Mid", "Centre"],
    "{labels:?}"
  );
  let step = labels[4]
    .strip_prefix("Grid ")
    .and_then(|value| value.parse::<f64>().ok())
    .unwrap_or_else(|| {
      panic!(
        "the grid chip must name its step in millimetres: {:?}",
        labels[4]
      )
    });
  assert_eq!(
    step, shell.grid_spacing,
    "the chip and the shell must name the same step"
  );
  let all = joined(&texts);
  for label in &labels {
    assert!(all.contains(label), "missing {label:?} in {all}");
  }
}

#[test]
fn the_snaps_chip_arms_and_disarms_the_magnet() {
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let _ = frame_with(&ctx, &mut shell, &mut app);
  assert!(shell.snapping, "the shell snaps until told otherwise");

  click_status_chip(&ctx, &mut shell, &mut app, "Snaps");
  assert!(!shell.snapping, "the Snaps chip turns the whole magnet off");

  click_status_chip(&ctx, &mut shell, &mut app, "Snaps");
  assert!(shell.snapping, "and turns it back on");
}

#[test]
fn the_point_chip_arms_and_disarms_its_own_snap_kind() {
  use cam_view::SnapKind;
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let _ = frame_with(&ctx, &mut shell, &mut app);
  assert!(
    shell.snap_armed(SnapKind::Endpoint),
    "endpoints start armed"
  );

  click_status_chip(&ctx, &mut shell, &mut app, "Point");
  assert!(
    !shell.snap_armed(SnapKind::Endpoint),
    "the Point chip is the endpoint snap, not a neighbour"
  );
  assert!(
    shell.snap_armed(SnapKind::Midpoint),
    "and it leaves the other kinds alone"
  );
}

#[test]
fn the_grid_chip_arms_the_grid_snap_its_own_word_names() {
  use cam_view::SnapKind;
  let ctx = egui::Context::default();
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let _ = frame_with(&ctx, &mut shell, &mut app);
  assert!(shell.snap_armed(SnapKind::Grid), "the grid starts armed");

  let label = click_status_chip(&ctx, &mut shell, &mut app, "Grid");
  assert!(label.starts_with("Grid "), "{label}");
  assert!(
    !shell.snap_armed(SnapKind::Grid),
    "the grid chip disarms the grid"
  );
}

#[test]
fn the_status_bar_says_what_just_happened_at_its_right_end() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.apply_action(
    &mut app,
    &UiAction::Draw(DrawShape::Circle {
      at: Point2::ORIGIN,
      radius: 10.0,
    }),
  );
  shell.apply_action(&mut app, &UiAction::Offset { distance: -12.0 });
  let happened = shell.status.clone();
  assert!(
    happened.contains("refused"),
    "the setup must produce a sentence to show: {happened}"
  );

  let (output, texts) = frame(&mut shell, &mut app);
  assert_eq!(
    output.drawn.message, happened,
    "the right end reports the sentence the last action produced"
  );
  let head: String = happened.chars().take(24).collect();
  assert!(
    joined(&texts).contains(&head),
    "and paints it, beginning {head:?} in {:?}",
    joined(&texts)
  );
}

#[test]
fn the_snap_chips_arm_and_disarm_a_kind_but_never_leave_none() {
  use cam_view::SnapKind;
  // No document and no frame: arming a snap kind is shell state, and this is the whole rule.
  let mut shell = Shell::new();
  assert!(shell.snap_armed(SnapKind::Midpoint));

  shell.toggle_snap(SnapKind::Midpoint);
  assert!(
    !shell.snap_armed(SnapKind::Midpoint),
    "a chip turns its kind off"
  );
  shell.toggle_snap(SnapKind::Midpoint);
  assert!(shell.snap_armed(SnapKind::Midpoint), "and back on");

  // The last armed kind stays: a snapping strip with everything off is a magnet with no field.
  for kind in [SnapKind::Endpoint, SnapKind::Midpoint, SnapKind::Center] {
    shell.toggle_snap(kind);
  }
  shell.toggle_snap(SnapKind::Grid);
  assert!(
    shell.snap_armed(SnapKind::Grid),
    "the last kind cannot be disarmed"
  );
}

#[test]
fn the_hint_names_the_current_gesture() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let (output, texts) = frame(&mut shell, &mut app);
  assert!(
    output.drawn.hint.contains("left→right"),
    "Select must explain the marquee directions: {}",
    output.drawn.hint
  );
  assert!(
    joined(&texts).contains("left→right"),
    "and the bar must show it"
  );

  shell.active_tool = Tool::Line;
  let (output, _) = frame(&mut shell, &mut app);
  assert!(
    output.drawn.hint.starts_with("Line"),
    "an armed tool names itself: {}",
    output.drawn.hint
  );

  shell.set_view_3d(true);
  let (output, _) = frame(&mut shell, &mut app);
  assert!(
    output.drawn.hint.contains("orbit"),
    "the 3D view has its own gestures: {}",
    output.drawn.hint
  );
}

#[test]
fn the_status_bar_reports_zoom_and_history() {
  // The save state is the rail's brand block (camui-prancheta S4, T2b-ii), which names the moment the file
  // was written; the bar keeps the two facts nothing else on the frame reports.
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let (output, _) = frame(&mut shell, &mut app);
  let line = &output.drawn.status_line;
  assert!(line.contains("px/mm"), "{line}");
  assert!(line.contains("undo 0"), "{line}");
}

#[test]
fn the_status_bar_distinguishes_undo_from_redo() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let _ = frame(&mut shell, &mut app);
  handle_action(
    &mut app,
    &UiAction::AddRectangle {
      width: 10.0,
      height: 10.0,
    },
  )
  .expect("added");
  handle_action(&mut app, &UiAction::Undo).expect("undone");

  let (output, _) = frame(&mut shell, &mut app);
  let line = &output.drawn.status_line;
  assert!(line.contains("undo 0 / redo 1"), "{line}");
  assert!(!line.contains("dropped"), "{line}");
}

#[test]
fn dropped_history_is_reported_separately_from_redo() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let _ = frame(&mut shell, &mut app);
  for _ in 0..(cam_app::MAX_HISTORY + 3) {
    handle_action(&mut app, &UiAction::SetSelection(Vec::new())).expect("no-op");
    app
      .execute(cam_app::Command::SetJob {
        width: 100.0,
        height: 100.0,
        thickness: 10.0,
      })
      .expect("applies");
  }
  let (output, _) = frame(&mut shell, &mut app);
  let line = &output.drawn.status_line;
  assert!(line.contains("3 step(s) dropped"), "{line}");
  assert!(line.contains("redo 0"), "{line}");
}

// ---------------------------------------------------------------------------------------------
// M2 authoring (A13): every authoring command is one `UiAction` handled by the same function the
// shell uses, and the status bar reports what the command produced.
// ---------------------------------------------------------------------------------------------
