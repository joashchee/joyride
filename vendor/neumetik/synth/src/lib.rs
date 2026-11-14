// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! **Neumetik Synth**: the synthesis methods Neumetik's other crates play,
//! each written from scratch on public material whose license lets
//! Neumetik ship it closed source (`docs/synth-methods.md` has each
//! method's sources and why it's clear).
//!
//! - [`opl`]: FM the OPL's way (Yamaha's OPL2 and OPL3): a patch played
//!   as a voice, for tracker modules' AdLib instruments, and the chip by
//!   its registers.
//! - [`sequence`]: OctaMED's synth and hybrid sounds, single-cycle
//!   waveforms stepped through by a waveform and a volume sequence.
//! - [`puma`]: Pumatracker's instruments, a volume and a pitch script
//!   over the song's samples and the replayer's built-in cycles.
//! - [`sel`]: Face The Music's SEL scripts, a script a track playing
//!   the song's samples by program (pitch, volume, sample, LFOs, other
//!   tracks), run by the player as its host.
//! - [`musicline`]: Musicline Editor's replayer, every channel its own
//!   sequence and speed, instruments whose waves five effects remake
//!   every tick (a morph, a phase squeeze, a mix, a resonance, a filter)
//!   or whose loop moves through the sample.
//! - [`ptk`]: ProTrekkr's and NoiseTrekker's replayer, sixteen tracks
//!   each through its own filter, flanger and compressor into a chorus
//!   and a reverb, instruments that play samples and a two-oscillator
//!   synth, and two TB-303s.
//! - [`replay`]: songs played by their own replayer (the synth and chip
//!   trackers): a [`replay::Machine`] plays any of them.
//! - [`ahx`]: AHX and HivelyTracker, every sound a filtered triangle,
//!   sawtooth, square or noise stepped through by a play list.
//! - [`paula`]: the Amiga's sound chip, as the Amiga replayers play it.
//! - [`fc`]: Future Composer 1.3 and 1.4, instruments of a frequency and
//!   a volume sequence over waveforms and samples.
//! - [`sidmon1`]: SidMon 1, read from its rip by its replayer's own
//!   instructions; waveforms and samples under an envelope and arpeggio.
//! - [`sidmon2`]: SidMon 2, samples stepped through lists under an
//!   envelope, a point of each inverted as it plays.
//! - [`soundmon`]: BP SoundMon 2.0 and 2.2, samples and synth waveforms
//!   remade as they play by an EG, averaging, morphing and modulation.
//! - [`deltamusic1`]: Delta Music 1, samples and waveform tables under an
//!   envelope, arpeggio, vibrato and bends the song's effects reshape.
//! - [`deltamusic2`]: Delta Music 2, samples and waveform tables under
//!   five-stage volumes and vibratos, each channel its own block list.
//! - [`mugician`]: Digital Mugician 1 and 2, samples and waveforms
//!   remade by fifteen effects as they play; 2's seven channels.
//! - [`sonic`]: Sonic Arranger, samples and waveforms remade by 17
//!   effects under ADSR and AMF tables, at the song's own tempo.
//! - [`instereo`]: InStereo! 1.0 and 2.0, samples and synth waveforms
//!   stepped through by envelope generators.
//! - [`musicassembler`]: Music Assembler, samples and sweeping
//!   waveforms under an attack, hold and release, from its rips.
//! - [`synthesis`]: Synthesis, InStereo! 1.0's sibling, its waveforms
//!   reshaped as they play by 16 effects.
//! - [`voodoo`]: Voodoo Supreme Synthesizer, two waveforms made into a
//!   third each tick (ring modulation, XOR, morphing), or a sample
//!   stepped through block by block at Paula's audio interrupt.
//! - [`sawteeth`]: Sawteeth, anti-aliased sawtooths, squares, triangles,
//!   noise and sines at 44.1 kHz in floating point, through six filters
//!   and two clippers under amp and filter envelopes.
//! - [`klystrack`]: Klystrack, klystron's SID-like oscillators ANDed
//!   together beside a wavetable, FM, ring modulation and sync, a filter,
//!   and chorus, reverb and bitcrusher buses, all in integers at 44.1 kHz.
//! - [`v2`]: farbrausch's V2 and its V2M songs: box-filtered oscillators,
//!   filters, distortion and a modulation matrix a voice, channel and mix
//!   effects, and a formant speech synth, in f32 at 44.1 kHz.
//! - [`monotone`]: MONOTONE, the PC speaker's one square wave shared
//!   between up to four tracks a tick at a time.
//! - [`pretracker`]: Pretracker, its waves drawn at load (oscillators,
//!   filter, envelope, modulator) and played on Paula.
//! - [`organya`]: Organya (Cave Story's music), single-cycle waveforms
//!   and drum samples played as DirectSound plays them.
//! - [`am`]: StarTrekker's AM instruments, a looped 32-point cycle
//!   under a five-stage envelope.
//! - [`jam`]: JamCracker's AM instruments, a looped 64-point cycle
//!   mixed with itself at a moving phase.
//! - [`sunvox`]: SunVox's engine, a net of synth, sampler, effect and
//!   controller modules driven by patterns on a timeline.
//! - [`renoise`]: Renoise's player, sampled instruments (keyzones,
//!   envelopes or modulation sets, effect chains, phrases) on tracks of
//!   note and effect columns, through native effect devices into groups,
//!   send tracks and the master; [`flac`] decodes its samples.
//! - [`stonetracker`]: StoneTracker's player, 4 to 8 sample tracks (the
//!   ones past Paula's four mixed in software, through the player's own
//!   volume curve) and an FX track that sends its effects to the others.
// private:start
//! - [`engines`]: the MIDI synthesizer's voices (subtractive with the
//!   supersaw and sync, two-operator FM, Karplus-Strong, drums), copied
//!   from neumetik-midi.
//! - [`procedural`]: every noise method and BGN's 61 procedural models of
//!   real sounds, copied from neumetik-bgn.
//! - [`sfx`]: sound effects for games synthesized whole: booms,
//!   rumbles and risers, twelve families of spell impacts, casts and
//!   accents, interface chimes.
// private:end
//! - [`cycle`]: single-cycle waveforms made into looping samples (the
//!   Tracker's synths).
//!
// private:start
//! The rule (README.md at the repository's root): every synthesis method
//! Neumetik has is here. A new one lands here, so it's written once and
//! every crate can play it; one a crate must keep for itself (MIDI, which
//! stands alone under the MIT license; BGN) is copied here by
//! `scripts/copies.py`, which also says when a copy has drifted.
// private:end

