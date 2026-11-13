// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! The Delay (two lines, each fed by its side, with feedback, an output
//! pan each and a line-synced mode), the Multitap Delay (four taps, each
//! feeding the next, filtered) and the Repeater (a captured slice repeated).

use super::super::filter::{Biquad, Shape};
use super::{balance, Ctx, Params, Proc};

/// A delay line with a fractional read and a smoothly moving length (the
/// manual's tape-like scrub when the time changes).
#[derive(Clone, Default)]
pub struct Line {
    buf: Vec<f32>,
    at: usize,
    len: f32,
}

impl Line {
    pub fn new(max: usize) -> Line {
        Line {
            buf: vec![0.0; max.max(4)],
            at: 0,
            len: 0.0,
        }
    }

    /// The point `len` frames back (fractional), the length gliding to `want`.
    pub fn read(&mut self, want: f32) -> f32 {
        let max = (self.buf.len() - 2) as f32;
        let want = want.clamp(1.0, max);
        if self.len == 0.0 {
            self.len = want;
        }
        self.len += (want - self.len) * 0.0005;
        let back = self.len;
        let n = self.buf.len();
        let pos = self.at as f32 + n as f32 - back;
        let i = pos.floor();
        let f = pos - i;
        let a = self.buf[i as usize % n];
        let b = self.buf[(i as usize + 1) % n];
        a + (b - a) * f
    }

    pub fn write(&mut self, x: f32) {
        self.buf[self.at] = if x.is_finite() { x } else { 0.0 };
        self.at = (self.at + 1) % self.buf.len();
    }
}

pub struct Delay {
    l: Line,
    r: Line,
}

impl Delay {
    pub fn new() -> Delay {
        Delay {
            l: Line::new(2 * 44100 + 64),
            r: Line::new(2 * 44100 + 64),
        }
    }
}

impl Proc for Delay {
    fn process(&mut self, p: &Params, l: &mut [f32], r: &mut [f32], ctx: &Ctx) {
        let send = p.get("TrackSend", 127.0) / 127.0;
        let (fl, fr) = (p.get("LFeedback", 0.3).clamp(0.0, 1.0), p.get("RFeedback", 0.3).clamp(0.0, 1.0));
        let dry = if p.get("MuteDrySignal", 0.0) >= 0.5 { 0.0 } else { 1.0 };
        let (dl, dr) = if p.get("LineSync", 0.0) >= 0.5 {
            let line = ctx.line * 1000.0;
            (
                p.get("LSyncTime", 1.0).max(1.0) * line * (1.0 + p.get("LSyncOffset", 0.0)),
                p.get("RSyncTime", 1.0).max(1.0) * line * (1.0 + p.get("RSyncOffset", 0.0)),
            )
        } else {
            (p.get("LDelay", 250.0), p.get("RDelay", 250.0))
        };
        let (dl, dr) = (dl.min(2000.0) * ctx.rate / 1000.0, dr.min(2000.0) * ctx.rate / 1000.0);
        let (lp, rp) = (balance(p.get("LTapPan", 0.0)), balance(p.get("RTapPan", 1.0)));
        for i in 0..l.len() {
            let (a, b) = (self.l.read(dl), self.r.read(dr));
            self.l.write(l[i] * send + a * fl);
            self.r.write(r[i] * send + b * fr);
            // Each line's echoes to its output pan.
            l[i] = l[i] * dry + a * lp.0 + b * rp.0;
            r[i] = r[i] * dry + a * lp.1 + b * rp.1;
        }
    }
}

/// The Multitap Delay: four taps, each fed by the input and the previous
/// tap, with a filter where its settings put it.
pub struct Multitap {
    lines: [[Line; 2]; 4],
    filters: [Biquad; 4],
    last: [[f32; 2]; 4],
}

impl Multitap {
    pub fn new() -> Multitap {
        let mk = || [Line::new(5 * 44100 + 64), Line::new(5 * 44100 + 64)];
        Multitap {
            lines: [mk(), mk(), mk(), mk()],
            filters: [Biquad::default(); 4],
            last: [[0.0; 2]; 4],
        }
    }
}

