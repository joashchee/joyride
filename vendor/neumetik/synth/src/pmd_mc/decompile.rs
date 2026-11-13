// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.
//
//! `.M` back to MML: PMD's compiled song turned into MC's MML that [`super::compile`] makes the same bytes of.
//! Each command byte is written as the MML MC writes exactly it with (`V` for a volume byte, `)%n` for a step,
//! `%n` lengths, `Z` for a bar length, the part-start commands as commands of part G), the loops and `L` as they
//! stand, the voices (`#Option /v`) as `@` definitions and the memo strings as their directives. MC merges a tie
//! to the same note and a rest after a rest, and has no way to keep them apart, so a `.M` holding those (other
//! compilers made it, or an older MC) can't come back: [`decompile`] compiles what it wrote and gives it only
//! when the bytes are the same. [`to_mml`] also takes MC 4.0 to 4.7's files and those merges: it gives MML that
//! compiles to the same music (every part's commands, the voices and the memo strings the same, rests and same-note
//! ties merged, loop addresses and counters aside).
//!
//! Checked on the MDXO archive's 4,593 `.M` files: 2,945 come back byte for byte and 1,447 more as the same music;
//! of the rest, 176 aren't laid out as PMD's (flag 6, mostly in its `_Error` folder) and about 25 hold values MC's
//! MML can't write (voice bits it never sets, a pan of 20, `\V64`). And on 2,767 random sources run through
//! MC.EXE: 2,747 byte for byte, 14 as the same music.

use super::compile;

thread_local! {
    /// Where [`write`] last gave up, for the tests.
    static WHY: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

fn why_not(s: impl FnOnce() -> String) -> Option<()> {
    if cfg!(test) {
        WHY.with(|w| *w.borrow_mut() = s());
    }
    None
}

/// What kind of part a stream is, for the commands whose MML differs.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Fm,
    Ssg,
    Pcm,
    /// The K part: pattern numbers and the rhythm source.
    K,
    /// An `R` pattern.
    R,
}

const KEYS: [&str; 13] = ["c", "c+", "d", "d+", "e", "f", "f+", "g", "g+", "a", "a+", "b", "x"];
const DRUMS: [u8; 6] = [b'b', b's', b'c', b'h', b't', b'i'];

struct Part {
    out: Vec<String>,
    octave: u8,
}

impl Part {
    fn new() -> Part {
        Part { out: Vec::new(), octave: 3 }
    }

    fn note(&mut self, b: u8) -> Option<String> {
        let (oct, key) = (b >> 4, b & 15);
        if key as usize >= KEYS.len() || oct >= 8 {
            return None;
        }
        let mut s = String::new();
        if oct != self.octave {
            s += &format!("o{}", oct + 1);
            self.octave = oct;
        }
        s += KEYS[usize::from(key)];
        Some(s)
    }
}

fn i8s(v: u8) -> String {
    let v = v as i8;
    if v < 0 {
        v.to_string()
    } else {
        format!("+{v}")
    }
}

fn i16s(lo: u8, hi: u8) -> String {
    i16::from_le_bytes([lo, hi]).to_string()
}

/// One part's commands from `at` to its end mark (0x80; 0xff for an `R` pattern). Returns the MML tokens.
fn part(m: &[u8], mut at: usize, kind: Kind) -> Option<Vec<String>> {
    let mut p = Part::new();
    let get = |i: usize| m.get(i).copied();
    let end = if kind == Kind::R { 0xff } else { 0x80 };
    // The last note or rest and its length, while a tie may follow it.
    let mut last: Option<(u8, u8)> = None;
    loop {
        let Some(b) = get(at) else {
            why_not(|| "a part runs off the file".into())?;
            unreachable!()
        };
        at += 1;
        if b == end {
            return Some(p.out);
        }
        let a1 = get(at);
        // A tie to the same note: MC merges it unless the two lengths pass 255, where `&%n` writes it as it is.
        if b == 0xfb {
            if let Some((nb, len)) = last {
                if get(at) == Some(nb) && get(at + 1).is_some_and(|n| u16::from(n) + u16::from(len) > 255) {
                    let n = get(at + 1)?;
                    at += 2;
                    p.out.push(format!("&%{n}"));
                    last = Some((nb, n));
                    continue;
                }
            }
        }
        last = None;
        let tok: String = match b {
            0x00..=0x7f if kind == Kind::K => format!("R{b}"),
            0x0f => {
                let n = a1?;
                at += 1;
                if n == 0 {
                    why_not(|| "a rest 0 long".into())?;
                }
                if kind != Kind::R {
                    last = Some((b, n));
                }
                format!("r%{n}")
            }
            0x00..=0x7f if kind == Kind::R => return None,
            0x00..=0x7f => {
                let n = a1?;
                at += 1;
                if n == 0 {
                    why_not(|| "a note 0 long".into())?;
                }
                let Some(t) = p.note(b) else {
                    why_not(|| format!("note {b:02x}"))?;
                    unreachable!()
                };
                last = Some((b, n));
                format!("{t}%{n}")
            }
            0x80..=0xbf if kind == Kind::R => {
                let (lo, n) = (a1?, get(at + 1)?);
                at += 2;
                if n == 0 {
                    return None;
                }
                format!("@{}c%{n}", u16::from(b & 0x3f) << 8 | u16::from(lo))
            }
            _ => {
                let Some((t, used)) = command(m, at, b, kind, &mut p) else {
                    let args: Vec<String> = m.get(at..at + 4).unwrap_or(&[]).iter().map(|x| format!("{x:02x}")).collect();
                    why_not(|| format!("command {b:02x} in {} [{}]", ["FM", "SSG", "PCM", "K", "R"][kind as usize], args.join(" ")))?;
                    unreachable!()
                };
                at += used;
                t
            }
        };
        p.out.push(tok);
    }
}