pub mod ahx;
pub mod am;
pub mod arkos;
pub(crate) mod arkos_at3;
pub mod arkos_xml;
pub mod ay;
pub mod cycle;
pub mod deltamusic1;
pub mod deltamusic2;
// private:start
#[cfg(feature = "copies")]
pub mod engines;
// private:end
pub mod deflemask;
pub mod deflemask_play;
pub mod famitracker;
pub mod famitracker_play;
pub mod gb;
pub mod huc6280;
pub mod fc;
pub mod flac;
pub mod fcbsi;
pub mod flmml;
pub mod sion;
pub mod fmp;
pub mod fmwork;
pub mod goattracker;
pub mod goattracker1;
pub mod instereo;
pub mod jam;
pub mod klystrack;
pub mod mgs;
pub mod m68k;
pub mod mdsdrv;
pub mod mdsmml;
pub mod mgsc;
pub mod mmlgb;
pub mod nsc;
pub mod nsf;
pub mod cpu6502;
pub mod monotone;
pub mod mugician;
pub mod nes;
pub mod nes_ext;
pub mod musicassembler;
pub mod musicline;
pub mod opl;
pub mod opll;
pub mod opna;
pub mod organya;
pub mod paula;
pub mod pc98;
pub mod pmd;
pub mod pmd_mc;
pub mod pmd_ssgeff;
pub mod ppz8;
pub mod pt3;
// private:start
#[cfg(feature = "copies")]
pub mod procedural;
pub mod sfx;
/// The sound-effect parts (oscillators, filters, reverb) other Neumetik crates build on.
#[doc(hidden)]
pub use sfx::kit;
// private:end
pub mod ptk;
pub mod pretracker;
pub mod puma;
pub mod pxtone;
pub mod renoise;
pub mod replay;
pub mod sawteeth;
pub mod sel;
pub mod sequence;
pub mod scc;
pub mod sid;
pub mod sn76489;
pub mod ymfm;
pub mod sidmon1;
pub mod sidmon2;
pub mod sonic;
pub mod soundmon;
pub mod stonetracker;
pub mod sunvox;
pub(crate) mod tremor;
mod tremor_tables;
pub mod synthesis;
pub mod v2;
pub(crate) mod xml;
pub mod vic;
pub mod victracker;
pub mod voodoo;
pub mod vorbis;
