// Copyright 2026 ansiapps. Neumetik BGN: proprietary, see the LICENSE at the repository root.

//! The Noise group: each of noise.rs's methods as a playable generator.

use super::super::dsp::{db, Block, Burst, Drift, OnePole, Rng, Smoothness, Svf, Ticker, BLOCK};
use super::super::noise::{Chaos, ColorNoise, Lfsr, Random, Shape, Spectral, CHIPS, COLORS, MAPS};
use super::{pick, tone_cut, Gen, Lp2};

pub struct NoiseGen {
    rate: f32,
    n: [ColorNoise; 2],
    low: [Svf; 2],
    high: [Svf; 2],
    drift: Drift,
}

impl NoiseGen {
    pub fn new(rate: f32, seed: u32) -> NoiseGen {
        NoiseGen {
            rate,
            n: [ColorNoise::new(COLORS[2], seed, rate), ColorNoise::new(COLORS[2], seed ^ 0xa5a5, rate)],
            low: [Svf::new(10.0, 0.707, rate); 2],
            high: [Svf::new(18_000.0, 0.707, rate); 2],
            drift: Drift::new(seed),
        }
    }
}

impl Gen for NoiseGen {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let color = COLORS[pick(v[0]).min(COLORS.len() - 1)];
        for (i, n) in self.n.iter_mut().enumerate() {
            n.color = color;
            n.density = v[4];
            self.low[i].set(v[1], 0.707, self.rate);
            self.high[i].set(v[2], 0.707, self.rate);
        }
        let d = self.drift.next(0.15, BLOCK as f32 / self.rate);
        let g = (1.0 + v[3] * 0.9 * d).max(0.0);
        let cut_low = v[1] > 11.0;
        for i in 0..BLOCK {
            let mut s = [0.0; 2];
            for c in 0..2 {
                let mut x = self.n[c].next();
                if cut_low {
                    x = self.low[c].high(x);
                }
                s[c] = self.high[c].low(x) * g;
            }
            out.add(i, (s[0], s[1]));
        }
    }
}

pub struct SpectralGen {
    rate: f32,
    s: Spectral,
    shaped: [f32; 6],
}

impl SpectralGen {
    pub fn new(rate: f32, seed: u32) -> SpectralGen {
        SpectralGen { rate, s: Spectral::new(seed), shaped: [f32::NAN; 6] }
    }
}

impl Gen for SpectralGen {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        if *v != self.shaped {
            self.shaped = *v;
            let [slope, low, high, peak, gain, width, ..] = *v;
            let exponent = slope / 6.0206;
            self.s.set_shape(self.rate, |hz| {
                let tilt = (hz / 1000.0).powf(exponent);
                let hp = 1.0 / (1.0 + (low / hz).powi(4)).sqrt();
                let lp = 1.0 / (1.0 + (hz / high).powi(4)).sqrt();
                let o = (hz / peak).log2() / (width * 0.5);
                tilt * hp * lp * db(gain * (-0.5 * o * o).exp())
            });
        }
        for i in 0..BLOCK {
            out.add(i, self.s.next());
        }
    }
}

pub struct DigitalGen {
    rate: f32,
    r: [Lfsr; 2],
    lp: Lp2,
}

impl DigitalGen {
    pub fn new(rate: f32, seed: u32) -> DigitalGen {
        DigitalGen { rate, r: [Lfsr::new(CHIPS[0], seed), Lfsr::new(CHIPS[0], seed.wrapping_mul(7919) | 2)], lp: Lp2::new(rate) }
    }
}

impl Gen for DigitalGen {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let chip = CHIPS[pick(v[0]).min(CHIPS.len() - 1)];
        let steps = 2f32.powf(v[2].round() - 1.0);
        self.lp.set(tone_cut(v[3]), self.rate);
        let mut s = Block::new();
        for i in 0..BLOCK {
            let mut x = [0.0; 2];
            for c in 0..2 {
                self.r[c].chip = chip;
                let y = self.r[c].next(v[1], self.rate);
                x[c] = if v[2] < 15.5 { (y * steps).round() / steps } else { y };
            }
            s.add(i, (x[0], x[1]));
        }
        super::add_filtered(&mut s, &mut self.lp, 0.8, out);
    }
}

pub struct RandomGen {
    rate: f32,
    r: [Random; 2],
    lp: Lp2,
    drift: Drift,
}

impl RandomGen {
    pub fn new(rate: f32, seed: u32) -> RandomGen {
        RandomGen { rate, r: [Random::new(seed), Random::new(seed ^ 0x3c3c)], lp: Lp2::new(rate), drift: Drift::of(Smoothness::Value, seed) }
    }
}

