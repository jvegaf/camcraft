//! Behaviour tests for application state and the undo history (A1, A2, A3).

use cam_app::{AppState, Command, CommandError, MAX_HISTORY};
use cam_model::{Document, Job, ModelError};

fn state() -> AppState {
  AppState::new(Document::new(Job::default_sheet()))
}

fn resize(width: f64, height: f64, thickness: f64) -> Command {
  Command::SetJob {
    width,
    height,
    thickness,
  }
}

#[test]
fn a_command_changes_the_document_and_marks_it_dirty() {
  let mut app = state();
  let before = app.document_json();
  assert!(!app.is_dirty());

  app.execute(resize(600.0, 400.0, 12.0)).expect("applies");

  assert!(app.is_dirty());
  assert_eq!(app.document().board.width.mm(), 600.0);
  assert_ne!(app.document_json(), before);
  assert!(app.can_undo());
  assert!(!app.can_redo());
  assert_eq!(app.history_depth(), 1);
}

#[test]
fn undo_then_redo_restores_both_documents_exactly() {
  let mut app = state();
  let original = app.document_json();

  let commands = [
    resize(600.0, 400.0, 12.0),
    resize(800.0, 600.0, 18.0),
    resize(1000.0, 500.0, 9.0),
    resize(400.0, 400.0, 6.0),
    resize(1219.0, 2438.0, 18.0),
  ];
  for command in &commands {
    app.execute(command.clone()).expect("applies");
  }
  let final_state = app.document_json();
  assert_eq!(app.history_depth(), commands.len());

  for _ in 0..commands.len() {
    assert!(app.undo(), "undo must report progress");
  }
  assert_eq!(
    app.document_json(),
    original,
    "undo must restore the document exactly"
  );
  assert!(!app.can_undo());

  for _ in 0..commands.len() {
    assert!(app.redo(), "redo must report progress");
  }
  assert_eq!(
    app.document_json(),
    final_state,
    "redo must re-apply the document exactly"
  );
  assert!(!app.can_redo());
}

#[test]
fn undo_and_redo_on_an_empty_history_are_no_ops() {
  let mut app = state();
  assert!(!app.can_undo());
  assert!(!app.undo());
  assert!(!app.can_redo());
  assert!(!app.redo());
  assert_eq!(app.history_depth(), 0);
}

#[test]
fn a_new_command_discards_the_redo_branch() {
  let mut app = state();
  app.execute(resize(600.0, 400.0, 12.0)).expect("applies");
  app.execute(resize(700.0, 400.0, 12.0)).expect("applies");
  assert!(app.undo());
  assert!(app.can_redo());

  app.execute(resize(900.0, 900.0, 9.0)).expect("applies");
  assert!(
    !app.can_redo(),
    "the abandoned branch must not come back after a new command"
  );
}

#[test]
fn history_is_bounded_and_reports_what_it_dropped() {
  let mut app = state();
  let extra = 10;
  for step in 0..(MAX_HISTORY + extra) {
    app
      .execute(resize(100.0 + step as f64, 400.0, 12.0))
      .expect("applies");
  }
  assert_eq!(app.history_depth(), MAX_HISTORY);
  assert_eq!(app.history_dropped(), extra);
}

#[test]
fn an_invalid_command_is_refused_without_touching_anything() {
  let mut app = state();
  app.execute(resize(600.0, 400.0, 12.0)).expect("applies");
  let before = app.document_json();
  let depth = app.history_depth();

  let error = app
    .execute(resize(600.0, 400.0, 0.0))
    .expect_err("a zero thickness must be refused");
  assert!(
    matches!(
      error,
      CommandError::Invalid(ModelError::NonPositiveJobDimension {
        field: "thickness",
        ..
      })
    ),
    "{error:?}"
  );
  assert_eq!(app.document_json(), before);
  assert_eq!(app.history_depth(), depth);
}

#[test]
fn marking_saved_clears_the_dirty_flag_but_keeps_history() {
  let mut app = state();
  app.execute(resize(600.0, 400.0, 12.0)).expect("applies");
  assert!(app.is_dirty());
  app.mark_saved();
  assert!(!app.is_dirty());
  assert!(
    app.can_undo(),
    "saving must not throw away the undo history"
  );
  app.undo();
  assert!(app.is_dirty(), "undoing after a save makes it dirty again");
}

#[test]
fn selection_is_ordered_and_unknown_ids_are_dropped() {
  let mut app = state();
  app.select(vec!["E9".to_owned(), "E2".to_owned(), "E9".to_owned()]);
  // Neither id exists in the document yet, so both are pruned: the selection never cites ghosts.
  assert!(app.selection().is_empty());

  let mut document = Document::new(Job::default_sheet());
  document.layers.push(cam_model::Layer {
    id: "L1".to_owned(),
    role: cam_model::LayerRole::Outline,
    side: cam_model::Side::Top,
    color: "#000000".to_owned(),
    visible: true,
    source: cam_model::Provenance::default(),
    entities: vec![
      cam_model::Entity {
        id: "E2".to_owned(),
        kind: cam_model::EntityKind::Contour {
          contour: cam_geom::Contour::from_points(
            &[
              cam_geom::Point2::new(0.0, 0.0),
              cam_geom::Point2::new(10.0, 0.0),
              cam_geom::Point2::new(10.0, 10.0),
            ],
            true,
          )
          .expect("triangle"),
        },
      },
      cam_model::Entity {
        id: "E3".to_owned(),
        kind: cam_model::EntityKind::Contour {
          contour: cam_geom::Contour::from_points(
            &[
              cam_geom::Point2::new(20.0, 0.0),
              cam_geom::Point2::new(30.0, 0.0),
              cam_geom::Point2::new(30.0, 10.0),
            ],
            true,
          )
          .expect("triangle"),
        },
      },
    ],
  });
  app.replace_document(document);
  app.select(vec!["E3".to_owned(), "E2".to_owned()]);
  assert_eq!(app.selection().len(), 2);
  assert_eq!(
    app.selection().ids().collect::<Vec<_>>(),
    vec!["E2", "E3"],
    "selection iteration must be deterministic"
  );
}

