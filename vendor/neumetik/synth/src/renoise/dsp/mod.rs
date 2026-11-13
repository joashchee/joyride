// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! Renoise's effect devices, each written here from what Renoise's manual
//! says it does, its settings the song's (their names and units the
//! song's XML gives). A device processes a tick's block in place.

use super::song::Device;

mod delay;
mod dynamics;
mod eq;
mod filters;
mod modfx;
mod reverb;
mod shape;

/// What a device knows of the song as it plays.
#[derive(Clone, Copy, Debug)]
pub struct Ctx {
    pub rate: f32,
    pub bpm: f32,
    /// A line's length in seconds.
    pub line: f32,
}

/// A device's settings as they are now: its parameters (which automation
/// and commands change) by name, and its other settings.
pub struct Params<'a> {
    pub def: &'a Device,
    pub values: &'a [f32],
}

impl Params<'_> {
    pub fn get(&self, name: &str, fallback: f32) -> f32 {
        self.def.params.iter().position(|(n, _)| n == name).and_then(|i| self.values.get(i).copied()).unwrap_or(fallback)
    }

    pub fn text(&self, name: &str) -> Option<&str> {
        self.def.text(name)
    }

    pub fn flag(&self, name: &str) -> bool {
        self.def.flag(name)
    }
}

/// A device's processing.
pub trait Proc: Send {
    fn process(&mut self, p: &Params, l: &mut [f32], r: &mut [f32], ctx: &Ctx);
    /// A "reset" (the hidden parameter some devices have): their modulation restarts.
    fn reset(&mut self, _at: f32) {}
}

/// The processor for a device kind; None for a device that leaves the
/// audio as it is here (meta devices, routing handled by the player,
/// plugins).
pub fn make(kind: &str) -> Option<Box<dyn Proc>> {
    Some(match kind {
        "DelayDevice" => Box::new(delay::Delay::new()),
        "MultitapDevice" => Box::new(delay::Multitap::new()),
        "RepeaterDevice" => Box::new(delay::Repeater::new()),
        "ReverbDevice" | "Reverb" => Box::new(reverb::Reverb::new()),
        "Reverb2Device" | "Reverb2Document" => Box::new(reverb::MpReverb::new(false)),
        "Reverb3Device" => Box::new(reverb::MpReverb::new(true)),
        "ConvolverDevice" => Box::new(reverb::Convolver::new()),
        "CompressorDevice" => Box::new(dynamics::Comp::new()),
        "ShaperDevice" => Box::new(dynamics::Comp::shaper()),
        "BusCompressorDevice" => Box::new(dynamics::Comp::bus()),
        "GateDevice" => Box::new(dynamics::Gate::new(false)),
        "Gate2Device" => Box::new(dynamics::Gate::new(true)),
        "MaximizerDevice" => Box::new(dynamics::Maximizer::new()),
        "Eq5Device" => Box::new(eq::Eq::new(5)),
        "Eq10Device" | "Eq10Document" | "Eq" => Box::new(eq::Eq::new(10)),
        "MixerEqDevice" => Box::new(eq::MixerEq::new()),
        "ExciterDevice" => Box::new(eq::Exciter::new()),
        "CabinetSimulatorDevice" => Box::new(eq::Cabinet::new()),
        "Filter1Device" | "Filter2Device" | "FilterDspDocument" => Box::new(filters::BankFilter::new(true)),
        "Filter3Device" => Box::new(filters::BankFilter::new(false)),
        "FilterDistortionDevice" => Box::new(filters::Scream::new()),
        "AnalogFilterDevice" => Box::new(filters::ModernFilter::new(true)),
        "DigitalFilterDevice" => Box::new(filters::ModernFilter::new(false)),
        "CombDevice" => Box::new(filters::Comb::new()),
        "ChorusDevice" => Box::new(modfx::Sweep::chorus(false)),
        "Chorus2Device" => Box::new(modfx::Sweep::chorus(true)),
        "FlangerDevice" | "FlangerDocument" => Box::new(modfx::Sweep::flanger()),
        "PhaserDevice" => Box::new(modfx::Phaser::new()),
        "RingModDevice" => Box::new(modfx::RingMod::new()),
        "DistortionDevice" | "DistortionDocument" => Box::new(shape::Distortion::new(false)),
        "Distortion2Device" => Box::new(shape::Distortion::new(true)),
        "LofiDevice" | "LoFiDocument" => Box::new(shape::Lofi::new(false)),
        "Lofi2Device" => Box::new(shape::Lofi::new(true)),
        "DcOffsetDevice" | "DCOffsetDocument" => Box::new(shape::DcOffset::new()),
        "StereoExpanderDevice" => Box::new(shape::Expander::new()),
        _ => return None,
    })
}

/// A balance pan's gains (0 left, 0.5 centre at full on both, 1 right).
pub fn balance(pan: f32) -> (f32, f32) {
    let p = pan.clamp(0.0, 1.0);
    ((2.0 * (1.0 - p)).min(1.0), (2.0 * p).min(1.0))
}
