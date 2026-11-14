//! The General MIDI SoundFonts that ship with JoyrIDE, and which one the person has chosen for MIDI to sound like.
//!
//! The list is fixed here and the files are in `src-tauri/resources/soundfonts/` (each with its licence). The choice is a short id kept in
//! the app-data folder; "builtin" is the synthesizer's own sound, which needs no bank, and [`DEFAULT`] is what nothing chosen means. A person's input is only ever matched against this
//! list, never used as a path.

use crate::{err, Result};
use std::path::{Path, PathBuf};

/// The id of the choice that uses no bank: Neumetik's own synthesized instruments.
pub const BUILTIN: &str = "builtin";

/// What MIDI sounds like until the person chooses (2026-11-14, chosen by the maintainer after listening to all four).
pub const DEFAULT: &str = "pocketsongs";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bundled {
    pub id: &'static str,
    pub name: &'static str,
    pub file: &'static str,
    /// Its licence file's name, beside it.
    pub licence_file: &'static str,
    pub licence: &'static str,
    pub author: &'static str,
    pub size: u64,
    pub sha256: &'static str,
    /// What it sounds like, for the person choosing.
    pub summary: &'static str,
}

pub const BUNDLED: &[Bundled] = &[
    Bundled {
        id: "p800",
        name: "Sony Ericsson P800",
        file: "sony-ericsson-p800-gm.sf2",
        licence_file: "LICENSE-sony-ericsson-p800.txt",
        licence: "BSD-3-Clause",
        author: "Brian Salter (Beatnik), SoundFont by DroidLite",
        size: 334_920,
        sha256: "4d78a19f60518664f6e2ccdd0cf3216a12eceb6bd27e2e85704be3660a40f5fa",
        summary: "The smallest: a 2000-era mobile phone General MIDI bank, small samples, plain and a little thin.",
    },
    Bundled {
        id: "pocketsongs",
        name: "PocketSongsGM",
        file: "pocketsongs-gm.sf2",
        licence_file: "LICENSE-pocketsongs-gm.txt",
        licence: "CC0-1.0",
        author: "Vini",
        size: 671_012,
        sha256: "99e5e0256ba4c657cb3126cbb63f4db4f88569a69a8bd3000b6a63e6bef5634b",
        summary: "A pocket-sized General MIDI bank with 111 short samples; public domain, nothing to credit.",
    },
    Bundled {
        id: "protosquare",
        name: "ProtoSquare",
        file: "protosquare.sf2",
        licence_file: "LICENSE-protosquare.txt",
        licence: "CC0-1.0",
        author: "Yingchun Soul",
        size: 364_238,
        sha256: "207ba845c1d729ea2c33629c3ae5b83daf8b16926b9e68c809adf7f76a5e74b5",
        summary: "Every instrument as an old sound chip's square wave (an SN76489, a Game Gear): chiptune, not a General MIDI imitation; public domain.",
    },
];

pub fn get(id: &str) -> Option<&'static Bundled> {
    BUNDLED.iter().find(|b| b.id == id)
}

/// The file of bundled bank `id` inside `dir` (the resources folder), if the choice is a bank and the file is there.
pub fn path(dir: &Path, id: &str) -> Option<PathBuf> {
    let b = get(id)?;
    let p = dir.join(b.file);
    p.is_file().then_some(p)
}

/// The id kept in `file`, if it is one in the list; otherwise [`DEFAULT`] (nothing chosen yet, or a file that names something else).
pub fn load_choice(file: &Path) -> String {
    let text = std::fs::read_to_string(file).unwrap_or_default();
    let id = text.trim();
    if id == BUILTIN || get(id).is_some() { id.to_string() } else { DEFAULT.to_string() }
}

pub fn save_choice(file: &Path, id: &str) -> Result<()> {
    if id != BUILTIN && get(id).is_none() {
        return err(format!("`{id}` is not one of the SoundFonts that ship with JoyrIDE"));
    }
    if let Some(d) = file.parent() {
        std::fs::create_dir_all(d)?;
    }
    std::fs::write(file, id)?;
    Ok(())
}

