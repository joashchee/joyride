// Copyright 2026 ansiapps. Neumetik BGN: proprietary, see the LICENSE at the repository root.

//! Noise, every way it's made (`docs/bgn-noise-methods.md` has the
//! research: where each comes from and why it's free to ship). All of it
//! is written here from the published descriptions, none copied from a
//! library.
//!
//! - Colors: white (uniform and Gaussian), pink three ways (Kellet's
//!   filter, Voss-McCartney, Bristow-Johnson's three poles), brown
//!   (leaky integration), blue and violet (differencing), grey (the ear's
//!   equal loudness, roughly), velvet (sparse ±1 pulses).
//! - Spectral: noise shaped in the frequency domain, frame by frame
//!   (random phases, inverse FFT, overlap-add), to any slope and band.
//! - Digital: linear-feedback shift registers like the ones old sound
//!   chips used, with a clock and a bit depth.
//! - Random: sample-and-hold, stepped, linear or cubic between values.
//! - Chaotic: the logistic map, Hénon's map, Lorenz's system,
//!   SuperCollider-style crackle, and random telegraph (popcorn) noise.
//! - Smooth: 1-D gradient (Perlin) noise, value noise and simplex noise,
//!   and their fractal sums, for slow movement rather than sound.

use super::dsp::{Rng, Svf};
use super::fft::Fft;

/// The colors `ColorNoise` makes, in the order the Noise generator lists them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Color {
    White,
    Gaussian,
    Pink,
    PinkVoss,
    PinkPoles,
    Brown,
    Blue,
    Violet,
    Grey,
    Velvet,
}

pub const COLORS: [Color; 10] =
    [Color::White, Color::Gaussian, Color::Pink, Color::PinkVoss, Color::PinkPoles, Color::Brown, Color::Blue, Color::Violet, Color::Grey, Color::Velvet];

/// Paul Kellet's refined pinking filter (music-dsp, 1999, given freely):
/// seven one-pole sections whose sum is within 0.05 dB of -3 dB an
/// octave above 9.2 Hz at 44.1 kHz.
#[derive(Clone, Copy, Default)]
struct Kellet([f32; 7]);

impl Kellet {
    #[inline]
    fn tick(&mut self, w: f32) -> f32 {
        let b = &mut self.0;
        b[0] = 0.99886 * b[0] + w * 0.0555179;
        b[1] = 0.99332 * b[1] + w * 0.0750759;
        b[2] = 0.96900 * b[2] + w * 0.153_852;
        b[3] = 0.86650 * b[3] + w * 0.3104856;
        b[4] = 0.55000 * b[4] + w * 0.5329522;
        b[5] = -0.7616 * b[5] - w * 0.0168980;
        let pink = b[0] + b[1] + b[2] + b[3] + b[4] + b[5] + b[6] + w * 0.5362;
        b[6] = w * 0.115926;
        pink
    }
}

/// Voss's algorithm as James McCartney arranged it: 16 rows of held
/// random values, the row a sample's counter has trailing zeros for
/// renewed each sample, so each octave down changes half as often.
#[derive(Clone, Copy, Default)]
struct Voss {
    rows: [f32; 16],
    sum: f32,
    counter: u32,
}

impl Voss {
    #[inline]
    fn tick(&mut self, rng: &mut Rng) -> f32 {
        self.counter = self.counter.wrapping_add(1);
        let row = self.counter.trailing_zeros() as usize;
        if row < 16 {
            let v = rng.bi();
            self.sum += v - self.rows[row];
            self.rows[row] = v;
        }
        self.sum + rng.bi()
    }
}

/// Robert Bristow-Johnson's pinking filter (music-dsp, 1998): three
/// poles and three zeros, within 0.3 dB over ten octaves.
#[derive(Clone, Copy, Default)]
struct Poles {
    x: [f32; 3],
    y: [f32; 3],
}

