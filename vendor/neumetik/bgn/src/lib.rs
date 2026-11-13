// Copyright 2026 ansiapps. Neumetik BGN: proprietary, see the LICENSE at the repository root.

//! **BGN**, background noise: atmospheres for games and films made in
//! layers, the way a sound editor builds them, and synthesized whole, no
//! recordings.
//!
//! A `Scene` has four layers (`catalog::LAYERS`): the **bed** (the steady
//! floor: room tone, wind, hum), the **texture** (what gives the place its
//! character: rain, a crowd, machinery), the **details** (sparse one-shots:
//! a bird, a drip, a door) and the **movement** (things passing, swells).
//! Each layer holds any number of sources, each one of the generators in
//! `catalog.rs`, with its own level, pan, width and parameters, and the
//! layer can swell slowly on its own. Over the whole scene: where you hear
//! it from (`PERSPECTIVES`: in the open, through a wall, underwater, on a
//! phone...), how far away, the room's size and how much of it, a tilt
//! of tone and a width.
//!
//! `Engine` plays a scene in real time and takes changes to it as it
//! plays, keeping every source that didn't change. `render` makes a
//! finished take, optionally a seamless loop. `bank.rs` is the preset
//! bank: every entry of the background noise list, categorized and
//! tagged.

mod bank;
pub mod catalog;
mod dsp;
mod fft;
mod gens;
mod noise;

pub use bank::{BankPreset, SourceDef, BANK, CATEGORIES};
pub use catalog::{generator, Generator, Kind, Param, GENERATORS, GROUPS, LAYERS};

use dsp::{db, soft, Block, Drift, OnePole, Smooth, Smoothness, Svf, BLOCK};

/// Where a scene is heard from.
pub const PERSPECTIVES: [&str; 7] = ["In the open", "Through a wall", "Behind a door", "Underwater", "On a phone", "On a radio", "In a vehicle"];

#[derive(Clone, Debug, PartialEq)]
pub struct Source {
    /// A generator's ID (`catalog::GENERATORS`).
    pub gen: String,
    /// Decibels, -60 to +12.
    pub level: f32,
    /// -1 left to 1 right.
    pub pan: f32,
    /// 0 mono to 1 full stereo.
    pub width: f32,
    pub mute: bool,
    /// The generator's parameters in its catalog order: 0..1 for a range,
    /// the index for a choice.
    pub params: [f32; 6],
    /// What makes this source's randomness its own.
    pub seed: u32,
}

