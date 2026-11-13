// Copyright 2026 ansiapps. Neumetik BGN: proprietary, see the LICENSE at the repository root.

//! People and what they do: walla from formant voices that talk in turns,
//! applause from a Poisson stream of claps (Perry Cook's PhISEM idea),
//! steps, dishes and keys from noise bursts and modal resonators, and
//! music from somewhere else from a little sequencer.

use super::super::dsp::{db, free, pan, sin1, Block, Burst, Drift, Env, Live, Modal, OnePole, Rng, Svf, Ticker, Voice, BLOCK, VOWELS};
use super::super::noise::{Color, ColorNoise};
use super::{add_filtered, far, pick, Gen, Lp2, CHORDS};

fn sum_bursts(voices: &mut [Burst], rng: &mut Rng, rate: f32) -> (f32, f32) {
    let mut x = (0.0, 0.0);
    for b in voices.iter_mut() {
        let y = b.tick(rng, rate);
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

/// One person in a crowd: talking in turns, a syllable at a time.
#[derive(Clone, Copy, Default)]
struct Talker {
    voice: Voice,
    talking: bool,
    turn: f32,
    syllable: f32,
    base: f32,
}

const MAX_TALKERS: usize = 24;

pub struct Crowd {
    rate: f32,
    rng: Rng,
    talkers: [Talker; MAX_TALKERS],
    roar: [ColorNoise; 2],
    roar_bp: [[Svf; 2]; 2],
    roar_drift: Drift,
    lp: Lp2,
}

impl Crowd {
    pub fn new(rate: f32, seed: u32) -> Crowd {
        let mut rng = Rng::new(seed);
        let mut talkers = [Talker::default(); MAX_TALKERS];
        for t in &mut talkers {
            t.turn = rng.range(0.0, 3.0);
            t.voice.pan = rng.bi() * 0.9;
            t.voice.glide = 25.0;
            t.base = rng.uni();
        }
        Crowd {
            rate,
            rng,
            talkers,
            roar: [ColorNoise::new(Color::Pink, seed, rate), ColorNoise::new(Color::Pink, seed ^ 9, rate)],
            roar_bp: [[Svf::new(500.0, 1.2, rate), Svf::new(1400.0, 1.5, rate)]; 2],
            roar_drift: Drift::new(seed),
            lp: Lp2::new(rate),
        }
    }
}

impl Gen for Crowd {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [people, excite, pitch, distance, children, _] = *v;
        let rate = self.rate;
        let (g, cut) = far(distance);
        self.lp.set(cut, rate);
        let n = (people.round() as usize).clamp(1, MAX_TALKERS);
        let bt = BLOCK as f32 / rate;
        for (k, t) in self.talkers.iter_mut().enumerate() {
            if k >= n {
                t.voice.on = false;
                continue;
            }
            // Turns: talk a while, listen a while; excitement talks more.
            t.turn -= bt;
            if t.turn <= 0.0 {
                t.talking = !t.talking;
                t.turn = if t.talking { self.rng.range(0.8, 4.0) * (1.0 + excite) } else { self.rng.range(0.4, 3.0) * (1.0 - 0.7 * excite) };
            }
            t.syllable -= bt;
            if t.syllable <= 0.0 {
                t.syllable = self.rng.range(0.07, 0.22) * (1.0 - 0.35 * excite);
                let child = t.base < children;
                let female = (t.base * 7.3).fract() < 0.5;
                let base = if child { 290.0 } else if female { 205.0 } else { 115.0 };
                t.voice.target_pitch = base * (0.8 + pitch * 0.4) * (1.0 + excite * 0.35) * self.rng.range(0.85, 1.2);
                let vowel = VOWELS[self.rng.below(6)];
                let scale = if child { 1.25 } else if female { 1.12 } else { 1.0 };
                t.voice.target_vowel = vowel.map(|f| f * scale * self.rng.range(0.92, 1.08));
                // Gaps between some syllables.
                t.voice.on = t.talking && self.rng.chance(0.82);
                t.voice.breath = 0.12 + 0.1 * self.rng.uni();
                t.voice.gain = (0.5 + 0.5 * excite) * if child { 0.6 } else { 1.0 };
                if t.voice.pitch == 0.0 {
                    t.voice.pitch = t.voice.target_pitch;
                }
            }
        }
        // Past two dozen, the rest of the crowd is a roar.
        let roar = if people > MAX_TALKERS as f32 { (people / MAX_TALKERS as f32).log2() * 0.12 * (0.6 + excite) } else { 0.0 };
        let rd = 1.0 + 0.2 * self.roar_drift.next(0.3, bt);
        let norm = 0.35 / (n as f32).sqrt();
        let mut s = Block::new();
        for i in 0..BLOCK {
            let mut x = (0.0, 0.0);
            for t in self.talkers.iter_mut().take(n) {
                let y = t.voice.tick(&mut self.rng, rate) * norm;
                let p = pan(y, t.voice.pan);
                x.0 += p.0;
                x.1 += p.1;
            }
            if roar > 0.0 {
                for c in 0..2 {
                    let w = self.roar[c].next() * 2.0;
                    let y = (self.roar_bp[c][0].band(w) + 0.5 * self.roar_bp[c][1].band(w)) * roar * rd;
                    if c == 0 {
                        x.0 += y;
                    } else {
                        x.1 += y;
                    }
                }
            }
            s.add(i, x);
        }
        add_filtered(&mut s, &mut self.lp, g, out);
    }
}

pub struct Applause {
    rate: f32,
    rng: Rng,
    claps: [Burst; 48],
    ticker: Ticker,
    voices: [Talker; 12],
    swell: Drift,
    beat: f32,
}

impl Applause {
    pub fn new(rate: f32, seed: u32) -> Applause {
        let mut rng = Rng::new(seed);
        let mut voices = [Talker::default(); 12];
        for t in &mut voices {
            t.voice.pan = rng.bi() * 0.9;
            t.voice.glide = 8.0;
            t.base = rng.uni();
        }
        Applause { rate, rng, claps: [Burst::default(); 48], ticker: Ticker::new(), voices, swell: Drift::new(seed), beat: 0.0 }
    }
}

impl Gen for Applause {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [kind, amount, intensity, swell, ..] = *v;
        let kind = pick(kind).min(4);
        let rate = self.rate;
        let bt = BLOCK as f32 / rate;
        let s = (0.5 + 0.5 * self.swell.next(0.07, bt)).powf(0.8);
        let level = (1.0 - swell) + swell * s * 1.3;
        let clappers = if kind == 0 { 5.0 + amount * 120.0 } else { amount * 10.0 };
        let claps = clappers * (2.0 + intensity * 3.0) * level;
        // Voices: cheers on "ah", chants on the beat, boos on "oo", laughs in bursts.
        self.beat = (self.beat + bt * 1.6).fract();
        let on_beat = self.beat < 0.45;
        for t in &mut self.voices {
            t.syllable -= bt;
            if t.syllable > 0.0 {
                continue;
            }
            let female = t.base < 0.5;
            let base = if female { 260.0 } else { 150.0 };
            match kind {
                1 => {
                    t.syllable = self.rng.range(0.3, 1.2);
                    t.voice.on = self.rng.chance(level * amount + 0.1);
                    t.voice.target_pitch = base * (1.3 + intensity * 0.6) * self.rng.range(0.9, 1.2);
                    t.voice.target_vowel = VOWELS[if self.rng.chance(0.7) { 0 } else { 3 }];
                    t.voice.breath = 0.35;
                }
                2 => {
                    t.syllable = 0.05;
                    t.voice.on = on_beat;
                    t.voice.target_pitch = base * 1.2;
                    t.voice.target_vowel = VOWELS[if self.beat < 0.2 { 3 } else { 0 }];
                    t.voice.breath = 0.25;
                }
                3 => {
                    t.syllable = self.rng.range(0.6, 2.0);
                    t.voice.on = self.rng.chance(0.8 * amount + 0.1);
                    t.voice.target_pitch = base * 0.85 * self.rng.range(0.9, 1.1);
                    t.voice.target_vowel = VOWELS[4];
                    t.voice.breath = 0.3;
                }
                4 => {
                    // "Ha": a fast pulse, falling in pitch through the laugh.
                    t.syllable = self.rng.range(0.1, 0.16);
                    t.voice.on = !t.voice.on && self.rng.chance(amount * 0.9 + 0.1);
                    t.voice.target_pitch = base * 1.3 * self.rng.range(0.85, 1.15);
                    t.voice.target_vowel = VOWELS[0];
                    t.voice.breath = 0.5;
                }
                _ => {
                    t.syllable = 0.5;
                    t.voice.on = false;
                }
            }
            t.voice.gain = 0.3 * level * (0.5 + intensity * 0.6);
            if t.voice.pitch == 0.0 {
                t.voice.pitch = t.voice.target_pitch;
            }
        }
        let dt = 1.0 / rate;
        for i in 0..BLOCK {
            while claps > 0.5 && self.ticker.fire(&mut self.rng, claps, 0.0, dt) {
                let r = &mut self.rng;
                let (f, g, p) = (r.range(700.0, 2600.0), r.range(0.1, 0.4) * (0.5 + intensity * 0.6), r.bi() * 0.95);
                if let Some(b) = free(&mut self.claps) {
                    b.start(f, 1.4, 0.0005, r.range(0.006, 0.014), g, p, rate);
                }
            }
            let mut x = sum_bursts(&mut self.claps, &mut self.rng, rate);
            for t in &mut self.voices {
                let y = t.voice.tick(&mut self.rng, rate);
                let p = pan(y, t.voice.pan);
                x.0 += p.0;
                x.1 += p.1;
            }
            out.add(i, x);
        }
    }
}

