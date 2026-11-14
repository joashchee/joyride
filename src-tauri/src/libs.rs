//! The library manager's commands over `joyride_core::library` (the curated
//! list) and `joyride_core::pkg` (install, lock). Adding runs `git`, so it
//! happens off the UI thread and only when the person clicks Add.

use crate::project::Project;
use joyride_core::detect::{self, Detection};
use joyride_core::library::Library;
use joyride_core::manifest::Manifest;
use joyride_core::{pkg, toolchain};
use serde::Serialize;
use std::path::Path;
use std::path::PathBuf;
use tauri::{AppHandle, Manager, State};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryView {
    name: String,
    summary: String,
    licence: String,
    git: String,
    /// The tag it was taken from, or the short commit.
    version: String,
    /// What is known about it on the project's LÖVE version.
    badge: String,
    broken: bool,
    installed: bool,
    /// "Lua", "C library (FFI)" or "C module".
    kind: String,
    /// For a C library: the binary that has to sit next to the game (desktop only; the web build has no FFI).
    native: Option<String>,
    /// A warning to read before adding it.
    caution: Option<String>,
    /// What the Benchmark Libraries sample measured for it (scores against what it was compared with, cost, findings).
    benchmark: Option<String>,
    /// One of the person's own, added from a project: not audited, not measured.
    custom: bool,
    /// Whether adding it has an address to fetch from.
    installable: bool,
}

/// Where the person's own libraries are kept: the app-data folder, never a project.
fn custom_file(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_data_dir().map(|d| d.join("custom-libraries.json")).map_err(|e| e.to_string())
}

fn offers_file(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_data_dir().map(|d| d.join("library-offers.json")).map_err(|e| e.to_string())
}

/// JoyrIDE's list and the person's own after it.
pub(crate) fn library(app: &AppHandle) -> Library {
    let custom = custom_file(app).map(|f| detect::load_custom(&f)).unwrap_or_default();
    Library::bundled().with_custom(&custom)
}

fn views(app: &AppHandle, root: &Path, query: &str) -> Result<Vec<LibraryView>, String> {
    let v = toolchain::project_love_version(root).map_err(|e| e.to_string())?;
    let installed: Vec<String> = std::fs::read_to_string(root.join("love.toml"))
        .ok()
        .and_then(|t| Manifest::parse(&t).ok())
        .map(|m| m.dependencies.keys().cloned().collect())
        .unwrap_or_default();
    let lib = library(app);
    Ok(lib
        .search(query, None)
        .into_iter()
        .map(|e| LibraryView {
            name: e.name.clone(),
            summary: e.summary.clone(),
            licence: e.licence.clone(),
            git: e.git.clone(),
            version: e.tag.clone().unwrap_or_else(|| e.rev.chars().take(8).collect()),
            // A C library cannot be loaded here without its binary, so no load result is claimed for it.
            badge: if e.kind.is_native() { "Not tested: needs its binary; desktop only".into() } else { e.badge(v).label() },
            broken: e.known_broken_on(v),
            installed: installed.contains(&e.name),
            kind: e.kind.label().into(),
            native: e.native.clone(),
            caution: e.caution.clone(),
            benchmark: e.benchmark.clone(),
            custom: e.custom,
            installable: e.installable(),
        })
        .collect())
}

#[tauri::command]
pub fn library_list(app: AppHandle, project: State<Project>, query: String) -> Result<Vec<LibraryView>, String> {
    views(&app, &project.root()?, &query)
}

/// What adding or removing a library did: the list, and what JoyrIDE did to `main.lua` for it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryChange {
    libraries: Vec<LibraryView>,
    /// The lines inserted into or removed from `main.lua` (empty when `integrate` was off).
    synced: joyride_core::integrate::Synced,
    /// The integration could not be done (the library itself was).
    integrate_error: Option<String>,
}

fn change(app: &AppHandle, root: &Path, query: &str, integrate: bool) -> Result<LibraryChange, String> {
    let libraries = views(app, root, query)?;
    let (synced, integrate_error) = if integrate {
        match joyride_core::integrate::sync(root, &library(app)) {
            Ok(s) => (s, None),
            Err(e) => (Default::default(), Some(e.to_string())),
        }
    } else {
        (Default::default(), None)
    };
    Ok(LibraryChange { libraries, synced, integrate_error })
}

