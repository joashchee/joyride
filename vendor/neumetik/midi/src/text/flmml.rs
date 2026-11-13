// Copyright 2026 ansiapps. Neumetik MIDI, MIT licensed: see LICENSE.
//
// The MML compiler here is a port of FlMML on HTML5's (src/flmml/MML.ts and
// the recording half of MTrack.ts, github.com/argentum384/flmml-on-html5):
//
// Copyright (c) 2015, argentum384
// Copyright (c) 2007, Takeshi OKUBO
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// * Redistributions of source code must retain the above copyright notice, this
//   list of conditions and the following disclaimer.
//
// * Redistributions in binary form must reproduce the above copyright notice,
//   this list of conditions and the following disclaimer in the documentation
//   and/or other materials provided with the distribution.
//
// * Neither the name of flmml-on-html5 nor the names of its
//   contributors may be used to endorse or promote products derived from
//   this software without specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
// FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
// DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
// SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
// CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
// OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
// OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

//! **FlMML**, the MML of FlMML (Flash, 2007) and FlMML on HTML5, read
//! by a port of its own compiler: comments (`/* */`, `` ` ` ``), the
//! `#` directives, macros (`$name{args}=...;`), repeats (`/:n ... / ... :/`),
//! group notes (`{cde}4`), then each track (`;` ends one): notes with `&`
//! ties and slurs, `*` portamento, `%` lengths in ticks (384 a whole note),
//! dots, `[ ]` chords, `q` and `@q` gates, `v`, `@v`, `( )`, `@x`, `@p`,
//! `o < >`, `ns`, `t`, and `@` tones. Every other command is read and
//! skipped as FlMML reads it.
//!
//! FlMML's own synthesizer isn't here: its tones are played by General
//! MIDI stand-ins (a pulse a square lead, a saw a saw lead, FM an
//! electric piano), and its noise tones as drums by pitch (low a kick,
//! middle a snare, high a hi-hat). Its envelopes, filters, LFOs and
//! detune aren't heard.

use super::{Builder, Frac};
use crate::score::Score;
use crate::Error;

/// Ticks a whole note.
const WHOLE: i64 = 384;

/// FlMML's tones (`@n`).
const SINE: i64 = 0;
const SAW: i64 = 1;
const TRIANGLE: i64 = 2;
const PULSE: i64 = 3;
const NOISE: i64 = 4;
const FC_PULSE: i64 = 5;
const FC_TRI: i64 = 6;
const FC_NOISE: i64 = 7;
const FC_S_NOISE: i64 = 8;
const FC_DPCM: i64 = 9;
const GB_WAVE: i64 = 10;
const GB_NOISE: i64 = 11;
const GB_S_NOISE: i64 = 12;
const WAVE: i64 = 13;
const OPM: i64 = 14;

/// What a track records, at its tick.
#[derive(Debug, Clone, PartialEq)]
enum Ev {
    /// A note starting (`true`) or a slur to it (`false`): key, velocity.
    On(i64, i64, bool),
    Off(i64),
    Form(i64),
    Expression(i64),
    Pan(i64),
}

/// A track as FlMML's MTrack records it.
#[derive(Debug, Clone)]
struct Track {
    events: Vec<(i64, Ev)>,
    tick: i64,
    gate: f64,
    gate2: i64,
    chord: bool,
    chord_begin: i64,
    chord_end: i64,
}

impl Track {
    fn new() -> Track {
        Track { events: vec![], tick: 0, gate: 15.0 / 16.0, gate2: 0, chord: false, chord_begin: 0, chord_end: 0 }
    }
    fn push(&mut self, e: Ev) {
        self.events.push((self.tick, e));
    }
    fn seek(&mut self, delta: i64) {
        self.tick += delta;
        self.chord_end = self.chord_end.max(self.tick);
    }
    fn note(&mut self, key: i64, len: i64, vel: i64, keyon: bool, keyoff: bool) {
        self.push(Ev::On(key, vel, keyon));
        if keyoff {
            let gate = (((len as f64) * self.gate) as i64 - self.gate2).max(0);
            self.seek(gate);
            self.push(Ev::Off(key));
            self.seek(len - gate);
            if self.chord {
                self.tick = self.chord_begin;
            }
        } else {
            self.seek(len);
        }
    }
    fn rest(&mut self, len: i64) {
        self.seek(len);
        if self.chord {
            self.chord_begin += len;
        }
    }
    fn chord_start(&mut self) {
        if !self.chord {
            self.chord = true;
            self.chord_begin = self.tick;
        }
    }
    fn chord_end(&mut self) {
        if self.chord {
            self.tick = self.chord_end;
            self.chord = false;
        }
    }
}

/// JavaScript's `indexOf` on characters.
fn index_of(s: &[char], pat: &[char], from: usize) -> Option<usize> {
    if pat.is_empty() {
        return Some(from.min(s.len()));
    }
    (from..s.len().saturating_sub(pat.len() - 1)).find(|&i| s[i..i + pat.len()] == *pat)
}

/// JavaScript's `substring`: clamped, and swapped when backwards.
fn substring(s: &[char], a: usize, b: usize) -> Vec<char> {
    let (a, b) = (a.min(s.len()), b.min(s.len()));
    let (a, b) = if a > b { (b, a) } else { (a, b) };
    s[a..b].to_vec()
}