/// Someone walking by.
#[derive(Clone, Copy, Default)]
struct Walker {
    phase: f32,
    pace: f32,
    pan: f32,
    drift: f32,
    near: f32,
}

pub struct Footsteps {
    rate: f32,
    rng: Rng,
    walkers: Vec<Walker>,
    bursts: Vec<Burst>,
    modes: Vec<Modal>,
    crunch: Vec<(f32, u32, f32)>,
    lp: Lp2,
}

impl Footsteps {
    pub fn new(rate: f32, seed: u32) -> Footsteps {
        let mut rng = Rng::new(seed);
        let walkers = (0..40).map(|_| Walker { phase: rng.uni(), pace: rng.range(0.85, 1.15), pan: rng.bi(), drift: rng.bi() * 0.15, near: rng.range(0.2, 1.0) }).collect();
        Footsteps { rate, rng, walkers, bursts: vec![Burst::default(); 48], modes: vec![Modal::default(); 10], crunch: Vec::with_capacity(40), lp: Lp2::new(rate) }
    }

    fn step(&mut self, surface: usize, gain: f32, p: f32) {
        let rate = self.rate;
        let r = &mut self.rng;
        match surface {
            // Gravel and snow crunch: a cluster of tiny collisions.
            2 | 3 => self.crunch.push((0.0, 8 + r.below(18) as u32, p)),
            1 => {
                let base = r.range(180.0, 320.0);
                if let Some(m) = free(&mut self.modes) {
                    m.start(base, &[(1.0, 0.05, 1.0), (2.3, 0.03, 0.5), (3.9, 0.02, 0.3)], 1.0, gain * 0.5, p, rate);
                }
            }
            5 => {
                let base = r.range(250.0, 500.0);
                if let Some(m) = free(&mut self.modes) {
                    m.start(base, &[(1.0, 0.25, 1.0), (2.76, 0.15, 0.6), (5.4, 0.08, 0.4)], 1.0, gain * 0.3, p, rate);
                }
            }
            _ => {}
        }
        // The heel strike and the body's thump, softer on carpet and mud.
        let (f, d, g) = match surface {
            0 => (r.range(1500.0, 3200.0), 0.015, 0.6),
            4 => (r.range(200.0, 400.0), 0.03, 0.35),
            6 => (r.range(300.0, 700.0), 0.05, 0.5),
            _ => (r.range(800.0, 2000.0), 0.012, 0.3),
        };
        if let Some(b) = free(&mut self.bursts) {
            b.start(f, 1.6, 0.001, d, gain * g, p, rate);
        }
        let f = r.range(90.0, 160.0);
        if let Some(b) = free(&mut self.bursts) {
            b.start(f, 0.8, 0.002, 0.04, gain * 0.5, p, rate);
        }
    }
}

