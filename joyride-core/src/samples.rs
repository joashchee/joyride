//! The sample projects that ship with JoyrIDE (`samples/<id>/` in the repository, `resources/samples/<id>/` in the app).
//!
//! A sample is a whole project (`love.toml`, `main.lua`, assets, a `.joyride/assets.toml` the Asset Library wrote). The app never
//! runs one in place: it **copies** it to a folder the person owns, so Run, the Asset Library and Auto-Port write there and the shipped
//! files stay as they were. A copy that is already there is opened as it is (the person's edits are kept); a fresh copy moves the old
//! one aside under a new name instead of deleting it.

use crate::{err, Result};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Sample {
    pub id: String,
    pub name: String,
    /// The first sentence of the README.
    pub summary: String,
}

/// A sample id is a folder name: lower-case letters, digits and hyphens only, so it cannot name a path.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 40 && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-') && !id.starts_with('-')
}

/// The first prose line of a README, cut at its first full stop.
fn summary_of(readme: &str) -> String {
    let line = readme.lines().map(str::trim).find(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with('|') && !l.starts_with("```")).unwrap_or("");
    match line.find(". ") {
        Some(i) => line[..=i].to_string(),
        None => line.to_string(),
    }
}

fn project_name(dir: &Path, fallback: &str) -> String {
    std::fs::read_to_string(dir.join("love.toml"))
        .ok()
        .and_then(|t| crate::manifest::Manifest::parse(&t).ok())
        .map(|m| m.project.name)
        .unwrap_or_else(|| fallback.to_string())
}

/// Every sample under `root` (one folder each, with a `love.toml` and a `main.lua`), by id.
pub fn list(root: &Path) -> Vec<Sample> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(root) else { return out };
    for e in rd.flatten() {
        let p = e.path();
        let Some(id) = p.file_name().and_then(|n| n.to_str()).map(str::to_string) else { continue };
        if !valid_id(&id) || !p.join("main.lua").is_file() || !p.join("love.toml").is_file() {
            continue;
        }
        let summary = std::fs::read_to_string(p.join("README.md")).map(|r| summary_of(&r)).unwrap_or_default();
        out.push(Sample { name: project_name(&p, &id), id, summary });
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

const SKIP: &[&str] = &[".DS_Store", ".scratch", ".git", ".claude"];

fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let name = e.file_name();
        if SKIP.iter().any(|s| name == *s) {
            continue;
        }
        let (src, dst) = (e.path(), to.join(&name));
        if e.file_type()?.is_dir() {
            copy_tree(&src, &dst)?;
        } else {
            std::fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

/// Puts a copy of sample `id` in `dest_parent/<id>` and returns that folder. If it is already there it is returned untouched, unless
/// `fresh`, which moves it aside to `<id>-old-<n>` first (nothing is deleted).
pub fn copy_to(root: &Path, id: &str, dest_parent: &Path, fresh: bool) -> Result<PathBuf> {
    if !valid_id(id) {
        return err(format!("`{id}` is not a sample name"));
    }
    let src = root.join(id);
    if !src.join("main.lua").is_file() {
        return err(format!("there is no sample called {id}"));
    }
    let dest = dest_parent.join(id);
    if dest.exists() {
        if !fresh {
            return Ok(dest);
        }
        let mut n = 1;
        let aside = loop {
            let p = dest_parent.join(format!("{id}-old-{n}"));
            if !p.exists() {
                break p;
            }
            n += 1;
        };
        std::fs::rename(&dest, &aside)?;
    }
    std::fs::create_dir_all(dest_parent)?;
    if let Err(e) = copy_tree(&src, &dest) {
        let _ = std::fs::remove_dir_all(&dest); // a half copy is worse than none
        return err(format!("could not copy the sample: {e}"));
    }
    Ok(dest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake(root: &Path, id: &str) {
        let d = root.join(id);
        std::fs::create_dir_all(d.join("assets")).unwrap();
        std::fs::write(d.join("main.lua"), "-- main\n").unwrap();
        std::fs::write(d.join("love.toml"), "[project]\nname = \"Fake Sample\"\nbundle_id = \"dev.x.fake\"\nversion = \"1.0.0\"\n").unwrap();
        std::fs::write(d.join("README.md"), "# Fake\n\nChecks things. Then more words.\n").unwrap();
        std::fs::write(d.join("assets/a.txt"), "a").unwrap();
        std::fs::write(d.join(".DS_Store"), "junk").unwrap();
    }

    #[test]
    fn ids_cannot_name_a_path() {
        assert!(valid_id("io-checker") && valid_id("a1"));
        for bad in ["", "../x", "a/b", "A", "-x", "a b", "x.y"] {
            assert!(!valid_id(bad), "{bad}");
        }
    }

    #[test]
    fn lists_only_whole_projects_with_their_name_and_summary() {
        let d = tempfile::tempdir().unwrap();
        fake(d.path(), "fake");
        std::fs::create_dir_all(d.path().join("not-a-project")).unwrap();
        std::fs::create_dir_all(d.path().join("Bad Name")).unwrap();
        let l = list(d.path());
        assert_eq!(l, vec![Sample { id: "fake".into(), name: "Fake Sample".into(), summary: "Checks things.".into() }]);
    }

    #[test]
    fn a_copy_is_kept_and_a_fresh_one_moves_the_old_aside() {
        let d = tempfile::tempdir().unwrap();
        let (root, dest) = (d.path().join("shipped"), d.path().join("mine"));
        fake(&root, "fake");
        let p = copy_to(&root, "fake", &dest, false).unwrap();
        assert!(p.join("assets/a.txt").is_file() && !p.join(".DS_Store").exists());
        std::fs::write(p.join("main.lua"), "-- edited\n").unwrap();
        // Opening again keeps the edit.
        let again = copy_to(&root, "fake", &dest, false).unwrap();
        assert_eq!(std::fs::read_to_string(again.join("main.lua")).unwrap(), "-- edited\n");
        // A fresh copy restores the original and keeps the edited one beside it.
        let fresh = copy_to(&root, "fake", &dest, true).unwrap();
        assert_eq!(std::fs::read_to_string(fresh.join("main.lua")).unwrap(), "-- main\n");
        assert_eq!(std::fs::read_to_string(dest.join("fake-old-1/main.lua")).unwrap(), "-- edited\n");
        copy_to(&root, "fake", &dest, true).unwrap();
        assert!(dest.join("fake-old-2").is_dir(), "a second fresh copy does not overwrite the first aside");
        // The shipped sample is untouched.
        assert_eq!(std::fs::read_to_string(root.join("fake/main.lua")).unwrap(), "-- main\n");
    }

    #[test]
    fn refuses_what_is_not_a_sample() {
        let d = tempfile::tempdir().unwrap();
        fake(d.path(), "fake");
        assert!(copy_to(d.path(), "../etc", d.path(), false).is_err());
        assert!(copy_to(d.path(), "nothing", d.path(), false).is_err());
    }

    /// The samples this repository really ships: each is a project JoyrIDE can open, with a valid manifest.
    #[test]
    fn the_shipped_samples_are_whole_projects() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../samples");
        let l = list(&root);
        assert!(l.iter().any(|s| s.id == "io-checker" && s.name == "IO Checker" && !s.summary.is_empty()), "{l:?}");
        for s in &l {
            let m = crate::manifest::Manifest::parse(&std::fs::read_to_string(root.join(&s.id).join("love.toml")).unwrap()).unwrap();
            assert!(m.problems().is_empty(), "{}: {:?}", s.id, m.problems());
        }
    }
}
