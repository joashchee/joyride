// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! Sawteeth (Jonas and Arvid Norberg's soft synth and tracker for BeOS,
//! 2000-2001): no samples, every sound a sawtooth, square, triangle,
//! noise or sine made at 44.1 kHz in floating point (the steps of the
//! sawtooth, square and noise placed between samples), stepped through by
//! its instrument's list, through one of six filters and two clippers,
//! under an amp and a filter envelope, vibrato and PWM. Each channel
//! plays its own list of parts (each its own speed and length, transposed
//! and damped), looping on its own; the song moves on a buffer at a time
//! (`spspal` samples: 882, PAL's 50 a second, unless the song says).
//!
//! Ported from the original replayer, whose source its authors released
//! under the MIT License: every step of its arithmetic in the same order
//! and precision, so the samples it makes are its own. Its notice:
//!
//! > Copyright 2001, Jonas Norberg. Distributed under the terms of the
//! > MIT Licence.
//! >
//! > Permission is hereby granted, free of charge, to any person
//! > obtaining a copy of this software and associated documentation files
//! > (the "Software"), to deal in the Software without restriction,
//! > including without limitation the rights to use, copy, modify, merge,
//! > publish, distribute, sublicense, and/or sell copies of the Software,
//! > and to permit persons to whom the Software is furnished to do so,
//! > subject to the following conditions:
//! >
//! > The above copyright notice and this permission notice shall be
//! > included in all copies or substantial portions of the Software.
//! >
//! > THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
//! > EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
//! > MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
//! > NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE
//! > LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
//! > OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION
//! > WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

use std::sync::{Arc, OnceLock};

use crate::replay::{Bufs, Driver, Patch, PatchBytes, Tune};

/// The replayer's rate: everything it makes is at 44.1 kHz.
pub const RATE: f64 = 44_100.0;
/// The newest file version the original reads (1.2).
const VERSION: i32 = 1200;
const MAX_CHANNELS: usize = 12;
const MAX_CHANNEL_STEPS: usize = 8192;
/// The longest a pattern here is, in rows.
const MAX_ROWS: u32 = 256;
/// A channel panned to one side at full volume peaks at about this.
const LEVEL: f32 = 0.4;

/// The original's constants are doubles cast to floats; these are cast
/// the same way. Its pi and two pi are its own approximations, kept.
const INV_RATE: f32 = 0.00002267573696145124f64 as f32;
#[allow(clippy::approx_constant)]
const LFO_STEP: f32 = (6.28 * 0.00002267573696145124f64) as f32;
#[allow(clippy::approx_constant)]
const PI: f32 = 3.14159265f64 as f32;

pub fn probe(d: &[u8]) -> bool {
    d.len() >= 8 && (d.starts_with(b"SWTD") || d.starts_with(b"SWTT"))
}

/// A step of an instrument's list: its note (0 keeps the last; relative
/// to the part's note or not) and waveform (0 keeps the last).
#[derive(Clone, Copy, Debug, Default)]
pub struct InsStep {
    pub note: u8,
    pub relative: bool,
    pub wave: u8,
}

#[derive(Clone, Debug)]
pub struct Instrument {
    pub name: String,
    /// The filter's and the amp's envelopes: (ticks to the point, level).
    pub filter: Vec<(u8, u8)>,
    pub amp: Vec<(u8, u8)>,
    /// 0 none, 1 simple low-pass, 2 the old low-pass, 3 to 6 the state
    /// variable filter's low-pass, high-pass, band-pass and band-stop.
    pub filter_mode: u8,
    /// 0 none, 1 hard clipping, 2 a sine's.
    pub clip_mode: u8,
    pub boost: u8,
    pub vib_speed: u8,
    pub vib_depth: u8,
    pub pwm_speed: u8,
    pub pwm_depth: u8,
    pub resonance: u8,
    /// Ticks a step of its list lasts.
    pub sps: u8,
    /// Where its list goes back to.
    pub loop_at: u8,
    pub steps: Vec<InsStep>,
}

impl Instrument {
    /// Instrument 0, which plays when none is given: silent.
    fn none() -> Instrument {
        Instrument {
            name: String::new(),
            filter: vec![(0, 0)],
            amp: vec![(0, 0)],
            filter_mode: 0,
            clip_mode: 0,
            boost: 1,
            vib_speed: 1,
            vib_depth: 1,
            pwm_speed: 1,
            pwm_depth: 1,
            resonance: 0,
            sps: 30,
            loop_at: 0,
            steps: vec![InsStep::default()],
        }
    }
}

/// A part's step: a note (0 none), an instrument (0 none) and an effect
/// (its command in the high nibble).
#[derive(Clone, Copy, Debug, Default)]
pub struct Step {
    pub note: u8,
    pub ins: u8,
    pub eff: u8,
}

#[derive(Clone, Debug)]
pub struct Part {
    pub name: String,
    /// Ticks a step lasts.
    pub sps: u8,
    pub steps: Vec<Step>,
}

impl Part {
    pub fn ticks(&self) -> u32 {
        u32::from(self.sps) * self.steps.len() as u32
    }
}

/// A channel's step: the part it plays, transposed, its volume lowered by
/// `damp` (of 255).
#[derive(Clone, Copy, Debug, Default)]
pub struct ChStep {
    pub part: u8,
    pub transpose: i8,
    pub damp: u8,
}

#[derive(Clone, Debug)]
pub struct Channel {
    pub left: u8,
    pub right: u8,
    /// Its list plays to `rloop`, then from `lloop`.
    pub lloop: u16,
    pub rloop: u16,
    pub steps: Vec<ChStep>,
}

/// A stretch of the song shown as a pattern: one of the first channel's
/// steps, `ticks` long, a row every `grain` ticks.
#[derive(Clone, Copy, Debug)]
pub struct Window {
    pub ticks: u32,
    pub grain: u32,
}

impl Window {
    pub fn rows(&self) -> usize {
        self.ticks.div_ceil(self.grain).max(1) as usize
    }
}

