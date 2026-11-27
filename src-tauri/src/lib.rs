use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, MutexGuard};

use cam_app::{AppState, Viewport};
use cam_model::{Board, Document, Job, OriginCorner};
use cam_sim::Heightfield;
use cam_store::tool_edit::{WriteToolError, new_tool, next_tool_number, rendered_name};
use cam_store::tools::{StoredTool, ToolKind, ToolLibrary, stored_tool_for_id};
use cam_units::Length;
use cam_view::{Camera3d, build_stock_box, build_surface};
use serde::{Deserialize, Serialize};
use tauri::{Manager, State};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandErrorDto {
  pub code: String,
  pub message: String,
}

impl CommandErrorDto {
  fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
    Self {
      code: code.into(),
      message: message.into(),
    }
  }

  fn state_poisoned() -> Self {
    Self::new("state_poisoned", "runtime state poisoned")
  }
}

const MAX_RECENTS: usize = 10;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RecentProjectDto {
  pub name: String,
  pub path: String,
  pub last_opened: String,
}

fn recents_file(data_dir: &Path) -> PathBuf {
  data_dir.join("recents.json")
}

fn read_recents(data_dir: &Path) -> Vec<RecentProjectDto> {
  let path = recents_file(data_dir);
  let text = match fs::read_to_string(&path) {
    Ok(text) => text,
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
    Err(_) => return Vec::new(),
  };
  let mut recents = serde_json::from_str::<Vec<RecentProjectDto>>(&text).unwrap_or_default();
  recents.truncate(MAX_RECENTS);
  recents
}

fn write_recents(data_dir: &Path, recents: &[RecentProjectDto]) -> Result<(), CommandErrorDto> {
  fs::create_dir_all(data_dir)
    .map_err(|e| CommandErrorDto::new("recents_dir_failed", e.to_string()))?;
  let path = recents_file(data_dir);
  let tmp = path.with_file_name("recents.json.tmp");
  let mut entries: Vec<RecentProjectDto> = recents.to_vec();
  entries.truncate(MAX_RECENTS);
  let text = serde_json::to_string_pretty(&entries)
    .map_err(|e| CommandErrorDto::new("recents_serialize_failed", e.to_string()))?;
  fs::write(&tmp, text).map_err(|e| CommandErrorDto::new("recents_write_failed", e.to_string()))?;
  fs::rename(&tmp, &path).map_err(|e| {
    let _ = fs::remove_file(&tmp);
    CommandErrorDto::new("recents_rename_failed", e.to_string())
  })
}

