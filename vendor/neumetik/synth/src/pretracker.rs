// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! Pretracker (Pink/Abyss, 2019; "PRT" and a version byte): an Amiga
//! tracker whose sounds are all made before the song plays. Each of its
//! up to 24 waves is drawn once, at load: a saw, triangle, square or
//! noise oscillator (a chord of up to four notes, a unison copy, a pitch
//! ramp, a sweeping phase), through a moving four-pole filter, a
//! volume envelope and a comb-like modulator, mixed with another wave,
//! with two octaves above it squeezed in after. The song then plays the
//! waves on Paula: four channels of 3-byte pattern steps, each note an
//! instrument (a small pattern of its own that picks waves, slides and
//! loops through a wave's part, ping-pong), an ADSR, vibrato, and a track
//! delay that echoes a channel on the next.
//!
//! Played as the original V1.0 replayer plays it: ported from Raspberry
//! Casket, Chris Hodges's MIT-licensed rewrite of it, with its bug fixes
//! and changes left out where the original differs (checked against the
//! original's binary run on a 68000: the same sample memory and Paula
//! registers every tick, on 64 songs). Version $1E (Pretracker 1.5) files
//! are read as Raspberry Casket reads them: their first sub-song, and the
//! first 32 of their 64 instruments (the rest are sound effects). Its
//! notice:
//!
//! > MIT License
//! >
//! > Copyright (c) 2022 Chris 'platon42' Hodges <chrisly@platon42.de>
//! >
//! > Permission is hereby granted, free of charge, to any person obtaining
//! > a copy of this software and associated documentation files (the
//! > "Software"), to deal in the Software without restriction, including
//! > without limitation the rights to use, copy, modify, merge, publish,
//! > distribute, sublicense, and/or sell copies of the Software, and to
//! > permit persons to whom the Software is furnished to do so, subject to
//! > the following conditions:
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

use std::sync::Arc;

use crate::paula::{self, Paula};
use crate::replay::{Bufs, Driver, Patch, PatchBytes, Tune};

const MAX_WAVES: usize = 24;
const MAX_INSTRUMENTS: usize = 32;
const NOTE_OFF: u8 = 0x3d;
/// The bytes of a wave's description.
const WAVE_SIZE: usize = 42;
/// An oscillator buffer: a saw, square and triangle of up to 128 points
/// and their length.
const OWB: usize = 385;

/// Each note of the octave's oscillator period, from these fractions.
const LOG12: [u32; 12] = [0x8000, 0x871D, 0x8F2F, 0x97B7, 0x9FC4, 0xA9DE, 0xB505, 0xBF49, 0xCB31, 0xD645, 0xE215, 0xF1A0];
const VIB_SPEED: [u8; 16] = [2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 20, 40, 80];
const VIB_DEPTH: [u8; 16] = [0, 8, 9, 10, 11, 12, 13, 14, 18, 20, 28, 40, 50, 70, 160, 255];
const VIB_DELAY: [u8; 16] = [0, 4, 8, 10, 12, 14, 16, 18, 20, 24, 32, 40, 56, 96, 150, 255];
const RAMP_UP_16: [u8; 16] = [0, 1, 3, 6, 7, 9, 10, 11, 12, 13, 14, 16, 19, 35, 55, 143];
const FAST_ROLL_OFF_16: [u16; 16] = [0x400, 0x200, 0x80, 0x64, 0x50, 0x40, 0x30, 0x20, 0x10, 14, 12, 10, 8, 4, 2, 1];
const ROLL_OFF: [u16; 144] = [
    1024, 512, 384, 320, 256, 192, 160, 128, 120, 116, 110, 105, 100, 90, 70, 64, 56, 48, 40, 32, 31, 30, 29, 28, 27, 26, 25, 24, 23, 22, 21, 20, 19, 18, 17, 16, 15, 14, 13, 13, 12, 12, 11, 11, 10,
    10, 9, 9, 8, 8, 8, 8, 7, 7, 7, 7, 6, 6, 6, 6, 5, 5, 5, 5, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 3, 4, 4, 3, 4, 4, 3, 4, 3, 4, 3, 4, 3, 4, 3, 3, 3, 3, 3, 3, 3, 3, 3, 2, 3, 3, 3, 2, 3, 3, 2, 3, 3, 2, 3, 3,
    2, 3, 2, 3, 2, 3, 2, 3, 2, 3, 2, 2, 2, 2, 2, 2, 2, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 1, 2, 1, 1, 1, 2, 1,
];
/// Which of a wave's octaves a pitch above $219 plays (a step every 64),
/// and how far down that takes the pitch.
const OCTAVE_SELECT: [u8; 24] = [1, 1, 1, 2, 2, 2, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3];
const OCTAVE_NOTES: [u8; 24] = [48, 48, 48, 96, 96, 96, 144, 144, 144, 144, 144, 144, 144, 144, 144, 144, 144, 144, 144, 144, 144, 144, 144, 144];
/// The track delay's small pitch shift by how far back the echo is.
const MINUS4PLUS4: [i8; 8] = [-4, -5, -1, 1, 2, 3, 4, 0];
const RAMP_UP_DOWN_32: [u16; 32] = [
    0, 32, 64, 96, 128, 160, 192, 224, 256, 288, 320, 352, 384, 416, 448, 480, 480, 448, 416, 384, 352, 320, 288, 256, 224, 192, 160, 128, 96, 64, 32, 0,
];
const MODULATOR_RAMP_8: [u16; 8] = [0x4D, 0x125, 0x21B, 0x437, 0x539, 0x755, 0x96D, 0xBD7];
/// The periods of three octaves (and the top again), filled in to 16 steps a semitone.
const PERIODS: [u16; 37] = [
    0x350, 0x320, 0x2F2, 0x2C8, 0x2A0, 0x279, 0x256, 0x236, 0x216, 0x1F8, 0x1DC, 0x1C0, 0x1A8, 0x190, 0x179, 0x164, 0x151, 0x13E, 0x12C, 0x11B, 0x10B, 0xFC, 0xEE, 0xE0, 0xD4, 0xC8, 0xBD, 0xB2, 0xA8,
    0x9F, 0x96, 0x8D, 0x86, 0x7E, 0x78, 0x71, 0x71,
];

/// An instrument's settings, unpacked as the replayer keeps them.
#[derive(Clone, Copy, Debug, Default)]
struct Inst {
    vib_delay: u16,
    vib_depth: i16,
    vib_speed: i16,
    release: u8,
    attack: u16,
    decay: u8,
    sustain: i16,
    steps: u8,
    /// Where its pattern of steps starts in the file.
    pattern: usize,
}

/// A wave's description: 42 bytes.
#[derive(Clone, Copy, Debug)]
struct Wave([u8; WAVE_SIZE]);

impl Default for Wave {
    fn default() -> Wave {
        Wave([0; WAVE_SIZE])
    }
}

impl Wave {
    fn w(&self, i: usize) -> u16 {
        u16::from_be_bytes([self.0[i], self.0[i + 1]])
    }
    fn loop_start(&self) -> u16 {
        self.w(0)
    }
    fn loop_end(&self) -> u16 {
        self.w(2)
    }
    fn subloop_len(&self) -> u16 {
        self.w(4)
    }
    fn allow_9xx(&self) -> u8 {
        self.0[6]
    }
    fn subloop_wait(&self) -> u8 {
        self.0[7]
    }
    fn subloop_step(&self) -> u16 {
        self.w(8)
    }
    fn chipram(&self) -> u16 {
        self.w(0xa)
    }
    fn loop_offset(&self) -> u16 {
        self.w(0xc)
    }
    fn flags(&self) -> u8 {
        self.0[0x14]
    }
    /// Length of its first octave: 128 to 32,768 points.
    fn len(&self) -> u32 {
        (u32::from(self.0[0x19]) + 1) << 7
    }
    /// Length with its higher octaves, when it has them.
    fn total(&self) -> u32 {
        if self.flags() & 4 != 0 {
            self.len() * 15 / 8
        } else {
            self.len()
        }
    }
}

/// A Pretracker song, its waves drawn.
#[derive(Clone, Debug)]
pub struct Song {
    pub title: String,
    pub author: String,
    pub version: u8,
    /// Positions in the song, and the one it goes back to at its end.
    pub length: u8,
    pub restart: u8,
    /// Steps (rows) in every pattern.
    pub steps: u8,
    pub instrument_names: Vec<String>,
    pub wave_names: Vec<String>,
    posd: usize,
    patt: usize,
    insts: [Inst; MAX_INSTRUMENTS],
    waves: [Wave; MAX_WAVES],
    /// Each wave's place in the sample memory.
    starts: [u32; MAX_WAVES],
    /// The sample memory, as the replayer makes it: two points of
    /// silence, then each wave (with its octaves).
    pub mem: Arc<Vec<i8>>,
    file: Arc<Vec<u8>>,
}

pub fn probe(d: &[u8]) -> bool {
    d.len() >= 0x5c && &d[..3] == b"PRT" && (0x18..=0x1e).contains(&d[3])
}

fn be32(d: &[u8], i: usize) -> Option<usize> {
    Some(u32::from_be_bytes(d.get(i..i + 4)?.try_into().ok()?) as usize)
}

/// A string of at most 23 bytes ended by a zero, and where the next starts.
fn name(d: &[u8], at: usize) -> (String, usize) {
    let mut i = at;
    for _ in 0..23 {
        let b = d.get(i).copied().unwrap_or(0);
        i += 1;
        if b == 0 {
            break;
        }
    }
    let s = d.get(at..i.min(d.len())).unwrap_or_default();
    let end = s.iter().position(|&b| b == 0).unwrap_or(s.len());
    (s[..end].iter().map(|&b| b as char).collect::<String>().trim_end().to_string(), i)
}

