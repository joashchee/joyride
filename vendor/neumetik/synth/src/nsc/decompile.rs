// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.
//
//! An NSF that nsc linked, back to NSD.Lib's MML: the driver build it was linked with (`#Code`), the song table's
//! BGM and SE blocks, the subroutines, envelopes and FDS, VRC7 and N163 voices they point to (each written where
//! nsc laid it out, so in the same order), and every command as the MML nsc writes exactly it with (`%` lengths,
//! `r=`-style rests, `#RepeatMode 1` so `[` is the driver's repeat everywhere), the metadata as its directives.
//! The DPCM samples come out as files beside the MML (`d000.dmc`..., named in the order nsc lays them out).
//! [`decompile`] compiles and links what it wrote and gives it only when the NSF comes out the same.

use super::{compile_with, link, op};

thread_local! {
    /// Where [`write`] last gave up, for the tests.
    static WHY: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

fn why_not<T>(s: impl FnOnce() -> String) -> Option<T> {
    if cfg!(test) {
        WHY.with(|w| *w.borrow_mut() = s());
    }
    None
}

const TBL: [u8; 16] = [96, 72, 48, 36, 32, 24, 18, 16, 12, 9, 8, 6, 4, 3, 2, 1];

/// What an item in the music data is.
#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord)]
enum What {
    Bgm(usize),
    Se(usize),
    Sub,
    Env,
    Fdsc,
    Fdsm,
    Vrc7,
    N163,
    Dpcm,
}

/// MML that nsc compiles and links to an NSF, and the DPCM sample files it names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decompiled {
    pub mml: Vec<u8>,
    pub files: Vec<(String, Vec<u8>)>,
}

fn word(m: &[u8], at: usize) -> Option<usize> {
    Some(usize::from(*m.get(at)?) | usize::from(*m.get(at + 1)?) << 8)
}

/// A relative address in a command at `at` (its op byte), `back` bytes after the op: where it points.
fn target(m: &[u8], at: usize, back: usize) -> Option<usize> {
    let a = word(m, at + back)?;
    Some((at + back).wrapping_add(a) & 0xffff)
}

/// The bytes after an op (not counting the op).
fn args(m: &[u8], at: usize) -> Option<usize> {
    let b = *m.get(at)?;
    Some(match b {
        0x00 | 0x0d..=0x0f | 0x18 | 0x19 | 0x20 | 0x21 | 0x28 | 0x29 | 0x2c | 0x2d => 0,
        0x01 | 0x02 | 0x04 | 0x05 | 0x06 | 0x10..=0x13 | 0x1a | 0x1c | 0x1d | 0x22 | 0x23 | 0x24 | 0x26 => 2,
        0x03 | 0x08 | 0x09 | 0x0a | 0x0b | 0x0c | 0x14 | 0x15 | 0x16 | 0x1b | 0x1f | 0x25 | 0x2a | 0x2b => 1,
        0x07 | 0x1e => 3,
        0x17 => 4,
        0x2f => match *m.get(at + 1)? {
            0 | 1 => 1,
            2 | 3 => 2,
            _ => return None,
        },
        0x30..=0x7f => 0,
        0x80..=0xff => usize::from(b & op::NOTE_LENGTH != 0) + usize::from(b & op::NOTE_GATE != 0),
        _ => return None,
    })
}

/// A track's (or a Sub's) commands from `at` to its end (the end of track, or the loop's jump): each command's
/// offset, and where the stream ends.
fn stream(m: &[u8], mut at: usize) -> Option<(Vec<usize>, usize)> {
    let mut ev = Vec::new();
    loop {
        let b = *m.get(at)?;
        ev.push(at);
        let n = args(m, at)?;
        at += 1 + n;
        if b == op::END_OF_TRACK || b == op::JUMP {
            return Some((ev, at));
        }
        if ev.len() > 100_000 {
            return None;
        }
    }
}

