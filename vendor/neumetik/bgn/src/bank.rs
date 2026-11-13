// Copyright 2026 ansiapps. Neumetik BGN: proprietary, see the LICENSE at the repository root.

//! The BGN sound bank: a preset for every entry of the background noise
//! list (`background-noise-list.md`), each a layered scene, filed under the
//! list's sections and subsections and tagged for search. The presets are
//! data in `bank_data.rs`, written by `scripts/bgn-bank.py` from the list
//! and its recipes; change the recipes there and run it again rather than
//! editing the data.

use super::catalog::generator;
use super::{Layer, Scene, Source};

/// One source in a preset: its generator, level (dB), pan and width, and
/// the parameters that differ from the generator's defaults, as (index,
/// stored value).
pub struct SourceDef {
    pub gen: &'static str,
    pub level: f32,
    pub pan: f32,
    pub width: f32,
    pub params: &'static [(u8, f32)],
}

pub struct BankPreset {
    /// Unique and stable: the category's and the name's slugs.
    pub id: &'static str,
    pub name: &'static str,
    /// The list's section ("Nature") and subsection ("Weather"), or the
    /// section again where it has none.
    pub category: &'static str,
    pub group: &'static str,
    /// The list's own note on it, from its parentheses ("tin, tile, ...").
    pub note: &'static str,
    pub tags: &'static [&'static str],
    pub layers: [&'static [SourceDef]; 4],
    /// Each layer's level (dB) and swell (0..1).
    pub levels: [f32; 4],
    pub swell: [f32; 4],
    pub perspective: u8,
    pub distance: f32,
    pub space: f32,
    pub size: f32,
    pub tone: f32,
    pub width: f32,
    /// Decibels, set so presets sit at about the same loudness.
    pub master: f32,
}

/// A stable 32-bit hash (FNV-1a), for each preset's seeds.
fn hash(s: &str) -> u32 {
    s.bytes().fold(0x811c_9dc5u32, |h, b| (h ^ u32::from(b)).wrapping_mul(0x0100_0193))
}

impl BankPreset {
    /// The preset as a scene to play or change.
    pub fn scene(&self) -> Scene {
        let base = hash(self.id);
        let mut scene = Scene {
            perspective: usize::from(self.perspective),
            distance: self.distance,
            space: self.space,
            size: self.size,
            tone: self.tone,
            width: self.width,
            master: self.master,
            ..Scene::default()
        };
        for (k, defs) in self.layers.iter().enumerate() {
            let mut layer = Layer { level: self.levels[k], swell: self.swell[k], ..Layer::default() };
            for (j, d) in defs.iter().enumerate() {
                let Some(g) = generator(d.gen) else { continue };
                let mut s = Source::new(g, base.wrapping_add(((k * 31 + j) as u32).wrapping_mul(0x9e37_79b9)));
                s.level = d.level;
                s.pan = d.pan;
                s.width = d.width;
                for &(i, v) in d.params {
                    if let Some(p) = s.params.get_mut(usize::from(i)) {
                        *p = v;
                    }
                }
                layer.sources.push(s);
            }
            scene.layers[k] = layer;
        }
        scene
    }

    /// Whether every word of `query` is in its name, category, group, note or tags.
    pub fn matches(&self, query: &str) -> bool {
        let hay = format!("{} {} {} {} {}", self.name, self.category, self.group, self.note, self.tags.join(" ")).to_lowercase();
        query.to_lowercase().split_whitespace().all(|w| hay.contains(w))
    }
}

include!("bank_data.rs");

#[cfg(test)]
mod tests {
    use super::super::catalog::Kind;
    use super::*;

