//! Installing a manifest's dependencies into `libs/` and writing `love.lock`
//! (spec 3D). Uses the `git` command line; arguments are never taken from the
//! manifest as options (`--` before every URL and ref).

use crate::manifest::{self, Dependency, Lock, Locked, Manifest};
use crate::{err, Result};
use std::path::Path;
use std::process::Command;

/// What happened to one dependency.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Installed,
    /// Already at the locked commit.
    UpToDate,
}

/// Installs every dependency (validating the manifest first), returning the
/// new lock. A dependency already installed at the locked commit for the
/// same spec is left alone.
pub fn install(project_dir: &Path, m: &Manifest, old: &Lock) -> Result<(Lock, Vec<(String, Outcome)>)> {
    manifest::require_valid(m)?;
    let libs = project_dir.join("libs");
    std::fs::create_dir_all(&libs)?;
    let mut lock = Lock::default();
    let mut outcomes = Vec::new();
    for (name, dep) in &m.dependencies {
        let dest = libs.join(name);
        let spec = dep.spec();
        if let Some(l) = old.get(name) {
            if dest.is_dir() && l.git == dep.git && l.spec == spec {
                lock.packages.push(l.clone());
                outcomes.push((name.clone(), Outcome::UpToDate));
                continue;
            }
        }
        let commit = fetch(&dest, dep)?;
        lock.packages.push(Locked { name: name.clone(), git: dep.git.clone(), spec, commit });
        outcomes.push((name.clone(), Outcome::Installed));
    }
    std::fs::write(project_dir.join("love.lock"), lock.to_toml())?;
    Ok((lock, outcomes))
}

fn git(args: &[&str], cwd: Option<&Path>) -> Result<String> {
    let mut c = Command::new("git");
    // No remote helpers, no prompts for credentials.
    c.args(["-c", "protocol.ext.allow=never", "-c", "protocol.allow=user"]).args(args).env("GIT_TERMINAL_PROMPT", "0");
    if let Some(d) = cwd {
        c.current_dir(d);
    }
    let out = c.output().map_err(|e| crate::Error::Msg(format!("could not run git: {e}")))?;
    if !out.status.success() {
        return err(format!("git {}: {}", args.first().unwrap_or(&""), String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Clones into a fresh folder next to `dest`, checks out the ref, drops
/// `.git`, then swaps it into place; the old copy is replaced last.
fn fetch(dest: &Path, d: &Dependency) -> Result<String> {
    let tmp = dest.with_extension("joyride-tmp");
    let _ = std::fs::remove_dir_all(&tmp);
    let tmp_s = tmp.to_string_lossy().to_string();
    let result = (|| {
        git(&["clone", "--no-checkout", "--", &d.git, &tmp_s], None)?;
        if let Some(r) = d.checkout_ref() {
            git(&["checkout", "--detach", r, "--"], Some(&tmp))?;
        } else {
            git(&["checkout", "--detach", "HEAD", "--"], Some(&tmp))?;
        }
        let commit = git(&["rev-parse", "HEAD"], Some(&tmp))?;
        std::fs::remove_dir_all(tmp.join(".git"))?;
        Ok(commit)
    })();
    match result {
        Ok(commit) => {
            if dest.exists() {
                std::fs::remove_dir_all(dest)?;
            }
            std::fs::rename(&tmp, dest)?;
            Ok(commit)
        }
        Err(e) => {
            let _ = std::fs::remove_dir_all(&tmp);
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::Dependency;
    use std::collections::BTreeMap;

    fn sh(dir: &Path, args: &[&str]) {
        let s = Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"])
            .args(args)
            .current_dir(dir)
            .status()
            .unwrap();
        assert!(s.success(), "git {args:?}");
    }

    /// A tiny library repo with two commits, the first tagged v1.
    fn lib_repo(root: &Path) -> (String, String, String) {
        let r = root.join("lib");
        std::fs::create_dir_all(&r).unwrap();
        sh(&r, &["init", "-q", "-b", "main"]);
        std::fs::write(r.join("lib.lua"), "return 1").unwrap();
        sh(&r, &["add", "."]);
        sh(&r, &["commit", "-q", "-m", "one"]);
        sh(&r, &["tag", "v1"]);
        let c1 = git(&["rev-parse", "HEAD"], Some(&r)).unwrap();
        std::fs::write(r.join("lib.lua"), "return 2").unwrap();
        sh(&r, &["commit", "-q", "-am", "two"]);
        let c2 = git(&["rev-parse", "HEAD"], Some(&r)).unwrap();
        (format!("file://{}", r.display()), c1, c2)
    }

    fn manifest_with(url: &str, tag: Option<&str>) -> Manifest {
        let mut deps = BTreeMap::new();
        deps.insert("lib".to_string(), Dependency { git: url.into(), tag: tag.map(Into::into), rev: None, branch: None });
        let mut m = Manifest::new_project("g", "dev.x.g");
        m.dependencies = deps;
        m
    }

    #[test]
    fn installs_at_the_tag_locks_and_skips_when_unchanged() {
        let d = tempfile::tempdir().unwrap();
        let (url, c1, c2) = lib_repo(d.path());
        let proj = d.path().join("game");
        std::fs::create_dir_all(&proj).unwrap();
        let (lock, out) = install(&proj, &manifest_with(&url, Some("v1")), &Lock::default()).unwrap();
        assert_eq!(out, vec![("lib".to_string(), Outcome::Installed)]);
        assert_eq!(lock.get("lib").unwrap().commit, c1);
        assert_eq!(std::fs::read_to_string(proj.join("libs/lib/lib.lua")).unwrap(), "return 1");
        assert!(!proj.join("libs/lib/.git").exists());
        assert!(std::fs::read_to_string(proj.join("love.lock")).unwrap().contains(&c1));
        // Again, with the lock: nothing to do.
        let (_, out2) = install(&proj, &manifest_with(&url, Some("v1")), &lock).unwrap();
        assert_eq!(out2[0].1, Outcome::UpToDate);
        // Change the spec to the default branch: re-fetched at the newest commit.
        let (lock3, out3) = install(&proj, &manifest_with(&url, None), &lock).unwrap();
        assert_eq!(out3[0].1, Outcome::Installed);
        assert_eq!(lock3.get("lib").unwrap().commit, c2);
        assert_eq!(std::fs::read_to_string(proj.join("libs/lib/lib.lua")).unwrap(), "return 2");
    }

    #[test]
    fn a_bad_ref_fails_and_leaves_nothing_half_installed() {
        let d = tempfile::tempdir().unwrap();
        let (url, _, _) = lib_repo(d.path());
        let proj = d.path().join("game");
        std::fs::create_dir_all(&proj).unwrap();
        assert!(install(&proj, &manifest_with(&url, Some("no-such-tag")), &Lock::default()).is_err());
        assert!(!proj.join("libs/lib").exists());
        assert!(!proj.join("libs/lib.joyride-tmp").exists());
    }

    #[test]
    fn an_invalid_manifest_is_refused_before_any_git() {
        let d = tempfile::tempdir().unwrap();
        let m = manifest_with("--upload-pack=x", None);
        assert!(install(d.path(), &m, &Lock::default()).is_err());
        assert!(!d.path().join("libs").exists());
    }
}
