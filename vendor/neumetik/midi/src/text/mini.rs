// Copyright 2026 ansiapps. Neumetik MIDI, MIT licensed: see LICENSE.

//! **Strudel and Tidal mini-notation**, from Strudel's and Tidal's
//! documentation: a cycle is a bar of 4/4. Steps share their cycle
//! equally; `~` (or `-`) rests; `[ ]` fits a sequence in a step; `< >`
//! plays one of its steps a cycle; `*n` and `/n` play a step faster or
//! slower; `!` repeats a step, `@n` (and `_`) makes it longer; `,`
//! plays sequences together; `?` drops a step half the time (or as
//! often as it says), by a seed, so the same text plays the same; `(k,n)`
//! and `(k,n,r)` are Euclidean rhythms. `.` groups the steps around it.
//! `{ }` is a polymeter: each of its layers steps through at the first
//! one's steps a cycle (or `%n`'s); `|` chooses one of the sequences it
//! splits each cycle, by the seed as `?` does.
//!
//! The text is either bare mini-notation, a layer a line, or Strudel or
//! Tidal code: each `note("...")`, `n("...")`, `s("...")`, `sound("...")`
//! (Tidal: `note "..."`, `n "..."`, `s "..."`, `sound "..."`) a part.
//! Notes are names (`c`, `eb4`, `f#2`) or numbers (Strudel's MIDI keys;
//! Tidal's semitones from middle C); sounds named like a drum machine's
//! (`bd`, `sd`, `hh`, `cp`...) are General MIDI drums, and a `.s("piano")`
//! on a note pattern is its instrument. `setcpm` and `setcps` set the
//! tempo; Strudel's default, half a cycle a second, is 120 beats a
//! minute. As many cycles are read as it takes the music to repeat, at
//! most 64.

use super::{bad, Builder, Frac};
use crate::score::{Score, DRUMS};
use crate::Error;

#[derive(Debug, Clone, PartialEq)]
enum Pat {
    Atom(String),
    Silence,
    /// Steps and their weights.
    Seq(Vec<(Pat, Frac)>),
    Alt(Vec<Pat>),
    Stack(Vec<Pat>),
    Fast(Box<Pat>, Frac),
    Degrade(Box<Pat>, f64, u64),
    /// One of them a cycle, chosen by a seed.
    Choose(Vec<Pat>, u64),
}

/// An event: when it starts and ends (in cycles), and its value.
type Event = (Frac, Frac, String);

fn floor(f: Frac) -> i64 {
    f.num().div_euclid(f.den())
}

/// Every event of a pattern starting in `[begin, end)`.
fn query(p: &Pat, begin: Frac, end: Frac, out: &mut Vec<Event>, depth: usize) {
    if depth > 64 || begin >= end || out.len() > 1 << 18 {
        return;
    }
    match p {
        Pat::Silence => {}
        Pat::Atom(v) => {
            let mut c = floor(begin);
            if Frac::int(c) < begin {
                c += 1;
            }
            while Frac::int(c) < end {
                out.push((Frac::int(c), Frac::int(c + 1), v.clone()));
                c += 1;
            }
        }
        Pat::Stack(ps) => {
            for p in ps {
                query(p, begin, end, out, depth + 1);
            }
        }
        Pat::Seq(steps) => {
            let total = steps.iter().fold(Frac::ZERO, |a, s| a.add(s.1));
            if !total.is_positive() {
                return;
            }
            for c in floor(begin)..=floor(end) {
                let mut offset = Frac::ZERO;
                for (child, w) in steps {
                    let (o, width) = (offset.div(total), w.div(total));
                    offset = offset.add(*w);
                    let mut sub = vec![];
                    query(child, Frac::int(c), Frac::int(c + 1), &mut sub, depth + 1);
                    for (s, e, v) in sub {
                        let map = |t: Frac| Frac::int(c).add(o).add(t.sub(Frac::int(c)).mul(width));
                        let (s, e) = (map(s), map(e));
                        if s >= begin && s < end {
                            out.push((s, e, v));
                        }
                    }
                }
            }
        }
        Pat::Alt(ps) => {
            let n = ps.len() as i64;
            if n == 0 {
                return;
            }
            for c in floor(begin)..=floor(end) {
                let child = &ps[c.rem_euclid(n) as usize];
                let k = c.div_euclid(n);
                let shift = Frac::int(c - k);
                let mut sub = vec![];
                query(child, Frac::int(k), Frac::int(k + 1), &mut sub, depth + 1);
                for (s, e, v) in sub {
                    let (s, e) = (s.add(shift), e.add(shift));
                    if s >= begin && s < end {
                        out.push((s, e, v));
                    }
                }
            }
        }
        Pat::Fast(p, f) => {
            if !f.is_positive() {
                return;
            }
            let mut sub = vec![];
            query(p, begin.mul(*f), end.mul(*f), &mut sub, depth + 1);
            out.extend(sub.into_iter().map(|(s, e, v)| (s.div(*f), e.div(*f), v)));
        }
        Pat::Degrade(p, prob, seed) => {
            let mut sub = vec![];
            query(p, begin, end, &mut sub, depth + 1);
            out.extend(sub.into_iter().filter(|(s, _, _)| chance(*seed, *s) >= *prob));
        }
        Pat::Choose(ps, seed) => {
            if ps.is_empty() {
                return;
            }
            for c in floor(begin)..=floor(end) {
                let pick = ((chance(*seed, Frac::int(c)) * ps.len() as f64) as usize).min(ps.len() - 1);
                let (from, to) = (begin.max(Frac::int(c)), end.min(Frac::int(c + 1)));
                query(&ps[pick], from, to, out, depth + 1);
            }
        }
    }
}

