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
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/toolchains");
    installed.filter(|p| p.is_dir()).or_else(|| source.is_dir().then_some(source))
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(project::Project::default())
        .manage(soundfonts::SoundfontCache::default())
        .manage(run::Game::default())
        .manage(preview::VideoSession::default())
        .manage(lsp::Lsp::default())
        .plugin(tauri_plugin_log::Builder::new().level(log::LevelFilter::Info).build())
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
            project::search_project,
            project::format_lua,
            project::lint_lua,
            project::color_literals,
            project::color_text,
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
            samples::sample_copy,
            run::run_project,
            run::stop_project,
            lsp::lsp_start,
            lsp::lsp_send,
            lsp::lsp_stop,
            lsp::lsp_settings,
            lsp::lsp_root
        ])
        .run(tauri::generate_context!())
        .expect("error while running JoyrIDE");
}
