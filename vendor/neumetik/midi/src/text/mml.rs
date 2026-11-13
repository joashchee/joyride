// Copyright 2026 ansiapps. Neumetik MIDI, MIT licensed: see LICENSE.

//! **MML, Music Macro Language, for General MIDI**: one core with each
//! dialect's settings, from each one's published manual or help.
//!
//! The core: `c d e f g a b` with `+`/`#` sharp and `-` flat, a length
//! (1 a whole note to 64; any number, so 12 is a triplet eighth) and dots;
//! `r` a rest; `n` a note by number; `o` the octave, `>` and `<` up and
//! down one; `l` the length after; `t` quarter notes a minute; `v` the
//! volume; `&` ties (the same key held, else played legato); `^` adds a
//! length; `[ ... ]n` repeats; `@` an instrument.
//!
//! | Dialect | What's different |
//! |---|---|
//! | Mabinogi `MML@a,b,c;` | up to 3 tracks and a singer's; `t` sets every track; `v0`-`v15`, `n0`-`n96` |
//! | ArcheAge | Mabinogi's syntax |
//! | MapleStory 2 `.ms2mml` | XML holding a track's MML in each `<melody>` and `<chord>` |
//! | GW-BASIC, BASICA, QBasic `PLAY` | one voice; `P` rests; `MN`, `ML`, `MS` (7/8, all, 3/4 of a note sounds); `N0`-`N84`; `X` plays a string variable |
//! | MSX BASIC `PLAY` | 3 voices; `R` rests; `V0`-`V15`; `S` and `M` (the PSG's envelope) skipped |
//! | MSX-MUSIC `PLAY #2` | FM voices `@0`-`@63` (as GM stand-ins), rhythm `B S M C H` |
//! | N88-BASIC `PLAY` | MSX's syntax; `@` voices |
//! | C128 BASIC 7 `PLAY` | `V1`-`V3` voices, `W H Q I S` lengths, `#`/`$`, `T0`-`T9` envelopes, `TEMPO` |
//! | ZX Spectrum 128 `PLAY` | lengths 1-12, capitals an octave up, `&` rests, `_` ties, `( )` repeats, `!` comments |
//!
//! A BASIC program is read for its `PLAY` statements, string variables
//! (`A$="..."`, joined with `+`), `TEMPO`, and `FOR`/`NEXT` loops with
//! number bounds; nothing else of BASIC runs.

use super::{bad, Builder, Frac};
use crate::score::{Score, DRUMS};
use crate::Error;

/// Which MML a text is written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    /// Plain MML: Mabinogi's syntax, a track a line, `@` a GM program.
    Generic,
    Mabinogi,
    ArcheAge,
    MapleStory2,
    GwBasic,
    Msx,
    MsxMusic,
    N88,
    C128,
    Zx128,
    /// Sakura (TextSakura, sakuramml): compiled by its own compiler.
    Sakura,
}

/// The dialect a text looks like it's in.
pub fn detect(text: &str) -> Dialect {
    let upper = text.to_ascii_uppercase();
    if upper.contains("MML@") {
        return Dialect::Mabinogi;
    }
    if upper.contains("<MS2") {
        return Dialect::MapleStory2;
    }
    if upper.contains("CALL MUSIC") || upper.contains("PLAY #2") || upper.contains("PLAY#2") {
        return Dialect::MsxMusic;
    }
    // (Before PLAY: Sakura has a Play command of its own.)
    if super::sakura::looks_like(text) {
        return Dialect::Sakura;
    }
    if !upper.contains("PLAY") {
        return Dialect::Generic;
    }
    let strings = basic_strings(text);
    if upper.contains("TEMPO") || strings.iter().any(|s| s.contains("V1") || s.contains("V2") || s.contains("V3")) && strings.iter().any(|s| s.chars().any(|c| matches!(c, 'Q' | 'I' | 'W' | 'H'))) {
        return Dialect::C128;
    }
    let lower = strings.iter().any(|s| s.chars().any(|c| c.is_ascii_lowercase()));
    if lower && strings.iter().any(|s| s.contains('&') || s.contains('_') || s.contains('!') || s.contains('(')) {
        return Dialect::Zx128;
    }
    // Several strings to a PLAY: MSX (or N88).
    let multi = text.lines().any(|l| {
        let u = l.to_ascii_uppercase();
        u.find("PLAY").is_some_and(|i| u[i..].contains("\",") || u[i..].contains("$,"))
    });
    if multi || strings.iter().any(|s| s.contains('R') || s.contains('V') && !s.contains("MB")) {
        return if strings.iter().any(|s| s.contains('@')) { Dialect::N88 } else { Dialect::Msx };
    }
    Dialect::GwBasic
}

/// Every quoted string in a BASIC program.
fn basic_strings(text: &str) -> Vec<String> {
    let mut out = vec![];
    for line in text.lines() {
        let mut rest = line;
        while let Some(i) = rest.find('"') {
            let after = &rest[i + 1..];
            let end = after.find('"').unwrap_or(after.len());
            out.push(after[..end].to_string());
            rest = after.get(end + 1..).unwrap_or("");
        }
    }
    out
}

/// What a dialect's notes are.
#[derive(Clone, Copy)]
struct Settings {
    /// The octave middle C (60) is in.
    middle_c_octave: i32,
    octave: i32,
    /// The key of `n0`.
    n_zero: i32,
    volume: i32,
    volume_max: i32,
    /// Notes sound this much of their length (MN, ML, MS).
    gate: Frac,
    /// `@` is a GM program.
    programs: bool,
    /// `r` (else `p`) is a rest.
    rest: char,
}

fn settings(d: Dialect) -> Settings {
    let base = Settings { middle_c_octave: 4, octave: 4, n_zero: 12, volume: 8, volume_max: 15, gate: Frac::int(1), programs: false, rest: 'r' };
    match d {
        Dialect::Generic => Settings { programs: true, ..base },
        Dialect::Mabinogi | Dialect::ArcheAge | Dialect::MapleStory2 => base,
        // GW-BASIC: octave 3 starts at middle C, N1 is the bottom C.
        Dialect::GwBasic => Settings { middle_c_octave: 3, octave: 4, n_zero: 23, volume: 15, gate: Frac::new(7, 8), rest: 'p', ..base },
        Dialect::Msx | Dialect::N88 | Dialect::MsxMusic => Settings { n_zero: 12, ..base },
        Dialect::C128 => base,
        // Sakura: o5 holds middle C, v is 0 to 127.
        Dialect::Sakura => Settings { middle_c_octave: 5, octave: 5, volume: 100, volume_max: 127, programs: true, ..base },
        Dialect::Zx128 => Settings { volume: 15, ..base },
    }
}

/// A voice of a BASIC program as it plays: its part, its time, its
/// settings, its length, the note it may tie from, whether it's tied.
type VoiceState = (usize, Frac, Settings, Frac, Option<(usize, u8)>, bool);

