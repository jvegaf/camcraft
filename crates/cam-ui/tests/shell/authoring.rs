use super::support::*;
use cam_app::{AlignMode, AppState, Transform, VertexEdit};
use cam_geom::{Contour, Point2, Vec2};
use cam_ui::{DrawShape, Shell, Tool, UiAction, DEFAULT_TOLERANCE_MM};

/// The contour of `id`, cloned so the document borrow ends before the assertions.
fn contour_of(app: &AppState, id: &str) -> Contour {
  app
    .document()
    .entity(id)
    .and_then(|entity| entity.kind.as_contour())
    .unwrap_or_else(|| panic!("{id} must be a contour"))
    .clone()
}

fn entity_count(app: &AppState) -> usize {
  app.document().entities().count()
}

/// The only selected id.
fn only_selected(app: &AppState) -> String {
  let ids: Vec<String> = app.selection().ids().map(str::to_owned).collect();
  assert_eq!(ids.len(), 1, "exactly one entity must be selected: {ids:?}");
  ids.into_iter().next().expect("one id")
}

#[test]
fn the_draw_group_reaches_every_drawing_command() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let shapes = [
    DrawShape::Rectangle {
      at: Point2::new(0.0, 0.0),
      width: 10.0,
      height: 5.0,
    },
    DrawShape::Line {
      from: Point2::new(0.0, 0.0),
      to: Point2::new(10.0, 0.0),
    },
    DrawShape::Polyline {
      points: vec![
        Point2::new(0.0, 0.0),
        Point2::new(10.0, 0.0),
        Point2::new(10.0, 10.0),
      ],
      closed: false,
    },
    DrawShape::Arc {
      start: Point2::new(0.0, 0.0),
      through: Point2::new(10.0, 10.0),
      end: Point2::new(20.0, 0.0),
    },
    DrawShape::Circle {
      at: Point2::new(50.0, 50.0),
      radius: 10.0,
    },
    DrawShape::Ellipse {
      at: Point2::new(50.0, 50.0),
      rx: 20.0,
      ry: 10.0,
      tolerance: DEFAULT_TOLERANCE_MM,
    },
    DrawShape::Polygon {
      at: Point2::new(50.0, 50.0),
      radius: 10.0,
      sides: 6,
      rotation: 0.0,
    },
    DrawShape::Star {
      at: Point2::new(50.0, 50.0),
      outer_radius: 10.0,
      inner_radius: 5.0,
      points: 5,
      rotation: 0.0,
    },
  ];

  for shape in shapes {
    let before = entity_count(&app);
    shell.apply_action(&mut app, &UiAction::Draw(shape.clone()));
    assert_eq!(
      entity_count(&app),
      before + 1,
      "{shape:?} must add exactly one entity: {}",
      shell.status
    );
    let drawn = only_selected(&app);
    assert_ne!(drawn, "E1", "{shape:?} must draw a new entity");
    assert!(
      app.document().entity(&drawn).is_some(),
      "{shape:?} must select what it drew: {}",
      shell.status
    );
    assert!(
      shell.status.contains("1 entity created"),
      "{shape:?}: {}",
      shell.status
    );
    assert_eq!(app.history_depth(), 1, "{shape:?} must be one undo step");
    shell.apply_action(&mut app, &UiAction::Undo);
    assert_eq!(
      entity_count(&app),
      before,
      "{shape:?} must be undone in one step"
    );
  }
}

#[test]
fn the_transform_group_reaches_translate_rotate_scale_and_mirror() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.apply_action(&mut app, &UiAction::SetSelection(vec!["E1".to_owned()]));

  shell.apply_action(
    &mut app,
    &UiAction::Transform(Transform::Translate {
      delta: Vec2::new(10.0, 5.0),
    }),
  );
  let bounds = contour_of(&app, "E1").bounds();
  assert!(
    bounds.min.is_near(Point2::new(10.0, 5.0), 1.0e-9)
      && bounds.max.is_near(Point2::new(110.0, 65.0), 1.0e-9),
    "translate: {bounds:?}"
  );

  shell.apply_action(
    &mut app,
    &UiAction::Transform(Transform::Rotate {
      radians: std::f64::consts::PI,
      about: Point2::ORIGIN,
    }),
  );
  let bounds = contour_of(&app, "E1").bounds();
  assert!(
    bounds.min.is_near(Point2::new(-110.0, -65.0), 1.0e-9)
      && bounds.max.is_near(Point2::new(-10.0, -5.0), 1.0e-9),
    "rotate: {bounds:?}"
  );

  shell.apply_action(
    &mut app,
    &UiAction::Transform(Transform::Scale {
      sx: 2.0,
      sy: 2.0,
      about: Point2::ORIGIN,
      tolerance: DEFAULT_TOLERANCE_MM,
    }),
  );
  let bounds = contour_of(&app, "E1").bounds();
  assert!(
    bounds.min.is_near(Point2::new(-220.0, -130.0), 1.0e-9)
      && bounds.max.is_near(Point2::new(-20.0, -10.0), 1.0e-9),
    "scale: {bounds:?}"
  );

  shell.apply_action(
    &mut app,
    &UiAction::Transform(Transform::Mirror {
      point: Point2::ORIGIN,
      direction: Vec2::Y_AXIS,
    }),
  );
  let bounds = contour_of(&app, "E1").bounds();
  assert!(
    bounds.min.is_near(Point2::new(20.0, -130.0), 1.0e-9)
      && bounds.max.is_near(Point2::new(220.0, -10.0), 1.0e-9),
    "mirror: {bounds:?}"
  );
  assert_eq!(
    app.history_depth(),
    4,
    "each transform is one undo step: {}",
    shell.status
  );
}

