use serde::Serialize;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
  format!("Hello, {}! You've been greeted from Rust!", name)
}

#[derive(Serialize)]
pub struct DocumentStatus {
  pub schema: String,
  pub ok: bool,
  pub model_type: String,
  pub app_state_type: String,
  pub store_error_type: String,
}

/// Spike F0: prueba que crates de dominio enlazan desde el host Tauri.
#[tauri::command]
fn document_status() -> DocumentStatus {
  DocumentStatus {
    schema: "cam-document-status@0".into(),
    ok: true,
    model_type: std::any::type_name::<cam_model::Document>().into(),
    app_state_type: std::any::type_name::<cam_app::AppState>().into(),
    store_error_type: std::any::type_name::<cam_store::StoreError>().into(),
  }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .plugin(
      tauri_plugin_log::Builder::new()
        .level(tauri_plugin_log::log::LevelFilter::Info)
        .build(),
    )
    .plugin(tauri_plugin_opener::init())
    .invoke_handler(tauri::generate_handler![greet, document_status])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