/// A note placed: when, how long, its key and velocity (in whole notes).
type Placed = (Frac, Frac, u8, u8);

/// A track's events as it's read.
struct Track<'a> {
    b: &'a mut Builder,
    part: usize,
    now: Frac,
    /// The note last played: its index in the part, its key; `&` after it.
    last: Option<(usize, u8)>,
    tie: bool,
}

/// MML in a dialect as a score and a title.
pub fn read(text: &str, dialect: Dialect) -> Result<(Score, String), Error> {
    if dialect == Dialect::Sakura {
        return super::sakura::read(text);
    }
    let mut b = Builder::new();
    b.tempo(Frac::ZERO, 120.0);
    match dialect {
        Dialect::Mabinogi | Dialect::ArcheAge => read_mabinogi(text, &mut b, dialect)?,
        Dialect::MapleStory2 => read_ms2(text, &mut b)?,
        Dialect::Generic => {
            let mut offset = 0;
            for line in text.split_inclusive('\n') {
                let body = line.split(['\'', ';']).next().unwrap_or("");
                if body.trim().is_empty() {
                    offset += line.len();
                    continue;
                }
                let part = b.part(&format!("Track {}", b.parts.len() + 1), 0, false);
                let mut st = settings(dialect);
                let mut t = Track { b: &mut b, part, now: Frac::ZERO, last: None, tie: false };
                core(text, offset, body, &mut st, &mut t, &mut Frac::new(1, 4))?;
                let end = t.now;
                b.end_at(end);
                offset += line.len();
            }
        }
        Dialect::GwBasic | Dialect::Msx | Dialect::N88 | Dialect::MsxMusic | Dialect::C128 | Dialect::Zx128 => read_basic(text, &mut b, dialect)?,
        Dialect::Sakura => unreachable!("compiled by Sakura above"),
    }
    let title = b.title.clone();
    Ok((b.finish(), title))
}

/// `MML@track,track,track;`, each part's tracks one part each.
fn read_mabinogi(text: &str, b: &mut Builder, dialect: Dialect) -> Result<(), Error> {
    let mut search = 0;
    let mut any = false;
    while let Some(i) = text[search..].find("MML@").map(|i| i + search) {
        let start = i + 4;
        let end = text[start..].find(';').map(|e| e + start).unwrap_or(text.len());
        let mut offset = start;
        for (n, track) in text[start..end].split(',').enumerate() {
            let name = if n == 3 { "Singer".to_string() } else { format!("Track {}", b.parts.len() + 1) };
            let part = b.part(&name, if n == 3 { 52 } else { 0 }, false);
            let mut st = settings(dialect);
            let mut t = Track { b, part, now: Frac::ZERO, last: None, tie: false };
            core(text, offset, track, &mut st, &mut t, &mut Frac::new(1, 4))?;
            let e = t.now;
            b.end_at(e);
            offset += track.len() + 1;
        }
        any = true;
        search = end;
    }
    if !any {
        return Err(bad(text, 0, "Mabinogi's MML starts with MML@"));
    }
    Ok(())
}

/// MapleStory 2: each `<melody>` and `<chord>` element's MML a track.
fn read_ms2(text: &str, b: &mut Builder) -> Result<(), Error> {
    let doc = super::xml::parse(text)?;
    let mut tracks = vec![];
    doc.walk(&mut |e| {
        if e.name == "melody" || e.name == "chord" {
            tracks.push(e.text());
        }
    });
    if tracks.is_empty() {
        return Err(bad(text, 0, "a MapleStory 2 file holds its MML in <melody> and <chord>"));
    }
    for (n, track) in tracks.iter().enumerate() {
        let part = b.part(&format!("Track {}", n + 1), 0, false);
        let mut st = settings(Dialect::MapleStory2);
        let mut t = Track { b, part, now: Frac::ZERO, last: None, tie: false };
        core(track, 0, track, &mut st, &mut t, &mut Frac::new(1, 4))?;
        let e = t.now;
        b.end_at(e);
    }
    Ok(())
}

/// A number at `i`, and where it ends.
fn number(s: &[u8], mut i: usize) -> (Option<i64>, usize) {
    let start = i;
    while i < s.len() && s[i].is_ascii_digit() {
        i += 1;
    }
    if i == start {
        return (None, i);
    }
    (std::str::from_utf8(&s[start..i]).ok().and_then(|n| n.parse().ok()), i)
}

/// A length's dots at `i`: the length they make, and where they end.
fn dots(s: &[u8], mut i: usize, mut len: Frac) -> (Frac, usize) {
    let mut add = len;
    while s.get(i) == Some(&b'.') {
        add = add.scale(1, 2);
        len = len.add(add);
        i += 1;
    }
    (len, i)
}

