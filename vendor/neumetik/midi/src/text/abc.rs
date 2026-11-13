// Copyright 2026 ansiapps. Neumetik MIDI, MIT licensed: see LICENSE.

//! **ABC**, from the ABC 2.1 standard (abcnotation.com): header fields
//! (`X: T: M: L: Q: K: V: P:`), keys with their modes and extra
//! accidentals, note lengths, broken rhythm, ties, tuplets, chords, grace
//! notes (skipped: they take no time), repeats and endings played out,
//! parts played in the header's `P:` order, voices, aligned lyrics
//! (`w:`), chord symbols, `%%MIDI program` and `channel`, and the
//! dynamics among the decorations as velocity.
//!
//! Written ([`write`]) from [`super::notation`]: a voice for each voice
//! of each part (`%%MIDI program`, `channel 10` for drums), bars with
//! their key, meter and tempo changes inline, ties, tuplets as
//! `(p:q:r`, accidentals on every note off the key signature (and
//! naturals where the bar had one), velocity as the nearest
//! dynamic, and chord symbols. Lyrics aren't written.
//!
//! A file holds many tunes (each starts at `X:`): [`tunes`] reads them
//! all, [`read`] the first, except a Lord of the Rings Online file, whose
//! tunes are an ensemble's parts, read together.

use super::{bad, key_signature, letter_index, signature_accidentals, Builder, Frac};
use crate::score::Score;
use crate::Error;

#[derive(Debug, Clone)]
enum Item {
    /// Notes (none: a rest) with their ties, a length, velocity, a chord
    /// symbol and a lyric.
    Ev { keys: Vec<(i32, bool)>, len: Frac, vel: u8, chord: Option<String>, lyric: Option<String> },
    Bar(Bar),
    Ending(Vec<u32>),
    Tempo(f64),
    Meter(u8, u8),
    Key(i8, bool),
    Program(u8),
    Part(char),
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Bar {
    Plain,
    Start,
    End,
    Both,
}

#[derive(Debug, Clone)]
struct Voice {
    id: String,
    name: String,
    items: Vec<Item>,
    program: Option<u8>,
    drums: bool,
    transpose: i32,
    /// The key signature's accidentals by letter, and those set in the bar
    /// by (letter, octave).
    sig: [i32; 7],
    explicit: Vec<(usize, i32, i32)>,
    /// Notes tied from: (letter, octave, key).
    tied: Vec<(usize, i32, i32)>,
    vel: u8,
    /// The events of the music line being read, and of the last one, for `w:`.
    line: Vec<usize>,
    last_line: Vec<usize>,
    part_seen: usize,
}

impl Voice {
    fn new(id: &str, sig: [i32; 7]) -> Voice {
        Voice { id: id.into(), name: String::new(), items: vec![], program: None, drums: false, transpose: 0, sig, explicit: vec![], tied: vec![], vel: 80, line: vec![], last_line: vec![], part_seen: 0 }
    }
}

struct Tune<'a> {
    text: &'a str,
    title: String,
    unit: Option<Frac>,
    meter: (u8, u8),
    /// The header's meter, which the music starts in.
    header_meter: (u8, u8),
    meter_frac: Frac,
    tempo: Option<f64>,
    sig: [i32; 7],
    voices: Vec<Voice>,
    cur: usize,
    order: Option<String>,
    part_count: usize,
    part_label: char,
    in_header: bool,
    /// A chord symbol waiting for its note.
    chord: Option<String>,
}

/// Whether a file's tunes are a Lord of the Rings Online ensemble's
/// parts, to play together.
pub fn is_ensemble(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let tunes = text.lines().filter(|l| l.trim_start().starts_with("X:")).count();
    tunes > 1 && (lower.contains("lotro") || lower.contains("lord of the rings") || lower.contains("maestro") || lower.contains("%%part-name"))
}

/// A file's first tune (or, an ensemble's, all of them together) as a
/// score and its title.
pub fn read(text: &str) -> Result<(Score, String), Error> {
    let starts = tune_starts(text);
    if starts.is_empty() {
        return Err(bad(text, 0, "an ABC tune starts with X: and has a K: line"));
    }
    let mut b = Builder::new();
    if is_ensemble(text) {
        let mut title = String::new();
        for (i, &(s, e)) in starts.iter().enumerate() {
            let t = parse_tune(text, s, e, &mut b, true)?;
            if i == 0 {
                title = t.split(['(', '[']).next().unwrap_or(&t).trim().trim_end_matches(['-', ' ']).to_string();
            }
        }
        b.title = title.clone();
        return Ok((b.finish(), title));
    }
    let (s, e) = starts[0];
    let title = parse_tune(text, s, e, &mut b, false)?;
    b.title = title.clone();
    Ok((b.finish(), title))
}

/// Every tune in a file, each a score and its title (or why it can't be read).
pub fn tunes(text: &str) -> Vec<Result<(Score, String), Error>> {
    tune_starts(text)
        .into_iter()
        .map(|(s, e)| {
            let mut b = Builder::new();
            let title = parse_tune(text, s, e, &mut b, false)?;
            b.title = title.clone();
            Ok((b.finish(), title))
        })
        .collect()
}

/// Where each tune starts and ends: X: lines (or the whole text when it
/// has a K: line and no X:).
fn tune_starts(text: &str) -> Vec<(usize, usize)> {
    let mut starts = vec![];
    let mut at = 0;
    for line in text.split_inclusive('\n') {
        if line.trim_start().starts_with("X:") {
            starts.push(at);
        }
        at += line.len();
    }
    if starts.is_empty() && text.lines().any(|l| l.starts_with("K:")) {
        starts.push(0);
    }
    let mut out = vec![];
    for (i, &s) in starts.iter().enumerate() {
        out.push((s, starts.get(i + 1).copied().unwrap_or(text.len())));
    }
    out
}

fn parse_meter(v: &str) -> Option<(u8, u8)> {
    let v = v.trim();
    match v {
        "C" => return Some((4, 4)),
        "C|" => return Some((2, 2)),
        "" | "none" => return None,
        _ => {}
    }
    let (n, d) = v.split_once('/')?;
    let beats: u32 = n.trim().trim_matches(['(', ')']).split('+').map(|x| x.trim().parse::<u32>().unwrap_or(0)).sum();
    let unit: u32 = d.split_whitespace().next()?.parse().ok()?;
    (beats > 0 && beats < 256 && unit.is_power_of_two() && unit <= 32).then_some((beats as u8, unit as u8))
}

fn parse_frac(v: &str) -> Option<Frac> {
    let v = v.trim();
    match v.split_once('/') {
        Some((n, d)) => {
            let (n, d): (i64, i64) = (n.trim().parse().ok()?, d.trim().parse().ok()?);
            (n > 0 && d > 0).then(|| Frac::new(n, d))
        }
        None => v.parse::<i64>().ok().filter(|&n| n > 0).map(Frac::int),
    }
}

