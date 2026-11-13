// Copyright 2026 ansiapps. Neumetik MIDI, MIT licensed: see LICENSE.
//
// The MML compiler here is a port of SiON's (org.si.sion.sequencer.base.MMLParser
// and MMLSequencer, and org.si.sion.sequencer.SiMMLSequencer and SiMMLTrack's
// note handling, github.com/keim/SiON):
//
// Copyright (c) 2008-2016 keim
// All rights reserved (each source file: "Distributed under BSD-style license";
// the repository's LICENSE.md is the MIT License, reproduced here).
//
// The MIT License (MIT)
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

//! **SiON**, the MML of SiON (Flash, 2008), read by a port of its own
//! compiler: `/* */` and `//` comments, macros (`#A=cde;` and `A`, `A(3)`
//! transposed, `#MACRO{dynamic}`), `![ ... !| ... !]3` repeats, then
//! each sequence (`;` ends one): notes, `r`, `l`, `^` ties, dots, `o`
//! with `<` up and `>` down (`#REV` flips them), `q` and `@q` gates, `v`
//! `(` `)` and `x`, `p` and `@p`, `&` slurs, `&&`, `*`, `[ | ]` repeats,
//! `t` (with `#TMODE`), `kt`, `!@ns`, `%` module types and `@` tones, and
//! the `#` commands (`#TITLE`, `#SIGN`, `#QUANT`, `#REV`, `#TMODE`,
//! `#FPS`, `#VMODE`, `#END`). Every other command is read and skipped as
//! SiON reads it.
//!
//! SiON's own synthesizer isn't here: its channels are played by General
//! MIDI stand-ins (a PSG, pulse or APU channel a square lead, a ramp a
//! saw lead, a sine an ocarina, FM an electric piano, Karplus-Strong a
//! guitar), and its noise and sampler channels as drums by pitch. Its
//! envelopes, filters, LFOs, detune, pitch bends and FM voice settings
//! aren't heard.

use super::{Builder, Frac};
use crate::score::Score;
use crate::Error;

/// Ticks a whole note (`MMLParserSetting.resolution`).
const RES: i64 = 1920;

/// SiON's `int.MIN_VALUE`, for a parameter left out.
const NONE: i64 = i32::MIN as i64;

/// The most steps one sequence takes, past any song, short of a trap.
const MOST_STEPS: usize = 1 << 23;

/// The most text macros and repeats expand to.
const MOST_TEXT: usize = 8 << 20;

/// What an event is (`MMLEvent`'s ids).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum K {
    Note,
    Rest,
    Quant,
    QuantCount,
    Volume,
    VolumeShift,
    FineVolume,
    Slur,
    SlurWeak,
    Bend,
    RepBegin,
    RepBreak,
    RepEnd,
    RepAll,
    ModType,
    ModParam,
    InPipe,
    OutPipe,
    Param,
    Tempo,
    System,
    Table,
    /// A command SiMMLSequencer registers by its letters.
    User(&'static str),
}

#[derive(Debug, Clone)]
struct E {
    k: K,
    data: i64,
    len: i64,
    /// A repeat's other end (begin: its end; end and break: their begin).
    jump: usize,
}

/// One `;`-ended sequence, as the parser chains it.
#[derive(Debug, Clone, Default)]
struct Seq {
    ev: Vec<E>,
    /// Tempo and table events in it (`SEQUENCE_HEAD.data`).
    globals: usize,
}

/// SiMMLSequencer's command letters (`newMMLEventListener`).
const USER: &[&str] = &[
    "k", "kt", "!@kr", "@mask", "p", "@p", "@f", "x", "%v", "%x", "%f", "@clock", "@al", "@fb", "@r", "%t", "%e", "i", "@rr", "@tl", "@ml", "@dt", "@ph", "@fx", "@se", "@er", "s", "@lfo", "mp", "ma", "@fps", "@@", "na",
    "np", "nt", "nf", "_@@", "_na", "_np", "_nt", "_nf", "!na", "po",
];

/// `MMLParserSetting` as SiMMLSequencer sets it, and what `#` commands change.
#[derive(Debug, Clone)]
struct Setting {
    max_quant_ratio: i64,
    default_quant_ratio: i64,
    octave_polarization: i64,
    volume_polarization: i64,
    key_sign: [i64; 7],
    /// `t`: 0 BPM, 1 TIMERB, 2 frame.
    tmode: u8,
    tres: f64,
    default_shift: i64,
}

impl Setting {
    fn new() -> Setting {
        Setting { max_quant_ratio: 8, default_quant_ratio: 6, octave_polarization: 1, volume_polarization: 1, key_sign: [0; 7], tmode: 0, tres: 1.0, default_shift: 4 }
    }
}

const KEY_SIGNS: [[i64; 7]; 15] = [
    [0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 1, 0, 0, 0],
    [1, 0, 0, 1, 0, 0, 0],
    [1, 0, 0, 1, 1, 0, 0],
    [1, 1, 0, 1, 1, 0, 0],
    [1, 1, 0, 1, 1, 1, 0],
    [1, 1, 1, 1, 1, 1, 0],
    [1, 1, 1, 1, 1, 1, 1],
    [0, 0, 0, 0, 0, 0, -1],
    [0, 0, -1, 0, 0, 0, -1],
    [0, 0, -1, 0, 0, -1, -1],
    [0, -1, -1, 0, 0, -1, -1],
    [0, -1, -1, 0, -1, -1, -1],
    [-1, -1, -1, 0, -1, -1, -1],
    [-1, -1, -1, -1, -1, -1, -1],
];

/// `MMLParser.keySign`: the signature a letter names.
fn key_sign(sign: &str) -> Option<[i64; 7]> {
    let n = match sign {
        "" | "C" | "Am" => 0,
        "G" | "Em" => 1,
        "D" | "Bm" => 2,
        "A" | "F+m" | "F#m" => 3,
        "E" | "C+m" | "C#m" => 4,
        "B" | "G+m" | "G#m" => 5,
        "F+" | "F#" | "D+m" | "D#m" => 6,
        "C+" | "C#" | "A+m" | "A#m" => 7,
        "F" | "Dm" => 8,
        "B-" | "Bb" | "Gm" => 9,
        "E-" | "Eb" | "Cm" => 10,
        "A-" | "Ab" | "Fm" => 11,
        "D-" | "Db" | "B-m" | "Bbm" => 12,
        "G-" | "Gb" | "E-m" | "Ebm" => 13,
        "C-" | "Cb" | "A-m" | "Abm" => 14,
        _ => return None,
    };
    Some(KEY_SIGNS[n])
}

