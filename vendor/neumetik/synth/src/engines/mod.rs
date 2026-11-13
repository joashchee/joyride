//! **The MIDI synthesizer's engines**, copied from neumetik-midi
//! (`synth/scripts/copies.py` keeps them the same): a voice is a
//! `Patch` played by one of three engines, then a filter, envelopes and
//! an LFO, or a `Drum`.
//!
//! - **Analog** (subtractive): two oscillators (sine, triangle, saw,
//!   square, pulse, noise, organ drawbars, band-limited by polyBLEP),
//!   osc 1 as up to seven detuned copies (the supersaw), hard sync,
//!   through a state-variable filter.
//! - **Fm**: a two-operator pair with feedback, its index falling from a
//!   peak to a sustain, and the **tine**, a second pair with a short
//!   decay that any engine can add.
//! - **Pluck**: Karplus-Strong, a burst of noise ringing round a tuned
//!   delay line.
//! - **Drums**: each a recipe of three parts: a swept sine (the body),
//!   filtered noise in bursts (wires, claps), and up to six square waves
//!   at inharmonic ratios through a high-pass (metal).

pub mod drums;
pub mod patch;
pub mod voice;

pub use drums::Drum;
pub use patch::Patch;
pub use voice::Voice;
