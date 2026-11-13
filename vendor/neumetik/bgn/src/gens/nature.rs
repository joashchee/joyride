// Copyright 2026 ansiapps. Neumetik BGN: proprietary, see the LICENSE at the repository root.

//! Weather, water, fire and earth. Wind, rain and fire follow Andy
//! Farnell's procedural recipes ("Designing Sound", 2010) in spirit:
//! shaped noise moved by slow random curves, plus the events (drops,
//! crackles) that sell it. Water is van den Doel's bubbles.

use super::super::dsp::{free, pan, sin1, Block, Bubble, Burst, Drift, Modal, OnePole, Rng, Svf, Ticker, BLOCK};
use super::super::noise::{Color, ColorNoise};
use super::{add_filtered, far, pick, tone_cut, Gen, Lp2};

fn pair(color: Color, seed: u32, rate: f32) -> [ColorNoise; 2] {
    [ColorNoise::new(color, seed, rate), ColorNoise::new(color, seed.wrapping_mul(2_654_435_761) ^ 0x55, rate)]
}

fn sum<const N: usize>(voices: &mut [Burst; N], rng: &mut Rng, rate: f32) -> (f32, f32) {
    let mut x = (0.0, 0.0);
    for b in voices.iter_mut() {
        let y = b.tick(rng, rate);
        x.0 += y.0;
        x.1 += y.1;
    }
    x
}

fn sum_bubbles<const N: usize>(voices: &mut [Bubble; N], rate: f32) -> (f32, f32) {
    let mut x = (0.0, 0.0);
    for b in voices.iter_mut() {
        let y = b.tick(rate);
        x.0 += y.0;
        x.1 += y.1;
    }
    x
}

fn sum_modal(voices: &mut [Modal], rng: &mut Rng) -> (f32, f32) {
    let mut x = (0.0, 0.0);
    for m in voices.iter_mut() {
        let y = m.tick(rng);
        x.0 += y.0;
        x.1 += y.1;
    }
    x
}

pub struct Wind {
    rate: f32,
    n: [ColorNoise; 2],
    body: [Svf; 2],
    whistle: [Svf; 3],
    white: ColorNoise,
    buffet: [Svf; 2],
    gust: [Drift; 2],
    wander: Drift,
    lp: Lp2,
}

impl Wind {
    pub fn new(rate: f32, seed: u32) -> Wind {
        Wind {
            rate,
            n: pair(Color::Pink, seed, rate),
            body: [Svf::new(400.0, 1.0, rate); 2],
            whistle: [Svf::new(800.0, 30.0, rate); 3],
            white: ColorNoise::new(Color::White, seed ^ 0x77, rate),
            buffet: [Svf::new(90.0, 0.7, rate); 2],
            gust: [Drift::new(seed), Drift::new(seed ^ 0x1234)],
            wander: Drift::new(seed + 5),
            lp: Lp2::new(rate),
        }
    }
}

impl Gen for Wind {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [strength, gusts, whistle, pitch, tone, speed] = *v;
        let rate = self.rate;
        let bt = BLOCK as f32 / rate;
        self.lp.set(tone_cut(tone), rate);
        let mut g = [0.0; 2];
        for c in 0..2 {
            // Gusts: a slow wander, sharpened so the wind rises and dies off.
            let d = 0.5 + 0.5 * self.gust[c].next(speed, bt);
            g[c] = (1.0 - gusts) * 0.7 + gusts * d.powf(1.6) * 1.4;
            let f = pitch * (0.5 + 1.1 * g[c]) * (0.8 + 0.4 * strength);
            self.body[c].set(f, 0.8 + 1.5 * (1.0 - strength), rate);
        }
        let gm = (g[0] + g[1]) * 0.5;
        let w = 1.0 + 0.1 * self.wander.next(0.05, bt);
        for (k, f) in self.whistle.iter_mut().enumerate() {
            f.set(pitch * [2.1, 3.3, 4.7][k] * (0.85 + 0.4 * gm) * w, 25.0 + 15.0 * k as f32, rate);
        }
        let level = strength * (0.6 + 0.4 * strength);
        let mut s = Block::new();
        for i in 0..BLOCK {
            let wn = self.white.next() * 2.0;
            let mut whistles = 0.0;
            if whistle > 0.0 {
                for f in &mut self.whistle {
                    whistles += f.band(wn);
                }
                whistles *= whistle * gm * gm * 0.6;
            }
            let mut x = [0.0; 2];
            for c in 0..2 {
                let n = self.n[c].next() * 2.5;
                let body = self.body[c].band(n) * g[c];
                let buffet = self.buffet[c].low(n) * g[c] * g[c] * strength * 0.8;
                x[c] = (body + buffet) * level + whistles;
            }
            s.add(i, (x[0], x[1]));
        }
        add_filtered(&mut s, &mut self.lp, 0.9, out);
    }
}

pub struct Foliage {
    rate: f32,
    rng: Rng,
    n: [ColorNoise; 2],
    band: [Svf; 2],
    grain: [f32; 2],
    gust: Drift,
    clacks: Vec<Modal>,
    lp: Lp2,
}

impl Foliage {
    pub fn new(rate: f32, seed: u32) -> Foliage {
        Foliage {
            rate,
            rng: Rng::new(seed),
            n: pair(Color::White, seed, rate),
            band: [Svf::new(3000.0, 0.8, rate); 2],
            grain: [0.0; 2],
            gust: Drift::new(seed),
            clacks: vec![Modal::default(); 6],
            lp: Lp2::new(rate),
        }
    }
}

