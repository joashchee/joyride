// Copyright 2026 ansiapps. Neumetik BGN: proprietary, see the LICENSE at the repository root.

//! Machines and vehicles: hums from their harmonic series, engines from
//! a firing pulse train ringing an exhaust's resonances, tires and wind
//! from shaped noise, creaks from stick-slip friction, rattles from
//! PhISEM-style bursts of collisions, and anything passing by with its
//! Doppler shift worked out from where it is.

use super::super::dsp::{free, pan, sin1, Block, Burst, Drift, Live, Modal, Mode, OnePole, Rng, Svf, Ticker, BLOCK};
use super::super::noise::{Color, ColorNoise};
use super::{add_filtered, far, pick, tone_cut, Gen, Lp2};

fn pair(color: Color, seed: u32, rate: f32) -> [ColorNoise; 2] {
    [ColorNoise::new(color, seed, rate), ColorNoise::new(color, seed.wrapping_mul(2_654_435_761) ^ 0x33, rate)]
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

fn sum_modal(voices: &mut [Modal], rng: &mut Rng) -> (f32, f32) {
    let mut x = (0.0, 0.0);
    for m in voices.iter_mut() {
        let y = m.tick(rng);
        x.0 += y.0;
        x.1 += y.1;
    }
    x
}

pub struct Hum {
    rate: f32,
    phase: f32,
    whine: f32,
    buzz_noise: ColorNoise,
    buzz_hp: Svf,
    wobble: Drift,
}

impl Hum {
    pub fn new(rate: f32, seed: u32) -> Hum {
        Hum { rate, phase: 0.0, whine: 0.0, buzz_noise: ColorNoise::new(Color::White, seed, rate), buzz_hp: Svf::new(2500.0, 0.7, rate), wobble: Drift::new(seed) }
    }
}

impl Gen for Hum {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [mains, harmonics, buzz, wobble, whine, _] = *v;
        let f0 = if pick(mains) == 0 { 50.0 } else { 60.0 };
        let rate = self.rate;
        let w = self.wobble.next(0.3, BLOCK as f32 / rate);
        let amp = 1.0 + wobble * 0.4 * w;
        let f = f0 * (1.0 + wobble * 0.002 * w);
        // A transformer hums loudest at twice the mains, from magnetostriction.
        let mut a = [0.0f32; 10];
        for (k, x) in a.iter_mut().enumerate() {
            let n = k + 1;
            *x = match n {
                1 => 0.35,
                2 => 0.6,
                _ => 0.5 * harmonics.powf((n - 2) as f32 * 0.7) / n as f32,
            };
        }
        let dt = 1.0 / rate;
        for i in 0..BLOCK {
            self.phase = (self.phase + f * dt).fract();
            let mut y = 0.0;
            for (k, x) in a.iter().enumerate() {
                y += sin1(self.phase * (k + 1) as f32) * x;
            }
            // A tube's buzz: noise gated twice a cycle, on the peaks.
            let gate = sin1(self.phase).abs().powi(8);
            let b = self.buzz_hp.high(self.buzz_noise.next()) * gate * buzz * 1.2;
            self.whine = (self.whine + 10_000.0 * dt).fract();
            let wh = sin1(self.whine) * whine * 0.03;
            let x = (y * 0.3 * amp + b + wh) * 0.8;
            out.add(i, (x, x * 0.97));
        }
    }
}

pub struct Air {
    rate: f32,
    rng: Rng,
    n: [ColorNoise; 2],
    body: [Svf; 2],
    blade: f32,
    motor: f32,
    fans: [f32; 6],
    rattle: [Burst; 6],
    cycle: f32,
    turbulence: Drift,
}

impl Air {
    pub fn new(rate: f32, seed: u32) -> Air {
        let mut rng = Rng::new(seed);
        let cycle = rng.uni() * 40.0;
        Air { rate, rng, n: pair(Color::Pink, seed, rate), body: [Svf::new(800.0, 0.6, rate); 2], blade: 0.0, motor: 0.0, fans: [0.0; 6], rattle: [Burst::default(); 6], cycle, turbulence: Drift::new(seed) }
    }
}

impl Gen for Air {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [kind, airflow, blade, speed, rattle, _] = *v;
        let kind = pick(kind).min(4);
        let rate = self.rate;
        let bt = BLOCK as f32 / rate;
        let turb = 1.0 + 0.15 * self.turbulence.next(0.7, bt);
        // A fridge runs its compressor in cycles, half a minute on, then off.
        self.cycle = (self.cycle + bt) % 60.0;
        let on = if kind == 3 { if self.cycle < 32.0 { 1.0 } else { 0.0 } } else { 1.0 };
        let (cut, q, noise) = [(900.0, 0.5, 1.0), (1500.0, 0.6, 0.8), (3500.0, 0.5, 0.9), (400.0, 0.7, 0.2), (5000.0, 0.5, 1.6)][kind];
        for f in &mut self.body {
            f.set(cut * (0.6 + speed * 0.8), q, rate);
        }
        let rot = 8.0 + 32.0 * speed;
        let bpf = rot * 5.0;
        let dt = 1.0 / rate;
        let mut s = Block::new();
        for i in 0..BLOCK {
            self.blade = (self.blade + bpf * dt).fract();
            self.motor = (self.motor + [100.0, 120.0, 240.0, 50.0, 420.0][kind] * (0.7 + speed * 0.6) * dt).fract();
            // The air pulses as each blade passes.
            let pulse = 1.0 - blade * 0.5 * (1.0 + sin1(self.blade));
            let mut tone = sin1(self.blade) * blade * 0.12 + sin1(self.motor) * 0.03 * (0.3 + blade);
            if kind == 2 {
                // A server room's fans whine at their own speeds.
                for (k, f) in self.fans.iter_mut().enumerate() {
                    *f = (*f + (3100.0 + 437.0 * k as f32) * (0.8 + speed * 0.4) * dt).fract();
                    tone += sin1(*f) * 0.006;
                }
            }
            if kind == 3 {
                tone = sin1(self.motor) * 0.12 + sin1(self.motor * 2.0) * 0.06;
            }
            if rattle > 0.0 && self.rng.chance(rattle * 20.0 * dt * on) {
                let (f, p) = (self.rng.range(800.0, 2500.0), self.rng.bi() * 0.3);
                if let Some(b) = free(&mut self.rattle) {
                    b.start(f, 3.0, 0.0005, 0.02, 0.2 * rattle, p, rate);
                }
            }
            let mut x = [0.0; 2];
            for c in 0..2 {
                x[c] = self.body[c].low(self.n[c].next() * 2.0) * airflow * noise * pulse * turb;
            }
            let r = sum_bursts(&mut self.rattle, &mut self.rng, rate);
            s.add(i, ((x[0] + tone) * on + r.0, (x[1] + tone) * on + r.1));
        }
        for i in 0..BLOCK {
            out.l[i] += s.l[i] * 0.8;
            out.r[i] += s.r[i] * 0.8;
        }
    }
}

