//! Finding the libraries a project uses, and which of them JoyrIDE's Libraries list does not know.
//!
//! Reads files only: nothing from the project is run, nothing is fetched. A library is evidence of one of these:
//!
//! * a dependency in `love.toml`;
//! * a folder or a single `.lua` file under a folder that is third-party by convention (`libs/`, `vendor/`, `third_party/`, ...);
//! * a folder or a `.lua` file anywhere else that carries a licence (a licence file, a `.git` folder, or a licence header), which is how
//!   third-party code looks and how a person's own code does not (`lib/core.lua` of your own is not a library);
//! * a `require` of a module that nothing in the project provides and LÖVE does not (a library that is used but not in the project).
//!
//! Each one is matched against the curated list by name, by the module it is required by, by the repository's name and by its git
//! address. A match is a library JoyrIDE already knows; the rest are offered. Offered ones are added to the person's own list
//! ([`Custom`], kept in the app-data folder) and are shown as not audited: JoyrIDE's curated list is audited, a person's own is not.

use crate::library::Library;
use crate::lua::{lex, Tok};
use crate::manifest::{valid_dep_name, valid_git_url, valid_ref, Manifest};
use crate::{err, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Folders that hold third-party code by convention: everything directly inside is a library.
const THIRD_PARTY_DIRS: &[&str] = &["libs", "vendor", "third_party", "thirdparty", "3rdparty", "deps", "external", "extern"];
/// Folders that are often a person's own code: only a licence makes what is inside a library.
const AMBIGUOUS_DIRS: &[&str] = &["lib", "libraries", "modules"];
/// Never searched: what the project is built from or builds.
const SKIP_DIRS: &[&str] = &[".git", ".joyride", ".claude", "node_modules", "build", "dist", "assets", "art", "audio", "sounds", "images", "fonts", "docs", "doc", "tests", "test", "spec", "samples", "results", "bin", "tools", "scripts"];
/// Module names that are the runtime's, not a library's.
const BUILTIN: &[&str] = &["love", "ffi", "bit", "jit", "socket", "ssl", "mime", "enet", "utf8", "string", "table", "math", "os", "io", "coroutine", "debug", "package", "https"];
/// Modules an editor injects while debugging; never part of a game.
const EDITOR_ONLY: &[&str] = &["lldebugger", "mobdebug", "debugger"];
/// Names too general to match a library by (cimgui-love's module is called `src`).
const GENERIC: &[&str] = &["src", "lib", "libs", "init", "main", "util", "utils", "core", "common", "base", "vendor", "module", "modules", "game", "scripts", "classes"];
const MAX_FILES: usize = 3000;
const MAX_FILE_BYTES: u64 = 512 * 1024;
const MAX_FOUND: usize = 200;

/// A library the project uses.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Found {
    /// A name that is a valid dependency name (what the library is called in the list).
    pub name: String,
    /// What `require` is given to load it, when something requires it.
    pub module: String,
    /// Why it counts as a library, one phrase each.
    pub how: Vec<String>,
    /// Where it is in the project (a folder or a file), relative to the project.
    pub path: Option<String>,
    pub git: Option<String>,
    pub rev: Option<String>,
    pub tag: Option<String>,
    pub licence: Option<String>,
    pub version: Option<String>,
    pub summary: Option<String>,
    /// Some project Lua file requires it.
    pub required: bool,
    /// Required, but nothing in the project provides it (and LÖVE does not).
    pub missing: bool,
}

/// A library the project uses that is already in JoyrIDE's list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Used {
    pub name: String,
    pub how: String,
    /// It is in this project's `love.toml`.
    pub installed: bool,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Detection {
    pub used: Vec<Used>,
    pub new: Vec<Found>,
    /// Required by the project but provided by nothing in it: a library installed elsewhere, or code mounted at run time. Nothing is
    /// known about them (no source, no licence), so they are shown but never offered. Each is "module (file:line)".
    pub unresolved: Vec<String>,
}

/// The characters that make two spellings of a name the same: `Bump.lua`, `bump_lua` and `bump` are one name.
fn norm(s: &str) -> String {
    let l = s.to_lowercase();
    let l = l.strip_suffix(".lua").unwrap_or(&l);
    l.chars().filter(|c| c.is_ascii_alphanumeric()).collect()
}