impl Gen for Foliage {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let (amount, plant, gusts) = (v[0], pick(v[1]).min(5), v[2]);
        let rate = self.rate;
        // Center, how smooth (0 granular, 1 a hiss) and how many rustles a second at full wind.
        let (f, smooth, grains) = [(3000.0, 0.45, 400.0), (5200.0, 0.85, 900.0), (4200.0, 0.6, 600.0), (6000.0, 0.1, 250.0), (2600.0, 0.4, 300.0), (2200.0, 0.55, 350.0)][plant];
        for b in &mut self.band {
            b.set(f, 0.7, rate);
        }
        self.lp.set(tone_cut(v[3]), rate);
        let d = 0.5 + 0.5 * self.gust.next(0.25, BLOCK as f32 / rate);
        let g = ((1.0 - gusts) * 0.6 + gusts * d.powf(1.5) * 1.5) * amount;
        let decay = (-1.0 / (0.005 * rate)).exp();
        let p = grains * g / rate;
        let mut s = Block::new();
        for i in 0..BLOCK {
            // Bamboo knocks against itself in the wind.
            if plant == 4 && self.rng.chance(g * g * 6.0 / rate) {
                let (base, pn, gain) = (self.rng.range(350.0, 800.0), self.rng.bi() * 0.8, self.rng.range(0.1, 0.3));
                if let Some(m) = free(&mut self.clacks) {
                    m.start(base, &[(1.0, 0.06, 1.0), (2.42, 0.03, 0.5), (3.9, 0.02, 0.3)], 1.0, gain, pn, rate);
                }
            }
            let mut x = [0.0; 2];
            for c in 0..2 {
                if self.rng.chance(p) {
                    self.grain[c] += self.rng.range(0.5, 1.5);
                }
                self.grain[c] *= decay;
                let env = smooth * g + (1.0 - smooth) * self.grain[c].min(2.5) * 0.7;
                x[c] = self.band[c].band(self.n[c].next() * 2.0) * env * 0.5;
            }
            let k = sum_modal(&mut self.clacks, &mut self.rng);
            s.add(i, (x[0] + k.0, x[1] + k.1));
        }
        add_filtered(&mut s, &mut self.lp, 1.0, out);
    }
}

pub struct Rain {
    rate: f32,
    rng: Rng,
    bed: [ColorNoise; 2],
    bed_lp: [Svf; 2],
    bed_hp: [Svf; 2],
    drops: [Burst; 48],
    rings: Vec<Modal>,
    bubbles: [Bubble; 24],
    ticker: Ticker,
    swell: Drift,
    lp: Lp2,
}

impl Rain {
    pub fn new(rate: f32, seed: u32) -> Rain {
        Rain {
            rate,
            rng: Rng::new(seed),
            bed: pair(Color::Pink, seed, rate),
            bed_lp: [Svf::new(6000.0, 0.6, rate); 2],
            bed_hp: [Svf::new(300.0, 0.6, rate); 2],
            drops: [Burst::default(); 48],
            rings: vec![Modal::default(); 10],
            bubbles: [Bubble::default(); 24],
            ticker: Ticker::new(),
            swell: Drift::new(seed),
            lp: Lp2::new(rate),
        }
    }

    fn drop(&mut self, surface: usize, size: f32, close: f32) {
        let rate = self.rate;
        let r = &mut self.rng;
        // Near drops are louder and spread wider.
        let near = r.chance(0.15 + 0.6 * close);
        let gain = if near { r.range(0.3, 1.0) } else { r.range(0.03, 0.25) } * (0.5 + size);
        let p = r.bi() * if near { 0.95 } else { 0.6 };
        let big = 1.0 - size * 0.6;
        match surface {
            // Tin and car roofs ring; windows ring a little.
            2 | 7 | 4 => {
                let (base, partials, decay): (f32, &[(f32, f32, f32)], f32) = match surface {
                    2 => (r.range(1800.0, 4200.0) * big, &[(1.0, 0.05, 1.0), (2.32, 0.03, 0.6), (4.25, 0.02, 0.4), (6.63, 0.015, 0.2)], 1.0),
                    7 => (r.range(500.0, 1200.0) * big, &[(1.0, 0.04, 1.0), (1.59, 0.03, 0.6), (2.14, 0.02, 0.4)], 1.0),
                    _ => (r.range(3500.0, 7000.0), &[(1.0, 0.02, 1.0), (2.7, 0.01, 0.4)], 1.0),
                };
                if let Some(m) = free(&mut self.rings) {
                    m.start(base, partials, decay, gain * 0.35, p, rate);
                }
                let f = r.range(2000.0, 6000.0);
                if let Some(b) = free(&mut self.drops) {
                    b.start(f, 0.9, 0.0002, 0.002, gain * 0.5, p, rate);
                }
            }
            // Into water: a bubble's plink and a small splash.
            8 | 9 => {
                let radius = r.range(1.2, 4.5) * (0.6 + size);
                if let Some(b) = free(&mut self.bubbles) {
                    b.start(radius, r.range(0.5, 2.0), gain * 0.4, p, rate);
                }
                let f = r.range(1500.0, 5000.0);
                if let Some(b) = free(&mut self.drops) {
                    b.start(f, 0.8, 0.0003, 0.003, gain * 0.3, p, rate);
                }
            }
            _ => {
                // Center, Q and decay for a tick on the ground, leaves, tile, a tent, an umbrella, snow.
                let (lo, hi, q, d) = match surface {
                    1 => (2500.0, 7000.0, 1.0, 0.004),
                    3 => (1000.0, 3200.0, 1.4, 0.004),
                    5 => (250.0, 800.0, 1.6, 0.014),
                    6 => (600.0, 1600.0, 1.3, 0.009),
                    10 => (800.0, 2500.0, 0.6, 0.002),
                    _ => (1200.0, 5000.0, 0.8, 0.004),
                };
                let f = r.range(lo, hi) * big;
                let g = if surface == 10 { gain * 0.1 } else { gain };
                if let Some(b) = free(&mut self.drops) {
                    b.start(f, q, 0.0002, d * (0.6 + size), g, p, rate);
                }
            }
        }
    }
}