impl Gen for Footsteps {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [surface, people, pace, distance, ..] = *v;
        let surface = pick(surface).min(6);
        let rate = self.rate;
        let (g, cut) = far(distance);
        self.lp.set(cut, rate);
        let n = (people.round() as usize).clamp(1, 40);
        let steps = 1.4 + pace * 1.4;
        let bt = BLOCK as f32 / rate;
        let mut due = Vec::new();
        for (k, w) in self.walkers.iter_mut().take(n).enumerate() {
            w.phase += steps * w.pace * bt;
            // Each walks across and away, then someone else comes.
            w.pan += w.drift * bt;
            if w.pan.abs() > 1.0 {
                w.pan = -w.pan.signum() * 0.95;
            }
            if w.phase >= 1.0 {
                w.phase -= 1.0;
                due.push((k, w.near * (1.0 - w.pan.abs() * 0.5), w.pan));
            }
        }
        for (_, gain, p) in due {
            let g = gain * self.rng.range(0.7, 1.0);
            self.step(surface, g, p);
        }
        let dt = 1.0 / rate;
        let mut s = Block::new();
        for i in 0..BLOCK {
            let mut k = 0;
            while k < self.crunch.len() {
                let (wait, left, p) = &mut self.crunch[k];
                *wait -= dt;
                if *wait <= 0.0 {
                    *wait = self.rng.range(0.002, 0.012);
                    *left -= 1;
                    let (f, gg, pp) = if surface == 3 { (self.rng.range(1500.0, 4000.0), 0.12, *p) } else { (self.rng.range(1200.0, 6000.0), 0.2, *p) };
                    let gain = self.rng.range(0.3, 1.0) * gg;
                    if let Some(b) = free(&mut self.bursts) {
                        b.start(f, 1.2, 0.0003, 0.004, gain, pp, rate);
                    }
                    if *left == 0 {
                        self.crunch.swap_remove(k);
                        continue;
                    }
                }
                k += 1;
            }
            let a = sum_bursts(&mut self.bursts, &mut self.rng, rate);
            let b = sum_modal(&mut self.modes, &mut self.rng);
            s.add(i, (a.0 + b.0, a.1 + b.1));
        }
        let norm = 1.0 / (n as f32).sqrt().max(1.0);
        add_filtered(&mut s, &mut self.lp, g * norm * 1.4, out);
    }
}

pub struct Clatter {
    rate: f32,
    rng: Rng,
    modes: Vec<Modal>,
    clicks: Vec<Burst>,
    ticker: Ticker,
    lp: Lp2,
}

impl Clatter {
    pub fn new(rate: f32, seed: u32) -> Clatter {
        Clatter { rate, rng: Rng::new(seed), modes: vec![Modal::default(); 14], clicks: vec![Burst::default(); 14], ticker: Ticker::new(), lp: Lp2::new(rate) }
    }
}

impl Gen for Clatter {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [material, per_sec, size, distance, ..] = *v;
        let material = pick(material).min(5);
        let rate = self.rate;
        let (g, cut) = far(distance);
        self.lp.set(cut, rate);
        let dt = 1.0 / rate;
        let mut s = Block::new();
        for i in 0..BLOCK {
            if self.ticker.fire(&mut self.rng, per_sec, 0.0, dt) {
                let r = &mut self.rng;
                let sz = 1.3 - size * 0.6;
                let (base, partials, decay): (f32, &[(f32, f32, f32)], f32) = match material {
                    0 => (r.range(1800.0, 3800.0) * sz, &[(1.0, 0.6, 1.0), (2.7, 0.3, 0.5), (5.2, 0.15, 0.3)], 1.0),
                    1 => (r.range(1300.0, 3000.0) * sz, &[(1.0, 0.12, 1.0), (2.1, 0.08, 0.6), (3.9, 0.05, 0.4)], 1.0),
                    2 => (r.range(2800.0, 5500.0) * sz, &[(1.0, 0.25, 1.0), (2.76, 0.15, 0.6), (5.4, 0.08, 0.4), (8.93, 0.05, 0.2)], 1.0),
                    3 => (r.range(250.0, 650.0) * sz, &[(1.0, 0.9, 1.0), (1.59, 0.6, 0.7), (2.14, 0.5, 0.5), (2.65, 0.4, 0.3)], 1.0),
                    4 => (r.range(4500.0, 8000.0) * sz, &[(1.0, 0.15, 1.0), (2.4, 0.1, 0.5)], 1.0),
                    _ => (r.range(2500.0, 4500.0) * sz, &[(1.0, 0.03, 1.0), (2.2, 0.02, 0.5)], 1.0),
                };
                let (gain, p) = (r.range(0.15, 0.5), r.bi() * 0.8);
                let count = if material >= 4 { 1 + r.below(4) } else { 1 };
                for _ in 0..count {
                    if let Some(m) = free(&mut self.modes) {
                        m.start(base * r.range(0.97, 1.03), partials, decay, gain, p, rate);
                    }
                }
                let f = r.range(2000.0, 6000.0);
                if let Some(b) = free(&mut self.clicks) {
                    b.start(f, 1.0, 0.0002, 0.002, gain * 0.5, p, rate);
                }
            }
            let a = sum_modal(&mut self.modes, &mut self.rng);
            let b = sum_bursts(&mut self.clicks, &mut self.rng, rate);
            s.add(i, (a.0 * 0.4 + b.0, a.1 * 0.4 + b.1));
        }
        add_filtered(&mut s, &mut self.lp, g, out);
    }
}

/// Someone typing: words, then pauses.
#[derive(Clone, Copy, Default)]
struct Typist {
    wait: f32,
    word: u32,
    pan: f32,
    gain: f32,
}

pub struct Typing {
    rate: f32,
    rng: Rng,
    typists: Vec<Typist>,
    clicks: Vec<Burst>,
    modes: Vec<Modal>,
    motor: f32,
    motor_lp: OnePole,
    motor_noise: ColorNoise,
    lp: Lp2,
}

impl Typing {
    pub fn new(rate: f32, seed: u32) -> Typing {
        let mut rng = Rng::new(seed);
        let typists = (0..30).map(|_| Typist { wait: rng.range(0.0, 2.0), word: 0, pan: rng.bi() * 0.9, gain: rng.range(0.3, 1.0) }).collect();
        Typing { rate, rng, typists, clicks: vec![Burst::default(); 40], modes: vec![Modal::default(); 10], motor: 0.0, motor_lp: OnePole::new(200.0, rate), motor_noise: ColorNoise::new(Color::Pink, seed, rate), lp: Lp2::new(rate) }
    }
}

