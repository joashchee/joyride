//! **Modland's index**, and one new module of each format picked from
//! it: for fetching a module of every format Neumetik reads, to check
//! each against what it's had before (`docs/tracker-sources.md`).
//!
//! Modland (`ftp.modland.com/pub/modules/`) files modules by format, a
//! folder each ("Protracker", "Fasttracker 2", "Startrekker AM"...),
//! then by artist. Its index, `pub/documents/allmods_md5.zip`, lists
//! every file's MD5 and path, a line each: `<md5> <path>`. Nothing here
//! touches the network; whoever fetches gives it the index's text and
//! fetches what it picks.

use std::collections::BTreeMap;

/// Where Modland's modules are, and its index.
pub const BASE: &str = "https://ftp.modland.com/pub/modules/";
pub const INDEX: &str = "https://ftp.modland.com/pub/documents/allmods_md5.zip";

/// Folders whose files share an extension with formats read here but
/// are something else: Deflemask's `.dmf` and FamiTracker's `.ftm` are
/// chip trackers' songs, with no samples to read; HVSC's `.sid` are the
/// C64's, not SidMon's.
const NOT_READ: &[&str] = &["Deflemask", "FamiTracker", "HVSC"];

/// Folders read whatever their files' names: SoundSmith's songs have
/// no extension, the Magnetic Fields Packer's are `mfp.<name>`.
const READ_ANY_NAME: &[&str] = &["SoundSmith", "Magnetic Fields Packer"];

/// A file's companions: what a module of some formats keeps beside it
/// (StarTrekker AM's `.nt` or `.as` instruments, SoundSmith's DOC RAM
/// `.w`, `.d`, `.doc` or `.wb`, Ken's `waves.kwv`, the Magnetic Fields
/// Packer's `smp.<name>` for `mfp.<name>`).
const COMPANION_EXTS: &[&str] = &["nt", "as", "w", "d", "doc", "wb"];
const COMPANION_NAMES: &[&str] = &["waves.kwv", "doc.data"];

/// A file in the index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// Its path under `pub/modules/`: format folder, artist, file.
    pub path: String,
    pub md5: String,
}

impl Entry {
    /// Its format folder.
    pub fn folder(&self) -> &str {
        self.path.split('/').next().unwrap_or("")
    }

    /// Its file's name.
    pub fn name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }

    /// The artist's folder, when it's filed under one ("- unknown" isn't).
    pub fn artist(&self) -> &str {
        let parts: Vec<&str> = self.path.split('/').collect();
        match parts.get(1) {
            Some(a) if parts.len() > 2 && !a.trim_start_matches(['-', '_', ' ']).eq_ignore_ascii_case("unknown") => a,
            _ => "",
        }
    }

    /// Its folder: the path without the file's name.
    pub fn dir(&self) -> &str {
        self.path.rsplit_once('/').map_or("", |(d, _)| d)
    }

    pub fn url(&self) -> String {
        format!("{BASE}{}", encode_path(&self.path))
    }
}

/// A path for a URL: each byte but letters, digits, `-._~` and `/`
/// as `%XX` (of its UTF-8).
pub fn encode_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len() + 16);
    for b in path.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~' | b'/') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The index's lines, read: `<md5> <path>`. Its text is Latin-1.
pub fn read_index(bytes: &[u8]) -> Vec<Entry> {
    let text: String = bytes.iter().map(|&b| b as char).collect();
    text.lines()
        .filter_map(|l| {
            let (md5, path) = l.trim_end_matches('\r').split_once(' ')?;
            (md5.len() == 32 && md5.bytes().all(|b| b.is_ascii_hexdigit()) && path.contains('/')).then(|| Entry { path: path.to_string(), md5: md5.to_ascii_lowercase() })
        })
        .collect()
}