/// A git address with what does not matter removed, so two spellings of the same repository compare equal.
fn norm_git(u: &str) -> String {
    let l = u.trim().to_lowercase();
    let l = l.trim_end_matches('/').trim_end_matches(".git");
    let l = l.strip_prefix("https://").or_else(|| l.strip_prefix("http://")).or_else(|| l.strip_prefix("ssh://")).unwrap_or(l);
    let l = l.strip_prefix("git@").map(|r| r.replacen(':', "/", 1)).unwrap_or_else(|| l.to_string());
    l.strip_prefix("www.").map(String::from).unwrap_or(l)
}

fn top(module: &str) -> &str {
    module.split('.').next().unwrap_or(module)
}

fn is_builtin(module: &str) -> bool {
    BUILTIN.contains(&top(module)) || EDITOR_ONLY.contains(&top(module))
}

/// A file's text, if it is a reasonable size and UTF-8 (lossy): a library's source is read, never a binary or a huge file.
fn read_small(p: &Path) -> Option<String> {
    let m = fs::metadata(p).ok()?;
    if !m.is_file() || m.len() > MAX_FILE_BYTES {
        return None;
    }
    fs::read(p).ok().map(|b| String::from_utf8_lossy(&b).into_owned())
}

fn head(text: &str) -> String {
    text.chars().take(4096).collect()
}

/// Whether a file's own header says it carries a licence.
fn header_has_licence(text: &str) -> bool {
    let h = head(text);
    h.contains("Permission is hereby granted") || h.contains("SPDX-License-Identifier") || h.contains("Copyright (c)") || h.contains("Copyright (C)") || h.contains("The MIT License") || h.contains("zlib License") || h.contains("This is free and unencumbered software")
}

fn is_licence_file(name: &str) -> bool {
    let l = name.to_lowercase();
    l.contains("licen") || l.starts_with("copying") || l == "unlicense"
}

fn lua_files_in(dir: &Path, depth: u32, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        if p.is_dir() {
            if depth > 0 && !SKIP_DIRS.contains(&name.as_str()) {
                lua_files_in(&p, depth - 1, out);
            }
        } else if p.extension().is_some_and(|x| x == "lua") {
            out.push(p);
        }
        if out.len() > MAX_FILES {
            return;
        }
    }
}

/// Whether a folder looks like someone else's library rather than the project's own code.
fn folder_has_evidence(dir: &Path) -> bool {
    let Ok(rd) = fs::read_dir(dir) else { return false };
    let mut any_lua = false;
    let mut evidence = false;
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let p = e.path();
        if name == ".git" || is_licence_file(&name) && p.is_file() {
            evidence = true;
        }
        if p.extension().is_some_and(|x| x == "lua") {
            any_lua = true;
            if !evidence && read_small(&p).is_some_and(|t| header_has_licence(&t)) {
                evidence = true;
            }
        }
    }
    evidence && (any_lua || {
        let mut v = Vec::new();
        lua_files_in(dir, 3, &mut v);
        !v.is_empty()
    })
}

fn main_file(dir: &Path, name: &str) -> Option<PathBuf> {
    for f in [format!("{name}.lua"), "init.lua".into(), format!("{}.lua", name.to_lowercase())] {
        let p = dir.join(&f);
        if p.is_file() {
            return Some(p);
        }
    }
    let mut v = Vec::new();
    lua_files_in(dir, 1, &mut v);
    v.sort();
    v.into_iter().next()
}

/// `_VERSION = "1.2"`, `version = "1.2"`: the first quoted version-looking string on a line that says version.
fn version_in(text: &str) -> Option<String> {
    for line in head(text).lines().chain(text.lines().take(400)) {
        let l = line.to_lowercase();
        if !(l.contains("version")) || l.trim_start().starts_with("--") && !l.contains('=') {
            continue;
        }
        let (a, rest) = line.split_once(['"', '\''])?;
        let _ = a;
        let end = rest.find(['"', '\''])?;
        let v = &rest[..end];
        if !v.is_empty() && v.len() <= 24 && v.chars().next().is_some_and(|c| c.is_ascii_digit() || c == 'v') && v.chars().any(|c| c.is_ascii_digit()) {
            return Some(v.to_string());
        }
    }
    None
}