fn text(d: &[u8], at: usize, n: usize) -> String {
    let s = d.get(at..at + n).unwrap_or_default();
    let end = s.iter().position(|&b| b == 0).unwrap_or(s.len());
    s[..end].iter().map(|&b| b as char).collect::<String>().trim_end().to_string()
}

impl Song {
    pub fn parse(d: &[u8]) -> Option<Song> {
        if !probe(d) {
            return None;
        }
        let version = d[3];
        let v15 = version == 0x1e;
        let (mut posd, patt, inst, wave) = (be32(d, 4)?, be32(d, 8)?, be32(d, 0xc)?, be32(d, 0x10)?);
        // Pretracker 1.5 keeps the first sub-song's header at $5C (its
        // restart, patterns, steps and length) and before its positions.
        let (restart, length, steps) = if v15 {
            posd += 8;
            (d[0x5c], d[0x5f], d[0x5e])
        } else {
            (d[0x3c], d[0x3e], d[0x3f])
        };
        let nwaves = usize::from(d[0x41]);
        if nwaves == 0 || nwaves > MAX_WAVES || steps == 0 || length == 0 {
            return None;
        }
        let mut at = inst;
        let mut instrument_names = Vec::new();
        for _ in 0..if v15 { 2 * MAX_INSTRUMENTS } else { MAX_INSTRUMENTS } {
            let (s, next) = name(d, at);
            instrument_names.push(s);
            at = next;
        }
        instrument_names.truncate(MAX_INSTRUMENTS);
        let ninst = usize::from(d[0x40]);
        let mut insts = [Inst::default(); MAX_INSTRUMENTS];
        let mut pattern = at + ninst * 8;
        for (k, i) in insts.iter_mut().enumerate().take(ninst.min(MAX_INSTRUMENTS)) {
            let b = d.get(at + k * 8..at + k * 8 + 8)?;
            let depth = VIB_DEPTH.get(usize::from(b[1])).copied().unwrap_or(0);
            let speed = VIB_SPEED.get(usize::from(b[2])).copied().unwrap_or(0);
            *i = Inst {
                vib_delay: u16::from(VIB_DELAY.get(usize::from(b[0])).copied().unwrap_or(0)) + 1,
                vib_depth: i16::from(depth),
                // The speed is kept times the depth, a sixteenth of it.
                vib_speed: (i16::from(speed) * i16::from(depth)) >> 4,
                attack: FAST_ROLL_OFF_16.get(usize::from(b[3])).copied().unwrap_or(0),
                decay: RAMP_UP_16.get(usize::from(b[4])).copied().unwrap_or(0),
                sustain: (if b[5] == 15 { 16 } else { i16::from(b[5]) }) << 6,
                release: RAMP_UP_16.get(usize::from(b[6])).copied().unwrap_or(0),
                steps: b[7],
                pattern,
            };
            pattern += usize::from(b[7]) * 3;
        }
        let mut at = wave;
        let mut wave_names = Vec::new();
        for _ in 0..MAX_WAVES {
            let (s, next) = name(d, at);
            wave_names.push(s);
            at = next;
        }
        at += at & 1;
        let mut waves = [Wave::default(); MAX_WAVES];
        for (k, w) in waves.iter_mut().enumerate().take(nwaves) {
            w.0.copy_from_slice(d.get(at + k * WAVE_SIZE..at + (k + 1) * WAVE_SIZE)?);
        }
        let mut starts = [0u32; MAX_WAVES];
        let mut size = 2u32;
        for k in 0..nwaves {
            starts[k] = size;
            size += waves[k].total();
        }
        let order: Vec<usize> = if version > 0x19 {
            d[0x42..0x42 + MAX_WAVES].iter().map(|&b| usize::from(b)).collect()
        } else {
            (0..MAX_WAVES).collect()
        };
        let mut mem = vec![0i8; size as usize];
        let osc = osc_buffers();
        for &w in order.iter().take(nwaves) {
            if w < nwaves {
                draw(&mut mem, &waves, &starts, w, &osc);
            }
        }
        Some(Song {
            title: text(d, 0x14, 20),
            author: text(d, 0x28, 20),
            version,
            length,
            restart,
            steps,
            instrument_names,
            wave_names,
            posd,
            patt,
            insts,
            waves,
            starts,
            mem: Arc::new(mem),
            file: Arc::new(d.to_vec()),
        })
    }

    fn b(&self, i: usize) -> u8 {
        self.file.get(i).copied().unwrap_or(0)
    }

    /// The pattern and transpose of channel `c` at position `p`.
    pub fn position(&self, p: usize, c: usize) -> (u8, i8) {
        let at = self.posd + p * 8 + c * 2;
        (self.b(at), self.b(at + 1) as i8)
    }

    /// Pattern `n`'s (1 up) step `row`: its three bytes.
    pub fn step(&self, n: u8, row: usize) -> [u8; 3] {
        let at = self.patt + usize::from(n.max(1) - 1) * usize::from(self.steps) * 3 + row * 3;
        [self.b(at), self.b(at + 1), self.b(at + 2)]
    }
}

impl Tune for Song {
    fn format(&self) -> &'static str {
        "prt"
    }
    fn file(&self) -> &[u8] {
        &self.file
    }
    fn driver(&self, _song: usize) -> Box<dyn Driver> {
        Box::new(Player::new(Arc::new(self.clone())))
    }

    /// Each named instrument: its settings, its steps (3 bytes each; the
    /// waves they pick numbered by their place in the patch), then each
    /// wave's 42-byte description and its points as drawn.
    fn patches(&self) -> Vec<Patch> {
        let mut out = Vec::new();
        for (k, i) in self.insts.iter().enumerate().take(self.instrument_names.len()) {
            if i.steps == 0 {
                continue;
            }
            let mut waves: Vec<usize> = Vec::new();
            let mut steps = Vec::with_capacity(usize::from(i.steps) * 3);
            for s in 0..usize::from(i.steps) {
                let at = i.pattern + s * 3;
                let (b0, b1, mut b2) = (self.b(at), self.b(at + 1), self.b(at + 2));
                if matches!(b1 & 15, 0 | 4) && b2 != 0 && usize::from(b2) <= MAX_WAVES {
                    let w = usize::from(b2 - 1);
                    b2 = waves.iter().position(|&x| x == w).unwrap_or_else(|| {
                        waves.push(w);
                        waves.len() - 1
                    }) as u8
                        + 1;
                }
                steps.extend([b0, b1, b2]);
            }
            let mut b = PatchBytes::default();
            b.u16(i.vib_delay).u16(i.vib_depth as u16).u16(i.vib_speed as u16).u8(i.release).u16(i.attack).u8(i.decay).u16(i.sustain as u16).bytes(&steps);
            for w in waves {
                let wave = &self.waves[w];
                b.bytes(&wave.0).mem(&self.mem, self.starts[w] as usize, wave.total() as usize);
            }
            out.push(b.patch(k + 1, &self.instrument_names[k]));
        }
        out
    }
}

/// The oscillators' buffers for each note of the octave: a saw, square
/// and triangle a period long, and the period.
fn osc_buffers() -> Vec<i8> {
    let mut t = vec![0i8; 12 * OWB];
    for (n, &f) in LOG12.iter().enumerate() {
        let b = n * OWB;
        let p = (0x40_0000 / f) as usize;
        t[b + 384] = p as i8;
        let frac = (0xff00 / p as u32) as u16;
        let (half, quarter) = (p >> 1, p >> 2);
        let mut tri = p - quarter;
        let mut acc = 0u16;
        for pos in 0..p {
            let d2 = (acc >> 8) as u8;
            t[b + pos] = 0x7fu8.wrapping_sub(d2) as i8;
            let d2 = d2.wrapping_add(d2);
            if pos == quarter {
                tri -= p;
            }
            let sq = if pos as i32 > i32::from(acc as i16) {
                t[b + 256 + tri] = 0x7fu8.wrapping_sub(d2) as i8;
                -0x80
            } else {
                t[b + 256 + tri] = d2.wrapping_add(0x80) as i8;
                if pos == half {
                    -0x80
                } else {
                    0x7f
                }
            };
            tri += 1;
            t[b + 128 + pos] = sq;
            acc = acc.wrapping_add(frac);
        }
    }
    t
}

fn clip8(x: i32) -> i8 {
    x.clamp(-0x80, 0x7f) as i8
}

/// A byte added to another, held at the ends as the replayer holds it.
fn add_sat(a: i8, b: i8) -> i8 {
    a.saturating_add(b)
}

fn get(mem: &[i8], i: i64) -> i8 {
    if i < 0 {
        return 0;
    }
    mem.get(i as usize).copied().unwrap_or(0)
}

fn put(mem: &mut [i8], i: i64, v: i8) {
    if i >= 0 {
        if let Some(x) = mem.get_mut(i as usize) {
            *x = v;
        }
    }
}

