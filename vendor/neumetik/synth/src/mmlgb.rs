// Copyright 2026 ansiapps. Neumetik Synth, proprietary: see LICENSE.
//
// A port of mmlgb (github.com/SimonLarsen/mmlgb), the MML parser (Java) and the GBDK sound driver (C) for
// the Game Boy:
//
// MIT License
//
// Copyright (c) 2017 Simon Larsen
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

//! mmlgb: Game Boy MML (channels `A` to `D`: two pulse, wave, noise), its compiler and its driver.
//!
//! `compile` is `Lexer.java`, `Parser.java` and `Song.getData` of mmlgb: MML in, the song's data out (the
//! header of channel, wave and macro offsets, the waves, the macros, the four channels), with mmlgb's own
//! behaviour kept where it is odd: a hex or binary number lexes as a decimal one then stray letters
//! (`0x10` is `0`, `10`), a character no token matches is skipped, a rest's length isn't split in 255s,
//! a macro can't use `@wave` and plays as channel A's. Where mmlgb hangs or crashes (a `s` command, a number
//! too big, a macro that isn't defined, a file ending inside a definition) this stops with an error, and a
//! source without a last line break gets one (mmlgb wants it after a `}`).
//!
//! `Player` is `music.c` run on `gb.rs`: the timer interrupt (4096 Hz, reload `TMA`) calls the four
//! channels' updates, which write the sound registers; between ticks the APU renders. Not heard on a
//! console, nor run against a reference (mmlgb's driver needs SDCC and GBDK); the tests check the data
//! by hand and the pitches by ear's arithmetic.

use crate::gb::Gb;
use crate::replay::{Bufs, Driver, Tune};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

/// The channels' names.
pub const CHANNEL_NAMES: [&str; 4] = ["Pulse 1", "Pulse 2", "Wave", "Noise"];

// The commands of `notes.h` (and `Song.CMD`).
const T_LENGTH: u8 = 0;
const T_OCTAVE: u8 = 1;
const T_OCT_UP: u8 = 2;
const T_OCT_DOWN: u8 = 3;
const T_VOL: u8 = 4;
const T_ENV: u8 = 5;
const T_WAVEDUTY: u8 = 6;
const T_PAN: u8 = 7;
const T_PORTAMENTO: u8 = 8;
const T_SLIDE: u8 = 9;
const T_VIBRATO: u8 = 10;
const T_VIBRATO_DELAY: u8 = 11;
const T_REP_START: u8 = 12;
const T_REP_END: u8 = 13;
const T_LOOP: u8 = 14;
const T_PITCH_OFFSET: u8 = 15;
const T_TEMPO: u8 = 16;
const T_NOISE_STEP: u8 = 17;
const T_WAVE: u8 = 18;
const T_MACRO: u8 = 19;
const T_EOF: u8 = 20;
const T_C: u8 = 21;
const T_B: u8 = 32;
const T_REST: u8 = 33;
const T_WAIT: u8 = 34;

const BEAT_STEPS: i32 = 48;
const BAR_STEPS: i32 = 4 * BEAT_STEPS;
const TIMA_SPEED: f32 = 4096.0;
const WAVE_VOLUMES: [i32; 4] = [0, 96, 64, 32];

/// Where mmlgb's parser stopped.
#[derive(Clone, Debug, PartialEq)]
pub struct Error {
    pub what: String,
    pub line: usize,
    pub pos: usize,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}, column {}: {}", self.line, self.pos, self.what)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    Number,
    Channel,
    Note,
    Sharp,
    Dash,
    Command,
    Dot,
    Comma,
    Tie,
    Macro,
    Assign,
    LCurly,
    RCurly,
    LBracket,
    RBracket,
    Newline,
    Eof,
}

#[derive(Clone, Debug)]
struct Token {
    kind: Kind,
    data: String,
    line: usize,
    pos: usize,
}

/// `Lexer.lex`: the alternatives in the order the Java regex has them; what none matches is skipped.
fn lex(input: &str) -> Vec<Token> {
    let c: Vec<char> = input.chars().collect();
    let mut out = Vec::new();
    let (mut line, mut linepos, mut i) = (1usize, 0usize, 0usize);
    let tok = |kind, data: String, line, pos| Token { kind, data, line, pos };
    while i < c.len() {
        let pos = i - linepos + 1;
        let ch = c[i];
        match ch {
            ';' => {
                while i < c.len() && !matches!(c[i], '\n' | '\r' | '\u{85}' | '\u{2028}' | '\u{2029}') {
                    i += 1;
                }
            }
            '0'..='9' => {
                let s = i;
                while i < c.len() && c[i].is_ascii_digit() {
                    i += 1;
                }
                out.push(tok(Kind::Number, c[s..i].iter().collect(), line, pos));
            }
            'A'..='D' => {
                out.push(tok(Kind::Channel, ch.to_string(), line, pos));
                i += 1;
            }
            'c' | 'd' | 'e' | 'f' | 'g' | 'a' | 'b' => {
                out.push(tok(Kind::Note, ch.to_string(), line, pos));
                i += 1;
            }
            '#' | '+' => {
                out.push(tok(Kind::Sharp, ch.to_string(), line, pos));
                i += 1;
            }
            '-' => {
                out.push(tok(Kind::Dash, ch.to_string(), line, pos));
                i += 1;
            }
            'r' | 'w' | 'o' | '<' | '>' | 'l' | 'v' | 't' | 'y' | 's' | 'L' => {
                out.push(tok(Kind::Command, ch.to_string(), line, pos));
                i += 1;
            }
            '.' => {
                out.push(tok(Kind::Dot, ".".into(), line, pos));
                i += 1;
            }
            ',' => {
                out.push(tok(Kind::Comma, ",".into(), line, pos));
                i += 1;
            }
            '^' => {
                out.push(tok(Kind::Tie, "^".into(), line, pos));
                i += 1;
            }
            '@' => {
                let rest: String = c[i..(i + 5).min(c.len())].iter().collect();
                match ["@@", "@po", "@p", "@s", "@ns", "@ve", "@v", "@wave", "@wd"].iter().find(|m| rest.starts_with(**m)) {
                    Some(m) => {
                        out.push(tok(Kind::Macro, (*m).into(), line, pos));
                        i += m.len();
                    }
                    None => i += 1,
                }
            }
            '=' => {
                out.push(tok(Kind::Assign, "=".into(), line, pos));
                i += 1;
            }
            '{' => {
                out.push(tok(Kind::LCurly, "{".into(), line, pos));
                i += 1;
            }
            '}' => {
                out.push(tok(Kind::RCurly, "}".into(), line, pos));
                i += 1;
            }
            '[' => {
                out.push(tok(Kind::LBracket, "[".into(), line, pos));
                i += 1;
            }
            ']' => {
                out.push(tok(Kind::RBracket, "]".into(), line, pos));
                i += 1;
            }
            '\n' => {
                out.push(tok(Kind::Newline, "\n".into(), line, pos));
                line += 1;
                i += 1;
                linepos = i;
            }
            _ => i += 1,
        }
    }
    out.push(tok(Kind::Eof, String::new(), line, 0));
    out
}

/// `Song.java`: what the parser collects.
#[derive(Default)]
struct Collected {
    channel: [Vec<i32>; 4],
    waves: Vec<Vec<i32>>,
    wave_map: HashMap<i32, i32>,
    macros: Vec<Vec<i32>>,
    macro_map: HashMap<i32, i32>,
}

impl Collected {
    fn add(&mut self, active: [bool; 4], v: i32) {
        for (c, on) in active.iter().enumerate() {
            if *on {
                self.channel[c].push(v);
            }
        }
    }

    fn add_all(&mut self, active: [bool; 4], v: &[i32]) {
        for (c, on) in active.iter().enumerate() {
            if *on {
                self.channel[c].extend_from_slice(v);
            }
        }
    }

