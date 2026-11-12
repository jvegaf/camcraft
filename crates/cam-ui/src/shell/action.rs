use std::path::{Path, PathBuf};

use cam_app::AppState;
use cam_geom::{Contour, Point2, Vec2};
use cam_model::Job;
use cam_units::Length;

use crate::actions::{handle_action_with_file, DocumentFileState};
use crate::{ActionReport, Shell, UiAction};

use super::default_material_size;

impl Shell {
  /// Apply one action and show what it produced in the status bar.
  ///
  /// This is what a host calls for every action [`Self::show`] returned. It keeps the one-command
  /// rule — the work itself happens in the action handler, which is
  /// [`handle_action`](crate::handle_action) with this shell's file handed in — and turns the outcome the
  /// command layer reports (entity count, ellipse radial error, arcs lost to tessellation) and any refusal,
  /// with the kernel's own message, into the sentence the user reads.
  pub fn apply_action(&mut self, app: &mut AppState, action: &UiAction) {
    // Nothing edits a document this window does not hold (start-screen S2). The three actions that can change that
    // fact are the ones the start screen emits — a new project, a file chosen in the dialog, a row pressed — and
    // everything else, a draw or a transform or an undo or a save, is refused with the sentence the status bar prints.
    // Without this guard a rail chip pressed into the empty workspace would edit an invisible empty document and mark
    // it dirty: the operator would be told nothing is open while something was being changed.
    if !self.project_open
      && !matches!(
        action,
        UiAction::NewProject | UiAction::RequestOpenFile | UiAction::OpenFile { .. }
      )
    {
      self.status = format!("refused: {}", crate::start_screen::NO_PROJECT_REFUSAL);
      return;
    }
    // The file behind the drawing is the shell's, not the application's: the path it was read from and the
    // archive entries this build does not interpret travel with the window, so they are handed to the one
    // handler that touches `AppState` instead of being patched back in here after the fact. That is what
    // lets an open carry its extras and a save write them back without a second copy of either.
    // The two program actions are the shell's own rather than the shared handler's, for the reason
    // `NO_SHELL_FOR_SAVE` gives in the other direction: the post-processor is window state, like the path the
    // drawing came from, and `handle_action_with_file` is reachable without a window at all. Intercepting them
    // here is what keeps one place holding the dialect and one place writing the file, without widening a
    // handler whose signature every file action already shares.
    let applied = match action {
      UiAction::NewProject => {
        app.replace_document(cam_model::Document::new(cam_model::Job::default_sheet()));
        self.open_material(app);
        self.material_size = default_material_size();
        self.clear_preview();
        self.auto_frame = true;
        Ok(ActionReport {
          label: action.label(),
          ..ActionReport::default()
        })
      }
      // `ImportFile` no longer imports: it parses the file and opens the modal that reports what it found
      // (shell-file-operations S3), so nothing replaces the drawing until the operator presses `Import`.
      // `handle_action_with_file` keeps the immediate import for callers with no window at all, which is why
      // this interception lives in the shell and not in the shared handler.
      UiAction::ImportFile { path } => self.open_import(path),
      // The tool library's two file actions are the window's for the same reason: the tools are shell state, so
      // the shared handler has none to write out and no snapshot to replace (S11).
      UiAction::ExportTools { path } => self.export_tools(action.label(), path),
      UiAction::ImportTools { path } => self.import_tools(action.label(), path),
      _ => handle_action_with_file(
        app,
        action,
        Some(&mut DocumentFileState {
          path: &mut self.document_path,
          extras: &mut self.document_extras,
        }),
      ),
    };
    // The brand block's stamp is read from the file the action just read or wrote, and only when the action
    // was one of those: a `stat` per frame would be a syscall in the paint loop, and a stamp cached from an
    // earlier action would describe a file that has since changed. `modified` answers `Err` on a filesystem
    // that does not keep mtimes, which is an honest `None` — the block then says the work is unsaved.
    if applied.is_ok()
      && matches!(
        action,
        UiAction::OpenFile { .. } | UiAction::SaveFile { .. } | UiAction::Save
      )
    {
      self.saved_at = self
        .document_path
        .as_deref()
        .and_then(|path| std::fs::metadata(path).ok())
        .and_then(|metadata| metadata.modified().ok());
    }
    // An open replaces the document, so everything the shell derived from the previous one goes with it.
    // None of these three is in the undo history — the preview is a picture of a program, the tool
    // snapshot belongs to the drawing that was showing, and the drafts are numbers the dialog has not
    // written yet — so a bare swap would leave the 3D panel playing the old drawing's program and the
    // dialog holding the old stock. The snapshot of **tools** is deliberately not part of this: the tool
    // library belongs to the application and not to any drawing (tool-library-app S1), so opening a
    // document neither reads nor replaces it, and a document whose `tools_ref` names a library keeps that
    // reference for `cam-cli` without this window honouring it. The view is handed back to the shell for
    // the same reason: the new drawing is framed, because the old drawing's zoom was never a choice about
    // this one.
    if applied.is_ok() && matches!(action, UiAction::OpenFile { .. }) {
      self.clear_preview();
      self.material_size = default_material_size();
      self.auto_frame = true;
    }
    // A project exists from the moment a document was created or read (start-screen S2): the modal closes and the
    // canvas starts drawing the stock the drawing is cut from. `ImportFile` does not open one — an import lands in a
    // project, which is why the guard above refuses it without one, and why this list is these three and no more.
    if applied.is_ok()
      && matches!(
        action,
        UiAction::NewProject | UiAction::OpenFile { .. } | UiAction::SaveFile { .. }
      )
    {
      self.project_open = true;
      self.show_start = false;
    }
    // The list of recent projects is a record of files, so the two actions that touched one are the two that write a
    // row (start-screen S3). The path is the shell's own — `handle_action_with_file` recorded it — so a recent row and
    // the app bar always name the same file.
    if applied.is_ok()
      && matches!(
        action,
        UiAction::OpenFile { .. } | UiAction::SaveFile { .. }
      )
    {
      if let Some(path) = self.document_path.clone() {
        self.record_recent(&path);
      }
    }
    self.status = match applied {
      Ok(report) => report.message(),
      Err(message) => format!("refused: {message}"),
    };
  }

