// Copyright 2026 ansiapps. Neumetik Synth, proprietary: see LICENSE.
//
// A port of SiON's effectors (org.si.sion.effector: SiEffectModule,
// SiEffectStream and the effectors, github.com/keim/SiON):
//
// Copyright (c) 2008-2016 keim, MIT License (see table.rs for the text).

//! The effect chain: a stream for each send slot (1 to 7) and one for the
//! master (0), each with a chain of effectors (`ws`, `eq`, `delay`,
//! `reverb`, `chorus`, `autopan`, `ds`, `speaker`, `comp`, `dist`,
//! `stereo` and the filters `lf hf bf nf pf af lb hb`), set up by
//! `#EFFECTn{...}`. Buffers are interleaved stereo. (The vowel filter and
//! the controlled filters `nlf` and `nhf` are not ported yet.)

use super::module::*;
use super::table::*;
use super::track::EnvTable;
use std::sync::Arc;

/// What a few effectors read from the song and the tables.
#[derive(Clone)]
pub struct Ctx {
    pub table: Arc<Table>,
    pub envelopes: Arc<Vec<Option<Arc<EnvTable>>>>,
}

/// An effector (`SiEffectBase`).
pub trait Effector {
    /// `mmlCallback`: the numbers of the command (NaN where left out).
    fn mml(&mut self, args: &[f64; 16]);
    /// `prepareProcess`: buffers cleared; the channel count it works with.
    fn prepare(&mut self) -> i32;
    /// `process`: `len` frames from `start` of an interleaved stereo buffer.
    fn process(&mut self, channels: i32, buf: &mut [f64], start: usize, len: usize) -> i32;
}

fn or(a: f64, d: f64) -> f64 {
    if a.is_nan() {
        d
    } else {
        a
    }
}

// wave shaper
struct WaveShaper {
    coefficient: i32,
    output_level: f64,
}

impl WaveShaper {
    fn new() -> WaveShaper {
        let mut w = WaveShaper { coefficient: 0, output_level: 1.0 };
        w.set(0.5, 1.0);
        w
    }

    fn set(&mut self, mut distortion: f64, output_level: f64) {
        if distortion >= 1.0 {
            distortion = 0.9999847412109375;
        }
        self.coefficient = to_i32(2.0 * distortion / (1.0 - distortion));
        self.output_level = output_level;
    }
}

impl Effector for WaveShaper {
    fn mml(&mut self, a: &[f64; 16]) {
        self.set(if a[0].is_nan() { 0.5 } else { a[0] * 0.01 }, if a[1].is_nan() { 1.0 } else { a[1] * 0.01 });
    }

    fn prepare(&mut self) -> i32 {
        2
    }

    fn process(&mut self, channels: i32, buf: &mut [f64], start: usize, len: usize) -> i32 {
        let c = f64::from(self.coefficient);
        let c1 = (1.0 + c) * self.output_level;
        let (s, e) = (start << 1, (start + len) << 1);
        if channels == 2 {
            for x in &mut buf[s..e] {
                let n = *x;
                *x = c1 * n / (1.0 + c * n.abs());
            }
        } else {
            let mut i = s;
            while i < e {
                let n = buf[i];
                let n = c1 * n / (1.0 + c * n.abs());
                buf[i] = n;
                buf[i + 1] = n;
                i += 2;
            }
        }
        channels
    }
}

// stereo delay
const DELAY_BITS: u32 = 16;
const DELAY_MASK: usize = (1 << DELAY_BITS) - 1;

struct StereoDelay {
    buf: [Vec<f64>; 2],
    read: usize,
    write: usize,
    feedback: f64,
    cross: usize,
    wet: f64,
}

impl StereoDelay {
    fn new() -> StereoDelay {
        let mut d = StereoDelay { buf: [vec![0.0; 1 << DELAY_BITS], vec![0.0; 1 << DELAY_BITS]], read: 0, write: 0, feedback: 0.0, cross: 0, wet: 0.0 };
        d.set(250.0, 0.25, false, 0.25);
        d
    }

    fn set(&mut self, delay_time: f64, feedback: f64, is_cross: bool, wet: f64) {
        let mut offset = to_i32(delay_time * 44.1);
        if offset as usize > DELAY_MASK {
            offset = DELAY_MASK as i32;
        }
        self.write = (self.read + offset.max(0) as usize) & DELAY_MASK;
        self.feedback = if feedback >= 1.0 {
            0.9990234375
        } else if feedback <= -1.0 {
            -0.9990234375
        } else {
            feedback
        };
        self.cross = usize::from(is_cross);
        self.wet = wet;
    }
}

impl Effector for StereoDelay {
    fn mml(&mut self, a: &[f64; 16]) {
        self.set(or(a[0], 250.0), if a[1].is_nan() { 0.25 } else { a[1] * 0.01 }, a[2] == 1.0, if a[3].is_nan() { 1.0 } else { a[3] * 0.01 });
    }

    fn prepare(&mut self) -> i32 {
        self.buf[0].fill(0.0);
        self.buf[1].fill(0.0);
        2
    }

    fn process(&mut self, channels: i32, buf: &mut [f64], start: usize, len: usize) -> i32 {
        let dry = 1.0 - self.wet;
        let (mut i, e) = (start << 1, (start + len) << 1);
        let (rl, rr) = (self.cross, 1 - self.cross);
        while i < e {
            let n = self.buf[rl][self.read];
            self.buf[0][self.write] = buf[i] - n * self.feedback;
            buf[i] = buf[i] * dry + n * self.wet;
            i += 1;
            let n = self.buf[rr][self.read];
            self.buf[1][self.write] = buf[i] - n * self.feedback;
            buf[i] = buf[i] * dry + n * self.wet;
            i += 1;
            self.write = (self.write + 1) & DELAY_MASK;
            self.read = (self.read + 1) & DELAY_MASK;
        }
        channels
    }
}

// stereo reverb
const REVERB_BITS: u32 = 13;
const REVERB_MASK: usize = (1 << REVERB_BITS) - 1;

struct StereoReverb {
    l: Vec<f64>,
    r: Vec<f64>,
    read0: usize,
    read1: usize,
    read2: usize,
    write: usize,
    fb0: f64,
    fb1: f64,
    fb2: f64,
    wet: f64,
}

impl StereoReverb {
    fn new() -> StereoReverb {
        let mut v = StereoReverb { l: vec![0.0; 1 << REVERB_BITS], r: vec![0.0; 1 << REVERB_BITS], read0: 0, read1: 0, read2: 0, write: 0, fb0: 0.0, fb1: 0.0, fb2: 0.0, wet: 0.0 };
        v.set(0.7, 0.4, 0.8, 0.3);
        v
    }

    fn set(&mut self, delay1: f64, delay2: f64, feedback: f64, wet: f64) {
        let d1 = delay1.clamp(0.01, 0.99);
        let d2 = delay2.clamp(0.01, 0.99);
        let m = REVERB_MASK as i32;
        self.write = (self.read0 + REVERB_MASK) & REVERB_MASK;
        self.read1 = (to_i32(self.read0 as f64 + f64::from(m) * (1.0 - d1)) & m) as usize;
        self.read2 = (to_i32(self.read0 as f64 + f64::from(m) * (1.0 - d2)) & m) as usize;
        let fb = feedback.clamp(-0.99, 0.99);
        self.fb0 = fb * 0.2;
        self.fb1 = fb * 0.3;
        self.fb2 = fb * 0.5;
        self.wet = wet;
    }
}

impl Effector for StereoReverb {
    fn mml(&mut self, a: &[f64; 16]) {
        let g = |i: usize, d: f64| if a[i].is_nan() { d } else { a[i] * 0.01 };
        self.set(g(0, 0.7), g(1, 0.4), g(2, 0.8), g(3, 1.0));
    }

    fn prepare(&mut self) -> i32 {
        self.l.fill(0.0);
        self.r.fill(0.0);
        2
    }

