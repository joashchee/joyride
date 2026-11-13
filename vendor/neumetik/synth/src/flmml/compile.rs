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

//! FlMML's MML compiler, keeping every event its player needs (a copy of
//! neumetik-midi's `text::flmml`, extended): the tracks as FlMML's MTrack
//! records them, and the tones the song defines.

use super::Tables;

/// Ticks a whole note.
const WHOLE: i64 = 384;

// MStatus.
pub const EOT: u8 = 0;
pub const NOTE_ON: u8 = 2;
pub const NOTE_OFF: u8 = 3;
pub const TEMPO: u8 = 4;
pub const NOTE: u8 = 6;
pub const FORM: u8 = 7;
pub const ENV1_ATK: u8 = 8;
pub const ENV1_ADD: u8 = 9;
pub const ENV1_REL: u8 = 10;
pub const NOISE_FREQ: u8 = 11;
pub const PWM: u8 = 12;
pub const PAN: u8 = 13;
pub const FORMANT: u8 = 14;
pub const DETUNE: u8 = 15;
pub const LFO_FMSF: u8 = 16;
pub const LFO_DPWD: u8 = 17;
pub const LFO_DLTM: u8 = 18;
pub const LFO_TARGET: u8 = 19;
pub const LPF_SWTAMT: u8 = 20;
pub const LPF_FRQRES: u8 = 21;
pub const CLOSE: u8 = 22;
pub const VOL_MODE: u8 = 23;
pub const ENV2_ATK: u8 = 24;
pub const ENV2_ADD: u8 = 25;
pub const ENV2_REL: u8 = 26;
pub const INPUT: u8 = 27;
pub const OUTPUT: u8 = 28;
pub const EXPRESSION: u8 = 29;
pub const RING: u8 = 30;
pub const SYNC: u8 = 31;
pub const PORTAMENTO: u8 = 32;
pub const MIDIPORT: u8 = 33;
pub const MIDIPORTRATE: u8 = 34;
pub const BASENOTE: u8 = 35;
pub const POLY: u8 = 36;
pub const HW_LFO: u8 = 39;

const MAX_PIPE: i64 = 3;
const MAX_SYNCSOURCE: i64 = 3;
const MAX_POLYVOICE: i64 = 64;
const PULSE: i64 = 3;
const SAMPLE_RATE: f64 = 44100.0;

/// An event (MEvent): what, its two values, ticks since the one before,
/// and the tick it was made at.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ev {
    pub status: u8,
    pub d0: f64,
    pub d1: f64,
    pub delta: i64,
    pub tick: i64,
}

/// Ticks a second at a tempo: a quarter note is 96.
pub fn spt(bpm: f64) -> f64 {
    SAMPLE_RATE / (bpm * 96.0 / 60.0)
}

/// A track as FlMML's MTrack records it: events linked by their deltas.
#[derive(Debug, Clone)]
pub struct Track {
    pub events: Vec<Ev>,
    delta: i64,
    tick: i64,
    gate: f64,
    gate2: i64,
    chord: bool,
    chord_begin: i64,
    chord_end: i64,
    pub poly_found: bool,
}