    /// `Song.getData`.
    fn data(&self) -> Vec<u8> {
        let wave_start = (4 + 1 + self.macros.len()) * 2;
        let mut pos = wave_start + self.waves.len() * 16;
        let macro_start: Vec<usize> = self
            .macros
            .iter()
            .map(|m| {
                let at = pos;
                pos += m.len() + 1;
                at
            })
            .collect();
        let c1 = pos;
        let c2 = c1 + self.channel[0].len();
        let c3 = c2 + self.channel[1].len();
        let c4 = c3 + self.channel[2].len();
        let mut d: Vec<u8> = Vec::new();
        for v in [c1, c2, c3, c4, wave_start].into_iter().chain(macro_start) {
            d.push(v as u8);
            d.push((v >> 8) as u8);
        }
        for w in &self.waves {
            for j in (0..32).step_by(2) {
                d.push(((w[j] << 4) | w[j + 1]) as u8);
            }
        }
        for m in &self.macros {
            d.extend(m.iter().map(|&v| v as u8));
            d.push(T_EOF);
        }
        for c in &self.channel {
            d.extend(c.iter().map(|&v| v as u8));
        }
        d
    }
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    song: Collected,
    // The song's own macros while a macro is parsed into a song of its own.
    root_macros: Vec<Vec<i32>>,
    root_macro_map: HashMap<i32, i32>,
    in_macro: bool,
}

type R<T> = Result<T, Error>;

