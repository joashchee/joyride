//! Autosave drafts, the error log and crash reports (`docs/recovery.md`). The
//! logic is `joyride_core::recovery`; this is the wiring: a panic hook, the
//! session marker, and the commands the window calls.

use crate::project::Project;
use joyride_core::recovery::{self as core, Draft, Drafts, Report, ReportInfo};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;

struct Env {
    reports: PathBuf,
    log_file: PathBuf,
    started: u64,
}

static ENV: OnceLock<Env> = OnceLock::new();
/// The report for an exit that never said goodbye, until the window has been told about it.
static PREVIOUS: Mutex<Option<ReportInfo>> = Mutex::new(None);
/// Window errors reported this session: after a few, the rest are only logged.
static FRONTEND_REPORTS: AtomicUsize = AtomicUsize::new(0);
const MAX_FRONTEND_REPORTS: usize = 5;
const LOG_LINES: usize = 80;

fn home() -> Option<String> {
    std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).ok()
}

/// Where a report goes: the app-data folder once the app is up, else the system's temp folder
/// (a panic before setup still leaves something).
fn reports_dir() -> PathBuf {
    ENV.get().map(|e| e.reports.clone()).unwrap_or_else(|| std::env::temp_dir().join("joyride-crash-reports"))
}

fn fill(mut r: Report) -> Report {
    if let Some(e) = ENV.get() {
        r.session_secs = Some(core::now_secs().saturating_sub(e.started));
        r.log_tail = core::log_tail(&e.log_file, LOG_LINES, home().as_deref());
    }
    r.summary = core::redact(&r.summary, home().as_deref());
    r.location = r.location.map(|l| core::redact(&l, home().as_deref()));
    r.backtrace = r.backtrace.map(|b| core::redact(&b, home().as_deref()));
    r
}

fn save(r: Report) {
    match core::write_report(&reports_dir(), &fill(r)) {
        Ok(p) => log::error!("report written: {}", core::redact(&p.to_string_lossy(), home().as_deref())),
        Err(e) => log::error!("could not write the report: {e}"),
    }
}

/// Installs the panic hook. Called first thing in `run()`, so even a panic during start-up is recorded.
/// A release build aborts on panic: the hook runs before that, so the report is written either way.
pub fn install_panic_hook() {
    static BUSY: AtomicBool = AtomicBool::new(false);
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // A panic while writing the report must not recurse.
        if !BUSY.swap(true, Ordering::SeqCst) {
            let msg = info
                .payload()
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| info.payload().downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "panic with no message".into());
            let loc = info.location().map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()));
            let mut r = Report::new("panic", env!("CARGO_PKG_VERSION"), msg.clone(), loc.clone());
            r.thread = std::thread::current().name().map(str::to_string);
            r.backtrace = Some(std::backtrace::Backtrace::force_capture().to_string());
            log::error!("panic at {}: {msg}", loc.as_deref().unwrap_or("?"));
            log::logger().flush();
            save(r);
            BUSY.store(false, Ordering::SeqCst);
        }
        previous(info);
    }));
}

/// Start of a session (the app is up and its folders are known): remembers where reports and the log
/// are, and turns a marker left by the last session into a report.
pub fn init(app: &AppHandle) {
    let (Ok(data), Ok(logs)) = (app.path().app_data_dir(), app.path().app_log_dir()) else { return };
    let env = Env { reports: data.join("crash-reports"), log_file: logs.join("joyride.log"), started: core::now_secs() };
    let reports = env.reports.clone();
    let _ = ENV.set(env);
    let version = env!("CARGO_PKG_VERSION");
    if let Some(prev) = core::begin_session(&data, version) {
        let mut r = Report::new(
            "unclean-exit",
            &prev.version,
            "the last session ended without closing (a crash outside Rust, a force quit, or a power cut)".into(),
            None,
        );
        r.session_secs = Some(core::now_secs().saturating_sub(prev.started));
        // The log still holds that session's last lines: it is only rotated when it grows.
        r.log_tail = core::log_tail(&ENV.get().unwrap().log_file, LOG_LINES, home().as_deref());
        let r = fill_keep_session(r);
        if let Ok(p) = core::write_report(&reports, &r) {
            log::warn!("the last session did not exit cleanly; report {}", p.file_name().unwrap_or_default().to_string_lossy());
            let name = p.file_name().unwrap_or_default().to_string_lossy().into_owned();
            *PREVIOUS.lock().unwrap() = core::list_reports(&reports).into_iter().find(|i| i.file == name);
        }
    }
    log::info!("JoyrIDE {version} started on {} {}", std::env::consts::OS, std::env::consts::ARCH);
}