pub struct Machine {
    rate: f32,
    rng: Rng,
    phase: f32,
    whine: f32,
    modes: Vec<Modal>,
    bursts: Vec<Burst>,
    hiss: [ColorNoise; 2],
    hiss_hp: [Svf; 2],
    whoosh: [Svf; 2],
    queue: Vec<(f32, u8)>,
}

impl Machine {
    pub fn new(rate: f32, seed: u32) -> Machine {
        Machine {
            rate,
            rng: Rng::new(seed),
            phase: 0.0,
            whine: 0.0,
            modes: vec![Modal::default(); 12],
            bursts: vec![Burst::default(); 16],
            hiss: pair(Color::White, seed, rate),
            hiss_hp: [Svf::new(3000.0, 0.7, rate); 2],
            whoosh: [Svf::new(400.0, 0.8, rate); 2],
            queue: Vec::with_capacity(16),
        }
    }

    fn event(&mut self, what: u8, weight: f32, p: f32) {
        let rate = self.rate;
        let r = &mut self.rng;
        let heavy = 1.4 - weight * 0.8;
        match what {
            // A metal clank.
            0 => {
                if let Some(m) = free(&mut self.modes) {
                    m.start(r.range(300.0, 700.0) * heavy, &[(1.0, 0.3, 1.0), (1.59, 0.25, 0.7), (2.14, 0.2, 0.5), (2.65, 0.15, 0.3)], 1.0, 0.22, p, rate);
                }
            }
            // A heavy thump.
            1 => {
                let f = r.range(50.0, 90.0) * heavy;
                if let Some(b) = free(&mut self.bursts) {
                    b.start(f, 0.9, 0.002, 0.15, 1.1, p, rate);
                }
            }
            // A wooden clack.
            2 => {
                if let Some(m) = free(&mut self.modes) {
                    m.start(r.range(600.0, 1100.0) * heavy, &[(1.0, 0.03, 1.0), (2.4, 0.02, 0.5)], 1.0, 0.35, p, rate);
                }
            }
            // A small click.
            3 => {
                let f = r.range(2000.0, 5000.0);
                if let Some(b) = free(&mut self.bursts) {
                    b.start(f, 1.5, 0.0002, 0.003, 0.2, p, rate);
                }
            }
            // A pneumatic hiss.
            _ => {
                let f = r.range(3000.0, 6000.0);
                if let Some(b) = free(&mut self.bursts) {
                    b.start(f, 0.6, 0.01, 0.3, 0.25, p, rate);
                }
            }
        }
    }
}

impl Gen for Machine {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [kind, cycles, weight, whine, hiss, irregular] = *v;
        let kind = pick(kind).min(7);
        let rate = self.rate;
        let bt = BLOCK as f32 / rate;
        for c in 0..2 {
            self.hiss_hp[c].set(2500.0, 0.7, rate);
            self.whoosh[c].set(250.0 + 300.0 * (1.0 - weight), 0.9, rate);
        }
        self.phase += cycles * bt * (1.0 + irregular * 0.3 * self.rng.bi());
        if self.phase >= 1.0 {
            self.phase -= self.phase.floor();
            if !self.rng.chance(irregular * 0.3) {
                // Each kind's cycle: what sounds, and when in the cycle.
                let pattern: &[(f32, u8)] = match kind {
                    0 => &[(0.0, 0), (0.5, 3), (0.7, 0)],
                    1 => &[(0.0, 1), (0.02, 0), (0.4, 4)],
                    2 => &[(0.0, 2), (0.25, 2), (0.5, 1)],
                    3 => &[(0.0, 1)],
                    4 => &[(0.0, 3), (0.33, 3), (0.66, 3)],
                    5 => &[(0.0, 3)],
                    6 => &[(0.0, 2), (0.2, 2), (0.5, 4)],
                    _ => &[(0.0, 3), (0.5, 3)],
                };
                let period = 1.0 / cycles.max(0.01);
                for &(at, what) in pattern {
                    self.queue.push((at * period, what));
                }
            }
        }
        let dt = 1.0 / rate;
        let mut s = Block::new();
        for i in 0..BLOCK {
            let mut k = 0;
            while k < self.queue.len() {
                self.queue[k].0 -= dt;
                if self.queue[k].0 <= 0.0 {
                    let (_, what) = self.queue.swap_remove(k);
                    let p = self.rng.bi() * 0.3;
                    self.event(what, weight, p);
                } else {
                    k += 1;
                }
            }
            self.whine = (self.whine + (220.0 + 600.0 * (1.0 - weight)) * dt).fract();
            let wh = (sin1(self.whine) + 0.3 * sin1(self.whine * 2.0)) * whine * 0.06;
            // A pump's piston pushes air in and out.
            let pump = if kind == 3 { 0.5 + 0.5 * sin1(self.phase) } else { 0.3 };
            let mut x = [0.0; 2];
            for c in 0..2 {
                let n = self.hiss[c].next();
                x[c] = self.hiss_hp[c].high(n) * hiss * 0.3 + self.whoosh[c].low(n * 3.0) * pump * 0.3 * (0.4 + weight);
            }
            let a = sum_modal(&mut self.modes, &mut self.rng);
            let b = sum_bursts(&mut self.bursts, &mut self.rng, rate);
            s.add(i, (x[0] + wh + a.0 + b.0, x[1] + wh + a.1 + b.1));
        }
        for i in 0..BLOCK {
            out.l[i] += s.l[i];
            out.r[i] += s.r[i];
        }
    }
}