impl Parser {
    fn next(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn err<T>(&self, what: impl Into<String>) -> R<T> {
        let t = self.next();
        Err(Error { what: what.into(), line: t.line, pos: t.pos })
    }

    fn eat(&mut self) -> R<()> {
        self.pos += 1;
        if self.pos >= self.tokens.len() {
            self.pos = self.tokens.len() - 1;
            return self.err("End of file reached.");
        }
        Ok(())
    }

    fn expect(&mut self, kind: Kind, what: &str) -> R<()> {
        if self.next().kind != kind {
            let found = self.next().data.clone();
            return self.err(format!("Found token {found}. Expected {what}."));
        }
        self.eat()
    }

    fn skip_newlines(&mut self) -> R<()> {
        while self.next().kind == Kind::Newline {
            self.eat()?;
        }
        Ok(())
    }

    fn int(&self, s: &str) -> R<i32> {
        match s.parse::<i32>() {
            Ok(v) => Ok(v),
            Err(_) => self.err(format!("Invalid number \"{s}\".")),
        }
    }

    /// The number `next` is, eaten.
    fn number(&mut self, what: &str) -> R<i32> {
        if self.next().kind != Kind::Number {
            return self.err(what);
        }
        let v = self.int(&self.next().data.clone())?;
        Ok(v)
    }

    fn parse(&mut self) -> R<()> {
        while self.next().kind != Kind::Eof {
            match self.next().kind {
                Kind::Channel => self.channel_line()?,
                Kind::Macro => {
                    if self.next().data == "@wave" {
                        self.wave_data()?;
                    } else if self.next().data == "@@" {
                        self.macro_data()?;
                    } else {
                        return self.err(format!("Unexpected token ({:?}, {}).", self.next().kind, self.next().data));
                    }
                }
                Kind::Newline => self.eat()?,
                _ => return self.err(format!("Unexpected token ({:?}, {}).", self.next().kind, self.next().data)),
            }
        }
        for c in 0..4 {
            self.song.channel[c].push(i32::from(T_EOF));
        }
        Ok(())
    }

    fn wave_data(&mut self) -> R<()> {
        self.eat()?;
        let id = self.number("Expected wave data id.")?;
        self.eat()?;
        self.expect(Kind::Assign, "=")?;
        self.expect(Kind::LCurly, "{")?;
        let mut samples = Vec::with_capacity(32);
        for _ in 0..32 {
            self.skip_newlines()?;
            let s = self.number("Invalid wave sample. Expected number.")?;
            self.eat()?;
            if s > 15 {
                return self.err(format!("Invalid wave sample {s}. Expected 0 to 15."));
            }
            samples.push(s);
            self.skip_newlines()?;
        }
        self.expect(Kind::RCurly, "}")?;
        self.expect(Kind::Newline, "Line break")?;
        self.song.wave_map.insert(id, self.song.waves.len() as i32);
        self.song.waves.push(samples);
        Ok(())
    }

    fn macro_data(&mut self) -> R<()> {
        self.eat()?;
        let id = self.number("Expected macro id.")?;
        self.eat()?;
        self.expect(Kind::Assign, "=")?;
        self.expect(Kind::LCurly, "{")?;
        let mut body: Vec<Token> = Vec::new();
        while self.next().kind != Kind::RCurly {
            self.skip_newlines()?;
            body.push(self.next().clone());
            self.eat()?;
            self.skip_newlines()?;
        }
        let end = self.next().clone();
        body.push(Token { kind: Kind::Newline, data: "\n".into(), line: end.line, pos: end.pos });
        self.expect(Kind::RCurly, "}")?;
        self.expect(Kind::Newline, "Line break")?;

        // Parse the body as channel A of a song of its own.
        let tokens = std::mem::replace(&mut self.tokens, body);
        let pos = std::mem::replace(&mut self.pos, 0);
        let mut mine = Collected::default();
        std::mem::swap(&mut self.song, &mut mine);
        self.root_macros = mine.macros.clone();
        self.root_macro_map = mine.macro_map.clone();
        self.in_macro = true;
        let r = self.commands([true, false, false, false]);
        self.in_macro = false;
        std::mem::swap(&mut self.song, &mut mine);
        self.tokens = tokens;
        self.pos = pos;
        r?;
        let data = std::mem::take(&mut mine.channel[0]);
        self.song.macro_map.insert(id, self.song.macros.len() as i32);
        self.song.macros.push(data);
        Ok(())
    }

    fn channel_line(&mut self) -> R<()> {
        let mut active = [false; 4];
        while self.next().kind == Kind::Channel {
            let c = (self.next().data.as_bytes()[0] - b'A') as usize;
            active[c] = true;
            self.eat()?;
        }
        self.commands(active)
    }

    fn commands(&mut self, active: [bool; 4]) -> R<()> {
        const NOTE_NAMES: [&str; 12] = ["c", "cs", "d", "ds", "e", "f", "fs", "g", "gs", "a", "as", "b"];
        while self.next().kind != Kind::Newline {
            match self.next().kind {
                Kind::Note => {
                    let mut note = i32::from(T_C);
                    for (i, n) in NOTE_NAMES.iter().enumerate() {
                        if self.next().data == *n {
                            note = i32::from(T_C) + i as i32;
                            break;
                        }
                    }
                    self.eat()?;
                    if self.next().kind == Kind::Sharp {
                        note += 1;
                        self.eat()?;
                    } else if self.next().kind == Kind::Dash {
                        note -= 1;
                        self.eat()?;
                    }
                    if note == i32::from(T_C) - 1 {
                        note = i32::from(T_B);
                    } else if note == i32::from(T_B) + 1 {
                        note = i32::from(T_C);
                    }
                    let length = self.length(false)?;
                    if length == 0 {
                        self.song.add(active, note);
                    } else {
                        self.song.add(active, note | 0x80);
                        self.song.add(active, length % 255);
                        for _ in 0..length / 255 {
                            self.song.add(active, i32::from(T_WAIT) | 0x80);
                            self.song.add(active, 255);
                        }
                    }
                }
                Kind::Command => self.command(active)?,
                Kind::Tie => {
                    self.eat()?;
                    let length = self.length(false)?;
                    if length == 0 {
                        self.song.add(active, i32::from(T_WAIT));
                    } else {
                        self.song.add(active, i32::from(T_WAIT) | 0x80);
                        self.song.add(active, length);
                    }
                }
                Kind::Macro => self.macro_command(active)?,
                Kind::LBracket => {
                    self.eat()?;
                    self.song.add(active, i32::from(T_REP_START));
                }
                Kind::RBracket => {
                    self.eat()?;
                    let reps = self.number("Expected repetition count.")?;
                    if reps < 2 {
                        return self.err("Invalid repetition count. Must be >= 2.");
                    }
                    self.eat()?;
                    self.song.add(active, i32::from(T_REP_END));
                    self.song.add(active, reps);
                }
                Kind::Eof => break,
                _ => return self.err(format!("Unexpected token ({:?}, {}).", self.next().kind, self.next().data)),
            }
        }
        Ok(())
    }

    fn command(&mut self, active: [bool; 4]) -> R<()> {
        let cmd = self.next().data.clone();
        match cmd.as_str() {
            "r" | "w" => {
                self.eat()?;
                let length = self.length(false)?;
                let t = if cmd == "r" { T_REST } else { T_WAIT };
                if length == 0 {
                    self.song.add(active, i32::from(t));
                } else {
                    self.song.add(active, i32::from(t) | 0x80);
                    self.song.add(active, length);
                    for _ in 0..length / 255 {
                        self.song.add(active, i32::from(T_WAIT) | 0x80);
                        self.song.add(active, 255);
                    }
                }
            }
            "o" => {
                self.eat()?;
                let octave = self.number("Expected number after octave command.")?;
                self.eat()?;
                self.song.add(active, i32::from(T_OCTAVE));
                self.song.add(active, octave);
            }
            "<" => {
                self.eat()?;
                self.song.add(active, i32::from(T_OCT_DOWN));
            }
            ">" => {
                self.eat()?;
                self.song.add(active, i32::from(T_OCT_UP));
            }
            "l" => {
                self.eat()?;
                let at = self.next().clone();
                let length = self.length(true)?;
                if length > 255 {
                    return Err(Error { what: "Length overflow. Lengths more than 255 frames not allowed for l command.".into(), line: at.line, pos: at.pos });
                }
                self.song.add(active, i32::from(T_LENGTH));
                self.song.add(active, length);
            }
            "v" => {
                self.eat()?;
                let volume = self.number("Invalid volume. Expected number.")?;
                if active[2] && volume > 3 {
                    return self.err("Invalid volume for wave channel. Expected 0 to 3.");
                }
                if volume > 15 {
                    return self.err("Invalid volume value. Expected 0 to 15.");
                }
                self.eat()?;
                for c in 0..4 {
                    if active[c] {
                        self.song.channel[c].push(i32::from(T_VOL));
                        self.song.channel[c].push(if c == 2 { WAVE_VOLUMES[volume as usize] } else { volume << 4 });
                    }
                }
            }
            "t" => {
                self.eat()?;
                let bpm = self.number("Invalid tempo command. Expected number.")?;
                self.eat()?;
                // Java's float arithmetic and Math.round.
                let ups = bpm as f32 / 60.0 * BEAT_STEPS as f32;
                let m = (TIMA_SPEED / ups + 0.5).floor() as i32;
                self.song.add(active, i32::from(T_TEMPO));
                self.song.add(active, 255i32.wrapping_sub(m));
            }
            "y" => {
                self.eat()?;
                let mut negative = false;
                if self.next().kind == Kind::Dash {
                    self.eat()?;
                    negative = true;
                }
                let pan = self.number("Invalid panning command. Expected number.")?;
                self.eat()?;
                let val = if pan == 0 {
                    17
                } else if negative {
                    16
                } else {
                    1
                };
                self.song.add(active, i32::from(T_PAN));
                self.song.add(active, val);
            }
            "L" => {
                self.eat()?;
                self.song.add(active, i32::from(T_LOOP));
            }
            // mmlgb's parser never moves past an `s` (it loops for ever).
            _ => return self.err(format!("Unexpected token ({:?}, {}).", self.next().kind, cmd)),
        }
        Ok(())
    }

    fn macro_command(&mut self, active: [bool; 4]) -> R<()> {
        let m = self.next().data.clone();
        match m.as_str() {
            "@wave" => {
                if active[0] || active[1] || active[3] {
                    return self.err("@wave only allowed in channel 3.");
                }
                self.eat()?;
                let id = self.number("Expected wave data id.")?;
                let Some(&idx) = self.song.wave_map.get(&id) else {
                    let d = self.next().data.clone();
                    return self.err(format!("Wave \"{d}\" not defined."));
                };
                self.eat()?;
                self.song.add(active, i32::from(T_WAVE));
                self.song.add(active, idx);
            }
            "@ve" => {
                self.eat()?;
                let mut increasing = true;
                if self.next().kind == Kind::Dash {
                    self.eat()?;
                    increasing = false;
                }
                let mut env = self.number("Invalid volume envelope. Expected number.")?;
                self.eat()?;
                if env > 7 {
                    return self.err("Invalid volume envelope. Expected values from -7 to 7.");
                }
                if increasing {
                    env |= 1 << 3;
                }
                self.song.add(active, i32::from(T_ENV));
                self.song.add(active, env);
            }
            "@wd" => {
                if active[2] || active[3] {
                    return self.err("@wd only allowed in channel 1 and 2.");
                }
                self.eat()?;
                let duty = self.number("Invalid wave duty. Expected number.")?;
                if !(0..=3).contains(&duty) {
                    return self.err("Invalid wave duty. Expected values 0-3.");
                }
                self.eat()?;
                self.song.add(active, i32::from(T_WAVEDUTY));
                self.song.add(active, duty << 6);
            }
            "@p" => {
                if active[2] {
                    return self.err("@p only allowed in channel 1, 2 and 4.");
                }
                self.eat()?;
                let speed = self.number("Expected speed after @p macro.")?;
                if speed > 127 {
                    return self.err(format!("Invalid portamento speed {speed}. Expected -127 to 127."));
                }
                self.eat()?;
                self.song.add(active, i32::from(T_PORTAMENTO));
                self.song.add(active, speed);
            }
            "@s" => {
                if active[2] {
                    return self.err("@s only allowed in channel 1, 2 and 4.");
                }
                self.eat()?;
                let mut negative = false;
                if self.next().kind == Kind::Dash {
                    negative = true;
                    self.eat()?;
                }
                let mut speed = self.number("Expected number after @s macro.")?;
                if speed > 127 {
                    return self.err(format!("Invalid pitch slide speed {speed}. Expected 0-127."));
                }
                self.eat()?;
                if negative {
                    speed = -speed;
                }
                self.song.add(active, i32::from(T_SLIDE));
                self.song.add(active, speed + 128);
            }
            "@po" => {
                if active[3] {
                    return self.err("@po only allowed in channel 1, 2 and 3.");
                }
                self.eat()?;
                let mut negative = false;
                if self.next().kind == Kind::Dash {
                    negative = true;
                    self.eat()?;
                }
                let mut offset = self.number("Expected number after @po macro.")?;
                if offset > 127 {
                    return self.err("Invalid pitch offset. Expacted values 0-127.");
                }
                self.eat()?;
                if negative {
                    offset = -offset;
                }
                self.song.add(active, i32::from(T_PITCH_OFFSET));
                self.song.add(active, offset + 128);
            }
            "@v" => {
                if active[2] || active[3] {
                    return self.err("@v only allowed in channel 1 and 2.");
                }
                self.eat()?;
                let speed = self.number("Expected vibrato speed after @v macro.")?;
                if !(0..=15).contains(&speed) {
                    return self.err("Invalid vibrato speed. Expected values 0-15.");
                }
                self.eat()?;
                self.expect(Kind::Comma, "comma")?;
                let depth = self.number("Expected vibrato depth after @v macro.")?;
                if !(0..=4).contains(&depth) {
                    return self.err("Invalid vibrato depth. Expected values 0-4.");
                }
                self.eat()?;
                let mut delay = 0;
                if self.next().kind == Kind::Comma {
                    self.eat()?;
                    delay = self.length(true)?;
                }
                if delay > 0 {
                    self.song.add(active, i32::from(T_VIBRATO_DELAY));
                    self.song.add(active, speed | (depth << 4));
                    self.song.add(active, delay);
                } else {
                    self.song.add(active, i32::from(T_VIBRATO));
                    self.song.add(active, speed | (depth << 4));
                }
            }
            "@ns" => {
                if active[0] || active[1] || active[2] {
                    return self.err("@ns only allowed in channel 4.");
                }
                self.eat()?;
                let state = self.number("Expected 0 or 1 after @ns macro.")?;
                if !(0..=1).contains(&state) {
                    return self.err("Expected 0 or 1 after @ns macro.");
                }
                self.eat()?;
                self.song.add(active, i32::from(T_NOISE_STEP));
                self.song.add(active, state);
            }
            _ => {
                // "@@"
                self.eat()?;
                let id = self.number("Expected macro id.")?;
                let (index, data) = if self.in_macro {
                    let i = self.root_macro_map.get(&id).copied();
                    (i, i.map(|i| self.root_macros[i as usize].clone()))
                } else {
                    (self.song.macro_map.get(&id).copied(), None)
                };
                let Some(index) = index else {
                    return self.err(format!("Macro @@{id} not found."));
                };
                self.eat()?;
                if let Some(d) = data {
                    self.song.add_all(active, &d);
                } else {
                    self.song.add(active, i32::from(T_MACRO));
                    self.song.add(active, index);
                }
            }
        }
        Ok(())
    }

    /// `parseLength`: a note's length in steps (192 a bar), 0 when none is given and none is required.
    fn length(&mut self, required: bool) -> R<i32> {
        let mut length = 0;
        if self.next().kind == Kind::Number {
            length = self.int(&self.next().data.clone())?;
            if length < 1 || length > BAR_STEPS {
                return self.err(format!("Invalid note length {length}. Expected 1-{BAR_STEPS}."));
            }
            self.eat()?;
            if BAR_STEPS / length * length != BAR_STEPS {
                return self.err(format!("Invalid note length {length}. Not enough precision."));
            }
            length = BAR_STEPS / length;
            let mut dot = length / 2;
            while self.next().kind == Kind::Dot {
                if dot <= 0 {
                    return self.err("Too many dots in length. Not enough precision.");
                }
                self.eat()?;
                length += dot;
                dot /= 2;
            }
        } else if self.next().kind == Kind::Assign {
            self.eat()?;
            if self.next().kind != Kind::Number {
                return self.err("Expected note frame length.");
            }
            length = self.int(&self.next().data.clone())?;
            if !(1..=255).contains(&length) {
                return self.err(format!("Invalid note frame length {length}. Expected 1-255."));
            }
            self.eat()?;
        } else if required {
            return self.err("Expected note length.");
        }
        if self.next().kind == Kind::Tie {
            self.eat()?;
            length += self.length(true)?;
        }
        Ok(length)
    }
}

/// Compiles mmlgb's MML into the song's data (what `MMLGB.jar` writes as `NAME_data`).
pub fn compile(src: &str) -> Result<Vec<u8>, Error> {
    let mut text = src.to_string();
    if !text.ends_with('\n') {
        text.push('\n');
    }
    let mut p = Parser { tokens: lex(&text), pos: 0, song: Collected::default(), root_macros: Vec::new(), root_macro_map: HashMap::new(), in_macro: false };
    p.parse()?;
    Ok(p.song.data())
}

/// The data as the assembler file `MMLGB.jar` writes (`.db` lines).
pub fn to_asm(name: &str, data: &[u8]) -> String {
    let mut s = format!(".globl _{0}_data\n_{0}_data:\n", name.to_lowercase());
    for b in data {
        s.push_str(&format!("\t.db 0x{b:02X}\n"));
    }
    s
}

// ---- the driver (music.c) ----

/// `freq.c`: the pulse and wave channels' frequency registers, 16 a octave (12 notes, 4 unused), from octave 2.
const FREQ: [[u16; 12]; 7] = [
    [44, 157, 263, 363, 458, 547, 631, 711, 786, 856, 923, 986],
    [1046, 1102, 1155, 1205, 1253, 1297, 1340, 1379, 1417, 1452, 1486, 1517],
    [1547, 1575, 1602, 1627, 1650, 1673, 1694, 1714, 1732, 1750, 1767, 1783],
    [1798, 1812, 1825, 1837, 1849, 1860, 1871, 1881, 1890, 1899, 1907, 1915],
    [1923, 1930, 1936, 1943, 1949, 1954, 1959, 1964, 1969, 1974, 1978, 1982],
    [1985, 1989, 1992, 1995, 1998, 2001, 2004, 2006, 2009, 2011, 2013, 2015],
    [2017, 2018, 2020, 2022, 2023, 2025, 2026, 2027, 2028, 2029, 2030, 2031],
];

/// `freq[(octave << 4) + note]`: zero where the table has none (it is read past its end in C).
fn freq_of(index: u32) -> u16 {
    let (oct, n) = ((index >> 4) as usize, (index & 15) as usize);
    if (2..9).contains(&oct) && n < 12 {
        FREQ[oct - 2][n]
    } else {
        0
    }
}

/// `noisefreq.c` (made by `scripts/gen_noise.lua`): NR43 for a note, from octave 1 (a octave down a row: the divisor
/// code from 7 to 0; a note up a clock shift down from 15).
fn noise_of(index: u32) -> u8 {
    let (row, n) = ((index >> 4) as usize, (index & 15) as usize);
    if row < 8 && n < 12 {
        ((15 - n) * 16 + (7 - row)) as u8
    } else {
        0
    }
}

/// `vib.c`.
const VIB: [[u8; 64]; 4] = [
    [
        1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
        2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1, 1, 1, 1, 1,
        1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1,
    ],
    [
        2, 2, 2, 3, 3, 3, 3, 3, 3, 4, 4, 4, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 4, 3, 3, 3, 3, 3, 3, 2, 2,
        2, 2, 2, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 2, 2,
    ],
    [
        4, 4, 5, 5, 6, 6, 6, 7, 7, 7, 7, 8, 8, 8, 8, 8,
        8, 8, 8, 8, 8, 8, 7, 7, 7, 7, 6, 6, 6, 5, 5, 4,
        4, 4, 3, 3, 2, 2, 2, 1, 1, 1, 1, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 3, 3, 4,
    ],
    [
        8, 9, 10, 10, 11, 12, 12, 13, 14, 14, 15, 15, 15, 16, 16, 16,
        16, 16, 16, 16, 15, 15, 15, 14, 14, 13, 12, 12, 11, 10, 10, 9,
        8, 7, 6, 6, 5, 4, 4, 3, 2, 2, 1, 1, 1, 0, 0, 0,
        0, 0, 0, 0, 1, 1, 1, 2, 2, 3, 4, 4, 5, 6, 6, 7,
    ],
];

/// Most commands one channel runs in a tick (the C driver would hang in its interrupt).
const MAX_STEPS: u32 = 100_000;

// Sound registers, as offsets from $FF10.
const NR12: u8 = 0x02;
const NR30: u8 = 0x0A;
const NR32: u8 = 0x0C;
const NR33: u8 = 0x0D;
const NR34: u8 = 0x0E;
const NR42: u8 = 0x11;
const NR43: u8 = 0x12;
const NR44: u8 = 0x13;
const NR50: u8 = 0x14;
const NR51: u8 = 0x15;
const NR52: u8 = 0x16;

/// What the driver does, tick by tick: `music.c` with its registers written to a `Gb`.
pub struct Music {
    song: Arc<Vec<u8>>,
    gb: Gb,
    nr51: u8,
    tma: u8,
    step: u8,
    data: [usize; 4],
    bak: [usize; 4],
    lp: [usize; 4],
    in_macro: [bool; 4],
    done: [bool; 4],
    wait: [u8; 4],
    octave: [u8; 4],
    length: [u8; 4],
    volume: [u8; 4],
    env: [u8; 4],
    pan: [u8; 4],
    duty: [u8; 2],
    freq: [u16; 3],
    freq4: u8,
    target: [u16; 2],
    target4: u8,
    // Channels 1, 2 and 4 (index 0, 1, 3).
    slide: [u8; 4],
    porta: [u8; 4],
    vib_speed: [u8; 2],
    vib_table: [usize; 2],
    vib_pos: [u8; 2],
    vib_delay: [u8; 2],
    noise_step: u8,
    po: [u8; 3],
    rep: [[usize; 256]; 4],
    repeats: [[u8; 256]; 4],
    depth: [u8; 4],
    /// Timer ticks done.
    pub steps: u64,
    /// Every note started, for a timeline: (step from 1, channel, MIDI note or None for a rest).
    heard: Option<Vec<(u64, usize, Option<u8>)>>,
}

impl Music {
    pub fn new(song: Arc<Vec<u8>>) -> Music {
        let word = |i: usize| -> usize {
            let at = i * 2;
            match (song.get(at), song.get(at + 1)) {
                (Some(&a), Some(&b)) => usize::from(a) | usize::from(b) << 8,
                _ => song.len(),
            }
        };
        let starts = [word(0), word(1), word(2), word(3)];
        let mut m = Music {
            gb: Gb::default(),
            nr51: 0,
            tma: 255 - 51,
            step: 0,
            data: starts,
            bak: [0; 4],
            lp: starts,
            in_macro: [false; 4],
            done: [false; 4],
            wait: [0; 4],
            octave: [4; 4],
            length: [48; 4],
            volume: [0xF0, 0xF0, 32, 0xF0],
            env: [3; 4],
            pan: [0x11; 4],
            duty: [0; 2],
            freq: [0; 3],
            freq4: 0,
            target: [0; 2],
            target4: 0,
            slide: [0; 4],
            porta: [0; 4],
            vib_speed: [0; 2],
            vib_table: [1; 2],
            vib_pos: [0; 2],
            vib_delay: [0; 2],
            noise_step: 0,
            po: [128; 3],
            rep: [[0; 256]; 4],
            repeats: [[0; 256]; 4],
            depth: [255; 4],
            steps: 0,
            heard: None,
            song,
        };
        // mus_init: sound on, everything to both sides, the channels killed.
        m.w(NR52, 0x80);
        m.w(NR51, 0xFF);
        m.w(NR50, 0xFF);
        for r in [0x00, NR12, 0x07, NR30, NR32, NR42] {
            m.w(r, 0);
        }
        m
    }