/// ActionScript's `int(string)`: digits as far as they go, else 0.
fn as_int(s: &str) -> i64 {
    let s = s.trim();
    let (neg, digits) = match s.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, s.strip_prefix('+').unwrap_or(s)),
    };
    let end = digits.find(|c: char| !c.is_ascii_digit()).unwrap_or(digits.len());
    let n = digits[..end].parse::<i64>().unwrap_or(0).min(i64::from(i32::MAX));
    if neg {
        -n
    } else {
        n
    }
}

fn starts(s: &[char], at: usize, pat: &str) -> bool {
    let mut i = at;
    for c in pat.chars() {
        if s.get(i) != Some(&c) {
            return false;
        }
        i += 1;
    }
    true
}

fn find(s: &[char], from: usize, pat: &str) -> Option<usize> {
    (from..s.len()).find(|&i| starts(s, i, pat))
}

fn find_char(s: &[char], from: usize, c: char) -> Option<usize> {
    (from..s.len()).find(|&i| s[i] == c)
}

fn text_of(s: &[char]) -> String {
    s.iter().collect()
}

/// Removes `/* */` and `// ... newline` as `onBeforeCompile` does.
fn strip_comments(s: &[char]) -> Vec<char> {
    let mut out = Vec::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        if starts(s, i, "/*") {
            if let Some(e) = find(s, i + 2, "*/") {
                i = e + 2;
                continue;
            }
        } else if starts(s, i, "//") {
            if let Some(n) = (i + 2..s.len()).find(|&j| matches!(s[j], '\r' | '\n')) {
                let mut j = n;
                while j < s.len() && matches!(s[j], '\r' | '\n') {
                    j += 1;
                }
                i = j;
                continue;
            }
        }
        out.push(s[i]);
        i += 1;
    }
    out
}

/// Macros and the commands before parsing.
struct Pre {
    macros: [String; 26],
    dynamic: bool,
    setting: Setting,
    title: String,
}

impl Pre {
    /// `_expandMacro`: every capital letter a macro, `A(n)` transposed by n.
    fn expand(&self, m: &[char], depth: usize) -> Result<Vec<char>, &'static str> {
        if depth > 64 {
            return Err("a macro uses itself");
        }
        let mut out = Vec::with_capacity(m.len());
        let mut i = 0;
        while i < m.len() {
            let c = m[i];
            if !c.is_ascii_uppercase() {
                out.push(c);
                i += 1;
                continue;
            }
            let id = (c as u8 - b'A') as usize;
            i += 1;
            // ( digits and minus signs )
            let mut arg = None;
            if m.get(i) == Some(&'(') {
                let mut j = i + 1;
                while j < m.len() && (m[j] == '-' || m[j].is_ascii_digit()) {
                    j += 1;
                }
                if j > i + 1 && m.get(j) == Some(&')') {
                    arg = Some(as_int(&text_of(&m[i + 1..j])));
                    i = j + 1;
                }
            }
            let body = &self.macros[id];
            if body.is_empty() {
                continue;
            }
            let body: Vec<char> = if self.dynamic { self.expand(&body.chars().collect::<Vec<_>>(), depth + 1)? } else { body.chars().collect() };
            if let Some(t) = arg {
                out.extend(format!("!@ns{t}").chars());
                out.extend(body);
                out.extend(format!("!@ns{}", -t).chars());
            } else {
                out.extend(body);
            }
            if out.len() > MOST_TEXT {
                return Err("the macros are too long");
            }
        }
        Ok(out)
    }

    /// `_parseSystemCommandBefore`: whether it was done now.
    fn system(&mut self, cmd: &str, prm: &[char]) -> Result<bool, &'static str> {
        // \s*(\d*)\s*(\{(.*?)\})?(.*)
        let mut i = 0;
        while i < prm.len() && prm[i].is_whitespace() {
            i += 1;
        }
        let d0 = i;
        while i < prm.len() && prm[i].is_ascii_digit() {
            i += 1;
        }
        let num = as_int(&text_of(&prm[d0..i]));
        while i < prm.len() && prm[i].is_whitespace() {
            i += 1;
        }
        let (no_data, dat, pfx) = if prm.get(i) == Some(&'{') {
            match find_char(prm, i + 1, '}') {
                Some(e) => (false, text_of(&prm[i + 1..e]), text_of(&prm[e + 1..])),
                None => (true, String::new(), text_of(&prm[i..])),
            }
        } else {
            (true, String::new(), text_of(&prm[i..]))
        };
        match cmd {
            "#TITLE" => self.title = if no_data { pfx } else { dat },
            "#FPS" | "#TABLE" | "#WAV" | "#WAVB" | "#SAMPLER" | "#PCMWAVE" | "#PCMVOICE" | "#WAVEXP" | "#PCMB" | "#PCMC" => {}
            "#@" | "#OPM@" | "#OPN@" | "#OPL@" | "#OPX@" | "#MA@" | "#AL@" => {}
            "#SIGN" => {
                let s = if no_data { pfx.trim().to_string() } else { dat };
                self.setting.key_sign = key_sign(&s).ok_or("it isn't a key signature")?;
            }
            "#MACRO" => {
                let d = if no_data { pfx.trim().to_string() } else { dat };
                match d.as_str() {
                    "dynamic" => self.dynamic = true,
                    "static" => self.dynamic = false,
                    _ => return Err("#MACRO is dynamic or static"),
                }
            }
            "#QUANT" => {
                if num > 0 {
                    self.setting.max_quant_ratio = num;
                    self.setting.default_quant_ratio = (num as f64 * 0.75) as i64;
                }
            }
            "#TMODE" => {
                // (unit|timerb|fps)=?([\d.]*)
                let d: Vec<char> = dat.chars().collect();
                for (name, mode) in [("unit", 0u8), ("timerb", 1), ("fps", 2)] {
                    if let Some(at) = find(&d, 0, name) {
                        let mut j = at + name.len();
                        if d.get(j) == Some(&'=') {
                            j += 1;
                        }
                        let k = (j..d.len()).find(|&k| !(d[k].is_ascii_digit() || d[k] == '.')).unwrap_or(d.len());
                        let n: f64 = text_of(&d[j..k]).parse().unwrap_or(0.0);
                        match mode {
                            0 => self.setting.tres = if n > 0.0 { 1.0 / n } else { 1.0 },
                            1 => self.setting.tres = (if n > 0.0 { n } else { 4000.0 }) * 1.220703125,
                            _ => self.setting.tres = if n > 0.0 { n * 60.0 } else { 3600.0 },
                        }
                        self.setting.tmode = mode;
                        break;
                    }
                }
            }
            "#VMODE" => {
                // %v<mode>,<shift>
                let d: Vec<char> = dat.chars().collect();
                let mut from = 0;
                while let Some(at) = find(&d, from, "%v") {
                    let mut j = at + 2;
                    while j < d.len() && d[j].is_ascii_digit() {
                        j += 1;
                    }
                    let mut k = j;
                    while k < d.len() && d[k].is_whitespace() {
                        k += 1;
                    }
                    if d.get(k) == Some(&',') {
                        k += 1;
                        while k < d.len() && d[k].is_whitespace() {
                            k += 1;
                        }
                    }
                    self.setting.default_shift = match d.get(k) {
                        Some(c) if c.is_ascii_digit() => i64::from(*c as u8 - b'0'),
                        _ => 4,
                    };
                    if !(0..8).contains(&self.setting.default_shift) {
                        self.setting.default_shift = 0;
                    }
                    from = j.max(at + 2);
                }
            }
            "#REV" => {
                let d = if no_data { pfx.trim().to_string() } else { dat };
                match d.as_str() {
                    "" => {
                        self.setting.octave_polarization = -1;
                        self.setting.volume_polarization = -1;
                    }
                    "octave" => self.setting.octave_polarization = -1,
                    "volume" => self.setting.volume_polarization = -1,
                    _ => return Err("#REV is octave or volume"),
                }
            }
            // After parsing: the FM connections, which aren't made here.
            "#FM" => return Ok(false),
            _ => {}
        }
        Ok(true)
    }
}