pub struct Engine {
    rate: f32,
    rng: Rng,
    phase: f32,
    cyl: u32,
    modes: [[Mode; 3]; 2],
    whine: f32,
    roar: [ColorNoise; 2],
    roar_lp: [Svf; 2],
    intake: [Svf; 2],
    knock: [Burst; 6],
    lp: Lp2,
    prop: f32,
}

impl Engine {
    pub fn new(rate: f32, seed: u32) -> Engine {
        Engine {
            rate,
            rng: Rng::new(seed),
            phase: 0.0,
            cyl: 0,
            modes: [[Mode::default(); 3]; 2],
            whine: 0.0,
            roar: pair(Color::Pink, seed, rate),
            roar_lp: [Svf::new(800.0, 0.7, rate); 2],
            intake: [Svf::new(1200.0, 1.5, rate); 2],
            knock: [Burst::default(); 6],
            lp: Lp2::new(rate),
            prop: 0.0,
        }
    }
}

impl Gen for Engine {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [kind, rpm, load, rough, inside, _] = *v;
        let kind = pick(kind).min(7);
        let rate = self.rate;
        // Firings a second, and the exhaust's resonances, by kind.
        let (rpm, fires_per_rev, res): (f32, f32, [f32; 3]) = match kind {
            0 => (rpm, 2.0, [170.0, 480.0, 1300.0]),
            1 => (rpm * 0.6, 3.0, [110.0, 330.0, 950.0]),
            2 => (rpm, 1.0, [240.0, 760.0, 2100.0]),
            3 => (rpm * 0.4, 4.5, [140.0, 420.0, 1100.0]),
            4 => (rpm, 0.0, [0.0; 3]),
            5 => (rpm * 0.12, 3.0, [55.0, 150.0, 400.0]),
            6 => (rpm, 0.0, [0.0; 3]),
            _ => (1800.0 + rpm * 0.05, 2.0, [190.0, 520.0, 1500.0]),
        };
        for c in 0..2 {
            for (k, m) in self.modes[c].iter_mut().enumerate() {
                m.set(res[k].max(30.0) * (1.0 + 0.01 * c as f32), [0.03, 0.015, 0.006][k], rate);
            }
            self.roar_lp[c].set(if kind == 4 { 600.0 + 3000.0 * load } else { 500.0 + 1500.0 * load }, 0.7, rate);
            self.intake[c].set(900.0 + 900.0 * load, 1.5, rate);
        }
        self.lp.set(if inside > 0.0 { 9000.0 * 0.08f32.powf(inside) } else { 18_000.0 }, rate);
        let fire = rpm / 60.0 * fires_per_rev;
        let dt = 1.0 / rate;
        let mut s = Block::new();
        for i in 0..BLOCK {
            let mut imp = 0.0;
            if fire > 0.0 {
                self.phase += fire * dt;
                // A V-twin's firings come unevenly: a long gap, a short one.
                let next = if kind == 2 && self.cyl % 2 == 1 { 0.6 } else if kind == 2 { 1.4 } else { 1.0 };
                if self.phase >= next {
                    self.phase -= next;
                    self.cyl = self.cyl.wrapping_add(1);
                    imp = 1.0 + rough * self.rng.bi() * 0.6;
                    if kind == 1 && self.rng.chance(0.5) {
                        let (f, p) = (self.rng.range(1500.0, 3500.0), self.rng.bi() * 0.1);
                        if let Some(b) = free(&mut self.knock) {
                            b.start(f, 1.5, 0.0002, 0.004, 0.25 * (0.3 + rough), p, rate);
                        }
                    }
                }
            }
            let mut x = [0.0; 2];
            for c in 0..2 {
                let n = self.roar[c].next();
                let mut y = 0.0;
                for (k, m) in self.modes[c].iter_mut().enumerate() {
                    y += m.tick(imp) * [1.0, 0.5, 0.25][k];
                }
                y *= 0.12 * (0.4 + load);
                // Intake and mechanical noise, louder under load.
                y += self.intake[c].band(n * 2.0) * (0.03 + 0.1 * load);
                match kind {
                    4 => {
                        y = self.roar_lp[c].low(n * 3.0) * (0.3 + 0.6 * load);
                    }
                    6 => {
                        // Steam: chuffs four times a wheel turn, over a hiss.
                        let chuff_rate = rpm / 600.0;
                        if c == 0 {
                            self.prop += chuff_rate * dt;
                        }
                        let u = self.prop.fract();
                        let env = (-u * 6.0).exp();
                        y = self.intake[c].band(n * 3.0) * env * (0.4 + load * 0.6) + n * 0.03;
                    }
                    _ => {}
                }
                x[c] = y;
            }
            if kind == 4 {
                self.whine = (self.whine + (1500.0 + rpm * 0.5) * dt).fract();
                let w = sin1(self.whine) * 0.05 * (0.5 + load);
                x[0] += w;
                x[1] += w;
            }
            if kind == 3 {
                // The propeller's beat.
                self.prop = (self.prop + rpm / 60.0 * 1.5 * dt).fract();
                let t = 1.0 - 0.3 * (0.5 + 0.5 * sin1(self.prop));
                x[0] *= t;
                x[1] *= t;
            }
            let k = sum_bursts(&mut self.knock, &mut self.rng, rate);
            s.add(i, (x[0] + k.0, x[1] + k.1));
        }
        let g = 1.0 + inside * 0.6;
        add_filtered(&mut s, &mut self.lp, g, out);
    }
}

