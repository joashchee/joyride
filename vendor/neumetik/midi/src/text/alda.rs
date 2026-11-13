// Copyright 2026 ansiapps. Neumetik MIDI, MIT licensed: see LICENSE.

//! **Alda**, from its documentation (alda.io): parts (`piano:`,
//! `violin "v1":`, `violin/viola:`), notes (`c+4.`, `b-8~16`, `e2s`,
//! `d500ms`), rests, octaves (`o3`, `>`, `<`), chords (`c/e/g`), ties and
//! slurs (`~`), voices (`V1:`, `V0:`), markers (`%a`, `@a`), variables
//! (`riff = c d e`), repeats (`[c d]*3`, `c*2`) with endings (`'1`, `'2-3`),
//! cram (`{c d e}2`), and the attributes `tempo`, `quant`, `vol`,
//! `track-vol`, `pan`, `key-sig`, `transpose`, `octave` and the dynamics
//! (`(mf)`), each with or without `!` (every part). Comments are `#`.
//! Read and written.

use super::{bad, key_signature, signature_accidentals, Builder, Frac};
use crate::score::{Score, DRUMS};
use crate::Error;

/// Whether a text looks like Alda: a part named before a colon.
pub fn looks_like(text: &str) -> bool {
    text.lines().map(|l| l.split('#').next().unwrap_or("").trim()).filter(|l| !l.is_empty()).take(8).any(|l| {
        let head = l.split(':').next().unwrap_or("");
        l.contains(':') && !head.is_empty() && head.split('/').all(|p| {
            let name = p.split_whitespace().next().unwrap_or("");
            name.len() > 1 && name.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        }) && super::program(head.split(['/', '"']).next().unwrap_or("").trim()).is_some()
            || l.starts_with("percussion:") || l.starts_with("midi-")
    })
}

#[derive(Debug, Clone)]
enum Item {
    /// Notes (key, its length), when the next event starts, velocity,
    /// quantization, slurred.
    Note { keys: Vec<(i32, Frac)>, adv: Frac, vel: u8, quant: Frac, slur: bool },
    Rest(Frac),
    Tempo(f64),
    Control(u8, u8),
    Marker(String),
    Jump(String),
    Voice(u32),
}

impl Item {
    fn scale(&mut self, f: Frac) {
        match self {
            Item::Note { keys, adv, .. } => {
                *adv = adv.mul(f);
                for k in keys {
                    k.1 = k.1.mul(f);
                }
            }
            Item::Rest(l) => *l = l.mul(f),
            _ => {}
        }
    }
    fn advance(&self) -> Frac {
        match self {
            Item::Note { adv, .. } => *adv,
            Item::Rest(l) => *l,
            _ => Frac::ZERO,
        }
    }
}

/// What a part's notes are read with.
#[derive(Debug, Clone)]
struct State {
    octave: i32,
    length: Frac,
    quant: Frac,
    vol: f64,
    tempo: f64,
    sig: [i32; 7],
    transpose: i32,
}

impl Default for State {
    fn default() -> State {
        State { octave: 4, length: Frac::new(1, 4), quant: Frac::new(9, 10), vol: 54.21, tempo: 120.0, sig: [0; 7], transpose: 0 }
    }
}

struct Reader<'a> {
    text: &'a str,
    vars: Vec<(String, (usize, String))>,
    depth: usize,
    /// Attributes for every part, by `!`.
    global: Vec<Item>,
}

/// Alda's dynamics, as volumes (percent): fourteen even steps from
/// `pppppp` (one velocity step) to `ffffff`, `mf` the default.
const DYNAMICS: [(&str, f64); 14] = [
    ("pppppp", 0.787),
    ("ppppp", 8.419),
    ("pppp", 16.051),
    ("ppp", 23.683),
    ("pp", 31.314),
    ("p", 38.946),
    ("mp", 46.578),
    ("mf", 54.21),
    ("f", 61.841),
    ("ff", 69.473),
    ("fff", 77.105),
    ("ffff", 84.737),
    ("fffff", 92.368),
    ("ffffff", 100.0),
];

/// The end of the bracket opened at `i`.
fn matching(s: &[u8], i: usize, open: u8, close: u8) -> Option<usize> {
    let mut depth = 0;
    let mut quoted = false;
    for (k, &c) in s.iter().enumerate().skip(i) {
        if c == b'"' {
            quoted = !quoted;
        }
        if quoted {
            continue;
        }
        if c == open {
            depth += 1;
        } else if c == close {
            depth -= 1;
            if depth == 0 {
                return Some(k);
            }
        }
    }
    None
}

