// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! V2 (Tammo "kb" Hinrichs, farbrausch, 2000-2008): the soft synth of
//! farbrausch's intros and of many keygens, and its songs, V2M files: a
//! MIDI sequence of 16 channels and the patches it plays. Each voice is
//! three oscillators (a box-filtered triangle/saw and pulse, a sine,
//! filtered noise, FM from the oscillators before it, or an aux bus),
//! two state-variable or Moog filters, a distortion, two envelopes and
//! two LFOs, all of it modulated by a matrix; each channel a compressor,
//! bass boost, distortion and chorus/flanger, with sends to a reverb, a
//! modulated stereo delay and two aux buses; the mix through low and
//! high cuts and a compressor. Channel 16 can speak: a formant speech
//! synth ("Ronan") filters its voices with a song's lyrics. Everything is
//! made in 128-sample frames at 44.1 kHz in floating point.
//!
//! Ported from V2's source, which its author placed in the public domain
//! (github.com/farbrausch/fr_public, `v2/`: "I hereby place this code ...
//! in the public domain"): its synth core (Fabian Giesen's C++ rendition
//! of the original assembly), its speech synth and its V2M version
//! converter, every float operation in the same order and precision, so
//! the samples it makes are its own. The V2M player was followed for what
//! it does (its file's header also names the Artistic License, which the
//! directory's licence file replaces). The tables kept as data are V2's
//! own: its patch parameters' versions and defaults, and its phonemes.
//!
//! Where the original leaves something to the platform this follows V2's
//! own, Windows on x86: a float too large for an integer becomes
//! 0x80000000, and the LFOs' random seeds come from the Microsoft C
//! runtime's rand() (as the player starts the synth twice). The speech
//! synth's x87 pow and exp are the C library's.
//!
//! Its mix is shared out among the channels for the Tracker: each
//! channel's own share of the reverb, delay, filters and the mix's
//! compressor (all linear, the compressor's gain being the mix's), and
//! what's left of the mix (rounding) in proportion. Frames played only to
//! seek run the song, the voices' settings, envelopes and LFOs (which
//! decide which voices play), not the sound.

// V2's constants are written as its source writes them.
#![allow(clippy::excessive_precision)]

use std::sync::Arc;

use crate::replay::{Bufs, Driver, Patch, PatchBytes, Tune};

/// The rate V2 makes everything at here.
pub const RATE: u32 = 44_100;
/// A frame: V2 works out its voices' settings once a frame.
const FRAME: usize = 128;
const CHANS: usize = 16;
const POLY: usize = 64;
const NVOICE: usize = 59;
const NCHAN: usize = 29;
const NGLOB: usize = 23;

// Natural and synth constants (all f32, as V2 has them).
const FCLOWEST: f32 = 1.220703125e-4;
#[allow(clippy::approx_constant)]
const FCPI_2: f32 = 1.570_796_4;
#[allow(clippy::approx_constant)]
const FCPI: f32 = 3.141_592_7;
const FC1P5PI: f32 = 4.712_389;
#[allow(clippy::approx_constant)]
const FC2PI: f32 = 6.283_185_5;
const FC32BIT: f32 = 2147483648.0;
const FCOSCBASE: f32 = 261.6255653;
const FCSRBASE: f32 = 44100.0;
const FCBOOSTFREQ: f32 = 150.0;
const FCFRAMEBASE: f32 = 128.0;
const FCDCFLT: f32 = 126.0;
const FCCFFRAME: f32 = 11.0;
const FCFMMAX: f32 = 2.0;
const FCATTACKMUL: f32 = -0.09375;
const FCATTACKADD: f32 = 7.0;
const FCSUSMUL: f32 = 0.0019375;
const FCGAIN: f32 = 0.6;
const FCGAINH: f32 = 0.6;
const FCMDLFOMUL: f32 = 1973915.49;
const FCCPDFALLOFF: f32 = 0.9998;
const FCDCOFFSET: f32 = 3.814697265625e-6;

// ---------------------------------------------------------------------
// Helpers.

fn bits2float(u: u32) -> f32 {
    f32::from_bits(u)
}

#[allow(clippy::approx_constant)]
fn fastatan(x: f32) -> f32 {
    let (sign, x) = if x < 0.0 { (-1.0f32, -x) } else { (1.0f32, x) };
    const C: [[f32; 6]; 2] = [[1.0, 0.43157974, 1.0, 0.05831938, 0.76443945, 0.0], [-0.431597974, -1.0, 0.05831938, 1.0, 0.76443945, 1.57079633]];
    let c = &C[usize::from(x >= 1.0)];
    let x2 = x * x;
    let r = (c[1] * x2 + c[0]) * x / ((c[4] * x2 + c[3]) * x2 + c[2]) + c[5];
    r * sign
}

fn fastsin(x: f32) -> f32 {
    let x2 = x * x;
    (((-0.00018542f32 * x2 + 0.0083143) * x2 - 0.16666) * x2 + 1.0) * x
}

/// V2's range-reduced sine (its FM's bug kept: negative values aren't brought back).
fn fastsinrc(x: f32) -> f32 {
    let mut x = x % FC2PI;
    if x > FC1P5PI {
        x -= FC2PI;
    } else if x > FCPI_2 {
        x = FCPI - x;
    }
    fastsin(x)
}

fn calcfreq(x: f32) -> f32 {
    2.0f32.powf((x - 1.0) * 10.0)
}

fn calcfreq2(x: f32) -> f32 {
    2.0f32.powf((x - 1.0) * FCCFFRAME)
}

fn sqr(x: f32) -> f32 {
    x * x
}

fn urandom(seed: &mut u32) -> u32 {
    *seed = seed.wrapping_mul(196314165).wrapping_add(907633515);
    *seed
}

fn frandom(seed: &mut u32) -> f32 {
    let bits = urandom(seed);
    bits2float((bits >> 9) | 0x40000000) - 3.0
}

fn utof23(x: u32) -> f32 {
    bits2float((x >> 9) | 0x3f800000) - 1.0
}

/// x86's float to integer: NaN or out of range gives 0x80000000.
fn x86i(x: f32) -> i32 {
    if x.is_nan() || x >= 2147483648.0 || x < -2147483648.0 {
        i32::MIN
    } else {
        x as i32
    }
}

fn ftou32(v: f32) -> u32 {
    2u32.wrapping_mul(x86i(v * FC32BIT) as u32)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + t * (b - a)
}

fn clampf(x: f32, min: f32, max: f32) -> f32 {
    if x < min {
        min
    } else if x > max {
        max
    } else {
        x
    }
}

/// The Microsoft C runtime's rand(), from its first call.
#[derive(Clone)]
struct Rand(u32);

impl Rand {
    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(214013).wrapping_add(2531011);
        (self.0 >> 16) & 0x7fff
    }
}

/// The rates V2 works out for 44.1 kHz.
struct Inst {
    samplesperms: f32,
    obasefrq: f32,
    linfreq: f32,
    boostcos: f32,
    boostsin: f32,
    dcfilter: f32,
    iframe: f32,
}

impl Inst {
    fn new() -> Inst {
        let sr = RATE as f32;
        let frame = (FCFRAMEBASE * sr / FCSRBASE + 0.5) as i32;
        let boost = (FCBOOSTFREQ * FC2PI) / sr;
        Inst {
            samplesperms: sr / 1000.0,
            obasefrq: (FCOSCBASE * FC32BIT) / sr,
            linfreq: FCSRBASE / sr,
            boostcos: boost.cos(),
            boostsin: boost.sin(),
            dcfilter: 1.0 - FCDCFLT / sr,
            iframe: 1.0 / frame as f32,
        }
    }
}

// ---------------------------------------------------------------------
// Building blocks.

#[derive(Clone, Copy, Default)]
struct Lrc {
    l: f32,
    b: f32,
}

impl Lrc {
    fn step(&mut self, input: f32, freq: f32, reso: f32) -> f32 {
        self.l += freq * self.b;
        let h = input - self.b * reso - self.l;
        self.b += freq * h;
        h
    }

    fn step_2x(&mut self, input: f32, freq: f32, reso: f32) -> f32 {
        let input = input + FCDCOFFSET;
        self.l += freq * self.b - FCDCOFFSET;
        self.b += freq * (input - self.b * reso - self.l);
        self.l += freq * self.b;
        let h = input - self.b * reso - self.l;
        self.b += freq * h;
        h
    }
}

#[derive(Clone, Copy, Default)]
struct Moog {
    b: [f32; 5],
}

impl Moog {
    fn step(&mut self, realin: f32, f: f32, p: f32, q: f32) -> f32 {
        let mut input = realin + FCDCOFFSET;
        input -= q * self.b[4];
        let t1 = self.b[1];
        self.b[1] = (input + self.b[0]) * p - self.b[1] * f;
        let t2 = self.b[2];
        self.b[2] = (t1 + self.b[1]) * p - self.b[2] * f;
        let t3 = self.b[3];
        self.b[3] = (t2 + self.b[2]) * p - self.b[3] * f;
        let mut b4 = (t3 + self.b[3]) * p - self.b[4] * f;
        b4 -= b4 * b4 * b4 * (1.0f32 / 6.0);
        b4 -= FCDCOFFSET;
        self.b[4] = b4 - FCDCOFFSET;
        self.b[0] = realin;
        b4
    }
}

#[derive(Clone, Copy, Default)]
struct Dcf {
    xm1: f32,
    ym1: f32,
}

impl Dcf {
    fn step(&mut self, input: f32, r: f32) -> f32 {
        let y = (FCDCOFFSET + r * self.ym1 - self.xm1 + input) - FCDCOFFSET;
        self.xm1 = input;
        self.ym1 = y;
        y
    }
}

#[derive(Clone)]
struct Delay {
    buf: Vec<f32>,
    pos: usize,
}

impl Delay {
    fn new(len: usize) -> Delay {
        Delay { buf: vec![0.0; len], pos: 0 }
    }
    fn reset(&mut self) {
        self.buf.iter_mut().for_each(|v| *v = 0.0);
        self.pos = 0;
    }
    fn fetch(&self) -> f32 {
        self.buf[self.pos]
    }
    fn feed(&mut self, v: f32) {
        self.buf[self.pos] = v;
        self.pos += 1;
        if self.pos == self.buf.len() {
            self.pos = 0;
        }
    }
}

// ---------------------------------------------------------------------
// The voice.

const OSC_SEEDS: [u32; 3] = [0xdeadbeef, 0xbaadf00d, 0xd3adc0de];

#[derive(Clone, Copy, Default)]
struct Osc {
    mode: i32,
    ring: bool,
    cnt: u32,
    freq: i32,
    brpt: u32,
    nffrq: f32,
    nfres: f32,
    nseed: u32,
    gain: f32,
    nf: Lrc,
    note: f32,
    pitch: f32,
}

/// The aux buses as an oscillator reads them.
struct AuxIn<'a> {
    a: &'a [f32],
    b: &'a [f32],
}

impl Osc {
    fn init(&mut self, idx: usize) {
        self.cnt = 0;
        self.nf = Lrc::default();
        self.nseed = OSC_SEEDS[idx];
    }

    fn chg_pitch(&mut self, inst: &Inst) {
        self.nffrq = inst.linfreq * calcfreq((self.pitch + 64.0) / 128.0);
        self.freq = x86i(inst.obasefrq * 2.0f32.powf((self.pitch + self.note - 60.0) / 12.0));
    }

    fn set(&mut self, p: &[f32], inst: &Inst) {
        self.mode = p[0] as i32;
        self.ring = (p[1] as i32) & 1 != 0;
        self.pitch = (p[2] - 64.0) + (p[3] - 64.0) / 128.0;
        self.chg_pitch(inst);
        self.gain = p[5] / 128.0;
        let col = p[4] / 128.0;
        self.brpt = ftou32(col);
        self.nfres = 1.0 - col.sqrt();
    }

    fn output(&self, d: &mut f32, x: f32) {
        if self.ring {
            *d *= x;
        } else {
            *d += x;
        }
    }

    fn osm_init(&self) -> u32 {
        if self.cnt.wrapping_sub(self.freq as u32) < self.brpt {
            3
        } else {
            0
        }
    }

    fn osm_tick(&mut self, state: &mut u32) -> u32 {
        *state = ((*state << 1) | u32::from(self.cnt < self.brpt)) & 3;
        let code = *state | if self.cnt < self.freq as u32 { 4 } else { 0 };
        self.cnt = self.cnt.wrapping_add(self.freq as u32);
        code
    }

    fn render(&mut self, dest: &mut [f32], aux: &AuxIn) {
        match self.mode & 7 {
            1 => self.render_trisaw(dest),
            2 => self.render_pulse(dest),
            3 => self.render_sin(dest),
            4 => self.render_noise(dest),
            5 => self.render_fmsin(dest),
            6 => self.render_aux(dest, aux.a),
            7 => self.render_aux(dest, aux.b),
            _ => {}
        }
    }

    fn render_trisaw(&mut self, dest: &mut [f32]) {
        let f = utof23(self.freq as u32);
        let omf = 1.0 - f;
        let rcpf = 1.0 / f;
        let col = utof23(self.brpt);
        let gain = self.gain;
        let c1 = gain / col;
        let c2 = -gain / (1.0 - col);
        let mut state = self.osm_init();
        for d in dest.iter_mut() {
            let p = utof23(self.cnt) - col;
            let y = match self.osm_tick(&mut state) {
                3 => c1 * (p + p - f),
                0 => c2 * (p + p - f),
                2 => rcpf * (c2 * sqr(p) - c1 * sqr(p - f)),
                5 => -rcpf * (gain + c2 * sqr(p + omf) - c1 * sqr(p)),
                7 => -rcpf * (gain + c1 * omf * (p + p + omf)),
                4 => -rcpf * (gain + c2 * omf * (p + p + omf)),
                _ => 0.0,
            };
            self.output(d, y + gain);
        }
    }

    fn render_pulse(&mut self, dest: &mut [f32]) {
        let f = utof23(self.freq as u32);
        let gain = self.gain;
        let gdf = gain / f;
        let col = utof23(self.brpt);
        let cc121 = gdf * 2.0 * (col - 1.0) + gain;
        let cc212 = gdf * 2.0 * col - gain;
        let mut state = self.osm_init();
        for d in dest.iter_mut() {
            let p = utof23(self.cnt);
            let out = match self.osm_tick(&mut state) {
                3 => gain,
                0 => -gain,
                2 => gdf * 2.0 * (col - p) + gain,
                5 => gdf * 2.0 * p - gain,
                7 => cc121,
                4 => cc212,
                _ => 0.0,
            };
            self.output(d, out);
        }
    }

    fn render_sin(&mut self, dest: &mut [f32]) {
        for d in dest.iter_mut() {
            let mut phase = self.cnt.wrapping_add(0x40000000);
            self.cnt = self.cnt.wrapping_add(self.freq as u32);
            if phase & 0x80000000 != 0 {
                phase = !phase;
            }
            let t = bits2float((phase >> 8) | 0x3f800000);
            let t = t * FCPI - FC1P5PI;
            self.output(d, self.gain * fastsin(t));
        }
    }