/// A file's extension as a format's: lower case, the old Amiga prefix
/// (`mod.name`) counted as one too.
pub fn ext_of(name: &str) -> String {
    let n = name.to_lowercase();
    let suffix = n.rsplit_once('.').map_or("", |(_, e)| e).to_string();
    match n.split_once('.') {
        Some((head, _)) if matches!(head, "mod" | "med" | "okt" | "dbm" | "digi" | "xm" | "s3m" | "it" | "stk" | "ult" | "mtm") => head.to_string(),
        _ => suffix,
    }
}

/// A file's name as two copies of it would share: lower case, letters
/// and digits only, and its extension.
pub fn norm_name(name: &str) -> String {
    let n = name.rsplit(['/', '\\']).next().unwrap_or(name).to_lowercase();
    let (stem, ext) = n.rsplit_once('.').unwrap_or((&n, ""));
    let stem: String = stem.chars().filter(|c| c.is_alphanumeric()).collect();
    format!("{stem}.{ext}")
}

/// One module picked from a format's folder, and the files it needs beside it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pick {
    pub folder: String,
    pub file: Entry,
    pub companions: Vec<Entry>,
}

/// Whether a file is a module's companion rather than a module.
fn is_companion(name: &str) -> bool {
    let n = name.to_lowercase();
    COMPANION_EXTS.contains(&ext_of(name).as_str()) || COMPANION_NAMES.contains(&n.as_str()) || n.starts_with("smp.")
}

/// Whether a file in `folder` is a module read here.
fn readable(folder: &str, name: &str, exts: &[&str]) -> bool {
    !is_companion(name) && (READ_ANY_NAME.contains(&folder) || exts.contains(&ext_of(name).as_str()))
}

/// The format folders a reader of `exts` reads: those most of whose
/// modules (companions aside) have one of them. With each, how many
/// files it has.
pub fn readable_folders(entries: &[Entry], exts: &[&str]) -> BTreeMap<String, usize> {
    let mut counts: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for e in entries.iter().filter(|e| !is_companion(e.name())) {
        let c = counts.entry(e.folder()).or_default();
        c.0 += 1;
        if readable(e.folder(), e.name(), exts) {
            c.1 += 1;
        }
    }
    counts.into_iter().filter(|(f, (all, read))| *read * 2 > *all && !NOT_READ.contains(f)).map(|(f, (all, _))| (f.to_string(), all)).collect()
}

/// One module from each folder read, not `had` and not in `skip`, chosen
/// by `seed` (the same seed, the same picks); `tries` more for a folder
/// whose pick turns out to have been had, in order.
pub fn one_of_each(entries: &[Entry], exts: &[&str], had: &dyn Fn(&Entry) -> bool, skip: &[String], seed: u64, tries: usize) -> Vec<Vec<Pick>> {
    let folders = readable_folders(entries, exts);
    let mut by_folder: BTreeMap<&str, Vec<&Entry>> = BTreeMap::new();
    let mut by_dir: BTreeMap<&str, Vec<&Entry>> = BTreeMap::new();
    for e in entries {
        if folders.contains_key(e.folder()) {
            by_folder.entry(e.folder()).or_default().push(e);
            by_dir.entry(e.dir()).or_default().push(e);
        }
    }
    let mut rng = seed | 1;
    let mut next = move || {
        // xorshift64*
        rng ^= rng >> 12;
        rng ^= rng << 25;
        rng ^= rng >> 27;
        rng.wrapping_mul(0x2545_f491_4f6c_dd1d)
    };
    let mut out = Vec::new();
    for (folder, files) in by_folder {
        if skip.iter().any(|s| s == folder) {
            continue;
        }
        let mut can: Vec<&Entry> = files.into_iter().filter(|e| readable(folder, e.name(), exts) && !had(e)).collect();
        let mut picks = Vec::new();
        while picks.len() <= tries && !can.is_empty() {
            let e = can.swap_remove((next() % can.len() as u64) as usize);
            let name = e.name().to_lowercase();
            let stem = name.rsplit_once('.').map_or(name.as_str(), |(s, _)| s).to_string();
            let companions = by_dir
                .get(e.dir())
                .map(|sib| {
                    sib.iter()
                        .filter(|s| {
                            let n = s.name().to_lowercase();
                            n != name && (COMPANION_EXTS.iter().any(|x| n == format!("{name}.{x}") || n == format!("{stem}.{x}")) || COMPANION_NAMES.contains(&n.as_str()) || name.strip_prefix("mfp.").is_some_and(|rest| n == format!("smp.{rest}")))
                        })
                        .map(|s| (*s).clone())
                        .collect()
                })
                .unwrap_or_default();
            picks.push(Pick { folder: folder.to_string(), file: e.clone(), companions });
        }
        if !picks.is_empty() {
            out.push(picks);
        }
    }
    out
}

