// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! The modulation devices: Chorus (both versions) and Flanger (a delay
//! swept by an LFO, fed back, its output through a filter of the first
//! filter bank), Phaser (all-passes swept between its floor and ceiling)
//! and Ringmod (an oscillator multiplied in). Their LFOs restart on the
//! "reset" command.

use std::f32::consts::TAU;

use super::super::filter::{self, note_hz, Biquad, Shape, VoiceFilter};
use super::delay::Line;
use super::{Ctx, Params, Proc};

/// A swept delay: chorus or flanger.
pub struct Sweep {
    lines: [Line; 2],
    phase: f32,
    filter: VoiceFilter,
    kind: i32,
    fb: [f32; 2],
    flanger: bool,
    v2: bool,
}

impl Sweep {
    pub fn chorus(v2: bool) -> Sweep {
        Sweep {
            lines: [Line::new(8192), Line::new(8192)],
            phase: 0.0,
            filter: VoiceFilter::new(filter::Kind::None),
            kind: -1,
            fb: [0.0; 2],
            flanger: false,
            v2,
        }
    }

    pub fn flanger() -> Sweep {
        Sweep {
            flanger: true,
            ..Sweep::chorus(false)
        }
    }
}

impl Proc for Sweep {
    fn process(&mut self, p: &Params, l: &mut [f32], r: &mut [f32], ctx: &Ctx) {
        let ms = ctx.rate / 1000.0;
        // (rate Hz, base delay frames, sweep frames, feedback, wet, dry, phase)
        let (rate, base, depth, fb, wet, dry, phase) = if self.flanger {
            let amount = p.get("Amount", 0.5);
            (
                p.get("Rate", 0.3),
                p.get("Delay", 1.0).clamp(0.0, 100.0) * ms * 0.1 + 0.5 * ms,
                p.get("Amplitude", 0.01) * 0.5 * ctx.rate,
                p.get("FeedBack", 0.0),
                amount,
                1.0,
                p.get("Dephase", 0.0),
            )
        } else {
            let (rate, depth, delay, mix, ph) = if self.v2 {
                (p.get("Rate", 0.5) * 4.0, p.get("Depth", 0.25), p.get("Delay", 0.0), p.get("DryWetMix", 0.5), p.get("Dephase", 0.0))
            } else {
                (p.get("LfoRate", 0.5), p.get("LfoDepth", 0.3), p.get("Delay", 0.0), p.get("DryWetMix", 0.5), p.get("Phase", 0.0))
            };
            (rate, (7.0 + delay * 30.0) * ms, depth * 5.0 * ms, p.get("Feedback", 0.0), mix, 1.0 - mix, ph)
        };
        let ftype = p.get("FilterType", 0.0).round() as i32;
        if ftype != self.kind {
            self.kind = ftype;
            self.filter = VoiceFilter::new(filter::kind(1, ftype));
        }
        self.filter.cutoff = (note_hz(p.get("FilterFreq", 127.0)) / 20.0).ln() / 1000f32.ln();
        self.filter.reso = p.get("FilterResonance", 0.0) / 127.0;
        self.filter.drive = p.get("FilterDrive", 0.0) / 127.0;
        let fb = fb.clamp(-0.98, 0.98);
        for i in 0..l.len() {
            let m = [(self.phase * TAU).sin(), ((self.phase + phase / TAU) * TAU).sin()];
            self.phase = (self.phase + rate / ctx.rate).fract();
            let x = [l[i], r[i]];
            let mut y = [0.0f32; 2];
            for ch in 0..2 {
                let d = (base + depth * (1.0 + m[ch]) * 0.5).max(1.0);
                y[ch] = self.lines[ch].read(d);
                self.lines[ch].write(x[ch] + y[ch] * fb);
            }
            if self.filter.on() {
                let (a, b) = self.filter.process(y[0], y[1]);
                y = [a, b];
            }
            l[i] = x[0] * dry + y[0] * wet;
            r[i] = x[1] * dry + y[1] * wet;
        }
        self.fb = [0.0; 2];
    }

    fn reset(&mut self, at: f32) {
        self.phase = at.clamp(0.0, 1.0);
    }
}

