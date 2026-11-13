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

/// Opens a GitHub repository page in the browser. Only `https://github.com/`
/// URLs: the frontend cannot use this to open anything else.
#[tauri::command]
pub fn open_github_url(app: tauri::AppHandle, url: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let ok = url.starts_with("https://github.com/") && !url.chars().any(|c| c.is_whitespace() || c.is_control());
    if !ok {
        return Err("only github.com pages can be opened from here".into());
    }
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn only_github_urls_are_opened() {
        // The check is the same expression as in `open_github_url`.
        let ok = |u: &str| u.starts_with("https://github.com/") && !u.chars().any(|c| c.is_whitespace() || c.is_control());
        assert!(ok("https://github.com/o/r"));
        for bad in ["http://github.com/o/r", "https://github.com.evil.com/x", "file:///etc/passwd", "https://github.com/a b", "javascript:alert(1)"] {
            assert!(!ok(bad), "{bad}");
        }
    }
}