  /// The modal's `Import`: write the parse the modal reported into the document (shell-file-operations S3).
  ///
  /// It is public because a modal is painted inside a frame and a headless test has no frame to press: without
  /// this, the only way to complete an import in a test would be to click a point, and the point would be a
  /// promise about the dialog's layout rather than about the import.
  ///
  /// The document comes from [`Self::import_parsed`], never from a second read of the file: the counts the
  /// modal drew and the geometry that replaces the drawing are then the same parse by construction. The
  /// modal's options are applied to that parse by [`import_into_document`], which is the only place they reach
  /// the drawing. A refusal (`into_document` failing on a parse that produced no bounds) stays in the modal —
  /// [`Self::import_error`] carries the sentence — because closing it would lose the file and the report the
  /// operator was reading.
  pub fn confirm_import(&mut self, app: &mut AppState) -> Result<ActionReport, String> {
    let Some(parsed) = self.import_parsed.clone() else {
      return Err(NO_IMPORT_PENDING.to_owned());
    };
    let settings = ImportSettings {
      join: self.import_join,
      smooth: self.import_smooth,
      center: self.import_center,
      tolerance: self.import_tolerance,
      adjust_material: self.import_adjust_material,
    };
    match import_into_document(app, &parsed, self.import_path.as_deref(), settings) {
      Ok(report) => {
        self.status = report.message();
        self.close_import();
        Ok(report)
      }
      Err(message) => {
        self.status = format!("refused: {message}");
        self.import_error = Some(message.clone());
        Err(message)
      }
    }
  }

  /// Re-parse the file the import modal is showing at a new flattening tolerance (shell-file-operations S3).
  ///
  /// This is the modal's tolerance field's change handler, and it is public for the reason
  /// [`Self::confirm_import`] is: a `DragValue` is edited inside a frame, and without this the only way to
  /// prove the field re-parses would be to guess at the dialog's layout and drag a point.
  ///
  /// The parse is the same file read again with `ImportOptions { tolerance, ..default }`, so the report above
  /// the field, the geometry `Import` will write and the number beside it are one parse by construction. A
  /// refusal — a number that is not a usable distance, or a file that has gone away since the modal opened —
  /// leaves [`Self::import_tolerance`] and [`Self::import_parsed`] exactly as they were, so the modal goes on
  /// describing something `Import` can still write, and [`Self::import_error`] carries the sentence.
  pub fn set_import_tolerance(&mut self, tolerance: f64) -> Result<ActionReport, String> {
    if !(tolerance.is_finite() && tolerance > 0.0) {
      return Err(self.import_refused(format!(
        "flattening tolerance must be a positive number of millimetres, got {tolerance} mm"
      )));
    }
    let Some(path) = self.import_path.clone() else {
      return Err(self.import_refused(NO_IMPORT_PENDING.to_owned()));
    };
    let options = cam_import::ImportOptions { tolerance };
    let parsed = cam_import::import_path(&path, &options)
      .map_err(|error| self.import_refused(error.to_string()))?;
    self.import_tolerance = tolerance;
    self.import_parsed = Some(parsed.clone());
    self.import_error = None;
    let report = ActionReport {
      label: "import file",
      note: Some(format!(
        "tolerance {tolerance} mm · {} contour(s) found, {} curve(s) flattened, {} skipped",
        parsed.report.contours,
        parsed.report.flattened_curves,
        parsed.report.skipped_count()
      )),
      ..ActionReport::default()
    };
    self.status = report.message();
    Ok(report)
  }

