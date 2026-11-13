// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! A voice's modulation: Renoise 1 and 2's instrument envelopes (volume,
//! panning, pitch, cutoff and resonance, each with its LFOs, sustain,
//! loop and the volume's fadeout), and Renoise 3's modulation sets
//! (chains of AHDSR, envelope, fader, LFO, key and velocity tracking,
//! operand and stepper devices for each domain).
//!
//! Their time scales are the manuals': envelopes count 24 ticks a beat;
//! the modulation devices' 0..1 times are read as up to 20 seconds
//! (cubed), or up to 32 beats when synced.

use std::f32::consts::TAU;
use std::sync::Arc;

use super::song::{Device, Envelope, Envelopes, Interp, Loop, ModSet};

/// What a voice's modulation gives it now.
#[derive(Clone, Copy, Debug)]
pub struct Out {
    pub volume: f32,
    /// -1..1 added to its panning.
    pub pan: f32,
    pub pitch: f32,
    /// 0..1, when a filter is modulated.
    pub cutoff: f32,
    pub reso: f32,
    pub drive: f32,
    /// The volume has ended (after a release): the voice can stop.
    pub done: bool,
}

impl Default for Out {
    fn default() -> Out {
        Out {
            volume: 1.0,
            pan: 0.0,
            pitch: 0.0,
            cutoff: 1.0,
            reso: 0.0,
            drive: 0.0,
            done: false,
        }
    }
}

/// A point of an envelope's curve at `t` (in its own ticks).
pub(crate) fn curve(points: &[(f32, f32)], t: f32, interp: Interp, fallback: f32) -> f32 {
    if points.is_empty() {
        return fallback;
    }
    if t <= points[0].0 {
        return points[0].1;
    }
    for w in points.windows(2) {
        let ((t0, v0), (t1, v1)) = (w[0], w[1]);
        if t < t1 {
            let f = if t1 > t0 { (t - t0) / (t1 - t0) } else { 1.0 };
            return match interp {
                Interp::Points => v0,
                Interp::Linear => v0 + (v1 - v0) * f,
                // Eased in and out.
                Interp::Curve => v0 + (v1 - v0) * (f * f * (3.0 - 2.0 * f)),
            };
        }
    }
    points[points.len() - 1].1
}

/// Where an envelope is: its position in ticks, moving through its loop.
#[derive(Clone, Copy, Debug, Default)]
struct Run {
    pos: f32,
    back: bool,
}

impl Run {
    fn advance(&mut self, dt: f32, sustain: Option<usize>, looping: Option<(Loop, usize, usize)>, released: bool, length: f32) {
        // Held at the sustain point until released.
        if let (Some(s), false) = (sustain, released) {
            let s = s as f32;
            if self.pos <= s {
                self.pos = (self.pos + dt).min(s);
                return;
            }
        }
        if let Some((mode, ls, le)) = looping {
            let (ls, le) = (ls as f32, le as f32);
            if le > ls && self.pos <= le {
                if self.back {
                    self.pos -= dt;
                    if self.pos <= ls {
                        if mode == Loop::PingPong {
                            self.pos = (ls + (ls - self.pos)).min(le);
                            self.back = false;
                        } else {
                            self.pos = (le - (ls - self.pos)).max(ls);
                        }
                    }
                    return;
                }
                self.pos += dt;
                if self.pos >= le {
                    match mode {
                        Loop::Forward => self.pos = ls + (self.pos - le) % (le - ls),
                        Loop::Backward | Loop::PingPong => {
                            self.pos = (le - (self.pos - le)).max(ls);
                            self.back = true;
                        }
                        Loop::Off => {}
                    }
                }
                return;
            }
        }
        self.pos = (self.pos + dt).min(length.max(0.0));
    }
}

