// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! Filters: a biquad (the RBJ cookbook's), and the filter a sample's
//! voice runs through, of the kinds Renoise's instruments name (its three
//! filter banks: Renoise 2's instrument filters, and Renoise 3's), each
//! made here of biquads, a ladder, waveshapers or modulators.

use std::f32::consts::PI;

/// The internal sample rate.
pub const RATE: f32 = 44100.0;

#[derive(Clone, Copy, Debug, Default)]
pub struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: [f32; 2],
    z2: [f32; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Low,
    High,
    Band,
    Notch,
    Peak,
    LowShelf,
    HighShelf,
    AllPass,
}

impl Biquad {
    /// Sets the coefficients: `f` Hz, `q`, `gain_db` (peaks and shelves).
    pub fn set(&mut self, shape: Shape, f: f32, q: f32, gain_db: f32) {
        let f = f.clamp(10.0, RATE * 0.49);
        let w = 2.0 * PI * f / RATE;
        let (s, c) = w.sin_cos();
        let q = q.max(0.05);
        let alpha = s / (2.0 * q);
        let a = 10f32.powf(gain_db / 40.0);
        let (b0, b1, b2, a0, a1, a2) = match shape {
            Shape::Low => ((1.0 - c) / 2.0, 1.0 - c, (1.0 - c) / 2.0, 1.0 + alpha, -2.0 * c, 1.0 - alpha),
            Shape::High => ((1.0 + c) / 2.0, -(1.0 + c), (1.0 + c) / 2.0, 1.0 + alpha, -2.0 * c, 1.0 - alpha),
            Shape::Band => (alpha, 0.0, -alpha, 1.0 + alpha, -2.0 * c, 1.0 - alpha),
            Shape::Notch => (1.0, -2.0 * c, 1.0, 1.0 + alpha, -2.0 * c, 1.0 - alpha),
            Shape::AllPass => (1.0 - alpha, -2.0 * c, 1.0 + alpha, 1.0 + alpha, -2.0 * c, 1.0 - alpha),
            Shape::Peak => (1.0 + alpha * a, -2.0 * c, 1.0 - alpha * a, 1.0 + alpha / a, -2.0 * c, 1.0 - alpha / a),
            Shape::LowShelf => {
                let sq = 2.0 * a.sqrt() * alpha;
                (
                    a * ((a + 1.0) - (a - 1.0) * c + sq),
                    2.0 * a * ((a - 1.0) - (a + 1.0) * c),
                    a * ((a + 1.0) - (a - 1.0) * c - sq),
                    (a + 1.0) + (a - 1.0) * c + sq,
                    -2.0 * ((a - 1.0) + (a + 1.0) * c),
                    (a + 1.0) + (a - 1.0) * c - sq,
                )
            }
            Shape::HighShelf => {
                let sq = 2.0 * a.sqrt() * alpha;
                (
                    a * ((a + 1.0) + (a - 1.0) * c + sq),
                    -2.0 * a * ((a - 1.0) + (a + 1.0) * c),
                    a * ((a + 1.0) + (a - 1.0) * c - sq),
                    (a + 1.0) - (a - 1.0) * c + sq,
                    2.0 * ((a - 1.0) - (a + 1.0) * c),
                    (a + 1.0) - (a - 1.0) * c - sq,
                )
            }
        };
        self.b0 = b0 / a0;
        self.b1 = b1 / a0;
        self.b2 = b2 / a0;
        self.a1 = a1 / a0;
        self.a2 = a2 / a0;
    }

    /// One point through channel `ch` (transposed direct form II).
    #[inline]
    pub fn run(&mut self, ch: usize, x: f32) -> f32 {
        let y = self.b0 * x + self.z1[ch];
        self.z1[ch] = self.b1 * x - self.a1 * y + self.z2[ch];
        self.z2[ch] = self.b2 * x - self.a2 * y;
        // Flush denormals and runaway states.
        if !y.is_finite() || y.abs() > 1e6 {
            self.z1[ch] = 0.0;
            self.z2[ch] = 0.0;
            return 0.0;
        }
        y
    }
}

/// A frequency for a 0..1 cutoff: 20 Hz to 20 kHz, by octaves.
pub fn cutoff_hz(norm: f32) -> f32 {
    20.0 * 1000f32.powf(norm.clamp(0.0, 1.0))
}

/// A frequency for a cutoff of 0..127 (Renoise 1 and 2's filters'
/// scale, a MIDI note's: 64 about 330 Hz).
pub fn note_hz(v: f32) -> f32 {
    (440.0 * 2f32.powf((v.clamp(0.0, 135.0) - 69.0) / 12.0)).clamp(10.0, RATE * 0.45)
}

/// A four-pole ladder (the Moog's), with resonance and a soft limit.
#[derive(Clone, Copy, Debug, Default)]
pub struct Ladder {
    s: [[f32; 4]; 2],
    /// The cutoff the coefficient was last worked out for, and it.
    f: f32,
    g: f32,
}