// ---- using a bank in a project -------------------------------------------------------------------------------------------------

/// Something to know before a bank is added to a project (or after: it is the same list).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Warning {
    /// `blocker` (nothing will be added), `warning` (it will be added, and this may bite) or `info`.
    pub level: &'static str,
    pub text: String,
}

fn w(level: &'static str, text: impl Into<String>) -> Warning {
    Warning { level, text: text.into() }
}

/// What adding a bank to a project would do.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Plan {
    /// Where the bank goes in the project.
    pub dest: String,
    /// Where its licence goes, beside it.
    pub licence_dest: String,
    pub warnings: Vec<Warning>,
    /// A blocker is among the warnings: adding does nothing.
    pub blocked: bool,
    /// The same bytes are already there: adding only records the use.
    pub already_there: bool,
}

pub fn dest_of(b: &Bundled) -> String {
    format!("assets/soundfonts/{}", b.file)
}

pub fn licence_dest_of(b: &Bundled) -> String {
    format!("assets/soundfonts/{}", b.licence_file)
}

/// The credit line the project's `CREDITS.txt` carries for a bank whose licence asks for one.
pub fn attribution(b: &Bundled) -> &'static str {
    match b.id {
        "p800" => "Brian Salter's General MIDI patches for Beatnik (2000) as a SoundFont by DroidLite (2021), from Beatnik's miniBAE. Copyright (c) 2009 Beatnik, Inc. New BSD licence; see LICENSE-sony-ericsson-p800.txt.",
        _ => "",
    }
}

/// Where the bank came from, for the project's asset record.
pub fn source_url(b: &Bundled) -> &'static str {
    match b.id {
        "p800" => "https://github.com/heyigor/miniBAE",
        "pocketsongs" => "https://archive.org/download/wbs-full_202412/PocketSongsGM.sf2",
        _ => "https://archive.org/download/wbs-full_202412/Game%20Gear.sf2",
    }
}

/// The few lines of Lua that play a song through the bank added at `dest`, with neumetik-midi.
pub fn snippet(b: &Bundled) -> String {
    format!(
        "local NM = require(\"libs.neumetik-midi.bindings.love.neumetik_midi\")\nlocal music = NM.new()\nmusic:loadSoundBank(love.filesystem.read(\"{}\"))   -- before loading the song\nmusic:load(love.filesystem.read(\"song.mid\"))\nmusic:setLooping(true)\nmusic:play()\n-- and in love.update: music:update()\n",
        dest_of(b)
    )
}

fn soundfonts_in(root: &Path, dir: &Path, depth: u32, out: &mut Vec<String>, lua_binding: &mut bool) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || ["node_modules", "build", "target"].contains(&name.as_str()) {
            continue;
        }
        let p = e.path();
        if p.is_dir() {
            if depth > 0 {
                soundfonts_in(root, &p, depth - 1, out, lua_binding);
            }
        } else {
            let l = name.to_lowercase();
            if l.ends_with(".sf2") || l.ends_with(".sf3") || l.ends_with(".dls") {
                out.push(p.strip_prefix(root).unwrap_or(&p).to_string_lossy().replace('\\', "/"));
            } else if l == "neumetik_midi.lua" {
                *lua_binding = true;
            }
        }
    }
}