/// The first run matching `[a-zA-Z_][a-zA-Z_0-9#+()]*`, as FlMML names macros.
fn name_in(s: &[char]) -> Option<Vec<char>> {
    let first = |c: char| c.is_ascii_alphabetic() || c == '_';
    let rest = |c: char| c.is_ascii_alphanumeric() || matches!(c, '_' | '#' | '+' | '(' | ')');
    let st = s.iter().position(|&c| first(c))?;
    let mut e = st + 1;
    while e < s.len() && rest(s[e]) {
        e += 1;
    }
    Some(s[st..e].to_vec())
}

fn is_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{3000}')
}

#[derive(Debug, Clone)]
struct Macro {
    id: Vec<char>,
    code: Vec<char>,
    /// Its parameters, longest name first, with their places.
    args: Vec<(Vec<char>, usize)>,
}

struct Mml {
    s: Vec<char>,
    letter: usize,
    tracks: Vec<Track>,
    track: usize,
    octave: i64,
    relative_dir: bool,
    velocity: i64,
    vel_detail: bool,
    vel_dir: bool,
    length: i64,
    tempo: f64,
    /// Tempo changes: when (in ticks) and quarter notes a minute.
    tempos: Vec<(i64, f64)>,
    keyoff: bool,
    gate: i64,
    max_gate: i64,
    form: i64,
    note_shift: i64,
    poly_voice: i64,
    title: String,
}

impl Mml {
    fn get_char(&self) -> char {
        self.s.get(self.letter).copied().unwrap_or('\0')
    }
    fn get_char_next(&mut self) -> char {
        let c = self.get_char();
        if self.letter < self.s.len() {
            self.letter += 1;
        }
        c
    }
    fn next(&mut self) {
        self.letter += 1;
    }
    fn len2tick(&self, len: i64) -> i64 {
        if len == 0 {
            self.length
        } else {
            WHOLE / len
        }
    }
    fn get_uint(&mut self, def: i64) -> i64 {
        let start = self.letter;
        let mut ret: i64 = 0;
        while let Some(d) = self.get_char().to_digit(10) {
            ret = ret.saturating_mul(10).saturating_add(i64::from(d));
            self.next();
        }
        if self.letter == start {
            def
        } else {
            ret
        }
    }
    fn get_unumber(&mut self, def: f64) -> f64 {
        let mut ret = self.get_uint(def as i64) as f64;
        if self.get_char() == '.' {
            self.next();
            let mut l = 1.0;
            loop {
                l *= 0.1;
                match self.get_char().to_digit(10) {
                    Some(d) => {
                        ret += f64::from(d) * l;
                        self.next();
                    }
                    None => break,
                }
            }
        }
        ret
    }
    fn get_sint(&mut self, def: i64) -> i64 {
        let mut sign = 1;
        match self.get_char() {
            '-' => {
                sign = -1;
                self.next();
            }
            '+' => self.next(),
            _ => {}
        }
        self.get_uint(def) * sign
    }
    fn get_dot(&mut self, mut tick: i64) -> i64 {
        let mut intick = tick;
        while self.get_char() == '.' {
            self.next();
            intick = intick.div_euclid(2);
            tick += intick;
        }
        tick
    }
    fn get_key_sig(&mut self) -> i64 {
        let mut k = 0;
        loop {
            match self.get_char() {
                '+' | '#' => k += 1,
                '-' => k -= 1,
                _ => return k,
            }
            self.next();
        }
    }
    fn cur(&mut self) -> &mut Track {
        &mut self.tracks[self.track]
    }
    fn create_track(&mut self) -> Track {
        self.octave = 4;
        self.velocity = 100;
        self.note_shift = 0;
        Track::new()
    }

    fn note(&mut self, note: i64) {
        let note = note + self.note_shift + self.get_key_sig();
        // A portamento's first note: the next note glides from it (not
        // heard here), and isn't played itself.
        if self.get_char() == '*' {
            self.next();
            return;
        }
        let mut tick = 0;
        let mut tie = false;
        let keyon = self.keyoff;
        self.keyoff = true;
        loop {
            let ticks_mode = self.get_char() == '%';
            if ticks_mode {
                self.next();
            }
            let len = self.get_uint(0);
            if tie && len == 0 {
                self.keyoff = false;
                break;
            }
            let t = if ticks_mode { len } else { self.len2tick(len) };
            tick += self.get_dot(t);
            tie = self.get_char() == '&';
            if !tie {
                break;
            }
            self.next();
        }
        let (key, vel, keyoff) = (note + self.octave * 12, self.velocity, self.keyoff);
        self.cur().note(key, tick, vel, keyon, keyoff);
    }

    fn rest(&mut self) {
        let ticks_mode = self.get_char() == '%';
        if ticks_mode {
            self.next();
        }
        let len = self.get_uint(0);
        let t = if ticks_mode { len } else { self.len2tick(len) };
        let t = self.get_dot(t);
        self.cur().rest(t);
    }