impl Source {
    /// A generator at its defaults.
    pub fn new(gen: &Generator, seed: u32) -> Source {
        Source { gen: gen.id.to_string(), level: -6.0, pan: 0.0, width: 1.0, mute: false, params: gen.defaults(), seed }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    /// Decibels, -60 to +12.
    pub level: f32,
    pub mute: bool,
    pub solo: bool,
    /// How much the layer swells and ebbs on its own, 0..1.
    pub swell: f32,
    /// How fast, in cycles a minute (about).
    pub swell_rate: f32,
    pub sources: Vec<Source>,
}

impl Default for Layer {
    fn default() -> Layer {
        Layer { level: 0.0, mute: false, solo: false, swell: 0.0, swell_rate: 4.0, sources: Vec::new() }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    pub layers: [Layer; 4],
    /// Decibels.
    pub master: f32,
    /// An index into `PERSPECTIVES`.
    pub perspective: usize,
    /// 0 here to 1 far off: quieter, duller, more room.
    pub distance: f32,
    /// How much of the room you hear, 0..1.
    pub space: f32,
    /// The room's size, 0 a closet to 1 a cathedral.
    pub size: f32,
    /// -1 dark to 1 bright.
    pub tone: f32,
    /// 0 mono to 1 as made, up to 1.5 wider.
    pub width: f32,
}

impl Default for Scene {
    fn default() -> Scene {
        Scene { layers: Default::default(), master: 0.0, perspective: 0, distance: 0.0, space: 0.2, size: 0.4, tone: 0.0, width: 1.0 }
    }
}

struct SourceState {
    gen_id: String,
    seed: u32,
    desc: &'static Generator,
    gen: Box<dyn gens::Gen>,
    p: [Smooth; 6],
    level: Smooth,
    pan: Smooth,
    width: Smooth,
    trim: f32,
}

impl SourceState {
    fn new(s: &Source, rate: f32) -> Option<SourceState> {
        let desc = generator(&s.gen)?;
        let gen = gens::make(&s.gen, rate, s.seed)?;
        Some(SourceState {
            gen_id: s.gen.clone(),
            seed: s.seed,
            desc,
            gen,
            p: s.params.map(|v| Smooth { value: v }),
            level: Smooth { value: if s.mute { 0.0 } else { db(s.level) } },
            pan: Smooth { value: s.pan },
            width: Smooth { value: s.width },
            trim: db(gens::trim(&s.gen)),
        })
    }
}

struct LayerState {
    sources: Vec<Option<SourceState>>,
    gain: Smooth,
    swell: Drift,
    peak: f32,
}

/// A feedback delay network: eight delay lines mixed through a Hadamard
/// matrix, each damped, decaying in the time the room's size sets.
struct Reverb {
    lines: Vec<Vec<f32>>,
    at: [usize; 8],
    len: [usize; 8],
    damp: [OnePole; 8],
    gain: [f32; 8],
    size: f32,
    rate: f32,
}

const FDN: [usize; 8] = [1031, 1327, 1523, 1871, 2087, 2399, 2711, 3049];

// The eight lines' state is indexed side by side.
#[allow(clippy::needless_range_loop)]
impl Reverb {
    fn new(rate: f32) -> Reverb {
        let scale = rate / 44_100.0;
        let lines = FDN.iter().map(|&n| vec![0.0; (n as f32 * scale * 1.6) as usize + 8]).collect();
        let mut r = Reverb { lines, at: [0; 8], len: [1; 8], damp: [OnePole::new(6000.0, rate); 8], gain: [0.0; 8], size: -1.0, rate };
        r.set(0.4);
        r
    }

    fn set(&mut self, size: f32) {
        if (size - self.size).abs() < 0.01 {
            return;
        }
        self.size = size;
        let scale = self.rate / 44_100.0 * (0.35 + size * 1.25);
        let t60 = 0.3 + size * size * 7.0;
        for k in 0..8 {
            self.len[k] = ((FDN[k] as f32 * scale) as usize).clamp(8, self.lines[k].len() - 1);
            self.gain[k] = 10f32.powf(-3.0 * self.len[k] as f32 / (t60 * self.rate));
            self.damp[k].set(9000.0 - size * 5000.0, self.rate);
        }
    }