/// What adding `b` to the project at `root` would do, and what to know first. Reads files only. `bundled` is the bank's file in this build
/// (`None`: this build does not have it).
pub fn plan(root: &Path, b: &Bundled, bundled: Option<&Path>) -> Plan {
    let dest = dest_of(b);
    let mut ws: Vec<Warning> = Vec::new();
    let mut already_there = false;

    if bundled.is_none() {
        ws.push(w("blocker", format!("{} is not in this build of JoyrIDE, so there is nothing to copy.", b.name)));
    }
    if !root.is_dir() {
        ws.push(w("blocker", "No project is open."));
        return Plan { dest, licence_dest: licence_dest_of(b), blocked: true, warnings: ws, already_there };
    }
    if !root.join("main.lua").is_file() {
        ws.push(w("warning", "This folder has no main.lua, so it may not be a LÖVE project: check you opened the game's own folder."));
    }

    // The destination: the same file is fine, a different one is not replaced.
    let at = root.join(&dest);
    if at.symlink_metadata().is_ok() {
        let same = bundled.and_then(|f| Some(std::fs::read(f).ok()? == std::fs::read(&at).ok()?)).unwrap_or(false);
        if same {
            already_there = true;
            ws.push(w("info", format!("{dest} is already in this project, the same file. Nothing more to add.")));
        } else {
            ws.push(w("blocker", format!("{dest} already exists and is a different file (or not a plain file). Nothing is ever replaced: rename or remove it first.")));
        }
    } else if root.join("assets/soundfonts").symlink_metadata().is_ok_and(|m| !m.is_dir()) || root.join("assets").symlink_metadata().is_ok_and(|m| !m.is_dir()) {
        ws.push(w("blocker", "assets or assets/soundfonts exists and is not a folder."));
    }

    // Other banks, and the library that plays them.
    let mut found = Vec::new();
    let mut binding = false;
    soundfonts_in(root, root, 4, &mut found, &mut binding);
    found.retain(|f| *f != dest);
    if !found.is_empty() {
        let list = found.iter().take(4).cloned().collect::<Vec<_>>().join(", ");
        ws.push(w("warning", format!("This project already has {} other SoundFont file{} ({list}{}). Each one is read into memory whole when it is loaded: ship the one you use.", found.len(), if found.len() == 1 { "" } else { "s" }, if found.len() > 4 { ", ..." } else { "" })));
    }
    let has_dep = std::fs::read_to_string(root.join("love.toml")).ok().and_then(|t| crate::manifest::Manifest::parse(&t).ok()).is_some_and(|m| m.dependencies.keys().any(|k| k.eq_ignore_ascii_case("neumetik-midi")));
    if !(has_dep || binding) {
        ws.push(w("warning", "A SoundFont does nothing by itself. This project does not have a player for it: add neumetik-midi from Libraries (the Lua below uses it), or load the file with a library of your own."));
    } else if !root.join("bin").is_dir() && !root.join("libs/neumetik-midi/bindings/love").join("libneumetik_midi.dylib").exists() {
        ws.push(w("info", "neumetik-midi is in this project; its native library (libneumetik_midi) has to be built for each platform you ship and kept beside the game, outside the .love file."));
    }

    // Where it can play at all.
    let patches = root.join(".joyride/patches");
    let web = patches.join("web").is_dir();
    let mobile = patches.join("mobile").is_dir() || patches.join("ios").is_dir();
    if web || mobile {
        ws.push(w("warning", format!("This project has {} build set up. neumetik-midi cannot run there ({}), so the bank would only add {} to the download. Render the music to a sound file for that build instead.", if web && mobile { "a web and a mobile" } else if web { "a web" } else { "a mobile" }, if web { "the web has no FFI" } else { "iOS needs the library embedded in the app" }, human(b.size))));
    } else {
        ws.push(w("info", "neumetik-midi plays on desktop, Android and PortMaster. It cannot run in a web build (no FFI), and on iOS the library has to be embedded in the app; on those the bank would only add its size."));
    }

    // The bank itself.
    ws.push(w("info", format!("Adds {} to the game ({} in memory once loaded, about the file's size).", human(b.size), human(b.size))));
    match b.id {
        "p800" => ws.push(w("warning", "BSD-3-Clause asks you to keep Beatnik's copyright notice and licence with the game. They are copied beside the bank, and the project's CREDITS.txt (made by Export) lists it: ship both. It also forbids using Beatnik's name to promote your game.")),
        "pocketsongs" => ws.push(w("warning", "PocketSongsGM is CC0 by its maker's own text, but nothing says where its 111 samples came from, and nobody has been able to check. For a game you sell, the Sony Ericsson P800 bank has a clearer origin.")),
        "protosquare" => ws.push(w("warning", "ProtoSquare is every instrument as an old sound chip's square wave. Music written for General MIDI will sound like a Game Gear, not like the instruments it names.")),
        _ => {}
    }
    ws.sort_by_key(|x| match x.level { "blocker" => 0, "warning" => 1, _ => 2 });
    let blocked = ws.iter().any(|x| x.level == "blocker");
    Plan { dest, licence_dest: licence_dest_of(b), warnings: ws, blocked, already_there }
}