/// A number from 0 to 1 for a time, the same every time (SplitMix64).
fn chance(seed: u64, at: Frac) -> f64 {
    let mut z = seed ^ (at.num() as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ (at.den() as u64).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 53) as f64
}

fn lcm(a: i64, b: i64) -> i64 {
    let (a, b) = (a.abs().max(1), b.abs().max(1));
    (a / super::gcd(a, b)).saturating_mul(b)
}

/// How many cycles until the pattern repeats (capped).
fn period(p: &Pat) -> i64 {
    let cap = |n: i64| n.clamp(1, 1 << 12);
    match p {
        Pat::Atom(_) | Pat::Silence => 1,
        Pat::Seq(s) => cap(s.iter().fold(1, |a, (c, _)| lcm(a, period(c)))),
        Pat::Stack(s) => cap(s.iter().fold(1, |a, c| lcm(a, period(c)))),
        Pat::Alt(s) => cap(s.len() as i64 * s.iter().fold(1, |a, c| lcm(a, period(c)))),
        // P cycles, f times faster: repeats every P / f cycles, made whole.
        Pat::Fast(c, f) => cap(Frac::int(period(c)).div(*f).num()),
        Pat::Degrade(c, _, _) => period(c),
        // Never quite: long enough to hear the choices.
        Pat::Choose(s, _) => cap(16 * s.iter().fold(1, |a, c| lcm(a, period(c)))),
    }
}

/// A sequence's steps and their weights.
type Steps = Vec<(Pat, Frac)>;

struct Parser<'a> {
    text: &'a str,
    s: &'a [u8],
    i: usize,
    /// Where the pattern starts in the whole text.
    offset: usize,
    seed: u64,
}

impl<'a> Parser<'a> {
    fn err(&self, why: &'static str) -> Error {
        bad(self.text, self.offset + self.i, why)
    }
    fn space(&mut self) {
        while self.i < self.s.len() && matches!(self.s[self.i], b' ' | b'\t' | b'\n' | b'\r') {
            self.i += 1;
        }
    }
    fn number(&mut self) -> Option<f64> {
        let st = self.i;
        while self.i < self.s.len() && (self.s[self.i].is_ascii_digit() || self.s[self.i] == b'.' || (self.i == st && self.s[self.i] == b'-')) {
            self.i += 1;
        }
        std::str::from_utf8(&self.s[st..self.i]).ok()?.parse().ok()
    }
    /// A sequence up to `close`: layers split by `,` (or choices by `|`,
    /// and then true), steps by spaces, groups by ` . `.
    fn sequence(&mut self, close: Option<u8>, depth: usize) -> Result<(Vec<Steps>, bool), Error> {
        if depth > 32 {
            return Err(self.err("patterns nested too deep"));
        }
        let mut layers: Vec<Vec<(Pat, Frac)>> = vec![];
        let mut groups: Vec<Vec<(Pat, Frac)>> = vec![];
        let mut steps: Vec<(Pat, Frac)> = vec![];
        // Split by | (true) or , (false), once either is seen.
        let mut split: Option<bool> = None;
        let finish_group = |groups: &mut Vec<Vec<(Pat, Frac)>>, steps: &mut Vec<(Pat, Frac)>| {
            groups.push(std::mem::take(steps));
        };
        let finish_layer = |layers: &mut Vec<Vec<(Pat, Frac)>>, groups: &mut Vec<Vec<(Pat, Frac)>>, steps: &mut Vec<(Pat, Frac)>| {
            if groups.is_empty() {
                layers.push(std::mem::take(steps));
            } else {
                groups.push(std::mem::take(steps));
                let seq = groups.drain(..).filter(|g| !g.is_empty()).map(|g| (Pat::Seq(g), Frac::int(1))).collect();
                layers.push(seq);
            }
        };
        loop {
            self.space();
            let Some(&c) = self.s.get(self.i) else {
                if close.is_some() {
                    return Err(self.err("a bracket that isn't closed"));
                }
                break;
            };
            if Some(c) == close {
                self.i += 1;
                break;
            }
            match c {
                b']' | b'>' | b'}' => return Err(self.err("a closing bracket with nothing open")),
                b',' | b'|' => {
                    let choose = c == b'|';
                    if split.is_some_and(|s| s != choose) {
                        return Err(self.err("a sequence splits by , or by |, not both"));
                    }
                    split = Some(choose);
                    self.i += 1;
                    finish_layer(&mut layers, &mut groups, &mut steps);
                }
                b'.' if self.s.get(self.i + 1).map_or(true, |c| *c == b' ') => {
                    self.i += 1;
                    finish_group(&mut groups, &mut steps);
                }
                b'_' => {
                    self.i += 1;
                    if let Some(last) = steps.last_mut() {
                        last.1 = last.1.add(Frac::int(1));
                    }
                }
                b'!' => {
                    // A lone ! repeats the step before.
                    self.i += 1;
                    let n = self.number().map(|n| n as usize).unwrap_or(1).clamp(1, 64);
                    if let Some(last) = steps.last().cloned() {
                        for _ in 0..n {
                            steps.push(last.clone());
                        }
                    }
                }
                _ => {
                    let step = self.step(depth)?;
                    steps.extend(step);
                }
            }
        }
        finish_layer(&mut layers, &mut groups, &mut steps);
        Ok((layers, split == Some(true)))
    }

