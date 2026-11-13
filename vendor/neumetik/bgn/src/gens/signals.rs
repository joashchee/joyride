// Copyright 2026 ansiapps. Neumetik BGN: proprietary, see the LICENSE at the repository root.

//! Electronic and tonal sounds and signals: beeps and consoles, energy
//! hums, data chatter, drones, shimmer, bells by their partials, ticks,
//! sirens, horns and distant battle.

use super::super::dsp::{db, free, pan, sin1, Block, Burst, Chirp, Drift, Live, Modal, Rng, Svf, Ticker, BLOCK};
use super::super::noise::{Color, ColorNoise};
use super::{add_filtered, far, pick, tone_cut, Gen, Lp2};

fn sum_chirps(voices: &mut [Chirp], rate: f32) -> (f32, f32) {
    let mut x = (0.0, 0.0);
    for c in voices.iter_mut() {
        let y = c.tick(rate);
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

fn sum_bursts(voices: &mut [Burst], rng: &mut Rng, rate: f32) -> (f32, f32) {
    let mut x = (0.0, 0.0);
    for b in voices.iter_mut() {
        let y = b.tick(rng, rate);
        x.0 += y.0;
        x.1 += y.1;
    }
    x
}

/// Notes queued to start later: (when, from Hz, to Hz, length, loudness, pan).
type Queue = Vec<(f32, f32, f32, f32, f32, f32)>;

fn run_queue(queue: &mut Queue, chirps: &mut [Chirp], dt: f32, rate: f32) {
    let mut k = 0;
    while k < queue.len() {
        queue[k].0 -= dt;
        if queue[k].0 <= 0.0 {
            let (_, from, to, len, g, p) = queue.swap_remove(k);
            if let Some(c) = free(chirps) {
                c.start(from, to, len, g, p, rate);
            }
        } else {
            k += 1;
        }
    }
}

pub struct Beeps {
    rate: f32,
    rng: Rng,
    chirps: Vec<Chirp>,
    queue: Queue,
    ticker: Ticker,
    bells: Vec<Modal>,
    echo: super::super::dsp::Delay,
}

impl Beeps {
    pub fn new(rate: f32, seed: u32) -> Beeps {
        Beeps { rate, rng: Rng::new(seed), chirps: vec![Chirp::default(); 16], queue: Vec::with_capacity(32), ticker: Ticker::new(), bells: vec![Modal::default(); 4], echo: super::super::dsp::Delay::new((rate * 0.9) as usize) }
    }
}

impl Gen for Beeps {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [kind, per_sec, pitch, variety, ..] = *v;
        let kind = pick(kind).min(6);
        let rate = self.rate;
        let dt = 1.0 / rate;
        let pf = 0.6 + pitch * 0.8;
        // A monitor, a crossing bell and a microwave keep time; the rest don't.
        let regular = match kind {
            0 | 5 => 0.98,
            2 => 0.9,
            _ => 0.0,
        };
        for i in 0..BLOCK {
            if self.ticker.fire(&mut self.rng, per_sec, regular, dt) {
                let r = &mut self.rng;
                let p = r.bi() * 0.4;
                match kind {
                    0 => self.queue.push((0.0, 960.0 * pf, 960.0 * pf, 0.12, 0.2, p)),
                    1 => {
                        let mut at = 0.0;
                        for _ in 0..(1 + r.below((1.0 + variety * 6.0) as usize)) {
                            let f = [880.0, 1175.0, 1320.0, 1568.0, 1760.0, 2093.0, 2637.0][r.below(7)] * pf;
                            let len = r.range(0.03, 0.12);
                            self.queue.push((at, f, f, len, 0.12, p));
                            at += len + r.range(0.01, 0.06);
                        }
                    }
                    2 => {
                        for k in 0..6 {
                            let f = if k % 2 == 0 { 1400.0 } else { 1050.0 } * pf;
                            self.queue.push((k as f32 * 0.25, f, f, 0.22, 0.18, p));
                        }
                    }
                    3 => self.queue.push((0.0, 1300.0 * pf, 1280.0 * pf, 0.9, 0.25, p)),
                    4 => {
                        self.queue.push((0.0, 400.0 * pf, 2200.0 * pf, 0.5, 0.1, p));
                        self.queue.push((0.55, 2200.0 * pf, 400.0 * pf, 0.5, 0.1, p));
                    }
                    5 => {
                        let base = 1250.0 * pf;
                        if let Some(m) = free(&mut self.bells) {
                            m.start(base, &[(1.0, 0.5, 1.0), (2.76, 0.2, 0.3)], 1.0, 0.2, p, rate);
                        }
                    }
                    _ => {
                        for k in 0..3 {
                            self.queue.push((k as f32 * 0.35, 2050.0 * pf, 2050.0 * pf, 0.18, 0.12, p));
                        }
                    }
                }
            }
            run_queue(&mut self.queue, &mut self.chirps, dt, rate);
            let mut x = sum_chirps(&mut self.chirps, rate);
            let b = sum_modal(&mut self.bells, &mut self.rng);
            x.0 += b.0;
            x.1 += b.1;
            // Sonar's ping comes back off the deep.
            if kind == 3 {
                let m = (x.0 + x.1) * 0.5;
                let e = self.echo.read(rate * 0.8) * 0.45;
                self.echo.push(m + e * 0.5);
                x.0 += e;
                x.1 += e;
            }
            out.add(i, x);
        }
    }
}

pub struct Energy {
    rate: f32,
    rng: Rng,
    phases: [f32; 6],
    pulse: f32,
    warble: f32,
    noise: [ColorNoise; 2],
    sweep: [Svf; 2],
    swirl: Drift,
    sparks: [Burst; 6],
}

impl Energy {
    pub fn new(rate: f32, seed: u32) -> Energy {
        Energy {
            rate,
            rng: Rng::new(seed),
            phases: [0.0; 6],
            pulse: 0.0,
            warble: 0.0,
            noise: [ColorNoise::new(Color::Pink, seed, rate), ColorNoise::new(Color::Pink, seed ^ 3, rate)],
            sweep: [Svf::new(800.0, 3.0, rate); 2],
            swirl: Drift::new(seed),
            sparks: [Burst::default(); 6],
        }
    }
}

impl Gen for Energy {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [kind, pitch, pulse, harmonics, warble, _] = *v;
        let kind = pick(kind).min(4);
        let rate = self.rate;
        let bt = BLOCK as f32 / rate;
        let s = self.swirl.next(0.15 + kind as f32 * 0.05, bt);
        for (c, f) in self.sweep.iter_mut().enumerate() {
            let center = pitch * [6.0, 16.0, 10.0, 4.0, 30.0][kind] * (1.0 + 0.5 * s * if c == 0 { 1.0 } else { -1.0 });
            f.set(center, 4.0, rate);
        }
        let pulse_rate = [1.5, 8.0, 0.4, 3.0, 12.0][kind];
        let ratios = [1.0, 2.0, 3.0, 1.005, 4.0, 0.5];
        let dt = 1.0 / rate;
        for i in 0..BLOCK {
            self.pulse = (self.pulse + pulse_rate * dt).fract();
            self.warble = (self.warble + 6.0 * dt).fract();
            let wb = 1.0 + warble * 0.02 * sin1(self.warble);
            let mut y = 0.0;
            for (k, ph) in self.phases.iter_mut().enumerate() {
                *ph = (*ph + pitch * ratios[k] * wb * dt).fract();
                let a = if k == 0 || k == 3 { 1.0 } else { harmonics * 0.5 / (k as f32) };
                y += if kind == 1 { (*ph * 2.0 - 1.0) * a * 0.6 } else { sin1(*ph) * a };
            }
            let pm = 1.0 - pulse * (0.5 + 0.5 * sin1(self.pulse));
            if (kind == 0 || kind == 4) && self.rng.chance(1.5 * dt) {
                let (f, p) = (self.rng.range(2000.0, 7000.0), self.rng.bi() * 0.6);
                if let Some(b) = free(&mut self.sparks) {
                    b.start(f, 1.0, 0.0002, 0.01, 0.15, p, rate);
                }
            }
            let sp = sum_bursts(&mut self.sparks, &mut self.rng, rate);
            let mut x = [0.0; 2];
            for c in 0..2 {
                let n = self.sweep[c].band(self.noise[c].next() * 2.0) * if kind == 2 { 0.5 } else { 0.2 };
                x[c] = (y * 0.12 + n) * pm;
            }
            out.add(i, (x[0] + sp.0, x[1] + sp.1));
        }
    }
}

