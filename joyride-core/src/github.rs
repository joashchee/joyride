//! GitHub through the `gh` command line, the way Claude Code does it: the
//! person signs in once with `gh auth login` in a terminal, and JoyrIDE only
//! runs `gh` and `git`. **JoyrIDE never sees, asks for, stores or logs a
//! token**: `gh` owns the login (its own keychain entry), and `git` gets its
//! GitHub credentials from `gh auth git-credential` for the one command that
//! needs them. Nothing here uses the network except through those two tools,
//! and only when the person clicks create, push or pull.

use crate::{err, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// `gh`: `JOYRIDE_GH`, then `PATH`, then where Homebrew and the installer put
/// it (a Mac app launched from the Dock has a short `PATH`).
pub fn find_gh() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("JOYRIDE_GH").map(PathBuf::from) {
        return p.is_file().then_some(p);
    }
    let on_path = std::env::var_os("PATH").into_iter().flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>());
    let fixed = ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"].map(PathBuf::from);
    let exe = if cfg!(windows) { "gh.exe" } else { "gh" };
    on_path.chain(fixed).map(|d| d.join(exe)).find(|p| p.is_file())
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Auth {
    /// `gh` is installed.
    pub installed: bool,
    pub logged_in: bool,
    /// The account, when `gh` says it.
    pub user: Option<String>,
    /// What to tell the person when not logged in.
    pub message: String,
}

fn gh_command(gh: &Path, dir: Option<&Path>) -> Command {
    let mut c = Command::new(gh);
    // No prompts, no colour, no update nags; a prompt would hang a command
    // that has no terminal.
    c.env("GH_PROMPT_DISABLED", "1").env("NO_COLOR", "1").env("GH_NO_UPDATE_NOTIFIER", "1").env("GH_NO_EXTENSION_UPDATE_NOTIFIER", "1").env("CLICOLOR", "0").stdin(Stdio::null());
    if let Some(d) = dir {
        c.current_dir(d);
    }
    c
}

fn run(mut c: Command) -> Result<(bool, String)> {
    let out = c.output().map_err(|e| crate::Error::Msg(format!("could not run {}: {e}", c.get_program().to_string_lossy())))?;
    let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
    let e = String::from_utf8_lossy(&out.stderr);
    if !e.trim().is_empty() {
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(&e);
    }
    Ok((out.status.success(), text.trim().to_string()))
}

/// Whether `gh` is installed and signed in, and as whom. Offline-safe: `gh
/// auth status` may check the token with GitHub, so a failure is reported as
/// "not signed in" with `gh`'s own words.
pub fn auth_status(gh: Option<&Path>) -> Auth {
    let Some(gh) = gh else {
        return Auth { installed: false, logged_in: false, user: None, message: "GitHub needs the gh command line tool. Install it (for example `brew install gh`), then run `gh auth login` in a terminal.".into() };
    };
    let mut c = gh_command(gh, None);
    c.args(["auth", "status", "--hostname", "github.com"]);
    match run(c) {
        Ok((true, text)) => Auth { installed: true, logged_in: true, user: parse_user(&text), message: String::new() },
        Ok((false, text)) => Auth { installed: true, logged_in: false, user: parse_user(&text), message: format!("Not signed in to GitHub. Run `gh auth login` in a terminal, then check again.\n{}", text.lines().take(4).collect::<Vec<_>>().join("\n")) },
        Err(e) => Auth { installed: true, logged_in: false, user: None, message: e.to_string() },
    }
}

/// "Logged in to github.com account joashchee (keyring)" → "joashchee".
fn parse_user(text: &str) -> Option<String> {
    let rest = text.split("account ").nth(1)?;
    let u = rest.split_whitespace().next()?;
    (!u.is_empty()).then(|| u.to_string())
}

/// The repository this project is linked to (`origin` on GitHub), if any.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RepoInfo {
    pub name_with_owner: String,
    pub url: String,
    pub visibility: String,
}

pub fn repo_info(gh: &Path, root: &Path) -> Option<RepoInfo> {
    let mut c = gh_command(gh, Some(root));
    c.args(["repo", "view", "--json", "nameWithOwner,url,visibility"]);
    let (ok, text) = run(c).ok()?;
    if !ok {
        return None;
    }
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    Some(RepoInfo { name_with_owner: v["nameWithOwner"].as_str()?.to_string(), url: v["url"].as_str()?.to_string(), visibility: v["visibility"].as_str().unwrap_or("").to_lowercase() })
}

