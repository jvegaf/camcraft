use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use cam_app::{AppState, Viewport};
use cam_model::{Board, Document, Job, OriginCorner};
use cam_sim::Heightfield;
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
  serde_json::to_value(operation)
    .ok()
    .and_then(|value| {
      value
        .get("kind")
        .and_then(|kind| kind.as_str())
        .map(str::to_string)
    })
    .ok_or_else(|| {
      CommandErrorDto::new(
        "operation_type_missing",
        "model operation serialized without a kind tag",
      )
    })
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
      cam_output
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
}