pub struct Data {
    rate: f32,
    rng: Rng,
    phase: f32,
    freq: f32,
    target: f32,
    left: f32,
    step: f32,
    hold: f32,
    held: f32,
    ticker: Ticker,
    pan: f32,
}

impl Data {
    pub fn new(rate: f32, seed: u32) -> Data {
        Data { rate, rng: Rng::new(seed), phase: 0.0, freq: 1000.0, target: 1000.0, left: 0.0, step: 0.0, hold: 0.0, held: 0.0, ticker: Ticker::new(), pan: 0.0 }
    }
}

impl Gen for Data {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [kind, per_sec, pitch, crush, ..] = *v;
        let kind = pick(kind).min(4);
        let rate = self.rate;
        let dt = 1.0 / rate;
        let pf = 0.5 + pitch;
        // A sample-rate reduction: hold each value this many samples.
        let hold = 1.0 + crush * 24.0;
        let levels = 2f32.powf(8.0 - crush * 6.0);
        for i in 0..BLOCK {
            if self.left <= 0.0 && self.ticker.fire(&mut self.rng, per_sec, 0.0, dt) {
                self.left = match kind {
                    0 => self.rng.range(0.2, 1.5),
                    1 => self.rng.range(0.05, 0.3),
                    2 => self.rng.range(1.0, 3.0),
                    3 => self.rng.range(0.1, 0.6),
                    _ => self.rng.range(0.5, 2.0),
                };
                self.pan = self.rng.bi() * 0.5;
            }
            let mut y = 0.0;
            if self.left > 0.0 {
                self.left -= dt;
                self.step -= dt;
                if self.step <= 0.0 {
                    // Each kind's symbol rate and pitches.
                    let (rate_hz, lo, hi) = match kind {
                        0 => (60.0, 600.0, 3000.0),
                        1 => (25.0, 80.0, 4000.0),
                        2 => (30.0, 1070.0, 2225.0),
                        3 => (8.0, 300.0, 2500.0),
                        _ => (120.0, 2000.0, 6000.0),
                    };
                    self.step = 1.0 / rate_hz * self.rng.range(0.7, 1.3);
                    self.target = if kind == 2 { if self.rng.chance(0.5) { lo } else { hi } } else { self.rng.range(lo, hi) } * pf;
                    if kind != 3 {
                        self.freq = self.target;
                    }
                }
                // Bleeps glide; the rest jump.
                self.freq += (self.target - self.freq) * 30.0 * dt;
                self.phase = (self.phase + self.freq * dt).fract();
                y = match kind {
                    1 => {
                        if self.rng.chance(0.5) {
                            self.rng.bi() * 0.6
                        } else if self.phase < 0.5 {
                            0.5
                        } else {
                            -0.5
                        }
                    }
                    2 => sin1(self.phase) * 0.5 + self.rng.bi() * 0.08,
                    _ => {
                        if self.phase < 0.5 {
                            0.4
                        } else {
                            -0.4
                        }
                    }
                };
            }
            // Crushed: held and quantized.
            self.hold += 1.0;
            if self.hold >= hold {
                self.hold -= hold;
                self.held = (y * levels).round() / levels;
            }
            let p = pan(self.held * 0.35, self.pan);
            out.add(i, p);
        }
    }
}

