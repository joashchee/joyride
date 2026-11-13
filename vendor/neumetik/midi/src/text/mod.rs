// Copyright 2026 ansiapps. Neumetik MIDI, MIT licensed: see LICENSE.

//! **Music written as text**, read into a [`Score`] and played through
//! [`Score::to_midi`]: MusicXML, ChordPro, ABC, MML for General MIDI (ten
//! dialects), FlMML's MML, Strudel/Tidal mini-notation, RTTTL, Humdrum
//! `**kern` and Alda. Each was written from its published description. Pure and
//! unit-tested.
//!
//! ```
//! use neumetik_midi::text;
//!
//! let (score, title) = text::read(b"Scale:d=8,o=5,b=120:c,d,e,f,g,a,b,c6", Some("rtttl"))?;
//! assert_eq!(title, "Scale");
//! assert_eq!(score.parts[0].notes.len(), 8);
//! let midi = score.to_midi();
//! # let _ = midi;
//! # Ok::<(), neumetik_midi::Error>(())
//! ```
//!
//! Reading is exact to what the format says (a quarter note is a quarter
//! note, a triplet a third of two); writing ([`write`]: MusicXML, ABC and
//! `**kern` on the shared [`notation`], and RTTTL, MML, mini-notation and
//! Alda) is approximate where a format holds less than a score does.
//!
//! What every reader shares:
//!
//! - [`Builder`] places notes in fractions of a whole note ([`Frac`]) and
//!   makes them ticks at a PPQ that every one of them divides into (480,
//!   or a multiple of it when a tuplet asks), so nothing is rounded.
//! - Tempo and meter changes, key signatures, section markers and chord
//!   symbols go on the score as the MIDI meta events that hold them
//!   (tempo, time signature, key signature, marker, text), lyrics on
//!   their part (lyric), so a MIDI file of the music keeps them.
//! - [`program`] finds a General MIDI program by an instrument's name in
//!   any of these formats.
//! - An error is [`Error::BadText`], with its line, column and why.

pub mod abc;
pub mod alda;
pub mod chordpro;
pub mod flmml;
pub mod sion;
pub mod inflate;
pub mod kern;
pub mod mini;
pub mod mml;
pub mod musicxml;
pub mod notation;
pub mod rhythm;
pub mod rtttl;
pub mod sakura;
mod names;
pub mod xml;

pub use names::{program, GM_NAMES};

use crate::score::{Kept, Note, Part, Score, DRUMS};
use crate::Error;

/// A fraction: a time or a length in whole notes (a quarter note is 1/4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Frac {
    n: i64,
    d: i64,
}

fn gcd(mut a: i64, mut b: i64) -> i64 {
    a = a.abs();
    b = b.abs();
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a.max(1)
}

// Named like the operators, and used as them: `a.add(b)`.
#[allow(clippy::should_implement_trait)]
impl Frac {
    pub const ZERO: Frac = Frac { n: 0, d: 1 };
    /// `n / d`, made as small as it goes. A zero `d` is taken as 1.
    pub fn new(n: i64, d: i64) -> Frac {
        let d = if d == 0 { 1 } else { d };
        let g = gcd(n, d);
        let s = if d < 0 { -1 } else { 1 };
        Frac { n: s * n / g, d: s * d / g }
    }
    pub fn int(n: i64) -> Frac {
        Frac { n, d: 1 }
    }
    pub fn num(self) -> i64 {
        self.n
    }
    pub fn den(self) -> i64 {
        self.d
    }
    pub fn mul(self, o: Frac) -> Frac {
        let (g1, g2) = (gcd(self.n, o.d), gcd(o.n, self.d));
        Frac::new((self.n / g1).saturating_mul(o.n / g2), (self.d / g2).saturating_mul(o.d / g1))
    }
    pub fn div(self, o: Frac) -> Frac {
        if o.n == 0 {
            return Frac::ZERO;
        }
        self.mul(Frac::new(o.d, o.n))
    }
    pub fn add(self, o: Frac) -> Frac {
        let g = gcd(self.d, o.d);
        Frac::new(self.n.saturating_mul(o.d / g).saturating_add(o.n.saturating_mul(self.d / g)), (self.d / g).saturating_mul(o.d))
    }
    pub fn sub(self, o: Frac) -> Frac {
        self.add(Frac { n: -o.n, d: o.d })
    }
    pub fn scale(self, n: i64, d: i64) -> Frac {
        self.mul(Frac::new(n, d))
    }
    pub fn max(self, o: Frac) -> Frac {
        if self >= o {
            self
        } else {
            o
        }
    }
    pub fn min(self, o: Frac) -> Frac {
        if self <= o {
            self
        } else {
            o
        }
    }
    pub fn f64(self) -> f64 {
        self.n as f64 / self.d as f64
    }
    pub fn is_positive(self) -> bool {
        self.n > 0
    }
}