    #[inline]
    fn tick(&mut self, l: f32, r: f32) -> (f32, f32) {
        let mut y = [0.0; 8];
        for k in 0..8 {
            let n = self.lines[k].len();
            let read = (self.at[k] + n - self.len[k]) % n;
            y[k] = self.damp[k].low(self.lines[k][read]) * self.gain[k];
        }
        let (ol, or) = (y[0] + y[2] + y[4] + y[6], y[1] + y[3] + y[5] + y[7]);
        // A fast Walsh-Hadamard transform, normalized.
        let mut h = 1;
        while h < 8 {
            for i in (0..8).step_by(h * 2) {
                for j in i..i + h {
                    let (a, b) = (y[j], y[j + h]);
                    y[j] = a + b;
                    y[j + h] = a - b;
                }
            }
            h *= 2;
        }
        let norm = 1.0 / 8f32.sqrt();
        for k in 0..8 {
            let input = if k % 2 == 0 { l } else { r };
            let n = self.lines[k].len();
            self.lines[k][self.at[k]] = y[k] * norm + input;
            self.at[k] = (self.at[k] + 1) % n;
        }
        (ol * 0.35, or * 0.35)
    }
}

/// What a perspective does: (high pass, low pass) cutoffs, a level, how
/// mono it goes, how hard it's driven.
fn perspective(p: usize) -> (f32, f32, f32, f32, f32) {
    match p {
        1 => (20.0, 420.0, db(-4.0), 0.6, 0.0),
        2 => (20.0, 1000.0, db(-3.0), 0.4, 0.0),
        3 => (20.0, 380.0, db(-2.0), 0.3, 0.0),
        4 => (380.0, 3300.0, db(-2.0), 1.0, 0.6),
        5 => (200.0, 5000.0, db(-2.0), 0.8, 0.3),
        6 => (25.0, 1300.0, db(-2.0), 0.2, 0.0),
        _ => (10.0, 20_000.0, 1.0, 0.0, 0.0),
    }
}

/// The scene's last stages: perspective, distance, tone, width, the room.
struct Post {
    hp: [[Svf; 2]; 2],
    lp: [[Svf; 2]; 2],
    dist: [Svf; 2],
    tilt: [OnePole; 2],
    reverb: Reverb,
    perspective: usize,
    wobble: Drift,
}

impl Post {
    fn new(rate: f32) -> Post {
        Post {
            hp: [[Svf::new(10.0, 0.707, rate); 2]; 2],
            lp: [[Svf::new(20_000.0, 0.707, rate); 2]; 2],
            dist: [Svf::new(20_000.0, 0.707, rate); 2],
            tilt: [OnePole::new(800.0, rate); 2],
            reverb: Reverb::new(rate),
            perspective: usize::MAX,
            wobble: Drift::new(17),
        }
    }
}

/// Plays a scene, a block at a time, and takes changes as it goes.
pub struct Engine {
    rate: f32,
    scene: Scene,
    layers: Vec<LayerState>,
    post: Post,
    master: Smooth,
    fade: Smooth,
    playing: bool,
    smooth: [Smooth; 6],
    out: Block,
    pos: usize,
    peak: f32,
    scratch: Block,
}

impl Engine {
    pub fn new(rate: u32) -> Engine {
        let rate = rate as f32;
        Engine {
            rate,
            scene: Scene::default(),
            layers: (0..4).map(|k| LayerState { sources: Vec::new(), gain: Smooth { value: 1.0 }, swell: Drift::of(Smoothness::Simplex, 101 + k), peak: 0.0 }).collect(),
            post: Post::new(rate),
            master: Smooth { value: 1.0 },
            fade: Smooth { value: 0.0 },
            playing: false,
            smooth: [Smooth { value: 0.0 }; 6],
            out: Block::new(),
            pos: BLOCK,
            peak: 0.0,
            scratch: Block::new(),
        }
    }

    pub fn rate(&self) -> u32 {
        self.rate as u32
    }

    /// Takes the scene to play. Sources whose generator and seed haven't
    /// changed keep playing (their settings glide to the new ones); the rest
    /// start afresh.
    pub fn set_scene(&mut self, scene: &Scene) {
        for (k, layer) in scene.layers.iter().enumerate() {
            let state = &mut self.layers[k];
            let mut next: Vec<Option<SourceState>> = Vec::with_capacity(layer.sources.len());
            let mut old: Vec<Option<SourceState>> = std::mem::take(&mut state.sources);
            for s in &layer.sources {
                let kept = old.iter_mut().find(|o| o.as_ref().is_some_and(|o| o.gen_id == s.gen && o.seed == s.seed)).and_then(Option::take);
                next.push(kept.or_else(|| SourceState::new(s, self.rate)));
            }
            state.sources = next;
        }
        // On the first scene, settings start where they are instead of gliding there.
        if self.scene.layers.iter().all(|l| l.sources.is_empty()) {
            self.smooth = [scene.distance, scene.space, scene.size, scene.tone, scene.width, 0.0].map(|value| Smooth { value });
        }
        self.scene = scene.clone();
    }

    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    /// Starts or stops, fading rather than cutting.
    pub fn set_playing(&mut self, playing: bool) {
        self.playing = playing;
    }

    pub fn playing(&self) -> bool {
        self.playing || self.fade.value > 0.0
    }

    /// Each layer's peak and the master's since the last call, 0..1 and up.
    pub fn take_meters(&mut self) -> [f32; 5] {
        let m = [self.layers[0].peak, self.layers[1].peak, self.layers[2].peak, self.layers[3].peak, self.peak];
        for l in &mut self.layers {
            l.peak = 0.0;
        }
        self.peak = 0.0;
        m
    }