/// A channel's step as the tracker shows it: when it starts (in ticks
/// from the song's start), its part's step and the channel's transposition.
#[derive(Clone, Copy, Debug)]
pub struct Event {
    pub tick: u32,
    pub step: Step,
    pub transpose: i8,
}

#[derive(Clone, Debug)]
pub struct Song {
    pub version: u16,
    /// Samples a tick (at 44.1 kHz).
    pub spspal: u16,
    pub channels: Vec<Channel>,
    pub parts: Vec<Part>,
    /// Instrument 0 is the silent one the original adds; the file's are 1 on.
    pub instruments: Vec<Instrument>,
    pub title: String,
    pub author: String,
    /// Kept as text (SWTT) rather than binary (SWTD).
    pub text: bool,
    /// The first channel's steps as patterns.
    pub windows: Vec<Window>,
    n2f: Vec<f32>,
    r2f: Vec<f32>,
    file: Arc<Vec<u8>>,
}

/// The original's reader: big-endian values in a binary file, a value a
/// line (after `//` comments) in a text one; strings to a 0 or a newline.
struct Reader<'a> {
    d: &'a [u8],
    at: usize,
    text: bool,
    eof: bool,
}

impl Reader<'_> {
    fn byte(&mut self) -> u8 {
        if let Some(&b) = self.d.get(self.at) {
            self.at += 1;
            b
        } else {
            self.eof = true;
            0
        }
    }

    fn string(&mut self) -> Vec<u8> {
        let mut s = Vec::new();
        loop {
            // A line too long for its buffer reads as an empty one.
            if s.len() >= 299 {
                return Vec::new();
            }
            let b = self.byte();
            if self.eof || b == 0 || b == b'\n' {
                return s;
            }
            s.push(b);
        }
    }

    fn val(&mut self, bytes: u32) -> i32 {
        if !self.text {
            let mut v = 0u32;
            for _ in 0..bytes {
                v = (v << 8) | u32::from(self.byte());
            }
            return v as i32;
        }
        loop {
            let s = self.string();
            if self.eof {
                return 0;
            }
            let s = &s[..s.iter().position(|&c| c == b'/').unwrap_or(s.len())];
            if !s.is_empty() {
                return atoi(s);
            }
        }
    }

    fn u8(&mut self) -> u8 {
        self.val(1) as u8
    }

    fn name(&mut self) -> String {
        text(&self.string())
    }
}

/// C's `atoi`: spaces, a sign, digits.
fn atoi(s: &[u8]) -> i32 {
    let mut it = s.iter().copied().skip_while(|c| matches!(c, b' ' | b'\t' | b'\n' | b'\r' | 0x0B | 0x0C)).peekable();
    let neg = match it.peek() {
        Some(b'-') => {
            it.next();
            true
        }
        Some(b'+') => {
            it.next();
            false
        }
        _ => false,
    };
    let mut v = 0i32;
    for c in it.take_while(u8::is_ascii_digit) {
        v = v.wrapping_mul(10).wrapping_add(i32::from(c - b'0'));
    }
    if neg {
        v.wrapping_neg()
    } else {
        v
    }
}

/// A name: UTF-8 (BeOS's), or Latin-1 if it isn't that.
fn text(b: &[u8]) -> String {
    let s = String::from_utf8(b.to_vec()).unwrap_or_else(|_| b.iter().map(|&c| c as char).collect());
    s.trim_end().to_string()
}

/// The notes' frequencies (from C at 1.022 Hz) and the relative notes'
/// ratios (from 1), 22 octaves of each, worked out as the original does.
fn tables_of(base: f64) -> Vec<f32> {
    const MUL: f64 = 1.0594630943593;
    let mut v = Vec::with_capacity(22 * 12);
    let mut octave = base;
    for _ in 0..22 {
        let mut f = octave;
        for _ in 0..12 {
            v.push(f as f32);
            f *= MUL;
        }
        octave *= 2.0;
    }
    v
}

fn read_instrument(r: &mut Reader, version: i32) -> Option<Instrument> {
    let points = |r: &mut Reader| -> Option<Vec<(u8, u8)>> {
        let n = r.u8();
        if n < 1 {
            return None;
        }
        Some((0..n).map(|_| (r.u8(), r.u8())).collect())
    };
    let filter = points(r)?;
    let amp = points(r)?;
    let filter_mode = r.u8();
    let clip = r.u8();
    let (vib_speed, vib_depth, pwm_speed, pwm_depth, resonance, sps) = (r.u8(), r.u8(), r.u8(), r.u8(), r.u8(), r.u8());
    if sps < 1 {
        return None;
    }
    let (len, loop_at) = if version < 900 {
        let x = r.u8();
        let len = x & 127;
        (len, if x & 1 != 0 { 0 } else { len.wrapping_sub(1) })
    } else {
        let (len, loop_at) = (r.u8(), r.u8());
        if loop_at >= len {
            return None;
        }
        (len, loop_at)
    };
    if len < 1 {
        return None;
    }
    let steps = (0..len)
        .map(|_| {
            let x = r.u8();
            InsStep {
                relative: x & 0x80 != 0,
                wave: x & 0xF,
                note: r.u8(),
            }
        })
        .collect();
    Some(Instrument {
        name: String::new(),
        filter,
        amp,
        filter_mode,
        clip_mode: clip >> 4,
        boost: clip & 15,
        vib_speed,
        vib_depth,
        pwm_speed,
        pwm_depth,
        resonance,
        sps,
        loop_at,
        steps,
    })
}

