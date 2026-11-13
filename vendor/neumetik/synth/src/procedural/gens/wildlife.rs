// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.
// Copied from bgn/src/gens/wildlife.rs, Neumetik BGN (bgn/, ansiapps' own), by synth/scripts/copies.py:
// change it there, then run the script with --update.

//! Animals: insects by amplitude-modulated tones and band-passed rasps,
//! birds by frequency-swept sines in phrases, and the voiced calls
//! (howls, barks, lowing) by a buzzing source through formant filters.

use super::super::dsp::{free, pan, sin1, Block, Chirp, Drift, Modal, Rng, Svf, Ticker, Voice, BLOCK, VOWELS};
use super::super::noise::{Color, ColorNoise};
use super::{add_filtered, far, pick, Gen, Lp2};

/// A note to start later: when (seconds from now), from and to Hz, its
/// length, loudness, pan, vibrato depth and rate, harmonics, rasp.
#[derive(Clone, Copy)]
struct Note {
    at: f32,
    from: f32,
    to: f32,
    len: f32,
    gain: f32,
    pan: f32,
    vib: (f32, f32),
    harm: f32,
    rough: f32,
    pulses: f32,
}

impl Note {
    fn new(at: f32, from: f32, to: f32, len: f32, gain: f32, pan: f32) -> Note {
        Note { at, from, to, len, gain, pan, vib: (0.0, 0.0), harm: 0.0, rough: 0.0, pulses: 0.0 }
    }
}

/// Notes waiting their turn, played on a pool of chirps.
struct Score {
    notes: Vec<Note>,
    chirps: Vec<Chirp>,
}

impl Score {
    fn new(voices: usize) -> Score {
        Score { notes: Vec::with_capacity(64), chirps: vec![Chirp::default(); voices] }
    }

    fn add(&mut self, n: Note) {
        if self.notes.len() < 256 {
            self.notes.push(n);
        }
    }

    #[inline]
    fn tick(&mut self, rate: f32) -> (f32, f32) {
        let dt = 1.0 / rate;
        let mut i = 0;
        while i < self.notes.len() {
            self.notes[i].at -= dt;
            if self.notes[i].at <= 0.0 {
                let n = self.notes.swap_remove(i);
                if let Some(c) = free(&mut self.chirps) {
                    c.start(n.from, n.to, n.len, n.gain, n.pan, rate);
                    c.vibrato(n.vib.0, n.vib.1);
                    c.harmonics(n.harm);
                    c.rough(n.rough);
                    c.pulses(n.pulses);
                }
            } else {
                i += 1;
            }
        }
        let mut x = (0.0, 0.0);
        for c in &mut self.chirps {
            let y = c.tick(rate);
            x.0 += y.0;
            x.1 += y.1;
        }
        x
    }
}

/// One insect or frog, calling on its own clock.
#[derive(Clone, Copy, Default)]
struct Caller {
    phase: f32,
    period: f32,
    pitch: f32,
    gain: f32,
    pan: f32,
}

fn callers(rng: &mut Rng, n: usize) -> Vec<Caller> {
    (0..n)
        .map(|_| Caller { phase: rng.uni(), period: rng.range(0.85, 1.15), pitch: rng.range(0.9, 1.1), gain: rng.range(0.2, 1.0).powi(2), pan: rng.bi() * 0.9 })
        .collect()
}

pub struct Crickets {
    rate: f32,
    rng: Rng,
    callers: Vec<Caller>,
    score: Score,
    clock: f32,
}

impl Crickets {
    pub fn new(rate: f32, seed: u32) -> Crickets {
        let mut rng = Rng::new(seed);
        let callers = callers(&mut rng, 24);
        Crickets { rate, rng, callers, score: Score::new(48), clock: 0.0 }
    }
}