impl Default for Frac {
    fn default() -> Frac {
        Frac::ZERO
    }
}

impl PartialOrd<Frac> for Frac {
    fn partial_cmp(&self, o: &Frac) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Frac {
    fn cmp(&self, o: &Frac) -> std::cmp::Ordering {
        (i128::from(self.n) * i128::from(o.d)).cmp(&(i128::from(o.n) * i128::from(self.d)))
    }
}

/// A part as it's built: notes and events in whole notes.
#[derive(Debug, Clone, Default)]
pub struct BuildPart {
    pub name: String,
    pub drums: bool,
    pub bank: u8,
    pub program: u8,
    /// Its notes: when, how long, the key and velocity.
    pub notes: Vec<(Frac, Frac, u8, u8)>,
    pub lyrics: Vec<(Frac, String)>,
    /// Later program changes.
    pub programs: Vec<(Frac, u8)>,
    /// Controllers: when, the number, the value.
    pub controls: Vec<(Frac, u8, u8)>,
    /// A channel asked for (0 to 15), else one is given.
    pub channel: Option<u8>,
}

/// A score being built from text, every time in whole notes.
#[derive(Debug, Clone, Default)]
pub struct Builder {
    pub title: String,
    pub parts: Vec<BuildPart>,
    /// Tempo changes: when and quarter notes a minute.
    pub tempos: Vec<(Frac, f64)>,
    /// Meter changes: when, beats, unit.
    pub meters: Vec<(Frac, u8, u8)>,
    /// Key signatures: when, sharps (flats negative), minor.
    pub keys: Vec<(Frac, i8, bool)>,
    pub markers: Vec<(Frac, String)>,
    pub chords: Vec<(Frac, String)>,
    /// The music's end, if it's past the last note.
    pub end: Frac,
}

/// The most notes one text makes: far past any song, short of a trap.
pub const MOST_NOTES: usize = 1 << 20;

impl Builder {
    pub fn new() -> Builder {
        Builder::default()
    }

    /// A new part, its index.
    pub fn part(&mut self, name: &str, program: u8, drums: bool) -> usize {
        self.parts.push(BuildPart { name: name.into(), program: program & 0x7f, drums, ..Default::default() });
        self.parts.len() - 1
    }

    /// The notes in every part so far.
    pub fn count(&self) -> usize {
        self.parts.iter().map(|p| p.notes.len()).sum()
    }

    /// A note, if it's long enough to hear and the key is in range.
    pub fn note(&mut self, part: usize, at: Frac, length: Frac, key: i32, velocity: u8) {
        if !length.is_positive() || !(0..=127).contains(&key) || at < Frac::ZERO || self.count() >= MOST_NOTES {
            return;
        }
        if let Some(p) = self.parts.get_mut(part) {
            p.notes.push((at, length, key as u8, velocity.clamp(1, 127)));
        }
    }

    pub fn tempo(&mut self, at: Frac, bpm: f64) {
        if bpm.is_finite() && bpm > 0.0 {
            self.tempos.retain(|t| t.0 != at);
            self.tempos.push((at, bpm.clamp(10.0, 1000.0)));
        }
    }

    pub fn meter(&mut self, at: Frac, beats: u8, unit: u8) {
        if beats > 0 && unit.is_power_of_two() && unit <= 32 {
            self.meters.retain(|m| m.0 != at);
            self.meters.push((at, beats, unit));
        }
    }

