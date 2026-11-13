//! The Auto-Port panel's commands: what the scanner finds for each target,
//! and writing the target's overlay under `.joyride/patches/<target>/`
//! (never touching the project's own files; `overlay::write` keeps an edited
//! `patch.toml` and `game.gptk`).

use crate::project::Project;
use joyride_core::overlay;
use joyride_core::scan::{self, Level, Target};
use serde::Serialize;
use tauri::State;

const TARGETS: [Target; 3] = [Target::PortMaster, Target::Mobile, Target::Web];

#[derive(Serialize)]
pub struct Finding {
    level: &'static str,
    file: String,
    line: u32,
    rule: &'static str,
    message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetReport {
    /// The overlay folder name: `portmaster`, `mobile` or `web`.
    name: &'static str,
    title: &'static str,
    findings: Vec<Finding>,
    blockers: usize,
    /// The overlay's files that exist, relative to the project.
    overlay_files: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    files: usize,
    keyboard: bool,
    mouse: bool,
    touch: bool,
    gamepad: bool,
    keys: Vec<String>,
    targets: Vec<TargetReport>,
}

fn title(t: Target) -> &'static str {
    match t {
        Target::PortMaster => "PortMaster handhelds",
        Target::Mobile => "iOS, iPadOS and Android",
        Target::Web => "Web",
    }
}

fn report(root: &std::path::Path) -> Report {
    let s = scan::scan_project(root);
    let targets = TARGETS
        .iter()
        .map(|&t| {
            let name = overlay::target_dir_name(t);
            let dir = format!(".joyride/patches/{name}");
            let overlay_files = ["patch.toml", "game.gptk", "_jr_shim.lua"].iter().map(|f| format!("{dir}/{f}")).filter(|p| root.join(p).is_file()).collect();
            TargetReport {
                name,
                title: title(t),
                findings: s
                    .for_target(t)
                    .map(|f| Finding {
                        level: match f.level {
                            Level::Info => "info",
                            Level::Warn => "warning",
                            Level::Blocker => "blocker",
                        },
                        file: f.file.clone(),
                        line: f.line,
                        rule: f.rule,
                        message: f.message.clone(),
                    })
                    .collect(),
                blockers: s.blockers(t),
                overlay_files,
            }
        })
        .collect();
    Report { files: s.files, keyboard: s.keyboard, mouse: s.mouse, touch: s.touch, gamepad: s.gamepad, keys: s.keys.iter().cloned().collect(), targets }
}

/// Scans the project (off the UI thread).
#[tauri::command]
pub async fn autoport_scan(project: State<'_, Project>) -> Result<Report, String> {
    let root = project.root()?;
    tauri::async_runtime::spawn_blocking(move || Ok(report(&root))).await.map_err(|e| e.to_string())?
}

/// Writes (or refreshes the generated parts of) a target's overlay and
/// returns the fresh report.
#[tauri::command]
pub async fn autoport_generate(project: State<'_, Project>, target: String) -> Result<Report, String> {
    let root = project.root()?;
    tauri::async_runtime::spawn_blocking(move || {
        let t = TARGETS.iter().copied().find(|&t| overlay::target_dir_name(t) == target).ok_or_else(|| format!("unknown target `{target}`"))?;
        let s = scan::scan_project(&root);
        overlay::write(&root, &overlay::generate(t, &s)).map_err(|e| e.to_string())?;
        Ok(report(&root))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_every_target_and_the_overlay_files_after_writing() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("main.lua"), "function love.keypressed(k) if k == 'space' then end end").unwrap();
        let r = report(d.path());
        assert_eq!(r.targets.iter().map(|t| t.name).collect::<Vec<_>>(), ["portmaster", "mobile", "web"]);
        assert!(r.keyboard && r.keys == ["space"]);
        assert!(r.targets.iter().all(|t| t.overlay_files.is_empty()));
        let s = scan::scan_project(d.path());
        overlay::write(d.path(), &overlay::generate(Target::PortMaster, &s)).unwrap();
        let r = report(d.path());
        let pm = &r.targets[0];
        assert!(pm.overlay_files.iter().any(|f| f.ends_with("game.gptk")) && pm.overlay_files.iter().any(|f| f.ends_with("patch.toml")), "{:?}", pm.overlay_files);
        assert!(r.targets[1].overlay_files.is_empty());
        assert_eq!(std::fs::read_to_string(d.path().join("main.lua")).unwrap(), "function love.keypressed(k) if k == 'space' then end end");
    }
}