impl<'a> Reader<'a> {
    fn err(&self, at: usize, why: &'static str) -> Error {
        bad(self.text, at, why)
    }

    /// Events in `s` (at `base` in the text) into items.
    fn events(&mut self, s: &str, base: usize, st: &mut State, out: &mut Vec<Item>) -> Result<(), Error> {
        self.depth += 1;
        if self.depth > 64 {
            return Err(self.err(base, "variables used inside themselves"));
        }
        let b = s.as_bytes();
        let mut i = 0;
        while i < b.len() {
            let c = b[i];
            let at = base + i;
            // A variable's name comes before anything it could be read as.
            if c.is_ascii_alphabetic() {
                let mut j = i;
                while j < b.len() && (b[j].is_ascii_alphanumeric() || matches!(b[j], b'-' | b'_')) {
                    j += 1;
                }
                let name = &s[i..j];
                if j - i > 1 {
                    if let Some((vat, value)) = self.vars.iter().rev().find(|v| v.0 == name).map(|v| v.1.clone()) {
                        let (times, j) = repeat(b, j);
                        for _ in 0..times {
                            self.events(&value, vat, st, out)?;
                        }
                        i = j;
                        continue;
                    }
                }
            }
            match c {
                b' ' | b'\t' | b'\n' | b'\r' | b'|' => i += 1,
                b'#' => {
                    i = s[i..].find('\n').map(|e| i + e).unwrap_or(b.len());
                }
                b'o' if b.get(i + 1).is_some_and(|c| c.is_ascii_digit() || *c == b'-') => {
                    let (n, j) = int(b, i + 1);
                    st.octave = n.unwrap_or(4) as i32;
                    i = j;
                }
                b'>' => {
                    st.octave += 1;
                    i += 1;
                }
                b'<' => {
                    st.octave -= 1;
                    i += 1;
                }
                b'(' => {
                    let end = matching(b, i, b'(', b')').ok_or_else(|| self.err(at, "an attribute in ( ) that isn't closed"))?;
                    self.attribute(&s[i + 1..end], at, st, out);
                    i = end + 1;
                }
                b'%' | b'@' => {
                    let st2 = i + 1;
                    let mut j = st2;
                    while j < b.len() && (b[j].is_ascii_alphanumeric() || matches!(b[j], b'-' | b'_')) {
                        j += 1;
                    }
                    let name = s[st2..j].to_string();
                    out.push(if c == b'%' { Item::Marker(name) } else { Item::Jump(name) });
                    i = j;
                }
                b'V' if b.get(i + 1).is_some_and(|c| c.is_ascii_digit()) => {
                    let (n, j) = int(b, i + 1);
                    if b.get(j) != Some(&b':') {
                        return Err(self.err(at, "a voice is V and a number, then a colon"));
                    }
                    out.push(Item::Voice(n.unwrap_or(0) as u32));
                    i = j + 1;
                }
                b'[' => {
                    let end = matching(b, i, b'[', b']').ok_or_else(|| self.err(at, "a [ that isn't closed"))?;
                    let inner = &s[i + 1..end];
                    let (times, j) = repeat(b, end + 1);
                    // Endings: events marked 'n play on those times only.
                    for t in 1..=times {
                        let filtered = endings(inner, t);
                        self.events(&filtered, base + i + 1, st, out)?;
                    }
                    i = j;
                }
                b'{' => {
                    let end = matching(b, i, b'{', b'}').ok_or_else(|| self.err(at, "a { that isn't closed"))?;
                    let mut inner = vec![];
                    self.events(&s[i + 1..end], base + i + 1, st, &mut inner)?;
                    let (len, j) = duration(b, end + 1, st);
                    let total = inner.iter().fold(Frac::ZERO, |a, x| a.add(x.advance()));
                    let target = len.unwrap_or(st.length);
                    if total.is_positive() {
                        let f = target.div(total);
                        for x in &mut inner {
                            x.scale(f);
                        }
                    }
                    let (times, j) = repeat(b, j);
                    for _ in 0..times {
                        out.extend(inner.iter().cloned());
                    }
                    i = j;
                }
                b'a'..=b'g' | b'r' => {
                    let start = out.len();
                    let j = self.note_or_chord(b, i, at, st, out)?;
                    let (times, j) = repeat(b, j);
                    let made: Vec<Item> = out[start..].to_vec();
                    for _ in 1..times {
                        out.extend(made.iter().cloned());
                    }
                    i = j;
                }
                _ if c.is_ascii_alphabetic() => return Err(self.err(at, "a name that isn't a note, an attribute or a variable set before")),
                _ => return Err(self.err(at, "something Alda doesn't have")),
            }
        }
        self.depth -= 1;
        Ok(())
    }