/// Installs a curated library at its pinned commit and (when `integrate`) adds the line that lets `require` find it to `main.lua`.
/// `integrate` is off when `main.lua` has unsaved edits in the editor: the shell never writes under them.
#[tauri::command]
pub async fn library_add(app: AppHandle, project: State<'_, Project>, name: String, query: String, integrate: bool) -> Result<LibraryChange, String> {
    let root = project.root()?;
    tauri::async_runtime::spawn_blocking(move || {
        pkg::add_library(&root, &library(&app), &name).map_err(|e| e.to_string())?;
        change(&app, &root, &query, integrate)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Removes a library, and the line JoyrIDE inserted for it if it is still as JoyrIDE wrote it.
#[tauri::command]
pub async fn library_remove(app: AppHandle, project: State<'_, Project>, name: String, query: String, integrate: bool) -> Result<LibraryChange, String> {
    let root = project.root()?;
    tauri::async_runtime::spawn_blocking(move || {
        pkg::remove_library(&root, &name).map_err(|e| e.to_string())?;
        change(&app, &root, &query, integrate)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Brings `main.lua` in line with the project's libraries (for a project whose libraries were added before, or while `main.lua` had unsaved edits).
#[tauri::command]
pub async fn integrate_sync(app: AppHandle, project: State<'_, Project>) -> Result<joyride_core::integrate::Synced, String> {
    let root = project.root()?;
    tauri::async_runtime::spawn_blocking(move || joyride_core::integrate::sync(&root, &library(&app)).map_err(|e| e.to_string())).await.map_err(|e| e.to_string())?
}

/// The lines of `path` that JoyrIDE inserted and that are still untouched in `text` (what the editor holds).
#[tauri::command]
pub fn integrate_marks(project: State<Project>, path: String, text: String) -> Result<Vec<joyride_core::integrate::Mark>, String> {
    joyride_core::integrate::marks(&project.root()?, &path, &text).map_err(|e| e.to_string())
}

/// The person touched an inserted line: it is theirs from now on.
#[tauri::command]
pub fn integrate_clear(project: State<Project>, id: String) -> Result<(), String> {
    joyride_core::integrate::clear(&project.root()?, &id).map_err(|e| e.to_string())
}

/// Takes one of the person's own libraries out of the list (it does not touch any project).
#[tauri::command]
pub fn library_custom_remove(app: AppHandle, project: State<Project>, name: String, query: String) -> Result<Vec<LibraryView>, String> {
    detect::remove_custom(&custom_file(&app)?, &name).map_err(|e| e.to_string())?;
    views(&app, &project.root()?, &query)
}

// ---- finding the libraries a project uses ---------------------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OfferView {
    /// The project folder's name.
    project: String,
    #[serde(flatten)]
    found: Detection,
    /// How many were left out because the person said not to ask again.
    hidden: usize,
}

fn project_key(root: &Path) -> String {
    root.to_string_lossy().into_owned()
}

fn project_name(root: &Path) -> String {
    root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Reads the open project's files (nothing is run, nothing fetched) and says which libraries it uses that the list does not have.
/// `force` shows the ones the person asked not to hear about again.
#[tauri::command]
pub async fn libraries_detect(app: AppHandle, project: State<'_, Project>, force: bool) -> Result<OfferView, String> {
    let root = project.root()?;
    tauri::async_runtime::spawn_blocking(move || {
        let mut d = detect::detect(&root, &library(&app)).map_err(|e| e.to_string())?;
        let skipped = if force { Vec::new() } else { offers_file(&app).map(|f| detect::load_dismissed(&f, &project_key(&root))).unwrap_or_default() };
        let before = d.new.len();
        d.new.retain(|f| !skipped.contains(&f.name));
        Ok(OfferView { project: project_name(&root), hidden: before - d.new.len(), found: d })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Adds the named libraries (found again now, so only what is really in the project) to the person's own list.
#[tauri::command]
pub async fn libraries_offer_add(app: AppHandle, project: State<'_, Project>, names: Vec<String>) -> Result<usize, String> {
    let root = project.root()?;
    tauri::async_runtime::spawn_blocking(move || {
        let d = detect::detect(&root, &library(&app)).map_err(|e| e.to_string())?;
        let chosen: Vec<_> = d.new.into_iter().filter(|f| names.contains(&f.name)).collect();
        detect::add_custom(&custom_file(&app)?, &chosen, &project_name(&root)).map_err(|e| e.to_string())?;
        Ok(chosen.len())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// "Do not ask again for these" in this project.
#[tauri::command]
pub fn libraries_offer_dismiss(app: AppHandle, project: State<Project>, names: Vec<String>) -> Result<(), String> {
    let root = project.root()?;
    detect::dismiss(&offers_file(&app)?, &project_key(&root), &names).map_err(|e| e.to_string())
}
