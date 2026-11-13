// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! SunVox's built-in modules (g_psynths), each a port of its psynths_*.cpp.

use super::net::{self, Module, Net};

mod adsr;
mod amplifier;
mod analog_generator;
mod compressor;
mod ctl2note;
mod dc_blocker;
mod delay;
mod distortion;
mod drumsynth;
mod echo;
mod eq;
mod feedback;
mod fft;
mod fmx;
mod filter;
mod filter_pro;
mod flanger;
mod fm;
mod generator;
mod glide;
mod input_gpio;
mod kicker;
mod lfo;
mod looper;
mod metamodule;
mod modulator;
mod multictl;
mod multisynth;
mod note2ctl;
mod pitch_detector;
mod pitch_shifter;
mod reverb;
mod sampler;
mod smooth;
mod sound2ctl;
mod spectravoice;
mod vibrato;
mod vocal_filter;
mod vorbis_player;
mod waveshaper;

/// A module's kind: its place in g_psynths.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Kind {
    /// psynth_empty: a module of a kind this engine doesn't have.
    Empty,
    AnalogGenerator,
    DrumSynth,
    Fm,
    Fmx,
    Generator,
    Input,
    Kicker,
    VorbisPlayer,
    Sampler,
    SpectraVoice,
    Amplifier,
    Compressor,
    DcBlocker,
    Delay,
    Distortion,
    Echo,
    Eq,
    Fft,
    Filter,
    FilterPro,
    Flanger,
    Lfo,
    Loop,
    Modulator,
    PitchShifter,
    Reverb,
    Smooth,
    VocalFilter,
    Vibrato,
    WaveShaper,
    Adsr,
    Ctl2Note,
    Feedback,
    Glide,
    Gpio,
    MetaModule,
    MultiCtl,
    MultiSynth,
    Pitch2Ctl,
    PitchDetector,
    Sound2Ctl,
    Velocity2Ctl,
}

/// What a module's handler answers without an instance.
pub(crate) struct Info {
    pub name: &'static str,
    pub flags: u32,
    pub flags2: u32,
    pub inputs: i32,
    pub outputs: i32,
}

use net::{
    FLAG2_GET_MUTED_COMMANDS as MUTED, FLAG2_NOTE_RECEIVER as RECV, FLAG2_NOTE_SENDER as SEND, FLAG_DONT_FILL_INPUT as DFI, FLAG_EFFECT as EFF, FLAG_FEEDBACK as FB, FLAG_GENERATOR as GEN, FLAG_GET_RENDER_SETUP_COMMANDS as RSC,
    FLAG_GET_SPEED_CHANGES as SPD, FLAG_GET_STOP_COMMANDS as STOPC, FLAG_NO_RENDER as NOR, FLAG_NO_SCOPE_BUF as NSB, FLAG_OUTPUT_IS_EMPTY as OIE, FLAG_USE_MUTEX as MTX,
};

