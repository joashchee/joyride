// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! A sample playing: its position through the sample (loops, reverse),
//! its interpolation, and the volume, panning and pitch the player and
//! its envelopes give it.

use std::sync::Arc;

use super::env::{Mods, OldEnv};
use super::filter::VoiceFilter;
use super::song::{Audio, Loop, SampleInterp};

/// Frames of a fade when a voice is cut, so it doesn't click.
pub const DECLICK: f32 = 64.0;

pub struct Voice {
    pub track: usize,
    pub column: usize,
    pub instr: usize,
    pub sample: usize,
    pub audio: Arc<Audio>,
    pub interp: SampleInterp,
    pub loop_mode: Loop,
    pub loop_start: f64,
    pub loop_end: f64,
    /// Where a slice ends (the sample's end otherwise).
    pub end: f64,
    pub start: f64,
    pub loop_release: bool,
    pub pos: f64,
    /// Playing backwards (the B command, or a reverse loop's way back).
    pub back: bool,
    /// Within a ping-pong or reverse loop, coming back.
    pub looping_back: bool,
    pub in_loop: bool,
    /// It has come round its forward loop at least once.
    pub wrapped: bool,
    /// The note's pitch in semitones (with the sample's transposition), and
    /// the frames a frame at the output for it.
    pub semis: f32,
    pub base_step: f64,
    /// Pitch from slides, glides, arpeggio and vibrato (semitones).
    pub bend: f32,
    pub glide_to: Option<f32>,
    pub vol: f32,
    pub sample_vol: f32,
    pub pan: f32,
    pub sample_pan: f32,
    pub released: bool,
    /// The volume as it fades out when cut.
    pub fade: f32,
    pub cutting: bool,
    pub env: Option<OldEnv>,
    pub mods: Option<Mods>,
    pub filter: VoiceFilter,
    /// The sample's own effect chain (Renoise 3), its index in the instrument.
    pub chain: i32,
    pub dead: bool,
    /// Last gains, ramped towards each block's.
    pub gain: (f32, f32),
    pub started: bool,
    pub oneshot: bool,
    pub mute_group: i32,
    /// Its age, for stealing the oldest.
    pub serial: u64,
    /// Frames to wait before it sounds (delayed within a block).
    pub wait: usize,
    /// The note it was started on, for glides.
    pub note: f32,
    /// The column it was started in (it stays its home once let go).
    pub home: usize,
    /// This tick's arpeggio and vibrato (semitones), tremolo (a gain) and autopan.
    pub arp: f32,
    pub vib: f32,
    pub trem: f32,
    pub auto_pan: f32,
}

impl Voice {
    /// Whether a note off lets it fade by its envelopes (rather than ending it).
    pub fn holds_release(&self) -> bool {
        self.env.as_ref().is_some_and(|e| e.volume_on()) || self.mods.as_ref().is_some_and(|m| m.holds_release())
    }

    /// Its pitch for another note, in its own semitones (for a glide there).
    pub fn semis_for(&self, note: f32) -> f32 {
        self.semis + (note - self.note)
    }

    /// The envelopes start again (a retrigger that doesn't restart the sample).
    pub fn restart_envelopes(&mut self) {
        if let Some(e) = self.env.as_mut() {
            e.seek(0.0);
        }
        if let Some(m) = self.mods.as_mut() {
            m.restart();
        }
    }

    fn point(&self, ch: usize, i: i64) -> f32 {
        let c = &self.audio.channels[ch.min(self.audio.channels.len() - 1)];
        if i < 0 || i as usize >= c.len() {
            return 0.0;
        }
        c[i as usize]
    }

    /// The point at the current position on channel `ch`.
    fn read(&self, ch: usize) -> f32 {
        let i = self.pos.floor();
        let f = (self.pos - i) as f32;
        let i = i as i64;
        // Within a forward loop the points wrap around it, both ways.
        let at = |k: i64| -> f32 {
            if self.in_loop && self.loop_mode == Loop::Forward && !self.back {
                let (ls, le) = (self.loop_start as i64, self.loop_end as i64);
                if le > ls && (k >= le || (k < ls && self.wrapped)) {
                    return self.point(ch, ls + (k - ls).rem_euclid(le - ls));
                }
            }
            self.point(ch, k)
        };
        match self.interp {
            SampleInterp::None => at(i),
            SampleInterp::Linear => {
                let (a, b) = (at(i), at(i + 1));
                a + (b - a) * f
            }
            _ => {
                let (y0, y1, y2, y3) = (at(i - 1), at(i), at(i + 1), at(i + 2));
                // Catmull-Rom (cubic Hermite).
                let c1 = 0.5 * (y2 - y0);
                let c2 = y0 - 2.5 * y1 + 2.0 * y2 - 0.5 * y3;
                let c3 = 0.5 * (y3 - y0) + 1.5 * (y1 - y2);
                ((c3 * f + c2) * f + c1) * f + y1
            }
        }
    }

