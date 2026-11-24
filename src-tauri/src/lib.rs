use std::path::PathBuf;
use std::sync::Mutex;

use cam_app::AppState;
use cam_model::{Board, Document};
use serde::{Deserialize, Serialize};
use tauri::State;

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

struct RuntimeState {
  app: AppState,
  path: Option<PathBuf>,
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
    path: state.path.as_ref().map(|p| p.to_string_lossy().into_owned()),
    layer_count: doc.layers.len(),
    toolpath_count: doc.toolpaths.len(),
    board_width_mm: doc.board.width.mm(),
    board_height_mm: doc.board.height.mm(),
    board_thickness_mm: doc.board.thickness.mm(),
  }
}

#[tauri::command]
fn document_status(runtime: State<'_, AppRuntime>) -> Result<DocumentStatusDto, CommandErrorDto> {
  let state = runtime.0.lock().map_err(|_| CommandErrorDto::state_poisoned())?;
  Ok(snapshot(&state))
}

#[tauri::command]
fn new_document(
  runtime: State<'_, AppRuntime>,
  request: NewDocumentRequest,
) -> Result<DocumentStatusDto, CommandErrorDto> {
  let mut state = runtime.0.lock().map_err(|_| CommandErrorDto::state_poisoned())?;
  let mut board = Board::default_board();
  board.width = cam_units::Length::from_mm(request.width_mm);
  board.height = cam_units::Length::from_mm(request.height_mm);
  board.thickness = cam_units::Length::from_mm(request.thickness_mm);
  let doc = Document::new(board);
  state.app.replace_document(doc);
  state.path = None;
  Ok(snapshot(&state))
}

#[tauri::command]
fn open_document(
  runtime: State<'_, AppRuntime>,
  path: String,
) -> Result<DocumentStatusDto, CommandErrorDto> {
  let doc = cam_store::document::load_document(&path)
    .map_err(|e| CommandErrorDto::new("document_open_failed", e.to_string()))?;
  let mut state = runtime.0.lock().map_err(|_| CommandErrorDto::state_poisoned())?;
  state.app.replace_document(doc);
  state.path = Some(PathBuf::from(path));
  Ok(snapshot(&state))
}

#[tauri::command]
fn save_document(
  runtime: State<'_, AppRuntime>,
  path: Option<String>,
) -> Result<DocumentStatusDto, CommandErrorDto> {
  let mut state = runtime.0.lock().map_err(|_| CommandErrorDto::state_poisoned())?;
  let target = path
    .map(PathBuf::from)
    .or_else(|| state.path.clone())
    .ok_or_else(|| CommandErrorDto::new("document_path_missing", "no document path; use save as"))?;
  let doc = state.app.document().clone();
  cam_store::document::save_document(&target, &doc)
    .map_err(|e| CommandErrorDto::new("document_save_failed", e.to_string()))?;
  state.path = Some(target);
  state.app.mark_saved();
  Ok(snapshot(&state))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  let initial = RuntimeState {
    app: AppState::new(Document::new(Board::default_board())),
    path: None,
  };

  tauri::Builder::default()
    .manage(AppRuntime(Mutex::new(initial)))
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
      save_document
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