    fn w(&mut self, reg: u8, v: u8) {
        if reg == NR51 {
            self.nr51 = v;
        }
        self.gb.write(reg, v);
    }

    fn read(&mut self, c: usize) -> u8 {
        let b = self.song.get(self.data[c]).copied().unwrap_or(T_EOF);
        self.data[c] += 1;
        b
    }

    fn peek(&self, c: usize) -> u8 {
        self.song.get(self.data[c]).copied().unwrap_or(T_EOF)
    }

    /// The timer's period now, in ticks of 4096 Hz.
    pub fn period(&self) -> u32 {
        256 - u32::from(self.tma)
    }

    /// Whether every channel has reached its end since this was last asked (the song has played once round).
    fn take_round(&mut self) -> bool {
        if self.done.iter().all(|d| *d) {
            self.done = [false; 4];
            true
        } else {
            false
        }
    }

    /// `mus_update`: one timer interrupt.
    pub fn update(&mut self) {
        self.update_sq(0);
        self.update_sq(1);
        self.update_wave();
        self.update_noise();
        self.step = self.step.wrapping_add(1);
        self.steps += 1;
    }

    fn note_event(&mut self, c: usize, note: Option<u8>) {
        let at = self.steps + 1;
        let octave = self.octave[c];
        if let Some(h) = &mut self.heard {
            h.push((at, c, note.map(|n| (12 * (u32::from(octave) + 1) + u32::from(n - T_C)).min(127) as u8)));
        }
    }