impl Gen for RandomGen {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let shape = [Shape::Step, Shape::Linear, Shape::Cubic][pick(v[0]).min(2)];
        self.lp.set(tone_cut(v[2]), self.rate);
        let d = self.drift.next(0.3, BLOCK as f32 / self.rate);
        let g = 0.3 * (1.0 + v[3] * d).max(0.0);
        let mut s = Block::new();
        for i in 0..BLOCK {
            let l = self.r[0].next(shape, v[1], self.rate);
            let r = self.r[1].next(shape, v[1], self.rate);
            s.add(i, (l, r));
        }
        super::add_filtered(&mut s, &mut self.lp, g, out);
    }
}

pub struct ChaosGen {
    rate: f32,
    c: [Chaos; 2],
    dc: [OnePole; 2],
    lp: Lp2,
}

impl ChaosGen {
    pub fn new(rate: f32, seed: u32) -> ChaosGen {
        ChaosGen { rate, c: [Chaos::new(MAPS[0], seed), Chaos::new(MAPS[0], seed.wrapping_add(37))], dc: [OnePole::new(20.0, rate); 2], lp: Lp2::new(rate) }
    }
}

impl Gen for ChaosGen {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let map = MAPS[pick(v[0]).min(MAPS.len() - 1)];
        self.lp.set(tone_cut(v[3]), self.rate);
        let mut s = Block::new();
        for i in 0..BLOCK {
            let mut x = [0.0; 2];
            for c in 0..2 {
                self.c[c].map = map;
                x[c] = self.dc[c].high(self.c[c].next(v[1], v[2], self.rate)).clamp(-1.5, 1.5);
            }
            s.add(i, (x[0], x[1]));
        }
        super::add_filtered(&mut s, &mut self.lp, 0.35, out);
    }
}

pub struct CrackleGen {
    rate: f32,
    rng: Rng,
    pops: [Burst; 24],
    ticker: Ticker,
    cluster: u32,
    cluster_wait: f32,
    hiss: [ColorNoise; 2],
    hiss_hp: [OnePole; 2],
    shutter: f32,
    lp: Lp2,
}

impl CrackleGen {
    pub fn new(rate: f32, seed: u32) -> CrackleGen {
        CrackleGen {
            rate,
            rng: Rng::new(seed),
            pops: [Burst::default(); 24],
            ticker: Ticker::new(),
            cluster: 0,
            cluster_wait: 0.0,
            hiss: [ColorNoise::new(COLORS[2], seed, rate), ColorNoise::new(COLORS[2], seed ^ 77, rate)],
            hiss_hp: [OnePole::new(700.0, rate); 2],
            shutter: 0.0,
            lp: Lp2::new(rate),
        }
    }

    fn pop(&mut self, kind: usize, size: f32) {
        let rate = self.rate;
        let r = &mut self.rng;
        let (f, q, d, g) = match kind {
            // Vinyl: sharp ticks, now and then a fat pop.
            0 if r.chance(0.06) => (r.range(250.0, 600.0), 0.8, 0.006, 0.9),
            0 => (r.range(1500.0, 7000.0), 0.7, r.range(0.0004, 0.002), r.uni() * r.uni()),
            // Fire: woody snaps.
            1 => (r.range(700.0, 4500.0), 1.2, r.range(0.002, 0.015), r.uni()),
            // Sparks: hard, bright and very short.
            2 => (r.range(3000.0, 9000.0), 1.5, r.range(0.0003, 0.001), r.range(0.3, 1.0)),
            // Old film: dust on the print.
            _ => (r.range(800.0, 4000.0), 0.9, r.range(0.001, 0.004), r.uni() * 0.8),
        };
        let p = r.bi() * 0.8;
        if let Some(b) = super::super::dsp::free(&mut self.pops) {
            b.start(f, q, 0.0001, d * (0.5 + size), g * (0.3 + size), p, rate);
        }
    }
}

impl Gen for CrackleGen {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let kind = pick(v[0]).min(3);
        let (density, size, hiss) = (v[1], v[2], v[3]);
        self.lp.set(tone_cut(v[4]), self.rate);
        let dt = 1.0 / self.rate;
        let mut s = Block::new();
        for i in 0..BLOCK {
            if self.ticker.fire(&mut self.rng, density, 0.0, dt) {
                self.pop(kind, size);
                // Sparks and fire come in clusters.
                if kind == 1 && self.rng.chance(0.3) {
                    self.cluster = 1 + self.rng.below(4) as u32;
                } else if kind == 2 && self.rng.chance(0.5) {
                    self.cluster = 4 + self.rng.below(14) as u32;
                }
            }
            if self.cluster > 0 {
                self.cluster_wait -= dt;
                if self.cluster_wait <= 0.0 {
                    self.cluster -= 1;
                    self.cluster_wait = self.rng.range(0.003, 0.03);
                    self.pop(kind, size);
                }
            }
            let mut x = (0.0, 0.0);
            for b in &mut self.pops {
                let y = b.tick(&mut self.rng, self.rate);
                x.0 += y.0;
                x.1 += y.1;
            }
            let mut h = [0.0; 2];
            for c in 0..2 {
                h[c] = self.hiss_hp[c].high(self.hiss[c].next()) * hiss * 0.5;
            }
            // Old film: the projector's shutter flutter under it.
            if kind == 3 {
                self.shutter = (self.shutter + 24.0 * dt).fract();
                let f = if self.shutter < 0.15 { 1.0 } else { 0.55 };
                h[0] *= f;
                h[1] *= f;
            }
            s.add(i, (x.0 + h[0], x.1 + h[1]));
        }
        super::add_filtered(&mut s, &mut self.lp, 0.8, out);
    }
}