#[test]
fn the_array_group_copies_the_selection_and_keeps_the_id_counter_in_step() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.apply_action(&mut app, &UiAction::SetSelection(vec!["E1".to_owned()]));

  shell.apply_action(
    &mut app,
    &UiAction::Array {
      rows: 2,
      columns: 2,
      dx: 200.0,
      dy: 100.0,
    },
  );
  assert_eq!(
    entity_count(&app),
    5,
    "a 2x2 array of one entity adds four copies: {}",
    shell.status
  );
  assert!(
    shell.status.contains("4 entities created"),
    "{}",
    shell.status
  );
  assert_eq!(
    app.selection().len(),
    4,
    "the copies are selected, the source is not"
  );

  // The ids the array minted must not collide with the next draw: the shell has to move its id
  // counter with the batch, or the next shape is refused as a duplicate.
  shell.apply_action(
    &mut app,
    &UiAction::Draw(DrawShape::Rectangle {
      at: Point2::new(0.0, 0.0),
      width: 1.0,
      height: 1.0,
    }),
  );
  assert!(
    !shell.status.contains("refused"),
    "a draw after an array must not collide: {}",
    shell.status
  );
  assert_eq!(entity_count(&app), 6, "{}", shell.status);
}

#[test]
fn the_align_group_lines_the_selection_up_on_its_own_bounds() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.apply_action(
    &mut app,
    &UiAction::Draw(DrawShape::Rectangle {
      at: Point2::new(200.0, 20.0),
      width: 20.0,
      height: 20.0,
    }),
  );
  let added = only_selected(&app);
  shell.apply_action(
    &mut app,
    &UiAction::SetSelection(vec!["E1".to_owned(), added.clone()]),
  );

  shell.apply_action(&mut app, &UiAction::Align(AlignMode::Left));

  let bounds = contour_of(&app, &added).bounds();
  assert!(
    (bounds.min.x - 0.0).abs() < 1.0e-9,
    "aligning left puts every left edge on the group's left edge: {bounds:?}"
  );
  assert!(
    (bounds.min.y - 20.0).abs() < 1.0e-9,
    "aligning left leaves the other axis alone: {bounds:?}"
  );
}

#[test]
fn the_offset_group_adds_a_contour_and_keeps_the_source() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.apply_action(&mut app, &UiAction::SetSelection(vec!["E1".to_owned()]));

  shell.apply_action(&mut app, &UiAction::Offset { distance: 2.0 });

  assert_eq!(entity_count(&app), 2, "{}", shell.status);
  let created = only_selected(&app);
  assert_ne!(created, "E1", "the offset is a new entity");
  let bounds = contour_of(&app, &created).bounds();
  assert!(
    bounds.min.is_near(Point2::new(-2.0, -2.0), 1.0e-9)
      && bounds.max.is_near(Point2::new(102.0, 62.0), 1.0e-9),
    "a rectangle offset by 2 mm grows by 2 mm on every side: {bounds:?}"
  );
  let source = contour_of(&app, "E1").bounds();
  assert!(
    source.min.is_near(Point2::ORIGIN, 1.0e-9),
    "the source must not move: {source:?}"
  );
}