/// `Q:`: `1/4=120`, `3/8=40`, `1/4 1/8=60`, `C=120`, `120` (unit notes a
/// minute), with any quoted text around; as quarter notes a minute.
fn parse_tempo(v: &str, unit: Frac) -> Option<f64> {
    let mut s = String::new();
    let mut quoted = false;
    for c in v.chars() {
        if c == '"' {
            quoted = !quoted;
        } else if !quoted {
            s.push(c);
        }
    }
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let (beat, n) = match s.split_once('=') {
        Some((l, r)) => {
            let l = l.trim();
            let beat = if l.starts_with('C') || l.starts_with('L') {
                unit
            } else {
                l.split_whitespace().filter_map(parse_frac).fold(Frac::ZERO, |a, b| a.add(b))
            };
            (beat, r.split_whitespace().next()?.parse::<f64>().ok()?)
        }
        None => (unit, s.split_whitespace().next()?.parse::<f64>().ok()?),
    };
    (beat.is_positive() && n > 0.0).then(|| n * beat.f64() * 4.0)
}

/// `K:`: a key and mode, `none`, `HP`/`Hp`, then any explicit accidentals
/// (`^f _b =c`) and other settings.
fn parse_key(v: &str) -> Option<([i32; 7], i8, bool)> {
    let mut words = v.split_whitespace().peekable();
    let mut sig = [0; 7];
    let (mut sharps, mut minor) = (0, false);
    if let Some(&first) = words.peek() {
        // `none` and `HP` (bagpipe music written without a signature).
        if first == "none" || first == "HP" {
            words.next();
        } else if first == "Hp" {
            sig[3] = 1;
            sig[0] = 1;
            sig[4] = 0;
            sharps = 2;
            words.next();
        } else if first.starts_with(|c: char| "ABCDEFG".contains(c)) {
            let mut name = words.next().unwrap_or("").to_string();
            // A mode as its own word.
            if let Some(&m) = words.peek() {
                let ml = m.to_ascii_lowercase();
                if !m.contains('=') && !m.starts_with(['^', '_', '=']) && ["maj", "min", "m", "ion", "aeo", "mix", "dor", "phr", "lyd", "loc", "major", "minor"].iter().any(|x| ml.starts_with(x)) {
                    name.push_str(m);
                    words.next();
                }
            }
            let (s, m) = key_signature(&name)?;
            sharps = s;
            minor = m;
            sig = signature_accidentals(s);
        }
    }
    for w in words {
        if w == "exp" {
            // Explicit: only the accidentals that follow.
            sig = [0; 7];
            continue;
        }
        let acc = match w.chars().next() {
            Some('^') => 1,
            Some('_') => -1,
            Some('=') => 0,
            _ => continue,
        };
        let w2 = w.trim_start_matches(['^', '_', '=']);
        let n = (w.len() - w2.len()) as i32;
        if let Some(i) = w2.chars().next().and_then(letter_index) {
            sig[i] = acc * n;
        }
    }
    Some((sig, sharps, minor))
}

/// The time a part order like `AABB`, `A2B`, `(AB)3C` plays, as labels.
fn part_order(s: &str) -> Vec<char> {
    fn go(chars: &[char], i: &mut usize, depth: usize) -> Vec<char> {
        let mut out = vec![];
        while *i < chars.len() {
            let c = chars[*i];
            *i += 1;
            let seq = match c {
                '(' if depth < 8 => go(chars, i, depth + 1),
                ')' => return out,
                c if c.is_ascii_alphabetic() => vec![c],
                _ => continue,
            };
            let mut n = 0usize;
            while *i < chars.len() && chars[*i].is_ascii_digit() {
                n = n * 10 + chars[*i].to_digit(10).unwrap_or(0) as usize;
                *i += 1;
            }
            for _ in 0..n.clamp(1, 32) {
                out.extend(&seq);
            }
            if out.len() > 4096 {
                break;
            }
        }
        out
    }
    let chars: Vec<char> = s.chars().collect();
    go(&chars, &mut 0, 0)
}

const DYNAMICS: [(&str, u8); 10] = [("pppp", 15), ("ppp", 30), ("pp", 45), ("p", 60), ("mp", 75), ("mf", 90), ("f", 105), ("ff", 120), ("fff", 127), ("ffff", 127)];

fn parse_tune(text: &str, start: usize, end: usize, b: &mut Builder, ensemble: bool) -> Result<String, Error> {
    let mut t = Tune {
        text,
        title: String::new(),
        unit: None,
        meter: (4, 4),
        header_meter: (4, 4),
        meter_frac: Frac::int(1),
        tempo: None,
        sig: [0; 7],
        voices: vec![],
        cur: 0,
        order: None,
        part_count: 0,
        part_label: ' ',
        in_header: true,
        chord: None,
    };
    let mut meter_set = false;
    let mut at = start;
    let mut continued = false;
    for raw in text[start..end].split_inclusive('\n') {
        let line_at = at;
        at += raw.len();
        let line = raw.trim_end_matches(['\n', '\r']);
        let trimmed = line.trim_start();
        if trimmed.starts_with("%%") {
            directive(&mut t, trimmed);
            continue;
        }
        if trimmed.starts_with('%') || (trimmed.is_empty() && !continued) {
            if trimmed.is_empty() && !t.in_header {
                // A blank line ends the tune.
                break;
            }
            continue;
        }
        // A field: a letter and a colon (not |: or a note).
        let bytes = trimmed.as_bytes();
        if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() && !continued {
            let value = trimmed[2..].split('%').next().unwrap_or("").trim();
            let field = bytes[0] as char;
            if field == 'w' && !t.in_header {
                lyrics(&mut t, value);
                continue;
            }
            field_line(&mut t, field, value, line_at, &mut meter_set)?;
            continue;
        }
        if t.in_header {
            // Music before K: ends the header there.
            t.in_header = false;
            finish_header(&mut t);
        }
        let mut body = line;
        continued = false;
        if let Some(stripped) = body.trim_end().split('%').next().unwrap_or("").strip_suffix('\\') {
            body = stripped;
            continued = true;
        }
        let body_at = line_at;
        music(&mut t, body, body_at)?;
        if !continued {
            let v = &mut t.voices[t.cur];
            v.last_line = std::mem::take(&mut v.line);
        }
    }
    if t.in_header {
        finish_header(&mut t);
    }
    perform(&mut t, b, ensemble);
    Ok(t.title)
}

fn finish_header(t: &mut Tune) {
    if t.unit.is_none() {
        t.unit = Some(if t.meter_frac < Frac::new(3, 4) { Frac::new(1, 16) } else { Frac::new(1, 8) });
    }
    if t.voices.is_empty() {
        t.voices.push(Voice::new("1", t.sig));
    }
}

