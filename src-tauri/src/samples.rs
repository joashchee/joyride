//! The sample projects that ship with the app (`joyride_core::samples`): list them, and copy one into the app-data folder to open.

use joyride_core::samples::{self, Sample};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

/// Where the shipped samples are: `Resources/_up_/samples` in a built app (the bundle config's `../samples` lands there), the
/// repository's `samples/` under `tauri dev`.
fn shipped(app: &AppHandle) -> Option<PathBuf> {
    let installed = app.path().resource_dir().ok().map(|d| d.join("_up_/samples"));
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../samples");
    installed.filter(|p| p.is_dir()).or_else(|| source.is_dir().then_some(source))
}

#[tauri::command]
pub fn sample_list(app: AppHandle) -> Vec<Sample> {
    shipped(&app).map(|r| samples::list(&r)).unwrap_or_default()
}

/// Copies a sample to `<app data>/samples/<id>` (keeping the person's edits if it is already there; `fresh` moves that copy aside and
/// copies again) and returns the folder, for the shell to open as the project.
#[tauri::command]
pub fn sample_copy(app: AppHandle, id: String, fresh: bool) -> Result<String, String> {
    let root = shipped(&app).ok_or("the samples are not in this build")?;
    let dest = app.path().app_data_dir().map_err(|e| e.to_string())?.join("samples");
    samples::copy_to(&root, &id, &dest, fresh).map(|p| p.display().to_string()).map_err(|e| e.to_string())
}
