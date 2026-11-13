//! **Rights**: who may use a sample, as far as can be told.
//!
//! Before a song is shared with its samples in it (an `.ntt`), whoever
//! shares it should check they may pass those samples on. This gathers
//! what helps them check:
//!
//! - **What's found.** Module texts (the song's message, which in many
//!   formats is the samples' names read together; its title, artist and
//!   instruments' names; its SAUCE record's comments) and a sample's own
//!   credit (a WAV's `ICOP` and `IART`, an AIFF's or 8SVX's `(c) ` and
//!   `AUTH`, a FLAC's `COPYRIGHT`) are read for license words ("public
//!   domain", "CC0", "CC BY", "free to use", "all rights reserved"...)
//!   and copyright lines ("(c) 1994 ..."). [`find`] does it.
//! - **What the module's source said.** A file `<module>.meta` beside a
//!   module (written by whatever fetched it, from the archive it came
//!   from) is read when it's scanned: [`Meta`].
//! - **What people say.** A sample can be marked free to use, or not,
//!   by hand; that wins over anything found.
//!
//! [`freedom`] puts it together: a sample is **free to use** (the green
//! dot) when someone marked it so, or when what's found says so (its own
//! credit or license, or a module it came from offered under a free
//! license) and nothing found says otherwise. It's a help to checking,
//! not legal advice: the words found are shown with it.

use super::db::{Db, ModuleRec, SampleRec};

/// What a license lets people do with what it covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Terms {
    /// Anything: public domain, CC0, "free to use", royalty free.
    Free,
    /// Anything, crediting its author (CC BY, CC BY-SA).
    Credit,
    /// Not without asking: all rights reserved, non-commercial or
    /// no-derivatives licenses, "don't rip".
    Restricted,
}

impl Terms {
    pub fn word(self) -> &'static str {
        match self {
            Terms::Free => "free",
            Terms::Credit => "free with credit",
            Terms::Restricted => "restricted",
        }
    }

    pub fn allows_use(self) -> bool {
        self != Terms::Restricted
    }
}

/// License words and what they mean, the narrower first (a text that
/// says "CC BY-NC" mustn't be read as "CC BY").
const WORDS: &[(&str, Terms, &str)] = &[
    ("all rights reserved", Terms::Restricted, "All rights reserved"),
    ("noncommercial", Terms::Restricted, "Non-commercial use only"),
    ("non-commercial", Terms::Restricted, "Non-commercial use only"),
    ("non commercial", Terms::Restricted, "Non-commercial use only"),
    ("cc by-nc", Terms::Restricted, "CC BY-NC"),
    ("cc-by-nc", Terms::Restricted, "CC BY-NC"),
    ("by-nc", Terms::Restricted, "CC BY-NC"),
    ("cc by-nd", Terms::Restricted, "CC BY-ND"),
    ("cc-by-nd", Terms::Restricted, "CC BY-ND"),
    ("noderivatives", Terms::Restricted, "No derivatives"),
    ("no derivatives", Terms::Restricted, "No derivatives"),
    ("no derivative works", Terms::Restricted, "No derivatives"),
    ("don't rip", Terms::Restricted, "Not to be ripped"),
    ("dont rip", Terms::Restricted, "Not to be ripped"),
    ("do not rip", Terms::Restricted, "Not to be ripped"),
    ("no ripping", Terms::Restricted, "Not to be ripped"),
    ("without permission", Terms::Restricted, "Not without permission"),
    ("without my permission", Terms::Restricted, "Not without permission"),
    ("cc by-sa", Terms::Credit, "CC BY-SA"),
    ("cc-by-sa", Terms::Credit, "CC BY-SA"),
    ("attribution-sharealike", Terms::Credit, "CC BY-SA"),
    ("attribution share alike", Terms::Credit, "CC BY-SA"),
    ("cc by", Terms::Credit, "CC BY"),
    ("cc-by", Terms::Credit, "CC BY"),
    ("creative commons attribution", Terms::Credit, "CC BY"),
    ("cc0", Terms::Free, "CC0"),
    ("creative commons zero", Terms::Free, "CC0"),
    ("public domain", Terms::Free, "Public domain"),
    ("no rights reserved", Terms::Free, "No rights reserved"),
    ("unlicense", Terms::Free, "The Unlicense"),
    ("royalty free", Terms::Free, "Royalty free"),
    ("royalty-free", Terms::Free, "Royalty free"),
    ("copyright free", Terms::Free, "Copyright free"),
    ("copyright-free", Terms::Free, "Copyright free"),
    ("free to use", Terms::Free, "Free to use"),
    ("free for use", Terms::Free, "Free to use"),
    ("free for any use", Terms::Free, "Free to use"),
    ("feel free to use", Terms::Free, "Free to use"),
    ("use freely", Terms::Free, "Free to use"),
    ("free samples", Terms::Free, "Free samples"),
];

