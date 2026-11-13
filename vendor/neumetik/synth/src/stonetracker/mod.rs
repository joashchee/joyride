// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! **StoneTracker** (Emmanuel Marty and Michael Lavaire's Amiga tracker,
//! 1994-95): a module ("SPM.") and its sample bank ("SPS.") in two
//! files. A module holds several songs, each of 4 to 8 tracks and an FX
//! track; a pattern is one track's (64 lines), each line a note, a sample
//! and up to 7 effects (8 on the FX track, which plays no sound and
//! sends its effects to the tracks it's been told to). Tracks 1 and 2
//! (and 3 and 4 when there are fewer) play on Paula as they are; the
//! others are mixed into Paula's third and fourth channels at about
//! 27.7 kHz, tracks 3, 6 and 7 into one and 4, 5 and 8 into the other.
//!
//! Read from the developer documentation's include files (the module's,
//! song's and sample bank's headers) and the manual (the effects); the
//! patterns' packing and the bank's Huffman packing were worked out from
//! the files themselves. How the player plays (its period table, timing,
//! effects, mixer and its volume curve) was measured by running its own
//! "no operating system" player as a black box on an emulated Paula and
//! CIA, never by reading its code.
//!
//! [`Song::parse`] reads a bundle of both files ([`bundle`]), which is
//! what a UMF keeps; [`play`] is the player.

pub mod play;

use std::sync::Arc;

use crate::replay::{Driver, Tune};

/// Lines in a pattern, when a module says nothing else (every module does).
pub const LINES: usize = 64;

/// The first effect's code in a pattern; codes below are notes.
const FX_BASE: u8 = 37;

/// A line of a track's pattern.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Line {
    /// 1 to 36 (C-1 to B-3), 0 for none.
    pub note: u8,
    /// 1-based, 0 for none.
    pub inst: u8,
    /// (effect, parameter), in the order they're read; the effects are the manual's 00 to 33.
    pub fx: Vec<(u8, u8)>,
}

/// A song of the module: its tracks' pattern lists, position by position.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SongDef {
    pub name: String,
    pub bpm: u8,
    /// Its tracks, 4 to 8.
    pub voices: usize,
    /// Each track's pattern, position by position.
    pub tracks: Vec<Vec<u16>>,
    /// The FX track's patterns, when it has its own.
    pub fx: Option<Vec<u16>>,
}

impl SongDef {
    pub fn positions(&self) -> usize {
        self.tracks.first().map_or(0, Vec::len)
    }
}

/// A sample of the bank, its points at `start` in [`Song::data`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Sample {
    pub name: String,
    pub start: usize,
    pub len: usize,
    /// Where its loop starts in the bank (not from the sample's start).
    pub loop_start: usize,
    /// 0 for none.
    pub loop_len: usize,
    /// 0 to 64.
    pub volume: u8,
    /// -16 to 15, eighths of a semitone.
    pub finetune: i8,
}

/// A module and its sample bank, read.
#[derive(Debug)]
pub struct Song {
    file: Vec<u8>,
    pub name: String,
    pub songs: Vec<SongDef>,
    /// Every track's pattern.
    pub patterns: Vec<Vec<Line>>,
    /// The FX track's patterns.
    pub fx_patterns: Vec<Vec<Line>>,
    /// Lines a pattern.
    pub lines: usize,
    pub samples: Vec<Sample>,
    /// The bank's points, unpacked.
    pub data: Arc<Vec<i8>>,
    /// The bank was packed (and how far its packed points went, when short).
    pub packed: bool,
    pub short_bank: bool,
}

/// What a bundle starts with: both files, the module's first.
const BUNDLE: &[u8; 8] = b"NMSTONE1";

/// The module and its sample bank as one file (what a UMF keeps).
pub fn bundle(spm: &[u8], sps: &[u8]) -> Vec<u8> {
    let mut b = BUNDLE.to_vec();
    b.extend_from_slice(&(spm.len() as u32).to_le_bytes());
    b.extend_from_slice(spm);
    b.extend_from_slice(sps);
    b
}