/// A command byte `b` (0xb1 and up), its arguments at `at`: the MML, and how many argument bytes it took.
fn command(m: &[u8], at: usize, b: u8, kind: Kind, p: &mut Part) -> Option<(String, usize)> {
    let a = |i: usize| m.get(at + i).copied();
    let one = |s: String| Some((s, 1));
    Some(match b {
        0xff => match kind {
            Kind::Fm => (format!("@{}", a(0)?), 1),
            Kind::Pcm if a(1) == Some(0xce) => {
                let w = |i: usize| -> Option<String> { Some(i16s(a(i)?, a(i + 1)?)) };
                (format!("@{},{},{},{}", a(0)?, w(2)?, w(4)?, w(6)?), 8)
            }
            Kind::Pcm => (format!("@{}", a(0)?), 1),
            _ => return None,
        },
        0xfe => {
            let q = a(0)?;
            if a(1) == Some(0xb1) {
                let d = a(2)?;
                let to = if d & 0x80 != 0 { i16::from(q) - i16::from(d & 0x7f) } else { i16::from(q) + i16::from(d) };
                if !(0..=255).contains(&to) || d == 0 {
                    return None;
                }
                (format!("q{q}-{to}"), 3)
            } else {
                (format!("q{q}"), 1)
            }
        }
        0xfd => one(format!("V{}", a(0)?))?,
        0xfc => match a(0)? {
            0xff => (format!("t{}", a(1)?), 2),
            0xfe if a(1)? != 0 => (format!("T{}", i8s(a(1)?)), 2),
            0xfd if a(1)? != 0 => (format!("t{}", i8s(a(1)?)), 2),
            n if n < 251 => (format!("T{n}"), 1),
            _ => return None,
        },
        0xfb => ("&".into(), 0),
        0xfa => (format!("D{}", i16s(a(0)?, a(1)?)), 2),
        0xf9 => ("[".into(), 2),
        0xf8 => (format!("]{}", a(0)?), 4),
        0xf7 => (":".into(), 2),
        0xf6 => ("L".into(), 0),
        0xf5 => one(format!("_{}", i8s(a(0)?)))?,
        0xf4 => (")".into(), 0),
        0xf3 => ("(".into(), 0),
        0xf2 | 0xbf => (format!("M{}{},{},{},{}", if b == 0xf2 { 'A' } else { 'B' }, a(0)?, a(1)?, a(2)?, a(3)?), 4),
        0xc2 | 0xb9 => one(format!("M{}{}", if b == 0xc2 { 'A' } else { 'B' }, a(0)?))?,
        0xf1 => one(format!("*A{}", a(0)?))?,
        0xbe => one(format!("*B{}", a(0)?))?,
        0xf0 => (format!("E{},{},{},{}", a(0)?, a(1)?, a(2)?, a(3)?), 4),
        0xcd => {
            let x = a(3)?;
            (format!("E{},{},{},{},{},{}", a(0)?, a(1)?, a(2)?, x & 15, x >> 4, a(4)?), 5)
        }
        0xc9 => one(format!("EX{}", a(0)?))?,
        0xef => (format!("y{},{}", a(0)?, a(1)?), 2),
        0xee => one(format!("w{}", a(0)?))?,
        0xd0 => one(format!("w{}", i8s(a(0)?)))?,
        0xed => one(format!("P{}", [0u8, 7, 56, 63].iter().position(|&v| v == a(0).unwrap_or(1))?))?,
        0xec if a(0)? < 4 => one(format!("p{}", a(0)?))?,
        0xc3 if a(1)? < 2 => {
            let s = if a(1)? == 1 { format!("px{},1", i8s(a(0)?)) } else { format!("px{}", i8s(a(0)?)) };
            (s, 2)
        }
        0xeb => {
            let v = a(0)?;
            let bits = v & 0x7f;
            if bits & 0x40 != 0 || bits == 0 {
                return None;
            }
            if v & 0x80 != 0 {
                if bits.count_ones() != 1 {
                    return None;
                }
                one(format!("\\{}p", DRUMS[bits.trailing_zeros() as usize] as char))?
            } else {
                let s: String = (0..6).filter(|k| bits & (1 << k) != 0).map(|k| format!("\\{}", DRUMS[k] as char)).collect();
                one(s)?
            }
        }
        0xe8 if a(0)? < 64 => one(format!("\\V{}", a(0)?))?,
        0xe6 => one(format!("\\V{}", i8s(a(0)?)))?,
        0xea => {
            let v = a(0)?;
            let d = usize::from(v >> 5);
            if !(1..=6).contains(&d) {
                return None;
            }
            one(format!("\\v{}{}", DRUMS[d - 1] as char, v & 31))?
        }
        0xe5 => {
            let d = usize::from(a(0)?);
            if !(1..=6).contains(&d) {
                return None;
            }
            (format!("\\v{}{}", DRUMS[d - 1] as char, i8s(a(1)?)), 2)
        }
        0xe9 => {
            let v = a(0)?;
            let d = usize::from(v >> 5);
            let side = match v & 31 {
                2 => 'l',
                3 => 'm',
                1 => 'r',
                _ => return None,
            };
            if !(1..=6).contains(&d) {
                return None;
            }
            one(format!("\\{side}{}", DRUMS[d - 1] as char))?
        }
        0xe7 => one(format!("__{}", i8s(a(0)?)))?,
        0xb2 => one(format!("_M{}", i8s(a(0)?)))?,
        0xe3 => one(format!("){}", format_args!("%{}", a(0)?)))?,
        0xe2 => one(format!("({}", format_args!("%{}", a(0)?)))?,
        0xde if a(0)? != 0 => one(format!(")^%{}", a(0)?))?,
        0xdd if a(0)? != 0 => one(format!("(^%{}", a(0)?))?,
        0xe1 if a(0)? & 0xc8 == 0 => one(format!("H{},{}", a(0)? & 7, a(0)? >> 4))?,
        0xe4 => one(format!("#D{}", a(0)?))?,
        0xd7 => one(format!("#f{}", a(0)?))?,
        0xd9 if a(0)? < 4 => one(format!("#w{}", a(0)?))?,
        0xd8 => {
            let v = a(0)?;
            one(if v & 0x80 != 0 { format!("#p{}", v & 0x7f) } else { format!("#a{v}") })?
        }
        0xe0 => match a(0)? {
            0 => one("#0".into())?,
            v if v & 0xf8 == 8 => one(format!("#1,{}", v & 7))?,
            _ => return None,
        },
        0xdf => one(format!("Z{}", a(0)?))?,
        0xdc => one(format!("~{}", a(0)?))?,
        0xdb => one(format!("~{}", i8s(a(0)?)))?,
        0xd2 => one(format!("F{}", a(0)?))?,
        0xb6 => {
            let v = a(0)?;
            let s = if v < 8 {
                format!("FB{v}")
            } else if (0xf9..=0xff).contains(&v) {
                format!("FB{}", v as i8)
            } else if (0x80..=0x87).contains(&v) {
                format!("FB+{}", v & 0x7f)
            } else {
                return None;
            };
            one(s)?
        }
        0xd4 => one(format!("n{}", a(0)?))?,
        0xd3 => one(format!("N{}", a(0)?))?,
        0xd5 => (format!("DD{}", i16s(a(0)?, a(1)?)), 2),
        0xcc => one(format!("DX{}", a(0)?))?,
        0xc0 => match a(0)? {
            v @ 0..=1 => one(format!("m{v}"))?,
            0xf7 => (format!("A{}", a(1)?), 2),
            sub @ (0xf5..=0xff) => {
                let (letter, rel) = match sub {
                    0xff => ('F', false),
                    0xfe => ('F', true),
                    0xfd => ('S', false),
                    0xfc => ('S', true),
                    0xfb => ('P', false),
                    0xfa => ('P', true),
                    0xf9 => ('R', false),
                    0xf8 => ('R', true),
                    0xf6 => ('Z', false),
                    0xf5 => ('Z', true),
                    _ => return None,
                };
                let v = a(1)?;
                (if rel { format!("D{letter}{}", i8s(v)) } else { format!("D{letter}{v}") }, 2)
            }
            _ => return None,
        },
        0xcf => one(format!("s{},{}", a(0)? >> 4, a(0)? & 15))?,
        0xc8 | 0xc7 => (format!("s{}{},{}", if b == 0xc8 { "d" } else { "dd" }, a(0)?, i16s(a(1)?, a(2)?)), 3),
        0xb5 => {
            let (d, x) = (a(0)?, a(1)?);
            (if d == 0 && x == 0 { "sk0".into() } else { format!("sk{},{x}", d) }, 2)
        }
        0xcb | 0xbc => one(format!("MW{}{}", if b == 0xcb { 'A' } else { 'B' }, a(0)?))?,
        0xca | 0xbb => one(format!("MX{}{}", if b == 0xca { 'A' } else { 'B' }, a(0)?))?,
        0xc5 | 0xba => one(format!("MM{}{}", if b == 0xc5 { 'A' } else { 'B' }, a(0)?))?,
        0xd6 | 0xbd => {
            let letter = if b == 0xd6 { 'A' } else { 'B' };
            let (x, y) = (a(0)?, a(1)?);
            if x != 0 && a(2) == Some(0xb7) {
                let z = a(3)?;
                if (z & 0x80 != 0) != (b == 0xbd) {
                    return None;
                }
                (format!("MD{letter}{x},{y},{}", z & 0x7f), 4)
            } else {
                (format!("MD{letter}{x},{y}"), 2)
            }
        }
        0xc4 => one(format!("Q%{}", !a(0)?))?,
        0xb3 => one(format!("q,{}", a(0)?))?,
        0xc1 => ("&&".into(), 0),
        0xda => {
            let (n1, n2, len) = (a(0)?, a(1)?, a(2)?);
            if n1 & 15 == 15 || n2 & 15 == 15 || len == 0 || n1 >= 0x80 || n2 >= 0x80 {
                return None;
            }
            let s = format!("{{{} {}}}%{len}", p.note(n1)?, p.note(n2)?);
            (s, 3)
        }
        0xb8 => {
            let (x, y) = (a(0)?, a(1)?);
            let s = if x < 16 {
                format!("O{x},{y}")
            } else if x & 0xf0 == 0xf0 {
                format!("O{},{}", x & 15, i8s(y))
            } else {
                return None;
            };
            (s, 2)
        }
        0xd2..=0xff => return None,
        _ => return None,
    })
}