impl Gen for Typing {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [kind, speed, typists, distance, ..] = *v;
        let kind = pick(kind).min(4);
        let rate = self.rate;
        let (g, cut) = far(distance);
        self.lp.set(cut, rate);
        let n = (typists.round() as usize).clamp(1, 30);
        let keys = match kind {
            2 => 2.0 + speed * 5.0,
            3 => 9.0 + speed * 4.0,
            4 => 0.2 + speed * 1.5,
            _ => 3.0 + speed * 9.0,
        };
        let dt = 1.0 / rate;
        let mut s = Block::new();
        for i in 0..BLOCK {
            for k in 0..n {
                let t = &mut self.typists[k];
                t.wait -= dt;
                if t.wait > 0.0 {
                    continue;
                }
                let r = &mut self.rng;
                let (p, gain) = (t.pan, t.gain);
                if kind == 3 {
                    t.wait = 1.0 / keys * r.range(0.95, 1.05);
                } else if t.word == 0 {
                    // Between words, sometimes a long think.
                    t.word = 2 + r.below(8) as u32;
                    t.wait = if r.chance(0.15) { r.range(1.0, 5.0) } else { r.range(0.15, 0.5) } / (0.5 + speed);
                    if kind == 2 && r.chance(0.08) {
                        // The typewriter's bell before the return.
                        let base = 2200.0;
                        if let Some(m) = free(&mut self.modes) {
                            m.start(base, &[(1.0, 1.2, 1.0), (2.76, 0.5, 0.3)], 1.0, gain * 0.3, p, rate);
                        }
                    }
                    continue;
                } else {
                    t.word -= 1;
                    t.wait = 1.0 / keys * r.range(0.5, 1.5);
                }
                let (f, d, a) = match kind {
                    1 => (r.range(2500.0, 4500.0), 0.006, 0.7),
                    2 => (r.range(1200.0, 2500.0), 0.012, 0.9),
                    3 => (r.range(1500.0, 3000.0), 0.006, 0.5),
                    4 => (r.range(3000.0, 5000.0), 0.003, 0.4),
                    _ => (r.range(2500.0, 5000.0), 0.004, 0.4),
                };
                if let Some(b) = free(&mut self.clicks) {
                    b.start(f, 1.5, 0.0002, d, gain * a, p, rate);
                }
                // The key's thock, a typewriter's typebar against the platen.
                let (lo, hi) = if kind == 2 { (400.0, 700.0) } else { (180.0, 380.0) };
                let base = r.range(lo, hi);
                if let Some(m) = free(&mut self.modes) {
                    m.start(base, &[(1.0, 0.02, 1.0), (2.3, 0.012, 0.4)], if kind == 2 { 3.0 } else { 1.0 }, gain * 0.25, p, rate);
                }
            }
            let a = sum_bursts(&mut self.clicks, &mut self.rng, rate);
            let b = sum_modal(&mut self.modes, &mut self.rng);
            // A teletype's motor runs under it.
            let mut m = 0.0;
            if kind == 3 {
                self.motor = (self.motor + 60.0 * dt).fract();
                m = self.motor_lp.low(self.motor_noise.next()) * 0.6 + sin1(self.motor) * 0.03;
            }
            s.add(i, (a.0 + b.0 + m, a.1 + b.1 + m));
        }
        add_filtered(&mut s, &mut self.lp, g / (n as f32).powf(0.3), out);
    }
}

pub struct Choir {
    rate: f32,
    rng: Rng,
    voices: [(Voice, f32, Drift); 24],
    vibrato: [f32; 24],
    motion: Drift,
}

impl Choir {
    pub fn new(rate: f32, seed: u32) -> Choir {
        let mut rng = Rng::new(seed);
        let voices = std::array::from_fn(|k| {
            let mut v = Voice::default();
            v.pan = rng.bi() * 0.9;
            v.glide = 3.0;
            v.gain = rng.range(0.6, 1.0);
            (v, rng.range(-1.0, 1.0), Drift::new(seed + k as u32 * 3))
        });
        Choir { rate, rng, voices, vibrato: std::array::from_fn(|k| k as f32 * 0.37), motion: Drift::new(seed ^ 0xc0) }
    }
}

impl Gen for Choir {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [pitch, vowel, voices, motion, chord, _] = *v;
        let rate = self.rate;
        let bt = BLOCK as f32 / rate;
        let n = (voices.round() as usize).clamp(1, 24);
        // The choir's chord set: Voices pad lists six, the drone seven.
        let chord = CHORDS[[0, 1, 2, 3, 4, 6][pick(chord).min(5)]];
        let vi = pick(vowel).min(5);
        let hum = vi == 5;
        let m = self.motion.next(0.05, bt);
        let base_vowel = if hum { [260.0, 1700.0, 2600.0] } else { VOWELS[vi] };
        for (k, (voice, detune, d)) in self.voices.iter_mut().enumerate().take(n) {
            let semis = chord[k % chord.len()] + if k >= chord.len() * 2 { 12.0 } else { 0.0 };
            let drift = d.next(0.1, bt);
            voice.target_pitch = pitch * 2f32.powf((semis + *detune * 0.12) / 12.0);
            if voice.pitch == 0.0 {
                voice.pitch = voice.target_pitch;
            }
            // Motion morphs the vowel and breathes the voices in and out.
            for j in 0..3 {
                voice.target_vowel[j] = base_vowel[j] * (1.0 + motion * 0.15 * (m + drift * 0.5));
            }
            voice.breath = if hum { 0.02 } else { 0.08 };
            voice.on = true;
            voice.gain = (0.7 + 0.3 * drift * motion) * if hum { 0.6 } else { 1.0 };
        }
        for (voice, ..) in self.voices.iter_mut().skip(n) {
            voice.on = false;
        }
        let norm = 0.5 / (n as f32).sqrt();
        let dt = 1.0 / rate;
        for i in 0..BLOCK {
            let mut x = (0.0, 0.0);
            for (k, (voice, ..)) in self.voices.iter_mut().enumerate() {
                if !voice.on && voice.env < 0.0005 {
                    continue;
                }
                self.vibrato[k] = (self.vibrato[k] + (4.8 + k as f32 * 0.07) * dt).fract();
                let target = voice.target_pitch;
                voice.target_pitch = target * (1.0 + 0.006 * sin1(self.vibrato[k]));
                let y = voice.tick(&mut self.rng, rate) * norm;
                voice.target_pitch = target;
                let p = pan(y, voice.pan);
                x.0 += p.0;
                x.1 += p.1;
            }
            out.add(i, x);
        }
    }
}

pub struct Whispers {
    rate: f32,
    rng: Rng,
    voices: [(Voice, f32, f32); 6],
    ticker: Ticker,
    hp: [Svf; 2],
}

impl Whispers {
    pub fn new(rate: f32, seed: u32) -> Whispers {
        let mut rng = Rng::new(seed);
        let ticker = Ticker::primed(&mut rng, 0.3);
        Whispers { rate, rng, voices: [(Voice::default(), 0.0, 0.0); 6], ticker, hp: [Svf::new(300.0, 0.7, rate); 2] }
    }
}