fn voice_index(t: &mut Tune, id: &str) -> usize {
    if let Some(i) = t.voices.iter().position(|v| v.id == id) {
        return i;
    }
    // The tune's first voice, unnamed until now, is this one.
    if t.voices.len() == 1 && t.voices[0].id == "1" && t.voices[0].items.is_empty() && t.voices[0].name.is_empty() {
        t.voices[0].id = id.into();
        return 0;
    }
    t.voices.push(Voice::new(id, t.sig));
    t.voices.len() - 1
}

fn field_line(t: &mut Tune, field: char, value: &str, at: usize, meter_set: &mut bool) -> Result<(), Error> {
    match field {
        'X' => {}
        'T' => {
            if t.title.is_empty() {
                t.title = value.to_string();
            }
        }
        'M' => {
            if let Some(m) = parse_meter(value) {
                t.meter = m;
                t.meter_frac = Frac::new(i64::from(m.0), i64::from(m.1));
                if t.in_header {
                    *meter_set = true;
                    t.header_meter = m;
                } else {
                    push(t, Item::Meter(m.0, m.1));
                }
            }
        }
        'L' => {
            if let Some(f) = parse_frac(value) {
                t.unit = Some(f);
            }
        }
        'Q' => {
            let unit = t.unit.unwrap_or(Frac::new(1, 8));
            if let Some(q) = parse_tempo(value, unit) {
                if t.in_header {
                    t.tempo = Some(q);
                } else {
                    push(t, Item::Tempo(q));
                }
            }
        }
        'K' => {
            let (sig, sharps, minor) = parse_key(value).ok_or_else(|| bad(t.text, at, "a key ABC doesn't have"))?;
            if t.in_header {
                t.sig = sig;
                t.in_header = false;
                finish_header(t);
                for v in &mut t.voices {
                    v.sig = sig;
                }
                push(t, Item::Key(sharps, minor));
            } else {
                let v = &mut t.voices[t.cur];
                v.sig = sig;
                push(t, Item::Key(sharps, minor));
            }
            if value.contains("transpose=") {
                let n = value.split("transpose=").nth(1).and_then(|r| r.split_whitespace().next()).and_then(|n| n.parse().ok()).unwrap_or(0);
                t.voices[t.cur].transpose = n;
            }
        }
        'V' => voice_field(t, value),
        'P' => {
            if t.in_header {
                t.order = Some(value.to_string());
            } else if let Some(c) = value.chars().find(|c| c.is_ascii_alphabetic()) {
                t.part_count += 1;
                t.part_label = c;
            }
        }
        'I' => {
            if let Some(rest) = value.strip_prefix("MIDI") {
                directive(t, &format!("%%MIDI {}", rest.trim_start_matches(['=', ' '])));
            }
        }
        _ => {}
    }
    Ok(())
}

fn voice_field(t: &mut Tune, value: &str) {
    if t.voices.is_empty() {
        let sig = t.sig;
        t.voices.push(Voice::new("1", sig));
    }
    let id = value.split_whitespace().next().unwrap_or("1").to_string();
    let i = voice_index(t, &id);
    t.cur = i;
    for key in ["name=", "nm="] {
        if let Some(r) = value.split(key).nth(1) {
            let name = if let Some(q) = r.strip_prefix('"') { q.split('"').next().unwrap_or("") } else { r.split_whitespace().next().unwrap_or("") };
            t.voices[i].name = name.to_string();
        }
    }
    if let Some(n) = value.split("transpose=").nth(1).and_then(|r| r.split_whitespace().next()).and_then(|n| n.parse().ok()) {
        t.voices[i].transpose = n;
    }
    if value.contains("perc") || value.contains("clef=perc") {
        t.voices[i].drums = true;
    }
}

fn directive(t: &mut Tune, line: &str) {
    let words: Vec<&str> = line.trim_start_matches('%').split_whitespace().collect();
    if words.first() != Some(&"MIDI") {
        return;
    }
    if t.voices.is_empty() {
        let sig = t.sig;
        t.voices.push(Voice::new("1", sig));
    }
    match words.get(1).copied() {
        Some("program") => {
            // program [channel] n
            let nums: Vec<u8> = words[2..].iter().filter_map(|w| w.parse().ok()).collect();
            if let Some(&p) = nums.last() {
                let p = p.min(127);
                if t.voices[t.cur].items.iter().any(|i| matches!(i, Item::Ev { .. })) {
                    push(t, Item::Program(p));
                } else {
                    t.voices[t.cur].program = Some(p);
                }
            }
        }
        Some("channel") => {
            if words.get(2) == Some(&"10") {
                t.voices[t.cur].drums = true;
            }
        }
        Some("transpose") => {
            if let Some(n) = words.get(2).and_then(|w| w.parse().ok()) {
                t.voices[t.cur].transpose = n;
            }
        }
        _ => {}
    }
}

fn push(t: &mut Tune, item: Item) {
    if t.voices.is_empty() {
        let sig = t.sig;
        t.voices.push(Voice::new("1", sig));
    }
    let v = &mut t.voices[t.cur];
    if v.part_seen != t.part_count {
        v.part_seen = t.part_count;
        v.items.push(Item::Part(t.part_label));
    }
    if matches!(item, Item::Ev { .. }) {
        v.line.push(v.items.len());
    }
    v.items.push(item);
}

/// `w:` lyrics onto the last music line's notes.
fn lyrics(t: &mut Tune, value: &str) {
    let v = &mut t.voices[t.cur];
    let notes: Vec<usize> = v.last_line.iter().copied().filter(|&i| matches!(&v.items[i], Item::Ev { keys, .. } if !keys.is_empty())).collect();
    let mut k = 0;
    let mut word = String::new();
    let mut syllables: Vec<(String, bool)> = vec![];
    let flush = |word: &mut String, syllables: &mut Vec<(String, bool)>, hyphen: bool| {
        if !word.is_empty() {
            syllables.push((std::mem::take(word).replace('~', " "), hyphen));
        }
    };
    let mut chars = value.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            ' ' | '\t' => flush(&mut word, &mut syllables, false),
            '-' => {
                if word.is_empty() {
                    syllables.push((String::new(), false));
                } else {
                    flush(&mut word, &mut syllables, true);
                }
            }
            '_' | '*' => {
                flush(&mut word, &mut syllables, false);
                syllables.push((String::new(), false));
            }
            '|' => flush(&mut word, &mut syllables, false),
            '\\' if chars.peek() == Some(&'-') => {
                word.push('-');
                chars.next();
            }
            _ => word.push(c),
        }
    }
    flush(&mut word, &mut syllables, false);
    for (s, hyphen) in syllables {
        let Some(&i) = notes.get(k) else { break };
        k += 1;
        if s.is_empty() {
            continue;
        }
        if let Item::Ev { lyric, .. } = &mut v.items[i] {
            *lyric = Some(if hyphen { format!("{s}-") } else { s });
        }
    }
}