fn word(m: &[u8], at: usize) -> Option<usize> {
    Some(usize::from(*m.get(at)?) | usize::from(*m.get(at + 1)?) << 8)
}

/// A memo string at `at` (to its zero), as a directive's text: printable bytes and tabs only.
fn text(m: &[u8], at: usize) -> Option<Vec<u8>> {
    let n = m.get(at..)?.iter().position(|&c| c == 0)?;
    let s = &m[at..at + n];
    if s.iter().any(|&c| c < b' ' && c != 9 && c != 0x1b) || s.first().is_some_and(|&c| c == b' ' || c == 9) {
        return None;
    }
    Some(s.to_vec())
}

/// The voice definitions of `#Option /v`, from the voice table at `at`: `@` lines.
fn voices(m: &[u8], mut at: usize, end: usize, opl: bool, dt2: &mut bool) -> Option<Vec<String>> {
    let mut lines = Vec::new();
    loop {
        // the end mark; else where the table must end (the memo strings, the file's end)
        if at >= end {
            return Some(lines);
        }
        let n = *m.get(at)?;
        if n == 0 && m.get(at + 1) == Some(&0xff) {
            return Some(lines);
        }
        at += 1;
        if opl {
            const TABLE: [(usize, u8, u32); 24] = [
                (8, 1, 0), (8, 7, 1),
                (4, 15, 4), (4, 15, 0), (6, 15, 0), (6, 15, 4), (2, 63, 0), (2, 3, 6), (0, 15, 0), (0, 1, 4), (0, 1, 5), (0, 1, 6), (0, 1, 7),
                (5, 15, 4), (5, 15, 0), (7, 15, 0), (7, 15, 4), (3, 63, 0), (3, 3, 6), (1, 15, 0), (1, 1, 4), (1, 1, 5), (1, 1, 6), (1, 1, 7),
            ];
            let v = m.get(at..at + 9)?;
            let ps: Vec<String> = TABLE.iter().map(|&(o, max, rot)| ((v[o] >> rot) & max).to_string()).collect();
            lines.push(format!("@{n} {}", ps.join(" ")));
            at += 9;
            continue;
        }
        let v = m.get(at..at + 25)?;
        at += 25;
        let mut s = format!("@{n} {} {}", v[24] & 7, (v[24] >> 3) & 7);
        if v[24] & 0xc0 != 0 {
            return None;
        }
        // slot order 1, 3, 2, 4 at a stride of four bytes
        for k in [0usize, 2, 1, 3] {
            let b = |i: usize| v[k + 4 * i];
            if b(0) & 0x80 != 0 || b(1) & 0x80 != 0 || b(2) & 0x20 != 0 || b(3) & 0x60 != 0 {
                return None;
            }
            if b(4) & 0xc0 != 0 {
                *dt2 = true;
            }
            s += &format!(
                "\r\n {} {} {} {} {} {} {} {} {}",
                b(2) & 0x1f,
                b(3) & 0x1f,
                b(4) & 0x1f,
                b(5) & 15,
                b(5) >> 4,
                b(1),
                b(2) >> 6,
                b(0) & 15,
                (b(0) >> 4) & 7
            );
            s += &format!(" {{DT2:{}}} {}", b(4) >> 6, b(3) >> 7);
        }
        lines.push(s);
    }
}

