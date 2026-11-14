use super::support::*;
use cam_app::AppState;
use cam_geom::Point2;
use cam_model::Job;
use cam_move::MoveStream;
use cam_sim::Heightfield;
use cam_store::document::DocumentFile;
use cam_ui::help::Command as HelpCommand;
use cam_ui::{handle_action, Shell, UiAction};
use cam_units::Length;

#[test]
fn opening_and_saving_stamp_the_brand_block_with_the_files_own_write() {
  // L7's rule: the line stamps the document. The stamp therefore comes from the file the action read, which
  // is the file's mtime — not from `now`, and not from the binary.
  let directory = std::env::temp_dir().join(format!("campilot-stamp-{}", std::process::id()));
  std::fs::remove_dir_all(&directory).ok();
  std::fs::create_dir_all(&directory).expect("scratch directory");
  let path = write_c2d(&directory, "stamped.cmc", &document_with_rectangle());
  let opened_at = std::fs::metadata(&path)
    .and_then(|metadata| metadata.modified())
    .expect("the file has a modification time");

  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.apply_action(&mut app, &UiAction::OpenFile { path: path.clone() });

  let stamp = cam_ui::stamp::format_stamp(opened_at);
  assert_eq!(shell.saved_stamp(), format!("Saved {stamp}"));
  let (_, texts) = frame(&mut shell, &mut app);
  assert!(
    joined(&texts).contains(&format!("Saved {stamp}")),
    "the rail must paint the file's own stamp: {}",
    joined(&texts)
  );

  // An edit makes the document unsaved without changing what the file last was: the block says the first,
  // because that is the question an operator asks before closing.
  shell.apply_action(
    &mut app,
    &UiAction::AddRectangle {
      width: 10.0,
      height: 10.0,
    },
  );
  assert!(app.is_dirty(), "the rectangle is an unsaved edit");
  let (_, texts) = frame(&mut shell, &mut app);
  assert!(
    joined(&texts).contains("Unsaved changes"),
    "an edited drawing reads unsaved: {}",
    joined(&texts)
  );

  // Saving it stamps it again, from the write that just happened.
  shell.apply_action(&mut app, &UiAction::Save);
  assert!(!app.is_dirty(), "a save writes the drawing");
  let written_at = std::fs::metadata(&path)
    .and_then(|metadata| metadata.modified())
    .expect("the file has a modification time");
  assert_eq!(
    shell.saved_stamp(),
    format!("Saved {}", cam_ui::stamp::format_stamp(written_at)),
    "the stamp follows the write"
  );
  std::fs::remove_dir_all(&directory).ok();
}

/// A two-line DXF, written to a scratch directory, so the shell test drives a real importer.
fn write_dxf(directory: &std::path::Path) -> std::path::PathBuf {
  let dxf = "0\nSECTION\n2\nENTITIES\n\
0\nLINE\n8\n0\n10\n0.0\n20\n0.0\n11\n100.0\n21\n0.0\n\
0\nLINE\n8\n0\n10\n100.0\n20\n0.0\n11\n100.0\n21\n60.0\n\
0\nENDSEC\n0\nEOF\n";
  let path = directory.join("part.dxf");
  std::fs::write(&path, dxf).expect("dxf written");
  path
}