fn push_recent(recents: &mut Vec<RecentProjectDto>, path: &Path) {
  let path_str = path.to_string_lossy().into_owned();
  let name = path
    .file_stem()
    .map(|s| s.to_string_lossy().into_owned())
    .unwrap_or_else(|| path_str.clone());
  recents.retain(|r| r.path != path_str);
  recents.insert(
    0,
    RecentProjectDto {
      name,
      path: path_str,
      last_opened: String::new(),
    },
  );
  recents.truncate(MAX_RECENTS);
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SettingsDto {
  pub theme: String,
}

impl Default for SettingsDto {
  fn default() -> Self {
    Self {
      theme: "dark".to_string(),
    }
  }
}

fn settings_file(data_dir: &Path) -> PathBuf {
  data_dir.join("settings.json")
}

fn read_settings(data_dir: &Path) -> SettingsDto {
  let path = settings_file(data_dir);
  let text = match fs::read_to_string(&path) {
    Ok(text) => text,
    Err(_) => return SettingsDto::default(),
  };
  serde_json::from_str::<SettingsDto>(&text).unwrap_or_default()
}

fn write_settings(data_dir: &Path, settings: &SettingsDto) -> Result<(), CommandErrorDto> {
  fs::create_dir_all(data_dir)
    .map_err(|e| CommandErrorDto::new("settings_dir_failed", e.to_string()))?;
  let path = settings_file(data_dir);
  let tmp = path.with_file_name("settings.json.tmp");
  let text = serde_json::to_string_pretty(settings)
    .map_err(|e| CommandErrorDto::new("settings_serialize_failed", e.to_string()))?;
  fs::write(&tmp, text)
    .map_err(|e| CommandErrorDto::new("settings_write_failed", e.to_string()))?;
  fs::rename(&tmp, &path).map_err(|e| {
    let _ = fs::remove_file(&tmp);
    CommandErrorDto::new("settings_rename_failed", e.to_string())
  })
}

struct RuntimeState {
  app: AppState,
  path: Option<PathBuf>,
  data_dir: Option<PathBuf>,
  recents: Vec<RecentProjectDto>,
  settings: SettingsDto,
}

struct AppRuntime(Mutex<RuntimeState>);

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentStatusDto {
  pub schema: String,
  pub document_schema: String,
  pub dirty: bool,
  pub path: Option<String>,
  pub layer_count: usize,
  pub toolpath_count: usize,
  pub board_width_mm: f64,
  pub board_height_mm: f64,
  pub board_thickness_mm: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewDocumentRequest {
  pub width_mm: f64,
  pub height_mm: f64,
  pub thickness_mm: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CamContextDto {
  pub schema: String,
  pub layer_names: Vec<String>,
  pub material: Option<String>,
  pub stock_width_mm: f64,
  pub stock_height_mm: f64,
  pub stock_thickness_mm: f64,
  pub setup_origin: OriginCorner,
  pub setup_side_up: cam_model::BoardSideUp,
}

fn cam_context_impl(runtime: &AppRuntime) -> Result<CamContextDto, CommandErrorDto> {
  let state = runtime
    .0
    .lock()
    .map_err(|_| CommandErrorDto::state_poisoned())?;
  let doc = state.app.document();
  // The stock is the document's board (the model's `Job`), so its dimensions and its setup come from
  // there; nothing here re-derives them. `canonical_job()` keeps its hardcoded `BottomLeft`/`Top`
  // untouched and is not a second source.
  let board = &doc.board;
  Ok(CamContextDto {
    schema: "cam-context@1".into(),
    // The layer model has no name field: its stable identifier is the name the app shows.
    layer_names: doc.layers.iter().map(|layer| layer.id.clone()).collect(),
    // The document model stores no material, so none is reported rather than invented.
    material: None,
    stock_width_mm: board.width.mm(),
    stock_height_mm: board.height.mm(),
    stock_thickness_mm: board.thickness.mm(),
    setup_origin: board.origin,
    setup_side_up: board.side_up,
  })
}

/// One toolpath of the open document, as the CAM browser lists it and the operation inspector shows
/// it.
///
/// Only what the document model carries is filled in. A `Toolpath` carries no feed, spindle speed,
/// hit count or machine time, so those fields are `None` here rather than sample numbers — the same
/// precedent T1 set for a document's missing material.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CamOperationDto {
  pub id: String,
  /// 1-based position in the document, the artboard's `1 · …` row prefix.
  pub index: usize,
  pub name: String,
  /// The operation's own serde tag: `isolation`, `non_copper_clear`, `cutout`, `paint`, `drilling`
  /// or `panelize`.
  pub operation_type: String,
  /// The tool's stable library id, or its number in `T` notation when the record carries only
  /// that. A `Toolpath` is bound to exactly one tool, so this is one value, not a list.
  pub tool: String,
  /// The operation's cut depth; a `panelize` operation carries none.
  pub z_cut_mm: Option<f64>,
  /// Absent: the document carries no feed; feeds live in the tool library, a different store.
  pub plunge_feed_mm_per_min: Option<f64>,
  /// Absent: the document carries no spindle speed; it lives in the tool library.
  pub spindle_rpm: Option<u32>,
  /// Absent: the document carries no hit count (hits are an Excellon import plan figure).
  pub hits: Option<u32>,
  /// Absent: nothing in the document stores or derives a machine time.
  pub duration_s: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CamOperationsDto {
  pub schema: String,
  pub operations: Vec<CamOperationDto>,
}

/// The operation's serde tag, read from the model's own representation so this is not a second list
/// of variants that could drift from `Operation`'s `#[serde(tag = "kind")]`.
///
/// A model operation always serializes a `kind`, so a miss here is a broken invariant rather than a
/// legitimate absence: it fails as a structured command error instead of publishing an empty type.
fn operation_type(operation: &cam_model::Operation) -> Result<String, CommandErrorDto> {
  let value = serde_json::to_value(operation).map_err(|_| operation_type_missing())?;
  operation_type_from_value(&value)
}

/// The same read, from an already serialized operation, so the failure contract can be exercised
/// with a payload no valid `Operation` produces.
fn operation_type_from_value(value: &serde_json::Value) -> Result<String, CommandErrorDto> {
  value
    .get("kind")
    .and_then(|kind| kind.as_str())
    .map(str::to_string)
    .ok_or_else(operation_type_missing)
}

fn operation_type_missing() -> CommandErrorDto {
  CommandErrorDto::new(
    "operation_type_missing",
    "model operation serialized without a kind tag",
  )
}

/// The operation's cut depth. Each variant that cuts carries its own `cut_depth`; `panelize` has
/// none, so its cut Z is absent rather than a made-up zero.
fn operation_cut_depth_mm(operation: &cam_model::Operation) -> Option<f64> {
  match operation {
    cam_model::Operation::Isolation(params) => Some(params.cut_depth.mm()),
    cam_model::Operation::NonCopperClear(params) => Some(params.cut_depth.mm()),
    cam_model::Operation::Cutout(params) => Some(params.cut_depth.mm()),
    cam_model::Operation::Paint(params) => Some(params.cut_depth.mm()),
    cam_model::Operation::Drilling(params) => Some(params.cut_depth.mm()),
    cam_model::Operation::Panelize(_) => None,
  }
}

/// The tool a toolpath is bound to: its library id when the document records one, otherwise the tool
/// number the model always carries, in the machine's `T` notation.
fn tool_identity(toolpath: &cam_model::Toolpath) -> String {
  toolpath
    .tool_id
    .clone()
    .unwrap_or_else(|| format!("T{}", toolpath.tool_number))
}

fn cam_operations_impl(runtime: &AppRuntime) -> Result<CamOperationsDto, CommandErrorDto> {
  let state = runtime
    .0
    .lock()
    .map_err(|_| CommandErrorDto::state_poisoned())?;
  let doc = state.app.document();
  let operations = doc
    .toolpaths
    .iter()
    .enumerate()
    .map(|(position, toolpath)| {
      Ok(CamOperationDto {
        id: toolpath.id.clone(),
        index: position + 1,
        name: toolpath.name.clone(),
        operation_type: operation_type(&toolpath.operation)?,
        tool: tool_identity(toolpath),
        z_cut_mm: operation_cut_depth_mm(&toolpath.operation),
        plunge_feed_mm_per_min: None,
        spindle_rpm: None,
        hits: None,
        duration_s: None,
      })
    })
    .collect::<Result<Vec<_>, CommandErrorDto>>()?;
  Ok(CamOperationsDto {
    schema: "cam-operations@1".into(),
    operations,
  })
}

/// The state of one CAM step the progress panel checks: simulation, and export/post.
///
/// The three answers are three values, not one falsy default: a build that cannot compute the step
/// says so, a build that can but has not says that, and a run carries what it produced.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum CamStepStateDto {
  /// Nothing in this build computes the step, so no result exists and none is pending.
  NotAvailableInBuild,
  /// The build computes the step, but nothing has run it for the open document.
  NotRunYet,
  /// The step ran; `summary` is what it reported.
  Ran { summary: String },
}

/// Whether the open document has been simulated and exported/post-processed.
///
/// This command only publishes that state; it builds no simulator and no post-processor. Neither
/// state exists to publish today: `AppState` (`cam-app/src/lib.rs:104-112`) holds the document, the
/// camera, the selection and the undo history and nothing else, and `Document`
/// (`cam-model/src/lib.rs:560-582`) stores no simulation result and no export record — its `postp`
/// is only the post-processor *filename* the store resolves, which says which post is configured,
/// not that one ran. The desktop build does not even link `cam-export` or `cam-post`
/// (`src-tauri/Cargo.toml`), and the one `Heightfield` in it cuts a hardcoded `canonical_job()`, not
/// the open document (`scene_snapshot_impl`, `src-tauri/src/lib.rs:579-590`). So both steps report
/// `NotAvailableInBuild` rather than a sample outcome.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CamOutputDto {
  pub schema: String,
  /// The document's post-processor reference, if it names one: configuration, not a run.
  pub post_processor: Option<String>,
  pub simulation: CamStepStateDto,
  pub export: CamStepStateDto,
}

fn cam_output_impl(runtime: &AppRuntime) -> Result<CamOutputDto, CommandErrorDto> {
  let state = runtime
    .0
    .lock()
    .map_err(|_| CommandErrorDto::state_poisoned())?;
  let doc = state.app.document();
  Ok(CamOutputDto {
    schema: "cam-output@1".into(),
    post_processor: doc.postp.clone(),
    // No producer exists: see `CamOutputDto`'s comment for where the state would have to live.
    simulation: CamStepStateDto::NotAvailableInBuild,
    export: CamStepStateDto::NotAvailableInBuild,
  })
}

fn snapshot(state: &RuntimeState) -> DocumentStatusDto {
  let doc = state.app.document();
  DocumentStatusDto {
    schema: "cam-document-status@1".into(),
    document_schema: doc.schema.clone(),
    dirty: state.app.is_dirty(),
    path: state
      .path
      .as_ref()
      .map(|p| p.to_string_lossy().into_owned()),
    layer_count: doc.layers.len(),
    toolpath_count: doc.toolpaths.len(),
    board_width_mm: doc.board.width.mm(),
    board_height_mm: doc.board.height.mm(),
    board_thickness_mm: doc.board.thickness.mm(),
  }
}

fn document_status_impl(runtime: &AppRuntime) -> Result<DocumentStatusDto, CommandErrorDto> {
  let state = runtime
    .0
    .lock()
    .map_err(|_| CommandErrorDto::state_poisoned())?;
  Ok(snapshot(&state))
}

fn new_document_impl(
  runtime: &AppRuntime,
  request: NewDocumentRequest,
) -> Result<DocumentStatusDto, CommandErrorDto> {
  let mut state = runtime
    .0
    .lock()
    .map_err(|_| CommandErrorDto::state_poisoned())?;
  let mut board = Board::default_board();
  board.width = cam_units::Length::from_mm(request.width_mm);
  board.height = cam_units::Length::from_mm(request.height_mm);
  board.thickness = cam_units::Length::from_mm(request.thickness_mm);
  let doc = Document::new(board);
  state.app.replace_document(doc);
  state.path = None;
  Ok(snapshot(&state))
}

fn open_document_impl(
  runtime: &AppRuntime,
  path: String,
) -> Result<DocumentStatusDto, CommandErrorDto> {
  let doc = cam_store::document::load_document(&path)
    .map_err(|e| CommandErrorDto::new("document_open_failed", e.to_string()))?;
  let mut state = runtime
    .0
    .lock()
    .map_err(|_| CommandErrorDto::state_poisoned())?;
  state.app.replace_document(doc);
  let pb = PathBuf::from(path);
  state.path = Some(pb.clone());
  record_recent(&mut state, &pb);
  Ok(snapshot(&state))
}

fn record_recent(state: &mut RuntimeState, path: &Path) {
  push_recent(&mut state.recents, path);
  if let Some(dir) = state.data_dir.clone() {
    let _ = write_recents(&dir, &state.recents);
  }
}

fn save_document_impl(
  runtime: &AppRuntime,
  path: Option<String>,
) -> Result<DocumentStatusDto, CommandErrorDto> {
  let mut state = runtime
    .0
    .lock()
    .map_err(|_| CommandErrorDto::state_poisoned())?;
  let target = path
    .map(PathBuf::from)
    .or_else(|| state.path.clone())
    .ok_or_else(|| {
      CommandErrorDto::new("document_path_missing", "no document path; use save as")
    })?;
  let doc = state.app.document().clone();
  cam_store::document::save_document(&target, &doc)
    .map_err(|e| CommandErrorDto::new("document_save_failed", e.to_string()))?;
  state.path = Some(target.clone());
  state.app.mark_saved();
  record_recent(&mut state, &target);
  Ok(snapshot(&state))
}

fn list_recents_impl(runtime: &AppRuntime) -> Result<Vec<RecentProjectDto>, CommandErrorDto> {
  let state = runtime
    .0
    .lock()
    .map_err(|_| CommandErrorDto::state_poisoned())?;
  Ok(state.recents.clone())
}

fn get_settings_impl(runtime: &AppRuntime) -> Result<SettingsDto, CommandErrorDto> {
  let state = runtime
    .0
    .lock()
    .map_err(|_| CommandErrorDto::state_poisoned())?;
  Ok(state.settings.clone())
}

fn update_settings_impl(
  runtime: &AppRuntime,
  settings: SettingsDto,
) -> Result<SettingsDto, CommandErrorDto> {
  let mut state = runtime
    .0
    .lock()
    .map_err(|_| CommandErrorDto::state_poisoned())?;
  state.settings = settings;
  if let Some(dir) = state.data_dir.clone() {
    write_settings(&dir, &state.settings)?;
  }
  Ok(state.settings.clone())
}

#[tauri::command]
fn document_status(runtime: State<'_, AppRuntime>) -> Result<DocumentStatusDto, CommandErrorDto> {
  document_status_impl(&runtime)
}

#[tauri::command]
fn new_document(
  runtime: State<'_, AppRuntime>,
  request: NewDocumentRequest,
) -> Result<DocumentStatusDto, CommandErrorDto> {
  new_document_impl(&runtime, request)
}

#[tauri::command]
fn open_document(
  runtime: State<'_, AppRuntime>,
  path: String,
) -> Result<DocumentStatusDto, CommandErrorDto> {
  open_document_impl(&runtime, path)
}

#[tauri::command]
fn save_document(
  runtime: State<'_, AppRuntime>,
  path: Option<String>,
) -> Result<DocumentStatusDto, CommandErrorDto> {
  save_document_impl(&runtime, path)
}

#[tauri::command]
fn list_recents(runtime: State<'_, AppRuntime>) -> Result<Vec<RecentProjectDto>, CommandErrorDto> {
  list_recents_impl(&runtime)
}

#[tauri::command]
fn get_settings(runtime: State<'_, AppRuntime>) -> Result<SettingsDto, CommandErrorDto> {
  get_settings_impl(&runtime)
}

#[tauri::command]
fn update_settings(
  runtime: State<'_, AppRuntime>,
  settings: SettingsDto,
) -> Result<SettingsDto, CommandErrorDto> {
  update_settings_impl(&runtime, settings)
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Point3Dto {
  pub x: f64,
  pub y: f64,
  pub z: f64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Line3Dto {
  pub start: Point3Dto,
  pub end: Point3Dto,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MeshVertexDto {
  pub position: Point3Dto,
  pub depth: f32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneSnapshotDto {
  pub stock_width: f64,
  pub stock_height: f64,
  pub stock_thickness: f64,
  pub stock_box: Vec<Line3Dto>,
  pub vertices: Vec<MeshVertexDto>,
  pub triangles: Vec<[u32; 3]>,
  pub camera_target: Point3Dto,
  pub camera_yaw: f64,
  pub camera_pitch: f64,
  pub camera_distance: f64,
}

fn canonical_job() -> Job {
  Job {
    width: Length::from_mm(100.0),
    height: Length::from_mm(100.0),
    thickness: Length::from_mm(20.0),
    origin: OriginCorner::BottomLeft,
    side_up: cam_model::BoardSideUp::Top,
  }
}

fn scene_snapshot_impl() -> Result<SceneSnapshotDto, CommandErrorDto> {
  let job = canonical_job();
  let mut field = Heightfield::new(&job, Length::from_mm(1.0))
    .map_err(|e| CommandErrorDto::new("scene_build_failed", format!("{e}")))?;
  let grid = field.grid();
  for row in 0..grid.rows {
    for column in 0..grid.columns {
      let centre = grid.centre(column, row);
      if centre.x >= 20.0 && centre.x <= 80.0 && centre.y >= 30.0 && centre.y <= 70.0 {
        field.lower(column, row, -5.0);
      }
    }
  }

  let target = cam_move::Point3::new(50.0, 50.0, -2.5);
  let camera = Camera3d {
    yaw: 0.75,
    pitch: 0.55,
    distance: Length::from_mm(220.0),
    target,
    viewport: Viewport::new(1280.0, 720.0),
  };

  let stock_box = build_stock_box(&job)
    .into_iter()
    .map(|line| Line3Dto {
      start: Point3Dto {
        x: line.start.x.mm(),
        y: line.start.y.mm(),
        z: line.start.z.mm(),
      },
      end: Point3Dto {
        x: line.end.x.mm(),
        y: line.end.y.mm(),
        z: line.end.z.mm(),
      },
    })
    .collect();

  let mesh = build_surface(&field, &camera, 2);
  let vertices = mesh
    .vertices
    .iter()
    .map(|v| MeshVertexDto {
      position: Point3Dto {
        x: v.position.x.mm(),
        y: v.position.y.mm(),
        z: v.position.z.mm(),
      },
      depth: v.depth,
    })
    .collect();
  let triangles = mesh
    .triangles
    .iter()
    .map(|t| [t[0] as u32, t[1] as u32, t[2] as u32])
    .collect();

  Ok(SceneSnapshotDto {
    stock_width: job.width.mm(),
    stock_height: job.height.mm(),
    stock_thickness: job.thickness.mm(),
    stock_box,
    vertices,
    triangles,
    camera_target: Point3Dto {
      x: target.x.mm(),
      y: target.y.mm(),
      z: target.z.mm(),
    },
    camera_yaw: camera.yaw,
    camera_pitch: camera.pitch,
    camera_distance: camera.distance.mm(),
  })
}

#[tauri::command]
fn scene_snapshot() -> Result<SceneSnapshotDto, CommandErrorDto> {
  scene_snapshot_impl()
}

#[tauri::command]
fn cam_context(runtime: State<'_, AppRuntime>) -> Result<CamContextDto, CommandErrorDto> {
  cam_context_impl(&runtime)
}

#[tauri::command]
fn cam_operations(runtime: State<'_, AppRuntime>) -> Result<CamOperationsDto, CommandErrorDto> {
  cam_operations_impl(&runtime)
}

#[tauri::command]
fn cam_output(runtime: State<'_, AppRuntime>) -> Result<CamOutputDto, CommandErrorDto> {
  cam_output_impl(&runtime)
}

/// The file the application's tool library lives in, inside its own data directory.
///
/// A library belongs to the application and not to any document (the shell already keeps one
/// `tools.sqlite` per installation, `cam-ui/src/tool_library/library.rs`): one set of tools for every
/// document that is open, still there after one is closed. A document may carry a `tools_ref` and
/// `cam-cli` still honours it; these commands do not read it, so the page can never show a different
/// library than the one the next document would edit.
const TOOL_LIBRARY_FILE: &str = "tools.sqlite";

/// The one lock every command that writes the tool library holds across its whole read-modify-write.
///
/// The store does not make this redundant: `cam_store`'s `save_tool` is a transaction over one tool's
/// three rows (`cam-store/src/tool_edit.rs:241`), and its inserts are per-row upserts
/// (`cam-store/src/tools.rs:665`, `:734`), but nothing compares a version, and this crate reads the
/// library through a second, read-only connection (`read_tool_library`) that no write transaction of
/// the store can span. So a save that read a snapshot before another write landed would put the old
/// values of every field it did not edit back, and two creates could derive the same id and the same
/// tool number from one snapshot. The lock closes that window between invocations of this process,
/// which is the only writer of the file; a cross-process writer and a version column inside
/// `cam-store` are a separate task.
static TOOL_LIBRARY_WRITE: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

/// Take a library write lock, or refuse the write with a structured error.
///
/// A poisoned lock is not swallowed and not cleared: a write that took it would be a write into a
/// library whose last reader panicked halfway through, so the command answers with its own code and
/// writes nothing. `lock` is a parameter rather than the static directly so a test can poison a lock
/// of its own instead of the process-wide one every other test shares.
fn lock_or_refuse(lock: &'static Mutex<()>) -> Result<MutexGuard<'static, ()>, CommandErrorDto> {
  lock.lock().map_err(|_| {
    CommandErrorDto::new(
      "tool_library_write_poisoned",
      "the tool library write lock is poisoned, so nothing was written",
    )
  })
}

/// The process-wide guard a command takes before it reads the library it means to write.
fn lock_tool_library() -> Result<MutexGuard<'static, ()>, CommandErrorDto> {
  lock_or_refuse(&TOOL_LIBRARY_WRITE)
}

/// The guard a tool-library read-modify-write holds for its whole critical section.
type ToolLibraryWriteGuard = MutexGuard<'static, ()>;

/// Run one whole library read-modify-write as the critical section of the process-wide write lock.
///
/// The lock is taken before the body runs and released only after it returns, and the body is handed
/// the guard: a body that reads the library cannot be written without one, so the read cannot be
/// hoisted before the lock or moved after it without the code failing to compile. The proof is
/// structural rather than timed — no sleep, no elapsed time, no scheduling assumption: a test enters
/// this function and a plain `TOOL_LIBRARY_WRITE.try_lock()` fails inside the body, which is only
/// possible while the body really runs under the lock.
fn with_tool_library_write<T>(
  write: impl FnOnce(&ToolLibraryWriteGuard) -> Result<T, CommandErrorDto>,
) -> Result<T, CommandErrorDto> {
  let guard = lock_tool_library()?;
  write(&guard)
}

fn tool_library_path(state: &RuntimeState) -> Result<PathBuf, CommandErrorDto> {
  state
    .data_dir
    .as_ref()
    .map(|dir| dir.join(TOOL_LIBRARY_FILE))
    .ok_or_else(|| {
      CommandErrorDto::new(
        "tool_library_path_missing",
        "no application data directory, so the tool library has no location",
      )
    })
}

/// A library that could not be read, as the page receives it.
///
/// This is the code the read commands already publish, kept for every read so a caller that
/// distinguishes a read failure from a write failure never sees a read reported under
/// `tool_library_unwritable`, which is reserved for a write that could not be made.
fn tool_library_read_error(err: cam_store::StoreError) -> CommandErrorDto {
  CommandErrorDto::new("tool_library_unreadable", err.to_string())
}

/// The library read as a snapshot: every tool, in the library's own order.
///
/// Read-only, and never a writer. A library file that does not exist yet is not an error and not a
/// reason to create one, so it reads as the empty library it would be; every refusal the store itself
/// makes — an older schema version, a WAL library, a file that is not a library — is reported as a
/// structured error naming the file and the reason. Validation of a tool is the store's too
/// (`cam_store::tool_edit::validate_tool`, run by `save_tool`), so nothing here repeats it.
fn read_tool_library(runtime: &AppRuntime) -> Result<Vec<StoredTool>, CommandErrorDto> {
  let path = {
    let state = runtime
      .0
      .lock()
      .map_err(|_| CommandErrorDto::state_poisoned())?;
    tool_library_path(&state)?
  };
  match fs::metadata(&path) {
    Ok(_) => {}
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
    Err(err) => {
      return Err(CommandErrorDto::new(
        "tool_library_unreadable",
        format!("{}: {err}", path.display()),
      ));
    }
  }
  let library = ToolLibrary::open_read_only(&path).map_err(tool_library_read_error)?;
  library.tools().map_err(tool_library_read_error)
}

/// One row of the tool tree: what the master list shows, and what a selection names.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolRowDto {
  /// The library's stable geometry id, which is what `tool_detail` is asked for.
  pub id: String,
  /// The program number the cutting data records; `None` for a tool the library holds without one.
  /// The page renders the `T` notation, because the model stores a number.
  pub number: Option<u32>,
  /// The display name: the stored `name_format` rendered the way the store renders it for its own
  /// screen, which is the tool's literal name when the record holds a plain string rather than a
  /// template. S9 leaves open whether that name comes from the file or from the template; publishing
  /// the store's own rendering does not decide it.
  pub name: String,
  /// The kind's distinguishing number, derived from the record: see [`row_meta`].
  pub meta: String,
}

/// One kind's group in the tool tree.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolGroupDto {
  /// The kind's file slug, the stable half of `ToolKind`'s vocabulary (`flat-end-mill`).
  pub kind: String,
  /// The kind's own singular word (`Flat end mill`). A plural heading such as `End mills` is page
  /// copy and is not derived here: the model publishes one label per kind and nothing that plurals it.
  pub label: String,
  /// How many tools the group holds.
  pub count: usize,
  pub tools: Vec<ToolRowDto>,
}

/// The tool tree: groups by kind, each with its count and its rows.
///
/// A group appears exactly when the library holds a tool of its kind, and groups appear in
/// first-appearance order in the library's own order rather than in a second ordering table here: the
/// artboard's `End mills`, `Drills`, `V-bits` is what a library numbered in that order produces
/// without anybody hard-coding it.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolTreeDto {
  pub schema: String,
  pub groups: Vec<ToolGroupDto>,
}

/// A number a row can show without the zeroes a reader does not need (`1`, `0.1`, `3.175`).
///
/// Three decimals is the precision the store writes, and the same trimming the old shell's rows do;
/// this crate does not link `cam-ui`, so the one-line rule is restated rather than imported.
fn trimmed_number(value: f64) -> String {
  let text = cam_units::Format::fixed(3).apply(value);
  let trimmed = text.trim_end_matches('0').trim_end_matches('.');
  if trimmed.is_empty() {
    "0".to_owned()
  } else {
    trimmed.to_owned()
  }
}

/// An angle in degrees, at the precision the library is written at.
///
/// The model stores an angle in radians, so `degrees()` of a stored `118.0` reads back
/// `118.00000000000001`: the value is the record's, the noise is the unit round trip. Three decimals
/// is the precision the store writes, so that is the number published for a page to put in a box.
fn degrees_at_library_precision(degrees: f64) -> f64 {
  cam_units::Format::fixed(3).round(degrees)
}

/// A row's dim meta: the number that says which tool of its kind this is.
///
/// S9's artboard quotes a different number per kind — `1F` for an end mill, `118°` for a drill, `0.1`
/// for a V-bit — because the number that tells two tools of one kind apart is the kind's own. The rule
/// is read off the record rather than written per kind as a constant: a kind whose tip is a cone is
/// quoted by that cone (a drill by its included angle, a V-bit by the flat at its tip, which is what
/// distinguishes two V-bits of one angle), and every other kind by its flute count. A number the
/// record does not hold falls through to the next, and a record holding none of them is a dash — never
/// an invented zero.
fn row_meta(tool: &StoredTool) -> String {
  let geometry = &tool.geometry;
  let angle = geometry
    .included_angle
    .map(|angle| format!("{}°", trimmed_number(angle.degrees())));
  let flat = geometry.flat_diameter.map(|flat| trimmed_number(flat.mm()));
  let flutes = geometry.num_flutes.map(|flutes| format!("{flutes}F"));
  match ToolKind::of(geometry) {
    ToolKind::Drill => angle.or(flutes).or(flat),
    ToolKind::VBit => flat.or(angle).or(flutes),
    _ => flutes.or(angle).or(flat),
  }
  .unwrap_or_else(|| "—".to_owned())
}

fn tool_tree_impl(runtime: &AppRuntime) -> Result<ToolTreeDto, CommandErrorDto> {
  let tools = read_tool_library(runtime)?;
  let mut groups: Vec<ToolGroupDto> = Vec::new();
  for tool in &tools {
    let kind = ToolKind::of(&tool.geometry);
    let row = ToolRowDto {
      id: tool.geometry.id.clone(),
      number: tool.cutting.tool_number,
      name: rendered_name(tool),
      meta: row_meta(tool),
    };
    match groups.iter_mut().find(|group| group.kind == kind.slug()) {
      Some(group) => {
        group.count += 1;
        group.tools.push(row);
      }
      None => groups.push(ToolGroupDto {
        kind: kind.slug().to_string(),
        label: kind.label().to_string(),
        count: 1,
        tools: vec![row],
      }),
    }
  }
  Ok(ToolTreeDto {
    schema: "tool-tree@1".into(),
    groups,
  })
}

/// One tool's detail, as the two sections of S10 read it.
///
/// Every value is the record's own or the model's own rendering of it; a value the library does not
/// hold is `None`, never a sample number. Which of these the page lets an operator edit is the page's
/// decision (S10 reads it off the artboard's `.ro` rows against its `<input>` rows), so nothing here
/// marks a field writable or read-only.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDetailDto {
  pub schema: String,
  /// The library's stable geometry id, the same one the tree row carries.
  pub id: String,
  /// The display name, rendered from the record's `name_format` exactly as the tree row's name is.
  pub name: String,
  /// The kind's file slug (`flat-end-mill`).
  pub kind: String,
  /// The kind's own word (`Flat end mill`). S10's `Type` also shows a shape word (`flat`): the shape
  /// families are `cam-ui`'s (`ToolFamily`), and this crate does not link that crate, so no shape is
  /// published rather than a second copy of the rule kept here.
  pub kind_label: String,
  pub number: Option<u32>,
  pub diameter_mm: Option<f64>,
  pub flutes: Option<u32>,
  /// The usable flute length, S10's `Cut length`.
  pub cut_length_mm: Option<f64>,
  pub shank_mm: Option<f64>,
  pub overall_length_mm: Option<f64>,
  /// A conical tip's included angle, which S10's `Type` row and a V-bit's or drill's shape need.
  pub included_angle_deg: Option<f64>,
  /// The flat at a V-bit's tip.
  pub flat_diameter_mm: Option<f64>,
  pub spindle_rpm: Option<u32>,
  pub cut_feed_mm_per_min: Option<f64>,
  pub plunge_feed_mm_per_min: Option<f64>,
  pub stepover_mm: Option<f64>,
  /// Absent, and published as absent: the model stores a stepover as a length in millimetres and
  /// holds no percentage of the diameter, so S10's `40 %` cannot be read off the record and is not
  /// derived here from a basis the model never states.
  pub stepover_percent: Option<f64>,
}

fn tool_detail_of(tool: &StoredTool) -> ToolDetailDto {
  let geometry = &tool.geometry;
  let cutting = &tool.cutting;
  let kind = ToolKind::of(geometry);
  ToolDetailDto {
    schema: "tool-detail@1".into(),
    id: geometry.id.clone(),
    name: rendered_name(tool),
    kind: kind.slug().to_string(),
    kind_label: kind.label().to_string(),
    number: cutting.tool_number,
    diameter_mm: geometry.diameter.map(Length::mm),
    flutes: geometry.num_flutes,
    cut_length_mm: geometry.flute_length.map(Length::mm),
    shank_mm: geometry.shank_diameter.map(Length::mm),
    overall_length_mm: geometry.overall_length.map(Length::mm),
    included_angle_deg: geometry
      .included_angle
      .map(|angle| degrees_at_library_precision(angle.degrees())),
    flat_diameter_mm: geometry.flat_diameter.map(Length::mm),
    spindle_rpm: cutting.spindle_speed,
    cut_feed_mm_per_min: cutting.feed_rate,
    plunge_feed_mm_per_min: cutting.plunge_rate,
    stepover_mm: cutting.stepover.map(Length::mm),
    stepover_percent: None,
  }
}

/// One tool's detail, looked up by the geometry id a tree row carries.
///
/// A tool the library no longer holds is `None`, which the frontend receives as `null`: a selection
/// that names nothing is an absence and not an empty tool made of nulls.
fn tool_detail_impl(
  runtime: &AppRuntime,
  id: &str,
) -> Result<Option<ToolDetailDto>, CommandErrorDto> {
  let tools = read_tool_library(runtime)?;
  // The store's own resolver, so a selection resolves to exactly the tool the emitter would.
  Ok(cam_store::tools::stored_tool_for_id(&tools, id).map(tool_detail_of))
}

#[tauri::command]
fn tool_library_tree(runtime: State<'_, AppRuntime>) -> Result<ToolTreeDto, CommandErrorDto> {
  tool_tree_impl(&runtime)
}

#[tauri::command]
fn tool_detail(
  runtime: State<'_, AppRuntime>,
  id: String,
) -> Result<Option<ToolDetailDto>, CommandErrorDto> {
  tool_detail_impl(&runtime, &id)
}

/// The file the application's tool library lives in, for a command that means to write it.
///
/// The same location [`tool_library_path`] answers for reading; this only lifts it out of the lock,
/// because a write opens the library after the lock is released rather than while it is held.
fn tool_library_write_path(runtime: &AppRuntime) -> Result<PathBuf, CommandErrorDto> {
  let state = runtime
    .0
    .lock()
    .map_err(|_| CommandErrorDto::state_poisoned())?;
  tool_library_path(&state)
}

/// The library opened for writing: the **one** place a command may create or migrate the file.
///
/// [`ToolLibrary::open`] creates a library that is not there and runs every pending migration, which is
/// exactly what a save or a create may do and a read may not. Reading keeps
/// [`ToolLibrary::open_read_only`] (`read_tool_library`), so the two paths differ by the store's own
/// constructor and not by a check here: a read never creates a file, never migrates one, and never
/// leaves a `-wal`/`-shm` companion beside it. A refusal to open for writing is a structured error
/// naming the file and the store's reason.
fn open_tool_library_for_write(runtime: &AppRuntime) -> Result<ToolLibrary, CommandErrorDto> {
  let path = tool_library_write_path(runtime)?;
  ToolLibrary::open(&path)
    .map_err(|err| CommandErrorDto::new("tool_library_unwritable", err.to_string()))
}

/// A store refusal, as the page receives it.
///
/// The rules are the store's (`cam_store::tool_edit::validate_tool`, run by `save_tool`), so nothing is
/// re-checked here: a refusal is mapped to one stable code and the store's own sentence, which is the
/// only wording of that rule. A library that could not be written is a different failure and keeps its
/// own code.
fn tool_write_error(err: WriteToolError) -> CommandErrorDto {
  match err {
    WriteToolError::Invalid(problem) => CommandErrorDto::new("tool_invalid", problem.sentence()),
    WriteToolError::Store(store) => {
      CommandErrorDto::new("tool_library_unwritable", store.to_string())
    }
  }
}

/// The fields of a tool the page lets an operator edit, and nothing else.
///
/// S10 reads the editable set off the artboard: `Name`, `Number` and `Flutes` are `<input>`s, the rest
/// of `Geometry` is `.ro`, and all of `Feeds & speeds` is read-only. The payload is built out of exactly
/// those three, so a save **cannot** express a change to a read-only field — a diameter, a feed, or
/// S10's `stepoverPercent`, which the model does not hold and T1 already publishes as an absence.
/// `deny_unknown_fields` turns an attempt into a rejected payload instead of a silently ignored key.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolSaveRequest {
  /// The library's stable geometry id, the same one a tree row carries.
  pub id: String,
  /// S10's `Name` box. The store's `name_format` column holds a plain name as readily as a template,
  /// so the box writes the string it shows (see `cam_store::tool_edit::rendered_name`).
  pub name: String,
  /// S10's `Number` box. Absent when the box is cleared, which the store refuses with its own
  /// `NoToolNumber` rather than a rule invented here.
  pub number: Option<u32>,
  /// S10's `Flutes` box. Absent when the box is cleared, which the store refuses with `NoFlutes`.
  pub flutes: Option<u32>,
}