  /// Record a refusal of one of the modal's options, and keep the report it is showing.
  ///
  /// Returns the sentence so the caller can hand it to its own `Err`: the status bar and the dialog must say
  /// the same words ([`Self::set_import_tolerance`] is the only caller, and its `Err` is that sentence).
  fn import_refused(&mut self, message: String) -> String {
    self.status = format!("refused: {message}");
    self.import_error = Some(message.clone());
    message
  }

  /// The modal's `Cancel`, and the `Esc` that means the same thing: forget the parse and write nothing.
  ///
  /// Public for the reason [`Self::confirm_import`] is: the state a cancelled modal leaves behind — no open
  /// flag, no path, no parse and no refusal — is a claim a headless test has to be able to make.
  pub fn close_import(&mut self) {
    self.show_import = false;
    self.import_path = None;
    self.import_parsed = None;
    self.import_error = None;
  }

  /// Parse `path` and open the import modal, replacing nothing yet (shell-file-operations S3).
  ///
  /// A parse that fails is refused here and never reaches the modal: a dialog whose only content is the reason
  /// a file could not be read would ask the operator to press `Import` on nothing. The status bar carries the
  /// refusal, the same way it carries a refused open, and the drawing is untouched because nothing has been read
  /// into it.
  fn open_import(&mut self, path: &str) -> Result<ActionReport, String> {
    let path = PathBuf::from(path);
    let options = cam_import::ImportOptions {
      tolerance: self.import_tolerance,
    };
    let parsed = cam_import::import_path(&path, &options).map_err(|error| error.to_string())?;
    self.import_path = Some(path);
    self.import_parsed = Some(parsed);
    self.import_error = None;
    self.show_import = true;
    Ok(ActionReport {
      label: "import file",
      note: Some("import pending".to_owned()),
      ..ActionReport::default()
    })
  }

  /// Hand the window the post-processor it writes programs in, as the file's label and its source text.
  ///
  /// The label is what the app bar shows and what a refusal names, so it is the file's own name rather than a
  /// path: a person reading `Post: grbl.pp` is reading the choice they made. Held as source text rather than as
  /// a parsed processor because a session can outlive the file it came from — a post-processor edited on disk
  /// mid-session is a different dialect, and re-parsing at use is what makes the program match the text in hand.
  pub fn set_post(&mut self, name: impl Into<String>, source: impl Into<String>) {
    self.post = Some((name.into(), source.into()));
  }

  /// Forget the held post-processor, so the next program refuses until another is chosen.
  pub fn clear_post(&mut self) {
    self.post = None;
  }
}
const NO_IMPORT_PENDING: &str = "no import is pending: choose a file to import first";

