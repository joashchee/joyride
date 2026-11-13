// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! The range of each device parameter, which automation (0 to 1) and the
//! pattern's device commands (00 to FF) are scaled over. Renoise doesn't
//! publish them: these are the ranges its manuals give and its songs'
//! values fall in, frequencies in Hz scaled by octaves.

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Curve {
    Linear,
    /// Octaves: a frequency in Hz (or a time) scaled logarithmically.
    Log,
}

/// A parameter's least and most value and how 0..1 spans them.
pub fn range(kind: &str, name: &str, doc: u32) -> (f32, f32, Curve) {
    use Curve::*;
    let mixer = kind.contains("Mixer") || kind.starts_with("Sequencer") || kind == "LineInDevice";
    if mixer {
        return match name {
            "Volume" | "PostVolume" => (0.0, 1.412_538, Linear),
            "Surround" if doc < 54 => (0.0, 127.0, Linear),
            "BpmAutomator" => (20.0, 999.0, Linear),
            "LpbAutomator" => (1.0, 256.0, Linear),
            "TplAutomator" => (1.0, 16.0, Linear),
            _ => (0.0, 1.0, Linear),
        };
    }
    let freq = name.starts_with("Frequency") && kind.starts_with("Eq") || kind == "CabinetSimulatorDevice" && name.starts_with("Frequency");
    if freq {
        return (20.0, 20000.0, Log);
    }
    if name.starts_with("Gain") && kind.starts_with("Eq") || kind == "CabinetSimulatorDevice" && name.starts_with("Gain") {
        return (-20.0, 20.0, Linear);
    }
    if name.starts_with("BandWidth") {
        return (0.0001, 4.0, Linear);
    }
    match (kind, name) {
        ("GainerDevice" | "GainerDocument", "Volume") => (0.0, 4.0, Linear),
        ("SendDevice" | "SequencerTrackSendDevice" | "Device", "DestSendTrack") => (0.0, 64.0, Linear),
        ("DelayDevice", "TrackSend") => (0.0, 127.0, Linear),
        ("DelayDevice", "LDelay" | "RDelay") => (1.0, 2000.0, Linear),
        ("DelayDevice", "LSyncTime" | "RSyncTime") => (1.0, 32.0, Linear),
        ("DelayDevice", "LSyncOffset" | "RSyncOffset") => (-1.0, 1.0, Linear),
        ("MultitapDevice", n) if n.ends_with("DelayLeft") || n.ends_with("DelayRight") => (1.0, 5000.0, Linear),
        ("MultitapDevice", n) if n.ends_with("SyncTime") => (1.0, 32.0, Linear),
        ("MultitapDevice", n) if n.ends_with("SyncOffset") || n.ends_with("Feedback") => (-1.0, 1.0, Linear),
        ("MultitapDevice", n) if n.ends_with("FilterResonance") => (0.0, 20.0, Linear),
        ("MultitapDevice", n) if n.ends_with("FilterType") => (0.0, 3.0, Linear),
        ("MultitapDevice", n) if n.ends_with("FilterMode") => (0.0, 4.0, Linear),
        ("Filter1Device" | "Filter2Device" | "FilterDspDocument", "Cutoff" | "Resonance") => (0.0, 127.0, Linear),
        ("Filter1Device" | "Filter2Device" | "FilterDspDocument", "Type") => (0.0, 55.0, Linear),
        ("Filter3Device", "Type") => (0.0, 5.0, Linear),
        ("Filter3Device", "Q") => (0.0, 14.0, Linear),
        ("Filter3Device", "Gain") => (-60.0, 12.0, Linear),
        ("MixerEqDevice", "LowGain" | "MidGain" | "HiGain") => (-18.0, 18.0, Linear),
        ("CompressorDevice", "LogThreshold") => (0.0, 15.0, Linear),
        ("CompressorDevice", "Ratio") => (1.0, 16.0, Linear),
        ("CompressorDevice", "Attack") => (0.001, 10.0, Log),
        ("CompressorDevice", "Release") => (0.001, 100.0, Log),
        ("CompressorDevice", "Gain") => (0.0, 6.0, Linear),
        ("ShaperDevice", "Threshold") => (0.0, 32768.0, Linear),
        ("ShaperDevice", "Ratio") => (1.0, 20.0, Linear),
        ("ShaperDevice", "Attack" | "Release") => (1.0, 100_000.0, Log),
        ("ShaperDevice", "Gain") => (0.0, 4.0, Linear),
        ("BusCompressorDevice", "Threshold") => (-60.0, 0.0, Linear),
        ("BusCompressorDevice", "MakeUp") => (0.0, 24.0, Linear),
        ("BusCompressorDevice", "Knee") => (0.0, 30.0, Linear),
        ("MaximizerDevice", "InputGain") => (0.0, 24.0, Linear),
        ("MaximizerDevice", "Threshold") => (-36.0, 0.0, Linear),
        ("MaximizerDevice", "TransientRelease") => (1.0, 100.0, Linear),
        ("MaximizerDevice", "LongTermRelease") => (1.0, 1000.0, Linear),
        ("MaximizerDevice", "Ceiling") => (-12.0, 3.0, Linear),
        ("GateDevice", "AttackInMs") => (0.01, 1000.0, Log),
        ("GateDevice", "HoldInMs" | "ReleaseInMs") => (0.0, 5000.0, Linear),
        ("Gate2Device", "Threshold") => (-100.0, 0.0, Linear),
        ("Gate2Device", "Attack") => (0.01, 1000.0, Log),
        ("Gate2Device", "Hold" | "Release") => (0.0, 5000.0, Linear),
        ("Gate2Device", "SideChainHpFreq" | "SideChainLpFreq") => (0.0, 127.0, Linear),
        ("Reverb2Device" | "Reverb2Document" | "Reverb3Device", "ReverbTime") => (1.0, 99.0, Linear),
        ("Reverb2Device" | "Reverb2Document" | "Reverb3Device", "LPFilter") => (10.0, 5000.0, Log),
        ("Reverb2Device" | "Reverb2Document" | "Reverb3Device", "PreDelay") => (0.0, 250.0, Linear),
        ("Reverb3Device", "LPGain") => (-18.0, 18.0, Linear),
        ("ChorusDevice" | "Chorus2Device", "LfoRate" | "Rate") => (0.01, 10.0, Log),
        ("ChorusDevice" | "Chorus2Device" | "FlangerDevice" | "FlangerDocument", "Feedback" | "FeedBack") => (-1.0, 1.0, Linear),
        ("ChorusDevice" | "Chorus2Device" | "FlangerDevice", "Phase" | "Dephase") => (0.0, std::f32::consts::PI, Linear),
        ("ChorusDevice" | "Chorus2Device" | "FlangerDevice", "FilterType") => (0.0, 19.0, Linear),
        ("ChorusDevice" | "Chorus2Device" | "FlangerDevice", "FilterFreq" | "FilterResonance" | "FilterDrive") => (0.0, 127.0, Linear),
        ("FlangerDevice" | "FlangerDocument", "Amount") => (-1.0, 1.0, Linear),
        ("FlangerDevice" | "FlangerDocument", "Rate") => (0.0, 5.0, Linear),
        ("FlangerDevice" | "FlangerDocument", "Amplitude") => (0.0, 0.04, Linear),
        ("FlangerDevice" | "FlangerDocument", "Delay") => (0.0, 100.0, Linear),
        ("PhaserDevice", "Ceilling" | "Floor") => (20.0, 20000.0, Log),
        ("PhaserDevice", "LFORate") => (0.01, 10.0, Log),
        ("PhaserDevice", "Depth") => (0.0, 8.0, Linear),
        ("PhaserDevice", "Phase") => (0.0, std::f32::consts::TAU, Linear),
        ("PhaserDevice", "Stages") => (1.0, 24.0, Linear),
        ("RingModDevice", "OscillatorType") => (0.0, 4.0, Linear),
        ("RingModDevice", "Phase") => (0.0, 360.0, Linear),
        ("CombDevice", "Feedback") => (-1.0, 1.0, Linear),
        ("DistortionDevice" | "DistortionDocument", "Threshold" | "LpOrClamp") => (0.0, 32768.0, Linear),
        ("DistortionDevice" | "DistortionDocument", "WetOut" | "DryOut") => (0.0, 128.0, Linear),
        ("Distortion2Device", "Type") => (0.0, 3.0, Linear),
        ("LofiDevice" | "LoFiDocument" | "Lofi2Device", "BitCrunsh" | "Bits") => (1.0, 16.0, Linear),
        ("LofiDevice" | "LoFiDocument", "Noise" | "WetOut" | "DryOut") => (0.0, 100.0, Linear),
        ("DcOffsetDevice" | "DCOffsetDocument", "DCOffset") => (-32768.0, 32768.0, Linear),
        ("LfoDevice", "DestTrack" | "DestEffect" | "DestParameter") => (-1.0, 64.0, Linear),
        ("LfoDevice", "Offset") => (-1.0, 1.0, Linear),
        ("LfoDevice", "Frequency") => (0.0001, 60.0, Log),
        ("LfoDevice", "Type") => (0.0, 4.0, Linear),
        ("CabinetSimulatorDevice", "Cabinet") => (0.0, 17.0, Linear),
        ("CabinetSimulatorDevice", "Routing") => (0.0, 3.0, Linear),
        ("MasterTrackMixerDevice", "BpmAutomator") => (20.0, 999.0, Linear),
        ("DooferDevice", _) => (0.0, 100.0, Linear),
        ("SignalFollowerDevice", "LpFreq" | "HpFreq") => (0.0, 127.0, Linear),
        ("RepeaterDevice", "Mode") => (0.0, 4.0, Linear),
        ("RepeaterDevice", "Divisor") => (1.0 / 128.0, 1.0, Log),
        (_, n) if n.starts_with("Out") && (n.ends_with("DestTrack") || n.ends_with("DestEffect") || n.ends_with("DestParameter")) => (-1.0, 64.0, Linear),
        _ => (0.0, 1.0, Linear),
    }
}

/// A parameter's value at `norm` (0..1) of its range.
pub fn value(kind: &str, name: &str, doc: u32, norm: f32) -> f32 {
    let (lo, hi, curve) = range(kind, name, doc);
    let n = norm.clamp(0.0, 1.0);
    match curve {
        Curve::Linear => lo + (hi - lo) * n,
        Curve::Log => lo * (hi / lo).powf(n),
    }
}