    /// Moves the position by `step` frames, through the loop; false once
    /// the sample has ended.
    fn advance(&mut self, step: f64) -> bool {
        let looping = self.loop_mode != Loop::Off && self.loop_end > self.loop_start && !(self.released && self.loop_release);
        if self.back {
            self.pos -= step;
            if self.in_loop && looping {
                match self.loop_mode {
                    Loop::PingPong => {
                        if self.pos < self.loop_start {
                            self.pos = (self.loop_start + (self.loop_start - self.pos)).min(self.loop_end - 1.0);
                            self.back = false;
                            self.looping_back = false;
                        }
                    }
                    _ => {
                        if self.pos < self.loop_start {
                            let len = self.loop_end - self.loop_start;
                            self.pos = self.loop_end - (self.loop_start - self.pos) % len;
                        }
                    }
                }
                return true;
            }
            return self.pos >= 0.0;
        }
        self.pos += step;
        if looping && self.pos >= self.loop_end {
            self.in_loop = true;
            match self.loop_mode {
                Loop::Forward => {
                    let len = self.loop_end - self.loop_start;
                    self.pos = self.loop_start + (self.pos - self.loop_start) % len;
                    self.wrapped = true;
                }
                Loop::Backward => {
                    self.pos = (self.loop_end - (self.pos - self.loop_end) - 1.0).max(self.loop_start);
                    self.back = true;
                }
                Loop::PingPong => {
                    self.pos = (self.loop_end - (self.pos - self.loop_end) - 1.0).max(self.loop_start);
                    self.back = true;
                    self.looping_back = true;
                }
                Loop::Off => {}
            }
            return true;
        }
        if self.pos >= self.loop_start && looping {
            self.in_loop = true;
        }
        self.pos < self.end
    }

    /// Renders `n` frames into `out` (left, right) with the per-block
    /// volume, panning and pitch the player worked out.
    pub fn render(&mut self, out: (&mut [f32], &mut [f32]), gains: (f32, f32), step: f64) {
        let n = out.0.len();
        if self.dead || self.audio.frames() == 0 {
            self.dead = true;
            return;
        }
        let stereo = self.audio.channels.len() > 1;
        // The gains ramp from the last block's, so changes don't click.
        let (pl, pr) = if self.started { self.gain } else { gains };
        self.started = true;
        let mut k = self.wait.min(n);
        self.wait -= k;
        let span = (n - k).max(1) as f32;
        let mut t = 0.0f32;
        while k < n {
            let a = t / span;
            let gl = pl + (gains.0 - pl) * a;
            let gr = pr + (gains.1 - pr) * a;
            let mut fade = 1.0;
            if self.cutting {
                self.fade -= 1.0 / DECLICK;
                if self.fade <= 0.0 {
                    self.dead = true;
                    break;
                }
                fade = self.fade;
            }
            let (mut l, mut r) = if stereo {
                (self.read(0), self.read(1))
            } else {
                let v = self.read(0);
                (v, v)
            };
            if self.filter.on() {
                (l, r) = self.filter.process(l, r);
            }
            out.0[k] += l * gl * fade;
            out.1[k] += r * gr * fade;
            if !self.advance(step) {
                self.dead = true;
                break;
            }
            k += 1;
            t += 1.0;
        }
        self.gain = gains;
    }

    /// Moves through `n` frames without sound (a tick no one hears).
    pub fn skip(&mut self, n: usize, step: f64) {
        let mut n = n;
        let w = self.wait.min(n);
        self.wait -= w;
        n -= w;
        if self.cutting {
            self.dead = true;
            return;
        }
        // Whole loops at once, then frame by frame.
        for _ in 0..n {
            if !self.advance(step) {
                self.dead = true;
                return;
            }
            if self.in_loop && self.loop_mode == Loop::Forward && !self.back {
                let left = n as f64 * step;
                let len = self.loop_end - self.loop_start;
                if len > 0.0 && left > len * 4.0 {
                    self.pos = self.loop_start + ((self.pos - self.loop_start) + left) % len;
                    return;
                }
            }
        }
    }
}