/// `fill`, but the session length and log of the report are already the dead session's own.
fn fill_keep_session(mut r: Report) -> Report {
    let (secs, tail) = (r.session_secs, std::mem::take(&mut r.log_tail));
    r = fill(r);
    r.session_secs = secs;
    r.log_tail = tail;
    r
}

/// A clean exit: the marker goes, so the next start knows this one ended well.
pub fn finish(app: &AppHandle) {
    log::info!("JoyrIDE exiting");
    log::logger().flush();
    if let Ok(data) = app.path().app_data_dir() {
        core::end_session(&data);
    }
}

fn drafts(app: &AppHandle, project: &Project) -> Result<(Drafts, PathBuf), String> {
    let root = project.root()?;
    let data = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok((Drafts::for_project(&data, &root), root))
}

/// Keeps an unsaved edit as a draft (the project's own files are not touched).
#[tauri::command]
pub async fn draft_save(app: AppHandle, state: State<'_, Project>, path: String, text: String) -> Result<(), String> {
    let (d, _) = drafts(&app, &state)?;
    d.save(&path, &text).map_err(|e| e.to_string())
}

/// The file was saved, or its edit thrown away: no draft is needed any more.
#[tauri::command]
pub async fn draft_discard(app: AppHandle, state: State<'_, Project>, path: String) -> Result<(), String> {
    drafts(&app, &state)?.0.discard(&path);
    Ok(())
}

/// Drafts of the open project that still differ from disk.
#[tauri::command]
pub async fn drafts_pending(app: AppHandle, state: State<'_, Project>) -> Result<Vec<Draft>, String> {
    let (d, root) = drafts(&app, &state)?;
    Ok(d.pending(&root))
}

#[tauri::command]
pub async fn drafts_clear(app: AppHandle, state: State<'_, Project>) -> Result<(), String> {
    drafts(&app, &state)?.0.clear();
    Ok(())
}

/// The report of the last session's unclean exit, once; `None` after the window has been told.
#[tauri::command]
pub fn recovery_startup() -> Option<ReportInfo> {
    PREVIOUS.lock().unwrap().take()
}

/// An error in the window (an uncaught error or a rejected promise): always logged, and the first few
/// are also written as reports.
#[tauri::command]
pub fn report_frontend_error(message: String, stack: Option<String>, source: Option<String>) {
    let cap = |s: String| s.chars().take(4000).collect::<String>();
    let message = cap(message);
    log::error!("window error: {message}");
    if FRONTEND_REPORTS.fetch_add(1, Ordering::SeqCst) < MAX_FRONTEND_REPORTS {
        let mut r = Report::new("frontend-error", env!("CARGO_PKG_VERSION"), message, source.map(cap));
        r.backtrace = stack.map(cap);
        save(r);
    }
}

#[tauri::command]
pub fn reports_list() -> Vec<ReportInfo> {
    core::list_reports(&reports_dir())
}

#[tauri::command]
pub fn report_read(file: String) -> Result<String, String> {
    core::read_report(&reports_dir(), &file).map_err(|e| e.to_string())
}

/// Shows the folder of crash reports (`logs` = false) or of the log (`logs` = true) in the file manager.
#[tauri::command]
pub fn reveal_diagnostics(app: AppHandle, logs: bool) -> Result<(), String> {
    let dir = if logs { app.path().app_log_dir().map_err(|e| e.to_string())? } else { reports_dir() };
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    app.opener().open_path(dir.to_string_lossy(), None::<&str>).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A panic through the real hook leaves a report that parses, names the place, and has no home folder in it.
    #[test]
    fn a_panic_writes_a_report() {
        let tmp = tempfile::tempdir().unwrap();
        // Before `init`, reports go under the temp folder: point that at a folder of ours.
        std::env::set_var("TMPDIR", tmp.path());
        std::env::set_var("TEMP", tmp.path());
        std::env::set_var("HOME", "/Users/tester");
        std::env::set_var("USERPROFILE", "/Users/tester");
        install_panic_hook();
        let r = std::panic::catch_unwind(|| {
            panic!("could not open /Users/tester/game/main.lua");
        });
        assert!(r.is_err());
        let dir = std::env::temp_dir().join("joyride-crash-reports");
        let list = core::list_reports(&dir);
        assert_eq!(list.len(), 1, "{list:?}");
        let text = core::read_report(&dir, &list[0].file).unwrap();
        let rep: Report = serde_json::from_str(&text).unwrap();
        assert_eq!((rep.kind.as_str(), rep.schema), ("panic", core::SCHEMA));
        assert_eq!(rep.summary, "could not open ~/game/main.lua");
        assert!(rep.location.as_deref().unwrap().contains("recovery.rs"));
        assert!(rep.backtrace.is_some());
        assert!(!text.contains("/Users/tester"), "home folder leaked");
    }
}