    fn atmark(&mut self) {
        let c = self.get_char();
        match c {
            'v' => {
                self.vel_detail = true;
                self.next();
                self.velocity = self.get_uint(self.velocity).min(127);
            }
            'x' => {
                self.next();
                let o = self.get_uint(127).min(127);
                self.cur().push(Ev::Expression(o));
            }
            'e' => {
                self.next();
                self.get_uint(1);
                if self.get_char() == ',' {
                    self.next();
                }
                self.get_uint(0);
                loop {
                    if self.get_char() == ',' {
                        self.next();
                    } else {
                        break;
                    }
                    let release_pos = self.letter - 1;
                    self.get_uint(64);
                    if self.get_char() == ',' {
                        self.next();
                    } else {
                        self.letter = release_pos;
                        break;
                    }
                    self.get_uint(32);
                }
                if self.get_char() == ',' {
                    self.next();
                }
                self.get_uint(0);
            }
            'm' => {
                self.next();
                if self.get_char() == 'h' {
                    self.next();
                    for i in 0..7 {
                        self.get_uint(0);
                        if i == 6 || self.get_char() != ',' {
                            break;
                        }
                        self.next();
                    }
                }
            }
            'n' => {
                self.next();
                if self.get_char() == 's' {
                    self.next();
                    self.note_shift += self.get_sint(0);
                } else {
                    self.get_uint(0);
                }
            }
            'w' => {
                self.next();
                self.get_sint(50);
            }
            'p' => {
                self.next();
                if self.get_char() == 'l' {
                    self.next();
                    self.get_uint(self.poly_voice);
                } else {
                    let o = self.get_uint(64).clamp(1, 127);
                    self.cur().push(Ev::Pan(o));
                }
            }
            '\'' => {
                self.next();
                if let Some(o) = index_of(&self.s, &['\''], self.letter) {
                    self.letter = o + 1;
                }
            }
            'd' => {
                self.next();
                self.get_sint(0);
            }
            'l' => {
                self.next();
                self.get_uint(0);
                if self.get_char() == ',' {
                    self.next();
                }
                self.get_uint(0);
                if self.get_char() == ',' {
                    self.next();
                    if self.get_char() == '-' {
                        self.next();
                    }
                    self.get_uint(1);
                    if self.get_char() == '-' {
                        self.next();
                        self.get_uint(0);
                    }
                    if self.get_char() == ',' {
                        self.next();
                        self.get_uint(0);
                        if self.get_char() == ',' {
                            self.next();
                            self.get_uint(0);
                            if self.get_char() == ',' {
                                self.next();
                                self.get_uint(0);
                            }
                        }
                    }
                }
            }
            'f' => {
                self.next();
                self.get_sint(0);
                if self.get_char() == ',' {
                    self.next();
                    self.get_sint(0);
                    if self.get_char() == ',' {
                        self.next();
                        self.get_uint(0);
                        if self.get_char() == ',' {
                            self.next();
                            self.get_uint(0);
                        }
                    }
                }
            }
            'q' => {
                self.next();
                let g = self.get_uint(2) * 2;
                self.cur().gate2 = g.max(0);
            }
            'i' | 'r' => {
                self.next();
                self.get_uint(0);
                if self.get_char() == ',' {
                    self.next();
                    self.get_uint(0);
                }
            }
            'o' | 's' => {
                self.next();
                self.get_uint(0);
                if self.get_char() == ',' {
                    self.next();
                    self.get_uint(0);
                }
            }
            'u' => {
                self.next();
                let mode = self.get_uint(0);
                if mode == 2 {
                    if self.get_char() == ',' {
                        self.next();
                        self.get_uint(0);
                    }
                } else if mode == 3 && self.get_char() == ',' {
                    self.next();
                    if self.get_char() == 'o' {
                        self.next();
                        self.get_uint(0);
                    }
                    if matches!(self.get_char(), 'c' | 'd' | 'e' | 'f' | 'g' | 'a' | 'b') {
                        self.next();
                        self.get_key_sig();
                    } else {
                        self.get_uint(60);
                    }
                }
            }
            _ => {
                self.form = self.get_uint(self.form);
                if self.get_char() == '-' {
                    self.next();
                    self.get_uint(0);
                }
                let f = self.form;
                self.cur().push(Ev::Form(f));
            }
        }
    }

