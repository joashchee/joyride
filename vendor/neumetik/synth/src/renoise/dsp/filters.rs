// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! The filter devices: Renoise 1's Filter and Filter 2 (the first filter
//! bank's types, a MIDI note's cutoff scale), Renoise 2's Filter (four
//! models, six types), the Scream Filter (a filter with a shift distortion
//! in its feedback), and Renoise 3's Analog, Digital and Comb filters.
//! Each glides its cutoff and resonance at its inertia.

use super::super::filter::{self, cutoff_hz, note_hz, Biquad, Kind, Ladder, Shape, VoiceFilter};
use super::{Ctx, Params, Proc};

/// A 0..1 cutoff for a frequency.
fn norm_of(hz: f32) -> f32 {
    ((hz / 20.0).max(1e-3).ln() / 1000f32.ln()).clamp(0.0, 1.0)
}

/// Frames between coefficient updates while gliding.
const STEP: usize = 16;

/// A step of a glide towards `to`, there once it's close.
fn glide(now: f32, to: f32, k: f32) -> f32 {
    let next = now + (to - now) * k;
    if (to - next).abs() < 1e-4 {
        to
    } else {
        next
    }
}

/// Renoise 1 and 2's Filter devices and the filter banks' kinds.
pub struct BankFilter {
    f: VoiceFilter,
    cutoff: f32,
    reso: f32,
    kind: Kind,
    fresh: bool,
    old: bool,
}

impl BankFilter {
    /// `old`: Filter / Filter 2 (types of the first bank, 0..127 values);
    /// else Renoise 2's Filter.
    pub fn new(old: bool) -> BankFilter {
        BankFilter {
            f: VoiceFilter::new(Kind::None),
            cutoff: 1.0,
            reso: 0.0,
            kind: Kind::None,
            fresh: true,
            old,
        }
    }
}

fn filter3_kind(model: &str, t: usize) -> Kind {
    let (n, moog) = match model {
        "Butterworth 4n" => (2, false),
        "Butterworth 8n" => (4, false),
        "24dB Moog" => (2, true),
        _ => (2, false),
    };
    match t {
        0 => {
            if moog {
                Kind::Ladder(false, 4)
            } else {
                Kind::Bq(Shape::Low, n)
            }
        }
        // The shelves: the part they bring down (below or above the cutoff).
        1 => Kind::Bq(Shape::Low, 1),
        2 => Kind::Bq(Shape::Band, n.min(2)),
        3 => Kind::Bq(Shape::Notch, n.min(2)),
        4 => Kind::Bq(Shape::High, 1),
        _ => {
            if moog {
                Kind::Ladder(true, 4)
            } else {
                Kind::Bq(Shape::High, n)
            }
        }
    }
}

impl Proc for BankFilter {
    fn process(&mut self, p: &Params, l: &mut [f32], r: &mut [f32], _ctx: &Ctx) {
        let (kind, cutoff, reso, inertia) = if self.old {
            let t = p.get("Type", 0.0).round() as i32;
            (
                filter::kind(1, t + 1),
                norm_of(note_hz(p.get("Cutoff", 127.0))),
                p.get("Resonance", 0.0) / 127.0,
                p.get("Innertia", 1.0 / 128.0),
            )
        } else {
            let t = p.get("Type", 0.0).round() as usize;
            (
                filter3_kind(p.text("Model").unwrap_or("24dB 4Pole"), t),
                p.get("Frequency", 1.0),
                (p.get("Q", 0.0) / 14.0).clamp(0.0, 1.0),
                p.get("Inertia", 1.0 / 128.0),
            )
        };
        if kind != self.kind {
            self.f = VoiceFilter::new(kind);
            self.kind = kind;
        }
        if self.fresh {
            self.cutoff = cutoff;
            self.reso = reso;
            self.fresh = false;
        }
        let k = (inertia * STEP as f32 * 4.0).clamp(0.0, 1.0);
        let gain = p.get("Gain", 0.0);
        let mut i = 0;
        while i < l.len() {
            self.cutoff = glide(self.cutoff, cutoff, k);
            self.reso = glide(self.reso, reso, k);
            self.f.cutoff = self.cutoff;
            self.f.reso = self.reso;
            let shelf = !self.old && matches!(p.get("Type", 0.0).round() as i32, 1 | 4);
            let end = (i + STEP).min(l.len());
            for j in i..end {
                let (a, b) = self.f.process(l[j], r[j]);
                if shelf {
                    // A shelf: the filtered part brought down by its gain (dB).
                    let g = 10f32.powf(gain / 20.0);
                    l[j] = l[j] - a + a * g;
                    r[j] = r[j] - b + b * g;
                } else {
                    l[j] = a;
                    r[j] = b;
                }
            }
            i = end;
        }
    }
}