    /// A note, rest, or chord (`c/e/g`) at `i`; where it ends.
    fn note_or_chord(&mut self, b: &[u8], mut i: usize, at: usize, st: &mut State, out: &mut Vec<Item>) -> Result<usize, Error> {
        let mut keys: Vec<(i32, Frac)> = vec![];
        let mut rest_len = None;
        let mut slur = false;
        loop {
            let c = b[i];
            i += 1;
            if c == b'r' {
                let (len, j) = duration(b, i, st);
                let len = len.unwrap_or(st.length);
                st.length = len;
                rest_len = Some(len);
                i = j;
            } else {
                let letter = c as char;
                let idx = super::letter_index(letter).ok_or_else(|| self.err(at, "a note is a to g"))?;
                let mut acc: Option<i32> = None;
                while let Some(&a) = b.get(i) {
                    match a {
                        b'+' => acc = Some(acc.unwrap_or(0) + 1),
                        b'-' => acc = Some(acc.unwrap_or(0) - 1),
                        b'_' => acc = Some(0),
                        _ => break,
                    }
                    i += 1;
                }
                let key = super::letter_key(letter).unwrap_or(60) + 12 * (st.octave - 4) + acc.unwrap_or(st.sig[idx]) + st.transpose;
                let (len, j) = duration(b, i, st);
                i = j;
                let len = len.unwrap_or(st.length);
                st.length = len;
                if b.get(i) == Some(&b'~') {
                    slur = true;
                    i += 1;
                }
                keys.push((key, len));
            }
            // A chord continues past `/` (octave changes may come between).
            let mut j = i;
            while j < b.len() && b[j] == b' ' {
                j += 1;
            }
            if b.get(j) == Some(&b'/') {
                j += 1;
                loop {
                    while j < b.len() && b[j] == b' ' {
                        j += 1;
                    }
                    match b.get(j) {
                        Some(b'>') => st.octave += 1,
                        Some(b'<') => st.octave -= 1,
                        Some(b'o') if b.get(j + 1).is_some_and(|c| c.is_ascii_digit()) => {
                            let (n, k) = int(b, j + 1);
                            st.octave = n.unwrap_or(4) as i32;
                            j = k;
                            continue;
                        }
                        _ => break,
                    }
                    j += 1;
                }
                if b.get(j).is_some_and(|c| (b'a'..=b'g').contains(c) || *c == b'r') {
                    i = j;
                    continue;
                }
            }
            break;
        }
        if keys.is_empty() {
            out.push(Item::Rest(rest_len.unwrap_or(st.length)));
        } else {
            let adv = keys.iter().map(|k| k.1).chain(rest_len).min().unwrap_or(st.length);
            let vel = (st.vol * 127.0 / 100.0).round().clamp(1.0, 127.0) as u8;
            out.push(Item::Note { keys, adv, vel, quant: st.quant, slur });
        }
        Ok(i)
    }