    fn first_letter(&mut self) {
        let c = self.get_char_next();
        match c {
            'c' => self.note(0),
            'd' => self.note(2),
            'e' => self.note(4),
            'f' => self.note(5),
            'g' => self.note(7),
            'a' => self.note(9),
            'b' => self.note(11),
            'r' => self.rest(),
            'o' => self.octave = self.get_uint(self.octave).clamp(-2, 8),
            'v' => {
                self.vel_detail = false;
                let def = (self.velocity - 7) as f64 / 8.0;
                // FlMML passes (v - 7) / 8 as the default and multiplies it back.
                let v = if self.get_char().is_ascii_digit() { self.get_uint(0) as f64 } else { def };
                self.velocity = ((v * 8.0 + 7.0) as i64).clamp(0, 127);
            }
            '(' | ')' => {
                let i = self.get_uint(1);
                let up = c == '(' && self.vel_dir || c == ')' && !self.vel_dir;
                let step = if self.vel_detail { i } else { 8 * i };
                self.velocity = if up { (self.velocity + step).min(127) } else { (self.velocity - step).max(0) };
            }
            'l' => {
                let n = self.get_uint(0);
                self.length = self.len2tick(n);
                self.length = self.get_dot(self.length);
            }
            't' => {
                self.tempo = self.get_unumber(self.tempo).max(1.0);
                let at = self.tracks[self.track].tick;
                self.tempos.push((at, self.tempo));
            }
            'q' => {
                self.gate = self.get_uint(self.gate);
                let g = self.gate as f64 / self.max_gate as f64;
                self.cur().gate = g;
            }
            '<' => self.octave += if self.relative_dir { 1 } else { -1 },
            '>' => self.octave += if self.relative_dir { -1 } else { 1 },
            ';' => {
                self.keyoff = true;
                if !self.tracks[self.track].events.is_empty() {
                    self.track += 1;
                }
                let t = self.create_track();
                if self.track < self.tracks.len() {
                    self.tracks[self.track] = t;
                } else {
                    self.tracks.push(t);
                }
            }
            '@' => self.atmark(),
            'x' => {
                self.get_uint(1);
            }
            'n' => {
                if self.get_char() == 's' {
                    self.next();
                    self.note_shift = self.get_sint(self.note_shift);
                }
            }
            '[' => self.cur().chord_start(),
            ']' => self.cur().chord_end(),
            _ => {}
        }
    }

    // ---- Before the tracks: comments, directives, macros, repeats, groups.

    fn process_comment(&mut self) {
        self.letter = 0;
        let mut start: Option<usize> = None;
        while self.letter < self.s.len() {
            match self.get_char_next() {
                '/' if self.get_char() == '*' => {
                    if start.is_none() {
                        start = Some(self.letter - 1);
                    }
                    self.next();
                }
                '*' if self.get_char() == '/' => {
                    if let Some(st) = start.take() {
                        let end = (self.letter + 1).min(self.s.len());
                        self.s.drain(st..end);
                        self.letter = st;
                    }
                }
                _ => {}
            }
        }
        // Quoted for other programs: ` ... `.
        self.letter = 0;
        let mut start: Option<usize> = None;
        while self.letter < self.s.len() {
            if self.get_char_next() == '`' {
                match start.take() {
                    None => start = Some(self.letter - 1),
                    Some(st) => {
                        self.s.drain(st..self.letter.min(self.s.len()));
                        self.letter = st;
                    }
                }
            }
        }
    }

    /// Every line `#NAME ...` taken out, and what follows each name.
    fn meta(&mut self, name: &str) -> Vec<String> {
        let text: String = self.s.iter().collect();
        let mut out = vec![];
        let mut kept = String::new();
        for line in text.split_inclusive('\n') {
            let body = line.trim_end_matches(['\n', '\r']);
            let matched = body.strip_prefix('#').and_then(|b| directive(b, name));
            match matched {
                Some(value) => {
                    if let Some(v) = value {
                        out.push(v);
                    }
                    // The line goes; its line break stays.
                    kept.push_str(&line[body.len()..]);
                }
                None => kept.push_str(line),
            }
        }
        self.s = kept.chars().collect();
        out
    }

    fn process_macro(&mut self) {
        self.directives();
        self.letter = 0;
        let mut top = true;
        let mut table: Vec<Macro> = vec![];
        while self.letter < self.s.len() {
            let c = self.get_char_next();
            match c {
                '$' => {
                    let mut defined = false;
                    if top {
                        if let Some(mut last) = index_of(&self.s, &[';'], self.letter).filter(|&l| l > self.letter) {
                            if let Some(name_end) = index_of(&self.s, &['='], self.letter).filter(|&n| n > self.letter && n < last) {
                                defined = true;
                                let start = self.letter;
                                let mut argspos = index_of(&self.s, &['{'], 0).unwrap_or(usize::MAX);
                                if argspos == usize::MAX || argspos >= name_end {
                                    argspos = name_end;
                                }
                                let id_part = substring(&self.s, start, argspos);
                                if let Some(id) = name_in(&id_part) {
                                    let mut args: Vec<(Vec<char>, usize)> = vec![];
                                    if argspos < name_end {
                                        let close = index_of(&self.s, &['}'], argspos).unwrap_or(0);
                                        let argstr = substring(&self.s, argspos + 1, close);
                                        for (i, a) in argstr.split(|&c| c == ',').enumerate() {
                                            args.push((name_in(a).unwrap_or_default(), i));
                                        }
                                        args.sort_by_key(|a| std::cmp::Reverse(a.0.len()));
                                    }
                                    self.letter = name_end + 1;
                                    let mut c = self.get_char_next();
                                    while self.letter < last {
                                        if c == '$' {
                                            if !self.replace_macro(&table) && self.s.get(self.letter..self.letter + id.len()) == Some(&id[..]) {
                                                self.letter -= 1;
                                                let end = (self.letter + id.len() + 1).min(self.s.len());
                                                self.s.drain(self.letter..end);
                                            }
                                            last = index_of(&self.s, &[';'], self.letter).unwrap_or(self.s.len());
                                        }
                                        c = self.get_char_next();
                                    }
                                    let mut pos = 0;
                                    while pos < table.len() {
                                        if table[pos].id == id {
                                            table.remove(pos);
                                            continue;
                                        }
                                        if table[pos].id.len() < id.len() {
                                            break;
                                        }
                                        pos += 1;
                                    }
                                    let code = substring(&self.s, name_end + 1, last);
                                    table.insert(pos, Macro { id, code, args });
                                    let end = (last + 1).min(self.s.len());
                                    self.s.drain(start - 1..end);
                                    self.letter = start - 1;
                                }
                            }
                        }
                    }
                    if !defined {
                        self.replace_macro(&table);
                        top = false;
                    }
                }
                ';' => top = true,
                c if !is_space(c) => top = false,
                _ => {}
            }
        }
    }

