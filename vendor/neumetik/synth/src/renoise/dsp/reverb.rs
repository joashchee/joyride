// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! The reverbs: Reverb (a room of combs and all-passes, Schroeder and
//! Moorer's design as Freeverb lays it out), mpReverb in both its
//! versions (a feedback delay network whose decay is the reverberation
//! time, damped by a low-pass) and the Convolver (the song's own impulse
//! response, convolved in partitions through an FFT).

use std::f32::consts::PI;

use super::super::filter::{Biquad, Shape};
use super::delay::Line;
use super::{balance, Ctx, Params, Proc};

const COMBS: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const ALLPASSES: [usize; 4] = [556, 441, 341, 225];
const SPREAD: usize = 23;

#[derive(Clone)]
struct Comb {
    buf: Vec<f32>,
    at: usize,
    store: f32,
}

impl Comb {
    fn new(n: usize) -> Comb {
        Comb { buf: vec![0.0; n], at: 0, store: 0.0 }
    }

    #[inline]
    fn run(&mut self, x: f32, feedback: f32, damp: f32) -> f32 {
        let y = self.buf[self.at];
        self.store = y * (1.0 - damp) + self.store * damp;
        self.buf[self.at] = x + self.store * feedback;
        self.at = (self.at + 1) % self.buf.len();
        y
    }
}

#[derive(Clone)]
struct AllPass {
    buf: Vec<f32>,
    at: usize,
}

impl AllPass {
    fn new(n: usize) -> AllPass {
        AllPass { buf: vec![0.0; n], at: 0 }
    }

    #[inline]
    fn run(&mut self, x: f32) -> f32 {
        let b = self.buf[self.at];
        self.buf[self.at] = x + b * 0.5;
        self.at = (self.at + 1) % self.buf.len();
        b - x
    }
}

pub struct Reverb {
    combs: [Vec<Comb>; 2],
    alls: [Vec<AllPass>; 2],
}

impl Reverb {
    pub fn new() -> Reverb {
        let side = |s: usize| (COMBS.iter().map(|&n| Comb::new(n + s)).collect(), ALLPASSES.iter().map(|&n| AllPass::new(n + s)).collect());
        let (c0, a0) = side(0);
        let (c1, a1) = side(SPREAD);
        Reverb { combs: [c0, c1], alls: [a0, a1] }
    }
}

impl Proc for Reverb {
    fn process(&mut self, p: &Params, l: &mut [f32], r: &mut [f32], _ctx: &Ctx) {
        let wet = p.get("Send", 0.3);
        let room = p.get("RoomSize", 0.5) * 0.28 + 0.7;
        let damp = p.get("Damp", 0.5) * 0.4;
        let width = p.get("Width", 1.0);
        let dry = p.get("DryMix", 1.0);
        let (w1, w2) = (wet * (width / 2.0 + 0.5), wet * ((1.0 - width) / 2.0));
        for i in 0..l.len() {
            let x = (l[i] + r[i]) * 0.015;
            let mut out = [0.0f32; 2];
            for ch in 0..2 {
                let mut s = 0.0;
                for c in self.combs[ch].iter_mut() {
                    s += c.run(x, room, damp);
                }
                for a in self.alls[ch].iter_mut() {
                    s = a.run(s);
                }
                out[ch] = s;
            }
            l[i] = l[i] * dry + out[0] * w1 + out[1] * w2;
            r[i] = r[i] * dry + out[1] * w1 + out[0] * w2;
        }
    }
}

/// Eight lines of a feedback delay network (frames at 44.1 kHz).
const FDN: [usize; 8] = [1427, 1871, 2251, 2647, 3001, 3343, 3779, 4133];

/// mpReverb, both versions: a feedback delay network.
pub struct MpReverb {
    lines: Vec<Line>,
    lp: [f32; 8],
    pre: [Line; 2],
    color: Biquad,
    v3: bool,
}

impl MpReverb {
    pub fn new(v3: bool) -> MpReverb {
        MpReverb {
            lines: FDN.iter().map(|&n| Line::new(n * 2 + 8)).collect(),
            lp: [0.0; 8],
            pre: [Line::new(44100 / 4 + 16), Line::new(44100 / 4 + 16)],
            color: Biquad::default(),
            v3,
        }
    }
}