    /// One attribute: `tempo 120`, `quant! 50`, `key-sig "f+"`, `mf`...
    fn attribute(&mut self, body: &str, _at: usize, st: &mut State, out: &mut Vec<Item>) {
        let body = body.trim();
        // `(do (tempo 90) (quant 50))` and the like: each inner one.
        if body.starts_with("do ") || body.contains('(') && !body.starts_with("key-sig") && !body.starts_with("key-signature") {
            let b = body.as_bytes();
            let mut i = 0;
            while let Some(k) = body[i..].find('(') {
                let s = i + k;
                let Some(e) = matching(b, s, b'(', b')') else { break };
                self.attribute(&body[s + 1..e], _at, st, out);
                i = e + 1;
            }
            return;
        }
        let mut words = body.splitn(2, char::is_whitespace);
        let name = words.next().unwrap_or("");
        let arg = words.next().unwrap_or("").trim();
        let global = name.ends_with('!');
        let name = name.trim_end_matches('!');
        let num = arg.trim_start_matches(':').parse::<f64>().ok();
        let mut items = vec![];
        match name {
            "tempo" => {
                if let Some(t) = num.filter(|t| *t > 0.0) {
                    st.tempo = t;
                    items.push(Item::Tempo(t));
                }
            }
            "quant" | "quantize" | "quantization" => {
                if let Some(q) = num {
                    st.quant = Frac::new(q.clamp(0.0, 100.0).round() as i64, 100);
                }
            }
            "vol" | "volume" => {
                if let Some(v) = num {
                    st.vol = v.clamp(0.0, 100.0);
                }
            }
            "track-vol" | "track-volume" => {
                if let Some(v) = num {
                    items.push(Item::Control(7, (v.clamp(0.0, 100.0) * 1.27).round() as u8));
                }
            }
            "pan" | "panning" => {
                if let Some(v) = num {
                    items.push(Item::Control(10, (v.clamp(0.0, 100.0) * 1.27).round() as u8));
                }
            }
            "transpose" | "transposition" => st.transpose = num.unwrap_or(0.0) as i32,
            "octave" => match arg {
                ":up" => st.octave += 1,
                ":down" => st.octave -= 1,
                _ => st.octave = num.unwrap_or(4.0) as i32,
            },
            "key-sig" | "key-signature" => {
                if let Some(sig) = parse_key_sig(arg) {
                    st.sig = sig;
                }
            }
            _ => {
                if let Some(&(_, v)) = DYNAMICS.iter().find(|d| d.0 == name) {
                    st.vol = v;
                }
            }
        }
        if global {
            self.global.extend(items.iter().cloned());
        }
        out.extend(items);
    }
}

/// `"f+ c+"`, `[:g :major]`, `'(e flat minor)`, `"g major"`.
fn parse_key_sig(arg: &str) -> Option<[i32; 7]> {
    let words: Vec<String> = arg.split(|c: char| !(c.is_ascii_alphanumeric() || c == '+' || c == '-')).filter(|w| !w.is_empty()).map(|w| w.to_ascii_lowercase()).collect();
    if words.iter().any(|w| ["major", "minor", "ionian", "aeolian", "dorian", "phrygian", "lydian", "mixolydian", "locrian"].contains(&w.as_str())) {
        let mut name = words.first()?.to_ascii_uppercase();
        for w in &words[1..] {
            match w.as_str() {
                "flat" => name.push('b'),
                "sharp" => name.push('#'),
                m => {
                    name.push(' ');
                    name.push_str(m);
                }
            }
        }
        let (s, _) = key_signature(&name)?;
        return Some(signature_accidentals(s));
    }
    let mut sig = [0; 7];
    for w in words {
        let mut chars = w.chars();
        let Some(i) = chars.next().and_then(super::letter_index) else { continue };
        sig[i] = chars.map(|c| if c == '+' { 1 } else if c == '-' { -1 } else { 0 }).sum();
    }
    Some(sig)
}

fn int(b: &[u8], i: usize) -> (Option<i64>, usize) {
    let mut j = i;
    if b.get(j) == Some(&b'-') {
        j += 1;
    }
    while j < b.len() && b[j].is_ascii_digit() {
        j += 1;
    }
    (std::str::from_utf8(&b[i..j]).ok().and_then(|s| s.parse().ok()), j)
}

/// A duration at `i`: numbers (`4`, `8.`), seconds (`2s`) and
/// milliseconds (`500ms`) joined by `~` ties. None if there's none.
fn duration(b: &[u8], mut i: usize, st: &State) -> (Option<Frac>, usize) {
    let mut total: Option<Frac> = None;
    loop {
        if !b.get(i).is_some_and(|c| c.is_ascii_digit()) {
            break;
        }
        let st0 = i;
        while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'.' && b.get(i + 1).is_some_and(|c| c.is_ascii_digit())) {
            i += 1;
        }
        let n: f64 = std::str::from_utf8(&b[st0..i]).ok().and_then(|s| s.parse().ok()).unwrap_or(4.0);
        let len = if b.get(i) == Some(&b'm') && b.get(i + 1) == Some(&b's') {
            i += 2;
            seconds(n / 1000.0, st.tempo)
        } else if b.get(i) == Some(&b's') {
            i += 1;
            seconds(n, st.tempo)
        } else {
            let mut l = Frac::new(1000, (n * 1000.0).round().max(1.0) as i64);
            let mut add = l;
            while b.get(i) == Some(&b'.') {
                add = add.scale(1, 2);
                l = l.add(add);
                i += 1;
            }
            l
        };
        total = Some(total.map_or(len, |t| t.add(len)));
        // A tie to another duration.
        if b.get(i) == Some(&b'~') && b.get(i + 1).is_some_and(|c| c.is_ascii_digit()) {
            i += 1;
            continue;
        }
        break;
    }
    (total, i)
}