    fn replace_macro(&mut self, table: &[Macro]) -> bool {
        for m in table {
            if self.s.get(self.letter..self.letter + m.id.len()) != Some(&m.id[..]) {
                continue;
            }
            let start = self.letter;
            let mut last = self.letter + m.id.len();
            let mut code = m.code.clone();
            self.letter += m.id.len();
            let mut c = self.get_char_next();
            while is_space(c) {
                c = self.get_char_next();
            }
            let mut args: Vec<Vec<char>> = vec![];
            if !m.args.is_empty() {
                if c == '{' {
                    c = self.get_char_next();
                    let mut quoted = false;
                    // (FlMML waits for ever on a quote that isn't closed.)
                    while (quoted || c != '}') && !(c == '\0' && self.letter >= self.s.len()) {
                        if c == '"' {
                            quoted = !quoted;
                        }
                        if c == '$' {
                            self.replace_macro(table);
                        }
                        c = self.get_char_next();
                    }
                    last = self.letter;
                    let argstr = substring(&self.s, start + m.id.len() + 1, last.saturating_sub(1));
                    let mut cur = vec![];
                    let mut q = false;
                    let mut pos = 0;
                    while pos < argstr.len() {
                        let ch = argstr[pos];
                        if !q && ch == '"' {
                            q = true;
                        } else if q && pos + 1 < argstr.len() && ch == '\\' && argstr[pos + 1] == '"' {
                            cur.push('"');
                            pos += 1;
                        } else if q && ch == '"' {
                            q = false;
                        } else if !q && ch == ',' {
                            args.push(std::mem::take(&mut cur));
                        } else {
                            cur.push(ch);
                        }
                        pos += 1;
                    }
                    args.push(cur);
                }
                // Each %name in the code, the longest name first.
                let mut i = 0;
                while i < code.len() {
                    for j in 0..args.len() {
                        if j >= m.args.len() {
                            break;
                        }
                        let (name, index) = &m.args[j];
                        let mut pat = vec!['%'];
                        pat.extend_from_slice(name);
                        if code.get(i..i + pat.len()) == Some(&pat[..]) {
                            let value = args.get(*index).cloned().unwrap_or_default();
                            code.splice(i..i + pat.len(), value.iter().copied());
                            i = (i + value.len()).saturating_sub(1);
                            break;
                        }
                    }
                    i += 1;
                }
            }
            let tail = self.s[last.min(self.s.len())..].to_vec();
            self.s.truncate(start - 1);
            self.s.extend_from_slice(&code);
            self.s.extend_from_slice(&tail);
            self.letter = start - 1;
            return true;
        }
        false
    }

    fn process_repeat(&mut self) {
        self.s = self.s.iter().flat_map(|c| c.to_lowercase()).collect();
        self.letter = 0;
        let (mut repeat, mut origin, mut start, mut last): (Vec<i64>, Vec<usize>, Vec<usize>, Vec<Option<usize>>) = (vec![], vec![], vec![], vec![]);
        let mut nest: isize = -1;
        let set = |v: &mut Vec<usize>, n: usize, x: usize| {
            if v.len() <= n {
                v.resize(n + 1, 0);
            }
            v[n] = x;
        };
        while self.letter < self.s.len() {
            match self.get_char_next() {
                '/' => {
                    if self.get_char() == ':' {
                        self.next();
                        nest += 1;
                        let n = nest as usize;
                        set(&mut origin, n, self.letter - 2);
                        if repeat.len() <= n {
                            repeat.resize(n + 1, 0);
                            last.resize(n + 1, None);
                        }
                        repeat[n] = self.get_uint(2);
                        set(&mut start, n, self.letter);
                        last[n] = None;
                    } else if nest >= 0 {
                        last[nest as usize] = Some(self.letter - 1);
                        self.s.remove(self.letter - 1);
                        self.letter -= 1;
                    }
                }
                ':' if self.get_char() == '/' && nest >= 0 => {
                    self.next();
                    let n = nest as usize;
                    let contents = substring(&self.s, start[n], self.letter - 2);
                    let mut out = substring(&self.s, 0, origin[n]);
                    let times = repeat[n].clamp(0, 1 << 12);
                    for i in 0..times {
                        match last[n] {
                            Some(l) if i == times - 1 => out.extend(substring(&self.s, start[n], l)),
                            _ => out.extend_from_slice(&contents),
                        }
                        if out.len() > 1 << 24 {
                            break;
                        }
                    }
                    let l = out.len();
                    out.extend_from_slice(&self.s[self.letter.min(self.s.len())..]);
                    self.s = out;
                    self.letter = l;
                    nest -= 1;
                }
                _ => {}
            }
        }
    }