impl Song {
    pub fn parse(d: &[u8]) -> Option<Song> {
        if !probe(d) {
            return None;
        }
        let text = d.starts_with(b"SWTT");
        let mut r = Reader { d, at: 4, text, eof: false };
        let version = r.val(2) as u16;
        if i32::from(version) > VERSION {
            return None;
        }
        let v = i32::from(version);
        let spspal = if v < 900 { 882 } else { r.val(2) as u16 };
        if spspal < 1 {
            return None;
        }
        let nch = usize::from(r.u8());
        if !(1..=MAX_CHANNELS).contains(&nch) {
            return None;
        }
        let mut channels = Vec::with_capacity(nch);
        for _ in 0..nch {
            let (left, right) = (r.u8(), r.u8());
            let len = r.val(2) as u16;
            let lloop = if v < 910 { 0 } else { r.val(2) as u16 };
            let mut rloop = if v < 1200 { len.wrapping_sub(1) } else { r.val(2) as u16 };
            if len < 1 || usize::from(len) > MAX_CHANNEL_STEPS {
                return None;
            }
            if rloop >= len {
                rloop = len - 1;
            }
            let steps = (0..len)
                .map(|_| ChStep {
                    part: r.u8(),
                    transpose: r.u8() as i8,
                    damp: r.u8(),
                })
                .collect();
            channels.push(Channel { left, right, lloop, rloop, steps });
        }
        let nparts = usize::from(r.u8());
        if nparts < 1 {
            return None;
        }
        let mut parts = Vec::with_capacity(nparts);
        for _ in 0..nparts {
            let sps = r.u8();
            if sps < 1 {
                return None;
            }
            let len = r.u8();
            if len < 1 {
                return None;
            }
            let steps = (0..len)
                .map(|_| Step {
                    ins: r.u8(),
                    eff: r.u8(),
                    note: r.u8(),
                })
                .collect();
            parts.push(Part { name: String::new(), sps, steps });
        }
        for ch in &mut channels {
            for s in &mut ch.steps {
                if usize::from(s.part) >= nparts {
                    s.part = (nparts - 1) as u8;
                }
            }
            // A loop start past the end would play nothing that's there.
            ch.lloop = ch.lloop.min(ch.steps.len() as u16 - 1);
        }
        // Its count is kept in a byte with instrument 0's added.
        let nins = r.u8().wrapping_add(1);
        if nins < 2 {
            return None;
        }
        let mut instruments = vec![Instrument::none()];
        for _ in 1..nins {
            instruments.push(read_instrument(&mut r, v)?);
        }
        let breakpoints = r.u8();
        for _ in 0..breakpoints {
            r.val(4);
            r.val(4);
        }
        let title = r.name();
        let author = r.name();
        for p in &mut parts {
            p.name = r.name();
        }
        for i in instruments.iter_mut().skip(1) {
            i.name = r.name();
        }
        if parts.iter().any(|p| p.steps.iter().any(|s| s.ins >= nins)) {
            return None;
        }
        let mut song = Song {
            version,
            spspal,
            channels,
            parts,
            instruments,
            title,
            author,
            text,
            windows: Vec::new(),
            n2f: tables_of(1.021_974_864_455_477_1),
            r2f: tables_of(1.0),
            file: Arc::new(d.to_vec()),
        };
        song.windows = song.layout();
        Some(song)
    }

    /// The part channel `c` plays at its step `s`.
    fn part_at(&self, c: usize, s: usize) -> &Part {
        &self.parts[usize::from(self.channels[c].steps[s].part)]
    }

    /// Ticks the song plays before it loops (the first channel's list to
    /// its loop end).
    pub fn ticks(&self) -> u32 {
        (0..=usize::from(self.channels[0].rloop)).map(|s| self.part_at(0, s).ticks()).sum()
    }

    /// Channel `c`'s steps that do something, as it plays them (looping
    /// on its own), up to `until` ticks.
    pub fn timeline(&self, c: usize, until: u32) -> Vec<Event> {
        let ch = &self.channels[c];
        let mut out = Vec::new();
        let (mut t, mut s) = (0u32, 0usize);
        while t < until {
            let cs = ch.steps[s];
            let part = &self.parts[usize::from(cs.part)];
            for (k, st) in part.steps.iter().enumerate() {
                let at = t + k as u32 * u32::from(part.sps);
                if at >= until {
                    break;
                }
                if st.note != 0 || st.ins != 0 || st.eff != 0 {
                    out.push(Event {
                        tick: at,
                        step: *st,
                        transpose: cs.transpose,
                    });
                }
            }
            t += part.ticks();
            s += 1;
            if s > usize::from(ch.rloop) {
                s = usize::from(ch.lloop);
            }
        }
        out
    }

    /// Where each of the first channel's steps starts, in ticks, played
    /// straight through.
    pub fn starts(&self) -> Vec<u32> {
        let mut t = 0;
        (0..self.channels[0].steps.len())
            .map(|s| {
                let at = t;
                t += self.part_at(0, s).ticks();
                at
            })
            .collect()
    }

    /// The first channel's steps as patterns: each its part's length, a
    /// row as many ticks as every channel's steps in it fall on (a
    /// coarser one if that's more than a pattern's rows).
    fn layout(&self) -> Vec<Window> {
        let starts = self.starts();
        let end = starts.last().copied().unwrap_or(0) + self.part_at(0, starts.len() - 1).ticks();
        let events: Vec<Vec<Event>> = (0..self.channels.len()).map(|c| self.timeline(c, end)).collect();
        starts
            .iter()
            .enumerate()
            .map(|(s, &at)| {
                let part = self.part_at(0, s);
                let ticks = part.ticks();
                let mut grain = u32::from(part.sps);
                for e in events.iter().flatten() {
                    if e.tick >= at && e.tick < at + ticks {
                        grain = gcd(grain, e.tick - at);
                    }
                }
                if ticks.div_ceil(grain) > MAX_ROWS {
                    grain = ticks.div_ceil(MAX_ROWS);
                }
                Window { ticks, grain: grain.max(1) }
            })
            .collect()
    }

    /// The tracker's note for a step's note and transposition, named as
    /// Sawteeth's editor names it (its note 0 is C-0). What's heard is
    /// the instrument's: most multiply it by a relative step (36, three
    /// octaves up, or more).
    pub fn note(note: u8, transpose: i8) -> i32 {
        i32::from(note) + i32::from(transpose) + 1
    }
}

fn gcd(a: u32, b: u32) -> u32 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