    /// Layers played together, or one of them a cycle.
    fn together(&mut self, mut pats: Vec<Pat>, choose: bool) -> Pat {
        if pats.len() == 1 {
            return pats.pop().unwrap_or(Pat::Silence);
        }
        if choose {
            self.seed = self.seed.wrapping_add(1);
            Pat::Choose(pats, self.seed)
        } else {
            Pat::Stack(pats)
        }
    }

    /// One step and its modifiers (a `!n` makes several).
    fn step(&mut self, depth: usize) -> Result<Vec<(Pat, Frac)>, Error> {
        let c = self.s[self.i];
        let mut pat = match c {
            b'[' | b'<' => {
                self.i += 1;
                let close = if c == b'[' { b']' } else { b'>' };
                let (layers, choose) = self.sequence(Some(close), depth + 1)?;
                let pats: Vec<Pat> = layers
                    .into_iter()
                    .filter(|l| !l.is_empty())
                    .map(|l| if c == b'[' { Pat::Seq(l) } else { Pat::Alt(l.into_iter().map(|(p, _)| p).collect()) })
                    .collect();
                self.together(pats, choose)
            }
            b'{' => {
                // A polymeter: every layer a step at a time, as many
                // steps a cycle as the first has (or `%n`).
                self.i += 1;
                let (layers, choose) = self.sequence(Some(b'}'), depth + 1)?;
                let layers: Vec<Vec<(Pat, Frac)>> = layers.into_iter().filter(|l| !l.is_empty()).collect();
                let weight = |l: &[(Pat, Frac)]| l.iter().fold(Frac::ZERO, |a, s| a.add(s.1));
                let mut steps = layers.first().map(|l| weight(l)).unwrap_or(Frac::int(1));
                if self.s.get(self.i) == Some(&b'%') {
                    self.i += 1;
                    steps = to_frac(self.number().filter(|n| *n > 0.0).ok_or_else(|| self.err("% is followed by a number of steps"))?);
                }
                let pats = layers
                    .into_iter()
                    .map(|l| {
                        let w = weight(&l);
                        Pat::Fast(Box::new(Pat::Seq(l)), steps.div(w))
                    })
                    .collect();
                self.together(pats, choose)
            }
            b'~' => {
                self.i += 1;
                Pat::Silence
            }
            b'-' if self.s.get(self.i + 1).map_or(true, |c| !c.is_ascii_digit()) => {
                self.i += 1;
                Pat::Silence
            }
            _ => {
                let st = self.i;
                while self.i < self.s.len() && (self.s[self.i].is_ascii_alphanumeric() || matches!(self.s[self.i], b'#' | b':' | b'-' | b'.' | b'\'') && !(self.s[self.i] == b'.' && self.s.get(self.i + 1).map_or(true, |c| !c.is_ascii_digit()))) {
                    self.i += 1;
                }
                if self.i == st {
                    return Err(self.err("something mini-notation doesn't have"));
                }
                Pat::Atom(String::from_utf8_lossy(&self.s[st..self.i]).to_string())
            }
        };
        let mut weight = Frac::int(1);
        let mut copies = 1;
        while let Some(&m) = self.s.get(self.i) {
            match m {
                b'*' | b'/' => {
                    self.i += 1;
                    let n = self.number().filter(|n| *n > 0.0).ok_or_else(|| self.err("* and / are followed by a number"))?;
                    let f = to_frac(n);
                    pat = Pat::Fast(Box::new(pat), if m == b'*' { f } else { Frac::int(1).div(f) });
                }
                b'@' => {
                    self.i += 1;
                    weight = to_frac(self.number().filter(|n| *n > 0.0).ok_or_else(|| self.err("@ is followed by a number"))?);
                }
                b'!' if self.s.get(self.i + 1).is_some_and(|c| c.is_ascii_digit()) => {
                    self.i += 1;
                    copies = self.number().map(|n| n as usize).unwrap_or(2).clamp(1, 64);
                }
                b'?' => {
                    self.i += 1;
                    let p = self.number().unwrap_or(0.5).clamp(0.0, 1.0);
                    self.seed = self.seed.wrapping_add(1);
                    pat = Pat::Degrade(Box::new(pat), p, self.seed);
                }
                b'(' => {
                    self.i += 1;
                    let mut nums = vec![];
                    loop {
                        self.space();
                        nums.push(self.number().ok_or_else(|| self.err("a Euclidean rhythm is (beats,steps) or (beats,steps,rotation)"))? as i64);
                        self.space();
                        match self.s.get(self.i) {
                            Some(b',') => self.i += 1,
                            Some(b')') => {
                                self.i += 1;
                                break;
                            }
                            _ => return Err(self.err("a Euclidean rhythm is (beats,steps) or (beats,steps,rotation)")),
                        }
                    }
                    let (k, n) = (*nums.first().unwrap_or(&1), *nums.get(1).unwrap_or(&1));
                    let r = *nums.get(2).unwrap_or(&0);
                    let hits = euclid(k.clamp(0, 256) as usize, n.clamp(1, 256) as usize, r);
                    pat = Pat::Seq(hits.into_iter().map(|h| (if h { pat.clone() } else { Pat::Silence }, Frac::int(1))).collect());
                }
                _ => break,
            }
        }
        Ok(vec![(pat, weight); copies])
    }
}

