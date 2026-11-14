//! The code JoyrIDE inserts so a project's libraries work, and the record that lets the editor show it.
//!
//! A library installed into `libs/` is not found by `require` until the project's require path says where to look. JoyrIDE adds that
//! line to `main.lua`, one per library, when a library is added, and takes it out again when the library is removed. What it inserted is
//! written down in `.joyride/inserted.json` (the file, the line, its exact text), so the editor can mark those lines with a colour bar
//! and clear the mark of any line the person touches:
//!
//! * a line the person has touched (changed, deleted, or written themselves first) is theirs: it is never changed, re-added or removed;
//! * a library's line is removed with the library only while it is still exactly what JoyrIDE wrote;
//! * nothing is inserted into a file with unsaved edits (the shell checks) and nothing outside `main.lua`.
//!
//! Compiled libraries (C modules) and libraries required by their path from the game's folder (`libs.g3d.g3d`) need no require path.

use crate::library::{Entry, Kind, Library};
use crate::manifest::Manifest;
use crate::{err, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The file the integration lines go in.
pub const TARGET: &str = "main.lua";
const RECORD: &str = ".joyride/inserted.json";

/// One line to insert.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    /// `path:<library>`.
    pub id: String,
    pub text: String,
    /// What the editor says when the line is hovered.
    pub note: String,
}

/// The require-path templates that make `entry` loadable from `libs/`, or none when it needs no path.
pub fn path_templates(e: &Entry) -> Vec<String> {
    let needs_none = e.kind == Kind::Native || e.caution.as_deref().is_some_and(|c| c.contains("`libs."));
    if needs_none {
        return Vec::new();
    }
    let n = &e.name;
    let mut t = vec!["libs/?.lua".to_string(), "libs/?/init.lua".into(), format!("libs/{n}/?.lua"), format!("libs/{n}/?/init.lua")];
    // Folders inside the library whose files require each other by bare name (Luafinding's src/).
    for p in &e.paths {
        t.push(format!("libs/{n}/{p}/?.lua"));
    }
    t
}

/// The line that adds `entry`'s templates to LÖVE's require path.
pub fn line_for(e: &Entry) -> Option<Line> {
    let t = path_templates(e);
    if t.is_empty() {
        return None;
    }
    Some(Line {
        id: format!("path:{}", e.name),
        text: format!("love.filesystem.setRequirePath(love.filesystem.getRequirePath() .. \";{}\")", t.join(";")),
        note: format!("Added by JoyrIDE so `require` finds {} in libs/{}. Edit this line and JoyrIDE leaves it alone from then on.", e.name, e.name),
    })
}

/// The lines the project's `love.toml` dependencies call for, by library name.
pub fn expected(root: &Path, lib: &Library) -> Vec<Line> {
    let Ok(text) = std::fs::read_to_string(root.join("love.toml")) else { return Vec::new() };
    let Ok(m) = Manifest::parse(&text) else { return Vec::new() };
    let mut out: Vec<Line> = m.dependencies.keys().filter_map(|k| lib.get(k)).filter_map(line_for).collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// The require-path entries a language server needs to resolve `require` the way the lines above make the game do (relative to the project).
pub fn runtime_path(root: &Path, lib: &Library) -> Vec<String> {
    let mut p: Vec<String> = vec!["?.lua".into(), "?/init.lua".into(), "libs/?.lua".into(), "libs/?/init.lua".into()];
    if let Ok(text) = std::fs::read_to_string(root.join("love.toml")) {
        if let Ok(m) = Manifest::parse(&text) {
            for k in m.dependencies.keys() {
                if let Some(e) = lib.get(k) {
                    for t in path_templates(e) {
                        if !p.contains(&t) {
                            p.push(t);
                        }
                    }
                }
            }
        }
    }
    p
}

// ---- the record --------------------------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Record {
    id: String,
    file: String,
    /// 1-based, where it was when last known (a hint: the text decides).
    line: usize,
    text: String,
    note: String,
    /// The person has touched it: it is theirs, no mark, never changed.
    #[serde(default)]
    owned: bool,
}