/// Wave `w` drawn into its place in `mem`, as the replayer draws it at
/// load: its oscillator (or noise) a chord at a time, then its filter,
/// modulator, envelope, mix and octaves.
fn draw(mem: &mut [i8], waves: &[Wave; MAX_WAVES], starts: &[u32; MAX_WAVES], w: usize, osc: &[i8]) {
    let wi = &waves[w];
    let b = |i: usize| wi.0[i];
    let start = i64::from(starts[w]);
    let len = wi.len() as u16;
    let end = start + i64::from(len);
    for i in start..end {
        put(mem, i, 0);
    }
    let flags = b(0x14);
    let linear = flags & 0x10 != 0;
    let base = b(0x17);
    let chord = [b(0xe), b(0xf), b(0x10)];
    let chord_flag = u32::from(chord.iter().all(|&n| n == 0));
    let pitches = [base, chord[0].wrapping_add(base), chord[1].wrapping_add(base), chord[2].wrapping_add(base)];
    let (phase_min, phase_max, phase_spd) = (u32::from(b(0x15)), u32::from(b(0x16)), u32::from(b(0x13)));
    let gain = i32::from(b(0x18));
    let density = b(0x29);
    let mut unisono = false;
    let mut kind_noise = flags & 3 == 3;
    loop {
        for note_num in 0..4u32 {
            let pitch = pitches[note_num as usize];
            if note_num != 0 && pitch == base {
                continue;
            }
            let d0 = i32::from(pitch as i8);
            let v = (d0 + 144) as u32;
            let note = (v % 12) as usize;
            let oct = (v / 12) as i32 - 12;
            let ob = note * OWB;
            let kind = flags & 3;
            kind_noise = kind == 3;
            let table = ob + [0usize, 256, 128, 0][usize::from(kind)];
            let d6: i32 = if oct > 0 { 0x8000 << oct } else { 0x8000 >> (-oct) };
            // The pitch ramp.
            let mut d3: i32 = i32::from(b(0x21) as i8);
            if linear {
                if d3 > 0 {
                    d3 *= d3;
                } else {
                    let sh = (oct as u16 as u32) & 63;
                    d3 = if sh >= 32 { 0 } else { ((d3 as u32) << sh) as i32 };
                    d3 = d3.wrapping_add(d3);
                }
            } else if d3 > 0 {
                d3 *= d3;
            }
            let mut d2: i32 = d3.wrapping_shl(10);
            let p = osc[ob + 384] as u8 as i32;
            let d7: i32 = p << 15;
            let d5f = ((15 - oct) as u32) << 3;
            let min_v = ((phase_min * d5f) << 6) as i32;
            let max_v = ((phase_max * d5f) << 6) as i32;
            let mut d5: i32 = (phase_spd << 11) as i32;
            let (lo, hi) = if max_v >= min_v {
                (min_v, max_v)
            } else {
                d5 = -d5;
                (max_v, min_v)
            };
            let shift = u32::from(b(0x11)) * ((chord_flag + note_num) & 0xff);
            let d4 = ((shift + phase_min) << 4) & 0xffff;
            let mut d0: i32 = ((((d6 as u32) >> 4) & 0xffff) * d4) as i32;
            while d0 > d7 {
                d0 -= d7;
            }
            if kind_noise {
                noise(mem, wi, start, end, oct, i32::from(pitch as i8), d2, linear);
                continue;
            }
            let mut d1: i32 = d6;
            if unisono {
                let k = 9 - ((density >> 3) & 3);
                d1 = (d6 >> k) + d6;
            }
            let mut a1: i32 = min_v;
            let mut a2: i32 = 0;
            let mut at = start;
            while at != end {
                let mut d4 = d0.wrapping_sub(a1);
                if d4 < 0 {
                    d4 = 0;
                }
                let idx = (d4 >> 15) as i16 as i64;
                let s = i32::from(get(osc, table as i64 + idx));
                let d3 = ((gain * s) as i16 >> 7) as i32;
                put(mem, at, clip8(i32::from(get(mem, at)) + d3));
                at += 1;
                d0 = d0.wrapping_add(d1);
                if d0 >= d7 {
                    d0 -= d7;
                    a1 = a1.wrapping_add(d5);
                    if a1 >= hi {
                        d5 = -d5;
                        a1 = hi;
                    }
                    if a1 <= lo {
                        d5 = -d5;
                        a1 = lo;
                    }
                }
                if d2 != 0 {
                    a2 = a2.wrapping_add(d2);
                    d1 = (a2 >> 10).wrapping_add(d6);
                    if linear {
                        d2 -= d2 >> 7;
                    }
                    if (d1 as u32) >= (d7 as u32) {
                        d2 = 0;
                        d1 = 0;
                    }
                }
            }
        }
        if density & 0x18 != 0 && !kind_noise && !unisono {
            unisono = true;
            continue;
        }
        break;
    }
    filter(mem, wi, start, end);
    if density & 0x20 == 0 {
        modulator(mem, wi, start, len);
    }
    envelope(mem, wi, start, len);
    if density & 0x20 != 0 {
        modulator(mem, wi, start, len);
    }
    let mix = b(0x1a);
    if mix != 0 && usize::from(mix - 1) != w && usize::from(mix - 1) < MAX_WAVES {
        let m = usize::from(mix - 1);
        let src = i64::from(starts[m]);
        let n = (len as i16).min(waves[m].len() as u16 as i16);
        let mut k = 0i64;
        let mut c = n.wrapping_sub(1) as u16;
        loop {
            put(mem, start + k, add_sat(get(mem, start + k), get(mem, src + k)));
            k += 1;
            if c == 0 {
                break;
            }
            c -= 1;
        }
    }
    if flags & 4 != 0 {
        let n = wi.len() as i64;
        let count = n * 7 / 8;
        for k in 0..count {
            put(mem, start + n + k, get(mem, start + 2 * k));
        }
    }
}

/// The noise oscillator: a 16-bit xorshift, each value held as long as
/// its speed says, added onto the wave.
#[allow(clippy::too_many_arguments)]
fn noise(mem: &mut [i8], wi: &Wave, start: i64, end: i64, oct: i32, note: i32, mut d2: i32, linear: bool) {
    let mut d5: i32 = 0x8000;
    let mut a5: i32 = 0x8000;
    if oct < 0 {
        let d3 = (-note) as u32;
        let q = d3 / 12;
        let sh = (q + 1) & 63;
        d5 = if sh >= 16 { 0 } else { (0x8000u32 >> sh) as i32 };
        let d4 = (d5 as u32 & 0xffff) / 12;
        let mut d0 = (d3 & 0xffff) as i16;
        loop {
            d0 = d0.wrapping_sub(12);
            if d0 < 0 {
                break;
            }
        }
        let d0 = (d0.wrapping_neg() as u16) as u32;
        d5 += (d0 * d4) as i32;
    }
    let mut d0 = u16::from(wi.0[0x15]) + u16::from(wi.0[0x11]) + 1;
    let mut a1: i32 = d5;
    let gain = i32::from(wi.0[0x18]);
    let mut d6: i32 = 0;
    let mut at = start;
    loop {
        d0 ^= d0 << 13;
        d0 ^= d0 >> 9;
        d0 ^= d0 << 7;
        if a5 != 0x8000 {
            let d4 = a5.wrapping_sub(0x8000);
            let d1 = (a5.wrapping_sub(0x8001) as u32 & 0xffff_8000) as i32;
            a5 = d4.wrapping_sub(d1);
        }
        let s = i32::from(d0 as u8 as i8);
        let d1 = clip8((gain * s) >> 7);
        let v = d1.wrapping_add(get(mem, at));
        loop {
            put(mem, at, v);
            at += 1;
            a5 = a5.wrapping_add(a1);
            if d2 != 0 {
                d6 = d6.wrapping_add(d2);
                a1 = (d6 >> 10).wrapping_add(d5);
                if linear {
                    d2 -= d2 >> 7;
                }
                if a1 <= 0x1ff {
                    d2 = 0;
                    a5 = 0;
                    a1 = 0x200;
                }
            }
            if at == end {
                return;
            }
            if a5 > 0x7fff {
                break;
            }
        }
    }
}

/// The filter: four one-pole stages (with resonance), the cutoff swept
/// between its bounds a 64-point block at a time, taken as a low-, high-,
/// band-pass or notch.
fn filter(mem: &mut [i8], wi: &Wave, start: i64, end: i64) {
    let kind = wi.0[0x1f];
    if !(1..=4).contains(&kind) || start == end {
        return;
    }
    let mut taps = [0i16; 4];
    let mut d0: i32 = i32::from(wi.0[0x22]) << 8;
    let d4: i32 = i32::from(wi.0[0x23]) << 8;
    let d5: i32 = i32::from(wi.0[0x24]) << 8;
    let mut d3: i32 = i32::from(wi.0[0x25] as i8) << 7;
    let res = i32::from(wi.0[0x20]);
    let mut at = start;
    loop {
        let mut a6 = d0.wrapping_add(d3);
        // Where the cutoff goes for this block, turning at its bounds.
        enum At {
            Pos,
            Load(u8),
            Fixed(u8),
        }
        let step = if d3 > 0 {
            if d0 > d5 {
                if d0 > 0xff00 || a6 <= 0xfeff {
                    At::Pos
                } else {
                    d3 = -d3;
                    a6 = 0xff00;
                    At::Fixed(0)
                }
            } else if a6 < d5 {
                At::Pos
            } else {
                if d4 == d5 {
                    a6 = d4;
                } else {
                    d3 = -d3;
                    a6 = d5;
                }
                At::Load(!wi.0[0x24])
            }
        } else if d0 < d4 {
            if d0 < 0 || a6 > 0 {
                At::Pos
            } else {
                d3 = -d3;
                a6 = 0;
                At::Fixed(0xff)
            }
        } else if a6 > d4 {
            At::Pos
        } else {
            a6 = d4;
            if d4 != d5 {
                d3 = -d3;
            }
            At::Load(!wi.0[0x23])
        };
        let d2 = match step {
            At::Pos => !(((a6 as u32 & 0xffff) >> 8) as u8),
            At::Load(x) | At::Fixed(x) => x,
        };
        let d2 = if kind & 1 == 0 { !d2 } else { d2 };
        let d2w = i32::from(d2);
        let mut f: i16 = (d2w * 2) as i16;
        if res != 0 {
            let d1 = (d2w << 8) as u32;
            let mut div = ((0x5b - res) * 2) as i16;
            if div < 0x36 {
                div = 0x36;
            }
            let q = (d1 / div as u16 as u32) as u16;
            f = (d2 as u16).wrapping_add(q) as i16;
        }
        let [mut t3, mut t4, mut t5, mut t6] = taps;
        let mul = |a: i16, b: i16| -> i16 { ((i32::from(a) * i32::from(b)) >> 8) as i16 };
        let g = d2 as i16;
        for _ in 0..64 {
            let x = i16::from(get(mem, at));
            let mut d7 = mul(t4.wrapping_sub(t5), f).wrapping_sub(t4);
            d7 = mul(d7.wrapping_add(x), g);
            t4 = t4.wrapping_add(d7);
            t5 = t5.wrapping_add(mul(t4.wrapping_sub(t5), g));
            t6 = t6.wrapping_add(mul(t5.wrapping_sub(t6), g));
            t3 = t3.wrapping_add(mul(t6.wrapping_sub(t3), g));
            let y = match kind {
                2 => t3.wrapping_sub(x),
                3 => t3.wrapping_sub(t4).wrapping_sub(t5).wrapping_sub(t6) >> 1,
                4 => t3.wrapping_sub(t4).wrapping_neg(),
                _ => t3,
            };
            put(mem, at, clip8(i32::from(y)));
            at += 1;
        }
        taps = [t3, t4, t5, t6];
        if at >= end {
            return;
        }
        d0 = a6;
    }
}

