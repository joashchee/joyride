// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! What the recipes are made of: noises, oscillators gliding on a curve,
//! modal resonators (pings, bells, bubbles), crackle, envelopes, a
//! state-variable filter that moves, saturation and a small reverb. Every
//! part is a buffer of mono `f32` at the kit's rate.

use std::f32::consts::{PI, TAU};

pub enum Wave {
    Sine,
    Saw,
    Square,
    Triangle,
}

#[derive(Clone, Copy)]
pub enum Mode {
    Low,
    Band,
    High,
}

/// A recipe's rate and its randomness, seeded so a recipe always comes
/// out the same.
pub struct Kit {
    pub rate: f32,
    state: u32,
}

impl Kit {
    pub fn new(rate: u32, seed: u32) -> Kit {
        Kit { rate: rate as f32, state: seed | 1 }
    }

    /// Samples in `s` seconds.
    pub fn len(&self, s: f32) -> usize {
        (s.max(0.0) * self.rate) as usize
    }

    /// 0..1 (xorshift32).
    pub fn rand(&mut self) -> f32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }

    /// -1..1.
    pub fn bi(&mut self) -> f32 {
        self.rand() * 2.0 - 1.0
    }

    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.rand()
    }

    pub fn white(&mut self, s: f32) -> Vec<f32> {
        (0..self.len(s)).map(|_| self.bi()).collect()
    }

    /// Pink, Paul Kellet's economy filter on white.
    pub fn pink(&mut self, s: f32) -> Vec<f32> {
        let (mut b0, mut b1, mut b2) = (0.0, 0.0, 0.0);
        (0..self.len(s))
            .map(|_| {
                let w = self.bi();
                b0 = 0.99765 * b0 + w * 0.0990460;
                b1 = 0.96300 * b1 + w * 0.2965164;
                b2 = 0.57000 * b2 + w * 1.0526913;
                (b0 + b1 + b2 + w * 0.1848) * 0.25
            })
            .collect()
    }

    /// Brown: white integrated, leaking so it stays centred.
    pub fn brown(&mut self, s: f32) -> Vec<f32> {
        let mut y = 0.0;
        (0..self.len(s))
            .map(|_| {
                y = y * 0.998 + self.bi() * 0.06;
                y
            })
            .collect()
    }

    /// An oscillator whose frequency is `f(t)` (t in seconds); the saw
    /// and square are band-limited (polyBLEP).
    pub fn tone(&self, s: f32, wave: Wave, f: impl Fn(f32) -> f32) -> Vec<f32> {
        let mut phase = 0.0f32;
        (0..self.len(s))
            .map(|i| {
                let dt = (f(i as f32 / self.rate) / self.rate).clamp(0.0, 0.49);
                let blep = |p: f32| {
                    if p < dt {
                        let t = p / dt;
                        2.0 * t - t * t - 1.0
                    } else if p > 1.0 - dt {
                        let t = (p - 1.0) / dt;
                        t * t + 2.0 * t + 1.0
                    } else {
                        0.0
                    }
                };
                let v = match wave {
                    Wave::Sine => (phase * TAU).sin(),
                    Wave::Saw => 2.0 * phase - 1.0 - blep(phase),
                    Wave::Square => (if phase < 0.5 { 1.0 } else { -1.0 }) + blep(phase) - blep((phase + 0.5) % 1.0),
                    Wave::Triangle => 1.0 - 4.0 * (phase - 0.5).abs(),
                };
                phase = (phase + dt) % 1.0;
                v
            })
            .collect()
    }

    /// Decaying sines: (frequency, seconds to -60 dB, level), struck at once.
    pub fn modal(&self, s: f32, modes: &[(f32, f32, f32)]) -> Vec<f32> {
        let mut out = vec![0.0; self.len(s)];
        for &(f, t60, a) in modes {
            if f >= self.rate * 0.45 {
                continue;
            }
            let (w, k) = (TAU * f / self.rate, (-6.9078 / (t60 * self.rate)).exp());
            let mut amp = a;
            for (i, o) in out.iter_mut().enumerate() {
                *o += amp * (w * i as f32).sin();
                amp *= k;
            }
        }
        out
    }

    /// A bell by FM: an inharmonic modulator whose depth fades faster
    /// than the tone, so it strikes bright and rings pure.
    pub fn bell(&self, s: f32, f: f32, t60: f32) -> Vec<f32> {
        let (mut pc, mut pm) = (0.0f32, 0.0f32);
        (0..self.len(s))
            .map(|i| {
                let t = i as f32 / self.rate;
                let index = 3.0 * (-t * 6.9 / (t60 * 0.4)).exp();
                let v = (pc * TAU + index * (pm * TAU).sin()).sin() * (-t * 6.9 / t60).exp();
                pc = (pc + f / self.rate) % 1.0;
                pm = (pm + f * 1.41 / self.rate) % 1.0;
                v
            })
            .collect()
    }

    /// `count` struck pings at random times in the first `spread` of the
    /// sound, `lo` to `hi` Hz.
    pub fn pings(&mut self, s: f32, count: usize, lo: f32, hi: f32, t60: f32, spread: f32) -> Vec<f32> {
        let mut out = vec![0.0; self.len(s)];
        for _ in 0..count {
            let r = self.rand();
            let at = self.len(r * s * spread);
            let f = lo * (hi / lo).powf(self.rand());
            let a = self.range(0.3, 1.0);
            let ping = self.modal(t60 * 3.0, &[(f, t60, a), (f * 2.76, t60 * 0.5, a * 0.4)]);
            add(&mut out, &ping, at, 1.0);
        }
        out
    }

    /// Bubbles: Minnaert's resonance, a sine gliding up as it dies, at
    /// random times and sizes.
    pub fn bubbles(&mut self, s: f32, count: usize, lo: f32, hi: f32, spread: f32) -> Vec<f32> {
        let mut out = vec![0.0; self.len(s)];
        for _ in 0..count {
            let r = self.rand();
            let at = self.len(r * s * spread);
            let f = lo * (hi / lo).powf(self.rand());
            let d = 0.02 + 6.0 / f;
            let a = self.range(0.3, 1.0);
            let mut b = self.tone(d, Wave::Sine, |t| f * (1.0 + 2.5 * t / d));
            let n = b.len();
            mul(&mut b, &decay(n, self.rate, 0.001, d));
            add(&mut out, &b, at, a);
        }
        out
    }

    /// Crackle: `per_sec` clicks a second, each a millisecond or two of
    /// noise at a random level.
    pub fn crackle(&mut self, s: f32, per_sec: f32) -> Vec<f32> {
        let n = self.len(s);
        let mut out = vec![0.0; n];
        let count = (per_sec * s) as usize;
        for _ in 0..count {
            let at = (self.rand() * n as f32) as usize;
            let a = self.rand().powi(2);
            let d = self.range(0.0005, 0.003);
            let len = self.len(d);
            for j in 0..len {
                if at + j < n {
                    out[at + j] += a * self.bi() * (1.0 - j as f32 / len as f32);
                }
            }
        }
        out
    }
}

