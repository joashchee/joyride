// Copyright 2026 ansiapps. Neumetik MIDI, MIT licensed: see LICENSE.

//! **A score as notation**, what the MusicXML, ABC and `**kern` writers
//! share: bars from the meter, each part's notes in voices, every note
//! and rest cut at bars and beats into lengths a score can show (plain or
//! dotted, tied), tuplets where the times ask for them, and each key
//! spelled by the key signature.
//!
//! Times come from the score's ticks exactly when every one is on a grid
//! of a 6720th of a whole note (64ths, and tuplets of 3, 5 and 7), as
//! music read from text is. Otherwise a time on the grid of 64ths or of
//! 48ths stays and any other goes to the nearer of the two (a file whose
//! triplets were written approximately), or, when most are off both (a
//! played MIDI file), to the nearer 32nd or 24th; a note is at least
//! one step of the grid long.

use super::{signature_accidentals, Frac};
use crate::score::{Score, DRUMS};

/// A key's letter (0 C to 6 B), its sharps (flats negative) and octave
/// (4 holds middle C).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spelled {
    pub step: u8,
    pub alter: i8,
    pub octave: i8,
}

impl Spelled {
    /// The letter, upper case.
    pub fn letter(self) -> char {
        b"CDEFGAB"[usize::from(self.step)] as char
    }
}

const NATURAL: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];

/// A key spelled in a key signature: a note of the scale by its letter,
/// else a natural letter, else a sharp in sharp keys (and C) and a flat
/// in flat keys.
pub fn spell(key: u8, sharps: i8) -> Spelled {
    let pc = i32::from(key) % 12;
    let sig = signature_accidentals(sharps);
    let pick = (0..7).find(|&s| (NATURAL[s] + sig[s]).rem_euclid(12) == pc).map(|s| (s, sig[s])).or_else(|| (0..7).find(|&s| NATURAL[s] == pc).map(|s| (s, 0))).unwrap_or_else(|| {
        if sharps >= 0 {
            let s = (0..7).find(|&s| (NATURAL[s] + 1) % 12 == pc).unwrap_or(0);
            (s, 1)
        } else {
            let s = (0..7).find(|&s| (NATURAL[s] + 11) % 12 == pc).unwrap_or(0);
            (s, -1)
        }
    });
    let natural = i32::from(key) - pick.1;
    // The letter's octave: B#3 sounds as C4, Cb4 as B3.
    let octave = (natural - NATURAL[pick.0]) / 12 - 1;
    Spelled { step: pick.0 as u8, alter: pick.1 as i8, octave: octave as i8 }
}

/// A title on one line, as a header field holds it.
pub fn one_line(title: &str) -> String {
    title.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A tuplet an item is in: `actual` notes in the time of `normal`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tuplet {
    pub actual: u32,
    pub normal: u32,
    /// The tuplet's first item, and its last.
    pub first: bool,
    pub last: bool,
    /// How many items it holds.
    pub count: u32,
}

/// A note, a chord or a rest in a voice's bar.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    /// From the bar's start, and how long it sounds, in whole notes.
    pub at: Frac,
    pub len: Frac,
    /// How it's written: the plain length (1 a whole, 1/4 a quarter) and
    /// its dots. In a tuplet it's longer than it sounds.
    pub base: Frac,
    pub dots: u8,
    /// Its keys, lowest first; none for a rest.
    pub keys: Vec<u8>,
    /// Each key's velocity, and the loudest.
    pub velocities: Vec<u8>,
    pub velocity: u8,
    /// Tied from the item before (the same note going on), and to the next.
    pub tie_from: bool,
    pub tie_to: bool,
    pub tuplet: Option<Tuplet>,
    /// The lyric sung on it.
    pub lyric: Option<String>,
}

impl Item {
    pub fn rest(&self) -> bool {
        self.keys.is_empty()
    }
}

/// A bar: when it starts and how long it is, its meter, key signature,
/// and the tempo when it changes there (the first bar always has one).
#[derive(Debug, Clone, PartialEq)]
pub struct Bar {
    pub start: Frac,
    pub len: Frac,
    pub beats: u8,
    pub unit: u8,
    pub sharps: i8,
    pub minor: bool,
    pub tempo: Option<f64>,
}

