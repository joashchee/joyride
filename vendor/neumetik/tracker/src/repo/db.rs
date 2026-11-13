//! **umf-samples**, the repository's database: every waveform, sample
//! and instrument kept, every synth patch, every module they were found
//! in, and what each is called, filed under and tagged.
//!
//! It's text, a record at a time, so it can be read, compared and
//! mended by hand:
//!
//! ```text
//! umf-samples 1
//! repository 6f1c...        this repository's own ID
//! name My samples
//! [sample 3a9e...]          a record: its kind and ID
//! wave 77b0...
//! loop forward 0 4096
//! name kick drum            a key may come again: a list
//! source 51c2... 1          the module it was found in, and its slot
//! tag punchy
//! [umf 9d04...]             a UMF file saved here, by its path's hash
//! path /Users/me/song.umf
//! sample 3a9e...            the samples it plays
//! ```
//!
//! What's known of a sample's rights (`rights.rs`) is kept too: `free
//! yes` or `free no` when someone said, `license` and `rights` lines
//! (what was found, with where) on samples and modules, and on a module
//! the `origin` it was fetched from and its `archive`.
//!
//! Values escape `\`, line ends and carriage returns as `\\`, `\n`
//! and `\r`. Keys it doesn't know are kept as they were, so a newer
//! Neumetik's records survive an older one's merge.

use std::collections::{BTreeMap, BTreeSet};

use super::canon::Id;
use crate::model::{Loop, LoopKind};

/// Where a sample or instrument was found: a module and its slot (1-based).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Source {
    pub module: Id,
    pub slot: u16,
}

/// What every record shares: when it came, and what people say of it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Catalog {
    pub categories: Vec<String>,
    pub tags: Vec<String>,
    /// 0 (none) to 5.
    pub rating: u8,
    pub notes: String,
    /// Seconds since 1970.
    pub added: u64,
    /// Keys this version doesn't know, kept for whoever does.
    pub extra: Vec<(String, String)>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WaveRec {
    pub id: Id,
    pub bits: u8,
    pub channels: u8,
    pub frames: u64,
    /// Bytes its file takes in the repository.
    pub stored: u64,
    pub peak: f32,
    pub rms: f32,
    pub crossings: f32,
    /// Its period in frames, 0 when it has no clear pitch.
    pub period: f32,
    pub added: u64,
    /// Its fingerprint (`analysis::fingerprint`), for finding samples
    /// that sound like it; empty until it's worked out.
    pub spectrum: Vec<u8>,
    pub extra: Vec<(String, String)>,
}

