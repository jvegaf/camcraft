use std::sync::Mutex;

use cam_app::AppState;
use cam_model::{Board, Document};
use serde::{Deserialize, Serialize};
use tauri::State;

struct AppRuntime(Mutex<AppState>);

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentStatusDto {
  pub schema: String,
  pub document_schema: String,
  pub dirty: bool,
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

fn snapshot(state: &AppState) -> DocumentStatusDto {
  let doc = state.document();
  DocumentStatusDto {
    schema: "cam-document-status@1".into(),
    document_schema: doc.schema.clone(),
    dirty: state.is_dirty(),
    layer_count: doc.layers.len(),
    toolpath_count: doc.toolpaths.len(),
    board_width_mm: doc.board.width.mm(),
    board_height_mm: doc.board.height.mm(),
    board_thickness_mm: doc.board.thickness.mm(),
  }
}

#[tauri::command]
fn document_status(runtime: State<'_, AppRuntime>) -> Result<DocumentStatusDto, String> {
  let state = runtime.0.lock().map_err(|_| "runtime state poisoned")?;
  Ok(snapshot(&state))
}

#[tauri::command]
fn new_document(
  runtime: State<'_, AppRuntime>,
  request: NewDocumentRequest,
) -> Result<DocumentStatusDto, String> {
  let mut state = runtime.0.lock().map_err(|_| "runtime state poisoned")?;
  let mut board = Board::default_board();
  board.width = cam_units::Length::from_mm(request.width_mm);
  board.height = cam_units::Length::from_mm(request.height_mm);
  board.thickness = cam_units::Length::from_mm(request.thickness_mm);
  let doc = Document::new(board);
  state.replace_document(doc);
  Ok(snapshot(&state))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  let initial = AppState::new(Document::new(Board::default_board()));

  tauri::Builder::default()
    .manage(AppRuntime(Mutex::new(initial)))
    .plugin(
      tauri_plugin_log::Builder::new()
        .level(tauri_plugin_log::log::LevelFilter::Info)
        .build(),
    )
    .plugin(tauri_plugin_opener::init())
    .invoke_handler(tauri::generate_handler![document_status, new_document])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