/// What a `+ New` tool starts as: a name, and nothing the store already decides.
///
/// The default shape, the flute count and the number come from the store's own starting point
/// (`cam_store::tool_edit::new_tool`, `next_tool_number`), so a new tool carries no dimension, feed or
/// number this command made up.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolCreateRequest {
  /// The name the page shows for the new tool.
  pub name: String,
}

/// A stable, unused geometry id for a tool the library does not hold yet.
///
/// The store derives its link ids from the geometry id and has no id generator of its own, so the
/// command owns this one: `tool-1`, `tool-2`, … — the first suffix no record already carries. It is
/// deterministic rather than random so a create is reproducible, and it is a fresh id rather than a
/// reused one because the library lists tools by geometry id.
fn fresh_tool_id(tools: &[StoredTool]) -> String {
  let mut candidate = 1;
  loop {
    let id = format!("tool-{candidate}");
    if !tools.iter().any(|tool| tool.geometry.id == id) {
      return id;
    }
    candidate += 1;
  }
}

/// The next tool number nothing in `tools` carries, checked against the very snapshot the create
/// writes into.
///
/// `next_tool_number` already answers the lowest free number; this states the invariant where the
/// create decides, so a create can never answer with success for a tool that took another tool's
/// number. A number that turns out to be taken steps to the next free one instead of being reused.
fn free_tool_number(tools: &[StoredTool]) -> u32 {
  let mut candidate = next_tool_number(tools);
  while tools
    .iter()
    .any(|tool| tool.cutting.tool_number == Some(candidate))
  {
    candidate += 1;
  }
  candidate
}