impl Gen for Whispers {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [density, close, pitch, breath, ..] = *v;
        let rate = self.rate;
        for f in &mut self.hp {
            f.set(200.0 + 600.0 * close, 0.7, rate);
        }
        let dt = 1.0 / rate;
        let shift = 0.75 + pitch * 0.5;
        for i in 0..BLOCK {
            if self.ticker.fire(&mut self.rng, density, 0.0, dt) {
                let r = &mut self.rng;
                if let Some((voice, left, syl)) = self.voices.iter_mut().find(|(vo, l, _)| *l <= 0.0 && vo.env < 0.001) {
                    *left = r.range(0.8, 2.5);
                    *syl = 0.0;
                    voice.pan = r.bi();
                    voice.gain = r.range(0.4, 1.0) * (0.3 + close);
                    voice.breath = breath;
                    voice.pitch = 140.0 * shift;
                    voice.target_pitch = voice.pitch;
                    voice.glide = 10.0;
                }
            }
            let mut x = (0.0, 0.0);
            for (voice, left, syl) in &mut self.voices {
                if *left > 0.0 {
                    *left -= dt;
                    *syl -= dt;
                    if *syl <= 0.0 {
                        *syl = self.rng.range(0.07, 0.16);
                        voice.target_vowel = VOWELS[self.rng.below(6)].map(|f| f * shift * 1.1);
                        voice.on = self.rng.chance(0.8);
                    }
                    if *left <= 0.0 {
                        voice.on = false;
                    }
                }
                let y = voice.tick(&mut self.rng, rate) * 0.9;
                let p = pan(y, voice.pan);
                x.0 += p.0;
                x.1 += p.1;
            }
            out.add(i, (self.hp[0].high(x.0), self.hp[1].high(x.1)));
        }
    }
}

pub struct Breathing {
    rate: f32,
    rng: Rng,
    noise: [ColorNoise; 2],
    band: [Svf; 2],
    reso: [Svf; 2],
    phase: f32,
    rasp: f32,
    hiss: ColorNoise,
    clicks: [Burst; 4],
}

impl Breathing {
    pub fn new(rate: f32, seed: u32) -> Breathing {
        Breathing {
            rate,
            rng: Rng::new(seed),
            noise: [ColorNoise::new(Color::Pink, seed, rate), ColorNoise::new(Color::Pink, seed ^ 2, rate)],
            band: [Svf::new(900.0, 1.2, rate); 2],
            reso: [Svf::new(1200.0, 6.0, rate); 2],
            phase: 0.0,
            rasp: 0.0,
            hiss: ColorNoise::new(Color::White, seed ^ 7, rate),
            clicks: [Burst::default(); 4],
        }
    }
}

impl Gen for Breathing {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [per_min, depth, kind, close, ..] = *v;
        let kind = pick(kind).min(3);
        let rate = self.rate;
        let bt = BLOCK as f32 / rate;
        let before = self.phase;
        self.phase = (self.phase + per_min / 60.0 * bt).fract();
        // In for 40% of the breath, out for the rest.
        let (inhale, u) = if self.phase < 0.4 { (true, self.phase / 0.4) } else { (false, (self.phase - 0.4) / 0.6) };
        let env = (std::f32::consts::PI * u).sin().powf(if inhale { 1.2 } else { 0.8 }) * if inhale { 0.8 } else { 1.0 };
        let f = if inhale { 700.0 + 900.0 * u } else { 1300.0 - 700.0 * u } * (0.8 + depth * 0.4);
        for c in 0..2 {
            self.band[c].set(f, if kind == 2 { 3.0 } else { 1.2 }, rate);
            self.reso[c].set(if kind == 3 { 950.0 } else { 1400.0 }, 6.0, rate);
        }
        // A valve clicks at the start of each half of a respirator's breath.
        let crossed = (before < 0.4 && self.phase >= 0.4) || self.phase < before;
        if kind == 3 && crossed {
            let p = self.rng.bi() * 0.1;
            if let Some(b) = free(&mut self.clicks) {
                b.start(2500.0, 2.0, 0.0002, 0.01, 0.4, p, rate);
            }
        }
        let level = env * (0.25 + 0.75 * depth) * (0.2 + close * 0.8) * if kind == 1 { 1.4 } else { 1.0 };
        let dt = 1.0 / rate;
        for i in 0..BLOCK {
            self.rasp = (self.rasp + 35.0 * dt).fract();
            let rasp = if kind == 1 || kind == 3 { 0.6 + 0.4 * sin1(self.rasp) } else { 1.0 };
            let mut x = [0.0; 2];
            for c in 0..2 {
                let n = self.noise[c].next() * 2.0;
                let mut y = self.band[c].band(n);
                if kind >= 2 {
                    y = y * 0.5 + self.reso[c].band(n) * 0.7;
                }
                x[c] = y * level * rasp;
            }
            // A helmet hisses with its air supply.
            let hiss = if kind == 2 { self.hiss.next() * 0.03 } else { 0.0 };
            let k = sum_bursts(&mut self.clicks, &mut self.rng, rate);
            out.add(i, (x[0] + hiss + k.0, x[1] + hiss + k.1));
        }
    }
}

pub struct Heartbeat {
    rate: f32,
    t: f32,
    beat: [(f32, f32, f32, f32); 2],
    lp: [Svf; 2],
}

impl Heartbeat {
    pub fn new(rate: f32, _seed: u32) -> Heartbeat {
        Heartbeat { rate, t: 0.0, beat: [(10.0, 0.0, 0.0, 0.0); 2], lp: [Svf::new(200.0, 0.7, rate); 2] }
    }
}

impl Gen for Heartbeat {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [bpm, strength, muffle, ..] = *v;
        let rate = self.rate;
        for f in &mut self.lp {
            f.set(500.0 - 380.0 * muffle, 0.7, rate);
        }
        let period = 60.0 / bpm;
        // "Lub" and "dub": the second closer behind the first the faster it beats.
        let gap = 0.3 * (period).sqrt().min(1.0);
        let dt = 1.0 / rate;
        for i in 0..BLOCK {
            let before = self.t;
            self.t += dt;
            if self.t >= period {
                self.t -= period;
                self.beat[0] = (0.0, 55.0, 1.0, 0.0);
            }
            if before < gap && self.t >= gap {
                self.beat[1] = (0.0, 70.0, 0.7, 0.0);
            }
            let mut y = 0.0;
            for b in &mut self.beat {
                let (t, f, a, ph) = b;
                if *t < 0.25 {
                    let env = (*t * 200.0).min(1.0) * (-*t * 28.0).exp();
                    let freq = *f * (1.0 - 0.4 * (*t * 8.0).min(1.0));
                    *ph = (*ph + freq * dt).fract();
                    y += sin1(*ph) * env * *a;
                }
                *t += dt;
            }
            let l = self.lp[0].low(y) * strength * 0.9;
            out.add(i, (l, self.lp[1].low(y) * strength * 0.9));
        }
    }
}