/// A note's length after its letter: `2`, `/`, `//`, `3/2`, `/4`.
fn length(s: &[u8], i: &mut usize) -> Frac {
    let mut n: i64 = 0;
    let mut has_n = false;
    while *i < s.len() && s[*i].is_ascii_digit() {
        n = n.saturating_mul(10).saturating_add(i64::from(s[*i] - b'0'));
        has_n = true;
        *i += 1;
    }
    let mut f = Frac::int(if has_n { n.max(1) } else { 1 });
    if has_n && n == 0 {
        f = Frac::int(1);
    }
    while *i < s.len() && s[*i] == b'/' {
        *i += 1;
        let mut d: i64 = 0;
        let mut has_d = false;
        while *i < s.len() && s[*i].is_ascii_digit() {
            d = d.saturating_mul(10).saturating_add(i64::from(s[*i] - b'0'));
            has_d = true;
            *i += 1;
        }
        f = f.div(Frac::int(if has_d { d.max(1) } else { 2 }));
    }
    f
}

struct Pitch {
    key: i32,
    letter: usize,
    octave: i32,
    explicit: Option<i32>,
}

/// A note: accidentals, a letter, octave marks. `None` if it isn't one.
fn pitch(s: &[u8], i: &mut usize) -> Option<Pitch> {
    let start = *i;
    let mut acc: Option<i32> = None;
    while *i < s.len() && matches!(s[*i], b'^' | b'_' | b'=') {
        let a = match s[*i] {
            b'^' => 1,
            b'_' => -1,
            _ => 0,
        };
        acc = Some(if a == 0 { 0 } else { acc.unwrap_or(0) + a });
        *i += 1;
    }
    // Microtones (^/ and the like) aren't played: a half sharp is none.
    while *i < s.len() && (s[*i] == b'/' || s[*i].is_ascii_digit()) && acc.is_some() && !s.get(*i).is_some_and(|c| c.is_ascii_alphabetic()) {
        *i += 1;
    }
    let c = *s.get(*i)? as char;
    let Some(letter) = letter_index(c).filter(|_| c.is_ascii_alphabetic() && "ABCDEFGabcdefg".contains(c)) else {
        *i = start;
        return None;
    };
    *i += 1;
    let mut octave = if c.is_ascii_lowercase() { 5 } else { 4 };
    while *i < s.len() {
        match s[*i] {
            b'\'' => octave += 1,
            b',' => octave -= 1,
            _ => break,
        }
        *i += 1;
    }
    let base = [0, 2, 4, 5, 7, 9, 11][letter] + 12 * (octave + 1);
    Some(Pitch { key: base, letter, octave, explicit: acc })
}

/// The key a pitch sounds in its voice, with the bar's accidentals.
fn resolve(v: &mut Voice, p: &Pitch) -> i32 {
    let acc = match p.explicit {
        Some(a) => {
            v.explicit.retain(|e| !(e.0 == p.letter && e.1 == p.octave));
            v.explicit.push((p.letter, p.octave, a));
            a
        }
        None => {
            if let Some(&(_, _, k)) = v.tied.iter().find(|x| x.0 == p.letter && x.1 == p.octave) {
                return k;
            }
            v.explicit.iter().find(|e| e.0 == p.letter && e.1 == p.octave).map(|e| e.2).unwrap_or(v.sig[p.letter])
        }
    };
    p.key + acc
}

const DECORATIONS: [&str; 40] = [
    "trill", "fermata", "accent", "emphasis", "staccato", "tenuto", "mordent", "pralltriller", "lowermordent", "uppermordent", "roll", "turn", "segno", "coda", "D.S.", "D.C.", "fine", "upbow", "downbow", "breath", "slide", "arpeggio", "open", "thumb", "snap",
    "invertedfermata", "wedge", "shortphrase", "mediumphrase", "longphrase", "crescendo(", "crescendo)", "diminuendo(", "diminuendo)", "<(", "<)", ">(", ">)", "0", "+",
];

/// One line of music for the current voice.
fn music(t: &mut Tune, body: &str, at: usize) -> Result<(), Error> {
    let s = body.as_bytes();
    let mut i = 0;
    let unit = t.unit.unwrap_or(Frac::new(1, 8));
    // A tuplet: its time factor and notes left; a broken rhythm's second
    // note's factor.
    let mut tuplet: Option<(Frac, u32)> = None;
    let mut broken_next: Option<Frac> = None;
    while i < s.len() {
        let c = s[i];
        let here = at + i;
        match c {
            b' ' | b'\t' | b'`' | b'y' | b')' => i += 1,
            b'%' => break,
            b'"' => {
                let end = body[i + 1..].find('"').map(|e| i + 1 + e).ok_or_else(|| bad(t.text, here, "a chord symbol in quotes that doesn't end"))?;
                let sym = &body[i + 1..end];
                if !sym.starts_with(['^', '_', '<', '>', '@']) && !sym.is_empty() {
                    t.chord = Some(sym.to_string());
                }
                i = end + 1;
            }
            b'!' | b'+' => {
                let close = body[i + 1..].find(c as char).map(|e| i + 1 + e);
                match close {
                    Some(end) if c == b'!' || DECORATIONS.contains(&&body[i + 1..end]) || DYNAMICS.iter().any(|d| d.0 == &body[i + 1..end]) || body[i + 1..end].chars().all(|c| c.is_ascii_alphabetic() && !"ABCDEFGabcdefg".contains(c)) => {
                        let name = &body[i + 1..end];
                        if let Some(&(_, v)) = DYNAMICS.iter().find(|d| d.0 == name) {
                            t.voices[t.cur].vel = v;
                        }
                        i = end + 1;
                    }
                    // ABC 1.6's +CEG+ chord.
                    Some(end) if c == b'+' => {
                        let inner = format!("[{}]", &body[i + 1..end]);
                        let mut j = 0;
                        event(t, inner.as_bytes(), &mut j, here, unit, &mut tuplet, &mut broken_next, true)?;
                        i = end + 1;
                    }
                    // An old ! line break.
                    _ => i += 1,
                }
            }
            // Decoration symbols: `.` and `~`, and every letter H to W and
            // h to w (U: may give any of them a meaning; none sounds).
            b'.' | b'~' | b'H'..=b'W' | b'h'..=b'w' => i += 1,
            // What a careless transcription leaves: a length with no note,
            // an octave mark, a stray brace.
            b'0'..=b'9' | b'/' | b',' | b'\'' | b'}' | b'&' | b'$' | b'*' | b';' | b'#' | b'@' | b'?' => i += 1,
            b'(' => {
                if s.get(i + 1).is_some_and(|c| c.is_ascii_digit()) {
                    i += 1;
                    let num = |i: &mut usize| -> Option<i64> {
                        let st = *i;
                        while *i < s.len() && s[*i].is_ascii_digit() {
                            *i += 1;
                        }
                        body[st..*i].parse().ok()
                    };
                    let p = num(&mut i).unwrap_or(3);
                    let (mut q, mut r) = (None, None);
                    if s.get(i) == Some(&b':') {
                        i += 1;
                        q = num(&mut i);
                        if s.get(i) == Some(&b':') {
                            i += 1;
                            r = num(&mut i);
                        }
                    }
                    // 2 to 9, as the standard has them; past 9 only with its
                    // own q, which says what it means.
                    let p = if q.is_some() { p.clamp(2, 64) } else { p.clamp(2, 9) };
                    let compound = t.meter.0 % 3 == 0 && t.meter.0 > 3;
                    let q = q.unwrap_or(match p {
                        2 | 4 | 8 => 3,
                        3 | 6 => 2,
                        _ => {
                            if compound {
                                3
                            } else {
                                2
                            }
                        }
                    });
                    tuplet = Some((Frac::new(q.max(1), p), r.unwrap_or(p) as u32));
                } else {
                    i += 1; // a slur
                }
            }
            b'{' => {
                // Grace notes take no time.
                let end = body[i..].find('}').map(|e| i + e).ok_or_else(|| bad(t.text, here, "grace notes in { } that don't end"))?;
                i = end + 1;
            }
            b'-' => i += 1,
            b'>' | b'<' => {
                // A broken rhythm: the last event lengthened or shortened.
                let mut n = 0;
                while i < s.len() && s[i] == c {
                    n += 1;
                    i += 1;
                }
                let n = n.min(3);
                let short = Frac::new(1, 1 << n);
                let long = Frac::int(2).sub(short);
                let (first, second) = if c == b'>' { (long, short) } else { (short, long) };
                let v = &mut t.voices[t.cur];
                if let Some(Item::Ev { keys: _, len, .. }) = v.items.iter_mut().rev().find(|x| matches!(x, Item::Ev { .. })) {
                    *len = len.mul(first);
                }
                broken_next = Some(second);
            }
            b'|' | b':' | b'[' if is_bar(s, i) => bar(t, s, &mut i),
            b'[' if s.get(i + 2) == Some(&b':') && s.get(i + 1).is_some_and(|c| c.is_ascii_alphabetic()) => {
                let end = body[i..].find(']').map(|e| i + e).ok_or_else(|| bad(t.text, here, "an inline field in [ ] that doesn't end"))?;
                let field = s[i + 1] as char;
                let value = &body[i + 3..end];
                let mut dummy = false;
                if field == 'V' {
                    voice_field(t, value);
                } else {
                    field_line(t, field, value, here, &mut dummy)?;
                }
                i = end + 1;
            }
            b'\\' => i += 1,
            _ => {
                event(t, s, &mut i, here, unit, &mut tuplet, &mut broken_next, false)?;
            }
        }
    }
    Ok(())
}