impl Proc for Multitap {
    fn process(&mut self, p: &Params, l: &mut [f32], r: &mut [f32], ctx: &Ctx) {
        let dry = if p.get("MuteDrySignal", 0.0) >= 0.5 { 0.0 } else { 1.0 };
        struct Tap {
            d: [f32; 2],
            input: f32,
            amount: f32,
            fb: [f32; 2],
            prev: f32,
            pan: [(f32, f32); 2],
            mode: u8,
        }
        let mut taps = Vec::new();
        for k in 0..4 {
            let n = |s: &str| format!("Tap{}{}", k + 1, s);
            let sync = p.get(&n("LineSync"), 0.0) >= 0.5;
            let d = if sync {
                let line = ctx.line * 1000.0;
                [
                    p.get(&n("LSyncTime"), 1.0).max(1.0) * line * (1.0 + p.get(&n("LSyncOffset"), 0.0)),
                    p.get(&n("RSyncTime"), 1.0).max(1.0) * line * (1.0 + p.get(&n("RSyncOffset"), 0.0)),
                ]
            } else {
                [p.get(&n("DelayLeft"), 250.0), p.get(&n("DelayRight"), 250.0)]
            };
            let inv = if p.get(&n("InvertFeedback"), 0.0) >= 0.5 { -1.0 } else { 1.0 };
            let ftype = p.get(&n("FilterType"), 0.0).round() as usize;
            let shape = [Shape::Low, Shape::High, Shape::Band, Shape::Notch][ftype.min(3)];
            let f = 20.0 * 1000f32.powf(p.get(&n("FilterFreq"), 1.0));
            self.filters[k].set(shape, f, 0.707 + p.get(&n("FilterResonance"), 0.0), 0.0);
            taps.push(Tap {
                d: [d[0].clamp(1.0, 5000.0) * ctx.rate / 1000.0, d[1].clamp(1.0, 5000.0) * ctx.rate / 1000.0],
                input: p.get(&n("InputAmount"), 1.0),
                amount: p.get(&n("Amount"), if k == 0 { 1.0 } else { 0.0 }),
                fb: [p.get(&n("LFeedback"), 0.0) * inv, p.get(&n("RFeedback"), 0.0) * inv],
                prev: if k > 0 { p.get(&n("PreviousTapInput"), 0.0) } else { 0.0 },
                pan: [balance(p.get(&n("LeftTapPan"), 0.0)), balance(p.get(&n("RightTapPan"), 1.0))],
                mode: p.get(&n("FilterMode"), 0.0).round() as u8,
            });
        }
        for i in 0..l.len() {
            let x = [l[i], r[i]];
            let mut out = [0.0f32; 2];
            for (k, t) in taps.iter().enumerate() {
                let prev = if k > 0 { self.last[k - 1] } else { [0.0; 2] };
                let mut y = [0.0f32; 2];
                for ch in 0..2 {
                    let echo = self.lines[k][ch].read(t.d[ch]);
                    let mut inp = x[ch] * t.input + prev[ch] * t.prev;
                    // Filter positions: 1 input, 2 feedback, 3 both, 4 output.
                    if t.mode == 1 || t.mode == 3 {
                        inp = self.filters[k].run(ch, inp);
                    }
                    let mut fb = echo * t.fb[ch];
                    if t.mode == 2 || t.mode == 3 {
                        fb = self.filters[k].run(ch, fb);
                    }
                    self.lines[k][ch].write(inp + fb);
                    y[ch] = if t.mode == 4 { self.filters[k].run(ch, echo) } else { echo };
                }
                self.last[k] = y;
                for ch in 0..2 {
                    out[0] += y[ch] * t.amount * t.pan[ch].0;
                    out[1] += y[ch] * t.amount * t.pan[ch].1;
                }
            }
            l[i] = l[i] * dry + out[0];
            r[i] = r[i] * dry + out[1];
        }
    }
}

/// The Repeater: while on, a slice of a whole note divided by its divisor
/// is captured and repeated.
pub struct Repeater {
    buf: [Vec<f32>; 2],
    at: usize,
    capturing: bool,
}

impl Repeater {
    pub fn new() -> Repeater {
        Repeater {
            buf: [Vec::new(), Vec::new()],
            at: 0,
            capturing: true,
        }
    }
}

impl Proc for Repeater {
    fn process(&mut self, p: &Params, l: &mut [f32], r: &mut [f32], ctx: &Ctx) {
        let mode = p.get("Mode", 0.0).round() as i32;
        if mode == 0 {
            self.buf[0].clear();
            self.buf[1].clear();
            self.capturing = true;
            return;
        }
        let whole = 4.0 * 60.0 / ctx.bpm.max(1.0);
        let div = p.get("Divisor", 0.25).max(1.0 / 128.0);
        let factor = match mode {
            3 => 2.0 / 3.0,
            4 => 1.5,
            _ => 1.0,
        };
        let n = ((whole * div * factor * ctx.rate) as usize).clamp(16, 4 * 44100);
        for i in 0..l.len() {
            if self.capturing {
                self.buf[0].push(l[i]);
                self.buf[1].push(r[i]);
                if self.buf[0].len() >= n {
                    self.capturing = false;
                    self.at = 0;
                }
                continue;
            }
            l[i] = self.buf[0][self.at % self.buf[0].len()];
            r[i] = self.buf[1][self.at % self.buf[1].len()];
            self.at += 1;
        }
    }
}