    fn process_group_notes(&mut self) {
        let mut group: Option<usize> = None;
        let mut count = 0i64;
        let mut def_len = 96;
        self.letter = 0;
        let is_note = |c: char| ('a'..='g').contains(&c) || c == 'r';
        while self.letter < self.s.len() {
            let c = self.get_char_next();
            match c {
                'l' => {
                    let n = self.get_uint(0);
                    def_len = self.len2tick(n);
                    def_len = self.get_dot(def_len);
                }
                '{' => {
                    group = Some(self.letter - 1);
                    count = 0;
                }
                '}' => {
                    let repend = self.letter;
                    let Some(gs) = group else { continue };
                    let mut tick = 0;
                    loop {
                        let ticks_mode = self.get_char() == '%';
                        if ticks_mode {
                            self.next();
                        }
                        let len = self.get_uint(0);
                        if len == 0 {
                            if tick == 0 {
                                tick = def_len;
                            }
                            break;
                        }
                        let t = if ticks_mode { len } else { self.len2tick(len) };
                        tick += self.get_dot(t);
                        if self.get_char() != '&' {
                            break;
                        }
                        self.next();
                    }
                    let group_end = self.letter;
                    self.letter = gs + 1;
                    let mut out = substring(&self.s, 0, gs);
                    let mut tick2 = 0i64;
                    let tickdiv = tick as f64 / count.max(1) as f64;
                    let mut n = 1;
                    let mut on = false;
                    while self.letter < repend {
                        let c = self.get_char_next();
                        if !matches!(c, '+' | '#' | '-') {
                            if is_note(c) && !on {
                                on = true;
                            } else {
                                if on {
                                    let mut t = (n as f64 * tickdiv - tick2 as f64 + 0.5).floor() as i64;
                                    n += 1;
                                    tick2 += t;
                                    if tick2 > tick {
                                        t -= tick2 - tick;
                                        tick2 = tick;
                                    }
                                    out.push('%');
                                    out.extend(t.to_string().chars());
                                }
                                on = is_note(c);
                            }
                        }
                        if c != '}' {
                            out.push(c);
                        }
                    }
                    self.letter = out.len();
                    out.extend_from_slice(&self.s[group_end.min(self.s.len())..]);
                    self.s = out;
                    group = None;
                }
                c if is_note(c) => count += 1,
                _ => {}
            }
        }
    }
}

/// `#NAME` (spaces in the name match runs of white space) at the start of
/// a directive line: `Some(None)` bare, `Some(Some(value))` with a value.
fn directive(body: &str, name: &str) -> Option<Option<String>> {
    let mut rest = body;
    for (i, word) in name.split(' ').enumerate() {
        if i > 0 {
            let t = rest.trim_start_matches([' ', '\t']);
            if t.len() == rest.len() {
                return None;
            }
            rest = t;
        }
        rest = rest.strip_prefix(word)?;
    }
    if rest.trim().is_empty() {
        return Some(None);
    }
    if !rest.starts_with([' ', '\t']) {
        return None;
    }
    Some(Some(rest.trim_start_matches([' ', '\t']).to_string()))
}

impl Mml {
    /// The directives before the macros: `#OCTAVE REVERSE`, `#VELOCITY
    /// REVERSE`, the song's details, and the tone definitions FlMML's
    /// synthesizer takes (`#OPM@`, `#OPN@`, `#WAVn`, `#FMGAIN`, `#USING
    /// POLY`), taken out.
    fn directives(&mut self) {
        let lines: String = self.s.iter().collect();
        let has = |n: &str| lines.lines().any(|l| l.strip_prefix('#').and_then(|b| directive(b, n)).is_some_and(|v| v.is_none()));
        if has("OCTAVE REVERSE") {
            self.relative_dir = false;
        }
        if has("VELOCITY REVERSE") {
            self.vel_dir = false;
        }
        self.meta("OCTAVE REVERSE");
        self.meta("VELOCITY REVERSE");
        self.title = self.meta("TITLE").join("\r\n");
        for n in ["ARTIST", "COMMENT", "CODING", "PRAGMA", "FMGAIN", "USING POLY"] {
            let v = self.meta(n);
            if n == "USING POLY" {
                if let Some(first) = v.first().and_then(|v| v.split_whitespace().next()).and_then(|n| n.parse::<i64>().ok()) {
                    self.poly_voice = first.clamp(1, 64);
                }
            }
        }
        // #OPM@n { ... } and #OPN@n { ... } (over lines), and #WAV9/10/13 lines.
        let mut text: String = self.s.iter().collect();
        for kind in ["#OPM@", "#OPN@"] {
            let mut from = 0;
            while let Some(i) = text[from..].find(kind).map(|i| i + from) {
                let at_line_start = i == 0 || text[..i].ends_with('\n');
                let close = text[i..].find('}').map(|c| i + c + 1);
                match (at_line_start, close) {
                    (true, Some(end)) => text.replace_range(i..end, ""),
                    _ => from = i + kind.len(),
                }
            }
        }
        self.s = text.chars().collect();
        for n in ["WAV10", "WAV13", "WAV9"] {
            self.meta(n);
        }
    }
}