/// A part as notation.
#[derive(Debug, Clone, PartialEq)]
pub struct Part {
    pub name: String,
    pub channel: u8,
    pub program: u8,
    pub drums: bool,
    /// Each voice's bars, every bar filled from start to end.
    pub voices: Vec<Vec<Vec<Item>>>,
}

impl Part {
    /// The middle of its keys, to choose a clef.
    pub fn middle(&self) -> u8 {
        let keys: Vec<u32> = self.voices.iter().flatten().flatten().flat_map(|i| i.keys.iter().map(|&k| u32::from(k))).collect();
        if keys.is_empty() {
            60
        } else {
            (keys.iter().sum::<u32>() / keys.len() as u32) as u8
        }
    }
}

/// A score as notation.
#[derive(Debug, Clone, PartialEq)]
pub struct Notation {
    pub bars: Vec<Bar>,
    pub parts: Vec<Part>,
    /// Chord symbols and section markers, in whole notes from the start.
    pub chords: Vec<(Frac, String)>,
    pub markers: Vec<(Frac, String)>,
}

/// A time on a grid of a 6720th of a whole note, as music read from
/// text has.
fn on_grid(t: Frac) -> bool {
    6720 % t.den() == 0
}

fn pow2(n: i64) -> bool {
    n > 0 && n & (n - 1) == 0
}

/// The largest power of two below `m` (m >= 2).
fn below(m: i64) -> i64 {
    let mut p = 1;
    while p * 2 < m {
        p *= 2;
    }
    p
}

fn lcm(a: i64, b: i64) -> i64 {
    let (mut x, mut y) = (a, b);
    while y != 0 {
        (x, y) = (y, x % y);
    }
    (a / x.max(1)).saturating_mul(b)
}

fn odd_part(mut n: i64) -> i64 {
    while n > 0 && n % 2 == 0 {
        n /= 2;
    }
    n.max(1)
}

/// A meta event's type and data, if it is one.
fn meta(bytes: &[u8]) -> Option<(u8, &[u8])> {
    if bytes.len() < 3 || bytes[0] != 0xff {
        return None;
    }
    let kind = bytes[1];
    // A variable-length count.
    let mut i = 2;
    let mut n = 0usize;
    while i < bytes.len() {
        n = (n << 7) | usize::from(bytes[i] & 0x7f);
        i += 1;
        if bytes[i - 1] & 0x80 == 0 {
            break;
        }
    }
    bytes.get(i..i + n).map(|d| (kind, d))
}

/// A segment of a voice in one bar: from, to (whole notes from the bar's
/// start), keys, their velocities, tied from before and on to after,
/// the lyric.
type Seg = (Frac, Frac, Vec<u8>, Vec<u8>, bool, bool, Option<String>);

/// A voice's chord (or note) before it's cut into bars: from, to, keys,
/// their velocities, the lyric.
type Chord = (Frac, Frac, Vec<u8>, Vec<u8>, Option<String>);

