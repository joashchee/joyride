// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! A Renoise song read from its Song.xml, every version's layout (the
//! file's doc_version, 2 to 66 seen) into one model: its global data, its
//! tracks (with their groups and devices), its instruments (samples, key
//! zones, the envelopes of Renoise 1 and 2 or the modulation sets and
//! effect chains of Renoise 3.1 on), its patterns and its sequence.

use std::sync::Arc;

use crate::xml::Node;

/// No note, instrument or value in a cell.
pub const NONE: u8 = 255;
/// A note off.
pub const OFF: u8 = 120;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrackKind {
    Normal,
    Group,
    Send,
    Master,
}

/// A device: its kind (the file's type name), whether it's on, its
/// automatable parameters in the file's order (which automation and the
/// pattern's device commands count them by), and its other settings.
#[derive(Clone, Debug, Default)]
pub struct Device {
    pub kind: String,
    pub active: bool,
    pub params: Vec<(String, f32)>,
    pub extra: Vec<(String, String)>,
    /// A Doofer's devices.
    pub inner: Vec<Device>,
    /// A modulation envelope's or stepper's curve.
    pub env: Option<Envelope>,
}

impl Device {
    pub fn param(&self, name: &str) -> Option<f32> {
        self.params.iter().find(|(n, _)| n == name).map(|&(_, v)| v)
    }

    pub fn get(&self, name: &str, fallback: f32) -> f32 {
        self.param(name).unwrap_or(fallback)
    }

    pub fn text(&self, name: &str) -> Option<&str> {
        self.extra.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str())
    }

    pub fn flag(&self, name: &str) -> bool {
        self.text(name).is_some_and(truth)
    }
}

#[derive(Clone, Debug)]
pub struct Track {
    pub kind: TrackKind,
    pub name: String,
    /// Off or muted tracks are silent.
    pub on: bool,
    pub note_columns: usize,
    pub fx_columns: usize,
    /// Each note column's "Off" state.
    pub column_off: Vec<bool>,
    /// -1 its group, 0 the master (and the sound card's outputs).
    pub routing: i32,
    pub group: Option<usize>,
    pub delay_ms: f32,
    pub devices: Vec<Device>,
}

