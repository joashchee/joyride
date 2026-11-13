// Copyright 2026 ansiapps. Neumetik Visual: proprietary, see the LICENSE at the repository root.

//! **Neumetik Visual**: music played by Neumetik seen as animated ANSI
//! art. Every visualization Neumetik has is here, and each draws in code
//! page 437 characters and the VGA's 16 colors, so whatever it shows can
//! be kept as ANSI art and opened in Stylus, ansiapps' ANSI art editor,
//! and art made in Stylus can be shown here.
//!
//! - [`analysis`]: the sound's spectrum, levels, beats, brightness, notes
//!   and stereo picture.
//! - [`visual`]: the [`Visualizer`], its [`Settings`] (kept with a song as
//!   a line of text), and the methods ([`KINDS`]): spectrum bars, an LED
//!   analyzer, an oscilloscope, VU meters, the hi-fi deck, a spectrogram,
//!   ridgelines, a radial spectrum, a vectorscope, a polar scope, phase
//!   space, a chromagram, a kaleidoscope, fire, plasma, a starfield, a
//!   tunnel, code rain, Conway's Life, ripples, metaballs, fireworks, the
//!   tracker's channels, a title scroller and the cover art.
//! - [`canvas`], [`palette`], [`cp437`], [`font`]: what they draw with.
//! - [`ansi`], [`sauce`], [`anim`]: ANSI, ANSImation, XBin and binary
//!   text files, read and written the way Stylus reads and writes them.
//! - [`cover`]: the cover painter, a song's ANSI art cover painted from
//!   what's known of it (its title, words, samples and tempo).
//!
//! No dependencies: every line is ansiapps' own (`docs/visual-methods.md`
//! has where each method is from).

pub mod analysis;
pub mod anim;
pub mod ansi;
pub mod canvas;
pub mod cover;
pub mod cp437;
pub mod font;
pub mod methods;
pub mod palette;
pub mod rng;
pub mod sauce;
pub mod visual;

pub use anim::{Animation, Frame};
pub use canvas::{Canvas, Cell};
pub use visual::{Input, Settings, Visualizer, Voice, KINDS};
