use super::support::*;

fn ids(tools: &[cam_store::tools::StoredTool]) -> Vec<String> {
  tools.iter().map(|tool| tool.geometry.id.clone()).collect()
}

fn question_hit(drawn: &cam_ui::ShellOutput, word: &str) -> egui::Pos2 {
  drawn
    .drawn
    .tool_import_question_hits
    .iter()
    .find(|(name, _)| name == word)
    .map(|(_, point)| egui::pos2(point.0, point.1))
    .unwrap_or_else(|| {
      panic!(
        "nothing called {word:?} is pressable in the import question: {:?}",
        drawn.drawn.tool_import_question_hits
      )
    })
}

#[test]
fn a_valid_import_asks_replace_all_add_or_cancel_and_changes_nothing_yet() {
  let scratch = Scratch::new("question");
  let ctx = egui::Context::default();
  let (mut app, mut shell, _opened, library_path) = opened_library_document(&ctx, &scratch);
  let before = ids(&tools_in(&library_path));
  let incoming = vec![flat_mill("X1", Some(10), 3.0)];
  let file = scratch.path().join("incoming.json");
  std::fs::write(
    &file,
    cam_store::tool_file::to_json(&incoming).expect("library json"),
  )
  .expect("file written");

  shell.apply_action(
    &mut app,
    &cam_ui::UiAction::ImportTools { path: file.clone() },
  );
  let (asked, _) = frame_with(&ctx, &mut shell, &mut app);

  assert_eq!(
    asked.drawn.tool_import_question_title.as_deref(),
    Some("Import tool library")
  );
  assert_eq!(
    asked.drawn.tool_import_question_sentence.as_deref(),
    Some("1 tool(s) in incoming.json. Replace the library, or add only the tools it does not already hold?")
  );
  assert_eq!(
    asked.drawn.tool_import_question_buttons,
    vec![
      "Replace all".to_owned(),
      "Add".to_owned(),
      "Cancel".to_owned()
    ]
  );
  assert!(!asked.drawn.tool_import_question_hits.is_empty());
  assert_eq!(ids(&tools_in(&library_path)), before);
  assert!(
    shell.show_tool_library,
    "the tool-library modal behind the question must stay open"
  );
}

#[test]
fn replace_all_behaves_like_replacing_the_library() {
  let scratch = Scratch::new("replace-all");
  let ctx = egui::Context::default();
  let (mut app, mut shell, _opened, library_path) = opened_library_document(&ctx, &scratch);
  let incoming = vec![flat_mill("X1", Some(10), 3.0), measured("X2", Some(11))];
  let file = scratch.path().join("incoming.json");
  std::fs::write(
    &file,
    cam_store::tool_file::to_json(&incoming).expect("library json"),
  )
  .expect("file written");

  shell.apply_action(&mut app, &cam_ui::UiAction::ImportTools { path: file });
  let _ = frame_with(&ctx, &mut shell, &mut app);
  let (settled, _) = frame_with(&ctx, &mut shell, &mut app);
  click_at(
    &ctx,
    &mut shell,
    &mut app,
    question_hit(&settled, "Replace all"),
  );

  let written = tools_in(&library_path);
  assert_eq!(ids(&written), ids(&incoming));
  let (after, _) = frame_with(&ctx, &mut shell, &mut app);
  assert_eq!(after.drawn.tool_saved, ids(&incoming));
  assert_eq!(after.drawn.tool_import_question_title, None);
  assert!(shell.show_tool_library);
}

#[test]
fn add_appends_only_new_tools_and_reports_counts() {
  let scratch = Scratch::new("add-only-new");
  let ctx = egui::Context::default();
  let (mut app, mut shell, _opened, library_path) = opened_library_document(&ctx, &scratch);
  let existing = library();
  let fresh = flat_mill("X1", Some(99), 4.0);
  let duplicate_by_id = existing[0].clone();
  let incoming = vec![fresh.clone(), duplicate_by_id];
  let file = scratch.path().join("incoming.json");
  std::fs::write(
    &file,
    cam_store::tool_file::to_json(&incoming).expect("library json"),
  )
  .expect("file written");

  shell.apply_action(&mut app, &cam_ui::UiAction::ImportTools { path: file });
  let _ = frame_with(&ctx, &mut shell, &mut app);
  let (settled, _) = frame_with(&ctx, &mut shell, &mut app);
  click_at(&ctx, &mut shell, &mut app, question_hit(&settled, "Add"));

  let written = tools_in(&library_path);
  assert_eq!(written.len(), existing.len() + 1);
  assert!(written
    .iter()
    .any(|tool| tool.geometry.id == fresh.geometry.id));
  let (after, _) = frame_with(&ctx, &mut shell, &mut app);
  assert_eq!(after.drawn.tool_saved, vec![fresh.geometry.id]);
  assert_eq!(
    after.drawn.tool_import_note.as_deref(),
    Some("1 tool(s) added, 1 already in the library.")
  );
  assert_eq!(after.drawn.tool_import_question_title, None);
  assert!(shell.show_tool_library);
}