pub struct Drone {
    rate: f32,
    oscs: Vec<(f32, f32, f32, f32)>,
    lp: [Svf; 2],
    motion: Drift,
    level: Vec<Drift>,
    grit: ColorNoise,
}

impl Drone {
    pub fn new(rate: f32, seed: u32) -> Drone {
        let mut rng = Rng::new(seed);
        // Up to four notes, three detuned saws each: phase, detune, pan, gain.
        let oscs = (0..12).map(|_| (rng.uni(), rng.bi(), rng.bi() * 0.8, rng.range(0.7, 1.0))).collect();
        Drone { rate, oscs, lp: [Svf::new(800.0, 0.8, rate); 2], motion: Drift::new(seed), level: (0..12).map(|k| Drift::new(seed + k * 11)).collect(), grit: ColorNoise::new(Color::White, seed, rate) }
    }
}

impl Gen for Drone {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [pitch, chord, detune, tone, motion, grit] = *v;
        let rate = self.rate;
        let bt = BLOCK as f32 / rate;
        let chord = super::CHORDS[pick(chord).min(6)];
        let m = self.motion.next(0.04, bt);
        for f in &mut self.lp {
            f.set((tone_cut(tone) * 0.4 * (1.0 + motion * 0.6 * m)).clamp(60.0, 12_000.0), 0.9, rate);
        }
        let mut gains = [0.0f32; 12];
        for (k, d) in self.level.iter_mut().enumerate() {
            gains[k] = 1.0 + motion * 0.5 * d.next(0.07, bt);
        }
        let notes = chord.len().min(4);
        let dt = 1.0 / rate;
        for i in 0..BLOCK {
            let mut x = (0.0, 0.0);
            for k in 0..notes * 3 {
                let (ph, det, p, g) = &mut self.oscs[k];
                let semis = chord[k / 3];
                let f = pitch * 2f32.powf((semis + *det * detune * 0.3) / 12.0);
                *ph = (*ph + f * dt).fract();
                let y = (*ph * 2.0 - 1.0) * *g * gains[k] * 0.08;
                let s = pan(y, *p);
                x.0 += s.0;
                x.1 += s.1;
            }
            let mut l = self.lp[0].low(x.0);
            let mut r = self.lp[1].low(x.1);
            if grit > 0.0 {
                let n = 1.0 + grit * self.grit.next();
                l = (l * (1.0 + grit * 3.0)).tanh() * n * 0.7;
                r = (r * (1.0 + grit * 3.0)).tanh() * n * 0.7;
            }
            out.add(i, (l, r));
        }
    }
}