/// An LFO's value (-1..1) at `phase` (cycles).
fn lfo_wave(mode: u8, phase: f32, random: f32) -> f32 {
    let p = phase.fract();
    match mode {
        1 => (p * TAU).sin(),
        2 => 1.0 - 2.0 * p,
        3 => {
            if p < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
        4 => random,
        _ => 0.0,
    }
}

/// A small random generator (xorshift), seeded per voice.
#[derive(Clone, Copy, Debug)]
pub struct Rng(pub u32);

impl Rng {
    pub fn next(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

/// Renoise 1 and 2's envelopes running for a voice.
#[derive(Clone, Debug)]
pub struct OldEnv {
    def: Arc<Envelopes>,
    runs: [Run; 5],
    fade: f32,
    /// LFO phases (cycles) and their random values.
    lfo: [[f32; 2]; 5],
    random: [[f32; 2]; 5],
    rng: Rng,
    points: [Vec<(f32, f32)>; 5],
}

impl OldEnv {
    pub fn new(def: Arc<Envelopes>, seed: u32) -> OldEnv {
        let pts = |e: &Envelope| e.points.iter().map(|&(t, v)| (t as f32, v)).collect::<Vec<_>>();
        let points = [pts(&def.volume), pts(&def.pan), pts(&def.pitch), pts(&def.cutoff), pts(&def.resonance)];
        let mut lfo = [[0.0; 2]; 5];
        for (k, e) in [&def.volume, &def.pan, &def.pitch, &def.cutoff, &def.resonance].iter().enumerate() {
            for (j, l) in e.lfos.iter().take(2).enumerate() {
                lfo[k][j] = l.phase as f32 / 360.0;
            }
        }
        OldEnv {
            def,
            runs: [Run::default(); 5],
            fade: 1.0,
            lfo,
            random: [[0.0; 2]; 5],
            rng: Rng(seed | 1),
            points,
        }
    }

    fn env(def: &Envelopes, k: usize) -> &Envelope {
        match k {
            0 => &def.volume,
            1 => &def.pan,
            2 => &def.pitch,
            3 => &def.cutoff,
            _ => &def.resonance,
        }
    }

    pub fn filter_type(&self) -> Option<i32> {
        (self.def.cutoff.on || self.def.resonance.on).then_some(self.def.filter_type)
    }

    /// Moves every envelope to `ticks` (the E command), fadeout undone.
    pub fn seek(&mut self, ticks: f32) {
        for r in self.runs.iter_mut() {
            *r = Run { pos: ticks, back: false };
        }
        self.fade = 1.0;
    }

    pub fn volume_on(&self) -> bool {
        self.def.volume.on
    }

    /// Moves `ticks` envelope ticks (24 a beat) on and gives the values.
    pub fn step(&mut self, ticks: f32, released: bool) -> Out {
        let mut out = Out::default();
        let mut vals = [1.0f32, 0.5, 0.5, 1.0, 0.0];
        let def = self.def.clone();
        for (k, v) in vals.iter_mut().enumerate() {
            let e = OldEnv::env(&def, k);
            if !e.on && e.lfos.iter().all(|l| l.mode == 0) {
                continue;
            }
            if e.on {
                let looping = e.loop_mode.map(|m| (m, e.loop_range.0, e.loop_range.1));
                *v = curve(&self.points[k], self.runs[k].pos, e.interp.unwrap_or(Interp::Curve), *v);
                self.runs[k].advance(ticks, e.sustain, looping, released, e.length.saturating_sub(1) as f32);
            }
            // LFOs run whether the envelope is on or not.
            for (j, l) in e.lfos.iter().take(2).enumerate() {
                if l.mode == 0 {
                    continue;
                }
                let before = self.lfo[k][j];
                // Frequency: sixteenths of a cycle a beat (24 ticks).
                self.lfo[k][j] += ticks / 24.0 * l.freq as f32 / 16.0;
                if l.mode == 4 && self.lfo[k][j].floor() != before.floor() {
                    self.random[k][j] = self.rng.next();
                }
                let w = lfo_wave(l.mode, self.lfo[k][j], self.random[k][j]);
                let amp = l.amp as f32 / 100.0;
                match k {
                    0 => *v *= 1.0 - amp * 0.5 * (1.0 - w),
                    _ => *v += w * amp * 0.5,
                }
            }
        }
        if released && self.def.volume.on {
            self.fade -= ticks * self.def.volume.decay.max(0) as f32 / 4096.0;
            if self.fade <= 0.0 {
                self.fade = 0.0;
                out.done = true;
            }
        }
        out.volume = vals[0].clamp(0.0, 1.0) * self.fade;
        out.pan = (vals[1] - 0.5) * 2.0;
        out.pitch = (vals[2] - 0.5) * 24.0;
        out.cutoff = vals[3].clamp(0.0, 1.0);
        out.reso = vals[4].clamp(0.0, 1.0);
        // A volume envelope that ended at silence ends the voice.
        let v = &self.def.volume;
        if v.on && v.loop_mode.is_none() && v.sustain.is_none() && self.runs[0].pos >= v.length.saturating_sub(1) as f32 && vals[0] <= 0.0 {
            out.done = true;
        }
        out
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    Volume,
    Pan,
    Pitch,
    Cutoff,
    Reso,
    Drive,
}

#[derive(Clone, Copy, Debug)]
enum Op {
    Add,
    Sub,
    Mul,
    Div,
}

/// A modulation device's state in a voice.
#[derive(Clone, Debug, Default)]
struct DevRun {
    t: f32,
    released_at: Option<f32>,
    /// The level when released (AHDSR, envelope).
    release_from: f32,
    level: f32,
    phase: f32,
    random: f32,
    run: Run,
    fade: f32,
}

/// A modulation set's time scale: seconds for a 0..1 time.
pub fn mod_seconds(n: f32, synced: bool, beat: f32) -> f32 {
    let n = n.clamp(0.0, 1.0);
    if synced {
        n * n * n * 32.0 * beat
    } else {
        n * n * n * 20.0
    }
}

/// Renoise 3's modulation set running for a voice.
#[derive(Clone, Debug)]
pub struct Mods {
    devices: Vec<(Device, Target, Op, bool)>,
    runs: Vec<DevRun>,
    input: [f32; 6],
    pub pitch_range: f32,
    note: f32,
    velocity: f32,
    rng: Rng,
    has_volume_env: bool,
}

impl Mods {
    pub fn new(set: &ModSet, note: u8, velocity: f32, seed: u32) -> Mods {
        let mut input = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
        let mut devices = Vec::new();
        for d in &set.devices {
            if d.kind == "SampleMixerModulationDevice" {
                input = [
                    d.get("Volume", 1.0),
                    d.get("Panning", 0.0),
                    d.get("Pitch", 0.0),
                    d.get("Cutoff", 127.0) / 127.0,
                    d.get("Resonance", 0.0) / 127.0,
                    d.get("Drive", 0.0),
                ];
                continue;
            }
            if !d.active {
                continue;
            }
            let target = match d.text("Target").unwrap_or("Volume") {
                "Panning" => Target::Pan,
                "Pitch" => Target::Pitch,
                "Cutoff" => Target::Cutoff,
                "Resonance" => Target::Reso,
                "Drive" => Target::Drive,
                _ => Target::Volume,
            };
            let op = match d.text("Operator").unwrap_or("*") {
                "+" => Op::Add,
                "-" => Op::Sub,
                "/" => Op::Div,
                _ => Op::Mul,
            };
            devices.push((d.clone(), target, op, d.flag("Bipolar")));
        }
        let has_volume_env = devices
            .iter()
            .any(|(d, t, ..)| *t == Target::Volume && matches!(d.kind.as_str(), "SampleAhdsrModulationDevice" | "SampleEnvelopeModulationDevice" | "SampleFaderModulationDevice"));
        let n = devices.len();
        let mut rng = Rng(seed | 1);
        let runs = (0..n)
            .map(|k| {
                let d = &devices[k].0;
                DevRun {
                    phase: d.get("Dephase", 0.0) / 360.0,
                    random: rng.next(),
                    fade: 1.0,
                    ..Default::default()
                }
            })
            .collect();
        Mods {
            devices,
            runs,
            input,
            pitch_range: set.pitch_range.max(1.0),
            note: f32::from(note),
            velocity,
            rng,
            has_volume_env,
        }
    }

    /// Every device starts again.
    pub fn restart(&mut self) {
        for (k, r) in self.runs.iter_mut().enumerate() {
            *r = DevRun {
                phase: self.devices[k].0.get("Dephase", 0.0) / 360.0,
                random: r.random,
                fade: 1.0,
                ..Default::default()
            };
        }
    }

    /// Whether a note off should end the voice at once (no volume
    /// envelope to release).
    pub fn holds_release(&self) -> bool {
        self.has_volume_env
    }

    /// Moves `dt` seconds on (a beat lasting `beat` seconds) and gives the values.
    pub fn step(&mut self, dt: f32, beat: f32, released: bool) -> Out {
        let mut v = self.input;
        let mut done = false;
        for k in 0..self.devices.len() {
            let (d, target, op, bipolar) = &self.devices[k];
            let synced = d.flag("TempoSynced");
            let r = &mut self.runs[k];
            if released && r.released_at.is_none() {
                r.released_at = Some(r.t);
                r.release_from = r.level;
            }
            let secs = |n: f32| mod_seconds(n, synced, beat);
            let mut o = match d.kind.as_str() {
                "SampleAhdsrModulationDevice" => {
                    let (a, h, dc, s, rl) = (
                        secs(d.get("Attack", 0.0)),
                        secs(d.get("Hold", 0.0)),
                        secs(d.get("Decay", 0.3)),
                        d.get("Sustain", 1.0),
                        secs(d.get("Release", 0.2)),
                    );
                    let lvl = if let Some(at) = r.released_at {
                        let x = r.t - at;
                        if x >= rl {
                            if *target == Target::Volume {
                                done = true;
                            }
                            0.0
                        } else {
                            r.release_from * (1.0 - x / rl.max(1e-4))
                        }
                    } else if r.t < a {
                        r.t / a.max(1e-4)
                    } else if r.t < a + h {
                        1.0
                    } else if r.t < a + h + dc {
                        1.0 - (1.0 - s) * (r.t - a - h) / dc.max(1e-4)
                    } else {
                        s
                    };
                    if r.released_at.is_none() {
                        r.level = lvl;
                    }
                    lvl
                }
                "SampleFaderModulationDevice" => {
                    let (from, to, dur, delay) = (d.get("From", 0.0), d.get("To", 1.0), secs(d.get("Duration", 0.3)), secs(d.get("Delay", 0.0)));
                    let x = ((r.t - delay) / dur.max(1e-4)).clamp(0.0, 1.0);
                    let shaped = match d.text("Scaling").unwrap_or("Linear") {
                        "Log Fast" => x.powf(0.25),
                        "Log Slow" => x.sqrt(),
                        "Exp Slow" => x * x,
                        "Exp Fast" => x.powi(4),
                        _ => x,
                    };
                    from + (to - from) * shaped
                }
                "SampleLfoModulationDevice" => {
                    let delay = secs(d.get("Delay", 0.0));
                    let n = d.get("Frequency", 0.3);
                    let hz = if synced { 1.0 / (beat * 2f32.powf(4.0 - n * 9.0)).max(1e-4) } else { 0.01 * 2000f32.powf(n) };
                    if r.t >= delay {
                        let before = r.phase;
                        r.phase += dt * hz;
                        if r.phase.floor() != before.floor() {
                            r.random = self.rng.next();
                        }
                    }
                    let mode = match d.text("Mode").unwrap_or("Sin") {
                        "Saw" => 2,
                        "Pulse" => 3,
                        "Random" => 4,
                        _ => 1,
                    };
                    let w = lfo_wave(mode, r.phase, r.random) * d.get("Amplitude", 0.5);
                    // 0..1 here; a bipolar device spans -1..1 below.
                    (w + 1.0) * 0.5
                }
                "SampleEnvelopeModulationDevice" => {
                    let Some(e) = d.env.as_ref() else { continue };
                    let pts: Vec<(f32, f32)> = e.points.iter().map(|&(t, v)| (t as f32, v)).collect();
                    let interp = e.interp.unwrap_or(Interp::Curve);
                    // Synced: 24 ticks a beat; free: milliseconds.
                    let tick = if synced { beat / 24.0 } else { 0.001 };
                    let looping = e.loop_mode.map(|m| (m, e.loop_range.0, e.loop_range.1));
                    let sustain = e.sustain;
                    let length = e.length as f32;
                    let x = curve(&pts, r.run.pos, interp, 1.0);
                    r.run.advance(dt / tick, sustain, looping, r.released_at.is_some(), length);
                    let mut lvl = x;
                    if r.released_at.is_some() && *target == Target::Volume {
                        r.fade -= dt / tick * d.get("Decay", 0.0).max(0.0) / 4096.0;
                        if r.fade <= 0.0 {
                            r.fade = 0.0;
                            done = true;
                        }
                        lvl *= r.fade;
                    }
                    if *target == Target::Volume && looping.is_none() && sustain.is_none() && r.run.pos >= length - 1.0 && x <= 0.0 {
                        done = true;
                    }
                    lvl
                }
                "SampleKeyTrackingModulationDevice" => {
                    let (lo, hi) = (d.get("Min", 0.0), d.get("Max", 119.0));
                    ((self.note - lo) / (hi - lo).max(1.0)).clamp(0.0, 1.0)
                }
                "SampleVelocityTrackingModulationDevice" => {
                    let (lo, hi) = (d.get("Min", 0.0), d.get("Max", 127.0));
                    let vel = self.velocity * 127.0;
                    if d.text("Mode") == Some("Scale") {
                        (lo + (hi - lo) * self.velocity) / 127.0
                    } else {
                        ((vel - lo) / (hi - lo).max(1.0)).clamp(0.0, 1.0)
                    }
                }
                "SampleOperandModulationDevice" => d.get("Value", 1.0),
                _ => 1.0,
            };
            r.t += dt;
            if *bipolar {
                o = o * 2.0 - 1.0;
            }
            let i = *target as usize;
            v[i] = match op {
                Op::Add => v[i] + o,
                Op::Sub => v[i] - o,
                Op::Mul => v[i] * o,
                Op::Div => v[i] / if o.abs() < 1e-4 { 1e-4 } else { o },
            };
        }
        Out {
            volume: v[0].clamp(0.0, 4.0),
            pan: v[1].clamp(-1.0, 1.0),
            pitch: v[2].clamp(-1.0, 1.0) * self.pitch_range,
            cutoff: v[3].clamp(0.0, 1.0),
            reso: v[4].clamp(0.0, 1.0),
            drive: v[5].clamp(0.0, 1.0),
            done,
        }
    }
}