/// The core MML (every dialect but C128's and the Spectrum's) of `body`,
/// at `offset` in `text`, onto a track.
fn core(text: &str, offset: usize, body: &str, st: &mut Settings, t: &mut Track, default: &mut Frac) -> Result<(), Error> {
    let s = body.as_bytes();
    let mut i = 0;
    // Repeats: where each opened, and the times left once it's closed.
    let mut loops: Vec<(usize, Option<i64>)> = vec![];
    let mut steps = 0usize;
    while i < s.len() {
        steps += 1;
        if steps > 1 << 22 {
            return Err(bad(text, offset + i, "the repeats play too long"));
        }
        let c = s[i].to_ascii_lowercase();
        let at = offset + i;
        i += 1;
        match c {
            b' ' | b'\t' | b'\r' | b'\n' | b'|' => {}
            b'a'..=b'g' | b'n' => {
                let key;
                let mut len = *default;
                if c == b'n' {
                    let (n, j) = number(s, i);
                    i = j;
                    let n = n.ok_or_else(|| bad(text, at, "n is followed by a note's number"))?;
                    if st.rest == 'p' && n == 0 {
                        // GW-BASIC's N0 is a rest.
                        t.now = t.now.add(len);
                        continue;
                    }
                    key = Some(st.n_zero + n as i32);
                    let (l, j) = dots(s, i, len);
                    len = l;
                    i = j;
                } else {
                    let mut k = super::letter_key(c as char).unwrap_or(60) + 12 * (st.octave - st.middle_c_octave);
                    while let Some(&a) = s.get(i) {
                        match a {
                            b'+' | b'#' => k += 1,
                            b'-' => k -= 1,
                            _ => break,
                        }
                        i += 1;
                    }
                    key = Some(k);
                    let (n, j) = number(s, i);
                    if let Some(n) = n {
                        if !(1..=192).contains(&n) {
                            return Err(bad(text, offset + i, "a length is from 1 (a whole note) to 64"));
                        }
                        len = Frac::new(1, n);
                    }
                    let (l, j) = dots(s, j, len);
                    len = l;
                    i = j;
                }
                // `^` adds lengths.
                while s.get(i) == Some(&b'^') {
                    let (n, j) = number(s, i + 1);
                    let add = n.map(|n| Frac::new(1, n.max(1))).unwrap_or(*default);
                    let (add, j) = dots(s, j, add);
                    len = len.add(add);
                    i = j;
                }
                play(t, key, len, st);
            }
            b'r' | b'p' if c as char == st.rest || (c == b'r' && st.rest == 'r') => {
                let (n, j) = number(s, i);
                let len = n.map(|n| Frac::new(1, n.max(1))).unwrap_or(*default);
                let (len, j) = dots(s, j, len);
                i = j;
                t.now = t.now.add(len);
                t.last = None;
                t.tie = false;
            }
            b'o' => {
                let (n, j) = number(s, i);
                i = j;
                st.octave = n.ok_or_else(|| bad(text, at, "o is followed by the octave"))? as i32;
            }
            b'>' => st.octave += 1,
            b'<' => st.octave -= 1,
            b'l' => {
                let (n, j) = number(s, i);
                let n = n.ok_or_else(|| bad(text, at, "l is followed by a length"))?;
                let (len, j) = dots(s, j, Frac::new(1, n.max(1)));
                *default = len;
                i = j;
            }
            b't' => {
                let (n, j) = number(s, i);
                i = j;
                let n = n.ok_or_else(|| bad(text, at, "t is followed by the tempo"))?;
                let now = t.now;
                t.b.tempo(now, n as f64);
            }
            b'v' => {
                let (n, j) = number(s, i);
                i = j;
                st.volume = n.ok_or_else(|| bad(text, at, "v is followed by the volume"))?.min(i64::from(st.volume_max)) as i32;
            }
            b'@' => {
                let (n, j) = number(s, i);
                i = j;
                let n = n.ok_or_else(|| bad(text, at, "@ is followed by an instrument's number"))?;
                let now = t.now;
                if st.programs {
                    t.b.program_at(t.part, now, n as u8);
                } else if let Some(p) = msx_music_voice(n) {
                    t.b.program_at(t.part, now, p);
                }
            }
            b'&' => t.tie = true,
            b'[' => loops.push((i, None)),
            b']' => {
                let (n, j) = number(s, i);
                let Some(top) = loops.last_mut() else { return Err(bad(text, at, "a ] with no [ before it")) };
                let left = top.1.get_or_insert(n.unwrap_or(2) - 1);
                if *left > 0 {
                    *left -= 1;
                    i = top.0;
                } else {
                    loops.pop();
                    i = j;
                }
            }
            // GW-BASIC's articulation and background: MN, ML, MS, MF, MB.
            b'm' => {
                match s.get(i).map(|c| c.to_ascii_lowercase()) {
                    Some(b'n') => st.gate = Frac::new(7, 8),
                    Some(b'l') => st.gate = Frac::int(1),
                    Some(b's') => st.gate = Frac::new(3, 4),
                    Some(b'f' | b'b') => {}
                    // MSX's M: the envelope's period, skipped.
                    _ => {
                        let (_, j) = number(s, i);
                        i = j;
                        continue;
                    }
                }
                i += 1;
            }
            // MSX's S: the envelope's shape, skipped.
            b's' => i = number(s, i).1,
            // N88's Q: gate time, 1 to 8 eighths.
            b'q' => {
                let (n, j) = number(s, i);
                i = j;
                if let Some(n) = n {
                    st.gate = Frac::new(n.clamp(1, 8), 8);
                }
            }
            b'x' => return Err(bad(text, at, "X plays a string variable this program doesn't set")),
            b'=' => return Err(bad(text, at, "a number from a variable (=X;) isn't read")),
            b';' | b',' => {}
            _ => return Err(bad(text, at, "something MML doesn't have")),
        }
    }
    Ok(())
}

/// A note (or a rest, with no key) onto the track, `&` tied to the last.
fn play(t: &mut Track, key: Option<i32>, len: Frac, st: &Settings) {
    let velocity = ((st.volume.clamp(0, st.volume_max) * 127 + st.volume_max / 2) / st.volume_max.max(1)).max(1) as u8;
    let Some(key) = key else {
        t.now = t.now.add(len);
        t.last = None;
        t.tie = false;
        return;
    };
    let now = t.now;
    if st.volume == 0 {
        t.now = now.add(len);
        t.last = None;
        t.tie = false;
        return;
    }
    if t.tie {
        if let Some((index, k)) = t.last {
            let note = &mut t.b.parts[t.part].notes[index];
            if i32::from(k) == key {
                // Held: the tied note's full length, then this one's gate.
                note.1 = now.sub(note.0).add(len.mul(st.gate));
                t.now = now.add(len);
                t.tie = false;
                return;
            }
            // Slurred: the last note sounds all its length.
            note.1 = now.sub(note.0);
        }
    }
    t.tie = false;
    let before = t.b.parts[t.part].notes.len();
    t.b.note(t.part, now, len.mul(st.gate), key, velocity);
    t.last = (t.b.parts[t.part].notes.len() > before).then_some((before, key as u8));
    t.now = now.add(len);
}

/// MSX-MUSIC's FM voices, `@0` to `@63`, as GM stand-ins.
fn msx_music_voice(n: i64) -> Option<u8> {
    const VOICES: [u8; 64] = [
        0, 1, 40, 73, 71, 68, 56, 19, 13, 16, 24, 15, 4, 7, 6, 6, // piano 1, 2, violin, flute, clarinet, oboe, trumpet, pipe organ, xylophone, organ, guitar, santool, e. piano, clavichord, harpsichord 1, 2
        11, 107, 116, 125, 98, 98, 14, 38, 81, 118, 118, 115, 113, 115, 115, 116, // vibraphone, koto, taiko, engine, UFO, synth bell, chime, synth bass, synthesizer, synth perc., synth rhythm, harmonic drum, cowbell, closed hi-hat, snare, bass drum
        0, 5, 15, 61, 73, 7, 7, 107, 19, 4, 4, 55, 55, 50, 18, 62, // piano 3, e. piano 2, santool 2, brass, flute 2, clavichord 2, 3, koto 2, pipe organ 2, Rhodes-like 1, 2, orchestra L, R, synth violin, synth organ, synth brass
        74, 106, 88, 99, 89, 30, 125, 125, 101, 101, 127, 122, 98, 98, 125, 0, // tube, shamisen, magical, huwawa, wander flat, hard rock, machine, machine V, comic, SE comic, laser, noise, star 1, 2, engine 2, silence
    ];
    VOICES.get(usize::try_from(n).ok()?).copied()
}