impl Tune for Song {
    fn format(&self) -> &'static str {
        "st"
    }
    fn file(&self) -> &[u8] {
        &self.file
    }
    fn driver(&self, _song: usize) -> Box<dyn Driver> {
        Box::new(Player::new(Arc::new(self.clone())))
    }
    fn length(&self, _song: usize) -> Option<f64> {
        // Playing it would make every tick's sound; its lists say as much.
        // It comes round on the tick after its last.
        Some(f64::from(self.ticks() + 1) * f64::from(self.spspal) / RATE)
    }

    /// Each of the file's instruments: its envelopes, settings and list.
    fn patches(&self) -> Vec<Patch> {
        let mut out = Vec::new();
        for (k, i) in self.instruments.iter().enumerate().skip(1) {
            let mut b = PatchBytes::default();
            for env in [&i.filter, &i.amp] {
                b.u16(env.len() as u16);
                for &(t, l) in env.iter() {
                    b.u8(t).u8(l);
                }
            }
            for v in [i.filter_mode, i.clip_mode, i.boost, i.vib_speed, i.vib_depth, i.pwm_speed, i.pwm_depth, i.resonance, i.sps, i.loop_at] {
                b.u8(v);
            }
            b.u16(i.steps.len() as u16);
            for x in &i.steps {
                b.u8(x.note).u8(u8::from(x.relative)).u8(x.wave);
            }
            out.push(b.patch(k, &i.name));
        }
        out
    }
}

/// The sine and triangle tables, 512 points and the first again.
struct Tables {
    sin: [f32; 513],
    tri: [f32; 513],
}

fn tables() -> &'static Tables {
    static T: OnceLock<Tables> = OnceLock::new();
    T.get_or_init(|| {
        let mut sin = [0f32; 513];
        for (c, s) in sin.iter_mut().enumerate() {
            *s = (c as f32 * (2f32 * PI) / 512f32).sin();
        }
        let mut tri = [0f32; 513];
        let (mut x, add) = (-1f32, 4f32 / 512f32);
        for t in &mut tri[..256] {
            *t = x;
            x += add;
        }
        for t in &mut tri[256..512] {
            *t = x;
            x -= add;
        }
        tri[512] = tri[0];
        Tables { sin, tri }
    })
}

/// The original's random numbers (Park and Miller's), one sequence the
/// whole song's noise shares.
fn random(seed: &mut i64) -> u64 {
    *seed = 16807 * (*seed % 127_773) - (*seed / 127_773) * 2836;
    if *seed < 0 {
        *seed += 0x7FFF_FFFF;
    }
    *seed as u64
}

const SAW: u8 = 1;
const SQUARE: u8 = 2;
const TRIANGLE: u8 = 3;
const NOISE: u8 = 4;
const SINE: u8 = 5;
/// A triangle and a sine from the tables with no interpolation.
const TRIANGLE_U: u8 = 6;
const SINE_U: u8 = 7;

/// An oscillator. The sawtooth, square and noise walk `curr` from -1 to
/// 1 a cycle, each step placed between samples where it falls.
#[derive(Clone, Default)]
struct Wave {
    form: u8,
    pwm_lo: bool,
    amp: f32,
    /// The level the last buffer ended at, ramped from to `amp`.
    from_amp: f32,
    pwm: f32,
    /// The triangle's place, -1 to 1.
    currval: f32,
    curr: f32,
    step: f32,
    noise: f32,
    sin_at: u32,
    sin_step: u32,
}

impl Wave {
    fn new() -> Wave {
        let mut w = Wave::default();
        w.set_form(SAW);
        w.set_freq(440.0);
        w
    }

    fn set_freq(&mut self, freq: f32) {
        self.step = freq * INV_RATE;
        if f64::from(self.step) > 1.9 {
            self.step = 1.9f64 as f32;
        }
        self.sin_step = ((1u32 << 22) as f32 * 512f32 * self.step) as u32;
    }

    fn set_pwm(&mut self, p: f32) {
        let lim = 0.9f64 as f32;
        self.pwm = if p > lim {
            lim
        } else if p < -lim {
            -lim
        } else {
            p
        };
    }

    fn set_form(&mut self, form: u8) {
        if self.form != form {
            if self.currval > 1.0 {
                self.currval = 0.0;
            }
            if self.curr > 1.0 {
                self.curr = 0.0;
            }
        }
        self.form = form;
    }

    /// The next buffer; false (and nothing moves on) when it's silent.
    fn next(&mut self, b: &mut [f32], seed: &mut i64) -> bool {
        if f64::from(self.amp) < 0.001 {
            return false;
        }
        match self.form {
            SAW => self.saw(b),
            SQUARE => self.square(b),
            NOISE => self.noise(b, seed),
            TRIANGLE => self.triangle(b),
            SINE => self.sine(b),
            TRIANGLE_U => self.table(b, &tables().tri),
            SINE_U => self.table(b, &tables().sin),
            _ => return false,
        }
        true
    }

    /// Samples to go before `curr` reaches 1 (at least, as the original
    /// counts; never below 0 here, where it would run on in the original).
    fn walk(&self) -> usize {
        let walkdiff = 1f32 - self.curr;
        ((walkdiff / self.step + 1f32) as i32).max(0) as usize
    }

    fn saw(&mut self, out: &mut [f32]) {
        let add = (self.amp - self.from_amp) / out.len() as f32;
        let mut amp = self.from_amp;
        let mut o = 0;
        while o < out.len() {
            if self.curr >= 1.0 {
                // Where between this sample and the last it wrapped.
                let d = (self.curr - 1f32) / self.step;
                self.curr -= 2.0;
                out[o] = amp * (-2f32 * d + 1f32);
                o += 1;
                amp += add;
                self.curr += self.step;
            }
            let steps = (out.len() - o).min(self.walk());
            for x in &mut out[o..o + steps] {
                *x = amp * self.curr;
                amp += add;
                self.curr += self.step;
            }
            o += steps;
        }
        self.from_amp = amp;
    }

