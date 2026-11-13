// Copyright 2026 ansiapps. Neumetik BGN: proprietary, see the LICENSE at the repository root.

//! The generators `catalog.rs` lists, each a procedural model of its
//! sound: noise shaped and moved the way the real thing moves, or the
//! events it's made of (drops, chirps, steps, strikes) scheduled at
//! random. Each reads its parameters in the catalog's order.

// The generators index their left and right (and their voices') state
// side by side; a loop over indexes reads better there than zipped iterators.
#[allow(clippy::needless_range_loop)]
mod machines;
#[allow(clippy::needless_range_loop)]
mod nature;
#[allow(clippy::needless_range_loop)]
mod noise;
#[allow(clippy::needless_range_loop)]
mod people;
#[allow(clippy::needless_range_loop)]
mod signals;
#[allow(clippy::needless_range_loop)]
mod wildlife;

use super::dsp::Block;

/// One running generator.
pub trait Gen: Send {
    /// Adds a block to `out`. `v` holds the parameters' values, in the
    /// catalog's order.
    fn render(&mut self, v: &[f32; 6], out: &mut Block);
}

/// A generator by its catalog ID, or None for one that isn't listed.
pub fn make(id: &str, rate: f32, seed: u32) -> Option<Box<dyn Gen>> {
    use machines::*;
    use nature::*;
    use noise::*;
    use people::*;
    use signals::*;
    use wildlife::*;
    let g: Box<dyn Gen> = match id {
        "noise" => Box::new(NoiseGen::new(rate, seed)),
        "spectral" => Box::new(SpectralGen::new(rate, seed)),
        "digital" => Box::new(DigitalGen::new(rate, seed)),
        "random" => Box::new(RandomGen::new(rate, seed)),
        "chaos" => Box::new(ChaosGen::new(rate, seed)),
        "crackle" => Box::new(CrackleGen::new(rate, seed)),
        "static" => Box::new(StaticGen::new(rate, seed)),
        "wind" => Box::new(Wind::new(rate, seed)),
        "foliage" => Box::new(Foliage::new(rate, seed)),
        "rain" => Box::new(Rain::new(rate, seed)),
        "thunder" => Box::new(Thunder::new(rate, seed)),
        "stream" => Box::new(Stream::new(rate, seed)),
        "surf" => Box::new(Surf::new(rate, seed)),
        "lapping" => Box::new(Lapping::new(rate, seed)),
        "torrent" => Box::new(Torrent::new(rate, seed)),
        "drips" => Box::new(Drips::new(rate, seed)),
        "bubbling" => Box::new(Bubbling::new(rate, seed)),
        "underwater" => Box::new(Underwater::new(rate, seed)),
        "ice" => Box::new(Ice::new(rate, seed)),
        "fire" => Box::new(Fire::new(rate, seed)),
        "rumble" => Box::new(Rumble::new(rate, seed)),
        "steam" => Box::new(Steam::new(rate, seed)),
        "crickets" => Box::new(Crickets::new(rate, seed)),
        "cicadas" => Box::new(Cicadas::new(rate, seed)),
        "frogs" => Box::new(Frogs::new(rate, seed)),
        "insects" => Box::new(Insects::new(rate, seed)),
        "birds" => Box::new(Birds::new(rate, seed)),
        "calls" => Box::new(Calls::new(rate, seed)),
        "crowd" => Box::new(Crowd::new(rate, seed)),
        "applause" => Box::new(Applause::new(rate, seed)),
        "footsteps" => Box::new(Footsteps::new(rate, seed)),
        "clatter" => Box::new(Clatter::new(rate, seed)),
        "typing" => Box::new(Typing::new(rate, seed)),
        "choir" => Box::new(Choir::new(rate, seed)),
        "whispers" => Box::new(Whispers::new(rate, seed)),
        "breathing" => Box::new(Breathing::new(rate, seed)),
        "heartbeat" => Box::new(Heartbeat::new(rate, seed)),
        "music" => Box::new(Music::new(rate, seed)),
        "knocks" => Box::new(Knocks::new(rate, seed)),
        "hum" => Box::new(Hum::new(rate, seed)),
        "air" => Box::new(Air::new(rate, seed)),
        "machine" => Box::new(Machine::new(rate, seed)),
        "engine" => Box::new(Engine::new(rate, seed)),
        "road" => Box::new(Road::new(rate, seed)),
        "traffic" => Box::new(Traffic::new(rate, seed)),
        "rail" => Box::new(Rail::new(rate, seed)),
        "aircraft" => Box::new(Aircraft::new(rate, seed)),
        "creaks" => Box::new(Creaks::new(rate, seed)),
        "rattle" => Box::new(Rattle::new(rate, seed)),
        "beeps" => Box::new(Beeps::new(rate, seed)),
        "energy" => Box::new(Energy::new(rate, seed)),
        "data" => Box::new(Data::new(rate, seed)),
        "drone" => Box::new(Drone::new(rate, seed)),
        "shimmer" => Box::new(Shimmer::new(rate, seed)),
        "bells" => Box::new(Bells::new(rate, seed)),
        "ticks" => Box::new(Ticks::new(rate, seed)),
        "sirens" => Box::new(Sirens::new(rate, seed)),
        "horns" => Box::new(Horns::new(rate, seed)),
        "battle" => Box::new(Battle::new(rate, seed)),
        "passby" => Box::new(Passby::new(rate, seed)),
        _ => return None,
    };
    Some(g)
}