/// MSX-MUSIC's rhythm string onto the drums: `B` bass drum, `S` snare,
/// `M` tom, `C` cymbal, `H` hi-hat, together until a length; `!` accent;
/// `R` rests; `V`, `L`, `T`, `@A`, `@V` set.
fn rhythm(text: &str, offset: usize, body: &str, t: &mut Track, st: &mut Settings) -> Result<(), Error> {
    let s = body.as_bytes();
    let mut i = 0;
    let mut default = Frac::new(1, 4);
    let mut hits: Vec<i32> = vec![];
    let mut accent = false;
    while i < s.len() {
        let c = s[i].to_ascii_uppercase();
        let at = offset + i;
        i += 1;
        let key = match c {
            b'B' => Some(36),
            b'S' => Some(38),
            b'M' => Some(45),
            b'C' => Some(49),
            b'H' => Some(42),
            _ => None,
        };
        if let Some(k) = key {
            hits.push(k);
            if s.get(i) == Some(&b'!') {
                accent = true;
                i += 1;
            }
            let (n, j) = number(s, i);
            if n.is_some() || !s.get(i).is_some_and(|c| b"BSMCHbsmch".contains(c)) {
                let len = n.map(|n| Frac::new(1, n.max(1))).unwrap_or(default);
                let (len, j) = dots(s, j, len);
                i = j;
                let now = t.now;
                let v = (st.volume * 127 / 15).clamp(1, 127) as u8;
                for &k in &hits {
                    t.b.note(t.part, now, Frac::new(1, 16).min(len), k, if accent { 127 } else { v });
                }
                hits.clear();
                accent = false;
                t.now = now.add(len);
            }
            continue;
        }
        match c {
            b' ' | b'\t' => {}
            b'R' => {
                let (n, j) = number(s, i);
                let (len, j) = dots(s, j, n.map(|n| Frac::new(1, n.max(1))).unwrap_or(default));
                i = j;
                t.now = t.now.add(len);
            }
            b'L' => {
                let (n, j) = number(s, i);
                let (len, j) = dots(s, j, Frac::new(1, n.unwrap_or(4).max(1)));
                default = len;
                i = j;
            }
            b'T' => {
                let (n, j) = number(s, i);
                i = j;
                let now = t.now;
                t.b.tempo(now, n.unwrap_or(120) as f64);
            }
            b'V' => {
                let (n, j) = number(s, i);
                i = j;
                st.volume = n.unwrap_or(8).min(15) as i32;
            }
            b'@' => {
                // @A (accent) or @V (a drum's volume): letters then a number.
                while i < s.len() && s[i].is_ascii_alphabetic() {
                    i += 1;
                }
                i = number(s, i).1;
            }
            b'Y' => {
                // Y: a register write, `Yr,v`.
                i = number(s, i).1;
                if s.get(i) == Some(&b',') {
                    i = number(s, i + 1).1;
                }
            }
            _ => return Err(bad(text, at, "something MSX-MUSIC's rhythm doesn't have")),
        }
    }
    Ok(())
}

/// A BASIC statement's string expression: literals and variables joined
/// by `+`, at `offset`.
fn expression(expr: &str, vars: &[(String, String)]) -> Option<String> {
    let mut out = String::new();
    for piece in split_outside_quotes(expr, '+') {
        let p = piece.trim();
        if let Some(lit) = p.strip_prefix('"') {
            out.push_str(lit.strip_suffix('"').unwrap_or(lit));
        } else if p.ends_with('$') || p.contains('$') {
            let name = p.to_ascii_uppercase();
            out.push_str(&vars.iter().rev().find(|v| v.0 == name)?.1);
        } else if !p.is_empty() {
            return None;
        }
    }
    Some(out)
}

fn split_outside_quotes(s: &str, sep: char) -> Vec<&str> {
    let mut out = vec![];
    let (mut quoted, mut start) = (false, 0);
    for (i, c) in s.char_indices() {
        if c == '"' {
            quoted = !quoted;
        } else if c == sep && !quoted {
            out.push(&s[start..i]);
            start = i + 1;
        }
    }
    out.push(&s[start..]);
    out
}