/// The modulator: up to seven delayed copies of the wave (each a little
/// later, sweeping, every other one turned over) added onto it.
fn modulator(mem: &mut [i8], wi: &Wave, start: i64, len: u16) {
    let wet = i32::from(wi.0[0x26]);
    let density = wi.0[0x29];
    let n = u16::from(density & 7);
    if wet == 0 || n == 0 {
        return;
    }
    let post = density & 0x20 != 0;
    for d7 in 0..n {
        let mut d5: u32 = u32::from(wi.0[0x27]) * u32::from(MODULATOR_RAMP_8[usize::from(d7)]);
        let mut d1: u32 = u32::from(wi.0[0x28]);
        if post {
            d1 <<= 8;
        } else {
            d5 >>= 2;
            d1 <<= 6;
        }
        d5 += d1;
        let mut d3: u16 = 0;
        let mut d2: u16 = 0;
        while d2 < len {
            d3 = d3.wrapping_add(d7).wrapping_add(8);
            let r = u32::from(RAMP_UP_DOWN_32[usize::from(d3 >> 11)]);
            let back = ((r + d5) >> 6) as u16;
            let d0 = d2.wrapping_sub(back) as i16;
            if d0 >= 0 {
                let mut x = i32::from(get(mem, start + i64::from(d0)));
                if d7 & 1 != 0 {
                    x = -x;
                }
                let x = ((x * wet) as i16 >> 8) as i8;
                let at = start + i64::from(d2);
                put(mem, at, add_sat(x, get(mem, at)));
            }
            d2 += 1;
        }
    }
}

/// The volume envelope: an attack, a hold at the top, a decay down a
/// roll-off curve to the sustain level, which the rest of the wave keeps.
fn envelope(mem: &mut [i8], wi: &Wave, start: i64, len: u16) {
    let flags = wi.0[0x14];
    let boost = flags & 8 != 0;
    let fast = flags & 0x20 != 0;
    let sustain = wi.0[0x1e];
    let mut a0 = start;
    // Points left, less one: below zero is the end.
    let mut d4: i16 = (len as i16).wrapping_sub(1);
    let scale = |x: i8, v: i32, sh: u32, clip: bool| -> i8 {
        let p = ((i32::from(x) * v) as i16 >> sh) as i32;
        if clip {
            clip8(p)
        } else {
            p as i8
        }
    };
    let attack = wi.0[0x1b];
    let mut d3: i32 = 0;
    let mut d1: i32 = 0;
    let do_attack = if attack != 0 {
        d1 = match attack {
            1 => 0x20000,
            2 => 0x10000,
            a => (0x20000 / u32::from(a)) as i32,
        };
        if fast {
            d1 <<= 4;
        }
        d3 += d1;
        d3 <= 0xff_ffff
    } else {
        if sustain == 0xff {
            return;
        }
        false
    };
    if do_attack {
        loop {
            let v = d3 >> 16;
            let x = get(mem, a0);
            put(mem, a0, if boost { scale(x, v, 6, true) } else { scale(x, v, 8, false) });
            a0 += 1;
            d4 = d4.wrapping_sub(1);
            if d4 < 0 {
                return;
            }
            d3 += d1;
            if d3 > 0xff_ffff {
                break;
            }
        }
    }
    // The hold: the top level (255) for a while.
    let hold = i64::from(u16::from(wi.0[0x1c]) << 4);
    let a1 = a0 + hold + 2;
    loop {
        let x = get(mem, a0);
        put(mem, a0, if boost { scale(x, 0xff, 6, true) } else { scale(x, 0xff, 8, false) });
        a0 += 1;
        if a0 == a1 {
            break;
        }
        d4 = d4.wrapping_sub(1);
        if d4 == -1 {
            return;
        }
    }
    d4 = d4.wrapping_sub(1);
    // The decay.
    let decay = u32::from(wi.0[0x1d]);
    if decay != 0 {
        let d7: u32 = ((decay * decay) as u16 >> 2) as u32 + decay;
        let mut d3: u32 = if fast { 0 } else { decay << 12 };
        let idx = (d3 >> 16) as usize;
        let mut a1 = idx + 1;
        let mut a2: i16 = ROLL_OFF[idx] as i16;
        let mut d2: u16 = idx as u16;
        let mut d1: u16 = 0;
        let mut d5: u16 = 0xffff;
        loop {
            d3 = d3.wrapping_add(d7);
            let pos = (d3 >> 16) as u16;
            if pos <= 0x8e {
                if pos > d2 {
                    a2 = ROLL_OFF.get(a1).copied().unwrap_or(0) as i16;
                    a1 = usize::from(pos);
                    d2 = pos;
                }
                d1 = a2 as u16;
                let d0 = (ROLL_OFF.get(a1).copied().unwrap_or(0) as i16).wrapping_sub(a2);
                if d0 != 0 {
                    let fr = ((d3 & 0xffff) >> 8) as i32;
                    d1 = ((((i32::from(d0) * fr) >> 8) as i16).wrapping_add(a2)) as u16;
                }
            }
            let (r, borrow) = d5.overflowing_sub(d1);
            d5 = r;
            if borrow || r == 0 {
                break;
            }
            let d6 = d5 >> 8;
            if d6 as u8 <= sustain {
                break;
            }
            let x = i32::from(get(mem, a0)) * i32::from(d6);
            put(mem, a0, if boost { clip8(x >> 6) } else { (x >> 8) as i8 });
            a0 += 1;
            d4 = d4.wrapping_sub(1);
            if d4 == -1 {
                return;
            }
        }
    }
    // The sustain, to the end.
    let s = i32::from(sustain);
    loop {
        let x = get(mem, a0);
        let v = if s == 0 {
            0
        } else if boost {
            scale(x, s, 6, true)
        } else {
            scale(x, s, 8, false)
        };
        put(mem, a0, v);
        a0 += 1;
        d4 = d4.wrapping_sub(1);
        if d4 == -1 {
            return;
        }
    }
}

/// What a channel sends Paula: a part of the sample memory, its loop
/// (an offset into it, none at $FFFF), period and volume, and whether it
/// starts afresh.
/// Chip address 0 as a place in the sample memory (which the replayer
/// puts at $300000 in the reference's memory): an echo slot never filled
/// points there, outside the samples, and plays silence.
const NULL_PTR: u32 = 0u32.wrapping_sub(0x30_0000);

#[derive(Clone, Copy, Debug, Default)]
struct Out {
    ptr: u32,
    len: u16,
    lof: u16,
    per: u16,
    vol: u8,
    trg: bool,
}

#[derive(Clone, Copy, Debug, Default)]
struct Chan {
    port_dest: i16,
    pat_pitch_slide: i16,
    pat_vol_ramp: u8,
    second4: u8,
    second_delay: u8,
    wave_offset: u8,
    inst_pitch_slide: i16,
    sel_arp: i16,
    note_pitch: i16,
    port_pitch: i16,
    line_ticks: u8,
    pinned: bool,
    inst_vol_slide: u8,
    step_pos: u8,
    /// The wave playing times four; $FF none yet.
    wave_num: u16,
    td_offset: u8,
    speed_stop: u8,
    inst_pitch: i16,
    inst_vol: u16,
    loaded_vol: u8,
    pat_vol: u8,
    arp: [u8; 3],
    last_len: u16,
    port_speed: u8,
    note_off_delay: u8,
    steps: u8,
    note_delay: u8,
    td_steps: u8,
    td_vol16: u8,
    inst_num4: u16,
    subloop_wait: i16,
    loop_offset: u16,
    inst: Option<usize>,
    /// The wave whose description is in use.
    wave: usize,
    adsr_phase: u16,
    adsr_vol: i16,
    adsr_speed: u8,
    /// $FF forward, 0 backward.
    ping_pong: u8,
    adsr_pos: i16,
    vol64: i16,
    new_inst: u8,
    vib_pos: i16,
    vib_delay: u16,
    vib_depth: i16,
    vib_speed: i16,
    release: u8,
    out: Out,
    delayed: [Out; 32],
}