impl Gen for Rain {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let (intensity, size, surface, close) = (v[0], v[1], pick(v[2]).min(10), v[3]);
        let rate = self.rate;
        self.lp.set(tone_cut(v[4]), rate);
        // The wash's brightness by surface: tin is loud and bright, snow all but silent.
        let (bed_cut, bed_gain) = [(5000.0, 1.0), (7000.0, 1.0), (8000.0, 1.3), (5000.0, 1.0), (6000.0, 0.6), (2500.0, 1.0), (3500.0, 0.7), (4000.0, 0.8), (4500.0, 0.9), (3500.0, 1.0), (1200.0, 0.25)][surface];
        let swell = 1.0 + 0.15 * self.swell.next(0.08, BLOCK as f32 / rate);
        for c in 0..2 {
            self.bed_lp[c].set(bed_cut * (0.7 + 0.6 * intensity), 0.6, rate);
            self.bed_hp[c].set(250.0, 0.6, rate);
        }
        let drops = if surface == 10 { 0.0 } else { 15.0 + 1800.0 * intensity * intensity };
        let bed = intensity.powf(0.8) * bed_gain * swell * (1.0 - 0.4 * close);
        let dt = 1.0 / rate;
        let mut s = Block::new();
        for i in 0..BLOCK {
            while drops > 0.0 && self.ticker.fire(&mut self.rng, drops, 0.0, dt) {
                self.drop(surface, size, close);
            }
            let mut x = [0.0; 2];
            for c in 0..2 {
                x[c] = self.bed_lp[c].low(self.bed_hp[c].high(self.bed[c].next())) * bed * 0.9;
            }
            let a = sum(&mut self.drops, &mut self.rng, rate);
            let b = sum_modal(&mut self.rings, &mut self.rng);
            let c = sum_bubbles(&mut self.bubbles, rate);
            s.add(i, (x[0] + (a.0 + b.0 + c.0) * 0.5, x[1] + (a.1 + b.1 + c.1) * 0.5));
        }
        add_filtered(&mut s, &mut self.lp, 1.0, out);
    }
}

/// One stroke of lightning, heard as a crack and a roll.
#[derive(Clone, Copy, Default)]
struct Strike {
    t: f32,
    len: f32,
    bumps: [(f32, f32, f32); 8],
    gain: f32,
    pan: f32,
    near: f32,
    crack: f32,
    lp: [Svf; 2],
}

impl Strike {
    fn live(&self) -> bool {
        self.t < self.len
    }
}

pub struct Thunder {
    rate: f32,
    rng: Rng,
    strikes: [Strike; 3],
    ticker: Ticker,
    n: [ColorNoise; 2],
    white: ColorNoise,
    crack_hp: Svf,
}

impl Thunder {
    pub fn new(rate: f32, seed: u32) -> Thunder {
        let mut rng = Rng::new(seed);
        let ticker = Ticker::primed(&mut rng, 0.05);
        Thunder {
            rate,
            rng,
            strikes: [Strike::default(); 3],
            ticker,
            n: pair(Color::Brown, seed, rate),
            white: ColorNoise::new(Color::White, seed ^ 3, rate),
            crack_hp: Svf::new(1500.0, 0.7, rate),
        }
    }
}

impl Gen for Thunder {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [per_min, distance, power, length, ..] = *v;
        let rate = self.rate;
        let dt = 1.0 / rate;
        // Seconds into a block are fine for scheduling thunder.
        if self.ticker.fire(&mut self.rng, per_min / 60.0, 0.0, BLOCK as f32 * dt) {
            let r = &mut self.rng;
            let d = (distance + r.range(-0.2, 0.2)).clamp(0.0, 1.0);
            if let Some(s) = self.strikes.iter_mut().find(|s| !s.live()) {
                s.t = 0.0;
                s.len = 3.0 + 12.0 * length * r.range(0.6, 1.2) + 4.0 * d;
                let mut at = if d < 0.3 { 0.0 } else { r.range(0.1, 0.6) };
                for b in &mut s.bumps {
                    *b = (at, r.range(0.15, 0.9) * s.len * 0.12, r.range(0.3, 1.0));
                    at += r.range(0.1, 0.35) * s.len * 0.45;
                }
                s.bumps[0].2 = 1.0;
                s.gain = power * (1.0 - 0.6 * d) * 1.1;
                s.pan = r.bi() * 0.7;
                s.near = 1.0 - d;
                s.crack = if d < 0.35 { 1.0 } else { 0.0 };
                let cut = 120.0 + 900.0 * (1.0 - d).powi(2);
                s.lp = [Svf::new(cut, 0.6, rate); 2];
            }
        }
        for i in 0..BLOCK {
            let n = [self.n[0].next() * 3.0, self.n[1].next() * 3.0];
            let w = self.white.next();
            let mut x = (0.0, 0.0);
            for s in self.strikes.iter_mut().filter(|s| s.live()) {
                let mut env = 0.0;
                for &(at, width, a) in &s.bumps {
                    let u = (s.t - at) / width.max(0.05);
                    if u > 0.0 {
                        env += a * u * (1.0 - u).exp();
                    }
                }
                let fade = 1.0 - (s.t / s.len).powi(3);
                env = env.min(1.6) * fade * s.gain;
                let l = s.lp[0].low(n[0]) * env;
                let r = s.lp[1].low(n[1]) * env;
                let mut c = 0.0;
                if s.crack > 0.0 && s.t < 0.6 {
                    c = self.crack_hp.high(w) * (-s.t * 9.0).exp() * s.gain * s.near * 0.9;
                }
                let (pl, pr) = pan(1.0, s.pan);
                x.0 += l * pl + c * pl;
                x.1 += r * pr + c * pr;
                s.t += dt;
            }
            out.add(i, x);
        }
    }
}

pub struct Stream {
    rate: f32,
    rng: Rng,
    bubbles: [Bubble; 64],
    ticker: Ticker,
    rush: [ColorNoise; 2],
    band: [Svf; 2],
    flow: Drift,
    lp: Lp2,
}

