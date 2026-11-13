//! The GitHub panel's commands over `joyride_core::github`: `gh` and `git`
//! only, never a token of ours. Everything that can touch the network runs
//! off the UI thread and only when the person clicks.

use crate::project::Project;
use joyride_core::github::{self, Auth, Repo, RepoInfo, Status};
use serde::Serialize;
use tauri::State;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    auth: Auth,
    /// The GitHub repository this folder is linked to, if any.
    repo: Option<RepoInfo>,
    git: Status,
    /// A suggested name for a new repository.
    suggested_name: String,
}

fn repo_for(root: &std::path::Path) -> Repo {
    Repo { root: root.to_path_buf(), gh: github::find_gh(), env: vec![] }
}

fn overview(root: &std::path::Path) -> Result<Overview, String> {
    let r = repo_for(root);
    let auth = github::auth_status(r.gh.as_deref());
    let git = r.status().map_err(|e| e.to_string())?;
    // `gh repo view` asks GitHub, so only when signed in and the folder is a repository.
    let repo = match (&r.gh, auth.logged_in && git.is_repo) {
        (Some(gh), true) => github::repo_info(gh, root),
        _ => None,
    };
    let name = joyride_core::pipeline::manifest_for(root).map(|m| joyride_core::export::slug(&m.project.name)).unwrap_or_else(|_| "game".into());
    Ok(Overview { auth, repo, git, suggested_name: name })
}

macro_rules! blocking {
    ($project:expr, |$root:ident| $body:expr) => {{
        let $root = $project.root()?;
        tauri::async_runtime::spawn_blocking(move || $body).await.map_err(|e| e.to_string())?
    }};
}

#[tauri::command]
pub async fn github_overview(project: State<'_, Project>) -> Result<Overview, String> {
    blocking!(project, |root| overview(&root))
}

#[tauri::command]
pub async fn git_commit(project: State<'_, Project>, message: String) -> Result<Overview, String> {
    blocking!(project, |root| {
        repo_for(&root).commit_all(&message).map_err(|e| e.to_string())?;
        overview(&root)
    })
}

#[tauri::command]
pub async fn git_init(project: State<'_, Project>) -> Result<Overview, String> {
    blocking!(project, |root| {
        repo_for(&root).init().map_err(|e| e.to_string())?;
        overview(&root)
    })
}

#[tauri::command]
pub async fn git_push(project: State<'_, Project>) -> Result<Overview, String> {
    blocking!(project, |root| {
        repo_for(&root).push().map_err(|e| e.to_string())?;
        overview(&root)
    })
}

#[tauri::command]
pub async fn git_pull(project: State<'_, Project>) -> Result<Overview, String> {
    blocking!(project, |root| {
        repo_for(&root).pull().map_err(|e| e.to_string())?;
        overview(&root)
    })
}

#[tauri::command]
pub async fn github_create_repo(project: State<'_, Project>, name: String, private: bool) -> Result<Overview, String> {
    blocking!(project, |root| {
        repo_for(&root).create_github_repo(&name, private).map_err(|e| e.to_string())?;
        overview(&root)
    })
}

#[tauri::command]
pub async fn github_release_workflow(project: State<'_, Project>) -> Result<String, String> {
    blocking!(project, |root| {
        let m = joyride_core::pipeline::manifest_for(&root).map_err(|e| e.to_string())?;
        let p = repo_for(&root).write_release_workflow(&joyride_core::export::slug(&m.project.name)).map_err(|e| e.to_string())?;
        Ok(p.strip_prefix(&root).unwrap_or(&p).to_string_lossy().into_owned())
    })
}