    fn square(&mut self, out: &mut [f32]) {
        let add = (self.amp - self.from_amp) / out.len() as f32;
        let mut amp = self.from_amp;
        let mut o = 0;
        while o < out.len() {
            if self.curr >= 1.0 {
                let d = ((f64::from(self.curr) - 1.0) / f64::from(self.step)) as f32;
                self.curr -= 1.0;
                if self.pwm_lo {
                    out[o] = amp * d + (1f32 - d) * -amp;
                    self.curr -= self.pwm;
                } else {
                    out[o] = -amp * d + (1f32 - d) * amp;
                    self.curr += self.pwm;
                }
                self.pwm_lo = !self.pwm_lo;
                o += 1;
                amp += add;
                self.curr += self.step;
            }
            let steps = (out.len() - o).min(self.walk());
            let (mut a, a_add) = if self.pwm_lo { (-amp, -add) } else { (amp, add) };
            for x in &mut out[o..o + steps] {
                *x = a;
                a += a_add;
            }
            amp += steps as i32 as f32 * add;
            self.curr += steps as i32 as f32 * self.step;
            o += steps;
        }
        self.from_amp = amp;
    }

    fn noise(&mut self, out: &mut [f32], seed: &mut i64) {
        let add = (self.amp - self.from_amp) / out.len() as f32;
        let mut amp = self.from_amp;
        let mut o = 0;
        while o < out.len() {
            if self.curr >= 1.0 {
                self.curr -= 2.0;
                self.noise = amp * (random(seed) as f32 / (64.0 * 256.0 * 256.0 * 256.0f64) as f32 - 1f32);
            }
            let steps = (out.len() - o).min(self.walk());
            out[o..o + steps].fill(self.noise);
            self.curr += steps as i32 as f32 * self.step;
            amp += steps as i32 as f32 * add;
            o += steps;
        }
        self.from_amp = amp;
        if self.curr >= 1.0 {
            self.curr -= 2.0;
        }
    }

    fn triangle(&mut self, b: &mut [f32]) {
        let add = (self.amp - self.from_amp) / b.len() as f32;
        let mut amp = self.from_amp;
        for x in b {
            let v = if self.currval > 0.0 { -self.currval } else { self.currval };
            *x = amp * (2f32 * v + 1f32);
            self.currval += self.step;
            if self.currval > 1.0 {
                self.currval -= 2.0;
            }
            amp += add;
        }
        self.from_amp = amp;
    }

    fn sine(&mut self, b: &mut [f32]) {
        let sin = &tables().sin;
        let add = (self.amp - self.from_amp) / b.len() as f32;
        let mut amp = self.from_amp;
        for x in b {
            self.sin_at = self.sin_at.wrapping_add(self.sin_step);
            let pos = (self.sin_at >> 23) as usize;
            let dec = (self.sin_at & ((1 << 23) - 1)) as f32 / ((1 << 23) - 1) as f32;
            *x = amp * (sin[pos + 1] * dec + sin[pos] * (1f32 - dec));
            amp += add;
        }
        self.from_amp = amp;
    }

    fn table(&mut self, b: &mut [f32], t: &[f32; 513]) {
        let add = (self.amp - self.from_amp) / b.len() as f32;
        let mut amp = self.from_amp;
        for x in b {
            self.sin_at = self.sin_at.wrapping_add(self.sin_step);
            *x = amp * t[(self.sin_at >> 23) as usize];
            amp += add;
        }
        self.from_amp = amp;
    }
}

/// A low-frequency oscillator: a cosine of a phase that only grows.
#[derive(Clone, Copy)]
struct Lfo {
    curr: f32,
    step: f32,
}

impl Lfo {
    fn set_freq(&mut self, f: f32) {
        self.step = f * LFO_STEP;
    }

    fn next(&mut self) -> f32 {
        self.curr += self.step;
        self.curr.cos()
    }
}

/// An instrument playing: its list, envelopes, LFOs, filter and clipper.
#[derive(Clone)]
struct InsPly {
    vib: Lfo,
    pwm: Lfo,
    vamp: f32,
    pamp: f32,
    pwm_offs: f32,
    wave: Wave,
    ins: usize,
    step: InsStep,
    stepc: usize,
    nexts: i32,
    trigged: bool,
    curr_amp: f32,
    amp_step: f32,
    adsr: usize,
    next_adsr: i32,
    curr_f: f32,
    f_step: f32,
    fadsr: usize,
    next_fadsr: i32,
    ins_freq: f32,
    // Set by the channel each tick.
    part_freq: f32,
    part_amp: f32,
    part_co: f32,
    res: f32,
    amp: f32,
    cutoff: f32,
    lo: f32,
    hi: f32,
    bp: f32,
    bs: f32,
}

impl InsPly {
    fn new(song: &Song) -> InsPly {
        let lfo = Lfo { curr: 0.0, step: LFO_STEP };
        let mut p = InsPly {
            vib: lfo,
            pwm: lfo,
            vamp: 0.0,
            pamp: 0.0,
            pwm_offs: 0.0,
            wave: Wave::new(),
            ins: 0,
            step: InsStep::default(),
            stepc: 0,
            nexts: 0,
            trigged: false,
            curr_amp: 0.0,
            amp_step: 0.0,
            adsr: 0,
            next_adsr: 0,
            curr_f: 0.0,
            f_step: 0.0,
            fadsr: 0,
            next_fadsr: 0,
            ins_freq: 440.0,
            part_freq: 440.0,
            part_amp: 0.0,
            part_co: 0.0,
            res: 0.0,
            amp: 0.0,
            cutoff: 0.0,
            lo: 0.0,
            hi: 0.0,
            bp: 0.0,
            bs: 0.0,
        };
        p.trig(song, 0);
        p
    }

    fn trig(&mut self, song: &Song, i: usize) {
        let ins = &song.instruments[i];
        self.pwm_offs = 0.0;
        self.trigged = true;
        self.ins = i;
        self.vib.set_freq(f32::from(ins.vib_speed) * 50f32);
        self.vamp = f32::from(ins.vib_depth) / 2000f32;
        self.pwm.set_freq(f32::from(ins.pwm_speed) * 5f32);
        self.pamp = f32::from(ins.pwm_depth) / 255.1f64 as f32;
        self.set_reso(f32::from(ins.resonance) / 255f32);
        self.adsr = 0;
        self.next_adsr = 0;
        self.fadsr = 0;
        self.next_fadsr = 0;
        self.stepc = 0;
        self.nexts = 0;
    }