/// The license a text states, when it states one: what it allows, and
/// its name. The most restrictive found wins.
pub fn license(text: &str) -> Option<(Terms, &'static str)> {
    let t = text.to_lowercase().replace(['\n', '\r', '\t'], " ");
    let words: Vec<(Terms, &'static str)> = WORDS.iter().filter(|(w, ..)| contains_word(&t, w)).map(|&(_, terms, name)| (terms, name)).collect();
    words.iter().copied().max_by_key(|(t, _)| *t)
}

/// `w` in `t`, not inside a longer word ("cc by" in "cc by-nc" counts;
/// "cc0" in "occ0" doesn't).
fn contains_word(t: &str, w: &str) -> bool {
    let mut from = 0;
    while let Some(i) = t[from..].find(w) {
        let at = from + i;
        let before = t[..at].chars().next_back();
        let after = t[at + w.len()..].chars().next();
        let edge = |c: Option<char>| c.is_none_or(|c| !c.is_alphanumeric());
        if edge(before) && edge(after) {
            return true;
        }
        from = at + w.len().max(1);
    }
    false
}

/// What a text says of its rights: the license it states, and its
/// copyright lines.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Found {
    pub license: Option<(Terms, &'static str)>,
    /// The lines that state a license or a copyright, as they were.
    pub lines: Vec<String>,
}

/// Reads a text for its rights.
pub fn find(text: &str) -> Found {
    let mut f = Found { license: license(text), lines: Vec::new() };
    for line in text.lines() {
        let l = line.trim();
        let low = l.to_lowercase();
        let copyright = low.contains("(c)") || l.contains('©') || contains_word(&low, "copyright") || contains_word(&low, "copr.");
        if (copyright || license(l).is_some()) && !f.lines.iter().any(|x| x == l) {
            f.lines.push(l.chars().take(200).collect());
        }
    }
    // A license across lines (a MOD's sample names) is shown whole.
    if f.license.is_some() && f.lines.is_empty() {
        f.lines.push(text.replace('\n', " ").chars().take(200).collect());
    }
    f
}

/// A module's texts that may state its rights, by where they're from.
pub fn module_texts(m: &crate::model::Module) -> Vec<(&'static str, String)> {
    let mut v = vec![("title", m.title.clone()), ("artist", m.artist.clone()), ("message", m.message.clone())];
    // The samples' names: often the only place a MOD says anything.
    let names: Vec<&str> = m.samples.iter().map(|s| s.name.trim_end()).collect();
    v.push(("sample names", names.join("\n")));
    let ins: Vec<&str> = m.instruments.iter().map(|i| i.name.trim_end()).collect();
    v.push(("instrument names", ins.join("\n")));
    if let Some(s) = &m.sauce {
        v.push(("SAUCE comments", s.comments.join("\n")));
    }
    v.retain(|(_, t)| !t.trim().is_empty());
    v
}

/// What's found in a module: its license (the most restrictive stated
/// anywhere in it) and the lines that say so, each with where it is.
pub fn in_module(m: &crate::model::Module) -> (Option<(Terms, &'static str)>, Vec<String>) {
    let mut best: Option<(Terms, &'static str)> = None;
    let mut lines = Vec::new();
    for (place, text) in module_texts(m) {
        let f = find(&text);
        if let Some(l) = f.license {
            if best.is_none_or(|b| l.0 > b.0) {
                best = Some(l);
            }
        }
        // The same line in two places (a message that's the samples' names) is shown once.
        let mut seen: Vec<String> = Vec::new();
        for l in f.lines {
            if !lines.iter().any(|x: &String| x.split_once(": ").is_some_and(|(_, t)| t == l)) && !seen.contains(&l) {
                lines.push(format!("{place}: {l}"));
                seen.push(l);
            }
        }
    }
    (best, lines)
}

/// What a module's `.meta` file says: where it came from and on what
/// terms, as the archive it was fetched from gave them. A line a field:
///
/// ```text
/// source https://example.org/song.xm
/// archive The Example Archive
/// license CC BY 4.0
/// copyright (c) 1997 Somebody
/// artist Somebody
/// ```
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Meta {
    pub sources: Vec<String>,
    pub archive: String,
    pub license: String,
    pub copyright: Vec<String>,
    pub artist: String,
}

impl Meta {
    /// The `.meta` file's name for a module's.
    pub fn file_for(module_file: &str) -> String {
        format!("{module_file}.meta")
    }

    pub fn read(text: &str) -> Meta {
        let mut m = Meta::default();
        for line in text.lines() {
            let Some((k, v)) = line.trim().split_once(' ') else { continue };
            let v = v.trim().to_string();
            match k {
                "source" | "url" => m.sources.push(v),
                "archive" => m.archive = v,
                "license" | "licence" => m.license = v,
                "copyright" => m.copyright.push(v),
                "artist" => m.artist = v,
                _ => {}
            }
        }
        m
    }

    pub fn write(&self) -> String {
        let mut s = String::new();
        for u in &self.sources {
            s.push_str(&format!("source {u}\n"));
        }
        for (k, v) in [("archive", &self.archive), ("license", &self.license), ("artist", &self.artist)] {
            if !v.is_empty() {
                s.push_str(&format!("{k} {v}\n"));
            }
        }
        for c in &self.copyright {
            s.push_str(&format!("copyright {c}\n"));
        }
        s
    }
}

/// Who said a sample is (or isn't) free to use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum By {
    /// Someone marked it.
    Person,
    /// Found in its credit, or in a module it came from.
    Found,
    /// Nothing's known.
    Nobody,
}

/// A sample's rights, put together.
#[derive(Clone, Debug, PartialEq)]
pub struct Freedom {
    /// Free to use: the green dot.
    pub free: bool,
    pub by: By,
    /// The license's name, when one's known ("CC0", "Free to use",
    /// "All rights reserved"...).
    pub license: String,
    pub terms: Option<Terms>,
    /// What was found, each with where: "its credit: (c) 1995 X",
    /// "song.xm, message: Samples free to use".
    pub why: Vec<String>,
}

/// A sample's rights: marked by hand, else what's found in its own
/// credit and in the modules it came from. Something restrictive found
/// anywhere outweighs something free found elsewhere.
pub fn freedom(db: &Db, s: &SampleRec) -> Freedom {
    let mut why: Vec<String> = s.rights.clone();
    let mut terms: Option<(Terms, String)> = None;
    let mut take = |t: Terms, name: &str| {
        if terms.as_ref().is_none_or(|(b, _)| t > *b) {
            terms = Some((t, name.to_string()));
        }
    };
    if !s.license.is_empty() {
        let t = license(&s.license).map_or(Terms::Restricted, |l| l.0);
        take(t, &s.license);
    }
    for r in &s.rights {
        if let Some((t, name)) = license(r) {
            take(t, name);
        }
    }
    for src in &s.sources {
        let Some(m) = db.modules.get(&src.module) else { continue };
        if let Some((t, name)) = module_license(m) {
            take(t, &name);
            why.push(format!("{}: {name}", m.file()));
        }
        for l in &m.rights {
            let line = format!("{}, {l}", m.file());
            if !why.contains(&line) {
                why.push(line);
            }
        }
    }
    let (t, name) = terms.map_or((None, String::new()), |(t, n)| (Some(t), n));
    match s.free {
        Some(free) => Freedom { free, by: By::Person, license: name, terms: t, why },
        None => Freedom { free: t.is_some_and(Terms::allows_use), by: if t.is_some() { By::Found } else { By::Nobody }, license: name, terms: t, why },
    }
}

/// A module's license: what its source said, else what's found in it.
pub fn module_license(m: &ModuleRec) -> Option<(Terms, String)> {
    if m.license.is_empty() {
        return None;
    }
    // An unknown license's name is shown, and counted as restrictive until someone says.
    Some(license(&m.license).map_or((Terms::Restricted, m.license.clone()), |(t, _)| (t, m.license.clone())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn licenses_are_told_apart() {
        assert_eq!(license("Released under CC0."), Some((Terms::Free, "CC0")));
        assert_eq!(license("license: CC BY-NC 4.0"), Some((Terms::Restricted, "CC BY-NC")));
        assert_eq!(license("CC BY 3.0, credit me"), Some((Terms::Credit, "CC BY")));
        assert_eq!(license("samples are\nFREE TO USE\nhave fun"), Some((Terms::Free, "Free to use")));
        assert_eq!(license("(c) 1994 me, all rights reserved. Public domain otherwise"), Some((Terms::Restricted, "All rights reserved")));
        assert_eq!(license("freeware module, enjoy"), None);
        assert_eq!(license("soccer0"), None);
    }

    #[test]
    fn copyright_lines_are_found() {
        let f = find("bass\n(c) 1993 Purple Motion\nsnare\n© Someone\nCopyright 1995 X");
        assert_eq!(f.lines, vec!["(c) 1993 Purple Motion", "© Someone", "Copyright 1995 X"]);
        assert_eq!(f.license, None);
    }

    #[test]
    fn meta_reads_back() {
        let m = Meta { sources: vec!["https://x/y.xm".into()], archive: "X".into(), license: "CC BY 4.0".into(), copyright: vec!["(c) me".into()], artist: "me".into() };
        assert_eq!(Meta::read(&m.write()), m);
    }

    #[test]
    fn a_person_outweighs_whats_found_and_restrictions_outweigh_freedom() {
        use crate::repo::canon::module_id;
        use crate::repo::db::Source;
        let mut db = Db::default();
        let free = ModuleRec { id: module_id(b"a"), files: vec!["a.xm".into()], license: "CC0".into(), ..Default::default() };
        let closed = ModuleRec { id: module_id(b"b"), files: vec!["b.xm".into()], license: "All rights reserved".into(), ..Default::default() };
        db.modules.insert(free.id, free.clone());
        db.modules.insert(closed.id, closed.clone());
        let mut s = SampleRec { sources: vec![Source { module: free.id, slot: 1 }], ..Default::default() };
        let f = freedom(&db, &s);
        assert!(f.free && f.by == By::Found && f.license == "CC0");
        s.sources.push(Source { module: closed.id, slot: 2 });
        assert!(!freedom(&db, &s).free);
        s.free = Some(true);
        let f = freedom(&db, &s);
        assert!(f.free && f.by == By::Person);
        let none = freedom(&db, &SampleRec::default());
        assert!(!none.free && none.by == By::Nobody);
    }
}