/// A GitHub repository name: letters, digits, `.`, `-`, `_`; not `.`/`..`;
/// never starting with `-` (it would read as an option).
pub fn valid_repo_name(n: &str) -> bool {
    !n.is_empty() && n.len() <= 100 && n != "." && n != ".." && !n.starts_with('-') && n.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

/// `.gitignore` for a LÖVE project: builds, `.love` archives, macOS litter.
pub const GITIGNORE: &str = "# JoyrIDE\nbuild/\n*.love\n.DS_Store\n.joyride-build-*/\n";

/// The workflow that builds `<slug>.love` and attaches it to the GitHub
/// release for a pushed `v*` tag. No third-party actions: checkout and the
/// runner's own `gh` and `zip`, with the repository's built-in token.
pub fn release_workflow(slug: &str) -> Result<String> {
    if slug.is_empty() || !slug.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_')) {
        return err(format!("`{slug}` is not a safe file name for a release"));
    }
    Ok(format!(
        r#"# Written by JoyrIDE. Push a tag like v1.0.0 to build {slug}.love and attach it to a release.
name: release
on:
  push:
    tags: ["v*"]
permissions:
  contents: write
jobs:
  love:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Build {slug}.love
        run: zip -9 -r "{slug}.love" . -x ".git/*" ".github/*" ".joyride/*" "build/*" "*.love" ".DS_Store"
      - name: Publish the release
        env:
          GH_TOKEN: ${{{{ github.token }}}}
        run: gh release create "$GITHUB_REF_NAME" "{slug}.love" --generate-notes --title "$GITHUB_REF_NAME"
"#
    ))
}

/// A project's git repository, and the GitHub login `gh` provides for it.
pub struct Repo {
    pub root: PathBuf,
    pub gh: Option<PathBuf>,
    /// Extra environment for `git` (tests set the author here).
    pub env: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Change {
    /// Two letters as `git status` shows them (`M `, ` M`, `??`, `A `...).
    pub status: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// The project is a git repository.
    pub is_repo: bool,
    pub branch: String,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub changes: Vec<Change>,
}

impl Repo {
    fn git(&self, args: &[&str]) -> Command {
        let mut c = Command::new("git");
        // No remote helpers, no prompts for credentials.
        c.args(["-c", "protocol.ext.allow=never"]).args(args).current_dir(&self.root).env("GIT_TERMINAL_PROMPT", "0").stdin(Stdio::null());
        for (k, v) in &self.env {
            c.env(k, v);
        }
        c
    }

    /// `git` for a command that talks to GitHub: its credentials come from
    /// `gh auth git-credential`, for github.com only, and nothing is stored.
    fn git_online(&self, args: &[&str]) -> Result<Command> {
        let Some(gh) = &self.gh else { return err("GitHub needs the gh command line tool") };
        let helper = format!("!{} auth git-credential", shell_quote(&gh.to_string_lossy()));
        let mut c = Command::new("git");
        c.args(["-c", "protocol.ext.allow=never", "-c", "credential.https://github.com.helper=", "-c"])
            .arg(format!("credential.https://github.com.helper={helper}"))
            .args(args)
            .current_dir(&self.root)
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(Stdio::null());
        for (k, v) in &self.env {
            c.env(k, v);
        }
        Ok(c)
    }

    fn check(&self, c: Command, what: &str) -> Result<String> {
        let (ok, text) = run(c)?;
        if ok {
            Ok(text)
        } else {
            err(format!("{what}: {text}"))
        }
    }

    pub fn status(&self) -> Result<Status> {
        let inside = run(self.git(&["rev-parse", "--is-inside-work-tree"]))?;
        if !inside.0 {
            return Ok(Status::default());
        }
        // The repository's top must be the project folder: a project inside
        // some other repository is not "a repository" of its own.
        let top = run(self.git(&["rev-parse", "--show-toplevel"]))?.1;
        if Path::new(&top).canonicalize().ok() != self.root.canonicalize().ok() {
            return Ok(Status::default());
        }
        let out = self.git(&["status", "--porcelain=v1", "-b", "-z", "--untracked-files=all"]).output()?;
        if !out.status.success() {
            return err(format!("git status: {}", String::from_utf8_lossy(&out.stderr).trim()));
        }
        Ok(parse_status(&String::from_utf8_lossy(&out.stdout)))
    }