    pub fn key(&mut self, at: Frac, sharps: i8, minor: bool) {
        self.keys.retain(|k| k.0 != at);
        self.keys.push((at, sharps.clamp(-7, 7), minor));
    }

    pub fn marker(&mut self, at: Frac, text: &str) {
        self.markers.push((at, text.into()));
    }

    pub fn chord(&mut self, at: Frac, text: &str) {
        self.chords.push((at, text.into()));
    }

    pub fn lyric(&mut self, part: usize, at: Frac, text: &str) {
        if let Some(p) = self.parts.get_mut(part) {
            p.lyrics.push((at, text.into()));
        }
    }

    pub fn program_at(&mut self, part: usize, at: Frac, program: u8) {
        if let Some(p) = self.parts.get_mut(part) {
            if at == Frac::ZERO && p.notes.iter().all(|n| n.0 > Frac::ZERO) && p.programs.is_empty() {
                p.program = program & 0x7f;
            } else {
                p.programs.push((at, program & 0x7f));
            }
        }
    }

    pub fn control(&mut self, part: usize, at: Frac, number: u8, value: u8) {
        if let Some(p) = self.parts.get_mut(part) {
            p.controls.push((at, number & 0x7f, value & 0x7f));
        }
    }

    /// Makes the music at least this long.
    pub fn end_at(&mut self, at: Frac) {
        self.end = self.end.max(at);
    }

    /// Every time in the builder, to find a PPQ for.
    fn times(&self) -> impl Iterator<Item = Frac> + '_ {
        let parts = self.parts.iter().flat_map(|p| {
            p.notes.iter().flat_map(|n| [n.0, n.1]).chain(p.lyrics.iter().map(|l| l.0)).chain(p.programs.iter().map(|x| x.0)).chain(p.controls.iter().map(|c| c.0))
        });
        parts
            .chain(self.tempos.iter().map(|t| t.0))
            .chain(self.meters.iter().map(|m| m.0))
            .chain(self.keys.iter().map(|k| k.0))
            .chain(self.markers.iter().map(|m| m.0))
            .chain(self.chords.iter().map(|c| c.0))
            .chain([self.end])
    }

    /// The ticks a quarter note that every time divides into: 480, or a
    /// multiple of it, at most 30720; past that, 480 and times rounded.
    pub fn ppq(&self) -> u16 {
        let mut ppq: i64 = 480;
        for t in self.times() {
            // ticks = t * 4 * ppq: an integer when t.den / gcd(t.den, 4) divides ppq.
            let need = t.d / gcd(t.d, 4);
            ppq = ppq / gcd(ppq, need) * need;
            if ppq > 30720 {
                return 480;
            }
        }
        ppq as u16
    }