/// Chords as semitones: unison, fifth, octaves, minor, major, tritone,
/// cluster (the drone's list; the voices pad uses all but the tritone).
pub(crate) const CHORDS: [&[f32]; 7] = [&[0.0], &[0.0, 7.0], &[0.0, 12.0, 24.0], &[0.0, 3.0, 7.0, 12.0], &[0.0, 4.0, 7.0, 12.0], &[0.0, 6.0, 12.0], &[0.0, 1.0, 5.0, 6.0]];

/// How much each generator is raised or lowered (dB) so that at a level of
/// 0 dB a continuous one sits near -20 dBFS RMS and a sparse one peaks
/// near -6 dBFS, so presets balance by ear. Measured at the defaults with
/// `cargo run --release --example bgn_catalog -- --levels`.
const TRIMS: [(&str, f32); 60] = [
    ("noise", -3.0),
    ("spectral", -8.0),
    ("digital", -1.0),
    ("random", -5.0),
    ("chaos", -4.0),
    ("crackle", 24.0),
    ("static", 5.0),
    ("wind", 14.0),
    ("foliage", 13.0),
    ("rain", 11.0),
    ("thunder", -6.0),
    ("stream", 12.0),
    ("surf", 2.0),
    ("lapping", 11.0),
    ("torrent", 1.0),
    ("drips", 2.0),
    ("bubbling", 6.0),
    ("underwater", -10.0),
    ("ice", -3.0),
    ("fire", 3.0),
    ("rumble", -13.0),
    ("steam", 18.0),
    ("crickets", 6.0),
    ("cicadas", 14.0),
    ("frogs", 14.0),
    ("insects", 16.0),
    ("birds", 12.0),
    ("calls", 13.0),
    ("crowd", 27.0),
    ("applause", 16.0),
    ("footsteps", 18.0),
    ("clatter", 14.0),
    ("typing", 15.0),
    ("choir", 12.0),
    ("whispers", 10.0),
    ("breathing", 12.0),
    ("heartbeat", 4.0),
    ("music", 6.0),
    ("knocks", 4.0),
    ("hum", -2.0),
    ("air", 2.0),
    ("machine", 1.0),
    ("engine", 20.0),
    ("road", -9.0),
    ("traffic", 8.0),
    ("rail", -12.0),
    ("aircraft", -8.0),
    ("creaks", 17.0),
    ("rattle", 7.0),
    ("beeps", 3.0),
    ("energy", 0.0),
    ("data", 9.0),
    ("drone", 0.0),
    ("shimmer", 5.0),
    ("bells", 15.0),
    ("ticks", 4.0),
    ("sirens", 13.0),
    ("horns", 16.0),
    ("battle", 10.0),
    ("passby", 4.0),
];

/// A generator's trim in dB (0 for one not listed).
pub fn trim(id: &str) -> f32 {
    TRIMS.iter().find(|(g, _)| *g == id).map_or(0.0, |(_, t)| *t)
}