pub struct Road {
    rate: f32,
    rng: Rng,
    n: [ColorNoise; 2],
    white: [ColorNoise; 2],
    band: [Svf; 2],
    low: [Svf; 2],
    spray: [Svf; 2],
    texture: [OnePole; 2],
    joint: f32,
    thump: [Burst; 4],
}

impl Road {
    pub fn new(rate: f32, seed: u32) -> Road {
        Road {
            rate,
            rng: Rng::new(seed),
            n: pair(Color::Pink, seed, rate),
            white: pair(Color::White, seed ^ 7, rate),
            band: [Svf::new(900.0, 0.7, rate); 2],
            low: [Svf::new(80.0, 0.7, rate); 2],
            spray: [Svf::new(4000.0, 0.7, rate); 2],
            texture: [OnePole::new(30.0, rate); 2],
            joint: 0.0,
            thump: [Burst::default(); 4],
        }
    }
}

impl Gen for Road {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [speed, wet, rough, inside, ..] = *v;
        let rate = self.rate;
        for c in 0..2 {
            let cut = (500.0 + 1200.0 * speed) * (1.0 - inside * 0.6);
            self.band[c].set(cut, 0.6, rate);
            self.low[c].set(50.0 + 60.0 * speed, 0.7, rate);
        }
        let dt = 1.0 / rate;
        for i in 0..BLOCK {
            // Expansion joints, every few seconds at speed.
            self.joint += speed * 0.6 * dt;
            if self.joint >= 1.0 {
                self.joint -= 1.0 + self.rng.uni();
                let f = self.rng.range(60.0, 110.0);
                if let Some(b) = free(&mut self.thump) {
                    b.start(f, 1.0, 0.003, 0.06, 0.6 * rough, 0.0, rate);
                }
                if let Some(b) = free(&mut self.thump) {
                    b.start(f, 1.0, 0.003, 0.06, 0.5 * rough, 0.0, rate);
                }
            }
            let mut x = [0.0; 2];
            for c in 0..2 {
                let n = self.n[c].next() * 2.0;
                let tex = 1.0 + rough * (self.texture[c].low(self.rng.bi() * 6.0));
                let tire = self.band[c].low(n) * tex * (0.2 + 0.6 * speed);
                let rumble = self.low[c].low(n * 2.0) * (0.3 + inside * 0.8) * speed;
                let spray = self.spray[c].high(self.white[c].next()) * wet * speed * (0.7 - inside * 0.4);
                x[c] = tire + rumble + spray;
            }
            let t = sum_bursts(&mut self.thump, &mut self.rng, rate);
            out.add(i, (x[0] + t.0, x[1] + t.1));
        }
    }
}

/// Something passing by: a car, a plane, a train, a whoosh.
#[derive(Clone, Copy, Default)]
struct Pass {
    kind: usize,
    t: f32,
    len: f32,
    speed: f32,
    lateral: f32,
    dir: f32,
    gain: f32,
    phase: f32,
    phase2: f32,
    pitch: f32,
    body: [Svf; 2],
    noise: u32,
}

impl Live for Pass {
    fn live(&self) -> bool {
        self.t < self.len
    }
}

/// Speed (m/s), closest distance (m), source pitch (Hz) and loudness for each kind.
const PASSES: [(f32, f32, f32, f32); 12] = [
    (16.0, 8.0, 85.0, 1.0),
    (14.0, 9.0, 48.0, 1.5),
    (20.0, 8.0, 170.0, 1.1),
    (180.0, 300.0, 0.0, 2.5),
    (55.0, 150.0, 110.0, 1.6),
    (40.0, 120.0, 18.0, 1.8),
    (28.0, 30.0, 0.0, 2.0),
    (14.0, 2.0, 0.0, 0.8),
    (140.0, 40.0, 70.0, 1.6),
    (40.0, 25.0, 520.0, 0.8),
    (6.0, 3.0, 0.0, 0.4),
    (3.0, 4.0, 0.0, 0.6),
];

