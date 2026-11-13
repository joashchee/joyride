// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! The equalizers: EQ 5 and EQ 10 (shelves at the ends, peaks between,
//! each band's width in octaves), the Mixer EQ (a DJ mixer's three bands),
//! the Exciter (harmonics added to three bands) and the Cabinet Simulator
//! (a tube, an EQ and a speaker cabinet's band).

use super::super::filter::{Biquad, Shape};
use super::{Ctx, Params, Proc};

/// A band's Q for a width in octaves.
fn q_of(octaves: f32) -> f32 {
    let bw = octaves.clamp(0.02, 4.0);
    1.0 / (2.0 * (std::f32::consts::LN_2 / 2.0 * bw).sinh())
}

/// How the EQ's stereo input mode applies it: (left gain sign, right gain sign).
fn mode(p: &Params) -> (f32, f32) {
    match p.text("InputMode").unwrap_or("L+R") {
        "L" => (1.0, 0.0),
        "R" => (0.0, 1.0),
        "L-R" => (1.0, -1.0),
        _ => (1.0, 1.0),
    }
}

/// Default band frequencies (for files that don't keep them).
const EQ5_HZ: [f32; 5] = [100.0, 400.0, 1000.0, 4000.0, 10000.0];
const EQ10_HZ: [f32; 10] = [30.0, 60.0, 120.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0];

pub struct Eq {
    bands: usize,
    left: Vec<Biquad>,
    right: Vec<Biquad>,
}

impl Eq {
    pub fn new(bands: usize) -> Eq {
        Eq {
            bands,
            left: vec![Biquad::default(); bands],
            right: vec![Biquad::default(); bands],
        }
    }
}

impl Proc for Eq {
    fn process(&mut self, p: &Params, l: &mut [f32], r: &mut [f32], _ctx: &Ctx) {
        let (ml, mr) = mode(p);
        let defaults: &[f32] = if self.bands == 5 { &EQ5_HZ } else { &EQ10_HZ };
        let mut active = Vec::new();
        for k in 0..self.bands {
            let gain = p.get(&format!("Gain{k}"), 0.0);
            let f = p.get(&format!("Frequency{k}"), p.text(&format!("Frequency{k}")).and_then(|t| t.parse().ok()).unwrap_or(defaults[k]));
            let bw = p.get(&format!("BandWidth{k}"), p.text(&format!("BandWidth{k}")).and_then(|t| t.parse().ok()).unwrap_or(1.0));
            let shape = if k == 0 {
                Shape::LowShelf
            } else if k == self.bands - 1 {
                Shape::HighShelf
            } else {
                Shape::Peak
            };
            let q = if shape == Shape::Peak { q_of(bw) } else { 0.707 };
            self.left[k].set(shape, f, q, gain * ml);
            self.right[k].set(shape, f, q, gain * mr);
            if gain.abs() > 0.01 {
                active.push(k);
            }
        }
        for i in 0..l.len() {
            for &k in &active {
                l[i] = self.left[k].run(0, l[i]);
                r[i] = self.right[k].run(1, r[i]);
            }
        }
    }
}

/// The Mixer EQ: a low shelf, a movable peak and a high shelf.
pub struct MixerEq {
    b: [[Biquad; 3]; 2],
}

impl MixerEq {
    pub fn new() -> MixerEq {
        MixerEq { b: [[Biquad::default(); 3]; 2] }
    }
}

impl Proc for MixerEq {
    fn process(&mut self, p: &Params, l: &mut [f32], r: &mut [f32], _ctx: &Ctx) {
        let (ml, mr) = mode(p);
        let mid_hz = 100.0 * 100f32.powf(p.get("MidFreq", 0.5));
        let q = 0.4 + p.get("MidQ", 0.3) * 6.0;
        for (k, m) in [ml, mr].iter().enumerate() {
            self.b[k][0].set(Shape::LowShelf, 250.0, 0.707, p.get("LowGain", 0.0) * m);
            self.b[k][1].set(Shape::Peak, mid_hz, q, p.get("MidGain", 0.0) * m);
            self.b[k][2].set(Shape::HighShelf, 4000.0, 0.707, p.get("HiGain", 0.0) * m);
        }
        for i in 0..l.len() {
            for j in 0..3 {
                l[i] = self.b[0][j].run(0, l[i]);
                r[i] = self.b[1][j].run(1, r[i]);
            }
        }
    }
}