/// A GM program for a melodic tone.
fn program(form: i64) -> u8 {
    match form {
        SINE => 79,                      // Ocarina
        SAW => 81,                       // Lead 2 (sawtooth)
        TRIANGLE | FC_TRI => 74,         // Recorder
        PULSE | FC_PULSE => 80,          // Lead 1 (square)
        GB_WAVE | WAVE => 80,            //
        OPM => 4,                        // Electric Piano 1
        _ => 80,
    }
}

fn is_noise(form: i64) -> bool {
    matches!(form, NOISE | FC_NOISE | FC_S_NOISE | FC_DPCM | GB_NOISE | GB_S_NOISE)
}

/// A noise tone's drum, by the note it's played at.
fn drum(key: i64) -> i32 {
    match key {
        k if k >= 80 => 42,
        k if k >= 60 => 38,
        _ => 36,
    }
}

/// Whether a text looks like FlMML's MML: its directives, macros,
/// repeats, envelopes or tones past `@13`.
pub fn looks_like(text: &str) -> bool {
    let directives = ["#TITLE", "#ARTIST", "#COMMENT", "#CODING", "#OPM@", "#OPN@", "#WAV9", "#WAV10", "#WAV13", "#USING POLY", "#OCTAVE REVERSE", "#VELOCITY REVERSE", "#FMGAIN"];
    let lower = text.to_ascii_lowercase();
    directives.iter().any(|d| text.lines().any(|l| l.trim_start().starts_with(d)))
        || (lower.contains("/:") && lower.contains(":/"))
        || lower.contains("@e1,")
        || text.lines().any(|l| {
            let l = l.trim_start();
            l.starts_with('$') && l.contains('=') && l.trim_end().ends_with(';')
        })
}