    /// The commands every channel shares; true when the command is one of them (its arguments read).
    fn shared(&mut self, c: usize, cmd: u8) -> bool {
        match cmd {
            T_LENGTH => self.length[c] = self.read(c),
            T_OCTAVE => self.octave[c] = self.read(c),
            T_OCT_UP => self.octave[c] = self.octave[c].wrapping_add(1),
            T_OCT_DOWN => self.octave[c] = self.octave[c].wrapping_sub(1),
            T_REP_START => {
                self.depth[c] = self.depth[c].wrapping_add(1);
                self.rep[c][usize::from(self.depth[c])] = self.data[c];
            }
            T_REP_END => {
                let n = self.read(c);
                let d = usize::from(self.depth[c]);
                if self.repeats[c][d] == 0 {
                    self.repeats[c][d] = n;
                    self.data[c] = self.rep[c][d];
                }
                self.repeats[c][d] = self.repeats[c][d].wrapping_sub(1);
                if self.repeats[c][d] != 0 {
                    self.data[c] = self.rep[c][d];
                } else {
                    self.depth[c] = self.depth[c].wrapping_sub(1);
                }
            }
            T_LOOP => self.lp[c] = self.data[c],
            T_TEMPO => self.tma = self.read(c),
            T_MACRO => {
                let n = usize::from(self.read(c));
                self.bak[c] = self.data[c];
                let at = 2 * (5 + n);
                self.data[c] = match (self.song.get(at), self.song.get(at + 1)) {
                    (Some(&a), Some(&b)) => usize::from(a) | usize::from(b) << 8,
                    _ => self.song.len(),
                };
                self.in_macro[c] = true;
            }
            _ => return false,
        }
        true
    }

    /// The end of the channel's data: back from a macro, or to its loop point. True when the tick's run for it is over.
    fn eof(&mut self, c: usize) -> bool {
        if self.in_macro[c] {
            self.data[c] = self.bak[c];
            self.in_macro[c] = false;
        } else {
            self.data[c] = self.lp[c];
            self.done[c] = true;
            if self.peek(c) == T_EOF {
                self.wait[c] = 255;
                return true;
            }
        }
        false
    }