    /// The score.
    pub fn finish(mut self) -> Score {
        let ppq = self.ppq();
        let ticks = |t: Frac| -> u32 { (t.mul(Frac::int(4 * i64::from(ppq))).f64().round()).clamp(0.0, f64::from(u32::MAX >> 1)) as u32 };
        let mut score = Score { ppq, bpm: 120.0, beats: 4, unit: 4, bars: 1, parts: vec![], kept: vec![] };
        self.tempos.sort_by_key(|t| t.0);
        self.meters.sort_by_key(|m| m.0);
        self.keys.sort_by_key(|k| k.0);
        for (i, &(at, bpm)) in self.tempos.iter().enumerate() {
            if i == 0 && at == Frac::ZERO {
                score.bpm = (bpm * 100.0).round() / 100.0;
            } else {
                let micros = (60_000_000.0 / bpm).round() as u32;
                score.kept.push(Kept { at: ticks(at), bytes: meta(0x51, &micros.to_be_bytes()[1..]) });
            }
        }
        for (i, &(at, beats, unit)) in self.meters.iter().enumerate() {
            if i == 0 && at == Frac::ZERO {
                (score.beats, score.unit) = (beats, unit);
            } else {
                score.kept.push(Kept { at: ticks(at), bytes: meta(0x58, &[beats, unit.trailing_zeros() as u8, 24, 8]) });
            }
        }
        for &(at, sharps, minor) in &self.keys {
            score.kept.push(Kept { at: ticks(at), bytes: meta(0x59, &[sharps as u8, u8::from(minor)]) });
        }
        for (at, text) in &self.markers {
            score.kept.push(Kept { at: ticks(*at), bytes: meta(0x06, text.as_bytes()) });
        }
        for (at, text) in &self.chords {
            score.kept.push(Kept { at: ticks(*at), bytes: meta(0x01, text.as_bytes()) });
        }
        score.kept.sort_by_key(|k| k.at);
        let mut end = ticks(self.end);
        // Channels: asked for, else the next free one past the drums.
        let mut next = 0u8;
        for p in &self.parts {
            let channel = match (p.drums, p.channel) {
                (true, _) => DRUMS,
                (false, Some(c)) => c & 0x0f,
                (false, None) => {
                    let c = next;
                    next = (next + 1) % 16;
                    if next == DRUMS {
                        next += 1;
                    }
                    if c == DRUMS {
                        c + 1
                    } else {
                        c
                    }
                }
            };
            let mut notes: Vec<Note> = p
                .notes
                .iter()
                .map(|&(at, len, key, velocity)| {
                    let from = ticks(at);
                    Note { at: from, length: ticks(at.add(len)).saturating_sub(from).max(1), key, velocity }
                })
                .collect();
            notes.sort_by_key(|n| (n.at, n.key));
            end = end.max(notes.iter().map(|n| n.at + n.length).max().unwrap_or(0));
            let ch = channel;
            let mut kept: Vec<Kept> = vec![];
            kept.extend(p.programs.iter().map(|&(at, prog)| Kept { at: ticks(at), bytes: vec![0xc0 | ch, prog] }));
            kept.extend(p.controls.iter().map(|&(at, num, val)| Kept { at: ticks(at), bytes: vec![0xb0 | ch, num, val] }));
            kept.extend(p.lyrics.iter().map(|(at, text)| Kept { at: ticks(*at), bytes: meta(0x05, text.as_bytes()) }));
            kept.sort_by_key(|k| k.at);
            if notes.is_empty() && kept.is_empty() {
                continue;
            }
            score.parts.push(Part { name: p.name.clone(), channel, bank: p.bank, program: p.program, notes, kept });
        }
        let bar = u64::from(score.bar_ticks());
        score.bars = u64::from(end).div_ceil(bar).clamp(1, 9999) as u32;
        score
    }
}

pub(crate) fn meta(kind: u8, data: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0xff, kind];
    crate::score::vlq(data.len() as u32, &mut bytes);
    bytes.extend_from_slice(data);
    bytes
}

/// An error at a byte of the text: its line and column (in characters).
pub fn bad(text: &str, offset: usize, why: &'static str) -> Error {
    let offset = offset.min(text.len());
    let mut at = offset;
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    let before = &text[..at];
    let line = before.matches('\n').count() as u32 + 1;
    let start = before.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let column = before[start..].chars().count() as u32 + 1;
    Error::BadText { line, column, why }
}

/// A key by its letter (`c` to `b`, either case) in octave 4 (60 is
/// middle C), before sharps or flats.
pub fn letter_key(letter: char) -> Option<i32> {
    Some(match letter.to_ascii_lowercase() {
        'c' => 60,
        'd' => 62,
        'e' => 64,
        'f' => 65,
        'g' => 67,
        'a' => 69,
        'b' => 71,
        _ => return None,
    })
}