fn to_frac(n: f64) -> Frac {
    // To a thousandth; most are whole numbers.
    let d = 1000i64;
    Frac::new((n * d as f64).round() as i64, d)
}

/// Bjorklund's rhythm, as Strudel makes it: `k` beats spread over `n`
/// steps, turned `r` steps later.
pub fn euclid(k: usize, n: usize, r: i64) -> Vec<bool> {
    fn go(a: Vec<Vec<bool>>, b: Vec<Vec<bool>>) -> (Vec<Vec<bool>>, Vec<Vec<bool>>) {
        if a.len().min(b.len()) <= 1 {
            return (a, b);
        }
        if a.len() > b.len() {
            let lb = b.len();
            let joined = a[..lb].iter().zip(&b).map(|(x, y)| [x.clone(), y.clone()].concat()).collect();
            go(joined, a[lb..].to_vec())
        } else {
            let la = a.len();
            let joined = a.iter().zip(&b[..la]).map(|(x, y)| [x.clone(), y.clone()].concat()).collect();
            go(joined, b[la..].to_vec())
        }
    }
    let k = k.min(n);
    let (a, b) = go(vec![vec![true]; k], vec![vec![false]; n - k]);
    let mut out: Vec<bool> = a.into_iter().chain(b).flatten().collect();
    if !out.is_empty() {
        let r = r.rem_euclid(out.len() as i64) as usize;
        out.rotate_right(r);
    }
    out
}

/// Parses a pattern at `offset` in `text`.
fn pattern(text: &str, offset: usize, body: &str, seed: u64) -> Result<Pat, Error> {
    let mut p = Parser { text, s: body.as_bytes(), i: 0, offset, seed };
    let (layers, choose) = p.sequence(None, 0)?;
    let pats: Vec<Pat> = layers.into_iter().filter(|l| !l.is_empty()).map(Pat::Seq).collect();
    Ok(p.together(pats, choose))
}

/// A sound's General MIDI drum key, by a drum machine's name for it.
fn drum(name: &str) -> Option<u8> {
    let n = name.split(':').next().unwrap_or("").to_ascii_lowercase();
    Some(match n.as_str() {
        "bd" | "kick" | "bassdrum" => 36,
        "sd" | "sn" | "snare" => 38,
        "rim" | "rs" => 37,
        "cp" | "clap" | "clp" => 39,
        "hh" | "ch" | "hihat" | "hat" => 42,
        "oh" | "openhat" => 46,
        "lt" | "tom" => 45,
        "mt" => 47,
        "ht" => 50,
        "cr" | "crash" | "cy" => 49,
        "rd" | "ride" => 51,
        "cb" | "cowbell" => 56,
        "sh" | "shaker" => 70,
        "tb" | "tambourine" | "tamb" => 54,
        "perc" => 76,
        "misc" => 81,
        "fx" => 55,
        _ => return None,
    })
}

/// A note's key: a name with an octave (else `octave`), or a number
/// (`numbers` added to it).
fn key(value: &str, octave: i32, numbers: i32) -> Option<i32> {
    let v = value.split(':').next().unwrap_or("");
    if let Ok(n) = v.parse::<f64>() {
        return Some(n.round() as i32 + numbers);
    }
    let mut chars = v.chars();
    let letter = chars.next()?;
    let mut k = super::letter_key(letter)?;
    let rest: String = chars.collect();
    let mut r = rest.as_str();
    loop {
        if let Some(x) = r.strip_prefix('#').or_else(|| r.strip_prefix('s')) {
            k += 1;
            r = x;
        } else if let Some(x) = r.strip_prefix('b').or_else(|| r.strip_prefix('f')) {
            k -= 1;
            r = x;
        } else {
            break;
        }
    }
    let o = if r.is_empty() { octave } else { r.parse().ok()? };
    Some(k + 12 * (o - 4))
}