    fn set_reso(&mut self, a: f32) {
        let r = 1f32 - a;
        self.res = 1f32 - r * r;
    }

    fn next(&mut self, song: &Song, b: &mut [f32], seed: &mut i64) -> bool {
        let ins = &song.instruments[self.ins];
        if self.nexts < 1 {
            self.step = ins.steps[self.stepc];
            self.nexts = i32::from(ins.sps);
            if self.step.note != 0 {
                let n = usize::from(self.step.note);
                self.ins_freq = if self.step.relative { song.r2f[n] } else { song.n2f[n] };
            }
            if self.step.wave != 0 {
                self.wave.set_form(self.step.wave);
            }
            self.stepc += 1;
            if self.stepc >= ins.steps.len() {
                self.stepc = usize::from(ins.loop_at);
            }
        }
        if self.next_adsr < 1 {
            if let Some(&(time, lev)) = ins.amp.get(self.adsr) {
                self.next_adsr = 1 + i32::from(time);
                self.amp_step = ((f64::from(lev) / 257.0 - f64::from(self.curr_amp)) as f32) / self.next_adsr as f32;
                self.adsr += 1;
            } else {
                self.amp_step = 0.0;
            }
        }
        if self.next_fadsr < 1 {
            if let Some(&(time, lev)) = ins.filter.get(self.fadsr) {
                self.next_fadsr = 1 + i32::from(time);
                let t = f32::from(lev) / 257f32;
                let target = t * (t * t);
                self.f_step = (target - self.curr_f) / self.next_fadsr as f32;
                self.fadsr += 1;
            } else {
                self.f_step = 0.0;
            }
        }
        self.next_adsr -= 1;
        self.next_fadsr -= 1;
        self.nexts -= 1;

        let vib = 1f32 + self.vamp * self.vib.next();
        if self.step.relative {
            self.wave.set_freq(self.part_freq * self.ins_freq * vib);
        } else {
            self.wave.set_freq(self.ins_freq * vib);
        }
        let pwm = self.pwm.next();
        self.wave.set_pwm(self.pwm_offs + self.pamp * pwm);
        self.curr_amp += self.amp_step;
        self.amp = self.curr_amp * self.part_amp;
        self.curr_f += self.f_step;
        self.cutoff = self.curr_f * self.part_co;

        self.wave.amp = self.amp;
        if self.trigged {
            self.trigged = false;
            self.wave.from_amp = self.wave.amp;
        }
        if !self.wave.next(b, seed) {
            return false;
        }
        match ins.filter_mode {
            1 => {
                for x in b.iter_mut() {
                    self.lo = self.cutoff * *x + self.lo * (1f32 - self.cutoff);
                    *x = self.lo;
                }
            }
            2 => {
                for x in b.iter_mut() {
                    self.lo = self.cutoff * *x + (1f32 - self.cutoff) * self.hi;
                    self.lo += (self.lo - self.bp) * self.res;
                    self.bp = self.hi;
                    self.hi = self.lo;
                    *x = self.lo;
                }
            }
            m @ 3..=6 => {
                // The state variable filter, its low-pass clipped at the amp.
                let damp = 1.8f64 as f32 - self.res * 1.8f64 as f32;
                for x in b.iter_mut() {
                    let t = self.lo + self.cutoff * self.bp;
                    self.hi = *x - self.lo - damp * self.bp;
                    self.bp += self.cutoff * self.hi;
                    self.lo = if t < -self.amp {
                        -self.amp
                    } else if t > self.amp {
                        self.amp
                    } else {
                        t
                    };
                    self.bs = self.lo + self.hi;
                    *x = match m {
                        3 => self.lo,
                        4 => self.hi,
                        5 => self.bp,
                        _ => self.bs,
                    };
                }
            }
            _ => {}
        }
        match ins.clip_mode {
            1 => {
                let mul = 2f32 * (1 + i32::from(ins.boost)) as f32;
                let near_one = (mul - 1f32).abs() < 0.1f64 as f32;
                for x in b.iter_mut() {
                    if !near_one {
                        *x *= mul;
                    }
                    *x = x.clamp(-1.0, 1.0);
                }
            }
            2 => {
                let mul = 0.7f64 as f32 * (1.3 + f64::from(ins.boost)) as f32;
                if (mul - 1f32).abs() < 0.1f64 as f32 {
                    for x in b.iter_mut() {
                        *x = x.sin();
                    }
                } else {
                    // The boost applied twice, as the original does.
                    for x in b.iter_mut() {
                        *x *= mul;
                        *x = (*x * mul).sin();
                    }
                }
            }
            _ => {}
        }
        true
    }
}

/// A channel playing its list of parts.
#[derive(Clone)]
struct Voice {
    ins: InsPly,
    seq: usize,
    stepc: usize,
    nexts: i32,
    part: usize,
    damp: f32,
    amp: f32,
    amp_step: f32,
    freq: f32,
    freq_step: f32,
    target_freq: f32,
    cutoff: f32,
    cutoff_step: f32,
    looped: bool,
    loop_pending: bool,
    /// The step playing: its place in the list, in its part, and its part's speed.
    shown: (usize, usize, i32),
}

impl Voice {
    fn new(song: &Song, c: usize) -> Voice {
        let cs = song.channels[c].steps[0];
        Voice {
            ins: InsPly::new(song),
            seq: 0,
            stepc: 0,
            nexts: 0,
            part: usize::from(cs.part),
            damp: (255f32 - f32::from(cs.damp)) / 255f32,
            amp: 1.0,
            amp_step: 0.0,
            freq: 1.0,
            freq_step: 1.0,
            target_freq: 0.0,
            cutoff: 1.0,
            cutoff_step: 1.0,
            looped: false,
            loop_pending: false,
            shown: (0, 0, 1),
        }
    }

