//! JoyrIDE's desktop shell: Tauri commands over `joyride-core` and
//! `asset-core`. The logic lives in those crates, never here, so a web build
//! can share it.

use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CoreInfo {
    version: &'static str,
}

/// The core crate's version, shown on the splash and in About.
#[tauri::command]
fn core_info() -> CoreInfo {
    CoreInfo { version: env!("CARGO_PKG_VERSION") }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_log::Builder::new().level(log::LevelFilter::Info).build())
        .invoke_handler(tauri::generate_handler![core_info])
        .run(tauri::generate_context!())
        .expect("error while running JoyrIDE");
}