/// A `.M`'s layout: its parts, patterns, voices, memo strings and version, as MC 4.1 to 4.8 lay them out.
struct Layout {
    opl: bool,
    x68: bool,
    towns: bool,
    /// Part A starts after the voice table's pointer (`#Option /v`, and every MC that has the word).
    prg: bool,
    /// Each part's first command (after the heads MC puts in: `c6`, `b4`, part G's `#Zenlength`).
    parts: Vec<usize>,
    fm3: Vec<usize>,
    pcm_ext: Vec<usize>,
    zenlen: u8,
    patterns: Vec<usize>,
    voices: Option<usize>,
    /// Where the voice table must end: the first memo string (MC 4.5 and before point at the strings when there
    /// are no voices), else the file's end.
    voices_end: usize,
    /// The memo strings by what they are (`#PPZFile`, `#PPSFile`, `#PCMFile`, `#Title`, `#Composer`, `#Arranger`,
    /// then each `#Memo`), absent ones empty.
    strings: Vec<Vec<u8>>,
}

const NAMES: [&str; 6] = ["#PPZFile", "#PPSFile", "#PCMFile", "#Title", "#Composer", "#Arranger"];

fn layout(file: &[u8], towns_46: bool) -> Option<Layout> {
    let flag = *file.first()?;
    if flag & !3 != 0 {
        why_not(|| "not laid out as MC lays it out".into())?;
    }
    let m = &file[1..];
    let mut parts: Vec<usize> = (0..11).map(|i| word(m, 2 * i)).collect::<Option<_>>()?;
    let r_table = word(m, 22)?;
    let prg = parts[0] == 26;
    if !prg && parts[0] != 24 {
        why_not(|| "not laid out as MC lays it out".into())?;
    }
    // Towns (MC 4.8's version 0x46): K is a part like J, and there are no R patterns.
    let towns = towns_46
        && if prg {
            word(m, 24).and_then(|t| m.get(t.checked_sub(2)?..t)) == Some(&[0x46, 0xfe])
        } else {
            m.get(r_table + 2..r_table + 4) == Some(&[0x46, 0xfe])
        };
    // The R patterns: as many as the K part's highest pattern number and one (MC's `maxprg`).
    let count = if towns {
        0
    } else {
        let k_toks = part(m, parts[10], Kind::K)?;
        k_toks.iter().filter_map(|t| t.strip_prefix('R')?.parse::<usize>().ok()).max().map_or(0, |n| n + 1)
    };
    let patterns: Vec<usize> = (0..count).map(|i| word(m, r_table + 2 * i)).collect::<Option<_>>()?;
    let mut r_end = r_table + 2 * count;
    for &at in &patterns {
        let mut i = at;
        loop {
            let b = *m.get(i)?;
            i += 1;
            if b == 0xff {
                break;
            }
            if b < 0xc0 {
                i += if b & 0x80 != 0 { 2 } else { 1 };
                continue;
            }
            let (_, used) = command(m, i, b, Kind::R, &mut Part::new())?;
            i += used;
        }
        r_end = r_end.max(i);
    }
    // The version and the memo pointer: before the voice table when part A starts after its pointer (where PMD
    // looks), else after the R data (and `lc`'s 8 bytes from 4.8 on).
    let (ver, memo) = if prg {
        let t = word(m, 24)?;
        match (m.get(t.checked_sub(2)?), m.get(t - 1)) {
            (Some(&v), Some(&0xfe)) if (0x41..=0x48).contains(&v) => (v, Some(word(m, t - 4)?)),
            // before 4.1: no version and no memo strings
            _ => (0x40, None),
        }
    } else if m.get(r_end + 10..r_end + 12) == Some(&[0x48, 0xfe]) {
        (0x48, Some(word(m, r_end + 8)?))
    } else if m.get(r_end + 3) == Some(&0xfe) {
        (*m.get(r_end + 2)?, Some(word(m, r_end)?))
    } else if r_end == m.len() {
        (0x40, None)
    } else {
        why_not(|| "no memo pointer where MC puts it".into())?;
        unreachable!()
    };
    if !(0x40..=0x48).contains(&ver) {
        why_not(|| format!("version {ver:02x}"))?;
    }
    let mut strings = Vec::new();
    let mut i = memo.unwrap_or(0);
    let mut voices_end = m.len();
    while memo.is_some() {
        let p = word(m, i)?;
        if p == 0 {
            break;
        }
        voices_end = voices_end.min(p);
        let Some(t) = text(m, p) else {
            why_not(|| "a memo string MML can't hold".into())?;
            unreachable!()
        };
        strings.push(t);
        i += 2;
    }
    // MC 4.8 writes the PPZ file first (not on Towns), 4.2 on the PPS file, all the PCM file.
    let skip = if memo.is_none() {
        0
    } else if ver >= 0x48 && !towns {
        0
    } else if ver >= 0x42 {
        1
    } else {
        2
    };
    let mut named = vec![Vec::new(); skip];
    named.extend(strings);
    if named.len() < NAMES.len() {
        named.resize(NAMES.len(), Vec::new());
    }
    let mut fm3 = Vec::new();
    let mut pcm_ext = Vec::new();
    let mut zenlen = 96u8;
    if m.get(parts[0]) == Some(&0xc6) {
        for k in 0..3 {
            let w = word(m, parts[0] + 1 + 2 * k)?;
            if w != 0 {
                fm3.push(w);
            }
        }
        parts[0] += 7;
    }
    if m.get(parts[9]) == Some(&0xb4) {
        for k in 0..8 {
            let w = word(m, parts[9] + 1 + 2 * k)?;
            if w != 0 {
                pcm_ext.push(w);
            }
        }
        parts[9] += 17;
    }
    if m.get(parts[6]) == Some(&0xdf) && m.get(parts[6] + 1) != Some(&96) {
        zenlen = *m.get(parts[6] + 1)?;
        parts[6] += 2;
    }
    Some(Layout {
        opl: flag & 2 != 0,
        x68: flag & 1 != 0,
        towns,
        prg,
        parts,
        fm3,
        pcm_ext,
        zenlen,
        patterns,
        voices: if prg { word(m, 24) } else { None },
        voices_end,
        strings: named,
    })
}

