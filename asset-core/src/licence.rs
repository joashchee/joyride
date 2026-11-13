//! Licences: what a licence allows, whether the user has confirmed it, and a
//! best guess from the text a project ships beside its assets. A guess is
//! only ever a guess: it becomes usable when a person confirms it.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tri {
    Yes,
    No,
    Unknown,
}

impl Tri {
    pub fn as_str(self) -> &'static str {
        match self {
            Tri::Yes => "yes",
            Tri::No => "no",
            Tri::Unknown => "unknown",
        }
    }
    pub fn parse(s: &str) -> Tri {
        match s {
            "yes" => Tri::Yes,
            "no" => Tri::No,
            _ => Tri::Unknown,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Licence {
    /// An SPDX identifier, or `LicenseRef-…` for a custom one.
    pub spdx: String,
    pub commercial_use: Tri,
    pub modification: Tri,
    pub redistribution: Tri,
    pub attribution_required: bool,
    /// Derivatives must carry the same licence (copyleft).
    pub share_alike: bool,
}

impl Licence {
    /// A licence nothing is known about: every permission unknown.
    pub fn unknown(id: &str) -> Licence {
        Licence {
            spdx: id.to_string(),
            commercial_use: Tri::Unknown,
            modification: Tri::Unknown,
            redistribution: Tri::Unknown,
            attribution_required: false,
            share_alike: false,
        }
    }

    /// A licence from its SPDX identifier, with what it permits, where known.
    pub fn from_spdx(id: &str) -> Licence {
        use Tri::*;
        let l = |c, m, r, attr, sa| Licence {
            spdx: id.to_string(),
            commercial_use: c,
            modification: m,
            redistribution: r,
            attribution_required: attr,
            share_alike: sa,
        };
        match id {
            "CC0-1.0" | "Unlicense" | "0BSD" => l(Yes, Yes, Yes, false, false),
            "Zlib" => l(Yes, Yes, Yes, false, false),
            "MIT" | "ISC" | "Apache-2.0" | "BSD-2-Clause" | "BSD-3-Clause" | "OFL-1.1" => {
                l(Yes, Yes, Yes, true, false)
            }
            "GPL-2.0-only" | "GPL-3.0-only" | "LGPL-2.1-only" | "LGPL-3.0-only" | "MPL-2.0" => {
                l(Yes, Yes, Yes, true, true)
            }
            "LicenseRef-AllRightsReserved" => l(No, No, No, false, false),
            s if s.starts_with("CC-BY-") => {
                let nc = s.contains("-NC");
                let nd = s.contains("-ND");
                let sa = s.contains("-SA");
                l(if nc { No } else { Yes }, if nd { No } else { Yes }, Yes, true, sa)
            }
            _ => Licence::unknown(id),
        }
    }

    /// Whether a game that sells can use this asset as it is.
    pub fn allows_commercial_use(&self) -> bool {
        self.commercial_use == Tri::Yes
    }
}

/// Where a licence stands for one asset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// Nothing known.
    Unknown,
    /// Guessed from text; not yet checked by a person.
    Guessed,
    /// A person said this is the licence.
    Confirmed { by: String, at: i64, note: String },
}

impl Status {
    pub fn is_confirmed(&self) -> bool {
        matches!(self, Status::Confirmed { .. })
    }
}

/// Guesses an SPDX identifier from licence or readme text.
pub fn guess_from_text(text: &str) -> Option<String> {
    // An explicit tag wins.
    for line in text.lines() {
        if let Some(i) = line.find("SPDX-License-Identifier:") {
            let id = line[i + "SPDX-License-Identifier:".len()..]
                .trim()
                .trim_end_matches("*/")
                .trim();
            if !id.is_empty() {
                return Some(id.to_string());
            }
        }
    }
    let t: String = text.to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ");
    let has = |s: &str| t.contains(s);

    if let Some(i) = t.find("creativecommons.org/licenses/").map(|i| i + "creativecommons.org/licenses/".len()) {
        let rest = &t[i..];
        let kind: String = rest.chars().take_while(|c| c.is_ascii_alphabetic() || *c == '-').collect();
        let version: String = rest[kind.len()..].trim_start_matches('/').chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
        if !kind.is_empty() {
            let v = if version.is_empty() { "4.0".to_string() } else { version };
            return Some(format!("CC-{}-{}", kind.to_uppercase(), v));
        }
    }
    if has("creativecommons.org/publicdomain/zero") || has("cc0 1.0") || has("creative commons zero") {
        return Some("CC0-1.0".into());
    }
    for (phrase, kind) in [
        ("attribution-noncommercial-sharealike", "BY-NC-SA"),
        ("attribution-noncommercial-noderivatives", "BY-NC-ND"),
        ("attribution-noncommercial", "BY-NC"),
        ("attribution-noderivatives", "BY-ND"),
        ("attribution-sharealike", "BY-SA"),
    ] {
        if has(phrase) && has("creative commons") {
            let v = if has("3.0") { "3.0" } else { "4.0" };
            return Some(format!("CC-{kind}-{v}"));
        }
    }
    if has("creative commons") && has("attribution 4.0") {
        return Some("CC-BY-4.0".into());
    }
    if has("creative commons") && has("attribution 3.0") {
        return Some("CC-BY-3.0".into());
    }
    if has("sil open font license") {
        return Some("OFL-1.1".into());
    }
    if has("this is free and unencumbered software released into the public domain") {
        return Some("Unlicense".into());
    }
    if has("permission is hereby granted, free of charge") {
        return Some("MIT".into());
    }
    if has("apache license") && has("version 2.0") {
        return Some("Apache-2.0".into());
    }
    if has("altered source versions must be plainly marked") {
        return Some("Zlib".into());
    }
    if has("mozilla public license") && has("2.0") {
        return Some("MPL-2.0".into());
    }
    if has("gnu lesser general public license") {
        return Some(if has("version 3") { "LGPL-3.0-only" } else { "LGPL-2.1-only" }.into());
    }
    if has("gnu general public license") {
        return Some(if has("version 3") { "GPL-3.0-only" } else { "GPL-2.0-only" }.into());
    }
    if has("redistribution and use in source and binary forms") {
        return Some(if has("neither the name") { "BSD-3-Clause" } else { "BSD-2-Clause" }.into());
    }
    None
}

/// A licence found beside an asset, and the file that said so.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Guess {
    pub spdx: String,
    pub source: PathBuf,
}

