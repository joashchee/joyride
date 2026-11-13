// Copyright 2026 ansiapps. Neumetik Synth, proprietary: see LICENSE.

//! **SiON**'s synthesizer, ported (roadmap 1.5, phases 5c and 5d): SiOPM,
//! its FM, PSG, pulse, noise, wave and sampler channels, the tracks that
//! play an MML, and the effects. The MML compiler for MIDI is in
//! neumetik-midi (`text/sion.rs`); this is the sound.

pub mod compile;
pub mod effects;
pub mod fm;
pub mod module;
pub mod pcm;
pub mod sequencer;
pub mod table;
pub mod track;
pub mod translate;

pub use pcm::{PcmData, PcmTable, SampleBank, SamplerData, SamplerTable, Sound, SoundReference};
pub use sequencer::{render_mml, render_mml_with, render_song, render_song_with, Rendered, Sequencer, RATE};