/// The score as notation.
pub fn notate(score: &Score) -> Notation {
    let whole = Frac::int(4 * i64::from(score.ppq.max(1)));
    let at = |t: u32| Frac::int(i64::from(t)).div(whole);
    let times: Vec<Frac> = score.parts.iter().flat_map(|p| p.notes.iter()).flat_map(|n| [at(n.at), at(n.at + n.length)]).collect();
    let exact = times.iter().all(|&t| on_grid(t));
    // Not exact: a notated file with approximate tuplets keeps what's on
    // the grids of 64ths and of 48ths and rounds the rest to the nearer;
    // a performance (most times off both) to 32nds or 24ths.
    let fine = |t: Frac| 64 % t.den() == 0 || 48 % t.den() == 0;
    let (binary, triple) = if times.iter().filter(|&&t| !fine(t)).count() * 4 > times.len() * 3 { (32, 24) } else { (64, 48) };
    let snap = |t: Frac| {
        if exact || binary % t.den() == 0 || triple % t.den() == 0 {
            return t;
        }
        let b = Frac::new((t.f64() * binary as f64).round() as i64, binary);
        let c = Frac::new((t.f64() * triple as f64).round() as i64, triple);
        if (b.sub(t)).f64().abs() <= (c.sub(t)).f64().abs() {
            b
        } else {
            c
        }
    };
    let least = if exact { Frac::new(1, 6720) } else { Frac::new(1, triple.max(binary)) };
    // Meters, keys, tempos, chord symbols and markers.
    let mut meters = vec![(Frac::ZERO, score.beats.max(1), score.unit.max(1))];
    let mut keys: Vec<(Frac, i8, bool)> = vec![];
    let mut tempos = vec![(Frac::ZERO, score.bpm)];
    let (mut chords, mut markers) = (vec![], vec![]);
    for k in &score.kept {
        let Some((kind, d)) = meta(&k.bytes) else { continue };
        let t = snap(at(k.at));
        match kind {
            0x58 if d.len() >= 2 && d[0] > 0 && d[1] <= 5 => meters.push((t, d[0], 1u8 << d[1])),
            0x59 if d.len() >= 2 => keys.push((t, (d[0] as i8).clamp(-7, 7), d[1] == 1)),
            0x51 if d.len() >= 3 => {
                let micros = u32::from(d[0]) << 16 | u32::from(d[1]) << 8 | u32::from(d[2]);
                if micros > 0 {
                    tempos.push((t, (60_000_000.0 / f64::from(micros) * 100.0).round() / 100.0));
                }
            }
            0x01 => chords.push((t, String::from_utf8_lossy(d).into_owned())),
            0x06 => markers.push((t, String::from_utf8_lossy(d).into_owned())),
            _ => {}
        }
    }
    meters.sort_by_key(|m| m.0);
    keys.sort_by_key(|k| k.0);
    tempos.sort_by_key(|t| t.0);
    // The notes of each part, in whole notes.
    struct N {
        at: Frac,
        end: Frac,
        key: u8,
        velocity: u8,
        lyric: Option<String>,
    }
    let mut parts_notes: Vec<Vec<N>> = vec![];
    let mut end = Frac::ZERO;
    for p in &score.parts {
        let lyrics: Vec<(Frac, String)> = p.kept.iter().filter_map(|k| meta(&k.bytes).filter(|m| m.0 == 0x05).map(|(_, d)| (snap(at(k.at)), String::from_utf8_lossy(d).into_owned()))).collect();
        let mut notes: Vec<N> = p
            .notes
            .iter()
            .map(|n| {
                let s = snap(at(n.at));
                let e = snap(at(n.at + n.length)).max(s.add(least));
                N { at: s, end: e, key: n.key, velocity: n.velocity, lyric: None }
            })
            .collect();
        notes.sort_by(|a, b| a.at.cmp(&b.at).then(b.key.cmp(&a.key)).then(b.end.cmp(&a.end)));
        for (t, text) in lyrics {
            if let Some(n) = notes.iter_mut().filter(|n| n.at == t).min_by_key(|n| std::cmp::Reverse(n.key)) {
                n.lyric = Some(text);
            }
        }
        end = end.max(notes.iter().map(|n| n.end).max().unwrap_or(Frac::ZERO));
        parts_notes.push(notes);
    }
    // The music is as long as the score's bars, or its last note.
    end = end.max(snap(at(score.bars.saturating_mul(score.bar_ticks()))));
    // The bars: each as long as its meter, a meter change starting a bar.
    let mut bars: Vec<Bar> = vec![];
    let (mut pos, mut mi, mut ki, mut ti) = (Frac::ZERO, 0, 0, 0);
    let (mut meter, mut key) = ((meters[0].1, meters[0].2), (0i8, false));
    let last_meter = meters.iter().map(|m| m.0).max().unwrap_or(Frac::ZERO);
    while bars.is_empty() || pos < end || pos <= last_meter && mi < meters.len() {
        while mi < meters.len() && meters[mi].0 <= pos {
            meter = (meters[mi].1, meters[mi].2);
            mi += 1;
        }
        let mut len = Frac::new(i64::from(meter.0), i64::from(meter.1));
        // A meter change inside this bar ends it there.
        if let Some(m) = meters.get(mi) {
            if m.0 < pos.add(len) {
                len = m.0.sub(pos);
            }
        }
        while ki < keys.len() && keys[ki].0 < pos.add(len) {
            key = (keys[ki].1, keys[ki].2);
            ki += 1;
        }
        let mut tempo = None;
        while ti < tempos.len() && tempos[ti].0 < pos.add(len) {
            tempo = Some(tempos[ti].1);
            ti += 1;
        }
        bars.push(Bar { start: pos, len, beats: meter.0, unit: meter.1, sharps: key.0, minor: key.1, tempo });
        pos = pos.add(len);
        if bars.len() > 100_000 {
            break;
        }
    }
    if bars[0].tempo.is_none() {
        bars[0].tempo = Some(score.bpm);
    }
    // Each part's voices.
    let mut parts = vec![];
    for (p, notes) in score.parts.iter().zip(parts_notes) {
        // Notes starting and ending together are a chord; a voice takes
        // what doesn't overlap what it has.
        let mut voices: Vec<Vec<Chord>> = vec![];
        for n in notes {
            // (A key sounding twice at once, two voices sharing a note, is
            // each voice's.)
            if let Some(c) = voices.iter_mut().flat_map(|v| v.iter_mut()).find(|c| c.0 == n.at && c.1 == n.end && !c.2.contains(&n.key)) {
                c.2.push(n.key);
                c.3.push(n.velocity);
                if c.4.is_none() {
                    c.4 = n.lyric;
                }
                continue;
            }
            match voices.iter().position(|v| v.last().map_or(true, |c| c.1 <= n.at)) {
                Some(i) => voices[i].push((n.at, n.end, vec![n.key], vec![n.velocity], n.lyric)),
                None => voices.push(vec![(n.at, n.end, vec![n.key], vec![n.velocity], n.lyric)]),
            }
        }
        if voices.is_empty() {
            voices.push(vec![]);
        }
        let mut out = vec![];
        for v in &mut voices {
            for c in v.iter_mut() {
                let mut kv: Vec<(u8, u8)> = c.2.iter().copied().zip(c.3.iter().copied()).collect();
                kv.sort_unstable();
                (c.2, c.3) = kv.into_iter().unzip();
            }
            let mut vbars = vec![];
            for bar in &bars {
                let (bs, be) = (bar.start, bar.start.add(bar.len));
                let mut segs: Vec<Seg> = vec![];
                let mut now = Frac::ZERO;
                for c in v.iter().filter(|c| c.0 < be && c.1 > bs) {
                    let s = c.0.max(bs).sub(bs);
                    let e = c.1.min(be).sub(bs);
                    if s > now {
                        segs.push((now, s, vec![], vec![], false, false, None));
                    }
                    segs.push((s, e, c.2.clone(), c.3.clone(), c.0 < bs, c.1 > be, if c.0 >= bs { c.4.clone() } else { None }));
                    now = e;
                }
                if now < bar.len {
                    segs.push((now, bar.len, vec![], vec![], false, false, None));
                }
                vbars.push(lay_out(&segs, bar));
            }
            out.push(vbars);
        }
        parts.push(Part { name: p.name.clone(), channel: p.channel, program: p.program, drums: p.channel == DRUMS, voices: out });
    }
    Notation { bars, parts, chords, markers }
}