    /// Fills both channels with the next samples.
    pub fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        let n = left.len().min(right.len());
        let mut i = 0;
        while i < n {
            if self.pos >= BLOCK {
                self.block();
                self.pos = 0;
            }
            let take = (BLOCK - self.pos).min(n - i);
            left[i..i + take].copy_from_slice(&self.out.l[self.pos..self.pos + take]);
            right[i..i + take].copy_from_slice(&self.out.r[self.pos..self.pos + take]);
            self.pos += take;
            i += take;
        }
    }

    fn block(&mut self) {
        let rate = self.rate;
        let bt = BLOCK as f32 / rate;
        self.out.clear();
        let fade = self.fade.step(if self.playing { 1.0 } else { 0.0 });
        if fade == 0.0 && !self.playing {
            return;
        }
        let scene = &self.scene;
        let soloing = scene.layers.iter().any(|l| l.solo);
        let mut bus = Block::new();
        for (k, layer) in scene.layers.iter().enumerate() {
            let state = &mut self.layers[k];
            let audible = !layer.mute && (!soloing || layer.solo);
            let swell = state.swell.next(layer.swell_rate / 60.0, bt);
            let target = if audible { db(layer.level) * (1.0 + layer.swell * 0.6 * swell).max(0.0) } else { 0.0 };
            let g = state.gain.step(target);
            let mut lb = Block::new();
            if g > 0.0 || target > 0.0 {
                for (src, def) in state.sources.iter_mut().zip(&layer.sources) {
                    let Some(src) = src else { continue };
                    let level = src.level.step(if def.mute { 0.0 } else { db(def.level) }) * src.trim;
                    let p = src.pan.step(def.pan);
                    let w = src.width.step(def.width);
                    let mut v = [0.0; 6];
                    for (j, q) in src.desc.params.iter().enumerate() {
                        let raw = if q.is_choice() {
                            src.p[j].value = def.params[j];
                            def.params[j]
                        } else {
                            src.p[j].step(def.params[j])
                        };
                        v[j] = q.value(raw);
                    }
                    if level < 1e-4 {
                        // Silent sources still run, so they come back in step.
                        self.scratch.clear();
                        src.gen.render(&v, &mut self.scratch);
                        continue;
                    }
                    self.scratch.clear();
                    src.gen.render(&v, &mut self.scratch);
                    // Width by mid and side, then a balance pan.
                    let (pl, pr) = (((1.0 - p) * 0.5).sqrt() * 1.414, ((1.0 + p) * 0.5).sqrt() * 1.414);
                    for i in 0..BLOCK {
                        let (l, r) = (self.scratch.l[i], self.scratch.r[i]);
                        let (m, s) = ((l + r) * 0.5, (l - r) * 0.5 * w);
                        lb.l[i] += (m + s) * pl.min(1.0) * level;
                        lb.r[i] += (m - s) * pr.min(1.0) * level;
                    }
                }
            }
            let mut peak = 0f32;
            for i in 0..BLOCK {
                let (l, r) = (lb.l[i] * g, lb.r[i] * g);
                peak = peak.max(l.abs()).max(r.abs());
                bus.l[i] += l;
                bus.r[i] += r;
            }
            state.peak = state.peak.max(peak);
        }
        self.post(&mut bus, fade);
        self.out = bus;
    }

