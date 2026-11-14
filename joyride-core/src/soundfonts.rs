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
}