/// The sharps (flats negative) of a key signature named like `F#`, `Bb`,
/// `Eb minor`, `Dm`, `A dorian`, and whether it's minor. Modes are taken
/// as their signature's major or minor.
pub fn key_signature(name: &str) -> Option<(i8, bool)> {
    let name = name.trim();
    let mut chars = name.chars();
    let tonic = chars.next()?.to_ascii_uppercase();
    let rest: String = chars.collect();
    let (accidental, mode) = if let Some(r) = rest.strip_prefix('#') {
        (1, r)
    } else if let Some(r) = rest.strip_prefix('b') {
        (-1, r)
    } else {
        (0, rest.as_str())
    };
    // The major key's sharps by tonic.
    let base: i32 = match tonic {
        'C' => 0,
        'G' => 1,
        'D' => 2,
        'A' => 3,
        'E' => 4,
        'B' => 5,
        'F' => -1,
        _ => return None,
    } + 7 * accidental;
    let mode = mode.trim().to_ascii_lowercase();
    let m3: String = mode.chars().take(3).collect();
    // Each mode's sharps from its tonic's major.
    let (shift, minor) = match m3.as_str() {
        "" | "maj" | "ion" => (0, false),
        "m" | "min" | "aeo" => (-3, true),
        "mix" => (-1, false),
        "dor" => (-2, false),
        "phr" => (-4, false),
        "lyd" => (1, false),
        "loc" => (-5, false),
        _ if mode.starts_with('m') && mode.len() == 1 => (-3, true),
        _ => return None,
    };
    let sharps = base + shift;
    if !(-7..=7).contains(&sharps) {
        // An enharmonic spelling past seven: wrap by twelve.
        let s = if sharps > 7 { sharps - 12 } else { sharps + 12 };
        return Some((s as i8, minor));
    }
    Some((sharps as i8, minor))
}

/// Each letter's accidental (in semitones) under a key signature.
pub fn signature_accidentals(sharps: i8) -> [i32; 7] {
    // c d e f g a b, by index.
    const SHARP_ORDER: [usize; 7] = [3, 0, 4, 1, 5, 2, 6]; // F C G D A E B
    const FLAT_ORDER: [usize; 7] = [6, 2, 5, 1, 4, 0, 3]; // B E A D G C F
    let mut acc = [0; 7];
    if sharps > 0 {
        for &i in SHARP_ORDER.iter().take(sharps as usize) {
            acc[i] = 1;
        }
    } else {
        for &i in FLAT_ORDER.iter().take((-sharps) as usize) {
            acc[i] = -1;
        }
    }
    acc
}

/// A letter's index, `c` 0 to `b` 6.
pub fn letter_index(letter: char) -> Option<usize> {
    "cdefgab".find(letter.to_ascii_lowercase())
}

/// Which format a text is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    MusicXml,
    /// Compressed MusicXML: a zip holding it.
    Mxl,
    ChordPro,
    Abc,
    Mml(mml::Dialect),
    /// FlMML's MML.
    FlMml,
    SiOn,
    Mini,
    Rtttl,
    Kern,
    Alda,
    Tmf,
}

/// The file names each format is saved under.
pub const EXTENSIONS: &[(&str, &str)] = &[
    ("musicxml", "MusicXML"),
    ("xml", "MusicXML"),
    ("mxl", "MusicXML (compressed)"),
    ("cho", "ChordPro"),
    ("chordpro", "ChordPro"),
    ("chopro", "ChordPro"),
    ("crd", "ChordPro"),
    ("pro", "ChordPro"),
    ("abc", "ABC"),
    ("mml", "MML"),
    ("ms2mml", "MapleStory 2 MML"),
    ("flmml", "FlMML"),
    ("sion", "SiON"),
    ("bas", "BASIC PLAY"),
    ("mini", "Strudel mini-notation"),
    ("strudel", "Strudel mini-notation"),
    ("tidal", "Tidal mini-notation"),
    ("rtttl", "RTTTL"),
    ("rtx", "RTTTL"),
    ("txt", "text"),
    ("krn", "Humdrum **kern"),
    ("alda", "Alda"),
    ("tmf", "TMF"),
];