    fn process(&mut self, channels: i32, buf: &mut [f64], start: usize, len: usize) -> i32 {
        let dry = 1.0 - self.wet;
        let (mut i, e) = (start << 1, (start + len) << 1);
        while i < e {
            let n = self.l[self.read0] * self.fb0 + self.l[self.read1] * self.fb1 + self.l[self.read2] * self.fb2;
            self.l[self.write] = buf[i] - n;
            buf[i] = buf[i] * dry + n * self.wet;
            i += 1;
            let n = self.r[self.read0] * self.fb0 + self.r[self.read1] * self.fb1 + self.r[self.read2] * self.fb2;
            self.r[self.write] = buf[i] - n;
            buf[i] = buf[i] * dry + n * self.wet;
            i += 1;
            self.write = (self.write + 1) & REVERB_MASK;
            self.read0 = (self.read0 + 1) & REVERB_MASK;
            self.read1 = (self.read1 + 1) & REVERB_MASK;
            self.read2 = (self.read2 + 1) & REVERB_MASK;
        }
        channels
    }
}

// stereo chorus
const CHORUS_BITS: u32 = 12;
const CHORUS_MASK: usize = (1 << CHORUS_BITS) - 1;

struct StereoChorus {
    l: Vec<f64>,
    r: Vec<f64>,
    read: usize,
    write: usize,
    feedback: f64,
    depth: f64,
    wet: f64,
    lfo_phase: usize,
    lfo_step: i32,
    lfo_residue: i32,
    phase_invert: i32,
    phase_table: Vec<i32>,
}

impl StereoChorus {
    fn new() -> StereoChorus {
        let mut c = StereoChorus {
            l: vec![0.0; 1 << CHORUS_BITS],
            r: vec![0.0; 1 << CHORUS_BITS],
            read: 0,
            write: 0,
            feedback: 0.0,
            depth: 0.0,
            wet: 0.0,
            lfo_phase: 0,
            lfo_step: 0,
            lfo_residue: 0,
            phase_invert: 1,
            phase_table: vec![],
        };
        c.set(20.0, 0.2, 4.0, 20.0, 0.5, true);
        c
    }

    fn set(&mut self, delay_time: f64, feedback: f64, frequency: f64, depth: f64, wet: f64, invert: bool) {
        if frequency == 0.0 || depth == 0.0 || delay_time == 0.0 {
            return;
        }
        let mut offset = to_i32(delay_time * 44.1);
        if offset as usize > CHORUS_MASK {
            offset = CHORUS_MASK as i32;
        }
        self.write = (self.read + offset.max(0) as usize) & CHORUS_MASK;
        self.feedback = if feedback >= 1.0 {
            0.9990234375
        } else if feedback <= -1.0 {
            -0.9990234375
        } else {
            feedback
        };
        self.depth = if depth >= f64::from(offset - 4) { f64::from(offset - 4) } else { depth };
        let mut table_size = to_i32(self.depth * 6.283185307179586);
        if f64::from(table_size) * frequency > 11025.0 {
            table_size = to_i32(11025.0 / frequency);
        }
        let table_size = table_size.max(1) as usize;
        self.phase_table.resize(table_size, 0);
        let dp = 6.283185307179586 / table_size as f64;
        let mut p = 0.0f64;
        for slot in self.phase_table.iter_mut() {
            *slot = to_i32(p.sin() * self.depth + 0.5);
            p += dp;
        }
        self.lfo_step = to_i32(44100.0 / (table_size as f64 * frequency));
        if self.lfo_step <= 4 {
            self.lfo_step = 4;
        }
        self.lfo_residue = self.lfo_step << 1;
        self.wet = wet;
        self.phase_invert = if invert { -1 } else { 1 };
        if self.lfo_phase >= self.phase_table.len() {
            self.lfo_phase = 0;
        }
    }

    fn lfo(&mut self, buf: &mut [f64], start: usize, len: usize) {
        let dry = 1.0 - self.wet;
        let dl = self.phase_table[self.lfo_phase];
        let dr = dl * self.phase_invert;
        let mut i = start;
        let e = start + len;
        while i < e {
            let p = ((self.read as i32 + dl) & CHORUS_MASK as i32) as usize;
            let n = self.l[p];
            self.l[self.write] = buf[i] - n * self.feedback;
            buf[i] = buf[i] * dry + n * self.wet;
            i += 1;
            let p = ((self.read as i32 + dr) & CHORUS_MASK as i32) as usize;
            let n = self.r[p];
            self.r[self.write] = buf[i] - n * self.feedback;
            buf[i] = buf[i] * dry + n * self.wet;
            i += 1;
            self.write = (self.write + 1) & CHORUS_MASK;
            self.read = (self.read + 1) & CHORUS_MASK;
        }
    }
}

impl Effector for StereoChorus {
    fn mml(&mut self, a: &[f64; 16]) {
        self.set(or(a[0], 20.0), if a[1].is_nan() { 0.2 } else { a[1] * 0.01 }, or(a[2], 4.0), or(a[3], 20.0), if a[4].is_nan() { 0.5 } else { a[4] * 0.01 }, if a[5].is_nan() { true } else { a[5] != 0.0 });
    }

    fn prepare(&mut self) -> i32 {
        self.lfo_phase = 0;
        self.lfo_residue = 0;
        self.read = 0;
        self.l.fill(0.0);
        self.r.fill(0.0);
        2
    }

    fn process(&mut self, channels: i32, buf: &mut [f64], start: usize, len: usize) -> i32 {
        let mut istep = self.lfo_residue;
        let mut i = (start << 1) as i32;
        let imax = ((start + len) << 1) as i32;
        while i < imax - istep {
            self.lfo(buf, i as usize, istep as usize);
            self.lfo_phase += 1;
            if self.lfo_phase == self.phase_table.len() {
                self.lfo_phase = 0;
            }
            i += istep;
            istep = self.lfo_step << 1;
        }
        self.lfo(buf, i as usize, (imax - i) as usize);
        self.lfo_residue = istep - (imax - i);
        channels
    }
}

// equaliser
struct Equaliser {
    f1: [[f64; 4]; 2],
    f2: [[f64; 4]; 2],
    sdm: [[f64; 3]; 2],
    lf: f64,
    hf: f64,
    lg: f64,
    mg: f64,
    hg: f64,
}

impl Equaliser {
    fn new() -> Equaliser {
        let mut e = Equaliser { f1: [[0.0; 4]; 2], f2: [[0.0; 4]; 2], sdm: [[0.0; 3]; 2], lf: 0.0, hf: 0.0, lg: 1.0, mg: 1.0, hg: 1.0 };
        e.set(1.0, 1.0, 1.0, 880.0, 5000.0);
        e
    }

    fn set(&mut self, lg: f64, mg: f64, hg: f64, lf: f64, hf: f64) {
        self.lg = lg;
        self.mg = mg;
        self.hg = hg;
        self.lf = 2.0 * (lf * 0.00007123792865282977).sin();
        self.hf = 2.0 * (hf * 0.00007123792865282977).sin();
    }

    #[inline]
    fn band(&mut self, ch: usize, n: f64) -> f64 {
        let (lf, hf) = (self.lf, self.hf);
        let f1 = &mut self.f1[ch];
        f1[0] += (lf * (n - f1[0])) + 2.3283064370807974e-10;
        f1[1] += lf * (f1[0] - f1[1]);
        f1[2] += lf * (f1[1] - f1[2]);
        f1[3] += lf * (f1[2] - f1[3]);
        let f2 = &mut self.f2[ch];
        f2[0] += (hf * (n - f2[0])) + 2.3283064370807974e-10;
        f2[1] += hf * (f2[0] - f2[1]);
        f2[2] += hf * (f2[1] - f2[2]);
        f2[3] += hf * (f2[2] - f2[3]);
        let l = f1[3];
        let h = self.sdm[ch][2] - f2[3];
        let m = self.sdm[ch][2] - (h + l);
        self.sdm[ch][2] = self.sdm[ch][1];
        self.sdm[ch][1] = self.sdm[ch][0];
        self.sdm[ch][0] = n;
        l * self.lg + m * self.mg + h * self.hg
    }
}

