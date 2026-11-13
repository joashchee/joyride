//! The library manager's commands over `joyride_core::library` (the curated
//! list) and `joyride_core::pkg` (install, lock). Adding runs `git`, so it
//! happens off the UI thread and only when the person clicks Add.

use crate::project::Project;
use joyride_core::library::Library;
use joyride_core::manifest::Manifest;
use joyride_core::{pkg, toolchain};
use serde::Serialize;
use std::path::Path;
use tauri::State;

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
}

fn views(root: &Path, query: &str) -> Result<Vec<LibraryView>, String> {
    let v = toolchain::project_love_version(root).map_err(|e| e.to_string())?;
    let installed: Vec<String> = std::fs::read_to_string(root.join("love.toml"))
        .ok()
        .and_then(|t| Manifest::parse(&t).ok())
        .map(|m| m.dependencies.keys().cloned().collect())
        .unwrap_or_default();
    let lib = Library::bundled();
    Ok(lib
        .search(query, None)
        .into_iter()
        .map(|e| LibraryView {
            name: e.name.clone(),
            summary: e.summary.clone(),
            licence: e.licence.clone(),
            git: e.git.clone(),
            version: e.tag.clone().unwrap_or_else(|| e.rev.chars().take(8).collect()),
            badge: e.badge(v).label(),
            broken: e.known_broken_on(v),
            installed: installed.contains(&e.name),
        })
        .collect())
}

#[tauri::command]
pub fn library_list(project: State<Project>, query: String) -> Result<Vec<LibraryView>, String> {
    views(&project.root()?, &query)
}

/// Installs a curated library at its pinned commit; returns the fresh list.
#[tauri::command]
pub async fn library_add(project: State<'_, Project>, name: String, query: String) -> Result<Vec<LibraryView>, String> {
    let root = project.root()?;
    tauri::async_runtime::spawn_blocking(move || {
        pkg::add_library(&root, &Library::bundled(), &name).map_err(|e| e.to_string())?;
        views(&root, &query)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn library_remove(project: State<'_, Project>, name: String, query: String) -> Result<Vec<LibraryView>, String> {
    let root = project.root()?;
    tauri::async_runtime::spawn_blocking(move || {
        pkg::remove_library(&root, &name).map_err(|e| e.to_string())?;
        views(&root, &query)
    })
    .await
    .map_err(|e| e.to_string())?
}