impl Stream {
    pub fn new(rate: f32, seed: u32) -> Stream {
        Stream {
            rate,
            rng: Rng::new(seed),
            bubbles: [Bubble::default(); 64],
            ticker: Ticker::new(),
            rush: pair(Color::Pink, seed, rate),
            band: [Svf::new(900.0, 0.6, rate); 2],
            flow: Drift::new(seed),
            lp: Lp2::new(rate),
        }
    }
}

impl Gen for Stream {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [flow, bubbles, pitch, turbulence, tone, _] = *v;
        let rate = self.rate;
        self.lp.set(tone_cut(tone), rate);
        let d = self.flow.next(0.3 + turbulence, BLOCK as f32 / rate);
        let surge = (1.0 + turbulence * 0.6 * d).max(0.1);
        for b in &mut self.band {
            b.set(500.0 + 2500.0 * flow, 0.5, rate);
        }
        let per_sec = (30.0 + 1200.0 * flow * flow) * surge * (0.2 + bubbles);
        let (r_lo, r_hi) = (0.6 + (1.0 - pitch) * 3.0, 2.5 + (1.0 - pitch) * 12.0);
        let rush = flow.powf(1.3) * (1.0 - 0.5 * bubbles) * surge * 0.6;
        let dt = 1.0 / rate;
        let mut s = Block::new();
        for i in 0..BLOCK {
            while self.ticker.fire(&mut self.rng, per_sec, 0.0, dt) {
                let r = &mut self.rng;
                // Many small bubbles, few large ones.
                let radius = r_lo * (r_hi / r_lo).powf(r.uni() * r.uni());
                let (rise, g, p) = (r.range(0.0, 1.5 + turbulence * 3.0), r.range(0.02, 0.12) * (0.4 + bubbles), r.bi() * 0.8);
                if let Some(b) = free(&mut self.bubbles) {
                    b.start(radius, rise, g, p, rate);
                }
            }
            let b = sum_bubbles(&mut self.bubbles, rate);
            let l = self.band[0].band(self.rush[0].next() * 2.0) * rush;
            let r = self.band[1].band(self.rush[1].next() * 2.0) * rush;
            s.add(i, (l + b.0, r + b.1));
        }
        add_filtered(&mut s, &mut self.lp, 1.0, out);
    }
}

pub struct Surf {
    rate: f32,
    rng: Rng,
    n: [ColorNoise; 2],
    white: [ColorNoise; 2],
    body: [Svf; 2],
    hiss: [Svf; 2],
    t: f32,
    period: f32,
    wave_pan: f32,
    wave_size: f32,
    rattle: [Burst; 24],
    bubbles: [Bubble; 24],
    lp: Lp2,
}

impl Surf {
    pub fn new(rate: f32, seed: u32) -> Surf {
        let mut rng = Rng::new(seed);
        let t = rng.uni() * 6.0;
        Surf {
            rate,
            rng,
            n: pair(Color::Pink, seed, rate),
            white: pair(Color::White, seed ^ 99, rate),
            body: [Svf::new(500.0, 0.6, rate); 2],
            hiss: [Svf::new(4000.0, 0.6, rate); 2],
            t,
            period: 8.0,
            wave_pan: 0.0,
            wave_size: 1.0,
            rattle: [Burst::default(); 24],
            bubbles: [Bubble::default(); 24],
            lp: Lp2::new(rate),
        }
    }
}

impl Gen for Surf {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [size, period, crash, shore, tone, _] = *v;
        let shore = pick(shore).min(3);
        let rate = self.rate;
        self.lp.set(tone_cut(tone), rate);
        let bt = BLOCK as f32 / rate;
        self.t += bt;
        if self.t >= self.period {
            self.t -= self.period;
            self.period = period * self.rng.range(0.75, 1.25);
            self.wave_pan = self.rng.bi() * 0.5;
            self.wave_size = self.rng.range(0.6, 1.2);
        }
        let u = self.t / self.period;
        // A wave's life: the swell, the break, the wash, the draw back.
        let (body, wash) = if shore == 3 {
            // Open sea: rolling swells, no break.
            (0.4 + 0.6 * (0.5 - 0.5 * (u * std::f32::consts::TAU).cos()), 0.15)
        } else if u < 0.3 {
            ((u / 0.3).powi(2) * 0.35, 0.0)
        } else if u < 0.38 {
            (0.35 + 0.65 * ((u - 0.3) / 0.08), (u - 0.3) / 0.08 * crash)
        } else {
            let k = (u - 0.38) / 0.62;
            ((1.0 - k).powi(2), (1.0 - k).powf(0.7) * (0.3 + 0.7 * crash))
        };
        let level = (0.3 + 0.7 * size) * self.wave_size;
        for c in 0..2 {
            self.body[c].set(150.0 + 1800.0 * body * (0.3 + crash * 0.7) * (0.5 + size), 0.6, rate);
            self.hiss[c].set(2500.0 + 3000.0 * wash, 0.7, rate);
        }
        // Shingle and rocks rattle and bubble as the water draws back.
        let backwash = u > 0.45 && shore != 3;
        let events = if backwash { wash * 300.0 } else { 0.0 };
        let mut s = Block::new();
        let dt = 1.0 / rate;
        for i in 0..BLOCK {
            if events > 0.0 && self.rng.chance(events * dt) {
                let p = self.wave_pan + self.rng.bi() * 0.6;
                match shore {
                    1 => {
                        let (f, g) = (self.rng.range(1500.0, 5000.0), self.rng.range(0.05, 0.3) * size);
                        if let Some(b) = free(&mut self.rattle) {
                            b.start(f, 1.5, 0.0002, 0.006, g, p, rate);
                        }
                    }
                    2 => {
                        let (rad, g) = (self.rng.range(2.0, 9.0), self.rng.range(0.03, 0.12) * size);
                        if let Some(b) = free(&mut self.bubbles) {
                            b.start(rad, 0.8, g, p, rate);
                        }
                    }
                    _ => {}
                }
            }
            let mut x = [0.0; 2];
            for c in 0..2 {
                let b = self.body[c].low(self.n[c].next() * 2.5) * body * level;
                let h = self.hiss[c].band(self.white[c].next()) * wash * level * if shore == 0 { 0.7 } else { 0.45 };
                x[c] = b + h;
            }
            let (pl, pr) = pan(1.0, self.wave_pan);
            let a = sum(&mut self.rattle, &mut self.rng, rate);
            let b = sum_bubbles(&mut self.bubbles, rate);
            s.add(i, (x[0] * pl + a.0 + b.0, x[1] * pr + a.1 + b.1));
        }
        add_filtered(&mut s, &mut self.lp, 0.9, out);
    }
}