impl Gen for Crickets {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [density, temp, pitch, sync, kind, _] = *v;
        let kind = pick(kind).min(2);
        let rate = self.rate;
        let n = (1.0 + density * 23.0) as usize;
        // Dolbear's law: chirps a minute = 4 (°F - 50) + 40.
        let f = 1.8 * temp + 32.0;
        let chirps = ((4.0 * (f - 50.0) + 40.0) / 60.0).max(0.3);
        let carrier = [4500.0, 2600.0, 7000.0][kind] * (0.75 + pitch * 0.5);
        let bt = BLOCK as f32 / rate;
        self.clock = (self.clock + chirps * bt).fract();
        for (k, c) in self.callers.iter_mut().take(n).enumerate() {
            let before = c.phase;
            c.phase += chirps * c.period * bt;
            // Sync pulls each one toward the shared clock.
            let mut diff = self.clock - c.phase.fract();
            if diff > 0.5 {
                diff -= 1.0;
            } else if diff < -0.5 {
                diff += 1.0;
            }
            c.phase += diff * sync * 0.05;
            if c.phase.floor() > before.floor() || c.phase < 0.0 {
                c.phase = c.phase.rem_euclid(1.0);
                match kind {
                    // A chirp of three to five wing strokes.
                    0 => {
                        let strokes = 3.0 + (k % 3) as f32;
                        let len = strokes / 30.0;
                        let mut note = Note::new(0.0, carrier * c.pitch, carrier * c.pitch * 0.98, len, c.gain * 0.18, c.pan);
                        note.pulses = 15.0;
                        self.score.add(note);
                    }
                    // Tree crickets trill on and on.
                    1 => {
                        let mut note = Note::new(0.0, carrier * c.pitch, carrier * c.pitch, 1.0 / chirps * 0.95, c.gain * 0.1, c.pan);
                        note.pulses = 25.0;
                        self.score.add(note);
                    }
                    // Katydids rasp "ka-ty-did".
                    _ => {
                        let r = &mut self.rng;
                        for s in 0..(2 + r.below(2)) {
                            let at = s as f32 * 0.16;
                            let mut note = Note::new(at, carrier * c.pitch * 0.5, carrier * c.pitch * 0.5, 0.09, c.gain * 0.12, c.pan);
                            note.pulses = 40.0;
                            note.rough = 0.9;
                            self.score.add(note);
                        }
                    }
                }
            }
        }
        for i in 0..BLOCK {
            out.add(i, self.score.tick(rate));
        }
    }
}

pub struct Cicadas {
    rate: f32,
    noise: [ColorNoise; 4],
    band: [Svf; 4],
    swell: [Drift; 4],
    phase: [f32; 4],
    pans: [f32; 4],
}

impl Cicadas {
    pub fn new(rate: f32, seed: u32) -> Cicadas {
        let mut rng = Rng::new(seed);
        Cicadas {
            rate,
            noise: std::array::from_fn(|k| ColorNoise::new(Color::White, seed + k as u32 * 31, rate)),
            band: [Svf::new(5000.0, 4.0, rate); 4],
            swell: std::array::from_fn(|k| Drift::new(seed + k as u32 * 7)),
            phase: [0.0; 4],
            pans: std::array::from_fn(|_| rng.bi() * 0.8),
        }
    }
}

impl Gen for Cicadas {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [density, pitch, swell, harsh, ..] = *v;
        let rate = self.rate;
        let bt = BLOCK as f32 / rate;
        let n = 1 + (density * 3.0).round() as usize;
        let mut env = [0.0; 4];
        for k in 0..4 {
            self.band[k].set((3500.0 + 3500.0 * pitch) * (1.0 + 0.07 * k as f32), 3.0 + 3.0 * (1.0 - harsh), rate);
            let s = 0.5 + 0.5 * self.swell[k].next(0.08, bt);
            env[k] = if k < n { (1.0 - swell) * 0.6 + swell * s.powf(2.0) * 1.2 } else { 0.0 };
        }
        let dt = 1.0 / rate;
        for i in 0..BLOCK {
            let mut x = (0.0, 0.0);
            for k in 0..n {
                // The tymbal clicks 100 to 300 times a second.
                self.phase[k] = (self.phase[k] + (140.0 + 60.0 * k as f32) * dt).fract();
                let pulse = 1.0 - harsh * (1.0 - (self.phase[k] * 2.0 - 1.0).abs());
                let y = self.band[k].band(self.noise[k].next() * 2.0) * pulse * env[k] * 0.25;
                let p = pan(y, self.pans[k]);
                x.0 += p.0;
                x.1 += p.1;
            }
            out.add(i, x);
        }
    }
}