/// The first comment line of a file that says what it is (not a licence, not an author line).
fn summary_in(text: &str) -> Option<String> {
    for line in text.lines().take(30) {
        let t = line.trim();
        let Some(c) = t.strip_prefix("--") else { if t.is_empty() { continue } else { break } };
        let c = c.trim_start_matches(['-', '[', '=', ' ']).trim();
        let low = c.to_lowercase();
        if c.len() < 12 || ["copyright", "license", "licence", "permission", "author", "@", "http", "released"].iter().any(|w| low.contains(w)) {
            continue;
        }
        return Some(c.chars().take(110).collect());
    }
    None
}

/// The remote a `.git` folder points at, read from its config (git is not run).
fn git_remote(dir: &Path) -> Option<(String, Option<String>)> {
    let cfg = fs::read_to_string(dir.join(".git/config")).ok()?;
    let url = cfg.lines().find_map(|l| l.trim().strip_prefix("url = ").map(|u| u.trim().to_string()))?;
    let rev = fs::read_to_string(dir.join(".git/HEAD")).ok().map(|h| h.trim().to_string()).filter(|h| h.len() == 40 && h.chars().all(|c| c.is_ascii_hexdigit()));
    Some((url, rev))
}

fn clean_name(raw: &str) -> String {
    let n = raw.strip_suffix(".lua").unwrap_or(raw);
    n.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '-' | '_') { c } else { '-' }).collect::<String>().trim_matches('-').to_string()
}

#[derive(Clone)]
struct Cand {
    found: Found,
    /// Project-relative prefix of what belongs to it (`libs/hump/`, `json.lua`).
    prefix: String,
}

fn rel(root: &Path, p: &Path) -> String {
    p.strip_prefix(root).unwrap_or(p).to_string_lossy().replace('\\', "/")
}

fn folder_cand(root: &Path, dir: &Path, how: &str) -> Option<Cand> {
    let name = clean_name(&dir.file_name()?.to_string_lossy());
    if name.is_empty() || !valid_dep_name(&name) {
        return None;
    }
    let main = main_file(dir, &name);
    let text = main.as_ref().and_then(|m| read_small(m)).unwrap_or_default();
    let (git, rev) = git_remote(dir).map(|(u, r)| (Some(u), r)).unwrap_or((None, None));
    let licence = ["LICENSE", "LICENSE.md", "LICENSE.txt", "license.txt", "license.md", "COPYING", "README.md", "readme.md"]
        .iter()
        .find_map(|f| asset_core::licence::guess_for(&dir.join(f), dir).map(|g| g.spdx))
        .or_else(|| asset_core::licence::guess_from_text(&head(&text)));
    let p = rel(root, dir);
    Some(Cand {
        prefix: format!("{p}/"),
        found: Found { name, module: String::new(), how: vec![how.to_string()], path: Some(p), git, rev, tag: None, licence, version: version_in(&text), summary: summary_in(&text), required: false, missing: false },
    })
}

fn file_cand(root: &Path, file: &Path, how: &str) -> Option<Cand> {
    let name = clean_name(&file.file_name()?.to_string_lossy());
    if name.is_empty() || !valid_dep_name(&name) {
        return None;
    }
    let text = read_small(file)?;
    let p = rel(root, file);
    Some(Cand {
        prefix: p.clone(),
        found: Found { name, module: String::new(), how: vec![how.to_string()], path: Some(p), git: None, rev: None, tag: None, licence: asset_core::licence::guess_from_text(&head(&text)), version: version_in(&text), summary: summary_in(&text), required: false, missing: false },
    })
}

/// The (module, file, line) of every `require "x"`, `require("x")` in a file's code.
fn requires_in(text: &str) -> Vec<(String, u32)> {
    let t = lex(text);
    let mut out = Vec::new();
    for (i, tok) in t.iter().enumerate() {
        if !tok.is_name("require") || (i > 0 && (t[i - 1].is_punct(".") || t[i - 1].is_punct(":"))) {
            continue;
        }
        let mut j = i + 1;
        if t.get(j).is_some_and(|x| x.is_punct("(")) {
            j += 1;
        }
        if let Some(Tok::Str(s)) = t.get(j).map(|x| &x.tok) {
            out.push((s.clone(), tok.line));
        }
    }
    out
}