/// What a `.meta` beside a module fetched from Modland says
/// (`repo::rights::Meta`): where it came from, and its artist.
pub fn meta_for(e: &Entry) -> String {
    let mut s = format!("source {}\narchive Modland\n", e.url());
    if !e.artist().is_empty() {
        s.push_str(&format!("artist {}\n", e.artist()));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    const INDEX_TEXT: &[u8] = b"0123456789abcdef0123456789abcdef Protracker/4-Mat/aurora.mod\n\
        1123456789abcdef0123456789abcdef Protracker/4-Mat/madness.mod\n\
        2123456789abcdef0123456789abcdef Startrekker AM/Ziphoid/tune.mod\n\
        3123456789abcdef0123456789abcdef Startrekker AM/Ziphoid/tune.mod.nt\n\
        4123456789abcdef0123456789abcdef Ken's Digital Music/Ken Silverman/waves.kwv\n\
        5123456789abcdef0123456789abcdef Ken's Digital Music/Ken Silverman/intro.kdm\n\
        6123456789abcdef0123456789abcdef FamiTracker/x/y.ftm\n\
        7123456789abcdef0123456789abcdef AHX/x/y.ahx\n\
        8123456789abcdef0123456789abcdef Fasttracker 2/A \xe9/b c.xm\n";

    #[test]
    fn one_new_module_of_each_format_with_its_companions() {
        let e = read_index(INDEX_TEXT);
        assert_eq!(e.len(), 9);
        let exts = ["mod", "kdm", "ftm", "xm"];
        assert_eq!(readable_folders(&e, &exts).keys().cloned().collect::<Vec<_>>(), ["Fasttracker 2", "Ken's Digital Music", "Protracker", "Startrekker AM"]);
        let had = |x: &Entry| x.name() == "aurora.mod";
        let picks = one_of_each(&e, &exts, &had, &[], 7, 0);
        assert_eq!(picks.len(), 4);
        let by = |f: &str| picks.iter().find(|p| p[0].folder == f).unwrap()[0].clone();
        assert_eq!(by("Protracker").file.name(), "madness.mod");
        assert_eq!(by("Startrekker AM").companions[0].name(), "tune.mod.nt");
        assert_eq!(by("Ken's Digital Music").file.name(), "intro.kdm");
        assert_eq!(by("Ken's Digital Music").companions[0].name(), "waves.kwv");
        assert_eq!(by("Fasttracker 2").file.url(), "https://ftp.modland.com/pub/modules/Fasttracker%202/A%20%C3%A9/b%20c.xm");
        assert_eq!(by("Fasttracker 2").file.artist(), "A \u{e9}");
        assert!(one_of_each(&e, &exts, &had, &["Protracker".into()], 7, 0).iter().all(|p| p[0].folder != "Protracker"));
    }

    #[test]
    fn names_compare_as_copies_would() {
        assert_eq!(norm_name("Mods/Back In_Business.MOD"), "backinbusiness.mod");
        assert_eq!(ext_of("mod.Blah"), "mod");
        assert_eq!(ext_of("tune.it"), "it");
    }
}
