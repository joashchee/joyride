//! The language server for the open project: started on demand, its messages
//! forwarded to the editor as `lsp-message` events (the frame work is
//! `joyride_core::lsp`).

use crate::project::Project;
use joyride_core::lsp::{self, Server};
use std::process::Command;
use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Default)]
pub struct Lsp {
    server: Mutex<Option<Server>>,
    /// Counts starts, so the editor can ignore a stopped server's late events.
    session: Arc<AtomicU64>,
}

#[derive(Serialize, Clone)]
struct Event {
    session: u64,
    text: Option<String>,
}

#[derive(Serialize)]
pub struct Started {
    uri: String,
    session: u64,
}

/// Starts `lua-language-server` for the open project (replacing a running
/// one) and returns the project's `file://` URI with a session number. Messages
/// arrive as `lsp-message` events carrying that number and the JSON text;
/// `lsp-end` says it stopped.
#[tauri::command]
pub fn lsp_start(app: AppHandle, project: State<Project>, lsp: State<Lsp>) -> Result<Started, String> {
    let root = project.root()?;
    let bundled = crate::bundled_toolchains(&app);
    let downloaded = app.path().app_data_dir().ok().map(|d| d.join("toolchains"));
    let bin = lsp::find_server(bundled.as_deref())
        .or_else(|| lsp::find_server(downloaded.as_deref()))
        .ok_or("The language server was not found, so completions and diagnostics are off. Run scripts/fetch-toolchains.py.")?;
    // Its logs and generated metadata go in the app's own folders, never the project's.
    let data = app.path().app_data_dir().map_err(|e| e.to_string())?.join("luals");
    let mut cmd = Command::new(bin);
    cmd.arg(format!("--logpath={}", data.join("log").display()))
        .arg(format!("--metapath={}", data.join("meta").display()))
        .current_dir(&root);
    let session = lsp.session.fetch_add(1, Ordering::SeqCst) + 1;
    let (a1, a2) = (app.clone(), app.clone());
    let server = Server::start(
        cmd,
        move |m| {
            let _ = a1.emit("lsp-message", Event { session, text: Some(m) });
        },
        move || {
            let _ = a2.emit("lsp-end", Event { session, text: None });
        },
    )
    .map_err(|e| e.to_string())?;
    if let Some(old) = lsp.server.lock().unwrap().replace(server) {
        old.stop();
    }
    Ok(Started { uri: lsp::path_to_uri(&root), session })
}

#[tauri::command]
pub fn lsp_send(lsp: State<Lsp>, message: String) -> Result<(), String> {
    match lsp.server.lock().unwrap().as_ref() {
        Some(s) => s.send(&message).map_err(|e| e.to_string()),
        None => Err("the language server is not running".into()),
    }
}

#[tauri::command]
pub fn lsp_stop(lsp: State<Lsp>) {
    if let Some(s) = lsp.server.lock().unwrap().take() {
        s.stop();
    }
}

/// What the server asks for with `workspace/configuration`.
#[tauri::command]
pub fn lsp_settings(app: AppHandle, project: tauri::State<crate::project::Project>) -> serde_json::Value {
    let bundled = crate::bundled_toolchains(&app);
    // The project's own require path, so the server finds what `main.lua` makes the game find.
    let path = project.root().ok().map(|root| joyride_core::integrate::runtime_path(&root, &crate::libs::library(&app)));
    lsp::settings_with(lsp::find_love_types(bundled.as_deref()).as_deref(), path)
}

/// The open project's `file://` URI, so the editor can name its models
/// before the server is ready.
#[tauri::command]
pub fn lsp_root(project: State<Project>) -> Result<String, String> {
    Ok(lsp::path_to_uri(&project.root()?))
}