impl Pass {
    fn start(&mut self, kind: usize, distance: f32, speed: f32, rng: &mut Rng, rate: f32) {
        let (v, d, pitch, g) = PASSES[kind];
        self.kind = kind;
        self.speed = v * (0.6 + speed * 0.8) * rng.range(0.85, 1.15);
        self.lateral = d * (0.4 + distance * 2.5) * rng.range(0.8, 1.3);
        // Long enough to come from far off and go far away.
        let span = (self.lateral * 8.0).max(40.0).min(self.speed * 25.0);
        self.len = span * 2.0 / self.speed;
        self.t = 0.0;
        self.dir = if rng.chance(0.5) { 1.0 } else { -1.0 };
        self.gain = g * rng.range(0.6, 1.0);
        self.pitch = pitch * rng.range(0.85, 1.15);
        self.phase = 0.0;
        self.phase2 = rng.uni();
        self.body = [Svf::new(800.0, 0.7, rate); 2];
        self.noise = rng.next_u32() | 1;
    }

    #[inline]
    fn noise(&mut self) -> f32 {
        self.noise ^= self.noise << 13;
        self.noise ^= self.noise >> 17;
        self.noise ^= self.noise << 5;
        (self.noise >> 8) as f32 / (1u32 << 23) as f32 - 1.0
    }

    /// Where it is now: Doppler factor, nearness (1 at its closest), pan.
    fn place(&self) -> (f32, f32, f32) {
        let x = self.dir * self.speed * (self.t - self.len * 0.5);
        let r = (x * x + self.lateral * self.lateral).sqrt();
        // How fast the distance is changing, and so the pitch (a slower
        // sound speed for spacecraft, so their shift is heard).
        let c = if self.kind == 8 { 150.0 } else { 343.0 };
        let dr = x * self.dir * self.speed / r;
        let doppler = c / (c + dr).max(c * 0.3);
        (doppler, self.lateral / r, x / r)
    }

    /// Retunes the body filter once a block.
    fn retune(&mut self, rate: f32) {
        let (doppler, near, _) = self.place();
        let cut = (400.0 + 9000.0 * near.powf(1.3)) * doppler;
        let center = match self.kind {
            7 => 600.0 * doppler * doppler + 2000.0 * near,
            9 => 1500.0 * doppler,
            _ => cut,
        };
        for f in &mut self.body {
            f.set(center, if self.kind == 7 { 1.2 } else { 0.6 }, rate);
        }
    }

    #[inline]
    fn tick(&mut self, rate: f32) -> (f32, f32) {
        if self.t >= self.len {
            return (0.0, 0.0);
        }
        let dt = 1.0 / rate;
        let (doppler, near, side) = self.place();
        let n = self.noise();
        let f = self.pitch * doppler;
        self.phase += f * dt;
        let pulse = if self.phase >= 1.0 {
            self.phase -= 1.0;
            1.0
        } else {
            0.0
        };
        let src = match self.kind {
            // Engines: a firing pulse over tire noise.
            0 | 1 | 2 | 4 => pulse * 3.0 + n * 0.5,
            // A jet: all roar.
            3 => n * 1.5,
            // A helicopter's rotor: a thwop per blade.
            5 => n * (0.2 + (1.0 - self.phase).powi(6) * 2.5),
            // A train: rumble, and wheels on the joints.
            6 => {
                self.phase2 += self.speed / 25.0 * dt;
                let clack = if self.phase2.fract() < 0.02 { 1.5 } else { 0.0 };
                n * 1.0 + clack * n
            }
            7 => n,
            8 => (self.phase * 2.0 - 1.0) * 0.6 + n * 0.6,
            9 => sin1(self.phase) * 0.5 + n * 0.3,
            // A bicycle's freewheel ticks.
            10 => {
                self.phase2 += 18.0 * dt;
                n * 0.3 + if self.phase2.fract() < 0.03 { n * 2.0 } else { 0.0 }
            }
            // Hooves: four beats a stride.
            _ => {
                self.phase2 += 4.0 * dt;
                let u = self.phase2.fract();
                n * 0.2 + if u < 0.05 { n * 3.0 * (1.0 - u * 20.0) } else { 0.0 }
            }
        };
        let fade = (self.t / self.len * std::f32::consts::PI).sin().max(0.0);
        let y = src * near * self.gain * fade * 0.2;
        self.t += dt;
        let (l, r) = pan(y, side * 0.9);
        (self.body[0].low(l), self.body[1].low(r))
    }
}

pub struct Traffic {
    rate: f32,
    rng: Rng,
    passes: [Pass; 10],
    ticker: Ticker,
    drone: [ColorNoise; 2],
    drone_lp: [Svf; 2],
    spray: [Svf; 2],
    swell: Drift,
}

impl Traffic {
    pub fn new(rate: f32, seed: u32) -> Traffic {
        Traffic {
            rate,
            rng: Rng::new(seed),
            passes: [Pass::default(); 10],
            ticker: Ticker::new(),
            drone: pair(Color::Pink, seed, rate),
            drone_lp: [Svf::new(500.0, 0.7, rate); 2],
            spray: [Svf::new(3500.0, 0.7, rate); 2],
            swell: Drift::new(seed),
        }
    }
}