/// An instrument note in the music.
#[derive(Clone)]
struct Tone {
    kind: Ins,
    phase: f32,
    freq: f32,
    glide: f32,
    env: Env,
    gate: f32,
    gain: f32,
    pan: f32,
    filter: Svf,
    ks: Vec<f32>,
    ks_len: usize,
    ks_at: usize,
}

#[derive(Clone, Copy, PartialEq)]
enum Ins {
    Kick,
    Snare,
    Hat,
    Tom,
    Bass,
    Keys,
    Organ,
    Pluck,
    Tine,
    Brass,
}

impl Live for Tone {
    fn live(&self) -> bool {
        self.env.live()
    }
}

impl Tone {
    fn new() -> Tone {
        Tone { kind: Ins::Kick, phase: 0.0, freq: 100.0, glide: 1.0, env: Env::default(), gate: 0.0, gain: 0.0, pan: 0.0, filter: Svf::default(), ks: vec![0.0; 2048], ks_len: 1, ks_at: 0 }
    }

    #[allow(clippy::too_many_arguments)]
    fn start(&mut self, kind: Ins, freq: f32, len: f32, gain: f32, pan: f32, rate: f32, rng: &mut Rng) {
        self.kind = kind;
        self.freq = freq;
        self.phase = 0.0;
        self.gain = gain;
        self.pan = pan;
        self.gate = len;
        self.glide = 1.0;
        let (attack, decay) = match kind {
            Ins::Kick => (0.001, 0.3),
            Ins::Snare => (0.001, 0.15),
            Ins::Hat => (0.0005, 0.04),
            Ins::Tom => (0.002, 0.5),
            Ins::Bass => (0.005, 0.4),
            Ins::Keys => (0.003, 1.2),
            Ins::Organ => (0.02, 8.0),
            Ins::Pluck => (0.001, 2.0),
            Ins::Tine => (0.001, 1.5),
            Ins::Brass => (0.05, 6.0),
        };
        self.env.start(attack, decay, rate);
        match kind {
            Ins::Kick | Ins::Tom => self.glide = 0.35f32.powf(1.0 / (0.08 * rate)),
            Ins::Hat => self.filter.set(8000.0, 0.7, rate),
            Ins::Snare => self.filter.set(1800.0, 0.6, rate),
            Ins::Bass => self.filter.set(freq * 4.0, 1.5, rate),
            Ins::Brass => self.filter.set(freq * 5.0, 1.0, rate),
            Ins::Pluck => {
                // Karplus-Strong: a noise burst in a delay line one period long.
                self.ks_len = ((rate / freq) as usize).clamp(2, 2047);
                for s in self.ks.iter_mut().take(self.ks_len) {
                    *s = rng.bi();
                }
                self.ks_at = 0;
            }
            _ => {}
        }
    }

    #[inline]
    fn tick(&mut self, rng: &mut Rng, rate: f32) -> (f32, f32) {
        if !self.env.live() {
            return (0.0, 0.0);
        }
        let dt = 1.0 / rate;
        // Sustained instruments hold until their gate, then fall away.
        if matches!(self.kind, Ins::Organ | Ins::Brass) {
            self.gate -= dt;
            if self.gate <= 0.0 {
                self.env.start(0.0001, 0.15, rate);
                self.kind = if self.kind == Ins::Organ { Ins::Keys } else { Ins::Bass };
            }
        }
        self.phase = (self.phase + self.freq * dt).fract();
        self.freq *= self.glide;
        let e = self.env.tick();
        let p = self.phase;
        let x = match self.kind {
            Ins::Kick | Ins::Tom => sin1(p) + if self.kind == Ins::Tom { rng.bi() * 0.05 } else { 0.0 },
            Ins::Snare => self.filter.band(rng.bi()) * 1.4 + sin1(p) * 0.3,
            Ins::Hat => self.filter.high(rng.bi()) * 0.6,
            Ins::Bass => self.filter.low(2.0 * p - 1.0),
            Ins::Keys => sin1(p) * 0.7 + sin1(p * 2.0) * 0.2 + sin1(p * 3.0) * 0.1,
            Ins::Organ => sin1(p) * 0.5 + sin1(p * 2.0) * 0.3 + sin1(p * 4.0) * 0.15 + sin1(p * 3.0) * 0.1,
            Ins::Pluck => {
                let a = self.ks_at;
                let b = (a + 1) % self.ks_len;
                let y = self.ks[a];
                self.ks[a] = (self.ks[a] + self.ks[b]) * 0.498;
                self.ks_at = b;
                y
            }
            Ins::Tine => sin1(p) + sin1(p * 4.2) * 0.25 * e,
            Ins::Brass => self.filter.low(2.0 * p - 1.0) * 0.8,
        };
        pan(x * e * self.gain, self.pan)
    }
}

pub struct Music {
    rate: f32,
    rng: Rng,
    tones: Vec<Tone>,
    step: f32,
    at: usize,
    bar: usize,
    root: f32,
    minor: bool,
    melody: usize,
    wow: Drift,
    lp: Lp2,
    hp: [Svf; 2],
}

fn hz(semis: f32) -> f32 {
    440.0 * 2f32.powf((semis - 69.0) / 12.0)
}

impl Music {
    pub fn new(rate: f32, seed: u32) -> Music {
        let mut rng = Rng::new(seed);
        let root = 45.0 + rng.below(8) as f32;
        let minor = rng.chance(0.5);
        Music { rate, rng, tones: (0..24).map(|_| Tone::new()).collect(), step: 0.0, at: 0, bar: 0, root, minor, melody: 0, wow: Drift::new(seed), lp: Lp2::new(rate), hp: [Svf::new(400.0, 0.7, rate); 2] }
    }

    fn play(&mut self, kind: Ins, semis: f32, len: f32, gain: f32, pan: f32, detune: f32) {
        let rate = self.rate;
        let f = hz(semis) * (1.0 + detune * self.rng.range(-0.02, 0.02));
        if let Some(t) = self.tones.iter_mut().find(|t| !t.live()) {
            t.start(kind, f, len, gain, pan, rate, &mut self.rng);
        }
    }