pub struct StaticGen {
    rate: f32,
    rng: Rng,
    hiss: [ColorNoise; 2],
    band: [Svf; 2],
    hp: [Svf; 2],
    crackles: [Burst; 12],
    ticker: Ticker,
    whistles: [(f32, f32, Drift); 3],
    fade: Drift,
    warble: f32,
    line: f32,
    lp: Lp2,
}

impl StaticGen {
    pub fn new(rate: f32, seed: u32) -> StaticGen {
        StaticGen {
            rate,
            rng: Rng::new(seed),
            hiss: [ColorNoise::new(COLORS[0], seed, rate), ColorNoise::new(COLORS[0], seed ^ 91, rate)],
            band: [Svf::new(1800.0, 0.5, rate); 2],
            hp: [Svf::new(250.0, 0.7, rate); 2],
            crackles: [Burst::default(); 12],
            ticker: Ticker::new(),
            whistles: [(0.0, 900.0, Drift::new(seed + 1)), (0.0, 1700.0, Drift::new(seed + 2)), (0.0, 2600.0, Drift::new(seed + 3))],
            fade: Drift::new(seed + 9),
            warble: 0.0,
            line: 0.0,
            lp: Lp2::new(rate),
        }
    }
}

impl Gen for StaticGen {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let kind = pick(v[0]).min(2);
        let (hiss, crackle, whistles, fading) = (v[1], v[2], v[3], v[4]);
        let rate = self.rate;
        self.lp.set(tone_cut(v[5]), rate);
        let bt = BLOCK as f32 / rate;
        let fade = 1.0 - fading * 0.85 * (0.5 + 0.5 * self.fade.next(if kind == 1 { 0.4 } else { 0.15 }, bt));
        let mut wf = [0.0; 3];
        for (k, w) in self.whistles.iter_mut().enumerate() {
            wf[k] = w.1 * (1.0 + 0.25 * w.2.next(0.05, bt));
        }
        let dt = 1.0 / rate;
        let mut s = Block::new();
        for i in 0..BLOCK {
            if self.ticker.fire(&mut self.rng, 2.0 + 60.0 * crackle, 0.0, dt) && crackle > 0.0 {
                let p = self.rng.bi() * 0.5;
                let (f, d, g) = (self.rng.range(400.0, 4000.0), self.rng.range(0.001, 0.02), self.rng.uni() * crackle * 1.5);
                if let Some(b) = super::super::dsp::free(&mut self.crackles) {
                    b.start(f, 0.6, 0.0002, d, g, p, rate);
                }
            }
            let mut x = [0.0; 2];
            for c in 0..2 {
                let w = self.hiss[c].next() * 2.0;
                x[c] = match kind {
                    // TV snow is the whole band, bright.
                    2 => w * 0.6,
                    _ => self.band[c].band(self.hp[c].high(w)) * 0.9,
                } * hiss;
            }
            let mut tone = 0.0;
            for (k, w) in self.whistles.iter_mut().enumerate() {
                w.0 = (w.0 + wf[k] * dt).fract();
                tone += super::super::dsp::sin1(w.0) * [0.06, 0.04, 0.025][k];
            }
            tone *= whistles * if kind == 2 { 0.2 } else { 1.0 };
            // Shortwave warbles; a TV's line whine sits up high.
            let mut am = 1.0;
            if kind == 1 {
                self.warble = (self.warble + 5.5 * dt).fract();
                am = 0.75 + 0.25 * super::super::dsp::sin1(self.warble);
            }
            if kind == 2 {
                self.line = (self.line + 15_734.0 * dt).fract();
                tone += super::super::dsp::sin1(self.line) * 0.01;
            }
            let mut cr = (0.0, 0.0);
            for b in &mut self.crackles {
                let y = b.tick(&mut self.rng, rate);
                cr.0 += y.0;
                cr.1 += y.1;
            }
            s.add(i, ((x[0] * am + tone) * fade + cr.0, (x[1] * am + tone) * fade + cr.1));
        }
        super::add_filtered(&mut s, &mut self.lp, 0.8, out);
    }
}