pub struct Lapping {
    rate: f32,
    rng: Rng,
    slaps: [Burst; 8],
    bubbles: [Bubble; 16],
    hollow: [Svf; 2],
    ticker: Ticker,
    bed: [ColorNoise; 2],
    bed_lp: [Svf; 2],
    lp: Lp2,
}

impl Lapping {
    pub fn new(rate: f32, seed: u32) -> Lapping {
        Lapping {
            rate,
            rng: Rng::new(seed),
            slaps: [Burst::default(); 8],
            bubbles: [Bubble::default(); 16],
            hollow: [Svf::new(170.0, 8.0, rate), Svf::new(260.0, 8.0, rate)],
            ticker: Ticker::new(),
            bed: pair(Color::Pink, seed, rate),
            bed_lp: [Svf::new(500.0, 0.6, rate); 2],
            lp: Lp2::new(rate),
        }
    }
}

impl Gen for Lapping {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [per_sec, size, hollow, tone, ..] = *v;
        let rate = self.rate;
        self.lp.set(tone_cut(tone), rate);
        let dt = 1.0 / rate;
        let mut s = Block::new();
        for i in 0..BLOCK {
            if self.ticker.fire(&mut self.rng, per_sec, 0.3, dt) {
                let r = &mut self.rng;
                let (f, d, g, p) = (r.range(250.0, 900.0) * (1.2 - size * 0.5), r.range(0.08, 0.25) * (0.5 + size), r.range(0.3, 0.9) * (0.4 + size), r.bi() * 0.6);
                if let Some(b) = free(&mut self.slaps) {
                    b.start(f, 0.9, r.range(0.01, 0.04), d, g, p, rate);
                }
                for _ in 0..(2 + self.rng.below(5)) {
                    let (rad, g, p) = (self.rng.range(2.0, 8.0), self.rng.range(0.02, 0.07), self.rng.bi() * 0.6);
                    if let Some(b) = free(&mut self.bubbles) {
                        b.start(rad, 1.0, g, p, rate);
                    }
                }
            }
            let a = sum(&mut self.slaps, &mut self.rng, rate);
            let b = sum_bubbles(&mut self.bubbles, rate);
            let m = (a.0 + a.1) * 0.5;
            let h = (self.hollow[0].band(m) + self.hollow[1].band(m)) * hollow * 1.5;
            let bed = [self.bed_lp[0].low(self.bed[0].next()) * 0.25, self.bed_lp[1].low(self.bed[1].next()) * 0.25];
            s.add(i, (a.0 + b.0 + h + bed[0], a.1 + b.1 + h + bed[1]));
        }
        add_filtered(&mut s, &mut self.lp, 1.0, out);
    }
}

pub struct Torrent {
    rate: f32,
    pink: [ColorNoise; 2],
    brown: [ColorNoise; 2],
    white: [ColorNoise; 2],
    body: [Svf; 2],
    rumble: [Svf; 2],
    spray: [Svf; 2],
    surge: Drift,
}

impl Torrent {
    pub fn new(rate: f32, seed: u32) -> Torrent {
        Torrent {
            rate,
            pink: pair(Color::Pink, seed, rate),
            brown: pair(Color::Brown, seed ^ 1, rate),
            white: pair(Color::White, seed ^ 2, rate),
            body: [Svf::new(2000.0, 0.5, rate); 2],
            rumble: [Svf::new(120.0, 0.7, rate); 2],
            spray: [Svf::new(5000.0, 0.7, rate); 2],
            surge: Drift::new(seed),
        }
    }
}

impl Gen for Torrent {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [power, distance, spray, rumble, ..] = *v;
        let rate = self.rate;
        let (g, cut) = far(distance);
        for c in 0..2 {
            self.body[c].set(cut.min(5000.0) * (0.5 + power * 0.5), 0.5, rate);
            self.spray[c].set(4000.0, 0.6, rate);
        }
        let s = 1.0 + 0.08 * self.surge.next(0.4, BLOCK as f32 / rate);
        let level = power * g * s;
        let spray = spray * (1.0 - distance) * 0.5;
        for i in 0..BLOCK {
            let mut x = [0.0; 2];
            for c in 0..2 {
                let b = self.body[c].low(self.pink[c].next() * 2.0);
                let r = self.rumble[c].low(self.brown[c].next() * 2.0) * rumble * 1.2;
                let sp = self.spray[c].high(self.white[c].next()) * spray;
                x[c] = (b + r + sp) * level;
            }
            out.add(i, (x[0], x[1]));
        }
    }
}

pub struct Drips {
    rate: f32,
    rng: Rng,
    bubbles: [Bubble; 8],
    clicks: [Burst; 8],
    rings: Vec<Modal>,
    ticker: Ticker,
}

impl Drips {
    pub fn new(rate: f32, seed: u32) -> Drips {
        Drips { rate, rng: Rng::new(seed), bubbles: [Bubble::default(); 8], clicks: [Burst::default(); 8], rings: vec![Modal::default(); 4], ticker: Ticker::new() }
    }
}