    /// This bar's chord as semitones over the key's root.
    fn chord(&self) -> [f32; 3] {
        let prog: [[f32; 3]; 4] = if self.minor { [[0.0, 3.0, 7.0], [-4.0, 0.0, 3.0], [3.0, 7.0, 10.0], [-2.0, 2.0, 5.0]] } else { [[0.0, 4.0, 7.0], [7.0, 11.0, 14.0], [9.0, 12.0, 16.0], [5.0, 9.0, 12.0]] };
        prog[self.bar % 4]
    }

    fn step_on(&mut self, style: usize, detune: f32) {
        let s = self.at;
        let c = self.chord();
        let root = self.root + c[0];
        let beat = s.is_multiple_of(4);
        let scale: [f32; 5] = if self.minor { [0.0, 3.0, 5.0, 7.0, 10.0] } else { [0.0, 2.0, 4.0, 7.0, 9.0] };
        match style {
            // Club: four on the floor, off-beat hats, a bass on the off-beats, a held pad.
            0 => {
                if beat {
                    self.play(Ins::Kick, 40.0, 0.3, 0.9, 0.0, 0.0);
                }
                if s % 4 == 2 {
                    self.play(Ins::Hat, 0.0, 0.05, 0.25, 0.3, 0.0);
                    self.play(Ins::Bass, root - 12.0, 0.2, 0.45, 0.0, detune);
                }
                if s == 4 || s == 12 {
                    self.play(Ins::Snare, 50.0, 0.1, 0.35, -0.1, 0.0);
                }
                if s == 0 {
                    for n in c {
                        self.play(Ins::Organ, self.root + n + 12.0, 1.8, 0.08, 0.0, detune);
                    }
                }
            }
            // A bar band: back beat, a walking bass, chords on the off-beats.
            1 => {
                if s == 0 || s == 8 || s == 10 {
                    self.play(Ins::Kick, 40.0, 0.3, 0.7, 0.0, 0.0);
                }
                if s == 4 || s == 12 {
                    self.play(Ins::Snare, 50.0, 0.1, 0.5, 0.0, 0.0);
                }
                if s.is_multiple_of(2) {
                    self.play(Ins::Hat, 0.0, 0.05, 0.15, 0.4, 0.0);
                }
                if beat {
                    let walk = [0.0, 4.0, 7.0, 9.0][s / 4];
                    self.play(Ins::Bass, root - 12.0 + walk, 0.3, 0.5, -0.2, detune);
                }
                if s % 4 == 2 {
                    for n in c {
                        self.play(Ins::Keys, self.root + n + 12.0, 0.2, 0.1, 0.25, detune);
                    }
                }
            }
            // Piano: the chord on the bar, broken chords between.
            2 => {
                if s == 0 {
                    self.play(Ins::Keys, root - 12.0, 1.0, 0.25, -0.2, detune);
                    for n in c {
                        self.play(Ins::Keys, self.root + n, 1.0, 0.12, 0.0, detune);
                    }
                }
                if s.is_multiple_of(2) && s > 0 {
                    let n = c[self.rng.below(3)] + if self.rng.chance(0.5) { 12.0 } else { 0.0 };
                    self.play(Ins::Keys, self.root + n + 12.0, 0.5, 0.12, 0.2, detune);
                }
            }
            // Organ: held chords.
            3 => {
                if s == 0 {
                    self.play(Ins::Organ, root - 12.0, 1.9, 0.18, 0.0, detune);
                    for n in c {
                        self.play(Ins::Organ, self.root + n + 12.0, 1.9, 0.1, 0.0, detune);
                    }
                }
            }
            // A pop song on a radio: drums, bass, chords and a tune.
            4 => {
                if s == 0 || s == 8 {
                    self.play(Ins::Kick, 40.0, 0.3, 0.6, 0.0, 0.0);
                }
                if s == 4 || s == 12 {
                    self.play(Ins::Snare, 50.0, 0.1, 0.45, 0.0, 0.0);
                }
                if beat {
                    self.play(Ins::Bass, root - 12.0, 0.3, 0.4, 0.0, detune);
                    for n in c {
                        self.play(Ins::Keys, self.root + n + 12.0, 0.3, 0.07, 0.0, detune);
                    }
                }
                if s.is_multiple_of(2) && self.rng.chance(0.7) {
                    self.melody = (self.melody + self.rng.below(3)).saturating_sub(1) % 5;
                    self.play(Ins::Tine, self.root + 24.0 + scale[self.melody], 0.25, 0.12, 0.0, detune);
                }
            }
            // A music box, plinking a tune.
            5 => {
                if s.is_multiple_of(2) {
                    self.melody = (self.melody + 1 + self.rng.below(3)) % 5;
                    let oct = if self.rng.chance(0.3) { 36.0 } else { 24.0 };
                    self.play(Ins::Tine, self.root + oct + scale[self.melody], 0.6, 0.18, 0.0, detune);
                }
                if s == 0 {
                    self.play(Ins::Tine, root + 12.0, 0.8, 0.12, 0.0, detune);
                }
            }
            // A brass band: oom-pah, a snare and brass chords.
            6 => {
                if s == 0 || s == 8 {
                    self.play(Ins::Brass, root - 12.0, 0.3, 0.35, 0.0, detune);
                }
                if s == 4 || s == 12 {
                    for n in c {
                        self.play(Ins::Brass, self.root + n, 0.25, 0.14, 0.2, detune);
                    }
                }
                if s.is_multiple_of(2) {
                    self.play(Ins::Snare, 50.0, 0.1, if s.is_multiple_of(4) { 0.35 } else { 0.15 }, -0.2, 0.0);
                }
            }
            // War drums: big toms in a driving pattern.
            7 => {
                if [0, 3, 6, 8, 11, 12, 14].contains(&s) {
                    let f = if s.is_multiple_of(8) { 30.0 } else { 36.0 };
                    let p = self.rng.bi() * 0.4;
                    self.play(Ins::Tom, f, 0.5, if s.is_multiple_of(8) { 1.0 } else { 0.6 }, p, 0.0);
                }
            }
            // Folk: plucked strings over a drone, a frame drum.
            8 => {
                if s.is_multiple_of(2) {
                    let n = c[(s / 2) % 3] + if s >= 8 { 12.0 } else { 0.0 };
                    self.play(Ins::Pluck, self.root + n + 12.0, 1.0, 0.35, 0.2, detune);
                }
                if s == 0 {
                    self.play(Ins::Organ, self.root - 12.0, 1.9, 0.06, -0.2, detune);
                    self.play(Ins::Tom, 45.0, 0.3, 0.4, 0.0, 0.0);
                }
                if s == 8 {
                    self.play(Ins::Tom, 47.0, 0.3, 0.3, 0.0, 0.0);
                }
            }
            // A carnival waltz on a calliope: oom-pah-pah.
            _ => {
                if s.is_multiple_of(4) {
                    let b = s / 4 % 3;
                    if b == 0 {
                        self.play(Ins::Organ, root - 12.0, 0.2, 0.2, 0.0, detune);
                    } else {
                        for n in c {
                            self.play(Ins::Organ, self.root + n + 12.0, 0.15, 0.08, 0.0, detune);
                        }
                    }
                }
                if s.is_multiple_of(2) && self.rng.chance(0.8) {
                    self.melody = (self.melody + 1 + self.rng.below(2)) % 5;
                    self.play(Ins::Organ, self.root + 24.0 + scale[self.melody], 0.12, 0.1, 0.0, detune);
                }
            }
        }
    }
}

