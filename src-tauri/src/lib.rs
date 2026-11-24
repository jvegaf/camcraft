use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use cam_app::{AppState, Viewport};
use cam_model::{Board, Document, Job, OriginCorner};
use cam_sim::Heightfield;
use cam_units::Length;
use cam_view::{build_stock_box, build_surface, Camera3d};
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
  fs::write(&tmp, text).map_err(|e| CommandErrorDto::new("settings_write_failed", e.to_string()))?;
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
      scene_snapshot
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