/// Write one parse into `app` as an undoable replacement, and report what arrived.
///
/// This is [`handle_action`](crate::handle_action)'s `ImportFile` branch without the read: the command layer
/// takes a path and imports it immediately, which is what a caller with no window needs, and the modal has
/// already paid for the parse it showed. The note is the same sentence, word for word, so the status bar cannot
/// describe the same file two ways depending on which route imported it.
///
/// The modal's options are post-processing of the parse, because [`cam_import`] has no such options: join and
/// smooth rewrite the contours before `into_document`, and position moves the finished geometry inside the
/// stock. `adjust_material` is checked after that geometry exists: the stock is then re-derived as the imported
/// contours' bounding box plus [`cam_import::PLACEMENT_MARGIN`] on every side, which is where the importer placed
/// the geometry, so it lands with that margin on all four sides and the thickness is left alone. Unchecked, the
/// stock is the material the drawing being replaced already had — the sheet this operator set up.
fn import_into_document(
  app: &mut AppState,
  parsed: &cam_import::Imported,
  path: Option<&Path>,
  settings: ImportSettings,
) -> Result<ActionReport, String> {
  let previous_job = app.document().job.clone();
  let processed = process_import(parsed, settings);
  let mut document =
    cam_import::into_document(&processed, None).map_err(|error| error.to_string())?;
  let job = if settings.adjust_material {
    stock_for_drawing(&document)
  } else {
    previous_job
  };
  // Centering happens against the job the import will keep, so the option means the same thing whether the
  // stock is fitted to the drawing or is the sheet the operator already had. Fitting makes the two placements
  // coincide — the stock is the drawing plus the same margin on every side — so the default is unchanged.
  if settings.center {
    center_drawing(&mut document, &job);
  }
  document.job = job;
  let created_entities: Vec<String> = document
    .entities()
    .map(|entity| entity.id.clone())
    .collect();
  let file_name = path
    .and_then(|path| path.file_name())
    .map(|name| name.to_string_lossy().into_owned())
    .or_else(|| path.map(|path| path.display().to_string()))
    .unwrap_or_default();
  let note = format!(
    "{} contour(s) from {file_name}, {} curve(s) flattened, {} skipped",
    parsed.report.contours,
    parsed.report.flattened_curves,
    parsed.report.skipped_count()
  );
  // One undo step, the same one the immediate import records: an import replaces the drawing, and one undo
  // brings back the drawing it replaced.
  app.replace_document_undoable(document);
  app.select(created_entities.clone());
  Ok(ActionReport {
    label: "import file",
    created: created_entities.len(),
    created_entities,
    note: Some(note),
    ..ActionReport::default()
  })
}

/// The modal's options, gathered once so the write path cannot read one of them halfway through.
#[derive(Clone, Copy)]
struct ImportSettings {
  join: bool,
  smooth: bool,
  center: bool,
  tolerance: f64,
  adjust_material: bool,
}

/// Rewrite the parse's contours with the modal's join and smooth options.
///
/// Both are post-processing rather than import options because [`cam_import`] has none: the report above the
/// fields keeps describing what the file held, while `Import` writes the geometry these two rules made of it.
fn process_import(parsed: &cam_import::Imported, settings: ImportSettings) -> cam_import::Imported {
  if !settings.join && !settings.smooth {
    return parsed.clone();
  }
  let mut processed = parsed.clone();
  for layer in &mut processed.layers {
    if settings.join {
      // A layer that is not exactly one chain is left as it arrived: the joiner refuses rather than picking
      // between two continuations, and a wrong chain is a cut path nobody asked for.
      if let Ok(joined) = cam_geom::join::join(&layer.contours, settings.tolerance) {
        layer.contours = vec![joined];
      }
    }
    if settings.smooth {
      layer.contours = layer
        .contours
        .iter()
        .map(|contour| simplify_contour(contour, settings.tolerance))
        .collect();
    }
  }
  processed
}

/// Center every contour on the job the import will keep.
///
/// The two placements coincide when the stock is fitted to the drawing, because that stock is the drawing plus
/// [`cam_import::PLACEMENT_MARGIN`] on every side; the option is visible on a sheet the operator already had.
fn center_drawing(document: &mut cam_model::Document, job: &Job) {
  let mut bounds = cam_geom::Bounds::empty();
  for entity in document.entities() {
    if let Some(contour) = entity.kind.as_contour() {
      bounds = bounds.union(contour.bounds());
    }
  }
  if bounds.is_empty() {
    return;
  }
  let target = Point2::new(
    (job.width.mm() - bounds.width()) * 0.5,
    (job.height.mm() - bounds.height()) * 0.5,
  );
  let delta = Vec2::new(target.x - bounds.min.x, target.y - bounds.min.y);
  if delta.x.abs() <= f64::EPSILON && delta.y.abs() <= f64::EPSILON {
    return;
  }
  for layer in &mut document.layers {
    for entity in &mut layer.entities {
      let cam_model::EntityKind::Contour { contour } = &mut entity.kind;
      if let Ok(moved) = cam_geom::transform::translate(contour, delta) {
        *contour = moved;
      }
    }
  }
}

/// Simplify one contour, keeping its arcs exactly when it has them.
fn simplify_contour(contour: &Contour, tolerance: f64) -> Contour {
  if contour.has_arcs() || contour.len() <= 2 || tolerance <= 0.0 {
    return contour.clone();
  }
  let points: Vec<Point2> = contour
    .vertices()
    .iter()
    .map(|vertex| vertex.point)
    .collect();
  let simplified = if contour.is_closed() {
    simplify_closed(&points, tolerance)
  } else {
    simplify_open(&points, tolerance)
  };
  if simplified.len() == points.len() {
    return contour.clone();
  }
  Contour::from_points(&simplified, contour.is_closed()).unwrap_or_else(|_| contour.clone())
}