/// One part found in the text: where its pattern is, what it holds.
struct Found {
    offset: usize,
    body: String,
    sounds: bool,
    instrument: Option<String>,
}

/// The quoted patterns of Strudel or Tidal code, or the lines of bare
/// mini-notation.
fn find(text: &str) -> (Vec<Found>, bool) {
    let mut found = vec![];
    let s = text.as_bytes();
    let tidal = text.contains("d1 $") || text.contains("d1 ") && text.contains('$') || text.contains("# s ") || text.contains("# sound ");
    let mut i = 0;
    while i < s.len() {
        let c = s[i];
        if c == b'"' || c == b'`' {
            let Some(end) = text[i + 1..].find(c as char).map(|e| i + 1 + e) else { break };
            // The function before: `note(` or `note `.
            let before = text[..i].trim_end_matches([' ', '(']);
            let name: String = before.chars().rev().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect::<String>().chars().rev().collect();
            let chained = before.len() > name.len() && before[..before.len() - name.len()].trim_end().ends_with(['.', '#']);
            let body = &text[i + 1..end];
            match name.as_str() {
                "note" | "n" | "up" => found.push(Found { offset: i + 1, body: body.to_string(), sounds: false, instrument: None }),
                "s" | "sound" => {
                    let all_drums = body.split(|c: char| !(c.is_ascii_alphanumeric() || c == ':')).filter(|w| !w.is_empty() && w.chars().next().is_some_and(|c| c.is_ascii_alphabetic())).all(|w| drum(w).is_some());
                    if chained && !all_drums {
                        if let Some(last) = found.last_mut().filter(|f| !f.sounds) {
                            last.instrument = Some(body.trim().split([' ', ':']).next().unwrap_or("").to_string());
                        }
                    } else {
                        found.push(Found { offset: i + 1, body: body.to_string(), sounds: true, instrument: None });
                    }
                }
                _ => {}
            }
            i = end + 1;
            continue;
        }
        i += 1;
    }
    if found.is_empty() && !text.contains('"') {
        let mut at = 0;
        for line in text.split_inclusive('\n') {
            let body = line.split("//").next().unwrap_or("");
            if !body.trim().is_empty() {
                let words: Vec<&str> = body.split(|c: char| !(c.is_ascii_alphanumeric() || c == ':' || c == '#')).filter(|w| w.chars().next().is_some_and(|c| c.is_ascii_alphabetic())).collect();
                let sounds = !words.is_empty() && words.iter().all(|w| drum(w).is_some());
                found.push(Found { offset: at, body: body.trim_end_matches(['\n', '\r']).to_string(), sounds, instrument: None });
            }
            at += line.len();
        }
    }
    (found, tidal)
}

/// A tempo from `setcpm(n)`, `setcps(x)` or Tidal's `setcps x`, in quarter
/// notes a minute (a cycle a bar of 4/4).
fn tempo(text: &str, tidal: bool) -> f64 {
    for (word, per_minute) in [("setcpm", true), ("setcps", false), ("setCpm", true), ("setCps", false)] {
        if let Some(i) = text.find(word) {
            let rest = text[i + word.len()..].trim_start();
            let expr = rest.strip_prefix('(').map(|r| {
                // To the matching parenthesis.
                let mut depth = 1;
                let end = r.char_indices().find(|&(_, c)| {
                    match c {
                        '(' => depth += 1,
                        ')' => depth -= 1,
                        _ => {}
                    }
                    depth == 0
                });
                &r[..end.map(|e| e.0).unwrap_or(r.len())]
            });
            let expr = expr.unwrap_or_else(|| rest.lines().next().unwrap_or(""));
            if let Some(v) = arithmetic(expr) {
                let cpm = if per_minute { v } else { v * 60.0 };
                if cpm > 0.0 {
                    return cpm * 4.0;
                }
            }
        }
    }
    if tidal {
        // Tidal's default: 0.5625 cycles a second.
        135.0
    } else {
        120.0
    }
}

/// `120/60/4` and the like: numbers, `+ - * /` and parentheses.
fn arithmetic(s: &str) -> Option<f64> {
    fn expr(s: &[u8], i: &mut usize) -> Option<f64> {
        let mut v = term(s, i)?;
        loop {
            skip(s, i);
            match s.get(*i) {
                Some(b'+') => {
                    *i += 1;
                    v += term(s, i)?;
                }
                Some(b'-') => {
                    *i += 1;
                    v -= term(s, i)?;
                }
                _ => return Some(v),
            }
        }
    }
    fn term(s: &[u8], i: &mut usize) -> Option<f64> {
        let mut v = atom(s, i)?;
        loop {
            skip(s, i);
            match s.get(*i) {
                Some(b'*') => {
                    *i += 1;
                    v *= atom(s, i)?;
                }
                Some(b'/') => {
                    *i += 1;
                    let d = atom(s, i)?;
                    v = if d == 0.0 { return None } else { v / d };
                }
                _ => return Some(v),
            }
        }
    }
    fn atom(s: &[u8], i: &mut usize) -> Option<f64> {
        skip(s, i);
        if s.get(*i) == Some(&b'(') {
            *i += 1;
            let v = expr(s, i)?;
            skip(s, i);
            if s.get(*i) == Some(&b')') {
                *i += 1;
            }
            return Some(v);
        }
        let st = *i;
        while *i < s.len() && (s[*i].is_ascii_digit() || s[*i] == b'.') {
            *i += 1;
        }
        std::str::from_utf8(&s[st..*i]).ok()?.parse().ok()
    }
    fn skip(s: &[u8], i: &mut usize) {
        while *i < s.len() && s[*i] == b' ' {
            *i += 1;
        }
    }
    expr(s.as_bytes(), &mut 0)
}

