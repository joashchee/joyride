//! **Parity**: does a module play the same once it's UMF? A module read
//! and the UMF made from it (its samples taken back from the repository,
//! or from the file) are played side by side and compared sample for
//! sample, and what the song holds (orders, patterns, samples' settings,
//! instruments) is compared field by field.
//!
//! When they differ, a **bug report** ([`Bug`]) says how: where the
//! sound first parted (the time, the order and row), by how much, which
//! channels, which samples went missing, what the UMF lost; and it
//! carries the module itself (and any file it took from beside it), so
//! the report alone is enough to see the fault again and to check a fix
//! (`examples/tracker_bug.rs` reads reports and checks each again).
//!
//! A report is text (`neumetik-bug 1`, then a field a line, like the
//! repository's database), the files after it in base64:
//!
//! ```text
//! neumetik-bug 1
//! kind parity
//! version 0.2.0
//! file song.xm
//! module 51c2…            the file's SHA-256
//! first 12.48 order 3 pattern 5 row 12
//! channels 2 5
//! differs sample 3: loop
//! …
//! [file song.xm 41201]
//! base64, 76 characters a line
//! [end]
//! ```

use std::sync::Arc;

use crate::model::*;
use crate::player::{Player, Position};
use crate::repo::canon::{canonical_wave, module_id, Id};
use crate::umf::Loaded;

/// The rate the two are played at to compare them.
pub const RATE: u32 = 22_050;
/// How much of the song is compared, in seconds.
pub const MOST: f64 = 600.0;
/// The most two frames may differ by and still be the same.
pub const TOLERANCE: f32 = 1e-4;
/// The largest module a report carries (a larger one is named only).
pub const CARRY_MAX: usize = 32 << 20;

/// What the comparison found.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Parity {
    /// Frames each played: the module's and the UMF's.
    pub frames: (usize, usize),
    /// The most any frame differed by.
    pub most: f32,
    /// The first frame that differed, and where the song was then.
    pub first: Option<(usize, Position)>,
    /// The channels that sound different played on their own.
    pub channels: Vec<usize>,
    /// The samples whose waveforms weren't found: (1-based slot, waveform ID).
    pub missing: Vec<(u16, Id)>,
    /// What the UMF holds differently, in words.
    pub differs: Vec<String>,
}

impl Parity {
    /// It plays the same and holds the same.
    pub fn ok(&self) -> bool {
        self.sounds_same() && self.missing.is_empty() && self.differs.is_empty()
    }

    pub fn sounds_same(&self) -> bool {
        self.frames.0 == self.frames.1 && self.most < TOLERANCE
    }

    /// In a line, for people.
    pub fn summary(&self) -> String {
        if self.ok() {
            return "plays the same".into();
        }
        let mut parts = Vec::new();
        if self.frames.0 != self.frames.1 {
            parts.push(format!("lengths {:.2} s and {:.2} s", self.frames.0 as f64 / f64::from(RATE), self.frames.1 as f64 / f64::from(RATE)));
        }
        if let Some((f, p)) = self.first {
            parts.push(format!("sound parts at {:.2} s (order {}, row {}) by up to {:.4}", f as f64 / f64::from(RATE), p.order, p.row, self.most));
        }
        if !self.channels.is_empty() {
            parts.push(format!("channel{} {}", if self.channels.len() == 1 { "" } else { "s" }, self.channels.iter().map(|c| (c + 1).to_string()).collect::<Vec<_>>().join(", ")));
        }
        if !self.missing.is_empty() {
            parts.push(format!("{} waveform{} missing", self.missing.len(), if self.missing.len() == 1 { "" } else { "s" }));
        }
        if !self.differs.is_empty() {
            parts.push(format!("{} thing{} held differently", self.differs.len(), if self.differs.len() == 1 { "" } else { "s" }));
        }
        parts.join("; ")
    }
}