/// Paula's registers for a channel, as the replayer last set them.
#[derive(Clone, Copy, Debug, Default)]
struct Regs {
    ptr: u32,
    /// In words.
    len: u16,
    per: u16,
    vol: u8,
    /// Started afresh this tick, from here, this many words.
    trig: Option<(u32, u16)>,
}

pub struct Player {
    song: Arc<Song>,
    paula: Paula,
    periods: Vec<u16>,
    ch: [Chan; 4],
    regs: [Regs; 4],
    row: u8,
    next_row: u8,
    next_pos: u8,
    /// The speed as set: above $2F, a shuffle of two (even rows' high
    /// nibble, odd rows' low).
    speed: u8,
    line: u8,
    running: bool,
    pos: u32,
    shown: (usize, usize),
    done: bool,
}

impl Player {
    pub fn new(song: Arc<Song>) -> Player {
        let mut periods = Vec::with_capacity(36 * 16);
        for i in 0..36 {
            let mut d0 = i32::from(PERIODS[i]) << 16;
            let d1 = (i32::from((PERIODS[i + 1] as i16).wrapping_sub(PERIODS[i] as i16)) << 16) >> 4;
            for _ in 0..16 {
                periods.push((d0 >> 16) as u16);
                d0 = d0.wrapping_add(d1);
            }
        }
        let mut ch = [Chan::default(); 4];
        for c in ch.iter_mut() {
            c.delayed = [Out { ptr: NULL_PTR, ..Out::default() }; 32];
            c.pat_vol = 64;
            c.td_offset = 0xff;
            c.adsr_phase = 3;
            c.out = Out {
                ptr: 0,
                len: 2,
                lof: 0,
                per: 0x7b,
                vol: 0,
                trg: false,
            };
        }
        let paula = Paula::new(4, song.mem.as_ref().clone());
        // The replayer's set-up leaves each channel playing silence: its
        // registers on chip address 0, a length of 2.
        let regs = [Regs {
            ptr: NULL_PTR,
            len: 2,
            ..Regs::default()
        }; 4];
        Player {
            song,
            paula,
            periods,
            ch,
            regs,
            row: 0,
            next_row: 0xff,
            next_pos: 0xff,
            speed: 6,
            line: 6,
            running: true,
            pos: 0,
            shown: (0, 0),
            done: false,
        }
    }

    /// A new instrument on the channel, its state cleared (the second
    /// instrument's keeps the portamento's pitch).
    fn trigger(&mut self, c: usize, num4: u16, keep_port: bool) {
        let idx = usize::from(num4 / 4).saturating_sub(1).min(MAX_INSTRUMENTS - 1);
        let steps = self.song.insts[idx].steps;
        let ch = &mut self.ch[c];
        ch.new_inst = num4 as u8;
        ch.inst_num4 = num4;
        ch.inst = Some(idx);
        ch.steps = steps;
        ch.port_dest = 0;
        ch.pat_pitch_slide = 0;
        ch.pat_vol_ramp = 0;
        ch.second4 = 0;
        ch.second_delay = 0;
        ch.wave_offset = 0;
        ch.inst_pitch_slide = 0;
        ch.sel_arp = 0;
        ch.note_pitch = 0;
        if !keep_port {
            ch.port_pitch = 0;
        }
        ch.line_ticks = 0;
        ch.pinned = false;
        ch.inst_vol_slide = 0;
        ch.step_pos = 0;
        ch.wave_num = 0xff;
        ch.td_offset = 0xff;
        ch.speed_stop = 1;
        ch.inst_pitch = 0x10;
        ch.inst_vol = 64;
        ch.loaded_vol = 64;
        ch.pat_vol = 64;
    }

    fn release(ch: &mut Chan) {
        let v = ch.adsr_vol >> 6;
        ch.vol64 = v;
        ch.adsr_pos = 16;
        ch.adsr_speed = (((16 - v) as u16 >> 1) as u8).wrapping_add(ch.release);
        ch.adsr_phase = 3;
    }

    /// A channel's part of the pattern this tick; true when it goes on
    /// to the track delay's skip.
    fn pattern_channel(&mut self, c: usize) -> bool {
        let ch = &mut self.ch[c];
        let mut fresh = true;
        if ch.second4 != 0 {
            if ch.second_delay != 0 {
                ch.second_delay -= 1;
            } else {
                let n = u16::from(ch.second4);
                self.trigger(c, n, true);
                fresh = false;
            }
        }
        let ch = &mut self.ch[c];
        if fresh {
            if ch.port_dest != 0 {
                let d3 = ch.port_dest;
                let mut d2 = ch.port_pitch;
                let d1 = i16::from(ch.port_speed);
                // The target is taken unsigned, the pitch signed: a
                // target below 0 is above every pitch.
                let dest = i32::from(d3 as u16);
                let reached = if dest <= i32::from(d2) {
                    d2 = d2.wrapping_sub(d1);
                    i32::from(d2) < dest
                } else {
                    d2 = d2.wrapping_add(d1);
                    i32::from(d2) > dest
                };
                if reached {
                    ch.port_dest = 0;
                    d2 = d3;
                }
                ch.port_pitch = d2;
            }
            if ch.pat_vol_ramp != 0 {
                let mut v = ch.pat_vol_ramp.wrapping_add(ch.pat_vol) as i8;
                if v < 0 {
                    v = 0;
                }
                ch.pat_vol = v.min(64) as u8;
            }
        }
        if ch.note_delay >= 0x80 {
            return true;
        }
        if ch.note_delay != 0 {
            ch.note_delay -= 1;
            if ch.note_delay != 0 {
                return false;
            }
            ch.note_delay = 0xff;
        }
        let song = self.song.clone();
        if self.row >= song.steps {
            return true;
        }
        let (patn, shift) = song.position(self.pos as usize, c);
        if patn == 0 {
            return true;
        }
        let [b0, b1, d5] = song.step(patn, usize::from(self.row));
        let mut d0 = i16::from(shift);
        let mut d2 = b1 & 15;
        let mut d4 = u16::from(b1 >> 4);
        let ch = &mut self.ch[c];
        if d2 == 0xe && ch.note_delay == 0 {
            let (x, y) = (d5 >> 4, d5 & 15);
            if x == 0xd {
                if self.speed != 0 {
                    ch.note_delay = y;
                    return false;
                }
            } else if x == 0xa {
                ch.note_off_delay = y;
            }
        }
        ch.note_delay = 0xff;
        let mut arp = false;
        if b0 & 0x80 != 0 {
            d4 += 16;
        }
        let d7 = b0 & 0x3f;
        d4 *= 4;
        if d4 != 0 && d7 == 0 {
            ch.pat_vol = ch.loaded_vol;
            if d4 == ch.inst_num4 {
                ch.adsr_phase = 0;
                ch.adsr_vol = 0;
            }
        }
        #[derive(PartialEq)]
        enum L {
            ArpDone,
            Trigger,
            ClearPorta,
            Effects,
        }
        let mut d3: u8 = 0;
        let mut l = L::ArpDone;
        if b0 & 0x40 == 0 {
            if d2 == 0 && d5 != 0 {
                let d1 = u16::from(d5 & 15) * 4;
                if d7 != 0 {
                    d3 = d4 as u8;
                    d4 = d1;
                } else {
                    d0 = d0.wrapping_add(1) << 4;
                    d3 = d4 as u8;
                    if d1 == 0 {
                        ch.arp = [0; 3];
                        d2 = d7;
                        l = L::ClearPorta;
                    } else {
                        d2 = 0;
                        d4 = d1;
                        l = L::Trigger;
                    }
                }
            }
        } else {
            if d2 | d5 != 0 {
                ch.arp = [d2, d5 >> 4, d5 & 15];
            }
            arp = true;
            d2 = 0;
        }
        let mut new_porta = false;
        if l == L::ArpDone {
            if d7 == NOTE_OFF {
                Self::release(ch);
                l = L::Effects;
            } else if d7 == 0 {
                l = L::Effects;
            } else {
                d0 = d0.wrapping_add(i16::from(d7)) << 4;
                if d4 as u8 != 0 {
                    if d2 == 3 {
                        // A portamento to the note, the instrument kept.
                        if arp {
                            new_porta = true;
                        } else {
                            ch.arp = [0; 3];
                            arp = false;
                            new_porta = true;
                        }
                        l = L::Effects;
                    } else {
                        l = L::Trigger;
                    }
                } else {
                    if !arp {
                        ch.arp = [0; 3];
                    }
                    new_porta = d2 == 3;
                    l = if new_porta { L::Effects } else { L::ClearPorta };
                }
            }
        }
        if l == L::Trigger {
            self.trigger(c, d4, false);
            let ch = &mut self.ch[c];
            if !arp {
                ch.arp = [0; 3];
            }
            new_porta = d2 == 3;
            l = if new_porta { L::Effects } else { L::ClearPorta };
        }
        let ch = &mut self.ch[c];
        if l == L::ClearPorta {
            ch.inst_pitch = 0x10;
            ch.port_pitch = d0;
            ch.port_dest = 0;
        } else if new_porta {
            d0 = d0.wrapping_add(0x10);
            ch.port_dest = d0;
            ch.port_pitch = ch.port_pitch.wrapping_add(ch.inst_pitch);
            ch.inst_pitch = 0;
            if d5 != 0 {
                ch.port_speed = d5;
            }
        }
        if d3 != 0 {
            ch.second4 = d3;
            ch.second_delay = d5 >> 4;
        }
        ch.pat_vol_ramp = 0;
        ch.pat_pitch_slide = 0;
        if arp {
            return true;
        }
        match d2 {
            0xf => {
                self.speed = d5;
                self.line = if d5 > 0x2f {
                    if self.row & 1 != 0 {
                        d5 & 15
                    } else {
                        d5 >> 4
                    }
                } else {
                    d5
                };
                self.running = d5 != 0;
            }
            0x4 => {
                ch.vib_pos = 0;
                ch.vib_delay = 1;
                let depth = VIB_DEPTH[usize::from(d5 & 15)];
                ch.vib_depth = i16::from(depth);
                ch.vib_speed = (i16::from(VIB_SPEED[usize::from(d5 >> 4)]) * i16::from(depth)) >> 4;
            }
            0x5 => {
                if c == 3 {
                    return true;
                }
                for e in self.ch[c + 1].delayed.iter_mut() {
                    e.vol = 0;
                }
                let ch = &mut self.ch[c];
                if d5 == 0 {
                    ch.td_steps = 0xff;
                } else {
                    ch.td_steps = (d5 & 15) * 2;
                    ch.td_vol16 = d5 >> 4;
                }
            }
            0xa => {
                if d5 != 0 {
                    ch.pat_vol_ramp = if d5 & 15 == 0 { d5 >> 4 } else { d5.wrapping_neg() };
                }
            }
            0x1 => ch.pat_pitch_slide = i16::from(d5),
            0x2 => ch.pat_pitch_slide = -i16::from(d5),
            0x9 => ch.wave_offset = d5,
            0xb => self.next_pos = d5,
            0xd => self.next_row = d5,
            0xc => ch.pat_vol = d5.min(64),
            _ => {}
        }
        true
    }