impl Gen for Drips {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [per_sec, pitch, regular, size, into, _] = *v;
        let into = pick(into).min(3);
        let rate = self.rate;
        let dt = 1.0 / rate;
        for i in 0..BLOCK {
            if self.ticker.fire(&mut self.rng, per_sec, regular, dt) {
                let r = &mut self.rng;
                let p = r.bi() * 0.7;
                let g = r.range(0.3, 0.8) * (0.3 + size);
                match into {
                    0 | 3 => {
                        // The classic plink: a bubble whose pitch rises.
                        let rad = (1.5 + (1.0 - pitch) * 4.0) * r.range(0.8, 1.25) * if into == 3 { 0.7 } else { 1.0 };
                        if let Some(b) = free(&mut self.bubbles) {
                            b.start(rad, r.range(2.0, 6.0), g * 0.6, p, rate);
                        }
                        let f = r.range(2000.0, 5000.0);
                        if let Some(b) = free(&mut self.clicks) {
                            b.start(f, 1.0, 0.0002, 0.002, g * 0.15, p, rate);
                        }
                    }
                    1 => {
                        let f = r.range(1500.0, 4000.0) * (0.6 + pitch * 0.8);
                        if let Some(b) = free(&mut self.clicks) {
                            b.start(f, 2.5, 0.0002, 0.004, g * 0.8, p, rate);
                        }
                    }
                    _ => {
                        let base = r.range(1200.0, 2600.0) * (0.6 + pitch * 0.8);
                        if let Some(m) = free(&mut self.rings) {
                            m.start(base, &[(1.0, 0.25, 1.0), (2.76, 0.12, 0.4), (5.4, 0.06, 0.2)], 1.0, g * 0.25, p, rate);
                        }
                    }
                }
            }
            let a = sum_bubbles(&mut self.bubbles, rate);
            let b = sum(&mut self.clicks, &mut self.rng, rate);
            let c = sum_modal(&mut self.rings, &mut self.rng);
            out.add(i, (a.0 + b.0 + c.0, a.1 + b.1 + c.1));
        }
    }
}

pub struct Bubbling {
    rate: f32,
    rng: Rng,
    bubbles: [Bubble; 16],
    pops: [Burst; 8],
    ticker: Ticker,
    simmer: [ColorNoise; 2],
    simmer_lp: [Svf; 2],
}

impl Bubbling {
    pub fn new(rate: f32, seed: u32) -> Bubbling {
        Bubbling {
            rate,
            rng: Rng::new(seed),
            bubbles: [Bubble::default(); 16],
            pops: [Burst::default(); 8],
            ticker: Ticker::new(),
            simmer: pair(Color::Brown, seed, rate),
            simmer_lp: [Svf::new(300.0, 0.7, rate); 2],
        }
    }
}

impl Gen for Bubbling {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [per_sec, size, thick, pitch, ..] = *v;
        let rate = self.rate;
        let dt = 1.0 / rate;
        for i in 0..BLOCK {
            if self.ticker.fire(&mut self.rng, per_sec, 0.0, dt) {
                let r = &mut self.rng;
                let rad = (3.0 + size * 20.0) * (1.4 - pitch * 0.8) * r.range(0.6, 1.4);
                let (g, p) = (r.range(0.2, 0.6), r.bi() * 0.6);
                if let Some(b) = free(&mut self.bubbles) {
                    b.start(rad, r.range(0.5, 2.0), g * (1.0 - thick * 0.5), p, rate);
                }
                // A thick liquid's bubbles burst with a low pop.
                let (f, d) = (r.range(150.0, 600.0) * (1.4 - size * 0.6), 0.02 + 0.08 * thick);
                if let Some(b) = free(&mut self.pops) {
                    b.start(f, 1.2, 0.002, d, g * thick * 1.2, p, rate);
                }
            }
            let a = sum_bubbles(&mut self.bubbles, rate);
            let b = sum(&mut self.pops, &mut self.rng, rate);
            let sl = self.simmer_lp[0].low(self.simmer[0].next()) * 0.3 * thick;
            let sr = self.simmer_lp[1].low(self.simmer[1].next()) * 0.3 * thick;
            out.add(i, (a.0 + b.0 + sl, a.1 + b.1 + sr));
        }
    }
}

pub struct Underwater {
    rate: f32,
    rng: Rng,
    brown: [ColorNoise; 2],
    pink: [ColorNoise; 2],
    low: [Svf; 2],
    murmur: [Svf; 2],
    bubbles: [Bubble; 32],
    stream_left: u32,
    stream_wait: f32,
    stream_pan: f32,
    throb: f32,
    drift: Drift,
}

impl Underwater {
    pub fn new(rate: f32, seed: u32) -> Underwater {
        Underwater {
            rate,
            rng: Rng::new(seed),
            brown: pair(Color::Brown, seed, rate),
            pink: pair(Color::Pink, seed ^ 4, rate),
            low: [Svf::new(300.0, 0.7, rate); 2],
            murmur: [Svf::new(700.0, 0.7, rate); 2],
            bubbles: [Bubble::default(); 32],
            stream_left: 0,
            stream_wait: 0.0,
            stream_pan: 0.0,
            throb: 0.0,
            drift: Drift::new(seed),
        }
    }
}