/// Seconds at a tempo, in whole notes (to a thousandth of a beat).
fn seconds(s: f64, bpm: f64) -> Frac {
    Frac::new((s * bpm / 60.0 * 1000.0).round() as i64, 4000)
}

/// `*n` after an event: the times it plays.
fn repeat(b: &[u8], i: usize) -> (u32, usize) {
    let mut j = i;
    while j < b.len() && b[j] == b' ' {
        j += 1;
    }
    if b.get(j) == Some(&b'*') {
        let (n, k) = int(b, j + 1);
        return (n.unwrap_or(1).clamp(1, 256) as u32, k);
    }
    (1, i)
}

/// A repeat's events for one time through: an event marked `'1` or `'2-3`
/// or `'1,3` kept only on those times.
fn endings(inner: &str, time: u32) -> String {
    if !inner.contains('\'') {
        return inner.to_string();
    }
    let mut out = String::new();
    // Tokens split by spaces; a mark applies to the token it ends.
    for token in inner.split(' ') {
        match token.rsplit_once('\'') {
            Some((event, mark)) if mark.chars().all(|c| c.is_ascii_digit() || c == ',' || c == '-') && !mark.is_empty() => {
                let on = mark.split(',').any(|p| match p.split_once('-') {
                    Some((a, b)) => (a.parse().unwrap_or(0)..=b.parse().unwrap_or(0)).contains(&time),
                    None => p.parse() == Ok(time),
                });
                if on {
                    out.push_str(event);
                    out.push(' ');
                }
            }
            _ => {
                out.push_str(token);
                out.push(' ');
            }
        }
    }
    out
}

/// A part (or group) declaration at the start of a line: names, an alias.
fn declaration(line: &str) -> Option<(Vec<String>, Option<String>, usize)> {
    let colon = line.find(':')?;
    let head = &line[..colon];
    if head.trim().is_empty() || head.trim_start().starts_with('V') && head.trim()[1..].chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let (names, alias) = match head.find('"') {
        Some(q) => {
            let alias = head[q + 1..].trim_end().trim_end_matches('"').to_string();
            (&head[..q], Some(alias))
        }
        None => (head, None),
    };
    let names: Vec<String> = names.split('/').map(|n| n.trim().to_string()).collect();
    if names.iter().any(|n| n.is_empty() || !n.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '\'')) || !n.chars().next().is_some_and(|c| c.is_ascii_alphabetic())) {
        return None;
    }
    Some((names, alias, colon + 1))
}

/// An instrument as declared: its name, whether it has an alias, and its
/// music's pieces (each where it is in the text).
type Declared = (String, bool, Vec<(usize, String)>);