pub struct Shimmer {
    rate: f32,
    rng: Rng,
    chirps: Vec<Chirp>,
    ticker: Ticker,
}

impl Shimmer {
    pub fn new(rate: f32, seed: u32) -> Shimmer {
        Shimmer { rate, rng: Rng::new(seed), chirps: vec![Chirp::default(); 24], ticker: Ticker::new() }
    }
}

impl Gen for Shimmer {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [density, pitch, glide, spread, ..] = *v;
        let rate = self.rate;
        let dt = 1.0 / rate;
        // A pentatonic scale, so sparkles never clash.
        const SCALE: [f32; 5] = [0.0, 2.0, 4.0, 7.0, 9.0];
        for i in 0..BLOCK {
            if self.ticker.fire(&mut self.rng, density, 0.0, dt) {
                let r = &mut self.rng;
                let semis = SCALE[r.below(5)] + 12.0 * r.below(3) as f32;
                let f = 1000.0 * (0.6 + pitch * 1.6) * 2f32.powf(semis / 12.0);
                let to = f * 2f32.powf(r.bi() * glide);
                let (len, g, p) = (r.range(0.08, 0.6), r.range(0.03, 0.12), r.bi() * spread);
                if let Some(c) = free(&mut self.chirps) {
                    c.start(f, to, len, g, p, rate);
                    c.vibrato(0.01, r.range(6.0, 14.0));
                }
            }
            out.add(i, sum_chirps(&mut self.chirps, rate));
        }
    }
}

pub struct Bells {
    rate: f32,
    rng: Rng,
    modes: Vec<Modal>,
    ticker: Ticker,
    queue: Vec<(f32, f32, f32, f32)>,
    gust: Drift,
    lp: Lp2,
}

impl Bells {
    pub fn new(rate: f32, seed: u32) -> Bells {
        let mut rng = Rng::new(seed);
        let ticker = Ticker::primed(&mut rng, 0.5);
        Bells { rate, rng, modes: vec![Modal::default(); 16], ticker, queue: Vec::with_capacity(32), gust: Drift::new(seed), lp: Lp2::new(rate) }
    }
}

