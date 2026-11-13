// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.
// Copied from bgn/src/dsp.rs, Neumetik BGN (bgn/, ansiapps' own), by synth/scripts/copies.py:
// change it there, then run the script with --update.

//! The small pieces every BGN generator is built from: a random number
//! generator, filters, resonators, envelopes, a delay line, panning and
//! the event voices (bursts, chirps, bubbles, modes, formant voices)
//! that details and textures are made of.

use std::f32::consts::{FRAC_PI_4, PI, TAU};

/// Samples in a block: parameters and coefficients are worked out once
/// a block, sounds are made a sample at a time.
pub const BLOCK: usize = 64;

/// A block of stereo output that generators add into.
pub struct Block {
    pub l: [f32; BLOCK],
    pub r: [f32; BLOCK],
}

impl Block {
    pub fn new() -> Block {
        Block { l: [0.0; BLOCK], r: [0.0; BLOCK] }
    }

    pub fn clear(&mut self) {
        self.l = [0.0; BLOCK];
        self.r = [0.0; BLOCK];
    }

    #[inline]
    pub fn add(&mut self, i: usize, (l, r): (f32, f32)) {
        self.l[i] += l;
        self.r[i] += r;
    }
}

/// xorshift64* seeded through SplitMix64: fast, good enough for audio,
/// and the same sound from the same seed every time.
#[derive(Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u32) -> Rng {
        let mut z = u64::from(seed).wrapping_add(0x9e37_79b9_7f4a_7c15);
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        Rng((z ^ (z >> 31)) | 1)
    }

    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }

    /// Uniform in [0, 1).
    #[inline]
    pub fn uni(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 * (1.0 / (1u64 << 24) as f32)
    }

    /// Uniform in [-1, 1).
    #[inline]
    pub fn bi(&mut self) -> f32 {
        self.uni() * 2.0 - 1.0
    }

    #[inline]
    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.uni()
    }

    /// Normal, mean 0 and deviation 1 (Box-Muller).
    pub fn gauss(&mut self) -> f32 {
        let u = self.uni().max(1e-7);
        let v = self.uni();
        (-2.0 * u.ln()).sqrt() * (TAU * v).cos()
    }

    #[inline]
    pub fn chance(&mut self, p: f32) -> bool {
        self.uni() < p
    }

    /// Seconds to the next event of a Poisson process `rate` times a second.
    pub fn interval(&mut self, rate: f32) -> f32 {
        -(1.0 - self.uni()).ln() / rate.max(1e-6)
    }

    pub fn below(&mut self, n: usize) -> usize {
        ((self.uni() * n as f32) as usize).min(n.saturating_sub(1))
    }
}

/// Fires at random, `rate` times a second on average (a Poisson process),
/// or nearly on a beat when `regular` is near 1.
pub struct Ticker {
    left: f32,
}

impl Ticker {
    pub fn new() -> Ticker {
        Ticker { left: 0.0 }
    }

    /// Starts the first wait part way through, so a scene doesn't open on
    /// every detail at once.
    pub fn primed(rng: &mut Rng, rate: f32) -> Ticker {
        Ticker { left: rng.interval(rate) }
    }

    #[inline]
    pub fn fire(&mut self, rng: &mut Rng, rate: f32, regular: f32, dt: f32) -> bool {
        self.left -= dt;
        if self.left > 0.0 {
            return false;
        }
        let random = rng.interval(rate);
        let beat = 1.0 / rate.max(1e-6);
        self.left += (random * (1.0 - regular) + beat * regular * rng.range(0.97, 1.03)).max(0.0);
        // A rate turned up from very low shouldn't wait out the old interval.
        if self.left > 4.0 * beat {
            self.left = rng.interval(rate);
        }
        true
    }
}

/// Cytomic's trapezoidal state-variable filter: low, band and high from
/// the same two states, stable at any cutoff.
#[derive(Clone, Copy, Default)]
pub struct Svf {
    ic1: f32,
    ic2: f32,
    a1: f32,
    a2: f32,
    a3: f32,
    k: f32,
}

impl Svf {
    pub fn new(cutoff: f32, q: f32, rate: f32) -> Svf {
        let mut f = Svf::default();
        f.set(cutoff, q, rate);
        f
    }

    /// `q` from 0.5 (gentle) up; 0.707 is Butterworth.
    pub fn set(&mut self, cutoff: f32, q: f32, rate: f32) {
        let g = (PI * cutoff.clamp(10.0, rate * 0.49) / rate).tan();
        let k = 1.0 / q.max(0.05);
        self.a1 = 1.0 / (1.0 + g * (g + k));
        self.a2 = g * self.a1;
        self.a3 = g * self.a2;
        self.k = k;
    }