/// Whether a bar line starts here.
fn is_bar(s: &[u8], i: usize) -> bool {
    match s[i] {
        b'|' => true,
        // A lone colon too: a repeat sign split from its bar line.
        b':' => true,
        b'[' => matches!(s.get(i + 1), Some(b'|')) || s.get(i + 1).is_some_and(|c| c.is_ascii_digit()),
        _ => false,
    }
}

fn bar(t: &mut Tune, s: &[u8], i: &mut usize) {
    let start = *i;
    while *i < s.len() && matches!(s[*i], b'|' | b':' | b'[' | b']') {
        // `[` only at the start (`[|`, `[1`), `]` only after `|`.
        if s[*i] == b'[' && *i != start {
            break;
        }
        if s[*i] == b']' && *i == start {
            break;
        }
        *i += 1;
    }
    let token: String = s[start..*i].iter().map(|&c| c as char).collect();
    let colons_before = token.starts_with(':');
    let colons_after = token.ends_with(':') && token.len() > 1 || token == "::";
    // A lone `:` opens a repeat when music follows it, else closes one.
    let lone = token == ":";
    let colons_after = colons_after || lone && s.get(*i).is_some_and(|c| !c.is_ascii_whitespace());
    let colons_before = colons_before && !(lone && colons_after);
    let kind = match (colons_before, colons_after) {
        (true, true) => Bar::Both,
        (true, false) => Bar::End,
        (false, true) => Bar::Start,
        _ => Bar::Plain,
    };
    {
        let v = &mut t.voices[t.cur];
        v.explicit.clear();
    }
    if kind != Bar::Plain || token != "[" {
        push(t, Item::Bar(kind));
    }
    // An ending's numbers: `[1`, `|2`, `:|2`, `[1,3`, `[1-3`.
    let mut j = *i;
    while j < s.len() && s[j] == b' ' && token.ends_with('[') {
        j += 1;
    }
    if j < s.len() && s[j].is_ascii_digit() {
        let st = j;
        while j < s.len() && (s[j].is_ascii_digit() || s[j] == b',' || s[j] == b'-') {
            j += 1;
        }
        let spec: String = s[st..j].iter().map(|&c| c as char).collect();
        let mut ns = vec![];
        for p in spec.split(',') {
            match p.split_once('-') {
                Some((a, b)) => {
                    let (a, b): (u32, u32) = (a.parse().unwrap_or(1), b.parse().unwrap_or(1));
                    ns.extend(a..=b.min(a + 16));
                }
                None => ns.extend(p.parse::<u32>().ok()),
            }
        }
        *i = j;
        push(t, Item::Ending(ns));
    }
}