#[test]
fn add_with_every_tool_already_present_writes_nothing_and_reports_it() {
  let scratch = Scratch::new("add-none");
  let ctx = egui::Context::default();
  let (mut app, mut shell, _opened, library_path) = opened_library_document(&ctx, &scratch);
  let existing = library();
  let incoming = vec![existing[0].clone(), existing[1].clone()];
  let before = ids(&tools_in(&library_path));
  let file = scratch.path().join("incoming.json");
  std::fs::write(
    &file,
    cam_store::tool_file::to_json(&incoming).expect("library json"),
  )
  .expect("file written");

  shell.apply_action(&mut app, &cam_ui::UiAction::ImportTools { path: file });
  let _ = frame_with(&ctx, &mut shell, &mut app);
  let (settled, _) = frame_with(&ctx, &mut shell, &mut app);
  click_at(&ctx, &mut shell, &mut app, question_hit(&settled, "Add"));

  assert_eq!(ids(&tools_in(&library_path)), before);
  let (after, _) = frame_with(&ctx, &mut shell, &mut app);
  assert!(
    after.drawn.tool_saved.is_empty(),
    "nothing was written, so nothing should be reported as saved"
  );
  assert_eq!(
    after.drawn.tool_import_note.as_deref(),
    Some("Nothing was added: every tool in incoming.json is already in the library.")
  );
  assert_eq!(after.drawn.tool_import_question_title, None);
  assert!(shell.show_tool_library);
}

#[test]
fn cancel_and_escape_close_the_question_write_nothing_and_leave_the_library_open() {
  let scratch = Scratch::new("cancel");
  let ctx = egui::Context::default();
  let (mut app, mut shell, _opened, library_path) = opened_library_document(&ctx, &scratch);
  let before = ids(&tools_in(&library_path));
  let incoming = vec![flat_mill("X1", Some(10), 3.0)];
  let file = scratch.path().join("incoming.json");
  std::fs::write(
    &file,
    cam_store::tool_file::to_json(&incoming).expect("library json"),
  )
  .expect("file written");

  // Cancel.
  shell.apply_action(
    &mut app,
    &cam_ui::UiAction::ImportTools { path: file.clone() },
  );
  let _ = frame_with(&ctx, &mut shell, &mut app);
  let (settled, _) = frame_with(&ctx, &mut shell, &mut app);
  click_at(&ctx, &mut shell, &mut app, question_hit(&settled, "Cancel"));
  assert_eq!(ids(&tools_in(&library_path)), before);
  let (after, _) = frame_with(&ctx, &mut shell, &mut app);
  assert_eq!(after.drawn.tool_import_question_title, None);
  assert!(shell.show_tool_library);

  // Escape.
  shell.apply_action(&mut app, &cam_ui::UiAction::ImportTools { path: file });
  let _ = frame_with(&ctx, &mut shell, &mut app);
  press(&ctx, &mut shell, &mut app, egui::Key::Escape);
  assert_eq!(ids(&tools_in(&library_path)), before);
  let (after, _) = frame_with(&ctx, &mut shell, &mut app);
  assert_eq!(after.drawn.tool_import_question_title, None);
  assert!(
    shell.show_tool_library,
    "Escape closes the question but leaves the tool-library modal open"
  );
}

#[test]
fn an_unreadable_file_asks_nothing() {
  let scratch = Scratch::new("unreadable");
  let ctx = egui::Context::default();
  let (mut app, mut shell, _opened, library_path) = opened_library_document(&ctx, &scratch);
  let before = ids(&tools_in(&library_path));
  let file = scratch.path().join("missing.json");

  shell.apply_action(&mut app, &cam_ui::UiAction::ImportTools { path: file });

  let (after, _) = frame_with(&ctx, &mut shell, &mut app);
  assert_eq!(after.drawn.tool_import_question_title, None);
  assert!(
    after.drawn.tool_error.is_some(),
    "a bad file says why and asks nothing"
  );
  assert_eq!(ids(&tools_in(&library_path)), before);
}