/// Whether `d` is a StoneTracker module (the "SPM." file).
pub fn probe(d: &[u8]) -> bool {
    d.len() >= 0x30 && &d[..3] == b"SPM" && d[3] == 2 && {
        let n = |o: usize| usize::from(u16::from_be_bytes([d[o], d[o + 1]]));
        n(0x24) > 0 && n(0x2a) > 0 && n(0x2a) <= 256 && 0x30 + 4 * (n(0x24) + n(0x26) + n(0x28)) <= d.len()
    }
}

/// Whether `d` is a StoneTracker sample bank (the "SPS." file).
pub fn probe_bank(d: &[u8]) -> bool {
    d.len() >= 6 && &d[..3] == b"SPS" && matches!(d[3], 1 | 2) && d.len() >= 6 + 32 * usize::from(d[5])
}

fn be16(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_be_bytes(d.get(o..o + 2)?.try_into().ok()?))
}

fn be32(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_be_bytes(d.get(o..o + 4)?.try_into().ok()?))
}

fn text(b: &[u8]) -> String {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    // Latin-1, as the Amiga wrote it.
    b[..end].iter().map(|&c| c as char).filter(|c| !c.is_control()).collect::<String>().trim().to_string()
}

/// A pattern's lines from its packed bytes: two bytes an item, FF n for
/// n+1 empty lines; otherwise bit 7 of the first ends the line, which is
/// a note (below 37) and its sample, or an effect (37 on) and its
/// parameter.
fn unpack_pattern(d: &[u8], lines: usize) -> Vec<Line> {
    let mut out = Vec::with_capacity(lines);
    let mut cur = Line::default();
    let mut p = 0;
    while p + 1 < d.len() && out.len() < lines {
        let (a, b) = (d[p], d[p + 1]);
        p += 2;
        if a == 0xFF {
            for _ in 0..=b {
                if out.len() < lines {
                    out.push(Line::default());
                }
            }
            continue;
        }
        let code = a & 0x7F;
        if code < FX_BASE {
            if cur.note == 0 && cur.inst == 0 && cur.fx.is_empty() {
                cur.note = code;
                cur.inst = b;
            }
        } else {
            cur.fx.push((code - FX_BASE, b));
        }
        if a & 0x80 != 0 {
            out.push(std::mem::take(&mut cur));
        }
    }
    out.resize(lines, Line::default());
    out
}

/// The bank's points from its Huffman packing ("psn"): the symbols a
/// running sum's steps (wrapping in a byte, from 0 across the whole
/// bank); a table of each symbol's code, then the codes, read a bit at a
/// time from each byte's top, a code's first bit its lowest. Returns the
/// points and whether the file ran out first.
fn unpack_bank(d: &[u8], version: u8) -> Option<(Vec<i8>, bool)> {
    if d.get(..3)? != b"psn" {
        return None;
    }
    let count = usize::from(*d.get(3)?) + 1;
    let size = be32(d, 4)? as usize;
    // At most 16 MB of samples (the Amiga's whole address space).
    if size > 16 << 20 {
        return None;
    }
    let mut q = 8;
    // Each symbol: its byte, its code's length, the code in len/8+1 bytes.
    let mut codes: Vec<(u8, u32, u8)> = Vec::with_capacity(count);
    for _ in 0..count {
        let sym = *d.get(q)?;
        let len = *d.get(q + 1)?;
        if len == 0 || len > 32 {
            return None;
        }
        let nb = usize::from(len) / 8 + 1;
        let mut code: u64 = 0;
        for &b in d.get(q + 2..q + 2 + nb)? {
            code = code << 8 | u64::from(b);
        }
        codes.push((len, (code & ((1u64 << len) - 1)) as u32, sym));
        q += 2 + nb;
    }
    // Version 1 banks keep a byte between the table and the codes.
    if version == 1 {
        q += 1;
    }
    // A decoding table: by length, the codes (as read, first bit lowest).
    let maxlen = codes.iter().map(|c| c.0).max()? as usize;
    let mut by_len: Vec<Vec<(u32, u8)>> = vec![Vec::new(); maxlen + 1];
    for &(len, code, sym) in &codes {
        by_len[usize::from(len)].push((code, sym));
    }
    for l in &mut by_len {
        l.sort_unstable();
    }
    let mut out = Vec::with_capacity(size);
    let mut acc: u8 = 0;
    let mut bit = q * 8;
    let total = d.len() * 8;
    'points: while out.len() < size {
        let mut code: u32 = 0;
        for len in 1..=maxlen {
            if bit >= total {
                break 'points;
            }
            code |= u32::from(d[bit >> 3] >> (7 - (bit & 7)) & 1) << (len - 1);
            bit += 1;
            if let Ok(i) = by_len[len].binary_search_by_key(&code, |&(c, _)| c) {
                acc = acc.wrapping_add(by_len[len][i].1);
                out.push(acc as i8);
                continue 'points;
            }
        }
        return None;
    }
    let short = out.len() < size;
    out.resize(size, 0);
    Some((out, short))
}