impl Gen for Traffic {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [density, distance, speed, wet, heavy, _] = *v;
        let rate = self.rate;
        let bt = BLOCK as f32 / rate;
        let (g, cut) = far(distance);
        for c in 0..2 {
            self.drone_lp[c].set((cut * 0.08).clamp(150.0, 1200.0), 0.7, rate);
        }
        for p in self.passes.iter_mut().filter(|p| p.live()) {
            p.retune(rate);
        }
        // Passing cars: how many a second, more when it's busy.
        let cars = 0.05 + density * 0.6;
        if self.ticker.fire(&mut self.rng, cars, 0.2, bt) {
            let kind = if self.rng.chance(heavy * 0.4) { 1 } else if self.rng.chance(0.08) { 2 } else { 0 };
            let (d, s) = ((distance + self.rng.range(-0.1, 0.3)).clamp(0.0, 1.0), speed);
            if let Some(p) = free(&mut self.passes) {
                p.start(kind, d, s, &mut self.rng, rate);
            }
        }
        let swell = 1.0 + 0.25 * self.swell.next(0.1, bt);
        let drone = (0.2 + density * 0.8) * swell * (0.4 + distance * 0.6);
        for i in 0..BLOCK {
            let mut x = (0.0, 0.0);
            for p in &mut self.passes {
                let y = p.tick(rate);
                x.0 += y.0 * g;
                x.1 += y.1 * g;
            }
            for c in 0..2 {
                let n = self.drone[c].next() * 2.0;
                let y = self.drone_lp[c].low(n) * drone * 0.5 + self.spray[c].high(n) * wet * 0.15 * g;
                if c == 0 {
                    x.0 += y;
                } else {
                    x.1 += y;
                }
            }
            out.add(i, x);
        }
    }
}

pub struct Rail {
    rate: f32,
    rng: Rng,
    rumble: [ColorNoise; 2],
    rumble_lp: [Svf; 2],
    car: f32,
    axles: usize,
    clacks: Vec<Modal>,
    squeal: f32,
    squeal_env: Drift,
    motor: f32,
    lp: Lp2,
}

impl Rail {
    pub fn new(rate: f32, seed: u32) -> Rail {
        Rail {
            rate,
            rng: Rng::new(seed),
            rumble: pair(Color::Brown, seed, rate),
            rumble_lp: [Svf::new(150.0, 0.7, rate); 2],
            car: 0.0,
            axles: 0,
            clacks: vec![Modal::default(); 8],
            squeal: 0.0,
            squeal_env: Drift::new(seed),
            motor: 0.0,
            lp: Lp2::new(rate),
        }
    }
}

impl Gen for Rail {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [kind, speed, clacks, squeal, inside, _] = *v;
        let kind = pick(kind).min(5);
        let rate = self.rate;
        let bt = BLOCK as f32 / rate;
        let mps = [8.0 + 25.0 * speed, 5.0 + 18.0 * speed, 6.0 + 20.0 * speed, 5.0 + 15.0 * speed, 3.0 + 10.0 * speed, 30.0 + 50.0 * speed][kind];
        self.lp.set(if inside > 0.0 { 12_000.0 * 0.06f32.powf(inside) } else { 16_000.0 }, rate);
        for f in &mut self.rumble_lp {
            f.set(80.0 + 4.0 * mps, 0.7, rate);
        }
        // A car is 25 m long; its four axles cross each rail joint in turn.
        const AXLES: [f32; 4] = [0.0, 2.5, 17.5, 20.0];
        let dt = 1.0 / rate;
        let car_time = 25.0 / mps;
        let sq = (0.5 + 0.5 * self.squeal_env.next(0.08, bt)).powi(4) * squeal;
        // High-speed track is welded: hardly a joint to hear.
        let clacks = clacks * if kind == 5 { 0.15 } else { 1.0 };
        let mut s = Block::new();
        for i in 0..BLOCK {
            self.car += dt / car_time;
            if self.car >= 1.0 {
                self.car -= 1.0;
                self.axles = 0;
            }
            while self.axles < 4 && self.car * 25.0 >= AXLES[self.axles] {
                self.axles += 1;
                let r = &mut self.rng;
                let heavy = if kind == 1 { 0.7 } else { 1.0 };
                if let Some(m) = free(&mut self.clacks) {
                    m.start(r.range(250.0, 500.0) * heavy, &[(1.0, 0.08, 1.0), (2.3, 0.05, 0.6), (3.7, 0.03, 0.4), (6.1, 0.02, 0.2)], 1.0, 0.25 * clacks * r.range(0.7, 1.0), r.bi() * 0.2, rate);
                }
            }
            self.squeal = (self.squeal + 3200.0 * dt).fract();
            self.motor = (self.motor + (mps * 12.0) * dt).fract();
            let wh = if kind == 2 || kind == 4 || kind == 5 { sin1(self.motor) * 0.04 } else { 0.0 };
            let sqv = (sin1(self.squeal) + 0.4 * sin1(self.squeal * 1.51)) * sq * 0.1;
            let mut x = [0.0; 2];
            for c in 0..2 {
                x[c] = self.rumble_lp[c].low(self.rumble[c].next() * 3.0) * (0.3 + speed * 0.5) * (0.6 + inside * 0.6);
            }
            let k = sum_modal(&mut self.clacks, &mut self.rng);
            s.add(i, (x[0] + k.0 + sqv + wh, x[1] + k.1 + sqv + wh));
        }
        add_filtered(&mut s, &mut self.lp, 1.0, out);
    }
}

pub struct Aircraft {
    rate: f32,
    rng: Rng,
    n: [ColorNoise; 2],
    body: [Svf; 2],
    low: [Svf; 2],
    phase: f32,
    whine: f32,
    burner: f32,
    burner_wait: f32,
    lp: Lp2,
}

impl Aircraft {
    pub fn new(rate: f32, seed: u32) -> Aircraft {
        let mut rng = Rng::new(seed);
        let burner_wait = rng.range(2.0, 10.0);
        Aircraft { rate, rng, n: pair(Color::Pink, seed, rate), body: [Svf::new(800.0, 0.6, rate); 2], low: [Svf::new(150.0, 0.8, rate); 2], phase: 0.0, whine: 0.0, burner: 0.0, burner_wait, lp: Lp2::new(rate) }
    }
}