    /// `mus_update1` and `mus_update2` (the registers differ by a base).
    fn update_sq(&mut self, c: usize) {
        let base = if c == 0 { 0 } else { 5 };
        if self.slide[c] != 0 && self.step & 3 == 0 {
            if self.target[c] > self.freq[c] {
                self.freq[c] = self.freq[c].wrapping_add(u16::from(self.slide[c]));
                if self.freq[c] > self.target[c] {
                    self.freq[c] = self.target[c];
                }
            } else if self.target[c] < self.freq[c] {
                self.freq[c] = self.freq[c].wrapping_sub(u16::from(self.slide[c]));
                if self.freq[c] < self.target[c] {
                    self.freq[c] = self.target[c];
                }
            }
            let f = self.freq[c];
            self.w(base + 3, f as u8);
            self.w(base + 4, (f >> 8) as u8);
        }
        if self.vib_delay[c] != 0 {
            self.vib_delay[c] -= 1;
        } else if self.vib_speed[c] != 0 {
            self.vib_pos[c] = self.vib_pos[c].wrapping_add(self.vib_speed[c]) & 63;
            let t = &VIB[self.vib_table[c] - 1];
            let tmp = i32::from(self.freq[c]) - i32::from(t[0]) + i32::from(t[usize::from(self.vib_pos[c])]);
            self.w(base + 3, tmp as u8);
            self.w(base + 4, (tmp >> 8) as u8);
        }
        if self.wait[c] != 0 {
            self.wait[c] -= 1;
            if self.wait[c] != 0 {
                return;
            }
        }
        for _ in 0..MAX_STEPS {
            let note = self.read(c);
            if note >= T_C {
                let mut n = note;
                if n & 0x80 != 0 {
                    n ^= 0x80;
                    self.wait[c] = self.read(c);
                } else {
                    self.wait[c] = self.length[c];
                }
                if n == T_WAIT {
                    return;
                } else if n == T_REST {
                    self.freq[c] = 0;
                    self.w(base + 2, 0);
                    self.note_event(c, None);
                } else {
                    let idx = (u32::from(self.octave[c]) << 4) + u32::from(n) - u32::from(T_C);
                    let tmp = i32::from(freq_of(idx)) + i32::from(self.po[c]) - 128;
                    if self.porta[c] != 0 {
                        self.target[c] = tmp as u16;
                    } else {
                        self.freq[c] = tmp as u16;
                    }
                    let v = self.volume[c] | self.env[c];
                    self.w(base + 2, v);
                    self.note_event(c, Some(n));
                }
                let f = self.freq[c];
                self.w(base + 3, f as u8);
                self.w(base + 4, (f >> 8) as u8 | 0x80);
                return;
            }
            if self.shared(c, note) {
                continue;
            }
            match note {
                T_VOL => {
                    self.volume[c] = self.read(c);
                    let v = self.volume[c] | self.env[c];
                    self.w(base + 2, v);
                }
                T_ENV => {
                    self.env[c] = self.read(c);
                    let v = self.volume[c] | self.env[c];
                    self.w(base + 2, v);
                }
                T_WAVEDUTY => {
                    self.duty[c] = self.read(c);
                    let d = self.duty[c];
                    self.w(base + 1, d);
                }
                T_PAN => {
                    self.pan[c] = self.read(c);
                    let mask = if c == 0 { 0xEE } else { 0xDD };
                    let v = (self.nr51 & mask) | self.pan[c].wrapping_shl(c as u32);
                    self.w(NR51, v);
                }
                T_PORTAMENTO => {
                    self.slide[c] = self.read(c);
                    self.porta[c] = self.slide[c];
                }
                T_SLIDE => {
                    self.slide[c] = self.read(c);
                    self.porta[c] = 0;
                    if self.slide[c] >= 128 {
                        self.target[c] = 0xFFFF;
                        self.slide[c] -= 128;
                    } else {
                        self.target[c] = 0;
                        self.slide[c] = 128 - self.slide[c];
                    }
                }
                T_VIBRATO | T_VIBRATO_DELAY => {
                    self.vib_pos[c] = 0;
                    let n = self.read(c);
                    self.vib_speed[c] = n & 15;
                    self.vib_table[c] = match n >> 4 {
                        1 => 1,
                        2 => 2,
                        3 => 3,
                        _ => 4,
                    };
                    self.vib_delay[c] = if note == T_VIBRATO_DELAY { self.read(c) } else { 0 };
                }
                T_PITCH_OFFSET => self.po[c] = self.read(c),
                // These do nothing in a pulse channel, and read no argument.
                T_NOISE_STEP | T_WAVE => {}
                T_EOF => {
                    if self.eof(c) {
                        return;
                    }
                }
                _ => {}
            }
        }
        self.wait[c] = 255;
    }

    /// `mus_update3`: the wave channel.
    fn update_wave(&mut self) {
        let c = 2;
        if self.wait[c] != 0 {
            self.wait[c] -= 1;
            if self.wait[c] != 0 {
                return;
            }
        }
        for _ in 0..MAX_STEPS {
            let note = self.read(c);
            if note >= T_C {
                let mut n = note;
                if n & 0x80 != 0 {
                    n ^= 0x80;
                    self.wait[c] = self.read(c);
                } else {
                    self.wait[c] = self.length[c];
                }
                if n == T_WAIT {
                    return;
                } else if n == T_REST {
                    self.freq[2] = 0;
                    self.w(NR32, 0);
                    self.note_event(c, None);
                } else {
                    let idx = (u32::from(self.octave[c]) << 4) + u32::from(n) - u32::from(T_C);
                    self.freq[2] = (i32::from(freq_of(idx)) + i32::from(self.po[2]) - 128) as u16;
                    let v = self.volume[2];
                    self.w(NR32, v);
                    self.note_event(c, Some(n));
                }
                let f = self.freq[2];
                self.w(NR30, 0);
                self.w(NR30, 0x80);
                self.w(NR33, f as u8);
                self.w(NR34, 0x80 | (f >> 8) as u8);
                return;
            }
            if self.shared(c, note) {
                continue;
            }
            match note {
                T_VOL => self.volume[2] = self.read(c),
                T_PAN => {
                    self.pan[2] = self.read(c);
                    let v = (self.nr51 & 0xBB) | self.pan[2].wrapping_shl(2);
                    self.w(NR51, v);
                }
                T_PITCH_OFFSET => self.po[2] = self.read(c),
                T_WAVE => {
                    let n = usize::from(self.read(c));
                    self.w(NR30, 0);
                    let at = 2 * 4;
                    let wave = match (self.song.get(at), self.song.get(at + 1)) {
                        (Some(&a), Some(&b)) => usize::from(a) | usize::from(b) << 8,
                        _ => self.song.len(),
                    };
                    for i in 0..16 {
                        let b = self.song.get(wave + (n << 4) + i).copied().unwrap_or(0);
                        self.w(0x20 + i as u8, b);
                    }
                    self.w(NR30, 0x80);
                }
                T_EOF => {
                    if self.eof(c) {
                        return;
                    }
                }
                // Envelope, duty, portamento, slide and vibrato do nothing here, and read no argument.
                _ => {}
            }
        }
        self.wait[c] = 255;
    }