impl Ladder {
    /// One point through the ladder: one-pole stages in trapezoidal form,
    /// the feedback solved for the same instant (so it's stable up to the
    /// top of the band), the input saturated.
    #[inline]
    pub fn run(&mut self, ch: usize, x: f32, f: f32, res: f32, high: bool, poles: usize) -> f32 {
        let n = poles.clamp(1, 4);
        if f != self.f {
            let g = (PI * f.clamp(10.0, RATE * 0.45) / RATE).tan();
            self.g = g / (1.0 + g);
            self.f = f;
        }
        let g = self.g;
        // Its resonance stops just short of self-oscillation.
        let k = res.clamp(0.0, 1.0) * 3.8;
        let s = &mut self.s[ch];
        // The output's part that doesn't depend on this instant's input.
        let mut gain = 1.0f32;
        let mut rest = 0.0f32;
        for st in s.iter().take(n) {
            rest = g * rest + (1.0 - g) * st;
            gain *= g;
        }
        let mut u = ((x - k * rest) / (1.0 + k * gain)).tanh();
        for st in s.iter_mut().take(n) {
            let v = g * (u - *st);
            let y = v + *st;
            *st = y + v;
            u = y;
        }
        if !u.is_finite() {
            *s = [0.0; 4];
            return 0.0;
        }
        if high {
            x - u
        } else {
            u * (1.0 + k * 0.5)
        }
    }
}

/// What a voice's filter is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Kind {
    #[default]
    None,
    /// A low-, high-, band-pass or notch biquad cascade of `n` stages.
    Bq(Shape, u8),
    /// A ladder: high-pass?, poles.
    Ladder(bool, u8),
    /// One pole low-pass.
    Single,
    /// A peaking or shelving EQ of this many dB at the cutoff.
    Eq(i8),
    Peaking,
    /// Distortion: 0 low, 1 mid, 2 high, 3 full band, 4 shape, 5 fold.
    Dist(u8),
    /// Amplitude (ring) modulation: 0 sine, 1 triangle, 2 saw, 3 pulse.
    Am(u8),
    Comb,
    Decimator,
    Vowel,
}

/// A filter kind by a modulation set's filter version and type number
/// (Renoise 2's instrument filters are version 2).
pub fn kind(version: i32, t: i32) -> Kind {
    use Kind::*;
    use Shape::*;
    match version {
        3 => match t {
            1 | 5 | 8 => Bq([Low, High, Band][[1, 5, 8].iter().position(|&x| x == t).unwrap()], 1),
            2 => Ladder(false, 2),
            3 => Ladder(false, 4),
            4 => Ladder(false, 4),
            6 => Bq(High, 2),
            7 => Ladder(true, 4),
            9 => Bq(Band, 2),
            10 => Bq(Band, 2),
            11 => Bq(Band, 1),
            12 => Bq(Notch, 1),
            13 => Vowel,
            14 => Comb,
            15 => Decimator,
            16 => Dist(4),
            17 => Dist(5),
            18..=21 => Am((t - 18) as u8),
            _ => None,
        },
        _ => match t {
            1 => Bq(Low, 2),
            2 => Bq(Low, 1),
            3 => Bq(Low, if version == 1 { 4 } else { 1 }),
            4 => Ladder(false, 4),
            5 => Single,
            6 => Bq(High, 2),
            7 => Bq(High, 1),
            8 => Ladder(true, 4),
            9 => Bq(Notch, 1),
            10 => Bq(Band, 1),
            11 => Eq(-15),
            12 => Eq(-6),
            13 => Eq(6),
            14 => Eq(15),
            15 => Peaking,
            16 => Dist(0),
            17 => Dist(1),
            18 => Dist(2),
            19 => Dist(3),
            20 => Am(0),
            _ => None,
        },
    }
}

/// A voice's filter: its kind and state; cutoff, resonance (0..1) and
/// drive set each block.
#[derive(Clone, Debug, Default)]
pub struct VoiceFilter {
    pub kind: Kind,
    pub cutoff: f32,
    pub reso: f32,
    pub drive: f32,
    bq: [Biquad; 4],
    ladder: Ladder,
    lp1: [f32; 2],
    phase: f32,
    comb: Vec<[f32; 2]>,
    comb_at: usize,
    hold: [f32; 2],
    hold_t: f32,
    set_for: (f32, f32),
}

impl VoiceFilter {
    pub fn new(kind: Kind) -> VoiceFilter {
        VoiceFilter {
            kind,
            cutoff: 1.0,
            reso: 0.0,
            set_for: (-1.0, -1.0),
            ..Default::default()
        }
    }

    pub fn on(&self) -> bool {
        self.kind != Kind::None
    }