    /// The row moves on, at the end of a row's ticks: to the next, the
    /// next position's first, a break's or a jump's.
    fn advance(&mut self) {
        self.line = self.line.wrapping_sub(1);
        if self.line != 0 {
            return;
        }
        for ch in self.ch.iter_mut() {
            ch.note_delay = 0;
        }
        let steps = self.song.steps;
        let last = i32::from(steps) - 1;
        let (len, restart) = (u32::from(self.song.length), u32::from(self.song.restart));
        let inc = |p: u32| {
            let p = (p & 0xff) + 1;
            if p >= len {
                restart
            } else {
                p
            }
        };
        let old = self.pos;
        let mut row = self.row;
        let mut pos = self.pos;
        // Whether the new row's speed follows its parity (a shuffle's).
        let parity;
        let mut jump = false;
        let mut end = false;
        if self.next_row < 0x80 {
            let d0 = i32::from(self.next_row);
            row = if last <= d0 { steps.wrapping_sub(1) } else { self.next_row };
            pos = inc(pos);
            self.next_row = 0xff;
            if self.next_pos < 0x80 {
                jump = true;
                parity = false;
            } else if last > i32::from(row) {
                parity = true;
            } else {
                end = true;
                parity = false;
            }
        } else if self.next_pos < 0x80 {
            jump = true;
            parity = false;
        } else if i32::from(row) >= last {
            end = true;
            parity = false;
        } else {
            row += 1;
            parity = true;
        }
        if jump {
            row = 0;
            pos = u32::from(self.next_pos);
            if pos >= len {
                pos = restart;
            }
            self.next_pos = 0xff;
            if steps <= 1 {
                pos = inc(pos);
            }
        }
        if end {
            row = 0;
            pos = inc(pos);
        }
        self.row = row;
        self.pos = pos;
        let s = self.speed;
        self.line = if s <= 0x2f {
            s
        } else if parity && row & 1 != 0 {
            s & 15
        } else {
            s >> 4
        };
        if pos <= old && (jump || end || pos != old) {
            self.done = true;
        }
    }

    /// A channel's instrument this tick: its pattern of steps, ADSR,
    /// volume, the wave's loop, the pitch and the part of the wave for
    /// it; true when the next channel takes this one's track delay.
    fn instrument_channel(&mut self, c: usize) -> bool {
        let song = self.song.clone();
        if let Some(ii) = self.ch[c].inst {
            let info = song.insts[ii];
            let ch = &mut self.ch[c];
            let s = ch.inst_pitch_slide.wrapping_add(ch.pat_pitch_slide);
            if s != 0 {
                ch.inst_pitch = s.wrapping_add(ch.inst_pitch).min(0x240);
            }
            if ch.inst_vol_slide != 0 {
                let v = ch.inst_vol_slide.wrapping_add(ch.inst_vol as u8) as i8;
                ch.inst_vol = (ch.inst_vol & 0xff00) | u16::from(v.clamp(0, 64) as u8);
            }
            let mut d2 = ch.line_ticks;
            if d2 == 0 {
                ch.inst_pitch_slide = 0;
                ch.inst_vol_slide = 0;
                let mut d0 = u16::from(ch.step_pos);
                if d0 as u8 >= ch.steps {
                    d2 = 0xffu8.wrapping_add(ch.speed_stop);
                } else {
                    d2 = self.inst_steps(c, &info, &song, d0, &mut d0);
                }
                let ch = &mut self.ch[c];
                ch.line_ticks = d2;
                ch.step_pos = d0 as u8;
            }
            let ch = &mut self.ch[c];
            if ch.wave_num & 0x80 != 0 {
                // No wave picked yet: the first.
                let wi = song.waves[0];
                ch.wave_num = 0;
                ch.wave = 0;
                ch.out.trg = true;
                let d5 = wi.loop_offset();
                if wi.subloop_len() == 0 {
                    ch.out.ptr = song.starts[0].wrapping_add(d5 as i16 as u32);
                    let d0 = wi.chipram();
                    ch.out.len = if (d0.wrapping_sub(1) as i16) <= d5 as i16 { 2 } else { d0.wrapping_sub(d5) };
                    ch.out.lof = 0xffff;
                } else {
                    ch.out.ptr = song.starts[0];
                    ch.out.lof = 0;
                }
                ch.subloop_wait = i16::from(wi.subloop_wait()) + 1;
                ch.loop_offset = d5;
                ch.ping_pong = 0xff;
            }
            if d2 != 0xff {
                ch.line_ticks = d2.wrapping_sub(1);
            }
        }
        let ch = &mut self.ch[c];
        let info = ch.inst.map(|i| song.insts[i]).unwrap_or_default();
        let d1 = ch.inst_vol;
        let mut d2: i16;
        let mut attack = false;
        if ch.new_inst != 0 {
            ch.loaded_vol = d1 as u8;
            ch.vib_delay = info.vib_delay;
            ch.vib_depth = info.vib_depth;
            ch.vib_speed = info.vib_speed;
            ch.release = info.release;
            ch.adsr_phase = 0;
            ch.adsr_vol = 0;
            ch.new_inst = 0;
            ch.vib_pos = 0;
            d2 = 0;
            attack = true;
        } else {
            d2 = ch.adsr_vol;
            let mut decay_release = None;
            match ch.adsr_phase {
                0 => attack = true,
                1 => decay_release = Some(true),
                2 => {}
                _ => {
                    let p = ch.adsr_pos.wrapping_add(ch.vol64);
                    ch.adsr_pos = p;
                    let d4 = p.wrapping_sub(16);
                    if d4 >= 0 {
                        ch.adsr_pos = d4;
                        decay_release = Some(false);
                    }
                }
            }
            if let Some(decay) = decay_release {
                let s = ch.adsr_speed;
                let (d4, d5) = if s >= 0x8f { (2, 0x8f) } else { (ROLL_OFF[usize::from(s)] as i16, s + 1) };
                ch.adsr_speed = d5;
                d2 = d2.wrapping_sub(d4);
                if decay {
                    if d2 <= info.sustain {
                        ch.adsr_phase = 2;
                        d2 = info.sustain;
                    }
                } else if d2 < 0 {
                    d2 = 0;
                }
            }
        }
        if attack {
            d2 = d2.wrapping_add(info.attack as i16);
            if d2 >= 0x400 {
                d2 = 0x400;
                ch.adsr_phase = 1;
                ch.adsr_speed = info.decay;
            }
        }
        ch.adsr_vol = d2;
        if ch.note_off_delay != 0 {
            ch.note_off_delay -= 1;
            if ch.note_off_delay == 0 {
                ch.adsr_vol = 0;
                ch.adsr_phase = 3;
            }
        }
        let v = (((d2 as u16 >> 4) & 127) as u32 * u32::from(d1 as u8 & 0x7f)) >> 6;
        ch.out.vol = ((v * u32::from(ch.pat_vol)) >> 6) as u8;

        // The wave's loop: a part (sub-loop) stepped through it, back and forth.
        let wi = song.waves[ch.wave];
        let d3 = wi.subloop_len();
        let wave_start = song.starts[usize::from(ch.wave_num / 4).min(MAX_WAVES - 1)];
        if d3 != 0 {
            ch.out.len = d3;
            let d2 = wi.subloop_step();
            let moved: Option<(u16, u8)> = if ch.wave_offset != 0 && wi.allow_9xx() != 0 {
                let x = u16::from(ch.wave_offset) << 7;
                ch.wave_offset = 0;
                let pp = ch.ping_pong;
                Some((if pp & 0x80 == 0 { x.wrapping_add(d2) } else { x.wrapping_sub(d2) }, pp))
            } else {
                // Held while the wait (unsigned: one taken below 0 holds
                // it long) is above 1.
                if ch.subloop_wait as u16 > 1 {
                    ch.subloop_wait -= 1;
                    None
                } else {
                    Some((ch.loop_offset, ch.ping_pong))
                }
            };
            if let Some((mut d1, pp)) = moved {
                ch.subloop_wait = i16::from(wi.subloop_wait());
                let r;
                let mut changed = false;
                if pp & 0x80 != 0 {
                    d1 = d1.wrapping_add(d2);
                    let d4 = d1.wrapping_add(d3);
                    let mut e = wi.loop_end();
                    if e < d1 {
                        e = wi.chipram();
                    }
                    let (x, borrow) = e.overflowing_sub(d4);
                    r = x;
                    if borrow || x == 0 {
                        d1 = d1.wrapping_add(x);
                        ch.ping_pong = 0;
                        changed = true;
                    }
                } else {
                    d1 = d1.wrapping_sub(d2);
                    let d4 = wi.loop_start();
                    r = d4.wrapping_sub(d1);
                    if r as i16 >= 0 {
                        d1 = d4;
                        ch.ping_pong = 0xff;
                        changed = true;
                    }
                }
                if changed && r == 0 {
                    ch.subloop_wait = ch.subloop_wait.wrapping_sub(1);
                }
                ch.loop_offset = d1;
            }
            ch.out.lof = ch.loop_offset;
            ch.out.ptr = wave_start;
        } else if ch.wave_offset != 0 && wi.allow_9xx() != 0 {
            let d1 = u16::from(ch.wave_offset) << 7;
            ch.wave_offset = 0;
            ch.out.trg = true;
            let r = wi.chipram().wrapping_sub(d1);
            ch.out.len = if r as i16 > 0 { r } else { 2 };
            ch.out.ptr = wave_start + u32::from(d1);
        }

        // The pitch.
        let mut d0 = ch.inst_pitch.wrapping_sub(0x10);
        if !ch.pinned {
            d0 = d0.wrapping_add(ch.sel_arp).wrapping_add(ch.port_pitch).wrapping_sub(0x10);
        }
        d0 = d0.wrapping_add(ch.note_pitch);
        let mut active = true;
        let low = ch.vib_delay as u8;
        if low != 0 {
            let l = low - 1;
            ch.vib_delay = (ch.vib_delay & 0xff00) | u16::from(l);
            active = l == 0;
        }
        if active && ch.vib_speed != 0 {
            let d2 = ch.vib_speed;
            let mut d4 = ch.vib_depth;
            let mut d1 = d2.wrapping_add(ch.vib_pos);
            let mut flip = d1 > d4;
            if !flip {
                d4 = d4.wrapping_neg();
                flip = d1 < d4;
            }
            if flip {
                ch.vib_speed = d2.wrapping_neg();
                d1 = d4;
            }
            ch.vib_pos = d1;
            d0 = d0.wrapping_add(d1 >> 3);
        }

        // The part of the wave for the pitch: above $219, one of its higher octaves.
        let mut d3 = ch.out.len;
        let d6: i16;
        if d0 > 0x219 {
            if wi.flags() & 4 == 0 {
                d6 = 0x231;
            } else {
                let d2 = wi.chipram();
                let d5 = (usize::from(d0.wrapping_sub(0x219) as u16 >> 6)).min(OCTAVE_SELECT.len() - 1);
                let d1 = u32::from(OCTAVE_SELECT[d5]);
                let mut offset = true;
                if ch.out.lof != 0xffff {
                    ch.out.lof >>= d1;
                    d3 >>= d1;
                    ch.out.len = d3;
                } else if ch.out.trg {
                    let d4 = ch.out.ptr.wrapping_sub(wave_start);
                    let d6x = d3.wrapping_add(d4 as u16);
                    let d7 = d2.wrapping_sub(d6x);
                    d3 = if d3 >= d7 { d3.wrapping_add(d6x).wrapping_sub(d2) >> d1 } else { 2 };
                    ch.out.len = d3;
                    let d4 = (d4 & 0xffff_0000) | u32::from((d4 as u16) >> d1);
                    ch.out.ptr = d4.wrapping_add(wave_start);
                } else {
                    offset = false;
                }
                if offset {
                    let mut d4 = 0u32;
                    let mut d2 = u32::from(d2);
                    for _ in 0..d1 {
                        d4 += d2;
                        d2 >>= 1;
                    }
                    ch.out.ptr = ch.out.ptr.wrapping_add(d4);
                }
                d0 = d0.wrapping_sub(i16::from(OCTAVE_NOTES[d5]) * 4).min(0x231);
                d6 = d0.max(0);
            }
        } else {
            d6 = d0.max(0);
        }
        if ch.out.trg && ch.out.lof != 0xffff {
            ch.out.ptr = ch.out.ptr.wrapping_add(u32::from(ch.out.lof));
            ch.out.lof = 0;
        }
        if d3 != ch.last_len {
            ch.last_len = d3;
            ch.out.trg = true;
        }
        ch.out.per = self.periods[d6 as usize];

        // The track delay: this channel's output a few ticks later on the next.
        let steps = ch.td_steps;
        if steps == 0 || c == 3 {
            return false;
        }
        let mut d0 = 31usize;
        let mut shift = 0i16;
        if steps == 0xff {
            self.ch[c].td_steps = 0;
            let n = &mut self.ch[c + 1];
            n.pat_vol = 0;
            n.td_steps = 0;
            n.td_offset = 0xff;
        } else {
            let mut e = self.ch[c].out;
            e.vol = ((u16::from(e.vol) * u16::from(self.ch[c].td_vol16)) >> 4) as u8;
            let n = &mut self.ch[c + 1];
            let d1 = n.td_offset.wrapping_add(1) & 31;
            n.td_offset = d1;
            // The echo's first tick starts it afresh.
            if n.td_steps == 0 {
                e.trg = true;
            }
            n.delayed[usize::from(d1)] = e;
            n.td_steps = steps;
            let x = d1.wrapping_sub(steps);
            d0 = usize::from(x & 31);
            shift = i16::from(MINUS4PLUS4[usize::from(x & 7)]);
        }
        let n = &mut self.ch[c + 1];
        n.out = n.delayed[d0];
        // A slot played is silenced, for when the echo comes round to it unfilled.
        n.delayed[d0].vol = 0;
        let per = n.out.per as i16;
        let p = (i32::from(shift) * i32::from(per)) << 4;
        n.out.per = per.wrapping_add((p >> 16) as i16) as u16;
        true
    }