/// A bell's partials (ratio, decay seconds, loudness), base pitch range and overall decay.
type Bell = (&'static [(f32, f32, f32)], (f32, f32), f32);

/// Each bell kind's partials, pitch range and decay.
fn bell(kind: usize) -> Bell {
    match kind {
        // A church bell: hum, prime, tierce, quint, nominal and above.
        0 => (&[(0.5, 1.0, 0.6), (1.0, 0.8, 1.0), (1.19, 0.6, 0.7), (1.5, 0.5, 0.4), (2.0, 0.45, 0.8), (2.5, 0.3, 0.3), (3.0, 0.25, 0.3), (4.0, 0.15, 0.2)], (180.0, 420.0), 6.0),
        // Tubes: free bars' partials.
        1 => (&[(1.0, 0.8, 1.0), (2.76, 0.5, 0.6), (5.40, 0.3, 0.3), (8.93, 0.15, 0.15)], (700.0, 2000.0), 2.5),
        2 => (&[(1.0, 0.15, 1.0), (2.4, 0.08, 0.5), (3.9, 0.05, 0.3)], (400.0, 900.0), 1.0),
        // A singing bowl, two close modes beating.
        3 => (&[(1.0, 1.0, 1.0), (1.004, 1.0, 0.8), (2.71, 0.6, 0.5), (5.15, 0.3, 0.2)], (200.0, 500.0), 10.0),
        4 => (&[(1.0, 0.8, 1.0), (2.0, 0.5, 0.5), (2.6, 0.4, 0.4), (3.3, 0.3, 0.3)], (500.0, 800.0), 3.0),
        5 => (&[(1.0, 0.8, 1.0), (2.76, 0.4, 0.4), (5.4, 0.2, 0.2)], (300.0, 600.0), 3.0),
        6 => (&[(1.0, 0.1, 1.0), (1.47, 0.08, 0.7), (2.8, 0.05, 0.4)], (3000.0, 6000.0), 1.0),
        7 => (&[(1.0, 0.3, 1.0), (1.5, 0.25, 0.7), (2.3, 0.15, 0.4)], (550.0, 900.0), 1.0),
        8 => (&[(1.0, 0.8, 1.0), (2.0, 0.6, 0.6), (3.0, 0.4, 0.3), (4.2, 0.3, 0.2)], (700.0, 1100.0), 2.0),
        _ => (&[(1.0, 1.0, 1.0), (1.53, 0.9, 0.8), (2.0, 0.8, 0.6), (2.43, 0.7, 0.5), (2.94, 0.6, 0.4), (3.52, 0.5, 0.3), (4.1, 0.4, 0.2)], (70.0, 140.0), 8.0),
    }
}

impl Gen for Bells {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [kind, per_sec, pitch, variety, distance, _] = *v;
        let kind = pick(kind).min(9);
        let rate = self.rate;
        let (g, cut) = far(distance);
        self.lp.set(cut, rate);
        let bt = BLOCK as f32 / rate;
        let gust = 0.5 + 0.5 * self.gust.next(0.2, bt);
        let (partials, range, decay) = bell(kind);
        let base = range.0 + (range.1 - range.0) * pitch;
        // Wind chimes are struck as the wind gusts.
        let per_sec = if kind <= 2 { per_sec * gust * gust * 2.0 } else { per_sec };
        let regular = if kind == 0 || kind == 4 || kind == 5 { 0.9 } else { 0.0 };
        let dt = 1.0 / rate;
        // A chime's notes, in the pentatonic or as Westminster's quarters.
        const PENTA: [f32; 5] = [0.0, 2.0, 4.0, 7.0, 9.0];
        let mut s = Block::new();
        for i in 0..BLOCK {
            if self.ticker.fire(&mut self.rng, per_sec, regular, dt) {
                let r = &mut self.rng;
                let p = r.bi() * 0.7;
                match kind {
                    // A ship's bell rings in pairs.
                    4 => {
                        for k in 0..(1 + r.below(4)) {
                            self.queue.push((k as f32 * 1.2, base, 0.5, p));
                            self.queue.push((k as f32 * 1.2 + 0.35, base, 0.5, p));
                        }
                    }
                    5 => {
                        for (k, semis) in [4.0, 2.0, 0.0, -5.0].iter().enumerate() {
                            self.queue.push((k as f32 * 0.7, base * 2f32.powf(semis / 12.0), 0.4, p));
                        }
                    }
                    // Sleigh bells jingle in a trot.
                    6 => {
                        for k in 0..8 {
                            self.queue.push((k as f32 * 0.18 + r.range(0.0, 0.03), base * r.range(0.9, 1.1), 0.2, p));
                        }
                    }
                    _ => {
                        let note = if kind <= 2 { PENTA[r.below(5)] + 12.0 * r.below(2) as f32 } else { 0.0 };
                        let f = base * 2f32.powf(note / 12.0) * (1.0 + variety * r.range(-0.03, 0.03));
                        self.queue.push((0.0, f, r.range(0.4, 0.8), p));
                    }
                }
            }
            let mut k = 0;
            while k < self.queue.len() {
                self.queue[k].0 -= dt;
                if self.queue[k].0 <= 0.0 {
                    let (_, f, gain, p) = self.queue.swap_remove(k);
                    if let Some(m) = free(&mut self.modes) {
                        m.start(f, partials, decay, gain * 0.25, p, rate);
                    }
                } else {
                    k += 1;
                }
            }
            s.add(i, sum_modal(&mut self.modes, &mut self.rng));
        }
        add_filtered(&mut s, &mut self.lp, g, out);
    }
}

pub struct Ticks {
    rate: f32,
    rng: Rng,
    modes: Vec<Modal>,
    clicks: Vec<Burst>,
    ticker: Ticker,
    tock: bool,
    lp: Lp2,
}

impl Ticks {
    pub fn new(rate: f32, seed: u32) -> Ticks {
        Ticks { rate, rng: Rng::new(seed), modes: vec![Modal::default(); 6], clicks: vec![Burst::default(); 24], ticker: Ticker::new(), tock: false, lp: Lp2::new(rate) }
    }
}