/// A bar's segments as items: cut into windows of a beat (joined where a
/// tuplet's note crosses between them), each window that holds times off
/// the binary grid a tuplet, then every segment cut into written lengths.
fn lay_out(segs: &[Seg], bar: &Bar) -> Vec<Item> {
    let beat = Frac::new(1, i64::from(bar.unit.max(1)));
    // Window edges: every beat, and the bar's end.
    let mut edges = vec![Frac::ZERO];
    let mut t = beat;
    while t < bar.len {
        edges.push(t);
        t = t.add(beat);
    }
    edges.push(bar.len);
    let binary = |t: Frac| pow2(t.den());
    // A segment crossing an edge with a time off the grid joins the
    // windows on both sides.
    loop {
        let before = edges.len();
        edges.retain(|&e| e == Frac::ZERO || e == bar.len || !segs.iter().any(|s| s.0 < e && s.1 > e && (!binary(s.0) || !binary(s.1))));
        if edges.len() == before {
            break;
        }
    }
    // Each window's tuplet: the odd part of every time's denominator in
    // it; windows without one run together.
    let mut windows: Vec<(Frac, Frac, i64)> = vec![];
    for w in edges.windows(2) {
        let (ws, we) = (w[0], w[1]);
        let mut d = we.sub(ws).den();
        for s in segs.iter().filter(|s| s.0 < we && s.1 > ws) {
            for x in [s.0.max(ws), s.1.min(we)] {
                d = lcm(d, x.den());
            }
        }
        let m = odd_part(d);
        match windows.last_mut() {
            Some(last) if m == 1 && last.2 == 1 => last.1 = we,
            _ => windows.push((ws, we, m)),
        }
    }
    let mut items = vec![];
    for (ws, we, m) in windows {
        let (actual, normal) = if m > 1 { (m, below(m)) } else { (1, 1) };
        let scale = Frac::new(actual, normal);
        let first = items.len();
        for s in segs.iter().filter(|s| s.0 < we && s.1 > ws) {
            let (from, to) = (s.0.max(ws), s.1.min(we));
            let rest = s.2.is_empty();
            let (tie_in, tie_out) = (s.4 || from > s.0, s.5 || to < s.1);
            // Written times, from the window's start (or the bar's).
            let origin = if actual > 1 { ws } else { Frac::ZERO };
            let (mut wpos, wend) = (from.sub(origin).mul(scale), to.sub(origin).mul(scale));
            let mut pieces = vec![];
            while wpos < wend {
                let (base, dots, len) = fit(wpos, wend.sub(wpos));
                pieces.push((wpos, base, dots, len));
                wpos = wpos.add(len);
            }
            let n = pieces.len();
            for (i, (wp, base, dots, len)) in pieces.into_iter().enumerate() {
                items.push(Item {
                    at: wp.div(scale).add(origin),
                    len: len.div(scale),
                    base,
                    dots,
                    keys: s.2.clone(),
                    velocities: s.3.clone(),
                    velocity: s.3.iter().copied().max().unwrap_or(0),
                    tie_from: !rest && (if i == 0 { tie_in } else { true }),
                    tie_to: !rest && (if i + 1 == n { tie_out } else { true }),
                    tuplet: None,
                    lyric: if i == 0 && from == s.0 { s.6.clone() } else { None },
                });
            }
        }
        if actual > 1 {
            let count = (items.len() - first) as u32;
            let last = items.len() - 1;
            for (i, it) in items.iter_mut().enumerate().skip(first) {
                it.tuplet = Some(Tuplet { actual: actual as u32, normal: normal as u32, first: i == first, last: i == last, count });
            }
        }
    }
    items
}