    /// An instrument's steps from `d0`, as far as they go at once (steps
    /// stitched to the next go on); the ticks to its next.
    fn inst_steps(&mut self, c: usize, _info: &Inst, song: &Song, start: u16, out_pos: &mut u16) -> u8 {
        let base = song.insts[self.ch[c].inst.unwrap_or(0)].pattern;
        let mut d0 = start;
        let mut at = base + usize::from(d0) * 3;
        let mut d3 = false;
        let mut d7 = false;
        let steps = self.ch[c].steps;
        loop {
            let b0 = song.b(at);
            let stitched = b0 & 0x80 != 0;
            let ch = &mut self.ch[c];
            if !d3 {
                let p = b0 & 0x3f;
                if p != 0 {
                    ch.note_pitch = i16::from(p - 1) << 4;
                    ch.pinned = b0 & 0x40 != 0;
                }
            }
            let cmd = song.b(at + 1) & 15;
            let d5 = u16::from(song.b(at + 2));
            at += 3;
            // Where to go after the command: on (to the next step when
            // stitched), straight to the next step, or done with a count.
            enum N {
                Cont,
                Fetch,
                Done(u8),
            }
            let mut n = N::Cont;
            match cmd {
                0 | 4 => {
                    let w = d5.wrapping_sub(1);
                    if w < MAX_WAVES as u16 && w * 4 != ch.wave_num {
                        let w = usize::from(w);
                        ch.wave_num = (w * 4) as u16;
                        ch.wave = w;
                        let wi = song.waves[w];
                        ch.out.trg = true;
                        let a1 = song.starts[w];
                        let lo = wi.loop_offset();
                        let chip = wi.chipram();
                        let one_shot = |a1: u32| -> (u32, u16) { (a1.wrapping_add(lo as i16 as u32), if chip > lo { chip - lo } else { 2 }) };
                        if cmd == 0 {
                            if wi.subloop_len() == 0 {
                                let (p, l) = one_shot(a1);
                                ch.out.ptr = p;
                                ch.out.len = l;
                                ch.out.lof = 0xffff;
                            } else {
                                ch.out.ptr = a1;
                                ch.out.lof = 0;
                            }
                            ch.ping_pong = 0xff;
                            ch.subloop_wait = i16::from(wi.subloop_wait()) + 1;
                            ch.loop_offset = lo;
                        } else {
                            let mut d5 = chip;
                            if wi.subloop_len() == 0 {
                                let (p, l) = one_shot(a1);
                                ch.out.ptr = p;
                                ch.out.len = l;
                                d5 = l;
                                ch.out.lof = 0xffff;
                            } else {
                                ch.out.ptr = a1;
                                ch.out.lof = 0;
                            }
                            let d6 = ch.loop_offset;
                            if d5 < d6 {
                                ch.ping_pong = 0xff;
                            }
                            ch.subloop_wait = 0;
                            ch.loop_offset = d6.wrapping_sub(wi.subloop_step());
                        }
                    }
                }
                1 => ch.inst_pitch_slide = d5 as i16,
                2 => ch.inst_pitch_slide = -(d5 as i16),
                3 => {
                    if d5 == 1 {
                        Self::release(ch);
                    } else if d5 == 2 {
                        ch.adsr_phase = 0;
                        ch.adsr_vol = 0;
                    }
                }
                0xa => {
                    ch.inst_vol_slide = if d5 & 15 == 0 { (d5 >> 4) as u8 } else { (d5 as u8).wrapping_neg() };
                }
                0xb => {
                    if (d5 as i16) < d0 as i16 {
                        d0 = d5;
                        if d7 {
                            *out_pos = d0;
                            return ch.speed_stop;
                        }
                        at = base + usize::from(d5) * 3;
                        n = N::Fetch;
                    }
                }
                0xc => ch.inst_vol = d5.min(64),
                0xe => {
                    let k = usize::from(d5 & 3);
                    if k == 0 {
                        ch.sel_arp = 0;
                    } else {
                        match d5 >> 4 {
                            0 => {
                                let a = ch.arp[k - 1];
                                if a != 0 {
                                    ch.sel_arp = i16::from(a) << 4;
                                } else {
                                    d0 += 1;
                                    n = N::Fetch;
                                }
                            }
                            1 => ch.sel_arp = i16::from(ch.arp[k - 1]) << 4,
                            _ => {}
                        }
                    }
                }
                0xf => ch.speed_stop = if d5 == 0 { 0xff } else { d5 as u8 },
                _ => {}
            }
            if let N::Cont = n {
                d0 += 1;
                if !stitched {
                    n = N::Done(0);
                } else {
                    n = N::Fetch;
                }
            }
            match n {
                N::Done(x) => {
                    *out_pos = d0;
                    return x.wrapping_add(self.ch[c].speed_stop);
                }
                N::Fetch => {
                    d7 = true;
                    d3 = stitched;
                    if (d0 as u8) < steps {
                        continue;
                    }
                    *out_pos = d0;
                    return 0xffu8.wrapping_add(self.ch[c].speed_stop);
                }
                N::Cont => unreachable!(),
            }
        }
    }