    fn render_noise(&mut self, dest: &mut [f32]) {
        let mut flt = self.nf;
        let mut seed = self.nseed;
        for d in dest.iter_mut() {
            let n = frandom(&mut seed);
            let h = flt.step(n, self.nffrq, self.nfres);
            let x = self.nfres * (flt.l + h) + flt.b;
            self.output(d, self.gain * x);
        }
        // V2 keeps the seed but not the filter's state.
        self.nseed = seed;
    }

    fn render_fmsin(&mut self, dest: &mut [f32]) {
        for d in dest.iter_mut() {
            let m = *d * FCFMMAX;
            let t = (utof23(self.cnt) + m) * FC2PI;
            self.cnt = self.cnt.wrapping_add(self.freq as u32);
            let out = self.gain * fastsinrc(t);
            if self.ring {
                *d *= out;
            } else {
                *d = out;
            }
        }
    }

    fn render_aux(&mut self, dest: &mut [f32], src: &[f32]) {
        let g = self.gain * FCGAIN;
        for (i, d) in dest.iter_mut().enumerate() {
            let mut aux = g * (src[2 * i] + src[2 * i + 1]);
            if self.ring {
                aux *= *d;
            }
            *d = aux;
        }
    }
}

const ENV_OFF: u8 = 0;
const ENV_RELEASE: u8 = 1;
const ENV_ATTACK: u8 = 2;
const ENV_DECAY: u8 = 3;
const ENV_SUSTAIN: u8 = 4;

#[derive(Clone, Copy, Default)]
struct Env {
    out: f32,
    state: u8,
    val: f32,
    atd: f32,
    dcf: f32,
    sul: f32,
    suf: f32,
    rf: f32,
    gain: f32,
}

impl Env {
    fn set(&mut self, p: &[f32]) {
        self.atd = 2.0f32.powf(p[0] * FCATTACKMUL + FCATTACKADD);
        self.dcf = 1.0 - calcfreq2(1.0 - p[1] / 128.0);
        self.sul = p[2];
        self.suf = 2.0f32.powf(FCSUSMUL * (p[3] - 64.0));
        self.rf = 1.0 - calcfreq2(1.0 - p[4] / 128.0);
        self.gain = p[5] / 128.0;
    }

    fn tick(&mut self, gate: bool) {
        if self.state <= ENV_RELEASE && gate {
            self.state = ENV_ATTACK;
        } else if self.state >= ENV_ATTACK && !gate {
            self.state = ENV_RELEASE;
        }
        match self.state {
            ENV_OFF => self.val = 0.0,
            ENV_ATTACK => {
                self.val += self.atd;
                if self.val >= 128.0 {
                    self.val = 128.0;
                    self.state = ENV_DECAY;
                }
            }
            ENV_DECAY => {
                self.val *= self.dcf;
                if self.val <= self.sul {
                    self.val = self.sul;
                    self.state = ENV_SUSTAIN;
                }
            }
            ENV_SUSTAIN => {
                self.val *= self.suf;
                if self.val > 128.0 {
                    self.val = 128.0;
                }
            }
            _ => self.val *= self.rf,
        }
        if self.val <= FCLOWEST {
            self.val = 0.0;
            self.state = ENV_OFF;
        }
        self.out = self.val * self.gain;
    }
}

const FLT_MOOGL: i32 = 6;

#[derive(Clone, Copy, Default)]
struct Flt {
    mode: i32,
    cfreq: f32,
    res: f32,
    moogf: f32,
    moogp: f32,
    moogq: f32,
    lrc: Lrc,
    moog: Moog,
}

impl Flt {
    fn init(&mut self) {
        self.lrc = Lrc::default();
        self.moog = Moog::default();
    }

    fn set(&mut self, mode: f32, cutoff: f32, reso: f32, inst: &Inst) {
        self.mode = mode as i32;
        let mut f = calcfreq(cutoff / 128.0) * inst.linfreq;
        let r = reso / 128.0;
        if self.mode < FLT_MOOGL {
            self.res = 1.0 - r;
            self.cfreq = f;
        } else {
            f *= 0.25;
            let t = 1.0 - f;
            self.moogp = f + 0.8 * f * t;
            self.moogf = 1.0 - self.moogp - self.moogp;
            self.moogq = 4.0 * r * (1.0 + 0.5 * t * (1.0 - t + 5.6 * t * t));
        }
    }