fn load(root: &Path) -> Vec<Record> {
    std::fs::read_to_string(root.join(RECORD)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn save(root: &Path, recs: &[Record]) -> Result<()> {
    let f = root.join(RECORD);
    if recs.is_empty() {
        let _ = std::fs::remove_file(&f);
        return Ok(());
    }
    if let Some(d) = f.parent() {
        std::fs::create_dir_all(d)?;
    }
    let tmp = f.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(recs).map_err(|e| crate::Error::Msg(e.to_string()))?)?;
    std::fs::rename(&tmp, &f)?;
    Ok(())
}

/// The index of the line of `text` equal to `want` nearest `hint` (0-based), if any.
fn find(lines: &[&str], want: &str, hint: usize) -> Option<usize> {
    lines.iter().enumerate().filter(|(_, l)| l.trim_end_matches('\r') == want).map(|(i, _)| i).min_by_key(|i| i.abs_diff(hint))
}

/// A mark the editor shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Mark {
    pub id: String,
    /// 1-based line in the text asked about.
    pub line: usize,
    pub note: String,
}

/// The inserted lines of `file` that are still there, untouched, in `text` (what the editor holds). A recorded line that is no longer in
/// the text was touched: it becomes the person's, and is not marked again.
pub fn marks(root: &Path, file: &str, text: &str) -> Result<Vec<Mark>> {
    let mut recs = load(root);
    let lines: Vec<&str> = text.split('\n').collect();
    let mut out = Vec::new();
    let mut changed = false;
    for r in recs.iter_mut().filter(|r| r.file == file && !r.owned) {
        match find(&lines, &r.text, r.line.saturating_sub(1)) {
            Some(i) => {
                if r.line != i + 1 {
                    r.line = i + 1;
                    changed = true;
                }
                out.push(Mark { id: r.id.clone(), line: i + 1, note: r.note.clone() });
            }
            None => {
                r.owned = true;
                changed = true;
            }
        }
    }
    if changed {
        save(root, &recs)?;
    }
    out.sort_by_key(|m| m.line);
    Ok(out)
}

/// The person touched the line: it is theirs from now on.
pub fn clear(root: &Path, id: &str) -> Result<()> {
    let mut recs = load(root);
    let mut changed = false;
    for r in recs.iter_mut().filter(|r| r.id == id && !r.owned) {
        r.owned = true;
        changed = true;
    }
    if changed {
        save(root, &recs)?;
    }
    Ok(())
}

/// What a sync did.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct Synced {
    /// Libraries whose line was added.
    pub added: Vec<String>,
    /// Libraries whose line was removed with them.
    pub removed: Vec<String>,
    /// A removed library's line the person had touched: left where it is.
    pub kept: Vec<String>,
    /// `main.lua` did not exist and was made.
    pub created: bool,
}