    /// Low, band and high pass.
    #[inline]
    pub fn tick(&mut self, v0: f32) -> (f32, f32, f32) {
        let v3 = v0 - self.ic2;
        let v1 = self.a1 * self.ic1 + self.a2 * v3;
        let v2 = self.ic2 + self.a2 * self.ic1 + self.a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        (v2, v1, v0 - self.k * v1 - v2)
    }

    #[inline]
    pub fn low(&mut self, x: f32) -> f32 {
        self.tick(x).0
    }

    /// Band pass with unity gain at the center.
    #[inline]
    pub fn band(&mut self, x: f32) -> f32 {
        self.tick(x).1 * self.k
    }

    #[inline]
    pub fn high(&mut self, x: f32) -> f32 {
        self.tick(x).2
    }
}

/// A one-pole low pass; `x - low` is its high pass.
#[derive(Clone, Copy, Default)]
pub struct OnePole {
    y: f32,
    a: f32,
}

impl OnePole {
    pub fn new(cutoff: f32, rate: f32) -> OnePole {
        let mut f = OnePole::default();
        f.set(cutoff, rate);
        f
    }

    pub fn set(&mut self, cutoff: f32, rate: f32) {
        self.a = 1.0 - (-TAU * cutoff.clamp(0.01, rate * 0.49) / rate).exp();
    }

    #[inline]
    pub fn low(&mut self, x: f32) -> f32 {
        self.y += self.a * (x - self.y);
        self.y
    }

    #[inline]
    pub fn high(&mut self, x: f32) -> f32 {
        x - self.low(x)
    }
}

/// A two-pole resonator: rung by an impulse, a decaying sine. Modal
/// synthesis is a few of these at an object's own frequencies.
#[derive(Clone, Copy, Default)]
pub struct Mode {
    y1: f32,
    y2: f32,
    b1: f32,
    b2: f32,
    g: f32,
}

impl Mode {
    pub fn set(&mut self, freq: f32, decay: f32, rate: f32) {
        let r = (-1.0 / (decay.max(0.0005) * rate)).exp();
        let w = TAU * freq.clamp(10.0, rate * 0.45) / rate;
        self.b1 = 2.0 * r * w.cos();
        self.b2 = -r * r;
        // An impulse rings it at the same height whatever its pitch.
        self.g = 0.15 * w.sin();
    }

    #[inline]
    pub fn tick(&mut self, x: f32) -> f32 {
        let y = self.b1 * self.y1 + self.b2 * self.y2 + x * self.g;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }

    pub fn quiet(&self) -> bool {
        self.y1.abs() + self.y2.abs() < 1e-5
    }
}

/// A value that moves toward its target smoothly, so a dragged slider
/// doesn't click.
#[derive(Clone, Copy)]
pub struct Smooth {
    pub value: f32,
}

impl Smooth {
    /// Moves a block's worth toward `target`, about 40 ms to settle.
    #[inline]
    pub fn step(&mut self, target: f32) -> f32 {
        self.value += (target - self.value) * 0.18;
        if (target - self.value).abs() < 1e-5 {
            self.value = target;
        }
        self.value
    }
}

/// A delay line read at any fraction of a sample (for Doppler shifts
/// and chorus).
pub struct Delay {
    line: Vec<f32>,
    at: usize,
}

impl Delay {
    pub fn new(len: usize) -> Delay {
        Delay { line: vec![0.0; len.max(4)], at: 0 }
    }

    #[inline]
    pub fn push(&mut self, x: f32) {
        self.at = (self.at + 1) % self.line.len();
        self.line[self.at] = x;
    }

    /// `delay` samples ago, between 1 and the line's length.
    #[inline]
    pub fn read(&self, delay: f32) -> f32 {
        let n = self.line.len();
        let d = delay.clamp(1.0, (n - 2) as f32);
        let i = d as usize;
        let f = d - i as f32;
        let a = self.line[(self.at + n - i) % n];
        let b = self.line[(self.at + n - i - 1) % n];
        a + (b - a) * f
    }
}

/// Equal-power pan, -1 left to 1 right.
#[inline]
pub fn pan(x: f32, p: f32) -> (f32, f32) {
    let a = (p.clamp(-1.0, 1.0) + 1.0) * FRAC_PI_4;
    (x * a.cos() * 1.414, x * a.sin() * 1.414)
}