impl Poles {
    const P: [f32; 3] = [0.995_727_5, 0.947_906_5, 0.535_675];
    const Z: [f32; 3] = [0.984_436, 0.833_923_3, 0.075_683_6];

    #[inline]
    fn tick(&mut self, mut v: f32) -> f32 {
        for i in 0..3 {
            let y = v - Self::Z[i] * self.x[i] + Self::P[i] * self.y[i];
            self.x[i] = v;
            self.y[i] = y;
            v = y;
        }
        v
    }
}

/// Noise of one color, at about the same loudness (RMS near 0.25)
/// whichever it is.
#[derive(Clone)]
pub struct ColorNoise {
    pub color: Color,
    rng: Rng,
    kellet: Kellet,
    voss: Voss,
    poles: Poles,
    brown: f32,
    last: f32,
    grey: [Svf; 2],
    /// Velvet: pulses a second, where this period's pulse falls, and
    /// how far through the period we are.
    pub density: f32,
    velvet_at: u32,
    velvet_n: u32,
    velvet_sign: f32,
    rate: f32,
}

impl ColorNoise {
    pub fn new(color: Color, seed: u32, rate: f32) -> ColorNoise {
        ColorNoise {
            color,
            rng: Rng::new(seed),
            kellet: Kellet::default(),
            voss: Voss::default(),
            poles: Poles::default(),
            brown: 0.0,
            last: 0.0,
            grey: [Svf::new(180.0, 0.6, rate), Svf::new(3200.0, 0.5, rate)],
            density: 2000.0,
            velvet_at: 0,
            velvet_n: 0,
            velvet_sign: 1.0,
            rate,
        }
    }

    #[inline]
    pub fn next(&mut self) -> f32 {
        let w = self.rng.bi();
        match self.color {
            Color::White => w * 0.43,
            Color::Gaussian => self.rng.gauss() * 0.25,
            Color::Pink => self.kellet.tick(w) * 0.08,
            Color::PinkVoss => self.voss.tick(&mut self.rng) * 0.105,
            Color::PinkPoles => self.poles.tick(w) * 0.3,
            Color::Brown => {
                // Leaky so it can't wander off; the leak sets its low corner (about 10 Hz).
                self.brown = self.brown * 0.9986 + w * 0.05;
                self.brown * 0.38
            }
            Color::Blue => {
                // Pink, differenced: +3 dB an octave.
                let p = self.kellet.tick(w);
                let d = p - self.last;
                self.last = p;
                d * 0.13
            }
            Color::Violet => {
                let d = w - self.last;
                self.last = w;
                d * 0.3
            }
            Color::Grey => {
                // Roughly the inverse of the ear's equal-loudness curve (A-weighting):
                // more low end, a dip where hearing is keenest.
                let low = self.grey[0].low(w);
                let mid = self.grey[1].band(w);
                (w * 0.55 + low * 1.9 - mid * 0.35) * 0.85
            }
            Color::Velvet => self.velvet(),
        }
    }

    /// Velvet noise (Karjalainen and Järveläinen): one pulse of random sign
    /// at a random place in each period. Smooth to the ear at a few
    /// thousand pulses a second, rough below that.
    fn velvet(&mut self) -> f32 {
        let period = (self.rate / self.density.max(1.0)).max(1.0) as u32;
        if self.velvet_n == 0 {
            self.velvet_at = self.rng.next_u32() % period;
            self.velvet_sign = if self.rng.chance(0.5) { 1.0 } else { -1.0 };
        }
        let out = if self.velvet_n == self.velvet_at { self.velvet_sign * (period as f32).sqrt() * 0.25 } else { 0.0 };
        self.velvet_n += 1;
        if self.velvet_n >= period {
            self.velvet_n = 0;
        }
        out
    }
}