const NOTICE_NAMES: [&str; 6] = ["license", "licence", "copying", "credits", "readme", "notice"];

/// Looks for the licence an asset travels with: a sidecar `<file>.license`
/// first, then licence/credits/readme files in its folder and each folder up
/// to (and including) `root`. The nearest folder wins.
pub fn guess_for(asset: &Path, root: &Path) -> Option<Guess> {
    if let Some(name) = asset.file_name().and_then(|n| n.to_str()) {
        let sidecar = asset.with_file_name(format!("{name}.license"));
        if let Some(g) = guess_in_file(&sidecar) {
            return Some(g);
        }
    }
    let mut dir = asset.parent();
    while let Some(d) = dir {
        if let Ok(rd) = std::fs::read_dir(d) {
            let mut files: Vec<PathBuf> = rd
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_file() && is_notice_file(p))
                .collect();
            // Licence files before credits before readmes.
            files.sort_by_key(|p| notice_rank(p));
            for f in files {
                if let Some(g) = guess_in_file(&f) {
                    return Some(g);
                }
            }
        }
        if d == root {
            break;
        }
        dir = d.parent();
        if !d.starts_with(root) {
            break;
        }
    }
    None
}

fn stem_lower(p: &Path) -> String {
    p.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_lowercase()
}

fn is_notice_file(p: &Path) -> bool {
    let s = stem_lower(p);
    NOTICE_NAMES.iter().any(|n| s == *n || s.starts_with(&format!("{n}-")) || s.starts_with(&format!("{n}_")))
}