    #[allow(clippy::needless_range_loop)]
    fn post(&mut self, bus: &mut Block, fade: f32) {
        let rate = self.rate;
        let scene = &self.scene;
        let distance = self.smooth[0].step(scene.distance);
        let space = self.smooth[1].step(scene.space);
        let size = self.smooth[2].step(scene.size);
        let tone = self.smooth[3].step(scene.tone);
        let width = self.smooth[4].step(scene.width);
        let post = &mut self.post;
        let (hp, lp, plevel, mono, drive) = perspective(scene.perspective);
        if post.perspective != scene.perspective {
            post.perspective = scene.perspective;
            for c in 0..2 {
                for f in &mut post.hp[c] {
                    f.set(hp, 0.707, rate);
                }
                for f in &mut post.lp[c] {
                    f.set(lp, 0.707, rate);
                }
            }
        }
        // Underwater wavers.
        if scene.perspective == 3 {
            let w = post.wobble.next(0.3, BLOCK as f32 / rate);
            for c in 0..2 {
                for f in &mut post.lp[c] {
                    f.set(lp * (1.0 + 0.25 * w), 0.707, rate);
                }
            }
        }
        let (dg, dcut) = (db(-18.0 * distance), 20_000.0 * 0.1f32.powf(distance));
        for f in &mut post.dist {
            f.set(dcut, 0.707, rate);
        }
        post.reverb.set(size);
        let send = (space * (1.0 + distance * 1.5)).min(1.5);
        let dry = 1.0 - 0.5 * space.min(1.0) * distance;
        let master = self.master.step(db(scene.master)) * fade;
        let filtered = scene.perspective != 0;
        let mut peak = self.peak;
        for i in 0..BLOCK {
            let mut x = [bus.l[i], bus.r[i]];
            for c in 0..2 {
                let mut y = x[c];
                if filtered {
                    for f in &mut post.hp[c] {
                        y = f.high(y);
                    }
                    for f in &mut post.lp[c] {
                        y = f.low(y);
                    }
                    if drive > 0.0 {
                        y = (y * (1.0 + drive * 4.0)).tanh() / (1.0 + drive * 2.0);
                    }
                    y *= plevel;
                }
                if distance > 0.001 {
                    y = post.dist[c].low(y) * dg;
                }
                // Tone tilts around 800 Hz.
                let low = post.tilt[c].low(y);
                y += tone * 0.7 * ((y - low) - low);
                x[c] = y;
            }
            let (m, s) = ((x[0] + x[1]) * 0.5, (x[0] - x[1]) * 0.5 * width * (1.0 - mono));
            let (l, r) = (m + s, m - s);
            let (wl, wr) = post.reverb.tick(l * send, r * send);
            let (l, r) = (soft((l * dry + wl) * master), soft((r * dry + wr) * master));
            peak = peak.max(l.abs()).max(r.abs());
            bus.l[i] = l;
            bus.r[i] = r;
        }
        self.peak = peak;
    }
}

/// A finished take of a scene: interleaved stereo, `seconds` long. With
/// `loop_seconds` over zero, the end is crossfaded into the start over
/// that long, so it loops without a seam. `progress` hears 0..1 as it goes.
pub fn render(scene: &Scene, rate: u32, seconds: f32, loop_seconds: f32, mut progress: impl FnMut(f32)) -> Vec<f32> {
    let mut e = Engine::new(rate);
    e.set_scene(scene);
    e.set_playing(true);
    e.fade.value = 1.0;
    // A second's run-up, so the take doesn't start on details all at once.
    let warm = rate as usize;
    let frames = (seconds.max(0.1) * rate as f32) as usize;
    let xf = ((loop_seconds.max(0.0) * rate as f32) as usize).min(frames / 2);
    let total = warm + frames + xf;
    let mut l = vec![0.0; total];
    let mut r = vec![0.0; total];
    let chunk = 4096;
    let mut done = 0;
    while done < total {
        let n = chunk.min(total - done);
        e.render(&mut l[done..done + n], &mut r[done..done + n]);
        done += n;
        progress(done as f32 / total as f32);
    }
    let (l, r) = (&l[warm..], &r[warm..]);
    let mut out = Vec::with_capacity(frames * 2);
    for i in 0..frames {
        let (mut a, mut b) = (l[i], r[i]);
        if i < xf {
            // Equal-power: the tail fading out over the head fading in.
            let t = i as f32 / xf as f32;
            let (fi, fo) = ((t * std::f32::consts::FRAC_PI_2).sin(), (t * std::f32::consts::FRAC_PI_2).cos());
            a = a * fi + l[frames + i] * fo;
            b = b * fi + r[frames + i] * fo;
        }
        out.push(a);
        out.push(b);
    }
    out
}

/// A 16-bit stereo WAV of interleaved samples.
pub fn wav(samples: &[f32], rate: u32) -> Vec<u8> {
    let data = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 4).to_le_bytes());
    out.extend_from_slice(&4u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rms(v: &[f32]) -> f32 {
        (v.iter().map(|x| x * x).sum::<f32>() / v.len().max(1) as f32).sqrt()
    }

    #[test]
    fn a_changed_scene_keeps_its_unchanged_sources() {
        let mut e = Engine::new(44_100);
        let mut scene = BANK[0].scene();
        e.set_scene(&scene);
        let before: Vec<*const ()> = e.layers[0].sources.iter().map(|s| s.as_ref().map_or(std::ptr::null(), |s| &*s.gen as *const _ as *const ())).collect();
        scene.layers[0].sources[0].level -= 3.0;
        e.set_scene(&scene);
        let after: Vec<*const ()> = e.layers[0].sources.iter().map(|s| s.as_ref().map_or(std::ptr::null(), |s| &*s.gen as *const _ as *const ())).collect();
        assert_eq!(before, after);
        scene.layers[0].sources[0].seed ^= 1;
        e.set_scene(&scene);
        let changed: Vec<*const ()> = e.layers[0].sources.iter().map(|s| s.as_ref().map_or(std::ptr::null(), |s| &*s.gen as *const _ as *const ())).collect();
        assert_ne!(before[0], changed[0]);
    }

    #[test]
    fn stopping_fades_out_and_mute_and_solo_work() {
        let mut scene = Scene::default();
        let noise = generator("noise").unwrap();
        scene.layers[0].sources.push(Source::new(noise, 1));
        scene.layers[1].sources.push(Source::new(generator("hum").unwrap(), 2));
        let mut e = Engine::new(44_100);
        e.set_scene(&scene);
        e.set_playing(true);
        let (mut l, mut r) = (vec![0.0; 44_100], vec![0.0; 44_100]);
        e.render(&mut l, &mut r);
        let both = rms(&l[22_050..]);
        assert!(both > 0.01);
        scene.layers[1].solo = true;
        e.set_scene(&scene);
        e.render(&mut l, &mut r);
        let solo = rms(&l[22_050..]);
        scene.layers[1].solo = false;
        scene.layers[1].mute = true;
        e.set_scene(&scene);
        e.render(&mut l, &mut r);
        let muted = rms(&l[22_050..]);
        assert!(solo < both && muted < both && (solo - muted).abs() > 0.001);
        e.set_playing(false);
        e.render(&mut l, &mut r);
        assert!(rms(&l[22_050..]) < 1e-6, "silent once stopped");
        assert!(!e.playing());
    }

    #[test]
    fn every_perspective_plays_and_a_loop_has_no_seam() {
        let mut scene = BANK.iter().find(|p| p.name.contains("Light rain")).unwrap().scene();
        for (p, name) in PERSPECTIVES.iter().enumerate() {
            scene.perspective = p;
            scene.distance = 0.5;
            let s = render(&scene, 22_050, 1.0, 0.0, |_| {});
            assert!(s.iter().all(|x| x.is_finite() && x.abs() <= 1.0));
            assert!(rms(&s) > 1e-4, "{name}");
        }
        scene.perspective = 0;
        let s = render(&scene, 22_050, 3.0, 0.5, |_| {});
        assert_eq!(s.len(), 3 * 22_050 * 2);
        // The jump from the last frame to the first is no bigger than a step inside it.
        let step = |a: usize, b: usize| (s[a] - s[b]).abs();
        let typical = (2..2000).map(|i| step(i * 2, i * 2 - 2)).fold(0.0f32, f32::max);
        assert!(step(0, s.len() - 2) <= typical * 1.5 + 1e-3);
    }

    #[test]
    fn a_wav_has_its_header() {
        let w = wav(&[0.0, 0.5, -0.5, 1.0], 48_000);
        assert_eq!(&w[..4], b"RIFF");
        assert_eq!(w.len(), 44 + 8);
    }
}