/// Whether the project has a Lua file for module `m` (at the root or under any of the folders a project keeps libraries in).
fn provides(root: &Path, m: &str) -> Option<String> {
    let path = m.replace('.', "/");
    let mut bases: Vec<String> = vec![String::new()];
    bases.extend(THIRD_PARTY_DIRS.iter().chain(AMBIGUOUS_DIRS).map(|d| format!("{d}/")));
    for b in &bases {
        for tail in [format!("{path}.lua"), format!("{path}/init.lua")] {
            let rp = format!("{b}{tail}");
            if root.join(&rp).is_file() {
                return Some(rp);
            }
        }
        // A library folder holds its own root on the path too (`libs/hump/class.lua` for `class`, `libs/jumper/jumper/...`).
        if !b.is_empty() {
            if let Ok(rd) = fs::read_dir(root.join(b.trim_end_matches('/'))) {
                for e in rd.flatten().take(400) {
                    if e.path().is_dir() {
                        let rp = format!("{b}{}/{path}.lua", e.file_name().to_string_lossy());
                        if root.join(&rp).is_file() {
                            return Some(rp);
                        }
                    }
                }
            }
        }
    }
    None
}

/// What JoyrIDE's list calls it, if it knows this one.
fn curated_match<'a>(lib: &'a Library, f: &Found) -> Option<&'a crate::library::Entry> {
    let mine: Vec<String> = [f.name.as_str(), top(&f.module)].iter().filter(|s| !s.is_empty()).map(|s| norm(s)).filter(|s| !GENERIC.contains(&s.as_str())).collect();
    let my_git = f.git.as_deref().map(norm_git);
    lib.entries.iter().find(|e| {
        if my_git.as_deref().is_some_and(|g| !g.is_empty() && g == norm_git(&e.git)) {
            return true;
        }
        let repo = e.git.trim_end_matches('/').trim_end_matches(".git").rsplit('/').next().unwrap_or("").to_string();
        let theirs = [norm(&e.name), norm(top(&e.module)), norm(&repo)];
        mine.iter().any(|m| !m.is_empty() && theirs.iter().any(|t| !GENERIC.contains(&t.as_str()) && t == m))
    })
}