impl Gen for Ticks {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [kind, per_sec, pitch, tone, ..] = *v;
        let kind = pick(kind).min(5);
        let rate = self.rate;
        self.lp.set(tone_cut(tone), rate);
        let dt = 1.0 / rate;
        let regular = if kind == 2 { 0.0 } else if kind == 3 { 0.3 } else { 1.0 };
        let pf = 0.6 + pitch * 0.8;
        let mut s = Block::new();
        for i in 0..BLOCK {
            if self.ticker.fire(&mut self.rng, per_sec, regular, dt) {
                self.tock = !self.tock;
                let r = &mut self.rng;
                let t = if self.tock { 1.0 } else { 0.85 };
                match kind {
                    0 | 1 | 4 => {
                        let (base, decay, g) = match kind {
                            0 => (2600.0, 0.02, 0.4),
                            1 => (900.0, 0.08, 0.6),
                            _ => (1400.0, 0.03, 0.6),
                        };
                        if let Some(m) = free(&mut self.modes) {
                            m.start(base * pf * t, &[(1.0, decay, 1.0), (2.3, decay * 0.6, 0.5), (4.1, decay * 0.4, 0.3)], 1.0, g, 0.0, rate);
                        }
                    }
                    2 => {
                        let f = r.range(3000.0, 6000.0);
                        if let Some(b) = free(&mut self.clicks) {
                            b.start(f, 0.8, 0.0001, 0.0015, 0.8, r.bi() * 0.1, rate);
                        }
                    }
                    3 => {
                        for _ in 0..(1 + r.below(3)) {
                            let f = r.range(800.0, 2500.0) * pf;
                            if let Some(b) = free(&mut self.clicks) {
                                b.start(f, 2.0, 0.0002, 0.008, 0.5, r.bi() * 0.5, rate);
                            }
                        }
                    }
                    _ => {
                        let f = if self.tock { 1800.0 } else { 1300.0 } * pf;
                        if let Some(b) = free(&mut self.clicks) {
                            b.start(f, 2.5, 0.0002, 0.012, 0.6, 0.0, rate);
                        }
                    }
                }
            }
            let a = sum_modal(&mut self.modes, &mut self.rng);
            let b = sum_bursts(&mut self.clicks, &mut self.rng, rate);
            s.add(i, (a.0 + b.0, a.1 + b.1));
        }
        add_filtered(&mut s, &mut self.lp, 1.0, out);
    }
}

/// A sound that comes and goes: a siren's run or a horn's blast.
#[derive(Clone, Copy, Default)]
struct Episode {
    t: f32,
    len: f32,
    phase: f32,
    lfo: f32,
    pan: f32,
    drift: f32,
    gain: f32,
}

impl Live for Episode {
    fn live(&self) -> bool {
        self.t < self.len
    }
}

pub struct Sirens {
    rate: f32,
    rng: Rng,
    runs: [Episode; 2],
    ticker: Ticker,
    lp: Lp2,
}

impl Sirens {
    pub fn new(rate: f32, seed: u32) -> Sirens {
        let mut rng = Rng::new(seed);
        let ticker = Ticker::primed(&mut rng, 0.1);
        Sirens { rate, rng, runs: [Episode::default(); 2], ticker, lp: Lp2::new(rate) }
    }
}