/// A volume or panning column's value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Col {
    #[default]
    None,
    /// 00 to 80.
    Set(u8),
    /// A command letter (Renoise 3's, or the older hex code read as one)
    /// and its digit.
    Cmd(u8, u8),
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NoteCell {
    /// 0 C-0 to 119 B-9, OFF, NONE.
    pub note: u8,
    pub instr: u8,
    pub vol: Col,
    pub pan: Col,
    /// 0 to 255 of a line.
    pub delay: u8,
    /// Renoise 3's sample effect column: two characters and a value.
    pub fx: Option<([u8; 2], u8)>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct FxCell {
    pub num: [u8; 2],
    pub val: u8,
}

#[derive(Clone, Debug, Default)]
pub struct Line {
    pub notes: Vec<NoteCell>,
    pub fx: Vec<FxCell>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interp {
    Points,
    Linear,
    Curve,
}

/// A graphical automation of a device's parameter in a pattern track.
#[derive(Clone, Debug)]
pub struct Automation {
    pub device: usize,
    pub param: usize,
    pub interp: Interp,
    /// Points: (line, 0..1), in time order.
    pub points: Vec<(f64, f32)>,
}

#[derive(Clone, Debug, Default)]
pub struct PatternTrack {
    /// Lines with something in them, in order.
    pub lines: Vec<(usize, Line)>,
    pub automation: Vec<Automation>,
    /// Another pattern whose track this one plays (an alias).
    pub alias: Option<usize>,
}

#[derive(Clone, Debug, Default)]
pub struct Pattern {
    pub name: String,
    pub lines: usize,
    pub tracks: Vec<PatternTrack>,
}

#[derive(Clone, Debug, Default)]
pub struct Entry {
    pub pattern: usize,
    pub muted: Vec<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nna {
    Cut,
    Off,
    Continue,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Loop {
    Off,
    Forward,
    Backward,
    PingPong,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleInterp {
    None,
    Linear,
    Cubic,
    Sinc,
}

/// A sample's audio: each channel's points (-1..1) at its rate.
#[derive(Clone, Debug, Default)]
pub struct Audio {
    pub rate: u32,
    pub channels: Vec<Vec<f32>>,
}

impl Audio {
    pub fn frames(&self) -> usize {
        self.channels.first().map_or(0, Vec::len)
    }
}

#[derive(Clone, Debug)]
pub struct Sample {
    pub name: String,
    pub audio: Arc<Audio>,
    /// Linear.
    pub volume: f32,
    /// 0 left, 0.5 centre, 1 right.
    pub pan: f32,
    /// The note it plays at its own pitch (48 C-4), Renoise 1 and 2.
    pub base_note: i32,
    pub transpose: i32,
    /// -127 to 127, 1/128ths of a semitone.
    pub finetune: i32,
    pub nna: Nna,
    pub interp: SampleInterp,
    pub loop_mode: Loop,
    pub loop_start: usize,
    pub loop_end: usize,
    pub loop_release: bool,
    /// Pitched to last this many lines.
    pub beat_sync: Option<u32>,
    pub autofade: bool,
    pub oneshot: bool,
    pub mute_group: i32,
    pub mod_set: i32,
    pub chain: i32,
    /// A slice of the instrument's first sample (Renoise 2.8 on): its start.
    pub alias: bool,
    pub slices: Vec<usize>,
}

/// Where a note plays a sample (a key zone).
#[derive(Clone, Copy, Debug)]
pub struct Zone {
    pub sample: usize,
    pub base_note: i32,
    pub notes: (u8, u8),
    pub vels: (u8, u8),
    pub vel_to_volume: bool,
    pub key_to_pitch: bool,
    /// Played on note off rather than note on.
    pub off_layer: bool,
}

/// An envelope of Renoise 1 and 2's instruments (volume, panning, pitch,
/// cutoff, resonance), with its LFOs.
#[derive(Clone, Debug, Default)]
pub struct Envelope {
    pub on: bool,
    pub interp: Option<Interp>,
    pub sustain: Option<usize>,
    pub loop_mode: Option<Loop>,
    pub loop_range: (usize, usize),
    /// The volume envelope's fadeout (Decay), 0 to 255(?), 128 by default.
    pub decay: i32,
    /// Length in envelope ticks (24 a beat).
    pub length: usize,
    pub points: Vec<(usize, f32)>,
    pub bipolar: bool,
    pub lfos: Vec<Lfo>,
    /// The filter envelopes' auto amp (attack, release, amount), when on.
    pub auto_amp: Option<(i32, i32, i32)>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Lfo {
    /// 0 off, 1 sine, 2 saw, 3 pulse, 4 random.
    pub mode: u8,
    pub freq: i32,
    pub amp: i32,
    pub phase: i32,
}

#[derive(Clone, Debug, Default)]
pub struct Envelopes {
    pub volume: Envelope,
    pub pan: Envelope,
    pub pitch: Envelope,
    pub cutoff: Envelope,
    pub resonance: Envelope,
    pub filter_type: i32,
}

/// A Renoise 3 modulation set: its devices, in order, and its filter.
#[derive(Clone, Debug, Default)]
pub struct ModSet {
    pub devices: Vec<Device>,
    pub filter_type: i32,
    pub filter_version: i32,
    pub pitch_range: f32,
}

#[derive(Clone, Debug, Default)]
pub struct Phrase {
    pub lines: Vec<(usize, Line)>,
    pub length: usize,
    pub lpb: u32,
    pub looping: bool,
    pub loop_range: (usize, usize),
    pub note_columns: usize,
    /// The phrase's note range in keymap mode, and its base note.
    pub keys: (u8, u8),
    pub base_note: i32,
}

#[derive(Clone, Debug, Default)]
pub struct Instrument {
    pub name: String,
    pub samples: Vec<Sample>,
    pub zones: Vec<Zone>,
    pub envelopes: Option<Arc<Envelopes>>,
    pub mod_sets: Vec<ModSet>,
    pub chains: Vec<Vec<Device>>,
    pub volume: f32,
    pub transpose: i32,
    /// A plugin or MIDI instrument (silent here).
    pub plugin: Option<String>,
    pub phrases: Vec<Phrase>,
    /// 0 off, 1 keymap, 2 selective.
    pub phrase_mode: u8,
    /// The phrase a selective instrument plays, when one's selected.
    pub phrase_selected: Option<usize>,
    pub macros: [f32; 8],
    /// The instrument as its XML, its name and what's only the editor's
    /// (file names and dates, selections, views) left out: its patch.
    pub xml: String,
}

#[derive(Clone, Debug, Default)]
pub struct Song {
    pub doc: u32,
    pub name: String,
    pub artist: String,
    pub comments: Vec<String>,
    pub bpm: f32,
    pub lpb: u32,
    pub tpl: u32,
    /// Renoise 1 timing (before 2.0): a tick lasts 2.5/BPM s, a line TPL ticks.
    pub speed_timing: bool,
    pub shuffle: Option<[f32; 4]>,
    pub headroom: f32,
    pub amiga_offsets: bool,
    pub amiga_pitch: bool,
    pub tracks: Vec<Track>,
    pub instruments: Vec<Instrument>,
    pub patterns: Vec<Pattern>,
    pub sequence: Vec<Entry>,
}

pub(crate) fn truth(s: &str) -> bool {
    let s = s.trim();
    s.eq_ignore_ascii_case("true") || s.eq_ignore_ascii_case("yes") || s == "1"
}

fn text(n: &Node, name: &str) -> String {
    n.child(name).map(|c| c.all_text().trim().to_string()).unwrap_or_default()
}

fn num(n: &Node, name: &str, fallback: f32) -> f32 {
    let t = text(n, name);
    if t.is_empty() {
        return fallback;
    }
    t.parse::<f32>().unwrap_or(fallback)
}

fn int(n: &Node, name: &str, fallback: i32) -> i32 {
    let t = text(n, name);
    if t.is_empty() {
        return fallback;
    }
    t.parse::<f64>().map(|v| v as i32).unwrap_or(fallback)
}

fn flag(n: &Node, name: &str) -> bool {
    truth(&text(n, name))
}

fn hex(s: &str) -> Option<u8> {
    u8::from_str_radix(s, 16).ok()
}

/// A note's name ("C-4", "C#4", "OFF") as a number.
fn note(s: &str) -> u8 {
    let b = s.as_bytes();
    if s.starts_with("OFF") {
        return OFF;
    }
    if b.len() < 3 {
        return NONE;
    }
    let base = match b[0] {
        b'C' => 0,
        b'D' => 2,
        b'E' => 4,
        b'F' => 5,
        b'G' => 7,
        b'A' => 9,
        b'B' => 11,
        _ => return NONE,
    };
    let sharp = i32::from(b[1] == b'#');
    let Some(oct) = (b[2] as char).to_digit(10) else { return NONE };
    let n = base + sharp + oct as i32 * 12;
    if (0..120).contains(&n) {
        n as u8
    } else {
        NONE
    }
}

/// A volume or panning column's two characters, as the file's version means them.
pub(crate) fn column(s: &str, doc: u32, panning: bool) -> Col {
    let b = s.as_bytes();
    if b.len() < 2 || b[0] == b'.' {
        return Col::None;
    }
    let digit = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
    if doc >= 37 {
        if let Some(v) = hex(s) {
            if v <= 0x80 {
                return Col::Set(v);
            }
        }
        return match digit(b[1]) {
            Some(d) if b[0].is_ascii_uppercase() => Col::Cmd(b[0], d),
            _ => Col::None,
        };
    }
    // Renoise 1 and 2: 00-80 a value, 9x to Fx commands (read as Renoise 3's letters).
    let Some(v) = hex(s) else { return Col::None };
    if v <= 0x80 {
        return Col::Set(v);
    }
    let d = v & 15;
    match v >> 4 {
        8 => Col::Set(0x80),
        9 => Col::Cmd(if panning { b'J' } else { b'I' }, d),
        0xa => Col::Cmd(if panning { b'K' } else { b'O' }, d),
        0xb => Col::Cmd(b'B', d),
        // MIDI commands (C0-C3) in the panning column; nothing here.
        0xc => Col::None,
        0xd => Col::Cmd(b'Q', d),
        0xe => Col::Cmd(b'R', d),
        _ => Col::Cmd(b'C', d),
    }
}

fn two(s: &str) -> [u8; 2] {
    let b = s.as_bytes();
    [b.first().copied().unwrap_or(b'0').to_ascii_uppercase(), b.get(1).copied().unwrap_or(b'0').to_ascii_uppercase()]
}

fn read_line(n: &Node, doc: u32) -> Line {
    let mut line = Line::default();
    if let Some(nc) = n.child("NoteColumns") {
        for c in &nc.children {
            let mut cell = NoteCell {
                note: NONE,
                instr: NONE,
                ..Default::default()
            };
            if c.name == "NoteColumn" {
                cell.note = note(&text(c, "Note"));
                cell.instr = hex(&text(c, "Instrument")).unwrap_or(NONE);
                cell.vol = column(&text(c, "Volume"), doc, false);
                cell.pan = column(&text(c, "Panning"), doc, true);
                cell.delay = hex(&text(c, "Delay")).unwrap_or(0);
                let (en, ev) = (text(c, "EffectNumber"), text(c, "EffectValue"));
                if !en.is_empty() || !ev.is_empty() {
                    cell.fx = Some((two(if en.is_empty() { "00" } else { &en }), hex(&ev).unwrap_or(0)));
                }
            }
            line.notes.push(cell);
        }
    }
    if let Some(ec) = n.child("EffectColumns") {
        for c in &ec.children {
            let (num, val) = (text(c, "Number"), text(c, "Value"));
            line.fx.push(FxCell {
                num: two(if num.is_empty() { "00" } else { &num }),
                val: hex(&val).unwrap_or(0),
            });
        }
    }
    line
}

/// Renoise 1.5's packed lines ("12:C-40040..,0F81"): a note column is nine
/// characters (note, instrument, volume, panning), an effect four.
fn packed_line(s: &str, doc: u32) -> Option<(usize, Line)> {
    let (at, rest) = s.split_once(':')?;
    let at: usize = at.trim().parse().ok()?;
    let mut line = Line::default();
    for part in rest.split(',') {
        if part.len() >= 9 {
            line.notes.push(NoteCell {
                note: note(&part[0..3]),
                instr: hex(&part[3..5]).unwrap_or(NONE),
                vol: column(&part[5..7], doc, false),
                pan: column(&part[7..9], doc, true),
                delay: 0,
                fx: None,
            });
        } else if part.len() == 4 {
            line.fx.push(FxCell {
                num: two(&part[0..2]),
                val: hex(&part[2..4]).unwrap_or(0),
            });
        }
    }
    Some((at, line))
}

fn read_lines(lines: &Node, doc: u32) -> Vec<(usize, Line)> {
    let mut out: Vec<(usize, Line)> = Vec::new();
    for l in lines.children("Line") {
        if l.children.is_empty() {
            if let Some(x) = packed_line(&l.all_text(), doc) {
                out.push(x);
            }
            continue;
        }
        let Some(at) = l.attrs.iter().find(|(k, _)| k == "index").and_then(|(_, v)| v.parse().ok()) else {
            continue;
        };
        out.push((at, read_line(l, doc)));
    }
    out.sort_by_key(|(at, _)| *at);
    out
}

fn interp(s: &str) -> Interp {
    match s {
        "Points" => Interp::Points,
        "Linear" => Interp::Linear,
        _ => Interp::Curve,
    }
}

fn read_automation(e: &Node, pattern_lines: usize) -> Option<Automation> {
    let device = text(e, "DeviceIndex").parse().ok()?;
    let param = text(e, "ParameterIndex").parse().ok()?;
    let env = e.child("Envelope")?;
    let length = int(env, "Length", pattern_lines as i32).max(1) as f64;
    let per_line = length / pattern_lines.max(1) as f64;
    let mut points: Vec<(f64, f32)> = env
        .child("Points")?
        .children("Point")
        .filter_map(|p| {
            let t = p.all_text();
            let (a, b) = t.split_once(',')?;
            Some((a.trim().parse::<f64>().ok()? / per_line, b.trim().parse::<f32>().ok()?))
        })
        .collect();
    points.sort_by(|a, b| a.0.total_cmp(&b.0));
    Some(Automation {
        device,
        param,
        interp: interp(&text(env, "PlayMode")),
        points,
    })
}

/// A device's parameters: every child with a value (but its on switch).
pub(crate) fn read_device(d: &Node) -> Device {
    let kind = d.attrs.iter().find(|(k, _)| k == "type").map_or_else(|| d.name.clone(), |(_, v)| v.clone());
    let mut dev = Device {
        kind,
        active: true,
        ..Default::default()
    };
    for c in &d.children {
        if let Some(v) = c.child("Value") {
            let value: f32 = v.all_text().trim().parse().unwrap_or(0.0);
            if c.name == "IsActive" {
                dev.active = value >= 0.5;
            } else {
                dev.params.push((c.name.clone(), value));
            }
        } else if c.name == "IsActive" {
            dev.active = truth(&c.all_text());
        } else if c.name == "Devices" {
            dev.inner = c.children.iter().map(read_device).collect();
        } else if c.name == "Nodes" {
            dev.env = Some(envelope(Some(d)));
        } else if c.name == "CustomEnvelope" {
            // The LFO's custom wave: its points over its length.
            let mut e = Envelope {
                length: int(c, "Length", 64).max(1) as usize,
                interp: Some(interp(&text(c, "PlayMode"))),
                on: true,
                ..Default::default()
            };
            if let Some(ps) = c.child("Points") {
                e.points = ps
                    .children("Point")
                    .filter_map(|p| {
                        let t = p.all_text();
                        let (a, b) = t.split_once(',')?;
                        Some((a.trim().parse::<f64>().ok()?.max(0.0) as usize, b.trim().parse().ok()?))
                    })
                    .collect();
            }
            dev.env = Some(e);
        } else if c.children.is_empty() {
            dev.extra.push((c.name.clone(), c.all_text().trim().to_string()));
        } else if c.name == "Parameters" {
            // A plugin's parameters: kept, unnamed, for automation's count.
            for p in c.children("Parameter") {
                dev.params.push((String::new(), num(p, "Value", 0.0)));
            }
        }
    }
    dev
}

fn devices(n: Option<&Node>) -> Vec<Device> {
    n.map_or_else(Vec::new, |d| d.children.iter().map(read_device).collect())
}

fn lfo(n: Option<&Node>) -> Lfo {
    let Some(n) = n else { return Lfo::default() };
    let mode = match text(n, "Mode").as_str() {
        "Sin" => 1,
        "Saw" => 2,
        "Pulse" => 3,
        "Random" => 4,
        _ => 0,
    };
    Lfo {
        mode,
        freq: int(n, "Frequency", 3),
        amp: int(n, "Amplitude", 27),
        phase: int(n, "Dephase", 0),
    }
}

fn loop_mode(s: &str) -> Loop {
    match s {
        "Forward" => Loop::Forward,
        "Backward" => Loop::Backward,
        "PingPong" => Loop::PingPong,
        _ => Loop::Off,
    }
}

fn envelope(n: Option<&Node>) -> Envelope {
    let Some(n) = n else { return Envelope::default() };
    let nodes = n.child("Nodes");
    let mut points: Vec<(usize, f32)> = nodes
        .and_then(|x| x.child("Points"))
        .map(|p| {
            p.children("Point")
                .filter_map(|p| {
                    let t = p.all_text();
                    let (a, b) = t.split_once(',')?;
                    Some((a.trim().parse::<f64>().ok()?.max(0.0) as usize, b.trim().parse().ok()?))
                })
                .collect()
        })
        .unwrap_or_default();
    points.sort_by_key(|p| p.0);
    let mode = loop_mode(&text(n, "LoopMode"));
    let mut lfos = vec![lfo(n.child("Lfo1")), lfo(n.child("Lfo2"))];
    if let Some(l) = n.child("Lfo") {
        lfos = vec![lfo(Some(l))];
    }
    let auto_amp = n
        .child("AutoAmp")
        .filter(|a| flag(a, "IsActive"))
        .map(|a| (int(a, "Attack", 100), int(a, "Release", 70), int(a, "Amount", 60)));
    let mut play = nodes.map(|x| text(x, "PlayMode")).unwrap_or_default();
    if play.is_empty() {
        play = text(n, "InterpolationMode");
    }
    Envelope {
        on: flag(n, "IsActive"),
        interp: Some(interp(&play)),
        sustain: flag(n, "SustainIsActive").then(|| int(n, "SustainPos", 0).max(0) as usize),
        loop_mode: (mode != Loop::Off).then_some(mode),
        loop_range: (int(n, "LoopStart", 0).max(0) as usize, int(n, "LoopEnd", 0).max(0) as usize),
        decay: int(n, "Decay", 128),
        length: nodes.map_or(72, |x| int(x, "Length", 72).max(1) as usize),
        points,
        bipolar: nodes.is_some_and(|x| text(x, "Polarity") == "Bipolar"),
        lfos,
        auto_amp,
    }
}

fn envelopes(n: &Node) -> Envelopes {
    Envelopes {
        volume: envelope(n.child("Volume")),
        pan: envelope(n.child("Pan")),
        pitch: envelope(n.child("Pitch")),
        cutoff: envelope(n.child("Cutoff")),
        resonance: envelope(n.child("Resonance")),
        filter_type: int(n, "FilterType", 0),
    }
}

/// What an instrument's patch leaves out: its name, the editor's state.
const PATCH_SKIP: &[&str] = &[
    "Name",
    "FileName",
    "FileModificationDate",
    "FileModificationDayTime",
    "SelectedPresetName",
    "SelectedPresetLibrary",
    "SelectedPresetIsModified",
    "IsSelected",
    "IsMaximized",
    "DisplayStart",
    "DisplayLength",
    "SelectionRangeStart",
    "SelectionRangeEnd",
    "SelectedChannel",
    "VZoomFactor",
    "SelectedSampleIndex",
    "SelectedModulationSetIndex",
    "SelectedDeviceChainIndex",
    "CopyIntoNewSampleNameCounter",
    "CopyIntoNewInstrumentNameCounter",
    "PluginEditorWindowPosition",
    "Visualization",
];

/// The instrument's samples' audio, by their place: (instrument, sample) → audio.
pub type Waves<'a> = &'a dyn Fn(usize, usize) -> Option<Arc<Audio>>;

fn sample(s: &Node, inst_nna: Nna, audio: Option<Arc<Audio>>) -> Sample {
    let nna = match text(s, "NewNoteAction").as_str() {
        "" => inst_nna,
        "NoteOff" => Nna::Off,
        "None" | "Continue" => Nna::Continue,
        _ => Nna::Cut,
    };
    let interp = match text(s, "InterpolationMode").as_str() {
        "None" | "Off" => SampleInterp::None,
        "Linear" => SampleInterp::Linear,
        "Sinc" => SampleInterp::Sinc,
        _ => SampleInterp::Cubic,
    };
    let slices = s
        .child("SliceMarkers")
        .map_or_else(Vec::new, |m| m.children.iter().filter_map(|c| text(c, "SamplePosition").parse().ok()).collect());
    let mapping = s.child("Mapping");
    Sample {
        name: text(s, "Name"),
        audio: audio.unwrap_or_default(),
        volume: num(s, "Volume", 1.0),
        pan: num(s, "Panning", 0.5),
        base_note: mapping.map_or_else(|| int(s, "BaseNote", 48), |m| int(m, "BaseNote", 48)),
        transpose: int(s, "Transpose", 0),
        finetune: int(s, "Finetune", 0),
        nna,
        interp,
        loop_mode: loop_mode(&text(s, "LoopMode")),
        loop_start: int(s, "LoopStart", 0).max(0) as usize,
        loop_end: int(s, "LoopEnd", 0).max(0) as usize,
        loop_release: flag(s, "LoopRelease"),
        beat_sync: flag(s, "BeatSyncIsActive").then(|| int(s, "BeatSyncLines", 16).max(1) as u32),
        autofade: flag(s, "AutoFade"),
        oneshot: flag(s, "OneShotTrigger"),
        mute_group: int(s, "MuteGroupIndex", -1),
        mod_set: int(s, "ModulationSetIndex", 0),
        chain: int(s, "DeviceChainIndex", 0),
        alias: flag(s, "IsAlias"),
        slices,
    }
}

fn zone_of(m: &Node, sample: usize) -> Zone {
    let b = |name: &str, f: i32| int(m, name, f).clamp(0, 127) as u8;
    let mvv = text(m, "MapVelocityToVolume");
    let k2p = text(m, "MapKeyToPitch");
    Zone {
        sample,
        base_note: int(m, "BaseNote", 48),
        notes: (b("NoteStart", 0), b("NoteEnd", 119)),
        vels: (b("VelocityStart", 0), b("VelocityEnd", 127)),
        vel_to_volume: mvv.is_empty() || truth(&mvv),
        key_to_pitch: k2p.is_empty() || truth(&k2p),
        off_layer: text(m, "Layer").contains("Off"),
    }
}

fn instrument(n: &Node, index: usize, doc: u32, waves: Waves) -> Instrument {
    let inst_nna = match text(n, "NewNoteAction").as_str() {
        "NoteOff" => Nna::Off,
        "None" | "Continue" => Nna::Continue,
        _ => Nna::Cut,
    };
    let mut ins = Instrument {
        name: text(n, "Name"),
        volume: 1.0,
        macros: [0.5; 8],
        ..Default::default()
    };
    n.write(PATCH_SKIP, &mut ins.xml);
    let gen = n.child("SampleGenerator");
    let samples_node = gen.and_then(|g| g.child("Samples")).or_else(|| n.child("Samples"));
    if let Some(ss) = samples_node {
        for (i, s) in ss.children("Sample").enumerate() {
            ins.samples.push(sample(s, inst_nna, waves(index, i)));
        }
    }
    // Slice aliases play the first sample's audio from their marker.
    if let Some(first) = ins.samples.first().cloned() {
        for (k, s) in ins.samples.iter_mut().enumerate().skip(1) {
            if s.alias && s.audio.frames() == 0 {
                s.audio = first.audio.clone();
                if let Some(&at) = first.slices.get(k - 1) {
                    s.slices = vec![at, first.slices.get(k).copied().unwrap_or(first.audio.frames())];
                }
            }
        }
    }
    if let Some(sm) = n.child("SplitMap") {
        // Renoise 1 and 2: a sample for each of 120 notes, at the sample's own base note.
        let splits: Vec<usize> = sm.children("Split").map(|s| s.all_text().trim().parse().unwrap_or(0)).collect();
        let mut at = 0;
        while at < splits.len() {
            let s = splits[at];
            let mut end = at;
            while end + 1 < splits.len() && splits[end + 1] == s {
                end += 1;
            }
            if s < ins.samples.len() {
                ins.zones.push(Zone {
                    sample: s,
                    base_note: ins.samples[s].base_note,
                    notes: (at as u8, end as u8),
                    vels: (0, 127),
                    vel_to_volume: true,
                    key_to_pitch: true,
                    off_layer: false,
                });
            }
            at = end + 1;
        }
    } else if let Some(sm) = n.child("SampleSplitMap") {
        for m in sm.child("NoteOnMappings").map(|x| x.children("NoteOnMapping").collect::<Vec<_>>()).unwrap_or_default() {
            let s = int(m, "SampleIndex", 0).max(0) as usize;
            ins.zones.push(zone_of(m, s));
        }
        for m in sm.child("NoteOffMappings").map(|x| x.children("NoteOffMapping").collect::<Vec<_>>()).unwrap_or_default() {
            let s = int(m, "SampleIndex", 0).max(0) as usize;
            let mut z = zone_of(m, s);
            z.off_layer = true;
            ins.zones.push(z);
        }
    } else if let Some(ss) = samples_node {
        for (i, s) in ss.children("Sample").enumerate() {
            if let Some(m) = s.child("Mapping") {
                ins.zones.push(zone_of(m, i));
            }
        }
    }
    if let Some(e) = n.child("Envelopes").or_else(|| n.child("SampleEnvelopes")) {
        ins.envelopes = Some(Arc::new(envelopes(e)));
    }
    if let Some(g) = gen {
        if let Some(ms) = g.child("ModulationSets") {
            for set in ms.children("ModulationSet") {
                let devs = devices(set.child("Devices"));
                let range = devs
                    .iter()
                    .find(|d| d.kind == "SampleMixerModulationDevice")
                    .and_then(|d| d.text("PitchModulationRange"))
                    .and_then(|t| t.parse().ok())
                    .unwrap_or(12.0);
                ins.mod_sets.push(ModSet {
                    devices: devs,
                    filter_type: int(set, "FilterType", 0),
                    filter_version: int(set, "FilterBankVersion", 1),
                    pitch_range: range,
                });
            }
        }
        if let Some(ch) = g.child("DeviceChains") {
            for c in ch.children("DeviceChain") {
                ins.chains.push(devices(c.child("Devices")));
            }
        }
    }
    if let Some(gp) = n.child("GlobalProperties") {
        ins.volume = num(gp, "Volume", 1.0);
        ins.transpose = int(gp, "Transpose", 0);
        for (k, m) in ins.macros.iter_mut().enumerate() {
            if let Some(x) = gp.child(&format!("Macro{k}")) {
                *m = num(x, "Value", 50.0) / 100.0;
            }
        }
    }
    let plugin = n
        .child("PluginGenerator")
        .or_else(|| n.child("PluginProperties"))
        .and_then(|p| p.child("PluginDevice"))
        .map(|d| text(d, "PluginIdentifier"))
        .filter(|s| !s.is_empty());
    let midi = n.child("MidiProperties").filter(|m| flag(m, "IsActive")).map(|_| "MIDI".to_string());
    ins.plugin = plugin.or(midi).filter(|_| ins.samples.iter().all(|s| s.audio.frames() == 0));
    if let Some(pg) = n.child("PhraseGenerator") {
        let map = pg.child("PhraseMap");
        let maps: Vec<&Node> = map.and_then(|m| m.child("Mappings")).map(|m| m.children.iter().collect()).unwrap_or_default();
        ins.phrase_mode = match text(pg, "PlaybackMode").as_str() {
            "Off" => 0,
            "Selective" if maps.is_empty() => 2,
            _ if map.is_some_and(|m| flag(m, "MappingsEnabled")) => 1,
            "Keymap" => 1,
            _ => 0,
        };
        ins.phrase_selected = usize::try_from(int(pg, "SelectedPhraseIndex", -1)).ok();
        if let Some(ps) = pg.child("Phrases") {
            for (k, p) in ps.children("Phrase").enumerate() {
                let m = maps.iter().copied().find(|m| int(m, "PhraseIndex", -1) == k as i32);
                ins.phrases.push(Phrase {
                    lines: p.child("Lines").map(|l| read_lines(l, doc)).unwrap_or_default(),
                    length: int(p, "NumberOfLines", 16).clamp(1, 512) as usize,
                    lpb: int(p, "LinesPerBeat", 4).clamp(1, 256) as u32,
                    looping: m.map_or(flag(p, "Looping"), |m| flag(m, "Loop")),
                    loop_range: (int(p, "LoopStart", 0).max(0) as usize, int(p, "LoopEnd", 0).max(0) as usize),
                    note_columns: int(p, "VisibleNoteColumns", 1).max(1) as usize,
                    keys: m.map_or((0, 119), |m| (int(m, "NoteRangeStart", 0).clamp(0, 119) as u8, int(m, "NoteRangeEnd", 119).clamp(0, 119) as u8)),
                    base_note: m.map_or(48, |m| int(m, "BaseNote", 48)),
                });
            }
        }
    }
    ins
}

fn track_kind(name: &str) -> TrackKind {
    match name {
        "SequencerGroupTrack" => TrackKind::Group,
        "SequencerSendTrack" => TrackKind::Send,
        "SequencerMasterTrack" => TrackKind::Master,
        _ => TrackKind::Normal,
    }
}

fn track(n: &Node) -> Track {
    let kind = track_kind(n.attrs.iter().find(|(k, _)| k == "type").map_or(n.name.as_str(), |(_, v)| v.as_str()));
    let state = text(n, "State");
    let column_off = n.child("NoteColumnStates").map_or_else(Vec::new, |s| s.children.iter().map(|c| c.all_text().trim() == "Off").collect());
    Track {
        kind,
        name: text(n, "Name"),
        on: state.is_empty() || state == "Active",
        note_columns: int(n, "NumberOfVisibleNoteColumns", 1).max(0) as usize,
        fx_columns: int(n, "NumberOfVisibleEffectColumns", 1).max(0) as usize,
        column_off,
        routing: int(n, "TrackRouting", 0),
        group: None,
        delay_ms: num(n, "TrackDelay", 0.0),
        devices: devices(n.child("FilterDevices").and_then(|f| f.child("Devices"))),
    }
}

/// Each track's group: members come before their group, one level deeper.
fn link_groups(tracks: &mut [Track], levels: &[i32]) {
    let mut pending: Vec<usize> = Vec::new();
    for t in 0..tracks.len() {
        if tracks[t].kind == TrackKind::Group {
            let level = levels[t];
            while let Some(&m) = pending.last() {
                if levels[m] <= level {
                    break;
                }
                pending.pop();
                if levels[m] == level + 1 {
                    tracks[m].group = Some(t);
                }
            }
        }
        if matches!(tracks[t].kind, TrackKind::Normal | TrackKind::Group) {
            pending.push(t);
        }
    }
}

impl Song {
    /// The song in a Song.xml; `waves` gives each sample's audio.
    pub fn read(xml: &[u8], waves: Waves) -> Option<Song> {
        let root = crate::xml::parse_xml(xml)?;
        if root.name != "RenoiseSong" {
            return None;
        }
        let doc: u32 = root.attrs.iter().find(|(k, _)| k == "doc_version").and_then(|(_, v)| v.parse().ok()).unwrap_or(0);
        let g = root.child("GlobalSongData")?;
        let mut song = Song {
            doc,
            name: text(g, "SongName"),
            artist: text(g, "Artist"),
            comments: g.child("SongComments").map_or_else(Vec::new, |c| c.children.iter().map(|x| x.all_text()).collect()),
            bpm: num(g, "BeatsPerMin", 125.0).clamp(1.0, 1000.0),
            lpb: int(g, "LinesPerBeat", 4).clamp(1, 256) as u32,
            tpl: int(g, "TicksPerLine", 12).clamp(1, 32) as u32,
            speed_timing: g.child("LinesPerBeat").is_none(),
            shuffle: None,
            headroom: num(g, "GlobalTrackHeadroom", 1.0),
            amiga_offsets: flag(g, "SampleOffsetCompatibilityMode") || flag(g, "AmigaFt2CompatibilityMode"),
            amiga_pitch: flag(g, "PitchEffectsCompatibilityMode") || flag(g, "AmigaFt2CompatibilityMode"),
            ..Default::default()
        };
        if flag(g, "ShuffleIsActive") {
            let a: Vec<f32> = g
                .child("ShuffleAmounts")
                .map_or_else(Vec::new, |s| s.children.iter().map(|c| c.all_text().trim().parse().unwrap_or(0.0)).collect());
            if a.iter().any(|&x| x != 0.0) {
                let at = |k: usize| a.get(k).copied().unwrap_or(0.0) / 100.0;
                song.shuffle = Some([at(0), at(1), at(2), at(3)]);
            }
        }
        if let Some(ts) = root.child("Tracks") {
            let mut levels = Vec::new();
            for t in &ts.children {
                levels.push(int(t, "GroupNestingLevel", 0));
                song.tracks.push(track(t));
            }
            link_groups(&mut song.tracks, &levels);
        }
        if let Some(is) = root.child("Instruments") {
            for (k, i) in is.children.iter().enumerate() {
                song.instruments.push(instrument(i, k, doc, waves));
            }
        }
        if let Some(pp) = root.child("PatternPool").and_then(|p| p.child("Patterns")) {
            for p in pp.children("Pattern") {
                let lines = int(p, "NumberOfLines", 64).clamp(1, 1024) as usize;
                let mut pat = Pattern {
                    name: text(p, "Name"),
                    lines,
                    tracks: Vec::new(),
                };
                if let Some(ts) = p.child("Tracks") {
                    for t in &ts.children {
                        let mut pt = PatternTrack {
                            lines: t.child("Lines").map(|l| read_lines(l, doc)).unwrap_or_default(),
                            alias: Some(int(t, "AliasPatternIndex", -1)).filter(|&a| a >= 0).map(|a| a as usize),
                            ..Default::default()
                        };
                        pt.lines.retain(|(at, _)| *at < lines);
                        if let Some(es) = t.child("Automations").and_then(|a| a.child("Envelopes")) {
                            pt.automation = es.children("Envelope").filter_map(|e| read_automation(e, lines)).collect();
                        }
                        pat.tracks.push(pt);
                    }
                }
                song.patterns.push(pat);
            }
        }
        if let Some(ps) = root.child("PatternSequence") {
            if let Some(es) = ps.child("SequenceEntries") {
                for e in es.children("SequenceEntry") {
                    let muted = e
                        .child("MutedTracks")
                        .map_or_else(Vec::new, |m| m.children.iter().filter_map(|x| x.all_text().trim().parse().ok()).collect());
                    song.sequence.push(Entry {
                        pattern: int(e, "Pattern", 0).max(0) as usize,
                        muted,
                    });
                }
            } else if let Some(list) = ps.child("PatternSequence") {
                for p in list.children("Pattern") {
                    song.sequence.push(Entry {
                        pattern: p.all_text().trim().parse().unwrap_or(0),
                        muted: Vec::new(),
                    });
                }
            }
        }
        song.sequence.retain(|e| e.pattern < song.patterns.len());
        if song.sequence.is_empty() && !song.patterns.is_empty() {
            song.sequence.push(Entry { pattern: 0, muted: Vec::new() });
        }
        Some(song)
    }

    /// A pattern track's own lines, through its alias.
    pub fn pattern_track(&self, pattern: usize, track: usize) -> Option<&PatternTrack> {
        let mut p = pattern;
        for _ in 0..16 {
            let t = self.patterns.get(p)?.tracks.get(track)?;
            match t.alias {
                Some(a) if a != p => p = a,
                _ => return Some(t),
            }
        }
        None
    }
}