/// An Alda score as a score (no title: Alda has none).
pub fn read(text: &str) -> Result<(Score, String), Error> {
    let mut r = Reader { text, vars: vec![], depth: 0, global: vec![] };
    // The parts, in order: their names, alias, and their music's pieces.
    // Each instrument (its name, whether it has an alias, its music), and
    // the names a declaration can use for several: aliases, `group.member`.
    let mut parts: Vec<Declared> = vec![];
    let mut aliases: Vec<(String, Vec<usize>)> = vec![];
    let mut cur: Vec<usize> = vec![];
    let mut preamble: Vec<(usize, String)> = vec![];
    let mut at = 0;
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let mut li = 0;
    while li < lines.len() {
        let raw = lines[li];
        let line_at = at;
        at += raw.len();
        li += 1;
        let line = raw.split('#').next().unwrap_or("").trim_end();
        let lead = line.len() - line.trim_start().len();
        let t = line.trim_start();
        if t.is_empty() {
            continue;
        }
        // A variable: `name = events`, maybe over several lines in [ ].
        if let Some(eq) = t.find('=') {
            let name = t[..eq].trim();
            if name.len() > 1 && name.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') && !name.contains(' ') {
                let mut value = t[eq + 1..].to_string();
                let value_at = line_at + lead + eq + 1;
                let open = |v: &str| v.matches('[').count() > v.matches(']').count() || v.matches('{').count() > v.matches('}').count();
                while open(&value) && li < lines.len() {
                    value.push('\n');
                    value.push_str(lines[li].split('#').next().unwrap_or(""));
                    at += lines[li].len();
                    li += 1;
                }
                r.vars.push((name.to_string(), (value_at, value)));
                continue;
            }
        }
        let mut rest = t;
        let mut rest_at = line_at + lead;
        if let Some((names, alias, end)) = declaration(t) {
            cur = match &alias {
                Some(a) if aliases.iter().any(|x| &x.0 == a) => aliases.iter().find(|x| &x.0 == a).map(|x| x.1.clone()).unwrap_or_default(),
                Some(a) => {
                    let mut made = vec![];
                    for n in &names {
                        parts.push((n.clone(), true, vec![]));
                        made.push(parts.len() - 1);
                        if names.len() > 1 {
                            aliases.push((format!("{a}.{n}"), vec![parts.len() - 1]));
                        }
                    }
                    aliases.push((a.clone(), made.clone()));
                    made
                }
                None => {
                    let mut found = vec![];
                    for n in &names {
                        if let Some(x) = aliases.iter().find(|x| &x.0 == n) {
                            found.extend(x.1.iter().copied());
                            continue;
                        }
                        let i = match parts.iter().position(|p| &p.0 == n && !p.1) {
                            Some(i) => i,
                            None => {
                                parts.push((n.clone(), false, vec![]));
                                parts.len() - 1
                            }
                        };
                        found.push(i);
                    }
                    found
                }
            };
            rest = &t[end..];
            rest_at += end;
        }
        if cur.is_empty() {
            // Before any part, only attributes: every part starts with them.
            if rest.trim_start().starts_with('(') {
                preamble.push((rest_at, rest.to_string()));
                continue;
            }
            return Err(bad(text, line_at + lead, "Alda's music starts with a part, like piano:"));
        }
        for &p in &cur {
            parts[p].2.push((rest_at, rest.to_string()));
        }
    }
    if parts.is_empty() {
        return Err(bad(text, 0, "Alda's music starts with a part, like piano:"));
    }
    let mut b = Builder::new();
    b.tempo(Frac::ZERO, 120.0);
    let mut markers: Vec<(String, Frac)> = vec![];
    let mut end = Frac::ZERO;
    let mut start = State::default();
    let mut first_items = vec![];
    for (o, piece) in &preamble {
        r.events(piece, *o, &mut start, &mut first_items)?;
    }
    if first_items.iter().any(|i| matches!(i, Item::Note { .. } | Item::Rest(_))) {
        return Err(bad(text, preamble[0].0, "Alda's music starts with a part, like piano:"));
    }
    b.tempo(Frac::ZERO, start.tempo);
    for (name, _, pieces) in &parts {
        let drums = name.contains("percussion");
        let program = super::program(name).unwrap_or(0);
        let index = b.part(name, program, drums);
        let mut st = start.clone();
        let mut items = vec![];
        for (o, piece) in pieces {
            r.events(piece, *o, &mut st, &mut items)?;
        }
        let e = perform(&items, &mut b, index, &mut markers);
        end = end.max(e);
    }
    b.end_at(end);
    Ok((b.finish(), String::new()))
}

/// A part's items into the builder; where it ends.
fn perform(items: &[Item], b: &mut Builder, part: usize, markers: &mut Vec<(String, Frac)>) -> Frac {
    let mut now = Frac::ZERO;
    // Voices: when they started, the furthest any reached, each one's time.
    let mut voices: Option<(Frac, Frac)> = None;
    let mut voice_now: Vec<(u32, Frac)> = vec![];
    let mut current_voice: Option<u32> = None;
    let mut furthest = Frac::ZERO;
    for item in items {
        match item {
            Item::Note { keys, adv, vel, quant, slur } => {
                for &(k, len) in keys {
                    let sounding = if *slur { len } else { len.mul(*quant) };
                    b.note(part, now, sounding, k, *vel);
                }
                now = now.add(*adv);
            }
            Item::Rest(l) => now = now.add(*l),
            Item::Tempo(t) => b.tempo(now, *t),
            Item::Control(n, v) => b.control(part, now, *n, *v),
            Item::Marker(m) => {
                markers.retain(|x| &x.0 != m);
                markers.push((m.clone(), now));
            }
            Item::Jump(m) => {
                if let Some(x) = markers.iter().find(|x| &x.0 == m) {
                    now = x.1;
                }
            }
            Item::Voice(n) => {
                if let Some(v) = current_voice {
                    match voice_now.iter_mut().find(|x| x.0 == v) {
                        Some(x) => x.1 = now,
                        None => voice_now.push((v, now)),
                    }
                }
                let (start, far) = voices.get_or_insert((now, now));
                *far = (*far).max(now);
                if *n == 0 {
                    now = voice_now.iter().map(|x| x.1).fold(*far, Frac::max);
                    voices = None;
                    voice_now.clear();
                    current_voice = None;
                } else {
                    now = voice_now.iter().find(|x| x.0 == *n).map(|x| x.1).unwrap_or(*start);
                    current_voice = Some(*n);
                }
            }
        }
        furthest = furthest.max(now);
    }
    furthest.max(now)
}