impl Gen for Aircraft {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [kind, power, inside, tone, ..] = *v;
        let kind = pick(kind).min(5);
        let rate = self.rate;
        let bt = BLOCK as f32 / rate;
        let cut = tone_cut(tone) * (1.0 - inside * 0.75);
        self.lp.set(cut, rate);
        let (body, low, noise, pulse_hz, whine_hz) = match kind {
            0 => (700.0, 140.0, 1.0, 0.0, 0.0),
            1 => (1200.0, 110.0, 0.6, 95.0, 0.0),
            2 => (900.0, 80.0, 0.5, 19.0, 1800.0),
            3 => (3000.0, 120.0, 0.9, 0.0, 4500.0),
            4 => (500.0, 70.0, 0.4, 22.0, 0.0),
            _ => (600.0, 90.0, 0.2, 0.0, 0.0),
        };
        for c in 0..2 {
            self.body[c].set(body * (0.7 + 0.6 * power), 0.6, rate);
            self.low[c].set(low, 0.8, rate);
        }
        // A hot air balloon's burner roars now and then.
        if kind == 5 {
            self.burner_wait -= bt;
            if self.burner_wait <= 0.0 {
                self.burner_wait = self.rng.range(8.0, 20.0);
                self.burner = self.rng.range(1.5, 4.0);
            }
            self.burner -= bt;
        }
        let burning = kind == 5 && self.burner > 0.0;
        let dt = 1.0 / rate;
        let mut s = Block::new();
        for i in 0..BLOCK {
            let mut mod_ = 1.0;
            if pulse_hz > 0.0 {
                self.phase = (self.phase + pulse_hz * (0.8 + 0.4 * power) * dt).fract();
                mod_ = if kind == 2 { 0.3 + (1.0 - self.phase).powi(5) * 2.0 } else { 0.6 + 0.4 * sin1(self.phase) };
            }
            let mut wh = 0.0;
            if whine_hz > 0.0 {
                self.whine = (self.whine + whine_hz * (0.9 + 0.2 * power) * dt).fract();
                wh = sin1(self.whine) * 0.02;
            }
            let mut x = [0.0; 2];
            for c in 0..2 {
                let n = self.n[c].next() * 2.0;
                let b = self.body[c].low(n) * noise * (0.3 + 0.7 * power);
                let l = self.low[c].low(n * 2.0) * (0.3 + inside * 0.6) * power;
                let burn = if burning { self.body[c].band(n) * 1.5 } else { 0.0 };
                x[c] = (b + l) * mod_ + burn;
            }
            s.add(i, (x[0] + wh, x[1] + wh));
        }
        add_filtered(&mut s, &mut self.lp, 0.8, out);
    }
}

/// One creak: stick-slip pulses at a wandering rate, ringing the
/// material's resonances.
#[derive(Clone, Copy, Default)]
struct Creak {
    t: f32,
    len: f32,
    phase: f32,
    from: f32,
    to: f32,
    modes: [Mode; 3],
    gain: f32,
    pan: f32,
}

impl Live for Creak {
    fn live(&self) -> bool {
        self.t < self.len
    }
}

pub struct Creaks {
    rate: f32,
    rng: Rng,
    creaks: [Creak; 4],
    ticker: Ticker,
    swing: f32,
}

impl Creaks {
    pub fn new(rate: f32, seed: u32) -> Creaks {
        let mut rng = Rng::new(seed);
        let ticker = Ticker::primed(&mut rng, 0.3);
        Creaks { rate, rng, creaks: [Creak::default(); 4], ticker, swing: 0.0 }
    }
}

impl Gen for Creaks {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [kind, per_sec, pitch, length, ..] = *v;
        let kind = pick(kind).min(6);
        let rate = self.rate;
        let dt = 1.0 / rate;
        // A swing and a rocking chair creak on their beat, not at random.
        let regular = if kind == 4 || kind == 6 { 0.9 } else { 0.0 };
        let pf = 0.6 + pitch * 0.8;
        for i in 0..BLOCK {
            if self.ticker.fire(&mut self.rng, per_sec, regular, dt) {
                let r = &mut self.rng;
                // Resonances, pulse rates and lengths by material.
                let (res, rates, len): ([f32; 3], (f32, f32), f32) = match kind {
                    0 => ([420.0, 900.0, 1700.0], (30.0, 120.0), 0.6),
                    1 => ([180.0, 420.0, 800.0], (15.0, 60.0), 1.2),
                    2 => ([900.0, 1800.0, 3200.0], (60.0, 200.0), 0.4),
                    3 => ([1100.0, 2300.0, 3800.0], (150.0, 600.0), 0.9),
                    4 => ([700.0, 1400.0, 2600.0], (80.0, 250.0), 0.5),
                    5 => ([110.0, 260.0, 470.0], (10.0, 40.0), 2.0),
                    _ => ([300.0, 700.0, 1300.0], (25.0, 70.0), 0.5),
                };
                if let Some(c) = free(&mut self.creaks) {
                    c.t = 0.0;
                    c.len = len * (0.4 + length * 1.4) * r.range(0.7, 1.3);
                    c.phase = 0.0;
                    let up = r.chance(0.5) || (kind == 4 && self.swing > 0.5);
                    let (a, b) = (r.range(rates.0, rates.1), r.range(rates.0, rates.1));
                    (c.from, c.to) = if up { (a.min(b), a.max(b)) } else { (a.max(b), a.min(b)) };
                    for (k, m) in c.modes.iter_mut().enumerate() {
                        m.set(res[k] * pf * r.range(0.9, 1.1), if kind == 5 { 0.08 } else { 0.02 }, rate);
                    }
                    c.gain = r.range(0.4, 0.9) * if kind == 5 { 0.6 } else { 0.35 };
                    c.pan = r.bi() * 0.7;
                }
                self.swing = 1.0 - self.swing;
            }
            let mut x = (0.0, 0.0);
            for c in self.creaks.iter_mut().filter(|c| c.live()) {
                let u = c.t / c.len;
                let rate_now = c.from + (c.to - c.from) * u + 10.0 * sin1(u * 3.0) * self.rng.uni();
                c.phase += rate_now * dt;
                let imp = if c.phase >= 1.0 {
                    c.phase -= 1.0;
                    self.rng.range(0.5, 1.0)
                } else {
                    0.0
                };
                let env = (u * 8.0).min(1.0) * (1.0 - u).powf(0.5);
                let mut y = 0.0;
                for (k, m) in c.modes.iter_mut().enumerate() {
                    y += m.tick(imp) * [1.0, 0.6, 0.3][k];
                }
                let p = pan(y * env * c.gain, c.pan);
                x.0 += p.0;
                x.1 += p.1;
                c.t += dt;
            }
            out.add(i, x);
        }
    }
}