/// The longest written length that fits at `pos` within `room` (both
/// binary): plain, starting where its length divides, or dotted,
/// starting where twice its plain length divides.
/// Returns the plain length, the dots and the whole length.
fn fit(pos: Frac, room: Frac) -> (Frac, u8, Frac) {
    let mut plain = Frac::int(2);
    while plain >= Frac::new(1, 1024) {
        let aligned = pos.div(plain).den() == 1;
        let dotted = plain.scale(3, 2);
        if dotted <= room && pos.div(plain.scale(2, 1)).den() == 1 && plain < Frac::int(2) {
            return (plain, 1, dotted);
        }
        if plain <= room && aligned {
            return (plain, 0, plain);
        }
        plain = plain.scale(1, 2);
    }
    // Off every grid (it shouldn't be): the rest as it is.
    (room, 0, room)
}

/// A plain length's name in MusicXML (`quarter`), if it has one.
pub fn type_name(base: Frac) -> &'static str {
    match (base.num(), base.den()) {
        (4, 1) => "long",
        (2, 1) => "breve",
        (1, 1) => "whole",
        (1, 2) => "half",
        (1, 4) => "quarter",
        (1, 8) => "eighth",
        (1, 16) => "16th",
        (1, 32) => "32nd",
        (1, 64) => "64th",
        (1, 128) => "128th",
        (1, 256) => "256th",
        (1, 512) => "512th",
        _ => "1024th",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::score::{Note, Part as SPart};

    fn score(notes: &[(u32, u32, u8)]) -> Score {
        Score {
            ppq: 480,
            bpm: 100.0,
            beats: 4,
            unit: 4,
            bars: 1,
            parts: vec![SPart { name: "Piano".into(), channel: 0, bank: 0, program: 0, notes: notes.iter().map(|&(at, length, key)| Note { at, length, key, velocity: 90 }).collect(), kept: vec![] }],
            kept: vec![],
        }
    }

    #[test]
    fn keys_are_spelled_by_the_signature() {
        let s = |k, sh| {
            let x = spell(k, sh);
            format!("{}{}{}", x.letter(), match x.alter {
                1 => "#",
                -1 => "b",
                _ => "",
            }, x.octave)
        };
        assert_eq!(s(60, 0), "C4");
        assert_eq!(s(61, 0), "C#4");
        assert_eq!(s(70, -1), "Bb4");
        assert_eq!(s(66, 1), "F#4");
        assert_eq!(s(63, -3), "Eb4");
        assert_eq!(s(60, 7), "B#3");
        assert_eq!(s(72, 2), "C5");
        assert_eq!(s(47, -2), "B2");
        assert_eq!(s(71, -7), "Cb5");
    }

    #[test]
    fn a_note_across_the_bar_is_tied_and_a_bar_is_filled_with_rests() {
        // A half note at beat 4 of bar 1: a quarter tied to a quarter.
        let n = notate(&score(&[(1440, 960, 60)]));
        assert_eq!(n.bars.len(), 2);
        let v = &n.parts[0].voices[0];
        let b1: Vec<(Frac, u8, bool, bool, bool)> = v[0].iter().map(|i| (i.base, i.dots, i.rest(), i.tie_from, i.tie_to)).collect();
        assert_eq!(b1, vec![(Frac::new(1, 2), 1, true, false, false), (Frac::new(1, 4), 0, false, false, true)]);
        assert!(v[1][0].tie_from && !v[1][0].tie_to);
        // Then a quarter rest and a half rest, not a dotted quarter off the beat.
        let b2: Vec<(Frac, bool)> = v[1][1..].iter().map(|i| (i.base, i.rest())).collect();
        assert_eq!(b2, vec![(Frac::new(1, 4), true), (Frac::new(1, 2), true)]);
    }

    #[test]
    fn triplets_are_found_and_written_as_eighths() {
        // Three triplet eighths on beat 1, then a quarter.
        let n = notate(&score(&[(0, 160, 60), (160, 160, 62), (320, 160, 64), (480, 480, 65)]));
        let v = &n.parts[0].voices[0][0];
        assert_eq!(v[0].base, Frac::new(1, 8));
        assert_eq!(v[0].tuplet.map(|t| (t.actual, t.normal, t.first, t.count)), Some((3, 2, true, 3)));
        assert!(v[2].tuplet.is_some_and(|t| t.last));
        assert_eq!(v[3].tuplet, None);
        assert_eq!(v[3].base, Frac::new(1, 4));
        // Triplet quarters (crossing beats): one tuplet over two beats.
        let n = notate(&score(&[(0, 320, 60), (320, 320, 62), (640, 320, 64)]));
        let v = &n.parts[0].voices[0][0];
        assert_eq!(v[0].base, Frac::new(1, 4));
        assert_eq!(v[0].tuplet.map(|t| t.count), Some(3));
    }

    #[test]
    fn overlapping_notes_go_to_a_second_voice_and_equal_ones_make_chords() {
        let n = notate(&score(&[(0, 1920, 48), (0, 480, 60), (0, 480, 64), (480, 480, 62)]));
        let p = &n.parts[0];
        assert_eq!(p.voices.len(), 2);
        assert_eq!(p.voices[0][0][0].keys, vec![60, 64]);
        assert_eq!(p.voices[1][0][0].keys, vec![48]);
    }

    /// Every note as (part, start, length, key) in whole notes, sorted.
    fn notes_of(s: &Score) -> Vec<(usize, Frac, Frac, u8)> {
        let whole = Frac::int(4 * i64::from(s.ppq));
        let mut v: Vec<_> = s.parts.iter().enumerate().flat_map(|(i, p)| p.notes.iter().map(move |n| (i, Frac::int(i64::from(n.at)).div(whole), Frac::int(i64::from(n.length)).div(whole), n.key))).collect();
        v.sort_by_key(|a| (a.0, a.1, a.3));
        v
    }

    /// Music with ties over bars, triplets of eighths and of quarters, a
    /// quintuplet, chords, a second voice, accidentals, a key and a meter
    /// change, and drums.
    fn busy() -> Score {
        let abc = "X:1\nT:Busy\nM:4/4\nL:1/8\nQ:1/4=96\nK:D\nV:1\n%%MIDI program 40\n(3def g2 a4- | a2 [Ac]2 (3B2A2G2 | (5ABcde ^c2 =c2 d2 |[K:Bb][M:3/4] B,2 E2 _A2 | z6 |]\nV:2\n%%MIDI program 32\nD,8- | D,4 A,,4 | D,2 z6 | B,,6 | B,,6 |]\nV:3\n%%MIDI channel 10\nC,2 ^F,2 D,2 ^F,2 | C,8 | z8 | C,6 | C,6 |]\n";
        super::super::abc::read(abc).expect("the test tune reads").0
    }

    #[test]
    fn musicxml_written_reads_back_the_same() {
        let s = busy();
        let xml = super::super::musicxml::write(&s, "Busy");
        let (back, title) = super::super::musicxml::read(&xml).expect("our MusicXML reads");
        assert_eq!(title, "Busy");
        assert_eq!(notes_of(&back), notes_of(&s));
        assert_eq!(back.parts.iter().map(|p| (p.channel == DRUMS, p.program)).collect::<Vec<_>>(), s.parts.iter().map(|p| (p.channel == DRUMS, p.program)).collect::<Vec<_>>());
        let vel = |s: &Score| s.parts.iter().flat_map(|p| p.notes.iter().map(|n| n.velocity)).collect::<Vec<_>>();
        assert_eq!(vel(&back), vel(&s));
        assert_eq!(back.bpm, 96.0);
    }

    #[test]
    fn abc_written_reads_back_the_same() {
        let s = busy();
        let abc = super::super::abc::write(&s, "Busy");
        let (back, title) = super::super::abc::read(&abc).unwrap_or_else(|e| panic!("{e}\n{abc}"));
        assert_eq!(title, "Busy");
        assert_eq!(notes_of(&back), notes_of(&s), "\n{abc}");
        assert_eq!(back.parts.iter().map(|p| (p.channel == DRUMS, p.program)).collect::<Vec<_>>(), s.parts.iter().map(|p| (p.channel == DRUMS, p.program)).collect::<Vec<_>>());
    }

    #[test]
    fn kern_written_reads_back_the_same() {
        let s = busy();
        let kern = super::super::kern::write(&s, "Busy");
        let (back, title) = super::super::kern::read(&kern).unwrap_or_else(|e| panic!("{e}\n{kern}"));
        assert_eq!(title, "Busy");
        // The parts come back last first, as the spines are written.
        let mut back = back;
        back.parts.reverse();
        assert_eq!(notes_of(&back), notes_of(&s), "\n{kern}");
        assert_eq!(back.parts.iter().map(|p| (p.channel == DRUMS, p.program)).collect::<Vec<_>>(), s.parts.iter().map(|p| (p.channel == DRUMS, p.program)).collect::<Vec<_>>());
        assert_eq!(back.bpm, 96.0);
    }

    #[test]
    fn kern_lengths_are_reciprocals() {
        use super::super::kern::write;
        let k = write(&score(&[(0, 160, 60), (160, 160, 62), (320, 160, 64), (480, 720, 65), (1200, 240, 67)]), "");
        assert!(k.contains("12c\t") || k.contains("12c\n"), "{k}");
        // A dotted quarter off the half bar: a quarter tied to an eighth.
        assert!(k.contains("[4f\n8f]\n"), "{k}");
        let k = write(&score(&[(0, 1440, 60)]), "");
        assert!(k.contains("2.c\n"), "{k}");
    }

}