impl WaveRec {
    /// Bytes at its own depth.
    pub fn raw_bytes(&self) -> u64 {
        self.frames * u64::from(self.channels) * if self.bits <= 8 { 1 } else { 2 }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SampleRec {
    pub id: Id,
    pub wave: Id,
    pub looped: Loop,
    pub sustain: Loop,
    /// Every name it's had, the first first.
    pub names: Vec<String>,
    pub filenames: Vec<String>,
    /// The rates it's been tuned to play middle C at.
    pub rates: Vec<u32>,
    pub volume: u8,
    pub pan: Option<u8>,
    /// The category it was guessed to be, before anyone said.
    pub guessed: String,
    pub sources: Vec<Source>,
    pub catalog: Catalog,
    /// Free to use, or not, as someone marked it.
    pub free: Option<bool>,
    /// Its license, as someone gave it or its own file said.
    pub license: String,
    /// What was found of its rights, each with where ("its credit: (c) 1995 X").
    pub rights: Vec<String>,
}

impl SampleRec {
    pub fn name(&self) -> &str {
        self.names.iter().find(|n| !n.trim().is_empty()).map_or("", |n| n.as_str())
    }

    /// Its categories, or the one guessed when nobody has said.
    pub fn categories(&self) -> Vec<&str> {
        if self.catalog.categories.is_empty() {
            vec![self.guessed.as_str()]
        } else {
            self.catalog.categories.iter().map(|s| s.as_str()).collect()
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct InstrumentRec {
    pub id: Id,
    pub names: Vec<String>,
    pub filenames: Vec<String>,
    /// The samples its keymap plays.
    pub samples: Vec<Id>,
    /// Its keymap in words: "C-0 to B-9: 3 samples".
    pub keymap: String,
    /// The envelopes it has: volume, panning, pitch, filter.
    pub envelopes: Vec<String>,
    pub fadeout: u32,
    pub sources: Vec<Source>,
    pub catalog: Catalog,
}

/// A synth patch (`patch.rs`): its kind, its bytes' size, the samples
/// it plays, and where it was found.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PatchRec {
    pub id: Id,
    /// The synth or format that plays it: `opl`, `med`, `ahx`, `kt`...
    pub kind: String,
    /// The format's name, as the module it was first found in gave it.
    pub format_name: String,
    pub names: Vec<String>,
    /// The samples it plays from.
    pub samples: Vec<Id>,
    /// Its bytes, and what its file takes in the repository.
    pub bytes: u64,
    pub stored: u64,
    pub sources: Vec<Source>,
    pub catalog: Catalog,
}

impl PatchRec {
    pub fn name(&self) -> &str {
        self.names.iter().find(|n| !n.trim().is_empty()).map_or("", |n| n.as_str())
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModuleRec {
    pub id: Id,
    /// The names its file has had.
    pub files: Vec<String>,
    /// Where it's been found.
    pub paths: Vec<String>,
    pub format: String,
    pub format_name: String,
    pub tracker: String,
    pub title: String,
    pub artist: String,
    pub message: String,
    /// From its file's SAUCE record: the artist's group, the date
    /// (`YYYY-MM-DD` when it is one) and the comment lines.
    pub group: String,
    pub date: String,
    pub comments: Vec<String>,
    pub bytes: u64,
    pub channels: u16,
    pub patterns: u16,
    pub orders: u16,
    /// What its samples took in it, all told.
    pub sample_bytes: u64,
    pub samples: Vec<(u16, Id)>,
    pub instruments: Vec<(u16, Id)>,
    /// UMF files saved from it.
    pub umfs: Vec<String>,
    pub scanned: u64,
    /// Its license: as the archive it came from gave it, else as found in it.
    pub license: String,
    /// The lines in it, or its `.meta`, that state a license or copyright, with where.
    pub rights: Vec<String>,
    /// Where it was fetched from (URLs), and the archive's name.
    pub origins: Vec<String>,
    pub archive: String,
    /// How long it plays once through, in seconds (0: not worked out).
    pub seconds: f32,
    /// Its tags as guessed (`tagging.rs`), apart from anyone's, and the
    /// rules' version that guessed them.
    pub guessed: Vec<String>,
    pub tagged: u32,
    pub extra: Vec<(String, String)>,
}

/// A UMF file saved from here, and the samples it plays, so a sample's
/// users can be found (and pointed at another when it's taken out).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UmfRec {
    /// The hash of its path.
    pub id: Id,
    pub path: String,
    pub samples: Vec<Id>,
    pub saved: u64,
}

impl ModuleRec {
    pub fn file(&self) -> &str {
        self.files.first().map_or("", |s| s.as_str())
    }
}

/// The whole database.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Db {
    /// This repository's own ID (random, made once), its name, when made.
    pub repository: String,
    pub name: String,
    pub created: u64,
    /// The repositories merged into this one.
    pub merged: Vec<String>,
    pub waves: BTreeMap<Id, WaveRec>,
    pub samples: BTreeMap<Id, SampleRec>,
    pub instruments: BTreeMap<Id, InstrumentRec>,
    pub patches: BTreeMap<Id, PatchRec>,
    pub modules: BTreeMap<Id, ModuleRec>,
    pub umfs: BTreeMap<Id, UmfRec>,
}

fn escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            c => o.push(c),
        }
    }
    o
}

fn unescape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.next() {
                Some('n') => o.push('\n'),
                Some('r') => o.push('\r'),
                Some(c) => o.push(c),
                None => o.push('\\'),
            }
        } else {
            o.push(c);
        }
    }
    o
}

fn unhex(s: &str) -> Vec<u8> {
    let b = s.trim().as_bytes();
    b.chunks_exact(2).filter_map(|p| u8::from_str_radix(std::str::from_utf8(p).ok()?, 16).ok()).collect()
}

fn loop_text(l: &Loop) -> String {
    match l.kind {
        LoopKind::Off => "off".into(),
        LoopKind::Forward => format!("forward {} {}", l.start, l.end),
        LoopKind::PingPong => format!("pingpong {} {}", l.start, l.end),
    }
}

fn parse_loop(v: &str) -> Loop {
    let mut p = v.split_whitespace();
    let kind = match p.next() {
        Some("forward") => LoopKind::Forward,
        Some("pingpong") => LoopKind::PingPong,
        _ => return Loop::default(),
    };
    let start = p.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let end = p.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    Loop { kind, start, end }
}

