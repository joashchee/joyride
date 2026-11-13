//! The Export Center's commands: builds go to `<project>/build/` (skipped
//! when copying, so it never ends up inside a build), made from a staged
//! copy; the project's own files are not touched (a missing target overlay is
//! written under `.joyride/`, as `joyride_core::pipeline` documents).

use crate::project::Project;
use joyride_core::compat::{self, Support};
use joyride_core::pipeline::{self, AssetGate, Build, Compat};
use joyride_core::toolchain::project_love_version;
use serde::Serialize;
use tauri::{AppHandle, State};

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
    /// Files this LÖVE version cannot use as they are: "path: why".
    pub unusable: Vec<String>,
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
    Ok(ExportInfo { love_version: v.to_string(), variant_offered: offered, uses, accepted: read_accepted(&root), unusable: asset_core::kind::unusable_files(&root, &v.to_string()).into_iter().map(|u| format!("{}: {}", u.path, u.message)).collect() })
}

/// Builds `kind` ("love" or "portmaster"); `variant` builds the 11.5 variant
/// of a 12.0 project (PortMaster needs it). Returns the file's path.
#[tauri::command]
pub async fn export_project(app: AppHandle, project: State<'_, Project>, kind: String, variant: bool, accept: Vec<String>, commercial: bool) -> Result<String, String> {
    let root = project.root()?;
    let library = crate::assets::library_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let out = root.join("build");
        if variant {
            // Remember what was accepted, so it is the same next time.
            let _ = std::fs::create_dir_all(root.join(".joyride"));
            let _ = std::fs::write(root.join(ACCEPT_FILE), toml::to_string(&Accepted { accepted: accept.clone() }).unwrap_or_default());
        }
        // The asset gate (CLAUDE.md rule 11): an asset with an unconfirmed licence, or one that cannot be
        // sold in a game that is sold, stops the build; CREDITS.txt is added to it.
        std::fs::create_dir_all(&library).map_err(|e| e.to_string())?;
        let catalog = asset_core::Catalog::open(&library.join("catalog.sqlite")).map_err(|e| e.to_string())?;
        let name = root.canonicalize().map_err(|e| e.to_string())?.file_name().and_then(|n| n.to_str()).unwrap_or("project").to_string();
        let today = today_utc();
        let gate = AssetGate { catalog: &catalog, project: &name, commercial, today: &today };
        let b = Build { root: &root, out_dir: &out, gate: Some(gate), compat: variant.then_some(Compat { allow: &accept }) };
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

/// Today's date in UTC as `YYYY-MM-DD` (days since 1970 to a civil date, Howard Hinnant's algorithm).
pub(crate) fn today_utc() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    civil(secs.div_euclid(86_400))
}

fn civil(days: i64) -> String {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{:04}-{:02}-{:02}", if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        assert_eq!(civil(0), "1970-01-01");
        assert_eq!(civil(19_723), "2024-01-01");
        assert_eq!(civil(19_782), "2024-02-29");
        assert_eq!(civil(20_406), "2025-11-14");
        assert_eq!(today_utc().len(), 10);
    }

    /// An unconfirmed library asset in the project stops the build; a confirmed one lets it through with credits.
    #[test]
    fn the_gate_stops_an_unconfirmed_asset() {
        use asset_core::catalog::NewAsset;
        use asset_core::{Catalog, Kind, Licence};
        let d = tempfile::tempdir().unwrap();
        let root = d.path().join("game");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("main.lua"), "function love.draw() end").unwrap();
        let mut cat = Catalog::in_memory().unwrap();
        let (id, _) = cat.add_asset(&NewAsset { hash: "cd".repeat(32), size: 1, kind: Kind::Image, name: "a.png".into(), ext: "png".into(), stored: "x".into() }).unwrap();
        cat.record_use(id, "game", "a.png").unwrap();
        let out = d.path().join("out");
        std::fs::create_dir_all(&out).unwrap();
        fn gate(cat: &Catalog, commercial: bool) -> AssetGate<'_> {
            AssetGate { catalog: cat, project: "game", commercial, today: "2026-11-14" }
        }
        let b = Build { root: &root, out_dir: &out, gate: Some(gate(&cat, false)), compat: None };
        assert!(pipeline::build_plain_love(&b).unwrap_err().to_string().contains("licence not confirmed"));
        cat.confirm_licence(id, &Licence::from_spdx("CC-BY-NC-4.0"), "J", "").unwrap();
        let b = Build { root: &root, out_dir: &out, gate: Some(gate(&cat, true)), compat: None };
        assert!(pipeline::build_plain_love(&b).unwrap_err().to_string().contains("does not allow commercial use"));
        let b = Build { root: &root, out_dir: &out, gate: Some(gate(&cat, false)), compat: None };
        assert!(pipeline::build_plain_love(&b).unwrap().is_file());
    }
}