/// Detects the libraries `root` uses, against `lib` (JoyrIDE's list, with the person's own entries if any).
pub fn detect(root: &Path, lib: &Library) -> Result<Detection> {
    if !root.is_dir() {
        return err("not a folder");
    }
    let mut cands: Vec<Cand> = Vec::new();

    // 1. love.toml
    let mut installed: Vec<String> = Vec::new();
    if let Ok(text) = fs::read_to_string(root.join("love.toml")) {
        if let Ok(m) = Manifest::parse(&text) {
            for (name, d) in &m.dependencies {
                installed.push(name.clone());
                cands.push(Cand {
                    prefix: format!("libs/{name}/"),
                    found: Found { name: name.clone(), module: String::new(), how: vec!["a dependency in love.toml".into()], path: Some(format!("libs/{name}")), git: Some(d.git.clone()), rev: d.rev.clone(), tag: d.tag.clone(), licence: None, version: d.tag.clone(), summary: None, required: false, missing: false },
                });
            }
        }
    }

    // 2. folders that are third-party by convention, and licensed folders and files anywhere
    let Ok(rd) = fs::read_dir(root) else { return err("cannot read the folder") };
    let mut top_entries: Vec<_> = rd.flatten().collect();
    top_entries.sort_by_key(|e| e.file_name());
    for e in &top_entries {
        let name = e.file_name().to_string_lossy().into_owned();
        let p = e.path();
        if name.starts_with('.') {
            continue;
        }
        if p.is_dir() {
            if THIRD_PARTY_DIRS.contains(&name.as_str()) || AMBIGUOUS_DIRS.contains(&name.as_str()) {
                let conventional = THIRD_PARTY_DIRS.contains(&name.as_str());
                if let Ok(inner) = fs::read_dir(&p) {
                    let mut items: Vec<_> = inner.flatten().collect();
                    items.sort_by_key(|e| e.file_name());
                    for it in items {
                        let iname = it.file_name().to_string_lossy().into_owned();
                        let ip = it.path();
                        if iname.starts_with('.') {
                            continue;
                        }
                        if ip.is_dir() {
                            let mut v = Vec::new();
                            lua_files_in(&ip, 3, &mut v);
                            if (conventional || folder_has_evidence(&ip)) && !v.is_empty() {
                                cands.extend(folder_cand(root, &ip, &format!("a folder in {name}/")));
                            } else if conventional && v.is_empty() && folder_has_evidence(&ip) {
                                // a compiled library's folder (no Lua of its own): still a library the person put there
                                cands.extend(folder_cand(root, &ip, &format!("a folder in {name}/")));
                            }
                        } else if ip.extension().is_some_and(|x| x == "lua") {
                            let licensed = read_small(&ip).is_some_and(|t| header_has_licence(&t));
                            if conventional || licensed {
                                cands.extend(file_cand(root, &ip, &format!("a file in {name}/")));
                            }
                        }
                    }
                }
            } else if !SKIP_DIRS.contains(&name.as_str()) && folder_has_evidence(&p) {
                cands.extend(folder_cand(root, &p, "a licensed folder"));
            }
        } else if p.extension().is_some_and(|x| x == "lua") && name != "main.lua" && name != "conf.lua" && read_small(&p).is_some_and(|t| header_has_licence(&t)) {
            cands.extend(file_cand(root, &p, "a licensed file"));
        }
        if cands.len() > MAX_FOUND {
            break;
        }
    }
    // The same library found twice (love.toml and libs/): one, with both reasons.
    let mut merged: Vec<Cand> = Vec::new();
    for c in cands {
        if let Some(m) = merged.iter_mut().find(|m| norm(&m.found.name) == norm(&c.found.name)) {
            for h in c.found.how {
                if !m.found.how.contains(&h) {
                    m.found.how.push(h);
                }
            }
            for (a, b) in [(&mut m.found.git, c.found.git), (&mut m.found.rev, c.found.rev), (&mut m.found.tag, c.found.tag), (&mut m.found.licence, c.found.licence), (&mut m.found.version, c.found.version), (&mut m.found.summary, c.found.summary)] {
                if a.is_none() {
                    *a = b;
                }
            }
        } else {
            merged.push(c);
        }
    }
    let mut cands = merged;

    let mut d = Detection::default();
    // 3. what the project's own code requires (not what the libraries require of each other)
    let mut files = Vec::new();
    lua_files_in(root, 6, &mut files);
    files.sort();
    let mut missing: BTreeMap<String, (String, String)> = BTreeMap::new(); // top -> (module, where)
    for f in &files {
        let r = rel(root, f);
        let inside = cands.iter().any(|c| r == c.prefix || r.starts_with(&c.prefix));
        let Some(text) = read_small(f) else { continue };
        for (m, line) in requires_in(&text) {
            if is_builtin(&m) {
                continue;
            }
            match provides(root, &m) {
                Some(file) => {
                    if !inside {
                        if let Some(c) = cands.iter_mut().find(|c| file == c.prefix || file.starts_with(&c.prefix)) {
                            c.found.required = true;
                            if c.found.module.is_empty() {
                                c.found.module = m.clone();
                            }
                        }
                    }
                }
                None if !inside => {
                    missing.entry(top(&m).to_string()).or_insert((m.clone(), format!("{r}:{line}")));
                }
                None => {}
            }
        }
    }
    for (t, (m, at)) in missing {
        // Own modules the project has under another name (a `src` folder, a path with the project's own prefix) are not libraries:
        // only a name no project folder or file carries counts.
        if root.join(&t).exists() || root.join(format!("{t}.lua")).exists() {
            continue;
        }
        let name = clean_name(&t);
        if name.is_empty() || !valid_dep_name(&name) {
            continue;
        }
        if let Some(c) = cands.iter_mut().find(|c| norm(&c.found.name) == norm(&name)) {
            c.found.required = true;
            continue;
        }
        // A curated library the project requires but does not have is still "used" (the person may want to add it).
        let probe = Found { name: name.clone(), module: m.clone(), how: vec![format!("required at {at}, but not in the project")], path: None, git: None, rev: None, tag: None, licence: None, version: None, summary: None, required: true, missing: true };
        match curated_match(lib, &probe) {
            Some(e) => {
                if !d.used.iter().any(|u| u.name == e.name) {
                    d.used.push(Used { name: e.name.clone(), how: probe.how.join("; "), installed: false });
                }
            }
            None => d.unresolved.push(format!("{m} ({at})")),
        }
    }

    // 4. compare with the list
    for c in cands {
        let mut f = c.found;
        if f.module.is_empty() {
            f.module = f.name.clone();
        }
        match curated_match(lib, &f) {
            Some(e) => {
                if !d.used.iter().any(|u| u.name == e.name) {
                    d.used.push(Used { name: e.name.clone(), how: f.how.join("; "), installed: installed.iter().any(|i| norm(i) == norm(&e.name) || norm(i) == norm(&f.name)) });
                }
            }
            None => d.new.push(f),
        }
    }
    d.used.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    d.new.sort_by(|a, b| (b.required, &a.name).cmp(&(a.required, &b.name)).then(a.name.cmp(&b.name)));
    Ok(d)
}