pub struct Frogs {
    rate: f32,
    rng: Rng,
    callers: Vec<Caller>,
    score: Score,
    voices: Vec<(Voice, f32)>,
    lp: Lp2,
}

impl Frogs {
    pub fn new(rate: f32, seed: u32) -> Frogs {
        let mut rng = Rng::new(seed);
        let callers = callers(&mut rng, 16);
        Frogs { rate, rng, callers, score: Score::new(32), voices: vec![(Voice::default(), 0.0); 6], lp: Lp2::new(rate) }
    }
}

impl Gen for Frogs {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [kind, density, pitch, distance, ..] = *v;
        let kind = pick(kind).min(3);
        let rate = self.rate;
        let (g, cut) = far(distance);
        self.lp.set(cut, rate);
        let n = (1.0 + density * 15.0) as usize;
        let bt = BLOCK as f32 / rate;
        // Calls a second for each frog.
        let calls = [1.2, 0.25, 0.8, 0.12][kind];
        let pf = 0.75 + pitch * 0.5;
        for c in self.callers.iter_mut().take(n) {
            c.phase += calls * c.period * bt * self.rng.range(0.5, 1.5);
            if c.phase < 1.0 {
                continue;
            }
            c.phase -= 1.0;
            match kind {
                // A peeper's rising whistle.
                0 => self.score.add(Note::new(0.0, 2500.0 * pf * c.pitch, 3200.0 * pf * c.pitch, 0.12, c.gain * 0.25, c.pan)),
                // A bullfrog's "jug-o-rum": a low, pulsed voice.
                1 => {
                    if let Some((voice, t)) = self.voices.iter_mut().find(|(vo, _)| !vo.on && vo.env < 0.001) {
                        voice.pitch = 90.0 * pf * c.pitch;
                        voice.target_pitch = voice.pitch * 0.85;
                        voice.vowel = [280.0, 700.0, 2000.0];
                        voice.target_vowel = [250.0, 600.0, 1800.0];
                        voice.breath = 0.15;
                        voice.gain = c.gain * 0.9;
                        voice.pan = c.pan;
                        voice.on = true;
                        voice.glide = 2.0;
                        *t = self.rng.range(0.5, 0.9);
                    }
                }
                // A tree frog's double croak.
                2 => {
                    for k in 0..2 {
                        let mut note = Note::new(k as f32 * 0.14, 1700.0 * pf * c.pitch, 1500.0 * pf * c.pitch, 0.08, c.gain * 0.2, c.pan);
                        note.pulses = 60.0;
                        note.harm = 0.6;
                        self.score.add(note);
                    }
                }
                // A toad's long trill.
                _ => {
                    let mut note = Note::new(0.0, 1300.0 * pf * c.pitch, 1250.0 * pf * c.pitch, self.rng.range(2.0, 6.0), c.gain * 0.12, c.pan);
                    note.pulses = 20.0;
                    self.score.add(note);
                }
            }
        }
        let dt = 1.0 / rate;
        let mut s = Block::new();
        for i in 0..BLOCK {
            let mut x = self.score.tick(rate);
            for (voice, t) in &mut self.voices {
                if voice.on {
                    *t -= dt;
                    if *t <= 0.0 {
                        voice.on = false;
                    }
                }
                // Three pulses a second through the croak.
                let y = voice.tick(&mut self.rng, rate) * (0.6 + 0.4 * sin1(*t * 3.0));
                let p = pan(y, voice.pan);
                x.0 += p.0;
                x.1 += p.1;
            }
            s.add(i, x);
        }
        add_filtered(&mut s, &mut self.lp, g, out);
    }
}

/// One insect flying about.
#[derive(Clone, Copy, Default)]
struct Flier {
    phase: f32,
    freq: f32,
    wander: f32,
    t: f32,
    path: f32,
}

pub struct Insects {
    rate: f32,
    fliers: Vec<Flier>,
    paths: Vec<Drift>,
    band: [Svf; 2],
    hive: [ColorNoise; 2],
    hive_bp: [Svf; 2],
}