/// Noise shaped in the frequency domain (the stochastic part of Xavier
/// Serra's spectral modeling): each frame a magnitude curve with random
/// phases, inverse FFT'd, Hann-windowed and overlap-added at a quarter
/// frame. Left and right come out of one complex transform, as its real
/// and imaginary parts, so they're independent.
pub struct Spectral {
    fft: Fft,
    window: Vec<f32>,
    pub shape: Vec<f32>,
    re: Vec<f32>,
    im: Vec<f32>,
    out_l: Vec<f32>,
    out_r: Vec<f32>,
    pos: usize,
    rng: Rng,
}

pub const SPECTRAL_FRAME: usize = 2048;
const HOP: usize = SPECTRAL_FRAME / 4;

impl Spectral {
    pub fn new(seed: u32) -> Spectral {
        let n = SPECTRAL_FRAME;
        let window = (0..n).map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / n as f32).cos()).collect();
        Spectral {
            fft: Fft::new(n),
            window,
            shape: vec![0.0; n / 2 + 1],
            re: vec![0.0; n],
            im: vec![0.0; n],
            out_l: vec![0.0; n],
            out_r: vec![0.0; n],
            pos: HOP,
            rng: Rng::new(seed),
        }
    }

    /// Sets the magnitude for each bin from `f(hz)`, normalized to the
    /// same loudness whatever its shape.
    pub fn set_shape(&mut self, rate: f32, f: impl Fn(f32) -> f32) {
        let n = SPECTRAL_FRAME as f32;
        let mut power = 0.0;
        for (k, m) in self.shape.iter_mut().enumerate() {
            let hz = k as f32 * rate / n;
            *m = if k == 0 { 0.0 } else { f(hz).max(0.0) };
            power += *m * *m;
        }
        let g = if power > 0.0 { 1.0 / power.sqrt() } else { 0.0 };
        for m in &mut self.shape {
            *m *= g;
        }
    }

    fn frame(&mut self) {
        let n = SPECTRAL_FRAME;
        // Two spectra, A for the left and B for the right, as A + iB.
        self.re.iter_mut().for_each(|v| *v = 0.0);
        self.im.iter_mut().for_each(|v| *v = 0.0);
        for k in 1..n / 2 {
            let m = self.shape[k];
            if m == 0.0 {
                continue;
            }
            let (pa, pb) = (self.rng.uni() * std::f32::consts::TAU, self.rng.uni() * std::f32::consts::TAU);
            let (ar, ai) = (m * pa.cos(), m * pa.sin());
            let (br, bi) = (m * pb.cos(), m * pb.sin());
            // X[k] = A[k] + iB[k]; X[n-k] = conj(A[k]) + i conj(B[k]).
            self.re[k] = ar - bi;
            self.im[k] = ai + br;
            self.re[n - k] = ar + bi;
            self.im[n - k] = -ai + br;
        }
        self.fft.inverse(&mut self.re, &mut self.im);
        // Shift out the hop just played, then add the new frame.
        self.out_l.copy_within(HOP.., 0);
        self.out_r.copy_within(HOP.., 0);
        let tail = n - HOP;
        self.out_l[tail..].iter_mut().for_each(|v| *v = 0.0);
        self.out_r[tail..].iter_mut().for_each(|v| *v = 0.0);
        // The shape has unit power over half the bins, so each channel's
        // samples have a variance of 2, and Hann squared at four-times
        // overlap sums to 1.5: a deviation of sqrt(3), scaled to 0.25.
        let g = 0.144;
        for i in 0..n {
            let w = self.window[i] * g;
            self.out_l[i] += self.re[i] * w;
            self.out_r[i] += self.im[i] * w;
        }
        self.pos = 0;
    }

    #[inline]
    pub fn next(&mut self) -> (f32, f32) {
        if self.pos >= HOP {
            self.frame();
        }
        let s = (self.out_l[self.pos], self.out_r[self.pos]);
        self.pos += 1;
        s
    }
}