fn human(bytes: u64) -> String {
    if bytes >= 1_000_000 { format!("{:.1} MB", bytes as f64 / 1_000_000.0) } else { format!("{} KB", bytes.div_ceil(1000)) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../src-tauri/resources/soundfonts")
    }

    #[test]
    fn every_bundled_file_is_there_with_its_licence_and_its_recorded_size_and_checksum() {
        let ids: std::collections::BTreeSet<_> = BUNDLED.iter().map(|b| b.id).collect();
        assert_eq!(ids.len(), BUNDLED.len(), "unique ids");
        assert!(!ids.contains(BUILTIN));
        for b in BUNDLED {
            let bytes = std::fs::read(dir().join(b.file)).unwrap_or_else(|e| panic!("{}: {e}", b.file));
            assert_eq!(bytes.len() as u64, b.size, "{}", b.file);
            let licence = std::fs::read_to_string(dir().join(b.licence_file)).unwrap_or_else(|e| panic!("{}: {e}", b.licence_file));
            assert!(licence.contains(b.sha256), "{}: its licence file records the checksum of the file", b.file);
            assert!(licence.contains(b.name.split(' ').next().unwrap()), "{}", b.licence_file);
            assert!(["BSD-3-Clause", "CC0-1.0"].contains(&b.licence), "only permissive licences ship: {}", b.licence);
            assert!(&bytes[..4] == b"RIFF" && &bytes[8..12] == b"sfbk", "{} is a SoundFont", b.file);
            assert!(!b.summary.is_empty() && !b.author.is_empty());
        }
        let total: u64 = BUNDLED.iter().map(|b| b.size).sum();
        assert!(total < 1_500_000, "the three together stay under 1.5 MB: {total}");
    }

    #[test]
    fn a_choice_is_an_id_from_the_list_and_nothing_else() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("soundfont.txt");
        assert_eq!(load_choice(&f), DEFAULT, "nothing chosen is the default");
        assert!(get(DEFAULT).is_some(), "the default is a bundled bank");
        save_choice(&f, "p800").unwrap();
        assert_eq!(load_choice(&f), "p800");
        save_choice(&f, BUILTIN).unwrap();
        assert_eq!(load_choice(&f), BUILTIN, "choosing the built-in sound is remembered, not turned back into the default");
        assert!(save_choice(&f, "../../etc/passwd").is_err());
        assert!(save_choice(&f, "").is_err());
        std::fs::write(&f, "../../etc/passwd").unwrap();
        assert_eq!(load_choice(&f), DEFAULT, "a file that names anything else is ignored");
        assert!(path(&dir(), "p800").is_some() && path(&dir(), "../x").is_none() && path(&dir(), BUILTIN).is_none());
    }

    fn project(files: &[(&str, &str)]) -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        for (p, t) in files {
            let f = d.path().join(p);
            std::fs::create_dir_all(f.parent().unwrap()).unwrap();
            std::fs::write(f, t).unwrap();
        }
        d
    }

    fn levels(p: &Plan) -> Vec<&str> {
        p.warnings.iter().map(|w| w.level).collect()
    }

    fn has(p: &Plan, level: &str, word: &str) -> bool {
        p.warnings.iter().any(|w| w.level == level && w.text.contains(word))
    }

    #[test]
    fn warns_about_a_project_with_no_player_and_says_where_it_cannot_play() {
        let d = project(&[("main.lua", "")]);
        let b = get("p800").unwrap();
        let src = dir().join(b.file);
        let p = plan(d.path(), b, Some(&src));
        assert!(!p.blocked && !p.already_there);
        assert_eq!(p.dest, "assets/soundfonts/sony-ericsson-p800-gm.sf2");
        assert!(has(&p, "warning", "does nothing by itself"), "{:?}", p.warnings);
        assert!(has(&p, "warning", "BSD-3-Clause"));
        assert!(has(&p, "info", "cannot run in a web build"));
        // Blockers and warnings come before the notes.
        let l = levels(&p);
        assert!(l.windows(2).all(|x| x[0] <= x[1] || (x[0] == "warning" && x[1] == "info")), "{l:?}");
    }

    #[test]
    fn a_player_in_the_project_quiets_that_warning_and_a_web_build_makes_the_other_one_loud() {
        let d = project(&[("main.lua", ""), ("love.toml", "[project]\nname = \"G\"\nbundle_id = \"dev.x.g\"\nversion = \"1\"\n\n[dependencies.neumetik-midi]\ngit = \"https://github.com/joashchee/neumetik-midi\"\nrev = \"e275a35e4b01f77b1a15d35c251637e9b25fe3a1\"\n"), (".joyride/patches/web/x.lua", "")]);
        let b = get("pocketsongs").unwrap();
        let p = plan(d.path(), b, Some(&dir().join(b.file)));
        assert!(!has(&p, "warning", "does nothing by itself"));
        assert!(has(&p, "warning", "web build set up") && has(&p, "warning", "no FFI"));
        assert!(has(&p, "warning", "where its 111 samples came from"));
        assert!(!has(&p, "warning", "BSD"), "CC0 asks for nothing");
    }

    #[test]
    fn the_same_file_is_fine_a_different_one_is_never_replaced() {
        let b = get("protosquare").unwrap();
        let src = dir().join(b.file);
        let same = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(same.path().join("assets/soundfonts")).unwrap();
        std::fs::write(same.path().join("main.lua"), "").unwrap();
        std::fs::copy(&src, same.path().join(dest_of(b))).unwrap();
        let p = plan(same.path(), b, Some(&src));
        assert!(p.already_there && !p.blocked && has(&p, "info", "already in this project"));
        std::fs::write(same.path().join(dest_of(b)), "not the same").unwrap();
        let q = plan(same.path(), b, Some(&src));
        assert!(q.blocked && has(&q, "blocker", "different file"));
        assert_eq!(q.warnings[0].level, "blocker", "blockers first");
        assert!(has(&q, "warning", "old sound chip"));
    }

    #[test]
    fn other_soundfonts_no_project_and_a_missing_file_are_all_said() {
        let d = project(&[("main.lua", ""), ("music/big.sf2", "x"), ("music/old.dls", "x"), ("music/neumetik_midi.lua", "")]);
        let b = get("p800").unwrap();
        let p = plan(d.path(), b, Some(&dir().join(b.file)));
        assert!(has(&p, "warning", "2 other SoundFont files") && has(&p, "warning", "big.sf2"));
        assert!(!has(&p, "warning", "does nothing by itself"), "a neumetik_midi.lua anywhere counts as a player");
        let none = plan(&d.path().join("nope"), b, Some(&dir().join(b.file)));
        assert!(none.blocked && has(&none, "blocker", "No project is open"));
        let missing = plan(d.path(), b, None);
        assert!(missing.blocked && has(&missing, "blocker", "not in this build"));
        let odd = project(&[("readme.md", "")]);
        assert!(has(&plan(odd.path(), b, Some(&dir().join(b.file))), "warning", "no main.lua"));
        let file_in_the_way = project(&[("main.lua", ""), ("assets", "a file")]);
        assert!(has(&plan(file_in_the_way.path(), b, Some(&dir().join(b.file))), "blocker", "not a folder"));
    }

    #[test]
    fn the_snippet_names_the_path_it_was_added_at() {
        for b in BUNDLED {
            let s = snippet(b);
            assert!(s.contains(&format!("read(\"{}\")", dest_of(b))) && s.contains("loadSoundBank") && s.contains("music:update()"));
        }
        assert!(attribution(get("p800").unwrap()).contains("Beatnik") && attribution(get("pocketsongs").unwrap()).is_empty());
    }
}