impl Gen for Sirens {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [kind, per_sec, distance, pitch, ..] = *v;
        let kind = pick(kind).min(3);
        let rate = self.rate;
        let (g, cut) = far(distance);
        self.lp.set(cut, rate);
        let bt = BLOCK as f32 / rate;
        if self.ticker.fire(&mut self.rng, per_sec, 0.0, bt) {
            if let Some(e) = free(&mut self.runs) {
                e.t = 0.0;
                e.len = if kind == 3 { self.rng.range(40.0, 70.0) } else { self.rng.range(8.0, 30.0) };
                e.lfo = self.rng.uni();
                e.pan = self.rng.bi() * 0.8;
                e.drift = self.rng.bi() * 0.04;
                e.gain = self.rng.range(0.6, 1.0);
            }
        }
        let pf = 0.8 + pitch * 0.4;
        let dt = 1.0 / rate;
        let mut s = Block::new();
        for i in 0..BLOCK {
            let mut x = (0.0, 0.0);
            for e in self.runs.iter_mut().filter(|e| e.live()) {
                // Each kind's sweep: wail slowly, yelp quickly, two-tone steps, air raid's slow rise.
                let lfo_rate = [0.2, 3.5, 1.0, 1.0 / 30.0][kind];
                e.lfo = (e.lfo + lfo_rate * dt).fract();
                let u = e.lfo;
                let freq = match kind {
                    0 | 1 => 650.0 + 700.0 * (0.5 - 0.5 * (u * std::f32::consts::TAU).cos()),
                    2 => {
                        if u < 0.5 {
                            440.0
                        } else {
                            585.0
                        }
                    }
                    _ => 180.0 + 420.0 * (1.0 - (2.0 * u - 1.0).abs()).powf(0.6),
                } * pf;
                e.phase = (e.phase + freq * dt).fract();
                let y = sin1(e.phase) * 0.6 + sin1(e.phase * 2.0) * 0.2 + sin1(e.phase * 3.0) * 0.12;
                let fade = ((e.t / 2.0).min(1.0)) * (((e.len - e.t) / 3.0).clamp(0.0, 1.0));
                e.pan = (e.pan + e.drift * dt).clamp(-1.0, 1.0);
                let p = pan(y * fade * e.gain * 0.2, e.pan);
                x.0 += p.0;
                x.1 += p.1;
                e.t += dt;
            }
            s.add(i, x);
        }
        add_filtered(&mut s, &mut self.lp, g, out);
    }
}

pub struct Horns {
    rate: f32,
    rng: Rng,
    blasts: Vec<(f32, f32, [f32; 5], f32, f32)>,
    queue: Vec<(f32, f32)>,
    ticker: Ticker,
    bell: Vec<Modal>,
    pan: f32,
    lp: Lp2,
}

impl Horns {
    pub fn new(rate: f32, seed: u32) -> Horns {
        let mut rng = Rng::new(seed);
        let ticker = Ticker::primed(&mut rng, 0.1);
        Horns { rate, rng, blasts: Vec::with_capacity(8), queue: Vec::with_capacity(16), ticker, bell: vec![Modal::default(); 4], pan: 0.0, lp: Lp2::new(rate) }
    }
}

impl Gen for Horns {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [kind, per_sec, distance, pitch, ..] = *v;
        let kind = pick(kind).min(5);
        let rate = self.rate;
        let (g, cut) = far(distance);
        self.lp.set(cut, rate);
        let bt = BLOCK as f32 / rate;
        let pf = 0.8 + pitch * 0.4;
        if self.ticker.fire(&mut self.rng, per_sec, 0.0, bt) {
            let r = &mut self.rng;
            self.pan = r.bi() * 0.8;
            // Each kind's pattern of blasts: (when, how long).
            let pattern: Vec<(f32, f32)> = match kind {
                0 => (0..(1 + r.below(3))).map(|k| (k as f32 * 0.45, r.range(0.15, 0.5))).collect(),
                1 => vec![(0.0, r.range(0.8, 2.0))],
                2 => vec![(0.0, r.range(2.0, 5.0))],
                3 => vec![(0.0, 2.5)],
                // A train at a crossing: long, long, short, long.
                4 => vec![(0.0, 1.6), (2.0, 1.6), (4.0, 0.6), (5.0, 2.2)],
                _ => vec![(0.0, 0.1), (0.25, 0.1)],
            };
            self.queue.extend(pattern);
        }
        let dt = 1.0 / rate;
        // Each kind's chord (Hz) before the pitch control.
        let chord: [f32; 5] = match kind {
            0 => [410.0, 500.0, 0.0, 0.0, 0.0],
            1 => [150.0, 190.0, 240.0, 0.0, 0.0],
            2 => [80.0, 120.0, 0.0, 0.0, 0.0],
            3 => [90.0, 0.0, 0.0, 0.0, 0.0],
            4 => [311.0, 370.0, 440.0, 523.0, 622.0],
            _ => [0.0; 5],
        };
        let mut s = Block::new();
        for i in 0..BLOCK {
            let mut k = 0;
            while k < self.queue.len() {
                self.queue[k].0 -= dt;
                if self.queue[k].0 <= 0.0 {
                    let (_, len) = self.queue.swap_remove(k);
                    if kind == 5 {
                        let (p, base) = (self.pan, 2300.0 * pf);
                        if let Some(m) = free(&mut self.bell) {
                            m.start(base, &[(1.0, 0.6, 1.0), (2.1, 0.3, 0.4)], 1.0, 0.3, p, rate);
                        }
                    } else {
                        self.blasts.push((0.0, len, [0.0; 5], 0.0, self.pan));
                    }
                } else {
                    k += 1;
                }
            }
            let mut x = sum_modal(&mut self.bell, &mut self.rng);
            let mut b = 0;
            while b < self.blasts.len() {
                let (t, len, phases, _, p) = &mut self.blasts[b];
                let attack = (*t / 0.03).min(1.0);
                let release = ((*len - *t) / 0.08).clamp(0.0, 1.0);
                // A foghorn's grunt at the end of its blast.
                let fall = if kind == 3 && *t > *len * 0.7 { 0.75 } else { 1.0 };
                let mut y = 0.0;
                for (j, f) in chord.iter().enumerate() {
                    if *f > 0.0 {
                        phases[j] = (phases[j] + f * pf * fall * dt).fract();
                        // A reedy square-ish wave.
                        y += (sin1(phases[j]) * 3.0).tanh() * 0.25;
                    }
                }
                let pp = pan(y * attack * release * 0.35, *p);
                x.0 += pp.0;
                x.1 += pp.1;
                *t += dt;
                if *t >= *len {
                    self.blasts.swap_remove(b);
                } else {
                    b += 1;
                }
            }
            s.add(i, x);
        }
        add_filtered(&mut s, &mut self.lp, g, out);
    }
}