impl Effector for Equaliser {
    fn mml(&mut self, a: &[f64; 16]) {
        let g = |i: usize| if a[i].is_nan() { 1.0 } else { a[i] * 0.01 };
        self.set(g(0), g(1), g(2), or(a[3], 880.0), or(a[4], 5000.0));
    }

    fn prepare(&mut self) -> i32 {
        self.f1 = [[0.0; 4]; 2];
        self.f2 = [[0.0; 4]; 2];
        self.sdm = [[0.0; 3]; 2];
        2
    }

    fn process(&mut self, channels: i32, buf: &mut [f64], start: usize, len: usize) -> i32 {
        let (mut i, e) = (start << 1, (start + len) << 1);
        if channels == 2 {
            while i < e {
                buf[i] = self.band(0, buf[i]);
                buf[i + 1] = self.band(1, buf[i + 1]);
                i += 2;
            }
        } else {
            while i < e {
                let n = self.band(0, buf[i]);
                buf[i] = n;
                buf[i + 1] = n;
                i += 2;
            }
        }
        channels
    }
}

// compressor
struct Compressor {
    window: Vec<f64>,
    at: usize,
    window_samples: i32,
    rms_total: f64,
    averaging: f64,
    threshold2: f64,
    att_rate: f64,
    rel_rate: f64,
    max_gain: f64,
    mixing: f64,
    gain: f64,
}

impl Compressor {
    fn new() -> Compressor {
        let mut c = Compressor { window: vec![], at: 0, window_samples: 0, rms_total: 0.0, averaging: 0.0, threshold2: 0.0, att_rate: 0.0, rel_rate: 0.0, max_gain: 0.0, mixing: 0.0, gain: 2.0 };
        c.set(0.7, 50.0, 20.0, 20.0, -6.0, 0.5);
        c
    }

    fn set(&mut self, thres: f64, wnd: f64, att: f64, rel: f64, max_gain: f64, mixing: f64) {
        self.threshold2 = thres * thres;
        self.window_samples = to_i32(wnd * 44.1);
        self.averaging = 1.0 / f64::from(self.window_samples);
        self.att_rate = if att == 0.0 { 0.5 } else { 2f64.powf(-1.0 / (att * 44.1)) };
        self.rel_rate = if rel == 0.0 { 2.0 } else { 2f64.powf(1.0 / (rel * 44.1)) };
        self.max_gain = 2f64.powf(-max_gain / 6.0);
        self.mixing = mixing;
    }
}

impl Effector for Compressor {
    fn mml(&mut self, a: &[f64; 16]) {
        self.set(if a[0].is_nan() { 0.7 } else { a[0] * 0.01 }, or(a[1], 50.0), or(a[2], 20.0), or(a[3], 20.0), if a[4].is_nan() { -6.0 } else { -a[4] }, if a[5].is_nan() { 0.5 } else { a[5] * 0.01 });
    }

    fn prepare(&mut self) -> i32 {
        self.window = vec![0.0; self.window_samples.max(1) as usize];
        self.at = 0;
        self.rms_total = 0.0;
        self.gain = 2.0;
        2
    }

    fn process(&mut self, channels: i32, buf: &mut [f64], start: usize, len: usize) -> i32 {
        if self.window.is_empty() {
            return channels;
        }
        let (mut i, e) = (start << 1, (start + len) << 1);
        while i < e {
            let (mut l, mut r) = (buf[i], buf[i + 1]);
            self.at = (self.at + 1) % self.window.len();
            self.rms_total -= self.window[self.at];
            self.window[self.at] = l * l + r * r;
            self.rms_total += self.window[self.at];
            let rms2 = self.rms_total * self.averaging;
            self.gain *= if rms2 > self.threshold2 { self.att_rate } else { self.rel_rate };
            if self.gain > self.max_gain {
                self.gain = self.max_gain;
            }
            l = (l * self.gain).clamp(-1.0, 1.0);
            r = (r * self.gain).clamp(-1.0, 1.0);
            buf[i] = l * self.mixing;
            buf[i + 1] = r * self.mixing;
            i += 2;
        }
        channels
    }
}

// distortion
const THRESHOLD: f64 = 0.0000152587890625;

struct Distortion {
    pre_scale: f64,
    limit: f64,
    filter: bool,
    a1: f64,
    a2: f64,
    b0: f64,
    b1: f64,
    b2: f64,
    in1: f64,
    in2: f64,
    out1: f64,
    out2: f64,
}

impl Distortion {
    fn new() -> Distortion {
        let mut d = Distortion { pre_scale: 0.0, limit: 0.0, filter: false, a1: 0.0, a2: 0.0, b0: 0.0, b1: 0.0, b2: 0.0, in1: 0.0, in2: 0.0, out1: 0.0, out2: 0.0 };
        d.set(-60.0, 18.0, 2400.0, 1.0);
        d
    }

    fn set(&mut self, pre: f64, post: f64, lpf_freq: f64, slope: f64) {
        let post_scale = 2f64.powf(-post / 6.0);
        self.pre_scale = 2f64.powf(-pre / 6.0) * post_scale;
        self.limit = post_scale;
        self.filter = lpf_freq > 0.0;
        if self.filter {
            let omg = lpf_freq * 0.00014247585730565955;
            let (cos, sin) = (omg.cos(), omg.sin());
            let ang = 0.34657359027997264 * slope * omg / sin;
            let alp = sin * (ang.exp() - (-ang).exp()) * 0.5;
            let ia0 = 1.0 / (1.0 + alp);
            self.a1 = -2.0 * cos * ia0;
            self.a2 = (1.0 - alp) * ia0;
            self.b1 = (1.0 - cos) * ia0;
            self.b0 = self.b1 * 0.5;
            self.b2 = self.b0;
        }
    }
}

impl Effector for Distortion {
    fn mml(&mut self, a: &[f64; 16]) {
        self.set(or(a[0], -60.0), or(a[1], 18.0), or(a[2], 2400.0), or(a[3], 1.0));
    }

    fn prepare(&mut self) -> i32 {
        self.in1 = 0.0;
        self.in2 = 0.0;
        self.out1 = 0.0;
        self.out2 = 0.0;
        1
    }

    fn process(&mut self, _channels: i32, buf: &mut [f64], start: usize, len: usize) -> i32 {
        if self.out1 < THRESHOLD {
            self.out1 = 0.0;
            self.out2 = 0.0;
        }
        let (mut i, e) = (start << 1, (start + len) << 1);
        while i < e {
            let n = (buf[i] * self.pre_scale).clamp(-self.limit, self.limit);
            let out = if self.filter {
                let out = self.b0 * n + self.b1 * self.in1 + self.b2 * self.in2 - self.a1 * self.out1 - self.a2 * self.out2;
                self.in2 = self.in1;
                self.in1 = n;
                self.out2 = self.out1;
                self.out1 = out;
                out
            } else {
                n
            };
            buf[i] = out;
            buf[i + 1] = out;
            i += 2;
        }
        1
    }
}

// auto pan
struct AutoPan {
    stereo: bool,
    lfo_step: i32,
    lfo_residue: i32,
    table: [f64; 256],
    pl: usize,
    pr: usize,
}

impl AutoPan {
    fn new() -> AutoPan {
        let mut a = AutoPan { stereo: false, lfo_step: 0, lfo_residue: 0, table: [0.0; 256], pl: 0, pr: 128 };
        a.set(1.0, 1.0);
        a
    }

    fn set(&mut self, frequency: f64, mut width: f64) {
        let frequency = frequency * 0.5;
        self.lfo_step = to_i32(172.265625 / frequency);
        if self.lfo_step <= 4 {
            self.lfo_step = 4;
        }
        self.stereo = false;
        if width == 0.0 {
            width = 1.0;
            self.stereo = true;
        }
        width *= 0.01227184630308513;
        for i in -128..128i32 {
            self.table[(self.pl + (i + 128) as usize) & 255] = (1.5707963267948965 + f64::from(i) * width).sin();
        }
        self.pr = (self.pl + 128) & 255;
    }