    /// The next tick's buffer; false when it's silent.
    fn next(&mut self, song: &Song, c: usize, b: &mut [f32], seed: &mut i64) -> bool {
        let ch = &song.channels[c];
        if self.nexts < 1 {
            let cs = ch.steps[self.seq];
            self.damp = (255f32 - f32::from(cs.damp)) / 255f32;
            let part = &song.parts[self.part];
            self.nexts = i32::from(part.sps);
            self.shown = (self.seq, self.stepc, self.nexts);
            let st = part.steps[self.stepc];
            if st.ins != 0 {
                self.ins.trig(song, usize::from(st.ins));
                self.amp = 1.0;
                self.amp_step = 0.0;
                self.cutoff = 1.0;
                self.cutoff_step = 1.0;
            }
            if st.note != 0 {
                let n = (i32::from(st.note) + i32::from(cs.transpose)).clamp(0, song.n2f.len() as i32 - 1);
                self.target_freq = song.n2f[n as usize];
                self.freq_step = 1.0;
                if st.eff & 0xF0 != 0x30 {
                    self.freq = self.target_freq;
                }
            }
            let x = i32::from(st.eff & 0xF);
            let speed = (x + 1) as f32;
            let divider = 50f32;
            match st.eff & 0xF0 {
                0x10 => {
                    self.target_freq = 44100.0;
                    self.freq_step = 1f32 + speed / (divider * 3f32);
                }
                0x20 => {
                    self.target_freq = 1.0;
                    self.freq_step = 1f32 - speed / (divider * 3f32);
                }
                0x30 => self.freq_step = if self.target_freq > self.freq { 1f32 + speed / divider } else { 1f32 - speed / divider },
                0x40 => self.ins.pwm_offs = x as f32 / 16.1f64 as f32,
                0x50 => self.ins.set_reso(x as f32 / 15f32),
                0x70 => self.cutoff_step = 1f32 - speed / 256f32,
                0x80 => self.cutoff_step = 1f32 + speed / 256f32,
                0x90 => {
                    self.cutoff_step = 1.0;
                    let co = speed / 16f32;
                    self.cutoff = co * co;
                }
                0xA0 => self.amp_step = -(x + 1) as f32 / 256f32,
                0xB0 => self.amp_step = speed / 256f32,
                0xC0 => {
                    self.amp_step = 0.0;
                    self.amp = x as f32 / 15f32;
                }
                _ => {}
            }
            self.looped = std::mem::take(&mut self.loop_pending);
            self.stepc += 1;
            if self.stepc >= part.steps.len() {
                self.stepc = 0;
                self.seq += 1;
                if self.seq > usize::from(ch.rloop) {
                    self.seq = usize::from(ch.lloop);
                    self.loop_pending = true;
                }
                self.part = usize::from(ch.steps[self.seq].part);
            }
        }
        self.nexts -= 1;

        self.cutoff *= self.cutoff_step;
        if self.cutoff < 0.0 {
            self.cutoff = 0.0;
            self.cutoff_step = 1.0;
        } else if self.cutoff > 1.0 {
            self.cutoff = 1.0;
            self.cutoff_step = 1.0;
        }
        self.freq *= self.freq_step;
        if f64::from(self.freq_step) > 1.0001 {
            if self.freq > self.target_freq {
                self.freq = self.target_freq;
                self.freq_step = 1.0;
            }
        } else if f64::from(self.freq_step) < 0.9999 && self.freq < self.target_freq {
            self.freq = self.target_freq;
            self.freq_step = 1.0;
        }
        self.amp += self.amp_step;
        if self.amp < 0.0 {
            self.amp = 0.0;
            self.amp_step = 0.0;
        } else if self.amp > 1.0 {
            self.amp = 1.0;
            self.amp_step = 0.0;
        }
        self.ins.part_amp = self.amp * self.damp;
        self.ins.part_freq = self.freq;
        self.ins.part_co = self.cutoff;
        self.ins.next(song, b, seed)
    }
}

/// The replayer: every channel's buffer made a tick at a time at 44.1
/// kHz, resampled to the host's rate a sample behind (so a buffer's
/// last samples need nothing of the next one's).
pub struct Player {
    song: Arc<Song>,
    voices: Vec<Voice>,
    bufs: Vec<Vec<f32>>,
    active: Vec<bool>,
    /// Each channel's last sample before its buffer now.
    prev: Vec<f32>,
    /// Each channel's level to the left and right (the original's mix).
    gains: Vec<(f32, f32)>,
    seed: i64,
    /// Where rendering is in the buffers, in samples at 44.1 kHz.
    pos: f64,
    done: bool,
}

impl Player {
    pub fn new(song: Arc<Song>) -> Player {
        let n = song.channels.len();
        let floor = 0.1f64 as f32;
        // The original's mix: quieter the more channels the song has.
        let mul = ((1f32 - floor) / n as f32 + floor) / 255f32;
        let gains = song.channels.iter().map(|c| (f32::from(c.left) * mul, f32::from(c.right) * mul)).collect();
        let len = usize::from(song.spspal);
        Player {
            voices: (0..n).map(|c| Voice::new(&song, c)).collect(),
            bufs: vec![vec![0.0; len]; n],
            active: vec![false; n],
            prev: vec![0.0; n],
            gains,
            seed: 1,
            pos: 0.0,
            done: false,
            song,
        }
    }

    /// The tick's samples mixed as the original mixes them, at 44.1 kHz,
    /// left and right interleaved.
    pub fn mix_native(&self, out: &mut [f32]) {
        let mut first = true;
        for c in 0..self.voices.len() {
            if !self.active[c] {
                continue;
            }
            let (l, r) = self.gains[c];
            for (o, &s) in out.as_chunks_mut::<2>().0.iter_mut().zip(&self.bufs[c]) {
                if first {
                    o[0] = s * l;
                    o[1] = s * r;
                } else {
                    o[0] += s * l;
                    o[1] += s * r;
                }
            }
            first = false;
        }
        if first {
            out.fill(0.0);
        }
    }
}