// ---- the person's own libraries ------------------------------------------------------------------------------------------------

/// A library the person added from a project's offer. Unlike the curated list it is not audited and not measured; it carries only what
/// was read from the project.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Custom {
    pub name: String,
    pub module: String,
    pub summary: String,
    pub licence: String,
    pub git: Option<String>,
    pub rev: Option<String>,
    pub tag: Option<String>,
    pub version: Option<String>,
    /// The folder name of the project it was found in.
    pub found_in: String,
}

impl Custom {
    /// What is kept of a [`Found`]: checked, so a project's text cannot make a name or an address JoyrIDE would refuse.
    pub fn from_found(f: &Found, project: &str) -> Result<Custom> {
        if !valid_dep_name(&f.name) {
            return err(format!("`{}` is not a usable library name", f.name));
        }
        let git = f.git.clone().filter(|g| valid_git_url(g));
        let rev = f.rev.clone().filter(|r| valid_ref(r));
        let tag = f.tag.clone().filter(|r| valid_ref(r));
        let clip = |s: &str, n: usize| s.chars().filter(|c| !c.is_control()).take(n).collect::<String>();
        Ok(Custom {
            name: f.name.clone(),
            module: clip(if f.module.is_empty() { &f.name } else { &f.module }, 80),
            summary: clip(f.summary.as_deref().unwrap_or("Found in a project; no description."), 140),
            licence: clip(f.licence.as_deref().unwrap_or("unknown"), 40),
            git,
            rev,
            tag,
            version: f.version.as_deref().map(|v| clip(v, 24)),
            found_in: clip(project, 80),
        })
    }
}

