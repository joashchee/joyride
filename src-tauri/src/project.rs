//! The open project: one folder at a time, held here so the frontend only
//! ever sends paths relative to it. All file access goes through
//! `joyride_core::files`, which keeps it inside the folder.

use joyride_core::files::{self, Listing};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Manager, State};

#[derive(Default)]
pub struct Project(Mutex<Option<PathBuf>>);

impl Project {
    pub fn root(&self) -> Result<PathBuf, String> {
        self.0.lock().unwrap().clone().ok_or_else(|| "no project is open".to_string())
    }
}

fn msg(e: joyride_core::Error) -> String {
    e.to_string()
}

/// Opens a folder as the project and lists it.
#[tauri::command]
pub fn open_project(app: AppHandle, state: State<Project>, path: String) -> Result<Listing, String> {
    let root = PathBuf::from(path);
    let listing = files::list(&root).map_err(msg)?;
    let canon = root.canonicalize().map_err(|e| e.to_string())?;
    remember(&app, &canon);
    *state.0.lock().unwrap() = Some(canon);
    Ok(listing)
}

/// Where the last project's path is kept: the app-data folder, never the project.
fn last_file(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_data_dir().ok().map(|d| d.join("last-project.txt"))
}

fn remember(app: &AppHandle, root: &std::path::Path) {
    if let Some(f) = last_file(app) {
        if let Some(d) = f.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = std::fs::write(f, root.to_string_lossy().as_bytes());
    }
}

/// The folder opened last time, if it is still a folder; the frontend opens it
/// after the first screen so the editor comes back where it was.
#[tauri::command]
pub fn last_project(app: AppHandle) -> Option<String> {
    let p = std::fs::read_to_string(last_file(&app)?).ok()?;
    let p = PathBuf::from(p.trim());
    p.is_dir().then(|| p.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn project_listing(state: State<Project>) -> Result<Listing, String> {
    files::list(&state.root()?).map_err(msg)
}

#[tauri::command]
pub fn read_project_file(state: State<Project>, path: String) -> Result<String, String> {
    files::read_text(&state.root()?, &path).map_err(msg)
}

#[tauri::command]
pub fn write_project_file(state: State<Project>, path: String, text: String) -> Result<(), String> {
    files::write_text(&state.root()?, &path, &text).map_err(msg)
}

/// Each change returns the fresh listing, so the tree never drifts from disk.
#[tauri::command]
pub fn create_project_file(state: State<Project>, path: String) -> Result<Listing, String> {
    let root = state.root()?;
    files::create_file(&root, &path).map_err(msg)?;
    files::list(&root).map_err(msg)
}

#[tauri::command]
pub fn create_project_dir(state: State<Project>, path: String) -> Result<Listing, String> {
    let root = state.root()?;
    files::create_dir(&root, &path).map_err(msg)?;
    files::list(&root).map_err(msg)
}

#[tauri::command]
pub fn rename_project_path(state: State<Project>, from: String, to: String) -> Result<Listing, String> {
    let root = state.root()?;
    files::rename(&root, &from, &to).map_err(msg)?;
    files::list(&root).map_err(msg)
}

/// Permanent: the frontend confirms first.
#[tauri::command]
pub fn delete_project_path(state: State<Project>, path: String) -> Result<Listing, String> {
    let root = state.root()?;
    files::delete(&root, &path).map_err(msg)?;
    files::list(&root).map_err(msg)
}

/// Plain-text search over the project, off the UI thread.
#[tauri::command]
pub async fn search_project(state: State<'_, Project>, query: String, case_sensitive: bool) -> Result<files::Found, String> {
    let root = state.root()?;
    tauri::async_runtime::spawn_blocking(move || files::search(&root, &query, case_sensitive).map_err(msg))
        .await
        .map_err(|e| e.to_string())?
}