/// Decibels to gain.
#[inline]
pub fn db(d: f32) -> f32 {
    10f32.powf(d / 20.0)
}

/// A sine by a phase in turns (0..1).
#[inline]
pub fn sin1(phase: f32) -> f32 {
    (phase * TAU).sin()
}

/// A soft clip that leaves quiet signals alone.
#[inline]
pub fn soft(x: f32) -> f32 {
    if x.abs() < 0.6 {
        x
    } else {
        x.signum() * (0.6 + 0.4 * ((x.abs() - 0.6) / 0.4).tanh())
    }
}

/// A decaying envelope with a short attack.
#[derive(Clone, Copy, Default)]
pub struct Env {
    v: f32,
    up: f32,
    k: f32,
    rising: bool,
}

impl Env {
    pub fn start(&mut self, attack: f32, decay: f32, rate: f32) {
        self.up = 1.0 / (attack.max(0.0001) * rate);
        self.k = (-6.9 / (decay.max(0.001) * rate)).exp();
        self.rising = true;
        self.v = 0.0;
    }

    #[inline]
    pub fn tick(&mut self) -> f32 {
        if self.rising {
            self.v += self.up;
            if self.v >= 1.0 {
                self.v = 1.0;
                self.rising = false;
            }
        } else {
            self.v *= self.k;
        }
        self.v
    }

    pub fn live(&self) -> bool {
        self.rising || self.v > 0.0005
    }
}

/// Something that sounds for a while and then is free for another event.
pub trait Live {
    fn live(&self) -> bool;
}

/// The first free voice, or None when all are sounding.
pub fn free<V: Live>(voices: &mut [V]) -> Option<&mut V> {
    voices.iter_mut().find(|v| !v.live())
}

/// A burst of band-passed noise: a raindrop's tick, a footstep, a clap,
/// a gunshot, a crackle's pop.
#[derive(Clone, Copy, Default)]
pub struct Burst {
    env: Env,
    filter: Svf,
    gain: f32,
    pan: f32,
    sweep: f32,
    freq: f32,
    q: f32,
}

impl Live for Burst {
    fn live(&self) -> bool {
        self.env.live()
    }
}

impl Burst {
    #[allow(clippy::too_many_arguments)]
    pub fn start(&mut self, freq: f32, q: f32, attack: f32, decay: f32, gain: f32, pan: f32, rate: f32) {
        self.env.start(attack, decay, rate);
        self.filter = Svf::new(freq, q, rate);
        self.freq = freq;
        self.q = q;
        self.sweep = 1.0;
        self.gain = gain;
        self.pan = pan;
    }

    #[inline]
    pub fn tick(&mut self, rng: &mut Rng, rate: f32) -> (f32, f32) {
        if !self.env.live() {
            return (0.0, 0.0);
        }
        if self.sweep != 1.0 {
            self.freq *= self.sweep;
            self.filter.set(self.freq, self.q, rate);
        }
        let x = self.filter.band(rng.bi()) * self.env.tick() * self.gain;
        pan(x, self.pan)
    }
}

/// A sine whose pitch and loudness follow short curves: a bird's note, a
/// cricket's pulse, a beep, a sparkle.
#[derive(Clone, Copy, Default)]
pub struct Chirp {
    phase: f32,
    freq: f32,
    glide: f32,
    t: f32,
    len: f32,
    gain: f32,
    pan: f32,
    /// Vibrato or trill: depth (a ratio) and rate.
    fm: f32,
    fm_rate: f32,
    fm_phase: f32,
    /// Amplitude pulses (a cricket's wing strokes) a second; 0 for none.
    am_rate: f32,
    /// Harmonics mixed in, 0 to 1.
    harm: f32,
    /// Ring-modulated by noise, 0 to 1: a caw's rasp.
    rough: f32,
    noise: u32,
}

impl Live for Chirp {
    fn live(&self) -> bool {
        self.t < self.len
    }
}

impl Chirp {
    /// A note from `from` to `to` Hz over `len` seconds.
    pub fn start(&mut self, from: f32, to: f32, len: f32, gain: f32, pan: f32, rate: f32) {
        self.phase = 0.0;
        self.freq = from;
        self.glide = (to / from).max(0.01).powf(1.0 / (len.max(0.001) * rate));
        self.t = 0.0;
        self.len = len;
        self.gain = gain;
        self.pan = pan;
        self.fm = 0.0;
        self.fm_rate = 0.0;
        self.am_rate = 0.0;
        self.harm = 0.0;
        self.rough = 0.0;
        self.noise = self.noise.wrapping_mul(747_796_405).wrapping_add(2_891_336_453) | 1;
    }