/// The Scream Filter: a resonant low- or high-pass with a shift
/// distortion in its feedback.
pub struct Scream {
    ladder: Ladder,
    cutoff: f32,
    fresh: bool,
}

impl Scream {
    pub fn new() -> Scream {
        Scream {
            ladder: Ladder::default(),
            cutoff: 1.0,
            fresh: true,
        }
    }
}

impl Proc for Scream {
    fn process(&mut self, p: &Params, l: &mut [f32], r: &mut [f32], _ctx: &Ctx) {
        let high = p.get("FilterType", 0.0) >= 0.5;
        let drive = p.get("Drive", 0.3);
        let cutoff = p.get("Cutoff", 0.5);
        let reso = p.get("Resonance", 0.3);
        let k = (p.get("Inertia", 1.0 / 128.0) * 64.0).clamp(0.0, 1.0);
        if self.fresh {
            self.cutoff = cutoff;
            self.fresh = false;
        }
        for i in 0..l.len() {
            if i % STEP == 0 {
                self.cutoff += (cutoff - self.cutoff) * k;
            }
            let f = cutoff_hz(self.cutoff);
            for (ch, x) in [&mut l[i], &mut r[i]].into_iter().enumerate() {
                let shifted = *x + drive * (x.abs() * 2.0 - 0.5) * 0.5;
                *x = self.ladder.run(ch, shifted * (1.0 + drive * 3.0), f, reso * 0.95, high, 2);
            }
        }
    }
}

/// Renoise 3's Analog Filter (ladders with drive) and Digital Filter
/// (biquad, Butterworth and Chebyshev cascades).
pub struct ModernFilter {
    analog: bool,
    f: VoiceFilter,
    kind: Kind,
    cutoff: f32,
    reso: f32,
    fresh: bool,
}

impl ModernFilter {
    pub fn new(analog: bool) -> ModernFilter {
        ModernFilter {
            analog,
            f: VoiceFilter::new(Kind::None),
            kind: Kind::None,
            cutoff: 1.0,
            reso: 0.0,
            fresh: true,
        }
    }
}

impl Proc for ModernFilter {
    fn process(&mut self, p: &Params, l: &mut [f32], r: &mut [f32], _ctx: &Ctx) {
        let t = p.get("Type", 0.0).round() as usize;
        let model = p.text("Model").unwrap_or("");
        let kind = if self.analog {
            let poles = if model.starts_with("2P") { 2 } else { 4 };
            match t {
                0 => Kind::Ladder(false, poles),
                1 => Kind::Bq(Shape::Band, 1),
                _ => Kind::Ladder(true, poles),
            }
        } else {
            let n = match model {
                "Butterworth 8n" | "Chebyshev 8n" => 4,
                "Butterworth 4n" | "Chebyshev 4n" => 2,
                _ => 1,
            };
            [Kind::Bq(Shape::Low, n), Kind::Bq(Shape::Band, n), Kind::Bq(Shape::Notch, n), Kind::Bq(Shape::High, n)][t.min(3)]
        };
        if kind != self.kind {
            self.f = VoiceFilter::new(kind);
            self.kind = kind;
        }
        let cutoff = p.get("Cutoff", 1.0);
        let reso = if self.analog { p.get("Resonance", 0.0) } else { p.get("Q", 0.0) };
        if self.fresh {
            self.cutoff = cutoff;
            self.reso = reso;
            self.fresh = false;
        }
        let k = (p.get("Inertia", 1.0 / 128.0) * STEP as f32 * 4.0).clamp(0.0, 1.0);
        self.f.drive = p.get("Drive", 0.0);
        let mut i = 0;
        while i < l.len() {
            self.cutoff = glide(self.cutoff, cutoff, k);
            self.reso = glide(self.reso, reso, k);
            self.f.cutoff = self.cutoff;
            self.f.reso = self.reso;
            let end = (i + STEP).min(l.len());
            for j in i..end {
                let (a, b) = self.f.process(l[j], r[j]);
                l[j] = a;
                r[j] = b;
            }
            i = end;
        }
    }
}

/// The Comb Filter: a delay a note's period long, fed back.
pub struct Comb {
    buf: [Vec<f32>; 2],
    at: usize,
    hz: f32,
    lp: Biquad,
}

impl Comb {
    pub fn new() -> Comb {
        Comb {
            buf: [vec![0.0; 8192], vec![0.0; 8192]],
            at: 0,
            hz: 0.0,
            lp: Biquad::default(),
        }
    }
}