#[test]
fn entity_ids_are_deterministic_and_monotonic() {
  let mut app = state();
  assert_eq!(app.next_entity_id(), "E1");
  assert_eq!(app.next_entity_id(), "E2");
  assert_eq!(app.next_entity_id(), "E3");
}

#[test]
fn replacing_the_document_resets_history_and_selection() {
  let mut app = state();
  app.execute(resize(600.0, 400.0, 12.0)).expect("applies");
  app.select(vec!["E1".to_owned()]);
  app.replace_document(Document::new(Job::default_sheet()));
  assert!(!app.can_undo());
  assert!(!app.can_redo());
  assert!(app.selection().is_empty());
  assert!(!app.is_dirty());
}

#[test]
fn entity_ids_continue_after_the_ids_already_in_the_document() {
  // Opening a document that already contains E1..E3 and then drawing must not reuse an id.
  let mut document = Document::new(Job::default_sheet());
  document.layers.push(cam_model::Layer {
    id: "L1".to_owned(),
    role: cam_model::LayerRole::Outline,
    side: cam_model::Side::Top,
    color: "#000000".to_owned(),
    visible: true,
    source: cam_model::Provenance::default(),
    entities: (1..=3)
      .map(|index| cam_model::Entity {
        id: format!("E{index}"),
        kind: cam_model::EntityKind::Contour {
          contour: cam_geom::Contour::from_points(
            &[
              cam_geom::Point2::new(0.0, 0.0),
              cam_geom::Point2::new(10.0, 0.0),
              cam_geom::Point2::new(10.0, 10.0),
            ],
            true,
          )
          .expect("triangle"),
        },
      })
      .collect(),
  });
  let mut app = AppState::new(document);
  assert_eq!(app.next_entity_id(), "E4");
  assert_eq!(app.next_entity_id(), "E5");
}

#[test]
fn replacing_the_document_also_ressets_the_id_counter() {
  let mut app = state();
  assert_eq!(app.next_entity_id(), "E1");
  let mut document = Document::new(Job::default_sheet());
  document.layers.push(cam_model::Layer {
    id: "L1".to_owned(),
    role: cam_model::LayerRole::Outline,
    side: cam_model::Side::Top,
    color: "#000000".to_owned(),
    visible: true,
    source: cam_model::Provenance::default(),
    entities: vec![cam_model::Entity {
      id: "E41".to_owned(),
      kind: cam_model::EntityKind::Contour {
        contour: cam_geom::Contour::from_points(
          &[
            cam_geom::Point2::new(0.0, 0.0),
            cam_geom::Point2::new(10.0, 0.0),
            cam_geom::Point2::new(10.0, 10.0),
          ],
          true,
        )
        .expect("triangle"),
      },
    }],
  });
  app.replace_document(document);
  assert_eq!(app.next_entity_id(), "E42");
}

#[test]
fn redo_depth_tracks_the_future_stack() {
  let mut app = state();
  assert_eq!(app.redo_depth(), 0);
  app.execute(resize(600.0, 400.0, 12.0)).expect("applies");
  app.execute(resize(700.0, 400.0, 12.0)).expect("applies");
  assert_eq!(app.redo_depth(), 0);

  app.undo();
  assert_eq!(app.redo_depth(), 1);
  app.undo();
  assert_eq!(app.redo_depth(), 2);

  app.redo();
  assert_eq!(app.redo_depth(), 1);

  // A new command discards the abandoned branch, so nothing remains to redo.
  app.execute(resize(800.0, 800.0, 9.0)).expect("applies");
  assert_eq!(app.redo_depth(), 0);
}

#[test]
fn replacing_a_document_undoably_keeps_the_previous_one_and_reseeds_the_ids() {
  let mut app = state();
  let original = app.document_json();
  assert_eq!(app.history_depth(), 0);

  let mut imported = Document::new(Job::default_sheet());
  imported.layers.push(cam_model::Layer {
    id: "L1".to_owned(),
    role: cam_model::LayerRole::Outline,
    side: cam_model::Side::Top,
    color: "#000000".to_owned(),
    visible: true,
    source: cam_model::Provenance::default(),
    entities: vec![cam_model::Entity {
      id: "E1".to_owned(),
      kind: cam_model::EntityKind::Contour {
        contour: cam_geom::primitive::circle(cam_geom::Point2::ORIGIN, 5.0).expect("circle"),
      },
    }],
  });
  app.replace_document_undoable(imported);

  assert_eq!(app.history_depth(), 1);
  assert!(app.is_dirty());
  assert!(app.selection().is_empty());
  assert_eq!(app.document().entities().count(), 1);
  // The counter is reseeded from the new document, so the next id cannot collide with `E1`.
  assert_eq!(app.next_entity_id(), "E2");

  assert!(app.undo(), "the replacement is one undo step");
  assert_eq!(app.document_json(), original);
}

#[test]
fn opening_a_document_still_clears_the_history() {
  let mut app = state();
  app.execute(resize(600.0, 400.0, 12.0)).expect("applies");
  assert_eq!(app.history_depth(), 1);

  app.replace_document(Document::new(Job::default_sheet()));

  assert_eq!(app.history_depth(), 0);
  assert!(!app.can_undo());
  assert!(!app.is_dirty());
}