impl Insects {
    pub fn new(rate: f32, seed: u32) -> Insects {
        let mut rng = Rng::new(seed);
        Insects {
            rate,
            fliers: (0..16).map(|_| Flier { phase: rng.uni(), freq: rng.range(0.9, 1.1), wander: rng.uni() * 50.0, t: 0.0, path: rng.uni() * 100.0 }).collect(),
            paths: (0..16).map(|k| Drift::new(seed + k * 13)).collect(),
            band: [Svf::new(800.0, 1.0, rate); 2],
            hive: [ColorNoise::new(Color::Pink, seed, rate), ColorNoise::new(Color::Pink, seed ^ 5, rate)],
            hive_bp: [Svf::new(230.0, 4.0, rate); 2],
        }
    }
}

impl Gen for Insects {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [kind, count, close, pitch, ..] = *v;
        let kind = pick(kind).min(4);
        let rate = self.rate;
        let base = [190.0, 600.0, 230.0, 210.0, 230.0][kind] * (0.75 + pitch * 0.5);
        let n = match kind {
            3 => 6 + (count * 10.0) as usize,
            4 => 0,
            _ => 1 + (count * 5.0) as usize,
        }
        .min(16);
        let bt = BLOCK as f32 / rate;
        for f in &mut self.band {
            f.set(base * 3.0, 0.8, rate);
        }
        for f in &mut self.hive_bp {
            f.set(base, 3.0, rate);
        }
        // Each flier's path: how near it is, where it is, how fast its wings go.
        let mut state = [(0.0f32, 0.0f32, 0.0f32); 16];
        for k in 0..n {
            let d = self.paths[k].next(0.25, bt);
            let near = (0.5 + 0.5 * d).clamp(0.0, 1.0);
            self.fliers[k].path += bt * 0.3;
            let side = (self.fliers[k].path * 1.7 + self.fliers[k].wander).sin();
            state[k] = (near * near * (0.3 + close * 1.4), side, 1.0 + 0.06 * d);
        }
        let dt = 1.0 / rate;
        for i in 0..BLOCK {
            let mut x = (0.0, 0.0);
            for k in 0..n {
                let f = &mut self.fliers[k];
                let (g, p, bend) = state[k];
                f.phase = (f.phase + base * f.freq * bend * dt).fract();
                f.t += dt;
                // A buzzy wingbeat: a narrow pulse's harmonics.
                let w = if f.phase < 0.2 { 1.0 } else { -0.25 };
                let y = pan(w * g * 0.08, p);
                x.0 += y.0;
                x.1 += y.1;
            }
            let l = self.band[0].band(x.0) + x.0 * 0.3;
            let r = self.band[1].band(x.1) + x.1 * 0.3;
            let mut hive = (0.0, 0.0);
            if kind == 4 {
                let g = 0.3 + count * 0.7;
                hive = (self.hive_bp[0].band(self.hive[0].next() * 3.0) * g * (0.3 + close), self.hive_bp[1].band(self.hive[1].next() * 3.0) * g * (0.3 + close));
            }
            out.add(i, (l + hive.0, r + hive.1));
        }
    }
}

pub struct Birds {
    rate: f32,
    rng: Rng,
    score: Score,
    ticker: Ticker,
    pecks: Vec<Modal>,
    pecks_left: u32,
    peck_wait: f32,
    peck_pan: f32,
    /// Each bird kind's few signature songs for this instance, so a
    /// scene's birds repeat themselves as real ones do.
    motifs: Vec<Vec<(f32, f32, f32, f32)>>,
    lp: Lp2,
}