impl Driver for Player {
    fn channels(&self) -> usize {
        self.voices.len()
    }

    fn tick(&mut self) {
        let song = self.song.clone();
        let n = usize::from(song.spspal);
        for (c, v) in self.voices.iter_mut().enumerate() {
            let b = &mut self.bufs[c];
            self.prev[c] = b[n - 1];
            self.active[c] = v.next(&song, c, b, &mut self.seed);
            if !self.active[c] {
                b.fill(0.0);
            }
        }
        if self.voices[0].looped {
            self.done = true;
        }
        self.pos = (self.pos - n as f64).max(0.0);
    }

    fn tick_seconds(&self) -> f64 {
        f64::from(self.song.spspal) / RATE
    }

    fn render(&mut self, rate: u32, frames: usize, out: &mut Bufs) {
        let step = RATE / f64::from(rate.max(1));
        let n = usize::from(self.song.spspal);
        for c in 0..self.voices.len() {
            let (gl, gr) = (self.gains[c].0 * LEVEL, self.gains[c].1 * LEVEL);
            let (b, prev) = (&self.bufs[c], self.prev[c]);
            let at = |k: isize| if k < 0 { prev } else { b[(k as usize).min(n - 1)] };
            let (l, r) = (&mut out.left[c][..frames], &mut out.right[c][..frames]);
            for i in 0..frames {
                let x = self.pos + i as f64 * step - 1.0;
                let k = x.floor();
                let f = (x - k) as f32;
                let (s0, s1) = (at(k as isize), at(k as isize + 1));
                let s = s0 + (s1 - s0) * f;
                l[i] = s * gl;
                r[i] = s * gr;
            }
        }
        self.pos += frames as f64 * step;
    }

    fn position(&self) -> (usize, usize) {
        let (seq, step, sps) = self.voices[0].shown;
        let w = self.song.windows[seq.min(self.song.windows.len() - 1)];
        let ticks = step as i32 * sps + (sps - 1 - self.voices[0].nexts).max(0);
        (seq, ticks as usize / w.grain as usize)
    }

    fn speed(&self) -> (u32, u32) {
        let seq = self.voices[0].shown.0.min(self.song.windows.len() - 1);
        (self.song.windows[seq].grain, (110_250.0 / f64::from(self.song.spspal)).round() as u32)
    }

    fn take_done(&mut self) -> bool {
        std::mem::take(&mut self.done)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay::Machine;

    /// A binary song: one channel, a part of a C-5 on instrument 1 and
    /// three empty steps, a sawtooth held at full volume.
    fn song() -> Vec<u8> {
        let mut d = b"SWTD".to_vec();
        d.extend([0x04, 0xB0, 0x03, 0x72]); // version 1200, 882 samples a tick
        d.push(1); // channels
        d.extend([255, 128, 0, 1, 0, 0, 0, 0]); // left, right, len, lloop, rloop
        d.extend([0, 0, 0]); // part 0, no transposition or damping
        d.push(1); // parts
        d.extend([6, 4]); // 6 ticks a step, 4 steps
        d.extend([1, 0, 60, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        d.push(1); // instruments
        d.extend([1, 0, 255]); // filter: a point
        d.extend([1, 0, 255]); // amp: a point
        d.extend([0, 0]); // no filter, no clipping
        d.extend([0, 0, 0, 0, 0, 1]); // no vibrato or PWM, no resonance, a tick a step
        d.extend([1, 0]); // a step long, looping to it
        d.extend([0x01, 0]); // the sawtooth, no note
        d.push(0); // breakpoints
        d.extend(b"Test\0Me\0Part\0Saw\0");
        d
    }

    #[test]
    fn reads_and_plays() {
        let s = Song::parse(&song()).expect("parses");
        assert_eq!((s.title.as_str(), s.author.as_str()), ("Test", "Me"));
        assert_eq!(s.instruments[1].name, "Saw");
        assert_eq!(s.ticks(), 24);
        assert_eq!(s.windows[0].grain, 6);
        let tune: Arc<dyn Tune> = Arc::new(s);
        // Comes round after its 24 ticks (the tick that starts it again),
        // as playing it finds.
        let len = Machine::length(&tune, 0, 10.0);
        assert!((len - 25.0 * 0.02).abs() < 1e-9, "{len}");
        let mut d = tune.driver(0);
        let mut t = 0.0;
        while !d.take_done() {
            d.tick();
            t += d.tick_seconds();
        }
        assert!((t - len).abs() < 1e-9, "{t}");
        let mut m = Machine::new(tune, 0);
        let (mut l, mut r) = (vec![0.0; 4410], vec![0.0; 4410]);
        m.render(44_100, 1.0, 1.0, &mut l, &mut r);
        let peak = l.iter().fold(0f32, |a, x| a.max(x.abs()));
        // One channel, hard left at full volume: the Tracker's level.
        assert!(peak > 0.3 && peak <= 0.41, "{peak}");
        assert!(r.iter().fold(0f32, |a, x| a.max(x.abs())) < peak * 0.6);
    }

    #[test]
    fn text_files_read_as_binary_ones() {
        let t = "SWTT\n\n// version\n1200\n882\n1\n255\n128\n1\n0\n0\n0\n0\n0\n1\n6\n4\n1\n0\n60\n0\n0\n0\n0\n0\n0\n0\n0\n0\n1\n1\n0\n255\n1\n0\n255\n0\n0\n0\n0\n0\n0\n0\n1\n1\n0\n1\n0\n0\nTest\nMe\nPart\nSaw\n";
        let (a, b) = (Song::parse(t.as_bytes()).expect("parses"), Song::parse(&song()).expect("parses"));
        assert_eq!(format!("{:?}", a.parts), format!("{:?}", b.parts));
        assert_eq!(format!("{:?}", a.instruments), format!("{:?}", b.instruments));
        assert!(a.text && !b.text);
    }

    #[test]
    fn damaged_files_are_refused() {
        let d = song();
        for cut in [8, 12, 30] {
            assert!(Song::parse(&d[..cut]).is_none(), "{cut}");
        }
    }
}