impl Layout {
    fn kind(&self, i: usize) -> Kind {
        if i == 10 && self.towns {
            Kind::Pcm
        } else if i == 10 {
            Kind::K
        } else if i == 9 {
            Kind::Pcm
        } else if self.opl || self.x68 || i < 6 {
            Kind::Fm
        } else {
            Kind::Ssg
        }
    }

    /// Every part's stream: (its letter, its kind, where it starts).
    fn streams(&self) -> Vec<(char, Kind, usize)> {
        let mut v: Vec<(char, Kind, usize)> = self.parts.iter().enumerate().map(|(i, &at)| ((b'A' + i as u8) as char, self.kind(i), at)).collect();
        v.extend(self.fm3.iter().enumerate().map(|(k, &at)| (FM3_LETTERS[k], Kind::Fm, at)));
        v.extend(self.pcm_ext.iter().enumerate().map(|(k, &at)| (PCM_LETTERS[k], Kind::Pcm, at)));
        v
    }
}

const FM3_LETTERS: [char; 3] = ['X', 'Y', 'Z'];
const PCM_LETTERS: [char; 8] = ['L', 'M', 'N', 'O', 'P', 'Q', 'S', 'T'];

/// `.M` back to MC's MML (Shift JIS text, CR LF lines) that [`compile`] makes the same bytes of; None when there
/// is no such MML (see the module's notes).
pub fn decompile(m: &[u8]) -> Option<Vec<u8>> {
    [true, false].into_iter().find_map(|towns| {
        let mml = write(m, towns)?;
        (compile(&mml).ok()? == m).then_some(mml)
    })
}