/// The shift registers `Lfsr` can be.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Chip {
    /// 15 bits, taps 0 and 1 (the NES APU's long mode): hiss.
    Long15,
    /// 15 bits, taps 0 and 6 (its short mode): a metallic, pitched buzz.
    Short15,
    /// 15 bits written back into bit 6 too (the Game Boy's 7-bit mode).
    Seven,
    /// 23 bits, taps 17 and 22, eight bits read out (the SID chip).
    Sid23,
    /// 17 bits, taps 17 and 12: a long polynomial counter.
    Poly17,
    /// 4 bits, taps 4 and 3: a 15-step buzz.
    Poly4,
}

pub const CHIPS: [Chip; 6] = [Chip::Long15, Chip::Short15, Chip::Seven, Chip::Sid23, Chip::Poly17, Chip::Poly4];

/// Digital noise from a linear-feedback shift register clocked at its own
/// rate and held between steps, as the old sound chips made it.
#[derive(Clone, Copy)]
pub struct Lfsr {
    pub chip: Chip,
    reg: u32,
    phase: f32,
    out: f32,
}

impl Lfsr {
    pub fn new(chip: Chip, seed: u32) -> Lfsr {
        Lfsr { chip, reg: (seed & 0x7fff) | 1, phase: 0.0, out: 0.0 }
    }

    fn step(&mut self) {
        let r = self.reg;
        match self.chip {
            Chip::Long15 | Chip::Short15 | Chip::Seven => {
                let tap = if self.chip == Chip::Short15 { 6 } else { 1 };
                let fb = (r ^ (r >> tap)) & 1;
                let mut n = (r >> 1) | (fb << 14);
                if self.chip == Chip::Seven {
                    n = (n & !0x40) | (fb << 6);
                }
                self.reg = n & 0x7fff;
                self.out = if self.reg & 1 == 0 { 1.0 } else { -1.0 };
            }
            Chip::Sid23 => {
                let r = if r & 0x7f_ffff == 0 { 0x7f_fff8 } else { r };
                let fb = ((r >> 22) ^ (r >> 17)) & 1;
                self.reg = ((r << 1) | fb) & 0x7f_ffff;
                let r = self.reg;
                let byte = ((r >> 20) & 1) << 7
                    | ((r >> 18) & 1) << 6
                    | ((r >> 14) & 1) << 5
                    | ((r >> 11) & 1) << 4
                    | ((r >> 9) & 1) << 3
                    | ((r >> 5) & 1) << 2
                    | ((r >> 2) & 1) << 1
                    | (r & 1);
                self.out = byte as f32 / 127.5 - 1.0;
            }
            Chip::Poly17 => {
                let r = if r & 0x1_ffff == 0 { 1 } else { r };
                let fb = ((r >> 16) ^ (r >> 11)) & 1;
                self.reg = ((r << 1) | fb) & 0x1_ffff;
                self.out = if self.reg & 1 == 0 { 1.0 } else { -1.0 };
            }
            Chip::Poly4 => {
                let r = if r & 0xf == 0 { 1 } else { r };
                let fb = ((r >> 3) ^ (r >> 2)) & 1;
                self.reg = ((r << 1) | fb) & 0xf;
                self.out = if self.reg & 1 == 0 { 1.0 } else { -1.0 };
            }
        }
    }

    /// The next sample with the register clocked `clock` times a second.
    #[inline]
    pub fn next(&mut self, clock: f32, rate: f32) -> f32 {
        self.phase += clock / rate;
        let mut steps = 0;
        while self.phase >= 1.0 && steps < 32 {
            self.phase -= 1.0;
            self.step();
            steps += 1;
        }
        if self.phase >= 1.0 {
            self.phase = self.phase.fract();
        }
        self.out * 0.4
    }
}

/// How `Random` moves between its values.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shape {
    Step,
    Linear,
    Cubic,
}

/// Sample-and-hold noise: a new random value `rate` times a second, held,
/// ramped or curved (Catmull-Rom) to the next. SuperCollider's LFNoise0,
/// 1 and 2 are the same idea.
#[derive(Clone, Copy)]
pub struct Random {
    rng_state: u64,
    pts: [f32; 4],
    phase: f32,
}

