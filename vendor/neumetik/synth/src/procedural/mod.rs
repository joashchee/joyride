//! **Procedural sound**, copied from neumetik-bgn
//! (`synth/scripts/copies.py` keeps them the same): every noise method
//! and the models of real sounds built from them.
//!
//! - [`noise`]: white, Gaussian, pink three ways, brown, blue, violet,
//!   grey, velvet, spectral (shaped in the frequency domain),
//!   shift-register, sample-and-hold, chaotic maps and smooth gradient
//!   noise (Perlin, value, simplex).
//! - [`dsp`]: the parts the models are built of: modal resonators,
//!   bubbles (Minnaert's resonance, gliding up), chirps, noise bursts,
//!   formant voices, slow random drift, envelopes, filters.
//! - [`gens`]: the 61 generators [`catalog`] lists (wind, rain, surf,
//!   fire, birds, crowds, engines, traffic, beeps...), each a procedural
//!   model of its sound; `gens::make` starts one by its ID.

#![allow(dead_code)]

pub mod catalog;
pub mod dsp;
mod fft;
pub mod gens;
pub mod noise;
