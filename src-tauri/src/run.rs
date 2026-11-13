//! Running the open project with LÖVE and streaming its output to the
//! console as events. The process work is `joyride_core::runner`.

use crate::project::Project;
use joyride_core::runner::{self, Running, Spec, Stream};
use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Default)]
pub struct Game(Mutex<Option<Running>>);

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Line {
    /// "out" or "err".
    stream: &'static str,
    text: String,
    /// The project file and line the text points at, when it stays inside the project.
    file: Option<String>,
    line: Option<u32>,
}

#[derive(Serialize, Clone)]
struct Exit {
    code: Option<i32>,
}

/// Starts the open project; output arrives as `run-line` events and the end
/// as `run-exit`. A game already running is stopped first.
#[tauri::command]
pub fn run_project(app: AppHandle, project: State<Project>, game: State<Game>, screen: Option<String>) -> Result<(), String> {
    let root = project.root()?;
    // The LÖVE shipped in the app first (scripts/fetch-toolchains.py), then one
    // downloaded into the app-data folder, then any installed LÖVE.
    let bundled = crate::bundled_toolchains(&app);
    let downloaded = app.path().app_data_dir().ok().map(|d| d.join("toolchains"));
    // The version the project asks for (`love.toml`, else 11.5).
    let version = joyride_core::toolchain::project_love_version(&root).map_err(|e| e.to_string())?.to_string();
    let love = runner::find_love(bundled.as_deref(), Some(&version))
        .or_else(|| runner::find_love(downloaded.as_deref(), Some(&version)))
        .ok_or_else(|| format!("LÖVE {version} was not found. Install it (for example as /Applications/love-{version}.app), or set JOYRIDE_LOVE to its executable."))?;
    // A simulated screen runs a staged copy with the target's overlay; links
    // in its output are mapped back to the project's own files.
    let (run_dir, staged) = match screen.as_deref() {
        Some(id) if !id.is_empty() => {
            let dir = app.path().app_cache_dir().map_err(|e| e.to_string())?.join("simulator");
            let _ = std::fs::remove_dir_all(&dir);
            (joyride_core::simulator::prepare(&root, id, &dir).map_err(|e| e.to_string())?, true)
        }
        _ => (root.clone(), false),
    };
    // Files this LÖVE version cannot use, said first in the console (a warning, the game still runs).
    for u in asset_core::kind::unusable_files(&root, &version) {
        let _ = app.emit("run-line", Line { stream: "err", text: format!("Warning: {}: {}", u.path, u.message), file: Some(u.path), line: Some(1) });
    }
    let spec = Spec { love, project: run_dir.clone(), env: vec![] };
    let (a1, a2) = (app.clone(), app.clone());
    let base = run_dir.canonicalize().map_err(|e| e.to_string())?;
    let real = root.canonicalize().map_err(|e| e.to_string())?;
    let running = runner::start(
        &spec,
        move |stream, text, link| {
            let (file, line) = match link.and_then(|l| {
                let p = runner::resolve_link(&base, &l)?;
                let rel = p.strip_prefix(&base).ok()?.to_string_lossy().replace('\\', "/");
                let rel = if staged { joyride_core::simulator::source_name(&rel).to_string() } else { rel };
                // Only files that exist in the project itself (not the shim).
                (!staged || real.join(&rel).is_file()).then_some((rel, l.line))
            }) {
                Some((f, n)) => (Some(f), Some(n)),
                None => (None, None),
            };
            let _ = a1.emit("run-line", Line { stream: if stream == Stream::Stdout { "out" } else { "err" }, text: text.to_string(), file, line });
        },
        move |code| {
            let _ = a2.emit("run-exit", Exit { code });
        },
    )
    .map_err(|e| e.to_string())?;
    if let Some(old) = game.0.lock().unwrap().replace(running) {
        old.stop();
    }
    Ok(())
}

#[tauri::command]
pub fn stop_project(game: State<Game>) {
    if let Some(g) = game.0.lock().unwrap().take() {
        g.stop();
    }
}