    fn lfo(&mut self, buf: &mut [f64], start: usize, len: usize) {
        let (c, s) = (self.table[self.pl], self.table[self.pr]);
        let (mut i, e) = (start, start + len);
        if self.stereo {
            while i < e {
                let (l, r) = (buf[i], buf[i + 1]);
                buf[i] = l * c - r * s;
                buf[i + 1] = l * s + r * c;
                i += 2;
            }
        } else {
            while i < e {
                let l = buf[i];
                buf[i] = l * c;
                buf[i + 1] = l * s;
                i += 2;
            }
        }
        self.pl = (self.pl + 1) & 255;
        self.pr = (self.pr + 1) & 255;
    }
}

impl Effector for AutoPan {
    fn mml(&mut self, a: &[f64; 16]) {
        self.set(or(a[0], 1.0), if a[1].is_nan() { 1.0 } else { a[1] * 0.01 });
    }

    fn prepare(&mut self) -> i32 {
        if self.stereo {
            2
        } else {
            1
        }
    }

    fn process(&mut self, _channels: i32, buf: &mut [f64], start: usize, len: usize) -> i32 {
        let mut istep = self.lfo_residue;
        let mut i = (start << 1) as i32;
        let imax = ((start + len) << 1) as i32;
        while i < imax - istep {
            self.lfo(buf, i as usize, istep as usize);
            i += istep;
            istep = self.lfo_step << 1;
        }
        self.lfo(buf, i as usize, (imax - i) as usize);
        self.lfo_residue = istep - (imax - i);
        2
    }
}

// stereo expander
struct StereoExpander {
    l2l: f64,
    r2l: f64,
    l2r: f64,
    r2r: f64,
    mono: bool,
}

impl StereoExpander {
    fn new() -> StereoExpander {
        let mut s = StereoExpander { l2l: 0.0, r2l: 0.0, l2r: 0.0, r2r: 0.0, mono: false };
        s.set(1.4, 0.0, false);
        s
    }

    fn set(&mut self, width: f64, rotation: f64, invert: bool) {
        self.mono = width == 0.0 && rotation == 0.0 && !invert;
        let half = width * 0.7853981633974483;
        let center = (rotation + 0.5) * 1.5707963267948965;
        let (langle, rangle) = (center - half, center + half);
        let inv = if invert { -1.0 } else { 1.0 };
        self.l2l = langle.cos();
        self.r2l = langle.sin();
        self.l2r = rangle.cos() * inv;
        self.r2r = rangle.sin() * inv;
        let (x, y) = (self.l2l + self.l2r, self.r2l + self.r2r);
        let l = (x * x + y * y).sqrt();
        if l > 0.01 {
            let l = 1.0 / l;
            self.l2l *= l;
            self.r2l *= l;
            self.l2r *= l;
            self.r2r *= l;
        }
    }
}

impl Effector for StereoExpander {
    fn mml(&mut self, a: &[f64; 16]) {
        self.set(if a[1].is_nan() { 1.4 } else { a[1] * 0.01 }, if a[2].is_nan() { 0.0 } else { a[2] * 0.01 }, if a[0].is_nan() { false } else { a[0] != 0.0 });
    }

    fn prepare(&mut self) -> i32 {
        2
    }

    fn process(&mut self, channels: i32, buf: &mut [f64], start: usize, len: usize) -> i32 {
        let (mut i, e) = (start << 1, (start + len) << 1);
        if self.mono {
            while i < e {
                let l = (buf[i] + buf[i + 1]) * 0.7071067811865476;
                buf[i] = l;
                buf[i + 1] = l;
                i += 2;
            }
            return 1;
        }
        while i < e {
            let (l, r) = (buf[i], buf[i + 1]);
            buf[i] = l * self.l2l + r * self.r2l;
            buf[i + 1] = l * self.l2r + r * self.r2r;
            i += 2;
        }
        let _ = channels;
        2
    }
}

// down sampler
struct DownSampler {
    freq_shift: i32,
    bit_conv0: f64,
    bit_conv1: f64,
    channel_count: i32,
}

impl DownSampler {
    fn new() -> DownSampler {
        let mut d = DownSampler { freq_shift: 0, bit_conv0: 1.0, bit_conv1: 1.0, channel_count: 2 };
        d.set(0, 16, 2);
        d
    }

    fn set(&mut self, freq_shift: i32, bit_rate: i32, channel_count: i32) {
        self.freq_shift = freq_shift;
        self.bit_conv0 = f64::from(1i32 << bit_rate.clamp(0, 30));
        self.bit_conv1 = 1.0 / self.bit_conv0;
        self.channel_count = channel_count;
    }
}

impl Effector for DownSampler {
    fn mml(&mut self, a: &[f64; 16]) {
        self.set(or(a[0], 0.0) as i32, or(a[1], 16.0) as i32, or(a[2], 2.0) as i32);
    }

    fn prepare(&mut self) -> i32 {
        2
    }

    fn process(&mut self, channels: i32, buf: &mut [f64], start: usize, len: usize) -> i32 {
        let (s, e) = (start << 1, (start + len) << 1);
        let q = |x: f64, me: &DownSampler| -> f64 { f64::from(to_i32(x)) * me.bit_conv1 };
        if self.channel_count == 1 {
            // Mono: the left and right of a group of frames together.
            let jmax = 2usize << self.freq_shift.clamp(0, 12);
            let bc0 = (1.0 / jmax as f64) * self.bit_conv0;
            let mut i = s;
            while i + jmax <= e {
                let l: f64 = buf[i..i + jmax].iter().sum();
                let l = q(l * bc0, self);
                buf[i..i + jmax].fill(l);
                i += jmax;
            }
        } else if self.freq_shift == 0 {
            for x in &mut buf[s..e] {
                *x = f64::from(to_i32(*x * self.bit_conv0)) * self.bit_conv1;
            }
        } else {
            let g = 1usize << self.freq_shift.clamp(0, 12);
            let bc0 = (1.0 / g as f64) * self.bit_conv0;
            let mut i = s;
            while i + g * 2 <= e {
                let (mut l, mut r) = (0.0, 0.0);
                for k in 0..g {
                    l += buf[i + k * 2];
                    r += buf[i + k * 2 + 1];
                }
                let (l, r) = (q(l * bc0, self), q(r * bc0, self));
                for k in 0..g {
                    buf[i + k * 2] = l;
                    buf[i + k * 2 + 1] = r;
                }
                i += g * 2;
            }
        }
        channels
    }
}

// speaker simulator
struct Speaker {
    spring: f64,
    pos: [f64; 2],
    prev: [f64; 2],
}

impl Speaker {
    fn new() -> Speaker {
        let mut s = Speaker { spring: 0.96, pos: [0.0; 2], prev: [0.0; 2] };
        s.set(0.2);
        s
    }

    fn set(&mut self, hardness: f64) {
        self.spring = 1.0 - hardness * hardness;
        if self.spring < 0.1 {
            self.spring = 0.1;
        }
    }
}

impl Effector for Speaker {
    fn mml(&mut self, a: &[f64; 16]) {
        self.set(if a[0].is_nan() { 0.2 } else { a[0] * 0.01 });
    }

    fn prepare(&mut self) -> i32 {
        self.pos = [0.0; 2];
        self.prev = [0.0; 2];
        2
    }

    fn process(&mut self, channels: i32, buf: &mut [f64], start: usize, len: usize) -> i32 {
        let (mut i, e) = (start << 1, (start + len) << 1);
        while i < e {
            for ch in 0..2 {
                let d = buf[i + ch] - self.prev[ch];
                self.pos[ch] = self.pos[ch] * self.spring + d;
                self.prev[ch] = buf[i + ch];
                buf[i + ch] = self.pos[ch];
            }
            i += 2;
        }
        channels
    }
}

/// The biquad filters (`SiFilterBase`) of SiON's effector.
#[derive(Clone, Copy, PartialEq, Eq)]
enum FilterKind {
    Low,
    High,
    Band,
    Notch,
    Peak,
    All,
    LowBoost,
    HighBoost,
}