/// The Exciter: its three bands each given harmonics (a soft saturation of
/// the band, weighted to its top by the sharpness).
pub struct Exciter {
    lo: Biquad,
    hi: Biquad,
    sharp: [Biquad; 3],
}

impl Exciter {
    pub fn new() -> Exciter {
        Exciter {
            lo: Biquad::default(),
            hi: Biquad::default(),
            sharp: [Biquad::default(); 3],
        }
    }
}

impl Proc for Exciter {
    fn process(&mut self, p: &Params, l: &mut [f32], r: &mut [f32], _ctx: &Ctx) {
        let lo_hz = 20.0 * 1000f32.powf(p.get("LowFrequency", 0.15));
        let hi_hz = 20.0 * 1000f32.powf(p.get("HighFrequency", 0.7));
        self.lo.set(Shape::Low, lo_hz, 0.707, 0.0);
        self.hi.set(Shape::High, hi_hz, 0.707, 0.0);
        let mut amount = [0.0f32; 3];
        for (k, a) in amount.iter_mut().enumerate() {
            *a = (p.get(&format!("Band{}Amount1", k + 1), 0.5) - 0.5).max(0.0) * 2.0;
            let s = p.get(&format!("Band{}Sharpness1", k + 1), 0.5);
            let edge = [lo_hz, (lo_hz * hi_hz).sqrt(), hi_hz][k];
            self.sharp[k].set(Shape::HighShelf, edge, 0.707, s * 12.0);
        }
        for i in 0..l.len() {
            for (ch, x) in [&mut l[i], &mut r[i]].into_iter().enumerate() {
                let low = self.lo.run(ch, *x);
                let high = self.hi.run(ch, *x);
                let mid = *x - low - high;
                let mut add = 0.0;
                for (k, band) in [low, mid, high].iter().enumerate() {
                    if amount[k] > 0.0 {
                        let s = self.sharp[k].run(ch, *band);
                        add += ((s * 3.0).tanh() / 3.0 - s * 0.9) * amount[k];
                    }
                }
                *x += add;
            }
        }
    }
}

/// The Cabinet Simulator: a tube (its gain, a saturation), its EQ, and a
/// cabinet (a speaker's band, by model).
pub struct Cabinet {
    eq: Eq,
    cab: [[Biquad; 3]; 2],
}

impl Cabinet {
    pub fn new() -> Cabinet {
        Cabinet {
            eq: Eq::new(5),
            cab: [[Biquad::default(); 3]; 2],
        }
    }
}

impl Proc for Cabinet {
    fn process(&mut self, p: &Params, l: &mut [f32], r: &mut [f32], ctx: &Ctx) {
        let model = p.get("Cabinet", 0.0).round() as usize;
        // Bass cabinets low, guitar cabinets mid, the rest brighter.
        let (lo, hi, peak) = match model {
            0..=3 => (40.0, 3000.0, 120.0),
            4..=11 => (80.0, 5000.0 - model as f32 * 150.0, 1600.0),
            _ => (60.0, 9000.0, 3000.0),
        };
        for c in self.cab.iter_mut() {
            c[0].set(Shape::High, lo, 0.707, 0.0);
            c[1].set(Shape::Low, hi, 0.9, 0.0);
            c[2].set(Shape::Peak, peak, 1.2, 4.0);
        }
        let drive = 1.0 + p.get("Distortion", 0.2) * 30.0;
        let (wet, dry) = (p.get("Wet", 1.0), p.get("Dry", 0.0));
        let mono = p.def.text("Stereo").is_some_and(|s| s == "false");
        let dl: Vec<f32> = l.to_vec();
        let dr: Vec<f32> = r.to_vec();
        for i in 0..l.len() {
            if mono {
                let m = (l[i] + r[i]) * 0.5;
                l[i] = m;
                r[i] = m;
            }
            l[i] = (l[i] * drive).tanh() / drive.sqrt();
            r[i] = (r[i] * drive).tanh() / drive.sqrt();
        }
        self.eq.process(p, l, r, ctx);
        for i in 0..l.len() {
            for j in 0..3 {
                l[i] = self.cab[0][j].run(0, l[i]);
                r[i] = self.cab[1][j].run(1, r[i]);
            }
            l[i] = l[i] * wet + dl[i] * dry;
            r[i] = r[i] * wet + dr[i] * dry;
        }
    }
}