impl Gen for Underwater {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [depth, bubbles, throb, murmur, ..] = *v;
        let rate = self.rate;
        let bt = BLOCK as f32 / rate;
        let d = self.drift.next(0.1, bt);
        for c in 0..2 {
            self.low[c].set(60.0 + 340.0 * (1.0 - depth) * (1.0 + 0.2 * d), 0.8, rate);
            self.murmur[c].set(400.0 + 600.0 * (1.0 - depth), 0.6, rate);
        }
        // A stream of bubbles now and then, rising past.
        if self.stream_left == 0 && self.rng.chance(bubbles * 0.25 * bt) {
            self.stream_left = 5 + self.rng.below(20) as u32;
            self.stream_pan = self.rng.bi() * 0.8;
        }
        let dt = 1.0 / rate;
        for i in 0..BLOCK {
            if self.stream_left > 0 {
                self.stream_wait -= dt;
                if self.stream_wait <= 0.0 {
                    self.stream_left -= 1;
                    self.stream_wait = self.rng.range(0.02, 0.12);
                    let (rad, g, p) = (self.rng.range(1.5, 6.0), self.rng.range(0.05, 0.18), self.stream_pan + self.rng.bi() * 0.1);
                    if let Some(b) = free(&mut self.bubbles) {
                        b.start(rad, 3.0, g, p, rate);
                    }
                }
            }
            self.throb = (self.throb + 0.35 * dt).fract();
            let t = 1.0 - throb * 0.6 * (0.5 + 0.5 * sin1(self.throb));
            let b = sum_bubbles(&mut self.bubbles, rate);
            let mut x = [0.0; 2];
            for c in 0..2 {
                let low = self.low[c].low(self.brown[c].next() * 2.5) * (0.4 + 0.6 * depth);
                let m = self.murmur[c].low(self.pink[c].next()) * murmur * 0.4;
                x[c] = (low + m) * t;
            }
            out.add(i, (x[0] + b.0, x[1] + b.1));
        }
    }
}

pub struct Ice {
    rate: f32,
    rng: Rng,
    zings: [super::super::dsp::Chirp; 6],
    booms: [Burst; 6],
    groan: Option<Groan>,
    rings: Vec<Modal>,
    ticker: Ticker,
}

/// A glacier's groan: stick-slip pulses through low resonances.
struct Groan {
    t: f32,
    len: f32,
    phase: f32,
    rate0: f32,
    rate1: f32,
    modes: [super::super::dsp::Mode; 3],
    gain: f32,
    pan: f32,
}

impl Ice {
    pub fn new(rate: f32, seed: u32) -> Ice {
        let mut rng = Rng::new(seed);
        let ticker = Ticker::primed(&mut rng, 0.3);
        Ice { rate, rng, zings: [Default::default(); 6], booms: [Burst::default(); 6], groan: None, rings: vec![Modal::default(); 8], ticker }
    }
}

impl Gen for Ice {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [per_sec, size, ring, kind, ..] = *v;
        let kind = pick(kind).min(2);
        let rate = self.rate;
        let dt = 1.0 / rate;
        for i in 0..BLOCK {
            if self.ticker.fire(&mut self.rng, per_sec, 0.0, dt) {
                let r = &mut self.rng;
                let p = r.bi() * 0.8;
                match kind {
                    // Lake ice: the dispersed zing of a crack running through a sheet.
                    0 => {
                        let len = 0.1 + 0.5 * ring * r.range(0.6, 1.4);
                        let (from, to) = (r.range(4000.0, 9000.0), r.range(150.0, 500.0) * (1.4 - size * 0.8));
                        if let Some(z) = free(&mut self.zings) {
                            z.start(from, to, len, r.range(0.2, 0.5), p, rate);
                        }
                        let f = r.range(800.0, 3000.0);
                        if let Some(b) = free(&mut self.booms) {
                            b.start(f, 0.7, 0.0002, 0.01, 0.4 * size, p, rate);
                        }
                    }
                    1 => {
                        if r.chance(0.5) && self.groan.is_none() {
                            let mut modes = [super::super::dsp::Mode::default(); 3];
                            for (k, m) in modes.iter_mut().enumerate() {
                                m.set([90.0, 160.0, 310.0][k] * r.range(0.8, 1.2) * (1.3 - size * 0.6), 0.05, rate);
                            }
                            self.groan = Some(Groan { t: 0.0, len: r.range(1.0, 4.0), phase: 0.0, rate0: r.range(15.0, 40.0), rate1: r.range(8.0, 30.0), modes, gain: 0.25 + 0.4 * size, pan: p });
                        } else {
                            let f = r.range(60.0, 200.0);
                            if let Some(b) = free(&mut self.booms) {
                                b.start(f, 0.8, 0.003, 0.4 + ring, 0.9 * size + 0.2, p, rate);
                            }
                        }
                    }
                    _ => {
                        let base = r.range(2500.0, 6000.0);
                        if let Some(m) = free(&mut self.rings) {
                            m.start(base, &[(1.0, 0.4, 1.0), (2.32, 0.2, 0.5), (4.25, 0.1, 0.3)], 0.3 + ring, r.range(0.05, 0.15), p, rate);
                        }
                    }
                }
            }
            let mut x = (0.0, 0.0);
            for z in &mut self.zings {
                let y = z.tick(rate);
                x.0 += y.0;
                x.1 += y.1;
            }
            let b = sum(&mut self.booms, &mut self.rng, rate);
            let m = sum_modal(&mut self.rings, &mut self.rng);
            let mut gr = (0.0, 0.0);
            if let Some(g) = &mut self.groan {
                let u = g.t / g.len;
                let pulse_rate = g.rate0 + (g.rate1 - g.rate0) * u;
                g.phase += pulse_rate * dt;
                let imp = if g.phase >= 1.0 {
                    g.phase -= 1.0;
                    1.0
                } else {
                    0.0
                };
                let env = (u * 6.0).min(1.0) * (1.0 - u);
                let mut y = 0.0;
                for md in &mut g.modes {
                    y += md.tick(imp * 0.5);
                }
                gr = pan(y * env * g.gain, g.pan);
                g.t += dt;
                if g.t >= g.len {
                    self.groan = None;
                }
            }
            out.add(i, (x.0 + b.0 + m.0 + gr.0, x.1 + b.1 + m.1 + gr.1));
        }
    }
}

pub struct Fire {
    rate: f32,
    rng: Rng,
    hiss: [ColorNoise; 2],
    hiss_hp: [Svf; 2],
    flick: [OnePole; 2],
    roar: [ColorNoise; 2],
    roar_lp: [Svf; 2],
    lap: [Drift; 2],
    cracks: [Burst; 24],
    ticker: Ticker,
}