/// How near [`to_mml`]'s MML comes back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fidelity {
    /// MC 4.8q compiles it to the very same bytes.
    Exact,
    /// The same music: every part's commands, the voices and the memo strings the same, but for what MC 4.8q
    /// can't write as it stands (a rest after a rest and a tie to the same note, which it merges into one; an
    /// older MC's version, memo layout and missing `lc` lengths).
    SameMusic,
}

/// `.M` back to MML, for any `.M` of MC 4.1 to 4.8 whose music MC 4.8q can write: the exact MML when there is one
/// ([`decompile`]), else MML that compiles to the same music.
pub fn to_mml(m: &[u8]) -> Option<(Vec<u8>, Fidelity)> {
    if let Some(mml) = decompile(m) {
        return Some((mml, Fidelity::Exact));
    }
    [true, false].into_iter().find_map(|towns| {
        let mml = write(m, towns)?;
        let got = compile(&mml).ok()?;
        (music(&got, towns)? == music(m, towns)?).then_some((mml, Fidelity::SameMusic))
    })
}

/// A `.M`'s music, for [`to_mml`]: each part's events with addresses and loop counters left out and rests and
/// same-note ties merged, the R patterns, the voices and the memo strings.
#[derive(PartialEq, Eq, Debug)]
struct Music {
    flag: u8,
    parts: Vec<Vec<(u8, u32, Vec<u8>)>>,
    voices: Vec<u8>,
    strings: Vec<Vec<u8>>,
}

fn music(file: &[u8], towns_46: bool) -> Option<Music> {
    let l = layout(file, towns_46)?;
    let m = &file[1..];
    let mut parts = Vec::new();
    let mut streams: Vec<(Kind, usize)> = l.streams().into_iter().map(|(_, k, at)| (k, at)).collect();
    streams.extend(l.patterns.iter().map(|&at| (Kind::R, at)));
    for (kind, mut at) in streams {
        // (note byte or command, length, the command's bytes)
        let mut ev: Vec<(u8, u32, Vec<u8>)> = Vec::new();
        let end = if kind == Kind::R { 0xff } else { 0x80 };
        loop {
            let b = *m.get(at)?;
            at += 1;
            if b == end {
                break;
            }
            if kind == Kind::K && b < 0x80 {
                ev.push((b, 0, Vec::new()));
                continue;
            }
            if kind == Kind::R && (0x80..0xc0).contains(&b) {
                ev.push((b, u32::from(*m.get(at + 1)?), vec![*m.get(at)?]));
                at += 2;
                continue;
            }
            if b < 0x80 {
                let n = u32::from(*m.get(at)?);
                at += 1;
                let len = ev.len();
                // a rest after a rest; a note tied to the same note
                // in an R pattern a rest after a strike lengthens it (a drum sounds once either way)
                if b == 0x0f && kind == Kind::R && len >= 1 && (0x80..0xc0).contains(&ev[len - 1].0) {
                    ev[len - 1].1 += n;
                    continue;
                }
                if b == 0x0f && len >= 1 && ev[len - 1].0 == 0x0f && ev[len - 1].2.is_empty() {
                    ev[len - 1].1 += n;
                    continue;
                }
                if len >= 2 && ev[len - 1].0 == 0xfb && ev[len - 2].0 == b && ev[len - 2].2.is_empty() {
                    ev.pop();
                    ev[len - 2].1 += n;
                    continue;
                }
                ev.push((b, n, Vec::new()));
                continue;
            }
            let (_, used) = command(m, at, b, kind, &mut Part::new())?;
            let mut args = m.get(at..at + used)?.to_vec();
            match b {
                // addresses, and the loop counter `lc` leaves
                0xf9 | 0xf7 => args.clear(),
                0xf8 => args.truncate(1),
                _ => {}
            }
            at += used;
            // a command's own event; its 0 length keeps it from merging
            ev.push((b, 0, args));
            if b != 0xfb {
                let i = ev.len() - 1;
                ev[i].2.push(0);
            }
        }
        parts.push(ev);
    }
    let voices = match l.voices {
        Some(at) => {
            let mut v = Vec::new();
            let mut i = at;
            let size = if l.opl { 10 } else { 26 };
            while i < l.voices_end && !(m.get(i)? == &0 && m.get(i + 1)? == &0xff) {
                v.extend_from_slice(m.get(i..i + size)?);
                i += size;
            }
            v
        }
        None => Vec::new(),
    };
    let mut strings = l.strings.clone();
    while strings.last().is_some_and(|s| s.is_empty()) && strings.len() > NAMES.len() {
        strings.pop();
    }
    Some(Music { flag: file[0], parts, voices, strings })
}