/// Brings `main.lua` in line with the project's libraries: adds the lines they call for, removes the ones of libraries that are gone (only
/// while untouched). Writes nothing if nothing changes.
pub fn sync(root: &Path, lib: &Library) -> Result<Synced> {
    if !root.is_dir() {
        return err("not a folder");
    }
    let want = expected(root, lib);
    let mut recs = load(root);
    let path: PathBuf = root.join(TARGET);
    let existed = path.is_file();
    let text = if existed { std::fs::read_to_string(&path)? } else { String::new() };
    let crlf = text.contains("\r\n");
    let nl = if crlf { "\r\n" } else { "\n" };
    let mut lines: Vec<String> = if text.is_empty() { Vec::new() } else { text.split('\n').map(|l| l.trim_end_matches('\r').to_string()).collect() };
    // A final newline makes an empty last element: keep it out of the way while editing.
    let trailing = lines.last().is_some_and(|l| l.is_empty());
    if trailing {
        lines.pop();
    }
    let mut out = Synced::default();

    // Locate what is recorded (against the current text) before anything moves.
    let mut located: Vec<(usize, Option<usize>)> = Vec::new(); // (record index, line index)
    for (ri, r) in recs.iter().enumerate().filter(|(_, r)| r.file == TARGET) {
        let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
        located.push((ri, if r.owned { None } else { find(&refs, &r.text, r.line.saturating_sub(1)) }));
    }
    // Lines that are not wanted any more.
    let mut drop_lines: Vec<usize> = Vec::new();
    let mut drop_recs: Vec<usize> = Vec::new();
    for (ri, li) in &located {
        let r = &recs[*ri];
        if want.iter().any(|w| w.id == r.id) {
            continue;
        }
        let name = r.id.trim_start_matches("path:").to_string();
        match li {
            Some(i) => {
                drop_lines.push(*i);
                out.removed.push(name);
            }
            None => {
                if !r.owned || lines.iter().any(|l| *l == r.text) {
                    out.kept.push(name);
                }
            }
        }
        drop_recs.push(*ri);
    }
    // Ids the person already has a say over (a record, touched or not) or whose exact line is already in the file are not added again.
    let mut to_add: Vec<&Line> = Vec::new();
    for w in &want {
        let has_record = recs.iter().any(|r| r.id == w.id && r.file == TARGET);
        let written_by_hand = lines.iter().any(|l| *l == w.text);
        if !has_record && !written_by_hand {
            to_add.push(w);
        } else if !has_record && written_by_hand {
            // theirs: remember it so a later removal does not take it out
            recs.push(Record { id: w.id.clone(), file: TARGET.into(), line: 1, text: w.text.clone(), note: w.note.clone(), owned: true });
        }
    }
    // Remove (from the bottom, so indexes hold), then insert after the last line still ours, or at the top.
    drop_lines.sort_unstable();
    drop_lines.dedup();
    let after_removal_anchor = {
        // the last surviving inserted line, as an index into the text after removals
        let surviving: Vec<usize> = located.iter().filter(|(ri, li)| li.is_some() && !drop_recs.contains(ri)).filter_map(|(_, li)| *li).collect();
        surviving.into_iter().max().map(|m| m - drop_lines.iter().filter(|d| **d < m).count() + 1)
    };
    for i in drop_lines.iter().rev() {
        lines.remove(*i);
    }
    let at = after_removal_anchor.unwrap_or(0).min(lines.len());
    for (k, w) in to_add.iter().enumerate() {
        lines.insert(at + k, w.text.clone());
        out.added.push(w.id.trim_start_matches("path:").to_string());
    }
    // Records: drop the removed ones, add the new ones, then re-locate every inserted line.
    let mut drop_recs_sorted = drop_recs.clone();
    drop_recs_sorted.sort_unstable();
    for ri in drop_recs_sorted.iter().rev() {
        recs.remove(*ri);
    }
    for w in &to_add {
        recs.push(Record { id: w.id.clone(), file: TARGET.into(), line: 1, text: w.text.clone(), note: w.note.clone(), owned: false });
    }
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    for r in recs.iter_mut().filter(|r| r.file == TARGET && !r.owned) {
        match find(&refs, &r.text, r.line.saturating_sub(1)) {
            Some(i) => r.line = i + 1,
            None => r.owned = true,
        }
    }
    let changed = !out.added.is_empty() || !out.removed.is_empty();
    if changed {
        let mut s = lines.join(nl);
        if trailing || !existed || !s.is_empty() {
            s.push_str(nl);
        }
        if !existed {
            out.created = true;
        }
        crate::files::write_text(root, TARGET, &s)?;
    }
    recs.sort_by(|a, b| a.id.cmp(&b.id));
    save(root, &recs)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(deps: &[&str], main: Option<&str>) -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        let mut t = String::from("[project]\nname = \"G\"\nbundle_id = \"dev.x.g\"\nversion = \"1\"\n");
        let lib = Library::bundled();
        for n in deps {
            let e = lib.get(n).unwrap();
            t.push_str(&format!("\n[dependencies.{}]\ngit = \"{}\"\nrev = \"{}\"\n", e.name, e.git, e.rev));
        }
        std::fs::write(d.path().join("love.toml"), t).unwrap();
        if let Some(m) = main {
            std::fs::write(d.path().join("main.lua"), m).unwrap();
        }
        d
    }

    fn main_of(d: &tempfile::TempDir) -> String {
        std::fs::read_to_string(d.path().join("main.lua")).unwrap()
    }

    #[test]
    fn a_library_whose_files_require_each_other_by_bare_name_gets_its_folder_on_the_path() {
        let lib = Library::bundled();
        let e = lib.get("Luafinding").unwrap();
        assert_eq!(e.paths, ["src"]);
        let l = line_for(e).unwrap();
        assert!(l.text.contains(";libs/Luafinding/src/?.lua\")"), "{}", l.text);
        assert!(l.text.contains("libs/Luafinding/?.lua"), "the usual templates stay: {}", l.text);
        // Most libraries have none.
        assert!(lib.get("bump").unwrap().paths.is_empty());
    }

    #[test]
    fn a_library_gets_a_require_path_line_and_a_compiled_one_does_not() {
        let lib = Library::bundled();
        let l = line_for(lib.get("bump").unwrap()).unwrap();
        assert!(l.text.starts_with("love.filesystem.setRequirePath(love.filesystem.getRequirePath() .. \";libs/?.lua;libs/?/init.lua;libs/bump/?.lua;libs/bump/?/init.lua\")"), "{}", l.text);
        assert!(line_for(lib.get("love-nuklear").unwrap()).is_none(), "a C module has no Lua path");
        assert!(line_for(lib.get("g3d").unwrap()).is_none(), "g3d is required by its path from the game's folder");
        assert!(line_for(lib.get("neumetik-midi").unwrap()).is_none());
    }

    #[test]
    fn adding_libraries_inserts_their_lines_at_the_top_and_records_them() {
        let d = project(&["bump", "hump"], Some("function love.load()\nend\n"));
        let r = sync(d.path(), &Library::bundled()).unwrap();
        assert_eq!(r.added, ["bump", "hump"]);
        let m = main_of(&d);
        let lines: Vec<&str> = m.lines().collect();
        assert!(lines[0].contains("libs/bump/") && lines[1].contains("libs/hump/"), "{m}");
        assert_eq!(lines[2], "function love.load()", "the person's code follows, untouched");
        let marks = marks(d.path(), "main.lua", &m).unwrap();
        assert_eq!(marks.iter().map(|m| (m.id.as_str(), m.line)).collect::<Vec<_>>(), [("path:bump", 1), ("path:hump", 2)]);
        // Again: nothing to do, nothing written.
        let again = sync(d.path(), &Library::bundled()).unwrap();
        assert_eq!(again, Synced::default());
        assert_eq!(main_of(&d), m);
    }

    #[test]
    fn a_project_without_main_lua_gets_one_and_a_second_library_goes_after_the_first() {
        let d = project(&["bump"], None);
        let r = sync(d.path(), &Library::bundled()).unwrap();
        assert!(r.created && main_of(&d).lines().count() == 1);
        // The person writes code under it, then adds another library: it goes beside the first, not above their code.
        std::fs::write(d.path().join("main.lua"), format!("{}function love.draw() end\n", main_of(&d))).unwrap();
        let mut t = std::fs::read_to_string(d.path().join("love.toml")).unwrap();
        let lib = Library::bundled();
        let e = lib.get("lume").unwrap();
        t.push_str(&format!("\n[dependencies.lume]\ngit = \"{}\"\nrev = \"{}\"\n", e.git, e.rev));
        std::fs::write(d.path().join("love.toml"), t).unwrap();
        let r = sync(d.path(), &lib).unwrap();
        assert_eq!(r.added, ["lume"]);
        let lines: Vec<String> = main_of(&d).lines().map(String::from).collect();
        assert!(lines[0].contains("libs/bump/") && lines[1].contains("libs/lume/") && lines[2] == "function love.draw() end", "{lines:?}");
    }

    #[test]
    fn removing_a_library_takes_its_untouched_line_and_leaves_a_touched_one() {
        let d = project(&["bump", "hump"], Some("print(1)\n"));
        sync(d.path(), &Library::bundled()).unwrap();
        // The person edits hump's line (a touched line is theirs), then both libraries go.
        let m = main_of(&d).replace("libs/hump/?/init.lua\")", "libs/hump/?/init.lua;libs/mine/?.lua\")");
        std::fs::write(d.path().join("main.lua"), &m).unwrap();
        std::fs::write(d.path().join("love.toml"), "[project]\nname = \"G\"\nbundle_id = \"dev.x.g\"\nversion = \"1\"\n").unwrap();
        let r = sync(d.path(), &Library::bundled()).unwrap();
        assert_eq!(r.removed, ["bump"]);
        assert_eq!(r.kept, ["hump"]);
        let after = main_of(&d);
        assert!(!after.contains("libs/bump/") && after.contains("libs/mine/") && after.ends_with("print(1)\n"), "{after}");
        assert!(marks(d.path(), "main.lua", &after).unwrap().is_empty(), "nothing is marked any more");
    }

    #[test]
    fn a_touched_line_is_never_marked_re_added_or_rewritten() {
        let d = project(&["bump"], Some("print(1)\n"));
        sync(d.path(), &Library::bundled()).unwrap();
        // Touch it in the editor: the shell reports it.
        clear(d.path(), "path:bump").unwrap();
        let m = main_of(&d);
        assert!(marks(d.path(), "main.lua", &m).unwrap().is_empty());
        // Delete it: still theirs; a later sync does not put it back.
        std::fs::write(d.path().join("main.lua"), "print(1)\n").unwrap();
        let r = sync(d.path(), &Library::bundled()).unwrap();
        assert_eq!(r, Synced::default());
        assert_eq!(main_of(&d), "print(1)\n");
    }

    #[test]
    fn a_line_changed_outside_the_editor_stops_being_ours_when_the_text_is_next_looked_at() {
        let d = project(&["bump"], Some("print(1)\n"));
        sync(d.path(), &Library::bundled()).unwrap();
        let edited = main_of(&d).replace("bump", "bumper");
        assert!(marks(d.path(), "main.lua", &edited).unwrap().is_empty());
        // And the unchanged text no longer comes back as ours either: it was given up.
        assert!(marks(d.path(), "main.lua", &main_of(&d)).unwrap().is_empty());
    }

    #[test]
    fn a_line_written_by_hand_first_is_left_alone_and_survives_the_library() {
        let d = project(&["lume"], None);
        let lib = Library::bundled();
        let theirs = line_for(lib.get("lume").unwrap()).unwrap().text;
        std::fs::write(d.path().join("main.lua"), format!("{theirs}\nprint(2)\n")).unwrap();
        let r = sync(d.path(), &lib).unwrap();
        assert_eq!(r, Synced::default(), "already there");
        assert!(marks(d.path(), "main.lua", &main_of(&d)).unwrap().is_empty());
        std::fs::write(d.path().join("love.toml"), "[project]\nname = \"G\"\nbundle_id = \"dev.x.g\"\nversion = \"1\"\n").unwrap();
        let r = sync(d.path(), &lib).unwrap();
        assert_eq!(r.removed, Vec::<String>::new());
        assert!(main_of(&d).contains("libs/lume/"), "the person's own line stays");
    }

    #[test]
    fn crlf_files_stay_crlf_and_lines_move_with_edits_above_them() {
        let d = project(&["bump"], Some("print(1)\r\nprint(2)\r\n"));
        sync(d.path(), &Library::bundled()).unwrap();
        assert!(main_of(&d).contains("\r\n") && !main_of(&d).replace("\r\n", "").contains('\n'));
        // The person adds three lines above it; the mark follows the text, not the number.
        let m = format!("-- a\n-- b\n-- c\n{}", main_of(&d));
        let marks = marks(d.path(), "main.lua", &m).unwrap();
        assert_eq!(marks.len(), 1);
        assert_eq!(marks[0].line, 4);
    }

    #[test]
    fn the_language_server_gets_the_same_paths() {
        let d = project(&["bump"], None);
        let p = runtime_path(d.path(), &Library::bundled());
        assert!(p.contains(&"libs/bump/?.lua".to_string()) && p.contains(&"?.lua".to_string()));
    }
}