const ALL: [(Kind, Info); 43] = [
    (Kind::AnalogGenerator, Info { name: "Analog generator", flags: GEN, flags2: 0, inputs: 0, outputs: 2 }),
    (Kind::DrumSynth, Info { name: "DrumSynth", flags: GEN, flags2: 0, inputs: 0, outputs: 2 }),
    (Kind::Fm, Info { name: "FM", flags: GEN, flags2: 0, inputs: 0, outputs: 2 }),
    (Kind::Fmx, Info { name: "FMX", flags: GEN | EFF, flags2: 0, inputs: 1, outputs: 2 }),
    (Kind::Generator, Info { name: "Generator", flags: GEN | EFF, flags2: 0, inputs: 1, outputs: 2 }),
    (Kind::Input, Info { name: "Input", flags: GEN, flags2: 0, inputs: 0, outputs: 2 }),
    (Kind::Kicker, Info { name: "Kicker", flags: GEN, flags2: 0, inputs: 0, outputs: 2 }),
    (Kind::VorbisPlayer, Info { name: "Vorbis player", flags: GEN | MTX, flags2: 0, inputs: 0, outputs: 2 }),
    (Kind::Sampler, Info { name: "Sampler", flags: GEN | SPD | EFF | MTX, flags2: 0, inputs: 2, outputs: 2 }),
    (Kind::SpectraVoice, Info { name: "SpectraVoice", flags: GEN, flags2: 0, inputs: 0, outputs: 2 }),
    (Kind::Amplifier, Info { name: "Amplifier", flags: EFF, flags2: 0, inputs: 2, outputs: 2 }),
    (Kind::Compressor, Info { name: "Compressor", flags: EFF | DFI, flags2: 0, inputs: 2, outputs: 2 }),
    (Kind::DcBlocker, Info { name: "DC Blocker", flags: EFF, flags2: 0, inputs: 2, outputs: 2 }),
    (Kind::Delay, Info { name: "Delay", flags: EFF | SPD, flags2: SEND | RECV, inputs: 2, outputs: 2 }),
    (Kind::Distortion, Info { name: "Distortion", flags: EFF, flags2: 0, inputs: 2, outputs: 2 }),
    (Kind::Echo, Info { name: "Echo", flags: EFF | SPD, flags2: 0, inputs: 2, outputs: 2 }),
    (Kind::Eq, Info { name: "EQ", flags: EFF, flags2: 0, inputs: 2, outputs: 2 }),
    (Kind::Fft, Info { name: "FFT", flags: EFF, flags2: 0, inputs: 2, outputs: 2 }),
    (Kind::Filter, Info { name: "Filter", flags: EFF | SPD, flags2: RECV, inputs: 2, outputs: 2 }),
    (Kind::FilterPro, Info { name: "Filter Pro", flags: EFF | SPD, flags2: RECV, inputs: 2, outputs: 2 }),
    (Kind::Flanger, Info { name: "Flanger", flags: EFF | SPD, flags2: RECV, inputs: 2, outputs: 2 }),
    (Kind::Lfo, Info { name: "LFO", flags: EFF | SPD, flags2: RECV, inputs: 2, outputs: 2 }),
    (Kind::Loop, Info { name: "Loop", flags: EFF | SPD, flags2: RECV, inputs: 2, outputs: 2 }),
    (Kind::Modulator, Info { name: "Modulator", flags: EFF | DFI, flags2: 0, inputs: 2, outputs: 2 }),
    (Kind::PitchShifter, Info { name: "Pitch shifter", flags: EFF, flags2: RECV, inputs: 2, outputs: 2 }),
    (Kind::Reverb, Info { name: "Reverb", flags: EFF, flags2: 0, inputs: 2, outputs: 2 }),
    (Kind::Smooth, Info { name: "Smooth", flags: EFF, flags2: 0, inputs: 2, outputs: 2 }),
    (Kind::VocalFilter, Info { name: "Vocal filter", flags: EFF, flags2: 0, inputs: 2, outputs: 2 }),
    (Kind::Vibrato, Info { name: "Vibrato", flags: EFF | SPD, flags2: RECV, inputs: 2, outputs: 2 }),
    (Kind::WaveShaper, Info { name: "WaveShaper", flags: EFF, flags2: 0, inputs: 2, outputs: 2 }),
    (Kind::Adsr, Info { name: "ADSR", flags: GEN | EFF, flags2: 0, inputs: 2, outputs: 2 }),
    (Kind::Ctl2Note, Info { name: "Ctl2Note", flags: NSB | STOPC | NOR, flags2: SEND | MUTED, inputs: 0, outputs: 0 }),
    (Kind::Feedback, Info { name: "Feedback", flags: EFF | RSC | FB, flags2: 0, inputs: 2, outputs: 2 }),
    (Kind::Glide, Info { name: "Glide", flags: GEN | NSB | OIE, flags2: SEND, inputs: 0, outputs: 0 }),
    (Kind::Gpio, Info { name: "GPIO", flags: EFF, flags2: 0, inputs: 1, outputs: 1 }),
    (Kind::MetaModule, Info { name: "MetaModule", flags: EFF | MTX, flags2: 0, inputs: 2, outputs: 2 }),
    (Kind::MultiCtl, Info { name: "MultiCtl", flags: EFF | NSB | OIE, flags2: 0, inputs: 0, outputs: 0 }),
    (Kind::MultiSynth, Info { name: "MultiSynth", flags: GEN | NSB | NOR, flags2: SEND | RECV | MUTED, inputs: 0, outputs: 0 }),
    (Kind::Pitch2Ctl, Info { name: "Pitch2Ctl", flags: GEN | NSB | NOR, flags2: RECV, inputs: 0, outputs: 0 }),
    (Kind::PitchDetector, Info { name: "Pitch Detector", flags: EFF | NSB | OIE | STOPC, flags2: SEND | MUTED, inputs: 1, outputs: 0 }),
    (Kind::Sound2Ctl, Info { name: "Sound2Ctl", flags: EFF | NSB | OIE, flags2: 0, inputs: 2, outputs: 0 }),
    (Kind::Velocity2Ctl, Info { name: "Velocity2Ctl", flags: GEN | NSB | NOR, flags2: RECV, inputs: 0, outputs: 0 }),
    (Kind::Empty, EMPTY),
];

const EMPTY: Info = Info { name: "", flags: 0, flags2: 0, inputs: 2, outputs: 2 };

/// The kind's static info (None: the Output module's psynth_empty).
pub(crate) fn info(kind: Option<Kind>) -> &'static Info {
    let kind = kind.unwrap_or(Kind::Empty);
    ALL.iter().find(|(k, _)| *k == kind).map(|(_, i)| i).unwrap_or(&EMPTY)
}

/// get_module_handler_by_name: the kind named, psynth_empty for none.
pub(crate) fn kind_by_name(name: &str) -> Kind {
    ALL.iter().find(|(k, i)| *k != Kind::Empty && i.name == name).map_or(Kind::Empty, |(k, _)| *k)
}