impl Track {
    fn new() -> Track {
        Track { events: vec![], delta: 0, tick: 0, gate: 15.0 / 16.0, gate2: 0, chord: false, chord_begin: 0, chord_end: 0, poly_found: false }
    }
    fn make(&mut self, status: u8, d0: f64, d1: f64) -> Ev {
        let e = Ev { status, d0, d1, delta: self.delta, tick: self.tick };
        self.delta = 0;
        e
    }
    fn push_event(&mut self, e: Ev) {
        if !self.chord {
            self.events.push(e);
        } else {
            self.insert_event(e);
        }
    }
    fn rec(&mut self, status: u8, d0: f64, d1: f64) {
        let e = self.make(status, d0, d1);
        self.push_event(e);
    }
    /// After every event at the same tick.
    fn insert_event(&mut self, mut e: Ev) {
        let mut pre = 0;
        for i in 0..self.events.len() {
            let next = pre + self.events[i].delta;
            if next > e.tick {
                self.events[i].delta = next - e.tick;
                e.delta = e.tick - pre;
                self.events.insert(i, e);
                return;
            }
            pre = next;
        }
        e.delta = e.tick - pre;
        self.events.push(e);
    }
    /// Before every event at the same tick but a tempo.
    fn rec_global(&mut self, tick: i64, mut e: Ev) {
        let mut pre = 0;
        for i in 0..self.events.len() {
            let next = pre + self.events[i].delta;
            if next > tick || (next == tick && self.events[i].status != TEMPO) {
                self.events[i].delta = next - tick;
                e.delta = tick - pre;
                self.events.insert(i, e);
                return;
            }
            pre = next;
        }
        e.delta = tick - pre;
        self.events.push(e);
    }
    fn seek(&mut self, delta: i64) {
        self.delta += delta;
        self.tick += delta;
        self.chord_end = self.chord_end.max(self.tick);
    }
    fn note(&mut self, key: i64, len: i64, vel: i64, keyon: bool, keyoff: bool) {
        if keyon {
            self.rec(NOTE_ON, key as f64, vel as f64);
        } else {
            self.rec(NOTE, key as f64, 0.0);
        }
        if keyoff {
            let gate = (i64::from(super::osc::i32_of(len as f64 * self.gate)) - self.gate2).max(0);
            self.seek(gate);
            self.rec(NOTE_OFF, key as f64, vel as f64);
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
            self.delta = match self.events.last() {
                Some(e) => self.chord_end - e.tick,
                None => 0,
            };
            self.tick = self.chord_end;
            self.chord = false;
        }
    }
    pub fn rec_tempo(&mut self, tick: i64, tempo: f64) {
        let e = Ev { status: TEMPO, d0: tempo * 100.0, d1: 0.0, delta: 0, tick };
        self.rec_global(tick, e);
    }
    /// A rest of `ms` at 120 beats a minute (the tempo a track starts at).
    pub fn rest_msec(&mut self, ms: f64) {
        let len = super::osc::i32_of(ms * SAMPLE_RATE / (spt(120.0) * 1000.0));
        self.seek(i64::from(len));
    }
    pub fn rec_close(&mut self) {
        self.rec(CLOSE, 0.0, 0.0);
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
    keyoff: bool,
    gate: i64,
    max_gate: i64,
    form: i64,
    note_shift: i64,
    poly_voice: i64,
    title: String,
    max_pipe: i64,
    max_sync: i64,
    before_note: i64,
    portamento: bool,
    using_poly: bool,
    poly_force: bool,
    tables: Tables,
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
        if self.get_char() == '*' {
            self.before_note = note + self.octave * 12;
            self.portamento = true;
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
        let key = note + self.octave * 12;
        if self.portamento {
            let d = self.before_note - key;
            self.cur().rec(PORTAMENTO, d as f64, tick as f64);
        }
        let (vel, keyoff) = (self.velocity, self.keyoff);
        self.cur().note(key, tick, vel, keyon, keyoff);
        if self.portamento {
            self.cur().rec(PORTAMENTO, 0.0, 0.0);
            self.portamento = false;
        }
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

    fn rec(&mut self, status: u8, d0: i64, d1: i64) {
        self.cur().rec(status, d0 as f64, d1 as f64);
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
                self.rec(EXPRESSION, o, 0);
            }
            'e' => {
                self.next();
                let o = self.get_uint(1);
                if self.get_char() == ',' {
                    self.next();
                }
                let a = self.get_uint(0);
                let (mut d, mut s) = (64, 32);
                let mut points: Vec<(i64, i64)> = vec![];
                loop {
                    if self.get_char() == ',' {
                        self.next();
                    } else {
                        break;
                    }
                    let release_pos = self.letter - 1;
                    d = self.get_uint(d);
                    if self.get_char() == ',' {
                        self.next();
                    } else {
                        self.letter = release_pos;
                        break;
                    }
                    s = self.get_uint(s);
                    points.push((d, s));
                }
                if points.is_empty() {
                    points.push((d, s));
                }
                if self.get_char() == ',' {
                    self.next();
                }
                let r = self.get_uint(0);
                let (atk, add, rel) = if o == 1 { (ENV1_ATK, ENV1_ADD, ENV1_REL) } else { (ENV2_ATK, ENV2_ADD, ENV2_REL) };
                self.rec(atk, a, 0);
                for (t, l) in points {
                    self.rec(add, t, l);
                }
                self.rec(rel, r, 0);
            }
            'm' => {
                self.next();
                if self.get_char() == 'h' {
                    self.next();
                    let mut v = [0i64, 0, 0, 0, 0, 0, 1];
                    for (i, x) in v.iter_mut().enumerate() {
                        *x = self.get_uint(*x);
                        if i == 6 || self.get_char() != ',' {
                            break;
                        }
                        self.next();
                    }
                    let [w, f, pmd, amd, pms, ams, _] = v;
                    let data = ((w & 3) << 27) | ((f & 0xff) << 19) | ((pmd & 0x7f) << 12) | ((amd & 0x7f) << 5) | ((pms & 7) << 2) | (ams & 3);
                    self.rec(HW_LFO, data, 0);
                }
            }
            'n' => {
                self.next();
                if self.get_char() == 's' {
                    self.next();
                    self.note_shift += self.get_sint(0);
                } else {
                    let mut o = self.get_uint(0);
                    if !(0..=127).contains(&o) {
                        o = 0;
                    }
                    self.rec(NOISE_FREQ, o, 0);
                }
            }
            'w' => {
                self.next();
                let mut o = self.get_sint(50);
                o = if o < 0 { o.clamp(-99, -1) } else { o.clamp(1, 99) };
                self.rec(PWM, o, 0);
            }
            'p' => {
                self.next();
                if self.get_char() == 'l' {
                    self.next();
                    let o = self.get_uint(self.poly_voice).clamp(0, self.poly_voice);
                    self.rec(POLY, o, 0);
                    self.cur().poly_found = true;
                } else {
                    let o = self.get_uint(64).clamp(1, 127);
                    self.rec(PAN, o, 0);
                }
            }
            '\'' => {
                self.next();
                if let Some(o) = index_of(&self.s, &['\''], self.letter) {
                    let v: String = self.s[self.letter..o].iter().collect();
                    let vowel = match v.as_str() {
                        "a" => 0,
                        "e" => 1,
                        "i" => 2,
                        "o" => 3,
                        "u" => 4,
                        _ => -1,
                    };
                    self.rec(FORMANT, vowel, 0);
                    self.letter = o + 1;
                }
            }
            'd' => {
                self.next();
                let o = self.get_sint(0);
                self.rec(DETUNE, o, 0);
            }
            'l' => {
                let (mut wd, mut fm, mut sf, mut rv, mut dl, mut tm, mut sw) = (0, 1, 0, 1, 0, 0, 0);
                self.next();
                let dp = self.get_uint(0);
                if self.get_char() == ',' {
                    self.next();
                }
                wd = self.get_uint(wd);
                if self.get_char() == ',' {
                    self.next();
                    if self.get_char() == '-' {
                        rv = -1;
                        self.next();
                    }
                    fm = (self.get_uint(fm) + 1) * rv;
                    if self.get_char() == '-' {
                        self.next();
                        sf = self.get_uint(0);
                    }
                    if self.get_char() == ',' {
                        self.next();
                        dl = self.get_uint(dl);
                        if self.get_char() == ',' {
                            self.next();
                            tm = self.get_uint(tm);
                            if self.get_char() == ',' {
                                self.next();
                                sw = self.get_uint(sw);
                            }
                        }
                    }
                }
                self.rec(LFO_FMSF, fm, sf);
                self.rec(LFO_DPWD, dp, wd);
                self.rec(LFO_DLTM, dl, tm);
                self.rec(LFO_TARGET, sw, 0);
            }
            'f' => {
                self.next();
                let swt = self.get_sint(0);
                let (mut amt, mut frq, mut res) = (0, 0, 0);
                if self.get_char() == ',' {
                    self.next();
                    amt = self.get_sint(0);
                    if self.get_char() == ',' {
                        self.next();
                        frq = self.get_uint(0);
                        if self.get_char() == ',' {
                            self.next();
                            res = self.get_uint(0);
                        }
                    }
                }
                self.rec(LPF_SWTAMT, swt, amt);
                self.rec(LPF_FRQRES, frq, res);
            }
            'q' => {
                self.next();
                let g = self.get_uint(2) * 2;
                self.cur().gate2 = g.max(0);
            }
            'i' | 'r' => {
                self.next();
                let sens = self.get_uint(0);
                let mut a = 0;
                if self.get_char() == ',' {
                    self.next();
                    a = self.get_uint(0).min(self.max_pipe);
                }
                self.rec(if c == 'i' { INPUT } else { RING }, sens, a);
            }
            'o' => {
                self.next();
                let mode = self.get_uint(0);
                let mut a = 0;
                if self.get_char() == ',' {
                    self.next();
                    a = self.get_uint(0);
                    if a > self.max_pipe {
                        self.max_pipe = a;
                        if self.max_pipe >= MAX_PIPE {
                            self.max_pipe = MAX_PIPE;
                            a = MAX_PIPE;
                        }
                    }
                }
                self.rec(OUTPUT, mode, a);
            }
            's' => {
                self.next();
                let mode = self.get_uint(0);
                let mut a = 0;
                if self.get_char() == ',' {
                    self.next();
                    a = self.get_uint(0);
                    if mode == 1 {
                        if a > self.max_sync {
                            self.max_sync = a;
                            if self.max_sync >= MAX_SYNCSOURCE {
                                self.max_sync = MAX_SYNCSOURCE;
                                a = MAX_SYNCSOURCE;
                            }
                        }
                    } else if mode == 2 && a > self.max_sync {
                        a = self.max_sync;
                    }
                }
                self.rec(SYNC, mode, a);
            }
            'u' => {
                self.next();
                let mode = self.get_uint(0);
                match mode {
                    0 | 1 => self.rec(MIDIPORT, mode, 0),
                    2 => {
                        let mut rate = 0;
                        if self.get_char() == ',' {
                            self.next();
                            rate = self.get_uint(0).clamp(0, 127);
                        }
                        self.rec(MIDIPORTRATE, rate, 0);
                    }
                    3 => {
                        if self.get_char() == ',' {
                            self.next();
                            let oct = if self.get_char() != 'o' {
                                self.octave
                            } else {
                                self.next();
                                self.get_uint(0)
                            };
                            let base = match self.get_char() {
                                'c' => 0,
                                'd' => 2,
                                'e' => 4,
                                'f' => 5,
                                'g' => 7,
                                'a' => 9,
                                'b' => 11,
                                _ => -1,
                            };
                            let mut base = if base >= 0 {
                                self.next();
                                base + self.note_shift + self.get_key_sig() + oct * 12
                            } else {
                                self.get_uint(60)
                            };
                            base = base.clamp(0, 127);
                            self.rec(BASENOTE, base, 0);
                        }
                    }
                    _ => {}
                }
            }
            _ => {
                self.form = self.get_uint(self.form);
                let mut a = 0;
                if self.get_char() == '-' {
                    self.next();
                    a = self.get_uint(0);
                }
                let f = self.form;
                self.rec(FORM, f, a);
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
                let t = self.tempo;
                self.tracks[0].rec_tempo(at, t);
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
                let m = self.get_uint(1);
                self.rec(VOL_MODE, m, 0);
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
        for n in ["ARTIST", "COMMENT", "CODING", "PRAGMA"] {
            self.meta(n);
        }
        // #OPM@n { ... } and #OPN@n { ... } (over lines).
        let mut text: String = self.s.iter().collect();
        for (kind, opn) in [("#OPM@", false), ("#OPN@", true)] {
            let mut from = 0;
            while let Some(i) = text[from..].find(kind).map(|i| i + from) {
                let at_line_start = i == 0 || text[..i].ends_with('\n');
                let rest = &text[i + kind.len()..];
                let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                let after = rest[digits.len()..].trim_start_matches([' ', '\t']);
                let open = after.starts_with('{');
                let close = text[i..].find('}').map(|c| i + c + 1);
                match (at_line_start && !digits.is_empty() && open, close) {
                    (true, Some(end)) => {
                        let body_start = text[i..].find('{').map(|c| i + c + 1).unwrap_or(end - 1);
                        let body = text[body_start..end - 1].to_string();
                        self.tables.set_timbre(digits.parse().unwrap_or(0), opn, &body);
                        text.replace_range(i..end, "");
                    }
                    _ => from = i + kind.len(),
                }
            }
        }
        self.s = text.chars().collect();
        for v in self.meta("FMGAIN") {
            self.tables.fm_gain = 20.0 * f64::from(parse_int(&v)) / 127.0;
        }
        let poly = self.meta("USING POLY").join(" ").to_lowercase();
        let words: Vec<&str> = poly.split(' ').collect();
        if !poly.is_empty() {
            self.using_poly = true;
            self.poly_voice = i64::from(parse_int(words[0])).clamp(1, MAX_POLYVOICE);
            self.poly_force = words[1..].contains(&"force");
            if self.poly_voice <= 1 {
                self.using_poly = false;
                self.poly_force = false;
            }
        }
        for v in self.meta("WAV10") {
            let v: String = v.split(' ').collect();
            let mut args = v.split(',');
            let n = parse_int(args.next().unwrap_or("")).clamp(0, 31) as usize;
            let w = format!("{}00000000000000000000000000000000", args.next().unwrap_or("").to_lowercase());
            self.tables.set_gb_wave(n, &w[..32.min(w.len())]);
        }
        for v in self.meta("WAV13") {
            let v: String = v.split(' ').collect();
            let mut args = v.split(',');
            let n = parse_int(args.next().unwrap_or("")).clamp(0, 31) as usize;
            self.tables.set_wave(n, &args.next().unwrap_or("").to_lowercase());
        }
        for v in self.meta("WAV9") {
            let v: String = v.split(' ').collect();
            let args: Vec<&str> = v.split(',').collect();
            let n = parse_int(args.first().copied().unwrap_or("")).clamp(0, 15) as usize;
            let vol = parse_int(args.get(1).copied().unwrap_or("")).clamp(0, 127);
            let looped = parse_int(args.get(2).copied().unwrap_or("")).clamp(0, 1) == 1;
            self.tables.set_dpcm(n, vol, looped, args.get(3).copied().unwrap_or(""));
        }
    }
}

/// JavaScript's parseInt on a decimal: the leading digits (with a sign),
/// 0 when there are none.
pub fn parse_int(s: &str) -> i32 {
    let t = s.trim_start();
    let (neg, t) = match t.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    let d: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
    let v = d.parse::<i64>().unwrap_or(0).clamp(-(1 << 31), (1 << 31) - 1) as i32;
    if neg {
        -v
    } else {
        v
    }
}

/// A compiled song: its tracks (the first the tempo track), how long it
/// plays, which tracks are polyphonic and with how many voices, the pipes
/// and sync sources it uses, its tones, and its title.
pub struct Song {
    pub tracks: Vec<Track>,
    pub total_msec: f64,
    pub poly: Vec<Option<usize>>,
    pub pipes: usize,
    pub syncs: usize,
    pub tables: Tables,
    pub title: String,
}

/// FlMML's MML compiled as FlMML compiles it.
pub fn compile(text: &str) -> Song {
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
        keyoff: true,
        gate: 15,
        max_gate: 16,
        form: PULSE,
        note_shift: 0,
        poly_voice: 1,
        title: String::new(),
        max_pipe: 0,
        max_sync: 0,
        before_note: 0,
        portamento: false,
        using_poly: false,
        poly_force: false,
        tables: Tables::new(),
    };
    m.process_comment();
    m.process_macro();
    m.s.retain(|&c| !matches!(c, ' ' | '\u{3000}' | '\n' | '\r' | '\t' | '\u{c}'));
    m.process_repeat();
    m.process_group_notes();
    m.letter = 0;
    while m.letter < m.s.len() {
        m.first_letter();
        if m.tracks.iter().map(|t| t.events.len()).sum::<usize>() > 1 << 22 {
            break;
        }
    }
    if m.tracks.len() > 2 && m.tracks.last().is_some_and(|t| t.events.is_empty()) {
        m.tracks.pop();
    }
    // Conduct: the tempo track's changes into every track, and the end.
    let n = m.tracks.len();
    let (mut tick, mut sample) = (0i64, 0.0f64);
    let mut per = spt(120.0);
    let tempo_events = m.tracks[0].events.clone();
    for e in &tempo_events {
        tick += e.delta;
        sample += e.delta as f64 * per;
        if e.status == TEMPO {
            let t = e.d0.floor() / 100.0;
            per = spt(t);
            for j in 1..n {
                m.tracks[j].rec_tempo(tick, t);
            }
        }
    }
    let max_tick = (1..n).map(|j| m.tracks[j].tick).max().unwrap_or(0).max(0);
    let close = m.tracks[0].make(CLOSE, 0.0, 0.0);
    m.tracks[0].rec_global(max_tick, close);
    sample += (max_tick - tick) as f64 * per;
    m.tracks[0].rest_msec(3000.0);
    m.tracks[0].rec(EOT, 0.0, 0.0);
    sample += 3.0 * SAMPLE_RATE;
    let total_msec = sample * 1000.0 / SAMPLE_RATE;
    let mut poly = vec![None; n];
    for (i, t) in m.tracks.iter_mut().enumerate().skip(1) {
        if m.using_poly && (m.poly_force || t.poly_found) {
            poly[i] = Some(m.poly_voice as usize);
        }
        t.rest_msec(2000.0);
        t.rec_close();
    }
    let title = m.title.lines().next().unwrap_or("").trim().to_string();
    Song { tracks: m.tracks, total_msec, poly, pipes: m.max_pipe as usize + 1, syncs: m.max_sync as usize + 1, tables: m.tables, title }
}