#[test]
fn importing_a_file_reports_what_it_found_before_replacing_the_drawing() {
  let directory = scratch("import");
  let path = write_dxf(&directory);
  let mut app = AppState::new(document_with_rectangle());
  let before = app.document_json();
  let mut shell = Shell::new();

  shell.apply_action(
    &mut app,
    &UiAction::ImportFile {
      path: path.display().to_string(),
    },
  );

  // The action parses the file and opens the modal (shell-file-operations S3): nothing replaces the drawing
  // until the operator has read what was found and pressed `Import`.
  assert!(
    shell.show_import,
    "the import modal must open on the file that was just read"
  );
  assert_eq!(shell.status, "import file — import pending");
  assert_eq!(
    app.document_json(),
    before,
    "an unconfirmed import leaves the drawing it was offered over"
  );
  assert_eq!(
    app.history_depth(),
    0,
    "an unconfirmed import records no step"
  );
  assert!(!app.is_dirty(), "an unconfirmed import edits nothing");

  // The dialog paints the parse's own report: the file's name in the title, then the format and the three
  // counts. An egui modal positions itself on a sizing pass, so the paint is read from the second frame — and a
  // frame of the open modal must not import by itself either.
  let ctx = egui::Context::default();
  let _ = frame_with(&ctx, &mut shell, &mut app);
  let (_, texts) = frame_with(&ctx, &mut shell, &mut app);
  let all = joined(&texts);
  assert!(
    all.contains("Import part.dxf"),
    "the title must name the file that was read: {all}"
  );
  assert!(
    all.contains("format dxf · 2 contour(s) found, 0 curve(s) flattened, 0 skipped"),
    "the dialog must report what the parse found: {all}"
  );
  assert_eq!(
    app.document_json(),
    before,
    "a frame of the open modal imports nothing"
  );

  let report = shell
    .confirm_import(&mut app)
    .expect("the parsed file imports");
  assert_eq!(report.created, 2);
  assert!(!shell.show_import, "a completed import closes the modal");
  assert_eq!(app.document().entities().count(), 2);
  assert_eq!(
    app.selection().len(),
    2,
    "the imported contours are what the shell selected"
  );
  assert_eq!(
    shell.status,
    "import file — 2 entities created, 2 contour(s) from part.dxf, 0 curve(s) flattened, 0 skipped"
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_failed_import_leaves_the_drawing_untouched() {
  let mut app = AppState::new(document_with_rectangle());
  let before = app.document_json();
  let mut shell = Shell::new();

  shell.apply_action(
    &mut app,
    &UiAction::ImportFile {
      path: "/nonexistent/part.dxf".to_owned(),
    },
  );

  assert_eq!(app.document_json(), before);
  assert_eq!(app.history_depth(), 0);
  assert!(!app.is_dirty());
  assert!(
    !shell.show_import,
    "a file that could not be read must not open a report about it"
  );
  assert!(shell.status.starts_with("refused: "), "{}", shell.status);
  assert!(
    shell.status.contains("/nonexistent/part.dxf"),
    "{}",
    shell.status
  );
}

#[test]
fn escape_closes_the_import_modal_and_imports_nothing() {
  let directory = scratch("import-esc");
  let path = write_dxf(&directory);
  let mut app = AppState::new(document_with_rectangle());
  let before = app.document_json();
  let mut shell = Shell::new();
  shell.apply_action(
    &mut app,
    &UiAction::ImportFile {
      path: path.display().to_string(),
    },
  );
  assert!(shell.show_import);

  // One frame first, so the modal is on screen when the key is read; `Esc` is then the Cancel the footer offers.
  let ctx = egui::Context::default();
  let _ = frame_with(&ctx, &mut shell, &mut app);
  let _ = press(&ctx, &mut shell, &mut app, egui::Key::Escape);

  assert!(!shell.show_import, "Esc must close the import modal");
  assert!(
    shell.import_path.is_none() && shell.import_parsed.is_none(),
    "a cancelled import forgets the file it was describing"
  );
  assert_eq!(
    app.document_json(),
    before,
    "a cancelled import leaves the drawing alone"
  );
  assert_eq!(app.history_depth(), 0, "a cancelled import records no step");
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn one_undo_after_an_import_brings_the_previous_drawing_back() {
  let directory = scratch("import-undo");
  let path = write_dxf(&directory);
  let mut app = AppState::new(document_with_rectangle());
  let original = app.document_json();
  let mut shell = Shell::new();

  shell.apply_action(
    &mut app,
    &UiAction::ImportFile {
      path: path.display().to_string(),
    },
  );
  shell
    .confirm_import(&mut app)
    .expect("the parsed file imports");
  assert_eq!(app.document().entities().count(), 2);

  shell.apply_action(&mut app, &UiAction::Undo);

  assert_eq!(app.document_json(), original);
  std::fs::remove_dir_all(&directory).ok();
}

// ---- The import options the modal offers (shell-file-operations S3) ----------------------------

/// A one-curve SVG: a cubic Bézier, so the flattening tolerance changes the polyline it becomes.
fn write_curved_svg(directory: &std::path::Path) -> std::path::PathBuf {
  let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100mm" height="100mm" viewBox="0 0 100 100">\
<path d="M 0 0 C 0 40 40 40 40 0"/></svg>"#;
  let path = directory.join("curve.svg");
  std::fs::write(&path, svg).expect("svg written");
  path
}

/// An open LWPOLYLINE drifting 0.001 mm off the line it draws: Douglas–Peucker at the default
/// flattening tolerance drops the interior vertices, a finer tolerance keeps them.
fn write_wavy_dxf(directory: &std::path::Path) -> std::path::PathBuf {
  let dxf = "0\nSECTION\n2\nENTITIES\n\
0\nLWPOLYLINE\n8\n0\n90\n5\n70\n0\n\
10\n0.0\n20\n0.0\n\
10\n10.0\n20\n0.001\n\
10\n20.0\n20\n0.0\n\
10\n30.0\n20\n0.001\n\
10\n40.0\n20\n0.0\n\
0\nENDSEC\n0\nEOF\n";
  let path = directory.join("wavy.dxf");
  std::fs::write(&path, dxf).expect("dxf written");
  path
}

/// A two-line DXF whose lines share no endpoint, so no chain can be formed.
fn write_parallel_dxf(directory: &std::path::Path) -> std::path::PathBuf {
  let dxf = "0\nSECTION\n2\nENTITIES\n\
0\nLINE\n8\n0\n10\n0.0\n20\n0.0\n11\n100.0\n21\n0.0\n\
0\nLINE\n8\n0\n10\n0.0\n20\n50.0\n11\n100.0\n21\n50.0\n\
0\nENDSEC\n0\nEOF\n";
  let path = directory.join("parallel.dxf");
  std::fs::write(&path, dxf).expect("dxf written");
  path
}

/// A closed LWPOLYLINE with one collinear vertex, so smoothing can drop it without opening the ring.
fn write_closed_polyline_dxf(directory: &std::path::Path) -> std::path::PathBuf {
  let dxf = "0\nSECTION\n2\nENTITIES\n\
0\nLWPOLYLINE\n8\n0\n90\n5\n70\n1\n\
10\n0.0\n20\n0.0\n\
10\n50.0\n20\n0.0\n\
10\n100.0\n20\n0.0\n\
10\n100.0\n20\n60.0\n\
10\n0.0\n20\n60.0\n\
0\nENDSEC\n0\nEOF\n";
  let path = directory.join("closed.dxf");
  std::fs::write(&path, dxf).expect("dxf written");
  path
}

/// The bounds of every contour the document holds.
fn document_bounds(app: &AppState) -> cam_geom::Bounds {
  let mut bounds = cam_geom::Bounds::empty();
  for entity in app.document().entities() {
    if let Some(contour) = entity.kind.as_contour() {
      bounds = bounds.union(contour.bounds());
    }
  }
  bounds
}

/// How many vertices the modal's parse holds for its one contour.
fn parsed_vertices(shell: &Shell) -> usize {
  shell
    .import_parsed
    .as_ref()
    .expect("the modal is showing a parse")
    .layers
    .first()
    .and_then(|layer| layer.contours.first())
    .expect("the file held one contour")
    .len()
}

#[test]
fn changing_the_flattening_tolerance_re_parses_the_file() {
  let directory = scratch("import-tolerance");
  let path = write_curved_svg(&directory);
  let mut app = AppState::new(document_with_rectangle());
  let before = app.document_json();
  let mut shell = Shell::new();

  shell.apply_action(
    &mut app,
    &UiAction::ImportFile {
      path: path.display().to_string(),
    },
  );

  // The field opens at the importer's own default, and the report echoes the number the parse used.
  assert_eq!(shell.import_tolerance, cam_import::DEFAULT_TOLERANCE);
  assert_eq!(
    shell
      .import_parsed
      .as_ref()
      .expect("the modal is showing a parse")
      .report
      .tolerance,
    cam_import::DEFAULT_TOLERANCE
  );
  let fine = parsed_vertices(&shell);

  let report = shell
    .set_import_tolerance(1.0)
    .expect("a coarser tolerance re-parses the same file");

  // The re-parse is a real one: the report echoes the new option and the curve is flattened to fewer points.
  assert_eq!(shell.import_tolerance, 1.0);
  assert_eq!(
    shell
      .import_parsed
      .as_ref()
      .expect("the modal is showing a parse")
      .report
      .tolerance,
    1.0,
    "the report must echo the tolerance the re-parse used"
  );
  let coarse = parsed_vertices(&shell);
  assert!(
    coarse < fine,
    "a coarser tolerance flattens the curve to fewer points: {coarse} vs {fine}"
  );
  assert_eq!(report.label, "import file");
  assert_eq!(
    shell.status,
    "import file — tolerance 1 mm · 1 contour(s) found, 1 curve(s) flattened, 0 skipped"
  );
  assert!(shell.show_import, "changing an option keeps the modal open");
  // A re-parse is a read, not an edit: the drawing the modal was opened over is untouched.
  assert_eq!(app.document_json(), before);
  assert_eq!(app.history_depth(), 0);
  assert!(!app.is_dirty());

  // The dialog draws both options and the number the parse used.
  let ctx = egui::Context::default();
  let _ = frame_with(&ctx, &mut shell, &mut app);
  let (_, texts) = frame_with(&ctx, &mut shell, &mut app);
  let all = joined(&texts);
  for drawn in [
    "Flattening tolerance",
    "1.000",
    "Join open vectors",
    "Smooth (Douglas-Peucker)",
    "Center on material",
    "Adjust material size to drawing",
    "format svg · 1 contour(s) found, 1 curve(s) flattened, 0 skipped",
  ] {
    assert!(
      all.contains(drawn),
      "the dialog never drew {drawn:?}: {all}"
    );
  }

  // And `Import` writes the parse the re-parse produced, not the one the modal opened with.
  shell
    .confirm_import(&mut app)
    .expect("the re-parsed file imports");
  assert_eq!(
    app.document().entities().count(),
    1,
    "the imported drawing is the one contour the re-parse last described"
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_failed_re_parse_keeps_the_report_and_says_why() {
  let directory = scratch("import-tolerance-refused");
  let path = write_curved_svg(&directory);
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.apply_action(
    &mut app,
    &UiAction::ImportFile {
      path: path.display().to_string(),
    },
  );
  let seeded = shell.import_tolerance;
  let fine = parsed_vertices(&shell);

  // A number that is not a usable distance is refused before the file is read.
  let refusal = shell
    .set_import_tolerance(0.0)
    .expect_err("zero is not a flattening tolerance");
  assert!(refusal.contains("tolerance"), "{refusal}");
  assert_eq!(
    shell.import_tolerance, seeded,
    "the field keeps the number the report was parsed at"
  );
  assert_eq!(parsed_vertices(&shell), fine);
  assert_eq!(shell.import_error.as_deref(), Some(refusal.as_str()));
  assert_eq!(shell.status, format!("refused: {refusal}"));

  // A file that went away after the modal opened is the same kind of refusal, and the report the operator was
  // reading survives it: `Import` would still import exactly what that report describes.
  std::fs::remove_file(&path).expect("the scratch file is removed");
  let refusal = shell
    .set_import_tolerance(0.5)
    .expect_err("a file that is gone cannot be re-parsed");
  assert!(
    refusal.contains("curve.svg"),
    "the refusal must name the file: {refusal}"
  );
  assert_eq!(shell.import_tolerance, seeded);
  assert_eq!(
    parsed_vertices(&shell),
    fine,
    "the parse the modal was showing survives the refusal"
  );
  assert!(shell.show_import, "a refusal keeps the modal open");

  let ctx = egui::Context::default();
  let _ = frame_with(&ctx, &mut shell, &mut app);
  let (_, texts) = frame_with(&ctx, &mut shell, &mut app);
  assert!(
    joined(&texts).contains(&refusal),
    "the refusal must be drawn in the modal: {}",
    joined(&texts)
  );
  assert_eq!(app.history_depth(), 0);
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn joining_open_vectors_chains_a_layer_into_one_cut_path() {
  let directory = scratch("import-join");
  let path = write_dxf(&directory);
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.apply_action(
    &mut app,
    &UiAction::ImportFile {
      path: path.display().to_string(),
    },
  );
  assert!(!shell.import_join, "joining is opt-in");

  shell.import_join = true;
  let report = shell
    .confirm_import(&mut app)
    .expect("the joined file imports");

  assert_eq!(report.created, 1, "the two lines become one entity");
  assert_eq!(app.document().entities().count(), 1);
  let contour = app
    .document()
    .entities()
    .next()
    .and_then(|entity| entity.kind.as_contour())
    .expect("the layer holds one contour");
  assert!(
    !contour.is_closed(),
    "the fixture's two lines form an open chain"
  );
  assert_eq!(
    contour.len(),
    3,
    "the join keeps the shared endpoint and both free ends"
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_layer_that_cannot_join_keeps_its_contours() {
  let directory = scratch("import-join-refused");
  let path = write_parallel_dxf(&directory);
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.apply_action(
    &mut app,
    &UiAction::ImportFile {
      path: path.display().to_string(),
    },
  );
  shell.import_join = true;

  let report = shell
    .confirm_import(&mut app)
    .expect("a layer that cannot be joined still imports");

  assert_eq!(report.created, 2, "the unjoinable layer keeps both lines");
  assert_eq!(app.document().entities().count(), 2);
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn smoothing_drops_vertices_within_the_flattening_tolerance() {
  let directory = scratch("import-smooth");
  let path = write_wavy_dxf(&directory);
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.apply_action(
    &mut app,
    &UiAction::ImportFile {
      path: path.display().to_string(),
    },
  );
  assert!(!shell.import_smooth, "smoothing is opt-in");
  assert_eq!(parsed_vertices(&shell), 5, "the parse keeps every vertex");

  shell.import_smooth = true;
  shell
    .confirm_import(&mut app)
    .expect("the smoothed file imports");

  let contour = app
    .document()
    .entities()
    .next()
    .and_then(|entity| entity.kind.as_contour())
    .expect("the file holds one contour");
  assert_eq!(
    contour.len(),
    2,
    "every interior vertex is within the flattening tolerance of the line that replaces it"
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_finer_flattening_tolerance_keeps_the_vertices_smoothing_would_drop() {
  let directory = scratch("import-smooth-fine");
  let path = write_wavy_dxf(&directory);
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.apply_action(
    &mut app,
    &UiAction::ImportFile {
      path: path.display().to_string(),
    },
  );
  shell
    .set_import_tolerance(0.0001)
    .expect("a finer tolerance re-parses the same file");
  shell.import_smooth = true;
  shell
    .confirm_import(&mut app)
    .expect("the finely smoothed file imports");

  let contour = app
    .document()
    .entities()
    .next()
    .and_then(|entity| entity.kind.as_contour())
    .expect("the file holds one contour");
  assert_eq!(
    contour.len(),
    5,
    "the 0.001 mm drift is above a 0.0001 mm tolerance, so nothing is dropped"
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn smoothing_keeps_a_closed_contour_closed_and_drops_collinear_vertices() {
  let directory = scratch("import-smooth-closed");
  let path = write_closed_polyline_dxf(&directory);
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.apply_action(
    &mut app,
    &UiAction::ImportFile {
      path: path.display().to_string(),
    },
  );
  shell.import_smooth = true;
  shell
    .confirm_import(&mut app)
    .expect("the smoothed closed file imports");

  let contour = app
    .document()
    .entities()
    .next()
    .and_then(|entity| entity.kind.as_contour())
    .expect("the file holds one contour");
  assert!(contour.is_closed(), "smoothing must not open a closed ring");
  assert_eq!(
    contour.len(),
    4,
    "only the collinear vertex is dropped, so the rectangle keeps its corners"
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn centering_the_drawing_places_it_in_the_middle_of_the_existing_sheet() {
  let directory = scratch("import-center");
  let path = write_dxf(&directory);
  let sheet = Job::default_sheet();
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  assert!(shell.import_center, "centering is the default position");
  shell.import_adjust_material = false;
  shell.apply_action(
    &mut app,
    &UiAction::ImportFile {
      path: path.display().to_string(),
    },
  );
  shell
    .confirm_import(&mut app)
    .expect("the parsed file imports");

  assert_eq!(
    app.document().board,
    sheet,
    "the material the operator already had is kept"
  );
  let bounds = document_bounds(&app);
  let expected = Point2::new(
    (sheet.width.mm() - bounds.width()) * 0.5,
    (sheet.height.mm() - bounds.height()) * 0.5,
  );
  assert!(
    bounds.min.is_near(expected, 1.0e-9),
    "the drawing must sit in the middle of the sheet: {:?} vs {expected:?}",
    bounds.min
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn keeping_the_original_position_leaves_the_import_at_the_importer_margin() {
  let directory = scratch("import-original-position");
  let path = write_dxf(&directory);
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.import_adjust_material = false;
  shell.import_center = false;
  shell.apply_action(
    &mut app,
    &UiAction::ImportFile {
      path: path.display().to_string(),
    },
  );
  shell
    .confirm_import(&mut app)
    .expect("the parsed file imports");

  let bounds = document_bounds(&app);
  assert!(
    bounds.min.is_near(Point2::new(10.0, 10.0), 1.0e-9),
    "unchecked, the geometry keeps the importer's bottom-left margin: {:?}",
    bounds.min
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn adjusting_the_material_sizes_the_stock_to_the_imported_drawing() {
  let directory = scratch("import-material");
  // `write_dxf` draws a 100 x 60 part, so its stock is 10 mm larger on every side.
  let path = write_dxf(&directory);
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  assert!(
    shell.import_adjust_material,
    "an import brings its own stock unless the operator says otherwise"
  );

  shell.apply_action(
    &mut app,
    &UiAction::ImportFile {
      path: path.display().to_string(),
    },
  );
  shell
    .confirm_import(&mut app)
    .expect("the parsed file imports");

  let job = app.document().board.clone();
  assert_eq!(job.width.mm(), 120.0, "100 mm of drawing plus 10 mm a side");
  assert_eq!(job.height.mm(), 80.0, "60 mm of drawing plus 10 mm a side");
  assert_eq!(
    job.thickness.mm(),
    18.0,
    "adjusting the material never changes how thick it is"
  );
  assert_eq!(
    job.origin,
    cam_model::OriginCorner::BottomLeft,
    "the stock is measured from its bottom-left corner"
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn without_adjusting_the_material_the_import_keeps_the_sheet_it_replaces() {
  let directory = scratch("import-material-off");
  let path = write_dxf(&directory);
  let mut app = AppState::new(document_with_rectangle());
  // The sheet the operator already had: the default 1219 x 2438 sheet of the drawing being replaced.
  let sheet = app.document().board.clone();
  let mut shell = Shell::new();
  shell.import_adjust_material = false;

  shell.apply_action(
    &mut app,
    &UiAction::ImportFile {
      path: path.display().to_string(),
    },
  );
  shell
    .confirm_import(&mut app)
    .expect("the parsed file imports");

  assert_eq!(
    app.document().board,
    sheet,
    "unchecked, the import keeps the material it was offered over"
  );
  assert_eq!(
    app.document().entities().count(),
    2,
    "unchecked, the geometry still arrives"
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn an_import_at_the_default_options_still_writes_what_the_importer_laid_out() {
  // PRESERVE (shell-file-operations S3): before the options column existed, `Import` wrote
  // `cam_import::into_document`'s own document, stock included. Both options default to exactly that, so a
  // caller that never touches them sees the sentence and the stock it saw before.
  let directory = scratch("import-default-options");
  let path = write_dxf(&directory);
  let parsed = cam_import::import_path(&path, &cam_import::ImportOptions::default())
    .expect("the fixture parses");
  let laid_out = cam_import::into_document(&parsed, None)
    .expect("the fixture becomes a document")
    .board;
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();

  shell.apply_action(
    &mut app,
    &UiAction::ImportFile {
      path: path.display().to_string(),
    },
  );
  shell
    .confirm_import(&mut app)
    .expect("the parsed file imports");

  assert_eq!(shell.import_tolerance, cam_import::DEFAULT_TOLERANCE);
  assert!(shell.import_adjust_material);
  assert!(!shell.import_join);
  assert!(!shell.import_smooth);
  assert!(shell.import_center);
  assert_eq!(app.document().board, laid_out);
  assert_eq!(
    shell.status,
    "import file — 2 entities created, 2 contour(s) from part.dxf, 0 curve(s) flattened, 0 skipped"
  );
  std::fs::remove_dir_all(&directory).ok();
}

// ---- Opening a document chosen at runtime ------------------------------------------------------

#[test]
fn opening_a_file_shows_its_document_and_its_name_in_the_app_bar() {
  let directory = scratch("open");
  let path = write_c2d(&directory, "bracket.cmc", &document_with_squares(2));
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  assert_eq!(shell.document_name(), "untitled.cmc");

  shell.apply_action(&mut app, &UiAction::OpenFile { path: path.clone() });

  assert_eq!(
    app.document().entities().count(),
    2,
    "the opened drawing is the one showing"
  );
  assert_eq!(shell.status, "open file — opened bracket.cmc");
  assert_eq!(shell.document_path(), Some(path.as_path()));
  assert_eq!(shell.document_name(), "bracket.cmc");

  let (_, texts) = frame(&mut shell, &mut app);
  let all = joined(&texts);
  assert!(
    all.contains("bracket.cmc"),
    "the bar must name the opened file: {all}"
  );
  assert!(
    !all.contains("untitled.cmc"),
    "the bar must stop saying untitled: {all}"
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_refused_open_changes_nothing_and_says_why() {
  let directory = scratch("open-refused");
  let opened = write_c2d(&directory, "bracket.cmc", &document_with_squares(2));
  let missing = directory.join("gone.cmc");
  let not_an_archive = directory.join("notes.cmc");
  std::fs::write(&not_an_archive, b"not a zip archive, just text").expect("text written");
  let invalid = directory.join("broken.cmc");
  let mut broken = document_with_squares(1);
  let repeated = broken.layers[0].clone();
  // Two layers with one id: a document the model refuses, so the file loads and the validation fails.
  broken.layers.push(repeated);
  // Built by hand, not through `DocumentFile::to_bytes`, which now validates: a corrupt archive is
  // something only a test needs to fabricate, and the product must not be able to.
  let json = serde_json::to_string(&broken).expect("serialises");
  let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
  writer
    .start_file(
      cam_store::document::DOCUMENT_ENTRY,
      zip::write::SimpleFileOptions::default(),
    )
    .expect("start entry");
  use std::io::Write as _;
  writer.write_all(json.as_bytes()).expect("write payload");
  std::fs::write(&invalid, writer.finish().expect("finish").into_inner()).expect("archive written");

  for (case, path) in [
    ("missing", missing),
    ("not an archive", not_an_archive),
    ("invalid document", invalid),
  ] {
    let mut app = AppState::new(document_with_rectangle());
    let mut shell = Shell::new();
    // One good open first, so "the name did not change" is a claim about a name that exists, and so the
    // history holds a step a refused open must not add to.
    shell.apply_action(
      &mut app,
      &UiAction::OpenFile {
        path: opened.clone(),
      },
    );
    let before = app.document_json();
    let history = app.history_depth();
    let dirty = app.is_dirty();

    shell.apply_action(&mut app, &UiAction::OpenFile { path: path.clone() });

    assert!(
      shell.status.starts_with("refused: "),
      "{case}: the status must refuse, got {:?}",
      shell.status
    );
    assert!(
      shell.status.contains(&path.display().to_string()),
      "{case}: the refusal must name the file, got {:?}",
      shell.status
    );
    assert_eq!(
      app.document_json(),
      before,
      "{case}: a refused open must leave the drawing alone"
    );
    assert_eq!(
      app.history_depth(),
      history,
      "{case}: a refused open must not record a step"
    );
    assert_eq!(
      app.is_dirty(),
      dirty,
      "{case}: a refused open must not change the dirty flag"
    );
    assert_eq!(
      shell.document_name(),
      "bracket.cmc",
      "{case}: a refused open must not change the name in the bar"
    );
  }
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn opening_a_file_clears_the_preview_and_the_material_drafts_and_keeps_the_library() {
  let directory = scratch("open-stale");
  let path = write_c2d(&directory, "cut.cmc", &document_with_toolpath(2, 4));
  let mut app = AppState::new(document_with_toolpath(1, 4));
  let mut shell = Shell::new();
  shell.set_tools(vec![stored_tool(4)]);
  let job = Job::default_sheet();
  shell.set_preview(
    Heightfield::new(&job, Length::from_mm(20.0)).expect("a field for the default sheet"),
    MoveStream::new(Length::from_mm(6.0), Point2::new(0.0, 0.0)),
    &job,
  );
  shell.material_size = (300.0, 200.0, 25.0);
  // The card resolves T4 while the snapshot is the one this window was given.
  let (output, _) = frame(&mut shell, &mut app);
  assert!(
    output
      .drawn
      .tool_card
      .iter()
      .any(|line| line.starts_with("T4 - ")),
    "the fixture must start with a resolvable tool: {:?}",
    output.drawn.tool_card
  );

  shell.apply_action(&mut app, &UiAction::OpenFile { path });

  assert_eq!(app.document().entities().count(), 2);
  assert!(
    shell.preview().is_none(),
    "the 3D panel must not keep playing the previous drawing's program"
  );
  assert_eq!(
    shell.material_size,
    (
      Job::default_sheet().width.mm(),
      Job::default_sheet().height.mm(),
      Job::default_sheet().thickness.mm(),
    ),
    "the drafts were seeded for a document nobody is looking at any more"
  );
  let (output, _) = frame(&mut shell, &mut app);
  assert!(
    output
      .drawn
      .tool_card
      .iter()
      .any(|line| line.starts_with("T4 - ")),
    "the tool library belongs to the application, so opening a drawing must not empty it: {:?}",
    output.drawn.tool_card
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn an_open_is_not_undoable_and_undo_leaves_the_opened_document_alone() {
  let directory = scratch("open-undo");
  let path = write_c2d(&directory, "bracket.cmc", &document_with_squares(3));
  let mut app = AppState::new(document_with_rectangle());
  let original = app.document_json();
  let mut shell = Shell::new();

  shell.apply_action(&mut app, &UiAction::OpenFile { path: path.clone() });

  assert_eq!(
    app.history_depth(),
    0,
    "an open records no undo step: it replaces the drawing and the recorded path together"
  );
  let opened = app.document_json();

  shell.apply_action(&mut app, &UiAction::Undo);

  assert_eq!(
    app.document_json(),
    opened,
    "an undo after an open leaves the opened drawing on screen"
  );
  assert_ne!(
    app.document_json(),
    original,
    "an undo must not reach back across the open to the drawing it replaced"
  );
  assert_eq!(
    shell.document_path(),
    Some(path.as_path()),
    "the recorded path still names the drawing on screen"
  );
  assert_eq!(
    app.history_depth(),
    0,
    "an undo with nothing to step back to records no step of its own"
  );
  std::fs::remove_dir_all(&directory).ok();
}

/// The class the verifier reproduced: an undo that crosses a document boundary lets a save write the wrong
/// drawing over the file the bar still names, dropping that file's archive entries.
///
/// Nothing in this shell can make that undo: an open records no history entry, so `Undo` after the second
/// open has nothing to step back to. The assertions below are one-sided on purpose — they fail if the open
/// is ever made undoable again, because the save would then write the first document, and its bytes and
/// archive entries, over the second file.
#[test]
fn an_undo_between_two_opens_cannot_make_a_save_overwrite_the_wrong_document() {
  let directory = scratch("open-two-undo-save");
  let first = write_c2d_with_extras(&directory, "first.cmc", &document_with_squares(3));
  let second = write_c2d_with_extras(&directory, "second.cmc", &document_with_squares(1));
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  shell.apply_action(&mut app, &UiAction::OpenFile { path: first });
  shell.apply_action(
    &mut app,
    &UiAction::OpenFile {
      path: second.clone(),
    },
  );
  let second_document = app.document_json();
  let before_bytes = std::fs::read(&second).expect("the second file as it was before the save");
  let before_extras = DocumentFile::load(&second)
    .expect("the second file loads before the save")
    .extras;

  shell.apply_action(&mut app, &UiAction::Undo);

  assert_eq!(
    app.document_json(),
    second_document,
    "an undo after opens must not step back across a document boundary"
  );
  shell.apply_action(&mut app, &UiAction::Save);

  assert_eq!(shell.status, "save document — wrote second.cmc");
  assert_eq!(
    shell.document_path(),
    Some(second.as_path()),
    "the recorded path still names the second file"
  );
  assert_eq!(
    std::fs::read(&second).expect("the second file after the save"),
    before_bytes,
    "the file the bar names must come out of an undo and a save byte for byte as it went in"
  );
  let after = DocumentFile::load(&second).expect("the second file loads after the save");
  assert_eq!(
    after.extras, before_extras,
    "the second file's archive entries must survive the save"
  );
  assert_eq!(
    &after.document,
    app.document(),
    "the drawing on disk is the one on screen"
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn an_open_frames_the_new_drawing_even_when_the_view_was_the_users() {
  let directory = scratch("open-frame");
  let path = write_c2d(&directory, "wide.cmc", &document_with_squares(2));
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let _ = frame(&mut shell, &mut app);
  // The user takes the view: panning and zooming stops the shell framing the drawing.
  shell.release_view();
  app
    .camera_mut()
    .zoom_at(cam_app::ScreenPoint::new(10.0, 10.0), 4.0);

  shell.apply_action(&mut app, &UiAction::OpenFile { path });
  let _ = frame(&mut shell, &mut app);

  let centre = app.camera().center();
  assert!(
    centre.is_near(Point2::new(135.0, 45.0), 5.0),
    "the opened drawing must be framed, not left at the old drawing's zoom: {centre:?}"
  );
  std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn the_rails_open_chip_asks_the_host_for_a_file() {
  let mut app = AppState::new(document_with_rectangle());
  let before = app.document_json();
  let mut shell = Shell::new();
  let ctx = egui::Context::default();
  let _ = frame_with(&ctx, &mut shell, &mut app);

  let (output, _) = click_palette_chip(&ctx, &mut shell, &mut app, HelpCommand::Open.label());

  assert_eq!(
    output.actions,
    vec![UiAction::RequestOpenFile],
    "the chip must ask the host, not open a file of its own"
  );
  assert_eq!(app.document_json(), before, "an intent touches no document");
  assert_eq!(
    shell.document_name(),
    "untitled.cmc",
    "an intent names no file"
  );
}

#[test]
fn the_rails_file_chips_still_emit_their_commands() {
  // PRESERVE (camui-prancheta T2b): these seven buttons lived in the app bar and emitted exactly these
  // actions. They emit the same actions from the rail, which is the move's whole claim — one surface, and
  // the command it runs unchanged.
  let mut app = AppState::new(document_with_rectangle());
  let mut shell = Shell::new();
  let ctx = egui::Context::default();
  let _ = frame_with(&ctx, &mut shell, &mut app);

  let (output, _) = click_palette_chip(&ctx, &mut shell, &mut app, HelpCommand::FitJob.label());
  assert_eq!(
    output.actions,
    vec![UiAction::ZoomToFit],
    "Fit the job still frames the drawing"
  );

  handle_action(
    &mut app,
    &UiAction::AddRectangle {
      width: 10.0,
      height: 10.0,
    },
  )
  .expect("the rectangle is added");
  let (output, _) = click_palette_chip(&ctx, &mut shell, &mut app, HelpCommand::Undo.label());
  assert_eq!(
    output.actions,
    vec![UiAction::Undo],
    "Undo still steps back"
  );
  shell.apply_action(&mut app, &UiAction::Undo);

  let (output, _) = click_palette_chip(&ctx, &mut shell, &mut app, HelpCommand::Redo.label());
  assert_eq!(
    output.actions,
    vec![UiAction::Redo],
    "Redo still steps forward"
  );
}

// ---- Saving the document the window opened (S2, S3, S4, S5) -----------------------------------