    /// The tick's output to Paula's registers, as the replayer writes them.
    fn output(&mut self) {
        for c in 0..4 {
            let o = &mut self.ch[c].out;
            let r = &mut self.regs[c];
            r.trig = None;
            if o.trg {
                o.trg = false;
                r.len = o.len >> 1;
                r.ptr = o.ptr;
                r.trig = Some((r.ptr, r.len));
            }
            if o.lof != 0xffff {
                r.ptr = o.ptr.wrapping_add(u32::from(o.lof));
            } else {
                r.ptr = 0;
                r.len = 1;
            }
            r.per = o.per;
            r.vol = o.vol;
            if let Some((p, l)) = r.trig {
                self.paula.play(c, p as usize, usize::from(l) * 2);
            }
            self.paula.set_loop(c, r.ptr as usize, usize::from(r.len) * 2);
            self.paula.set_period(c, u32::from(r.per));
            self.paula.set_volume(c, u32::from(r.vol));
        }
    }

    fn step(&mut self) {
        if self.running {
            self.shown = (self.pos as usize, usize::from(self.row));
            let mut c = 0;
            loop {
                let cont = self.pattern_channel(c);
                if cont && self.ch[c].td_steps != 0 {
                    if c >= 2 {
                        break;
                    }
                    c += 2;
                    continue;
                }
                if c == 3 {
                    break;
                }
                c += 1;
            }
            self.advance();
            if !self.running {
                self.done = true;
            }
        }
        let mut c = 0;
        loop {
            let skip = self.instrument_channel(c);
            if self.ch[c].td_steps != 0 && c == 3 {
                break;
            }
            if skip {
                if c + 1 == 3 {
                    break;
                }
                c += 2;
                continue;
            }
            if c == 3 {
                break;
            }
            c += 1;
        }
        self.output();
    }
}

impl Driver for Player {
    fn channels(&self) -> usize {
        4
    }

    fn tick(&mut self) {
        self.step();
    }

    fn tick_seconds(&self) -> f64 {
        // Called each vertical blank.
        1.0 / paula::VBLANK
    }

    fn render(&mut self, rate: u32, frames: usize, out: &mut Bufs) {
        self.paula.render(rate, frames, out);
    }

    fn position(&self) -> (usize, usize) {
        self.shown
    }

    fn speed(&self) -> (u32, u32) {
        let s = self.speed;
        (u32::from(if s > 0x2f { s >> 4 } else { s }), 125)
    }

    fn take_done(&mut self) -> bool {
        std::mem::take(&mut self.done)
    }

    fn playing(&self) -> bool {
        self.running
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay::Machine;

    /// A version 1.0 song: one 128-point saw wave looping whole, an
    /// instrument that picks it, and a pitch 13 (C-2) on the first channel.
    fn song() -> Vec<u8> {
        let mut d = b"PRT\x1b".to_vec();
        let (posd, patt, inst) = (0x5cu32, 0x64u32, 0x70u32);
        let wave = inst + 32 + 8 + 3;
        for o in [posd, patt, inst, wave] {
            d.extend(o.to_be_bytes());
        }
        let mut title = b"Test".to_vec();
        title.resize(20, 0);
        d.extend(&title);
        d.extend([0; 20]);
        d.extend([0, 1, 1, 4, 1, 1]); // restart, patterns, length, steps, instruments, waves
        d.extend(0..24u8); // the waves' drawing order
        d.extend([0, 0]);
        d.extend([1, 0, 0, 0, 0, 0, 0, 0]); // position 0: pattern 1 on the first channel
        d.extend([13, 0x10, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]); // C-2 with instrument 1, then three empty steps
        d.extend([0; 32]); // instrument names
        d.extend([0, 0, 0, 0, 0, 15, 0, 1]); // no vibrato, instant attack, full sustain, one step
        d.extend([1, 0, 1]); // its step: wave 1
        d.extend([0; 24]); // wave names
        if d.len() % 2 == 1 {
            d.push(0);
        }
        let mut w = [0u8; WAVE_SIZE];
        w[2..4].copy_from_slice(&128u16.to_be_bytes()); // loop end
        w[4..6].copy_from_slice(&128u16.to_be_bytes()); // a sub-loop of the whole wave
        w[0xa..0xc].copy_from_slice(&128u16.to_be_bytes()); // its length
        w[0x18] = 0x80; // gain 1
        w[0x1e] = 0xff; // no envelope
        d.extend(w);
        d
    }

    #[test]
    fn plays_a_saw_at_c2() {
        let s = Song::parse(&song()).expect("parses");
        assert_eq!((s.title.as_str(), s.length, s.steps, s.mem.len()), ("Test", 1, 4, 130));
        // A saw falling from 127 over its 128 points.
        assert_eq!((s.mem[2], s.mem[3], s.mem[129]), (127, 126, -126));
        let tune: Arc<dyn Tune> = Arc::new(s);
        let mut m = Machine::new(tune.clone(), 0);
        let (mut l, mut r) = (vec![0f32; 44100], vec![0f32; 44100]);
        m.render(44100, 1.0, 1.0, &mut l, &mut r);
        // Period 424 over 128 points: 65.35 Hz, a fall through 0 a cycle.
        let falls = l.windows(2).filter(|w| w[0] > 0.0 && w[1] <= 0.0).count();
        assert!((64..=66).contains(&falls), "{falls} cycles");
        let peak = l.iter().fold(0f32, |a, &v| a.max(v.abs()));
        assert!((0.3..=0.4).contains(&peak), "peak {peak}");
        // Four rows of six ticks, at the PAL screen's rate.
        assert!((Machine::length(&tune, 0, 60.0) - 24.0 / paula::VBLANK).abs() < 1e-9);
    }

    /// Each tick's Paula registers, as the reference harness logs them.
    fn log(song: &Arc<Song>, ticks: usize) -> String {
        let mut p = Player::new(song.clone());
        let mut s = String::new();
        for t in 0..ticks {
            p.step();
            s += &format!("{t}");
            for c in 0..4 {
                let r = &p.regs[c];
                match r.trig {
                    Some((ptr, len)) => s += &format!(" | T {ptr:x} {len:x}"),
                    None => s += " | -",
                }
                s += &format!(" {:x} {:x} {:x} {:x}", r.ptr, r.len, r.per, r.vol);
            }
            s += "\n";
        }
        s
    }

    /// Compares each song in `PRT_DIR` with the original replayer's run
    /// (`<song>.smp`, its sample memory, and `<song>.log`, its registers
    /// each tick, in `PRT_REF`).
    #[test]
    #[ignore]
    fn matches_the_original() {
        let (Ok(dir), Ok(refs)) = (std::env::var("PRT_DIR"), std::env::var("PRT_REF")) else { return };
        let mut names: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "prt"))
            .collect();
        names.sort();
        let (mut ok, mut bad) = (0, 0);
        for p in names {
            let name = p.file_stem().unwrap().to_string_lossy().to_string();
            if std::env::var("PRT_ONLY").is_ok_and(|o| name != o) {
                continue;
            }
            let Ok(smp) = std::fs::read(format!("{refs}/{name}.smp")) else { continue };
            let Ok(reflog) = std::fs::read_to_string(format!("{refs}/{name}.log")) else { continue };
            let song = Arc::new(Song::parse(&std::fs::read(&p).unwrap()).expect("parses"));
            let mem: Vec<u8> = song.mem.iter().map(|&x| x as u8).collect();
            let mut msg = String::new();
            if std::env::var("PRT_ONLY").is_ok() {
                std::fs::write(format!("{refs}/{name}.ours"), &mem).unwrap();
            }
            if mem != smp {
                let first = mem.iter().zip(&smp).position(|(a, b)| a != b);
                let wave = first.map(|f| song.starts.iter().rposition(|&s| s as usize <= f && s != 0).unwrap_or(0));
                msg += &format!("samples differ (ours {}, ref {}) first at {first:?}, wave {wave:?}; ", mem.len(), smp.len());
            }
            let ours = log(&song, reflog.lines().count());
            if let Some((i, (a, b))) = ours.lines().zip(reflog.lines()).enumerate().find(|(_, (a, b))| a != b) {
                msg += &format!("tick {i}:\n  ours {a}\n  ref  {b}");
            }
            if msg.is_empty() {
                ok += 1;
            } else {
                bad += 1;
                eprintln!("{name}: {msg}");
            }
        }
        eprintln!("{ok} the same, {bad} not");
        assert_eq!(bad, 0);
    }
}