impl Birds {
    pub fn new(rate: f32, seed: u32) -> Birds {
        let mut rng = Rng::new(seed);
        let ticker = Ticker::primed(&mut rng, 0.5);
        let motifs = (0..6)
            .map(|_| {
                let notes = 3 + rng.below(7);
                let mut at = 0.0;
                (0..notes)
                    .map(|_| {
                        let (f, to, len) = (rng.range(2200.0, 6000.0), rng.range(0.7, 1.4), rng.range(0.04, 0.18));
                        let n = (at, f, to, len);
                        at += len + rng.range(0.02, 0.09);
                        n
                    })
                    .collect()
            })
            .collect();
        Birds { rate, rng, score: Score::new(40), ticker, pecks: vec![Modal::default(); 6], pecks_left: 0, peck_wait: 0.0, peck_pan: 0.0, motifs, lp: Lp2::new(rate) }
    }

    fn sing(&mut self, kind: usize, pitch: f32, variety: f32) {
        let r = &mut self.rng;
        let pf = 0.7 + pitch * 0.6;
        let p = r.bi() * 0.9;
        let g = r.range(0.25, 1.0);
        match kind {
            // Songbirds and the dawn chorus: a remembered motif, or now and then a new one.
            0 | 2 => {
                let m = if r.chance(variety * 0.4) { None } else { Some(r.below(self.motifs.len())) };
                let notes: Vec<(f32, f32, f32, f32)> = match m {
                    Some(m) => self.motifs[m].clone(),
                    None => {
                        let mut at = 0.0;
                        (0..(2 + r.below(8)))
                            .map(|_| {
                                let n = (at, r.range(2000.0, 6500.0), r.range(0.6, 1.5), r.range(0.03, 0.2));
                                at += n.3 + r.range(0.02, 0.08);
                                n
                            })
                            .collect()
                    }
                };
                let shift = r.range(0.9, 1.1) * pf;
                for (at, f, to, len) in notes {
                    let mut n = Note::new(at, f * shift, f * to * shift, len, g * 0.2, p);
                    n.vib = (r.range(0.0, 0.05), r.range(20.0, 60.0));
                    self.score.add(n);
                }
            }
            // A sparrow's chips and trill.
            1 => {
                let f = r.range(3500.0, 5500.0) * pf;
                let count = 4 + r.below(10);
                let gap = r.range(0.05, 0.11);
                for k in 0..count {
                    self.score.add(Note::new(k as f32 * gap, f * 1.15, f * 0.85, gap * 0.6, g * 0.15, p));
                }
            }
            // Gulls: "kee-ow", rich and falling, repeated.
            3 => {
                for k in 0..(1 + r.below(4)) {
                    let mut n = Note::new(k as f32 * 0.45, 1900.0 * pf, 1100.0 * pf, r.range(0.25, 0.4), g * 0.25, p);
                    n.harm = 0.8;
                    n.rough = 0.25;
                    self.score.add(n);
                }
            }
            // Crows: a harsh "caw", two to four times.
            4 => {
                for k in 0..(2 + r.below(3)) {
                    let mut n = Note::new(k as f32 * 0.38, 650.0 * pf, 520.0 * pf, 0.24, g * 0.3, p);
                    n.harm = 1.0;
                    n.rough = 0.85;
                    self.score.add(n);
                }
            }
            // Owls: soft low hoots.
            5 => {
                let f = r.range(330.0, 420.0) * pf;
                let pattern: &[(f32, f32)] = if r.chance(0.5) { &[(0.0, 0.5), (0.7, 0.25), (1.05, 0.25), (1.4, 0.7)] } else { &[(0.0, 0.6), (0.9, 0.9)] };
                for &(at, len) in pattern {
                    self.score.add(Note::new(at, f, f * 0.96, len, g * 0.3, p));
                }
            }
            // Doves: "coo-oo".
            6 => {
                let f = r.range(450.0, 560.0) * pf;
                for &(at, len, a, b) in &[(0.0, 0.35, 1.0, 1.08), (0.45, 0.6, 1.08, 0.95), (1.2, 0.35, 1.0, 0.97), (1.65, 0.35, 0.97, 0.95)] {
                    let mut n = Note::new(at, f * a, f * b, len, g * 0.25, p);
                    n.harm = 0.15;
                    self.score.add(n);
                }
            }
            // Tropical whistles: wide glides with vibrato.
            7 => {
                let mut at = 0.0;
                for _ in 0..(2 + r.below(5)) {
                    let (f, to, len) = (r.range(900.0, 4000.0) * pf, r.range(0.5, 2.0), r.range(0.1, 0.5));
                    let mut n = Note::new(at, f, f * to, len, g * 0.2, p);
                    n.vib = (r.range(0.0, 0.08), r.range(5.0, 30.0));
                    n.harm = r.range(0.0, 0.5);
                    self.score.add(n);
                    at += len + r.range(0.03, 0.2);
                }
            }
            // A woodpecker drums.
            8 => {
                self.pecks_left = 10 + r.below(20) as u32;
                self.peck_pan = p;
            }
            // Loons: a long wail.
            9 => {
                let f = r.range(700.0, 900.0) * pf;
                let mut n = Note::new(0.0, f, f * 1.45, 1.4, g * 0.25, p);
                n.vib = (0.01, 5.0);
                self.score.add(n);
                let mut n = Note::new(1.4, f * 1.45, f * 1.1, 1.2, g * 0.25, p);
                n.vib = (0.015, 5.0);
                self.score.add(n);
            }
            // Parrots squawk.
            10 => {
                for k in 0..(1 + r.below(3)) {
                    let mut n = Note::new(k as f32 * 0.3, r.range(1200.0, 2200.0) * pf, r.range(900.0, 2500.0) * pf, 0.2, g * 0.3, p);
                    n.harm = 1.0;
                    n.rough = 0.7;
                    self.score.add(n);
                }
            }
            // A hawk's descending scream.
            _ => {
                let mut n = Note::new(0.0, 2800.0 * pf, 1900.0 * pf, 1.3, g * 0.25, p);
                n.harm = 0.6;
                n.rough = 0.4;
                n.vib = (0.02, 12.0);
                self.score.add(n);
            }
        }
    }
}