/// `onBeforeCompile`: comments, macros and the `#` commands out, the
/// sequences left, with `![ !]` repeats written out.
fn precompile(text: &str) -> Result<(Vec<char>, Pre), &'static str> {
    let mut pre = Pre { macros: Default::default(), dynamic: false, setting: Setting::new(), title: String::new() };
    let mut s: Vec<char> = text.chars().collect();
    s.push('\n');
    let mut s = strip_comments(&s);
    while s.last().is_some_and(|c| matches!(c, ' ' | '\t' | '\r' | '\n')) {
        s.pop();
    }
    if s.is_empty() {
        return Err("there is no MML in it");
    }
    if s.last() != Some(&';') {
        s.push(';');
    }
    let mut out: Vec<char> = vec![];
    let mut i = 0;
    while i < s.len() {
        while i < s.len() && matches!(s[i], ' ' | '\t' | '\r' | '\n') {
            i += 1;
        }
        if i >= s.len() {
            break;
        }
        // (#([A-Z@\-]+)(\+=|=)?)?
        let mut name: Option<String> = None;
        let mut op = 0u8;
        if s[i] == '#' {
            let mut j = i + 1;
            while j < s.len() && (s[j].is_ascii_uppercase() || s[j] == '@' || s[j] == '-') {
                j += 1;
            }
            if j > i + 1 {
                name = Some(text_of(&s[i + 1..j]));
                i = j;
                if starts(&s, i, "+=") {
                    op = 2;
                    i += 2;
                } else if s.get(i) == Some(&'=') {
                    op = 1;
                    i += 1;
                }
            }
        }
        // ([^;{]*({.*?})?[^;]*);
        let mut p = i;
        while p < s.len() && s[p] != ';' && s[p] != '{' {
            p += 1;
        }
        if s.get(p) == Some(&'{') {
            let mut c = p + 1;
            while let Some(q) = find_char(&s, c, '}') {
                if find_char(&s, q + 1, ';').is_some() {
                    p = q + 1;
                    break;
                }
                c = q + 1;
            }
        }
        let Some(end) = find_char(&s, p, ';') else { break };
        let body = &s[i..end];
        i = end + 1;
        match (name, op) {
            (None, _) => {
                out.extend(pre.expand(body, 0)?);
                out.push(';');
            }
            (Some(n), 0) => {
                if n == "END" {
                    break;
                }
                let cmd = format!("#{n}");
                if !pre.system(&cmd, body)? {
                    out.extend(format!("{cmd}").chars());
                    out.extend(body);
                    out.push(';');
                }
            }
            (Some(n), op) => {
                // Macro ids: ([A-Z])?(-([A-Z])?)? again and again.
                let ids: Vec<char> = n.chars().collect();
                let value = if pre.dynamic { body.to_vec() } else { pre.expand(body, 0)? };
                let mut k = 0;
                loop {
                    let from = k;
                    let first = ids.get(k).copied().filter(|c| c.is_ascii_uppercase());
                    if first.is_some() {
                        k += 1;
                    }
                    let mut dash = false;
                    let mut last = None;
                    if ids.get(k) == Some(&'-') {
                        dash = true;
                        k += 1;
                        if let Some(c) = ids.get(k).copied().filter(|c| c.is_ascii_uppercase()) {
                            last = Some(c);
                            k += 1;
                        }
                    }
                    if k == from {
                        break;
                    }
                    let start = first.map_or(0, |c| (c as u8 - b'A') as usize);
                    let stop = if dash { last.map_or(25, |c| (c as u8 - b'A') as usize) } else { start };
                    for m in start..=stop.min(25) {
                        let v = text_of(&value);
                        if op == 2 {
                            pre.macros[m].push_str(&v);
                        } else {
                            pre.macros[m] = v;
                        }
                    }
                }
            }
        }
        if out.len() > MOST_TEXT {
            return Err("the macros are too long");
        }
    }
    Ok((expand_repeats(&out)?, pre))
}