struct Out(String);

impl Out {
    fn kv(&mut self, k: &str, v: impl std::fmt::Display) {
        let v = v.to_string();
        self.0.push_str(k);
        self.0.push(' ');
        self.0.push_str(&escape(&v));
        self.0.push('\n');
    }

    fn opt(&mut self, k: &str, v: &str) {
        if !v.is_empty() {
            self.kv(k, v);
        }
    }

    fn list(&mut self, k: &str, v: &[String]) {
        for s in v {
            self.kv(k, s);
        }
    }

    fn catalog(&mut self, c: &Catalog) {
        self.list("category", &c.categories);
        self.list("tag", &c.tags);
        if c.rating > 0 {
            self.kv("rating", c.rating);
        }
        self.opt("notes", &c.notes);
        self.kv("added", c.added);
        for (k, v) in &c.extra {
            self.kv(k, v);
        }
    }

    fn sources(&mut self, s: &[Source]) {
        for s in s {
            self.0.push_str(&format!("source {} {}\n", s.module, s.slot));
        }
    }
}

/// Adds `v` to `list` unless it's there (or empty).
pub fn add_unique<T: PartialEq + Clone>(list: &mut Vec<T>, v: &T) -> bool {
    if list.contains(v) {
        false
    } else {
        list.push(v.clone());
        true
    }
}

fn add_text(list: &mut Vec<String>, v: &str) {
    let v = v.trim();
    if !v.is_empty() && !list.iter().any(|s| s == v) {
        list.push(v.to_string());
    }
}

impl Db {
    pub fn write(&self) -> String {
        let mut o = Out(String::with_capacity(256 + self.samples.len() * 400));
        o.0.push_str("umf-samples 1\n");
        o.kv("repository", &self.repository);
        o.opt("name", &self.name);
        o.kv("created", self.created);
        for m in &self.merged {
            o.kv("merged", m);
        }
        for w in self.waves.values() {
            o.0.push_str(&format!("\n[wave {}]\n", w.id));
            o.kv("bits", w.bits);
            o.kv("channels", w.channels);
            o.kv("frames", w.frames);
            o.kv("stored", w.stored);
            o.kv("peak", w.peak);
            o.kv("rms", w.rms);
            o.kv("crossings", w.crossings);
            o.kv("period", w.period);
            o.kv("added", w.added);
            if !w.spectrum.is_empty() {
                o.kv("spectrum", crate::hash::hex(&w.spectrum));
            }
            for (k, v) in &w.extra {
                o.kv(k, v);
            }
        }
        for s in self.samples.values() {
            o.0.push_str(&format!("\n[sample {}]\n", s.id));
            o.kv("wave", s.wave);
            o.kv("loop", loop_text(&s.looped));
            o.kv("sustain", loop_text(&s.sustain));
            o.list("name", &s.names);
            o.list("filename", &s.filenames);
            for r in &s.rates {
                o.kv("rate", r);
            }
            o.kv("volume", s.volume);
            if let Some(p) = s.pan {
                o.kv("pan", p);
            }
            o.opt("guessed", &s.guessed);
            o.sources(&s.sources);
            if let Some(f) = s.free {
                o.kv("free", if f { "yes" } else { "no" });
            }
            o.opt("license", &s.license);
            o.list("rights", &s.rights);
            o.catalog(&s.catalog);
        }
        for i in self.instruments.values() {
            o.0.push_str(&format!("\n[instrument {}]\n", i.id));
            o.list("name", &i.names);
            o.list("filename", &i.filenames);
            for s in &i.samples {
                o.kv("sample", s);
            }
            o.opt("keymap", &i.keymap);
            o.list("envelope", &i.envelopes);
            o.kv("fadeout", i.fadeout);
            o.sources(&i.sources);
            o.catalog(&i.catalog);
        }
        for p in self.patches.values() {
            o.0.push_str(&format!("\n[patch {}]\n", p.id));
            o.kv("kind", &p.kind);
            o.opt("format-name", &p.format_name);
            o.list("name", &p.names);
            for s in &p.samples {
                o.kv("sample", s);
            }
            o.kv("bytes", p.bytes);
            o.kv("stored", p.stored);
            o.sources(&p.sources);
            o.catalog(&p.catalog);
        }
        for m in self.modules.values() {
            o.0.push_str(&format!("\n[module {}]\n", m.id));
            o.list("file", &m.files);
            o.list("path", &m.paths);
            o.kv("format", &m.format);
            o.opt("format-name", &m.format_name);
            o.opt("tracker", &m.tracker);
            o.opt("title", &m.title);
            o.opt("artist", &m.artist);
            o.opt("message", &m.message);
            o.opt("group", &m.group);
            o.opt("date", &m.date);
            o.list("comment", &m.comments);
            o.kv("bytes", m.bytes);
            o.kv("channels", m.channels);
            o.kv("patterns", m.patterns);
            o.kv("orders", m.orders);
            o.kv("sample-bytes", m.sample_bytes);
            for (slot, id) in &m.samples {
                o.0.push_str(&format!("sample {slot} {id}\n"));
            }
            for (slot, id) in &m.instruments {
                o.0.push_str(&format!("instrument {slot} {id}\n"));
            }
            o.list("umf", &m.umfs);
            o.kv("scanned", m.scanned);
            o.opt("license", &m.license);
            o.list("rights", &m.rights);
            o.list("origin", &m.origins);
            o.opt("archive", &m.archive);
            if m.seconds > 0.0 {
                o.kv("seconds", m.seconds);
            }
            o.list("guessed", &m.guessed);
            if m.tagged > 0 {
                o.kv("tagged", m.tagged);
            }
            for (k, v) in &m.extra {
                o.kv(k, v);
            }
        }
        for u in self.umfs.values() {
            o.0.push_str(&format!("\n[umf {}]\n", u.id));
            o.kv("path", &u.path);
            for s in &u.samples {
                o.kv("sample", s);
            }
            o.kv("saved", u.saved);
        }
        o.0
    }