/// An envelope's length: its release point, then values, holds and loop bytes; each `R` writes a loop byte, and
/// the part after the last (the release part, from the point the first byte names) ends at its own loop byte.
fn env_len(m: &[u8], at: usize) -> Option<usize> {
    let release = usize::from(*m.get(at)?);
    let mut pt = release.max(1);
    while *m.get(at + pt)? < 0xc0 {
        pt += 1;
    }
    Some(pt + 1)
}

/// A string as nsc reads it back (its escapes on): Shift JIS as it stands, `"`, `\\` and control bytes as `\\xNN`.
fn esc(s: &[u8]) -> Vec<u8> {
    let mut o = vec![b'"'];
    let mut hexed = false;
    for &c in s {
        let plain = c >= 0x20 && c != 0x7f && c != b'"' && c != b'\\';
        if plain && !(hexed && c.is_ascii_hexdigit()) {
            o.push(c);
            hexed = false;
        } else {
            o.extend_from_slice(format!("\\x{c:02x}").as_bytes());
            hexed = true;
        }
    }
    o.push(b'"');
    o
}

fn i8s(v: u8) -> String {
    (v as i8).to_string()
}

struct Decoder<'a> {
    m: &'a [u8],
    /// Each item's start, what it is and its number in the MML.
    items: Vec<(usize, What, usize)>,
}