struct Filter {
    kind: FilterKind,
    a1: f64,
    a2: f64,
    b0: f64,
    b1: f64,
    b2: f64,
    in1: [f64; 2],
    in2: [f64; 2],
    out1: [f64; 2],
    out2: [f64; 2],
}

fn sinh(n: f64) -> f64 {
    (n.exp() - (-n).exp()) * 0.5
}

impl Filter {
    fn new(kind: FilterKind) -> Filter {
        let mut f = Filter { kind, a1: 0.0, a2: 0.0, b0: 0.0, b1: 0.0, b2: 0.0, in1: [0.0; 2], in2: [0.0; 2], out1: [0.0; 2], out2: [0.0; 2] };
        match kind {
            FilterKind::Low => f.set(800.0, 1.0, 6.0),
            FilterKind::High | FilterKind::HighBoost => f.set(5500.0, 1.0, 6.0),
            _ => f.set(3000.0, 1.0, 6.0),
        }
        f
    }

    fn set(&mut self, freq: f64, band: f64, gain: f64) {
        let omg = freq * 0.00014247585730565955;
        let (cos, sin) = (omg.cos(), omg.sin());
        match self.kind {
            FilterKind::LowBoost | FilterKind::HighBoost => {
                let slope = band.max(1.0);
                let a = 10f64.powf(gain * 0.025);
                let alp = sin * 0.5 * ((a + 1.0 / a) * (1.0 / slope - 1.0) + 2.0).sqrt();
                let alps_a2 = alp * a.sqrt() * 2.0;
                if self.kind == FilterKind::LowBoost {
                    let ia0 = 1.0 / ((a + 1.0) + (a - 1.0) * cos + alps_a2);
                    self.a1 = -2.0 * ((a - 1.0) + (a + 1.0) * cos) * ia0;
                    self.a2 = ((a + 1.0) + (a - 1.0) * cos - alps_a2) * ia0;
                    self.b0 = ((a + 1.0) - (a - 1.0) * cos + alps_a2) * a * ia0;
                    self.b1 = 2.0 * ((a - 1.0) - (a + 1.0) * cos) * a * ia0;
                    self.b2 = ((a + 1.0) - (a - 1.0) * cos - alps_a2) * a * ia0;
                } else {
                    let ia0 = 1.0 / ((a + 1.0) - (a - 1.0) * cos + alps_a2);
                    self.a1 = 2.0 * ((a - 1.0) - (a + 1.0) * cos) * ia0;
                    self.a2 = ((a + 1.0) - (a - 1.0) * cos - alps_a2) * ia0;
                    self.b0 = ((a + 1.0) + (a - 1.0) * cos + alps_a2) * a * ia0;
                    self.b1 = -2.0 * ((a - 1.0) + (a + 1.0) * cos) * a * ia0;
                    self.b2 = ((a + 1.0) + (a - 1.0) * cos - alps_a2) * a * ia0;
                }
            }
            kind => {
                let alp = sin * sinh(0.34657359027997264 * band * omg / sin);
                match kind {
                    FilterKind::Peak => {
                        let a = 10f64.powf(gain * 0.025);
                        let (alp_a, alp_ia) = (alp * a, alp / a);
                        let ia0 = 1.0 / (1.0 + alp_ia);
                        self.a1 = -2.0 * cos * ia0;
                        self.b1 = self.a1;
                        self.a2 = (1.0 - alp_ia) * ia0;
                        self.b0 = (1.0 + alp_a) * ia0;
                        self.b2 = (1.0 - alp_a) * ia0;
                    }
                    _ => {
                        let ia0 = 1.0 / (1.0 + alp);
                        match kind {
                            FilterKind::Low => {
                                self.a1 = -2.0 * cos * ia0;
                                self.a2 = (1.0 - alp) * ia0;
                                self.b1 = (1.0 - cos) * ia0;
                                self.b0 = self.b1 * 0.5;
                                self.b2 = self.b0;
                            }
                            FilterKind::High => {
                                self.a1 = -2.0 * cos * ia0;
                                self.a2 = (1.0 - alp) * ia0;
                                self.b1 = -(1.0 + cos) * ia0;
                                self.b0 = -self.b1 * 0.5;
                                self.b2 = self.b0;
                            }
                            FilterKind::Band => {
                                self.a1 = -2.0 * cos * ia0;
                                self.a2 = (1.0 - alp) * ia0;
                                self.b1 = 0.0;
                                self.b0 = alp * ia0;
                                self.b2 = -self.b0;
                            }
                            FilterKind::Notch => {
                                self.a1 = -2.0 * cos * ia0;
                                self.a2 = (1.0 - alp) * ia0;
                                // SiON assigns b1 = a1 and then overwrites it.
                                self.b1 = -(1.0 + cos) * ia0;
                                self.b0 = 1.0;
                                self.b2 = 1.0;
                            }
                            _ => {
                                self.a1 = -2.0 * cos * ia0;
                                self.b1 = self.a1;
                                self.a2 = (1.0 - alp) * ia0;
                                self.b0 = self.a2;
                                self.b2 = 1.0;
                            }
                        }
                    }
                }
            }
        }
    }
}

impl Effector for Filter {
    fn mml(&mut self, a: &[f64; 16]) {
        let (df, dg) = match self.kind {
            FilterKind::Low => (800.0, 6.0),
            FilterKind::High | FilterKind::HighBoost => (5500.0, 6.0),
            _ => (3000.0, 6.0),
        };
        self.set(or(a[0], df), or(a[1], 1.0), or(a[2], dg));
    }

    fn prepare(&mut self) -> i32 {
        self.in1 = [0.0; 2];
        self.in2 = [0.0; 2];
        self.out1 = [0.0; 2];
        self.out2 = [0.0; 2];
        2
    }

    fn process(&mut self, channels: i32, buf: &mut [f64], start: usize, len: usize) -> i32 {
        for ch in 0..2 {
            if self.out1[ch] < THRESHOLD {
                self.out1[ch] = 0.0;
                self.out2[ch] = 0.0;
            }
        }
        let (mut i, e) = (start << 1, (start + len) << 1);
        while i < e {
            let mut step = |me: &mut Filter, ch: usize, input: f64| -> f64 {
                let out = (me.b0 * input + me.b1 * me.in1[ch] + me.b2 * me.in2[ch] - me.a1 * me.out1[ch] - me.a2 * me.out2[ch]).clamp(-1.0, 1.0);
                me.in2[ch] = me.in1[ch];
                me.in1[ch] = input;
                me.out2[ch] = me.out1[ch];
                me.out1[ch] = out;
                out
            };
            if channels == 2 {
                buf[i] = step(self, 0, buf[i]);
                buf[i + 1] = step(self, 1, buf[i + 1]);
            } else {
                let o = step(self, 0, buf[i]);
                buf[i] = o;
                buf[i + 1] = o;
            }
            i += 2;
        }
        channels
    }
}


// vowel (formant) filter
struct Formant {
    ab1: f64,
    a2: f64,
    b0: f64,
    b2: f64,
}

struct Vowel {
    table: Arc<Table>,
    alpha: Arc<Vec<Vec<f64>>>,
    cos: Arc<Vec<f64>>,
    gain_table: Arc<Vec<f64>>,
    formant: [Formant; 6],
    output_level: f64,
    taps: [[f64; 4]; 6],
}

const IBAND: [f64; 8] = [0.25, 0.5, 0.75, 1.0, 1.5, 2.0, 3.0, 4.0];

fn calc_freq_index(frequency: f64) -> usize {
    let i = to_i32((frequency.ln() * 1.4426950408889633 - 5.643856189774724) * 32.0);
    i.clamp(0, 1023) as usize
}