    /// Recomputes coefficients when the cutoff or resonance moved.
    fn prepare(&mut self) {
        if self.set_for == (self.cutoff, self.reso) {
            return;
        }
        self.set_for = (self.cutoff, self.reso);
        let f = cutoff_hz(self.cutoff);
        let q = 0.707 + self.reso * self.reso * 12.0;
        match self.kind {
            Kind::Bq(shape, n) => {
                // A cascade resonates in its last stage only.
                let n = usize::from(n);
                for (k, b) in self.bq.iter_mut().take(n).enumerate() {
                    b.set(shape, f, if k + 1 == n { q } else { 0.707 }, 0.0);
                }
            }
            Kind::Eq(db) => self.bq[0].set(Shape::Peak, f, 1.0 + self.reso * 4.0, f32::from(db)),
            Kind::Peaking => self.bq[0].set(Shape::Peak, f, 0.7 + self.reso * 8.0, 12.0),
            Kind::Dist(k) if k < 3 => self.bq[0].set([Shape::Low, Shape::Band, Shape::High][usize::from(k)], f, 0.707, 0.0),
            Kind::Vowel => {
                // Two formants moving with the cutoff (a to i).
                let t = self.cutoff;
                self.bq[0].set(Shape::Band, 700.0 + (300.0 - 700.0) * t, 6.0 + self.reso * 10.0, 0.0);
                self.bq[1].set(Shape::Band, 1200.0 + (2300.0 - 1200.0) * t, 8.0 + self.reso * 10.0, 0.0);
            }
            _ => {}
        }
    }

    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        self.prepare();
        let mut x = [l, r];
        let f = cutoff_hz(self.cutoff);
        match self.kind {
            Kind::None => {}
            Kind::Bq(_, n) => {
                for (ch, v) in x.iter_mut().enumerate() {
                    for b in self.bq.iter_mut().take(usize::from(n)) {
                        *v = b.run(ch, *v);
                    }
                }
            }
            Kind::Ladder(high, poles) => {
                for (ch, v) in x.iter_mut().enumerate() {
                    *v = self.ladder.run(ch, *v * (1.0 + self.drive * 4.0), f, self.reso, high, usize::from(poles));
                }
            }
            Kind::Single => {
                let a = (-2.0 * PI * f / RATE).exp();
                for (ch, v) in x.iter_mut().enumerate() {
                    self.lp1[ch] = *v * (1.0 - a) + self.lp1[ch] * a;
                    *v = self.lp1[ch];
                }
            }
            Kind::Eq(_) | Kind::Peaking => {
                for (ch, v) in x.iter_mut().enumerate() {
                    *v = self.bq[0].run(ch, *v);
                }
            }
            Kind::Dist(k) => {
                let gain = 1.0 + (self.reso + self.drive) * 20.0;
                for (ch, v) in x.iter_mut().enumerate() {
                    let band = if k < 3 { self.bq[0].run(ch, *v) } else { *v };
                    let rest = if k < 3 { *v - band } else { 0.0 };
                    let shaped = match k {
                        5 => {
                            // Folded back at ±1: a triangle of the input.
                            let t = (band * gain + 1.0) * 0.25;
                            1.0 - 4.0 * (t - t.floor() - 0.5).abs()
                        }
                        _ => (band * gain).tanh() / gain.sqrt().max(1.0),
                    };
                    *v = rest + shaped;
                }
            }
            Kind::Am(shape) => {
                let fr = f.min(5000.0);
                let m = match shape {
                    0 => (self.phase * 2.0 * PI).sin(),
                    1 => 1.0 - 4.0 * (self.phase - 0.5).abs(),
                    2 => 2.0 * self.phase - 1.0,
                    _ => {
                        if self.phase < 0.5 {
                            1.0
                        } else {
                            -1.0
                        }
                    }
                };
                self.phase = (self.phase + fr / RATE).fract();
                let depth = 0.5 + self.reso * 0.5;
                for v in x.iter_mut() {
                    *v *= 1.0 - depth + depth * m;
                }
            }
            Kind::Comb => {
                let n = ((RATE / f.max(20.0)) as usize).clamp(1, 4096);
                if self.comb.len() < 4096 {
                    self.comb = vec![[0.0; 2]; 4096];
                }
                let back = (self.comb_at + 4096 - n) % 4096;
                let fb = self.reso * 0.95;
                for (ch, v) in x.iter_mut().enumerate() {
                    let y = *v + self.comb[back][ch] * fb;
                    self.comb[self.comb_at][ch] = y;
                    *v = y * (1.0 - fb * 0.5);
                }
                self.comb_at = (self.comb_at + 1) % 4096;
            }
            Kind::Decimator => {
                self.hold_t += f / RATE;
                if self.hold_t >= 1.0 {
                    self.hold_t = self.hold_t.fract();
                    self.hold = x;
                }
                x = self.hold;
            }
            Kind::Vowel => {
                for (ch, v) in x.iter_mut().enumerate() {
                    *v = self.bq[0].run(ch, *v) + self.bq[1].run(ch, *v);
                }
            }
        }
        (x[0], x[1])
    }
}