/// A note, chord or rest, with its length, at `i`.
#[allow(clippy::too_many_arguments)]
fn event(t: &mut Tune, s: &[u8], i: &mut usize, here: usize, unit: Frac, tuplet: &mut Option<(Frac, u32)>, broken_next: &mut Option<Frac>, _plus: bool) -> Result<(), Error> {
    let c = s[*i];
    let mut keys: Vec<(i32, bool)> = vec![];
    let len;
    let mut rest = false;
    let mut multi_bar = None;
    match c {
        b'z' | b'x' => {
            *i += 1;
            len = unit.mul(length(s, i));
            rest = true;
        }
        b'Z' | b'X' => {
            *i += 1;
            let st = *i;
            while *i < s.len() && s[*i].is_ascii_digit() {
                *i += 1;
            }
            let n: i64 = std::str::from_utf8(&s[st..*i]).ok().and_then(|n| n.parse().ok()).unwrap_or(1);
            multi_bar = Some(n.max(1));
            len = t.meter_frac.mul(Frac::int(n.max(1)));
            rest = true;
        }
        b'[' => {
            *i += 1;
            let mut first: Option<Frac> = None;
            let mut tie_all = vec![];
            while *i < s.len() && s[*i] != b']' {
                // A rest in a chord holds nothing.
                if matches!(s[*i], b'z' | b'x') {
                    *i += 1;
                    let _ = length(s, i);
                    continue;
                }
                if matches!(s[*i], b' ' | b'.' | b'~' | b'!' | b'"' | b'(' | b')') {
                    if s[*i] == b'!' || s[*i] == b'"' {
                        let q = s[*i];
                        *i += 1;
                        while *i < s.len() && s[*i] != q {
                            *i += 1;
                        }
                    }
                    *i += 1;
                    continue;
                }
                let Some(p) = pitch(s, i) else { return Err(bad(t.text, here, "a chord in [ ] holds only notes")) };
                let l = length(s, i);
                let tie = s.get(*i) == Some(&b'-');
                if tie {
                    *i += 1;
                }
                let v = &mut t.voices[t.cur];
                let k = resolve(v, &p);
                tie_all.push((p.letter, p.octave, k, tie));
                keys.push((k, tie));
                first.get_or_insert(l);
            }
            *i += 1; // ]
            let outer = length(s, i);
            len = unit.mul(first.unwrap_or(Frac::int(1))).mul(outer);
            if s.get(*i) == Some(&b'-') {
                *i += 1;
                for k in &mut keys {
                    k.1 = true;
                }
                for x in &mut tie_all {
                    x.3 = true;
                }
            }
            let v = &mut t.voices[t.cur];
            v.tied = tie_all.iter().filter(|x| x.3).map(|x| (x.0, x.1, x.2)).collect();
        }
        _ => {
            let Some(p) = pitch(s, i) else {
                return Err(bad(t.text, here, "something ABC doesn't have"));
            };
            len = unit.mul(length(s, i));
            let tie = s.get(*i) == Some(&b'-');
            if tie {
                *i += 1;
            }
            let v = &mut t.voices[t.cur];
            let k = resolve(v, &p);
            v.tied = if tie { vec![(p.letter, p.octave, k)] } else { vec![] };
            keys.push((k, tie));
        }
    }
    if rest {
        t.voices[t.cur].tied.clear();
    }
    let mut len = len;
    if let Some(f) = broken_next.take() {
        len = len.mul(f);
    }
    if let Some((f, left)) = tuplet.as_mut() {
        if multi_bar.is_none() {
            len = len.mul(*f);
            *left -= 1;
            if *left == 0 {
                *tuplet = None;
            }
        }
    }
    let transpose = t.voices[t.cur].transpose;
    let keys = keys.into_iter().map(|(k, tie)| (k + transpose, tie)).collect();
    let vel = t.voices[t.cur].vel;
    let chord = t.chord.take();
    push(t, Item::Ev { keys, len, vel, chord, lyric: None });
    Ok(())
}

/// Plays a voice's items, repeats and endings played out.
fn play_items(items: &[Item], b: &mut Builder, part: usize, now: &mut Frac, first: bool) {
    // Tied notes: key, index in the part's notes.
    let mut open: Vec<(i32, usize)> = vec![];
    let (mut i, mut pass, mut start) = (0usize, 1u32, 0usize);
    let mut steps = 0usize;
    while i < items.len() {
        steps += 1;
        if steps > 1 << 20 {
            break;
        }
        match &items[i] {
            Item::Ev { keys, len, vel, chord, lyric } => {
                if first {
                    if let Some(c) = chord {
                        b.chord(*now, c);
                    }
                }
                if let Some(l) = lyric {
                    b.lyric(part, *now, l);
                }
                let mut next_open = vec![];
                for &(k, tie) in keys {
                    if let Some(pos) = open.iter().position(|o| o.0 == k) {
                        let (_, idx) = open.remove(pos);
                        let note = &mut b.parts[part].notes[idx];
                        note.1 = now.add(*len).sub(note.0);
                        if tie {
                            next_open.push((k, idx));
                        }
                        continue;
                    }
                    let before = b.parts[part].notes.len();
                    b.note(part, *now, *len, k, *vel);
                    if tie && b.parts[part].notes.len() > before {
                        next_open.push((k, before));
                    }
                }
                open = next_open;
                *now = now.add(*len);
            }
            Item::Bar(kind) => match kind {
                Bar::Start => {
                    start = i + 1;
                    pass = 1;
                }
                Bar::End | Bar::Both => {
                    if pass == 1 {
                        pass = 2;
                        i = start;
                        open.clear();
                        continue;
                    }
                    pass = 1;
                    start = i + 1;
                }
                Bar::Plain => {}
            },
            Item::Ending(ns) => {
                if !ns.contains(&pass) {
                    // Skip to the next ending (or to the end).
                    let mut j = i + 1;
                    while j < items.len() && !matches!(items[j], Item::Ending(_)) && !matches!(items[j], Item::Bar(Bar::Start)) {
                        j += 1;
                    }
                    i = j;
                    continue;
                }
                if pass > 1 && ns.iter().max() == Some(&pass) {
                    // The last ending: the repeat is over.
                    pass = 1;
                    start = i + 1;
                }
            }
            Item::Tempo(q) => {
                if first {
                    b.tempo(*now, *q);
                }
            }
            Item::Meter(n, d) => {
                if first {
                    b.meter(*now, *n, *d);
                }
            }
            Item::Key(s, m) => {
                if first {
                    b.key(*now, *s, *m);
                }
            }
            Item::Program(p) => b.program_at(part, *now, *p),
            Item::Part(_) => {}
        }
        i += 1;
    }
}

fn perform(t: &mut Tune, b: &mut Builder, ensemble: bool) {
    if let Some(q) = t.tempo {
        b.tempo(Frac::ZERO, q);
    } else if b.tempos.is_empty() {
        b.tempo(Frac::ZERO, 120.0);
    }
    b.meter(Frac::ZERO, t.header_meter.0, t.header_meter.1);
    let order = t.order.as_deref().map(part_order).filter(|o| !o.is_empty());
    let first_part = b.parts.len();
    let mut end = Frac::ZERO;
    for (n, v) in t.voices.iter().enumerate() {
        if !v.items.iter().any(|i| matches!(i, Item::Ev { keys, .. } if !keys.is_empty())) {
            continue;
        }
        let name = if !v.name.is_empty() {
            v.name.clone()
        } else if ensemble {
            t.title.clone()
        } else if t.voices.len() > 1 {
            format!("Voice {}", v.id)
        } else if t.title.is_empty() {
            "Tune".into()
        } else {
            t.title.clone()
        };
        let program = v.program.or_else(|| super::program(&v.name)).or_else(|| ensemble.then(|| lotro_program(&t.title)).flatten()).unwrap_or(0);
        let part = b.part(&name, program, v.drums);
        let mut now = Frac::ZERO;
        let first = n == 0 && b.parts.len() == first_part + 1;
        match &order {
            Some(order) => {
                // Sections by label; what's before the first is played first.
                let mut sections: Vec<(char, Vec<Item>)> = vec![(' ', vec![])];
                for item in &v.items {
                    match item {
                        Item::Part(c) => sections.push((*c, vec![])),
                        other => sections.last_mut().map(|s| s.1.push(other.clone())).unwrap_or(()),
                    }
                }
                play_items(&sections[0].1, b, part, &mut now, first);
                for label in order {
                    if let Some(sec) = sections.iter().find(|s| s.0 == *label) {
                        play_items(&sec.1, b, part, &mut now, first);
                    }
                }
            }
            None => play_items(&v.items, b, part, &mut now, first),
        }
        end = end.max(now);
    }
    b.end_at(end);
}