    /// Reads the database's text. Lines it can't make out are skipped;
    /// a record with a bad ID is left out.
    pub fn read(text: &str) -> Result<Db, String> {
        let mut lines = text.lines();
        match lines.next() {
            Some(l) if l.starts_with("umf-samples ") => {}
            _ => return Err("it isn't a umf-samples database".into()),
        }
        let mut db = Db::default();
        enum Rec {
            None,
            Wave(WaveRec),
            Sample(SampleRec),
            Instrument(InstrumentRec),
            Patch(PatchRec),
            Module(ModuleRec),
            Umf(UmfRec),
        }
        let mut cur = Rec::None;
        let finish = |db: &mut Db, cur: Rec| match cur {
            Rec::Wave(w) => {
                db.waves.insert(w.id, w);
            }
            Rec::Sample(s) => {
                db.samples.insert(s.id, s);
            }
            Rec::Instrument(i) => {
                db.instruments.insert(i.id, i);
            }
            Rec::Patch(p) => {
                db.patches.insert(p.id, p);
            }
            Rec::Module(m) => {
                db.modules.insert(m.id, m);
            }
            Rec::Umf(u) => {
                db.umfs.insert(u.id, u);
            }
            Rec::None => {}
        };
        for line in lines {
            let line = line.trim_end_matches('\r');
            if line.is_empty() {
                continue;
            }
            if let Some(head) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                finish(&mut db, std::mem::replace(&mut cur, Rec::None));
                let (kind, id) = head.split_once(' ').unwrap_or((head, ""));
                let Ok(id) = id.parse::<Id>() else { continue };
                cur = match kind {
                    "wave" => Rec::Wave(WaveRec { id, ..Default::default() }),
                    "sample" => Rec::Sample(SampleRec { id, ..Default::default() }),
                    "instrument" => Rec::Instrument(InstrumentRec { id, ..Default::default() }),
                    "patch" => Rec::Patch(PatchRec { id, ..Default::default() }),
                    "module" => Rec::Module(ModuleRec { id, ..Default::default() }),
                    "umf" => Rec::Umf(UmfRec { id, ..Default::default() }),
                    _ => Rec::None,
                };
                continue;
            }
            let (k, raw) = line.split_once(' ').unwrap_or((line, ""));
            let v = unescape(raw);
            let num = |v: &str| v.trim().parse::<u64>().unwrap_or(0);
            let flt = |v: &str| v.trim().parse::<f32>().unwrap_or(0.0);
            let source = |v: &str| -> Option<Source> {
                let (m, s) = v.split_once(' ')?;
                Some(Source { module: m.parse().ok()?, slot: s.trim().parse().ok()? })
            };
            let catalog = |c: &mut Catalog, k: &str, v: &str| match k {
                "category" => add_text(&mut c.categories, v),
                "tag" => add_text(&mut c.tags, v),
                "rating" => c.rating = num(v).min(5) as u8,
                "notes" => c.notes = v.to_string(),
                "added" => c.added = num(v),
                _ => c.extra.push((k.to_string(), v.to_string())),
            };
            match &mut cur {
                Rec::None => match k {
                    "repository" => db.repository = v,
                    "name" => db.name = v,
                    "created" => db.created = num(&v),
                    "merged" => add_text(&mut db.merged, &v),
                    _ => {}
                },
                Rec::Wave(w) => match k {
                    "bits" => w.bits = num(&v) as u8,
                    "channels" => w.channels = num(&v) as u8,
                    "frames" => w.frames = num(&v),
                    "stored" => w.stored = num(&v),
                    "peak" => w.peak = flt(&v),
                    "rms" => w.rms = flt(&v),
                    "crossings" => w.crossings = flt(&v),
                    "period" => w.period = flt(&v),
                    "added" => w.added = num(&v),
                    "spectrum" => w.spectrum = unhex(&v),
                    _ => w.extra.push((k.to_string(), v)),
                },
                Rec::Sample(s) => match k {
                    "wave" => s.wave = v.parse().unwrap_or_default(),
                    "loop" => s.looped = parse_loop(&v),
                    "sustain" => s.sustain = parse_loop(&v),
                    "name" => s.names.push(v),
                    "filename" => add_text(&mut s.filenames, &v),
                    "rate" => {
                        let r = num(&v) as u32;
                        add_unique(&mut s.rates, &r);
                    }
                    "volume" => s.volume = num(&v).min(64) as u8,
                    "pan" => s.pan = Some(num(&v).min(255) as u8),
                    "guessed" => s.guessed = v,
                    "free" => s.free = Some(v.trim() == "yes"),
                    "license" => s.license = v,
                    "rights" => add_text(&mut s.rights, &v),
                    "source" => {
                        if let Some(src) = source(&v) {
                            add_unique(&mut s.sources, &src);
                        }
                    }
                    _ => catalog(&mut s.catalog, k, &v),
                },
                Rec::Instrument(i) => match k {
                    "name" => i.names.push(v),
                    "filename" => add_text(&mut i.filenames, &v),
                    "sample" => {
                        if let Ok(id) = v.parse() {
                            i.samples.push(id);
                        }
                    }
                    "keymap" => i.keymap = v,
                    "envelope" => add_text(&mut i.envelopes, &v),
                    "fadeout" => i.fadeout = num(&v) as u32,
                    "source" => {
                        if let Some(src) = source(&v) {
                            add_unique(&mut i.sources, &src);
                        }
                    }
                    _ => catalog(&mut i.catalog, k, &v),
                },
                Rec::Patch(p) => match k {
                    "kind" => p.kind = v,
                    "format-name" => p.format_name = v,
                    "name" => p.names.push(v),
                    "sample" => {
                        if let Ok(id) = v.parse() {
                            p.samples.push(id);
                        }
                    }
                    "bytes" => p.bytes = num(&v),
                    "stored" => p.stored = num(&v),
                    "source" => {
                        if let Some(src) = source(&v) {
                            add_unique(&mut p.sources, &src);
                        }
                    }
                    _ => catalog(&mut p.catalog, k, &v),
                },
                Rec::Module(m) => match k {
                    "file" => add_text(&mut m.files, &v),
                    "path" => add_text(&mut m.paths, &v),
                    "format" => m.format = v,
                    "format-name" => m.format_name = v,
                    "tracker" => m.tracker = v,
                    "title" => m.title = v,
                    "artist" => m.artist = v,
                    "message" => m.message = v,
                    "group" => m.group = v,
                    "date" => m.date = v,
                    "comment" => m.comments.push(v),
                    "bytes" => m.bytes = num(&v),
                    "channels" => m.channels = num(&v) as u16,
                    "patterns" => m.patterns = num(&v) as u16,
                    "orders" => m.orders = num(&v) as u16,
                    "sample-bytes" => m.sample_bytes = num(&v),
                    "sample" | "instrument" => {
                        if let Some((slot, id)) = v.split_once(' ').and_then(|(s, i)| Some((s.parse::<u16>().ok()?, i.parse::<Id>().ok()?))) {
                            if k == "sample" {
                                m.samples.push((slot, id));
                            } else {
                                m.instruments.push((slot, id));
                            }
                        }
                    }
                    "umf" => add_text(&mut m.umfs, &v),
                    "scanned" => m.scanned = num(&v),
                    "license" => m.license = v,
                    "rights" => add_text(&mut m.rights, &v),
                    "origin" => add_text(&mut m.origins, &v),
                    "archive" => m.archive = v,
                    "seconds" => m.seconds = v.trim().parse().unwrap_or(0.0),
                    "guessed" => add_text(&mut m.guessed, &v),
                    "tagged" => m.tagged = num(&v) as u32,
                    _ => m.extra.push((k.to_string(), v)),
                },
                Rec::Umf(u) => match k {
                    "path" => u.path = v,
                    "sample" => {
                        if let Ok(id) = v.parse() {
                            add_unique(&mut u.samples, &id);
                        }
                    }
                    "saved" => u.saved = num(&v),
                    _ => {}
                },
            }
        }
        finish(&mut db, cur);
        Ok(db)
    }

    /// Takes in another repository's database: records new here are
    /// added, records both have are joined (names, tags, categories,
    /// sources and notes from both; the earlier date; the higher
    /// rating). Nothing here is lost.
    pub fn merge(&mut self, other: &Db) -> MergeReport {
        let mut r = MergeReport::default();
        if !other.repository.is_empty() && other.repository != self.repository {
            add_text(&mut self.merged, &other.repository);
        }
        for m in &other.merged {
            if *m != self.repository {
                add_text(&mut self.merged, m);
            }
        }
        for (id, w) in &other.waves {
            match self.waves.get_mut(id) {
                None => {
                    self.waves.insert(*id, w.clone());
                    r.waves += 1;
                }
                Some(mine) => {
                    if mine.spectrum.is_empty() {
                        mine.spectrum = w.spectrum.clone();
                    }
                }
            }
        }
        for (id, u) in &other.umfs {
            let mine = self.umfs.entry(*id).or_insert_with(|| UmfRec { id: *id, path: u.path.clone(), ..Default::default() });
            for s in &u.samples {
                add_unique(&mut mine.samples, s);
            }
            mine.saved = mine.saved.max(u.saved);
        }
        for (id, s) in &other.samples {
            match self.samples.get_mut(id) {
                None => {
                    self.samples.insert(*id, s.clone());
                    r.samples += 1;
                }
                Some(mine) => {
                    for n in &s.names {
                        add_text(&mut mine.names, n);
                    }
                    for f in &s.filenames {
                        add_text(&mut mine.filenames, f);
                    }
                    for x in &s.rates {
                        add_unique(&mut mine.rates, x);
                    }
                    for x in &s.sources {
                        add_unique(&mut mine.sources, x);
                    }
                    let rights = (mine.free, mine.license.clone(), mine.rights.len());
                    // Marked both ways, it's not free until someone looks again.
                    mine.free = match (mine.free, s.free) {
                        (Some(a), Some(b)) => Some(a && b),
                        (a, b) => a.or(b),
                    };
                    if mine.license.is_empty() {
                        mine.license = s.license.clone();
                    }
                    for x in &s.rights {
                        add_text(&mut mine.rights, x);
                    }
                    let rights_changed = rights != (mine.free, mine.license.clone(), mine.rights.len());
                    if merge_catalog(&mut mine.catalog, &s.catalog) || rights_changed {
                        r.joined += 1;
                    }
                }
            }
        }
        for (id, i) in &other.instruments {
            match self.instruments.get_mut(id) {
                None => {
                    self.instruments.insert(*id, i.clone());
                    r.instruments += 1;
                }
                Some(mine) => {
                    for n in &i.names {
                        add_text(&mut mine.names, n);
                    }
                    for x in &i.sources {
                        add_unique(&mut mine.sources, x);
                    }
                    merge_catalog(&mut mine.catalog, &i.catalog);
                }
            }
        }
        for (id, p) in &other.patches {
            match self.patches.get_mut(id) {
                None => {
                    self.patches.insert(*id, p.clone());
                    r.patches += 1;
                }
                Some(mine) => {
                    for n in &p.names {
                        add_text(&mut mine.names, n);
                    }
                    for x in &p.sources {
                        add_unique(&mut mine.sources, x);
                    }
                    if mine.format_name.is_empty() {
                        mine.format_name = p.format_name.clone();
                    }
                    merge_catalog(&mut mine.catalog, &p.catalog);
                }
            }
        }
        for (id, m) in &other.modules {
            match self.modules.get_mut(id) {
                None => {
                    self.modules.insert(*id, m.clone());
                    r.modules += 1;
                }
                Some(mine) => {
                    for f in &m.files {
                        add_text(&mut mine.files, f);
                    }
                    for p in &m.paths {
                        add_text(&mut mine.paths, p);
                    }
                    for u in &m.umfs {
                        add_text(&mut mine.umfs, u);
                    }
                    if mine.license.is_empty() {
                        mine.license = m.license.clone();
                    }
                    if mine.archive.is_empty() {
                        mine.archive = m.archive.clone();
                    }
                    for x in &m.rights {
                        add_text(&mut mine.rights, x);
                    }
                    for x in &m.origins {
                        add_text(&mut mine.origins, x);
                    }
                    if mine.seconds == 0.0 {
                        mine.seconds = m.seconds;
                    }
                    // What both say of it may tell more: guessed again.
                    mine.tagged = 0;
                }
            }
        }
        r
    }

    /// Every tag used, and how often, most used first.
    pub fn tags(&self) -> Vec<(String, usize)> {
        let mut count: BTreeMap<&str, usize> = BTreeMap::new();
        for s in self.samples.values() {
            for t in &s.catalog.tags {
                *count.entry(t.as_str()).or_default() += 1;
            }
        }
        let mut v: Vec<(String, usize)> = count.into_iter().map(|(k, n)| (k.to_string(), n)).collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        v
    }

    /// Every tag guessed for the modules (`tagging.rs`), and how often,
    /// by namespace and then most used first.
    pub fn module_tags(&self) -> Vec<(String, usize)> {
        let mut count: BTreeMap<&str, usize> = BTreeMap::new();
        for m in self.modules.values() {
            for t in &m.guessed {
                *count.entry(t.as_str()).or_default() += 1;
            }
        }
        let ns = |t: &str| t.split_once(':').map_or(t, |(n, _)| n).to_string();
        let mut v: Vec<(String, usize)> = count.into_iter().map(|(k, n)| (k.to_string(), n)).collect();
        v.sort_by(|a, b| ns(&a.0).cmp(&ns(&b.0)).then(b.1.cmp(&a.1)).then(a.0.cmp(&b.0)));
        v
    }

    /// Guesses each module's tags again whose were guessed by older rules
    /// (or never), or every module's when `all`. Returns the modules changed.
    pub fn retag_modules(&mut self, all: bool) -> usize {
        let stale: Vec<Id> = self.modules.values().filter(|m| all || m.tagged < super::tagging::VERSION).map(|m| m.id).collect();
        let mut n = 0;
        for id in stale {
            let tags = super::tagging::module_tags(self, &self.modules[&id]);
            let m = self.modules.get_mut(&id).expect("listed");
            m.tagged = super::tagging::VERSION;
            if m.guessed != tags {
                m.guessed = tags;
                n += 1;
            }
        }
        n
    }

    /// Every category used (the guessed included), and how often.
    pub fn categories(&self) -> Vec<(String, usize)> {
        let mut count: BTreeMap<&str, usize> = BTreeMap::new();
        for s in self.samples.values() {
            for c in s.categories() {
                *count.entry(c).or_default() += 1;
            }
        }
        count.into_iter().map(|(k, n)| (k.to_string(), n)).collect()
    }

    /// A tag renamed on every sample (merged into one that's there).
    /// Returns the samples changed.
    pub fn rename_tag(&mut self, from: &str, to: &str) -> usize {
        rename_in(self.samples.values_mut().map(|s| &mut s.catalog.tags), from, to)
    }

    /// A category renamed on every sample (merged into one that's there).
    pub fn rename_category(&mut self, from: &str, to: &str) -> usize {
        rename_in(self.samples.values_mut().map(|s| &mut s.catalog.categories), from, to)
    }

    /// The UMF files that play a sample.
    pub fn umfs_using(&self, sample: &Id) -> Vec<&UmfRec> {
        self.umfs.values().filter(|u| u.samples.contains(sample)).collect()
    }

    /// The modules a sample was found in, as records.
    pub fn sources_of(&self, sample: &SampleRec) -> Vec<(&ModuleRec, u16)> {
        sample.sources.iter().filter_map(|s| self.modules.get(&s.module).map(|m| (m, s.slot))).collect()
    }

    /// The waveforms no sample uses.
    pub fn orphan_waves(&self) -> Vec<Id> {
        let used: BTreeSet<Id> = self.samples.values().map(|s| s.wave).collect();
        self.waves.keys().filter(|w| !used.contains(w)).copied().collect()
    }
}