impl Song {
    /// Reads a bundle ([`bundle`]).
    pub fn parse(file: &[u8]) -> Option<Song> {
        if file.get(..8)? != BUNDLE {
            return None;
        }
        let n = u32::from_le_bytes(file.get(8..12)?.try_into().ok()?) as usize;
        let spm = file.get(12..12usize.checked_add(n)?)?;
        let sps = &file[12 + n..];
        Song::read(spm, sps, file.to_vec())
    }

    /// Reads the two files.
    pub fn from_files(spm: &[u8], sps: &[u8]) -> Option<Song> {
        Song::read(spm, sps, bundle(spm, sps))
    }

    fn read(spm: &[u8], sps: &[u8], file: Vec<u8>) -> Option<Song> {
        if !probe(spm) || !probe_bank(sps) {
            return None;
        }
        let name = text(&spm[4..35]);
        let nsongs = usize::from(be16(spm, 0x24)?);
        let npat = usize::from(be16(spm, 0x26)?);
        let nfx = usize::from(be16(spm, 0x28)?);
        let lines = usize::from(be16(spm, 0x2a)?);
        let total = nsongs + npat + nfx;
        let offs: Vec<usize> = (0..total).map(|i| be32(spm, 0x30 + 4 * i).map(|o| o as usize)).collect::<Option<_>>()?;
        // Each block runs to the next one's start (or the file's end).
        let block = |i: usize| -> &[u8] {
            let start = offs[i].min(spm.len());
            let end = offs.iter().copied().filter(|&o| o > start).min().unwrap_or(spm.len()).min(spm.len());
            &spm[start..end]
        };
        let mut songs = Vec::with_capacity(nsongs);
        for i in 0..nsongs {
            let b = block(i);
            if b.len() < 36 {
                return None;
            }
            let voices = usize::from(b[32]);
            if !(4..=8).contains(&voices) {
                return None;
            }
            let has_fx = b[33] != 0;
            let positions = usize::from(be16(b, 34)?);
            let list = |k: usize| -> Option<Vec<u16>> { (0..positions).map(|p| be16(b, 36 + 2 * (k * positions + p))).collect() };
            let tracks = (0..voices).map(list).collect::<Option<Vec<_>>>()?;
            let fx = if has_fx { list(voices) } else { None };
            songs.push(SongDef {
                name: text(&b[..31]),
                bpm: b[31],
                voices,
                tracks,
                fx,
            });
        }
        let patterns = (0..npat).map(|i| unpack_pattern(block(nsongs + i), lines)).collect();
        let fx_patterns = (0..nfx).map(|i| unpack_pattern(block(nsongs + npat + i), lines)).collect();

        let count = usize::from(sps[5]);
        let head = 6 + 32 * count;
        let packed = sps[4] & 1 != 0;
        let (data, short_bank) = if packed {
            unpack_bank(&sps[head..], sps[3])?
        } else {
            (sps[head..].iter().map(|&b| b as i8).collect(), false)
        };
        let samples = (0..count)
            .map(|k| {
                let r = &sps[6 + 32 * k..6 + 32 * (k + 1)];
                let at = |o: usize| u32::from_be_bytes(r[o..o + 4].try_into().unwrap()) as usize;
                Sample {
                    name: text(&r[..8]),
                    start: at(8),
                    len: at(12),
                    loop_start: at(16),
                    loop_len: at(20),
                    volume: i16::from_be_bytes([r[24], r[25]]).clamp(0, 64) as u8,
                    finetune: i16::from_be_bytes([r[26], r[27]]).clamp(-16, 15) as i8,
                }
            })
            .collect();
        Some(Song {
            file,
            name,
            songs,
            patterns,
            fx_patterns,
            lines,
            samples,
            data: Arc::new(data),
            packed,
            short_bank,
        })
    }