/// Mini-notation (bare, or in Strudel or Tidal code) as a score.
pub fn read(text: &str) -> Result<(Score, String), Error> {
    let (found, tidal) = find(text);
    if found.is_empty() {
        return Err(bad(text, 0, "there's no pattern in it"));
    }
    let mut b = Builder::new();
    b.tempo(Frac::ZERO, tempo(text, tidal));
    b.meter(Frac::ZERO, 4, 4);
    let mut end = 0i64;
    let mut pats = vec![];
    for (n, f) in found.iter().enumerate() {
        let p = pattern(text, f.offset, &f.body, 0x6d69_6e69 + n as u64 * 7919)?;
        end = end.max(period(&p));
        pats.push(p);
    }
    let cycles = end.clamp(1, 64);
    for (f, p) in found.iter().zip(&pats) {
        let mut events = vec![];
        query(p, Frac::ZERO, Frac::int(cycles), &mut events, 0);
        let (name, program) = match (&f.instrument, f.sounds) {
            (_, true) => ("Drums".to_string(), 0),
            (Some(i), false) => (i.clone(), super::program(i).or_else(|| synth_program(i)).unwrap_or(0)),
            (None, false) => ("Notes".to_string(), 0),
        };
        let part = b.part(&name, program, f.sounds);
        for (s, e, v) in events {
            let k = if f.sounds {
                match drum(&v) {
                    Some(k) => Some(i32::from(k)),
                    // A pitched sound among drums: played as a note.
                    None => key(&v, 4, 0),
                }
            } else if tidal {
                key(&v, 5, 60)
            } else {
                key(&v, 3, 0)
            };
            match k {
                Some(k) => b.note(part, s, e.sub(s), k, 100),
                None => return Err(bad(text, f.offset, if f.sounds { "a sound that isn't a drum or a note" } else { "a note that isn't a name like c4 or a number" })),
            }
        }
    }
    b.end_at(Frac::int(cycles));
    Ok((b.finish(), String::new()))
}

/// Strudel's built-in synths' sounds, as GM stand-ins.
fn synth_program(name: &str) -> Option<u8> {
    Some(match name {
        "sawtooth" | "saw" | "supersaw" => 81,
        "square" | "pulse" => 80,
        "triangle" | "tri" | "sine" | "sin" => 80,
        "superpiano" => 0,
        "gm_piano" => 0,
        _ => return None,
    })
}

const NAMES: [&str; 12] = ["c", "c#", "d", "eb", "e", "f", "f#", "g", "ab", "a", "bb", "b"];

/// One bar of a line as mini-notation: steps on the finest grid its
/// notes need (to 48 a bar), chords as `[c4,e4]`, held notes `@n`.
fn bar_mini(notes: &[(Frac, Frac, String)]) -> String {
    if notes.is_empty() {
        return "~".into();
    }
    let grids = [1i64, 2, 3, 4, 6, 8, 12, 16, 24, 32, 48];
    let fits = |g: i64| notes.iter().all(|n| n.0.mul(Frac::int(g)).den() == 1 && n.0.add(n.1).mul(Frac::int(g)).den() == 1);
    let g = grids.iter().copied().find(|&g| fits(g)).unwrap_or(48);
    let step = |f: Frac| (f.mul(Frac::int(g)).f64().round() as i64).clamp(0, g);
    let mut cells: Vec<(i64, i64, Vec<String>)> = vec![];
    for (at, len, v) in notes {
        let (s, e) = (step(*at), step(at.add(*len)).max(step(*at) + 1).min(g));
        if s >= g {
            continue;
        }
        match cells.iter_mut().find(|c| c.0 == s) {
            Some(c) => {
                c.1 = c.1.max(e);
                if !c.2.contains(v) {
                    c.2.push(v.clone());
                }
            }
            None => cells.push((s, e, vec![v.clone()])),
        }
    }
    cells.sort_by_key(|c| c.0);
    let mut out: Vec<String> = vec![];
    let mut now = 0;
    for (i, (s, e, vs)) in cells.iter().enumerate() {
        let (s, mut e) = (*s, *e);
        if let Some(next) = cells.get(i + 1) {
            e = e.min(next.0);
        }
        if s > now {
            let rest = s - now;
            out.push(if rest == 1 { "~".into() } else { format!("~@{rest}") });
        }
        let what = if vs.len() == 1 { vs[0].clone() } else { format!("[{}]", vs.join(",")) };
        out.push(if e - s == 1 { what } else { format!("{what}@{}", e - s) });
        now = e;
    }
    if now < g {
        let rest = g - now;
        out.push(if rest == 1 { "~".into() } else { format!("~@{rest}") });
    }
    if out.len() == 1 {
        return out[0].split('@').next().unwrap_or("~").to_string();
    }
    format!("[{}]", out.join(" "))
}