    pub fn rough(&mut self, r: f32) {
        self.rough = r;
    }

    pub fn vibrato(&mut self, depth: f32, rate: f32) {
        self.fm = depth;
        self.fm_rate = rate;
    }

    pub fn pulses(&mut self, rate: f32) {
        self.am_rate = rate;
    }

    pub fn harmonics(&mut self, h: f32) {
        self.harm = h;
    }

    #[inline]
    pub fn tick(&mut self, rate: f32) -> (f32, f32) {
        if self.t >= self.len {
            return (0.0, 0.0);
        }
        let dt = 1.0 / rate;
        let u = self.t / self.len;
        // A quick rise and a fall that fits any length.
        let mut env = (u * 12.0).min(1.0) * (1.0 - u) * (1.0 - u) * 1.6;
        if self.am_rate > 0.0 {
            let s = sin1(self.t * self.am_rate);
            env *= s * s;
        }
        self.fm_phase = (self.fm_phase + self.fm_rate * dt).fract();
        let f = self.freq * (1.0 + self.fm * sin1(self.fm_phase));
        self.phase = (self.phase + f * dt).fract();
        let mut x = sin1(self.phase);
        if self.harm > 0.0 {
            x += self.harm * (0.5 * sin1(self.phase * 2.0) + 0.25 * sin1(self.phase * 3.0));
        }
        if self.rough > 0.0 {
            self.noise ^= self.noise << 13;
            self.noise ^= self.noise >> 17;
            self.noise ^= self.noise << 5;
            let n = (self.noise >> 8) as f32 / (1u32 << 23) as f32 - 1.0;
            x *= 1.0 - self.rough + self.rough * n * 1.7;
        }
        self.freq *= self.glide;
        self.t += dt;
        pan(x * env * self.gain, self.pan)
    }
}

/// One bubble, as Kees van den Doel modeled them: a sine at the bubble's
/// Minnaert frequency, rising as it nears the surface, dying away. Many
/// of them make a stream, a pour, rain into puddles.
#[derive(Clone, Copy, Default)]
pub struct Bubble {
    phase: f32,
    freq: f32,
    rise: f32,
    amp: f32,
    k: f32,
    pan: f32,
}

impl Live for Bubble {
    fn live(&self) -> bool {
        self.amp > 0.0005
    }
}

impl Bubble {
    /// A bubble of `radius` millimetres; `rise` is how fast its pitch climbs.
    pub fn start(&mut self, radius: f32, rise: f32, gain: f32, pan: f32, rate: f32) {
        // Minnaert: f is about 3.26 m/s over the radius, 3.26 kHz at 1 mm.
        self.freq = (3260.0 / radius.max(0.1)).min(rate * 0.4);
        // Damping rises with frequency (van den Doel's simplified fit).
        let damping = 0.13 * self.freq + 0.0072 * self.freq.powf(1.5);
        self.k = (-damping / rate).exp();
        self.rise = (1.0 + rise * damping / rate).max(1.0);
        self.amp = gain;
        self.phase = 0.0;
        self.pan = pan;
    }

    #[inline]
    pub fn tick(&mut self, rate: f32) -> (f32, f32) {
        if self.amp <= 0.0005 {
            return (0.0, 0.0);
        }
        self.phase = (self.phase + self.freq / rate).fract();
        let x = sin1(self.phase) * self.amp;
        self.amp *= self.k;
        self.freq *= self.rise;
        pan(x, self.pan)
    }
}

/// An object's modes rung at once: a bell, a glass, a plank, a pan.
#[derive(Clone, Default)]
pub struct Modal {
    modes: Vec<(Mode, f32)>,
    env: f32,
    pan: f32,
    noise: f32,
    noise_k: f32,
}

impl Live for Modal {
    fn live(&self) -> bool {
        self.env > 0.0 || self.modes.iter().any(|(m, _)| !m.quiet())
    }
}

impl Modal {
    /// `partials`: (frequency ratio, decay in seconds, loudness).
    pub fn start(&mut self, base: f32, partials: &[(f32, f32, f32)], decay: f32, gain: f32, pan: f32, rate: f32) {
        self.modes.clear();
        for &(ratio, d, a) in partials {
            let f = base * ratio;
            if f < rate * 0.45 {
                let mut m = Mode::default();
                m.set(f, d * decay, rate);
                self.modes.push((m, a * gain));
            }
        }
        // A short noise strike rings the modes.
        self.env = 1.0;
        self.noise = 1.0;
        self.noise_k = (-1.0 / (0.002 * rate)).exp();
        self.pan = pan;
    }