/// FlMML's MML as a score and its title (`#TITLE`).
pub fn read(text: &str) -> Result<(Score, String), Error> {
    let mut m = Mml {
        s: text.chars().collect(),
        letter: 0,
        tracks: vec![Track::new(), Track::new()],
        track: 1,
        octave: 4,
        relative_dir: true,
        velocity: 100,
        vel_detail: true,
        vel_dir: true,
        length: WHOLE / 4,
        tempo: 120.0,
        tempos: vec![],
        keyoff: true,
        gate: 15,
        max_gate: 16,
        form: PULSE,
        note_shift: 0,
        poly_voice: 1,
        title: String::new(),
    };
    if m.s.len() > 4 << 20 {
        return Err(super::bad(text, 0, "the MML is too long"));
    }
    m.process_comment();
    m.process_macro();
    m.s.retain(|&c| !matches!(c, ' ' | '\u{3000}' | '\n' | '\r' | '\t' | '\u{c}'));
    m.process_repeat();
    m.process_group_notes();
    m.letter = 0;
    while m.letter < m.s.len() {
        m.first_letter();
        if m.tracks.iter().map(|t| t.events.len()).sum::<usize>() > super::MOST_NOTES * 2 {
            break;
        }
    }
    // Each track (the first is FlMML's tempo track) as parts: its notes,
    // and its noise as drums.
    let mut b = Builder::new();
    b.tempo(Frac::ZERO, 120.0);
    for &(at, t) in &m.tempos {
        b.tempo(Frac::new(at, WHOLE), t);
    }
    let whole = |t: i64| Frac::new(t, WHOLE);
    let mut end = 0i64;
    for (n, t) in m.tracks.iter().enumerate().skip(1) {
        if !t.events.iter().any(|e| matches!(e.1, Ev::On(..))) {
            continue;
        }
        let name = format!("Track {n}");
        let mut melodic: Option<usize> = None;
        let mut drums: Option<usize> = None;
        let mut form = PULSE;
        let mut prog: Option<u8> = None;
        // Sounding: key, start, velocity, drum.
        let mut open: Vec<(i64, i64, i64, bool)> = vec![];
        let mut events = t.events.clone();
        events.sort_by_key(|e| e.0);
        let close = |b: &mut Builder, o: (i64, i64, i64, bool), at: i64, melodic: Option<usize>, drums: Option<usize>| {
            let (key, from, vel, drum_note) = o;
            if vel <= 0 {
                return;
            }
            let len = (at - from).max(1);
            let part = if drum_note { drums } else { melodic };
            if let Some(p) = part {
                let k = if drum_note { drum(key) } else { key as i32 };
                b.note(p, whole(from), whole(len), k, vel as u8);
            }
        };
        for (at, e) in events {
            end = end.max(at);
            match e {
                Ev::Form(f) => form = f,
                Ev::Expression(x) => {
                    if let Some(p) = melodic {
                        b.control(p, whole(at), 11, x as u8);
                    }
                }
                Ev::Pan(x) => {
                    if let Some(p) = melodic {
                        b.control(p, whole(at), 10, (x - 1).clamp(0, 127) as u8);
                    }
                }
                Ev::On(key, vel, keyon) => {
                    // A slur, or a note still held by a tie into this one, ends here.
                    if !keyon || open.iter().any(|o| o.0 == key) {
                        let (ended, kept): (Vec<_>, Vec<_>) = open.into_iter().partition(|o| !keyon || o.0 == key);
                        open = kept;
                        for o in ended {
                            close(&mut b, o, at, melodic, drums);
                        }
                    }
                    let drum_note = is_noise(form);
                    if drum_note {
                        if drums.is_none() {
                            drums = Some(b.part(&format!("{name} noise"), 0, true));
                        }
                    } else {
                        let p = *melodic.get_or_insert_with(|| b.part(&name, program(form), false));
                        let want = program(form);
                        if prog != Some(want) {
                            if prog.is_some() {
                                b.program_at(p, whole(at), want);
                            } else {
                                b.parts[p].program = want;
                            }
                            prog = Some(want);
                        }
                    }
                    open.push((key, at, vel, drum_note));
                }
                Ev::Off(key) => {
                    // The note of that key, else (after a slur changed it) the last.
                    let i = open.iter().rposition(|o| o.0 == key).or_else(|| open.len().checked_sub(1));
                    if let Some(i) = i {
                        let o = open.remove(i);
                        close(&mut b, o, at, melodic, drums);
                    }
                }
            }
        }
        let last = t.tick.max(end);
        for o in std::mem::take(&mut open) {
            close(&mut b, o, last, melodic, drums);
        }
        end = end.max(t.tick);
    }
    if b.count() == 0 {
        return Err(super::bad(text, 0, "there are no notes in it"));
    }
    b.end_at(whole(end));
    let title = m.title.lines().next().unwrap_or("").trim().to_string();
    Ok((b.finish(), title))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each part's notes: (start, length, key, velocity) in ticks of 384 a whole note.
    fn notes(text: &str) -> Vec<Vec<(i64, i64, u8, u8)>> {
        let (s, _) = read(text).unwrap();
        let k = i64::from(s.ppq) * 4;
        s.parts.iter().map(|p| p.notes.iter().map(|n| (i64::from(n.at) * WHOLE / k, i64::from(n.length) * WHOLE / k, n.key, n.velocity)).collect()).collect()
    }

    #[test]
    fn notes_lengths_gates_and_octaves() {
        // o4c is key 48; quarter = 96 ticks; q15 of 16 by default.
        assert_eq!(notes("cdef"), vec![vec![(0, 90, 48, 100), (96, 90, 50, 100), (192, 90, 52, 100), (288, 90, 53, 100)]]);
        // FlMML's > goes down an octave (and < up), unless #OCTAVE REVERSE.
        assert_eq!(notes("q16 l8 o5 c d8. e%24 >c<c"), vec![vec![(0, 48, 60, 100), (48, 72, 62, 100), (120, 24, 64, 100), (144, 48, 48, 100), (192, 48, 60, 100)]]);
        // @q takes ticks (times two) off each note; v is 8 steps up from 7.
        assert_eq!(notes("q16 @q4 v15 c4 @v50 d4"), vec![vec![(0, 88, 48, 127), (96, 88, 50, 50)]]);
    }

    #[test]
    fn ties_slurs_chords_and_tracks() {
        // c4&c8 is one note; c4&d4 slurs into d.
        assert_eq!(notes("q16 c4&8 c4&d4"), vec![vec![(0, 144, 48, 100), (144, 96, 48, 100), (240, 96, 50, 100)]]);
        // A chord, then the next note after its longest.
        assert_eq!(notes("q16 [c2eg4] a4"), vec![vec![(0, 192, 48, 100), (0, 96, 52, 100), (0, 96, 55, 100), (192, 96, 57, 100)]]);
        let n = notes("t150 c; o5 e;");
        assert_eq!(n.len(), 2);
        assert_eq!(n[1][0].2, 64);
        assert_eq!(read("t150 c; e").unwrap().0.bpm, 150.0);
    }

    #[test]
    fn macros_repeats_groups_and_comments() {
        assert_eq!(notes("$a=cd;\n$b{x}=e%x;\n/* not this */ q16 $a $b{f} `nor this`"), notes("q16 cd e f"));
        assert_eq!(notes("q16 /:3 c / d :/"), notes("q16 cdcdc"));
        // Three notes in a quarter: 32 ticks each.
        assert_eq!(notes("q16 {cde}4"), vec![vec![(0, 32, 48, 100), (32, 32, 50, 100), (64, 32, 52, 100)]]);
        let (_, title) = read("#TITLE My Song\n#OPM@0 { 1 2\n 3 }\nc").unwrap();
        assert_eq!(title, "My Song");
        assert_eq!(notes("#OCTAVE REVERSE\nq16 c > c"), vec![vec![(0, 96, 48, 100), (96, 96, 60, 100)]]);
    }

    #[test]
    fn tones_become_programs_and_noise_drums() {
        let (s, _) = read("@1 c @4 o7 c o3 c").unwrap();
        assert_eq!(s.parts.len(), 2);
        assert_eq!(s.parts[0].program, 81);
        assert_eq!(s.parts[1].channel, crate::score::DRUMS);
        assert_eq!(s.parts[1].notes.iter().map(|n| n.key).collect::<Vec<_>>(), vec![42, 36]);
    }

    #[test]
    fn it_is_told_apart_from_other_mml() {
        assert!(looks_like("#TITLE x\nt120 cde;"));
        assert!(looks_like("$m=cde;\n$m"));
        assert!(!looks_like("MML@cde,efg,;"));
        assert!(!looks_like("t120 l8 cdefgab"));
    }
}