/// Which format a text is, from its file name's extension (without the
/// dot) when it says, else by what's in it.
pub fn detect(bytes: &[u8], extension: Option<&str>) -> Option<Format> {
    let ext = extension.map(|e| e.trim_start_matches('.').to_ascii_lowercase());
    if bytes.starts_with(b"PK\x03\x04") {
        return Some(Format::Mxl);
    }
    let text = String::from_utf8_lossy(bytes);
    let t = text.trim_start_matches('\u{feff}').trim_start();
    match ext.as_deref() {
        Some("musicxml") => return Some(Format::MusicXml),
        Some("mxl") => return Some(Format::Mxl),
        Some("cho" | "chordpro" | "chopro" | "crd" | "pro") => return Some(Format::ChordPro),
        Some("abc") => return Some(Format::Abc),
        Some("mini" | "strudel" | "tidal") => return Some(Format::Mini),
        Some("rtttl" | "rtx") => return Some(Format::Rtttl),
        Some("krn") => return Some(Format::Kern),
        Some("alda") => return Some(Format::Alda),
        Some("tmf") => return Some(Format::Tmf),
        Some("ms2mml") => return Some(Format::Mml(mml::Dialect::MapleStory2)),
        Some("flmml") => return Some(Format::FlMml),
        Some("sion") => return Some(Format::SiOn),
        Some("mml") if sion::looks_like(t) => return Some(Format::SiOn),
        Some("mml") if flmml::looks_like(t) => return Some(Format::FlMml),
        Some("mml" | "bas") => return Some(Format::Mml(mml::detect(t))),
        _ => {}
    }
    if t.starts_with("<?xml") || t.starts_with("<score-partwise") || t.starts_with("<score-timewise") || t.starts_with("<!DOCTYPE score") {
        if t.contains("<ms2>") || t.contains("<ms2 ") {
            return Some(Format::Mml(mml::Dialect::MapleStory2));
        }
        return Some(Format::MusicXml);
    }
    if crate::tmf::starts_in(t) {
        return Some(Format::Tmf);
    }
    if t.starts_with("**kern") || t.lines().next().is_some_and(|l| l.split('\t').any(|s| s == "**kern")) || t.starts_with("!!!") && t.contains("**kern") {
        return Some(Format::Kern);
    }
    if t.lines().any(|l| l.trim_start().starts_with("X:")) && t.lines().any(|l| l.trim_start().starts_with("K:")) {
        return Some(Format::Abc);
    }
    if t.contains("MML@") {
        return Some(Format::Mml(mml::Dialect::Mabinogi));
    }
    if sion::looks_like(t) {
        return Some(Format::SiOn);
    }
    if flmml::looks_like(t) {
        return Some(Format::FlMml);
    }
    if t.lines().any(|l| {
        let l = l.trim_start();
        l.starts_with("{title") || l.starts_with("{t:") || l.starts_with("{start_of_chorus") || l.starts_with("{soc")
    }) {
        return Some(Format::ChordPro);
    }
    if rtttl::looks_like(t) {
        return Some(Format::Rtttl);
    }
    if sakura::looks_like(t) {
        return Some(Format::Mml(mml::Dialect::Sakura));
    }
    let upper = t.to_ascii_uppercase();
    if upper.contains("PLAY") {
        return Some(Format::Mml(mml::detect(t)));
    }
    if alda::looks_like(t) {
        return Some(Format::Alda);
    }
    if chordpro::looks_like(t) {
        return Some(Format::ChordPro);
    }
    if ext.as_deref() == Some("txt") && !t.is_empty() {
        return Some(Format::Mini);
    }
    None
}

/// Text music as a score and a title, its format found by [`detect`].
pub fn read(bytes: &[u8], extension: Option<&str>) -> Result<(Score, String), Error> {
    let format = detect(bytes, extension).ok_or(Error::BadText { line: 1, column: 1, why: "it isn't music written in a format Neumetik reads" })?;
    read_as(bytes, format)
}

/// Text music in a given format, as a score and a title.
pub fn read_as(bytes: &[u8], format: Format) -> Result<(Score, String), Error> {
    if format == Format::Mxl {
        let xml = musicxml::unzip(bytes)?;
        return musicxml::read(&xml);
    }
    let text = String::from_utf8_lossy(bytes);
    let text = text.trim_start_matches('\u{feff}');
    match format {
        Format::MusicXml => musicxml::read(text),
        Format::ChordPro => chordpro::read(text),
        Format::Abc => abc::read(text),
        Format::Mml(d) => mml::read(text, d),
        Format::FlMml => flmml::read(text),
        Format::SiOn => sion::read(text),
        Format::Mini => mini::read(text),
        Format::Rtttl => rtttl::read(text),
        Format::Kern => kern::read(text),
        Format::Alda => alda::read(text),
        Format::Tmf => crate::tmf::read(text),
        Format::Mxl => unreachable!(),
    }
}