impl Vowel {
    fn new(table: Arc<Table>) -> Vowel {
        let mut alpha = vec![vec![0.0; 1024]; 8];
        for (iband, row) in alpha.iter_mut().enumerate() {
            let band = IBAND[iband];
            let mut freq = 50.0f64;
            for slot in row.iter_mut() {
                let omg = freq * 0.00014247585730565955;
                let sin = omg.sin();
                let angh = 0.34657359027997264 * band * omg / sin;
                *slot = sin * (angh.exp() - (-angh).exp()) * 0.5;
                freq *= 1.0218971486541166;
            }
        }
        let mut cos = vec![0.0; 1024];
        let mut freq = 50.0f64;
        for slot in cos.iter_mut() {
            *slot = (freq * 0.00014247585730565955).cos();
            freq *= 1.0218971486541166;
        }
        let gain_table: Vec<f64> = (0..128).map(|g| 10f64.powf(f64::from(g - 32) * 0.025)).collect();
        let f = || Formant { ab1: 0.0, a2: 0.0, b0: 1.0, b2: 0.0 };
        let mut v = Vowel { table, alpha: Arc::new(alpha), cos: Arc::new(cos), gain_table: Arc::new(gain_table), formant: [f(), f(), f(), f(), f(), f()], output_level: 1.0, taps: [[0.0; 4]; 6] };
        v.set_band([(800.0, 36), (1300.0, 24), (2200.0, 12), (3500.0, 9), (4500.0, 6), (5500.0, 3)], 3);
        v
    }

    fn update(&mut self, n: usize, ifreq: usize, iband: usize, gain: i32) {
        let gain = (gain + 32).clamp(0, 127) as usize;
        let alp = self.alpha[iband][ifreq];
        let a = self.gain_table[gain];
        let (alp_a, alp_ia) = (alp * a, alp / a);
        let ia0 = 1.0 / (1.0 + alp_ia);
        let f = &mut self.formant[n];
        f.ab1 = -2.0 * self.cos[ifreq] * ia0;
        f.a2 = (1.0 - alp_ia) * ia0;
        f.b0 = (1.0 + alp_a) * ia0;
        f.b2 = (1.0 - alp_a) * ia0;
    }

    fn set_band(&mut self, bands: [(f64, i32); 6], bandwidth: usize) {
        for (n, (freq, gain)) in bands.into_iter().enumerate() {
            self.update(n, calc_freq_index(freq), bandwidth, gain);
        }
    }
}

impl Effector for Vowel {
    fn mml(&mut self, a: &[f64; 16]) {
        self.output_level = if a[0].is_nan() { 1.0 } else { a[0] * 0.01 };
        let pair = |i: usize, f: f64, g: f64| (or(a[i], f), to_i32(or(a[i + 1], g)));
        self.set_band([pair(1, 800.0, 30.0), pair(3, 1300.0, 24.0), pair(5, 2200.0, 12.0), pair(7, 3500.0, 9.0), pair(9, 4500.0, 6.0), pair(11, 5500.0, 6.0)], 3);
        let _ = &self.table;
    }

    fn prepare(&mut self) -> i32 {
        self.taps = [[0.0; 4]; 6];
        1
    }

    fn process(&mut self, _channels: i32, buf: &mut [f64], start: usize, len: usize) -> i32 {
        let (mut i, e) = (start << 1, (start + len) << 1);
        while i < e {
            let mut input = buf[i];
            let mut output = input;
            for n in 0..6 {
                let f = &self.formant[n];
                let t = &mut self.taps[n];
                // taps: in0, in1, out0, out1
                output = f.b0 * input + f.ab1 * t[0] + f.b2 * t[1] - f.ab1 * t[2] - f.a2 * t[3];
                t[1] = t[0];
                t[0] = input;
                t[3] = t[2];
                input = output;
                t[2] = input;
            }
            output *= self.output_level;
            let output = output.clamp(-1.0, 1.0);
            buf[i] = output;
            buf[i + 1] = output;
            i += 2;
        }
        1
    }
}

// controlled filters (nlf, nhf)
struct CtrlFilter {
    high: bool,
    ctx: Ctx,
    ptr_cut: Option<(Arc<EnvTable>, usize)>,
    ptr_res: Option<(Arc<EnvTable>, usize)>,
    cut_index: i32,
    res: f64,
    lfo_step: i32,
    lfo_residue: i32,
    p0l: f64,
    p1l: f64,
    p0r: f64,
    p1r: f64,
}

impl CtrlFilter {
    fn new(high: bool, ctx: Ctx) -> CtrlFilter {
        let mut f = CtrlFilter { high, ctx, ptr_cut: None, ptr_res: None, cut_index: 128, res: 0.0, lfo_step: 0, lfo_residue: 0, p0l: 0.0, p1l: 0.0, p0r: 0.0, p1r: 0.0 };
        f.set(255, 255, 20.0);
        // control(1, 0)
        f.lfo_step = 2048;
        f.lfo_residue = 4096;
        f.cut_index = 128;
        f.res = 0.0;
        f
    }

    fn set(&mut self, cut: i32, res: i32, fps: f64) {
        let env = |n: i32| -> Option<(Arc<EnvTable>, usize)> { if (0..255).contains(&n) { self.ctx.envelopes[n as usize].clone().map(|t| (t, 0)) } else { None } };
        self.ptr_cut = env(cut);
        self.ptr_res = env(res);
        self.cut_index = self.ptr_cut.as_ref().map_or(128, |(t, p)| t.values[*p]);
        self.res = self.ptr_res.as_ref().map_or(0.0, |(t, p)| f64::from(t.values[*p]) * 0.007751937984496124);
        self.lfo_step = to_i32(44100.0 / fps);
        if self.lfo_step <= 44 {
            self.lfo_step = 44;
        }
        self.lfo_residue = self.lfo_step << 1;
    }

    fn lfo(&mut self, buf: &mut [f64], start: usize, len: usize) {
        let idx = self.cut_index.clamp(0, 128) as usize;
        let cut = self.ctx.table.filter_cutoff_table[idx];
        let fb = self.res * self.ctx.table.filter_feedback_table[idx];
        let (mut i, e) = (start, start + len);
        while i < e {
            let n = buf[i];
            self.p0l += cut * (n - self.p0l + fb * (self.p0l - self.p1l));
            self.p1l += cut * (self.p0l - self.p1l);
            buf[i] = if self.high { n - self.p0l } else { self.p1l };
            i += 1;
            let n = buf[i];
            self.p0r += cut * (n - self.p0r + fb * (self.p0r - self.p1r));
            self.p1r += cut * (self.p0r - self.p1r);
            buf[i] = if self.high { n - self.p0r } else { self.p1r };
            i += 1;
        }
    }
}

impl Effector for CtrlFilter {
    fn mml(&mut self, a: &[f64; 16]) {
        self.set(if a[0].is_nan() { 255 } else { to_i32(a[0]) }, if a[1].is_nan() { 255 } else { to_i32(a[1]) }, or(a[2], 20.0));
    }

    fn prepare(&mut self) -> i32 {
        self.lfo_residue = 0;
        self.p0l = 0.0;
        self.p1l = 0.0;
        self.p0r = 0.0;
        self.p1r = 0.0;
        2
    }

    fn process(&mut self, channels: i32, buf: &mut [f64], start: usize, len: usize) -> i32 {
        let mut istep = self.lfo_residue;
        let mut i = (start << 1) as i32;
        let imax = ((start + len) << 1) as i32;
        while i < imax - istep {
            self.lfo(buf, i as usize, istep as usize);
            if let Some((t, p)) = self.ptr_cut.as_mut() {
                *p = t.next(*p);
                self.cut_index = t.values[*p];
            }
            if let Some((t, p)) = self.ptr_res.as_mut() {
                *p = t.next(*p);
                self.res = f64::from(t.values[*p]) * 0.007751937984496124;
            }
            i += istep;
            istep = self.lfo_step << 1;
        }
        self.lfo(buf, i as usize, (imax - i) as usize);
        self.lfo_residue = istep - (imax - i);
        channels
    }
}