/// Plays `a` and `b` side by side (at most `most` seconds at `rate`):
/// the frames each made, the most they differed by, and the first
/// frame that differed with where `a` was in its song then.
fn race(a: Arc<Module>, b: Arc<Module>, rate: u32, most: f64) -> ((usize, usize), f32, Option<(usize, Position)>) {
    const BLOCK: usize = 256;
    let mut pa = Player::new(a, rate);
    let mut pb = Player::new(b, rate);
    pa.repeat = false;
    pb.repeat = false;
    let limit = (most * f64::from(rate)) as usize;
    let (mut la, mut ra, mut lb, mut rb) = (vec![0f32; BLOCK], vec![0f32; BLOCK], vec![0f32; BLOCK], vec![0f32; BLOCK]);
    let (mut fa, mut fb) = (0usize, 0usize);
    let (mut done_a, mut done_b) = (false, false);
    let mut worst = 0f32;
    let mut first = None;
    while (!done_a || !done_b) && fa.max(fb) < limit {
        let at = pa.position();
        for (p, l, r, done, frames) in [(&mut pa, &mut la, &mut ra, &mut done_a, &mut fa), (&mut pb, &mut lb, &mut rb, &mut done_b, &mut fb)] {
            l.fill(0.0);
            r.fill(0.0);
            if !*done {
                p.render(l, r);
                *frames += BLOCK;
                *done = p.ended();
            }
        }
        for i in 0..BLOCK {
            let d = (la[i] - lb[i]).abs().max((ra[i] - rb[i]).abs());
            if d.is_nan() || d >= TOLERANCE {
                if first.is_none() {
                    first = Some((fa.max(fb) - BLOCK + i, at));
                }
                worst = worst.max(if d.is_nan() { f32::INFINITY } else { d });
            }
        }
    }
    ((fa, fb), worst, first)
}

/// Compares a module with the UMF read back from it.
pub fn check(original: &Module, back: &Loaded) -> Parity {
    let a = Arc::new(original.clone());
    let b = Arc::new(back.module.clone());
    let (frames, most, first) = race(a.clone(), b.clone(), RATE, MOST);
    let mut p = Parity { frames, most, first, missing: back.missing.clone(), differs: differences(original, &back.module), ..Default::default() };
    // Which channels: each played alone, up to a little past where they parted.
    if let Some((f, _)) = p.first {
        let span = (f as f64 / f64::from(RATE) + 1.0).min(MOST);
        for ch in 0..original.channels.len() {
            if original.channels[ch].muted {
                continue;
            }
            let solo = |m: &Module| {
                let mut m = m.clone();
                for (i, c) in m.channels.iter_mut().enumerate() {
                    c.muted = i != ch;
                }
                Arc::new(m)
            };
            let (_, d, _) = race(solo(&a), solo(&b), RATE, span);
            if d >= TOLERANCE {
                p.channels.push(ch);
            }
        }
    }
    p
}