/// A frequency gliding exponentially from `f0` to `f1` over `secs`, then held.
pub fn glide(f0: f32, f1: f32, secs: f32) -> impl Fn(f32) -> f32 {
    move |t| f0 * (f1 / f0).powf((t / secs).min(1.0))
}

/// Straight lines between (seconds, level) points; the last level holds.
pub fn env(n: usize, rate: f32, points: &[(f32, f32)]) -> Vec<f32> {
    (0..n)
        .map(|i| {
            let t = i as f32 / rate;
            match points.iter().position(|p| p.0 > t) {
                Some(0) => points[0].1,
                Some(j) => {
                    let (a, b) = (points[j - 1], points[j]);
                    a.1 + (b.1 - a.1) * (t - a.0) / (b.0 - a.0)
                }
                None => points.last().map_or(0.0, |p| p.1),
            }
        })
        .collect()
}

/// A quick linear attack, then an exponential fall to -60 dB in `t60`.
pub fn decay(n: usize, rate: f32, attack: f32, t60: f32) -> Vec<f32> {
    (0..n)
        .map(|i| {
            let t = i as f32 / rate;
            if t < attack {
                t / attack
            } else {
                (-(t - attack) * 6.9078 / t60).exp()
            }
        })
        .collect()
}

pub fn mul(x: &mut [f32], e: &[f32]) {
    for (v, g) in x.iter_mut().zip(e) {
        *v *= g;
    }
    let n = e.len().min(x.len());
    x[n..].fill(0.0);
}