/// `SiEffectModule.getInstance`: an effector by its name in MML.
fn instance(name: &str, ctx: &Ctx) -> Option<Box<dyn Effector>> {
    Some(match name {
        "ws" => Box::new(WaveShaper::new()),
        "eq" => Box::new(Equaliser::new()),
        "delay" => Box::new(StereoDelay::new()),
        "reverb" => Box::new(StereoReverb::new()),
        "chorus" => Box::new(StereoChorus::new()),
        "autopan" => Box::new(AutoPan::new()),
        "ds" => Box::new(DownSampler::new()),
        "speaker" => Box::new(Speaker::new()),
        "comp" => Box::new(Compressor::new()),
        "dist" => Box::new(Distortion::new()),
        "stereo" => Box::new(StereoExpander::new()),
        "lf" => Box::new(Filter::new(FilterKind::Low)),
        "hf" => Box::new(Filter::new(FilterKind::High)),
        "bf" => Box::new(Filter::new(FilterKind::Band)),
        "nf" => Box::new(Filter::new(FilterKind::Notch)),
        "pf" => Box::new(Filter::new(FilterKind::Peak)),
        "af" => Box::new(Filter::new(FilterKind::All)),
        "lb" => Box::new(Filter::new(FilterKind::LowBoost)),
        "hb" => Box::new(Filter::new(FilterKind::HighBoost)),
        "vowel" => Box::new(Vowel::new(ctx.table.clone())),
        "nlf" => Box::new(CtrlFilter::new(false, ctx.clone())),
        "nhf" => Box::new(CtrlFilter::new(true, ctx.clone())),
        _ => return None,
    })
}

/// `SiEffectStream`: a chain and where it sends.
pub struct EffectStream {
    ctx: Ctx,
    chain: Vec<Box<dyn Effector>>,
    channels: i32,
    pan: i32,
    has_effect_send: bool,
    volumes: [f64; STREAM_SEND_SIZE],
}

impl EffectStream {
    fn new(ctx: Ctx) -> EffectStream {
        let mut s = EffectStream { ctx, chain: vec![], channels: 2, pan: 64, has_effect_send: false, volumes: [0.0; STREAM_SEND_SIZE] };
        s.initialize();
        s
    }

    fn initialize(&mut self) {
        self.chain.clear();
        self.volumes = [0.0; STREAM_SEND_SIZE];
        self.volumes[0] = 1.0;
        self.pan = 64;
        self.has_effect_send = false;
    }

    fn set_stream_send(&mut self, stream: usize, volume: f64) {
        self.volumes[stream] = volume;
        if stream == 0 {
            return;
        }
        self.has_effect_send = self.volumes[1..].iter().any(|&v| v > 0.0);
    }

    fn output_directly(&self) -> bool {
        !self.has_effect_send && self.volumes[0] == 1.0 && self.pan == 64
    }

    /// `parseMML`: the effectors of `#EFFECTn{mml}postfix`, and its pan and volumes.
    fn parse_mml(&mut self, slot: usize, mml: &str, postfix: &str) {
        self.initialize();
        let c: Vec<char> = mml.chars().collect();
        let mut args = [f64::NAN; 16];
        let mut argc = 0usize;
        let mut cmd = String::new();
        let number = |s: &[char]| -> f64 { s.iter().collect::<String>().parse::<f64>().unwrap_or(f64::NAN) };
        // ([a-zA-Z_]+|,)\s*([.\-\d]+)?
        let scan = |c: &[char], pos: &mut usize, names: &[&str]| -> Option<(String, f64)> {
            while *pos < c.len() {
                let start = *pos;
                let word: Option<String> = if c[start] == ',' {
                    *pos += 1;
                    Some(",".into())
                } else if let Some(n) = names.iter().filter(|n| c[start..].iter().take(n.chars().count()).copied().eq(n.chars())).max_by_key(|n| n.len()) {
                    *pos += n.chars().count();
                    Some((*n).into())
                } else if names.is_empty() && (c[start].is_ascii_alphabetic() || c[start] == '_') {
                    while *pos < c.len() && (c[*pos].is_ascii_alphabetic() || c[*pos] == '_') {
                        *pos += 1;
                    }
                    Some(c[start..*pos].iter().collect())
                } else {
                    None
                };
                let Some(w) = word else {
                    *pos += 1;
                    continue;
                };
                let mut q = *pos;
                while q < c.len() && c[q].is_whitespace() {
                    q += 1;
                }
                let ds = q;
                while q < c.len() && (c[q].is_ascii_digit() || c[q] == '.' || c[q] == '-') {
                    q += 1;
                }
                if q > ds {
                    *pos = q;
                    return Some((w, number(&c[ds..q])));
                }
                return Some((w, f64::NAN));
            }
            None
        };
        let ctx = self.ctx.clone();
        let connect = |chain: &mut Vec<Box<dyn Effector>>, cmd: &str, argc: usize, args: &[f64; 16]| {
            if argc == 0 {
                return;
            }
            if let Some(mut e) = instance(cmd, &ctx) {
                e.mml(args);
                chain.push(e);
            }
        };
        let mut pos = 0;
        while let Some((w, n)) = scan(&c, &mut pos, &[]) {
            if w == "," {
                if argc < 16 {
                    args[argc] = n;
                    argc += 1;
                }
            } else {
                connect(&mut self.chain, &cmd, argc, &args);
                args = [f64::NAN; 16];
                cmd = w;
                args[0] = n;
                argc = 1;
            }
        }
        connect(&mut self.chain, &cmd, argc, &args);
        // the postfix: p, @p, @v
        let c: Vec<char> = postfix.chars().collect();
        args = [f64::NAN; 16];
        argc = 0;
        cmd.clear();
        let mut pos = 0;
        while let Some((w, n)) = scan(&c, &mut pos, &["@p", "@v", "p"]) {
            if w == "," {
                if argc < 16 {
                    args[argc] = n;
                    argc += 1;
                }
            } else {
                self.set_volume(slot, &cmd, argc, &args);
                args = [f64::NAN; 16];
                cmd = w;
                args[0] = n;
                argc = 1;
            }
        }
        self.set_volume(slot, &cmd, argc, &args);
    }

    fn set_volume(&mut self, slot: usize, cmd: &str, mut argc: usize, args: &[f64; 16]) {
        if argc == 0 {
            return;
        }
        let int = |x: f64| to_i32(x);
        match cmd {
            "p" => self.pan = (((int(args[0])) << 4) - 64 + 64).clamp(0, 128),
            "@p" => self.pan = (int(args[0]) + 64).clamp(0, 128),
            "@v" => {
                let v = f64::from(int(args[0])) * 0.0078125;
                self.set_stream_send(0, v.clamp(0.0, 1.0));
                if argc + slot >= STREAM_SEND_SIZE {
                    argc = STREAM_SEND_SIZE - slot - 1;
                }
                for i in 1..argc {
                    let v = f64::from(int(args[i])) * 0.0078125;
                    self.set_stream_send(i + slot, v.clamp(0.0, 1.0));
                }
            }
            _ => {}
        }
    }

    /// `prepareProcess`: the channels the chain starts with (0 if it has no effectors).
    fn prepare(&mut self) -> i32 {
        if self.chain.is_empty() {
            return 0;
        }
        self.channels = self.chain[0].prepare();
        for e in self.chain.iter_mut().skip(1) {
            e.prepare();
        }
        self.channels
    }

    fn run(&mut self, buf: &mut [f64], frames: usize) {
        let mut channels = self.channels;
        for e in self.chain.iter_mut() {
            channels = e.process(channels, buf, 0, frames);
        }
    }
}

/// `SiEffectModule`: the master's chain and the sends' (slots 1 to 7).
pub struct EffectModule {
    ctx: Ctx,
    streams: Vec<Option<EffectStream>>,
}

impl EffectModule {
    pub fn new(ctx: Ctx) -> EffectModule {
        let mut streams: Vec<Option<EffectStream>> = (0..STREAM_SEND_SIZE).map(|_| None).collect();
        streams[0] = Some(EffectStream::new(ctx.clone()));
        EffectModule { ctx, streams }
    }