/// A BASIC program's PLAY statements, in order, each a voice's string.
fn read_basic(text: &str, b: &mut Builder, dialect: Dialect) -> Result<(), Error> {
    // Statements: line numbers off, split at colons outside strings.
    let mut statements: Vec<(usize, String)> = vec![];
    let mut offset = 0;
    let raw = !text.to_ascii_uppercase().contains("PLAY");
    for line in text.split_inclusive('\n') {
        let body = line.trim_end_matches(['\n', '\r']);
        let skip = body.len() - body.trim_start_matches(|c: char| c.is_ascii_digit() || c == ' ').len();
        let mut at = offset + skip;
        for st in split_outside_quotes(&body[skip..], ':') {
            statements.push((at, st.to_string()));
            at += st.len() + 1;
        }
        offset += line.len();
    }
    if raw {
        // Just the PLAY strings, one a voice a line.
        statements = text.lines().scan(0, |o, l| {
            let at = *o;
            *o += l.len() + 1;
            Some((at, format!("PLAY \"{}\"", l.trim())))
        }).filter(|(_, s)| s != "PLAY \"\"").collect();
    }
    let mut vars: Vec<(String, String)> = vec![];
    let mut voices: Vec<VoiceState> = vec![];
    let mut tempo_c128 = 8.0f64;
    // FOR loops: the statement after FOR, the variable, now, the end.
    let mut fors: Vec<(usize, String, i64, i64)> = vec![];
    let mut nums: Vec<(String, i64)> = vec![];
    let mut pc = 0usize;
    let mut steps = 0usize;
    while pc < statements.len() {
        steps += 1;
        if steps > 1 << 20 {
            return Err(bad(text, statements[pc].0, "the program's loops run too long"));
        }
        let (at, st) = statements[pc].clone();
        pc += 1;
        let trimmed = st.trim();
        let upper = trimmed.to_ascii_uppercase();
        let lead = st.len() - st.trim_start().len();
        if upper.starts_with("REM") || upper.starts_with('\'') {
            continue;
        }
        if let Some(rest) = upper.strip_prefix("FOR") {
            // FOR I=1 TO 4
            let Some((var, range)) = rest.split_once('=') else { continue };
            let Some((from, to)) = range.split_once("TO") else { continue };
            let (Ok(from), Ok(to)) = (from.trim().parse::<i64>(), to.split_whitespace().next().unwrap_or("").parse::<i64>()) else {
                return Err(bad(text, at + lead, "only FOR loops with numbers for bounds are read"));
            };
            let var = var.trim().to_string();
            nums.retain(|n| n.0 != var);
            nums.push((var.clone(), from));
            fors.push((pc, var, from, to));
            continue;
        }
        if upper.starts_with("NEXT") {
            if let Some(top) = fors.last_mut() {
                top.2 += 1;
                let (var, v) = (top.1.clone(), top.2);
                if let Some(n) = nums.iter_mut().find(|n| n.0 == var) {
                    n.1 = v;
                }
                if top.2 <= top.3 {
                    pc = top.0;
                } else {
                    fors.pop();
                }
            }
            continue;
        }
        if let Some(rest) = upper.strip_prefix("TEMPO") {
            tempo_c128 = rest.trim().parse().unwrap_or(8.0);
            continue;
        }
        if upper.starts_with("CALL") || upper.starts_with("GOTO") || upper.starts_with("END") {
            continue;
        }
        // A$="..." (or LET A$=...).
        let assign = upper.strip_prefix("LET").unwrap_or(&upper).trim_start().to_string();
        if let Some(eq) = assign.find('=') {
            let name = assign[..eq].trim();
            if name.ends_with('$') && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '$') {
                let off = trimmed.len() - assign.len();
                let expr = &trimmed[off + eq + 1..];
                let v = expression(expr, &vars).ok_or_else(|| bad(text, at + lead, "a string variable set from something other than strings"))?;
                vars.retain(|x| x.0 != name);
                vars.push((name.to_string(), v));
                continue;
            }
        }
        let Some(play_at) = upper.find("PLAY") else { continue };
        let mut args = &trimmed[play_at + 4..];
        let mut arg_at = at + lead + play_at + 4;
        let mut music = false;
        if let Some(r) = args.trim_start().strip_prefix('#') {
            // PLAY #2, ...: MSX-MUSIC.
            let r = r.trim_start_matches(|c: char| c.is_ascii_digit() || c == ' ');
            let r = r.strip_prefix(',').unwrap_or(r);
            arg_at += args.len() - r.len();
            args = r;
            music = true;
        }
        let strings: Vec<&str> = split_outside_quotes(args, ',');
        let mut a = arg_at;
        let mut texts = vec![];
        for s in strings {
            let lit_at = a + s.find('"').map(|i| i + 1).unwrap_or(0);
            a += s.len() + 1;
            let v = expression(s, &vars).ok_or_else(|| bad(text, lit_at, "PLAY plays strings and string variables"))?;
            texts.push((lit_at, v, s.trim_start().starts_with('"')));
        }
        // C128: one string, voices inside it by V.
        if dialect == Dialect::C128 {
            for (lit_at, s, literal) in texts {
                c128(text, if literal { lit_at } else { at }, &s, b, &mut voices, tempo_c128)?;
            }
            continue;
        }
        for (n, (lit_at, s, literal)) in texts.into_iter().enumerate() {
            let off = if literal { lit_at } else { at + lead };
            while voices.len() <= n {
                let part = b.part(&format!("Voice {}", voices.len() + 1), if dialect == Dialect::Msx || dialect == Dialect::GwBasic { 80 } else { 0 }, false);
                voices.push((part, Frac::ZERO, settings(dialect), Frac::new(1, 4), None, false));
            }
            let v = &mut voices[n];
            let rhythm_string = music && is_rhythm(&s);
            if rhythm_string {
                b.parts[v.0].drums = true;
                b.parts[v.0].name = "Rhythm".into();
            }
            let mut t = Track { b, part: v.0, now: v.1, last: v.4, tie: v.5 };
            if rhythm_string {
                rhythm(text, off, &s, &mut t, &mut v.2)?;
            } else if dialect == Dialect::Zx128 {
                zx(text, off, &s, &mut t, &mut v.2, &mut v.3)?;
            } else {
                let x = expand_x(text, off, &s, &vars)?;
                core(text, off, &x, &mut v.2, &mut t, &mut v.3)?;
            }
            v.1 = t.now;
            v.4 = t.last;
            v.5 = t.tie;
        }
    }
    for v in &voices {
        b.end_at(v.1);
    }
    if voices.is_empty() {
        return Err(bad(text, 0, "there's no PLAY in it"));
    }
    Ok(())
}

/// A string of MSX-MUSIC rhythm: only `B S M C H`, lengths and settings.
fn is_rhythm(s: &str) -> bool {
    let u = s.to_ascii_uppercase();
    u.chars().any(|c| matches!(c, 'B' | 'S' | 'M' | 'C' | 'H')) && u.chars().all(|c| c.is_ascii_digit() || " .!BSMCHRLTV@AY,".contains(c))
}

/// GW-BASIC's `X` with a string variable (`XA$;`) put in its place.
fn expand_x(text: &str, offset: usize, s: &str, vars: &[(String, String)]) -> Result<String, Error> {
    if !s.to_ascii_uppercase().contains('X') {
        return Ok(s.to_string());
    }
    let mut out = String::new();
    let mut rest = s;
    let mut depth = 0;
    while let Some(i) = rest.find(['X', 'x']) {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        let end = after.find(';').unwrap_or(after.len());
        let name = after[..end].trim().to_ascii_uppercase();
        let v = vars.iter().rev().find(|v| v.0 == name).ok_or_else(|| bad(text, offset + (s.len() - rest.len()) + i, "X plays a string variable this program doesn't set"))?;
        out.push_str(&v.1);
        rest = after.get(end + 1..).unwrap_or("");
        depth += 1;
        if depth > 1000 {
            break;
        }
    }
    out.push_str(rest);
    Ok(out)
}

/// The C128's PLAY string: `V1`-`V3` choose the voice, `O0`-`O6`,
/// `T0`-`T9` the envelope (an instrument), `U` volume, `X` filter;
/// lengths `W H Q I S` before the note, `.` dotted; `#` and `$` sharp and
/// flat before it too; `R` rests; `M` waits for every voice.
fn c128(text: &str, offset: usize, s: &str, b: &mut Builder, voices: &mut Vec<VoiceState>, tempo: f64) -> Result<(), Error> {
    // TEMPO n: a whole note lasts 19.22 / n seconds.
    let bpm = 4.0 * 60.0 * tempo.max(1.0) / 19.22;
    const ENVELOPES: [u8; 10] = [0, 21, 82, 118, 73, 24, 6, 16, 56, 13];
    while voices.len() < 3 {
        let part = b.part(&format!("Voice {}", voices.len() + 1), 0, false);
        voices.push((part, Frac::ZERO, settings(Dialect::C128), Frac::new(1, 4), None, false));
    }
    let mut voice = 0usize;
    let bytes = s.as_bytes();
    let mut i = 0;
    let mut len = voices[voice].3;
    let mut dotted = false;
    let mut acc = 0;
    let first = voices.iter().map(|v| v.1).min().unwrap_or(Frac::ZERO);
    b.tempo(first, (bpm * 100.0).round() / 100.0);
    while i < bytes.len() {
        let c = bytes[i].to_ascii_uppercase();
        let at = offset + i;
        i += 1;
        match c {
            b' ' => {}
            b'V' | b'O' | b'T' | b'U' | b'X' => {
                let (n, j) = number(bytes, i);
                i = j;
                let n = n.ok_or_else(|| bad(text, at, "this is followed by a number"))?;
                match c {
                    b'V' => {
                        voice = (n.clamp(1, 3) - 1) as usize;
                        len = voices[voice].3;
                    }
                    b'O' => voices[voice].2.octave = n as i32,
                    b'T' => {
                        let (part, now) = (voices[voice].0, voices[voice].1);
                        if let Some(&p) = ENVELOPES.get(n as usize) {
                            b.program_at(part, now, p);
                        }
                    }
                    b'U' => voices[voice].2.volume = n.min(15) as i32,
                    _ => {}
                }
            }
            b'W' => len = Frac::int(1),
            b'H' => len = Frac::new(1, 2),
            b'Q' => len = Frac::new(1, 4),
            b'I' => len = Frac::new(1, 8),
            b'S' => len = Frac::new(1, 16),
            b'.' => dotted = true,
            b'#' => acc += 1,
            b'$' => acc -= 1,
            b'M' => {
                let end = voices.iter().map(|v| v.1).max().unwrap_or(Frac::ZERO);
                for v in voices.iter_mut() {
                    v.1 = end;
                }
            }
            b'R' | b'A'..=b'G' => {
                let l = if dotted { len.scale(3, 2) } else { len };
                let v = &mut voices[voice];
                v.3 = len;
                if c != b'R' {
                    let key = super::letter_key(c as char).unwrap_or(60) + acc + 12 * (v.2.octave - 4);
                    let mut t = Track { b, part: v.0, now: v.1, last: None, tie: false };
                    play(&mut t, Some(key), l, &v.2);
                    v.1 = t.now;
                } else {
                    v.1 = v.1.add(l);
                }
                dotted = false;
                acc = 0;
            }
            _ => return Err(bad(text, at, "something the C128's PLAY doesn't have")),
        }
    }
    Ok(())
}

