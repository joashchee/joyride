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
pub fn run_project(app: AppHandle, project: State<Project>, game: State<Game>) -> Result<(), String> {
    let root = project.root()?;
    // The LÖVE shipped in the app first (scripts/fetch-toolchains.py), then one
    // downloaded into the app-data folder, then any installed LÖVE.
    let bundled = crate::bundled_toolchains(&app);
    let downloaded = app.path().app_data_dir().ok().map(|d| d.join("toolchains"));
    let love = runner::find_love(bundled.as_deref(), Some("11.5"))
        .or_else(|| runner::find_love(downloaded.as_deref(), Some("11.5")))
        .or_else(|| runner::find_love(None, None))
        .ok_or("LÖVE was not found. Install LÖVE, or set JOYRIDE_LOVE to its executable.")?;
    let spec = Spec { love, project: root.clone(), env: vec![] };
    let (a1, a2) = (app.clone(), app.clone());
    let base = root.canonicalize().map_err(|e| e.to_string())?;
    let running = runner::start(
        &spec,
        move |stream, text, link| {
            let (file, line) = match link.and_then(|l| {
                let p = runner::resolve_link(&base, &l)?;
                Some((p.strip_prefix(&base).ok()?.to_string_lossy().replace('\\', "/"), l.line))
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
