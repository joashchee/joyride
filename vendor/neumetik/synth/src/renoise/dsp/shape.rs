// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! The shape and tool devices: Renoise 1's Distortion (amp and fuzz
//! modes) and Renoise 2's (razor, shape, fold and shift, a tone filter
//! before them), the LofiMat in both versions (bits, rate, noise), DC
//! Offset and Stereo Expander.

use super::super::filter::{Biquad, Shape};
use super::{Ctx, Params, Proc};

pub struct Distortion {
    tone: Biquad,
    lp: Biquad,
    v2: bool,
    /// The shift mode's DC correction: last input and output, each channel.
    dc: [(f32, f32); 2],
}

impl Distortion {
    pub fn new(v2: bool) -> Distortion {
        Distortion {
            tone: Biquad::default(),
            lp: Biquad::default(),
            v2,
            dc: [(0.0, 0.0); 2],
        }
    }
}

impl Proc for Distortion {
    fn process(&mut self, p: &Params, l: &mut [f32], r: &mut [f32], _ctx: &Ctx) {
        if self.v2 {
            let mode = p.get("Type", 1.0).round() as i32;
            let drive = p.get("Drive", 0.3);
            let tone = p.get("Tone", 0.5) * 2.0 - 1.0;
            let (wet, dry) = (p.get("WetOut", 1.0), p.get("DryOut", 0.0));
            self.tone.set(Shape::HighShelf, 1500.0, 0.707, tone * 12.0);
            let g = 1.0 + drive * drive * 60.0;
            for i in 0..l.len() {
                for (ch, x) in [&mut l[i], &mut r[i]].into_iter().enumerate() {
                    let s = self.tone.run(ch, *x) * g;
                    let y = match mode {
                        0 => s.clamp(-1.0, 1.0),
                        2 => {
                            let t = (s + 1.0) * 0.25;
                            1.0 - 4.0 * (t - t.floor() - 0.5).abs()
                        }
                        // Folded above the DC line.
                        3 => s.abs().min(1.0),
                        _ => s.tanh(),
                    };
                    // The offset any shaping leaves is taken off.
                    let (px, py) = self.dc[ch];
                    let y2 = y - px + 0.999 * py;
                    self.dc[ch] = (y, y2);
                    *x = *x * dry + y2 * wet;
                }
            }
            return;
        }
        // Renoise 1: amp (drive and a low-pass) or fuzz (threshold and clamp).
        let fuzz = p.get("Type", 1.0) >= 0.5;
        let a = p.get("Threshold", 16384.0) / 32768.0;
        let b = p.get("LpOrClamp", 16000.0);
        let switch = p.get("GateOrFilter", 0.0) >= 0.5;
        let (wet, dry) = (p.get("WetOut", 128.0) / 128.0, p.get("DryOut", 0.0) / 128.0);
        if !fuzz && switch {
            self.lp.set(Shape::Low, b.clamp(20.0, 20000.0), 0.707, 0.0);
        }
        for i in 0..l.len() {
            for (ch, x) in [&mut l[i], &mut r[i]].into_iter().enumerate() {
                let shaped = if fuzz {
                    let t = a.max(1e-3);
                    let clamp = (b / 32768.0).max(1e-3);
                    let v = (*x / t).clamp(-1.0, 1.0) * clamp;
                    if switch && x.abs() < t * 0.05 {
                        0.0
                    } else {
                        v
                    }
                } else {
                    let v = (*x * (1.0 + a * 40.0)).tanh();
                    if switch {
                        self.lp.run(ch, v)
                    } else {
                        v
                    }
                };
                let (px, py) = self.dc[ch];
                let y = shaped - px + 0.999 * py;
                self.dc[ch] = (shaped, y);
                *x = *x * dry + y * wet;
            }
        }
    }
}

pub struct Lofi {
    hold: [f32; 2],
    prev: [f32; 2],
    t: f32,
    rng: u32,
    v2: bool,
}

impl Lofi {
    pub fn new(v2: bool) -> Lofi {
        Lofi {
            hold: [0.0; 2],
            prev: [0.0; 2],
            t: 1.0,
            rng: 0x9e37_79b9,
            v2,
        }
    }
}