    /// `mus_update4`: the noise channel.
    fn update_noise(&mut self) {
        let c = 3;
        if self.slide[c] != 0 && self.step & 3 == 0 {
            if self.target4 > self.freq4 {
                self.freq4 = self.freq4.wrapping_add(self.slide[c]);
                if self.freq4 > self.target4 {
                    self.freq4 = self.target4;
                }
            } else if self.target4 < self.freq4 {
                self.freq4 = self.freq4.wrapping_sub(self.slide[c]);
                if self.freq4 < self.target4 {
                    self.freq4 = self.target4;
                }
            }
            let v = self.freq4 | self.noise_step;
            self.w(NR43, v);
        }
        if self.wait[c] != 0 {
            self.wait[c] -= 1;
            if self.wait[c] != 0 {
                return;
            }
        }
        for _ in 0..MAX_STEPS {
            let note = self.read(c);
            if note >= T_C {
                let mut n = note;
                if n & 0x80 != 0 {
                    n ^= 0x80;
                    self.wait[c] = self.read(c);
                } else {
                    self.wait[c] = self.length[c];
                }
                if n == T_WAIT {
                    return;
                } else if n == T_REST {
                    self.freq4 = 0;
                    self.w(NR42, 0);
                    self.note_event(c, None);
                } else {
                    // ((octave - 1) << 4) + note, in C's unsigned arithmetic.
                    let idx = u32::from(self.octave[c]).wrapping_sub(1).wrapping_shl(4).wrapping_add(u32::from(n)).wrapping_sub(u32::from(T_C));
                    let tmp = noise_of(idx);
                    if self.porta[c] != 0 {
                        self.target4 = tmp;
                    } else {
                        self.freq4 = tmp;
                    }
                    let v = self.volume[3] | self.env[3];
                    self.w(NR42, v);
                    self.note_event(c, Some(n));
                }
                let v = self.freq4 | self.noise_step;
                self.w(NR43, v);
                self.w(NR44, 0x80);
                return;
            }
            if self.shared(c, note) {
                continue;
            }
            match note {
                T_VOL => {
                    self.volume[3] = self.read(c);
                    let v = self.volume[3] | self.env[3];
                    self.w(NR42, v);
                }
                T_ENV => {
                    self.env[3] = self.read(c);
                    let v = self.volume[3] | self.env[3];
                    self.w(NR42, v);
                }
                T_PAN => {
                    self.pan[3] = self.read(c);
                    let v = (self.nr51 & 0x77) | self.pan[3].wrapping_shl(3);
                    self.w(NR51, v);
                }
                T_PORTAMENTO => {
                    self.slide[c] = self.read(c);
                    self.porta[c] = self.slide[c];
                }
                T_SLIDE => {
                    self.slide[c] = self.read(c);
                    self.porta[c] = 0;
                    if self.slide[c] >= 128 {
                        self.target4 = 0xFF;
                        self.slide[c] -= 128;
                    } else {
                        self.target4 = 0;
                        self.slide[c] = 128 - self.slide[c];
                    }
                }
                T_NOISE_STEP => self.noise_step = self.read(c).wrapping_shl(3),
                T_EOF => {
                    if self.eof(c) {
                        return;
                    }
                }
                // Duty, vibrato, pitch offset and wave do nothing here, and read no argument.
                _ => {}
            }
        }
        self.wait[c] = 255;
    }
}

/// A song: the data `compile` makes.
pub struct Song {
    data: Arc<Vec<u8>>,
}

impl Song {
    /// Reads a song's data, when its header is in order (its offsets rise and stay inside it).
    pub fn parse(d: &[u8]) -> Option<Song> {
        let word = |i: usize| -> Option<usize> { Some(usize::from(*d.get(i * 2)?) | usize::from(*d.get(i * 2 + 1)?) << 8) };
        let wave = word(4)?;
        if wave < 10 || wave % 2 != 0 || wave > d.len() {
            return None;
        }
        let macros = wave / 2 - 5;
        let mut prev = wave;
        for i in [0, 1, 2, 3] {
            let at = word(i)?;
            if at < prev || at > d.len() {
                return None;
            }
            prev = at;
        }
        for i in 0..macros {
            if word(5 + i)? < wave || word(5 + i)? > d.len() {
                return None;
            }
        }
        // Every channel ends with an end mark.
        if d.last() != Some(&T_EOF) {
            return None;
        }
        Some(Song { data: Arc::new(d.to_vec()) })
    }

    /// Compiles MML and keeps the result.
    pub fn from_mml(src: &str) -> Result<Song, Error> {
        let d = compile(src)?;
        Ok(Song { data: Arc::new(d) })
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// The song's notes, followed without sound (at most a quarter hour).
    pub fn timeline(&self) -> Timeline {
        let mut m = Music::new(self.data.clone());
        m.heard = Some(Vec::new());
        let mut seconds = 0.0;
        let mut rounds: Vec<(f64, u64)> = Vec::new();
        while seconds < 900.0 && rounds.len() < 2 {
            let period = m.period();
            m.update();
            // The tick that finds every channel at its end starts the song again: the song is the ticks before it.
            if m.take_round() {
                rounds.push((seconds, m.steps - 1));
            }
            seconds += f64::from(period) / 4096.0;
        }
        let (once_seconds, once_steps) = rounds.first().copied().unwrap_or((seconds, m.steps));
        let restart = match rounds[..] {
            [(_, a), (_, b)] if b > a => Some(a.saturating_sub(b - a)),
            _ => None,
        };
        let mut events = m.heard.take().unwrap_or_default();
        events.retain(|e| e.0 <= once_steps);
        Timeline { events, steps: once_steps, restart, seconds: once_seconds }
    }
}

/// A song's notes.
pub struct Timeline {
    /// Step (from 1; 192 a bar), channel, MIDI note (None where a rest cuts it).
    pub events: Vec<(u64, usize, Option<u8>)>,
    pub steps: u64,
    pub restart: Option<u64>,
    pub seconds: f64,
}

impl Tune for Song {
    fn format(&self) -> &'static str {
        "mmlgb"
    }

    fn file(&self) -> &[u8] {
        &self.data
    }

    fn driver(&self, _song: usize) -> Box<dyn Driver> {
        Box::new(Player::new(self.data.clone()))
    }

    fn length(&self, _song: usize) -> Option<f64> {
        Some(self.timeline().seconds)
    }
}

// Four channels at full volume reach about this each.
const LEVEL: f32 = 0.12;

/// The song played: the driver's timer ticks, the APU between them.
pub struct Player {
    m: Music,
    period: u32,
    done: bool,
}

impl Player {
    pub fn new(song: Arc<Vec<u8>>) -> Player {
        let m = Music::new(song);
        let period = m.period();
        Player { m, period, done: false }
    }
}

impl Driver for Player {
    fn channels(&self) -> usize {
        4
    }

    fn tick(&mut self) {
        // The timer reloaded with the tempo as it stood before this tick's commands.
        self.period = self.m.period();
        self.m.update();
        if self.m.take_round() {
            self.done = true;
        }
    }

    fn tick_seconds(&self) -> f64 {
        f64::from(self.period) / 4096.0
    }

    fn render(&mut self, rate: u32, frames: usize, out: &mut Bufs) {
        let (l, r) = (&mut out.left, &mut out.right);
        let ([a, b, c, d, ..], [e, f, g, h, ..]) = (l.as_mut_slice(), r.as_mut_slice()) else { return };
        self.m.gb.render(
            rate,
            &mut [&mut a[..frames], &mut b[..frames], &mut c[..frames], &mut d[..frames]],
            &mut [&mut e[..frames], &mut f[..frames], &mut g[..frames], &mut h[..frames]],
        );
        for ch in l.iter_mut().chain(r.iter_mut()).take(8) {
            for v in ch[..frames].iter_mut() {
                *v *= LEVEL;
            }
        }
    }

    fn position(&self) -> (usize, usize) {
        // A pattern a bar (192 steps), a row a sixteenth.
        let s = self.m.steps;
        ((s / 192) as usize, ((s % 192) / 12) as usize)
    }

    fn speed(&self) -> (u32, u32) {
        // Steps are 48 to a beat.
        (1, (4096 * 60 / (48 * self.m.period())).max(1))
    }