impl Gen for Birds {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [kind, density, pitch, variety, distance, _] = *v;
        let kind = pick(kind).min(11);
        let rate = self.rate;
        let (g, cut) = far(distance);
        self.lp.set(cut, rate);
        let dt = 1.0 / rate;
        // A dawn chorus is everyone at once.
        let density = if kind == 2 { density * 3.0 } else { density };
        let mut s = Block::new();
        for i in 0..BLOCK {
            if self.ticker.fire(&mut self.rng, density, 0.0, dt) {
                // In a dawn chorus other species join the songbirds.
                let k = if kind == 2 && self.rng.chance(variety * 0.5) { [1, 6, 7, 4][self.rng.below(4)] } else { kind };
                self.sing(k, pitch, variety);
            }
            if self.pecks_left > 0 {
                self.peck_wait -= dt;
                if self.peck_wait <= 0.0 {
                    self.pecks_left -= 1;
                    self.peck_wait = 1.0 / self.rng.range(16.0, 22.0);
                    let p = self.peck_pan;
                    if let Some(m) = free(&mut self.pecks) {
                        m.start(1100.0, &[(1.0, 0.02, 1.0), (2.3, 0.01, 0.5), (4.1, 0.008, 0.3)], 1.0, 0.25, p, rate);
                    }
                }
            }
            let mut x = self.score.tick(rate);
            for m in &mut self.pecks {
                let y = m.tick(&mut self.rng);
                x.0 += y.0;
                x.1 += y.1;
            }
            s.add(i, x);
        }
        add_filtered(&mut s, &mut self.lp, g, out);
    }
}

/// A voiced call's shape: pitch keyframes (when, Hz), the vowel
/// (index into VOWELS) at its start and end, its length, breathiness,
/// vibrato, and how many times it's repeated with what gap.
struct CallShape {
    pitch: &'static [(f32, f32)],
    vowels: (usize, usize),
    len: (f32, f32),
    breath: f32,
    vibrato: (f32, f32),
    repeats: (usize, usize),
    gap: f32,
}