/// A choice parameter's index.
#[inline]
pub(crate) fn pick(v: f32) -> usize {
    v.max(0.0) as usize
}

/// How a distance (0..1) sounds: quieter and duller.
pub(crate) fn far(d: f32) -> (f32, f32) {
    (super::dsp::db(-26.0 * d), 16_000.0 * 0.07f32.powf(d))
}

/// A tone amount (0..1) as a low-pass cutoff, 300 Hz to 18 kHz.
pub(crate) fn tone_cut(t: f32) -> f32 {
    300.0 * 60f32.powf(t.clamp(0.0, 1.0))
}

/// A low pass per channel, retuned once a block.
pub(crate) struct Lp2 {
    pub f: [super::dsp::Svf; 2],
}

impl Lp2 {
    pub fn new(rate: f32) -> Lp2 {
        Lp2 { f: [super::dsp::Svf::new(18_000.0, 0.707, rate); 2] }
    }

    pub fn set(&mut self, cut: f32, rate: f32) {
        for f in &mut self.f {
            f.set(cut, 0.707, rate);
        }
    }

    /// Filters a block in place.
    pub fn run(&mut self, out: &mut Block, from: usize) {
        for i in from..super::dsp::BLOCK {
            out.l[i] = self.f[0].low(out.l[i]);
            out.r[i] = self.f[1].low(out.r[i]);
        }
    }
}

/// Renders into a scratch block, then low-passes it (by distance or
/// tone) and adds it to `out`: what most event generators do.
pub(crate) fn add_filtered(scratch: &mut Block, lp: &mut Lp2, gain: f32, out: &mut Block) {
    lp.run(scratch, 0);
    for i in 0..super::dsp::BLOCK {
        out.l[i] += scratch.l[i] * gain;
        out.r[i] += scratch.r[i] * gain;
    }
}

#[cfg(test)]
mod tests {
    use super::super::catalog::GENERATORS;
    use super::super::dsp::{Block, BLOCK};
    use super::*;

    /// Every generator, at its defaults and at every choice, makes sound in
    /// a sane range and doesn't blow up at the extremes.
    #[test]
    fn every_generator_sounds_at_every_setting_and_stays_sane() {
        let rate = 44_100.0;
        for g in GENERATORS {
            let choices: Vec<(usize, usize)> = g
                .params
                .iter()
                .enumerate()
                .filter_map(|(i, p)| match p.kind {
                    super::super::catalog::Kind::Choice(o) => Some((i, o.len())),
                    _ => None,
                })
                .collect();
            let runs = choices.first().map_or(1, |c| c.1);
            for run in 0..runs {
                for extreme in [None, Some(0.0), Some(1.0)] {
                    let mut p = g.defaults();
                    if let Some(x) = extreme {
                        for (i, q) in g.params.iter().enumerate() {
                            if !q.is_choice() {
                                p[i] = x;
                            }
                        }
                    }
                    if let Some(&(i, _)) = choices.first() {
                        p[i] = run as f32;
                    }
                    let mut v = [0.0; 6];
                    for (i, q) in g.params.iter().enumerate() {
                        v[i] = q.value(p[i]);
                    }
                    let mut gen = make(g.id, rate, 11).unwrap_or_else(|| panic!("{} isn't made", g.id));
                    let (mut peak, mut energy) = (0f32, 0f64);
                    let mut block = Block::new();
                    // Twenty seconds: long enough for rare details to come.
                    for _ in 0..(20.0 * rate) as usize / BLOCK {
                        block.clear();
                        gen.render(&v, &mut block);
                        for i in 0..BLOCK {
                            let (l, r) = (block.l[i], block.r[i]);
                            assert!(l.is_finite() && r.is_finite(), "{} {:?} run {run}: not finite", g.id, extreme);
                            peak = peak.max(l.abs()).max(r.abs());
                            energy += f64::from(l * l + r * r);
                        }
                    }
                    let what = format!("{} ({}) {:?}", g.id, run, extreme);
                    assert!(peak < 4.0, "{what}: peak {peak}");
                    // At their lowest some are meant to be all but silent.
                    if extreme != Some(0.0) {
                        assert!(energy > 1e-3, "{what}: silent");
                    }
                }
            }
        }
    }
}