/// The MML, not yet checked; `towns_46`: version 0x46 is MC 4.8's Towns mode (else MC 4.6).
pub(crate) fn write(file: &[u8], towns_46: bool) -> Option<Vec<u8>> {
    let l = layout(file, towns_46)?;
    let m = &file[1..];
    let mut head: Vec<Vec<u8>> = Vec::new();
    let mut opts = Vec::new();
    if l.prg {
        opts.push("/v");
    }
    if l.x68 {
        opts.push("/m");
    }
    if l.opl {
        opts.push("/l");
    }
    if l.towns {
        opts.push("/t");
    }
    if !opts.is_empty() {
        head.push(format!("#Option {}", opts.join(" ")).into_bytes());
    }
    for (k, s) in l.strings.iter().enumerate() {
        if s.is_empty() {
            // an empty #Memo can't be written
            if k >= NAMES.len() {
                why_not(|| "an empty #Memo".into())?;
            }
            continue;
        }
        if k == 0 && l.towns {
            why_not(|| "a PPZ file on Towns".into())?;
        }
        let mut line = NAMES.get(k).copied().unwrap_or("#Memo").as_bytes().to_vec();
        line.push(b' ');
        line.extend_from_slice(s);
        head.push(line);
    }
    // `/m` turns DT2 on: its voices always have the parameter.
    let mut dt2 = l.x68;
    let mut voice_lines = Vec::new();
    if let Some(at) = l.voices {
        let Some(v) = voices(m, at, l.voices_end, l.opl, &mut dt2) else {
            why_not(|| "voices".into())?;
            unreachable!()
        };
        voice_lines = v;
    }
    let voice_lines: Vec<String> = voice_lines
        .into_iter()
        .map(|line| {
            if dt2 {
                line.replace("{DT2:", "").replace('}', "")
            } else {
                // without #DT2Flag there is no DT2 parameter (and every DT2 is 0)
                let mut o = String::new();
                let mut rest = line.as_str();
                while let Some(i) = rest.find("{DT2:") {
                    o += &rest[..i];
                    let j = rest[i..].find('}').unwrap();
                    rest = &rest[i + j + 2..];
                }
                o + rest
            }
        })
        .collect();
    if dt2 && !l.x68 {
        head.push(b"#DT2Flag on".to_vec());
    }
    if !l.fm3.is_empty() {
        head.push(format!("#FM3Extend {}", FM3_LETTERS[..l.fm3.len()].iter().collect::<String>()).into_bytes());
    }
    if !l.pcm_ext.is_empty() {
        head.push(format!("#PCMExtend {}", PCM_LETTERS[..l.pcm_ext.len()].iter().collect::<String>()).into_bytes());
    }
    if l.zenlen != 96 {
        head.push(format!("#Zenlength {}", l.zenlen).into_bytes());
    }
    let mut body: Vec<(char, Vec<String>)> = Vec::new();
    for (letter, kind, at) in l.streams() {
        body.push((letter, part(m, at, kind)?));
    }
    let mut out: Vec<u8> = Vec::new();
    for line in head {
        out.extend_from_slice(&line);
        out.extend_from_slice(b"\r\n");
    }
    for line in voice_lines {
        out.extend_from_slice(line.as_bytes());
        out.extend_from_slice(b"\r\n");
    }
    for (letter, toks) in body {
        for chunk in toks.chunks(32) {
            out.extend_from_slice(format!("{letter} {}\r\n", chunk.join(" ")).as_bytes());
        }
    }
    for &at in &l.patterns {
        out.extend_from_slice(format!("R {}\r\n", part(m, at, Kind::R)?.join(" ")).as_bytes());
    }
    Some(out)
}

/// Why a `.M` doesn't come back, for the tests: the MML written, and where its compile first differs.
#[cfg(test)]
pub(crate) fn why(m: &[u8]) -> String {
    WHY.with(|w| w.borrow_mut().clear());
    let Some(mml) = write(m, true).or_else(|| write(m, false)) else { return format!("no MML: {}", WHY.with(|w| w.borrow().clone())) };
    match compile(&mml) {
        Err(e) => format!("MC refuses it: {e:?}"),
        Ok(got) => match (0..got.len().max(m.len())).find(|&i| got.get(i) != m.get(i)) {
            None => "the same".into(),
            Some(i) => format!("differs at {i:#x} (of {:#x}/{:#x})", m.len(), got.len()),
        },
    }
}

