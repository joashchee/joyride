//! The open project's files, for the IDE's tree and editor: a listing, text
//! reads and writes, all confined to the project folder (symlinks that lead
//! out are refused, so a hostile project cannot make the IDE read or write
//! elsewhere).

use crate::{err, Result};
use serde::Serialize;
use std::fs;
use std::path::{Component, Path, PathBuf};

/// Largest text file the editor opens.
pub const MAX_TEXT_BYTES: u64 = 4 * 1024 * 1024;
/// Most entries one listing returns; the rest is reported as `truncated`.
pub const MAX_ENTRIES: usize = 5000;

const HIDDEN_DIRS: &[&str] = &[".git", "node_modules", ".DS_Store"];

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    /// Path relative to the project, with `/` separators.
    pub path: String,
    pub name: String,
    pub dir: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Listing {
    pub name: String,
    pub entries: Vec<Entry>,
    pub truncated: bool,
    /// `main.lua` is at the root, so the folder is a LÖVE project.
    pub has_main: bool,
}

/// Resolves `rel` inside `root`; it may name a file that does not exist yet,
/// but its nearest existing parent must be inside the project.
pub fn resolve(root: &Path, rel: &str) -> Result<PathBuf> {
    let p = Path::new(rel);
    if p.is_absolute() || p.components().any(|c| matches!(c, Component::ParentDir | Component::Prefix(_))) {
        return err(format!("`{rel}` leaves the project"));
    }
    let root = root.canonicalize()?;
    let joined = root.join(p);
    let mut probe = joined.as_path();
    while !probe.exists() {
        probe = match probe.parent() {
            Some(x) => x,
            None => return err(format!("`{rel}` leaves the project")),
        };
    }
    if !probe.canonicalize()?.starts_with(&root) {
        return err(format!("`{rel}` leaves the project"));
    }
    Ok(joined)
}

/// Lists the project depth-first, folders before files, each sorted by name.
pub fn list(root: &Path) -> Result<Listing> {
    let root = root.canonicalize()?;
    if !root.is_dir() {
        return err("not a folder");
    }
    let mut entries = Vec::new();
    let mut truncated = false;
    walk(&root, &root, &mut entries, &mut truncated)?;
    Ok(Listing {
        name: root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        has_main: root.join("main.lua").is_file(),
        entries,
        truncated,
    })
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<Entry>, truncated: &mut bool) -> Result<()> {
    let mut items: Vec<_> = fs::read_dir(dir)?.filter_map(|e| e.ok()).collect();
    items.sort_by_key(|e| {
        let is_dir = e.path().is_dir();
        (!is_dir, e.file_name().to_string_lossy().to_lowercase())
    });
    for e in items {
        let name = e.file_name().to_string_lossy().into_owned();
        if HIDDEN_DIRS.contains(&name.as_str()) {
            continue;
        }
        if out.len() >= MAX_ENTRIES {
            *truncated = true;
            return Ok(());
        }
        let path = e.path();
        // A symlink is listed but never followed, so the tree cannot leave.
        let is_link = e.file_type().map(|t| t.is_symlink()).unwrap_or(false);
        let dir_entry = !is_link && path.is_dir();
        let rel = path.strip_prefix(root).unwrap().components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/");
        out.push(Entry { path: rel, name, dir: dir_entry });
        if dir_entry {
            walk(root, &path, out, truncated)?;
            if *truncated {
                return Ok(());
            }
        }
    }
    Ok(())
}

/// Reads a UTF-8 text file; binary and oversized files are refused.
pub fn read_text(root: &Path, rel: &str) -> Result<String> {
    let p = resolve(root, rel)?;
    if !p.canonicalize()?.starts_with(root.canonicalize()?) {
        return err(format!("`{rel}` leaves the project"));
    }
    let meta = fs::metadata(&p)?;
    if !meta.is_file() {
        return err(format!("`{rel}` is not a file"));
    }
    if meta.len() > MAX_TEXT_BYTES {
        return err(format!("`{rel}` is too large to edit ({} MB)", meta.len() / (1024 * 1024)));
    }
    String::from_utf8(fs::read(&p)?).or_else(|_| err(format!("`{rel}` is not a text file")))
}

/// Writes a text file atomically (temporary file, then rename), creating
/// folders as needed.
pub fn write_text(root: &Path, rel: &str, text: &str) -> Result<()> {
    let p = resolve(root, rel)?;
    if p.is_dir() {
        return err(format!("`{rel}` is a folder"));
    }
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent)?;
        if !parent.canonicalize()?.starts_with(root.canonicalize()?) {
            return err(format!("`{rel}` leaves the project"));
        }
    }
    let tmp = p.with_extension(format!("{}.jr-tmp", p.extension().and_then(|e| e.to_str()).unwrap_or("")));
    fs::write(&tmp, text)?;
    fs::rename(&tmp, &p).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir_all(d.path().join("src/sub")).unwrap();
        fs::create_dir_all(d.path().join(".git")).unwrap();
        fs::write(d.path().join("main.lua"), "print(1)").unwrap();
        fs::write(d.path().join("src/a.lua"), "x").unwrap();
        fs::write(d.path().join("src/sub/b.lua"), "y").unwrap();
        d
    }

    #[test]
    fn lists_folders_first_and_hides_git() {
        let d = project();
        let l = list(d.path()).unwrap();
        let paths: Vec<_> = l.entries.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, ["src", "src/sub", "src/sub/b.lua", "src/a.lua", "main.lua"]);
        assert!(l.has_main && !l.truncated);
    }

    #[test]
    fn round_trips_and_creates_folders() {
        let d = project();
        write_text(d.path(), "new/dir/c.lua", "z = 1\n").unwrap();
        assert_eq!(read_text(d.path(), "new/dir/c.lua").unwrap(), "z = 1\n");
        write_text(d.path(), "main.lua", "print(2)").unwrap();
        assert_eq!(read_text(d.path(), "main.lua").unwrap(), "print(2)");
        assert!(!d.path().join("main.jr-tmp").exists());
    }

    #[test]
    fn refuses_paths_that_leave() {
        let d = project();
        assert!(read_text(d.path(), "../x").is_err());
        assert!(read_text(d.path(), "/etc/passwd").is_err());
        assert!(write_text(d.path(), "src/../../x.lua", "").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn refuses_symlinks_out() {
        let d = project();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("secret.txt"), "s").unwrap();
        std::os::unix::fs::symlink(outside.path(), d.path().join("link")).unwrap();
        assert!(read_text(d.path(), "link/secret.txt").is_err());
        assert!(write_text(d.path(), "link/new.txt", "x").is_err());
        assert!(!outside.path().join("new.txt").exists());
        let l = list(d.path()).unwrap();
        assert!(l.entries.iter().any(|e| e.path == "link" && !e.dir));
        assert!(!l.entries.iter().any(|e| e.path.starts_with("link/")));
    }

    #[test]
    fn refuses_binary_and_big() {
        let d = project();
        fs::write(d.path().join("img.png"), [0x89, 0x50, 0xff, 0xfe]).unwrap();
        assert!(read_text(d.path(), "img.png").is_err());
        assert!(read_text(d.path(), "src").is_err());
    }
}