impl Proc for MpReverb {
    fn process(&mut self, p: &Params, l: &mut [f32], r: &mut [f32], ctx: &Ctx) {
        // Its time: tenths of a second until it's 60 dB down.
        let rt = (p.get("ReverbTime", 15.0) * 0.1).max(0.05);
        let pre = (p.get("PreDelay", 0.0) * ctx.rate / 1000.0).max(1.0);
        let cutoff = p.get("LPFilter", 4000.0).clamp(10.0, 20000.0);
        // The damping filter's gain: how much above its cutoff gets through.
        let lp_gain = if self.v3 { 10f32.powf(p.get("LPGain", -6.0) / 20.0).min(1.0) } else { 0.0 };
        let (wet, dry) = if self.v3 {
            (p.get("WetMix", 0.5), p.get("DryMix", 1.0))
        } else {
            (p.get("Send", 0.5), p.get("DryMix", 1.0))
        };
        let width = if self.v3 { p.get("Width", 1.0) } else { 1.0 };
        let (pl, pr) = if self.v3 { balance(p.get("Pan", 0.5)) } else { (1.0, 1.0) };
        if self.v3 {
            self.color.set(Shape::Peak, 1200.0, 0.7, (p.get("Color", 0.5) - 0.5) * 12.0);
        }
        let a = (-2.0 * PI * cutoff / ctx.rate).exp();
        let gains: Vec<f32> = FDN.iter().map(|&n| 10f32.powf(-3.0 * n as f32 / (rt * ctx.rate))).collect();
        for i in 0..l.len() {
            let (xl, xr) = (self.pre[0].read(pre), self.pre[1].read(pre));
            self.pre[0].write(l[i]);
            self.pre[1].write(r[i]);
            let mut y = [0.0f32; 8];
            for k in 0..8 {
                y[k] = self.lines[k].read(FDN[k] as f32);
            }
            // A Householder reflection mixes the lines.
            let sum: f32 = y.iter().sum::<f32>() * 0.25;
            let (mut ol, mut or) = (0.0, 0.0);
            for k in 0..8 {
                let mut v = (y[k] - sum) * gains[k];
                // The damping: a low-pass whose loss the low gain sets.
                self.lp[k] = v * (1.0 - a) + self.lp[k] * a;
                v = self.lp[k] + (v - self.lp[k]) * lp_gain;
                let input = if k % 2 == 0 { xl } else { xr };
                self.lines[k].write(v + input * 0.35);
                if k % 2 == 0 {
                    ol += y[k];
                } else {
                    or += y[k];
                }
            }
            let (mut ol, mut or) = (ol * 0.3, or * 0.3);
            if self.v3 {
                ol = self.color.run(0, ol);
                or = self.color.run(1, or);
                let mid = (ol + or) * 0.5;
                ol = mid + (ol - mid) * width;
                or = mid + (or - mid) * width;
            }
            l[i] = l[i] * dry + ol * wet * pl;
            r[i] = r[i] * dry + or * wet * pr;
        }
    }
}

/// A complex FFT of a power-of-two length, in place (radix 2).
pub fn fft(re: &mut [f32], im: &mut [f32], inverse: bool) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = 2.0 * PI / len as f32 * if inverse { 1.0 } else { -1.0 };
        let (wr, wi) = (ang.cos(), ang.sin());
        for i in (0..n).step_by(len) {
            let (mut cr, mut ci) = (1.0f32, 0.0f32);
            for k in 0..len / 2 {
                let (a, b) = (i + k, i + k + len / 2);
                let (tr, ti) = (re[b] * cr - im[b] * ci, re[b] * ci + im[b] * cr);
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
                let nr = cr * wr - ci * wi;
                ci = cr * wi + ci * wr;
                cr = nr;
            }
        }
        len <<= 1;
    }
    if inverse {
        let s = 1.0 / n as f32;
        for v in re.iter_mut().chain(im.iter_mut()) {
            *v *= s;
        }
    }
}

const PART: usize = 512;

/// One channel's partitioned convolution.
struct Conv {
    /// The impulse's partitions in the frequency domain.
    parts: Vec<(Vec<f32>, Vec<f32>)>,
    /// Past input blocks in the frequency domain, newest first.
    history: Vec<(Vec<f32>, Vec<f32>)>,
    input: Vec<f32>,
    output: Vec<f32>,
    overlap: Vec<f32>,
    fill: usize,
}

