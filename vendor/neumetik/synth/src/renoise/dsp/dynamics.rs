// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! The dynamics devices: the Compressor (and Renoise 1's Shaper), the
//! Bus Compressor (with a look-ahead and a soft knee), the Gate (both
//! versions, with its key filters and ducking) and the Maximizer.

use super::super::filter::{note_hz, Biquad, Shape};
use super::delay::Line;
use super::{Ctx, Params, Proc};

fn db(x: f32) -> f32 {
    20.0 * x.max(1e-9).log10()
}

fn undb(d: f32) -> f32 {
    10f32.powf(d / 20.0)
}

/// A one-pole time constant for `ms`.
fn coef(ms: f32, rate: f32) -> f32 {
    (-1.0 / (ms.max(0.001) * 0.001 * rate)).exp()
}

/// The shared compressor: an envelope follower (stereo-linked peak) and a
/// gain computer with a knee, in dB.
#[derive(Default)]
pub struct Comp {
    env: f32,
    gain_db: f32,
    look: Option<[Line; 2]>,
    /// Renoise 1's Shaper: its times are in frames.
    frames: bool,
    bus: bool,
}

impl Comp {
    pub fn new() -> Comp {
        Comp::default()
    }

    pub fn shaper() -> Comp {
        Comp { frames: true, ..Default::default() }
    }