/// The ZX Spectrum 128's PLAY string.
fn zx(text: &str, offset: usize, s: &str, t: &mut Track, st: &mut Settings, len: &mut Frac) -> Result<(), Error> {
    // Lengths 1 to 9: semiquaver, dotted, quaver, dotted, crotchet,
    // dotted, minim, dotted, semibreve; 10 to 12 triplets of a semiquaver,
    // a quaver, a crotchet.
    const LENGTHS: [(i64, i64); 12] = [(1, 16), (3, 32), (1, 8), (3, 16), (1, 4), (3, 8), (1, 2), (3, 4), (1, 1), (1, 24), (1, 12), (1, 6)];
    if st.octave == 4 && st.middle_c_octave == 4 {
        st.octave = 5;
        st.middle_c_octave = 5;
    }
    let bytes = s.as_bytes();
    let mut i = 0;
    // Repeats: where each opened, and whether it's been played twice.
    let mut repeats: Vec<(usize, bool)> = vec![];
    let mut acc = 0;
    let mut tie = false;
    let mut steps = 0;
    while i < bytes.len() {
        steps += 1;
        if steps > 1 << 20 {
            return Err(bad(text, offset + i, "the repeats play too long"));
        }
        let c = bytes[i];
        let at = offset + i;
        i += 1;
        match c {
            b' ' => {}
            b'!' => {
                // A comment, to the next !.
                i = bytes[i..].iter().position(|&c| c == b'!').map(|p| i + p + 1).unwrap_or(bytes.len());
            }
            b'0'..=b'9' => {
                let (n, j) = number(bytes, i - 1);
                i = j;
                let n = n.unwrap_or(5);
                let &(a, d) = LENGTHS.get((n.clamp(1, 12) - 1) as usize).unwrap_or(&(1, 4));
                *len = Frac::new(a, d);
            }
            b'O' | b'T' | b'V' | b'U' | b'W' | b'X' | b'Y' | b'Z' | b'M' => {
                let (n, j) = number(bytes, i);
                i = j;
                match (c, n) {
                    (b'O', Some(n)) => st.octave = n as i32,
                    (b'T', Some(n)) => {
                        let now = t.now;
                        t.b.tempo(now, n as f64);
                    }
                    (b'V', Some(n)) => st.volume = n.min(15) as i32,
                    _ => {}
                }
            }
            b'#' => acc += 1,
            b'$' => acc -= 1,
            b'_' => tie = true,
            b'(' => repeats.push((i, false)),
            b')' => match repeats.last_mut() {
                Some(r) if !r.1 => {
                    r.1 = true;
                    i = r.0;
                }
                Some(_) => {
                    repeats.pop();
                }
                // A ) with no (: from the start, once more.
                None => {
                    repeats.push((0, true));
                    i = 0;
                }
            },
            b'&' => {
                t.now = t.now.add(*len);
                t.last = None;
            }
            b'H' => break,
            b'N' => {}
            b'a'..=b'g' | b'A'..=b'G' => {
                let up = if c.is_ascii_uppercase() { 12 } else { 0 };
                let key = super::letter_key(c as char).unwrap_or(60) + acc + up + 12 * (st.octave - st.middle_c_octave);
                t.tie = tie;
                play(t, Some(key), *len, st);
                acc = 0;
                tie = false;
            }
            _ => return Err(bad(text, at, "something the Spectrum's PLAY doesn't have")),
        }
    }
    Ok(())
}

/// Lengths as MML writes them, longest first, each a fraction of a whole
/// note and its text: 1 to 64, dotted or not.
fn mml_lengths() -> Vec<(Frac, String)> {
    let mut v = vec![];
    for d in [1i64, 2, 4, 8, 16, 32, 64] {
        v.push((Frac::new(3, 2 * d), format!("{d}.")));
        v.push((Frac::new(1, d), d.to_string()));
    }
    v.retain(|x| x.1 != "64.");
    v.sort_by_key(|x| std::cmp::Reverse(x.0));
    v
}

/// A time as lengths that add to it (to a 64th note; triplets of 3, 6,
/// 12, 24 and 48 when they fit exactly).
/// Returns them and how long they add to.
fn split_sum(mut left: Frac) -> (Vec<String>, Frac) {
    let table = mml_lengths();
    let mut out = vec![];
    for d in [3i64, 6, 12, 24, 48] {
        if left == Frac::new(1, d) {
            return (vec![d.to_string()], left);
        }
    }
    let mut sum = Frac::ZERO;
    while left >= Frac::new(1, 64) && out.len() < 256 {
        let pick = table.iter().find(|x| x.0 <= left).unwrap_or(&table[table.len() - 1]);
        out.push(pick.1.clone());
        left = left.sub(pick.0);
        sum = sum.add(pick.0);
    }
    (out, sum)
}

