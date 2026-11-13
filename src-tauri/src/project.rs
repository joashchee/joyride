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

/// Formats Lua text with the bundled StyLua (off the UI thread). The project
/// folder is the working directory, so its `stylua.toml` applies.
#[tauri::command]
pub async fn format_lua(app: AppHandle, state: State<'_, Project>, text: String) -> Result<String, String> {
    let root = state.root()?;
    let tc = crate::bundled_toolchains(&app);
    tauri::async_runtime::spawn_blocking(move || {
        let bin = joyride_core::format::find_stylua(tc.as_deref()).ok_or("StyLua was not found. Run scripts/fetch-toolchains.py.")?;
        joyride_core::format::format_lua(&bin, &root, &text).map_err(msg)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LintItem {
    line: u32,
    /// "error" or "warning".
    severity: &'static str,
    rule: &'static str,
    message: String,
}

/// The LÖVE lints (`joyride_core::lint`) for one file's text, calibrated to
/// the project's LÖVE version. Fast (a lexer pass), so it runs inline.
#[tauri::command]
pub fn lint_lua(state: State<Project>, path: String, text: String) -> Result<Vec<LintItem>, String> {
    let root = state.root()?;
    let v = joyride_core::toolchain::project_love_version(&root).map_err(msg)?;
    Ok(joyride_core::lint::lint(&text, &path, v)
        .into_iter()
        .map(|d| LintItem {
            line: d.line,
            severity: if d.severity == joyride_core::lint::Severity::Error { "error" } else { "warning" },
            rule: d.rule,
            message: d.message,
        })
        .collect())
}

#[derive(serde::Serialize)]
pub struct ColorItem {
    /// UTF-16 offsets (what the editor counts) of the numbers inside the parentheses.
    start: usize,
    end: usize,
    r: f64,
    g: f64,
    b: f64,
    a: f64,
}

/// `love.graphics.setColor(0.2, 0.4, 1)` literals, for the editor's colour swatches.
#[tauri::command]
pub fn color_literals(text: String) -> Vec<ColorItem> {
    let u16_at = |byte: usize| text.get(..byte).map(|s| s.encode_utf16().count()).unwrap_or(0);
    joyride_core::editor::color_literals(&text)
        .into_iter()
        .map(|c| ColorItem { start: u16_at(c.start), end: u16_at(c.end), r: c.r, g: c.g, b: c.b, a: c.a })
        .collect()
}

/// The text to write back for a picked colour: `0.2, 0.4, 1`.
#[tauri::command]
pub fn color_text(r: f64, g: f64, b: f64, a: f64) -> String {
    joyride_core::editor::format_color(r, g, b, a)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colour_offsets_count_utf16_not_bytes() {
        // "é" is two bytes but one UTF-16 unit, "😀" four bytes and two units.
        let text = "-- é😀\nlove.graphics.setColor(0.5, 0.25, 1)";
        let c = &color_literals(text.to_string())[0];
        let units: Vec<u16> = text.encode_utf16().collect();
        let inside = String::from_utf16(&units[c.start..c.end]).unwrap();
        assert_eq!(inside, "0.5, 0.25, 1");
        assert_eq!(color_text(0.5, 0.25, 1.0, 1.0), "0.5, 0.25, 1");
    }
}

#[derive(serde::Serialize)]
pub struct FileWarning {
    path: String,
    message: String,
}

/// Files in the project that its LÖVE version cannot use as they are
/// (`asset_core::kind::unusable_files`). Off the UI thread.
#[tauri::command]
pub async fn project_warnings(state: State<'_, Project>) -> Result<Vec<FileWarning>, String> {
    let root = state.root()?;
    tauri::async_runtime::spawn_blocking(move || {
        let v = joyride_core::toolchain::project_love_version(&root).map_err(msg)?.to_string();
        Ok(asset_core::kind::unusable_files(&root, &v).into_iter().map(|u| FileWarning { path: u.path, message: u.message }).collect())
    })
    .await
    .map_err(|e| e.to_string())?
}
