//! The open project: one folder at a time, held here so the frontend only
//! ever sends paths relative to it. All file access goes through
//! `joyride_core::files`, which keeps it inside the folder.

use joyride_core::files::{self, Listing};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::State;

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
pub fn open_project(state: State<Project>, path: String) -> Result<Listing, String> {
    let root = PathBuf::from(path);
    let listing = files::list(&root).map_err(msg)?;
    *state.0.lock().unwrap() = Some(root.canonicalize().map_err(|e| e.to_string())?);
    Ok(listing)
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