/// Where two `.M`s' music first differs, for the tests.
#[cfg(test)]
pub(crate) fn music_diff(a: &[u8], b: &[u8]) -> String {
    let (Some(x), Some(y)) = (music(a, false), music(b, false)) else { return "no music".into() };
    if x.flag != y.flag {
        return "flag".into();
    }
    if x.voices != y.voices {
        return "voices".into();
    }
    if x.strings != y.strings {
        return format!("strings {:?} / {:?}", x.strings.iter().map(|s| String::from_utf8_lossy(s).to_string()).collect::<Vec<_>>(), y.strings.iter().map(|s| String::from_utf8_lossy(s).to_string()).collect::<Vec<_>>());
    }
    for (i, (p, q)) in x.parts.iter().zip(&y.parts).enumerate() {
        if let Some(k) = (0..p.len().max(q.len())).find(|&k| p.get(k) != q.get(k)) {
            return format!("stream {i} event {k}: {:x?} / {:x?}", &p[k.saturating_sub(2)..(k + 3).min(p.len())], &q[k.saturating_sub(2)..(k + 3).min(q.len())]);
        }
    }
    if x.parts.len() != y.parts.len() {
        return "stream count".into();
    }
    "the same music".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    // The `.M` files MC.EXE itself made in `pmd_mc_ref/` (FM3 and PCM extension parts, OPM, OPL, Towns, voices,
    // loops, the R part...): each comes back byte for byte.
    #[test]
    fn mc_exe_s_files_come_back() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/pmd_mc_ref");
        let mut n = 0;
        for e in std::fs::read_dir(&dir).unwrap() {
            let p = e.unwrap().path();
            if p.extension().is_some_and(|x| x == "m") {
                let m = std::fs::read(&p).unwrap();
                assert!(decompile(&m).is_some(), "{}: {}", p.display(), why(&m));
                n += 1;
            }
        }
        assert!(n >= 40, "{n}");
    }

    // Every command, in each kind of part it goes in, compiled, decompiled, and compiled back the same.
    #[test]
    fn every_command_comes_back() {
        let cmds = [
            "c%24 o5d+%12 o1x%3 r%7", "v10 V100 )3 (3 )%3 (%3 ) ( )^2 (^%3", "t120 T200 t+3 t-2 T+5", "q3 q3-5 q9-2 q,4 Q4 Q%5",
            "@5", "p2 px-3 px4,1", "[c%24 [d%24 : e%24]3 ]0 L f%24", "c%24& d%24 c%24&& d%24 c%200&%100", "D10 D-300 DD-3 DX1",
            "DF1 DF-1 DS2 DP+3 DR4 DZ-5", "_2 _-3 __4 _M-5", "MA1,2,3,4 MB5,6,7,8 MA9 MB10 MWA1 MWB2 MXA1 MXB0 MMA3 MMB4",
            "MDA3,4 MDB5,6,7 MDA0,2", "*A1 *B0", "E1,2,3,4 E1,2,3,4,5,6 EX1", "y1,2 w5 w+3 P0 P3", "m1 m0 A3", "s3 s1,5 sd2,-30 sdd3,40 sk0 sk2,5",
            "n1 N2 F3 FB5 FB-3 FB+2", "~3 ~+2 ~-1", "Z96 H3,2 #D5 #f3 #w2 #a7 #p9 #0 #1,5", "{c g}%24 {o3c o5c}%96", "O1,2 O3,-4",
        ];
        for part in ["A", "G", "J"] {
            for c in cmds {
                let src = format!("{part} {c}\r\n");
                let Ok(m) = compile(src.as_bytes()) else { continue };
                assert!(decompile(&m).is_some(), "{src}: {}", why(&m));
            }
        }
        for src in [
            "K R0 R1 \\b\\s\\c \\hp \\V30 \\V+3 \\vb20 \\vs-2 \\lc \\mh \\ri R1\r\nR @1c%24 r%12 @1023c%6\r\nR @2c%12 V10\r\n",
            "#Zenlength 192\r\n#Tempo 100\r\n#Timer 200\r\n#VolumeDown F-4,S+2,P8\r\n#Transpose 2\r\nA c%24\r\nG c%24\r\n",
            "#FM3Extend XYZ\r\n#PCMExtend LMN\r\nA c%24\r\nX d%24\r\nY e%24\r\nZ f%24\r\nL @1,2,3,4 g%24\r\nM a%24\r\nN b%24\r\n",
            "#Option /v\r\n#Title song\r\n#Composer me\r\n#Arranger you\r\n#PCMFile X.PPC\r\n#Memo one\r\n#Memo two\r\n@1 4 7\r\n 31 0 0 15 0 0 0 1 3 0\r\n 31 2 3 4 5 6 1 2 7 1\r\n 1 2 3 4 5 6 3 4 0 0\r\n 30 29 28 15 14 127 2 15 5 1\r\nA @1 c%24\r\n",
            "#Option /m /v\r\n@3 4 7\r\n 31 0 0 15 0 0 0 1 3 2 0\r\n 31 2 3 4 5 6 1 2 7 1 1\r\n 1 2 3 4 5 6 3 4 0 3 0\r\n 30 29 28 15 14 127 2 15 5 0 1\r\nA @3 c%24\r\nH c%24\r\n",
            "#Option /t\r\nA c%24\r\nK c%24\r\n",
        ] {
            let m = compile(src.as_bytes()).unwrap_or_else(|e| panic!("{src}: {e:?}"));
            assert!(decompile(&m).is_some(), "{src}: {}", why(&m));
        }
    }

    // A `.M` MC 4.8q can't write as it stands (two rests in a row, which it merges): no exact MML, but MML that
    // compiles to the same music.
    #[test]
    fn rests_in_a_row_come_back_as_the_same_music() {
        let m = compile(b"A c%24 r%48 d%24\r\nB e%24\r\n").unwrap();
        // the rest split in two, everything after it 2 bytes on (the part pointers, the memo pointer and its list)
        let at = m.windows(2).position(|w| w == [0x0f, 0x30]).unwrap();
        let mut x = m[..at].to_vec();
        x.extend_from_slice(&[0x0f, 0x18, 0x0f, 0x18]);
        x.extend_from_slice(&m[at + 2..]);
        let shift = |x: &mut Vec<u8>, w: usize| {
            let v = u16::from_le_bytes([x[w], x[w + 1]]);
            if usize::from(v) + 1 > at {
                x[w..w + 2].copy_from_slice(&(v + 2).to_le_bytes());
            }
        };
        for i in 0..12 {
            shift(&mut x, 1 + 2 * i);
        }
        let bp = x.windows(2).rposition(|w| w == [0x48, 0xfe]).unwrap() - 2;
        shift(&mut x, bp);
        let list = 1 + usize::from(u16::from_le_bytes([x[bp], x[bp + 1]]));
        let mut i = list;
        while x[i] != 0 || x[i + 1] != 0 {
            shift(&mut x, i);
            i += 2;
        }
        assert!(decompile(&x).is_none());
        let (mml, how) = to_mml(&x).unwrap();
        assert_eq!(how, Fidelity::SameMusic);
        assert_eq!(compile(&mml).unwrap(), m);
        assert_eq!(to_mml(&m).unwrap().1, Fidelity::Exact);
    }
}