    pub fn bus() -> Comp {
        Comp {
            bus: true,
            look: Some([Line::new(512), Line::new(512)]),
            ..Default::default()
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn run(&mut self, l: &mut [f32], r: &mut [f32], rate: f32, threshold_db: f32, ratio: f32, attack_ms: f32, release_ms: f32, makeup: f32, knee: f32) {
        let (ca, cr) = (coef(attack_ms, rate), coef(release_ms, rate));
        let look = 0.003 * rate;
        for i in 0..l.len() {
            let level = l[i].abs().max(r[i].abs());
            let c = if level > self.env { ca } else { cr };
            self.env = level + (self.env - level) * c;
            let over = db(self.env) - threshold_db;
            let red = if knee > 0.0 && over.abs() < knee / 2.0 {
                (1.0 / ratio - 1.0) * (over + knee / 2.0).powi(2) / (2.0 * knee)
            } else if over > 0.0 {
                over * (1.0 / ratio - 1.0)
            } else {
                0.0
            };
            self.gain_db = red;
            let g = undb(self.gain_db) * makeup;
            if let Some(lines) = self.look.as_mut() {
                let (a, b) = (lines[0].read(look), lines[1].read(look));
                lines[0].write(l[i]);
                lines[1].write(r[i]);
                l[i] = a * g;
                r[i] = b * g;
            } else {
                l[i] *= g;
                r[i] *= g;
            }
        }
    }
}

impl Proc for Comp {
    fn process(&mut self, p: &Params, l: &mut [f32], r: &mut [f32], ctx: &Ctx) {
        if self.bus {
            // Its ratio 0..1 runs 1:1 to limiting; attack and release 0..1 by decades.
            let ratio = 1.0 / (1.0 - p.get("Ratio", 0.5).clamp(0.0, 0.99));
            let attack = 0.01 * 10f32.powf(p.get("Attack", 0.3) * 4.0);
            let release = 1.0 * 10f32.powf(p.get("Release", 0.3) * 3.5);
            self.run(l, r, ctx.rate, p.get("Threshold", -12.0), ratio, attack, release, undb(p.get("MakeUp", 0.0)), p.get("Knee", 0.0));
        } else if self.frames {
            let t = db(p.get("Threshold", 16384.0) / 32768.0);
            let to_ms = 1000.0 / ctx.rate;
            self.run(
                l,
                r,
                ctx.rate,
                t,
                p.get("Ratio", 2.0).max(1.0),
                p.get("Attack", 100.0) * to_ms,
                p.get("Release", 20000.0) * to_ms,
                p.get("Gain", 1.0),
                0.0,
            );
        } else {
            // Its threshold: two dB down for each unit, and its makeup
            // gain at least unity (the songs' chains of compressors only
            // balance read so; Renoise 1's own scales aren't known).
            let t = -2.0 * p.get("LogThreshold", 6.0);
            self.run(
                l,
                r,
                ctx.rate,
                t,
                p.get("Ratio", 2.0).max(1.0),
                p.get("Attack", 1.0),
                p.get("Release", 30.0),
                p.get("Gain", 1.0).max(1.0),
                0.0,
            );
        }
    }
}

/// The Gate: Renoise 1's (linear threshold, times in ms) and 2's (dB,
/// key filters, a floor, ducking).
pub struct Gate {
    env: f32,
    gain: f32,
    hold: f32,
    hp: Biquad,
    lp: Biquad,
    v2: bool,
}

impl Gate {
    pub fn new(v2: bool) -> Gate {
        Gate {
            env: 0.0,
            gain: 0.0,
            hold: 0.0,
            hp: Biquad::default(),
            lp: Biquad::default(),
            v2,
        }
    }
}

impl Proc for Gate {
    fn process(&mut self, p: &Params, l: &mut [f32], r: &mut [f32], ctx: &Ctx) {
        let (threshold, attack, hold, release, floor, duck) = if self.v2 {
            (
                undb(p.get("Threshold", -40.0)),
                p.get("Attack", 1.0),
                p.get("Hold", 10.0),
                p.get("Release", 100.0),
                p.get("Floor", 0.0),
                p.get("Type", 0.0) >= 0.5,
            )
        } else {
            (
                p.get("Threshold", 0.1),
                p.get("AttackInMs", 1.0),
                p.get("HoldInMs", 10.0),
                p.get("ReleaseInMs", 100.0),
                p.get("GatedVolume", 0.0),
                false,
            )
        };
        if self.v2 {
            self.hp.set(Shape::High, note_hz(p.get("SideChainHpFreq", 0.0)), 0.707, 0.0);
            self.lp.set(Shape::Low, note_hz(p.get("SideChainLpFreq", 127.0)), 0.707, 0.0);
        }
        let (ca, cr) = (1.0 - coef(attack, ctx.rate), 1.0 - coef(release, ctx.rate));
        let det = coef(1.0, ctx.rate);
        let hold_frames = hold * ctx.rate / 1000.0;
        for i in 0..l.len() {
            let mut k = l[i].abs().max(r[i].abs());
            if self.v2 {
                let m = (l[i] + r[i]) * 0.5;
                k = self.lp.run(0, self.hp.run(0, m)).abs();
            }
            self.env = k.max(self.env * det);
            let open = (self.env >= threshold) != duck;
            if open {
                self.hold = hold_frames;
                self.gain += (1.0 - self.gain) * ca;
            } else if self.hold > 0.0 {
                self.hold -= 1.0;
            } else {
                self.gain += (0.0 - self.gain) * cr;
            }
            let g = floor + (1.0 - floor) * self.gain;
            l[i] *= g;
            r[i] *= g;
        }
    }
}

/// The Maximizer: boost, then a limiter at the threshold that looks a
/// little ahead (so a peak is caught before it passes) and lets go
/// quickly after short peaks and slowly after held ones; the ceiling last.
pub struct Maximizer {
    gain: f32,
    slow: f32,
    hist: [Vec<f32>; 2],
    at: usize,
}

/// The look-ahead, in frames.
const AHEAD: usize = 64;

impl Maximizer {
    pub fn new() -> Maximizer {
        Maximizer {
            gain: 1.0,
            slow: 1.0,
            hist: [vec![0.0; AHEAD], vec![0.0; AHEAD]],
            at: 0,
        }
    }
}

impl Proc for Maximizer {
    fn process(&mut self, p: &Params, l: &mut [f32], r: &mut [f32], ctx: &Ctx) {
        let boost = undb(p.get("InputGain", 0.0));
        let threshold = undb(p.get("Threshold", -0.5)).max(1e-4);
        let ceiling = undb(p.get("Ceiling", -0.3));
        let fast = 1.0 - coef(p.get("TransientRelease", 5.0), ctx.rate);
        let slow = 1.0 - coef(p.get("LongTermRelease", 200.0), ctx.rate);
        for i in 0..l.len() {
            let (xl, xr) = (l[i] * boost, r[i] * boost);
            // The oldest point goes out as the newest comes in.
            let (yl, yr) = (self.hist[0][self.at], self.hist[1][self.at]);
            self.hist[0][self.at] = xl;
            self.hist[1][self.at] = xr;
            self.at = (self.at + 1) % AHEAD;
            let peak = self.hist[0].iter().chain(self.hist[1].iter()).fold(0.0f32, |a, &v| a.max(v.abs()));
            let want = if peak > threshold { threshold / peak } else { 1.0 };
            if want < self.gain {
                self.gain = want;
            } else {
                // Short peaks let go fast; the slow level follows behind.
                self.slow += (want - self.slow) * slow;
                let target = want.min(self.slow.max(self.gain));
                self.gain += (target - self.gain) * fast;
                self.gain = self.gain.min(want);
            }
            self.slow = self.slow.min(1.0).max(self.gain);
            let g = self.gain * ceiling / threshold;
            l[i] = (yl * g).clamp(-ceiling, ceiling);
            r[i] = (yr * g).clamp(-ceiling, ceiling);
        }
    }
}