/// A Lord of the Rings Online part's instrument, from its title.
fn lotro_program(title: &str) -> Option<u8> {
    let l = title.to_ascii_lowercase();
    for (word, p) in [("theorbo", 32u8), ("flute", 73), ("lute", 24), ("harp", 46), ("clarinet", 71), ("horn", 60), ("bagpipe", 109), ("pibgorn", 68), ("cowbell", 113), ("drum", 116), ("fiddle", 110), ("bassoon", 70), ("viol", 41)] {
        if l.contains(word) {
            return Some(p);
        }
    }
    None
}

/// A key signature's name for `K:`.
fn key_name(sharps: i8, minor: bool) -> &'static str {
    const MAJOR: [&str; 15] = ["Cb", "Gb", "Db", "Ab", "Eb", "Bb", "F", "C", "G", "D", "A", "E", "B", "F#", "C#"];
    const MINOR: [&str; 15] = ["Abm", "Ebm", "Bbm", "Fm", "Cm", "Gm", "Dm", "Am", "Em", "Bm", "F#m", "C#m", "G#m", "D#m", "A#m"];
    let i = (i32::from(sharps.clamp(-7, 7)) + 7) as usize;
    if minor {
        MINOR[i]
    } else {
        MAJOR[i]
    }
}

/// A length in units of `L:1/8`: `2`, `3/2`, `/2`, nothing for one.
fn abc_length(len: Frac) -> String {
    let r = len.mul(Frac::int(8));
    match (r.num(), r.den()) {
        (1, 1) => String::new(),
        (n, 1) => n.to_string(),
        (1, d) => format!("/{d}"),
        (n, d) => format!("{n}/{d}"),
    }
}