/// What `b` (the UMF) holds differently from `a` (the module), in words.
/// The waveforms aren't compared (the UMF keeps them cut to what's
/// heard, `repo::canon`), nor what UMF says of itself (its format).
pub fn differences(a: &Module, b: &Module) -> Vec<String> {
    let mut out = Vec::new();
    let mut field = |what: &str, same: bool| {
        if !same {
            out.push(what.to_string());
        }
    };
    field("title", a.title == b.title);
    field("artist", a.artist == b.artist);
    field("message", a.message == b.message);
    field("kind (whose effect rules)", a.kind == b.kind);
    field("flags", a.flags == b.flags);
    field("speed", a.speed == b.speed);
    field("tempo", a.tempo == b.tempo);
    field("global volume", a.global_volume == b.global_volume);
    field("mix volume", a.mix_volume == b.mix_volume);
    field("separation", a.separation == b.separation);
    field("restart", a.restart == b.restart);
    field("orders", a.orders == b.orders);
    field("tempo mode", a.tempo_mode == b.tempo_mode);
    field("swing", a.swing == b.swing);
    field("order lists", a.sequences == b.sequences);
    field("SEL scripts", a.sel == b.sel);
    if a.channels.len() != b.channels.len() {
        out.push(format!("channels: {} and {}", a.channels.len(), b.channels.len()));
    } else {
        for (i, (x, y)) in a.channels.iter().zip(&b.channels).enumerate() {
            if x != y {
                out.push(format!("channel {}", i + 1));
            }
        }
    }
    if a.patterns.len() != b.patterns.len() {
        out.push(format!("patterns: {} and {}", a.patterns.len(), b.patterns.len()));
    } else {
        for (i, (x, y)) in a.patterns.iter().zip(&b.patterns).enumerate() {
            if x != y {
                out.push(format!("pattern {i}"));
            }
        }
    }
    if a.samples.len() != b.samples.len() {
        out.push(format!("samples: {} and {}", a.samples.len(), b.samples.len()));
    } else {
        for (i, (x, y)) in a.samples.iter().zip(&b.samples).enumerate() {
            let mut what = Vec::new();
            let mut f = |w: &str, same: bool| {
                if !same {
                    what.push(w.to_string());
                }
            };
            f("name", x.name == y.name);
            f("file name", x.filename == y.filename);
            f("rate", x.c5 == y.c5);
            f("volume", x.volume == y.volume);
            f("global volume", x.global_volume == y.global_volume);
            f("panning", x.pan == y.pan);
            f("loop", x.looped == y.looped);
            f("sustain loop", x.sustain == y.sustain);
            f("auto-vibrato", x.vibrato == y.vibrato);
            f("credit", x.credit == y.credit);
            f("slices", x.slices == y.slices);
            f("FM instrument", x.fm == y.fm);
            f("synth sound", same_program(x.program.as_deref(), y.program.as_deref()));
            f("depth or channels", x.wave.bits == y.wave.bits && x.wave.channels == y.wave.channels || x.wave.is_empty() || y.wave.is_empty());
            // A waveform all silence is kept as none.
            f("waveform (missing)", canonical_wave(x).is_empty() == y.wave.is_empty());
            if !what.is_empty() {
                out.push(format!("sample {}: {}", i + 1, what.join(", ")));
            }
        }
    }
    if a.instruments.len() != b.instruments.len() {
        out.push(format!("instruments: {} and {}", a.instruments.len(), b.instruments.len()));
    } else {
        for (i, (x, y)) in a.instruments.iter().zip(&b.instruments).enumerate() {
            if x != y {
                out.push(format!("instrument {}", i + 1));
            }
        }
    }
    out
}

/// Two synth sounds the same, but for the waveforms that are the song's
/// samples (Pumatracker's first ten, an OctaMED hybrid's first): those
/// are the samples', compared with them, and kept cut to what's heard.
fn same_program(a: Option<&Program>, b: Option<&Program>) -> bool {
    let (Some(a), Some(b)) = (a, b) else { return a.is_none() && b.is_none() };
    let own = a.sources.len().max(usize::from(a.hybrid()));
    a.sound == b.sound
        && a.sources == b.sources
        && a.waves.len() == b.waves.len()
        && a.waves.iter().zip(&b.waves).skip(own).all(|(x, y)| x == y)
        && a.loops.iter().skip(own).eq(b.loops.iter().skip(own))
}

/// A bug report: what went wrong with a module, and the module.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Bug {
    /// What was being done: `parity` (a module and its UMF differ).
    pub kind: String,
    /// The fields, in order (`file`, `module`, `first`...).
    pub fields: Vec<(String, String)>,
    /// The module and the files it took from beside it: (name, bytes).
    pub files: Vec<(String, Vec<u8>)>,
}