/// The formats [`write`] makes, by file extension, and their names.
pub const WRITES: &[(&str, &str)] = &[
    ("musicxml", "MusicXML"),
    ("abc", "ABC"),
    ("krn", "Humdrum **kern"),
    ("cho", "ChordPro"),
    ("mml", "MML"),
    ("alda", "Alda"),
    ("mini", "Strudel mini-notation"),
    ("rtttl", "RTTTL"),
];

/// A score written as text in the format a file extension names (see
/// [`WRITES`]; `xml`, `strudel`, `tidal` and `rtx` too), titled where
/// the format has a title; `None` for a format that isn't written.
pub fn write(score: &Score, title: &str, extension: &str) -> Option<String> {
    Some(match extension.trim_start_matches('.').to_ascii_lowercase().as_str() {
        "musicxml" | "xml" => musicxml::write(score, title),
        "abc" => abc::write(score, title),
        "krn" => kern::write(score, title),
        "cho" | "chordpro" | "chopro" | "crd" | "pro" => chordpro::write(score, title),
        "mml" => mml::write(score, mml::Dialect::Generic),
        "alda" => alda::write(score),
        "mini" | "strudel" | "tidal" => mini::write(score),
        "rtttl" | "rtx" => rtttl::write(score, if title.is_empty() { "Song" } else { title }),
        _ => return None,
    })
}

/// Text music as a MIDI file, by [`read`].
pub fn to_midi(bytes: &[u8], extension: Option<&str>) -> Result<Vec<u8>, Error> {
    read(bytes, extension).map(|(s, _)| s.to_midi())
}

/// How two scores' notes compare, for checking a reader against a
/// reference's MIDI file: notes matched by onset (in quarter notes, to a
/// 64th note), key and length (to a 64th note), whatever their part.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Comparison {
    pub matched: usize,
    /// In ours, not the reference: (onset in quarter notes, key, length).
    pub extra: Vec<(f64, u8, f64)>,
    /// In the reference, not ours.
    pub missing: Vec<(f64, u8, f64)>,
    /// Matched by onset and key, but a different length.
    pub lengths: usize,
}