pub struct Rattle {
    rate: f32,
    rng: Rng,
    bursts: Vec<Burst>,
    modes: Vec<Modal>,
    ticker: Ticker,
    shake: f32,
    energy: f32,
    pan: f32,
}

impl Rattle {
    pub fn new(rate: f32, seed: u32) -> Rattle {
        let mut rng = Rng::new(seed);
        let ticker = Ticker::primed(&mut rng, 0.3);
        Rattle { rate, rng, bursts: vec![Burst::default(); 32], modes: vec![Modal::default(); 12], ticker, shake: 0.0, energy: 0.0, pan: 0.0 }
    }
}

impl Gen for Rattle {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [kind, per_sec, intensity, size, ..] = *v;
        let kind = pick(kind).min(5);
        let rate = self.rate;
        let dt = 1.0 / rate;
        let sz = 1.3 - size * 0.6;
        for i in 0..BLOCK {
            // PhISEM: a shake puts energy in, and collisions happen at a rate
            // that follows the energy as it decays.
            if self.ticker.fire(&mut self.rng, per_sec, 0.0, dt) {
                self.shake = self.rng.range(0.3, 1.5) * (0.5 + intensity);
                self.energy = 1.0;
                self.pan = self.rng.bi() * 0.7;
            }
            if self.energy > 0.001 {
                self.energy *= (-dt / (0.1 + self.shake * 0.4)).exp();
                let collisions = 600.0 * self.energy * (0.3 + intensity);
                if self.rng.chance(collisions * dt) {
                    let r = &mut self.rng;
                    let g = self.energy * r.range(0.2, 1.0) * 0.5;
                    let p = self.pan + r.bi() * 0.3;
                    match kind {
                        // Chains jingle and shutters bang: modes.
                        1 | 2 | 5 => {
                            let (base, partials): (f32, &[(f32, f32, f32)]) = match kind {
                                1 => (r.range(2200.0, 4500.0) * sz, &[(1.0, 0.12, 1.0), (2.76, 0.06, 0.5), (5.4, 0.03, 0.3)]),
                                2 => (r.range(220.0, 450.0) * sz, &[(1.0, 0.06, 1.0), (2.3, 0.04, 0.5), (3.9, 0.02, 0.3)]),
                                _ => (r.range(900.0, 1800.0) * sz, &[(1.0, 0.025, 1.0), (2.4, 0.015, 0.5)]),
                            };
                            if let Some(m) = free(&mut self.modes) {
                                m.start(base, partials, 1.0, g * 0.6, p, rate);
                            }
                        }
                        // Gravel, debris, sand: band-passed ticks.
                        _ => {
                            let (lo, hi, d) = match kind {
                                0 => (800.0, 3500.0, 0.004),
                                3 => (200.0, 1500.0, 0.012),
                                _ => (3000.0, 9000.0, 0.0015),
                            };
                            let f = r.range(lo, hi) * sz;
                            if let Some(b) = free(&mut self.bursts) {
                                b.start(f, 1.2, 0.0002, d, g, p, rate);
                            }
                        }
                    }
                }
            }
            let a = sum_bursts(&mut self.bursts, &mut self.rng, rate);
            let b = sum_modal(&mut self.modes, &mut self.rng);
            out.add(i, (a.0 + b.0, a.1 + b.1));
        }
    }
}

pub struct Passby {
    rate: f32,
    rng: Rng,
    passes: [Pass; 4],
    ticker: Ticker,
}

impl Passby {
    pub fn new(rate: f32, seed: u32) -> Passby {
        let mut rng = Rng::new(seed);
        let ticker = Ticker::primed(&mut rng, 0.3);
        Passby { rate, rng, passes: [Pass::default(); 4], ticker }
    }
}

impl Gen for Passby {
    fn render(&mut self, v: &[f32; 6], out: &mut Block) {
        let [kind, per_sec, distance, speed, ..] = *v;
        let kind = pick(kind).min(PASSES.len() - 1);
        let rate = self.rate;
        let bt = BLOCK as f32 / rate;
        if self.ticker.fire(&mut self.rng, per_sec, 0.0, bt) {
            if let Some(p) = free(&mut self.passes) {
                p.start(kind, distance, speed, &mut self.rng, rate);
            }
        }
        for p in self.passes.iter_mut().filter(|p| p.live()) {
            p.retune(rate);
        }
        for i in 0..BLOCK {
            let mut x = (0.0, 0.0);
            for p in &mut self.passes {
                let y = p.tick(rate);
                x.0 += y.0;
                x.1 += y.1;
            }
            out.add(i, x);
        }
    }
}