/// Splits a part's notes into lines that don't overlap (a chord's notes
/// each a line), as (start, length, key, velocity) in whole notes.
fn voices_of(score: &Score) -> Vec<(String, u8, Vec<Placed>)> {
    let q = Frac::int(4 * i64::from(score.ppq));
    let mut out = vec![];
    for part in &score.parts {
        if part.channel == DRUMS {
            continue;
        }
        let mut lines: Vec<Vec<(Frac, Frac, u8, u8)>> = vec![];
        let mut notes = part.notes.clone();
        notes.sort_by_key(|n| (n.at, std::cmp::Reverse(n.key)));
        for n in notes {
            let (at, len) = (Frac::new(i64::from(n.at), 1).div(q), Frac::new(i64::from(n.length), 1).div(q));
            let free = lines.iter().position(|l| l.last().map_or(true, |x| x.0.add(x.1) <= at));
            match free {
                Some(i) => lines[i].push((at, len, n.key, n.velocity)),
                None => lines.push(vec![(at, len, n.key, n.velocity)]),
            }
        }
        for (i, l) in lines.into_iter().enumerate() {
            let name = if i == 0 { part.name.clone() } else { format!("{} {}", part.name, i + 1) };
            out.push((name, part.program, l));
        }
    }
    out
}

/// One line's MML: `t` at the start when asked, then the notes.
fn line_mml(line: &[(Frac, Frac, u8, u8)], tempo: Option<f64>, dialect: Dialect) -> String {
    let names = ["c", "c+", "d", "d+", "e", "f", "f+", "g", "g+", "a", "a+", "b"];
    let st = settings(dialect);
    let mut out = String::new();
    if let Some(t) = tempo {
        out.push_str(&format!("t{}", t.round().clamp(32.0, 255.0) as u32));
    }
    let mut octave = st.octave;
    out.push_str(&format!("o{octave}"));
    let mut volume = -1;
    let mut now = Frac::ZERO;
    let rest = if st.rest == 'p' { "p" } else { "r" };
    if dialect == Dialect::Sakura {
        // Notes as long as written (Sakura's own gate is 80%).
        out.push_str("q100");
    }
    // `now` is the time written so far, so rounding doesn't add up.
    for &(at, len, key, velocity) in line {
        if at > now {
            let (ls, sum) = split_sum(at.sub(now));
            for l in ls {
                out.push_str(&format!("{rest}{l}"));
            }
            now = now.add(sum);
        }
        let v = ((i32::from(velocity) * st.volume_max + 63) / 127).clamp(1, st.volume_max);
        if v != volume && dialect != Dialect::GwBasic {
            out.push_str(&format!("v{v}"));
            volume = v;
        }
        let o = i32::from(key) / 12 - 5 + st.middle_c_octave;
        let o = o.clamp(0, 8);
        while octave < o {
            out.push('>');
            octave += 1;
        }
        while octave > o {
            out.push('<');
            octave -= 1;
        }
        let name = names[usize::from(key % 12)];
        let (ls, mut sum) = split_sum(len);
        let tie = if dialect == Dialect::GwBasic { "" } else { "&" };
        for (i, l) in ls.iter().enumerate() {
            if i > 0 {
                out.push_str(tie);
            }
            out.push_str(name);
            out.push_str(l);
        }
        if ls.is_empty() {
            out.push_str(name);
            out.push_str("64");
            sum = Frac::new(1, 64);
        }
        now = now.add(sum);
    }
    out
}

/// The score as MML in a dialect: `MML@a,b,c;` for Mabinogi and
/// ArcheAge (its first three lines), MapleStory 2's XML, a BASIC program
/// for GW-BASIC (the first line) and MSX (three), or a track a line.
/// Chords are split into lines; lengths are to a 64th note, tied.
pub fn write(score: &Score, dialect: Dialect) -> String {
    let lines = voices_of(score);
    let tempo = Some(score.bpm);
    match dialect {
        Dialect::Mabinogi | Dialect::ArcheAge => {
            let tracks: Vec<String> = lines.iter().take(3).enumerate().map(|(i, l)| line_mml(&l.2, if i == 0 { tempo } else { None }, dialect)).collect();
            format!("MML@{};", tracks.join(","))
        }
        Dialect::MapleStory2 => {
            let mut out = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<ms2>\n");
            for (i, l) in lines.iter().take(10).enumerate() {
                let tag = if i == 0 { "melody".to_string() } else { format!("chord index=\"{i}\"") };
                let close = if i == 0 { "melody" } else { "chord" };
                out.push_str(&format!("  <{tag}>\n    <![CDATA[{}]]>\n  </{close}>\n", line_mml(&l.2, tempo, dialect)));
            }
            out.push_str("</ms2>\n");
            out
        }
        Dialect::GwBasic => {
            let mml = lines.first().map(|l| line_mml(&l.2, tempo, dialect)).unwrap_or_default().to_ascii_uppercase();
            // GW-BASIC's strings stop at 255 characters: a PLAY a piece.
            let mut out = String::new();
            let mut n = 10;
            for chunk in chunks(&mml, 200) {
                out.push_str(&format!("{n} PLAY \"{chunk}\"\n"));
                n += 10;
            }
            out
        }
        Dialect::Msx | Dialect::N88 | Dialect::MsxMusic | Dialect::C128 | Dialect::Zx128 => {
            let voices: Vec<String> = lines.iter().take(3).enumerate().map(|(i, l)| line_mml(&l.2, if i == 0 || dialect == Dialect::Msx { tempo } else { None }, Dialect::Msx).to_ascii_uppercase().replace('&', "")).collect();
            let mut out = String::new();
            for (i, v) in voices.iter().enumerate() {
                out.push_str(&format!("{} {}$=\"{}\"\n", 10 + i * 10, (b'A' + i as u8) as char, v.chars().take(250).collect::<String>()));
            }
            let names: Vec<String> = (0..voices.len()).map(|i| format!("{}$", (b'A' + i as u8) as char)).collect();
            out.push_str(&format!("{} PLAY {}\n", 10 + voices.len() * 10, names.join(",")));
            out
        }
        Dialect::Generic => lines.iter().enumerate().map(|(i, l)| format!("@{}{}\n", l.1, line_mml(&l.2, if i == 0 { tempo } else { None }, dialect))).collect(),
        // A track a line, its program 1 to 128 as Sakura counts them.
        Dialect::Sakura => {
            let mut out = format!("Tempo = {}\n", score.bpm.round().clamp(1.0, 999.0) as u32);
            for (i, l) in lines.iter().enumerate() {
                out.push_str(&format!("TR({}) @{} {}\n", i + 1, u32::from(l.1) + 1, line_mml(&l.2, None, Dialect::Sakura)));
            }
            out
        }
    }
}