/// The name Alda gives a General MIDI program: `midi-` and GM's name.
fn alda_name(program: u8) -> String {
    let name = super::GM_NAMES[usize::from(program & 0x7f)].to_ascii_lowercase();
    let mut out = String::from("midi-");
    let mut dash = false;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            if dash && !out.ends_with('-') {
                out.push('-');
            }
            out.push(c);
            dash = false;
        } else {
            dash = true;
        }
    }
    out
}

/// A length as Alda durations tied with `~` (to a 64th note; triplets
/// as 3, 6, 12, 24).
fn alda_length(len: Frac) -> String {
    for d in [3i64, 6, 12, 24, 48] {
        if len == Frac::new(1, d) {
            return d.to_string();
        }
    }
    let mut table = vec![];
    for d in [1i64, 2, 4, 8, 16, 32, 64] {
        table.push((Frac::new(3, 2 * d), format!("{d}.")));
        table.push((Frac::new(1, d), d.to_string()));
    }
    table.sort_by_key(|x| std::cmp::Reverse(x.0));
    let mut left = len;
    let mut parts = vec![];
    while left >= Frac::new(1, 64) && parts.len() < 64 {
        let pick = table.iter().find(|x| x.0 <= left).unwrap_or(&table[table.len() - 1]);
        parts.push(pick.1.clone());
        left = left.sub(pick.0);
    }
    if parts.is_empty() {
        parts.push("64".into());
    }
    parts.join("~")
}