impl Conv {
    fn new(ir: &[f32]) -> Conv {
        let parts: Vec<_> = ir
            .chunks(PART)
            .map(|c| {
                let mut re = vec![0.0; PART * 2];
                let mut im = vec![0.0; PART * 2];
                re[..c.len()].copy_from_slice(c);
                fft(&mut re, &mut im, false);
                (re, im)
            })
            .collect();
        let n = parts.len();
        Conv {
            parts,
            history: vec![(vec![0.0; PART * 2], vec![0.0; PART * 2]); n],
            input: vec![0.0; PART],
            output: vec![0.0; PART],
            overlap: vec![0.0; PART],
            fill: 0,
        }
    }

    fn run(&mut self, x: f32) -> f32 {
        self.input[self.fill] = x;
        let y = self.output[self.fill];
        self.fill += 1;
        if self.fill == PART {
            self.fill = 0;
            let mut re = vec![0.0; PART * 2];
            let mut im = vec![0.0; PART * 2];
            re[..PART].copy_from_slice(&self.input);
            fft(&mut re, &mut im, false);
            self.history.rotate_right(1);
            self.history[0] = (re, im);
            let mut ar = vec![0.0; PART * 2];
            let mut ai = vec![0.0; PART * 2];
            for (h, p) in self.history.iter().zip(&self.parts) {
                for k in 0..PART * 2 {
                    ar[k] += h.0[k] * p.0[k] - h.1[k] * p.1[k];
                    ai[k] += h.0[k] * p.1[k] + h.1[k] * p.0[k];
                }
            }
            fft(&mut ar, &mut ai, true);
            for k in 0..PART {
                self.output[k] = ar[k] + self.overlap[k];
                self.overlap[k] = ar[k + PART];
            }
        }
        y
    }
}

/// The Convolver: the song's impulse response (as Renoise keeps it in
/// the song), its start, length and resampling applied when it's loaded.
pub struct Convolver {
    conv: Option<[Conv; 2]>,
    pre: [Line; 2],
    color: Biquad,
}

impl Convolver {
    pub fn new() -> Convolver {
        Convolver {
            conv: None,
            pre: [Line::new(2 * 44100 + 8), Line::new(2 * 44100 + 8)],
            color: Biquad::default(),
        }
    }
}

fn floats(b64: &str) -> Vec<f32> {
    crate::xml::base64(b64).chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
}

impl Proc for Convolver {
    fn process(&mut self, p: &Params, l: &mut [f32], r: &mut [f32], ctx: &Ctx) {
        if self.conv.is_none() {
            let left = floats(p.text("ImpulseDataLeft").unwrap_or(""));
            let mut right = floats(p.text("ImpulseDataRight").unwrap_or(""));
            if right.is_empty() || !p.flag("Stereo") {
                right = left.clone();
            }
            let prep = |v: &[f32]| -> Vec<f32> {
                let start = (p.get("Start", 0.0).clamp(0.0, 1.0) * v.len() as f32) as usize;
                let len = (p.get("Length", 1.0).clamp(0.0, 1.0) * (v.len() - start.min(v.len())) as f32) as usize;
                let v = &v[start.min(v.len())..(start + len).min(v.len())];
                // Resample: 0.5 the impulse's own speed? Read 1.0 as itself.
                let speed = (p.get("Resample", 0.5) * 2.0).clamp(0.25, 4.0);
                let n = ((v.len() as f32 / speed) as usize).min(4 * 44100);
                (0..n).map(|i| v.get((i as f32 * speed) as usize).copied().unwrap_or(0.0)).collect()
            };
            self.conv = Some([Conv::new(&prep(&left)), Conv::new(&prep(&right))]);
        }
        let gain = p.get("Gain", 0.5);
        let (dry, wet) = (p.get("Dry", 1.0), p.get("Wet", 0.3));
        let pre = (p.get("PreDelay", 0.0) * ctx.rate / 1000.0).max(1.0);
        self.color.set(Shape::Peak, 1000.0, 0.7, (p.get("Color", 0.5) - 0.5) * 12.0);
        let Some(conv) = self.conv.as_mut() else { return };
        for i in 0..l.len() {
            let (xl, xr) = (self.pre[0].read(pre), self.pre[1].read(pre));
            self.pre[0].write(l[i]);
            self.pre[1].write(r[i]);
            let yl = self.color.run(0, conv[0].run(xl) * gain);
            let yr = self.color.run(1, conv[1].run(xr) * gain);
            l[i] = l[i] * dry + yl * wet;
            r[i] = r[i] * dry + yr * wet;
        }
    }
}