/// Douglas–Peucker on a **closed** ring, keeping at least three vertices.
///
/// The ring is cut at vertex 0 and at the vertex farthest from it, the two open chains between them are
/// simplified, and the results are joined back together. Every kept vertex is an original vertex, so
/// simplification never moves anything: it only drops vertices, each within `tolerance` of the polyline that
/// replaced it.
fn simplify_closed(points: &[Point2], tolerance: f64) -> Vec<Point2> {
  if tolerance <= 0.0 || points.len() <= 3 {
    return points.to_vec();
  }

  let anchor = points[0];
  let mut far = 0;
  let mut far_distance = -1.0;
  for (index, point) in points.iter().enumerate() {
    let distance = point.distance_to(anchor);
    if distance > far_distance {
      far_distance = distance;
      far = index;
    }
  }
  if far == 0 || far + 1 == points.len() {
    return points.to_vec();
  }

  let mut kept = simplify_open(&points[..=far], tolerance);
  let mut second = points[far..].to_vec();
  second.push(anchor);
  let reduced = simplify_open(&second, tolerance);
  // The first vertex of the second chain (`far`) and its last (`points[0]`) are already kept.
  kept.extend(
    reduced
      .iter()
      .skip(1)
      .take(reduced.len().saturating_sub(2))
      .copied(),
  );

  if kept.len() >= 3 {
    return kept;
  }

  // A tolerance large enough to flatten the whole ring leaves two vertices; keep the three most
  // spread out original vertices instead of a contour the kernel would refuse.
  let mut third = 0;
  let mut third_distance = -1.0;
  for (index, point) in points.iter().enumerate() {
    let distance = distance_to_segment(*point, anchor, points[far]);
    if distance > third_distance {
      third_distance = distance;
      third = index;
    }
  }
  let mut indices = [0, far, third];
  indices.sort_unstable();
  indices.iter().map(|&index| points[index]).collect()
}

/// Douglas–Peucker on an open chain, endpoints kept.
fn simplify_open(points: &[Point2], tolerance: f64) -> Vec<Point2> {
  let count = points.len();
  if count <= 2 {
    return points.to_vec();
  }
  let mut keep = vec![false; count];
  keep[0] = true;
  keep[count - 1] = true;
  let mut pending = vec![(0usize, count - 1)];
  while let Some((start, end)) = pending.pop() {
    if end <= start + 1 {
      continue;
    }
    let mut farthest = start;
    let mut farthest_distance = -1.0;
    for (index, point) in points.iter().enumerate().take(end).skip(start + 1) {
      let distance = distance_to_segment(*point, points[start], points[end]);
      if distance > farthest_distance {
        farthest_distance = distance;
        farthest = index;
      }
    }
    if farthest_distance > tolerance {
      keep[farthest] = true;
      pending.push((start, farthest));
      pending.push((farthest, end));
    }
  }
  points
    .iter()
    .zip(keep.iter().copied())
    .filter_map(|(point, kept)| kept.then_some(*point))
    .collect()
}

/// Distance from `point` to the segment `start`–`end`.
fn distance_to_segment(point: Point2, start: Point2, end: Point2) -> f64 {
  let span = end - start;
  let length_squared = span.length_squared();
  if length_squared <= f64::EPSILON {
    return point.distance_to(start);
  }
  let t = ((point - start).dot(span) / length_squared).clamp(0.0, 1.0);
  point.distance_to(start.lerp(end, t))
}

/// The stock an import writes when it is asked to fit the material to the drawing.
///
/// The job is the imported contours' bounding box plus [`cam_import::PLACEMENT_MARGIN`] on every side, which is
/// the margin the importer placed the geometry at — so the drawing lands in the stock with that margin on all
/// four sides. Everything else about the job is kept: the thickness is how thick the material is, not a fact
/// about the drawing, and the origin is the imported document's own bottom-left corner, measured the same way
/// the geometry was placed.
fn stock_for_drawing(document: &cam_model::Document) -> Job {
  let margin = cam_import::PLACEMENT_MARGIN;
  let mut bounds = cam_geom::Bounds::empty();
  for entity in document.entities() {
    if let Some(contour) = entity.kind.as_contour() {
      bounds = bounds.union(contour.bounds());
    }
  }
  let mut job = document.job.clone();
  job.width = Length::from_mm(bounds.width() + 2.0 * margin);
  job.height = Length::from_mm(bounds.height() + 2.0 * margin);
  job
}