impl Bug {
    /// A parity report for module `file` (its bytes `data`, the files
    /// it took from beside it `beside`), from where it was (`path`).
    pub fn parity(name: &str, path: Option<&str>, data: &[u8], beside: &[(String, Vec<u8>)], module: &Module, p: &Parity) -> Bug {
        let mut f: Vec<(String, String)> = Vec::new();
        let mut put = |k: &str, v: String| f.push((k.to_string(), v));
        put("version", env!("CARGO_PKG_VERSION").to_string());
        put("created", crate::repo::now().to_string());
        put("file", name.to_string());
        if let Some(path) = path {
            put("path", path.to_string());
        }
        put("module", module_id(data).to_string());
        put("bytes", data.len().to_string());
        put("format", module.format.clone());
        put("format-name", module.format_name.clone());
        if !module.tracker.is_empty() {
            put("tracker", module.tracker.clone());
        }
        put("summary", p.summary());
        put("rate", RATE.to_string());
        put("frames", format!("{} {}", p.frames.0, p.frames.1));
        put("most", format!("{}", p.most));
        if let Some((frame, at)) = p.first {
            put("first", format!("{:.3} order {} pattern {} row {}", frame as f64 / f64::from(RATE), at.order, at.pattern, at.row));
        }
        if !p.channels.is_empty() {
            put("channels", p.channels.iter().map(|c| (c + 1).to_string()).collect::<Vec<_>>().join(" "));
        }
        for (slot, id) in &p.missing {
            put("missing", format!("{slot} {id}"));
        }
        for d in &p.differs {
            put("differs", d.clone());
        }
        let mut files = Vec::new();
        if data.len() <= CARRY_MAX {
            files.push((name.to_string(), data.to_vec()));
            files.extend(beside.iter().filter(|(_, b)| b.len() <= CARRY_MAX).cloned());
        }
        Bug { kind: "parity".into(), fields: f, files }
    }

    /// The first value of a field.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.fields.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    /// A file name for it that no other module's report has: the
    /// module's name and its ID's first digits. The same module's
    /// report again takes the same name, so it replaces the old one.
    pub fn file_name(&self) -> String {
        let stem = self.get("file").unwrap_or("module");
        let stem: String = stem.chars().map(|c| if c.is_alphanumeric() || " -_.()".contains(c) { c } else { '_' }).collect();
        let id = self.get("module").map(|m| m.get(..12).unwrap_or(m).to_string()).unwrap_or_default();
        format!("{} {id}.bug", stem.trim())
    }

    pub fn write(&self) -> String {
        let esc = |v: &str| v.replace('\\', "\\\\").replace('\n', "\\n").replace('\r', "\\r");
        let mut s = String::from("neumetik-bug 1\n");
        s.push_str(&format!("kind {}\n", esc(&self.kind)));
        for (k, v) in &self.fields {
            s.push_str(&format!("{k} {}\n", esc(v)));
        }
        for (name, bytes) in &self.files {
            s.push_str(&format!("[file {} {}]\n", esc(name), bytes.len()));
            let text = base64(bytes);
            for line in text.as_bytes().chunks(76) {
                s.push_str(std::str::from_utf8(line).unwrap_or(""));
                s.push('\n');
            }
        }
        s.push_str("[end]\n");
        s
    }