/// `![n ... !| ... !]n`, written out n times.
fn expand_repeats(s: &[char]) -> Result<Vec<char>, &'static str> {
    let mut out = Vec::with_capacity(s.len());
    let mut i = 0;
    'outer: while i < s.len() {
        if starts(s, i, "![") {
            let mut j = i + 2;
            let d1 = j;
            while j < s.len() && s[j].is_ascii_digit() {
                j += 1;
            }
            let digits1 = text_of(&s[d1..j]);
            // (.*?) lazily, then (!\|(.*?))?!\]
            let mut k = j;
            while k <= s.len() {
                let mut hit: Option<(usize, Option<(usize, usize)>)> = None;
                if starts(s, k, "!|") {
                    if let Some(e) = find(s, k + 2, "!]") {
                        hit = Some((e, Some((k + 2, e))));
                    }
                }
                if hit.is_none() && starts(s, k, "!]") {
                    hit = Some((k, None));
                }
                if let Some((e, tail)) = hit {
                    let body = &s[j..k];
                    let mut after = e + 2;
                    let d5 = after;
                    while after < s.len() && s[after].is_ascii_digit() {
                        after += 1;
                    }
                    let digits5 = text_of(&s[d5..after]);
                    let n = if !digits1.is_empty() {
                        as_int(&digits1) - 1
                    } else if !digits5.is_empty() {
                        as_int(&digits5) - 1
                    } else {
                        1
                    }
                    .min(256);
                    let mut unit = body.to_vec();
                    if let Some((a, b)) = tail {
                        unit.extend_from_slice(&s[a..b]);
                    }
                    for _ in 0..n.max(0) {
                        out.extend_from_slice(&unit);
                        if out.len() > MOST_TEXT {
                            return Err("the repeats are too long");
                        }
                    }
                    out.extend_from_slice(body);
                    i = after;
                    continue 'outer;
                }
                k += 1;
            }
        }
        out.push(s[i]);
        i += 1;
    }
    Ok(out)
}