const CALLS: [CallShape; 15] = [
    // Wolf: a long rising and falling howl.
    CallShape { pitch: &[(0.0, 300.0), (0.15, 460.0), (0.7, 430.0), (1.0, 290.0)], vowels: (4, 3), len: (2.5, 5.0), breath: 0.1, vibrato: (0.01, 5.0), repeats: (1, 1), gap: 0.0 },
    // Coyote: yips and a quavering howl.
    CallShape { pitch: &[(0.0, 700.0), (0.5, 1100.0), (1.0, 800.0)], vowels: (0, 4), len: (0.12, 0.25), breath: 0.15, vibrato: (0.04, 9.0), repeats: (4, 10), gap: 0.14 },
    // Dog: a bark, falling.
    CallShape { pitch: &[(0.0, 520.0), (0.3, 620.0), (1.0, 380.0)], vowels: (0, 3), len: (0.12, 0.2), breath: 0.35, vibrato: (0.0, 0.0), repeats: (1, 4), gap: 0.35 },
    // Whale: slow, wide glides.
    CallShape { pitch: &[(0.0, 180.0), (0.4, 520.0), (0.8, 260.0), (1.0, 140.0)], vowels: (4, 3), len: (2.5, 6.0), breath: 0.05, vibrato: (0.005, 3.0), repeats: (1, 2), gap: 1.0 },
    // Cow: "moo".
    CallShape { pitch: &[(0.0, 120.0), (0.3, 150.0), (1.0, 105.0)], vowels: (4, 0), len: (1.2, 2.0), breath: 0.15, vibrato: (0.0, 0.0), repeats: (1, 1), gap: 0.0 },
    // Sheep: a bleat, its vibrato wide.
    CallShape { pitch: &[(0.0, 260.0), (0.2, 300.0), (1.0, 240.0)], vowels: (1, 0), len: (0.6, 1.0), breath: 0.2, vibrato: (0.06, 7.0), repeats: (1, 2), gap: 0.5 },
    // Goat: a higher, harsher bleat.
    CallShape { pitch: &[(0.0, 420.0), (0.2, 480.0), (1.0, 380.0)], vowels: (1, 0), len: (0.5, 0.9), breath: 0.3, vibrato: (0.08, 9.0), repeats: (1, 2), gap: 0.4 },
    // Horse: a whinny, falling and shaking.
    CallShape { pitch: &[(0.0, 900.0), (0.2, 1250.0), (1.0, 500.0)], vowels: (2, 0), len: (1.0, 1.6), breath: 0.25, vibrato: (0.08, 14.0), repeats: (1, 1), gap: 0.0 },
    // Monkey: a run of rising hoots.
    CallShape { pitch: &[(0.0, 350.0), (1.0, 800.0)], vowels: (4, 0), len: (0.2, 0.35), breath: 0.25, vibrato: (0.0, 0.0), repeats: (4, 9), gap: 0.1 },
    // Elephant: a trumpet.
    CallShape { pitch: &[(0.0, 450.0), (0.3, 650.0), (1.0, 520.0)], vowels: (0, 1), len: (0.8, 1.4), breath: 0.45, vibrato: (0.03, 18.0), repeats: (1, 1), gap: 0.0 },
    // Lion: a roar, then grunts.
    CallShape { pitch: &[(0.0, 140.0), (0.3, 210.0), (1.0, 110.0)], vowels: (0, 3), len: (1.2, 2.0), breath: 0.55, vibrato: (0.02, 6.0), repeats: (1, 4), gap: 0.6 },
    // Seal: barks.
    CallShape { pitch: &[(0.0, 600.0), (1.0, 480.0)], vowels: (0, 3), len: (0.2, 0.3), breath: 0.4, vibrato: (0.0, 0.0), repeats: (2, 6), gap: 0.25 },
    // Rooster: "cock-a-doodle-doo".
    CallShape { pitch: &[(0.0, 520.0), (0.2, 700.0), (0.6, 720.0), (1.0, 560.0)], vowels: (3, 4), len: (1.3, 1.8), breath: 0.3, vibrato: (0.02, 10.0), repeats: (1, 1), gap: 0.0 },
    // Pig: grunts.
    CallShape { pitch: &[(0.0, 220.0), (1.0, 180.0)], vowels: (3, 4), len: (0.12, 0.25), breath: 0.5, vibrato: (0.0, 0.0), repeats: (2, 5), gap: 0.2 },
    // Hyena: a rising "whoop".
    CallShape { pitch: &[(0.0, 300.0), (1.0, 950.0)], vowels: (4, 0), len: (0.6, 0.9), breath: 0.15, vibrato: (0.0, 0.0), repeats: (2, 5), gap: 0.5 },
];