/// `from` made `to` in each list (or taken out where `to` is there
/// already, or empty). Returns the lists changed.
fn rename_in<'a>(lists: impl Iterator<Item = &'a mut Vec<String>>, from: &str, to: &str) -> usize {
    let to = to.trim();
    let mut n = 0;
    for list in lists {
        let Some(i) = list.iter().position(|t| t == from) else { continue };
        if to.is_empty() || list.iter().any(|t| t == to) {
            list.remove(i);
        } else {
            list[i] = to.to_string();
        }
        n += 1;
    }
    n
}

fn merge_catalog(mine: &mut Catalog, theirs: &Catalog) -> bool {
    let before = mine.clone();
    for c in &theirs.categories {
        add_text(&mut mine.categories, c);
    }
    for t in &theirs.tags {
        add_text(&mut mine.tags, t);
    }
    mine.rating = mine.rating.max(theirs.rating);
    let theirs_notes = theirs.notes.trim();
    if !theirs_notes.is_empty() && !mine.notes.contains(theirs_notes) {
        mine.notes = if mine.notes.trim().is_empty() { theirs_notes.to_string() } else { format!("{}\n\n{}", mine.notes.trim_end(), theirs_notes) };
    }
    if theirs.added != 0 && (mine.added == 0 || theirs.added < mine.added) {
        mine.added = theirs.added;
    }
    for e in &theirs.extra {
        add_unique(&mut mine.extra, e);
    }
    *mine != before
}