impl Fire {
    pub fn new(rate: f32, seed: u32) -> Fire {
        Fire {
            rate,
            rng: Rng::new(seed),
            hiss: pair(Color::White, seed, rate),
            hiss_hp: [Svf::new(2500.0, 0.7, rate); 2],
            flick: [OnePole::new(12.0, rate); 2],
            roar: pair(Color::Brown, seed ^ 8, rate),
            roar_lp: [Svf::new(300.0, 0.8, rate); 2],
            lap: [Drift::new(seed), Drift::new(seed ^ 0xf1)],
            cracks: [Burst::default(); 24],
            ticker: Ticker::new(),
        }
    }
}

impl Gen for Fire {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [size, crackle, hiss, roar, ..] = *v;
        let rate = self.rate;
        let bt = BLOCK as f32 / rate;
        let mut lap = [0.0; 2];
        for c in 0..2 {
            // Flames lap: the roar's loudness and pitch wander a few times a second.
            lap[c] = 0.5 + 0.5 * self.lap[c].next(1.5 + size, bt);
            self.roar_lp[c].set((180.0 + 500.0 * lap[c]) * (1.3 - size * 0.6), 0.9, rate);
        }
        let dt = 1.0 / rate;
        let cr = 1.0 + 40.0 * crackle * (0.4 + size);
        for i in 0..BLOCK {
            if self.ticker.fire(&mut self.rng, cr, 0.0, dt) {
                let r = &mut self.rng;
                let big = r.chance(0.08);
                let (f, d, g) = if big { (r.range(300.0, 1200.0), r.range(0.01, 0.04), r.range(0.5, 1.0)) } else { (r.range(1000.0, 5000.0), r.range(0.002, 0.012), r.uni() * r.uni()) };
                let p = r.bi() * (0.3 + size * 0.6);
                if let Some(b) = free(&mut self.cracks) {
                    b.start(f, 1.1, 0.0001, d, g * (0.4 + crackle * 0.8), p, rate);
                }
            }
            let mut x = [0.0; 2];
            for c in 0..2 {
                // The hiss flickers: white noise gated by its own slowed, squared copy.
                let w = self.hiss[c].next();
                let f = self.flick[c].low(w.abs() * 3.0);
                let h = self.hiss_hp[c].high(w) * f * f * hiss * 1.5;
                let ro = self.roar_lp[c].low(self.roar[c].next() * 3.0) * lap[c] * roar * (0.4 + size);
                x[c] = h + ro;
            }
            let k = sum(&mut self.cracks, &mut self.rng, rate);
            out.add(i, ((x[0] + k.0) * (0.5 + size * 0.6), (x[1] + k.1) * (0.5 + size * 0.6)));
        }
    }
}

pub struct Rumble {
    rate: f32,
    n: [ColorNoise; 2],
    lp: [[Svf; 2]; 2],
    phase: f32,
    drift: Drift,
}

impl Rumble {
    pub fn new(rate: f32, seed: u32) -> Rumble {
        Rumble { rate, n: pair(Color::Brown, seed, rate), lp: [[Svf::new(60.0, 0.7, rate); 2]; 2], phase: 0.0, drift: Drift::new(seed) }
    }
}

impl Gen for Rumble {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [depth, throb, throb_rate, power, ..] = *v;
        let rate = self.rate;
        for c in 0..2 {
            for f in &mut self.lp[c] {
                f.set(depth, 0.7, rate);
            }
        }
        let d = self.drift.next(throb_rate * 0.3, BLOCK as f32 / rate);
        let dt = 1.0 / rate;
        for i in 0..BLOCK {
            self.phase = (self.phase + throb_rate * dt).fract();
            let t = 1.0 - throb * (0.5 + 0.35 * sin1(self.phase) + 0.15 * d);
            let mut x = [0.0; 2];
            for c in 0..2 {
                let n = self.n[c].next() * 4.0;
                let y0 = self.lp[c][0].low(n);
                let y = self.lp[c][1].low(y0);
                x[c] = y * t * power * 1.4;
            }
            out.add(i, (x[0], x[1]));
        }
    }
}

pub struct Steam {
    rate: f32,
    rng: Rng,
    n: [ColorNoise; 2],
    hp: [Svf; 2],
    burst: f32,
    burst_k: f32,
    bubbles: [Bubble; 8],
    lp: Lp2,
}

impl Steam {
    pub fn new(rate: f32, seed: u32) -> Steam {
        Steam { rate, rng: Rng::new(seed), n: pair(Color::White, seed, rate), hp: [Svf::new(2000.0, 0.7, rate); 2], burst: 0.0, burst_k: 0.0, bubbles: [Bubble::default(); 8], lp: Lp2::new(rate) }
    }
}

impl Gen for Steam {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [pressure, bursts, tone, gurgle, ..] = *v;
        let rate = self.rate;
        self.lp.set(tone_cut(tone), rate);
        for f in &mut self.hp {
            f.set(800.0 + 3000.0 * pressure, 0.6, rate);
        }
        let bt = BLOCK as f32 / rate;
        if self.rng.chance(bursts * bt) {
            self.burst = 1.0;
            self.burst_k = (-1.0 / (self.rng.range(0.4, 2.5) * rate)).exp();
        }
        let dt = 1.0 / rate;
        let mut s = Block::new();
        for i in 0..BLOCK {
            self.burst *= self.burst_k;
            if gurgle > 0.0 && self.rng.chance(gurgle * 8.0 * dt) {
                let (rad, g, p) = (self.rng.range(4.0, 12.0), self.rng.range(0.1, 0.3) * gurgle, self.rng.bi() * 0.4);
                if let Some(b) = free(&mut self.bubbles) {
                    b.start(rad, 0.5, g, p, rate);
                }
            }
            let level = pressure * 0.35 + self.burst * 0.9;
            let x = [self.hp[0].high(self.n[0].next()) * level, self.hp[1].high(self.n[1].next()) * level];
            let b = sum_bubbles(&mut self.bubbles, rate);
            s.add(i, (x[0] + b.0, x[1] + b.1));
        }
        add_filtered(&mut s, &mut self.lp, 1.0, out);
    }
}
