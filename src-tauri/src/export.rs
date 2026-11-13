//! The Export Center's commands: builds go to `<project>/build/` (skipped
//! when copying, so it never ends up inside a build), made from a staged
//! copy; the project's own files are not touched (a missing target overlay is
//! written under `.joyride/`, as `joyride_core::pipeline` documents).

use crate::project::Project;
use joyride_core::pipeline::{self, Build, Compat};
use joyride_core::toolchain::project_love_version;
use serde::Serialize;
use tauri::State;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportInfo {
    pub love_version: String,
    /// A 12.0 project can build an 11.5 variant.
    pub variant_offered: bool,
}

#[tauri::command]
pub fn export_info(project: State<Project>) -> Result<ExportInfo, String> {
    let root = project.root()?;
    let v = project_love_version(&root).map_err(|e| e.to_string())?;
    Ok(ExportInfo { love_version: v.to_string(), variant_offered: joyride_core::compat::offered(v) })
}

/// Builds `kind` ("love" or "portmaster"); `variant` builds the 11.5 variant
/// of a 12.0 project (PortMaster needs it). Returns the file's path.
#[tauri::command]
pub async fn export_project(project: State<'_, Project>, kind: String, variant: bool) -> Result<String, String> {
    let root = project.root()?;
    tauri::async_runtime::spawn_blocking(move || {
        let out = root.join("build");
        let b = Build { root: &root, out_dir: &out, gate: None, compat: variant.then_some(Compat { allow: &[] }) };
        let path = match kind.as_str() {
            "love" => pipeline::build_plain_love(&b),
            "portmaster" => pipeline::build_portmaster(&b),
            other => return Err(format!("unknown export `{other}`")),
        }
        .map_err(|e| e.to_string())?;
        Ok(path.to_string_lossy().into_owned())
    })
    .await
    .map_err(|e| e.to_string())?
}