    /// Filters `n` samples of `buf` in place, from `at`, every `step`th.
    fn render(&mut self, buf: &mut [f32], at: usize, n: usize, step: usize) {
        let (cfreq, res) = (self.cfreq, self.res);
        match self.mode & 7 {
            1..=5 => {
                let mut flt = self.lrc;
                let mode = self.mode & 7;
                for i in 0..n {
                    let k = at + i * step;
                    let h = flt.step_2x(buf[k], cfreq, res);
                    buf[k] = match mode {
                        1 => flt.l,
                        2 => flt.b,
                        3 => h,
                        4 => flt.l + h,
                        _ => flt.l + flt.b + h,
                    };
                }
                self.lrc = flt;
            }
            6 | 7 => {
                let mut m = self.moog;
                let high = self.mode & 7 == 7;
                for i in 0..n {
                    let k = at + i * step;
                    let input = buf[k];
                    m.step(input, self.moogf, self.moogp, self.moogq);
                    let o = m.step(input, self.moogf, self.moogp, self.moogq);
                    buf[k] = if high { input - o } else { o };
                }
                self.moog = m;
            }
            _ => {}
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Lfo {
    out: f32,
    mode: i32,
    sync: bool,
    eg: bool,
    freq: i32,
    cntr: u32,
    cphase: u32,
    gain: f32,
    dc: f32,
    nseed: u32,
    last: u32,
}

impl Lfo {
    fn init(&mut self, rand: &mut Rand) {
        self.cntr = 0;
        self.last = 0;
        self.nseed = rand.next();
    }

    fn set(&mut self, p: &[f32]) {
        self.mode = p[0] as i32;
        self.sync = p[1] as i32 != 0;
        self.eg = p[2] as i32 != 0;
        self.freq = (0.5 * FC32BIT * calcfreq(p[3] / 128.0)) as i32;
        self.cphase = ftou32(p[4] / 128.0);
        match p[5] as i32 {
            0 => {
                self.gain = p[6];
                self.dc = 0.0;
            }
            1 => {
                self.gain = -p[6];
                self.dc = 0.0;
            }
            2 => {
                self.gain = p[6];
                self.dc = -0.5 * p[6];
            }
            _ => {}
        }
    }

    fn key_on(&mut self) {
        if self.sync {
            self.cntr = self.cphase;
            self.last = !0;
        }
    }

    fn tick(&mut self) {
        let v = match self.mode & 7 {
            1 => utof23((self.cntr << 1) ^ ((self.cntr as i32 >> 31) as u32)),
            2 => utof23((self.cntr as i32 >> 31) as u32),
            3 => fastsinrc(utof23(self.cntr) * FC2PI) * 0.5 + 0.5,
            4 => {
                if self.cntr < self.last {
                    self.nseed = urandom(&mut self.nseed);
                }
                self.last = self.cntr;
                utof23(self.nseed)
            }
            _ => utof23(self.cntr),
        };
        self.out = v * self.gain + self.dc;
        self.cntr = self.cntr.wrapping_add(self.freq as u32);
        if self.cntr < self.freq as u32 && self.eg {
            self.cntr = !0;
        }
    }
}

const DIST_OFF: i32 = 0;
const DIST_OVERDRIVE: i32 = 1;
const DIST_CLIP: i32 = 2;
const DIST_BITCRUSHER: i32 = 3;
const DIST_DECIMATOR: i32 = 4;

#[derive(Clone, Copy, Default)]
struct Dist {
    mode: i32,
    gain1: f32,
    gain2: f32,
    offs: f32,
    crush1: f32,
    crush2: i32,
    crxor: i32,
    dcount: u32,
    dfreq: u32,
    dvall: f32,
    dvalr: f32,
    fltl: Flt,
    fltr: Flt,
}

impl Dist {
    fn init(&mut self) {
        self.dcount = 0;
        self.dvall = 0.0;
        self.dvalr = 0.0;
        self.fltl.init();
        self.fltr.init();
    }

    fn set(&mut self, p: &[f32], inst: &Inst) {
        self.mode = p[0] as i32;
        self.gain1 = 2.0f32.powf((p[1] - 32.0) / 16.0);
        match self.mode {
            DIST_OFF => {}
            DIST_OVERDRIVE => {
                self.gain2 = (p[2] / 128.0) / self.gain1.atan();
                self.offs = self.gain1 * 2.0 * ((p[3] / 128.0) - 0.5);
            }
            DIST_CLIP => {
                self.gain2 = p[2] / 128.0;
                self.offs = self.gain1 * 2.0 * ((p[3] / 128.0) - 0.5);
            }
            DIST_BITCRUSHER => {
                let x = p[2] * 256.0 + 1.0;
                self.crush2 = x as i32;
                self.crush1 = self.gain1 * (32768.0 / x);
                self.crxor = (p[3] as i32) << 9;
            }
            DIST_DECIMATOR => self.dfreq = ftou32(calcfreq(p[2] / 128.0)),
            m => {
                let mode = (m - DIST_DECIMATOR) as f32;
                self.fltl.set(mode, p[2], p[3], inst);
                self.fltr.set(mode, p[2], p[3], inst);
            }
        }
    }

    fn overdrive(&self, x: f32) -> f32 {
        self.gain2 * fastatan(x * self.gain1 + self.offs)
    }
    fn clip(&self, x: f32) -> f32 {
        self.gain2 * clampf(x * self.gain1 + self.offs, -1.0, 1.0)
    }
    fn bitcrusher(&self, x: f32) -> f32 {
        let t = x86i(x * self.crush1);
        let t = t.wrapping_mul(self.crush2).clamp(-0x7fff, 0x7fff) ^ self.crxor;
        t as f32 / 32768.0
    }
    fn decimator_tick(&mut self, l: f32, r: f32) {
        self.dcount = self.dcount.wrapping_add(self.dfreq);
        if self.dcount < self.dfreq {
            self.dvall = l;
            self.dvalr = r;
        }
    }

    /// `n` samples of `buf` from `at` as one stream.
    fn render_mono(&mut self, buf: &mut [f32], at: usize, n: usize) {
        let b = &mut buf[at..at + n];
        match self.mode {
            DIST_OFF => {}
            DIST_OVERDRIVE => b.iter_mut().for_each(|v| *v = self.overdrive(*v)),
            DIST_CLIP => b.iter_mut().for_each(|v| *v = self.clip(*v)),
            DIST_BITCRUSHER => b.iter_mut().for_each(|v| *v = self.bitcrusher(*v)),
            DIST_DECIMATOR => {
                for v in b.iter_mut() {
                    self.decimator_tick(*v, 0.0);
                    *v = self.dvall;
                }
            }
            _ => self.fltl.render(buf, at, n, 1),
        }
    }

    /// `n` interleaved stereo samples.
    fn render_stereo(&mut self, buf: &mut [f32], n: usize) {
        match self.mode {
            DIST_DECIMATOR => {
                for i in 0..n {
                    self.decimator_tick(buf[2 * i], buf[2 * i + 1]);
                    buf[2 * i] = self.dvall;
                    buf[2 * i + 1] = self.dvalr;
                }
            }
            5..=9 => {
                self.fltl.render(buf, 0, n, 2);
                self.fltr.render(buf, 1, n, 2);
            }
            // The rest (the Moog filters among them) as one stream of both sides.
            _ => self.render_mono(buf, 0, n * 2),
        }
    }
}

#[derive(Clone, Copy, Default)]
struct DcFilter {
    fl: Dcf,
    fr: Dcf,
}

impl DcFilter {
    fn render_mono(&mut self, buf: &mut [f32], r: f32) {
        for v in buf.iter_mut() {
            *v = self.fl.step(*v, r);
        }
    }
    fn render_stereo(&mut self, buf: &mut [f32], n: usize, r: f32) {
        for i in 0..n {
            buf[2 * i] = self.fl.step(buf[2 * i], r);
            buf[2 * i + 1] = self.fr.step(buf[2 * i + 1], r);
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Voice {
    note: i32,
    velo: f32,
    gate: bool,
    curvol: f32,
    volramp: f32,
    xpose: f32,
    fmode: i32,
    lvol: f32,
    rvol: f32,
    f1gain: f32,
    f2gain: f32,
    keysync: i32,
    osc: [Osc; 3],
    vcf: [Flt; 2],
    env: [Env; 2],
    lfo: [Lfo; 2],
    dist: Dist,
    dcf: DcFilter,
}

impl Voice {
    fn init(&mut self, rand: &mut Rand) {
        for (i, o) in self.osc.iter_mut().enumerate() {
            o.init(i);
        }
        for f in self.vcf.iter_mut() {
            f.init();
        }
        for e in self.env.iter_mut() {
            e.state = ENV_OFF;
        }
        for l in self.lfo.iter_mut() {
            l.init(rand);
        }
        self.dist.init();
        self.dcf = DcFilter::default();
    }

    fn tick(&mut self, inst: &Inst) {
        for e in self.env.iter_mut() {
            e.tick(self.gate);
        }
        for l in self.lfo.iter_mut() {
            l.tick();
        }
        self.volramp = (self.env[0].out / 128.0 - self.curvol) * inst.iframe;
    }

    fn update_note(&mut self) {
        let n = self.xpose + self.note as f32;
        for o in self.osc.iter_mut() {
            o.note = n;
        }
    }

    fn set(&mut self, p: &[f32; NVOICE], inst: &Inst) {
        self.xpose = p[1] - 64.0;
        self.update_note();
        self.fmode = p[26] as i32;
        self.keysync = p[58] as i32;
        let pan = p[0] / 128.0;
        self.lvol = (1.0 - pan).sqrt();
        self.rvol = pan.sqrt();
        let x = (p[27] - 64.0) / 64.0;
        if x >= 0.0 {
            self.f2gain = 1.0;
            self.f1gain = 1.0 - x;
        } else {
            self.f1gain = 1.0;
            self.f2gain = 1.0 + x;
        }
        for (i, o) in self.osc.iter_mut().enumerate() {
            o.set(&p[2 + 6 * i..8 + 6 * i], inst);
        }
        for (i, e) in self.env.iter_mut().enumerate() {
            e.set(&p[32 + 6 * i..38 + 6 * i]);
        }
        for (i, f) in self.vcf.iter_mut().enumerate() {
            f.set(p[20 + 3 * i], p[21 + 3 * i], p[22 + 3 * i], inst);
        }
        for (i, l) in self.lfo.iter_mut().enumerate() {
            l.set(&p[44 + 7 * i..51 + 7 * i]);
        }
        self.dist.set(&p[28..32], inst);
    }

    fn note_on(&mut self, note: i32, vel: i32, inst: &Inst) {
        self.note = note;
        self.update_note();
        self.velo = vel as f32;
        self.gate = true;
        for e in self.env.iter_mut() {
            e.state = ENV_ATTACK;
        }
        if self.keysync == 2 {
            for e in self.env.iter_mut() {
                e.val = 0.0;
            }
            self.curvol = 0.0;
            for (i, o) in self.osc.iter_mut().enumerate() {
                o.init(i);
            }
            for f in self.vcf.iter_mut() {
                f.init();
            }
            self.dist.init();
        }
        if self.keysync == 1 || self.keysync == 2 {
            for o in self.osc.iter_mut() {
                o.cnt = 0;
            }
        }
        for o in self.osc.iter_mut() {
            o.chg_pitch(inst);
        }
        for l in self.lfo.iter_mut() {
            l.key_on();
        }
        self.dcf = DcFilter::default();
    }

    /// Adds the voice's frame to `dest` (interleaved stereo).
    fn render(&mut self, dest: &mut [f32], n: usize, scratch: &mut [f32], scratch2: &mut [f32], aux: &AuxIn, inst: &Inst) {
        let voice = &mut scratch[..n];
        voice.iter_mut().for_each(|v| *v = 0.0);
        for o in self.osc.iter_mut() {
            o.render(voice, aux);
        }
        match self.fmode {
            0 => self.vcf[0].render(voice, 0, n, 1),
            2 => {
                let voice2 = &mut scratch2[..n];
                voice2.copy_from_slice(voice);
                self.vcf[1].render(voice2, 0, n, 1);
                self.vcf[0].render(voice, 0, n, 1);
                for i in 0..n {
                    voice[i] = voice[i] * self.f1gain + voice2[i] * self.f2gain;
                }
            }
            _ => {
                self.vcf[0].render(voice, 0, n, 1);
                self.vcf[1].render(voice, 0, n, 1);
            }
        }
        self.dist.render_mono(voice, 0, n);
        self.dcf.render_mono(voice, inst.dcfilter);
        let mut cv = self.curvol;
        for i in 0..n {
            let out = voice[i] * cv;
            cv += self.volramp;
            dest[2 * i] += self.lvol * out + FCDCOFFSET;
            dest[2 * i + 1] += self.rvol * out + FCDCOFFSET;
        }
        self.curvol = cv;
    }
}

// ---------------------------------------------------------------------
// Channel and mix effects.

#[derive(Clone, Copy, Default)]
struct Boost {
    enabled: bool,
    a1: f32,
    a2: f32,
    b0: f32,
    b1: f32,
    b2: f32,
    x1: [f32; 2],
    x2: [f32; 2],
    y1: [f32; 2],
    y2: [f32; 2],
}

impl Boost {
    fn set(&mut self, amount: f32, inst: &Inst) {
        self.enabled = amount as i32 != 0;
        if !self.enabled {
            return;
        }
        let a = 2.0f32.powf(amount / 128.0);
        let beta = (2.0 * a).sqrt();
        let bs = beta * inst.boostsin;
        let am1 = a - 1.0;
        let ap1 = a + 1.0;
        let cam1 = am1 * inst.boostcos;
        let cap1 = ap1 * inst.boostcos;
        let ia0 = 1.0 / (ap1 + cam1 + bs);
        self.b1 = 2.0 * a * (am1 - cap1) * ia0;
        self.a1 = -2.0 * (am1 + cap1) * ia0;
        self.a2 = (ap1 + cam1 - bs) * ia0;
        self.b0 = a * (ap1 - cam1 + bs) * ia0;
        self.b2 = a * (ap1 - cam1 - bs) * ia0;
    }

    fn render(&mut self, buf: &mut [f32], n: usize) {
        if !self.enabled {
            return;
        }
        for ch in 0..2 {
            let (mut xm1, mut xm2, mut ym1, mut ym2) = (self.x1[ch], self.x2[ch], self.y1[ch], self.y2[ch]);
            for i in 0..n {
                let x = buf[2 * i + ch] + FCDCOFFSET;
                let y = self.b0 * x + self.b1 * xm1 + self.b2 * xm2 - self.a1 * ym1 - self.a2 * ym2;
                ym2 = ym1;
                ym1 = y;
                xm2 = xm1;
                xm1 = x;
                buf[2 * i + ch] = y;
            }
            self.x1[ch] = xm1;
            self.x2[ch] = xm2;
            self.y1[ch] = ym1;
            self.y2[ch] = ym2;
        }
    }
}

#[derive(Clone)]
struct ModDel {
    db: [Vec<f32>; 2],
    mask: u32,
    dbptr: u32,
    dboffs: [u32; 2],
    mcnt: u32,
    mfreq: i32,
    mphase: u32,
    mmaxoffs: u32,
    fbval: f32,
    dryout: f32,
    wetout: f32,
}

impl ModDel {
    fn new(len: usize) -> ModDel {
        ModDel { db: [vec![0.0; len], vec![0.0; len]], mask: len as u32 - 1, dbptr: 0, dboffs: [0; 2], mcnt: 0, mfreq: 0, mphase: 0, mmaxoffs: 0, fbval: 0.0, dryout: 0.0, wetout: 0.0 }
    }

    fn reset(&mut self) {
        self.dbptr = 0;
        self.mcnt = 0;
        for d in self.db.iter_mut() {
            d.iter_mut().for_each(|v| *v = 0.0);
        }
    }

    fn set(&mut self, p: &[f32], inst: &Inst) {
        self.wetout = (p[0] - 64.0) / 64.0;
        self.dryout = 1.0 - self.wetout.abs();
        self.fbval = (p[1] - 64.0) / 64.0;
        let lenscale = (self.mask as f32 - 1023.0) / 128.0;
        self.dboffs[0] = (p[2] * lenscale) as i32 as u32;
        self.dboffs[1] = (p[3] * lenscale) as i32 as u32;
        self.mfreq = (inst.linfreq * FCMDLFOMUL * calcfreq(p[4] / 128.0)) as i32;
        self.mmaxoffs = (p[5] * 1023.0 / 128.0) as i32 as u32;
        self.mphase = ftou32((p[6] - 64.0) / 128.0);
    }

    fn chan_sample(&mut self, input: f32, ch: usize, dry: f32) -> f32 {
        let mut counter = self.mcnt.wrapping_add(if ch == 1 { self.mphase } else { 0 });
        counter = if counter < 0x80000000 { counter.wrapping_mul(2) } else { 0xffffffffu32.wrapping_sub(counter.wrapping_mul(2)) };
        let offs = u64::from(counter) * u64::from(self.mmaxoffs);
        let offs_int = ((offs >> 32) as u32).wrapping_add(self.dboffs[ch]);
        let index = self.dbptr.wrapping_sub(offs_int);
        let buf = &mut self.db[ch];
        let x = utof23(offs as u32);
        let delayed = lerp(buf[(index & self.mask) as usize], buf[(index.wrapping_sub(1) & self.mask) as usize], x);
        buf[self.dbptr as usize] = input + delayed * self.fbval;
        input * dry + delayed * self.wetout
    }

    fn sample(&mut self, l: f32, r: f32, dry: f32) -> (f32, f32) {
        let ol = self.chan_sample(l, 0, dry);
        let or = self.chan_sample(r, 1, dry);
        self.mcnt = self.mcnt.wrapping_add(self.mfreq as u32);
        self.dbptr = (self.dbptr + 1) & self.mask;
        (ol, or)
    }

    /// The delay's output from a mono send, added to `dest`.
    fn render_aux2main(&mut self, dest: &mut [f32], aux2: &[f32], n: usize) {
        if self.wetout == 0.0 {
            return;
        }
        for i in 0..n {
            let input = aux2[i] + FCDCOFFSET;
            let (l, r) = self.sample(input, input, 0.0);
            dest[2 * i] += l;
            dest[2 * i + 1] += r;
        }
    }

    fn render_chan(&mut self, buf: &mut [f32], n: usize) {
        if self.wetout == 0.0 {
            return;
        }
        let dry = self.dryout;
        for i in 0..n {
            let (l, r) = self.sample(buf[2 * i] + FCDCOFFSET, buf[2 * i + 1] + FCDCOFFSET, dry);
            buf[2 * i] = l;
            buf[2 * i + 1] = r;
        }
    }
}

const COMPDLEN: usize = 5700;
const RMSLEN: usize = 8192;
const COMP_RMS: i32 = 1;
const COMP_STEREO: i32 = 2;
const COMP_OFF: i32 = 4;

#[derive(Clone)]
struct Comp {
    mode: i32,
    invol: f32,
    ratio: f32,
    outvol: f32,
    attack: f32,
    release: f32,
    dblen: u32,
    dbcnt: u32,
    curgain: [f32; 2],
    peakval: [f32; 2],
    rmsval: [f32; 2],
    rmscnt: usize,
    dbuf: Vec<f32>,
    rmsbuf: Vec<f32>,
}

impl Comp {
    fn new() -> Comp {
        let mut c = Comp { mode: 0, invol: 0.0, ratio: 0.0, outvol: 0.0, attack: 0.0, release: 0.0, dblen: 0, dbcnt: 0, curgain: [0.0; 2], peakval: [0.0; 2], rmsval: [0.0; 2], rmscnt: 0, dbuf: vec![0.0; COMPDLEN * 2], rmsbuf: vec![0.0; RMSLEN * 2] };
        c.init();
        c
    }

    fn init(&mut self) {
        self.mode = COMP_STEREO;
        self.dbuf.iter_mut().for_each(|v| *v = 0.0);
        self.reset();
    }

    fn reset(&mut self) {
        self.peakval = [0.0; 2];
        self.rmsval = [0.0; 2];
        self.curgain = [1.0; 2];
        self.rmsbuf.iter_mut().for_each(|v| *v = 0.0);
        self.rmscnt = 0;
    }

    fn set(&mut self, p: &[f32], inst: &Inst) {
        let old = self.mode;
        match p[0] as i32 {
            0 => self.mode = COMP_OFF,
            1 => self.mode = 0,
            2 => self.mode = COMP_RMS,
            _ => {}
        }
        if p[1] != 0.0 {
            self.mode |= COMP_STEREO;
        }
        if self.mode != old {
            self.reset();
        }
        self.dblen = (p[3] * inst.samplesperms) as i32 as u32;
        let mut thresh = 8.0 * calcfreq(p[4] / 128.0);
        self.invol = 1.0 / thresh;
        if p[2] != 0.0 {
            thresh = 1.0;
        }
        self.outvol = thresh * 2.0f32.powf((p[8] - 64.0) / 16.0);
        self.ratio = p[5] / 128.0;
        self.attack = 2.0f32.powf(-p[6] * 12.0 / 128.0);
        self.release = 2.0f32.powf(-p[7] * 16.0 / 128.0);
    }

    fn peak(&mut self, x: f32, ch: usize) -> f32 {
        let a = self.peakval[ch] * FCCPDFALLOFF + FCDCOFFSET;
        let b = x.abs();
        self.peakval[ch] = if a > b { a } else { b };
        self.peakval[ch]
    }

    fn rms(&mut self, x: f32, ch: usize) -> f32 {
        let insq = sqr(x + FCDCOFFSET);
        self.rmsval[ch] += insq - self.rmsbuf[self.rmscnt * 2 + ch];
        self.rmsbuf[self.rmscnt * 2 + ch] = insq;
        (self.rmsval[ch] / RMSLEN as f32).sqrt()
    }

    /// Compresses `n` interleaved samples; each sample's gain left and
    /// right goes to `gains`, for the channels' shares.
    fn render(&mut self, buf: &mut [f32], n: usize, levels: &mut [f32], gains: Option<&mut [f32]>) {
        if self.mode & COMP_OFF != 0 {
            return;
        }
        match self.mode & (COMP_RMS | COMP_STEREO) {
            0 => {
                for i in 0..n {
                    let v = self.invol * self.peak(0.5 * (buf[2 * i] + buf[2 * i + 1]), 0);
                    levels[2 * i] = v;
                    levels[2 * i + 1] = v;
                }
            }
            COMP_RMS => {
                for i in 0..n {
                    let v = self.invol * self.rms(0.5 * (buf[2 * i] + buf[2 * i + 1]), 0);
                    levels[2 * i] = v;
                    levels[2 * i + 1] = v;
                    self.rmscnt = (self.rmscnt + 1) & (RMSLEN - 1);
                }
            }
            COMP_STEREO => {
                for i in 0..n {
                    levels[2 * i] = self.invol * self.peak(buf[2 * i], 0);
                    levels[2 * i + 1] = self.invol * self.peak(buf[2 * i + 1], 1);
                }
            }
            _ => {
                for i in 0..n {
                    levels[2 * i] = self.invol * self.rms(buf[2 * i], 0);
                    levels[2 * i + 1] = self.invol * self.rms(buf[2 * i + 1], 1);
                    self.rmscnt = (self.rmscnt + 1) & (RMSLEN - 1);
                }
            }
        }
        let mut gains = gains;
        for ch in 0..2 {
            let mut gain = self.curgain[ch];
            let mut dbind = self.dbcnt;
            for i in 0..n {
                let v = self.outvol * self.dbuf[dbind as usize * 2 + ch];
                self.dbuf[dbind as usize * 2 + ch] = self.invol * buf[2 * i + ch];
                dbind += 1;
                if dbind >= self.dblen {
                    dbind = 0;
                }
                let mut dgain = 1.0f32;
                let lvl = levels[2 * i + ch];
                if lvl >= 1.0 {
                    dgain = 1.0 / (1.0 + self.ratio * (lvl - 1.0));
                }
                gain += (if dgain < gain { self.attack } else { self.release }) * (dgain - gain);
                buf[2 * i + ch] = v * gain;
                if let Some(g) = gains.as_deref_mut() {
                    g[2 * i + ch] = gain;
                }
            }
            self.curgain[ch] = gain;
            if ch == 1 {
                self.dbcnt = dbind;
            }
        }
    }
}

const COMB_LEN: [[usize; 4]; 2] = [[1309, 1635, 1811, 1926], [1327, 1631, 1833, 1901]];
const ALL_LEN: [[usize; 2]; 2] = [[220, 74], [205, 77]];

#[derive(Clone)]
struct Reverb {
    gainc: [f32; 4],
    gaina: [f32; 2],
    gainin: f32,
    damp: f32,
    lowcut: f32,
    combd: [[Delay; 4]; 2],
    combl: [[f32; 4]; 2],
    alld: [[Delay; 2]; 2],
    hpf: [f32; 2],
}

impl Reverb {
    fn new() -> Reverb {
        Reverb {
            gainc: [0.0; 4],
            gaina: [0.0; 2],
            gainin: 0.0,
            damp: 0.0,
            lowcut: 0.0,
            combd: COMB_LEN.map(|c| c.map(Delay::new)),
            combl: [[0.0; 4]; 2],
            alld: ALL_LEN.map(|c| c.map(Delay::new)),
            hpf: [0.0; 2],
        }
    }

    fn reset(&mut self) {
        for ch in 0..2 {
            for i in 0..4 {
                self.combd[ch][i].reset();
                self.combl[ch][i] = 0.0;
            }
            for i in 0..2 {
                self.alld[ch][i].reset();
            }
            self.hpf[ch] = 0.0;
        }
    }

    fn set(&mut self, p: &[f32], inst: &Inst) {
        const GAINC: [f32; 4] = [0.966384599, 0.958186359, 0.953783929, 0.950933178];
        const GAINA: [f32; 2] = [0.994260075, 0.998044717];
        let e = inst.linfreq * sqr(64.0 / (p[0] + 1.0));
        self.gainc = GAINC.map(|g| g.powf(e));
        self.gaina = GAINA.map(|g| g.powf(e));
        self.damp = inst.linfreq * (p[1] / 128.0);
        self.gainin = p[3] / 128.0;
        self.lowcut = inst.linfreq * sqr(sqr(p[2] / 128.0));
    }

    /// The reverb of a mono send, added to `dest`.
    fn render(&mut self, dest: &mut [f32], aux1: &[f32], n: usize) {
        for i in 0..n {
            let input = aux1[i] * self.gainin + FCDCOFFSET;
            for ch in 0..2 {
                let mut cur = 0.0f32;
                for j in 0..4 {
                    let dv = self.gainc[j] * self.combd[ch][j].fetch();
                    let nv = if j & 1 == 1 { dv - input } else { dv + input };
                    // V2 never keeps this filter's state: it stays 0.
                    let lp = self.combl[ch][j] + self.damp * (nv - self.combl[ch][j]);
                    self.combd[ch][j].feed(lp);
                    cur += lp;
                }
                for j in 0..2 {
                    let dv = self.alld[ch][j].fetch();
                    let dz = cur + self.gaina[j] * dv;
                    self.alld[ch][j].feed(dz);
                    cur = dv - self.gaina[j] * dz;
                }
                self.hpf[ch] += self.lowcut * (cur - self.hpf[ch]);
                dest[2 * i + ch] += cur - self.hpf[ch];
            }
        }
    }
}

#[derive(Clone)]
struct Chan {
    chgain: f32,
    a1gain: f32,
    a2gain: f32,
    aasnd: f32,
    absnd: f32,
    aarcv: f32,
    abrcv: f32,
    fxr: i32,
    dcf1: DcFilter,
    boost: Boost,
    dist: Dist,
    dcf2: DcFilter,
    chorus: ModDel,
    comp: Comp,
}

impl Chan {
    fn new() -> Chan {
        Chan { chgain: 0.0, a1gain: 0.0, a2gain: 0.0, aasnd: 0.0, absnd: 0.0, aarcv: 0.0, abrcv: 0.0, fxr: 0, dcf1: DcFilter::default(), boost: Boost::default(), dist: Dist::default(), dcf2: DcFilter::default(), chorus: ModDel::new(2048), comp: Comp::new() }
    }

    fn init(&mut self) {
        self.dcf1 = DcFilter::default();
        self.dist.init();
        self.dcf2 = DcFilter::default();
        self.chorus.reset();
        self.comp.init();
    }

    fn set(&mut self, p: &[f32; NCHAN], inst: &Inst) {
        self.aarcv = p[1] / 128.0;
        self.abrcv = p[2] / 128.0;
        self.aasnd = FCGAIN * (p[3] / 128.0);
        self.absnd = FCGAIN * (p[4] / 128.0);
        self.chgain = FCGAIN * (p[0] / 128.0);
        self.a1gain = self.chgain * FCGAINH * (p[5] / 128.0);
        self.a2gain = self.chgain * FCGAINH * (p[6] / 128.0);
        self.fxr = p[7] as i32;
        self.dist.set(&p[9..13], inst);
        self.chorus.set(&p[13..20], inst);
        self.comp.set(&p[20..29], inst);
        self.boost.set(p[8], inst);
    }
}

fn accumulate(dest: &mut [f32], src: &[f32], n: usize, gain: f32) {
    if gain == 0.0 {
        return;
    }
    for i in 0..2 * n {
        dest[i] += gain * src[i];
    }
}

fn accumulate_mono(dest: &mut [f32], src: &[f32], n: usize, gain: f32) {
    if gain == 0.0 {
        return;
    }
    for i in 0..n {
        dest[i] += gain * (src[2 * i] + src[2 * i + 1]);
    }
}

// ---------------------------------------------------------------------
// Ronan, the speech synth.

/// A phoneme: formant frequencies and bandwidths, levels (dB), duration and rank.
#[derive(Clone, Copy, Default)]
struct Phoneme {
    f: [f32; 19],
}

// Field indexes in a phoneme, as V2 lays them out.
const P_F1F: usize = 0;
const P_F1B: usize = 1;
const P_F2F: usize = 2;
const P_F2B: usize = 3;
const P_F3F: usize = 4;
const P_F3B: usize = 5;
const P_FNF: usize = 6;
const P_VOICING: usize = 7;
const P_ASPIRATION: usize = 8;
const P_FRICATION: usize = 9;
const P_BYPASS: usize = 10;
const P_A1: usize = 11;
const P_A2: usize = 12;
const P_A3: usize = 13;
const P_A4: usize = 14;
const P_AN: usize = 15;
const P_A56: usize = 16;
const P_DURATION: usize = 17;
const P_RANK: usize = 18;

const NPHONEMES: usize = 69;
const PMULT: [u8; 19] = [10, 10, 20, 10, 20, 10, 10, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1];

/// V2's phonemes, as it keeps them: each field's 69 values in turn, each
/// the difference from the one before (signed bytes).
#[rustfmt::skip]
const RAW_PHONEMES: [u8; 1311] = [
    49, 30,241,  0, 15,226,226,  0,  0,  0, 21,235,  9,  0,247,  0, 45,217, 24,  9,238,235,  0,  0, 30,247,247, 18, 30,196,  9,247,  0,  0, 27,  0,  2,  0,  0, 13,244,  0,244,  0, 12, 30,214,238,  0,  0, 30,235, 21,  0,247,  0,235, 21,235,  0, 51,211,  3,247,  0,  6,  3,  0,  0,
   234,  7,249,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,254,  0, 12,246,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0, 14,242,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,
    68, 15,247, 21,199,253,253,  0,  0, 51, 12,244,247,  0,  9,  0, 12, 15,214,253,  0,  3,  0,  0,  0, 30,  6,220,226, 45, 12,229,  0,  0,  0,229,  3, 39,208,  3, 30,223,  9,  0,247, 24,241,244,  0,  0, 36,253,244,  0, 27, 24,235,  0,  0,  0,235,232, 27,223, 36, 51,217, 15,241,
   179,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  8, 13,241,250,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,
   116,  0,  0,  0,  0,  0,  0,  0,  0,  9,250,  6,250,  0,  6,  0,247, 35,221,  0,  3,  3,  0,  0,250,  3, 18,235,  0,  9,250,  3,  0,  0,250,  0,241, 21,  9,241,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  3,208,  0, 51,253,  6,  0,  0,  0,247,241, 18,229, 30, 18,235,  0,  0,
   143,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,253, 14,240,  5,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,248, 15,249,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,
    12,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  9,  9,247,247,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,
    35,  0,  0,  0,  0,  0,  0,  0,  0,194,  0, 62,230, 26,  0,  0,  0,  0,194, 62,194, 62,  0,  0,194, 62,  0,  0,  0,  0,  0,194,  0,  0, 62,  0,  0,  0,246, 10,  0,  0,  0,  0,  0,  0,  0,194,  0,  0,  0, 62,  0,244,206,  0,  0,  0,  0,  0, 62,  0,  0,  0,194, 62,234, 22,  0,
   194,  0,  0,  0,  0,  0,  0,  0,  0, 60,  0,196,  0,  0,  0,  0,  0,  0,  0,  0, 32,224,  0,  0, 60,196,  0,  0,  0,  0,  0, 60,  0,  0,196,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0, 60,  0,  0,196,  0,  0,  0, 32, 28,  0,  0,  0,  0,196,  0,  0,  0, 60,196, 20,236,  0,
     0,  0,  0,  0,  0,  0,  0,  0,  0, 60,  0,196, 60,196,  0,  0,  0,  0,  0,  0, 54,202,  0,  0, 60,196,  0,  0,  0,  0,  0, 60,  0,  0,196,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0, 60,  0,  0,196,  0,  0,  0, 60,  0,  0,  0,  0,  0,196,  0,  0,  0, 60,196, 60,196,  0,
   237,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0, 70,186,  0,  0,  0,  0,  0,  0, 70,186,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,
    83,  0,  0,  0,  0,  0,230,  0,202,  0,  0, 61,254,  0,  9,  0, 12,  0,176, 80,176, 65,  0,  0,  0, 15,  0,  0,  0,237,254,197,  0,  0, 66,  0,246,  9,241, 30,  0,  0,  0,  0,  0,  0,  0,176, 54,  0,202, 59, 13,  0,184,  0,  0,  0,  0,  0, 80,  0,235, 14,183, 80,235,  0,  0,
    21,253,254,253,  7,252,181, 79,177,  0, 61,195, 61,  0,  7,246, 14,247,193, 75,211,226, 75,246,  1,  0,255, 15,255,177, 56,200, 80,246,242,  0,  4,  5,251, 17,  3,251,253,  0,  3,  2,251,184, 79,245,188, 70,251,  0,249,  3,195, 56,200,  0, 73,251,  2,244, 12,249,247,  2,254,
     9,  5,253,  3,247,249,204, 73,183,  0, 72,184, 56,  0,  9,245, 14,254,190, 63,223,226, 70,245,253,  9,  1,253,252,197, 66,190, 80,246,242,  0,  7,  2,  0,243, 11,245,  6,  0,250, 13,249,198, 73,246,193, 66,255,  0,249, 14,184, 58, 10,246,  3,242, 19,241, 19,254,242, 12,244,
     2,  5,254,  2,247,251,209, 68,188,  0, 61,195, 58,  0, 17,246,252,  0,195, 56,200,  0, 54,246,  8,  7,  2,251,252,204, 56,200, 59,246,  2,  0,205, 50,236, 15, 11,247,  5,  0,251, 11,250,204, 68,246,198, 63,193,  0, 70,247,195, 52, 28,246,240,242, 23,193, 49, 12,  5,246, 10,
   190,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0, 72,  0,  0,184,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0, 56,200,  0, 66,246, 10,246,  0,246,210,  0, 56,200, 46,210, 66,236,  0,
   225,  0,  0,  0,  0,  0,  0,  0,  0, 46,246,  0, 10,236,246,240,  0,  0,  0,  0, 46,210,  0,  0, 26,230,  0,  0,  0, 26,246, 10,  0,246,240,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0, 46,236, 10,246,230,  0,  0,  0,  0,  0,  0,  0,  0, 26,  0,
   235,  1,  1,  0,  9,251,  2,245,255,  4,  4,  0,252,  0,253,  0,  3,  3,254, 11,252,  0,245,  1,  3,  1,  0,  0,  0,254,255,  5,249,  3,  4,  0,  0,  0,  0,254,  0,  0,254,  2,  0,  0,  0,  2,249,  1,  4,250, 11,255,  2,  0,250,  9,242,  1,  4,  3,251,  4,  4,251,253,  0,  0,
   254,  0,  0,  0,  0,  0, 24,  3,253,253,251,  8,250,  0,  9,253,232,  0, 29,227, 16,  8,  3,253,239,249,  0,  0,  0, 24,250,  3,  6,250,244,  0,  0,  0,  0,247,  0,  0,  0,  0,  0,  0,  0, 21,  6,250,  6,  1,236,  0,  8,  0,  5,251, 11,250,235,  0, 18,246,  8,248, 10,  0,  0,
];

/// Each syllable's spelling and up to four phonemes.
const SYLS: [(&[u8], [i8; 4]); 52] = [
    (b"sil", [50, -1, -1, -1]),
    (b"ng", [38, -1, -1, -1]),
    (b"th", [57, -1, -1, -1]),
    (b"sh", [55, -1, -1, -1]),
    (b"dh", [12, 51, 13, -1]),
    (b"zh", [67, 51, 67, -1]),
    (b"ch", [9, 10, -1, -1]),
    (b"ih", [25, -1, -1, -1]),
    (b"eh", [16, -1, -1, -1]),
    (b"ae", [1, -1, -1, -1]),
    (b"ah", [60, -1, -1, -1]),
    (b"oh", [39, -1, -1, -1]),
    (b"uh", [42, -1, -1, -1]),
    (b"ax", [0, -1, -1, -1]),
    (b"iy", [17, -1, -1, -1]),
    (b"er", [19, -1, -1, -1]),
    (b"aa", [4, -1, -1, -1]),
    (b"ao", [5, -1, -1, -1]),
    (b"uw", [61, -1, -1, -1]),
    (b"ey", [2, 25, -1, -1]),
    (b"ay", [28, 25, -1, -1]),
    (b"oy", [41, 25, -1, -1]),
    (b"aw", [45, 46, -1, -1]),
    (b"ow", [40, 46, -1, -1]),
    (b"ia", [26, 27, -1, -1]),
    (b"ea", [3, 27, -1, -1]),
    (b"ua", [43, 27, -1, -1]),
    (b"ll", [35, -1, -1, -1]),
    (b"wh", [63, -1, -1, -1]),
    (b"ix", [0, -1, -1, -1]),
    (b"el", [34, -1, -1, -1]),
    (b"rx", [53, -1, -1, -1]),
    (b"h", [24, -1, -1, -1]),
    (b"p", [47, 48, 49, -1]),
    (b"t", [56, 58, 59, -1]),
    (b"k", [31, 32, 33, -1]),
    (b"b", [6, 7, 8, -1]),
    (b"d", [11, 14, 15, -1]),
    (b"g", [21, 22, 23, -1]),
    (b"m", [36, -1, -1, -1]),
    (b"n", [37, -1, -1, -1]),
    (b"f", [20, -1, -1, -1]),
    (b"s", [54, -1, -1, -1]),
    (b"v", [62, 51, 62, -1]),
    (b"z", [66, 51, 68, -1]),
    (b"l", [34, -1, -1, -1]),
    (b"r", [52, -1, -1, -1]),
    (b"w", [63, -1, -1, -1]),
    (b"q", [51, -1, -1, -1]),
    (b"y", [65, -1, -1, -1]),
    (b"j", [29, 30, 51, 30]),
    (b" ", [18, -1, -1, -1]),
];

#[derive(Clone, Copy, Default)]
struct ResDef {
    a: f32,
    b: f32,
    c: f32,
}

#[derive(Clone, Copy, Default)]
struct Res {
    p1: f32,
    p2: f32,
}

impl Res {
    fn tick(&mut self, d: &ResDef, input: f32) -> f32 {
        let x = d.a * input + d.b * self.p1 + d.c * self.p2;
        self.p2 = self.p1;
        self.p1 = x;
        x
    }
}

/// The 25 values Ronan glides between frames: seven resonators' coefficients and four levels.
#[derive(Clone, Copy, Default)]
struct RFrame {
    v: [f32; 25],
}

impl RFrame {
    fn def(&self, i: usize) -> ResDef {
        ResDef { a: self.v[3 * i], b: self.v[3 * i + 1], c: self.v[3 * i + 2] }
    }
}

struct RonanG {
    fcminuspi_sr: f32,
    fc2pi_sr: f32,
    phonemes: [Phoneme; NPHONEMES],
    peq1: ResDef,
}

#[derive(Clone, Default)]
struct Ronan {
    cur: RFrame,
    newframe: RFrame,
    res: [Res; 7],
    lastin2: f32,
    /// The song's lyrics: up to 64 texts.
    texts: Vec<Option<Arc<[u8]>>>,
    /// Which text, and where in it.
    text: Option<Arc<[u8]>>,
    base: usize,
    ptr: usize,
    pitch: f32,
    framerate: i32,
    nseed: u32,
    nout: f32,
    framecount: i32,
    spos: i32,
    scounter: i32,
    cursyl: usize,
    durfactor: i32,
    invdur: f32,
    curp1: usize,
    curp2: usize,
    wait4on: bool,
    wait4off: bool,
    hpb1: f32,
    hpb2: f32,
    peq1: Res,
}

fn resdef(g: &RonanG, f: f32, bw: f32, gain: f32) -> ResDef {
    let r = (f64::from(g.fcminuspi_sr * bw)).exp() as f32;
    let c = -(r * r);
    let b = r * (g.fc2pi_sr * f).cos() * 2.0;
    let a = gain * (1.0 - b - c);
    ResDef { a, b, c }
}

fn flerp(a: f32, b: f32, x: f32) -> f32 {
    a + x * (b - a)
}

fn db2lin(db1: f32, db2: f32, x: f32) -> f32 {
    pow_x87(2.0, f64::from(flerp(db1, db2, x) - 70.0) / 6.0) as f32
}

/// The speech synth's pow (x87 in V2; the C library's here).
fn pow_x87(a: f64, b: f64) -> f64 {
    if a == 0.0 {
        0.0
    } else {
        a.powf(b)
    }
}

impl RonanG {
    fn new() -> RonanG {
        let pi = 4.0f32 * 1.0f32.atan();
        let fc2pi_sr = 2.0 * pi / RATE as f32;
        let mut phonemes = [Phoneme::default(); NPHONEMES];
        let mut val = 0i32;
        let mut k = 0;
        for (f, &mul) in PMULT.iter().enumerate() {
            for p in phonemes.iter_mut() {
                val += i32::from(RAW_PHONEMES[k] as i8);
                k += 1;
                p.f[f] = f32::from(mul) * val as f32;
            }
        }
        let mut g = RonanG { fcminuspi_sr: -fc2pi_sr * 0.5, fc2pi_sr, phonemes, peq1: ResDef::default() };
        g.peq1 = resdef(&g, 12000.0, 4000.0, 2.0);
        g
    }

    fn frame(&self, p1: &Phoneme, p2: &Phoneme, x: f32, pitch: f32) -> RFrame {
        const F: [f32; 3] = [3200.0, 4000.0, 6000.0];
        const B: [f32; 4] = [100.0, 200.0, 500.0, 800.0];
        let ff = |p: &Phoneme| [p.f[P_FNF], p.f[P_F1F], p.f[P_F2F], p.f[P_F3F], F[0], F[1], F[2]];
        let fb = |p: &Phoneme| [B[0], p.f[P_F1B], p.f[P_F2B], p.f[P_F3B], B[1], B[2], B[3]];
        let fa = |p: &Phoneme| [p.f[P_AN], p.f[P_A1], p.f[P_A2], p.f[P_A3], p.f[P_A4], p.f[P_A56], p.f[P_A56]];
        let (f1, f2, b1, b2, a1, a2) = (ff(p1), ff(p2), fb(p1), fb(p2), fa(p1), fa(p2));
        let mut out = RFrame::default();
        for i in 0..7 {
            let d = resdef(self, flerp(f1[i], f2[i], x) * pitch, flerp(b1[i], b2[i], x), db2lin(a1[i], a2[i], x));
            out.v[3 * i] = d.a;
            out.v[3 * i + 1] = d.b;
            out.v[3 * i + 2] = d.c;
        }
        out.v[21] = db2lin(p1.f[P_VOICING], p2.f[P_VOICING], x);
        out.v[22] = db2lin(p1.f[P_ASPIRATION], p2.f[P_ASPIRATION], x);
        out.v[23] = db2lin(p1.f[P_FRICATION], p2.f[P_FRICATION], x);
        out.v[24] = db2lin(p1.f[P_BYPASS], p2.f[P_BYPASS], x);
        out
    }
}

impl Ronan {
    fn byte(&self, at: usize) -> u8 {
        self.text.as_ref().and_then(|t| t.get(at).copied()).unwrap_or(0)
    }

    fn reset(&mut self, g: &RonanG) {
        let off = g.phonemes[18];
        self.cur = g.frame(&off, &off, 0.0, self.pitch);
        self.newframe = g.frame(&off, &off, 0.0, self.pitch);
        self.curp1 = 18;
        self.curp2 = 18;
        self.spos = 4;
    }

    fn init(&mut self, g: &RonanG) {
        self.reset(g);
        self.framerate = 3;
        self.pitch = 1.0;
        self.text = self.texts.first().cloned().flatten();
        self.base = 0;
        self.ptr = 0;
        self.lastin2 = 0.0;
    }

    fn noise(&mut self) -> f32 {
        self.nseed = self.nseed.wrapping_mul(196314165).wrapping_add(907633515);
        let f = bits2float((self.nseed >> 9) | 0x40000000);
        self.nout = (f - 3.0) + 0.75 * self.nout;
        self.nout * 0.25
    }

    fn tick(&mut self, g: &RonanG) {
        if self.wait4off || self.wait4on {
            return;
        }
        if self.framecount <= 0 {
            self.framecount = self.framerate;
            if self.scounter <= 0 {
                self.spos += 1;
                if self.spos >= 4 || SYLS[self.cursyl].1[self.spos as usize] == -1 {
                    let c = self.byte(self.ptr);
                    if c == 0 {
                        // The end of the text: silence, and back to its start.
                        self.durfactor = 1;
                        self.framecount = 1;
                        self.cursyl = SYLS.len() - 1;
                        self.spos = 0;
                        self.ptr = self.base;
                    } else if c == b'!' {
                        self.framecount = 0;
                        self.scounter = 0;
                        self.wait4on = true;
                        self.ptr += 1;
                        return;
                    } else if c == b'_' {
                        self.framecount = 0;
                        self.scounter = 0;
                        self.wait4off = true;
                        self.ptr += 1;
                        return;
                    }
                    let c = self.byte(self.ptr);
                    if c != 0 && c != b'!' && c != b'_' {
                        self.durfactor = 0;
                        while self.byte(self.ptr).is_ascii_digit() {
                            self.durfactor = 10 * self.durfactor + i32::from(self.byte(self.ptr) - b'0');
                            self.ptr += 1;
                        }
                        if self.durfactor == 0 {
                            self.durfactor = 1;
                        }
                        let mut len = 1;
                        let mut fs = 0;
                        while fs < SYLS.len() - 1 {
                            let l = self.matches(SYLS[fs].0);
                            if l != 0 {
                                len = l;
                                break;
                            }
                            fs += 1;
                        }
                        self.cursyl = fs;
                        self.spos = 0;
                        self.ptr += len;
                    }
                }
                self.curp1 = self.curp2;
                self.curp2 = SYLS[self.cursyl].1[self.spos as usize] as usize;
                self.scounter = (g.phonemes[self.curp2].f[P_DURATION] * self.durfactor as f32).round_ties_even() as i32;
                if self.scounter == 0 {
                    self.scounter = 1;
                }
                self.invdur = 1.0 / (self.scounter as f32 * self.framerate as f32);
            }
            self.scounter -= 1;
        }
        self.framecount -= 1;
        let x = (self.scounter * self.framerate + self.framecount) as f32 * self.invdur;
        let p1 = g.phonemes[self.curp1];
        let p2 = g.phonemes[self.curp2];
        let x = pow_x87(f64::from(x), f64::from(p1.f[P_RANK] / p2.f[P_RANK])) as f32;
        let other = if (p2.f[P_RANK] - p1.f[P_RANK]).abs() > 8.0 { p2 } else { p1 };
        self.newframe = g.frame(&p2, &other, x, self.pitch);
    }

    /// How much of the text at `ptr` spells `syl` (its letters in any case), or 0.
    fn matches(&self, syl: &[u8]) -> usize {
        let mut l = 0;
        let mut i = 0;
        while i < syl.len() && self.byte(self.ptr + i) != 0 {
            if (syl[i] | 0x20) != (self.byte(self.ptr + i) | 0x20) {
                return 0;
            }
            l += 1;
            i += 1;
        }
        if i < syl.len() {
            0
        } else {
            l
        }
    }

    fn set_ctl(&mut self, ctl: u32, val: u32, g: &RonanG) {
        match ctl {
            4 => {
                if val < 63 {
                    self.reset(g);
                    self.text = self.texts.get(val as usize).cloned().flatten();
                    self.base = 0;
                    self.ptr = 0;
                } else {
                    self.framerate = val as i32 - 63;
                }
            }
            5 => self.pitch = pow_x87(2.0, f64::from((val as f32 - 64.0) / 128.0)) as f32,
            _ => {}
        }
    }

    /// Speaks through channel 16's frame: its left side is the excitation.
    fn process(&mut self, buf: &mut [f32], len: usize, g: &RonanG) {
        let mul = 1.0 / len as f32;
        let mut delta = RFrame::default();
        for i in 0..25 {
            delta.v[i] = (self.newframe.v[i] - self.cur.v[i]) * mul;
        }
        for i in 0..len {
            for k in 0..25 {
                self.cur.v[k] += delta.v[k];
            }
            let input = buf[2 * i];
            let input = input * self.cur.v[21] + self.noise() * self.cur.v[22];
            let mut out = self.res[0].tick(&self.cur.def(0), input) + self.res[1].tick(&self.cur.def(1), input);
            let lin = input;
            let input = (self.noise() * self.cur.v[23]) + input - self.lastin2;
            self.lastin2 = lin;
            for r in 2..7 {
                out = self.res[r].tick(&self.cur.def(r), input) - out;
            }
            out = input * self.cur.v[24] - out;
            out -= self.hpb1;
            self.hpb1 += 0.012 * out;
            out -= self.hpb2;
            self.hpb2 += 0.012 * out;
            out = self.peq1.tick(&g.peq1, out) - out;
            buf[2 * i] = out;
            buf[2 * i + 1] = out;
        }
    }
}

// ---------------------------------------------------------------------
// The synth.

/// A channel's frame as it reaches the mix: its channel, what it puts in
/// directly (stereo), and its sends to the reverb and the delay.
type Sent = (usize, Vec<f32>, Vec<f32>, Vec<f32>);

/// A channel's share of the mix's effects, so it can be heard alone.
#[derive(Clone)]
struct Share {
    reverb: Reverb,
    delay: ModDel,
    dcf: DcFilter,
    lcbuf: [f32; 2],
    hcbuf: [f32; 2],
    dbuf: Vec<f32>,
    dbcnt: u32,
}

struct Synth {
    patchmap: Arc<[u8]>,
    mrstat: u32,
    curalloc: u32,
    chanmap: [i32; POLY],
    allocpos: [u32; POLY],
    voicemap: [i32; CHANS],
    pgm: [u8; CHANS],
    ctl: [[u8; 7]; CHANS],
    voicesv: Vec<[f32; NVOICE]>,
    voices: Vec<Voice>,
    chansv: Vec<[f32; NCHAN]>,
    chans: Vec<Chan>,
    globals: [f32; NGLOB],
    reverb: Reverb,
    delay: ModDel,
    dcf: DcFilter,
    compr: Comp,
    lcfreq: f32,
    lcbuf: [f32; 2],
    hcfreq: f32,
    hcbuf: [f32; 2],
    inst: Inst,
    ronan: Ronan,
    rg: Arc<RonanG>,
    rand: Rand,
    // Frame buffers (stereo ones interleaved).
    vcebuf: Vec<f32>,
    vcebuf2: Vec<f32>,
    levelbuf: Vec<f32>,
    chanbuf: Vec<f32>,
    aux1: Vec<f32>,
    aux2: Vec<f32>,
    mix: Vec<f32>,
    auxa: Vec<f32>,
    auxb: Vec<f32>,
    gains: Vec<f32>,
    /// Each channel's share of the frame's mix, and its effects' state.
    out: Vec<Vec<f32>>,
    shares: Vec<Option<Box<Share>>>,
    last: usize,
}

impl Synth {
    fn new(patchmap: Arc<[u8]>, rg: Arc<RonanG>) -> Synth {
        let mut s = Synth {
            patchmap,
            mrstat: 0,
            curalloc: 0,
            chanmap: [-1; POLY],
            allocpos: [0; POLY],
            voicemap: [0; CHANS],
            pgm: [0; CHANS],
            ctl: [[0; 7]; CHANS],
            voicesv: vec![[0.0; NVOICE]; POLY],
            voices: vec![Voice::default(); POLY],
            chansv: vec![[0.0; NCHAN]; CHANS],
            chans: (0..CHANS).map(|_| Chan::new()).collect(),
            globals: [0.0; NGLOB],
            reverb: Reverb::new(),
            delay: ModDel::new(32768),
            dcf: DcFilter::default(),
            compr: Comp::new(),
            lcfreq: 0.0,
            lcbuf: [0.0; 2],
            hcfreq: 0.0,
            hcbuf: [0.0; 2],
            inst: Inst::new(),
            ronan: Ronan::default(),
            rg,
            rand: Rand(1),
            vcebuf: vec![0.0; FRAME],
            vcebuf2: vec![0.0; FRAME],
            levelbuf: vec![0.0; FRAME * 2],
            chanbuf: vec![0.0; FRAME * 2],
            aux1: vec![0.0; FRAME],
            aux2: vec![0.0; FRAME],
            mix: vec![0.0; FRAME * 2],
            auxa: vec![0.0; FRAME * 2],
            auxb: vec![0.0; FRAME * 2],
            gains: vec![0.0; FRAME * 2],
            out: vec![vec![0.0; FRAME * 2]; CHANS],
            shares: (0..CHANS).map(|_| None).collect(),
            last: 0,
        };
        s.ronan.texts = vec![None; 64];
        s
    }

    /// V2's synthInit: what it sets again (it leaves the rest as it was).
    fn init(&mut self) {
        for i in 0..POLY {
            self.chanmap[i] = -1;
            self.voices[i].init(&mut self.rand);
        }
        for i in 0..CHANS {
            self.ctl[i][6] = 0x7f;
            self.chans[i].init();
        }
        self.reverb.reset();
        self.delay.reset();
        let rg = self.rg.clone();
        self.ronan.init(&rg);
        self.compr.init();
        self.dcf = DcFilter::default();
    }

    fn set_globals(&mut self, para: &[u8]) {
        for i in 0..NGLOB {
            self.globals[i] = f32::from(para.get(i).copied().unwrap_or(0));
        }
        let g = self.globals;
        self.reverb.set(&g[0..4], &self.inst);
        self.delay.set(&g[4..11], &self.inst);
        self.compr.set(&g[13..22], &self.inst);
        self.lcfreq = sqr((g[11] + 1.0) / 128.0);
        self.hcfreq = sqr((g[12] + 1.0) / 128.0);
    }

    fn set_lyrics(&mut self, texts: &[Option<Arc<[u8]>>]) {
        for i in 0..64 {
            self.ronan.texts[i] = texts.get(i).cloned().flatten();
        }
        self.ronan.text = self.ronan.texts[0].clone();
        self.ronan.base = 0;
        self.ronan.ptr = 0;
    }

    /// The patch for a program: its bytes (none when the map doesn't have it).
    fn patch(&self, pgm: u8) -> &[u8] {
        let at = usize::from(pgm) * 4;
        let off = self.patchmap.get(at..at + 4).map_or(usize::MAX, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize);
        self.patchmap.get(off..).unwrap_or(&[])
    }

    fn mod_source(&self, v: usize, chan: usize, source: u8) -> f32 {
        let voice = &self.voices[v];
        match source {
            0 => voice.velo,
            1..=7 => f32::from(self.ctl[chan][usize::from(source) - 1]),
            8 | 9 => voice.env[usize::from(source) - 8].out,
            10 | 11 => voice.lfo[usize::from(source) - 10].out,
            _ => 2.0 * (voice.note as f32 - 48.0),
        }
    }

    fn store_voice(&mut self, v: usize) {
        let Ok(chan) = usize::try_from(self.chanmap[v]) else { return };
        let patch = self.patch(self.pgm[chan]).to_vec();
        let byte = |i: usize| patch.get(i).copied().unwrap_or(0);
        let mut p = [0f32; NVOICE];
        for (i, x) in p.iter_mut().enumerate() {
            *x = f32::from(byte(i));
        }
        let modnum = usize::from(byte(NVOICE + NCHAN + 1));
        for m in 0..modnum {
            let at = NVOICE + NCHAN + 2 + 3 * m;
            let (src, val, dest) = (byte(at), byte(at + 1), usize::from(byte(at + 2)));
            if dest >= NVOICE {
                continue;
            }
            let scale = (f32::from(val) - 64.0) / 64.0;
            p[dest] = clampf(p[dest] + scale * self.mod_source(v, chan, src), 0.0, 128.0);
        }
        self.voicesv[v] = p;
        self.voices[v].set(&p, &self.inst);
    }

    fn store_chan(&mut self, chan: usize) {
        let patch = self.patch(self.pgm[chan]).to_vec();
        let byte = |i: usize| patch.get(i).copied().unwrap_or(0);
        let mut p = [0f32; NCHAN];
        for (i, x) in p.iter_mut().enumerate() {
            *x = f32::from(byte(NVOICE + i));
        }
        let v = self.voicemap[chan] as usize % POLY;
        let modnum = usize::from(byte(NVOICE + NCHAN + 1));
        for m in 0..modnum {
            let at = NVOICE + NCHAN + 2 + 3 * m;
            let (src, val, dest) = (byte(at), byte(at + 1), i32::from(byte(at + 2)) - NVOICE as i32);
            if dest < 0 || dest >= NCHAN as i32 {
                continue;
            }
            let d = dest as usize;
            let scale = (f32::from(val) - 64.0) / 64.0;
            p[d] = clampf(p[d] + scale * self.mod_source(v, chan, src), 0.0, 128.0);
        }
        self.chansv[chan] = p;
        self.chans[chan].set(&p, &self.inst);
    }

    fn process_midi(&mut self, cmd: &[u8]) {
        let at = |i: usize| cmd.get(i).copied().unwrap_or(0xfd);
        let mut i = 0;
        let rg = self.rg.clone();
        while at(i) != 0xfd {
            if at(i) & 0x80 != 0 {
                self.mrstat = u32::from(at(i));
                i += 1;
            }
            if self.mrstat < 0x80 {
                break;
            }
            let chan = (self.mrstat & 0xf) as usize;
            let mut kind = (self.mrstat >> 4) & 7;
            if kind == 1 && at(i + 1) == 0 {
                kind = 0;
            }
            match kind {
                1 => {
                    if chan == CHANS - 1 {
                        self.ronan.wait4on = false;
                    }
                    let maxpoly = self.patch(self.pgm[chan]).get(NVOICE + NCHAN).copied().unwrap_or(0);
                    let npoly = self.chanmap.iter().filter(|&&c| c == chan as i32).count();
                    let mut usevoice: i32 = -1;
                    let (chanmask, chanfind) = if npoly == 0 || npoly < usize::from(maxpoly) {
                        usevoice = self.chanmap.iter().position(|&c| c < 0).map_or(-1, |p| p as i32);
                        (0, 0)
                    } else {
                        (0xf, chan as i32)
                    };
                    if usevoice < 0 {
                        let mut oldest = self.curalloc;
                        for v in 0..POLY {
                            if (self.chanmap[v] & chanmask) == chanfind && !self.voices[v].gate && self.allocpos[v] < oldest {
                                oldest = self.allocpos[v];
                                usevoice = v as i32;
                            }
                        }
                    }
                    if usevoice < 0 {
                        let mut oldest = self.curalloc;
                        for v in 0..POLY {
                            if (self.chanmap[v] & chanmask) == chanfind && self.allocpos[v] < oldest {
                                oldest = self.allocpos[v];
                                usevoice = v as i32;
                            }
                        }
                    }
                    if usevoice >= 0 {
                        let u = usevoice as usize;
                        self.chanmap[u] = chan as i32;
                        self.voicemap[chan] = usevoice;
                        self.allocpos[u] = self.curalloc;
                        self.curalloc = self.curalloc.wrapping_add(1);
                        self.store_voice(u);
                        self.voices[u].note_on(i32::from(at(i)), i32::from(at(i + 1)), &self.inst);
                    }
                    i += 2;
                }
                0 => {
                    if chan == CHANS - 1 {
                        self.ronan.wait4off = false;
                    }
                    let note = i32::from(at(i));
                    for v in 0..POLY {
                        if self.chanmap[v] == chan as i32 && self.voices[v].note == note && self.voices[v].gate {
                            self.voices[v].gate = false;
                        }
                    }
                    i += 2;
                }
                2 => i += 1,
                3 => {
                    let (ctrl, val) = (at(i), at(i + 1));
                    if (1..=7).contains(&ctrl) {
                        self.ctl[chan][usize::from(ctrl) - 1] = val;
                        if chan == CHANS - 1 {
                            self.ronan.set_ctl(u32::from(ctrl), u32::from(val), &rg);
                        }
                    } else if ctrl == 120 {
                        for v in 0..POLY {
                            if self.chanmap[v] == chan as i32 {
                                self.voices[v].init(&mut self.rand);
                                self.chanmap[v] = -1;
                            }
                        }
                    } else if ctrl == 123 {
                        if chan == CHANS - 1 {
                            self.ronan.wait4off = false;
                        }
                        for v in 0..POLY {
                            if self.chanmap[v] == chan as i32 {
                                self.voices[v].gate = false;
                            }
                        }
                    }
                    i += 2;
                }
                4 => {
                    let pgm = at(i) & 0x7f;
                    i += 1;
                    if self.pgm[chan] != pgm {
                        self.pgm[chan] = pgm;
                        for v in 0..POLY {
                            if self.chanmap[v] == chan as i32 {
                                self.chanmap[v] = -1;
                            }
                        }
                    }
                    for c in 0..6 {
                        self.ctl[chan][c] = 0;
                    }
                }
                5 | 6 => i += 2,
                _ => {
                    // System messages: V2 starts over on a reset; the player never sends one.
                    if chan == 0xf {
                        self.init();
                    }
                    break;
                }
            }
        }
    }

    /// The frame's settings: voices' and channels' parameters, envelopes and LFOs.
    fn tick_settings(&mut self) {
        for v in 0..POLY {
            if self.chanmap[v] < 0 {
                continue;
            }
            self.store_voice(v);
            self.voices[v].tick(&self.inst);
            if self.voices[v].env[0].state == ENV_OFF {
                self.chanmap[v] = -1;
            }
        }
        for c in 0..CHANS {
            self.store_chan(c);
        }
        let rg = self.rg.clone();
        self.ronan.tick(&rg);
    }

    /// One frame: its settings, then its sound (V2's tick and renderFrame).
    #[cfg(test)]
    fn frame(&mut self) {
        self.tick_settings();
        self.render_frame();
    }

    fn render_frame(&mut self) {
        let n = FRAME;
        let rg = self.rg.clone();
        self.mix.iter_mut().for_each(|v| *v = 0.0);
        self.aux1.iter_mut().for_each(|v| *v = 0.0);
        self.aux2.iter_mut().for_each(|v| *v = 0.0);
        self.auxa.iter_mut().for_each(|v| *v = 0.0);
        self.auxb.iter_mut().for_each(|v| *v = 0.0);
        let mut sends: Vec<Sent> = Vec::new();
        for chan in 0..CHANS {
            if !self.chanmap.contains(&(chan as i32)) {
                continue;
            }
            self.chanbuf.iter_mut().for_each(|v| *v = 0.0);
            for v in 0..POLY {
                if self.chanmap[v] != chan as i32 {
                    continue;
                }
                let aux = AuxIn { a: &self.auxa, b: &self.auxb };
                self.voices[v].render(&mut self.chanbuf, n, &mut self.vcebuf, &mut self.vcebuf2, &aux, &self.inst);
            }
            if chan == CHANS - 1 {
                self.ronan.process(&mut self.chanbuf, n, &rg);
            }
            // The channel's processing (V2Chan::process).
            let ch = &mut self.chans[chan];
            let cb = &mut self.chanbuf;
            accumulate(cb, &self.auxa, n, ch.aarcv);
            accumulate(cb, &self.auxb, n, ch.abrcv);
            ch.dcf1.render_stereo(cb, n, self.inst.dcfilter);
            ch.comp.render(cb, n, &mut self.levelbuf, None);
            ch.boost.render(cb, n);
            if ch.fxr == 0 {
                ch.dist.render_stereo(cb, n);
                ch.dcf2.render_stereo(cb, n, self.inst.dcfilter);
                ch.chorus.render_chan(cb, n);
            } else {
                ch.chorus.render_chan(cb, n);
                ch.dist.render_stereo(cb, n);
                ch.dcf2.render_stereo(cb, n, self.inst.dcfilter);
            }
            accumulate_mono(&mut self.aux1, cb, n, ch.a1gain);
            accumulate_mono(&mut self.aux2, cb, n, ch.a2gain);
            accumulate(&mut self.auxa, cb, n, ch.aasnd);
            accumulate(&mut self.auxb, cb, n, ch.absnd);
            accumulate(&mut self.mix, cb, n, ch.chgain);
            // What this channel puts into the mix and sends to the reverb and delay.
            let mut direct = vec![0.0f32; 2 * n];
            accumulate(&mut direct, cb, n, ch.chgain);
            let mut s1 = vec![0.0f32; n];
            accumulate_mono(&mut s1, cb, n, ch.a1gain);
            let mut s2 = vec![0.0f32; n];
            accumulate_mono(&mut s2, cb, n, ch.a2gain);
            sends.push((chan, direct, s1, s2));
            self.last = chan;
        }
        // The mix's effects (V2 synth's renderFrame).
        let mix = &mut self.mix;
        self.reverb.render(mix, &self.aux1, n);
        self.delay.render_aux2main(mix, &self.aux2, n);
        self.dcf.render_stereo(mix, n, self.inst.dcfilter);
        let (lcf, hcf) = (self.lcfreq, self.hcfreq);
        for i in 0..n {
            for ch in 0..2 {
                let mut x = mix[2 * i + ch] - self.lcbuf[ch];
                self.lcbuf[ch] += lcf * x;
                if hcf != 1.0 {
                    self.hcbuf[ch] += hcf * (x - self.hcbuf[ch]);
                    x = self.hcbuf[ch];
                }
                mix[2 * i + ch] = x;
            }
        }
        let compressing = self.compr.mode & COMP_OFF == 0;
        self.compr.render(mix, n, &mut self.levelbuf, Some(&mut self.gains));
        self.share(&sends, compressing);
    }

    /// Each channel's share of the mix: its own copy of the mix's effects
    /// (the compressor's gain the mix's), and what's left in proportion.
    fn share(&mut self, sends: &[Sent], compressing: bool) {
        let n = FRAME;
        for c in 0..CHANS {
            self.out[c].iter_mut().for_each(|v| *v = 0.0);
        }
        for (c, _, _, _) in sends {
            if self.shares[*c].is_none() {
                self.shares[*c] = Some(Box::new(Share {
                    reverb: {
                        let mut r = self.reverb.clone();
                        r.reset();
                        r
                    },
                    delay: {
                        let mut d = self.delay.clone();
                        d.db.iter_mut().for_each(|b| b.iter_mut().for_each(|v| *v = 0.0));
                        d
                    },
                    dcf: DcFilter::default(),
                    lcbuf: [0.0; 2],
                    hcbuf: [0.0; 2],
                    dbuf: vec![0.0; COMPDLEN * 2],
                    dbcnt: self.compr.dbcnt,
                }));
            }
        }
        let zero = vec![0.0f32; 2 * n];
        let zero1 = vec![0.0f32; n];
        let (lcf, hcf, r) = (self.lcfreq, self.hcfreq, self.inst.dcfilter);
        let (invol, outvol, dblen) = (self.compr.invol, self.compr.outvol, self.compr.dblen);
        for c in 0..CHANS {
            let Some(sh) = self.shares[c].as_mut() else { continue };
            let sent = sends.iter().find(|s| s.0 == c);
            let (direct, s1, s2) = match sent {
                Some((_, d, a, b)) => (d, a, b),
                None => (&zero, &zero1, &zero1),
            };
            let out = &mut self.out[c];
            out.copy_from_slice(direct);
            sh.reverb.render(out, s1, n);
            sh.delay.render_aux2main(out, s2, n);
            sh.dcf.render_stereo(out, n, r);
            for i in 0..n {
                for ch in 0..2 {
                    let mut x = out[2 * i + ch] - sh.lcbuf[ch];
                    sh.lcbuf[ch] += lcf * x;
                    if hcf != 1.0 {
                        sh.hcbuf[ch] += hcf * (x - sh.hcbuf[ch]);
                        x = sh.hcbuf[ch];
                    }
                    out[2 * i + ch] = x;
                }
            }
            if compressing {
                for ch in 0..2 {
                    let mut dbind = sh.dbcnt;
                    for i in 0..n {
                        let v = outvol * sh.dbuf[dbind as usize * 2 + ch];
                        sh.dbuf[dbind as usize * 2 + ch] = invol * out[2 * i + ch];
                        dbind += 1;
                        if dbind >= dblen {
                            dbind = 0;
                        }
                        out[2 * i + ch] = v * self.gains[2 * i + ch];
                    }
                    if ch == 1 {
                        sh.dbcnt = dbind;
                    }
                }
            }
        }
        // What the shares don't make of the mix, in proportion.
        for k in 0..2 * n {
            let (mut sum, mut weight) = (0f32, 0f32);
            for c in 0..CHANS {
                sum += self.out[c][k];
                weight += self.out[c][k].abs();
            }
            let rest = self.mix[k] - sum;
            if rest == 0.0 {
                continue;
            }
            if weight > 0.0 {
                for c in 0..CHANS {
                    let w = self.out[c][k].abs() / weight;
                    self.out[c][k] += rest * w;
                }
            } else {
                self.out[self.last][k] += rest;
            }
        }
    }
}

// ---------------------------------------------------------------------
// The V2M file.

/// The patch parameters (89) and globals (23): the synth version each came in, and its default.
#[rustfmt::skip]
const PARM_VERSION: [u8; 89] = [
    0, 2, 0, 2, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 5, 0, 0, 0, 6, 6, 6, 6,
    0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0,
];
#[rustfmt::skip]
const PARM_INIT: [u8; 89] = [
    64, 64, 1, 0, 64, 64, 0, 127, 0, 0, 64, 64, 32, 127, 0, 0, 64, 64, 32, 127, 1, 127, 0, 0, 64, 0,
    0, 64, 0, 32, 0, 64, 0, 64, 127, 64, 80, 0, 0, 64, 127, 64, 80, 64, 1, 1, 0, 64, 2, 0, 0, 1, 1,
    0, 64, 2, 0, 127, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 32, 100, 64, 64, 64, 32, 32, 0, 0, 64, 0, 0,
    1, 2, 90, 32, 20, 64, 64, 1,
];
const GPARM_VERSION: [u8; 23] = [0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 6];
const GPARM_INIT: [u8; 23] = [64, 64, 32, 127, 100, 80, 64, 64, 0, 0, 64, 0, 127, 0, 0, 1, 2, 90, 32, 20, 64, 64, 0];

fn le32(d: &[u8], at: usize) -> Option<u32> {
    d.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// The newest synth version, and each version's patch and globals sizes.
fn versions() -> (usize, Vec<usize>, Vec<usize>) {
    let newest = usize::from(*PARM_VERSION.iter().chain(GPARM_VERSION.iter()).max().unwrap_or(&0));
    let mut vs = vec![0usize; newest + 1];
    let mut gs = vec![0usize; newest + 1];
    for &v in &PARM_VERSION {
        vs[usize::from(v)..].iter_mut().for_each(|n| *n += 1);
    }
    for &v in &GPARM_VERSION {
        gs[usize::from(v)..].iter_mut().for_each(|n| *n += 1);
    }
    for v in vs.iter_mut() {
        *v += 1 + 255 * 3;
    }
    (newest, vs, gs)
}

#[derive(Clone, Copy, Default, Debug)]
struct ChanLayout {
    notenum: usize,
    noteptr: usize,
    pcnum: usize,
    pcptr: usize,
    pbnum: usize,
    pbptr: usize,
    ctl: [(usize, usize); 7],
}

/// A V2M's layout: its sequence, globals, patches and lyrics, by offset.
#[derive(Clone, Debug, Default)]
struct Layout {
    timediv: u32,
    maxtime: u32,
    gdnum: usize,
    gptr: usize,
    chan: [ChanLayout; 16],
    midisize: usize,
    globsize: usize,
    globals: usize,
    patchsize: usize,
    patchmap: usize,
    speech: Option<(usize, usize)>,
}

fn read_layout(d: &[u8]) -> Option<Layout> {
    let mut l = Layout { timediv: le32(d, 0)?, maxtime: le32(d, 4)?, gdnum: le32(d, 8)? as usize, ..Default::default() };
    let mut p = 12;
    l.gptr = p;
    p = p.checked_add(l.gdnum.checked_mul(10)?)?;
    for ch in 0..16 {
        let c = &mut l.chan[ch];
        c.notenum = le32(d, p)? as usize;
        p += 4;
        if c.notenum > 0 {
            c.noteptr = p;
            p = p.checked_add(c.notenum.checked_mul(5)?)?;
            c.pcnum = le32(d, p)? as usize;
            p += 4;
            c.pcptr = p;
            p = p.checked_add(c.pcnum.checked_mul(4)?)?;
            c.pbnum = le32(d, p)? as usize;
            p += 4;
            c.pbptr = p;
            p = p.checked_add(c.pbnum.checked_mul(5)?)?;
            for cc in c.ctl.iter_mut() {
                cc.0 = le32(d, p)? as usize;
                p += 4;
                cc.1 = p;
                p = p.checked_add(cc.0.checked_mul(4)?)?;
            }
        }
    }
    l.midisize = p;
    l.globsize = le32(d, p)? as usize;
    if l.globsize > 131072 {
        return None;
    }
    p += 4;
    l.globals = p;
    p += l.globsize;
    l.patchsize = le32(d, p)? as usize;
    if l.patchsize > 1048576 {
        return None;
    }
    p += 4;
    l.patchmap = p;
    p += l.patchsize;
    if p > d.len() {
        return None;
    }
    if p < d.len() {
        // The lyrics may run past the file's end (V2 reads on): what's there is taken.
        let size = le32(d, p).unwrap_or(0) as usize;
        if size > 0 && size < 8192 {
            l.speech = Some((p + 4, size));
        }
    }
    Some(l)
}

/// The highest program the song plays, plus one (V2's CheckV2MVersion counts them so).
fn patches_used(d: &[u8], l: &Layout) -> usize {
    let mut maxp = 0usize;
    for ch in 0..16 {
        let c = &l.chan[ch];
        let (pcn, nn) = (c.pcnum, c.notenum);
        let at = |base: usize, i: usize| usize::from(d.get(base + i).copied().unwrap_or(0));
        let (mut p, mut pct, mut np, mut nt) = (0u8, 0i64, 0usize, 0i64);
        for pcp in 0..pcn {
            let td = 0x10000 * at(c.pcptr, 2 * pcn + pcp) + 0x100 * at(c.pcptr, pcn + pcp) + at(c.pcptr, pcp);
            pct += td as i64;
            if pct > nt && usize::from(p) >= maxp {
                maxp = usize::from(p) + 1;
            }
            p = p.wrapping_add(at(c.pcptr, 3 * pcn + pcp) as u8);
            while np < nn && nt <= pct {
                let td = 0x10000 * at(c.noteptr, 2 * nn + np) + 0x100 * at(c.noteptr, nn + np) + at(c.noteptr, np);
                nt += td as i64;
                np += 1;
            }
        }
        if np < nn && usize::from(p) >= maxp {
            maxp = usize::from(p) + 1;
        }
    }
    maxp
}

/// How many versions older than the newest the file's patches are (V2's CheckV2MVersion).
fn version_delta(d: &[u8], l: &Layout) -> Option<usize> {
    if l.patchsize == 0 {
        return None;
    }
    let maxp = patches_used(d, l);
    if maxp == 0 {
        return None;
    }
    let (newest, vs, gs) = versions();
    let poff = |p: usize| le32(d, l.patchmap + 4 * p).map(|v| v as i32 as i64);
    let mut best = None;
    for i in 0..=newest {
        if l.globsize != gs[i] {
            continue;
        }
        let ok = (0..maxp - 1).all(|p| {
            let (Some(a), Some(b)) = (poff(p), poff(p + 1)) else { return false };
            let dd = (b - a) - (vs[i] as i64 - 3 * 255);
            if dd % 3 != 0 {
                return false;
            }
            let at = l.patchmap as i64 + a + vs[i] as i64 - 3 * 255 - 1;
            usize::try_from(at).ok().and_then(|at| d.get(at)).is_some_and(|&m| i64::from(m) == dd / 3)
        });
        if ok {
            best = Some(i);
        }
    }
    best.map(|b| newest - b)
}

/// Brings an older V2M up to the newest synth's patches (V2's ConvertV2M).
fn convert(d: &[u8]) -> Option<Vec<u8>> {
    let l = read_layout(d)?;
    let vdelta = version_delta(d, &l)?;
    if vdelta == 0 {
        let mut out = d.to_vec();
        out.extend([0; 4]);
        return Some(out);
    }
    let (newest, vs, gs) = versions();
    let from = newest - vdelta;
    let maxp = le32(d, l.patchmap)? as usize / 4;
    let gdiff = gs[newest] - gs[from];
    let pdiff = vs[newest] - vs[from];
    let mut out = Vec::with_capacity(d.len() + gdiff + maxp * pdiff + 4);
    out.extend_from_slice(d.get(..l.midisize)?);
    out.extend((GPARM_INIT.len() as u32).to_le_bytes());
    let mut old = d.get(l.globals..)?.iter();
    for i in 0..GPARM_INIT.len() {
        out.push(if usize::from(GPARM_VERSION[i]) <= from { *old.next().unwrap_or(&0) } else { GPARM_INIT[i] });
    }
    out.extend(((l.patchsize + maxp * pdiff) as u32).to_le_bytes());
    for p in 0..maxp {
        out.extend(((le32(d, l.patchmap + 4 * p)? as usize + p * pdiff) as u32).to_le_bytes());
    }
    for p in 0..maxp {
        let mut src = l.patchmap + le32(d, l.patchmap + 4 * p)? as usize;
        let mut next = || {
            let b = d.get(src).copied().unwrap_or(0);
            src += 1;
            b
        };
        for i in 0..PARM_INIT.len() {
            out.push(if usize::from(PARM_VERSION[i]) <= from { next() } else { PARM_INIT[i] });
        }
        let modnum = next();
        out.push(modnum);
        for _ in 0..modnum {
            let (s, v) = (next(), next());
            let mut dest = next();
            let mut k = 0;
            while k <= usize::from(dest) && k < PARM_VERSION.len() {
                if usize::from(PARM_VERSION[k]) > from {
                    dest = dest.wrapping_add(1);
                }
                k += 1;
            }
            out.extend([s, v, dest]);
        }
    }
    match l.speech {
        Some((at, size)) => {
            out.extend((size as u32).to_le_bytes());
            let end = out.len() + size;
            out.extend_from_slice(d.get(at..(at + size).min(d.len())).unwrap_or(&[]));
            out.resize(end, 0);
        }
        None => out.extend([0; 4]),
    }
    Some(out)
}

#[derive(Debug)]
struct Data {
    file: Arc<[u8]>,
    /// The file brought up to the newest synth version.
    v2m: Arc<[u8]>,
    layout: Layout,
    patchmap: Arc<[u8]>,
    globals: Vec<u8>,
    texts: Vec<Option<Arc<[u8]>>>,
    patches: usize,
    version: usize,
}

/// A V2M song.
#[derive(Clone, Debug)]
pub struct Song(Arc<Data>);

pub fn probe(d: &[u8]) -> bool {
    // A V2M has no tag: its layout and patches have to make sense.
    read_layout(d).is_some_and(|l| l.timediv > 0 && l.timediv < 1 << 16 && l.gdnum < 1 << 16 && version_delta(d, &l).is_some())
}

/// A note, a program change or a controller at its time in the song's ticks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    /// A note and its velocity (0: off).
    Note(u8, u8),
    Program(u8),
}

impl Song {
    pub fn parse(file: &[u8]) -> Option<Song> {
        if !probe(file) {
            return None;
        }
        let l0 = read_layout(file)?;
        let version = version_delta(file, &l0)?;
        let v2m = convert(file)?;
        let layout = read_layout(&v2m)?;
        let patchmap: Arc<[u8]> = v2m.get(layout.patchmap..layout.patchmap + layout.patchsize)?.into();
        let globals = v2m.get(layout.globals..layout.globals + layout.globsize)?.to_vec();
        let mut texts = vec![None; 64];
        if let Some((at, size)) = layout.speech {
            let mut sp = v2m.get(at..(at + size).min(v2m.len())).unwrap_or(&[]).to_vec();
            sp.resize(size, 0);
            let n = le32(&sp, 0).unwrap_or(0) as usize;
            for (i, t) in texts.iter_mut().enumerate().take(n.min(64)) {
                let off = le32(&sp, 4 + 4 * i).unwrap_or(u32::MAX) as usize;
                if let Some(s) = sp.get(off..) {
                    let end = s.iter().position(|&c| c == 0).unwrap_or(s.len());
                    *t = Some(s[..end].into());
                }
            }
        } else {
            for t in texts.iter_mut() {
                *t = Some(b" ".as_slice().into());
            }
        }
        let patches = le32(&patchmap, 0).map_or(0, |o| o as usize / 4);
        Some(Song(Arc::new(Data { file: file.into(), v2m: v2m.into(), layout, patchmap, globals, texts, patches, version })))
    }

    /// How many synth versions older than V2's last the file was made with.
    pub fn version(&self) -> usize {
        self.0.version
    }
    /// How many patches the song has.
    pub fn patches(&self) -> usize {
        self.0.patches
    }
    /// The song's ticks a quarter note, and its length in ticks.
    pub fn timing(&self) -> (u32, u32) {
        (self.0.layout.timediv, self.0.layout.maxtime)
    }
    /// The song's lyrics (for the speech synth), those it has.
    pub fn lyrics(&self) -> Vec<String> {
        if self.0.layout.speech.is_none() {
            return Vec::new();
        }
        self.0.texts.iter().flatten().map(|t| t.iter().map(|&c| c as char).collect()).collect()
    }

    /// A channel's notes and program changes, each at its time in ticks.
    pub fn events(&self, ch: usize) -> Vec<(u32, Event)> {
        let d = &self.0.v2m;
        let c = &self.0.layout.chan[ch];
        let at = |base: usize, i: usize| u32::from(d.get(base + i).copied().unwrap_or(0));
        let mut out = Vec::new();
        let (mut t, mut p) = (0u32, 0u8);
        for i in 0..c.pcnum {
            t = t.wrapping_add(at(c.pcptr, i) | at(c.pcptr, c.pcnum + i) << 8 | at(c.pcptr, 2 * c.pcnum + i) << 16);
            p = p.wrapping_add(at(c.pcptr, 3 * c.pcnum + i) as u8);
            out.push((t, Event::Program(p)));
        }
        let (mut t, mut n, mut v) = (0u32, 0u8, 0u8);
        for i in 0..c.notenum {
            t = t.wrapping_add(at(c.noteptr, i) | at(c.noteptr, c.notenum + i) << 8 | at(c.noteptr, 2 * c.notenum + i) << 16);
            n = n.wrapping_add(at(c.noteptr, 3 * c.notenum + i) as u8);
            v = v.wrapping_add(at(c.noteptr, 4 * c.notenum + i) as u8);
            out.push((t, Event::Note(n, v)));
        }
        out.sort_by_key(|e| (e.0, matches!(e.1, Event::Note(..))));
        out
    }

    /// Each tempo change: its time in ticks and microseconds a quarter note.
    pub fn tempos(&self) -> Vec<(u32, u32)> {
        let d = &self.0.v2m;
        let l = &self.0.layout;
        let g = l.gdnum;
        let at = |i: usize| u32::from(d.get(l.gptr + i).copied().unwrap_or(0));
        let mut t = 0u32;
        (0..g)
            .map(|i| {
                t = t.wrapping_add(at(i) | at(g + i) << 8 | at(2 * g + i) << 16);
                (t, le32(d, l.gptr + 3 * g + 4 * i).unwrap_or(500_000))
            })
            .collect()
    }
}

impl Tune for Song {
    fn format(&self) -> &'static str {
        "v2m"
    }
    fn file(&self) -> &[u8] {
        &self.0.file
    }
    fn driver(&self, _song: usize) -> Box<dyn Driver> {
        Box::new(Player::new(self.clone()))
    }
    fn length(&self, _song: usize) -> Option<f64> {
        let mut s = Sequencer::new(self.0.clone());
        s.reset();
        Some(s.end_sample() as f64 / f64::from(RATE))
    }

    /// Each patch the map has, its bytes as the newest synth version has
    /// them (to the next patch's start). A V2M keeps no names.
    fn patches(&self) -> Vec<Patch> {
        let d = &self.0;
        let map = &d.patchmap;
        let offsets: Vec<usize> = (0..d.patches).filter_map(|p| map.get(p * 4..p * 4 + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)).collect();
        let mut out = Vec::new();
        for (p, &at) in offsets.iter().enumerate() {
            let end = offsets.iter().copied().filter(|&o| o > at).min().unwrap_or(map.len()).min(map.len());
            let Some(bytes) = map.get(at..end).filter(|b| !b.is_empty()) else { continue };
            let mut b = PatchBytes::default();
            b.bytes(bytes);
            out.push(b.patch(p + 1, ""));
        }
        out
    }
}

// ---------------------------------------------------------------------
// The sequencer (the V2M player's timing and MIDI).

#[derive(Clone, Copy, Default)]
struct ChanState {
    noteptr: usize,
    notenr: usize,
    notent: u32,
    lastnte: u8,
    lastvel: u8,
    pcptr: usize,
    pcnr: usize,
    pcnt: u32,
    lastpc: u8,
    pbptr: usize,
    pbnr: usize,
    pbnt: u32,
    lastpb0: u8,
    lastpb1: u8,
    ctl: [(usize, u32, usize, u8); 7],
}

struct Sequencer {
    d: Arc<Data>,
    playing: bool,
    time: u32,
    nexttime: u32,
    gnt: u32,
    gnr: usize,
    usecs: u32,
    chan: [ChanState; 16],
    smpldelta: u32,
    smplrem: u32,
    /// The sample the next tick is due at, and the one the last was at.
    due: u64,
    at: u64,
    midi: Vec<u8>,
}

impl Sequencer {
    fn new(d: Arc<Data>) -> Sequencer {
        Sequencer { d, playing: false, time: 0, nexttime: u32::MAX, gnt: 0, gnr: 0, usecs: 0, chan: [ChanState::default(); 16], smpldelta: 0, smplrem: 0, due: 0, at: 0, midi: Vec::with_capacity(256) }
    }

    fn byte(&self, at: usize) -> u32 {
        u32::from(self.d.v2m.get(at).copied().unwrap_or(0))
    }

    fn delta(&self, p: usize, w: usize) -> u32 {
        self.byte(p) + (self.byte(p + w) << 8) + (self.byte(p + 2 * w) << 16)
    }

    /// UPDATENT: the next event of a list, scheduled.
    fn update(&mut self, n: usize, w: usize, p: usize) -> Option<u32> {
        if n < w {
            let v = self.time.wrapping_add(self.delta(p, w));
            if v < self.nexttime {
                self.nexttime = v;
            }
            Some(v)
        } else {
            None
        }
    }

    /// UPDATENT2: the next event's time, when it's later.
    fn update2(&self, n: usize, w: usize, p: usize, v: &mut u32) {
        if n < w && self.delta(p, w) != 0 {
            *v = self.time.wrapping_add(self.delta(p, w));
        }
    }

    /// UPDATENT3: scheduled.
    fn update3(&mut self, n: usize, w: usize, v: u32) {
        if n < w && v < self.nexttime {
            self.nexttime = v;
        }
    }

    fn reset(&mut self) {
        let l = self.d.layout.clone();
        self.time = 0;
        self.nexttime = u32::MAX;
        self.gnr = 0;
        if let Some(v) = self.update(0, l.gdnum, l.gptr) {
            self.gnt = v;
        }
        for ch in 0..16 {
            let bc = l.chan[ch];
            if bc.notenum == 0 {
                continue;
            }
            let mut sc = ChanState { noteptr: bc.noteptr, pcptr: bc.pcptr, pbptr: bc.pbptr, ..Default::default() };
            if let Some(v) = self.update(0, bc.notenum, bc.noteptr) {
                sc.notent = v;
            }
            if let Some(v) = self.update(0, bc.pcnum, bc.pcptr) {
                sc.pcnt = v;
            }
            // V2's player times the pitch bends by the program changes' count.
            if let Some(v) = self.update(0, bc.pcnum, bc.pbptr) {
                sc.pbnt = v;
            }
            for cn in 0..7 {
                let (num, ptr) = bc.ctl[cn];
                sc.ctl[cn] = (ptr, self.update(0, num, ptr).unwrap_or(0), 0, 0);
            }
            self.chan[ch] = sc;
        }
        self.usecs = 5000 * RATE;
        self.smplrem = 0;
        self.smpldelta = 0;
        self.playing = true;
        self.due = 0;
        self.at = 0;
    }

    /// One tick of the player: the MIDI due now, into `midi`.
    fn tick(&mut self) {
        if !self.playing {
            return;
        }
        let l = self.d.layout.clone();
        self.time = self.nexttime;
        self.nexttime = u32::MAX;
        self.midi.clear();
        let mut laststat = u32::MAX;
        let mut put = |midi: &mut Vec<u8>, s: u32| {
            if laststat != s {
                laststat = s;
                midi.push(s as u8);
            }
        };
        let g = l.gdnum;
        if self.gnr < g && self.time == self.gnt {
            let tempo = le32(&self.d.v2m, l.gptr + 3 * g + 4 * self.gnr).unwrap_or(0);
            self.usecs = tempo.wrapping_mul(RATE / 100);
            self.gnr += 1;
            let mut v = self.gnt;
            self.update2(self.gnr, g, l.gptr + self.gnr, &mut v);
            self.gnt = v;
        }
        let gnt = self.gnt;
        self.update3(self.gnr, g, gnt);
        let mut midi = std::mem::take(&mut self.midi);
        for ch in 0..16 {
            let bc = l.chan[ch];
            if bc.notenum == 0 {
                continue;
            }
            let mut sc = self.chan[ch];
            if sc.pcnr < bc.pcnum && self.time == sc.pcnt {
                put(&mut midi, 0xc0 | ch as u32);
                sc.lastpc = sc.lastpc.wrapping_add(self.byte(sc.pcptr + 3 * bc.pcnum) as u8);
                midi.push(sc.lastpc);
                sc.pcnr += 1;
                sc.pcptr += 1;
                self.update2(sc.pcnr, bc.pcnum, sc.pcptr, &mut sc.pcnt);
            }
            self.update3(sc.pcnr, bc.pcnum, sc.pcnt);
            for cn in 0..7 {
                let (num, _) = bc.ctl[cn];
                let (mut ptr, mut nt, mut nr, mut last) = sc.ctl[cn];
                if nr < num && self.time == nt {
                    put(&mut midi, 0xb0 | ch as u32);
                    midi.push(cn as u8 + 1);
                    last = last.wrapping_add(self.byte(ptr + 3 * num) as u8);
                    midi.push(last);
                    nr += 1;
                    ptr += 1;
                    self.update2(nr, num, ptr, &mut nt);
                }
                self.update3(nr, num, nt);
                sc.ctl[cn] = (ptr, nt, nr, last);
            }
            if sc.pbnr < bc.pbnum && self.time == sc.pbnt {
                put(&mut midi, 0xe0 | ch as u32);
                sc.lastpb0 = sc.lastpb0.wrapping_add(self.byte(sc.pbptr + 3 * bc.pcnum) as u8);
                midi.push(sc.lastpb0);
                sc.lastpb1 = sc.lastpb1.wrapping_add(self.byte(sc.pbptr + 4 * bc.pcnum) as u8);
                midi.push(sc.lastpb1);
                sc.pbnr += 1;
                sc.pbptr += 1;
                self.update2(sc.pbnr, bc.pbnum, sc.pbptr, &mut sc.pbnt);
            }
            self.update3(sc.pbnr, bc.pbnum, sc.pbnt);
            while sc.notenr < bc.notenum && self.time == sc.notent {
                put(&mut midi, 0x90 | ch as u32);
                sc.lastnte = sc.lastnte.wrapping_add(self.byte(sc.noteptr + 3 * bc.notenum) as u8);
                midi.push(sc.lastnte);
                sc.lastvel = sc.lastvel.wrapping_add(self.byte(sc.noteptr + 4 * bc.notenum) as u8);
                midi.push(sc.lastvel);
                sc.notenr += 1;
                sc.noteptr += 1;
                self.update2(sc.notenr, bc.notenum, sc.noteptr, &mut sc.notent);
            }
            self.update3(sc.notenr, bc.notenum, sc.notent);
            self.chan[ch] = sc;
        }
        midi.push(0xfd);
        self.midi = midi;
        if self.nexttime == u32::MAX {
            self.playing = false;
        }
    }

    /// The samples to the next tick (the player's UpdateSampleDelta).
    fn schedule(&mut self) {
        if !self.playing {
            return;
        }
        let td2 = u64::from(10000u32.wrapping_mul(self.d.layout.timediv)).max(1);
        let t = u64::from(self.nexttime.wrapping_sub(self.time)) * u64::from(self.usecs);
        let mut q = (t / td2) as u32;
        let r = (t % td2) as u32;
        let old = self.smplrem;
        self.smplrem = self.smplrem.wrapping_add(r);
        if self.smplrem < old {
            q = q.wrapping_add(1);
        }
        self.smpldelta = q;
        self.at = self.due;
        self.due += u64::from(q);
    }

    /// The ticks due by `sample`, each's MIDI given to `f`.
    fn run_to(&mut self, sample: u64, mut f: impl FnMut(&[u8])) {
        while self.playing && self.due <= sample {
            self.tick();
            f(&self.midi);
            self.schedule();
        }
    }

    /// Where the song ends, in samples: its last event, or its length if later.
    fn end_sample(&mut self) -> u64 {
        self.run_to(u64::MAX, |_| {});
        let end = self.d.layout.maxtime;
        let extra = if end > self.time { u64::from(end - self.time) * u64::from(self.usecs) / u64::from(10000u32.wrapping_mul(self.d.layout.timediv)).max(1) } else { 0 };
        self.due + extra
    }
}

// ---------------------------------------------------------------------
// Playing.

/// Plays a V2M: the player and synth at 44.1 kHz, a frame a tick.
pub struct Player {
    seq: Sequencer,
    synth: Synth,
    /// The sample the next frame starts at.
    frame_at: u64,
    end: u64,
    fresh: bool,
    left: Vec<Vec<f32>>,
    right: Vec<Vec<f32>>,
    prev: Vec<(f32, f32)>,
    pos: f64,
    done: bool,
    stopped: bool,
}

impl Player {
    pub fn new(song: Song) -> Player {
        let rg = Arc::new(RonanG::new());
        let d = song.0.clone();
        let mut synth = Synth::new(d.patchmap.clone(), rg);
        let mut seq = Sequencer::new(d.clone());
        // V2's player starts the synth when it opens a song and again when it plays.
        for _ in 0..2 {
            synth.init();
            synth.set_globals(&d.globals);
            synth.set_lyrics(&d.texts);
        }
        let end = {
            let mut s = Sequencer::new(d.clone());
            s.reset();
            s.end_sample()
        };
        seq.reset();
        Player { seq, synth, frame_at: 0, end, fresh: false, left: vec![vec![0.0; FRAME]; CHANS], right: vec![vec![0.0; FRAME]; CHANS], prev: vec![(0.0, 0.0); CHANS], pos: 0.0, done: false, stopped: false }
    }

    fn advance(&mut self) {
        let synth = &mut self.synth;
        self.seq.run_to(self.frame_at, |m| synth.process_midi(m));
    }

    /// The frame's sound into the channels' buffers.
    fn make(&mut self) {
        self.synth.render_frame();
        for c in 0..CHANS {
            for i in 0..FRAME {
                self.left[c][i] = self.synth.out[c][2 * i];
                self.right[c][i] = self.synth.out[c][2 * i + 1];
            }
        }
    }

    #[cfg(test)]
    fn next_mix(&mut self) -> &[f32] {
        self.advance();
        self.synth.frame();
        self.frame_at += FRAME as u64;
        &self.synth.mix
    }
}

impl Driver for Player {
    fn channels(&self) -> usize {
        CHANS
    }

    fn tick(&mut self) {
        if self.fresh {
            // A frame passed over: its settings ran, not its sound.
            for p in self.prev.iter_mut() {
                *p = (0.0, 0.0);
            }
        } else {
            for c in 0..CHANS {
                self.prev[c] = (self.left[c][FRAME - 1], self.right[c][FRAME - 1]);
            }
        }
        self.pos = (self.pos - FRAME as f64).max(0.0);
        self.advance();
        self.synth.tick_settings();
        self.frame_at += FRAME as u64;
        self.fresh = true;
        if self.frame_at >= self.end && !self.stopped {
            self.stopped = true;
            self.done = true;
        }
    }

    fn tick_seconds(&self) -> f64 {
        FRAME as f64 / f64::from(RATE)
    }

    fn render(&mut self, rate: u32, frames: usize, out: &mut Bufs) {
        if self.fresh {
            self.make();
            self.fresh = false;
        }
        let step = f64::from(RATE) / f64::from(rate.max(1));
        for c in 0..CHANS {
            let (bl, br, prev) = (&self.left[c], &self.right[c], self.prev[c]);
            let at = |k: isize| if k < 0 { prev } else { (bl[(k as usize).min(FRAME - 1)], br[(k as usize).min(FRAME - 1)]) };
            let (l, r) = (&mut out.left[c][..frames], &mut out.right[c][..frames]);
            for i in 0..frames {
                let x = self.pos + i as f64 * step - 1.0;
                let k = x.floor();
                let f = (x - k) as f32;
                let (a, b) = (at(k as isize), at(k as isize + 1));
                l[i] = a.0 + (b.0 - a.0) * f;
                r[i] = a.1 + (b.1 - a.1) * f;
            }
        }
        self.pos += frames as f64 * step;
    }

    fn position(&self) -> (usize, usize) {
        let row = self.row();
        (row / ROWS, row % ROWS)
    }

    fn speed(&self) -> (u32, u32) {
        let tempo = self.seq.usecs / (RATE / 100).max(1);
        (6, (60_000_000 / tempo.max(1)).clamp(1, 999))
    }

    fn take_done(&mut self) -> bool {
        std::mem::take(&mut self.done)
    }

    fn playing(&self) -> bool {
        !self.stopped
    }
}

/// Rows a pattern on the screen: a row a sixteenth note.
pub const ROWS: usize = 64;

impl Player {
    /// The row the song is at: its ticks now, a sixteenth note a row.
    fn row(&self) -> usize {
        let td = self.seq.d.layout.timediv.max(1);
        let since = self.frame_at.saturating_sub(self.seq.at);
        let ticks = u64::from(self.seq.time) + since * u64::from(10000u32.wrapping_mul(td)) / u64::from(self.seq.usecs.max(1));
        (ticks * 4 / u64::from(td)) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Compares each song in `V2_DIR` with V2's own render beside it
    /// (`<song>.f32`, stereo floats, in `V2_REF`), sample for sample.
    #[test]
    #[ignore]
    fn matches_v2() {
        let (Ok(dir), Ok(refs)) = (std::env::var("V2_DIR"), std::env::var("V2_REF")) else { return };
        let mut names: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "v2m")).collect();
        names.sort();
        let (mut exact, mut total) = (0, 0);
        for p in names {
            let name = p.file_name().unwrap().to_string_lossy().to_string();
            if std::env::var("V2_ONLY").is_ok_and(|o| name != o) {
                continue;
            }
            let Ok(r) = std::fs::read(format!("{refs}/{name}.f32")) else { continue };
            let want: Vec<f32> = r.as_chunks::<4>().0.iter().map(|&c| f32::from_le_bytes(c)).collect();
            let Some(song) = Song::parse(&std::fs::read(&p).unwrap()) else {
                println!("{name}: not read");
                continue;
            };
            let mut pl = Player::new(song);
            let mut got: Vec<f32> = Vec::with_capacity(want.len() + 256);
            while got.len() < want.len() {
                let m = pl.next_mix().to_vec();
                got.extend(m);
            }
            let n = want.len();
            let bad = (0..n).filter(|&i| got[i].to_bits() != want[i].to_bits()).count();
            let first = (0..n).find(|&i| got[i].to_bits() != want[i].to_bits());
            let maxd = (0..n).map(|i| (got[i] - want[i]).abs()).fold(0f32, f32::max);
            total += 1;
            if bad == 0 {
                exact += 1;
            }
            println!("{name}: {} frames, {bad} differ, first {:?}, most {maxd:e}", n / 2, first.map(|i| (i / 2, got[i], want[i])));
        }
        println!("{exact} of {total} exact");
    }
}