    pub fn read(text: &str) -> Result<Bug, String> {
        let unesc = |v: &str| {
            let mut out = String::new();
            let mut it = v.chars();
            while let Some(c) = it.next() {
                if c == '\\' {
                    match it.next() {
                        Some('n') => out.push('\n'),
                        Some('r') => out.push('\r'),
                        Some(o) => out.push(o),
                        None => {}
                    }
                } else {
                    out.push(c);
                }
            }
            out
        };
        let mut lines = text.lines();
        if lines.next().map(str::trim) != Some("neumetik-bug 1") {
            return Err("not a Neumetik bug report".into());
        }
        let mut bug = Bug::default();
        let mut file: Option<(String, usize, String)> = None;
        let finish = |file: &mut Option<(String, usize, String)>, bug: &mut Bug| -> Result<(), String> {
            if let Some((name, len, text)) = file.take() {
                let bytes = unbase64(&text).ok_or_else(|| format!("{name}'s bytes are damaged"))?;
                if bytes.len() != len {
                    return Err(format!("{name} is {} bytes, not {len}", bytes.len()));
                }
                bug.files.push((name, bytes));
            }
            Ok(())
        };
        for line in lines {
            if let Some(head) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                finish(&mut file, &mut bug)?;
                if head == "end" {
                    break;
                }
                let rest = head.strip_prefix("file ").ok_or("a section that isn't a file")?;
                let (name, len) = rest.rsplit_once(' ').ok_or("a file with no length")?;
                file = Some((unesc(name), len.parse().map_err(|_| "a file's length isn't a number")?, String::new()));
            } else if let Some((_, _, t)) = file.as_mut() {
                t.push_str(line.trim());
            } else if let Some((k, v)) = line.split_once(' ') {
                if k == "kind" {
                    bug.kind = unesc(v);
                } else {
                    bug.fields.push((k.to_string(), unesc(v)));
                }
            }
        }
        finish(&mut file, &mut bug)?;
        Ok(bug)
    }
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Base64 (RFC 4648), with padding.
pub fn base64(b: &[u8]) -> String {
    let mut s = String::with_capacity(b.len().div_ceil(3) * 4);
    for c in b.chunks(3) {
        let n = u32::from(c[0]) << 16 | u32::from(*c.get(1).unwrap_or(&0)) << 8 | u32::from(*c.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= c.len() {
                s.push(B64[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

pub fn unbase64(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    let (mut acc, mut bits) = (0u32, 0);
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            b' ' | b'\n' | b'\r' | b'\t' => continue,
            _ => return None,
        };
        acc = acc << 6 | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_goes_there_and_back() {
        for n in 0..10 {
            let b: Vec<u8> = (0..n).map(|i| (i * 37 + 5) as u8).collect();
            assert_eq!(unbase64(&base64(&b)).unwrap(), b);
        }
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
    }

    #[test]
    fn a_report_reads_back() {
        let mut m = Module::new(4);
        m.title = "a\nb".into();
        let p = Parity { frames: (10, 12), most: 0.5, first: Some((3, Position { order: 1, pattern: 2, row: 3, looped: 0 })), channels: vec![1], differs: vec!["sample 1: loop".into()], ..Default::default() };
        let bug = Bug::parity("song.mod", Some("/x/song.mod"), b"hello world", &[("WAVES.KWV".into(), vec![1, 2, 3])], &m, &p);
        let back = Bug::read(&bug.write()).unwrap();
        assert_eq!(back, bug);
        assert_eq!(back.get("channels"), Some("2"));
        assert!(back.file_name().starts_with("song.mod ") && back.file_name().ends_with(".bug"));
    }

    #[test]
    fn a_difference_is_found_and_placed() {
        let mut m = Module::new(4);
        let mut s = Sample::empty();
        s.wave = Arc::new(Wave::new(8, 1, (0..2000).map(|i| ((i * 97 % 255) as i16 - 128) << 8).collect()));
        s.looped = Loop::forward(0, 2000);
        m.samples.push(s);
        let mut pat = Pattern::new(64, 4);
        pat.cells[16 * 4 + 2] = Cell { note: 61, instr: 1, ..Default::default() };
        m.patterns = vec![Arc::new(pat)];
        let mut other = m.clone();
        other.samples[0].volume = 32;
        let p = check(&m, &Loaded { module: other, ..Default::default() });
        assert!(!p.ok() && p.most > 0.0);
        let (_, at) = p.first.unwrap();
        assert_eq!((at.order, at.row), (0, 16));
        assert_eq!(p.channels, vec![2]);
        assert_eq!(p.differs, vec!["sample 1: volume"]);
    }

    #[test]
    fn a_song_matches_itself() {
        let m = Module::new(4);
        let back = Loaded { module: m.clone(), ..Default::default() };
        assert!(check(&m, &back).ok());
    }
}