/// The person's own libraries from `path` (none if the file is not there or is not readable: it is theirs, never fatal).
pub fn load_custom(path: &Path) -> Vec<Custom> {
    fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

/// Adds `found` (replacing one of the same name) and writes the list.
pub fn add_custom(path: &Path, found: &[Found], project: &str) -> Result<Vec<Custom>> {
    let mut list = load_custom(path);
    for f in found {
        let c = Custom::from_found(f, project)?;
        list.retain(|x| !x.name.eq_ignore_ascii_case(&c.name));
        list.push(c);
    }
    list.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    if let Some(d) = path.parent() {
        fs::create_dir_all(d)?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, serde_json::to_string_pretty(&list).map_err(|e| crate::Error::Msg(e.to_string()))?)?;
    fs::rename(&tmp, path)?;
    Ok(list)
}

/// Removes one of the person's own libraries.
pub fn remove_custom(path: &Path, name: &str) -> Result<Vec<Custom>> {
    let mut list = load_custom(path);
    list.retain(|x| !x.name.eq_ignore_ascii_case(name));
    fs::write(path, serde_json::to_string_pretty(&list).map_err(|e| crate::Error::Msg(e.to_string()))?)?;
    Ok(list)
}

/// "Not now, and do not ask again for these": names kept per project folder, in the app-data folder.
pub fn load_dismissed(path: &Path, project: &str) -> Vec<String> {
    let map: BTreeMap<String, Vec<String>> = fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    map.get(project).cloned().unwrap_or_default()
}

pub fn dismiss(path: &Path, project: &str, names: &[String]) -> Result<()> {
    let mut map: BTreeMap<String, Vec<String>> = fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    let e = map.entry(project.to_string()).or_default();
    for n in names {
        if !e.contains(n) {
            e.push(n.clone());
        }
    }
    if let Some(d) = path.parent() {
        fs::create_dir_all(d)?;
    }
    fs::write(path, serde_json::to_string_pretty(&map).map_err(|e| crate::Error::Msg(e.to_string()))?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, p: &str, text: &str) {
        let f = root.join(p);
        fs::create_dir_all(f.parent().unwrap()).unwrap();
        fs::write(f, text).unwrap();
    }

    const MIT: &str = "--[[\nPermission is hereby granted, free of charge, to any person obtaining a copy\n]]\n";

    #[test]
    fn a_curated_library_is_known_not_offered() {
        let d = tempfile::tempdir().unwrap();
        write(d.path(), "main.lua", "local hump = require 'hump.class'\nlocal lume = require(\"lume\")\n");
        write(d.path(), "libs/hump/class.lua", MIT);
        write(d.path(), "libs/lume/lume.lua", &format!("{MIT}return {{}}"));
        let r = detect(d.path(), &Library::bundled()).unwrap();
        assert!(r.new.is_empty(), "{:?}", r.new);
        let names: Vec<_> = r.used.iter().map(|u| u.name.as_str()).collect();
        assert_eq!(names, ["hump", "lume"]);
    }

    #[test]
    fn an_unknown_vendored_library_is_offered_with_what_was_read() {
        let d = tempfile::tempdir().unwrap();
        write(d.path(), "main.lua", "local f = require('frobnicate')\nf.go()\n");
        write(d.path(), "libs/frobnicate/frobnicate.lua", &format!("-- Frobnicates things for games\n{MIT}local M = {{ _VERSION = \"2.3.1\" }}\nreturn M\n"));
        write(d.path(), "libs/frobnicate/LICENSE", "MIT License\n\nPermission is hereby granted, free of charge, to any person obtaining a copy of this software\n");
        let r = detect(d.path(), &Library::bundled()).unwrap();
        assert_eq!(r.new.len(), 1, "{:?}", r.new);
        let f = &r.new[0];
        assert_eq!(f.name, "frobnicate");
        assert!(f.required && !f.missing);
        assert_eq!(f.module, "frobnicate");
        assert_eq!(f.version.as_deref(), Some("2.3.1"));
        assert_eq!(f.licence.as_deref(), Some("MIT"));
        assert_eq!(f.path.as_deref(), Some("libs/frobnicate"));
        assert_eq!(f.summary.as_deref(), Some("Frobnicates things for games"));
    }

    #[test]
    fn a_single_vendored_file_counts() {
        let d = tempfile::tempdir().unwrap();
        write(d.path(), "main.lua", "local g = require 'gizmo'\n");
        write(d.path(), "libs/gizmo.lua", "return {}\n");
        let r = detect(d.path(), &Library::bundled()).unwrap();
        assert_eq!(r.new.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["gizmo"]);
    }

    #[test]
    fn your_own_code_in_lib_is_not_a_library_but_a_licensed_one_is() {
        let d = tempfile::tempdir().unwrap();
        write(d.path(), "main.lua", "require 'lib.core'\nrequire 'lib.zed.zed'\nrequire 'player'\n");
        write(d.path(), "lib/core.lua", "return {}\n");
        write(d.path(), "player.lua", "return {}\n");
        write(d.path(), "lib/zed/zed.lua", &format!("{MIT}return {{}}"));
        let r = detect(d.path(), &Library::bundled()).unwrap();
        assert_eq!(r.new.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["zed"], "{:?}", r.new);
    }

    #[test]
    fn a_require_nothing_provides_is_a_missing_library_but_the_runtimes_modules_are_not() {
        let d = tempfile::tempdir().unwrap();
        write(d.path(), "main.lua", "local ffi = require 'ffi'\nlocal e = require 'love.timer'\nlocal s = require('socket.http')\nlocal w = require 'widgetry.core'\nlocal b = require 'bump'\n");
        let r = detect(d.path(), &Library::bundled()).unwrap();
        assert!(r.new.is_empty(), "nothing is known about a module nothing provides, so it is not offered: {:?}", r.new);
        assert_eq!(r.unresolved, ["widgetry.core (main.lua:4)"]);
        // bump is the curated library: used, though not installed
        assert!(r.used.iter().any(|u| u.name == "bump" && !u.installed));
    }

    #[test]
    fn requires_inside_a_library_do_not_count_as_the_projects() {
        let d = tempfile::tempdir().unwrap();
        write(d.path(), "main.lua", "require 'gizmo'\n");
        write(d.path(), "libs/gizmo.lua", "local x = require 'secret_dependency'\nreturn {}\n");
        let r = detect(d.path(), &Library::bundled()).unwrap();
        assert_eq!(r.new.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["gizmo"]);
    }

    #[test]
    fn love_toml_dependencies_match_by_address_even_under_another_name() {
        let d = tempfile::tempdir().unwrap();
        write(d.path(), "love.toml", "[project]\nname = \"G\"\nbundle_id = \"dev.x.g\"\nversion = \"1\"\n\n[dependencies.mybump]\ngit = \"https://github.com/kikito/bump.lua.git\"\ntag = \"v3.1.7\"\n\n[dependencies.other]\ngit = \"https://example.org/me/other\"\nrev = \"0123456789abcdef0123456789abcdef01234567\"\n");
        write(d.path(), "main.lua", "print(1)\n");
        let r = detect(d.path(), &Library::bundled()).unwrap();
        assert_eq!(r.used.len(), 1);
        assert_eq!(r.used[0].name, "bump");
        assert!(r.used[0].installed);
        assert_eq!(r.new.len(), 1);
        assert_eq!(r.new[0].name, "other");
        assert_eq!(r.new[0].git.as_deref(), Some("https://example.org/me/other"));
        assert_eq!(r.new[0].rev.as_deref().map(str::len), Some(40));
    }

    #[test]
    fn a_git_folder_is_read_for_its_address_without_running_git() {
        let d = tempfile::tempdir().unwrap();
        write(d.path(), "main.lua", "require 'thing'\n");
        write(d.path(), "libs/thing/thing.lua", "return {}\n");
        write(d.path(), "libs/thing/.git/config", "[remote \"origin\"]\n\turl = https://github.com/someone/thing.git\n");
        write(d.path(), "libs/thing/.git/HEAD", "0123456789abcdef0123456789abcdef01234567\n");
        let r = detect(d.path(), &Library::bundled()).unwrap();
        assert_eq!(r.new[0].git.as_deref(), Some("https://github.com/someone/thing.git"));
        assert_eq!(r.new[0].rev.as_deref(), Some("0123456789abcdef0123456789abcdef01234567"));
    }

    #[test]
    fn the_shipped_samples_have_nothing_to_offer() {
        // The IO Checker's own lib/ and ui/ are the project's code; the Benchmark Libraries' libs/ are all in the list.
        for s in ["io-checker", "benchmark-libraries"] {
            let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../samples").join(s);
            let r = detect(&root, &Library::bundled()).unwrap();
            assert!(r.new.is_empty(), "{s}: {:?}", r.new.iter().map(|f| (&f.name, &f.how)).collect::<Vec<_>>());
        }
        let r = detect(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../samples/benchmark-libraries"), &Library::bundled()).unwrap();
        assert!(r.used.len() >= 35, "only {} of its libraries were recognised", r.used.len());
    }

    #[test]
    fn custom_libraries_are_checked_saved_and_merged_into_the_list() {
        let d = tempfile::tempdir().unwrap();
        let store = d.path().join("custom-libraries.json");
        let good = Found { name: "frob".into(), module: "frob".into(), how: vec![], path: None, git: Some("https://example.org/f/frob".into()), rev: Some("0123456789abcdef0123456789abcdef01234567".into()), tag: None, licence: Some("MIT".into()), version: None, summary: Some("Frobs.\u{7}".into()), required: true, missing: false };
        let evil = Found { name: "../etc".into(), ..good.clone() };
        assert!(add_custom(&store, &[evil], "p").is_err());
        let bad_git = Found { git: Some("--upload-pack=x".into()), ..good.clone() };
        // an address git would read as an option is dropped, not stored
        let list = add_custom(&store, &[bad_git], "proj").unwrap();
        assert!(list[0].git.is_none());
        let list = add_custom(&store, &[good.clone()], "proj").unwrap();
        assert_eq!(list.len(), 1, "the same name replaces");
        let list = load_custom(&store);
        assert_eq!(list[0].git.as_deref(), Some("https://example.org/f/frob"));
        assert_eq!(list[0].summary, "Frobs.");
        let lib = Library::bundled().with_custom(&list);
        let e = lib.get("frob").unwrap();
        assert!(e.custom);
        assert_eq!(e.found_in.as_deref(), Some("proj"));
        assert!(Library::bundled().get("frob").is_none());
        // a custom library never shadows a curated one
        let clash = Custom { name: "bump".into(), ..list[0].clone() };
        assert_eq!(Library::bundled().with_custom(&[clash]).entries.iter().filter(|e| e.name == "bump").count(), 1);
        assert!(remove_custom(&store, "FROB").unwrap().is_empty());
    }

    #[test]
    fn dismissing_is_per_project() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("offers.json");
        dismiss(&f, "/a", &["x".into()]).unwrap();
        dismiss(&f, "/a", &["x".into(), "y".into()]).unwrap();
        assert_eq!(load_dismissed(&f, "/a"), ["x", "y"]);
        assert!(load_dismissed(&f, "/b").is_empty());
    }
}