/// The score as Strudel code: a pattern a part, a cycle a bar (of the
/// score's meter), each bar its own step of `< >`; drums as `s()` with
/// a drum machine's names (others left out), notes as `note()`.
pub fn write(score: &Score) -> String {
    let bar = u64::from(score.bar_ticks());
    let bars = score.bars.max(1) as usize;
    let mut lines = vec![];
    for part in &score.parts {
        let drums = part.channel == DRUMS;
        let mut per_bar: Vec<Vec<(Frac, Frac, String)>> = vec![vec![]; bars];
        for n in &part.notes {
            let b = (u64::from(n.at) / bar) as usize;
            if b >= bars {
                continue;
            }
            let at = Frac::new((u64::from(n.at) % bar) as i64, bar as i64);
            let len = Frac::new(i64::from(n.length), bar as i64).min(Frac::int(1).sub(at));
            let v = if drums {
                match n.key {
                    35 | 36 => "bd",
                    38 | 40 => "sd",
                    37 => "rim",
                    39 => "cp",
                    42 | 44 => "hh",
                    46 => "oh",
                    41 | 43 | 45 => "lt",
                    47 | 48 => "mt",
                    50 => "ht",
                    49 | 52 | 55 | 57 => "cr",
                    51 | 53 | 59 => "rd",
                    56 => "cb",
                    54 => "tb",
                    69 | 70 => "sh",
                    _ => continue,
                }
                .to_string()
            } else {
                format!("{}{}", NAMES[usize::from(n.key % 12)], i32::from(n.key) / 12 - 1)
            };
            per_bar[b].push((at, len, v));
        }
        if per_bar.iter().all(|b| b.is_empty()) {
            continue;
        }
        let bars_text: Vec<String> = per_bar.iter().map(|b| bar_mini(b)).collect();
        let pattern = if bars_text.len() == 1 { bars_text[0].clone() } else { format!("<{}>", bars_text.join(" ")) };
        let program = super::GM_NAMES[usize::from(part.program & 0x7f)].to_ascii_lowercase().replace([' ', '(', ')', '-'], "_").replace("__", "_").trim_end_matches('_').to_string();
        lines.push(if drums { format!("  s(\"{pattern}\")") } else { format!("  note(\"{pattern}\").s(\"gm_{program}\")") });
    }
    // A cycle is a bar: bars a minute.
    let quarters = f64::from(score.beats) * 4.0 / f64::from(score.unit.max(1));
    let cpm = score.bpm / quarters;
    let cpm = (cpm * 1000.0).round() / 1000.0;
    format!("setcpm({cpm})\nstack(\n{}\n)\n", lines.join(",\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn events(p: &str, cycles: i64) -> Vec<(String, Frac, Frac)> {
        let pat = pattern(p, 0, p, 1).unwrap();
        let mut out = vec![];
        query(&pat, Frac::ZERO, Frac::int(cycles), &mut out, 0);
        out.sort_by(|a, b| a.0.cmp(&b.0).then(a.2.cmp(&b.2)));
        out.into_iter().map(|(s, e, v)| (v, s, e.sub(s))).collect()
    }

    fn f(n: i64, d: i64) -> Frac {
        Frac::new(n, d)
    }

    #[test]
    fn sequences_and_subsequences() {
        assert_eq!(events("a [b c] ~", 1), vec![("a".into(), f(0, 1), f(1, 3)), ("b".into(), f(1, 3), f(1, 6)), ("c".into(), f(1, 2), f(1, 6))]);
        assert_eq!(events("a . b c", 1), vec![("a".into(), f(0, 1), f(1, 2)), ("b".into(), f(1, 2), f(1, 4)), ("c".into(), f(3, 4), f(1, 4))]);
    }

    #[test]
    fn alternation_speed_and_weight() {
        let e = events("<a b c>", 3);
        assert_eq!(e.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(), vec!["a", "b", "c"]);
        assert_eq!(e[1].1, f(1, 1));
        let e = events("a*2 b", 1);
        assert_eq!(e.iter().map(|x| (x.0.as_str(), x.1)).collect::<Vec<_>>(), vec![("a", f(0, 1)), ("a", f(1, 4)), ("b", f(1, 2))]);
        let e = events("a/2", 2);
        assert_eq!(e, vec![("a".into(), f(0, 1), f(2, 1))]);
        let e = events("a@3 b", 1);
        assert_eq!(e[1].1, f(3, 4));
        let e = events("a _ _ b", 1);
        assert_eq!(e[1].1, f(3, 4));
        let e = events("a! b!2", 1);
        assert_eq!(e.len(), 4);
        let e = events("<a b>*2", 1);
        assert_eq!(e.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(), vec!["a", "b"]);
    }

    #[test]
    fn stacks_euclid_and_chance() {
        let e = events("a b, c", 1);
        assert_eq!(e.len(), 3);
        let e = events("a(3,8)", 1);
        assert_eq!(e.iter().map(|x| x.1).collect::<Vec<_>>(), vec![f(0, 1), f(3, 8), f(6, 8)]);
        assert_eq!(euclid(3, 8, 0), vec![true, false, false, true, false, false, true, false]);
        assert_eq!(euclid(5, 8, 0), vec![true, false, true, true, false, true, true, false]);
        let a = events("a*16?", 4);
        assert!(a.len() > 16 && a.len() < 48, "{}", a.len());
        assert_eq!(a, events("a*16?", 4), "the same each time");
        assert!(events("a*16?0.9", 1).len() < 8);
    }

    #[test]
    fn strudel_code() {
        let code = "setcpm(30)\nstack(\n  note(\"<c3 e3> [g3 b3]\").s(\"sawtooth\"),\n  s(\"bd*2 [~ sd]\")\n)";
        let (score, _) = read(code).unwrap();
        assert_eq!(score.bpm, 120.0);
        assert_eq!(score.parts.len(), 2);
        assert_eq!(score.parts[0].program, 81);
        let n: Vec<(u32, u8)> = score.parts[0].notes.iter().map(|n| (n.at, n.key)).collect();
        assert_eq!(n, vec![(0, 48), (960, 55), (1440, 59), (1920, 52), (2880, 55), (3360, 59)]);
        let d: Vec<(u32, u8)> = score.parts[1].notes.iter().map(|n| (n.at, n.key)).collect();
        assert_eq!(d, vec![(0, 36), (480, 36), (1440, 38), (1920, 36), (2400, 36), (3360, 38)]);
        assert_eq!(score.parts[1].channel, DRUMS);
    }

    #[test]
    fn tidal_and_bare() {
        let (score, _) = read("setcps (120/60/4)\nd1 $ n \"0 4 7\" # s \"superpiano\"\n").unwrap();
        assert_eq!(score.bpm, 120.0);
        assert_eq!(score.parts[0].notes.iter().map(|n| n.key).collect::<Vec<_>>(), vec![60, 64, 67]);
        let (score, _) = read("bd hh sd hh\nc4 e4\n").unwrap();
        assert_eq!(score.parts[0].channel, DRUMS);
        assert_eq!(score.parts[1].notes[1].key, 64);
    }

    #[test]
    fn polymeters_step_through_at_the_first_layers_rate() {
        let f = Frac::new;
        let names = |e: &[(String, Frac, Frac)]| e.iter().map(|x| (x.0.clone(), x.1)).collect::<Vec<_>>();
        // Four steps a cycle through three: the next cycle goes on from d.
        let e = events("{c d e}%4", 2);
        let want: Vec<(String, Frac)> = ["c", "d", "e", "c", "d", "e", "c", "d"].iter().enumerate().map(|(i, n)| (n.to_string(), f(i as i64, 4))).collect();
        assert_eq!(names(&e), want);
        // A second layer at the first's three steps a cycle.
        let mut e = events("{c d e, f g}", 1);
        e.sort_by(|a, b| (a.1, &a.0).cmp(&(b.1, &b.0)));
        assert_eq!(names(&e), vec![("c".into(), f(0, 1)), ("f".into(), f(0, 1)), ("d".into(), f(1, 3)), ("g".into(), f(1, 3)), ("e".into(), f(2, 3)), ("f".into(), f(2, 3))]);
    }

    #[test]
    fn a_choice_is_one_sequence_a_cycle_and_the_same_each_time() {
        let e = events("c d | e f", 16);
        assert_eq!(e.len(), 32);
        for c in 0..16 {
            let cycle: Vec<&str> = e.iter().filter(|x| x.1 >= Frac::int(c) && x.1 < Frac::int(c + 1)).map(|x| x.0.as_str()).collect();
            assert!(cycle == ["c", "d"] || cycle == ["e", "f"], "{cycle:?}");
        }
        assert!(e.iter().any(|x| x.0 == "c") && e.iter().any(|x| x.0 == "e"));
        assert_eq!(e, events("c d | e f", 16));
        assert!(matches!(read("c, d | e"), Err(Error::BadText { .. })));
    }

    #[test]
    fn errors() {
        assert!(matches!(read("note(\"c3 [e3\")"), Err(Error::BadText { line: 1, .. })));
        assert!(matches!(read("c3 }"), Err(Error::BadText { line: 1, column: 4, .. })));
    }

    #[test]
    fn writes_what_it_reads() {
        let code = "setcpm(30)\nstack(\n  note(\"<[c3 e3] [g3,b3]> ~ c4@2\"),\n  s(\"bd*2 [~ sd]\")\n)";
        let (score, _) = read(code).unwrap();
        let text = write(&score);
        let (again, _) = read(&text).unwrap();
        for (a, b) in again.parts.iter().zip(&score.parts) {
            assert_eq!(a.notes, b.notes, "{text}");
        }
        assert_eq!(again.bpm, score.bpm);
    }
}