impl Random {
    pub fn new(seed: u32) -> Random {
        let mut r = Random { rng_state: u64::from(seed) | 1, pts: [0.0; 4], phase: 0.0 };
        for i in 0..4 {
            r.pts[i] = r.draw();
        }
        r
    }

    fn draw(&mut self) -> f32 {
        // A small LCG is plenty for the held values.
        self.rng_state = self.rng_state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        ((self.rng_state >> 40) as f32 / (1u64 << 24) as f32) * 2.0 - 1.0
    }

    #[inline]
    pub fn next(&mut self, shape: Shape, rate: f32, sample_rate: f32) -> f32 {
        self.phase += rate / sample_rate;
        while self.phase >= 1.0 {
            self.phase -= 1.0;
            self.pts = [self.pts[1], self.pts[2], self.pts[3], 0.0];
            self.pts[3] = self.draw();
        }
        let t = self.phase;
        let [p0, p1, p2, p3] = self.pts;
        match shape {
            Shape::Step => p1,
            Shape::Linear => p1 + (p2 - p1) * t,
            Shape::Cubic => {
                let a = -0.5 * p0 + 1.5 * p1 - 1.5 * p2 + 0.5 * p3;
                let b = p0 - 2.5 * p1 + 2.0 * p2 - 0.5 * p3;
                let c = -0.5 * p0 + 0.5 * p2;
                ((a * t + b) * t + c) * t + p1
            }
        }
    }
}

/// The chaotic and burst generators `Chaos` can be.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Map {
    Logistic,
    Henon,
    Lorenz,
    Crackle,
    Telegraph,
}

pub const MAPS: [Map; 5] = [Map::Logistic, Map::Henon, Map::Lorenz, Map::Crackle, Map::Telegraph];

/// Noise from chaos rather than randomness: a map iterated `rate` times a
/// second, or a system integrated at that speed. `amount` (0..1) is how
/// far into chaos it's pushed.
#[derive(Clone, Copy)]
pub struct Chaos {
    pub map: Map,
    x: f32,
    y: f32,
    z: f32,
    phase: f32,
    out: f32,
    rng: u32,
}

impl Chaos {
    pub fn new(map: Map, seed: u32) -> Chaos {
        Chaos { map, x: 0.1 + (seed % 100) as f32 * 0.003, y: 0.2, z: 0.3, phase: 0.0, out: 0.0, rng: seed | 1 }
    }

    fn step(&mut self, amount: f32) {
        match self.map {
            Map::Logistic => {
                let r = 3.57 + 0.43 * amount;
                self.x = (r * self.x * (1.0 - self.x)).clamp(1e-6, 1.0 - 1e-6);
                self.out = self.x * 2.0 - 1.0;
            }
            Map::Henon => {
                let a = 1.05 + 0.35 * amount;
                let nx = 1.0 - a * self.x * self.x + self.y;
                self.y = 0.3 * self.x;
                self.x = if nx.is_finite() && nx.abs() < 4.0 { nx } else { 0.1 };
                self.out = self.x / 1.5;
            }
            Map::Lorenz => {
                // Euler steps; `amount` speeds it along.
                let dt = 0.002 + 0.01 * amount;
                let (x, y, z) = (self.x, self.y, self.z);
                self.x += dt * 10.0 * (y - x);
                self.y += dt * (x * (28.0 - z) - y);
                self.z += dt * (x * y - 8.0 / 3.0 * z);
                if !self.x.is_finite() || self.x.abs() > 100.0 {
                    (self.x, self.y, self.z) = (0.1, 0.2, 0.3);
                }
                self.out = self.x / 20.0;
            }
            Map::Crackle => {
                // SuperCollider's Crackle: y = |p y1 - y2 - 0.05|, p from 1 to 2.
                let p = 1.0 + amount * 0.99;
                let y = (p * self.x - self.y - 0.05).abs();
                self.y = self.x;
                self.x = y.min(4.0);
                self.out = self.x - 0.5;
            }
            Map::Telegraph => {
                // Random telegraph (popcorn) noise: two levels, flipping at random.
                self.rng ^= self.rng << 13;
                self.rng ^= self.rng >> 17;
                self.rng ^= self.rng << 5;
                if ((self.rng >> 8) as f32 / (1u32 << 24) as f32) < 0.02 + 0.3 * amount {
                    self.x = if self.x > 0.0 { -1.0 } else { 1.0 };
                }
                self.out = self.x.signum();
            }
        }
    }

