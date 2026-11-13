// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! **Sound effects**, synthesized whole: one-shots for games, each a
//! recipe of layers from [`kit`]: sub-bass booms and rumbles, risers and
//! implosions, spell impacts and casts for twelve families of magic
//! (arcane, fire, cold, water, acid, gas, electric, mind, poison,
//! paralysis, holy, necromantic) with their high accents, and interface
//! chimes. [`render`] makes one by its ID; [`RECIPES`] lists them.
//!
//! Every sound is made from scratch by textbook means (oscillators,
//! filtered noise, modal resonators, FM bells, a Schroeder reverb); no
//! recording is in it.

pub mod kit;
mod recipes;

pub use recipes::RECIPES;

pub struct Recipe {
    /// Unique and stable.
    pub id: &'static str,
    /// What it sounds like.
    pub name: &'static str,
    make: fn(&mut kit::Kit) -> Vec<f32>,
}

/// The sound `id` makes, mono at `rate`, peaking at 1, its last few
/// milliseconds faded; `None` for an ID no recipe has.
pub fn render(id: &str, rate: u32) -> Option<Vec<f32>> {
    let recipe = RECIPES.iter().find(|r| r.id == id)?;
    // Seeded by the ID, so a sound is the same every time it's made.
    let seed = id.bytes().fold(0x811c_9dc5u32, |h, b| (h ^ u32::from(b)).wrapping_mul(0x0100_0193));
    let mut k = kit::Kit::new(rate, seed);
    // Off with any DC, then the silence at the end, then a fade.
    let mut x = kit::highpass(&(recipe.make)(&mut k), k.rate, 12.0);
    let floor = kit::peak(&x) * 0.001;
    let end = x.iter().rposition(|v| v.abs() > floor).map_or(0, |i| i + 1);
    x.truncate(end.max(1));
    let fade = (k.rate * 0.01) as usize;
    let n = x.len();
    for (i, v) in x.iter_mut().enumerate().skip(n.saturating_sub(fade)) {
        *v *= (n - i) as f32 / fade as f32;
    }
    kit::norm(&mut x);
    Some(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_recipe_makes_a_clean_sound() {
        let mut ids = std::collections::HashSet::new();
        for r in RECIPES {
            assert!(ids.insert(r.id), "{} twice", r.id);
            let x = render(r.id, 44100).unwrap();
            assert!(x.len() > 441, "{} is too short", r.id);
            assert!(x.len() < 44100 * 8, "{} is {} s long", r.id, x.len() / 44100);
            assert!(x.iter().all(|v| v.is_finite()), "{} isn't finite", r.id);
            assert!((kit::peak(&x) - 1.0).abs() < 1e-3, "{} peaks at {}", r.id, kit::peak(&x));
            assert_eq!(x, render(r.id, 44100).unwrap(), "{} changes", r.id);
        }
    }
}