    /// `git init` (branch `main`) and a `.gitignore` if the project has none.
    pub fn init(&self) -> Result<()> {
        self.check(self.git(&["init", "-q", "-b", "main"]), "git init")?;
        let gi = self.root.join(".gitignore");
        if !gi.exists() {
            std::fs::write(gi, GITIGNORE)?;
        }
        Ok(())
    }

    /// Stages everything and commits it. An empty message or nothing to
    /// commit is an error saying so.
    pub fn commit_all(&self, message: &str) -> Result<()> {
        let m = message.trim();
        if m.is_empty() {
            return err("write a commit message first");
        }
        self.check(self.git(&["add", "-A"]), "git add")?;
        let (ok, text) = run(self.git(&["commit", "-q", "-m", m]))?;
        if !ok {
            return err(if text.contains("nothing to commit") || text.contains("nothing added") { "there is nothing to commit".to_string() } else { format!("git commit: {text}") });
        }
        Ok(())
    }

    /// Pushes the current branch (setting its upstream the first time).
    pub fn push(&self) -> Result<String> {
        let has_upstream = self.status()?.upstream.is_some();
        let args: &[&str] = if has_upstream { &["push"] } else { &["push", "-u", "origin", "HEAD"] };
        let c = self.git_online(args)?;
        self.check(c, "git push")
    }

    /// Fast-forward pull only: it never makes a merge commit behind the person's back.
    pub fn pull(&self) -> Result<String> {
        let c = self.git_online(&["pull", "--ff-only"])?;
        self.check(c, "git pull")
    }

    /// Creates the GitHub repository for this project and links it as `origin`.
    pub fn create_github_repo(&self, name: &str, private: bool) -> Result<()> {
        if !valid_repo_name(name) {
            return err(format!("`{name}` is not a valid repository name"));
        }
        let Some(gh) = &self.gh else { return err("GitHub needs the gh command line tool") };
        if !self.status()?.is_repo {
            self.init()?;
        }
        let mut c = gh_command(gh, Some(&self.root));
        c.args(["repo", "create", name, if private { "--private" } else { "--public" }, "--source=.", "--remote=origin"]);
        self.check(c, "gh repo create")?;
        Ok(())
    }