    #[inline]
    pub fn next(&mut self, rate: f32, amount: f32, sample_rate: f32) -> f32 {
        self.phase += rate / sample_rate;
        let mut steps = 0;
        while self.phase >= 1.0 && steps < 16 {
            self.phase -= 1.0;
            self.step(amount);
            steps += 1;
        }
        if self.phase >= 1.0 {
            self.phase = self.phase.fract();
        }
        self.out
    }
}

/// Smooth 1-D noise on a lattice of 256 random gradients: Ken Perlin's
/// gradient noise, value noise between random values, and simplex noise
/// (Perlin's 2001 kernel, its patent expired in 2022).
pub struct Gradient {
    g: [f32; 256],
}

impl Gradient {
    pub fn new(seed: u32) -> Gradient {
        let mut rng = Rng::new(seed ^ 0x5eed);
        Gradient { g: std::array::from_fn(|_| rng.bi()) }
    }

    #[inline]
    fn at(&self, i: i32) -> f32 {
        self.g[(i & 255) as usize]
    }

    /// Perlin's gradient noise: each lattice point's slope, blended by his
    /// quintic fade. About -0.5..0.5.
    pub fn perlin(&self, x: f32) -> f32 {
        let i = x.floor() as i32;
        let f = x - i as f32;
        let a = self.at(i) * f;
        let b = self.at(i + 1) * (f - 1.0);
        let t = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
        a + (b - a) * t
    }

    /// Value noise: random values, smoothstepped between.
    pub fn value(&self, x: f32) -> f32 {
        let i = x.floor() as i32;
        let f = x - i as f32;
        let t = f * f * (3.0 - 2.0 * f);
        let (a, b) = (self.at(i), self.at(i + 1));
        a + (b - a) * t
    }

    /// Simplex noise in one dimension: the two nearest lattice points' slopes
    /// under a radial (1 - d^2)^4 kernel.
    pub fn simplex(&self, x: f32) -> f32 {
        let i = x.floor() as i32;
        let mut n = 0.0;
        for j in [i, i + 1] {
            let d = x - j as f32;
            let t = 1.0 - d * d;
            if t > 0.0 {
                let t2 = t * t;
                n += t2 * t2 * self.at(j) * d;
            }
        }
        n * 2.5
    }