/// The Phaser: a cascade of all-passes swept between floor and ceiling.
pub struct Phaser {
    ap: [[Biquad; 24]; 2],
    phase: f32,
    last: [f32; 2],
}

impl Phaser {
    pub fn new() -> Phaser {
        Phaser {
            ap: [[Biquad::default(); 24]; 2],
            phase: 0.0,
            last: [0.0; 2],
        }
    }
}

impl Proc for Phaser {
    fn process(&mut self, p: &Params, l: &mut [f32], r: &mut [f32], ctx: &Ctx) {
        let (lo, hi) = (p.get("Floor", 300.0).clamp(20.0, 20000.0), p.get("Ceilling", 5000.0).clamp(20.0, 20000.0));
        let (lo, hi) = (lo.min(hi), lo.max(hi));
        let rate = p.get("LFORate", 0.5);
        let mix = (p.get("Depth", 4.0) / 4.0).clamp(0.0, 1.0);
        let fb = p.get("Feedback", 0.3).clamp(0.0, 0.95);
        let ph = p.get("Phase", 0.0) / TAU;
        let stages = (p.get("Stages", 4.0).round() as usize).clamp(1, 24);
        let mut i = 0;
        while i < l.len() {
            let end = (i + 32).min(l.len());
            for ch in 0..2 {
                let m = 0.5 + 0.5 * ((self.phase + if ch == 1 { ph } else { 0.0 }) * TAU).sin();
                let f = lo * (hi / lo).powf(m);
                for b in self.ap[ch].iter_mut().take(stages) {
                    b.set(Shape::AllPass, f, 0.6, 0.0);
                }
            }
            self.phase = (self.phase + rate * (end - i) as f32 / ctx.rate).fract();
            for j in i..end {
                for (ch, x) in [&mut l[j], &mut r[j]].into_iter().enumerate() {
                    let mut y = *x + self.last[ch] * fb;
                    for b in self.ap[ch].iter_mut().take(stages) {
                        y = b.run(ch, y);
                    }
                    self.last[ch] = y;
                    *x = *x * (1.0 - mix * 0.5) + y * mix * 0.5;
                }
            }
            i = end;
        }
    }

    fn reset(&mut self, at: f32) {
        self.phase = at.clamp(0.0, 1.0);
    }
}

/// Ringmod: the signal times an oscillator of a note's frequency.
pub struct RingMod {
    phase: [f32; 2],
    hz: f32,
    rng: u32,
}

impl RingMod {
    pub fn new() -> RingMod {
        RingMod {
            phase: [0.0; 2],
            hz: 0.0,
            rng: 0x1234_5678,
        }
    }
}

impl Proc for RingMod {
    fn process(&mut self, p: &Params, l: &mut [f32], r: &mut [f32], ctx: &Ctx) {
        let hz = note_hz(p.get("Frequency", 0.5) * 119.0 + 12.0);
        if self.hz == 0.0 {
            self.hz = hz;
        }
        let k = (p.get("Inertia", 1.0 / 128.0) * 64.0).clamp(0.0, 1.0);
        let amount = p.get("Amount", 1.0);
        let shape = p.get("OscillatorType", 0.0).round() as i32;
        let offset = p.get("Phase", 0.0) / 360.0;
        for i in 0..l.len() {
            if i % 16 == 0 {
                self.hz += (hz - self.hz) * k;
            }
            for (ch, x) in [&mut l[i], &mut r[i]].into_iter().enumerate() {
                let ph = (self.phase[ch] + if ch == 1 { offset } else { 0.0 }).fract();
                let o = match shape {
                    1 => 1.0 - 4.0 * (ph - 0.5).abs(),
                    2 => 2.0 * ph - 1.0,
                    3 => {
                        if ph < 0.5 {
                            1.0
                        } else {
                            -1.0
                        }
                    }
                    4 => {
                        self.rng ^= self.rng << 13;
                        self.rng ^= self.rng >> 17;
                        self.rng ^= self.rng << 5;
                        self.rng as f32 / u32::MAX as f32 * 2.0 - 1.0
                    }
                    _ => (ph * TAU).sin(),
                };
                *x = *x * (1.0 - amount) + *x * o * amount;
                self.phase[ch] = (self.phase[ch] + self.hz / ctx.rate).fract();
            }
        }
    }
}