/// Mixes `src` into `dst` from sample `at`, growing `dst` to fit.
pub fn add(dst: &mut Vec<f32>, src: &[f32], at: usize, gain: f32) {
    if dst.len() < at + src.len() {
        dst.resize(at + src.len(), 0.0);
    }
    for (d, s) in dst[at..].iter_mut().zip(src) {
        *d += s * gain;
    }
}

pub fn scale(x: &mut [f32], gain: f32) {
    x.iter_mut().for_each(|v| *v *= gain);
}

/// Soft saturation: harmonics a sub-bass needs to be heard on small
/// speakers.
pub fn drive(x: &mut [f32], amount: f32) {
    let norm = amount.tanh();
    x.iter_mut().for_each(|v| *v = (*v * amount).tanh() / norm);
}

pub fn peak(x: &[f32]) -> f32 {
    x.iter().fold(0.0f32, |m, v| m.max(v.abs()))
}

/// Scales `x` to peak at 1.
pub fn norm(x: &mut [f32]) {
    let p = peak(x);
    if p > 0.0 {
        scale(x, 1.0 / p);
    }
}

/// A state-variable filter (the topology-preserving form), its cutoff
/// `cut(t)` Hz moving as it plays.
pub fn filter(x: &[f32], rate: f32, mode: Mode, cut: impl Fn(f32) -> f32, q: f32) -> Vec<f32> {
    let k = 1.0 / q.max(0.1);
    let (mut ic1, mut ic2) = (0.0f32, 0.0f32);
    x.iter()
        .enumerate()
        .map(|(i, &v0)| {
            let f = cut(i as f32 / rate).clamp(10.0, rate * 0.45);
            let g = (PI * f / rate).tan();
            let a1 = 1.0 / (1.0 + g * (g + k));
            let (a2, a3) = (g * a1, g * g * a1);
            let v3 = v0 - ic2;
            let v1 = a1 * ic1 + a2 * v3;
            let v2 = ic2 + a2 * ic1 + a3 * v3;
            ic1 = 2.0 * v1 - ic1;
            ic2 = 2.0 * v2 - ic2;
            match mode {
                Mode::Low => v2,
                Mode::Band => v1,
                Mode::High => v0 - k * v1 - v2,
            }
        })
        .collect()
}

pub fn lowpass(x: &[f32], rate: f32, cut: f32) -> Vec<f32> {
    filter(x, rate, Mode::Low, |_| cut, 0.707)
}

pub fn highpass(x: &[f32], rate: f32, cut: f32) -> Vec<f32> {
    filter(x, rate, Mode::High, |_| cut, 0.707)
}

pub fn bandpass(x: &[f32], rate: f32, cut: f32, q: f32) -> Vec<f32> {
    filter(x, rate, Mode::Band, |_| cut, q)
}

/// A small room: Schroeder's four damped combs into two allpasses,
/// ringing to -60 dB in `t60`; the tail is added to the end.
pub fn reverb(x: &[f32], rate: f32, t60: f32, mix: f32) -> Vec<f32> {
    let n = x.len() + (t60 * rate) as usize;
    let mut wet = vec![0.0f32; n];
    for ms in [29.7f32, 37.1, 41.1, 43.7] {
        let d = (ms * 0.001 * rate) as usize;
        let g = 10f32.powf(-3.0 * ms * 0.001 / t60);
        let mut line = vec![0.0f32; d];
        let mut lp = 0.0f32;
        for i in 0..n {
            let out = line[i % d];
            lp = out * 0.6 + lp * 0.4;
            line[i % d] = x.get(i).copied().unwrap_or(0.0) + lp * g;
            wet[i] += out * 0.25;
        }
    }
    for ms in [5.0f32, 1.7] {
        let d = (ms * 0.001 * rate) as usize;
        let mut line = vec![0.0f32; d];
        for v in wet.iter_mut() {
            let buf = line[0];
            let y = buf - 0.7 * *v;
            line.rotate_left(1);
            line[d - 1] = *v + 0.7 * y;
            *v = y;
        }
    }
    (0..n).map(|i| x.get(i).copied().unwrap_or(0.0) * (1.0 - mix) + wet[i] * mix).collect()
}