    /// Fractal Brownian motion: `octaves` of Perlin noise, each twice as
    /// fast and half as loud. About -1..1.
    pub fn fbm(&self, x: f32, octaves: u32) -> f32 {
        let (mut sum, mut amp, mut f) = (0.0, 1.0, 1.0);
        for o in 0..octaves {
            sum += self.perlin(x * f + o as f32 * 17.3) * amp;
            amp *= 0.5;
            f *= 2.0;
        }
        sum * 1.6
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rms(v: &[f32]) -> f32 {
        (v.iter().map(|x| x * x).sum::<f32>() / v.len() as f32).sqrt()
    }

    /// Power below and above 1 kHz: a quick check of a color's tilt.
    fn tilt(v: &[f32], rate: f32) -> f32 {
        let mut lp = Svf::new(1000.0, 0.707, rate);
        let (mut lo, mut hi) = (0.0, 0.0);
        for &x in v {
            let (l, _, h) = lp.tick(x);
            lo += l * l;
            hi += h * h;
        }
        10.0 * (lo / hi).log10()
    }

    #[test]
    fn every_color_is_about_as_loud_and_tilts_its_own_way() {
        let rate = 44_100.0;
        let mut tilts = Vec::new();
        for c in COLORS {
            let mut n = ColorNoise::new(c, 7, rate);
            let v: Vec<f32> = (0..200_000).map(|_| n.next()).collect();
            let r = rms(&v);
            assert!((0.12..0.42).contains(&r), "{c:?} rms {r}");
            tilts.push((c, tilt(&v, rate)));
        }
        let t = |c: Color| tilts.iter().find(|(k, _)| *k == c).unwrap().1;
        assert!(t(Color::Brown) > t(Color::Pink) && t(Color::Pink) > t(Color::White));
        assert!(t(Color::White) > t(Color::Blue) && t(Color::Blue) > t(Color::Violet));
        assert!((t(Color::PinkVoss) - t(Color::Pink)).abs() < 4.0);
        assert!((t(Color::PinkPoles) - t(Color::Pink)).abs() < 4.0);
    }

    #[test]
    fn spectral_noise_follows_its_shape_in_both_channels() {
        let rate = 44_100.0;
        let mut s = Spectral::new(3);
        s.set_shape(rate, |hz| if hz < 1000.0 { 1.0 } else { 0.0 });
        let (l, r): (Vec<f32>, Vec<f32>) = (0..100_000).map(|_| s.next()).unzip();
        assert!(tilt(&l, rate) > 8.0 && tilt(&r, rate) > 8.0, "{} {}", tilt(&l, rate), tilt(&r, rate));
        assert!((0.1..0.5).contains(&rms(&l)), "rms {}", rms(&l));
        let corr: f32 = l.iter().zip(&r).map(|(a, b)| a * b).sum::<f32>() / (rms(&l) * rms(&r) * l.len() as f32);
        assert!(corr.abs() < 0.1, "left and right should be independent: {corr}");
    }

    #[test]
    fn the_shift_registers_run_their_full_cycles() {
        let mut l = Lfsr::new(Chip::Long15, 1);
        let start = l.reg;
        let mut n = 0;
        loop {
            l.step();
            n += 1;
            if l.reg == start || n > 40_000 {
                break;
            }
        }
        assert_eq!(n, 32_767);
        let mut s = Lfsr::new(Chip::Short15, 1);
        let start = s.reg;
        let mut n = 0;
        loop {
            s.step();
            n += 1;
            if s.reg == start || n > 40_000 {
                break;
            }
        }
        assert!(n < 1000, "the short mode is a short loop: {n}");
        for chip in CHIPS {
            let mut c = Lfsr::new(chip, 5);
            let v: Vec<f32> = (0..20_000).map(|_| c.next(20_000.0, 44_100.0)).collect();
            assert!(rms(&v) > 0.1, "{chip:?}");
        }
    }

    #[test]
    fn chaos_and_random_stay_in_range() {
        for map in MAPS {
            let mut c = Chaos::new(map, 9);
            for i in 0..50_000 {
                let v = c.next(8000.0, (i % 100) as f32 / 100.0, 44_100.0);
                assert!(v.is_finite() && v.abs() <= 4.0, "{map:?} {v}");
            }
        }
        for shape in [Shape::Step, Shape::Linear, Shape::Cubic] {
            let mut r = Random::new(4);
            for _ in 0..50_000 {
                let v = r.next(shape, 300.0, 44_100.0);
                assert!(v.abs() <= 1.5);
            }
        }
        let g = Gradient::new(1);
        for i in 0..10_000 {
            let x = i as f32 * 0.013;
            assert!(g.perlin(x).abs() <= 1.0 && g.simplex(x).abs() <= 1.5 && g.value(x).abs() <= 1.0 && g.fbm(x, 4).abs() <= 2.0);
        }
    }
}