    /// `parseMML` for `#EFFECTn{...}postfix`.
    pub fn parse_mml(&mut self, slot: usize, mml: &str, postfix: &str) {
        if slot >= STREAM_SEND_SIZE {
            return;
        }
        let ctx = self.ctx.clone();
        let s = self.streams[slot].get_or_insert_with(|| EffectStream::new(ctx));
        s.parse_mml(slot, mml, postfix);
    }

    /// `_prepareProcess`: the sends that have effectors get their streams.
    pub fn prepare_process(&mut self, module: &mut Module) {
        for slot in 1..STREAM_SEND_SIZE {
            module.slots[slot] = None;
            module.slot_channels[slot] = 2;
            if let Some(s) = self.streams[slot].as_mut() {
                let channels = s.prepare();
                if channels > 0 {
                    module.slots[slot] = Some(vec![0.0; module.buffer_length * 2]);
                    module.slot_channels[slot] = channels;
                }
            }
        }
        module.slot_channels[0] = 2;
        if let Some(m) = self.streams[0].as_mut() {
            let channels = m.prepare();
            if channels > 0 {
                module.slot_channels[0] = channels;
            }
        }
    }

    /// `_endProcess`: every send's chain, mixed into the output, then the master's.
    pub fn end_process(&mut self, module: &mut Module) {
        let frames = module.buffer_length;
        for slot in 1..STREAM_SEND_SIZE {
            let Some(s) = self.streams[slot].as_mut() else { continue };
            let Some(mut buf) = module.slots[slot].take() else { continue };
            s.run(&mut buf, frames);
            if s.output_directly() {
                if let Some(out) = module.slots[0].as_mut() {
                    for (o, b) in out.iter_mut().zip(&buf) {
                        *o += b;
                    }
                }
            } else {
                for dest in 0..STREAM_SEND_SIZE {
                    if s.has_effect_send && s.volumes[dest] > 0.0 || (!s.has_effect_send && dest == 0) {
                        let vol = s.volumes[dest];
                        let vl = module.table.pan_table[(128 - s.pan).clamp(0, 128) as usize] * vol;
                        let vr = module.table.pan_table[s.pan.clamp(0, 128) as usize] * vol;
                        if dest != slot {
                            let mono = module.slot_channels[dest] == 1;
                            if let Some(out) = module.slots[dest].as_mut() {
                                for k in 0..frames {
                                    if mono {
                                        let n = (buf[k * 2] + buf[k * 2 + 1]) * vol * 0.5;
                                        out[k * 2] += n;
                                        out[k * 2 + 1] += n;
                                    } else {
                                        out[k * 2] += buf[k * 2] * vl;
                                        out[k * 2 + 1] += buf[k * 2 + 1] * vr;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            module.slots[slot] = Some(buf);
        }
        if let Some(m) = self.streams[0].as_mut() {
            if let Some(out) = module.slots[0].as_mut() {
                m.run(out, frames);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> Ctx {
        let mut envelopes: Vec<Option<Arc<EnvTable>>> = vec![None; 512];
        envelopes[3] = Some(Arc::new(EnvTable::new(vec![0, 128], None)));
        Ctx { table: Arc::new(Table::new(44100)), envelopes: Arc::new(envelopes) }
    }

    fn run(mml: &str, input: &[f64]) -> Vec<f64> {
        let mut s = EffectStream::new(ctx());
        s.parse_mml(0, mml, "");
        s.prepare();
        let mut buf = input.to_vec();
        s.run(&mut buf, input.len() / 2);
        buf
    }

    fn sine(n: usize, hz: f64, amp: f64) -> Vec<f64> {
        (0..n).flat_map(|i| {
            let v = amp * (i as f64 * hz * std::f64::consts::TAU / 44100.0).sin();
            [v, v]
        }).collect()
    }

    fn energy(v: &[f64]) -> f64 {
        v.iter().map(|x| x * x).sum()
    }

    #[test]
    fn the_mml_makes_a_chain_of_effectors() {
        let mut s = EffectStream::new(ctx());
        s.parse_mml(1, "ws95lf4000,2delay300,50", "@v64,32p2");
        assert_eq!(s.chain.len(), 3);
        assert_eq!(s.volumes[0], 0.5);
        assert_eq!(s.volumes[2], 0.25);
        assert_eq!(s.pan, 64 + (2 << 4) - 64);
        assert!(instance("nope", &ctx()).is_none());
    }

    #[test]
    fn a_low_pass_filter_takes_out_the_highs_and_a_high_pass_the_lows() {
        let low = sine(4000, 200.0, 0.5);
        let high = sine(4000, 8000.0, 0.5);
        let lf_low = energy(&run("lf1000", &low)[2000..]);
        let lf_high = energy(&run("lf1000", &high)[2000..]);
        assert!(lf_low > lf_high * 20.0, "{lf_low} {lf_high}");
        let hf_low = energy(&run("hf3000", &low)[2000..]);
        let hf_high = energy(&run("hf3000", &high)[2000..]);
        assert!(hf_high > hf_low * 20.0, "{hf_high} {hf_low}");
    }

    #[test]
    fn a_delay_echoes_and_a_reverb_rings_on() {
        let mut input = vec![0.0; 44100 * 2];
        input[0] = 1.0;
        input[1] = 1.0;
        // 100 ms is 4410 frames; wet 50%.
        let out = run("delay100,0,0,50", &input);
        assert!((out[0] - 0.5).abs() < 1e-9);
        assert!((out[4410 * 2] - 0.5).abs() < 1e-9, "{}", out[4410 * 2]);
        assert!(out[4000 * 2].abs() < 1e-9);
        let verb = run("reverb", &input);
        assert!(verb[100..].iter().any(|x| x.abs() > 1e-6));
    }

    #[test]
    fn a_waveshaper_and_a_compressor_hold_the_level_down() {
        let loud = sine(4000, 440.0, 0.9);
        let ws = run("ws90", &loud);
        assert!(ws.iter().all(|x| x.abs() <= 1.0 + 1e-9));
        let comp = run("comp50,20,5,50,-12,100", &loud);
        assert!(energy(&comp[4000..]) < energy(&loud[4000..]));
        // A stereo expander at width 0 makes mono.
        let mut input = vec![0.0; 8];
        input[0] = 1.0;
        let m = run("stereo0,0,0", &input);
        assert!((m[0] - m[1]).abs() < 1e-12);
    }

    #[test]
    fn a_down_sampler_steps_the_signal() {
        let input: Vec<f64> = (0..32).map(|i| i as f64 / 64.0).collect();
        let out = run("ds1,16,2", &input);
        // Every other frame repeats the one before.
        assert_eq!(out[0], out[2]);
        assert_eq!(out[1], out[3]);
        assert!(out[4] != out[2]);
    }

    #[test]
    fn the_vowel_filter_shapes_a_buzz_into_formants() {
        // A saw-like buzz in, formants out: not the same, still audible, and it stays in range.
        let buzz: Vec<f64> = (0..8000).flat_map(|i| { let v = ((i % 100) as f64 / 50.0 - 1.0) * 0.6; [v, v] }).collect();
        let out = run("vowel100,800,30,1300,24", &buzz);
        assert!(energy(&out[4000..]) > 1e-3);
        assert!(out.iter().all(|x| x.abs() <= 1.0));
        let diff: f64 = out.iter().zip(&buzz).map(|(a, b)| (a - b).abs()).sum();
        assert!(diff > 100.0);
    }

    #[test]
    fn the_controlled_filters_follow_their_tables() {
        let high = sine(8000, 6000.0, 0.5);
        // Table 3 opens the cutoff from 0 to 128 and holds it there: a low-pass lets the highs
        // through only as it opens. With no table the cutoff is full and everything passes.
        let open = energy(&run("nlf", &high)[8000..]);
        let swept = run("nlf3,255,50", &high);
        let closed_start = energy(&swept[2000..4000]);
        let late = energy(&swept[12000..]);
        assert!(open > 1.0);
        assert!(late > closed_start, "{late} {closed_start}");
        // The high-pass is the other way round.
        let low = sine(8000, 100.0, 0.5);
        assert!(energy(&run("nhf", &low)[8000..]) < energy(&low[8000..]));
    }
}