    /// Writes `.github/workflows/release.yml` for the project (never over an
    /// existing one).
    pub fn write_release_workflow(&self, slug: &str) -> Result<PathBuf> {
        let p = self.root.join(".github/workflows/release.yml");
        if p.exists() {
            return err(".github/workflows/release.yml already exists");
        }
        std::fs::create_dir_all(p.parent().unwrap())?;
        std::fs::write(&p, release_workflow(slug)?)?;
        Ok(p)
    }
}

/// POSIX single-quoting, for the credential helper's path.
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Parses `git status --porcelain=v1 -b -z` output.
fn parse_status(out: &str) -> Status {
    let mut s = Status { is_repo: true, ..Default::default() };
    let mut recs = out.split('\0').filter(|r| !r.is_empty());
    if let Some(head) = recs.next().and_then(|h| h.strip_prefix("## ")) {
        // "main...origin/main [ahead 1, behind 2]", "main", "No commits yet on main", "HEAD (no branch)"
        let (names, counts) = match head.split_once(" [") {
            Some((n, c)) => (n, c.trim_end_matches(']')),
            None => (head, ""),
        };
        let names = names.strip_prefix("No commits yet on ").unwrap_or(names);
        match names.split_once("...") {
            Some((b, u)) => {
                s.branch = b.to_string();
                s.upstream = Some(u.to_string());
            }
            None => s.branch = names.to_string(),
        }
        for part in counts.split(", ") {
            if let Some(n) = part.strip_prefix("ahead ") {
                s.ahead = n.parse().unwrap_or(0);
            } else if let Some(n) = part.strip_prefix("behind ") {
                s.behind = n.parse().unwrap_or(0);
            }
        }
    }
    while let Some(r) = recs.next() {
        if r.len() < 4 {
            continue;
        }
        let (xy, path) = r.split_at(2);
        if xy.contains('R') || xy.contains('C') {
            recs.next(); // the original name of a rename or copy
        }
        s.changes.push(Change { status: xy.to_string(), path: path.trim_start().to_string() });
    }
    s
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn fake_gh(dir: &Path, script: &str) -> PathBuf {
        let p = dir.join("gh");
        std::fs::write(&p, format!("#!/bin/sh\n{script}\n")).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    fn repo(dir: &Path, gh: Option<PathBuf>) -> Repo {
        let env = [("GIT_AUTHOR_NAME", "T"), ("GIT_AUTHOR_EMAIL", "t@t"), ("GIT_COMMITTER_NAME", "T"), ("GIT_COMMITTER_EMAIL", "t@t"), ("GIT_CONFIG_GLOBAL", "/dev/null"), ("GIT_CONFIG_SYSTEM", "/dev/null")];
        Repo { root: dir.to_path_buf(), gh, env: env.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect() }
    }

    #[test]
    fn no_gh_is_reported_with_what_to_do() {
        let a = auth_status(None);
        assert!(!a.installed && !a.logged_in && a.message.contains("gh auth login"));
    }

    #[test]
    fn signed_in_and_signed_out_are_told_apart() {
        let d = tempfile::tempdir().unwrap();
        let ok = fake_gh(d.path(), "echo 'github.com'; echo '  ✓ Logged in to github.com account octo (keyring)'");
        let a = auth_status(Some(&ok));
        assert!(a.installed && a.logged_in);
        assert_eq!(a.user.as_deref(), Some("octo"));
        let d2 = tempfile::tempdir().unwrap();
        let bad = fake_gh(d2.path(), "echo 'X Failed to log in to github.com account octo (keyring)'; echo 'The token in keyring is invalid.'; exit 1");
        let a = auth_status(Some(&bad));
        assert!(a.installed && !a.logged_in);
        assert!(a.message.contains("gh auth login") && a.message.contains("token in keyring is invalid"));
    }

    #[test]
    fn gh_is_never_given_a_token_or_a_prompt() {
        let d = tempfile::tempdir().unwrap();
        let g = fake_gh(d.path(), "echo \"prompt=$GH_PROMPT_DISABLED args=$*\"");
        let (_, t) = run({
            let mut c = gh_command(&g, None);
            c.args(["auth", "status"]);
            c
        })
        .unwrap();
        assert_eq!(t, "prompt=1 args=auth status");
        assert!(!t.to_lowercase().contains("token"));
    }

    #[test]
    fn repo_names_are_checked() {
        for ok in ["my-game", "game_2", "a.b", "X"] {
            assert!(valid_repo_name(ok), "{ok}");
        }
        for bad in ["", "-x", "--public", ".", "..", "a b", "a/b", "é", &"x".repeat(101)] {
            assert!(!valid_repo_name(bad), "{bad:?}");
        }
    }

    #[test]
    fn workflow_is_generated_for_a_safe_slug_only() {
        let w = release_workflow("my-game").unwrap();
        assert!(w.contains("my-game.love") && w.contains("tags: [\"v*\"]") && w.contains("github.token"));
        assert!(!w.contains("{{{") && !w.contains("}}}"), "braces were escaped wrongly:\n{w}");
        assert!(w.contains("GH_TOKEN: ${{ github.token }}"));
        for bad in ["", "a b", "a;b", "$(x)", "a/b", "\"x"] {
            assert!(release_workflow(bad).is_err(), "{bad:?}");
        }
        let d = tempfile::tempdir().unwrap();
        let r = repo(d.path(), None);
        r.write_release_workflow("g").unwrap();
        assert!(r.write_release_workflow("g").is_err(), "never over an existing one");
    }

    #[test]
    fn status_init_commit_and_parse() {
        let d = tempfile::tempdir().unwrap();
        let r = repo(d.path(), None);
        assert!(!r.status().unwrap().is_repo);
        std::fs::write(d.path().join("main.lua"), "print(1)").unwrap();
        r.init().unwrap();
        assert_eq!(std::fs::read_to_string(d.path().join(".gitignore")).unwrap(), GITIGNORE);
        let s = r.status().unwrap();
        assert!(s.is_repo && s.branch == "main" && s.upstream.is_none());
        assert!(s.changes.iter().any(|c| c.path == "main.lua" && c.status == "??"));
        assert!(r.commit_all("  ").is_err(), "empty message");
        r.commit_all("first").unwrap();
        assert!(r.status().unwrap().changes.is_empty());
        assert!(r.commit_all("again").unwrap_err().to_string().contains("nothing to commit"));
        std::fs::write(d.path().join("my file.lua"), "x").unwrap();
        std::fs::write(d.path().join("main.lua"), "print(2)").unwrap();
        let s = r.status().unwrap();
        assert!(s.changes.iter().any(|c| c.path == "my file.lua"), "{:?}", s.changes);
        assert!(s.changes.iter().any(|c| c.path == "main.lua" && c.status == " M"));
    }

    #[test]
    fn a_project_inside_another_repository_is_not_a_repository() {
        let d = tempfile::tempdir().unwrap();
        repo(d.path(), None).init().unwrap();
        let inner = d.path().join("game");
        std::fs::create_dir_all(&inner).unwrap();
        assert!(!repo(&inner, None).status().unwrap().is_repo);
    }

    #[test]
    fn status_parsing_handles_branches_counts_and_renames() {
        let s = parse_status("## main...origin/main [ahead 2, behind 1]\0 M a.lua\0R  new name.lua\0old.lua\0?? b c.lua\0");
        assert_eq!((s.branch.as_str(), s.upstream.as_deref(), s.ahead, s.behind), ("main", Some("origin/main"), 2, 1));
        assert_eq!(s.changes.iter().map(|c| c.path.as_str()).collect::<Vec<_>>(), ["a.lua", "new name.lua", "b c.lua"]);
        assert_eq!(parse_status("## No commits yet on main\0").branch, "main");
        assert_eq!(parse_status("## HEAD (no branch)\0").branch, "HEAD (no branch)");
    }

    #[test]
    fn push_and_pull_use_gh_for_credentials_and_work_against_a_remote() {
        let d = tempfile::tempdir().unwrap();
        let remote = d.path().join("remote.git");
        let ok = Command::new("git").args(["init", "-q", "--bare", "-b", "main"]).arg(&remote).status().unwrap();
        assert!(ok.success());
        let work = d.path().join("work");
        std::fs::create_dir_all(&work).unwrap();
        std::fs::write(work.join("main.lua"), "1").unwrap();
        let gh = fake_gh(d.path(), "exit 0");
        let r = repo(&work, Some(gh.clone()));
        r.init().unwrap();
        r.commit_all("one").unwrap();
        r.check(r.git(&["remote", "add", "origin", &remote.to_string_lossy()]), "remote").unwrap();
        r.push().unwrap(); // first push sets the upstream
        let s = r.status().unwrap();
        assert_eq!(s.upstream.as_deref(), Some("origin/main"));
        // A second clone pushes a change; pull brings it in.
        let other = d.path().join("other");
        let mut clone = Command::new("git");
        clone.args(["clone", "-q"]).arg(&remote).arg(&other);
        r.check(clone, "clone").unwrap();
        std::fs::write(other.join("two.lua"), "2").unwrap();
        let o = repo(&other, Some(gh));
        o.commit_all("two").unwrap();
        o.push().unwrap();
        r.pull().unwrap();
        assert!(work.join("two.lua").is_file());
        // The helper is wired in, and quoted.
        let c = r.git_online(&["push"]).unwrap();
        let args: Vec<String> = c.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
        assert!(args.iter().any(|a| a.starts_with("credential.https://github.com.helper=!'") && a.ends_with("' auth git-credential")), "{args:?}");
        assert!(r.git_online(&["push"]).is_ok() && repo(&work, None).git_online(&["push"]).is_err());
    }

    #[test]
    fn creating_a_repo_runs_gh_with_checked_arguments() {
        let d = tempfile::tempdir().unwrap();
        let log = d.path().join("log");
        let gh = fake_gh(d.path(), &format!("echo \"$*\" >> {}", log.display()));
        let work = d.path().join("w");
        std::fs::create_dir_all(&work).unwrap();
        let r = repo(&work, Some(gh));
        assert!(r.create_github_repo("--public", false).is_err());
        assert!(r.create_github_repo("a b", false).is_err());
        r.create_github_repo("my-game", true).unwrap();
        assert!(work.join(".git").is_dir(), "initialised first");
        assert_eq!(std::fs::read_to_string(&log).unwrap().trim(), "repo create my-game --private --source=. --remote=origin");
    }

    #[test]
    fn repo_info_reads_gh_json_and_a_failure_means_none() {
        let d = tempfile::tempdir().unwrap();
        let ok = fake_gh(d.path(), "echo '{\"nameWithOwner\":\"o/r\",\"url\":\"https://github.com/o/r\",\"visibility\":\"PUBLIC\"}'");
        let i = repo_info(&ok, d.path()).unwrap();
        assert_eq!((i.name_with_owner.as_str(), i.visibility.as_str()), ("o/r", "public"));
        let d2 = tempfile::tempdir().unwrap();
        let bad = fake_gh(d2.path(), "echo 'no git remotes found' >&2; exit 1");
        assert!(repo_info(&bad, d2.path()).is_none());
    }
}