/// What the regular expression of `MMLParser` makes of one place.
enum Tok {
    Space,
    System(String),
    Note(char, i64),
    User(&'static str),
    Event(&'static str),
    Table,
}

const DEFAULT_EVENTS: &[&str] = &["@q", "@v", "@i", "@o", "&&", "!@ns", "@", "r", "l", "q", "o", "v", "t", "^", "<", ">", "(", ")", "[", "]", "/", "|", "$", "%", "&", "*", ",", ";"];

/// The token at `i` (if any) and where it ends: the alternatives in
/// the order the regular expression tries them.
fn token(s: &[char], i: usize) -> Option<(Tok, usize)> {
    let c = s[i];
    if c.is_whitespace() {
        let mut j = i;
        while j < s.len() && s[j].is_whitespace() {
            j += 1;
        }
        return Some((Tok::Space, j));
    }
    if c == '#' {
        let j = find_char(s, i, ';').unwrap_or(s.len());
        return Some((Tok::System(text_of(&s[i..j])), j));
    }
    if ('a'..='g').contains(&c) {
        let mut shift = 0;
        let mut j = i + 1;
        match s.get(j) {
            Some('+' | '#') => {
                shift = 1;
                j += 1;
            }
            Some('-') => {
                shift = -1;
                j += 1;
            }
            _ => {}
        }
        return Some((Tok::Note(c, shift), j));
    }
    // User events, the longest of them (the expression lists them backwards).
    if let Some(u) = USER.iter().filter(|u| starts(s, i, u)).max_by_key(|u| u.len()) {
        return Some((Tok::User(u), i + u.chars().count()));
    }
    // The defaults: @q @v @i @o, &&, !@ns, @, then the single letters.
    if let Some(d) = DEFAULT_EVENTS.iter().find(|d| starts(s, i, d)) {
        return Some((Tok::Event(d), i + d.chars().count()));
    }
    if c == '{' {
        let e = find_char(s, i, '}')?;
        let mut j = e + 1;
        while j < s.len() && s[j].is_ascii_digit() {
            j += 1;
        }
        if s.get(j) == Some(&'*') {
            j += 1;
        }
        while j < s.len() && (s[j].is_ascii_digit() || matches!(s[j], '-' | '.')) {
            j += 1;
        }
        if s.get(j) == Some(&'+') {
            j += 1;
        }
        while j < s.len() && (s[j].is_ascii_digit() || matches!(s[j], '-' | '.')) {
            j += 1;
        }
        return Some((Tok::Table, j));
    }
    None
}

/// `MMLParser.parse` over the expanded text.
fn parse(text: &str, s: &[char], setting: &Setting) -> Result<Vec<Seq>, Error> {
    let err = |why: &'static str| super::bad(text, 0, why);
    let mml2nn = 60 - 5 * 12;
    let mut seqs: Vec<Seq> = vec![Seq::default()];
    let mut length = RES / 4;
    let mut octave = 5i64;
    let mut note_shift = 0i64;
    let mut last_is_length = false;
    let mut stack: Vec<usize> = vec![];
    const KEY_SCALE: [i64; 7] = [0, 2, 4, 5, 7, 9, 11];
    let mut i = 0;
    let mut steps = 0usize;
    while i < s.len() {
        steps += 1;
        if steps > MOST_STEPS {
            return Err(err("the MML is too long"));
        }
        let Some((tok, end)) = token(s, i) else {
            i += 1;
            continue;
        };
        // \s*(-?[0-9]*)\s*(\.*)
        let mut j = end;
        while j < s.len() && s[j].is_whitespace() {
            j += 1;
        }
        let p0 = j;
        if s.get(j) == Some(&'-') {
            j += 1;
        }
        while j < s.len() && s[j].is_ascii_digit() {
            j += 1;
        }
        let param: Option<i64> = if j > p0 { Some(as_int(&text_of(&s[p0..j]))) } else { None };
        while j < s.len() && s[j].is_whitespace() {
            j += 1;
        }
        let mut periods = 0;
        while s.get(j) == Some(&'.') {
            periods += 1;
            j += 1;
        }
        // A system command takes the rest up to the `;`, so no tail.
        i = if matches!(tok, Tok::System(_)) { end } else { j };
        let seq = seqs.last_mut().unwrap();
        // `__calcLength()` at the parse: the length in ticks, or none.
        let len_param = || -> Result<Option<i64>, Error> {
            match param {
                None => Ok(None),
                Some(0) => Ok(Some(0)),
                Some(n) => {
                    let l = RES / n;
                    if !(1..=RES).contains(&l) {
                        return Err(err("a length must be 1 to 1920"));
                    }
                    Ok(Some(l))
                }
            }
        };
        // `__calcLength(iLength, period)`.
        let calc = |l: Option<i64>, length: i64| -> i64 {
            let base = l.unwrap_or(length);
            let mut total = base;
            let mut p = periods;
            while p > 0 {
                total += base >> p.min(62);
                p -= 1;
            }
            total
        };
        let push = |seq: &mut Seq, k: K, data: i64, len: i64| {
            if k == K::Rest && seq.ev.last().is_some_and(|e| e.k == K::Rest) {
                seq.ev.last_mut().unwrap().len += len;
            } else {
                seq.ev.push(E { k, data, len, jump: 0 });
                if matches!(k, K::Tempo | K::Table) {
                    seq.globals += 1;
                }
            }
        };
        // A note option (`&`, `&&`, `*`): after a note, taking its length.
        let option = |seq: &mut Seq, k: K| -> Result<(), Error> {
            match seq.ev.last_mut() {
                Some(e) if e.k == K::Note => {
                    let len = e.len;
                    e.len = 0;
                    seq.ev.push(E { k, data: 0, len, jump: 0 });
                    Ok(())
                }
                _ => Err(err("a * or & needs a note before it")),
            }
        };
        let mut was_length = false;
        match tok {
            Tok::Space => {}
            Tok::Note(c, shift) => {
                let n = c as i64 - 'c' as i64;
                let n = if n < 0 { n + 7 } else { n } as usize;
                let shift = setting.key_sign[n] + shift;
                let len = calc(len_param()?, length);
                let key = (KEY_SCALE[n] + shift + mml2nn + octave * 12 + note_shift).clamp(0, 127);
                push(seq, K::Note, key, len);
            }
            Tok::User(u) => push(seq, K::User(u), param.unwrap_or(NONE), 0),
            Tok::Event(e) => match e {
                "r" => {
                    let len = calc(len_param()?, length);
                    push(seq, K::Rest, 0, len);
                }
                "l" => {
                    length = calc(len_param()?, length);
                    was_length = true;
                }
                "^" => {
                    let len = calc(len_param()?, length);
                    if last_is_length {
                        length += len;
                        was_length = true;
                    } else if let Some(l) = seq.ev.last_mut().filter(|l| matches!(l.k, K::Rest | K::Note)) {
                        l.len += len;
                    } else {
                        return Err(err("a tie needs a note or rest before it"));
                    }
                }
                "o" => {
                    let o = param.unwrap_or(5);
                    if !(0..=9).contains(&o) {
                        return Err(err("o is 0 to 9"));
                    }
                    octave = o;
                }
                "q" => {
                    let q = param.unwrap_or(setting.default_quant_ratio);
                    if !(0..=setting.max_quant_ratio).contains(&q) {
                        return Err(err("q is out of range"));
                    }
                    push(seq, K::Quant, q, 0);
                }
                "@q" => {
                    let q = param.unwrap_or(0);
                    if !(-192..=192).contains(&q) {
                        return Err(err("@q is -192 to 192"));
                    }
                    push(seq, K::QuantCount, q, 0);
                }
                "v" => {
                    let v = param.unwrap_or(256);
                    if !(0..=512).contains(&v) {
                        return Err(err("v is 0 to 512"));
                    }
                    push(seq, K::Volume, v, 0);
                }
                "@v" => {
                    let v = param.unwrap_or(64);
                    if !(0..=128).contains(&v) {
                        return Err(err("@v is 0 to 128"));
                    }
                    push(seq, K::FineVolume, v, 0);
                }
                "%" => push(seq, K::ModType, param.unwrap_or(NONE), 0),
                "@" => push(seq, K::ModParam, param.unwrap_or(NONE), 0),
                "@i" => push(seq, K::InPipe, param.unwrap_or(0), 0),
                "@o" => push(seq, K::OutPipe, param.unwrap_or(0), 0),
                "(" | ")" => {
                    let p = param.unwrap_or(1) * if e == "(" { 1 } else { -1 } * setting.volume_polarization;
                    match seq.ev.last_mut() {
                        Some(l) if matches!(l.k, K::VolumeShift | K::Volume) => l.data += p,
                        _ => push(seq, K::VolumeShift, p, 0),
                    }
                }
                "<" => octave += param.unwrap_or(1) * setting.octave_polarization,
                ">" => octave += -param.unwrap_or(1) * setting.octave_polarization,
                "&" => option(seq, K::Slur)?,
                "&&" => option(seq, K::SlurWeak)?,
                "*" => option(seq, K::Bend)?,
                "," => push(seq, K::Param, param.unwrap_or(NONE), 0),
                ";" => {
                    if !seq.ev.is_empty() {
                        seqs.push(Seq::default());
                        length = RES / 4;
                        octave = 5;
                        note_shift = 0;
                        stack.clear();
                    }
                }
                "$" => push(seq, K::RepAll, 0, 0),
                "[" => {
                    let n = param.unwrap_or(2);
                    if !(1..=65535).contains(&n) {
                        return Err(err("[ is 1 to 65535 times"));
                    }
                    push(seq, K::RepBegin, n, 0);
                    stack.push(seq.ev.len() - 1);
                }
                "|" => {
                    let Some(&b) = stack.last() else { return Err(err("a | is outside a repeat")) };
                    push(seq, K::RepBreak, 0, 0);
                    seq.ev.last_mut().unwrap().jump = b;
                }
                "]" => {
                    let Some(b) = stack.pop() else { return Err(err("a ] has no [")) };
                    push(seq, K::RepEnd, 0, 0);
                    let e = seq.ev.len() - 1;
                    seq.ev[e].jump = b;
                    seq.ev[b].jump = e;
                    if let Some(n) = param {
                        if !(1..=65535).contains(&n) {
                            return Err(err("] is 1 to 65535 times"));
                        }
                        seq.ev[b].data = n;
                    }
                }
                "!@ns" => note_shift += param.unwrap_or(0),
                "t" => push(seq, K::Tempo, param.unwrap_or(120), 0),
                // `/` is a letter of the expression, but not a command.
                _ => return Err(err("it isn't a command")),
            },
            Tok::System(t) => {
                if !seq.ev.is_empty() {
                    return Err(err("a # command belongs at the start of a sequence"));
                }
                let _ = t;
                seq.ev.push(E { k: K::System, data: 0, len: 0, jump: 0 });
            }
            Tok::Table => push(seq, K::Table, 0, 0),
        }
        last_is_length = was_length;
    }
    if !stack.is_empty() {
        return Err(err("a [ has no ]"));
    }
    seqs.retain(|s| !s.ev.is_empty() && s.ev[0].k != K::System);
    Ok(seqs)
}

/// The order a sequence's events run in, repeats and all: `MMLExecutor`.
struct Walk {
    pos: usize,
    counters: Vec<i64>,
}

impl Walk {
    fn new() -> Walk {
        Walk { pos: 0, counters: vec![] }
    }

    /// The next event to do, or none at the end.
    fn step(&mut self, ev: &[E]) -> Option<usize> {
        loop {
            let i = self.pos;
            let e = ev.get(i)?;
            self.pos = i + 1;
            match e.k {
                K::RepBegin => self.counters.push(e.data),
                K::RepBreak => {
                    if self.counters.last() == Some(&1) {
                        self.counters.pop();
                        self.pos = ev[e.jump].jump + 1;
                    }
                }
                K::RepEnd => {
                    let Some(c) = self.counters.last_mut() else { continue };
                    *c -= 1;
                    if *c == 0 {
                        self.counters.pop();
                    } else {
                        self.pos = e.jump + 1;
                    }
                }
                _ => return Some(i),
            }
        }
    }
}

/// `_abstructGlobalSequence`: each tempo and the tick it comes at, and
/// which of a sequence's tempo events it took out.
fn tempos(seqs: &[Seq]) -> (Vec<(i64, i64)>, Vec<Vec<bool>>) {
    let mut found: Vec<(i64, i64)> = vec![];
    let mut taken = vec![];
    for seq in seqs {
        let mut gone = vec![false; seq.ev.len()];
        let mut count = seq.globals;
        if count > 0 {
            let mut walk = Walk::new();
            let mut pos = 0i64;
            let mut only_rests = true;
            let mut steps = 0usize;
            while count > 0 || only_rests {
                steps += 1;
                if steps > MOST_STEPS {
                    break;
                }
                let Some(i) = walk.step(&seq.ev) else { break };
                let e = &seq.ev[i];
                match e.k {
                    K::Tempo if !gone[i] => {
                        gone[i] = true;
                        found.push((pos, e.data));
                        count = count.saturating_sub(1);
                    }
                    K::Table if !gone[i] => {
                        gone[i] = true;
                        count = count.saturating_sub(1);
                    }
                    _ if e.len != 0 => {
                        pos += e.len;
                        if e.k != K::Rest {
                            only_rests = false;
                        }
                    }
                    _ => {}
                }
            }
        }
        taken.push(gone);
    }
    (found, taken)
}

/// A note as the track played it, in ticks.
#[derive(Debug, Clone, Copy)]
struct Played {
    start: i64,
    end: i64,
    key: i64,
    vel: u8,
    drum: bool,
    program: u8,
}

/// What a track did besides its notes: controller changes.
struct Track {
    notes: Vec<Played>,
    pans: Vec<(i64, u8)>,
    expressions: Vec<(i64, u8)>,
    end: i64,
}

/// A module type and tone as a GM stand-in: (drum, program).
fn stand_in(module: i64, tone: i64) -> (bool, u8) {
    match module {
        0 if tone == 1 => (true, 0),
        2 | 10 => (true, 0),
        5 => match tone {
            0 => (false, 79),
            1 | 2 | 11 => (false, 81),
            3 | 4 => (false, 74),
            6 | 16..=21 | 24..=26 => (true, 0),
            128..=255 => (false, 81),
            _ => (false, 80),
        },
        6 | 15..=20 => (false, 4),
        9 => (false, 81),
        11 => (false, 25),
        _ => (false, 80),
    }
}

/// The tones a module type has (`SiMMLChannelSetting`'s channel counts).
fn tones(module: i64) -> i64 {
    match module {
        0 | 1 | 10 | 12 => 4,
        2 => 16,
        3 | 8 => 32,
        4 => 256,
        5 => 512,
        6 => 1,
        7 | 9 => 128,
        11 | 13 | 14 => 3,
        _ => 1,
    }
}

/// A drum for a note of noise, by pitch.
fn drum(key: i64) -> i32 {
    match key {
        k if k >= 80 => 42,
        k if k >= 60 => 38,
        _ => 36,
    }
}

/// `SiMMLTrack` playing a sequence: when its notes sound, in ticks.
#[allow(unused_assignments)]
fn play(seq: &Seq, gone: &[bool], setting: &Setting) -> Track {
    let mut t = Track { notes: vec![], pans: vec![], expressions: vec![], end: 0 };
    let mut walk = Walk::new();
    let mut time = 0i64;
    let mut ratio = 1.0f64;
    let mut quant_count = 0i64;
    let mut delay = 0i64;
    let mut velocity = 256i64;
    let mut shift = setting.default_shift;
    let mut transpose = 0i64;
    let mut module = 0i64;
    let mut tone = 0i64;
    let mut no_key_on = false;
    let mut note_on: Option<(i64, i64, u8, bool, u8)> = None; // key, start, velocity, drum, program
    let mut fire: Option<i64> = None;
    let mut pending_key = 0i64;
    let mut key_on_length = 0i64;
    let mut steps = 0usize;
    let mid = |v: i64| -> u8 {
        if v <= 0 {
            0
        } else {
            (v * 127 / 256).clamp(1, 127) as u8
        }
    };
    macro_rules! off {
        ($at:expr) => {{
            if let Some((key, start, vel, drum, program)) = note_on.take() {
                if $at > start && vel > 0 {
                    t.notes.push(Played { start, end: $at, key, vel, drum, program });
                }
            }
            fire = None;
        }};
    }
    // The note at `$at`: SiMMLTrack._keyOn().
    macro_rules! on {
        ($at:expr) => {{
            let key = pending_key + transpose;
            let (drum, program) = stand_in(module, tone);
            let vel = mid(velocity);
            if !no_key_on {
                if let Some((key, start, vel, drum, program)) = note_on.take() {
                    if $at > start && vel > 0 {
                        t.notes.push(Played { start, end: $at, key, vel, drum, program });
                    }
                }
                note_on = Some((key, $at, vel, drum, program));
            } else if let Some((k, start, v, d, p)) = note_on {
                // A slur: the same key goes on, another is a new note without a gap.
                if k != key || d != drum {
                    if $at > start && v > 0 {
                        t.notes.push(Played { start, end: $at, key: k, vel: v, drum: d, program: p });
                    }
                    note_on = Some((key, $at, v, drum, program));
                }
            }
            no_key_on = false;
            fire = if key_on_length > 0 { Some($at + key_on_length) } else { None };
        }};
    }
    // Whatever was due by `$to`: the counter reaching zero toggles the key.
    macro_rules! due {
        ($to:expr) => {{
            while let Some(f) = fire.filter(|&f| f <= $to) {
                if note_on.is_some() {
                    off!(f);
                } else {
                    on!(f);
                }
            }
        }};
    }
    while let Some(i) = walk.step(&seq.ev) {
        steps += 1;
        if steps > MOST_STEPS || t.notes.len() >= super::MOST_NOTES {
            break;
        }
        due!(time);
        let e = &seq.ev[i];
        // The parameters after a command.
        let params = |n: usize| -> (Vec<i64>, usize) {
            let mut p = vec![e.data];
            let mut last = i;
            while p.len() < n && seq.ev.get(last + 1).is_some_and(|x| x.k == K::Param) {
                last += 1;
                p.push(seq.ev[last].data);
            }
            (p, last)
        };
        match e.k {
            K::Tempo | K::Table if gone[i] => {}
            K::Rest => {
                no_key_on = false;
                time += e.len;
            }
            K::Note => {
                key_on_length = ((e.len as f64 * ratio) as i64 - quant_count - delay).max(1);
                pending_key = e.data;
                if delay > 0 {
                    off!(time);
                    fire = Some(time + delay);
                } else {
                    on!(time);
                }
                time += e.len;
            }
            K::Slur => {
                no_key_on = true;
                fire = None;
                time += e.len;
            }
            K::SlurWeak => {
                fire = None;
                time += e.len;
            }
            K::Bend => {
                if seq.ev.get(i + 1).is_some_and(|n| n.k == K::Note) {
                    no_key_on = true;
                    fire = None;
                    time += e.len;
                }
            }
            K::Quant => ratio = e.data as f64 / setting.max_quant_ratio as f64,
            K::QuantCount => {
                let (p, last) = params(2);
                walk.pos = last + 1;
                quant_count = if p[0] == NONE { 0 } else { p[0] * RES / 192 };
                delay = if p.get(1).is_none_or(|&d| d == NONE) { 0 } else { p[1] * RES / 192 };
            }
            K::Volume => velocity = (e.data << shift).clamp(0, 512),
            K::VolumeShift => velocity = (velocity + (e.data << shift)).clamp(0, 512),
            K::ModType => {
                let (p, last) = params(2);
                walk.pos = last + 1;
                module = if (0..21).contains(&p[0]) { p[0] } else { 5 };
                let n = p.get(1).copied().unwrap_or(NONE);
                tone = if (0..tones(module)).contains(&n) { n } else { 0 };
            }
            K::ModParam => {
                let (p, last) = params(16);
                walk.pos = last + 1;
                if p[0] != NONE {
                    tone = if (0..tones(module).max(1)).contains(&p[0]) || module == 5 { p[0] } else { 0 };
                }
            }
            K::User(u) => match u {
                "p" => {
                    let pan = if e.data == NONE { 0 } else { (e.data << 4) - 64 };
                    t.pans.push((time, (64 + pan).clamp(0, 127) as u8));
                }
                "@p" => {
                    let pan = if e.data == NONE { 0 } else { e.data };
                    t.pans.push((time, (64 + pan).clamp(0, 127) as u8));
                }
                "x" => {
                    let x = if e.data == NONE { 128 } else { e.data.clamp(0, 128) };
                    t.expressions.push((time, (x * 127 / 128) as u8));
                }
                "kt" => transpose = if e.data == NONE { 0 } else { e.data },
                "%v" => {
                    let (p, last) = params(8);
                    walk.pos = last + 1;
                    shift = if p.get(1).is_none_or(|&s| s == NONE) { 4 } else { p[1] };
                }
                _ => {}
            },
            _ => {}
        }
    }
    // The track ends: a note still sounding lasts to its end.
    due!(time);
    off!(time.max(note_on.map_or(0, |n| n.1)));
    t.end = time;
    t
}

/// Whether a text looks like SiON's MML: its own `#` commands, macros,
/// repeats or module types.
pub fn looks_like(text: &str) -> bool {
    let commands = ["#TMODE", "#VMODE", "#QUANT", "#SIGN", "#FPS", "#MACRO", "#REV", "#TABLE", "#FM", "#WAVB", "#SAMPLER", "#PCMWAVE", "#PCMVOICE", "#END", "#EFFECT"];
    text.lines().any(|l| {
        let l = l.trim_start();
        commands.iter().any(|c| l.starts_with(c) && !l[c.len()..].starts_with(|ch: char| ch.is_ascii_alphabetic()))
            || (l.starts_with('#') && l[1..].find('=').is_some_and(|e| e > 0 && l[1..1 + e].trim_end_matches('+').chars().all(|c| c.is_ascii_uppercase() || c == '-')))
    }) || (text.contains("![") && text.contains("!]"))
        || text.contains("%6@")
        || text.contains("@mask")
}

/// SiON's MML as a score and its title (`#TITLE`).
pub fn read(text: &str) -> Result<(Score, String), Error> {
    let (expanded, pre) = precompile(text).map_err(|why| super::bad(text, 0, why))?;
    let seqs = parse(text, &expanded, &pre.setting)?;
    let (found, taken) = tempos(&seqs);
    let setting = &pre.setting;
    // t: a tempo as SiON's `_calcBPMfromTcommand` has it.
    let bpm = |p: i64| -> f64 {
        match setting.tmode {
            0 => p as f64 * setting.tres,
            2 => {
                if p != 0 {
                    setting.tres / p as f64
                } else {
                    120.0
                }
            }
            _ => {
                if (0..256).contains(&p) {
                    setting.tres / (256 - p) as f64
                } else {
                    120.0
                }
            }
        }
    };
    let mut b = Builder::new();
    let whole = |t: i64| Frac::new(t, RES);
    let mut order = found.clone();
    order.sort_by_key(|x| x.0);
    // The first tempo at the start is SiON's default; the rest are changes.
    let mut initial = 120.0;
    for &(at, p) in &order {
        if at == 0 {
            initial = bpm(p);
        }
    }
    b.tempo(Frac::ZERO, initial);
    for &(at, p) in order.iter().filter(|x| x.0 > 0) {
        b.tempo(whole(at), bpm(p));
    }
    let mut end = 0i64;
    for (n, seq) in seqs.iter().enumerate() {
        let t = play(seq, &taken[n], setting);
        end = end.max(t.end);
        if t.notes.is_empty() {
            continue;
        }
        let name = format!("Track {}", n + 1);
        let mut melodic: Option<usize> = None;
        let mut drums: Option<usize> = None;
        let mut prog: Option<u8> = None;
        let mut notes = t.notes.clone();
        notes.sort_by_key(|x| x.start);
        for p in notes {
            if p.drum {
                let d = *drums.get_or_insert_with(|| b.part(&format!("{name} noise"), 0, true));
                b.note(d, whole(p.start), whole(p.end - p.start), drum(p.key), p.vel);
            } else {
                let m = *melodic.get_or_insert_with(|| b.part(&name, p.program, false));
                if prog != Some(p.program) {
                    if prog.is_some() {
                        b.program_at(m, whole(p.start), p.program);
                    } else {
                        b.parts[m].program = p.program;
                    }
                    prog = Some(p.program);
                }
                b.note(m, whole(p.start), whole(p.end - p.start), p.key as i32, p.vel);
            }
        }
        for part in [melodic, drums].into_iter().flatten() {
            let mut last = None;
            for &(at, v) in &t.pans {
                if last != Some(v) {
                    b.control(part, whole(at), 10, v);
                    last = Some(v);
                }
            }
            let mut last = None;
            for &(at, v) in &t.expressions {
                if last != Some(v) {
                    b.control(part, whole(at), 11, v);
                    last = Some(v);
                }
            }
        }
    }
    if b.count() == 0 {
        return Err(super::bad(text, 0, "there are no notes in it"));
    }
    b.end_at(whole(end));
    let title = pre.title.lines().next().unwrap_or("").trim().to_string();
    Ok((b.finish(), title))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each part's notes: (start, length, key, velocity) in ticks of 1920 a whole note.
    fn notes(text: &str) -> Vec<Vec<(i64, i64, u8, u8)>> {
        let (s, _) = read(text).unwrap();
        let k = i64::from(s.ppq) * 4;
        s.parts.iter().map(|p| p.notes.iter().map(|n| (i64::from(n.at) * RES / k, i64::from(n.length) * RES / k, n.key, n.velocity)).collect()).collect()
    }

    #[test]
    fn notes_octaves_and_the_gate() {
        // o5 is middle C; a quarter is 480 ticks; a track starts with the gate shut (full length).
        assert_eq!(notes("cdef"), vec![vec![(0, 480, 60, 127), (480, 480, 62, 127), (960, 480, 64, 127), (1440, 480, 65, 127)]]);
        // < goes up an octave and > down; dots; l; flats and sharps.
        assert_eq!(notes("l8 o4 c< c> c. d+ e-"), vec![vec![(0, 240, 48, 127), (240, 240, 60, 127), (480, 360, 48, 127), (840, 240, 51, 127), (1080, 240, 51, 127)]]);
        assert_eq!(notes("#REV;\no5 c< c")[0].iter().map(|n| n.2).collect::<Vec<_>>(), vec![60, 48]);
        // q is out of 8: q4 is half; @q takes 10 ticks a step off.
        assert_eq!(notes("q4 c")[0][0].1, 240);
        assert_eq!(notes("@q6 c")[0][0].1, 420);
        // v8 is half the volume (128 of 256).
        assert_eq!(notes("v8 c")[0][0].3, 63);
    }

    #[test]
    fn ties_slurs_and_rests() {
        // c4&c4 stays one note; c4&d4 changes pitch without a gap.
        assert_eq!(notes("c&c d")[0], vec![(0, 960, 60, 127), (960, 480, 62, 127)]);
        assert_eq!(notes("c&d")[0], vec![(0, 480, 60, 127), (480, 480, 62, 127)]);
        // c^ is a longer c; r^ a longer rest.
        assert_eq!(notes("c^ r^ d")[0], vec![(0, 960, 60, 127), (1920, 480, 62, 127)]);
        // A weak slur holds a note full length and starts the next again.
        assert_eq!(notes("q4 c&&d")[0], vec![(0, 480, 60, 127), (480, 240, 62, 127)]);
    }

    #[test]
    fn repeats_and_macros() {
        assert_eq!(notes("[3 c ]"), notes("c c c"));
        assert_eq!(notes("[c d|e]2"), notes("c d e c d"));
        assert_eq!(notes("#A=cd;\nA e"), notes("c d e"));
        assert_eq!(notes("#A=c;\n#B=A d;\nB"), notes("c d"));
        assert_eq!(notes("#A=cd;\nA(2)"), notes("d e"));
        assert_eq!(notes("#A-C=c;\nABC"), notes("ccc"));
        assert_eq!(notes("#A=c;\n#A+=d;\nA"), notes("cd"));
        assert_eq!(notes("![2 c !| d !] e"), notes("c d c e"));
        assert_eq!(notes("// c\nd /* e */ f"), notes("d f"));
    }

    #[test]
    fn tempo_tracks_and_titles() {
        let (s, title) = read("#TITLE My Song;\nt150 c; o6 e").unwrap();
        assert_eq!(title, "My Song");
        assert_eq!(s.bpm, 150.0);
        assert_eq!(s.parts.len(), 2);
        assert_eq!(s.parts[1].notes[0].key, 76);
        // A tempo in the second track comes where it falls.
        let (s, _) = read("c4 c4; r4 t90 c4").unwrap();
        assert!(s.kept.iter().any(|k| k.bytes == [0xff, 0x51, 3, 0x0a, 0x2c, 0x2b]));
        // #TMODE{unit=0.5}: t counts in half-beats.
        assert_eq!(read("#TMODE{unit=2};\nt60 c").unwrap().0.bpm, 30.0);
        // #SIGN{G}: F is sharp.
        assert_eq!(notes("#SIGN{G};\nf")[0][0].2, 66);
    }

    #[test]
    fn modules_become_programs_and_noise_drums() {
        let (s, _) = read("%5@1 c %9 d %2 o3 c").unwrap();
        assert_eq!(s.parts[0].program, 81);
        assert_eq!(s.parts.len(), 2);
        assert_eq!(s.parts[1].channel, crate::score::DRUMS);
        let (s, _) = read("%6 c").unwrap();
        assert_eq!(s.parts[0].program, 4);
        // Pan: p4 is the middle, p0 the left.
        let (s, _) = read("p0 c p8 c").unwrap();
        let pans: Vec<u8> = s.parts[0].kept.iter().filter(|k| k.bytes.len() == 3 && k.bytes[1] == 10).map(|k| k.bytes[2]).collect();
        assert_eq!(pans, vec![0, 127]);
    }

    #[test]
    fn mistakes_are_told() {
        assert!(read("c ]").is_err());
        // A [ with no ] is dropped at the next ; as SiON drops it, and plays once.
        assert_eq!(notes("[2 c"), notes("c"));
        assert!(read("c4096").is_err());
        assert!(read("&c").is_err());
        assert!(read("").is_err());
        assert!(read("r").is_err());
    }

    #[test]
    fn it_is_told_apart_from_other_mml() {
        assert!(looks_like("#TMODE{unit=2};\nt120 cde"));
        assert!(looks_like("#A=cde;\nA"));
        assert!(looks_like("c![2 de!]"));
        assert!(!looks_like("#TITLE x\nt120 cde;"));
        assert!(!looks_like("MML@cde,efg,;"));
        assert!(!looks_like("t120 l8 cdefgab"));
    }
}
