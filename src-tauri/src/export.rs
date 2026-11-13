//! The Export Center's commands: builds go to `<project>/build/` (skipped
//! when copying, so it never ends up inside a build), made from a staged
//! copy; the project's own files are not touched (a missing target overlay is
//! written under `.joyride/`, as `joyride_core::pipeline` documents).

use crate::project::Project;
use joyride_core::compat::{self, Support};
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
    /// The 12.0-only APIs the project uses, one row each, for the variant.
    pub uses: Vec<UseRow>,
    /// The unsupported APIs the person accepted earlier (`.joyride/variant.toml`).
    pub accepted: Vec<String>,
}

#[derive(Serialize)]
pub struct UseRow {
    api: String,
    /// "exact", "reduced" or "unsupported".
    support: &'static str,
    /// What is given up, or why it cannot be emulated.
    note: String,
    /// The first place it is used.
    file: String,
    line: u32,
    count: usize,
}

const ACCEPT_FILE: &str = ".joyride/variant.toml";

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct Accepted {
    #[serde(default)]
    accepted: Vec<String>,
}

fn read_accepted(root: &std::path::Path) -> Vec<String> {
    std::fs::read_to_string(root.join(ACCEPT_FILE)).ok().and_then(|t| toml::from_str::<Accepted>(&t).ok()).map(|a| a.accepted).unwrap_or_default()
}

#[tauri::command]
pub fn export_info(project: State<Project>) -> Result<ExportInfo, String> {
    let root = project.root()?;
    let v = project_love_version(&root).map_err(|e| e.to_string())?;
    let offered = compat::offered(v);
    let mut uses: Vec<UseRow> = Vec::new();
    if offered {
        for u in compat::analyze_project(&root).uses {
            if let Some(r) = uses.iter_mut().find(|r| r.api == u.api) {
                r.count += 1;
                continue;
            }
            let (support, note) = match u.support {
                Support::Exact => ("exact", "behaves the same on 11.5".to_string()),
                Support::Reduced(w) => ("reduced", w.to_string()),
                Support::Unsupported(w) => ("unsupported", w.to_string()),
            };
            uses.push(UseRow { api: u.api, support, note, file: u.file, line: u.line, count: 1 });
        }
    }
    Ok(ExportInfo { love_version: v.to_string(), variant_offered: offered, uses, accepted: read_accepted(&root) })
}

/// Builds `kind` ("love" or "portmaster"); `variant` builds the 11.5 variant
/// of a 12.0 project (PortMaster needs it). Returns the file's path.
#[tauri::command]
pub async fn export_project(project: State<'_, Project>, kind: String, variant: bool, accept: Vec<String>) -> Result<String, String> {
    let root = project.root()?;
    tauri::async_runtime::spawn_blocking(move || {
        let out = root.join("build");
        if variant {
            // Remember what was accepted, so it is the same next time.
            let _ = std::fs::create_dir_all(root.join(".joyride"));
            let _ = std::fs::write(root.join(ACCEPT_FILE), toml::to_string(&Accepted { accepted: accept.clone() }).unwrap_or_default());
        }
        let b = Build { root: &root, out_dir: &out, gate: None, compat: variant.then_some(Compat { allow: &accept }) };
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