impl Proc for Lofi {
    fn process(&mut self, p: &Params, l: &mut [f32], r: &mut [f32], _ctx: &Ctx) {
        let (bits, rate, noise, wet, dry) = if self.v2 {
            (p.get("Bits", 16.0), p.get("Rate", 1.0), p.get("Noise", 0.0), p.get("WetOut", 1.0), p.get("DryOut", 0.0))
        } else {
            (
                p.get("BitCrunsh", 16.0),
                p.get("Quality", 1.0),
                p.get("Noise", 0.0) / 100.0,
                p.get("WetOut", 100.0) / 100.0,
                p.get("DryOut", 0.0) / 100.0,
            )
        };
        let smooth = p.get("Interpolate", 0.0) >= 0.5;
        let steps = 2f32.powf(bits.clamp(1.0, 16.0) - 1.0);
        let step = (rate.clamp(0.0, 1.0).powi(2)).max(0.002);
        for i in 0..l.len() {
            self.t += step;
            let x = [l[i], r[i]];
            if self.t >= 1.0 {
                self.t -= self.t.floor();
                self.prev = self.hold;
                for ch in 0..2 {
                    self.rng ^= self.rng << 13;
                    self.rng ^= self.rng >> 17;
                    self.rng ^= self.rng << 5;
                    let n = (self.rng as f32 / u32::MAX as f32 * 2.0 - 1.0) * noise * 0.1;
                    self.hold[ch] = ((x[ch] + n) * steps).round() / steps;
                }
            }
            for (ch, o) in [&mut l[i], &mut r[i]].into_iter().enumerate() {
                let y = if smooth { self.prev[ch] + (self.hold[ch] - self.prev[ch]) * self.t } else { self.hold[ch] };
                *o = x[ch] * dry + y * wet;
            }
        }
    }
}

pub struct DcOffset {
    x1: [f32; 2],
    y1: [f32; 2],
}

impl DcOffset {
    pub fn new() -> DcOffset {
        DcOffset { x1: [0.0; 2], y1: [0.0; 2] }
    }
}

impl Proc for DcOffset {
    fn process(&mut self, p: &Params, l: &mut [f32], r: &mut [f32], _ctx: &Ctx) {
        let off = p.get("DCOffset", 0.0) / 32768.0;
        let auto = p.get("AutoDC", 0.0) >= 0.5;
        for i in 0..l.len() {
            for (ch, x) in [&mut l[i], &mut r[i]].into_iter().enumerate() {
                let mut v = *x + off;
                if auto {
                    let y = v - self.x1[ch] + 0.9995 * self.y1[ch];
                    self.x1[ch] = v;
                    self.y1[ch] = y;
                    v = y;
                }
                *x = v;
            }
        }
    }
}

pub struct Expander {
    ap: Biquad,
}

impl Expander {
    pub fn new() -> Expander {
        Expander { ap: Biquad::default() }
    }
}

impl Proc for Expander {
    fn process(&mut self, p: &Params, l: &mut [f32], r: &mut [f32], _ctx: &Ctx) {
        // Expand 0..1: 0 mono, 0.5 as it was, 1 twice as wide.
        let width = p.get("StereoWidth", 0.5) * 2.0;
        let surround = p.get("SurroundWidth", 0.0);
        let mono = p.text("MonoMixMode").unwrap_or("L+R");
        self.ap.set(Shape::AllPass, 800.0, 0.5, 0.0);
        for i in 0..l.len() {
            let mid = match mono {
                "L" => l[i],
                "R" => r[i],
                _ => (l[i] + r[i]) * 0.5,
            };
            let side = (l[i] - r[i]) * 0.5;
            let mut a = mid + side * width;
            let b = mid - side * width;
            if surround > 0.0 {
                // The surround: the left channel phase-shifted.
                let shifted = self.ap.run(0, a);
                a = a * (1.0 - surround) - shifted * surround;
            }
            l[i] = a;
            r[i] = b;
        }
    }
}