/// What a merge brought in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MergeReport {
    pub waves: usize,
    pub samples: usize,
    pub instruments: usize,
    pub patches: usize,
    pub modules: usize,
    /// Samples both had, with something new said of them.
    pub joined: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::canon::module_id;

    fn sample(n: u8, name: &str) -> SampleRec {
        SampleRec {
            id: module_id(&[n]),
            wave: module_id(&[n, n]),
            looped: Loop::forward(4, 400),
            names: vec![name.into()],
            volume: 64,
            sources: vec![Source { module: module_id(b"m"), slot: u16::from(n) }],
            catalog: Catalog { tags: vec!["punchy".into()], notes: "line one\nline two \\ slash".into(), added: 100, ..Default::default() },
            ..Default::default()
        }
    }

    #[test]
    fn the_text_reads_back_as_it_was() {
        let mut db = Db { repository: "abc".into(), name: "Mine".into(), created: 5, ..Default::default() };
        let mut s = sample(1, "kick [1]");
        s.free = Some(true);
        s.license = "CC0".into();
        s.rights = vec!["its credit: (c) 1995 X".into()];
        db.samples.insert(s.id, s);
        db.waves.insert(module_id(b"w"), WaveRec { id: module_id(b"w"), bits: 8, channels: 1, frames: 1000, peak: 0.5, ..Default::default() });
        db.modules.insert(module_id(b"m"), ModuleRec { id: module_id(b"m"), files: vec!["song.mod".into()], format: "mod".into(), message: "hi\r\nthere".into(), samples: vec![(1, module_id(&[1]))], license: "CC BY 4.0".into(), rights: vec!["message: (c) me".into()], origins: vec!["https://x/song.mod".into()], archive: "X".into(), ..Default::default() });
        let back = Db::read(&db.write()).unwrap();
        assert_eq!(back, db);
    }

    #[test]
    fn a_merge_joins_what_both_have_and_loses_nothing() {
        let mut a = Db { repository: "a".into(), ..Default::default() };
        let mut b = Db { repository: "b".into(), ..Default::default() };
        let s = sample(1, "kick");
        a.samples.insert(s.id, s.clone());
        let mut s2 = s.clone();
        s2.names = vec!["bd".into()];
        s2.sources = vec![Source { module: module_id(b"n"), slot: 3 }];
        s2.catalog.tags = vec!["909".into()];
        s2.catalog.rating = 4;
        s2.catalog.added = 50;
        b.samples.insert(s2.id, s2);
        let other = sample(2, "snare");
        b.samples.insert(other.id, other);
        let r = a.merge(&b);
        assert_eq!((r.samples, r.joined), (1, 1));
        let m = &a.samples[&s.id];
        assert_eq!(m.names, vec!["kick", "bd"]);
        assert_eq!(m.sources.len(), 2);
        assert_eq!(m.catalog.tags, vec!["punchy", "909"]);
        assert_eq!((m.catalog.rating, m.catalog.added), (4, 50));
        assert_eq!(a.merged, vec!["b"]);
        // Merging again changes nothing.
        let before = a.clone();
        a.merge(&b);
        assert_eq!(a, before);
    }
}