    /// A track's pattern at a position (empty past the patterns).
    pub fn pattern(&self, n: u16) -> Option<&[Line]> {
        self.patterns.get(usize::from(n)).map(Vec::as_slice)
    }
}

impl Tune for Song {
    fn format(&self) -> &'static str {
        "stone"
    }

    fn file(&self) -> &[u8] {
        &self.file
    }

    fn songs(&self) -> usize {
        self.songs.len().max(1)
    }

    fn driver(&self, song: usize) -> Box<dyn Driver> {
        Box::new(play::Player::new(Arc::new(self.shared()), song))
    }
}

impl Song {
    /// What the player needs, shared.
    fn shared(&self) -> play::Shared {
        play::Shared {
            songs: self.songs.clone(),
            patterns: self.patterns.clone(),
            fx_patterns: self.fx_patterns.clone(),
            lines: self.lines,
            samples: self.samples.clone(),
            data: self.data.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The files of a song: "x.spm" and "x.sps", or Amiga's "SPM.x" and "SPS.x".
    fn pairs(dir: &str) -> Vec<(String, Vec<u8>, Vec<u8>)> {
        let mut out = Vec::new();
        let mut names: Vec<_> = std::fs::read_dir(dir).unwrap().filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().to_string()).collect();
        names.sort();
        for n in &names {
            let bank = if let Some(base) = n.strip_suffix(".spm") {
                format!("{base}.sps")
            } else if let Some(base) = n.strip_prefix("SPM.") {
                format!("SPS.{base}")
            } else {
                continue;
            };
            let (Ok(a), Ok(b)) = (std::fs::read(format!("{dir}/{n}")), std::fs::read(format!("{dir}/{bank}"))) else {
                continue;
            };
            out.push((n.clone(), a, b));
        }
        out
    }

    /// The player as a host plays it (ticks, then frames) against the
    /// reference's grids: each channel's frames summed back to Paula's
    /// point times volume, at a rate whose frames fall on the grid.
    #[test]
    #[ignore]
    fn driver_vs_grid() {
        let dir = std::env::var("STONE_DIR").unwrap();
        let refs = std::env::var("STONE_REF").unwrap();
        let secs: f64 = std::env::var("STONE_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(10.0);
        let n = 64.0;
        // Frames of exactly 5 colour clocks: every 64th falls on a grid point.
        let rate = 709_379u32;
        for (name, spm, sps) in pairs(&dir) {
            let song = Song::from_files(&spm, &sps).unwrap();
            let Ok(r) = std::fs::read(format!("{refs}/{name}.0.grid")) else { continue };
            let rg: Vec<[i16; 4]> = r.chunks_exact(8).map(|c| std::array::from_fn(|k| i16::from_le_bytes([c[2 * k], c[2 * k + 1]]))).collect();
            let mut d = song.driver(0);
            let tracks = d.channels();
            let mut bufs = crate::replay::Bufs {
                left: vec![vec![0.0; 1 << 16]; tracks],
                right: vec![vec![0.0; 1 << 16]; tracks],
            };
            let mut frames: Vec<[f32; 4]> = Vec::new();
            let mut left = 0.0f64;
            while (frames.len() as f64) < secs * f64::from(rate) {
                d.tick();
                left += d.tick_seconds() * f64::from(rate);
                let k = left.floor() as usize;
                left -= k as f64;
                d.render(rate, k, &mut bufs);
                for i in 0..k {
                    let mut f = [0f32; 4];
                    for t in 0..tracks {
                        // Paula's channel for the track: 1-4, then 3 6 7 to the third, 4 5 8 to the fourth.
                        let c = [0, 1, 2, 3, 3, 2, 2, 3][t];
                        f[c] += (bufs.left[t][i] + bufs.right[t][i]) / 0.4 * 128.0 * 64.0;
                    }
                    frames.push(f);
                }
            }
            let pairs: Vec<(usize, usize)> = (0..frames.len()).step_by(64).map(|i| (i, i * 5 / 64)).filter(|&(_, g)| g < rg.len()).collect();
            let near = |a: f32, b: i16| (a - f32::from(b)).abs() <= 1.0;
            let same: Vec<usize> = (0..4).map(|c| pairs.iter().filter(|&&(i, g)| near(frames[i][c], rg[g][c])).count()).collect();
            if std::env::var("STONE_SHOW").is_ok() {
                if let Some(&(i, g)) = pairs.iter().find(|&&(i, g)| !near(frames[i][0], rg[g][0])) {
                    println!("  first ch0 difference: frame {i} (grid {g}, tick {:.2})", g as f64 * n / 70940.0);
                    for &(i, g) in pairs.iter().skip(i.saturating_sub(3)).take(12) {
                        println!("    {i} {} {}", frames[i][0], rg[g][0]);
                    }
                }
            }
            println!(
                "{name}: {}",
                same.iter().map(|&s| format!("{:.2}%", 100.0 * s as f64 / pairs.len() as f64)).collect::<Vec<_>>().join(" ")
            );
        }
    }

    /// Each tick's period and volume on a channel: STONE_FILE (the .spm), STONE_CH, STONE_TICKS.
    #[test]
    #[ignore]
    fn trace() {
        let f = std::env::var("STONE_FILE").unwrap();
        let bank = f.strip_suffix(".spm").map(|b| format!("{b}.sps")).unwrap_or_else(|| f.replace("SPM.", "SPS."));
        let song = Song::from_files(&std::fs::read(&f).unwrap(), &std::fs::read(bank).unwrap()).unwrap();
        let c: usize = std::env::var("STONE_CH").ok().and_then(|s| s.parse().ok()).unwrap_or(0);
        let n: usize = std::env::var("STONE_TICKS").ok().and_then(|s| s.parse().ok()).unwrap_or(100);
        let sub: usize = std::env::var("STONE_SONG").ok().and_then(|s| s.parse().ok()).unwrap_or(0);
        let mut p = play::Player::new(Arc::new(song.shared()), sub);
        for (k, (pos, line, tick, per, vol)) in p.trace(n, c).into_iter().enumerate() {
            println!("T {k} {pos} {line} {tick} {per} {vol}");
        }
    }

    /// Our player against the reference's grids (STONE_REF/<name>.<song>.grid,
    /// each channel's point times its volume every STONE_GRID colour clocks
    /// from the first tick): STONE_DIR, STONE_SECS, STONE_ONLY.
    #[test]
    #[ignore]
    fn against() {
        let dir = std::env::var("STONE_DIR").unwrap();
        let refs = std::env::var("STONE_REF").unwrap();
        let secs: f64 = std::env::var("STONE_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(30.0);
        let n: u64 = std::env::var("STONE_GRID").ok().and_then(|s| s.parse().ok()).unwrap_or(64);
        let only = std::env::var("STONE_ONLY").ok();
        let mut all = 0usize;
        let mut same_songs = 0usize;
        for (name, spm, sps) in pairs(&dir) {
            if only.as_ref().is_some_and(|o| !name.contains(o.as_str())) {
                continue;
            }
            let song = Song::from_files(&spm, &sps).unwrap_or_else(|| panic!("{name} doesn't read"));
            for s in 0..song.songs.len() {
                let Ok(r) = std::fs::read(format!("{refs}/{name}.{s}.grid")) else { continue };
                let rg: Vec<[i16; 4]> = r.chunks_exact(8).map(|c| std::array::from_fn(|k| i16::from_le_bytes([c[2 * k], c[2 * k + 1]]))).collect();
                let mut p = play::Player::new(Arc::new(song.shared()), s);
                let og = p.run_grid(secs, n);
                if let Ok(d) = std::env::var("STONE_DUMP") {
                    let bytes: Vec<u8> = og.iter().flat_map(|g| g.iter().flat_map(|v| v.to_le_bytes())).collect();
                    std::fs::write(format!("{d}/{name}.{s}.ours"), bytes).unwrap();
                }
                let len = og.len().min(rg.len());
                let mut line = format!("{name} song {s}:");
                let mut all_same = true;
                for c in 0..4 {
                    let same = (0..len).filter(|&i| og[i][c] == rg[i][c]).count();
                    let first = (0..len).find(|&i| og[i][c] != rg[i][c]);
                    all_same &= first.is_none();
                    line += &format!(" ch{c} {:.1}%", 100.0 * same as f64 / len.max(1) as f64);
                    if let Some(i) = first {
                        line += &format!(" (from {:.3}s)", (i as u64 * n) as f64 / crate::paula::CLOCK);
                    }
                }
                all += 1;
                same_songs += usize::from(all_same);
                println!("{line}");
                if let Some(c) = std::env::var("STONE_SHOW").ok().and_then(|v| v.parse::<usize>().ok()) {
                    if let Some(i) = (0..len).find(|&i| og[i][c] != rg[i][c]) {
                        let tick = crate::paula::CLOCK / 50.0;
                        println!(
                            "  ch{c} first difference at grid {i} ({} cc, about tick {:.2} at 125 BPM):",
                            i as u64 * n,
                            (i as u64 * n) as f64 / (tick / 1.0)
                        );
                        for k in i.saturating_sub(4)..(i + 24).min(len) {
                            println!("    {k}: ours {:6} ref {:6}{}", og[k][c], rg[k][c], if og[k][c] != rg[k][c] { "  <" } else { "" });
                        }
                    }
                }
            }
        }
        println!("{same_songs} of {all} songs the same");
    }

    #[test]
    fn patterns_unpack() {
        // A note with an effect after it, two empty lines, an effect alone.
        let d = [0x0D, 0x01, 0x80 | (FX_BASE + 0x0C), 0x20, 0xFF, 0x01, 0x80 | (FX_BASE + 0x0A), 0x02];
        let p = unpack_pattern(&d, 5);
        assert_eq!(
            p[0],
            Line {
                note: 13,
                inst: 1,
                fx: vec![(0x0C, 0x20)]
            }
        );
        assert_eq!(p[1], Line::default());
        assert_eq!(p[2], Line::default());
        assert_eq!(
            p[3],
            Line {
                note: 0,
                inst: 0,
                fx: vec![(0x0A, 0x02)]
            }
        );
        assert_eq!(p[4], Line::default());
    }

    #[test]
    fn banks_unpack() {
        // Two symbols: +1 (code 0) and -1 (code 1), each a bit.
        let mut d = b"psn".to_vec();
        d.push(1);
        d.extend_from_slice(&6u32.to_be_bytes());
        d.extend_from_slice(&[1, 1, 0, 0xFF, 1, 1]);
        // 0 0 0 1 1 1 then padding: +1 +1 +1 -1 -1 -1.
        d.push(0b0001_1100);
        let (pts, short) = unpack_bank(&d, 2).unwrap();
        assert_eq!(pts, [1, 2, 3, 2, 1, 0]);
        assert!(!short);
    }
}