/// The score as Alda: a part each, its chords in voices, notes held their
/// full length (`(quant 100)`), each velocity a volume.
pub fn write(score: &Score) -> String {
    let q = Frac::int(4 * i64::from(score.ppq));
    let mut out = String::new();
    let names = ["c", "c+", "d", "e-", "e", "f", "f+", "g", "a-", "a", "b-", "b"];
    for (n, part) in score.parts.iter().enumerate() {
        let drums = part.channel == DRUMS;
        let mut lines: Vec<Vec<(Frac, Frac, u8, u8)>> = vec![];
        let mut notes = part.notes.clone();
        notes.sort_by_key(|n| (n.at, std::cmp::Reverse(n.key)));
        for x in notes {
            let (at, len) = (Frac::int(i64::from(x.at)).div(q), Frac::int(i64::from(x.length)).div(q));
            match lines.iter().position(|l| l.last().map_or(true, |p| p.0.add(p.1) <= at)) {
                Some(i) => lines[i].push((at, len, x.key, x.velocity)),
                None => lines.push(vec![(at, len, x.key, x.velocity)]),
            }
        }
        if lines.is_empty() {
            continue;
        }
        let instrument = if drums { "percussion".to_string() } else { alda_name(part.program) };
        out.push_str(&format!("{instrument} \"part{}\":\n", n + 1));
        if n == 0 {
            out.push_str(&format!("  (tempo! {})\n", (score.bpm * 100.0).round() / 100.0));
        }
        out.push_str("  (quant 100)\n");
        let many = lines.len() > 1;
        for (v, line) in lines.iter().enumerate() {
            let mut s = String::new();
            if many {
                s.push_str(&format!("  V{}: ", v + 1));
            } else {
                s.push_str("  ");
            }
            let mut octave = 4;
            s.push_str("o4 ");
            let mut vol = -1i64;
            let mut now = Frac::ZERO;
            for &(at, len, key, vel) in line {
                if at > now {
                    s.push_str(&format!("r{} ", alda_length(at.sub(now))));
                }
                let v = (f64::from(vel) * 100.0 / 127.0).round() as i64;
                if v != vol {
                    s.push_str(&format!("(vol {v}) "));
                    vol = v;
                }
                let o = i32::from(key) / 12 - 1;
                while octave < o {
                    s.push('>');
                    octave += 1;
                }
                while octave > o {
                    s.push('<');
                    octave -= 1;
                }
                s.push_str(names[usize::from(key % 12)]);
                s.push_str(&alda_length(len));
                s.push(' ');
                now = at.add(len).max(at.add(Frac::new(1, 64)));
            }
            out.push_str(s.trim_end());
            out.push('\n');
        }
        if many {
            out.push_str("  V0:\n");
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notes(score: &Score, p: usize) -> Vec<(u32, u32, u8, u8)> {
        score.parts[p].notes.iter().map(|n| (n.at, n.length, n.key, n.velocity)).collect()
    }

    #[test]
    fn a_part() {
        let (score, _) = read("piano:\n  (tempo 90) (quant 100) o4 c4 d8. e16 | f+2 > c4~8 r8 <b-1\n").unwrap();
        assert_eq!(score.bpm, 90.0);
        assert_eq!(score.parts[0].program, 0);
        assert_eq!(notes(&score, 0), vec![(0, 480, 60, 69), (480, 360, 62, 69), (840, 120, 64, 69), (960, 960, 66, 69), (1920, 720, 72, 69), (2880, 1920, 70, 69)]);
    }

    #[test]
    fn quant_chords_and_voices() {
        let (score, _) = read("violin \"v\":\n  (mf) c/e/g4 d\n  V1: c2 V2: e4 f g V0: a\n").unwrap();
        let n = notes(&score, 0);
        let vel = (54.21f64 * 1.27).round() as u8;
        assert_eq!(&n[..3], &[(0, 432, 60, vel), (0, 432, 64, vel), (0, 432, 67, vel)]);
        assert_eq!(n[3], (480, 432, 62, vel));
        // Voices start together; the next note waits for the longest.
        assert_eq!(n[4].0, 960);
        assert_eq!(n[5].0, 960);
        assert_eq!(n.last().unwrap().0, 960 + 1440);
        assert_eq!(score.parts[0].program, 40);
    }

    #[test]
    fn variables_repeats_cram_markers() {
        let text = "riff = c8 d e\npiano:\n  (quant 100) riff*2 [c4 d'1 e'2]*2 {c d e}4 %here g1\nflute:\n  (quant 100) @here c1\n";
        let (score, _) = read(text).unwrap();
        let keys: Vec<u8> = notes(&score, 0).iter().map(|n| n.2).collect();
        assert_eq!(keys, vec![60, 62, 64, 60, 62, 64, 60, 62, 60, 64, 60, 62, 64, 67]);
        // The cram: three notes in a quarter.
        let n = notes(&score, 0);
        assert_eq!((n[10].0, n[10].1), (720 * 2 + 960 * 2, 160));
        let here = n[13].0;
        assert_eq!(score.parts[1].notes[0].at, here);
    }

    #[test]
    fn keys_and_seconds() {
        let (score, _) = read("piano: (key-sig \"f+ c+\") (quant 100) f c c_ (key-sig '(b flat major)) b e (tempo 60) c1s").unwrap();
        let keys: Vec<u8> = notes(&score, 0).iter().map(|n| n.2).collect();
        assert_eq!(keys, vec![66, 61, 60, 70, 63, 60]);
        assert_eq!(notes(&score, 0)[5].1, 480);
        assert!(looks_like("piano:\n c d e"));
        assert!(!looks_like("hello: world"));
    }

    #[test]
    fn errors() {
        assert!(matches!(read("c d e"), Err(Error::BadText { line: 1, column: 1, .. })));
        assert!(matches!(read("piano: c d zz"), Err(Error::BadText { line: 1, column: 12, .. })));
    }

    #[test]
    fn writes_what_it_reads() {
        let (score, _) = read("piano:\n (tempo 100) (quant 100) c4 d8 e8 c/e/g2 r4 (vol 50) f+4.\npercussion:\n (quant 100) o2 c8 d8 c4\n").unwrap();
        let text = write(&score);
        let (again, _) = read(&text).unwrap();
        assert_eq!(again.bpm, 100.0);
        for (a, b) in again.parts.iter().zip(&score.parts) {
            assert_eq!(a.notes, b.notes, "{text}");
            assert_eq!((a.channel, a.program), (b.channel, b.program));
        }
    }
}
