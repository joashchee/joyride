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

/// A path for a change (create, rename, delete): inside the project, not the
/// project folder itself, and with no empty or `.`-only parts.
fn target(root: &Path, rel: &str) -> Result<PathBuf> {
    if rel.trim().is_empty() || Path::new(rel).components().any(|c| matches!(c, Component::CurDir)) {
        return err(format!("`{rel}` is not a path in the project"));
    }
    if rel.split('/').any(|part| part.trim().is_empty() || part.contains('\\') || part.contains('\0')) {
        return err(format!("`{rel}` is not a valid name"));
    }
    resolve(root, rel)
}

/// Creates an empty file (and its folders); refuses to replace anything.
pub fn create_file(root: &Path, rel: &str) -> Result<()> {
    let p = target(root, rel)?;
    if p.exists() || p.symlink_metadata().is_ok() {
        return err(format!("`{rel}` already exists"));
    }
    write_text(root, rel, "")
}

/// Creates a folder (and its parents); refuses if the name is taken by a file.
pub fn create_dir(root: &Path, rel: &str) -> Result<()> {
    let p = target(root, rel)?;
    if p.is_file() {
        return err(format!("`{rel}` is a file"));
    }
    fs::create_dir_all(&p)?;
    if !p.canonicalize()?.starts_with(root.canonicalize()?) {
        let _ = fs::remove_dir(&p);
        return err(format!("`{rel}` leaves the project"));
    }
    Ok(())
}

/// Moves or renames a file or folder inside the project; refuses to replace
/// anything and to move a folder into itself.
pub fn rename(root: &Path, from: &str, to: &str) -> Result<()> {
    let (a, b) = (target(root, from)?, target(root, to)?);
    if !a.exists() {
        return err(format!("`{from}` does not exist"));
    }
    if b.symlink_metadata().is_ok() {
        return err(format!("`{to}` already exists"));
    }
    if b.starts_with(&a) {
        return err(format!("`{from}` cannot move into itself"));
    }
    if let Some(parent) = b.parent() {
        fs::create_dir_all(parent)?;
        if !parent.canonicalize()?.starts_with(root.canonicalize()?) {
            return err(format!("`{to}` leaves the project"));
        }
    }
    fs::rename(&a, &b)?;
    Ok(())
}

/// Deletes a file or a folder with everything in it. This is permanent (the
/// UI asks first); the project folder itself is never deleted.
pub fn delete(root: &Path, rel: &str) -> Result<()> {
    let p = target(root, rel)?;
    let meta = fs::symlink_metadata(&p).map_err(|_| crate::Error::Msg(format!("`{rel}` does not exist")))?;
    if meta.is_dir() {
        fs::remove_dir_all(&p)?;
    } else {
        fs::remove_file(&p)?;
    }
    Ok(())
}

/// Moves a file or folder of the project into `trash_dir` (the person's Trash), under a name that
/// is not taken there, and returns where it went. Nothing is copied or deleted: it is a rename, so
/// it fails (and leaves the file where it was) when the Trash is on another volume.
pub fn trash_into(root: &Path, rel: &str, trash_dir: &Path) -> Result<PathBuf> {
    let p = target(root, rel)?;
    fs::symlink_metadata(&p).map_err(|_| crate::Error::Msg(format!("`{rel}` does not exist")))?;
    fs::create_dir_all(trash_dir)?;
    let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("item");
    let (stem, ext) = match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i..]),
        _ => (name, ""),
    };
    let mut dest = trash_dir.join(name);
    let mut n = 2;
    while fs::symlink_metadata(&dest).is_ok() {
        dest = trash_dir.join(format!("{stem} {n}{ext}"));
        n += 1;
    }
    fs::rename(&p, &dest)?;
    Ok(dest)
}