/// Save an existing tool's editable fields, answering with the record as it now stands.
///
/// The whole read-modify-write is the critical section [`with_tool_library_write`] runs under the
/// process-wide write lock, and the library is read **inside** that section: a snapshot taken outside
/// it is the lost update the lock exists to close, because the write would put back every field this
/// command did not edit.
///
/// The tool is read through the read-only path, so a save that names nothing the library holds never
/// opens the file for writing — and a refused save changes no byte of it, because the write only
/// starts once the record it merges into exists.
fn tool_library_save_impl(
  runtime: &AppRuntime,
  request: ToolSaveRequest,
) -> Result<ToolDetailDto, CommandErrorDto> {
  with_tool_library_write(|lock| save_tool_under_write_lock(lock, runtime, request))
}

/// The body of a save: runs only under the library write lock, through [`with_tool_library_write`].
fn save_tool_under_write_lock(
  _lock: &ToolLibraryWriteGuard,
  runtime: &AppRuntime,
  request: ToolSaveRequest,
) -> Result<ToolDetailDto, CommandErrorDto> {
  let tools = read_tool_library(runtime)?;
  let stored = stored_tool_for_id(&tools, &request.id).ok_or_else(|| {
    CommandErrorDto::new(
      "tool_not_found",
      format!(
        "the tool library holds no tool with id {}, so nothing was saved",
        request.id
      ),
    )
  })?;

  // Only the three editable fields are assigned. Everything else is the record the library already
  // holds, which is how a read-only field stays read-only: there is no assignment to reach it.
  let mut tool = stored.clone();
  tool.geometry.name_format = request.name;
  tool.cutting.tool_number = request.number;
  tool.geometry.num_flutes = request.flutes;

  // The store's own writer runs the store's own validation and refuses the whole tool atomically, so
  // the fields merged above are written together or not at all.
  let library = open_tool_library_for_write(runtime)?;
  library.save_tool(&tool).map_err(tool_write_error)?;
  Ok(tool_detail_of(&tool))
}

/// Create a tool, answering with the created record so the page can select it.
///
/// The whole read-modify-write is the critical section [`with_tool_library_write`] runs under the
/// process-wide write lock, and the snapshot is read inside it, through the same connection this
/// create writes with: the fresh id and the next tool number are derived from the library as it stands
/// now, so two creates cannot choose the same identity from one stale snapshot.
fn tool_library_create_impl(
  runtime: &AppRuntime,
  request: ToolCreateRequest,
) -> Result<ToolDetailDto, CommandErrorDto> {
  with_tool_library_write(|lock| create_tool_under_write_lock(lock, runtime, request))
}

/// The body of a create: runs only under the library write lock, through
/// [`with_tool_library_write`].
fn create_tool_under_write_lock(
  _lock: &ToolLibraryWriteGuard,
  runtime: &AppRuntime,
  request: ToolCreateRequest,
) -> Result<ToolDetailDto, CommandErrorDto> {
  // The write path is the one place a command may create or migrate the file, and reading through it
  // is what makes the identity below the library's current one rather than a second snapshot. A
  // failure to read that snapshot is a read failure and keeps the read code: only a write that could
  // not be made answers `tool_library_unwritable`.
  let library = open_tool_library_for_write(runtime)?;
  let tools = library.tools().map_err(tool_library_read_error)?;
  let tool = new_tool(
    &fresh_tool_id(&tools),
    &request.name,
    free_tool_number(&tools),
  );
  library.save_tool(&tool).map_err(tool_write_error)?;
  Ok(tool_detail_of(&tool))
}

#[tauri::command]
fn tool_library_save(
  runtime: State<'_, AppRuntime>,
  request: ToolSaveRequest,
) -> Result<ToolDetailDto, CommandErrorDto> {
  tool_library_save_impl(&runtime, request)
}

