//! JoyrIDE's desktop shell: Tauri commands over `joyride-core` and
//! `asset-core`. The logic lives in those crates, never here, so a web build
//! can share it.

use serde::Serialize;

mod assets;
mod autoport;
mod docs;
mod export;
mod games;
mod github;
mod libs;
mod lsp;
mod project;
mod recovery;
mod fonts;
mod preview;
mod run;
mod samples;
mod soundfonts;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CoreInfo {
    version: &'static str,
}

/// The core crate's version, shown on the splash and in About.
#[tauri::command]
fn core_info() -> CoreInfo {
    CoreInfo { version: env!("CARGO_PKG_VERSION") }
}

/// The tools shipped inside the app: `Resources/resources/toolchains` in a
/// built app, the source tree's copy under `tauri dev`.
pub(crate) fn bundled_toolchains(app: &tauri::AppHandle) -> Option<std::path::PathBuf> {
    use tauri::Manager;
    let installed = app.path().resource_dir().ok().map(|d| d.join("resources/toolchains"));
    installed.filter(|p| p.is_dir()).or_else(|| source_path("resources/toolchains").filter(|p| p.is_dir()))
}

/// A path inside the source tree, for `tauri dev` (the app runs from
/// `target/debug`, with its resources still in the repository). A release
/// build has none: `env!("CARGO_MANIFEST_DIR")` would put the builder's home
/// folder in the binary, `--remap-path-prefix` does not rewrite an `env!`
/// string, and `scripts/build-release.sh` refuses an app with a home folder
/// in it. Tests that need the tree use `env!` themselves.
#[cfg(debug_assertions)]
pub(crate) fn source_path(rel: &str) -> Option<std::path::PathBuf> {
    Some(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel))
}
#[cfg(not(debug_assertions))]
pub(crate) fn source_path(_rel: &str) -> Option<std::path::PathBuf> {
    None
}

pub fn run() {
    recovery::install_panic_hook();
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(project::Project::default())
        .manage(soundfonts::SoundfontCache::default())
        .manage(run::Game::default())
        .manage(preview::VideoSession::default())
        .manage(lsp::Lsp::default())
        .plugin(
            // The error log: `joyride.log` in the OS log folder, 1 MB a file, the last three kept. Nothing leaves the machine.
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .targets([
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir { file_name: Some("joyride".into()) }),
                ])
                .max_file_size(1_000_000)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepSome(3))
                .build(),
        )
        .setup(|app| {
            recovery::init(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            core_info,
            project::open_project,
            project::last_project,
            project::project_listing,
            project::project_warnings,
            project::read_project_file,
            project::write_project_file,
            project::create_project_file,
            project::create_project_dir,
            project::rename_project_path,
            project::delete_project_path,
            project::trash_project_path,
            project::search_project,
            project::format_lua,
            project::lint_lua,
            project::color_literals,
            project::color_text,
            recovery::draft_save,
            recovery::draft_discard,
            recovery::drafts_pending,
            recovery::drafts_clear,
            recovery::recovery_startup,
            recovery::report_frontend_error,
            recovery::reports_list,
            recovery::report_read,
            recovery::reveal_diagnostics,
            export::export_info,
            export::export_project,
            autoport::autoport_scan,
            autoport::autoport_generate,
            github::github_overview,
            github::git_commit,
            github::git_init,
            github::git_push,
            github::git_pull,
            github::github_create_repo,
            github::github_release_workflow,
            github::open_github_url,
            docs::docs_search,
            docs::docs_get,
            games::games_search,
            games::games_open_url,
            libs::library_list,
            libs::library_add,
            libs::library_remove,
            libs::library_custom_remove,
            libs::integrate_sync,
            libs::integrate_marks,
            libs::integrate_clear,
            libs::libraries_detect,
            libs::libraries_offer_add,
            libs::libraries_offer_dismiss,
            assets::assets_search,
            assets::assets_import_plan,
            assets::assets_import_apply,
            assets::assets_confirm,
            assets::assets_use,
            fonts::font_inspect,
            fonts::font_add,
            preview::preview_file,
            preview::preview_video_frames,
            samples::sample_list,
            soundfonts::soundfont_list,
            soundfonts::soundfont_choose,
            soundfonts::soundfont_plan,
            soundfonts::soundfont_use,
            samples::sample_copy,
            run::run_project,
            run::stop_project,
            lsp::lsp_start,
            lsp::lsp_send,
            lsp::lsp_stop,
            lsp::lsp_settings,
            lsp::lsp_root
        ])
        .build(tauri::generate_context!())
        .expect("error while building JoyrIDE")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                recovery::finish(app);
            }
        });
}