    fn take_done(&mut self) -> bool {
        std::mem::take(&mut self.done)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bufs(frames: usize) -> Bufs {
        Bufs { left: vec![vec![0.0; frames]; 4], right: vec![vec![0.0; frames]; 4] }
    }

    #[test]
    fn a_short_song_compiles_to_what_mmlgb_writes() {
        // c4 d8: lengths 48 and 24 steps; the header is 10 bytes (no macros), then each channel's data.
        let d = compile("A c4 d8\n").unwrap();
        assert_eq!(d, [10, 0, 15, 0, 16, 0, 17, 0, 10, 0, 21 | 0x80, 48, 23 | 0x80, 24, T_EOF, T_EOF, T_EOF, T_EOF]);
        // No last line break needed here.
        assert_eq!(compile("A c4 d8").unwrap(), d);
    }

    #[test]
    fn commands_have_their_values() {
        let d = compile("B o5 l8 v15 @ve-3 @wd2 t120 y-1 y0 y1 [ c ]3 L < r4 w2 > ^4 @po-2 @s3 @p5 @v4,2,8\n").unwrap();
        let b = &d[usize::from(d[2]) + usize::from(d[3]) * 256..usize::from(d[4])];
        assert_eq!(
            b,
            [
                T_OCTAVE, 5, T_LENGTH, 24, T_VOL, 0xF0, T_ENV, 3, T_WAVEDUTY, 128, T_TEMPO, 255 - 43, T_PAN, 16, T_PAN, 17, T_PAN, 1, T_REP_START, T_C, T_REP_END, 3, T_LOOP,
                T_OCT_DOWN, T_REST | 0x80, 48, T_WAIT | 0x80, 96, T_OCT_UP, T_WAIT | 0x80, 48, T_PITCH_OFFSET, 126, T_SLIDE, 131, T_PORTAMENTO, 5, T_VIBRATO_DELAY, 4 | (2 << 4), 24,
                T_EOF
            ]
        );
    }

    #[test]
    fn the_wave_channel_has_its_own_volumes_and_waves() {
        let mut src = String::from("@wave 7 = {");
        for i in 0..32 {
            src.push_str(&format!("{} ", i % 16));
        }
        src.push_str("}\nC @wave 7 v1 c4\n");
        let d = compile(&src).unwrap();
        assert_eq!(d[8], 10); // the waves start after the header
        assert_eq!(&d[10..14], [0x01, 0x23, 0x45, 0x67]);
        let at = usize::from(d[4]) + usize::from(d[5]) * 256;
        assert_eq!(&d[at..at + 6], [T_WAVE, 0, T_VOL, 96, T_C | 0x80, 48]);
        assert!(compile("C v4 c\n").is_err());
        assert!(compile("A @wave 0\n").is_err());
    }

    #[test]
    fn a_macro_is_played_or_pasted_in() {
        let d = compile("@@1 = { o5 c8 }\n@@2 = { @@1 d8 }\nA @@1\nB @@2\n").unwrap();
        // The header names two macros: starts after the header (14 bytes, no waves).
        assert_eq!(d[8], 14);
        assert_eq!(usize::from(d[10]), 14);
        let m1 = &d[14..14 + 5];
        assert_eq!(m1, [T_OCTAVE, 5, T_C | 0x80, 24, T_EOF]);
        // The second macro has the first one's data in it.
        assert_eq!(usize::from(d[12]), 19);
        let m2 = &d[19..26];
        assert_eq!(m2, [T_OCTAVE, 5, T_C | 0x80, 24, (T_C + 2) | 0x80, 24, T_EOF]);
        assert!(compile("A @@3\n").is_err());
    }

    #[test]
    fn what_mmlgb_does_oddly_is_kept() {
        // Hex numbers lex as a decimal then a skipped letter: 0x10 is 0 and 10.
        assert!(compile("A l0x10\n").is_err());
        // A character no token matches is skipped.
        assert_eq!(compile("A c4 ! ? c4\n").unwrap(), compile("A c4 c4\n").unwrap());
        // A rest's length is written whole, a note's is cut at 255 (a whole note tied five times).
        let n = compile("A c1^1^1^1^1\n").unwrap();
        assert_eq!(&n[10..18], [T_C | 0x80, 195, T_WAIT | 0x80, 255, T_WAIT | 0x80, 255, T_WAIT | 0x80, 255]);
        // A length the bar doesn't divide by.
        assert!(compile("A c5\n").is_err());
        assert!(compile("A c193\n").is_err());
        // A macro can't use @wave.
        assert!(compile("@wave 0 = {0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0}\n@@0 = { @wave 0 }\n").is_err());
    }

    // Samples to a mono mix, and how often it crosses its middle.
    fn crossings(mml: &str, seconds: f64) -> (f64, f32) {
        let song = Song::from_mml(mml).unwrap();
        let mut p = Player::new(song.data.clone());
        let rate = 48_000u32;
        let mut samples: Vec<f32> = Vec::new();
        let mut t = 0.0;
        while t < seconds {
            p.tick();
            let frames = (p.tick_seconds() * f64::from(rate)).round() as usize;
            let mut b = bufs(frames);
            p.render(rate, frames, &mut b);
            for i in 0..frames {
                samples.push((0..4).map(|c| b.left[c][i] + b.right[c][i]).sum());
            }
            t += p.tick_seconds();
        }
        let peak = samples.iter().cloned().fold(0.0f32, f32::max);
        let mid = peak / 2.0;
        let mut n = 0;
        let mut above = false;
        for &s in &samples {
            if s > mid && !above {
                n += 1;
                above = true;
            } else if s < mid * 0.5 {
                above = false;
            }
        }
        (f64::from(n) / t, peak)
    }

    #[test]
    fn a_pulse_note_sounds_at_its_pitch() {
        // Middle C: 4 octave, register 1547 is 131072 / (2048 - 1547) = 261.6 Hz.
        let (hz, peak) = crossings("A @ve0 o4 c1\n", 0.6);
        assert!(peak > 0.05, "peak {peak}");
        assert!((hz - 261.6).abs() < 4.0, "{hz}");
        // An octave up is twice that.
        let (hz, _) = crossings("A @ve0 o5 c1\n", 0.6);
        assert!((hz - 523.3).abs() < 8.0, "{hz}");
    }

    #[test]
    fn the_wave_channel_plays_its_wave() {
        let mut src = String::from("@wave 0 = {");
        for i in 0..32 {
            src.push_str(&format!("{} ", if i < 16 { 15 } else { 0 }));
        }
        src.push_str("}\nC @wave 0 v3 o4 c1\n");
        let (hz, peak) = crossings(&src, 0.6);
        assert!(peak > 0.02, "peak {peak}");
        // The wave plays 65536 / (2048 - f) times a second: an octave under the pulse's for the same register.
        assert!((hz - 130.8).abs() < 3.0, "{hz}");
    }

    #[test]
    fn the_noise_channel_makes_noise() {
        let (_, peak) = crossings("D @ve0 o8 a1\n", 0.3);
        assert!(peak > 0.02, "peak {peak}");
    }

    #[test]
    fn the_tempo_sets_the_timer() {
        let song = Song::from_mml("A t120 c4\n").unwrap();
        let mut p = Player::new(song.data.clone());
        assert_eq!(p.m.period(), 52);
        p.tick();
        // t120: 4096 / (120 / 60 * 48) = 42.67, rounded 43, a period of 44 timer ticks.
        assert_eq!(p.m.period(), 44);
        assert!((p.tick_seconds() - 52.0 / 4096.0).abs() < 1e-9);
        p.tick();
        assert!((p.tick_seconds() - 44.0 / 4096.0).abs() < 1e-9);
        assert_eq!(p.speed().1, 116);
    }

    #[test]
    fn a_song_comes_round_and_has_a_timeline() {
        let song = Song::from_mml("A c4 d4\nB e2\nC r1\nD r1\n").unwrap();
        let t = song.timeline();
        // The longest channel is a whole note (192 steps); A and B's notes are in it.
        assert_eq!(t.steps, 192);
        assert_eq!(t.restart, Some(0));
        let notes: Vec<_> = t.events.iter().filter(|e| e.2.is_some()).collect();
        assert_eq!(notes[0], &(1, 0, Some(60)));
        assert_eq!(notes[1], &(1, 1, Some(64)));
        assert_eq!(notes[2], &(49, 0, Some(62)));
        assert!((t.seconds - 192.0 * 52.0 / 4096.0).abs() < 0.2, "{}", t.seconds);
        // The Tune glue: it reads back by its id.
        assert_eq!(song.format(), "mmlgb");
        assert!(Song::parse(song.data()).is_some());
    }

    #[test]
    fn a_loop_point_is_where_it_comes_back_to() {
        let song = Song::from_mml("A c4 L d4\nB r4 L r4\nC r4 L r4\nD r4 L r4\n").unwrap();
        let t = song.timeline();
        assert_eq!(t.steps, 96);
        assert_eq!(t.restart, Some(48));
    }

    #[test]
    fn repeats_play_their_count() {
        let song = Song::from_mml("A [ c8 ]4\nB r1\nC r1\nD r1\n").unwrap();
        let t = song.timeline();
        let n = t.events.iter().filter(|e| e.0 <= 96 && e.1 == 0 && e.2.is_some()).count();
        assert_eq!(n, 4, "{:?}", t.events);
    }

    #[test]
    fn the_asm_is_the_data_as_bytes() {
        let a = to_asm("Song", &[1, 255]);
        assert_eq!(a, ".globl _song_data\n_song_data:\n\t.db 0x01\n\t.db 0xFF\n");
    }
}