impl Decoder<'_> {
    fn id(&self, at: usize, kinds: &[What]) -> Option<usize> {
        self.items.iter().find(|(o, w, _)| *o == at && kinds.contains(w)).map(|&(_, _, n)| n)
    }

    /// A stream's MML tokens.
    fn tokens(&self, start: usize) -> Option<Vec<String>> {
        let m = self.m;
        let (ev, _) = stream(m, start)?;
        // where the loop's jump goes: an `L` before the command there
        let loop_to = ev.last().and_then(|&at| (m[at] == op::JUMP).then(|| target(m, at, 1)).flatten());
        let mut out = Vec::new();
        // A stream of only its end: something the optimizer drops later (the volume it starts at) makes nsc write
        // the end at all.
        if ev.len() == 1 {
            out.push("v15".to_string());
        }
        let mut default_len: i32 = -1;
        for &at in &ev {
            if Some(at) == loop_to {
                out.push("L".to_string());
            }
            let b = m[at];
            let a = |i: usize| m[at + 1 + i];
            let w = |i: usize| u16::from(a(i)) | u16::from(a(i + 1)) << 8;
            let t = match b {
                op::END_OF_TRACK | op::JUMP => continue,
                op::CALL => {
                    default_len = -1;
                    format!("S{}", self.id(target(m, at, 1)?, &[What::Sub])?)
                }
                op::CALL_SE => {
                    let to = target(m, at, 1)?;
                    let n = self.items.iter().find_map(|&(o, w, _)| match w {
                        What::Se(n) if o == to => Some(n),
                        _ => None,
                    })?;
                    format!("SE{n}")
                }
                op::REPEAT_A_START => format!("[{}", a(0)),
                op::REPEAT_A_BRANCH => ":".into(),
                op::REPEAT_A_END => {
                    default_len = -1;
                    "]".into()
                }
                op::REPEAT_B_START => "|:".into(),
                op::REPEAT_B_BRANCH => "\\".into(),
                op::REPEAT_B_END => {
                    default_len = -1;
                    ":|".into()
                }
                op::POKE => format!("y{},{}", w(0), a(2)),
                op::TEMPO => format!("t{}", a(0)),
                op::RELATIVE_TEMPO => format!("t_{}", i8s(a(0))),
                op::LENGTH => {
                    if TBL.contains(&a(0)) || a(0) == 0 {
                        return None;
                    }
                    default_len = i32::from(a(0));
                    format!("l%{}", a(0))
                }
                0x40..=0x4f => {
                    default_len = i32::from(TBL[usize::from(b - 0x40)]);
                    format!("l%{}", default_len)
                }
                op::GATE_Q => {
                    if a(0) < 16 {
                        return None;
                    }
                    format!("q{}", a(0))
                }
                0x50..=0x5f => format!("q{}", b - 0x50),
                op::GATE_U => {
                    if a(0) == 0 {
                        "u0".into()
                    } else {
                        format!("u%{}", a(0))
                    }
                }
                0x0d..=0x0f => format!("Rm{}", b - 0x0d),
                op::ENV_VOICE | op::ENV_VOLUME | op::ENV_FREQUENCY | op::ENV_NOTE => {
                    let name = ["E@", "Ev", "Em", "En"][usize::from(b - op::ENV_VOICE)];
                    if w(0) == 0 && b != op::ENV_VOICE {
                        format!("{name}*")
                    } else {
                        format!("{name}{}", self.id(target(m, at, 1)?, &[What::Env])?)
                    }
                }
                op::DETUNE_CENT => format!("D{}", i8s(a(0))),
                op::DETUNE_REGISTER => format!("D%{}", i8s(a(0))),
                op::SWEEP => format!("s{}", a(0)),
                op::PORTAMENTO => {
                    let decay = (i32::from(a(0)) + 255) & 0xff;
                    let rate = if a(1) == 0 { 256 } else { i32::from(a(1)) };
                    format!("P{decay},{rate},{},{}", i8s(a(2)), i8s(a(3)))
                }
                op::VOICE => format!("@{}", a(0)),
                op::VRC7 => format!("@V{}", self.id(target(m, at, 1)?, &[What::Vrc7])?),
                op::VRC7_REG => {
                    if a(0) > 0x40 {
                        return None;
                    }
                    format!("yV{},{}", a(0), a(1))
                }
                op::N163 => {
                    if a(0) > 63 {
                        return None;
                    }
                    format!("@N{},{}", a(0), self.id(target(m, at, 2)?, &[What::N163])?)
                }
                op::N163_CHANNEL => {
                    if a(0) > 7 {
                        return None;
                    }
                    format!("NC{}", a(0) + 1)
                }
                op::VOLUME_DOWN => "(1".into(),
                op::VOLUME_UP => ")1".into(),
                op::FDS_CAREER => format!("@FC{}", self.id(target(m, at, 1)?, &[What::Fdsc])?),
                op::FDS_MODULATOR => format!("@FM{}", self.id(target(m, at, 1)?, &[What::Fdsm])?),
                op::FDS_FREQUENCY => {
                    if w(0) > 0xfff {
                        return None;
                    }
                    format!("@FF{}", w(0))
                }
                op::FDS_VOLUME => {
                    if a(0) > 3 {
                        return None;
                    }
                    format!("@FV{}", a(0))
                }
                op::FME7_FREQUENCY => format!("F{}", w(0)),
                op::OCTAVE_DOWN => "<".into(),
                op::OCTAVE_UP => ">".into(),
                op::OCTAVE_DOWN_1 => "\"".into(),
                op::OCTAVE_UP_1 => "`".into(),
                op::TRANSPOSE => format!("_{}", i8s(a(0))),
                op::RELATIVE_TRANSPOSE => format!("__{}", i8s(a(0))),
                op::SUB_COMMAND => match a(0) {
                    op::SUB_JUMP_ON => "j1".into(),
                    op::SUB_JUMP_OFF => "j0".into(),
                    op::SUB_DETUNE_CENT => format!("D_{}", i8s(a(1))),
                    _ => format!("D%_{}", i8s(a(1))),
                },
                0x30..=0x37 => format!("R@{}", b - 0x30),
                0x38..=0x3f => format!("o{}", b - 0x38 + 1),
                0x60..=0x6f => format!("v{}", b - 0x60),
                0x70..=0x7f => format!("Rv{}", b - 0x70),
                0x80..=0xff => {
                    let code = b & 0x0f;
                    let mut s = String::from(match code {
                        0x0c => "r*",
                        0x0d => "r-",
                        0x0e => "r#",
                        0x0f => "r=",
                        k => ["c", "c+", "d", "d+", "e", "f", "f+", "g", "g+", "a", "a+", "b"][usize::from(k)],
                    });
                    let mut i = 0;
                    if b & op::NOTE_LENGTH != 0 {
                        if a(0) == 0 {
                            return None;
                        }
                        // A length the same as `l`'s: nsc leaves it out unless something forgot the default, and
                        // an empty `[: :]` forgets it, writing nothing.
                        if i32::from(a(0)) == default_len {
                            out.push("[: :]2".into());
                            default_len = -1;
                        }
                        s += &format!("%{}", a(0));
                        i += 1;
                    }
                    if b & op::NOTE_GATE != 0 {
                        s += &if a(i) == 0 { ",0".to_string() } else { format!(",%{}", a(i)) };
                    }
                    if b & op::NOTE_SLUR != 0 {
                        s.push('&');
                    }
                    s
                }
                _ => return why_not(|| format!("op {b:02x}")),
            };
            out.push(t);
        }
        Some(out)
    }
}