#[test]
fn the_join_group_merges_touching_entities_into_one() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.apply_action(
    &mut app,
    &UiAction::Draw(DrawShape::Line {
      from: Point2::new(0.0, 0.0),
      to: Point2::new(10.0, 0.0),
    }),
  );
  let first = only_selected(&app);
  shell.apply_action(
    &mut app,
    &UiAction::Draw(DrawShape::Line {
      from: Point2::new(10.0, 0.0),
      to: Point2::new(10.0, 10.0),
    }),
  );
  let second = only_selected(&app);
  shell.apply_action(
    &mut app,
    &UiAction::SetSelection(vec![first.clone(), second.clone()]),
  );

  shell.apply_action(&mut app, &UiAction::Join);

  assert_eq!(
    entity_count(&app),
    2,
    "the rectangle plus the joined line: {}",
    shell.status
  );
  assert!(
    app.document().entity(&second).is_none(),
    "the second line is merged into the first"
  );
  let joined = contour_of(&app, &first);
  assert!(!joined.is_closed(), "two lines joined end to end stay open");
  assert!(
    (joined.perimeter() - 20.0).abs() < 1.0e-9,
    "joining loses no length, got {}",
    joined.perimeter()
  );
}

#[test]
fn the_trim_group_replaces_the_target_with_the_kept_piece() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.apply_action(
    &mut app,
    &UiAction::Draw(DrawShape::Line {
      from: Point2::new(50.0, -10.0),
      to: Point2::new(50.0, 70.0),
    }),
  );
  let cutter = only_selected(&app);

  shell.apply_action(
    &mut app,
    &UiAction::Trim {
      target: "E1".to_owned(),
      cutter: cutter.clone(),
    },
  );

  assert_eq!(
    entity_count(&app),
    2,
    "the rectangle becomes one open piece beside the cutter: {}",
    shell.status
  );
  assert!(
    app.document().entity("E1").is_none(),
    "the trimmed target id is freed"
  );
  let kept = only_selected(&app);
  assert_ne!(kept, cutter, "the kept piece is the new entity");
  assert!(
    !contour_of(&app, &kept).is_closed(),
    "trimming a closed contour opens it"
  );
}

#[test]
fn the_vertex_edit_group_moves_inserts_and_deletes_nodes() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.apply_action(&mut app, &UiAction::SetSelection(vec!["E1".to_owned()]));

  shell.apply_action(
    &mut app,
    &UiAction::EditVertex(VertexEdit::Move {
      index: 0,
      to: Point2::new(-10.0, -10.0),
    }),
  );
  assert!(
    contour_of(&app, "E1").vertices()[0]
      .point
      .is_near(Point2::new(-10.0, -10.0), 1.0e-9),
    "moving vertex 0 must move it: {}",
    shell.status
  );

  shell.apply_action(
    &mut app,
    &UiAction::EditVertex(VertexEdit::Insert {
      segment_index: 1,
      at: Point2::new(110.0, 30.0),
    }),
  );
  assert_eq!(
    contour_of(&app, "E1").len(),
    5,
    "inserting a node adds one vertex: {}",
    shell.status
  );

  shell.apply_action(
    &mut app,
    &UiAction::EditVertex(VertexEdit::Delete { index: 2 }),
  );
  assert_eq!(
    contour_of(&app, "E1").len(),
    4,
    "deleting the inserted node removes it again: {}",
    shell.status
  );
  assert_eq!(app.history_depth(), 3, "each node edit is one undo step");
}

#[test]
fn the_text_group_adds_one_entity_per_glyph_stroke() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();

  shell.apply_action(
    &mut app,
    &UiAction::AddText {
      text: "AB".to_owned(),
      height: 10.0,
      origin: Point2::new(0.0, -20.0),
    },
  );

  let added = entity_count(&app) - 1;
  assert!(
    added >= 2,
    "\"AB\" must draw more than one stroke, got {added}: {}",
    shell.status
  );
  assert_eq!(
    app.selection().len(),
    added,
    "the whole word is selected: {}",
    shell.status
  );
  assert!(
    shell.status.contains("entities created"),
    "{}",
    shell.status
  );

  // Text mints an unknown number of ids, so the counter has to be topped up by what was created.
  shell.apply_action(
    &mut app,
    &UiAction::Draw(DrawShape::Rectangle {
      at: Point2::new(0.0, 0.0),
      width: 1.0,
      height: 1.0,
    }),
  );
  assert!(
    !shell.status.contains("refused"),
    "a draw after text must not collide: {}",
    shell.status
  );
}