/// A score as an ABC tune, titled.
pub fn write(score: &Score, title: &str) -> String {
    use super::notation::{notate, spell};
    let n = notate(score);
    let first = &n.bars[0];
    let mut o = String::new();
    o.push_str("X:1\n");
    let title = super::notation::one_line(title);
    o.push_str(&format!("T:{}\n", if title.is_empty() { "Untitled" } else { &title }));
    o.push_str(&format!("M:{}/{}\nL:1/8\n", first.beats, first.unit));
    o.push_str(&format!("Q:1/4={}\n", first.tempo.unwrap_or(score.bpm).round()));
    // Voices: a part's own, one after another.
    let mut voices: Vec<(usize, usize)> = vec![];
    for (pi, p) in n.parts.iter().enumerate() {
        for v in 0..p.voices.len() {
            voices.push((pi, v));
        }
    }
    o.push_str(&format!("K:{}\n", key_name(first.sharps, first.minor)));
    // Each voice named where it starts (music21 takes voices declared in
    // the header for parts of their own).
    for (i, &(pi, v)) in voices.iter().enumerate() {
        let p = &n.parts[pi];
        let name = if p.name.is_empty() { super::GM_NAMES[usize::from(p.program & 127)].to_string() } else { p.name.clone() };
        let name = if p.voices.len() > 1 { format!("{name} {}", v + 1) } else { name };
        o.push_str(&format!("V:{} name=\"{}\"{}\n", i + 1, name.replace('"', "'"), if p.middle() < 57 && !p.drums { " clef=bass" } else { "" }));
        o.push_str(&format!("%%MIDI program {}\n", p.program));
        if p.drums {
            o.push_str("%%MIDI channel 10\n");
        }
        let mut vel = 80u8;
        let mut line = String::new();
        for (bi, bar) in n.bars.iter().enumerate() {
            if bi > 0 {
                let prev = &n.bars[bi - 1];
                if (prev.sharps, prev.minor) != (bar.sharps, bar.minor) {
                    line.push_str(&format!("[K:{}] ", key_name(bar.sharps, bar.minor)));
                }
                if (prev.beats, prev.unit) != (bar.beats, bar.unit) {
                    line.push_str(&format!("[M:{}/{}] ", bar.beats, bar.unit));
                }
                if let Some(t) = bar.tempo.filter(|_| i == 0) {
                    line.push_str(&format!("[Q:1/4={}] ", t.round()));
                }
            }
            let sig = signature_accidentals(bar.sharps);
            // Letters given an accidental this bar, in any octave, and
            // what each letter and octave is now.
            let mut touched = [false; 7];
            let mut now: Vec<((u8, i8), i8)> = vec![];
            let end = bar.start.add(bar.len);
            for it in &p.voices[v][bi] {
                if let Some(t) = it.tuplet.filter(|t| t.first) {
                    line.push_str(&format!("({}:{}:{}", t.actual, t.normal, t.count));
                }
                if i == 0 {
                    for c in n.chords.iter().filter(|c| c.0 >= bar.start && c.0 < end && c.0.sub(bar.start) == it.at) {
                        line.push_str(&format!("\"{}\"", c.1.replace('"', "'")));
                    }
                }
                if !it.rest() {
                    let level = DYNAMICS.iter().filter(|d| d.0.len() <= 3 && d.0 != "pppp").min_by_key(|d| (i32::from(d.1) - i32::from(it.velocity)).abs()).copied();
                    if let Some((mark, v)) = level {
                        if v != vel {
                            line.push_str(&format!("!{mark}!"));
                            vel = v;
                        }
                    }
                }
                let mut note = String::new();
                if it.rest() {
                    note.push('z');
                } else {
                    if it.keys.len() > 1 {
                        note.push('[');
                    }
                    for &k in &it.keys {
                        let sp = spell(k, bar.sharps);
                        let step = usize::from(sp.step);
                        let current = now.iter().find(|x| x.0 == (sp.step, sp.octave)).map(|x| x.1).unwrap_or(sig[step] as i8);
                        // Every note off the key signature has its accidental
                        // (readers differ on carrying one through the bar), and a
                        // natural follows one in the bar.
                        if sp.alter != current || sp.alter != sig[step] as i8 || touched[step] && it.tie_from {
                            note.push_str(match sp.alter {
                                2 => "^^",
                                1 => "^",
                                -1 => "_",
                                -2 => "__",
                                _ => "=",
                            });
                            touched[step] = true;
                            now.retain(|x| x.0 != (sp.step, sp.octave));
                            now.push(((sp.step, sp.octave), sp.alter));
                        }
                        let letter = sp.letter();
                        if sp.octave >= 5 {
                            note.push(letter.to_ascii_lowercase());
                            for _ in 5..sp.octave {
                                note.push('\'');
                            }
                        } else {
                            note.push(letter);
                            for _ in sp.octave..4 {
                                note.push(',');
                            }
                        }
                    }
                    if it.keys.len() > 1 {
                        note.push(']');
                    }
                }
                let written = if it.dots > 0 { it.base.scale(3, 2) } else { it.base };
                note.push_str(&abc_length(written));
                if it.tie_to {
                    note.push('-');
                }
                line.push_str(&note);
                line.push(' ');
            }
            line.push_str(if bi + 1 == n.bars.len() { "|]" } else { "| " });
            if bi % 4 == 3 || bi + 1 == n.bars.len() {
                o.push_str(line.trim_end());
                o.push('\n');
                line.clear();
            }
        }
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(score: &Score, part: usize) -> Vec<(u32, u32, u8)> {
        score.parts[part].notes.iter().map(|n| (n.at, n.length, n.key)).collect()
    }

    #[test]
    fn a_meter_changed_in_the_body_starts_where_it_is_and_not_at_the_start() {
        let (score, _) = read("X:1\nM:4/4\nL:1/4\nK:C\nCDEF|[M:3/4]GAB|]\n").expect("reads");
        assert_eq!((score.beats, score.unit), (4, 4));
        assert!(score.kept.iter().any(|k| k.at == 4 * u32::from(score.ppq) && k.bytes[1] == 0x58 && k.bytes[3] == 3));
    }

    #[test]
    fn a_reel() {
        let tune = "X:1\nT:Test Reel\nM:4/4\nL:1/8\nQ:1/4=100\nK:G\nGABc d2e2|f>g e<f (3gag z2|]\n";
        let (score, title) = read(tune).unwrap();
        assert_eq!(title, "Test Reel");
        assert_eq!(score.bpm, 100.0);
        let k = keys(&score, 0);
        assert_eq!(k[0], (0, 240, 67));
        assert_eq!(k[3], (720, 240, 72));
        assert_eq!(k[4], (960, 480, 74));
        // f is sharp in G; f>g: a dotted eighth and a sixteenth.
        assert_eq!(k[6], (1920, 360, 78));
        assert_eq!(k[7], (2280, 120, 79));
        assert_eq!(k[8], (2400, 120, 76));
        assert_eq!(k[9], (2520, 360, 78));
        // A triplet: three in the time of two eighths.
        assert_eq!(k[10], (2880, 160, 79));
        assert_eq!(k[12], (3200, 160, 79));
    }

    #[test]
    fn accidentals_last_the_bar() {
        let (score, _) = read("X:1\nK:C\nL:1/4\n^F F =F F | F ^c'' c''\n").unwrap();
        let k: Vec<u8> = keys(&score, 0).iter().map(|x| x.2).collect();
        assert_eq!(k, vec![66, 66, 65, 65, 65, 97, 97]);
        let (score, _) = read("X:1\nL:1/4\nK:Bb\nB c ^c- | c C, C,,\n").unwrap();
        let k: Vec<u8> = keys(&score, 0).iter().map(|x| x.2).collect();
        assert_eq!(k, vec![70, 72, 73, 48, 36]);
        assert_eq!(score.parts[0].notes[2].length, 960, "tied over the bar");
    }

    #[test]
    fn repeats_and_endings() {
        let (score, _) = read("X:1\nL:1/4\nK:C\n|:C D|1 E:|2 F|]G\n").unwrap();
        let k: Vec<u8> = keys(&score, 0).iter().map(|x| x.2).collect();
        assert_eq!(k, vec![60, 62, 64, 60, 62, 65, 67]);
        let (score, _) = read("X:1\nL:1/4\nK:C\nC D :| E F ::G:|\n").unwrap();
        let k: Vec<u8> = keys(&score, 0).iter().map(|x| x.2).collect();
        assert_eq!(k, vec![60, 62, 60, 62, 64, 65, 64, 65, 67, 67]);
    }

    #[test]
    fn chords_lengths_and_rests() {
        let (score, _) = read("X:1\nM:3/4\nK:C\n[CEG]2 z/ c/ \"Am\"A3/2 B//|Z2|c\n").unwrap();
        let k = keys(&score, 0);
        assert_eq!(&k[..3], &[(0, 480, 60), (0, 480, 64), (0, 480, 67)]);
        assert_eq!(k[3], (600, 120, 72));
        assert_eq!(k[4], (720, 360, 69));
        assert_eq!(k[5], (1080, 60, 71));
        // Z2: two bars of 3/4.
        assert_eq!(k[6].0, 1140 + 2 * 1440);
        assert!(score.kept.iter().any(|x| x.bytes == crate::text::meta(0x01, b"Am")));
    }

    #[test]
    fn voices_parts_and_lyrics() {
        let tune = "X:1\nT:Two\nP:ABA\nL:1/4\nM:2/4\n%%MIDI program 40\nK:D\nP:A\nV:1\nd e\nw:Hel-lo\nV:2\n%%MIDI program 42\nD E\nP:B\nV:1\nf2\nV:2\nF2\n";
        let (score, _) = read(tune).unwrap();
        assert_eq!(score.parts.len(), 2);
        assert_eq!(score.parts[0].program, 40);
        assert_eq!(score.parts[1].program, 42);
        let k: Vec<(u32, u8)> = score.parts[0].notes.iter().map(|n| (n.at, n.key)).collect();
        assert_eq!(k, vec![(0, 74), (480, 76), (960, 78), (1920, 74), (2400, 76)]);
        assert_eq!(score.parts[1].notes[2].key, 66);
        assert!(score.parts[0].kept.iter().any(|x| x.bytes == crate::text::meta(0x05, b"Hel-")));
        assert!(score.parts[0].kept.iter().any(|x| x.bytes == crate::text::meta(0x05, b"lo")));
    }

    #[test]
    fn fields() {
        assert_eq!(parse_meter("C|"), Some((2, 2)));
        assert_eq!(parse_meter("2+3/8"), Some((5, 8)));
        assert_eq!(parse_tempo("\"Allegro\" 3/8=40", Frac::new(1, 8)), Some(60.0));
        assert_eq!(parse_tempo("120", Frac::new(1, 8)), Some(60.0));
        assert_eq!(parse_key("Ador").unwrap().1, 1);
        assert_eq!(parse_key("F#m").unwrap().1, 3);
        assert_eq!(parse_key("D exp ^f _b").unwrap().0, [0, 0, 0, 1, 0, 0, -1]);
        assert_eq!(part_order("A2(BC)2"), vec!['A', 'A', 'B', 'C', 'B', 'C']);
    }

    #[test]
    fn many_tunes() {
        let file = "X:1\nT:One\nK:C\nC\n\nX:2\nT:Two\nK:C\nD\n";
        let all = tunes(file);
        assert_eq!(all.len(), 2);
        assert_eq!(all[1].as_ref().unwrap().1, "Two");
        let lotro = "%%lotro\nX:1\nT:Song (Lute)\nK:C\nC\n\nX:2\nT:Song (Flute)\nK:C\nE\n";
        let (score, title) = read(lotro).unwrap();
        assert_eq!(title, "Song");
        assert_eq!(score.parts.len(), 2);
        assert_eq!((score.parts[0].program, score.parts[1].program), (24, 73));
    }

    #[test]
    fn errors() {
        assert!(matches!(read("X:1\nK:C\nC D Y\n"), Err(Error::BadText { line: 3, column: 5, .. })));
        assert!(read("no tune here").is_err());
    }

}