/// Moves a project path to the Trash where there is one JoyrIDE can use (macOS: `~/.Trash`).
/// `Ok(None)` means there is none here, and nothing was changed: the caller decides whether to delete for good.
pub fn trash(root: &Path, rel: &str) -> Result<Option<PathBuf>> {
    #[cfg(target_os = "macos")]
    {
        if let Some(home) = std::env::var_os("HOME") {
            return trash_into(root, rel, &Path::new(&home).join(".Trash")).map(Some);
        }
    }
    let _ = (root, rel);
    Ok(None)
}

/// One match of a text search.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Hit {
    pub path: String,
    /// 1-based line.
    pub line: u32,
    /// The line, trimmed and cut to a readable length.
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Found {
    pub hits: Vec<Hit>,
    /// More matches exist than `hits` holds.
    pub truncated: bool,
}

/// Most hits one search returns.
pub const MAX_HITS: usize = 500;

/// Finds `query` as plain text (not a pattern) in the project's text files,
/// in tree order. Files over [`MAX_TEXT_BYTES`], non-UTF-8 files and
/// symlinks are skipped; hidden folders are not searched.
pub fn search(root: &Path, query: &str, case_sensitive: bool) -> Result<Found> {
    if query.is_empty() {
        return Ok(Found { hits: vec![], truncated: false });
    }
    let root = root.canonicalize()?;
    let listing = list(&root)?;
    let needle = if case_sensitive { query.to_string() } else { query.to_lowercase() };
    let mut hits = Vec::new();
    for e in listing.entries.iter().filter(|e| !e.dir) {
        let p = root.join(&e.path);
        let Ok(meta) = fs::symlink_metadata(&p) else { continue };
        if !meta.is_file() || meta.len() > MAX_TEXT_BYTES {
            continue;
        }
        let Ok(bytes) = fs::read(&p) else { continue };
        let Ok(text) = String::from_utf8(bytes) else { continue };
        for (i, line) in text.lines().enumerate() {
            let found = if case_sensitive { line.contains(&needle) } else { line.to_lowercase().contains(&needle) };
            if !found {
                continue;
            }
            if hits.len() >= MAX_HITS {
                return Ok(Found { hits, truncated: true });
            }
            let t = line.trim();
            let t: String = if t.chars().count() > 200 { t.chars().take(200).collect::<String>() + "…" } else { t.to_string() };
            hits.push(Hit { path: e.path.clone(), line: i as u32 + 1, text: t });
        }
    }
    Ok(Found { hits, truncated: false })
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
    fn trashing_moves_the_path_and_never_replaces_what_is_in_the_trash() {
        let d = project();
        let t = tempfile::tempdir().unwrap();
        fs::write(t.path().join("a.lua"), "old").unwrap();
        let dest = trash_into(d.path(), "src/a.lua", t.path()).unwrap();
        assert_eq!(dest, t.path().join("a 2.lua"));
        assert_eq!(fs::read_to_string(t.path().join("a.lua")).unwrap(), "old");
        assert!(!d.path().join("src/a.lua").exists());
        let dir = trash_into(d.path(), "src/sub", t.path()).unwrap();
        assert_eq!(fs::read_to_string(dir.join("b.lua")).unwrap(), "y");
        assert!(!d.path().join("src/sub").exists());
        assert!(trash_into(d.path(), "../x", t.path()).is_err());
        assert!(trash_into(d.path(), "nope.lua", t.path()).is_err());
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

    #[test]
    fn creates_renames_and_deletes_inside_the_project() {
        let d = project();
        let r = d.path();
        create_file(r, "src/new/c.lua").unwrap();
        assert_eq!(read_text(r, "src/new/c.lua").unwrap(), "");
        assert!(create_file(r, "main.lua").is_err(), "never replaces");
        create_dir(r, "assets/img").unwrap();
        assert!(r.join("assets/img").is_dir());
        assert!(create_dir(r, "main.lua").is_err());
        rename(r, "src/a.lua", "src/sub/a2.lua").unwrap();
        assert!(!r.join("src/a.lua").exists() && r.join("src/sub/a2.lua").is_file());
        assert!(rename(r, "src/sub/a2.lua", "main.lua").is_err(), "never replaces");
        assert!(rename(r, "src", "src/sub/deeper").is_err(), "not into itself");
        assert!(rename(r, "nope.lua", "x.lua").is_err());
        delete(r, "src/sub").unwrap();
        assert!(!r.join("src/sub").exists());
        delete(r, "main.lua").unwrap();
        assert!(delete(r, "main.lua").is_err());
    }

    #[test]
    fn changes_refuse_odd_and_escaping_paths() {
        let d = project();
        let r = d.path();
        for bad in ["", " ", ".", "./x", "../x", "/abs", "a//b", "src/../../x", "a\\b"] {
            assert!(create_file(r, bad).is_err(), "{bad:?}");
            assert!(delete(r, bad).is_err(), "{bad:?}");
        }
        assert!(rename(r, "main.lua", "../moved.lua").is_err());
        assert!(r.join("main.lua").exists());
    }

    #[cfg(unix)]
    #[test]
    fn changes_cannot_follow_a_link_out() {
        let d = project();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("keep.txt"), "k").unwrap();
        std::os::unix::fs::symlink(outside.path(), d.path().join("link")).unwrap();
        assert!(delete(d.path(), "link/keep.txt").is_err());
        assert!(create_file(d.path(), "link/new.txt").is_err());
        assert!(create_dir(d.path(), "link/sub").is_err());
        assert!(rename(d.path(), "main.lua", "link/main.lua").is_err());
        assert!(outside.path().join("keep.txt").exists());
        assert!(!outside.path().join("new.txt").exists() && !outside.path().join("sub").exists());
        assert!(d.path().join("main.lua").exists());
    }

    #[test]
    fn searches_text_files_only() {
        let d = project();
        let r = d.path();
        fs::write(r.join("src/a.lua"), "local Player = 1\nprint(player)\n").unwrap();
        fs::write(r.join("blob.bin"), [0xff, 0xfe, b'p', b'l', b'a', b'y', b'e', b'r']).unwrap();
        fs::write(r.join(".git/config"), "player").unwrap();
        let f = search(r, "player", false).unwrap();
        let got: Vec<_> = f.hits.iter().map(|h| (h.path.as_str(), h.line)).collect();
        assert_eq!(got, [("src/a.lua", 1), ("src/a.lua", 2)], "no binary, no .git");
        assert_eq!(search(r, "player", true).unwrap().hits.len(), 1, "case sensitive");
        assert!(search(r, "", false).unwrap().hits.is_empty());
        assert_eq!(search(r, "t(p", false).unwrap().hits.len(), 1, "plain text, not a pattern");
    }

    #[test]
    fn search_is_capped_and_cuts_long_lines() {
        let d = project();
        fs::write(d.path().join("big.txt"), "x\n".repeat(MAX_HITS + 10)).unwrap();
        fs::write(d.path().join("long.txt"), format!("needle {}", "y".repeat(500))).unwrap();
        let f = search(d.path(), "x", true).unwrap();
        assert_eq!(f.hits.len(), MAX_HITS);
        assert!(f.truncated);
        let l = search(d.path(), "needle", true).unwrap();
        assert!(l.hits[0].text.chars().count() <= 201 && l.hits[0].text.ends_with('…'));
    }

    #[cfg(unix)]
    #[test]
    fn search_does_not_read_through_links() {
        let d = project();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("s.txt"), "needle").unwrap();
        std::os::unix::fs::symlink(outside.path().join("s.txt"), d.path().join("l.txt")).unwrap();
        std::os::unix::fs::symlink(outside.path(), d.path().join("ldir")).unwrap();
        assert!(search(d.path(), "needle", true).unwrap().hits.is_empty());
    }
}