#[tauri::command]
fn tool_library_create(
  runtime: State<'_, AppRuntime>,
  request: ToolCreateRequest,
) -> Result<ToolDetailDto, CommandErrorDto> {
  tool_library_create_impl(&runtime, request)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  let initial = RuntimeState {
    app: AppState::new(Document::new(Board::default_board())),
    path: None,
    data_dir: None,
    recents: Vec::new(),
    settings: SettingsDto::default(),
  };

  tauri::Builder::default()
    .manage(AppRuntime(Mutex::new(initial)))
    .setup(|app| {
      let data_dir = app.path().app_data_dir().ok().map(|d| d.join("camcraft"));
      if let Some(dir) = data_dir.clone() {
        let recents = read_recents(&dir);
        let settings = read_settings(&dir);
        let runtime = app.state::<AppRuntime>();
        if let Ok(mut state) = runtime.0.lock() {
          state.data_dir = Some(dir);
          state.recents = recents;
          state.settings = settings;
        }
      }
      Ok(())
    })
    .plugin(
      tauri_plugin_log::Builder::new()
        .level(tauri_plugin_log::log::LevelFilter::Info)
        .build(),
    )
    .plugin(tauri_plugin_opener::init())
    .plugin(tauri_plugin_dialog::init())
    .invoke_handler(tauri::generate_handler![
      document_status,
      new_document,
      open_document,
      save_document,
      list_recents,
      get_settings,
      update_settings,
      scene_snapshot,
      cam_context,
      cam_operations,
      cam_output,
      tool_library_tree,
      tool_detail,
      tool_library_save,
      tool_library_create
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::fs;

  fn temp_path(name: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
      "camcraft-tauri-{name}-{}-{}.cmc",
      std::process::id(),
      std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time")
        .as_nanos()
    ));
    p
  }

  fn runtime() -> AppRuntime {
    AppRuntime(Mutex::new(RuntimeState {
      app: AppState::new(Document::new(Board::default_board())),
      path: None,
      data_dir: None,
      recents: Vec::new(),
      settings: SettingsDto::default(),
    }))
  }

  #[test]
  fn snapshot_reports_default_document() {
    let rt = runtime();
    let dto = document_status_impl(&rt).expect("status");
    assert_eq!(dto.schema, "cam-document-status@1");
    assert_eq!(dto.document_schema, cam_model::DOCUMENT_SCHEMA);
    assert!(!dto.dirty);
    assert_eq!(dto.path, None);
    assert_eq!(dto.layer_count, 0);
    assert_eq!(dto.toolpath_count, 0);
    assert!(dto.board_width_mm > 0.0);
  }

  fn layer(id: &str, role: cam_model::LayerRole) -> cam_model::Layer {
    cam_model::Layer {
      id: id.to_string(),
      role,
      side: cam_model::Side::Top,
      color: "#dc5a5a".to_string(),
      visible: true,
      entities: Vec::new(),
      source: cam_model::Provenance::default(),
    }
  }

  #[test]
  fn cam_context_reports_document_layers_material_and_setup() {
    let mut board = Board::default_board();
    board.width = cam_units::Length::from_mm(300.0);
    board.height = cam_units::Length::from_mm(200.0);
    board.thickness = cam_units::Length::from_mm(4.0);
    let mut doc = Document::new(board);
    doc.layers = vec![
      layer("top-copper", cam_model::LayerRole::CopperTop),
      layer("drill", cam_model::LayerRole::Drill),
    ];

    let rt = AppRuntime(Mutex::new(RuntimeState {
      app: AppState::new(doc),
      path: None,
      data_dir: None,
      recents: Vec::new(),
      settings: SettingsDto::default(),
    }));

    let dto = cam_context_impl(&rt).expect("cam context");
    assert_eq!(dto.schema, "cam-context@1");
    assert_eq!(
      dto.layer_names,
      vec!["top-copper".to_string(), "drill".to_string()]
    );
    // The document model stores no material, so the command reports that it has none instead of
    // inventing one.
    assert_eq!(dto.material, None);
    assert_eq!(dto.stock_width_mm, 300.0);
    assert_eq!(dto.stock_height_mm, 200.0);
    assert_eq!(dto.stock_thickness_mm, 4.0);
    assert_eq!(dto.setup_origin, OriginCorner::BottomLeft);
    assert_eq!(dto.setup_side_up, cam_model::BoardSideUp::Top);

    // The payload as the frontend receives it: every key it reads, in serde's casing, and the enum
    // encodings too, so a rename cannot silently break the consumer.
    assert_eq!(
      serde_json::to_value(&dto).expect("json"),
      serde_json::json!({
        "schema": "cam-context@1",
        "layerNames": ["top-copper", "drill"],
        "material": null,
        "stockWidthMm": 300.0,
        "stockHeightMm": 200.0,
        "stockThicknessMm": 4.0,
        "setupOrigin": "bottom_left",
        "setupSideUp": "top"
      })
    );
  }

  #[test]
  fn cam_context_setup_matches_the_canonical_job_defaults() {
    let rt = runtime();
    let dto = cam_context_impl(&rt).expect("cam context");
    let job = canonical_job();
    // A default document's setup is exactly what `canonical_job()` hardcodes: publishing it does
    // not fix a second source of truth nor change the job.
    assert!(dto.layer_names.is_empty());
    assert_eq!(dto.setup_origin, job.origin);
    assert_eq!(dto.setup_side_up, job.side_up);
    // The stock is the open document's board, so the dimensions are the default board's, not the
    // scene demo's 100x100x20.
    assert_eq!(dto.stock_width_mm, 1219.0);
    assert_eq!(dto.stock_height_mm, 2438.0);
    assert_eq!(dto.stock_thickness_mm, 18.0);
  }

  fn toolpath(
    id: &str,
    name: &str,
    tool_id: Option<&str>,
    tool_number: u32,
    operation: cam_model::Operation,
  ) -> cam_model::Toolpath {
    cam_model::Toolpath {
      id: id.to_string(),
      name: name.to_string(),
      entity_ids: Vec::new(),
      tool_id: tool_id.map(str::to_string),
      tool_number,
      operation,
    }
  }

  fn isolation(cut_depth_mm: f64) -> cam_model::Operation {
    cam_model::Operation::Isolation(cam_model::IsolationParams {
      passes: 1,
      overlap: 0.5,
      direction: cam_model::MillingDirection::Climb,
      kind: cam_model::IsolationKind::Full,
      cut_depth: cam_units::Length::from_mm(cut_depth_mm),
      keep_pours: false,
    })
  }

  fn drilling(cut_depth_mm: f64) -> cam_model::Operation {
    cam_model::Operation::Drilling(cam_model::DrillingParams {
      cut_depth: cam_units::Length::from_mm(cut_depth_mm),
      peck_depth: cam_units::Length::from_mm(0.0),
      dwell: 0.0,
    })
  }

  fn panelize() -> cam_model::Operation {
    cam_model::Operation::Panelize(cam_model::PanelizeParams {
      rows: 2,
      columns: 2,
      spacing_x: cam_units::Length::from_mm(5.0),
      spacing_y: cam_units::Length::from_mm(5.0),
      constrain_x: None,
      constrain_y: None,
    })
  }

  #[test]
  fn cam_operations_reports_document_toolpaths_with_their_detail() {
    let mut doc = Document::new(Board::default_board());
    doc.toolpaths = vec![
      toolpath(
        "tp-isolation",
        "Isolation top",
        Some("flat-0.8"),
        1,
        isolation(1.8),
      ),
      toolpath("tp-drill", "Drill 0.8 / 1.0", None, 2, drilling(1.6)),
      toolpath("tp-panel", "Panel 2x2", None, 3, panelize()),
    ];
    let rt = AppRuntime(Mutex::new(RuntimeState {
      app: AppState::new(doc),
      path: None,
      data_dir: None,
      recents: Vec::new(),
      settings: SettingsDto::default(),
    }));

    let dto = cam_operations_impl(&rt).expect("cam operations");
    assert_eq!(dto.schema, "cam-operations@1");
    assert_eq!(dto.operations.len(), 3);

    let first = &dto.operations[0];
    assert_eq!(first.id, "tp-isolation");
    assert_eq!(first.index, 1);
    assert_eq!(first.name, "Isolation top");
    assert_eq!(first.operation_type, "isolation");
    assert_eq!(first.tool, "flat-0.8");
    assert_eq!(first.z_cut_mm, Some(1.8));
    // The document model carries no feed, spindle, hit count or time estimate on a toolpath, so
    // those are published as absent instead of being invented from sample values.
    assert_eq!(first.plunge_feed_mm_per_min, None);
    assert_eq!(first.spindle_rpm, None);
    assert_eq!(first.hits, None);
    assert_eq!(first.duration_s, None);

    let second = &dto.operations[1];
    assert_eq!(second.id, "tp-drill");
    assert_eq!(second.index, 2);
    assert_eq!(second.name, "Drill 0.8 / 1.0");
    assert_eq!(second.operation_type, "drilling");
    // Without a `tool_id`, the tool is the number the model also carries.
    assert_eq!(second.tool, "T2");
    assert_eq!(second.z_cut_mm, Some(1.6));

    let third = &dto.operations[2];
    assert_eq!(third.index, 3);
    assert_eq!(third.operation_type, "panelize");
    // A panelize operation carries no cut depth in the model: absent, not zero.
    assert_eq!(third.z_cut_mm, None);

    // The payload as the frontend receives it: every key it reads, in serde's casing. One toolpath
    // carries exactly one tool, so there is one `tool` key and no plural duplicate of it; the
    // operation type is the model's own tag and the enums it carries are pinned by their encoding.
    assert_eq!(
      serde_json::to_value(&dto).expect("json"),
      serde_json::json!({
        "schema": "cam-operations@1",
        "operations": [
          {
            "id": "tp-isolation",
            "index": 1,
            "name": "Isolation top",
            "operationType": "isolation",
            "tool": "flat-0.8",
            "zCutMm": 1.8,
            "plungeFeedMmPerMin": null,
            "spindleRpm": null,
            "hits": null,
            "durationS": null
          },
          {
            "id": "tp-drill",
            "index": 2,
            "name": "Drill 0.8 / 1.0",
            "operationType": "drilling",
            "tool": "T2",
            "zCutMm": 1.6,
            "plungeFeedMmPerMin": null,
            "spindleRpm": null,
            "hits": null,
            "durationS": null
          },
          {
            "id": "tp-panel",
            "index": 3,
            "name": "Panel 2x2",
            "operationType": "panelize",
            "tool": "T3",
            "zCutMm": null,
            "plungeFeedMmPerMin": null,
            "spindleRpm": null,
            "hits": null,
            "durationS": null
          }
        ]
      })
    );
  }

  #[test]
  fn cam_operations_of_an_empty_document_is_an_empty_list() {
    let rt = runtime();
    let dto = cam_operations_impl(&rt).expect("cam operations");
    assert_eq!(dto.schema, "cam-operations@1");
    assert!(dto.operations.is_empty());
  }

  #[test]
  fn operation_type_without_a_kind_tag_is_a_structured_error() {
    // A model operation always serializes a `kind`, so this payload is what a broken model would
    // produce. Driving the value-level helper with it reaches the branch a valid `Operation`
    // cannot, which is the only way to prove the failure contract: no valid input can. Nothing in
    // production changes to make it testable.
    let payload = serde_json::json!({ "passes": 2, "overlap": 0.5, "keep_pours": false });
    let err = operation_type_from_value(&payload)
      .expect_err("an operation payload without a kind tag is an error, not an empty type");

    // The failure as the frontend receives it: the stable code and its message, never an empty
    // `operationType` string.
    assert_eq!(err.code, "operation_type_missing");
    assert_eq!(
      serde_json::to_value(&err).expect("json"),
      serde_json::json!({
        "code": "operation_type_missing",
        "message": "model operation serialized without a kind tag"
      })
    );

    // The same helper still reads a real operation's own tag, so the branch above is not dead code.
    assert_eq!(operation_type(&isolation(1.8)).expect("type"), "isolation");
    assert_eq!(operation_type(&panelize()).expect("type"), "panelize");
  }

  #[test]
  fn cam_output_reports_simulation_and_export_state_without_running_them() {
    let rt = runtime();
    let dto = cam_output_impl(&rt).expect("cam output");
    // Nothing in this build computes or stores a simulation of the open document or an export/post
    // run, so both steps report that absence instead of a falsy default.
    assert_eq!(dto.schema, "cam-output@1");
    assert_eq!(dto.post_processor, None);
    assert_eq!(dto.simulation, CamStepStateDto::NotAvailableInBuild);
    assert_eq!(dto.export, CamStepStateDto::NotAvailableInBuild);

    // The three answers are three values a frontend can tell apart: this build cannot compute the
    // step, this build can but has not run it, and a run has an outcome.
    assert_eq!(
      serde_json::to_value(CamStepStateDto::NotAvailableInBuild).expect("json"),
      serde_json::json!({ "state": "notAvailableInBuild" })
    );
    assert_eq!(
      serde_json::to_value(CamStepStateDto::NotRunYet).expect("json"),
      serde_json::json!({ "state": "notRunYet" })
    );
    assert_eq!(
      serde_json::to_value(CamStepStateDto::Ran {
        summary: "gouged 0.0 mm3, uncut 0.0 mm3".to_string(),
      })
      .expect("json"),
      serde_json::json!({ "state": "ran", "summary": "gouged 0.0 mm3, uncut 0.0 mm3" })
    );

    // Every field of the payload, as the frontend receives it.
    assert_eq!(
      serde_json::to_value(&dto).expect("json"),
      serde_json::json!({
        "schema": "cam-output@1",
        "postProcessor": null,
        "simulation": { "state": "notAvailableInBuild" },
        "export": { "state": "notAvailableInBuild" }
      })
    );

    // A post reference the document holds is published as the reference it is, not as a run.
    let mut doc = Document::new(Board::default_board());
    doc.postp = Some("grbl.h".to_string());
    let rt = AppRuntime(Mutex::new(RuntimeState {
      app: AppState::new(doc),
      path: None,
      data_dir: None,
      recents: Vec::new(),
      settings: SettingsDto::default(),
    }));
    let dto = cam_output_impl(&rt).expect("cam output");
    assert_eq!(dto.post_processor.as_deref(), Some("grbl.h"));
    assert_eq!(dto.export, CamStepStateDto::NotAvailableInBuild);
  }

  #[test]
  fn new_document_replaces_state_and_clears_path() {
    let rt = runtime();
    {
      let mut state = rt.0.lock().expect("lock");
      state.path = Some(PathBuf::from("/tmp/old.cmc"));
    }
    let dto = new_document_impl(
      &rt,
      NewDocumentRequest {
        width_mm: 100.0,
        height_mm: 200.0,
        thickness_mm: 3.0,
      },
    )
    .expect("new document");
    assert_eq!(dto.board_width_mm, 100.0);
    assert_eq!(dto.board_height_mm, 200.0);
    assert_eq!(dto.board_thickness_mm, 3.0);
    assert_eq!(dto.path, None);
    assert!(!dto.dirty);
  }

  #[test]
  fn save_and_open_round_trip_document() {
    let path = temp_path("roundtrip");
    let _ = fs::remove_file(&path);

    let rt = runtime();
    new_document_impl(
      &rt,
      NewDocumentRequest {
        width_mm: 321.0,
        height_mm: 654.0,
        thickness_mm: 5.0,
      },
    )
    .expect("new");

    let saved = save_document_impl(&rt, Some(path.to_string_lossy().into_owned())).expect("save");
    assert_eq!(saved.path.as_deref(), path.to_str());
    assert!(!saved.dirty);

    let reopened_rt = runtime();
    let opened =
      open_document_impl(&reopened_rt, path.to_string_lossy().into_owned()).expect("open");
    assert_eq!(opened.board_width_mm, 321.0);
    assert_eq!(opened.board_height_mm, 654.0);
    assert_eq!(opened.board_thickness_mm, 5.0);
    assert!(!opened.dirty);

    let _ = fs::remove_file(&path);
  }

  #[test]
  fn save_without_path_returns_structured_error() {
    let rt = runtime();
    let err = save_document_impl(&rt, None).expect_err("missing path");
    assert_eq!(err.code, "document_path_missing");
  }

  #[test]
  fn save_and_open_record_recents() {
    let dir = std::env::temp_dir().join(format!(
      "camcraft-tauri-recents-{}-{}",
      std::process::id(),
      std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time")
        .as_nanos()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("mkdir");

    let path = dir.join("board.cmc");
    let rt = AppRuntime(Mutex::new(RuntimeState {
      app: AppState::new(Document::new(Board::default_board())),
      path: None,
      data_dir: Some(dir.clone()),
      recents: Vec::new(),
      settings: SettingsDto::default(),
    }));

    save_document_impl(&rt, Some(path.to_string_lossy().into_owned())).expect("save");
    let recents = list_recents_impl(&rt).expect("recents");
    assert_eq!(recents.len(), 1);
    assert_eq!(recents[0].path, path.to_string_lossy());

    let rt2 = AppRuntime(Mutex::new(RuntimeState {
      app: AppState::new(Document::new(Board::default_board())),
      path: None,
      data_dir: Some(dir.clone()),
      recents: Vec::new(),
      settings: SettingsDto::default(),
    }));
    open_document_impl(&rt2, path.to_string_lossy().into_owned()).expect("open");
    let recents2 = list_recents_impl(&rt2).expect("recents");
    assert_eq!(recents2.len(), 1);
    assert_eq!(recents2[0].name, "board");

    let _ = fs::remove_dir_all(&dir);
  }

  #[test]
  fn settings_round_trip_and_persist() {
    let dir = std::env::temp_dir().join(format!(
      "camcraft-tauri-settings-{}-{}",
      std::process::id(),
      std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time")
        .as_nanos()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("mkdir");

    let rt = AppRuntime(Mutex::new(RuntimeState {
      app: AppState::new(Document::new(Board::default_board())),
      path: None,
      data_dir: Some(dir.clone()),
      recents: Vec::new(),
      settings: SettingsDto::default(),
    }));

    let got = get_settings_impl(&rt).expect("get");
    assert_eq!(got.theme, "dark");

    let updated = update_settings_impl(
      &rt,
      SettingsDto {
        theme: "light".to_string(),
      },
    )
    .expect("update");
    assert_eq!(updated.theme, "light");

    let rt2 = AppRuntime(Mutex::new(RuntimeState {
      app: AppState::new(Document::new(Board::default_board())),
      path: None,
      data_dir: Some(dir.clone()),
      recents: Vec::new(),
      settings: read_settings(&dir),
    }));
    let got2 = get_settings_impl(&rt2).expect("get2");
    assert_eq!(got2.theme, "light");

    let _ = fs::remove_dir_all(&dir);
  }

  /// A scratch directory for one test's library, removed by the test that made it.
  fn library_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
      "camcraft-tauri-{name}-{}-{}",
      std::process::id(),
      std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time")
        .as_nanos()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("mkdir");
    dir
  }

  /// A runtime whose application data directory holds the tool library, which is how the window
  /// finds it: `data_dir/tools.sqlite`.
  fn runtime_with_data_dir(dir: &Path) -> AppRuntime {
    AppRuntime(Mutex::new(RuntimeState {
      app: AppState::new(Document::new(Board::default_board())),
      path: None,
      data_dir: Some(dir.to_path_buf()),
      recents: Vec::new(),
      settings: SettingsDto::default(),
    }))
  }

  /// One stored tool with the fields every command reads; each test sets what it needs on top.
  fn library_tool(
    id: &str,
    name: &str,
    kind: cam_store::tools::ToolKind,
    number: Option<u32>,
  ) -> cam_store::tools::StoredTool {
    cam_store::tools::StoredTool {
      geometry: cam_store::tools::ToolGeometry {
        id: id.to_string(),
        name_format: name.to_string(),
        tool_type: kind.raw(),
        units: cam_units::UnitSystem::Mm,
        diameter: Some(Length::from_mm(1.0)),
        included_angle: None,
        flat_diameter: None,
        num_flutes: Some(1),
        flute_length: None,
        tip_radius: None,
        shank_diameter: None,
        overall_length: None,
        custom_attributes: None,
      },
      cutting: cam_store::tools::CuttingData {
        id: format!("{id}-cut"),
        rate_units: cam_units::UnitSystem::Mm,
        feed_rate: None,
        plunge_rate: None,
        spindle_speed: None,
        stepdown: None,
        stepover: None,
        clear_stepover: None,
        tool_number: number,
        notes: None,
      },
    }
  }

  /// Write a real library file through the store's own writer, in `dir`, and close it.
  fn seed_library(dir: &Path, tools: &[cam_store::tools::StoredTool]) {
    let library = cam_store::tools::ToolLibrary::open(dir.join("tools.sqlite"))
      .expect("the owner writes the library");
    for (position, tool) in tools.iter().enumerate() {
      library.put_geometry(&tool.geometry).expect("geometry");
      library.put_cutting_data(&tool.cutting).expect("cutting");
      library
        .link(
          &format!("link-{position}"),
          &tool.geometry.id,
          &tool.cutting.id,
          None,
          None,
        )
        .expect("link");
    }
  }

  #[test]
  fn tool_tree_of_an_empty_library_reports_no_groups() {
    let dir = library_dir("tool-tree-empty");
    let rt = runtime_with_data_dir(&dir);

    // No library file yet. Reading a library must not create one, so the tree is empty rather than
    // an error and the file is still absent afterwards.
    let dto = tool_tree_impl(&rt).expect("tree");
    assert_eq!(dto.schema, "tool-tree@1");
    assert!(dto.groups.is_empty());
    assert!(
      fs::metadata(dir.join("tools.sqlite")).is_err(),
      "a read command never creates the library it reads"
    );
    assert_eq!(
      serde_json::to_value(&dto).expect("json"),
      serde_json::json!({ "schema": "tool-tree@1", "groups": [] })
    );

    // A library that exists and holds nothing reads the same way: empty groups, no invented heading.
    seed_library(&dir, &[]);
    assert_eq!(
      serde_json::to_value(tool_tree_impl(&rt).expect("tree")).expect("json"),
      serde_json::json!({ "schema": "tool-tree@1", "groups": [] })
    );

    let _ = fs::remove_dir_all(&dir);
  }

  #[test]
  fn tool_tree_groups_tools_by_kind_with_a_count_and_a_derived_meta() {
    let dir = library_dir("tool-tree");

    let mut flat_one = library_tool(
      "flat-1",
      "Flat 1.0 mm",
      cam_store::tools::ToolKind::FlatEndMill,
      Some(1),
    );
    flat_one.geometry.num_flutes = Some(1);

    let mut flat_two = library_tool(
      "flat-2",
      "Flat 2.0 mm",
      cam_store::tools::ToolKind::FlatEndMill,
      Some(2),
    );
    flat_two.geometry.diameter = Some(Length::from_mm(2.0));
    flat_two.geometry.num_flutes = Some(2);

    // A record the library holds without a number and without a flute count: both are published as
    // absent, a `null` number and a dash meta, never as an invented zero.
    let mut flat_bare = library_tool(
      "flat-bare",
      "End mill 3.175 mm",
      cam_store::tools::ToolKind::FlatEndMill,
      None,
    );
    flat_bare.geometry.diameter = Some(Length::from_mm(3.175));
    flat_bare.geometry.num_flutes = None;

    let mut drill = library_tool(
      "drill-08",
      "Drill 0.8 mm",
      cam_store::tools::ToolKind::Drill,
      Some(4),
    );
    drill.geometry.diameter = Some(Length::from_mm(0.8));
    drill.geometry.included_angle = Some(cam_units::Angle::from_degrees(118.0));

    let mut vbit = library_tool(
      "vbit-30",
      "V-bit 30°",
      cam_store::tools::ToolKind::VBit,
      Some(6),
    );
    vbit.geometry.included_angle = Some(cam_units::Angle::from_degrees(30.0));
    vbit.geometry.flat_diameter = Some(Length::from_mm(0.1));

    seed_library(&dir, &[flat_one, flat_two, flat_bare, drill, vbit]);
    let rt = runtime_with_data_dir(&dir);

    let dto = tool_tree_impl(&rt).expect("tree");
    assert_eq!(dto.schema, "tool-tree@1");
    assert_eq!(dto.groups.len(), 3, "one group per kind the library holds");
    assert_eq!(dto.groups[0].count, 3);
    assert_eq!(dto.groups[1].count, 1);
    assert_eq!(dto.groups[2].count, 1);

    // The payload as the frontend receives it: every key, the group order the library's own order
    // produces, and the meta of each kind derived from the record instead of a per-kind constant.
    assert_eq!(
      serde_json::to_value(&dto).expect("json"),
      serde_json::json!({
        "schema": "tool-tree@1",
        "groups": [
          {
            "kind": "flat-end-mill",
            "label": "Flat end mill",
            "count": 3,
            "tools": [
              { "id": "flat-1", "number": 1, "name": "Flat 1.0 mm", "meta": "1F" },
              { "id": "flat-2", "number": 2, "name": "Flat 2.0 mm", "meta": "2F" },
              { "id": "flat-bare", "number": null, "name": "End mill 3.175 mm", "meta": "—" }
            ]
          },
          {
            "kind": "drill",
            "label": "Drill",
            "count": 1,
            "tools": [
              { "id": "drill-08", "number": 4, "name": "Drill 0.8 mm", "meta": "118°" }
            ]
          },
          {
            "kind": "v-bit",
            "label": "V-bit",
            "count": 1,
            "tools": [
              { "id": "vbit-30", "number": 6, "name": "V-bit 30°", "meta": "0.1" }
            ]
          }
        ]
      })
    );

    let _ = fs::remove_dir_all(&dir);
  }

  #[test]
  fn tool_detail_reports_the_geometry_and_feeds_the_library_holds() {
    let dir = library_dir("tool-detail");

    let mut flat = library_tool(
      "flat-1",
      "End mill 1.0 mm - 3.175 mm",
      cam_store::tools::ToolKind::FlatEndMill,
      Some(22),
    );
    flat.geometry.diameter = Some(Length::from_mm(1.0));
    flat.geometry.num_flutes = Some(1);
    flat.geometry.flute_length = Some(Length::from_mm(8.0));
    flat.geometry.shank_diameter = Some(Length::from_mm(3.175));
    flat.geometry.overall_length = Some(Length::from_mm(38.0));
    flat.cutting.spindle_speed = Some(18_000);
    flat.cutting.feed_rate = Some(800.0);
    flat.cutting.plunge_rate = Some(200.0);
    flat.cutting.stepover = Some(Length::from_mm(0.4));

    let mut drill = library_tool(
      "drill-08",
      "Drill 0.8 mm",
      cam_store::tools::ToolKind::Drill,
      Some(4),
    );
    drill.geometry.diameter = Some(Length::from_mm(0.8));
    drill.geometry.included_angle = Some(cam_units::Angle::from_degrees(118.0));

    seed_library(&dir, &[flat, drill]);
    let rt = runtime_with_data_dir(&dir);

    let dto = tool_detail_impl(&rt, "flat-1")
      .expect("the query runs")
      .expect("the library holds flat-1");
    assert_eq!(dto.schema, "tool-detail@1");
    assert_eq!(dto.number, Some(22));

    // The payload as the frontend receives it. `stepoverPercent` is an explicit absence: the model
    // stores a stepover as a length in millimetres and holds no percentage, so no percentage is
    // published for the page to echo as if the library had said it.
    assert_eq!(
      serde_json::to_value(&dto).expect("json"),
      serde_json::json!({
        "schema": "tool-detail@1",
        "id": "flat-1",
        "name": "End mill 1.0 mm - 3.175 mm",
        "kind": "flat-end-mill",
        "kindLabel": "Flat end mill",
        "number": 22,
        "diameterMm": 1.0,
        "flutes": 1,
        "cutLengthMm": 8.0,
        "shankMm": 3.175,
        "overallLengthMm": 38.0,
        "includedAngleDeg": null,
        "flatDiameterMm": null,
        "spindleRpm": 18000,
        "cutFeedMmPerMin": 800.0,
        "plungeFeedMmPerMin": 200.0,
        "stepoverMm": 0.4,
        "stepoverPercent": null
      })
    );

    // The lookup resolves the tool the caller named, not its neighbour, and a drill's own numbers
    // read back where a mill's are absent.
    let drill = tool_detail_impl(&rt, "drill-08")
      .expect("the query runs")
      .expect("the library holds drill-08");
    assert_eq!(drill.kind, "drill");
    assert_eq!(drill.included_angle_deg, Some(118.0));
    assert_eq!(drill.flutes, Some(1));
    assert_eq!(drill.cut_feed_mm_per_min, None);
    assert_eq!(drill.stepover_mm, None);

    let _ = fs::remove_dir_all(&dir);
  }

  #[test]
  fn tool_detail_of_a_tool_the_library_does_not_hold_is_absent() {
    let dir = library_dir("tool-detail-missing");
    seed_library(
      &dir,
      &[library_tool(
        "flat-1",
        "Flat 1.0 mm",
        cam_store::tools::ToolKind::FlatEndMill,
        Some(1),
      )],
    );
    let rt = runtime_with_data_dir(&dir);

    // A selection the library no longer holds is an absence, not a made-up empty tool.
    let dto = tool_detail_impl(&rt, "no-such-tool").expect("the query runs");
    assert_eq!(dto, None);
    assert_eq!(
      serde_json::to_value(&dto).expect("json"),
      serde_json::json!(null)
    );

    let _ = fs::remove_dir_all(&dir);
  }

  #[test]
  fn tool_library_save_writes_the_editable_fields_and_the_change_lands() {
    let dir = library_dir("tool-save");

    // S10's editor: `Name`, `Number` and `Flutes` are inputs; the geometry's other numbers and all
    // of `Feeds & speeds` are read-only rows. The seed holds every one of them so the save can be
    // proved to leave them alone.
    let mut flat = library_tool(
      "flat-22",
      "End mill 1.0 mm",
      cam_store::tools::ToolKind::FlatEndMill,
      Some(22),
    );
    flat.geometry.diameter = Some(Length::from_mm(1.0));
    flat.geometry.num_flutes = Some(1);
    flat.geometry.flute_length = Some(Length::from_mm(8.0));
    flat.geometry.shank_diameter = Some(Length::from_mm(3.175));
    flat.geometry.overall_length = Some(Length::from_mm(38.0));
    flat.cutting.spindle_speed = Some(18_000);
    flat.cutting.feed_rate = Some(800.0);
    flat.cutting.plunge_rate = Some(200.0);
    flat.cutting.stepover = Some(Length::from_mm(0.4));
    seed_library(&dir, &[flat]);
    let rt = runtime_with_data_dir(&dir);

    let saved = tool_library_save_impl(
      &rt,
      ToolSaveRequest {
        id: "flat-22".to_string(),
        name: "End mill 1.0 mm - 3.175 mm".to_string(),
        number: Some(7),
        flutes: Some(3),
      },
    )
    .expect("the store accepts an edited tool");

    // The payload as the frontend receives it: the record it now holds, reusing the detail contract
    // the page already reads. The read-only fields are the ones the library held, untouched.
    assert_eq!(
      serde_json::to_value(&saved).expect("json"),
      serde_json::json!({
        "schema": "tool-detail@1",
        "id": "flat-22",
        "name": "End mill 1.0 mm - 3.175 mm",
        "kind": "flat-end-mill",
        "kindLabel": "Flat end mill",
        "number": 7,
        "diameterMm": 1.0,
        "flutes": 3,
        "cutLengthMm": 8.0,
        "shankMm": 3.175,
        "overallLengthMm": 38.0,
        "includedAngleDeg": null,
        "flatDiameterMm": null,
        "spindleRpm": 18000,
        "cutFeedMmPerMin": 800.0,
        "plungeFeedMmPerMin": 200.0,
        "stepoverMm": 0.4,
        "stepoverPercent": null
      })
    );

    // The persisted effect, read back through T1's own read command: the three editable fields
    // changed and the read-only ones did not move.
    let read_back = tool_detail_impl(&rt, "flat-22")
      .expect("the query runs")
      .expect("the library still holds flat-22");
    assert_eq!(read_back.name, "End mill 1.0 mm - 3.175 mm");
    assert_eq!(read_back.number, Some(7));
    assert_eq!(read_back.flutes, Some(3));
    assert_eq!(read_back.diameter_mm, Some(1.0));
    assert_eq!(read_back.cut_length_mm, Some(8.0));
    assert_eq!(read_back.shank_mm, Some(3.175));
    assert_eq!(read_back.overall_length_mm, Some(38.0));
    assert_eq!(read_back.spindle_rpm, Some(18_000));
    assert_eq!(read_back.cut_feed_mm_per_min, Some(800.0));
    assert_eq!(read_back.plunge_feed_mm_per_min, Some(200.0));
    assert_eq!(read_back.stepover_mm, Some(0.4));

    // The tree reads the saved name and number too, so the master list and the detail agree.
    let tree = tool_tree_impl(&rt).expect("tree");
    assert_eq!(tree.groups.len(), 1);
    assert_eq!(tree.groups[0].tools[0].name, "End mill 1.0 mm - 3.175 mm");
    assert_eq!(tree.groups[0].tools[0].number, Some(7));

    let _ = fs::remove_dir_all(&dir);
  }

  #[test]
  fn tool_library_create_writes_a_new_tool_and_returns_it() {
    let dir = library_dir("tool-create");
    let rt = runtime_with_data_dir(&dir);
    let path = dir.join("tools.sqlite");
    assert!(
      fs::metadata(&path).is_err(),
      "nothing has written the library yet"
    );

    let created = tool_library_create_impl(
      &rt,
      ToolCreateRequest {
        name: "End mill 1.0 mm".to_string(),
      },
    )
    .expect("the store accepts a new tool");

    // The created record, so the page can select it: the store's own `new_tool` starting point, with
    // the number the store's `next_tool_number` picks and no invented feeds.
    assert_eq!(
      serde_json::to_value(&created).expect("json"),
      serde_json::json!({
        "schema": "tool-detail@1",
        "id": "tool-1",
        "name": "End mill 1.0 mm",
        "kind": "flat-end-mill",
        "kindLabel": "Flat end mill",
        "number": 1,
        "diameterMm": 6.0,
        "flutes": 2,
        "cutLengthMm": null,
        "shankMm": null,
        "overallLengthMm": null,
        "includedAngleDeg": null,
        "flatDiameterMm": null,
        "spindleRpm": null,
        "cutFeedMmPerMin": null,
        "plungeFeedMmPerMin": null,
        "stepoverMm": null,
        "stepoverPercent": null
      })
    );

    // The write path is the one place a command may create the library, and it did.
    assert!(
      fs::metadata(&path).is_ok(),
      "the write command created the library"
    );

    // The persisted effect, read back through the read commands.
    let read_back = tool_detail_impl(&rt, "tool-1")
      .expect("the query runs")
      .expect("the library holds the created tool");
    assert_eq!(read_back.name, "End mill 1.0 mm");
    assert_eq!(read_back.number, Some(1));
    assert_eq!(read_back.flutes, Some(2));
    assert_eq!(
      serde_json::to_value(&tool_tree_impl(&rt).expect("tree")).expect("json"),
      serde_json::json!({
        "schema": "tool-tree@1",
        "groups": [{
          "kind": "flat-end-mill",
          "label": "Flat end mill",
          "count": 1,
          "tools": [{ "id": "tool-1", "number": 1, "name": "End mill 1.0 mm", "meta": "2F" }]
        }]
      })
    );

    // A second `New` gets the next free identity and number rather than overwriting the first.
    let second = tool_library_create_impl(
      &rt,
      ToolCreateRequest {
        name: "End mill 2.0 mm".to_string(),
      },
    )
    .expect("the store accepts a second tool");
    assert_eq!(second.id, "tool-2");
    assert_eq!(second.number, Some(2));
    assert_eq!(tool_tree_impl(&rt).expect("tree").groups[0].count, 2);

    let _ = fs::remove_dir_all(&dir);
  }

  #[test]
  fn tool_library_save_refusal_is_structured_and_leaves_the_library_untouched() {
    let dir = library_dir("tool-save-refused");
    seed_library(
      &dir,
      &[library_tool(
        "flat-22",
        "End mill 1.0 mm",
        cam_store::tools::ToolKind::FlatEndMill,
        Some(22),
      )],
    );
    let rt = runtime_with_data_dir(&dir);
    let path = dir.join("tools.sqlite");
    let before = fs::read(&path).expect("the seeded library reads");

    // A tool number of zero is one of the store's own refusals. The command maps it to a stable code
    // and carries the store's sentence, which is the only wording of the rule.
    let err = tool_library_save_impl(
      &rt,
      ToolSaveRequest {
        id: "flat-22".to_string(),
        name: "End mill 1.0 mm".to_string(),
        number: Some(0),
        flutes: Some(1),
      },
    )
    .expect_err("T0 is refused by the store");
    assert_eq!(err.code, "tool_invalid");
    assert_eq!(
      serde_json::to_value(&err).expect("json"),
      serde_json::json!({
        "code": "tool_invalid",
        "message": "A tool number has to be at least 1, because no program uses T0."
      })
    );

    // A cleared `Flutes` box is the same gate, not a second copy of it.
    let err = tool_library_save_impl(
      &rt,
      ToolSaveRequest {
        id: "flat-22".to_string(),
        name: "End mill 1.0 mm".to_string(),
        number: Some(22),
        flutes: Some(0),
      },
    )
    .expect_err("a tool with no cuts is refused by the store");
    assert_eq!(err.code, "tool_invalid");
    assert_eq!(
      err.message,
      "A tool needs at least one cut, because a tool with none cannot cut."
    );

    // The rejected write left the library file byte-for-byte as it was, and the read command agrees.
    assert_eq!(
      fs::read(&path).expect("the library still reads"),
      before,
      "a refused save never writes the library"
    );
    let read_back = tool_detail_impl(&rt, "flat-22")
      .expect("the query runs")
      .expect("the library still holds flat-22");
    assert_eq!(read_back.name, "End mill 1.0 mm");
    assert_eq!(read_back.number, Some(22));
    assert_eq!(read_back.flutes, Some(1));

    // The payload carries only the editable fields by construction, so a caller cannot even express
    // a change to a read-only one such as `Diameter` or to the absent `stepoverPercent`.
    let forged = serde_json::json!({
      "id": "flat-22",
      "name": "End mill 1.0 mm",
      "number": 22,
      "flutes": 1,
      "diameterMm": 9.9,
      "stepoverPercent": 40.0
    });
    assert!(
      serde_json::from_value::<ToolSaveRequest>(forged).is_err(),
      "a save payload names the editable fields and nothing else"
    );

    let _ = fs::remove_dir_all(&dir);
  }

  #[test]
  fn tool_library_save_of_a_tool_the_library_does_not_hold_is_an_absent_target() {
    let dir = library_dir("tool-save-missing");
    seed_library(
      &dir,
      &[library_tool(
        "flat-1",
        "Flat 1.0 mm",
        cam_store::tools::ToolKind::FlatEndMill,
        Some(1),
      )],
    );
    let rt = runtime_with_data_dir(&dir);

    let err = tool_library_save_impl(
      &rt,
      ToolSaveRequest {
        id: "no-such-tool".to_string(),
        name: "x".to_string(),
        number: Some(1),
        flutes: Some(1),
      },
    )
    .expect_err("a save names a tool the library holds");
    assert_eq!(
      serde_json::to_value(&err).expect("json"),
      serde_json::json!({
        "code": "tool_not_found",
        "message": "the tool library holds no tool with id no-such-tool, so nothing was saved"
      })
    );

    // The library is unchanged: the tool the payload did not name is the only one it holds.
    let tree = tool_tree_impl(&rt).expect("tree");
    assert_eq!(tree.groups.len(), 1);
    assert_eq!(tree.groups[0].count, 1);
    assert_eq!(tree.groups[0].tools[0].id, "flat-1");

    let _ = fs::remove_dir_all(&dir);
  }

  /// The lock-before-read property, proved structurally instead of through elapsed time.
  ///
  /// A `try_lock()` that fails *inside* the critical section can only fail because the section really
  /// holds the same static: no sleep, no thread, no timeout, and therefore no scheduling assumption
  /// that a lock-less implementation could slip through. This is the proof the previous timing test
  /// only approximated.
  #[test]
  fn with_tool_library_write_runs_its_body_while_the_write_lock_is_held() {
    let free = with_tool_library_write(|_lock| {
      let probe = TOOL_LIBRARY_WRITE.try_lock();
      let free = probe.is_ok();
      drop(probe);
      Ok::<bool, CommandErrorDto>(free)
    })
    .expect("the write lock is free");
    assert!(
      !free,
      "the tool library critical section ran without the write lock"
    );
  }

  /// A save reads the library inside the critical section, which is where the lost update lived.
  ///
  /// The body is entered through the same function the command uses and cannot re-take the lock, so it
  /// runs while the process-wide guard is held; the read it merges into then sees the library as it
  /// stands, and the field the payload cannot name keeps its stored value.
  #[test]
  fn tool_library_save_reads_the_library_under_the_write_lock() {
    let dir = library_dir("tool-lock-save");
    let mut flat = library_tool(
      "flat-22",
      "End mill 1.0 mm",
      cam_store::tools::ToolKind::FlatEndMill,
      Some(22),
    );
    flat.geometry.diameter = Some(Length::from_mm(1.0));
    seed_library(&dir, &[flat]);
    let rt = runtime_with_data_dir(&dir);

    let saved = with_tool_library_write(|lock| {
      let probe = TOOL_LIBRARY_WRITE.try_lock();
      let free = probe.is_ok();
      drop(probe);
      assert!(!free, "a save read the library outside the write lock");
      save_tool_under_write_lock(
        lock,
        &rt,
        ToolSaveRequest {
          id: "flat-22".to_string(),
          name: "End mill 1.0 mm - 3.175 mm".to_string(),
          number: Some(7),
          flutes: Some(3),
        },
      )
    })
    .expect("the save runs under the lock");

    assert_eq!(saved.name, "End mill 1.0 mm - 3.175 mm");
    assert_eq!(saved.number, Some(7));
    assert_eq!(saved.flutes, Some(3));
    // The read happened under the lock, so the write merged into the stored record rather than into a
    // stale snapshot: the diameter the payload cannot name is the one the library holds.
    assert_eq!(saved.diameter_mm, Some(1.0));
    let read_back = tool_detail_impl(&rt, "flat-22")
      .expect("the query runs")
      .expect("the library still holds flat-22");
    assert_eq!(read_back.diameter_mm, Some(1.0));
    assert_eq!(read_back.name, "End mill 1.0 mm - 3.175 mm");

    let _ = fs::remove_dir_all(&dir);
  }

  /// A create derives its id and its number from a snapshot it reads inside the critical section.
  ///
  /// Same deterministic mechanism as the save: the body is entered through
  /// [`with_tool_library_write`], so it cannot run without the process-wide guard, and the probe shows
  /// the guard is the one this section holds rather than a second lock invented for the test.
  #[test]
  fn tool_library_create_reads_the_library_under_the_write_lock() {
    let dir = library_dir("tool-lock-create");
    seed_library(
      &dir,
      &[library_tool(
        "tool-1",
        "Flat 1.0 mm",
        cam_store::tools::ToolKind::FlatEndMill,
        Some(1),
      )],
    );
    let rt = runtime_with_data_dir(&dir);

    let created = with_tool_library_write(|lock| {
      let probe = TOOL_LIBRARY_WRITE.try_lock();
      let free = probe.is_ok();
      drop(probe);
      assert!(!free, "a create read the library outside the write lock");
      create_tool_under_write_lock(
        lock,
        &rt,
        ToolCreateRequest {
          name: "End mill 3.0 mm".to_string(),
        },
      )
    })
    .expect("the store accepts the new tool");

    // The seeded tool owns `tool-1` and number 1, so a create that read the library under the lock
    // takes the next free identity instead of colliding with it.
    assert_eq!(created.id, "tool-2");
    assert_eq!(created.number, Some(2));
    let tree = tool_tree_impl(&rt).expect("tree");
    assert_eq!(tree.groups[0].count, 2);
    let numbers: Vec<Option<u32>> = tree.groups[0].tools.iter().map(|row| row.number).collect();
    assert_eq!(numbers, vec![Some(1), Some(2)]);

    let _ = fs::remove_dir_all(&dir);
  }

  /// A read failure keeps the read code and a write that cannot even open the library keeps the write
  /// code, so a caller that distinguishes them is never told a read went wrong as a write.
  #[test]
  fn tool_library_read_and_write_failures_keep_their_own_codes() {
    // The read side is the one mapping every read of the library uses, so the create path cannot
    // answer a read failure under the write-specific code.
    let read = tool_library_read_error(cam_store::StoreError::ToolLibrary {
      path: "tools.sqlite".to_string(),
      reason: "it is in WAL journal mode".to_string(),
    });
    assert_eq!(
      serde_json::to_value(&read).expect("json"),
      serde_json::json!({
        "code": "tool_library_unreadable",
        "message": "tool library tools.sqlite cannot be used: it is in WAL journal mode"
      })
    );

    // The write side, through the command: a file the write path cannot open as a library is a write
    // failure, and it keeps the write-specific code.
    let dir = library_dir("tool-create-unwritable");
    fs::write(dir.join("tools.sqlite"), b"this is not a tool library").expect("seed the bad file");
    let rt = runtime_with_data_dir(&dir);
    let err = tool_library_create_impl(
      &rt,
      ToolCreateRequest {
        name: "End mill 1.0 mm".to_string(),
      },
    )
    .expect_err("a file that is not a library cannot be written");
    assert_eq!(err.code, "tool_library_unwritable");
    assert!(
      err.message.contains("tools.sqlite"),
      "the write failure names the library: {}",
      err.message
    );

    let _ = fs::remove_dir_all(&dir);
  }

  /// Two creates in sequence keep distinct ids and distinct numbers, and neither overwrites the other.
  #[test]
  fn two_creates_in_sequence_cannot_share_an_id_or_a_number() {
    let dir = library_dir("tool-create-sequence");
    let rt = runtime_with_data_dir(&dir);

    let first = tool_library_create_impl(
      &rt,
      ToolCreateRequest {
        name: "End mill 1.0 mm".to_string(),
      },
    )
    .expect("the store accepts the first tool");
    let second = tool_library_create_impl(
      &rt,
      ToolCreateRequest {
        name: "End mill 2.0 mm".to_string(),
      },
    )
    .expect("the store accepts the second tool");

    assert_ne!(first.id, second.id);
    assert_ne!(first.number, second.number);
    assert!(first.number.is_some() && second.number.is_some());
    // Both records are still there: a create that reused an identity would have replaced one.
    let tree = tool_tree_impl(&rt).expect("tree");
    assert_eq!(tree.groups.len(), 1);
    assert_eq!(tree.groups[0].count, 2);
    let ids: Vec<&str> = tree.groups[0]
      .tools
      .iter()
      .map(|row| row.id.as_str())
      .collect();
    assert_eq!(ids, vec![first.id.as_str(), second.id.as_str()]);
    let numbers: Vec<Option<u32>> = tree.groups[0].tools.iter().map(|row| row.number).collect();
    assert_eq!(numbers, vec![first.number, second.number]);

    let _ = fs::remove_dir_all(&dir);
  }

  /// A create derives its id and its number from the library as it stands, so a number already taken
  /// by a tool of another id is stepped over instead of being handed to the new tool.
  #[test]
  fn tool_library_create_never_takes_a_number_the_library_already_uses() {
    let dir = library_dir("tool-create-numbers");
    seed_library(
      &dir,
      &[
        library_tool(
          "tool-1",
          "Flat 1.0 mm",
          cam_store::tools::ToolKind::FlatEndMill,
          Some(1),
        ),
        library_tool(
          "tool-2",
          "Flat 2.0 mm",
          cam_store::tools::ToolKind::FlatEndMill,
          Some(3),
        ),
      ],
    );
    let rt = runtime_with_data_dir(&dir);

    let created = tool_library_create_impl(
      &rt,
      ToolCreateRequest {
        name: "End mill 3.0 mm".to_string(),
      },
    )
    .expect("the store accepts the new tool");

    // The ids `tool-1`/`tool-2` and the numbers 1 and 3 are taken, so the new tool gets the lowest
    // free number, 2, and the next free id.
    assert_eq!(created.id, "tool-3");
    assert_eq!(created.number, Some(2));
    // Nothing was overwritten: three tools are in the library, the two seeded ones untouched.
    let tree = tool_tree_impl(&rt).expect("tree");
    assert_eq!(tree.groups[0].count, 3);
    let numbers: Vec<Option<u32>> = tree.groups[0].tools.iter().map(|row| row.number).collect();
    assert_eq!(numbers, vec![Some(1), Some(3), Some(2)]);
    let created_row = tree.groups[0]
      .tools
      .iter()
      .find(|row| row.id == "tool-3")
      .expect("the created tool is listed");
    assert_eq!(created_row.number, Some(2));
    let seeded = tool_detail_impl(&rt, "tool-1")
      .expect("the query runs")
      .expect("tool-1 is still there");
    assert_eq!(seeded.number, Some(1));
    assert_eq!(seeded.name, "Flat 1.0 mm");

    let _ = fs::remove_dir_all(&dir);
  }

  /// A poisoned write lock is a structured refusal, not a write attempted anyway and not a panic.
  #[test]
  fn tool_library_write_of_a_poisoned_lock_is_structured() {
    // A lock of this test's own, so poisoning it leaves the process-wide one the other tests take
    // healthy; what is under test is the mapping a write does when its guard cannot be taken.
    static POISONED: Mutex<()> = Mutex::new(());
    let poisoned = std::panic::catch_unwind(|| {
      let _guard = POISONED.lock().expect("the scratch lock is fresh");
      panic!("poison the tool library write lock");
    });
    assert!(poisoned.is_err(), "the scratch lock is poisoned");

    let err = lock_or_refuse(&POISONED).expect_err("a poisoned lock refuses the write");
    assert_eq!(err.code, "tool_library_write_poisoned");
    assert_eq!(
      serde_json::to_value(&err).expect("json"),
      serde_json::json!({
        "code": "tool_library_write_poisoned",
        "message": "the tool library write lock is poisoned, so nothing was written"
      })
    );
  }
}