/// Compares our notes with a reference's, in quarter notes (so tempo
/// doesn't matter). A drum part's notes are compared as drums, by key.
pub fn compare(ours: &Score, reference: &Score) -> Comparison {
    let flatten = |s: &Score| {
        let mut v: Vec<(i64, u8, i64, bool)> = s
            .parts
            .iter()
            .flat_map(|p| {
                let q = f64::from(s.ppq);
                p.notes.iter().map(move |n| ((f64::from(n.at) / q * 16.0).round() as i64, n.key, (f64::from(n.length) / q * 16.0).round() as i64, p.channel == DRUMS))
            })
            .collect();
        v.sort();
        v
    };
    let (a, mut b) = (flatten(ours), flatten(reference));
    let mut c = Comparison::default();
    let mut used = vec![false; b.len()];
    for n in &a {
        let exact = b.iter().enumerate().position(|(i, m)| !used[i] && m.0 == n.0 && m.1 == n.1 && m.3 == n.3 && (m.2 - n.2).abs() <= 1);
        let loose = || b.iter().enumerate().position(|(i, m)| !used[i] && m.0 == n.0 && m.1 == n.1 && m.3 == n.3);
        match exact.or_else(loose) {
            Some(i) => {
                if (b[i].2 - n.2).abs() > 1 {
                    c.lengths += 1;
                }
                used[i] = true;
                c.matched += 1;
            }
            None => c.extra.push((n.0 as f64 / 16.0, n.1, n.2 as f64 / 16.0)),
        }
    }
    for (i, m) in b.drain(..).enumerate() {
        if !used[i] {
            c.missing.push((m.0 as f64 / 16.0, m.1, m.2 as f64 / 16.0));
        }
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fractions() {
        let third = Frac::new(1, 12);
        assert_eq!(third.add(third).add(third), Frac::new(1, 4));
        assert_eq!(Frac::new(2, -4), Frac::new(-1, 2));
        assert!(Frac::new(1, 3) > Frac::new(1, 4));
        assert_eq!(Frac::new(3, 8).div(Frac::new(1, 8)), Frac::int(3));
    }

    #[test]
    fn a_ppq_every_time_divides() {
        let mut b = Builder::new();
        let p = b.part("Piano", 0, false);
        b.note(p, Frac::ZERO, Frac::new(1, 4), 60, 100);
        assert_eq!(b.ppq(), 480);
        // A quintuplet sixteenth: 1/80 of a whole note.
        b.note(p, Frac::new(1, 80), Frac::new(1, 80), 62, 100);
        assert_eq!(b.ppq(), 480);
        b.note(p, Frac::new(1, 7), Frac::new(1, 28), 64, 100);
        assert_eq!(b.ppq(), 3360);
        let score = b.finish();
        assert_eq!(score.ppq, 3360);
        assert_eq!(score.parts[0].notes[1], Note { at: 168, length: 168, key: 62, velocity: 100 });
        assert_eq!(score.parts[0].notes[2].at, 4 * 3360 / 7);
    }

    #[test]
    fn events_become_meta_events() {
        let mut b = Builder::new();
        let p = b.part("Voice", 52, false);
        b.tempo(Frac::ZERO, 90.0);
        b.tempo(Frac::int(1), 60.0);
        b.meter(Frac::ZERO, 3, 4);
        b.key(Frac::ZERO, -2, false);
        b.marker(Frac::ZERO, "A");
        b.chord(Frac::ZERO, "Bb");
        b.note(p, Frac::ZERO, Frac::new(3, 4), 70, 80);
        b.lyric(p, Frac::ZERO, "Hel-");
        let score = b.finish();
        assert_eq!((score.bpm, score.beats, score.unit, score.bars), (90.0, 3, 4, 1));
        assert!(score.kept.iter().any(|k| k.bytes == meta(0x51, &1_000_000u32.to_be_bytes()[1..]) && k.at == 1920));
        assert!(score.kept.iter().any(|k| k.bytes == vec![0xff, 0x59, 2, 0xfe, 0]));
        assert!(score.parts[0].kept.iter().any(|k| k.bytes == meta(0x05, b"Hel-")));
        // And a MIDI file keeps them.
        let again = Score::from_midi(&score.to_midi()).unwrap();
        assert_eq!(again.parts[0].notes, score.parts[0].notes);
        assert!(again.kept.iter().any(|k| k.bytes == meta(0x06, b"A")));
    }

    #[test]
    fn errors_say_where() {
        let e = bad("one\ntwo three", 8, "a wrong note");
        assert_eq!(e, Error::BadText { line: 2, column: 5, why: "a wrong note" });
        assert_eq!(e.to_string(), "line 2, column 5: a wrong note");
        assert_eq!(e.code(), -12);
    }

    #[test]
    fn key_signatures() {
        assert_eq!(key_signature("G"), Some((1, false)));
        assert_eq!(key_signature("Bb"), Some((-2, false)));
        assert_eq!(key_signature("F#m"), Some((3, true)));
        assert_eq!(key_signature("Ebmin"), Some((-6, true)));
        assert_eq!(key_signature("D dorian"), Some((0, false)));
        assert_eq!(key_signature("A mix"), Some((2, false)));
        assert_eq!(key_signature("C#"), Some((7, false)));
        assert_eq!(signature_accidentals(2), [1, 0, 0, 1, 0, 0, 0]);
        assert_eq!(signature_accidentals(-1), [0, 0, 0, 0, 0, 0, -1]);
    }

    #[test]
    fn compares_notes() {
        let mut b = Builder::new();
        let p = b.part("", 0, false);
        b.note(p, Frac::ZERO, Frac::new(1, 4), 60, 100);
        b.note(p, Frac::new(1, 4), Frac::new(1, 4), 62, 100);
        let ours = b.clone().finish();
        b.parts[0].notes[1].2 = 64;
        let c = compare(&ours, &b.finish());
        assert_eq!((c.matched, c.extra.len(), c.missing.len()), (1, 1, 1));
    }
}