impl Gen for Music {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [style, tempo, muffle, detune, ..] = *v;
        let style = pick(style).min(9);
        let rate = self.rate;
        // The muffle: a wall or a distance takes the top off; a radio is tinny.
        self.lp.set(150.0 * 50f32.powf(1.0 - muffle), rate);
        for f in &mut self.hp {
            f.set(if style == 4 { 450.0 } else { 20.0 }, 0.7, rate);
        }
        let bt = BLOCK as f32 / rate;
        let steps_per_bar = if style == 9 { 12 } else { 16 };
        let wow = 1.0 + detune * 0.015 * self.wow.next(0.6, bt);
        self.step += tempo / 60.0 * 4.0 * bt * wow;
        while self.step >= 1.0 {
            self.step -= 1.0;
            self.step_on(style, detune);
            self.at += 1;
            if self.at >= steps_per_bar {
                self.at = 0;
                self.bar += 1;
            }
        }
        let mut s = Block::new();
        for i in 0..BLOCK {
            let mut x = (0.0, 0.0);
            for t in &mut self.tones {
                let y = t.tick(&mut self.rng, rate);
                x.0 += y.0;
                x.1 += y.1;
            }
            s.add(i, (self.hp[0].high(x.0), self.hp[1].high(x.1)));
        }
        add_filtered(&mut s, &mut self.lp, 0.7 * db(-10.0 * muffle), out);
    }
}

pub struct Knocks {
    rate: f32,
    rng: Rng,
    modes: Vec<Modal>,
    bursts: Vec<Burst>,
    ticker: Ticker,
    queue: Vec<(f32, usize, f32, f32)>,
    lp: Lp2,
}

impl Knocks {
    pub fn new(rate: f32, seed: u32) -> Knocks {
        let mut rng = Rng::new(seed);
        let ticker = Ticker::primed(&mut rng, 0.3);
        Knocks { rate, rng, modes: vec![Modal::default(); 10], bursts: vec![Burst::default(); 10], ticker, queue: Vec::with_capacity(32), lp: Lp2::new(rate) }
    }

    fn hit(&mut self, kind: usize, size: f32, gain: f32, p: f32) {
        let rate = self.rate;
        let r = &mut self.rng;
        let sz = 1.3 - size * 0.6;
        match kind {
            // A knuckle on a door: wood.
            0 => {
                if let Some(m) = free(&mut self.modes) {
                    m.start(r.range(170.0, 260.0) * sz, &[(1.0, 0.05, 1.0), (2.3, 0.03, 0.6), (4.1, 0.02, 0.4)], 1.0, gain * 0.6, p, rate);
                }
            }
            // A thud, footsteps overhead, a slam: low and through a wall.
            1..=3 => {
                let (f, d) = match kind {
                    1 => (r.range(60.0, 120.0) * sz, 0.12),
                    2 => (r.range(70.0, 130.0) * sz, 0.08),
                    _ => (r.range(50.0, 90.0) * sz, 0.3),
                };
                if let Some(b) = free(&mut self.bursts) {
                    b.start(f, 0.9, 0.002, d, gain * 1.4, p, rate);
                }
                if kind == 3 {
                    if let Some(m) = free(&mut self.modes) {
                        m.start(r.range(120.0, 200.0), &[(1.0, 0.15, 1.0), (1.7, 0.1, 0.6), (2.9, 0.08, 0.4)], 1.0, gain * 0.4, p, rate);
                    }
                }
            }
            // A metal bang or a hammer on metal.
            _ => {
                if let Some(m) = free(&mut self.modes) {
                    m.start(r.range(220.0, 500.0) * sz, &[(1.0, 1.2, 1.0), (1.59, 0.9, 0.7), (2.14, 0.7, 0.5), (2.65, 0.5, 0.4), (3.9, 0.3, 0.2)], 1.0, gain * 0.25, p, rate);
                }
                let f = r.range(1500.0, 4000.0);
                if let Some(b) = free(&mut self.bursts) {
                    b.start(f, 1.0, 0.0003, 0.004, gain * 0.5, p, rate);
                }
            }
        }
    }
}

impl Gen for Knocks {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [kind, per_sec, size, distance, ..] = *v;
        let kind = pick(kind).min(5);
        let rate = self.rate;
        let (g, cut) = far(distance);
        // Footsteps upstairs come through a ceiling.
        self.lp.set(if kind == 2 { cut.min(400.0) } else { cut }, rate);
        let dt = 1.0 / rate;
        let mut s = Block::new();
        for i in 0..BLOCK {
            if self.ticker.fire(&mut self.rng, per_sec, 0.0, dt) {
                let r = &mut self.rng;
                let p = r.bi() * 0.8;
                let (count, gap) = match kind {
                    0 => (2 + r.below(3), 0.16),
                    2 => (4 + r.below(8), 0.55),
                    5 => (3 + r.below(8), 0.4),
                    _ => (1, 0.0),
                };
                for k in 0..count {
                    let gain = r.range(0.6, 1.0);
                    self.queue.push((k as f32 * gap * r.range(0.9, 1.1), kind, gain, p));
                }
            }
            let mut k = 0;
            while k < self.queue.len() {
                self.queue[k].0 -= dt;
                if self.queue[k].0 <= 0.0 {
                    let (_, kind, gain, p) = self.queue.swap_remove(k);
                    self.hit(kind, size, gain, p);
                } else {
                    k += 1;
                }
            }
            let a = sum_modal(&mut self.modes, &mut self.rng);
            let b = sum_bursts(&mut self.bursts, &mut self.rng, rate);
            s.add(i, (a.0 + b.0, a.1 + b.1));
        }
        add_filtered(&mut s, &mut self.lp, g, out);
    }
}