/// Whether this engine plays the kind (the rest render silence).
pub(crate) fn implemented(kind: Kind) -> bool {
    matches!(kind, Kind::Empty | Kind::Generator | Kind::Kicker | Kind::Filter | Kind::Echo | Kind::Distortion | Kind::Flanger | Kind::Reverb | Kind::DcBlocker | Kind::Eq | Kind::Fm | Kind::Vibrato | Kind::Compressor | Kind::DrumSynth | Kind::Amplifier | Kind::Lfo | Kind::MultiSynth | Kind::Delay | Kind::Sampler | Kind::SpectraVoice | Kind::AnalogGenerator | Kind::MetaModule | Kind::VocalFilter | Kind::PitchShifter | Kind::FilterPro | Kind::WaveShaper | Kind::Sound2Ctl | Kind::Loop | Kind::MultiCtl | Kind::Feedback | Kind::Modulator | Kind::Glide | Kind::VorbisPlayer | Kind::Smooth | Kind::Adsr | Kind::Ctl2Note | Kind::Velocity2Ctl | Kind::Pitch2Ctl | Kind::Input | Kind::Gpio | Kind::Fft | Kind::PitchDetector | Kind::Fmx)
}

/// A module of the kind, its data zeroed (as SMEM_ZALLOC leaves it).
pub(crate) fn make(kind: Option<Kind>) -> Box<dyn Module> {
    match kind.unwrap_or(Kind::Empty) {
        Kind::Generator => Box::<generator::Generator>::default(),
        Kind::Kicker => Box::<kicker::Kicker>::default(),
        Kind::Filter => Box::<filter::Filter>::default(),
        Kind::Echo => Box::<echo::Echo>::default(),
        Kind::Distortion => Box::<distortion::Distortion>::default(),
        Kind::Flanger => Box::<flanger::Flanger>::default(),
        Kind::Reverb => Box::<reverb::Reverb>::default(),
        Kind::DcBlocker => Box::<dc_blocker::DcBlocker>::default(),
        Kind::Eq => Box::<eq::Eq>::default(),
        Kind::Fm => Box::<fm::Fm>::default(),
        Kind::Vibrato => Box::<vibrato::Vibrato>::default(),
        Kind::Compressor => Box::<compressor::Compressor>::default(),
        Kind::DrumSynth => Box::<drumsynth::DrumSynth>::default(),
        Kind::Amplifier => Box::<amplifier::Amplifier>::default(),
        Kind::Lfo => Box::<lfo::Lfo>::default(),
        Kind::MultiSynth => Box::<multisynth::MultiSynth>::default(),
        Kind::Delay => Box::<delay::Delay>::default(),
        Kind::Sampler => Box::<sampler::Sampler>::default(),
        Kind::Smooth => Box::<smooth::Smooth>::default(),
        Kind::Adsr => Box::<adsr::Adsr>::default(),
        Kind::Ctl2Note => Box::<ctl2note::Ctl2Note>::default(),
        Kind::Velocity2Ctl => Box::new(note2ctl::Note2Ctl::new(note2ctl::Which::Velocity)),
        Kind::Pitch2Ctl => Box::new(note2ctl::Note2Ctl::new(note2ctl::Which::Pitch)),
        Kind::Input => Box::new(input_gpio::Silent(input_gpio::Which::Input)),
        Kind::Gpio => Box::new(input_gpio::Silent(input_gpio::Which::Gpio)),
        Kind::Fft => Box::<fft::Fft>::default(),
        Kind::PitchDetector => Box::<pitch_detector::PitchDetector>::default(),
        Kind::Fmx => Box::<fmx::Fmx>::default(),
        Kind::VorbisPlayer => Box::<vorbis_player::VorbisPlayer>::default(),
        Kind::Sound2Ctl => Box::<sound2ctl::Sound2Ctl>::default(),
        Kind::Loop => Box::<looper::Looper>::default(),
        Kind::MultiCtl => Box::<multictl::MultiCtl>::default(),
        Kind::Feedback => Box::<feedback::Feedback>::default(),
        Kind::Modulator => Box::<modulator::Modulator>::default(),
        Kind::Glide => Box::<glide::Glide>::default(),
        Kind::FilterPro => Box::<filter_pro::FilterPro>::default(),
        Kind::WaveShaper => Box::<waveshaper::WaveShaper>::default(),
        Kind::VocalFilter => Box::<vocal_filter::VocalFilter>::default(),
        Kind::PitchShifter => Box::<pitch_shifter::PitchShifter>::default(),
        Kind::MetaModule => Box::<metamodule::MetaModule>::default(),
        Kind::AnalogGenerator => Box::<analog_generator::AnalogGenerator>::default(),
        Kind::SpectraVoice => Box::<spectravoice::SpectraVoice>::default(),
        _ => Box::new(EmptyModule),
    }
}

/// psynth_empty.
struct EmptyModule;

impl Module for EmptyModule {
    fn handle(&mut self, _n: usize, _e: &mut net::Event, _net: &mut Net) -> isize {
        0
    }
}