impl Proc for Comb {
    fn process(&mut self, p: &Params, l: &mut [f32], r: &mut [f32], ctx: &Ctx) {
        // Frequency 0..1 spans the notes C-0 to B-9.
        let hz = note_hz(p.get("Frequency", 0.5) * 119.0 + 12.0);
        if self.hz == 0.0 {
            self.hz = hz;
        }
        let k = (p.get("Inertia", 1.0 / 128.0) * 64.0).clamp(0.0, 1.0);
        let fb = p.get("Feedback", 0.5).clamp(-0.99, 0.99);
        let (wet, dry) = (p.get("WetOut", 1.0), p.get("DryOut", 0.0));
        self.lp.set(Shape::Low, 12000.0, 0.707, 0.0);
        for i in 0..l.len() {
            if i % STEP == 0 {
                self.hz += (hz - self.hz) * k;
            }
            let d = (ctx.rate / self.hz.max(5.0)).clamp(1.0, 8190.0);
            for (ch, x) in [&mut l[i], &mut r[i]].into_iter().enumerate() {
                let pos = self.at as f32 + 8192.0 - d;
                let j = pos.floor();
                let f = pos - j;
                let a = self.buf[ch][j as usize % 8192];
                let b = self.buf[ch][(j as usize + 1) % 8192];
                let y = *x + (a + (b - a) * f) * fb;
                self.buf[ch][self.at] = self.lp.run(ch, y);
                *x = *x * dry + y * wet * (1.0 - fb.abs() * 0.5);
            }
            self.at = (self.at + 1) % 8192;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renoise::song::Device;

    #[test]
    fn filter3_highpass_is_clean() {
        for (t, freq, model) in [
            (5.0, 0.6, "24dB 4Pole"),
            (5.0, 0.02, "Butterworth 8n"),
            (0.0, 0.97, "Butterworth 8n"),
            (5.0, 0.02, "Butterworth 4n"),
            (0.0, 0.97, "Butterworth 4n"),
            (5.0, 0.3, "24dB Moog"),
            (0.0, 0.3, "24dB Moog"),
            (0.0, 0.9, "24dB Moog"),
            (0.0, 1.0, "24dB Moog"),
        ] {
            check(t, freq, model);
        }
    }

    fn check(t: f32, freq: f32, model: &str) {
        let def = Device {
            kind: "Filter3Device".into(),
            active: true,
            params: vec![("Type".into(), t), ("Frequency".into(), freq), ("Q".into(), 0.0), ("Gain".into(), -60.0), ("Inertia".into(), 0.0078125)],
            extra: vec![("Model".into(), model.into())],
            ..Default::default()
        };
        let values: Vec<f32> = def.params.iter().map(|p| p.1).collect();
        let mut f = BankFilter::new(false);
        let ctx = Ctx {
            rate: 44100.0,
            bpm: 120.0,
            line: 0.125,
        };
        let mut seed = 1u32;
        let mut l: Vec<f32> = (0..4410)
            .map(|i| {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                (i as f32 * 2.0 * std::f32::consts::PI * 3000.0 / 44100.0).sin() * 0.5 + (seed >> 8) as f32 / (1 << 24) as f32 * 0.02
            })
            .collect();
        let mut r = l.clone();
        f.process(&Params { def: &def, values: &values }, &mut l, &mut r, &ctx);
        // A 3 kHz sine passes; nothing alternates sample to sample.
        let alt = l.windows(2).skip(100).filter(|w| (w[0] > 0.0) != (w[1] > 0.0)).count();
        let peak = l[100..].iter().fold(0.0f32, |a, &x| a.max(x.abs()));
        println!("{t} {freq} {model}: crossings {alt}, peak {peak}");
        assert!(alt < 4410 * 3000 * 2 / 44100 + 20, "{alt}");
    }
}

#[cfg(test)]
mod ladder_tests {
    use super::*;

    #[test]
    fn open_ladder_stays_quiet() {
        let mut lad = Ladder::default();
        let mut peak = 0.0f32;
        for i in 0..44100 {
            let x = if i < 4000 { 0.0 } else { ((i * 7919) % 13) as f32 * 1e-5 };
            let y = lad.run(0, x, 19845.0, 0.0, false, 4);
            peak = peak.max(y.abs());
        }
        println!("ladder peak {peak}");
        let mut vf = VoiceFilter::new(Kind::Ladder(false, 4));
        vf.cutoff = 1.0;
        let mut peak = 0.0f32;
        for i in 0..44100 {
            let x = if i < 4000 { 0.0 } else { ((i * 7919) % 13) as f32 * 1e-5 };
            let (y, _) = vf.process(x, x);
            peak = peak.max(y.abs());
        }
        println!("voice filter peak {peak}");
        assert!(peak < 0.01);
    }
}