/// An NSF nsc linked (version 2, its metadata after the program) back to NSD.Lib's MML that nsc compiles and links
/// to the very same file; None when there is none (another driver, DPCM, values nsc's MML can't write).
pub fn decompile(nsf: &[u8]) -> Option<Decompiled> {
    let d = write(nsf)?;
    (relink(&d)? == nsf).then_some(d)
}

/// The NSF nsc makes of decompiled MML.
pub(crate) fn relink(d: &Decompiled) -> Option<Vec<u8>> {
    let files = d.files.clone();
    let reader = move |f: &str| files.iter().find(|(n, _)| n == f).map(|(_, b)| b.clone());
    link::nsf(&compile_with(&d.mml, &reader).ok()?).ok()
}

/// The MML, not yet checked.
pub(crate) fn write(nsf: &[u8]) -> Option<Decompiled> {
    if nsf.len() < 0x80 || &nsf[..5] != b"NESM\x1a" || nsf[5] != 2 {
        return None;
    }
    // The driver build: the code after the header.
    let (code_name, rom) = link::ROMS.iter().find(|(_, r)| nsf.get(0x80..0x80 + r.len() - 0x80) == Some(&r[0x80..]))?;
    let code_len = rom.len() - 0x80;
    let banked = rom[0x70..0x78].iter().any(|&b| b != 0);
    let t0 = 0x80 + code_len;
    let (nbgm, nse) = (usize::from(*nsf.get(t0)?), usize::from(*nsf.get(t0 + 1)?));
    let table = 4 + (nbgm + nse) * 2;
    let base = if banked { table } else { 0x8000 + code_len + table };
    let dpcm_ptr = word(nsf, t0 + 2)?;
    let size = usize::from(nsf[0x7d]) | usize::from(nsf[0x7e]) << 8 | usize::from(nsf[0x7f]) << 16;
    let data_end = (0x80 + size).min(nsf.len());
    let m = nsf.get(t0 + table..data_end)?;
    let offs: Vec<usize> = (0..nbgm + nse).map(|i| word(nsf, t0 + 4 + 2 * i).and_then(|p| p.checked_sub(base))).collect::<Option<_>>()?;
    // Every item, from the song table and the pointers in the streams.
    let mut items: Vec<(usize, What)> = offs.iter().enumerate().map(|(i, &o)| (o, if i < nbgm { What::Bgm(i) } else { What::Se(i - nbgm) })).collect();
    if dpcm_ptr != 0 {
        items.push((dpcm_ptr.checked_sub(base)?, What::Dpcm));
    }
    let mut todo: Vec<usize> = Vec::new();
    let mut ends: Vec<(usize, usize)> = Vec::new();
    for &(o, _) in items.clone().iter() {
        let count = usize::from(*m.get(o)?);
        let mut end = o + 2 + 2 * count;
        for k in 0..count {
            let p = word(m, o + 2 + 2 * k)?;
            if p != 0 {
                let (ev, e) = stream(m, o + p)?;
                end = end.max(e);
                todo.extend(ev);
            }
        }
        ends.push((o, end));
    }
    let mut subs_done = std::collections::BTreeSet::new();
    while let Some(at) = todo.pop() {
        let b = m[at];
        let (what, back) = match b {
            op::CALL => (What::Sub, 1),
            op::ENV_VOICE..=op::ENV_NOTE if word(m, at + 1)? != 0 || b == op::ENV_VOICE => (What::Env, 1),
            op::FDS_CAREER => (What::Fdsc, 1),
            op::FDS_MODULATOR => (What::Fdsm, 1),
            op::VRC7 => (What::Vrc7, 1),
            op::N163 => (What::N163, 2),
            _ => continue,
        };
        let to = target(m, at, back)?;
        if items.iter().any(|&(o, w)| o == to && w == what) {
            continue;
        }
        items.push((to, what));
        let end = match what {
            What::Sub => {
                if subs_done.insert(to) {
                    let (ev, e) = stream(m, to)?;
                    todo.extend(ev);
                    e
                } else {
                    continue;
                }
            }
            What::Env => to + env_len(m, to)?,
            What::Fdsc => to + 64,
            What::Fdsm => to + 32,
            What::Vrc7 => to + 8,
            _ => to + 1 + usize::from(*m.get(to)?),
        };
        ends.push((to, end));
    }
    // The items tile the data, in order.
    let mut order: Vec<(usize, usize, What)> = items.iter().map(|&(o, w)| (o, ends.iter().find(|e| e.0 == o).map_or(o, |e| e.1), w)).collect();
    order.sort();
    order.dedup();
    // The DPCM table: up to the next item (or the music's end, which is where the zero padding begins).
    for i in 0..order.len() {
        if order[i].2 == What::Dpcm {
            order[i].1 = match order.get(i + 1) {
                Some(n) => n.0,
                None => {
                    let w = if banked { 6 } else { 4 };
                    // the last entry that isn't all zeros ends it
                    let mut e = order[i].0;
                    let mut k = order[i].0;
                    while k + w <= m.len() && k < order[i].0 + 256 * w {
                        if m[k..k + w].iter().any(|&b| b != 0) {
                            e = k + w;
                        }
                        k += w;
                    }
                    e
                }
            };
        }
    }
    let mut at = 0;
    for &(o, e, w) in &order {
        if o != at {
            return why_not(|| format!("the items don't tile: {w:?} at {o:#x}, {at:#x} expected"));
        }
        at = e;
    }
    // (the DPCM samples come after the music; the relink checks them)
    if dpcm_ptr == 0 && m[at..].iter().any(|&b| b != 0) {
        return why_not(|| "data after the items".into());
    }
    // Their numbers in the MML: the song table's for BGM and SE, else in order of each kind.
    let mut numbered = Vec::new();
    let mut counts = std::collections::BTreeMap::new();
    for &(o, _, w) in &order {
        let n = match w {
            What::Bgm(n) | What::Se(n) => n,
            _ => {
                let c = counts.entry(w).or_insert(0usize);
                *c += 1;
                *c - 1
            }
        };
        numbered.push((o, w, n));
    }
    let d = Decoder { m, items: numbered.clone() };
    // The DPCM samples: a file for each start the table names, in address order.
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let mut dpcm_lines = Vec::new();
    let mut offset_pcm = None;
    if let Some(&(o, e, _)) = order.iter().find(|x| x.2 == What::Dpcm) {
        let w = if banked { 6 } else { 4 };
        if (e - o) % w != 0 {
            return why_not(|| "a DPCM table of odd size".into());
        }
        let entries: Vec<&[u8]> = m[o..e].chunks(w).collect();
        // each sample: where it is in the NSF, and its length (`size` sixteens and one)
        let place = |c: &[u8]| -> usize {
            if banked {
                0x80 + usize::from(c[4]) * 0x1000 + usize::from(c[2]) * 64
            } else {
                0x80 + 0xc000 + usize::from(c[2]) * 64 - 0x8000
            }
        };
        let mut starts: Vec<(usize, u8)> = entries.iter().filter(|c| c.iter().any(|&b| b != 0)).map(|c| (place(c), c[3])).collect();
        starts.sort();
        starts.dedup_by_key(|x| x.0);
        for (k, &(at, size)) in starts.iter().enumerate() {
            let len = usize::from(size) * 16 + 1;
            files.push((format!("d{k:03}.dmc"), nsf.get(at..at + len)?.to_vec()));
        }
        if !banked {
            let first = starts.first()?.0;
            offset_pcm = Some(first - 0x80 + 0x8000);
        }
        for (note, c) in entries.iter().enumerate() {
            if c.iter().all(|&b| b == 0) {
                continue;
            }
            let k = starts.iter().position(|x| x.0 == place(c))?;
            let (freq, mode) = (c[0] & 15, c[0] >> 6);
            if c[0] & 0x30 != 0 || (mode == 2 && !banked) || mode == 3 {
                return why_not(|| "a DPCM entry nsc can't write".into());
            }
            let mut l = format!(" n{note}, \"d{k:03}.dmc\", {freq}, {mode}, {}", c[1] as i8);
            if banked && mode == 2 {
                l += &format!(", {}", c[5] as i8);
            }
            l += ",0";
            if c[3] != starts[k].1 {
                l += &format!(",{}", c[3]);
            }
            dpcm_lines.push(l);
        }
    }
    // The header and its metadata.
    let mut head: Vec<Vec<u8>> = vec![format!("#Code \"{code_name}.bin\"").into_bytes(), b"#RepeatMode 1".to_vec(), format!("#BGM {nbgm}").into_bytes(), format!("#SE {nse}").into_bytes()];
    if banked {
        head.push(b"#Bank".to_vec());
    }
    if let Some(p) = offset_pcm {
        head.push(format!("#OffsetPCM {p}").into_bytes());
    }
    if nsf[0x7b] != rom[0x7b] {
        head.push(format!("#External {}", nsf[0x7b]).into_bytes());
    }
    let field = |o: usize| -> Vec<u8> {
        let f = &nsf[o..o + 32];
        f[..f.iter().position(|&c| c == 0).unwrap_or(32)].to_vec()
    };
    let (mut title, mut composer, mut copyright, mut maker) = (field(0x0e), field(0x2e), field(0x4e), Vec::new());
    let mut text: Option<Vec<u8>> = None;
    let mut tracks_meta: Vec<(Option<Vec<u8>>, Option<Vec<u8>>, Option<i32>, Option<i32>)> = vec![(None, None, None, None); nbgm + nse];
    let mut chunks_head = Vec::new();
    if nsf[0x7c] & 0x80 != 0 {
        let mut i = data_end;
        let strings = |d: &[u8]| -> Vec<Vec<u8>> {
            let mut v: Vec<Vec<u8>> = d.split(|&c| c == 0).map(|s| s.to_vec()).collect();
            v.pop();
            v
        };
        while i + 8 <= nsf.len() {
            let len = word(nsf, i)? | word(nsf, i + 2)? << 16;
            let id = &nsf[i + 4..i + 8];
            let data = nsf.get(i + 8..i + 8 + len)?;
            match id {
                b"auth" => {
                    let s = strings(data);
                    if s.len() != 4 {
                        return None;
                    }
                    (title, composer, copyright, maker) = (s[0].clone(), s[1].clone(), s[2].clone(), s[3].clone());
                }
                b"text" => text = Some(data.strip_suffix(&[0])?.to_vec()),
                b"NEND" => {}
                b"tlbl" | b"taut" => {
                    let s = strings(data);
                    if s.len() != nbgm + nse {
                        return None;
                    }
                    for (k, v) in s.into_iter().enumerate() {
                        if id == b"tlbl" {
                            tracks_meta[k].0 = Some(v);
                        } else {
                            tracks_meta[k].1 = Some(v);
                        }
                    }
                }
                b"time" | b"fade" => {
                    if data.len() != 4 * (nbgm + nse) {
                        return None;
                    }
                    for (k, c) in data.chunks(4).enumerate() {
                        let v = i32::from_le_bytes([c[0], c[1], c[2], c[3]]);
                        if id == b"time" {
                            tracks_meta[k].2 = Some(v);
                        } else {
                            tracks_meta[k].3 = Some(v);
                        }
                    }
                }
                b"plst" | b"psfx" => {
                    let add = if id == b"psfx" { nbgm } else { 0 };
                    let v: Vec<String> = data.iter().map(|&x| (usize::from(x).wrapping_sub(add)).to_string()).collect();
                    chunks_head.push(format!("#{} {{{}}}", String::from_utf8_lossy(id), v.join(",")));
                }
                b"VRC7" if data.len() == 1 => chunks_head.push(format!("#VRC7 {}", data[0])),
                b"mixe" => {
                    let chips = ["APU1", "APU2", "VRC6", "VRC7", "FDS", "MMC5", "N163", "SN5B"];
                    let mut s = Vec::new();
                    for c in data.chunks(3) {
                        let v = i16::from_le_bytes([*c.get(1)?, *c.get(2)?]);
                        s.push(format!("{} {v}", chips.get(usize::from(c[0]))?));
                    }
                    chunks_head.push(format!("#mixe {{{}}}", s.join(",")));
                }
                _ => return None,
            }
            i += 8 + len;
        }
    }
    for (name, v) in [("#Title", &title), ("#Composer", &composer), ("#Copyright", &copyright), ("#Maker", &maker)] {
        if !v.is_empty() {
            head.push([format!("{name} ").into_bytes(), esc(v)].concat());
        }
    }
    if let Some(t) = &text {
        head.push([b"#Text ".to_vec(), esc(t)].concat());
    }
    head.extend(chunks_head.into_iter().map(String::into_bytes));
    // The items, in order.
    let mut body: Vec<Vec<u8>> = Vec::new();
    for &(o, w, n) in &numbered {
        match w {
            What::Bgm(_) | What::Se(_) => {
                let se = matches!(w, What::Se(_));
                body.push(format!("{}({n}) {{", if se { "SE" } else { "BGM" }).into_bytes());
                let k = if se { nbgm + n } else { n };
                let (l, a, t, f) = &tracks_meta[k];
                if let Some(l) = l.as_ref().filter(|l| !l.is_empty()) {
                    body.push([b" #Title ".to_vec(), esc(l)].concat());
                }
                if let Some(a) = a.as_ref().filter(|a| !a.is_empty()) {
                    body.push([b" #Author ".to_vec(), esc(a)].concat());
                }
                if let Some(t) = t.filter(|&t| t >= 0) {
                    body.push(format!(" #Time {t}").into_bytes());
                }
                if let Some(f) = f.filter(|&f| f >= 0) {
                    body.push(format!(" #Fade {f}").into_bytes());
                }
                if se && m[o + 1] != 0 {
                    body.push(format!(" #Priority {}", m[o + 1]).into_bytes());
                } else if !se && m[o + 1] != 0 {
                    return None;
                }
                let count = usize::from(m[o]);
                let mut trs = Vec::new();
                for tr in 0..count {
                    let p = word(m, o + 2 + 2 * tr)?;
                    let toks = if p == 0 { Vec::new() } else { d.tokens(o + p).or_else(|| why_not(|| format!("a track at {:#x}", o + p)))? };
                    trs.push((tr, toks));
                }
                // `j0` puts a jump-on at TR1's start unless a `j1` came first in the block: tracks with a `j1` first
                trs.sort_by_key(|(_, t)| !t.iter().any(|x| x == "j1"));
                for (tr, toks) in trs {
                    body.push(format!(" TR{} {}", tr + 1, toks.join(" ")).into_bytes());
                }
                body.push(b"}".to_vec());
            }
            What::Sub => body.push(format!("Sub({n}) {{ {} }}", d.tokens(o)?.join(" ")).into_bytes()),
            What::Env => {
                let release = usize::from(m[o]);
                let len = env_len(m, o)?;
                let el = |pt: usize| m[o + pt];
                let loops: Vec<usize> = (1..len).filter(|&pt| el(pt) >= 0xc0).collect();
                let last = *loops.last()?;
                // A loop byte pointing at the element just before it is what nsc writes with no `|`.
                let real = |pt: usize| usize::from(el(pt) & 0x3f) != pt - 1;
                let first = loops[0];
                // `|` before the first `R` sets the loop every `R` writes; without one each `R` points at the
                // element before it
                let rs: Vec<usize> = loops.iter().copied().filter(|&p| p != last || release == 0).collect();
                let normal_lp = rs.iter().any(|&p| real(p)).then(|| usize::from(el(first) & 0x3f)).filter(|&p| p < first);
                // the release part's loop is set by a `|` after the first `R`
                let release_lp = (release != 0 && real(last)).then(|| usize::from(el(last) & 0x3f));
                if release_lp.is_some_and(|p| p <= first) {
                    return why_not(|| "an envelope's release loop before its R".into());
                }
                // the value nsc last wrote in this part (an equal one would only lengthen it)
                let mut cur: Option<u8> = None;
                let mut s = Vec::new();
                let mut hold = 0usize;
                let flush = |s: &mut Vec<String>, hold: &mut usize| {
                    if *hold > 0 {
                        s.push(format!("D{}", *hold - 1));
                        *hold = 0;
                    }
                };
                for pt in 1..len {
                    let b = el(pt);
                    let in_release = release != 0 && pt >= release;
                    let _ = in_release;
                    if b >= 0xc0 {
                        flush(&mut s, &mut hold);
                        if pt != last {
                            s.push("R".into());
                        }
                        cur = None;
                        continue;
                    }
                    if normal_lp == Some(pt) || (release_lp == Some(pt) && pt > first) {
                        flush(&mut s, &mut hold);
                        s.push("|".into());
                        cur = None;
                    }
                    if b < 0x80 {
                        flush(&mut s, &mut hold);
                        // a sweep of one step writes its value whatever came before
                        s.push(if cur == Some(b) { format!("({b},{b},1)") } else { b.to_string() });
                        cur = Some(b);
                    } else {
                        hold += usize::from(b & 0x0f) + 1;
                    }
                }
                body.push(format!("E({n}) {{ {} }}", s.join(" ")).into_bytes());
            }
            What::Fdsc | What::Fdsm => {
                let len = if w == What::Fdsc { 64 } else { 32 };
                let v: Vec<String> = m[o..o + len].iter().map(|b| b.to_string()).collect();
                body.push(format!("{}({n}) {{ {} }}", if w == What::Fdsc { "FDSC" } else { "FDSM" }, v.join(",")).into_bytes());
            }
            What::Vrc7 => {
                let v: Vec<String> = m[o..o + 8].iter().map(|b| b.to_string()).collect();
                body.push(format!("VRC7({n}) {{ @R {} }}", v.join(",")).into_bytes());
            }
            What::N163 => {
                let k = usize::from(m[o]);
                let mut v = Vec::new();
                for &b in &m[o + 1..o + 1 + k] {
                    v.push((b & 15).to_string());
                    v.push((b >> 4).to_string());
                }
                body.push(format!("N163({n}) {{ {} }}", v.join(",")).into_bytes());
            }
            What::Dpcm => {
                body.push(b"DPCM {".to_vec());
                body.extend(dpcm_lines.drain(..).map(String::into_bytes));
                body.push(b"}".to_vec());
            }
        }
    }
    let mut out = Vec::new();
    for l in head.iter().chain(&body) {
        out.extend_from_slice(l);
        out.extend_from_slice(b"\r\n");
    }
    Some(Decompiled { mml: out, files })
}

/// Why an NSF doesn't come back, for the tests.
#[cfg(test)]
pub(crate) fn why(nsf: &[u8]) -> String {
    WHY.with(|w| w.borrow_mut().clear());
    let Some(d) = write(nsf) else { return format!("no MML: {}", WHY.with(|w| w.borrow().clone())) };
    let files = d.files.clone();
    let reader = move |f: &str| files.iter().find(|(n, _)| n == f).map(|(_, b)| b.clone());
    match compile_with(&d.mml, &reader) {
        Err(e) => format!("nsc stops: {e}"),
        Ok(o) => match link::nsf(&o) {
            Err(e) => format!("link: {e}"),
            Ok(got) => match (0..got.len().max(nsf.len())).find(|&i| got.get(i) != nsf.get(i)) {
                None => "the same".into(),
                Some(i) => format!("differs at {i:#x} (of {:#x}/{:#x})", nsf.len(), got.len()),
            },
        },
    }
}