pub struct Battle {
    rate: f32,
    rng: Rng,
    cracks: Vec<Burst>,
    booms: Vec<Burst>,
    shots: Ticker,
    shells: Ticker,
    burst_left: u32,
    burst_wait: f32,
    burst_pan: f32,
    lp: Lp2,
}

impl Battle {
    pub fn new(rate: f32, seed: u32) -> Battle {
        let mut rng = Rng::new(seed);
        let shells = Ticker::primed(&mut rng, 0.2);
        Battle { rate, rng, cracks: vec![Burst::default(); 24], booms: vec![Burst::default(); 8], shots: Ticker::new(), shells, burst_left: 0, burst_wait: 0.0, burst_pan: 0.0, lp: Lp2::new(rate) }
    }

    fn shot(&mut self, p: f32, near: f32) {
        let rate = self.rate;
        let r = &mut self.rng;
        let f = r.range(1500.0, 4000.0);
        if let Some(b) = free(&mut self.cracks) {
            b.start(f, 0.7, 0.0002, 0.01 + 0.03 * (1.0 - near), r.range(0.3, 0.8) * near, p, rate);
        }
        let f = r.range(120.0, 300.0);
        if let Some(b) = free(&mut self.booms) {
            b.start(f, 0.8, 0.002, 0.25, r.range(0.3, 0.6), p, rate);
        }
    }
}

impl Gen for Battle {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [gunfire, artillery, distance, intensity, ..] = *v;
        let rate = self.rate;
        let (g, cut) = far(distance);
        self.lp.set(cut, rate);
        let near = 1.0 - distance;
        let dt = 1.0 / rate;
        let mut s = Block::new();
        for i in 0..BLOCK {
            if gunfire > 0.0 && self.shots.fire(&mut self.rng, gunfire * (0.3 + intensity * 2.0), 0.0, dt) {
                let p = self.rng.bi() * 0.9;
                // A machine gun's burst, or a single shot.
                if self.rng.chance(0.3 + intensity * 0.3) {
                    self.burst_left = 4 + self.rng.below(16) as u32;
                    self.burst_pan = p;
                } else {
                    self.shot(p, near);
                }
            }
            if self.burst_left > 0 {
                self.burst_wait -= dt;
                if self.burst_wait <= 0.0 {
                    self.burst_left -= 1;
                    self.burst_wait = self.rng.range(0.08, 0.11);
                    let p = self.burst_pan;
                    self.shot(p, near);
                }
            }
            if artillery > 0.0 && self.shells.fire(&mut self.rng, artillery * (0.05 + intensity * 0.4), 0.0, dt) {
                let r = &mut self.rng;
                let (f, d, gain, p) = (r.range(35.0, 80.0), r.range(1.5, 3.5), r.range(0.8, 1.5) * db(-6.0 * distance), r.bi() * 0.8);
                if let Some(b) = free(&mut self.booms) {
                    b.start(f, 0.7, 0.005, d, gain, p, rate);
                }
            }
            let a = sum_bursts(&mut self.cracks, &mut self.rng, rate);
            let b = sum_bursts(&mut self.booms, &mut self.rng, rate);
            s.add(i, (a.0 + b.0 * 1.5, a.1 + b.1 * 1.5));
        }
        add_filtered(&mut s, &mut self.lp, g, out);
    }
}