    #[test]
    fn every_preset_is_well_formed_and_unique() {
        let mut ids = std::collections::HashSet::new();
        assert!(BANK.len() > 100);
        for p in BANK {
            assert!(ids.insert(p.id), "{} twice", p.id);
            assert!(CATEGORIES.iter().any(|(c, groups)| *c == p.category && groups.contains(&p.group)), "{}: {} / {}", p.id, p.category, p.group);
            assert!(!p.tags.is_empty(), "{} has no tags", p.id);
            assert!(p.layers.iter().any(|l| !l.is_empty()), "{} is empty", p.id);
            for l in p.layers {
                for s in l {
                    let g = generator(s.gen).unwrap_or_else(|| panic!("{}: no generator {}", p.id, s.gen));
                    for &(i, v) in s.params {
                        let q = g.params.get(usize::from(i)).unwrap_or_else(|| panic!("{}: {} has no parameter {i}", p.id, s.gen));
                        match q.kind {
                            Kind::Choice(o) => assert!((v as usize) < o.len() && v.fract() == 0.0, "{}: {} {}", p.id, s.gen, q.name),
                            Kind::Range { .. } => assert!((0.0..=1.0).contains(&v), "{}: {} {} = {v}", p.id, s.gen, q.name),
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn search_finds_by_name_tag_and_note() {
        assert!(BANK.iter().any(|p| p.matches("rain roof")));
        assert!(BANK.iter().any(|p| p.matches("crickets")));
        assert!(!BANK.iter().any(|p| p.matches("zzzz-nothing")));
    }

    /// `cargo test --release bgn_bank_levels -- --ignored --nocapture`:
    /// every preset's loudness, flagging any too quiet or too loud.
    #[test]
    #[ignore]
    fn bgn_bank_levels() {
        let mut flagged = 0;
        for p in BANK {
            let s = super::super::render(&p.scene(), 22_050, 12.0, 0.0, |_| {});
            let rms = (s.iter().map(|x| x * x).sum::<f32>() / s.len() as f32).sqrt();
            let peak = s.iter().fold(0f32, |m, x| m.max(x.abs()));
            let (r, k) = (20.0 * rms.max(1e-9).log10(), 20.0 * peak.max(1e-9).log10());
            let flag = r < -40.0 || r > -10.0 || k > -0.5;
            if flag {
                flagged += 1;
            }
            println!("{}{:6.1} rms {:6.1} peak  {}", if flag { "!" } else { " " }, r, k, p.id);
        }
        println!("{flagged} flagged of {}", BANK.len());
    }

    /// `cargo test --release bgn_bank_speed -- --ignored --nocapture`: how
    /// many times faster than real time the slowest presets play at 48 kHz.
    #[test]
    #[ignore]
    fn bgn_bank_speed() {
        let mut times: Vec<(f64, &str)> = BANK
            .iter()
            .map(|p| {
                let mut e = super::super::Engine::new(48_000);
                e.set_scene(&p.scene());
                e.set_playing(true);
                let (mut l, mut r) = (vec![0.0; 48_000], vec![0.0; 48_000]);
                let start = std::time::Instant::now();
                for _ in 0..4 {
                    e.render(&mut l, &mut r);
                }
                (4.0 / start.elapsed().as_secs_f64(), p.id)
            })
            .collect();
        times.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (x, id) in times.iter().take(12) {
            println!("{x:7.1}x  {id}");
        }
        assert!(times[0].0 > 10.0, "the slowest preset should play at least ten times faster than real time");
    }

    /// `OUT=dir cargo test --release bgn_bank_demo -- --ignored` writes a few
    /// seconds of every preset, a WAV each, to listen through.
    #[test]
    #[ignore]
    fn bgn_bank_demo() {
        let dir = std::env::var("OUT").unwrap_or_else(|_| "bgn-demo".into());
        std::fs::create_dir_all(&dir).unwrap();
        let only = std::env::var("ONLY").unwrap_or_default();
        for p in BANK.iter().filter(|p| p.matches(&only)) {
            let s = super::super::render(&p.scene(), 44_100, 8.0, 0.0, |_| {});
            std::fs::write(format!("{dir}/{}.wav", p.id), super::super::wav(&s, 44_100)).unwrap();
        }
    }
}