/// Splits MML into pieces of at most `n` characters, each at a command.
fn chunks(s: &str, n: usize) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    for c in s.chars() {
        if cur.len() >= n && c.is_ascii_alphabetic() {
            out.push(std::mem::take(&mut cur));
        }
        cur.push(c);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::score::Note;

    fn notes(score: &Score, part: usize) -> Vec<(u32, u32, u8, u8)> {
        score.parts[part].notes.iter().map(|n| (n.at, n.length, n.key, n.velocity)).collect()
    }

    #[test]
    fn mabinogi() {
        let (score, _) = read("MML@t150l8o4cdef+.g-&g4,o3v15c1,;", Dialect::Mabinogi).unwrap();
        assert_eq!(score.bpm, 150.0);
        assert_eq!(score.parts.len(), 2);
        let v = (8 * 127 + 7) / 15;
        assert_eq!(notes(&score, 0)[..3], [(0, 240, 60, v as u8), (240, 240, 62, v as u8), (480, 240, 64, v as u8)]);
        assert_eq!(notes(&score, 0)[3], (720, 360, 66, v as u8));
        // g- tied to g: slurred, both played.
        assert_eq!(notes(&score, 0)[4].2, 66);
        assert_eq!(notes(&score, 0)[5], (1320, 480, 67, v as u8));
        assert_eq!(notes(&score, 1)[0], (0, 1920, 48, 127));
    }

    #[test]
    fn ties_hold_and_repeats_repeat() {
        let (score, _) = read("c4&c8 [d16]3 e4^8", Dialect::Generic).unwrap();
        let n = notes(&score, 0);
        assert_eq!(n[0].1, 720);
        assert_eq!(n.iter().filter(|x| x.2 == 62).count(), 3);
        assert_eq!(n.last().unwrap().1, 720);
    }

    #[test]
    fn gw_basic() {
        let program = "10 A$=\"CDE\"\n20 PLAY \"T120 O3 L4 MS XA$; P4 N37\"\n";
        assert_eq!(detect(program), Dialect::GwBasic);
        let (score, _) = read(program, Dialect::GwBasic).unwrap();
        let n = notes(&score, 0);
        assert_eq!(n[0], (0, 360, 60, 127));
        assert_eq!(n[2].2, 64);
        // N37: the C above middle C... N1 is the bottom C, so N37 is O3's C.
        assert_eq!(n[3], (480 * 4, 360, 60, 127));
    }

    #[test]
    fn msx_three_voices() {
        let program = "10 PLAY \"O4L4CDE\",\"O3L2C\",\"R4V10G\"\n20 PLAY \"F\",\"G\"";
        assert_eq!(detect(program), Dialect::Msx);
        let (score, _) = read(program, Dialect::Msx).unwrap();
        assert_eq!(score.parts.len(), 3);
        assert_eq!(notes(&score, 0).last().unwrap().0, 480 * 3);
        assert_eq!(notes(&score, 1)[1], (960, 960, 55, 68));
        assert_eq!(notes(&score, 2)[0].0, 480);
    }

    #[test]
    fn msx_music_rhythm() {
        let program = "10 CALL MUSIC\n20 PLAY #2,\"@0O4C\",\"B!H8S8\"";
        assert_eq!(detect(program), Dialect::MsxMusic);
        let (score, _) = read(program, Dialect::MsxMusic).unwrap();
        let drums = score.parts.iter().find(|p| p.channel == DRUMS).unwrap();
        assert_eq!(drums.notes.iter().map(|n| (n.at, n.key)).collect::<Vec<_>>(), vec![(0, 36), (0, 42), (240, 38)]);
        assert_eq!(drums.notes[0].velocity, 127);
    }

    #[test]
    fn c128() {
        let program = "10 TEMPO 10\n20 PLAY \"V1O4T0QCDV2O3HCM V1 WE\"";
        assert_eq!(detect(program), Dialect::C128);
        let (score, _) = read(program, Dialect::C128).unwrap();
        assert_eq!(notes(&score, 0)[0].2, 60);
        assert_eq!(notes(&score, 1)[0], (0, 960, 48, 68));
        // M waits for every voice: they're both at a half note.
        assert_eq!(notes(&score, 0)[2], (960, 1920, 64, 68));
    }

    #[test]
    fn spectrum() {
        let program = "10 LET a$=\"O5 5c_c3&!rest!(d)E#f\"\n20 PLAY a$";
        assert_eq!(detect(program), Dialect::Zx128);
        let (score, _) = read(program, Dialect::Zx128).unwrap();
        let n = notes(&score, 0);
        assert_eq!(n[0], (0, 960, 60, 127));
        assert_eq!(n[1].2, 62);
        assert_eq!(n[1].0, 1200);
        assert_eq!(n[2].2, 62);
        assert_eq!(n[3].2, 76);
        assert_eq!(n[4].2, 66);
    }

    #[test]
    fn errors() {
        assert_eq!(read("MML@cdz;", Dialect::Mabinogi), Err(Error::BadText { line: 1, column: 7, why: "something MML doesn't have" }));
        assert!(read("10 PLAY \"XB$;\"", Dialect::GwBasic).is_err());
    }

    #[test]
    fn writes_and_reads_back() {
        let (score, _) = read("MML@t140l8o4cdefg4.r8>c2,o3c1;", Dialect::Mabinogi).unwrap();
        for d in [Dialect::Mabinogi, Dialect::MapleStory2, Dialect::Generic, Dialect::Msx] {
            let text = write(&score, d);
            let (again, _) = read(&text, if d == Dialect::Msx { detect(&text) } else { d }).unwrap();
            let all = |s: &Score| {
                let mut v: Vec<Note> = s.parts.iter().flat_map(|p| p.notes.iter().map(|n| Note { velocity: 0, ..*n })).collect();
                v.sort_by_key(|n| (n.at, n.key));
                v
            };
            assert_eq!(all(&again), all(&score), "{d:?}: {text}");
            assert_eq!(again.bpm, 140.0);
        }
        let gw = write(&score, Dialect::GwBasic);
        assert!(gw.starts_with("10 PLAY \"T140O4"), "{gw}");
    }

    #[test]
    fn sakura_is_compiled_by_its_own_compiler_and_written_back() {
        let song = "TrackName = {\"Two\"}\nTempo = 100\nTR(1) @1 l8 o5 c d e4 f&f g2\nTR(2) @33 o3 l2 c g\n";
        assert_eq!(detect(song), Dialect::Sakura);
        let (score, title) = read(song, Dialect::Sakura).unwrap();
        assert_eq!(title, "Two");
        assert_eq!(score.bpm, 100.0);
        let back = write(&score, Dialect::Sakura);
        let (again, _) = read(&back, Dialect::Sakura).unwrap_or_else(|e| panic!("{e}\n{back}"));
        let notes = |s: &Score| {
            let mut v: Vec<(u32, u8)> = s.parts.iter().flat_map(|p| p.notes.iter().map(|n| (n.at * 480 / u32::from(s.ppq), n.key))).collect();
            v.sort();
            v
        };
        assert_eq!(notes(&again), notes(&score), "\n{back}");
        assert_eq!(again.parts.iter().map(|p| p.program).collect::<Vec<_>>(), score.parts.iter().map(|p| p.program).collect::<Vec<_>>());
    }
}