fn notice_rank(p: &Path) -> usize {
    let s = stem_lower(p);
    NOTICE_NAMES.iter().position(|n| s.starts_with(n)).unwrap_or(NOTICE_NAMES.len())
}

fn guess_in_file(p: &Path) -> Option<Guess> {
    // Licence texts are small; never read a huge file looking for one.
    let meta = std::fs::metadata(p).ok()?;
    if !meta.is_file() || meta.len() > 512 * 1024 {
        return None;
    }
    let text = std::fs::read_to_string(p).ok()?;
    guess_from_text(&text).map(|spdx| Guess { spdx, source: p.to_path_buf() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_follow_the_licence() {
        let cc0 = Licence::from_spdx("CC0-1.0");
        assert!(cc0.allows_commercial_use() && !cc0.attribution_required);
        let nc = Licence::from_spdx("CC-BY-NC-4.0");
        assert_eq!(nc.commercial_use, Tri::No);
        let sa = Licence::from_spdx("CC-BY-SA-4.0");
        assert!(sa.share_alike && sa.attribution_required && sa.allows_commercial_use());
        assert_eq!(Licence::from_spdx("CC-BY-ND-4.0").modification, Tri::No);
        assert_eq!(Licence::from_spdx("Weird-1.0").commercial_use, Tri::Unknown);
    }

    #[test]
    fn guesses_common_texts() {
        assert_eq!(guess_from_text("SPDX-License-Identifier: MIT"), Some("MIT".into()));
        assert_eq!(
            guess_from_text("Permission is hereby granted,\nfree of charge, to any person"),
            Some("MIT".into())
        );
        assert_eq!(
            guess_from_text("Licensed under https://creativecommons.org/licenses/by-sa/4.0/ ok"),
            Some("CC-BY-SA-4.0".into())
        );
        assert_eq!(guess_from_text("CC0 1.0 Universal"), Some("CC0-1.0".into()));
        assert_eq!(
            guess_from_text("Creative Commons Attribution-NonCommercial 3.0"),
            Some("CC-BY-NC-3.0".into())
        );
        assert_eq!(guess_from_text("Apache License\nVersion 2.0, January 2004"), Some("Apache-2.0".into()));
        assert_eq!(guess_from_text("just some words"), None);
    }

    #[test]
    fn nc_is_not_mistaken_for_plain_by() {
        let id = guess_from_text("Creative Commons Attribution-NonCommercial-ShareAlike 4.0").unwrap();
        assert_eq!(id, "CC-BY-NC-SA-4.0");
        assert_eq!(Licence::from_spdx(&id).commercial_use, Tri::No);
    }

    #[test]
    fn nearest_notice_wins_and_sidecar_beats_folder() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        std::fs::create_dir_all(root.join("art/sfx")).unwrap();
        std::fs::write(root.join("LICENSE"), "Permission is hereby granted, free of charge").unwrap();
        std::fs::write(root.join("art/CREDITS.txt"), "Art is CC0 1.0 Universal").unwrap();
        std::fs::write(root.join("art/hero.png"), b"x").unwrap();
        std::fs::write(root.join("art/sfx/boom.wav"), b"x").unwrap();
        std::fs::write(root.join("art/sfx/boom.wav.license"), "SPDX-License-Identifier: CC-BY-4.0").unwrap();
        assert_eq!(guess_for(&root.join("art/hero.png"), root).unwrap().spdx, "CC0-1.0");
        assert_eq!(guess_for(&root.join("art/sfx/boom.wav"), root).unwrap().spdx, "CC-BY-4.0");
        std::fs::write(root.join("loose.png"), b"x").unwrap();
        assert_eq!(guess_for(&root.join("loose.png"), root).unwrap().spdx, "MIT");
    }

    #[test]
    fn nothing_found_is_none() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("a.png"), b"x").unwrap();
        assert!(guess_for(&d.path().join("a.png"), d.path()).is_none());
    }
}