    #[inline]
    pub fn tick(&mut self, rng: &mut Rng) -> (f32, f32) {
        let strike = if self.env > 0.0 {
            let s = rng.bi() * self.noise;
            self.noise *= self.noise_k;
            if self.noise < 0.001 {
                self.env = 0.0;
            }
            s
        } else {
            0.0
        };
        let mut x = 0.0;
        for (m, a) in &mut self.modes {
            x += m.tick(strike) * *a;
        }
        pan(x, self.pan)
    }
}

/// Vowel formants (Hz) for a voice: a, e, i, o, u, and a schwa.
pub const VOWELS: [[f32; 3]; 6] = [
    [730.0, 1090.0, 2440.0],
    [530.0, 1840.0, 2480.0],
    [270.0, 2290.0, 3010.0],
    [570.0, 840.0, 2410.0],
    [300.0, 870.0, 2240.0],
    [500.0, 1500.0, 2500.0],
];

/// A voice for walla, choirs, calls and whispers: a buzzing source (or
/// breath for a whisper) through three formant filters.
#[derive(Clone, Copy, Default)]
pub struct Voice {
    phase: f32,
    pub pitch: f32,
    pub target_pitch: f32,
    formants: [Svf; 3],
    pub vowel: [f32; 3],
    pub target_vowel: [f32; 3],
    pub breath: f32,
    pub gain: f32,
    pub env: f32,
    pub pan: f32,
    pub on: bool,
    pub glide: f32,
    count: u8,
}

impl Voice {
    #[inline]
    pub fn tick(&mut self, rng: &mut Rng, rate: f32) -> f32 {
        let target = if self.on { 1.0 } else { 0.0 };
        self.env += (target - self.env) * (60.0 / rate);
        if self.env < 0.0005 && !self.on {
            return 0.0;
        }
        let g = self.glide.max(1.0) / rate;
        self.pitch += (self.target_pitch - self.pitch) * g;
        for i in 0..3 {
            self.vowel[i] += (self.target_vowel[i] - self.vowel[i]) * 12.0 / rate;
        }
        self.phase += self.pitch / rate;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        // A glottal-ish pulse: a falling ramp, darker than a saw.
        let buzz = (1.0 - 2.0 * self.phase) * (1.0 - self.phase);
        let src = buzz * (1.0 - self.breath) + rng.bi() * self.breath * 0.7;
        // The formants move slowly: retuned every 16 samples, not each one.
        self.count = self.count.wrapping_add(1);
        let retune = self.count & 15 == 1;
        let mut y = 0.0;
        for (i, f) in self.formants.iter_mut().enumerate() {
            if retune {
                f.set(self.vowel[i], 6.0 + i as f32 * 2.0, rate);
            }
            y += f.band(src) * [1.0, 0.5, 0.25][i];
        }
        y * self.env * self.gain
    }
}

/// A slowly wandering value in -1..1: a few octaves of 1-D gradient
/// noise (fractal Brownian motion), or of simplex or value noise. Swells,
/// gusts and drift use it.
pub struct Drift {
    t: f32,
    noise: super::noise::Gradient,
    kind: Smoothness,
}

#[derive(Clone, Copy)]
pub enum Smoothness {
    Perlin,
    Simplex,
    Value,
}

impl Drift {
    pub fn new(seed: u32) -> Drift {
        Drift::of(Smoothness::Perlin, seed)
    }

    pub fn of(kind: Smoothness, seed: u32) -> Drift {
        Drift { t: seed as f32 % 97.0, noise: super::noise::Gradient::new(seed), kind }
    }

    /// Moves on by `dt` seconds at `rate` (cycles a second, about).
    pub fn next(&mut self, rate: f32, dt: f32) -> f32 {
        self.t += rate * dt;
        if self.t > 4096.0 {
            self.t -= 4096.0;
        }
        let n = &self.noise;
        let t = self.t;
        match self.kind {
            Smoothness::Perlin => n.fbm(t, 3),
            Smoothness::Simplex => n.simplex(t) + 0.5 * n.simplex(t * 2.0 + 31.0) + 0.25 * n.simplex(t * 4.0 + 57.0),
            Smoothness::Value => (n.value(t) + 0.5 * n.value(t * 2.0 + 13.0)) * 0.7,
        }
        .clamp(-1.0, 1.0)
    }
}