/// One call being voiced.
#[derive(Clone, Copy, Default)]
struct Calling {
    voice: Voice,
    shape: usize,
    t: f32,
    len: f32,
    pitch: f32,
    repeats: usize,
    gap: f32,
    vib: f32,
}

pub struct Calls {
    rate: f32,
    rng: Rng,
    calls: [Calling; 4],
    ticker: Ticker,
    lp: Lp2,
}

impl Calls {
    pub fn new(rate: f32, seed: u32) -> Calls {
        let mut rng = Rng::new(seed);
        let ticker = Ticker::primed(&mut rng, 0.2);
        Calls { rate, rng, calls: [Calling::default(); 4], ticker, lp: Lp2::new(rate) }
    }
}

impl Gen for Calls {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [animal, per_sec, pitch, distance, ..] = *v;
        let animal = pick(animal).min(CALLS.len() - 1);
        let rate = self.rate;
        let (g, cut) = far(distance);
        self.lp.set(cut, rate);
        let dt = 1.0 / rate;
        let mut s = Block::new();
        for i in 0..BLOCK {
            if self.ticker.fire(&mut self.rng, per_sec, 0.0, dt) {
                let shape = &CALLS[animal];
                let r = &mut self.rng;
                if let Some(c) = self.calls.iter_mut().find(|c| c.repeats == 0 && c.voice.env < 0.001) {
                    c.shape = animal;
                    c.t = 0.0;
                    c.len = r.range(shape.len.0, shape.len.1);
                    c.pitch = (0.75 + pitch * 0.5) * r.range(0.92, 1.08);
                    c.repeats = shape.repeats.0 + r.below(shape.repeats.1 - shape.repeats.0 + 1);
                    c.gap = 0.0;
                    c.voice.pitch = shape.pitch[0].1 * c.pitch;
                    c.voice.vowel = VOWELS[shape.vowels.0];
                    c.voice.breath = shape.breath;
                    c.voice.gain = r.range(0.5, 1.0) * 0.5;
                    c.voice.pan = r.bi() * 0.8;
                    c.voice.glide = 400.0;
                    c.voice.on = true;
                }
            }
            let mut x = (0.0, 0.0);
            for c in self.calls.iter_mut() {
                if c.repeats > 0 {
                    let shape = &CALLS[c.shape];
                    if c.gap > 0.0 {
                        c.gap -= dt;
                        if c.gap <= 0.0 {
                            c.t = 0.0;
                            c.voice.on = true;
                        }
                    } else {
                        let u = (c.t / c.len).min(1.0);
                        let k = shape.pitch.iter().rposition(|&(at, _)| at <= u).unwrap_or(0);
                        let (a, b) = (shape.pitch[k], shape.pitch[(k + 1).min(shape.pitch.len() - 1)]);
                        let f = if b.0 > a.0 { a.1 + (b.1 - a.1) * (u - a.0) / (b.0 - a.0) } else { a.1 };
                        c.vib = (c.vib + shape.vibrato.1 * dt).fract();
                        c.voice.target_pitch = f * c.pitch * (1.0 + shape.vibrato.0 * sin1(c.vib));
                        let (va, vb) = (VOWELS[shape.vowels.0], VOWELS[shape.vowels.1]);
                        for j in 0..3 {
                            c.voice.target_vowel[j] = va[j] + (vb[j] - va[j]) * u;
                        }
                        c.t += dt;
                        if c.t >= c.len {
                            c.voice.on = false;
                            c.repeats -= 1;
                            if c.repeats > 0 {
                                c.gap = shape.gap * self.rng.range(0.7, 1.3);
                            }
                        }
                    }
                }
                let y = c.voice.tick(&mut self.rng, rate);
                let p = pan(y, c.voice.pan);
                x.0 += p.0;
                x.1 += p.1;
            }
            s.add(i, x);
        }
        add_filtered(&mut s, &mut self.lp, g, out);
    }
}