#[test]
fn a_non_uniform_scale_reports_the_arcs_it_lost_in_the_status_bar() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.apply_action(
    &mut app,
    &UiAction::Draw(DrawShape::Circle {
      at: Point2::ORIGIN,
      radius: 10.0,
    }),
  );

  shell.apply_action(
    &mut app,
    &UiAction::Transform(Transform::Scale {
      sx: 2.0,
      sy: 1.0,
      about: Point2::ORIGIN,
      tolerance: DEFAULT_TOLERANCE_MM,
    }),
  );

  let (output, _) = frame(&mut shell, &mut app);
  assert!(
    output
      .drawn
      .status_line
      .contains("2 arc(s) lost to tessellation"),
    "a circle has two arcs, and the status bar must say both were lost: {}",
    output.drawn.status_line
  );
}

#[test]
fn drawing_an_ellipse_reports_the_radial_error_its_arcs_achieved() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.apply_action(
    &mut app,
    &UiAction::Draw(DrawShape::Ellipse {
      at: Point2::ORIGIN,
      rx: 40.0,
      ry: 10.0,
      tolerance: DEFAULT_TOLERANCE_MM,
    }),
  );

  let (output, _) = frame(&mut shell, &mut app);
  assert!(
    output.drawn.status_line.contains("radial error"),
    "the achieved error must reach the user: {}",
    output.drawn.status_line
  );
}

#[test]
fn a_refused_command_puts_the_kernel_reason_in_the_status_bar_and_changes_nothing() {
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.apply_action(
    &mut app,
    &UiAction::Draw(DrawShape::Circle {
      at: Point2::ORIGIN,
      radius: 10.0,
    }),
  );
  let before = app.document().clone();
  let depth = app.history_depth();

  // Offsetting a radius-10 circle inwards by 12 mm has nowhere to go; the kernel refuses it.
  shell.apply_action(&mut app, &UiAction::Offset { distance: -12.0 });

  assert!(
    shell.status.contains("refused"),
    "a refusal must say so: {}",
    shell.status
  );
  assert!(
    shell.status.contains("arc radius would collapse"),
    "the kernel's own reason must survive: {}",
    shell.status
  );
  assert_eq!(
    app.document(),
    &before,
    "a refused command must leave the document untouched"
  );
  assert_eq!(
    app.history_depth(),
    depth,
    "a refused command must not grow the history"
  );

  let (output, _) = frame(&mut shell, &mut app);
  assert!(
    output
      .drawn
      .status_line
      .contains("arc radius would collapse"),
    "the refusal must be on screen: {}",
    output.drawn.status_line
  );
}

#[test]
fn the_tools_tab_offers_every_authoring_command() {
  let mut app = AppState::new(document_with_rectangle());
  // The authoring body lives behind `TOOLPATHS` since camui-prancheta S6 replaced the tab row with the
  // ribbon: the panel's idle state is the selection, so a test that wants the command list opens the task
  // the way an operator does.
  let mut shell = Shell::new();
  shell.open_task(cam_ui::Category::Toolpaths);
  let ctx = egui::Context::default();
  let (output, first) = frame_with(&ctx, &mut shell, &mut app);
  // Below the ribbon and the task header: the pointer has to be over the scrolling body for the wheel to
  // reach it, and since S6 the panel carries three category rows above that body.
  let inspector = egui::pos2(
    (output.canvas.x + output.canvas.width) as f32 + 100.0,
    (output.canvas.y + 260.0) as f32,
  );
  let mut texts = first;
  texts.extend(texts_scrolled_to_the_end(
    &ctx, &mut shell, &mut app, inspector,
  ));
  let all = joined(&texts);
  for label in [
    "Select",
    "Line",
    "Rectangle",
    "Polyline",
    "Arc",
    "Circle",
    "Ellipse",
    "Polygon",
    "Star",
    "Move",
    "Rotate",
    "Scale",
    "Mirror",
    "Array",
    "Left",
    "Right",
    "Top",
    "Bottom",
    "Centre H",
    "Centre V",
    "Offset",
    "Join",
    "Trim",
    "Move node",
    "Insert node",
    "Delete node",
    "Add text",
    "Add rectangle",
  ] {
    assert!(all.contains(label), "missing {label:?} in {all}");
  }
}

#[test]
fn every_tool_can_be_armed_and_the_shell_still_runs_headless() {
  for tool in Tool::ALL {
    let mut app = AppState::new(document_with_rectangle());
    let mut shell = Shell::new();
    shell.active_tool = tool;
    let (output, texts) = frame(&mut shell, &mut app);
    assert!(
      output.canvas.width > 0.0 && output.canvas.height > 0.0,
      "{tool:?} must still lay out a canvas"
    );
    assert!(
      output.drawn.painted_items > 0,
      "{tool:?} must still paint the document"
    );
    assert!(
      output.actions.is_empty(),
      "{tool:?} must not act without input: {:?}",
      output.actions
    );
    assert!(joined(&texts).contains("Inspector"));
  }
}
