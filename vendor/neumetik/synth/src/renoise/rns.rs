// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! Renoise 1's own songs (.rns, Renoise 0.5 to 1.28, 2002-2005), read
//! into the same model as the Song.xml of later versions, so they play
//! on the same replayer.
//!
//! No description of the format was ever published; this was worked out
//! from the songs themselves, and checked against the four of them their
//! authors saved again from Renoise 1.5 as .xrns. A song is "RNS" and its
//! version (05 to 018), then chunks, each ">>> Chunk Start <<<", a name
//! and version ("PatternEditor V02"), its data and ">>>  Chunk End  <<<":
//! Header (name, artist, comments), PatternEditor (tracks and patterns),
//! Sequencer, Player (tempo and ticks a line), TrackDsp (each track's
//! devices: binary up to V02, named parameters in V03), TrackEnvelopes
//! (automation), Instruments, Envelopes (the instruments' envelopes) and
//! SampleData (every sample's 16-bit points, delta-coded, one channel
//! after the other). Numbers are little-endian; strings are a length and
//! that many bytes, the last a NUL.

use std::sync::Arc;

use super::song::{
    column, Audio, Automation, Col, Device, Entry, Envelope, Envelopes, FxCell, Instrument, Interp, Lfo, Line, Loop, Nna, NoteCell, Pattern, PatternTrack, Sample, SampleInterp, Song, Track,
    TrackKind, Zone, NONE, OFF,
};

const START: &[u8] = b">>> Chunk Start <<<\0";
const END: &[u8] = b">>>  Chunk End  <<<\0";

/// Whether a file is a Renoise 1 song.
pub fn is_rns(d: &[u8]) -> bool {
    d.starts_with(b"RNS0") && d.get(4).is_some_and(u8::is_ascii_digit)
}

/// A cursor over a chunk's bytes.
struct R<'a> {
    d: &'a [u8],
    p: usize,
}

impl<'a> R<'a> {
    fn new(d: &'a [u8]) -> R<'a> {
        R { d, p: 0 }
    }
    fn left(&self) -> usize {
        self.d.len().saturating_sub(self.p)
    }
    fn bytes(&mut self, n: usize) -> Option<&'a [u8]> {
        let b = self.d.get(self.p..self.p.checked_add(n)?)?;
        self.p += n;
        Some(b)
    }
    fn skip(&mut self, n: usize) -> Option<()> {
        self.bytes(n).map(|_| ())
    }
    fn u8(&mut self) -> Option<u8> {
        self.bytes(1).map(|b| b[0])
    }
    fn u16(&mut self) -> Option<u16> {
        self.bytes(2).map(|b| u16::from_le_bytes([b[0], b[1]]))
    }
    fn u32(&mut self) -> Option<u32> {
        self.bytes(4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn f32(&mut self) -> Option<f32> {
        self.u32().map(f32::from_bits).map(|x| if x.is_finite() { x } else { 0.0 })
    }
    fn str(&mut self) -> Option<String> {
        let n = self.u32()? as usize;
        if n > 1 << 16 {
            return None;
        }
        self.bytes(n).map(text)
    }
    fn peek(&self, s: &[u8]) -> bool {
        self.d.get(self.p..).is_some_and(|d| d.starts_with(s))
    }
}

/// Text up to its NUL, each byte a Latin-1 character.
fn text(b: &[u8]) -> String {
    b.iter().take_while(|&&c| c != 0).map(|&c| c as char).collect::<String>().trim_end().to_string()
}

fn find(d: &[u8], what: &[u8], from: usize) -> Option<usize> {
    d.get(from..)?.windows(what.len()).position(|w| w == what).map(|i| i + from)
}

/// The song's chunks: name, version, data.
fn chunks(d: &[u8]) -> Vec<(String, u32, &[u8])> {
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(i) = find(d, START, at) {
        let Some(head) = d.get(i + START.len()..i + START.len() + 20) else { break };
        let head = text(head);
        let (name, ver) = match head.rsplit_once(" V") {
            Some((n, v)) => (n.trim().to_string(), v.trim().parse().unwrap_or(0)),
            None => (head.clone(), 0),
        };
        let start = i + START.len() + 20;
        // Sample data runs to the file's end (its points could hold the end mark).
        let end = if name == "SampleData" { d.len() } else { find(d, END, start).unwrap_or(d.len()) };
        out.push((name, ver, &d[start..end]));
        at = end;
    }
    out
}

/// Strings a chunk holds from `p` on: (where, text), for the comments.
fn strings_in(d: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut r = R::new(d);
    while r.left() >= 5 {
        let at = r.p;
        let n = R { d, p: at }.u32().unwrap_or(0) as usize;
        if (1..512).contains(&n) && at + 4 + n <= d.len() && d[at + 3 + n] == 0 && d[at + 4..at + 3 + n].iter().all(|&c| c >= 32 || c == 9) {
            r.p = at + 4;
            if let Some(s) = r.bytes(n) {
                out.push(text(s));
            }
        } else {
            r.p = at + 1;
        }
    }
    out
}

struct Tracks {
    names: Vec<String>,
    /// Note and effect columns of each sequencer track, then each master or send track's effect columns.
    notes: Vec<usize>,
    fx: Vec<usize>,
    extra: usize,
}

/// A pattern as read: its lines, each track's lines.
struct Pat {
    lines: usize,
    name: String,
    tracks: Vec<Vec<(usize, Line)>>,
}

/// A note column's cell, as the file keeps it.
fn cell(note: u8, instr: u8, vol: u8, pan: u8) -> Option<NoteCell> {
    if note >= 121 && instr == 255 && vol == 255 && pan == 255 {
        return None;
    }
    let vol = match vol {
        255 => Col::None,
        // Renoise 1's volumes run to 40; Renoise 1.5 doubled them to 80.
        v @ 0..=0x40 => Col::Set(v * 2),
        0x41..=0x7f => Col::Set(0x80),
        v => column(&format!("{v:02X}"), 4, false),
    };
    let pan = match pan {
        255 => Col::None,
        v => column(&format!("{v:02X}"), 4, true),
    };
    Some(NoteCell {
        note: match note {
            0..=119 => note,
            120 => OFF,
            _ => NONE,
        },
        instr: if instr == 255 { NONE } else { instr },
        vol,
        pan,
        delay: 0,
        fx: None,
    })
}

fn hex2(v: u8) -> [u8; 2] {
    let h = b"0123456789ABCDEF";
    [h[usize::from(v >> 4)], h[usize::from(v & 15)]]
}

/// Puts a track's columns, read one after the other, into its lines.
struct Grid {
    lines: Vec<Line>,
}

impl Grid {
    fn new(n: usize, notes: usize, fx: usize) -> Grid {
        let empty = NoteCell {
            note: NONE,
            instr: NONE,
            ..Default::default()
        };
        Grid {
            lines: vec![
                Line {
                    notes: vec![empty; notes],
                    fx: vec![FxCell { num: *b"00", val: 0 }; fx]
                };
                n
            ],
        }
    }
    fn into_lines(self) -> Vec<(usize, Line)> {
        self.lines
            .into_iter()
            .enumerate()
            .filter(|(_, l)| l.notes.iter().any(|c| c.note != NONE || c.instr != NONE || c.vol != Col::None || c.pan != Col::None) || l.fx.iter().any(|f| f.num != *b"00" || f.val != 0))
            .collect()
    }
}

/// A track's data in a pattern: its note columns (four bytes a line, or
/// in Renoise 0.5 each field's lines in turn), then its effect columns
/// (the values' lines, then the numbers').
fn track_data(r: &mut R, lines: usize, notes: usize, fx: usize, by_field: bool) -> Option<Vec<(usize, Line)>> {
    let mut g = Grid::new(lines, notes, fx);
    for c in 0..notes {
        if by_field {
            let (n, i, v, p) = (r.bytes(lines)?, r.bytes(lines)?, r.bytes(lines)?, r.bytes(lines)?);
            for l in 0..lines {
                if let Some(x) = cell(n[l], i[l], v[l], p[l]) {
                    g.lines[l].notes[c] = x;
                }
            }
        } else {
            for l in 0..lines {
                let b = r.bytes(4)?;
                if let Some(x) = cell(b[0], b[1], b[2], b[3]) {
                    g.lines[l].notes[c] = x;
                }
            }
        }
    }
    for c in 0..fx {
        let (vals, nums) = (r.bytes(lines)?, r.bytes(lines)?);
        for l in 0..lines {
            g.lines[l].fx[c] = FxCell { num: hex2(nums[l]), val: vals[l] };
        }
    }
    Some(g.into_lines())
}

fn pattern_editor(d: &[u8], ver: u32) -> Option<(Tracks, Vec<Pat>)> {
    let mut r = R::new(d);
    let mut pats = Vec::new();
    match ver {
        0 => {
            // Renoise 0.5: no master or send tracks.
            let nt = usize::from(r.u8()?);
            let np = usize::from(r.u8()?);
            let names: Vec<String> = (0..nt).map(|_| r.bytes(50).map(text)).collect::<Option<_>>()?;
            let lens: Vec<usize> = (0..np).map(|_| r.u16().map(usize::from)).collect::<Option<_>>()?;
            let notes: Vec<usize> = r.bytes(nt)?.iter().map(|&x| usize::from(x)).collect();
            let fx: Vec<usize> = r.bytes(nt)?.iter().map(|&x| usize::from(x)).collect();
            for &lines in &lens {
                let mut tracks = Vec::new();
                for t in 0..nt {
                    tracks.push(track_data(&mut r, lines, notes[t], fx[t], true)?);
                }
                pats.push(Pat { lines, name: String::new(), tracks });
            }
            Some((Tracks { names, notes, fx, extra: 0 }, pats))
        }
        1 => {
            let nt = r.u32()? as usize;
            let extra = r.u32()? as usize;
            let np = r.u32()? as usize;
            if nt > 256 || extra > 64 || np > 4096 {
                return None;
            }
            let names: Vec<String> = (0..nt).map(|_| r.bytes(50).map(text)).collect::<Option<_>>()?;
            let lens: Vec<usize> = (0..np).map(|_| r.u16().map(usize::from)).collect::<Option<_>>()?;
            let notes: Vec<usize> = r.bytes(nt)?.iter().map(|&x| usize::from(x)).collect();
            let fx: Vec<usize> = r.bytes(nt + extra)?.iter().map(|&x| usize::from(x)).collect();
            r.skip(2 * nt)?;
            for &lines in &lens {
                let mut tracks = Vec::new();
                for (t, &f) in fx.iter().enumerate() {
                    tracks.push(track_data(&mut r, lines, notes.get(t).copied().unwrap_or(0), f, false)?);
                }
                pats.push(Pat { lines, name: String::new(), tracks });
            }
            Some((Tracks { names, notes, fx, extra }, pats))
        }
        _ => {
            let pool = r.peek(b"PatternPool01\0");
            if pool {
                r.skip(14)?;
            }
            let nt = r.u32()? as usize;
            let extra = r.u32()? as usize;
            r.skip(4)?;
            if nt > 256 || extra > 64 {
                return None;
            }
            let (mut names, mut notes, mut fx) = (Vec::new(), Vec::new(), Vec::new());
            for _ in 0..nt {
                names.push(r.str()?);
                let b = r.bytes(4)?;
                notes.push(usize::from(b[0]));
                fx.push(usize::from(b[1]));
            }
            for _ in 0..extra {
                fx.push(usize::from(r.u8()?));
            }
            let np = r.u32()? as usize;
            if np > 4096 {
                return None;
            }
            for _ in 0..np {
                let name = if pool { r.str()? } else { String::new() };
                let lines = r.u32()? as usize;
                if lines > 4096 {
                    return None;
                }
                let mut tracks = Vec::new();
                for (t, &f) in fx.iter().enumerate() {
                    // 0: the track has lines here, 1: it's empty.
                    tracks.push(if r.u8()? == 0 {
                        track_data(&mut r, lines, notes.get(t).copied().unwrap_or(0), f, false)?
                    } else {
                        Vec::new()
                    });
                }
                pats.push(Pat { lines, name, tracks });
            }
            Some((Tracks { names, notes, fx, extra }, pats))
        }
    }
}

fn sequence(d: &[u8]) -> Vec<usize> {
    let mut r = R::new(d);
    if r.peek(b"PatternSequence01\0") {
        r.p = 18;
        let n = r.u32().unwrap_or(0) as usize;
        r.p += 12;
        return (0..n.min(4096)).map_while(|_| r.u32().map(|x| x as usize)).collect();
    }
    let n = usize::from(r.u8().unwrap_or(0));
    r.p = 2;
    r.bytes(n).map_or_else(Vec::new, |b| b.iter().map(|&x| usize::from(x)).collect())
}

/// A sample as its instrument keeps it.
struct RawSample {
    name: String,
    volume: f32,
    pan: f32,
    frames: usize,
    channels: usize,
    finetune: i32,
    base: i32,
    sync: Option<u32>,
    looping: u8,
    loop_start: usize,
    loop_end: usize,
}

struct RawInstrument {
    name: String,
    split: Vec<u8>,
    nna: u32,
    slots: Vec<usize>,
    samples: Vec<RawSample>,
    plugin: bool,
}

/// A sample's settings after its name: volume, (panning,) frames, channels,
/// finetune, base note, beat sync, loop.
fn sample_body(r: &mut R, name: String, pan: bool) -> Option<RawSample> {
    let volume = r.f32()?;
    let pan = if pan { r.f32()? } else { 0.5 };
    let frames = r.u32()? as usize;
    let channels = usize::from(r.u8()?).clamp(1, 2);
    let finetune = i32::from(r.u8()? as i8);
    let base = i32::from(r.u8()?);
    let sync = r.u8()? != 0;
    let lines = u32::from(r.u16()?);
    let looping = r.u8()?;
    let loop_start = r.u32()? as usize;
    let loop_end = r.u32()? as usize;
    r.skip(4)?;
    Some(RawSample {
        name,
        volume,
        pan,
        frames,
        channels,
        finetune,
        base,
        sync: sync.then_some(lines.max(1)),
        looping,
        loop_start,
        loop_end,
    })
}

fn instruments(d: &[u8], ver: u32) -> Option<Vec<RawInstrument>> {
    let mut r = R::new(d);
    let mut out = Vec::new();
    if ver >= 4 {
        r.skip(12)?;
    }
    for _ in 0..128 {
        if r.left() < 140 {
            break;
        }
        let tagged = r.peek(b"Instrument01\0");
        if tagged {
            r.skip(13)?;
        }
        let name = if ver >= 4 { r.str()? } else { text(r.bytes(20)?) };
        let split = r.bytes(120)?.to_vec();
        let nna = r.u32()?;
        let flags = r.bytes(16)?;
        let slots: Vec<usize> = (0..16).filter(|&k| flags[k] != 0).collect();
        let mut samples = Vec::new();
        for _ in &slots {
            if ver >= 4 {
                let sname = r.str()?;
                r.skip(5)?;
                samples.push(sample_body(&mut r, sname, tagged)?);
            } else {
                let sname = text(r.bytes(20)?);
                r.skip(256)?;
                samples.push(sample_body(&mut r, sname, false)?);
            }
        }
        r.skip(2)?;
        let mut plugin = false;
        while r.peek(b"Vsti0") {
            r.skip(7)?;
            let n = r.u32()? as usize;
            r.skip(n)?;
            plugin = true;
        }
        out.push(RawInstrument {
            name,
            split,
            nna,
            slots,
            samples,
            plugin,
        });
    }
    Some(out)
}

/// The points of every sample, in order: each channel's 16-bit deltas in turn.
fn audio(d: &[u8], ins: &[RawInstrument]) -> Vec<Vec<Arc<Audio>>> {
    let mut at = 0usize;
    let mut out = Vec::new();
    for i in ins {
        let mut v = Vec::new();
        for s in &i.samples {
            let mut channels = Vec::new();
            for _ in 0..s.channels {
                let bytes = d.get(at..(at + s.frames * 2).min(d.len())).unwrap_or(&[]);
                let mut acc = 0i16;
                let ch: Vec<f32> = bytes
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|b| {
                        acc = acc.wrapping_add(i16::from_le_bytes([b[0], b[1]]));
                        f32::from(acc) / 32768.0
                    })
                    .collect();
                at += s.frames * 2;
                channels.push(ch);
            }
            // A file cut short: its channels as long as the shortest.
            let n = channels.iter().map(Vec::len).min().unwrap_or(0);
            for c in channels.iter_mut() {
                c.truncate(n);
            }
            v.push(Arc::new(Audio { rate: 44100, channels }));
        }
        out.push(v);
    }
    out
}

fn loop_of(v: u32) -> Option<Loop> {
    match v {
        1 => Some(Loop::Forward),
        2 => Some(Loop::Backward),
        3 => Some(Loop::PingPong),
        _ => None,
    }
}

fn lfo(mode: u32, freq: u32, amp: u32, phase: u32) -> Lfo {
    Lfo {
        mode: mode.min(4) as u8,
        freq: freq as i32,
        amp: amp as i32,
        phase: phase as i32,
    }
}

/// An envelope: its curve (a value 0-100 for each tick, FF none; or, from
/// Renoise 1.28, its points), loop, sustain, fadeout and LFOs.
fn envelope(r: &mut R, k: usize, filter_type: &mut i32) -> Option<Envelope> {
    let mut e = Envelope::default();
    if r.peek(b"Instr.Envelope01\0") {
        r.skip(17)?;
        e.on = r.u8()? != 0;
        e.interp = Some(if r.u32()? == 0 { Interp::Linear } else { Interp::Curve });
        e.length = (r.u32()? as usize).max(1);
        r.skip(8)?;
        let n = r.u32()? as usize;
        if n > 4096 {
            return None;
        }
        for _ in 0..n {
            let at = r.u32()? as usize;
            e.points.push((at, r.f32()?));
        }
    } else {
        e.on = r.u8()? != 0;
        e.interp = Some(if r.u32()? == 0 { Interp::Linear } else { Interp::Curve });
        let n = r.u32()? as usize;
        if n > 4096 {
            return None;
        }
        e.length = n.max(1);
        e.points = r.bytes(n)?.iter().enumerate().filter(|(_, &v)| v != 255).map(|(i, &v)| (i, f32::from(v) / 100.0)).collect();
    }
    let start = r.u32()? as usize;
    let end = r.u32()? as usize;
    e.loop_mode = loop_of(r.u32()?);
    e.loop_range = (start, end);
    let sustain = r.u8()? != 0;
    let at = r.u32()? as usize;
    e.sustain = sustain.then_some(at);
    e.decay = r.u32()? as i32;
    if k < 3 {
        let v: Vec<u32> = (0..8).map(|_| r.u32()).collect::<Option<_>>()?;
        e.lfos = vec![lfo(v[0], v[2], v[4], v[6]), lfo(v[1], v[3], v[5], v[7])];
        if k == 2 {
            *filter_type = r.u32()? as i32;
        }
    } else {
        let amp_on = r.u8()? != 0;
        let v: Vec<u32> = (0..7).map(|_| r.u32()).collect::<Option<_>>()?;
        e.auto_amp = amp_on.then_some((v[0] as i32, v[1] as i32, v[2] as i32));
        e.lfos = vec![lfo(v[3], v[4], v[5], v[6])];
    }
    Some(e)
}

/// Each instrument's envelopes: a flag for each of 128 instruments, then
/// for each one flagged its volume, panning, pitch, cutoff and resonance.
fn envelopes(d: &[u8]) -> Vec<Option<Envelopes>> {
    let mut out = vec![None; 128];
    let mut r = R::new(d);
    let Some(flags) = (0..128).map(|_| r.u32()).collect::<Option<Vec<u32>>>() else { return out };
    for (k, &f) in flags.iter().enumerate() {
        if f == 0 {
            continue;
        }
        let mut ft = 0;
        let mut es = Vec::new();
        for j in 0..5 {
            match envelope(&mut r, j, &mut ft) {
                Some(e) => es.push(e),
                None => return out,
            }
        }
        let mut it = es.into_iter();
        out[k] = Some(Envelopes {
            volume: it.next().unwrap_or_default(),
            pan: it.next().unwrap_or_default(),
            pitch: it.next().unwrap_or_default(),
            cutoff: it.next().unwrap_or_default(),
            resonance: it.next().unwrap_or_default(),
            filter_type: ft,
        });
    }
    out
}

/// A Renoise 1 device as the replayer knows it (the .xrns type), and its
/// parameters' names there, in the order the file and automation count
/// them, from the names Renoise 1.28 saves them under.
fn device_kind(kind: &str) -> Option<(&'static str, &'static [(&'static str, &'static str)])> {
    Some(match kind {
        "TrackVolPan" | "MasterProcessor" => ("SequencerTrackDevice", &[("Panning", "Panning"), ("Volume", "Volume"), ("Width", "Surround")]),
        "Delay" => (
            "DelayDevice",
            &[
                ("Send", "TrackSend"),
                ("L Delay", "LDelay"),
                ("R Delay", "RDelay"),
                ("L Feedb.", "LFeedback"),
                ("R Feedb.", "RFeedback"),
                ("", "MuteDrySignal"),
            ],
        ),
        "*SendDevice" => ("SendDevice", &[("Amount", "SendAmount"), ("DestSend", "DestSendTrack"), ("", "MuteSource")]),
        "Filter" => ("Filter1Device", &[("Cutoff", "Cutoff"), ("Reso. / Q", "Resonance"), ("Inertia", "Innertia"), ("Type", "Type")]),
        "Filter2" => ("Filter2Device", &[("Cutoff", "Cutoff"), ("Reso. / Q", "Resonance"), ("Inertia", "Innertia"), ("Type", "Type")]),
        "EQ 10" | "Eq 10" => (
            "Eq10Device",
            &[
                ("50 Hz", "Gain0"),
                ("100 Hz", "Gain1"),
                ("300 Hz", "Gain2"),
                ("600 Hz", "Gain3"),
                ("1200 Hz", "Gain4"),
                ("2000 Hz", "Gain5"),
                ("3000 Hz", "Gain6"),
                ("5000 Hz", "Gain7"),
                ("10000 Hz", "Gain8"),
                ("15000 Hz", "Gain9"),
            ],
        ),
        "EQ 5" | "Eq 5" => (
            "Eq5Device",
            &[("100 Hz", "Gain0"), ("1000 Hz", "Gain1"), ("4000 Hz", "Gain2"), ("8000 Hz", "Gain3"), ("12000 Hz", "Gain4")],
        ),
        "Gainer" => ("GainerDevice", &[("Gain", "Volume"), ("Panning", "Panning")]),
        "Phaser" => (
            "PhaserDevice",
            &[
                ("Ceiling", "Ceilling"),
                ("Floor", "Floor"),
                ("LFO Rate", "LFORate"),
                ("Depth", "Depth"),
                ("Feedback", "Feedback"),
                ("Phase", "Phase"),
            ],
        ),
        "Reverb" => (
            "ReverbDevice",
            &[("Send", "Send"), ("Room Size", "RoomSize"), ("Width", "Width"), ("Damp", "Damp"), ("Dry Mix", "DryMix")],
        ),
        "mpReverb" => (
            "Reverb2Device",
            &[("Duration", "ReverbTime"), ("LP Filter", "LPFilter"), ("Predelay", "PreDelay"), ("Dry Mix", "DryMix"), ("Send", "Send")],
        ),
        "Compressor/Limiter" | "Compressor" => (
            "CompressorDevice",
            &[("Threshold", "LogThreshold"), ("Ratio", "Ratio"), ("Attack", "Attack"), ("Release", "Release"), ("Post Gain", "Gain")],
        ),
        "Distortion" | "Distorsion" => (
            "DistortionDevice",
            &[
                ("Threshold", "Threshold"),
                ("LP/Clamp", "LpOrClamp"),
                ("Wet Out", "WetOut"),
                ("Dry Out", "DryOut"),
                ("Gate/Filter", "GateOrFilter"),
                ("Type", "Type"),
            ],
        ),
        "LofiMat" => (
            "LofiDevice",
            &[
                ("Bit Crunch", "BitCrunsh"),
                ("Quality", "Quality"),
                ("Noise", "Noise"),
                ("Wet Out", "WetOut"),
                ("Dry Out", "DryOut"),
                ("Smooth", "Interpolate"),
            ],
        ),
        "Flanger" => (
            "FlangerDevice",
            &[
                ("Amount", "Amount"),
                ("Rate", "Rate"),
                ("Amplitude", "Amplitude"),
                ("Feedback", "FeedBack"),
                ("Delay", "Delay"),
                ("Phase", "Dephase"),
            ],
        ),
        "Shaper" => (
            "ShaperDevice",
            &[("Threshold", "Threshold"), ("Ratio", "Ratio"), ("Attack", "Attack"), ("Release", "Release"), ("Gain", "Gain")],
        ),
        "StereoExpander" => ("StereoExpanderDevice", &[("Amount", "StereoWidth")]),
        "DC Offset" => ("DcOffsetDevice", &[("DC Offset", "DCOffset"), ("Auto DC", "AutoDC")]),
        "Mixer EQ" => (
            "MixerEqDevice",
            &[("Low Gain", "LowGain"), ("Mid Gain", "MidGain"), ("Mid Freq.", "MidFreq"), ("Mid Q", "MidQ"), ("Hi Gain", "HiGain")],
        ),
        "*LfoDevice" => (
            "LfoDevice",
            &[
                ("DestEffect", "DestEffect"),
                ("Parameter", "DestParameter"),
                ("Amplitude", "Amplitude"),
                ("Offset", "Offset"),
                ("Frequency", "Frequency"),
                ("Type", "Type"),
            ],
        ),
        _ => return None,
    })
}

/// A device from its kind and the values its file gives, by .xrns name.
fn device(kind: &str, active: bool, values: &[(&str, f32)], track: TrackKind) -> Device {
    let Some((xml, names)) = device_kind(kind) else {
        // A plugin, or a device that's gone: silent, kept so automation's device numbers hold.
        return Device {
            kind: format!("Rns:{kind}"),
            active: false,
            ..Default::default()
        };
    };
    let xml = match (xml, track) {
        ("SequencerTrackDevice", TrackKind::Master) => "SequencerMasterTrackDevice",
        ("SequencerTrackDevice", TrackKind::Send) => "SequencerSendTrackDevice",
        _ => xml,
    };
    let mut params: Vec<(String, f32)> = names
        .iter()
        .map(|&(_, n)| (n.to_string(), values.iter().find(|(v, _)| *v == n).map_or_else(|| default(xml, n), |&(_, x)| x)))
        .collect();
    if xml.starts_with("Sequencer") {
        params.push(("PostPanning".into(), 0.5));
        params.push(("PostVolume".into(), 1.0));
    }
    Device {
        kind: xml.to_string(),
        active,
        params,
        ..Default::default()
    }
}

fn default(kind: &str, name: &str) -> f32 {
    match (kind, name) {
        (_, "Panning") => 0.5,
        (_, "Volume" | "StereoWidth" | "Quality" | "Send") => 1.0,
        ("DelayDevice", "TrackSend") => 127.0,
        _ => 0.0,
    }
}

/// A device's binary data (TrackDsp up to V02, after the name its
/// version may give it): its values in the order Renoise 1 keeps them.
fn binary_device(kind: &str, body: &[u8], old: bool, rate: f32, track: TrackKind) -> Device {
    // A name and version first ("Delay01", "TrackDevice00"), from Renoise 1.1 on.
    let mut b = body;
    let mut tagged = false;
    let mut send_version = 0;
    if let Some(z) = b.iter().take(24).position(|&c| c == 0) {
        let name = &b[..z];
        if z >= 3 && name.iter().all(|c| c.is_ascii_alphanumeric() || *c == b'_') && name[0].is_ascii_alphabetic() && name.last().is_some_and(u8::is_ascii_digit) {
            if name.starts_with(b"SendDevice") {
                send_version = name[z - 1] - b'0';
            }
            tagged = true;
            b = &b[z + 1..];
        }
    }
    let mut r = R::new(b);
    let f = |r: &mut R| r.f32().unwrap_or(0.0);
    let u = |r: &mut R| r.u32().unwrap_or(0) as f32;
    let byte = |r: &mut R| f32::from(r.u8().unwrap_or(0));
    let act = |r: &mut R| r.u8().is_none_or(|a| a != 0);
    let mut values: Vec<(&str, f32)> = Vec::new();
    let mut active = true;
    match kind {
        "TrackVolPan" | "MasterProcessor" => {
            values.push(("Panning", f(&mut r)));
            values.push(("Volume", f(&mut r)));
            if tagged {
                values.push(("Surround", u(&mut r)));
            } else if let Some(at) = find(b, b"SurroundWidth\0", 0) {
                r.p = at + 14;
                values.push(("Surround", u(&mut r)));
            }
        }
        "Delay" => {
            active = act(&mut r);
            let (lf, rf) = (f(&mut r), f(&mut r));
            let (mut ld, mut rd) = (u(&mut r), u(&mut r));
            if old {
                // Renoise 1.0 kept the delays in samples.
                ld = ld * 1000.0 / rate;
                rd = rd * 1000.0 / rate;
            }
            let mute = u(&mut r);
            let send = u(&mut r);
            values.extend([
                ("LFeedback", lf),
                ("RFeedback", rf),
                ("LDelay", ld),
                ("RDelay", rd),
                ("MuteDrySignal", mute.min(1.0)),
                ("TrackSend", send),
            ]);
        }
        "Filter" | "Filter2" => {
            active = act(&mut r);
            let ty = u(&mut r);
            let reso = if tagged || kind == "Filter2" { f(&mut r) } else { u(&mut r) };
            let cutoff = f(&mut r);
            let inertia = f(&mut r);
            values.extend([("Type", ty), ("Resonance", reso), ("Cutoff", cutoff), ("Innertia", inertia)]);
        }
        "EQ 10" | "Eq 10" | "EQ 5" | "Eq 5" => {
            if tagged {
                active = act(&mut r);
            }
            let names: &[&str] = if kind.ends_with("10") {
                &["Gain0", "Gain1", "Gain2", "Gain3", "Gain4", "Gain5", "Gain6", "Gain7", "Gain8", "Gain9"]
            } else {
                &["Gain0", "Gain1", "Gain2", "Gain3", "Gain4"]
            };
            for n in names {
                values.push((n, f(&mut r)));
            }
        }
        "Gainer" => {
            if tagged {
                active = act(&mut r);
            }
            values.push(("Volume", f(&mut r)));
            if tagged {
                values.push(("Panning", f(&mut r)));
            }
        }
        "Phaser" => {
            active = act(&mut r);
            let phase = f(&mut r);
            let (ceil, floor) = (u(&mut r), u(&mut r));
            values.extend([
                ("Phase", phase),
                ("Ceilling", ceil),
                ("Floor", floor),
                ("LFORate", f(&mut r)),
                ("Depth", f(&mut r)),
                ("Feedback", f(&mut r)),
            ]);
        }
        "Reverb" => {
            active = act(&mut r);
            values.extend([("RoomSize", f(&mut r)), ("Damp", f(&mut r)), ("Width", f(&mut r)), ("Send", f(&mut r)), ("DryMix", f(&mut r))]);
        }
        "Distortion" | "Distorsion" => {
            active = act(&mut r);
            let ty = byte(&mut r);
            values.extend([
                ("Type", ty),
                ("Threshold", f(&mut r)),
                ("LpOrClamp", f(&mut r)),
                ("GateOrFilter", byte(&mut r)),
                ("WetOut", u(&mut r)),
                ("DryOut", u(&mut r)),
            ]);
        }
        "LofiMat" => {
            active = act(&mut r);
            let smooth = byte(&mut r);
            values.extend([
                ("Interpolate", smooth),
                ("BitCrunsh", f(&mut r)),
                ("Quality", f(&mut r)),
                ("Noise", f(&mut r)),
                ("WetOut", u(&mut r)),
                ("DryOut", u(&mut r)),
            ]);
        }
        "Shaper" => {
            active = act(&mut r);
            r.skip(1);
            values.extend([("Threshold", u(&mut r)), ("Ratio", f(&mut r)), ("Attack", u(&mut r)), ("Release", u(&mut r)), ("Gain", f(&mut r))]);
        }
        "Flanger" => {
            active = act(&mut r);
            let amount = f(&mut r);
            r.skip(4);
            // The rate in radians a sample at a quarter speed; Renoise 1.5 shows it in Hz.
            let rate_hz = f(&mut r) * rate / 4.0;
            values.extend([
                ("Amount", amount),
                ("Rate", rate_hz),
                ("Amplitude", f(&mut r)),
                ("FeedBack", f(&mut r)),
                ("Delay", f(&mut r)),
                ("Dephase", f(&mut r)),
            ]);
        }
        "Compressor/Limiter" => {
            active = act(&mut r);
            if b.len() >= 31 {
                r.skip(1);
            }
            values.extend([("LogThreshold", f(&mut r)), ("Ratio", f(&mut r)), ("Attack", f(&mut r)), ("Release", f(&mut r)), ("Gain", f(&mut r))]);
        }
        "StereoExpander" => {
            active = act(&mut r);
            values.push(("StereoWidth", f(&mut r)));
        }
        "DC Offset" => {
            values.extend([("DCOffset", f(&mut r)), ("AutoDC", byte(&mut r))]);
        }
        "*SendDevice" => {
            if send_version >= 2 {
                active = act(&mut r);
            }
            values.extend([("SendAmount", f(&mut r)), ("DestSendTrack", u(&mut r)), ("MuteSource", byte(&mut r).min(1.0))]);
        }
        "*LfoDevice" => {
            active = act(&mut r);
            values.extend([
                ("DestEffect", u(&mut r)),
                ("DestParameter", u(&mut r)),
                ("Amplitude", f(&mut r)),
                ("Offset", f(&mut r)),
                ("Frequency", f(&mut r)),
                ("Type", u(&mut r)),
            ]);
        }
        _ => active = false,
    }
    device(kind, active, &values, track)
}

/// The size of each device's data in TrackDsp V00 and V01, which don't give it.
fn old_size(kind: &str) -> Option<usize> {
    Some(match kind {
        "TrackVolPan" | "MasterProcessor" => 37,
        "Delay" | "Phaser" => 25,
        "Filter" | "Flanger" => 29,
        "Reverb" => 21,
        "Distortion" | "Distorsion" => 19,
        "Gainer" => 4,
        "LofiMat" | "Compressor" => 22,
        "Eq 5" => 20,
        _ => return None,
    })
}

/// A V03 device: its version's name, on switch, and named parameters.
fn named_device(kind: &str, body: &[u8], track: TrackKind) -> Device {
    let mut r = R::new(body);
    let Some(z) = body.iter().position(|&c| c == 0) else { return device(kind, false, &[], track) };
    r.p = z + 1;
    r.skip(1);
    let n = r.u32().unwrap_or(0) as usize;
    let names = device_kind(kind).map_or(&[][..], |(_, n)| n);
    let mut values: Vec<(&str, f32)> = Vec::new();
    // A parameter's name, then four bytes and its value and one more; the
    // sync switches have no value. Now and then mapping data follows.
    let is_name = |p: usize| -> bool {
        let Some(len) = body.get(p..p + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize) else {
            return false;
        };
        (2..64).contains(&len) && body.get(p + 3 + len) == Some(&0) && body.get(p + 4..p + 3 + len).is_some_and(|s| s.iter().all(|c| (32..127).contains(c)))
    };
    for k in 0..n.min(512) {
        let Some(name) = r.str() else { break };
        let at = r.p;
        let size = if name.starts_with("Sync ") { 5 } else { 9 };
        let mut v = R { d: body, p: at + 4 }.f32().unwrap_or(0.0);
        r.p = at + size;
        if k + 1 < n && !is_name(r.p) {
            if let Some(next) = (at + 5..(at + 96).min(body.len())).find(|&p| is_name(p)) {
                v = R { d: body, p: at + 5 }.f32().unwrap_or(v);
                r.p = next;
            }
        }
        if let Some(&(_, xml)) = names.iter().find(|(rn, _)| *rn == name) {
            values.push((xml, v));
        }
    }
    device(kind, true, &values, track)
}

fn track_kind(t: usize, nt: usize) -> TrackKind {
    match t.checked_sub(nt) {
        None => TrackKind::Normal,
        Some(0) => TrackKind::Master,
        Some(_) => TrackKind::Send,
    }
}

/// Each track's devices.
fn track_dsp(d: &[u8], ver: u32, count: usize, nt: usize, rate: f32) -> Vec<Vec<Device>> {
    let mut out = Vec::new();
    let mut r = R::new(d);
    'tracks: for t in 0..count {
        let kind_of = track_kind(t, nt);
        let mut devs = Vec::new();
        if ver >= 3 {
            if !r.peek(b"GlobalTrackDocument00\0") {
                break;
            }
            r.p += 22;
            let Some(_) = r.u32() else { break };
            let Some(n) = r.u32() else { break };
            if r.skip(4 * n as usize).is_none() || r.str().is_none() || r.skip(10).is_none() {
                break;
            }
        }
        let Some(n) = r.u32() else { break };
        if n > 64 {
            break;
        }
        for _ in 0..n {
            let Some(kind) = r.str() else { break 'tracks };
            if ver >= 2 {
                let Some(size) = r.u32() else { break 'tracks };
                let Some(body) = r.bytes(size as usize) else { break 'tracks };
                devs.push(if ver >= 3 {
                    named_device(&kind, body, kind_of)
                } else {
                    binary_device(&kind, body, false, rate, kind_of)
                });
            } else {
                let Some(size) = old_size(&kind) else { break 'tracks };
                let Some(body) = r.bytes(size) else { break 'tracks };
                devs.push(binary_device(&kind, body, true, rate, kind_of));
            }
        }
        out.push(devs);
    }
    out
}

/// Automation: for each pattern, each track's envelopes (device, parameter,
/// and its points; up to Renoise 1.27 a value 0-127 on each line, FF none).
fn automation(d: &[u8], pats: &mut [Pattern], tracks: &[Track]) {
    let nt = tracks.len().max(1);
    let mut r = R::new(d);
    let mut slot = 0usize;
    while r.left() >= 8 {
        let (p, t) = (slot / nt, slot % nt);
        slot += 1;
        r.skip(4);
        let Some(n) = r.u32() else { return };
        if n > 256 {
            return;
        }
        let lines = pats.get(p).map_or(64, |x| x.lines);
        for _ in 0..n {
            let a = if r.peek(b"TrackEnvelope03\0") {
                r.p += 16;
                let (Some(device), Some(param), Some(len), Some(mode), Some(k)) = (r.u32(), r.u32(), r.u32(), r.u32(), r.u32()) else {
                    return;
                };
                let per = f64::from(len.max(1)) / lines.max(1) as f64;
                let mut points = Vec::new();
                for _ in 0..k.min(65536) {
                    let (Some(at), Some(v)) = (r.u32(), r.f32()) else { return };
                    points.push((f64::from(at) / per, v));
                }
                let interp = match mode {
                    1 => Interp::Points,
                    2 => Interp::Linear,
                    _ => Interp::Curve,
                };
                Automation {
                    device: device as usize,
                    param: param as usize,
                    interp,
                    points,
                }
            } else {
                let (device, param, len) = if r.peek(b"TrackEnvelope02\0") {
                    r.p += 16;
                    let (Some(len), Some(device)) = (r.u32(), r.u32()) else { return };
                    r.skip(1);
                    let Some(param) = r.u32() else { return };
                    (device, param, len as usize)
                } else {
                    if r.peek(b"TrackEnvelope01\0") {
                        r.p += 16;
                    }
                    r.skip(1);
                    let (Some(device), Some(param)) = (r.u32(), r.u32()) else { return };
                    (device, param, lines)
                };
                let Some(table) = r.bytes(len) else { return };
                let points = table.iter().enumerate().filter(|(_, &v)| v != 255).map(|(i, &v)| (i as f64, f32::from(v.min(127)) / 127.0)).collect();
                Automation {
                    device: device as usize,
                    param: param as usize,
                    interp: Interp::Linear,
                    points,
                }
            };
            let mut a = a;
            // Renoise 1's track volume ran to 0 dB; 1.5's to +3 dB.
            if a.param == 1 && tracks.get(t).and_then(|x| x.devices.get(a.device)).is_some_and(|d| d.kind.starts_with("Sequencer")) {
                for p in a.points.iter_mut() {
                    p.1 /= 1.412_538;
                }
            }
            if let Some(pt) = pats.get_mut(p).and_then(|x| x.tracks.get_mut(t)) {
                pt.automation.push(a);
            }
        }
    }
}

/// A short description of an instrument for its patch: what it plays and how.
fn patch(ins: &RawInstrument, env: Option<&Envelopes>) -> String {
    let mut s = format!("<RnsInstrument nna=\"{}\" split=\"", ins.nna);
    for b in &ins.split {
        s.push_str(&format!("{b:x}"));
    }
    s.push('"');
    s.push('>');
    for x in &ins.samples {
        s.push_str(&format!(
            "<Sample volume=\"{}\" pan=\"{}\" frames=\"{}\" finetune=\"{}\" base=\"{}\" loop=\"{} {} {}\"/>",
            x.volume, x.pan, x.frames, x.finetune, x.base, x.looping, x.loop_start, x.loop_end
        ));
    }
    if let Some(e) = env {
        s.push_str(&format!("<Envelopes>{e:?}</Envelopes>"));
    }
    s.push_str("</RnsInstrument>");
    s
}

/// The song in a .rns file.
pub fn read(d: &[u8]) -> Option<Song> {
    if !is_rns(d) {
        return None;
    }
    let version: u32 = text(&d[3..d.len().min(8)]).parse().ok()?;
    let cs = chunks(d);
    let get = |name: &str| cs.iter().find(|c| c.0 == name).map(|c| (c.1, c.2));
    let mut song = Song {
        doc: 4,
        speed_timing: true,
        lpb: 4,
        tpl: 6,
        bpm: 125.0,
        headroom: 1.0,
        ..Default::default()
    };
    if let Some((ver, h)) = get("Header") {
        let mut r = R::new(h);
        if ver == 0 {
            song.name = r.bytes(20).map(text).unwrap_or_default();
            song.artist = r.bytes(20).map(text).unwrap_or_default();
        } else {
            song.name = r.str().unwrap_or_default();
            song.artist = r.str().unwrap_or_default();
            song.comments = strings_in(&h[r.p.min(h.len())..]);
        }
    }
    let mut rate = 44100.0;
    if let Some((_, p)) = get("Player") {
        let mut r = R::new(p);
        if r.peek(b"GlobalSongData00\0") {
            r.p = 17;
        }
        r.skip(4);
        rate = r.u32().filter(|&x| (8000..=192_000).contains(&x)).unwrap_or(44100) as f32;
        r.skip(4);
        song.bpm = (r.u32().unwrap_or(125) as f32).clamp(32.0, 999.0);
        song.tpl = r.u32().unwrap_or(6).clamp(1, 32);
    }
    let (ver, pe) = get("PatternEditor")?;
    let (tr, pats) = pattern_editor(pe, ver)?;
    let count = tr.names.len() + tr.extra;
    let dsp = get("TrackDsp").map_or_else(Vec::new, |(v, b)| track_dsp(b, v, count, tr.names.len(), rate));
    for t in 0..count {
        let kind = track_kind(t, tr.names.len());
        let name = match kind {
            TrackKind::Normal => tr.names[t].clone(),
            TrackKind::Master => "Master".to_string(),
            _ => format!("S{:02}", t - tr.names.len() - 1),
        };
        let mut devices = dsp.get(t).cloned().unwrap_or_default();
        if !devices.first().is_some_and(|d| d.kind.starts_with("Sequencer")) {
            devices.insert(0, device("TrackVolPan", true, &[], kind));
        }
        song.tracks.push(Track {
            kind,
            name,
            on: true,
            note_columns: tr.notes.get(t).copied().unwrap_or(0),
            fx_columns: tr.fx.get(t).copied().unwrap_or(1),
            column_off: Vec::new(),
            routing: 0,
            group: None,
            delay_ms: 0.0,
            devices,
        });
    }
    song.patterns = pats
        .into_iter()
        .map(|p| Pattern {
            name: p.name,
            lines: p.lines.max(1),
            tracks: p
                .tracks
                .into_iter()
                .map(|lines| PatternTrack {
                    lines,
                    automation: Vec::new(),
                    alias: None,
                })
                .collect(),
        })
        .collect();
    if let Some((v, b)) = get("TrackEnvelopes") {
        if v >= 1 {
            automation(b, &mut song.patterns, &song.tracks);
        }
    }
    song.sequence = get("Sequencer")
        .map_or_else(Vec::new, |(_, b)| sequence(b))
        .into_iter()
        .filter(|&p| p < song.patterns.len())
        .map(|pattern| Entry { pattern, muted: Vec::new() })
        .collect();
    if song.sequence.is_empty() && !song.patterns.is_empty() {
        song.sequence.push(Entry { pattern: 0, muted: Vec::new() });
    }
    let raw = get("Instruments").and_then(|(v, b)| instruments(b, v)).unwrap_or_default();
    let waves = get("SampleData").map_or_else(Vec::new, |(_, b)| audio(b, &raw));
    // Renoise 0.5's envelopes are laid out otherwise: not read.
    let envs = get("Envelopes").filter(|(v, _)| *v >= 1 || version > 5).map_or_else(|| vec![None; 128], |(_, b)| envelopes(b));
    for (k, ins) in raw.iter().enumerate() {
        let nna = match ins.nna {
            1 => Nna::Off,
            2 => Nna::Continue,
            _ => Nna::Cut,
        };
        let samples: Vec<Sample> = ins
            .samples
            .iter()
            .enumerate()
            .map(|(j, s)| Sample {
                name: s.name.clone(),
                audio: waves.get(k).and_then(|w| w.get(j)).cloned().unwrap_or_default(),
                volume: s.volume,
                pan: s.pan,
                base_note: s.base,
                transpose: 0,
                finetune: s.finetune,
                nna,
                interp: SampleInterp::Cubic,
                loop_mode: loop_of(u32::from(s.looping)).unwrap_or(Loop::Off),
                loop_start: s.loop_start,
                loop_end: s.loop_end.min(s.frames),
                loop_release: false,
                beat_sync: s.sync,
                autofade: false,
                oneshot: false,
                mute_group: -1,
                mod_set: 0,
                chain: 0,
                alias: false,
                slices: Vec::new(),
            })
            .collect();
        // Each note's sample, by its slot (only filled slots are kept).
        let mut zones = Vec::new();
        let mut at = 0;
        while at < ins.split.len() {
            let slot = usize::from(ins.split[at]);
            let mut end = at;
            while end + 1 < ins.split.len() && ins.split[end + 1] == ins.split[at] {
                end += 1;
            }
            if let Some(s) = ins.slots.iter().position(|&x| x == slot) {
                zones.push(Zone {
                    sample: s,
                    base_note: samples[s].base_note,
                    notes: (at as u8, end as u8),
                    vels: (0, 127),
                    vel_to_volume: true,
                    key_to_pitch: true,
                    off_layer: false,
                });
            }
            at = end + 1;
        }
        let env = envs.get(k).cloned().flatten();
        song.instruments.push(Instrument {
            name: ins.name.clone(),
            xml: patch(ins, env.as_ref()),
            samples,
            zones,
            envelopes: env.map(Arc::new),
            volume: 1.0,
            plugin: (ins.plugin && ins.samples.iter().all(|s| s.frames == 0)).then(|| "VSTi".to_string()),
            macros: [0.5; 8],
            ..Default::default()
        });
    }
    Some(song)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A one-track Renoise 1.16 song: its header, a pattern, the sequence,
    /// the player, an instrument with a looped square wave, its samples.
    fn little_rns() -> Vec<u8> {
        fn chunk(out: &mut Vec<u8>, name: &str, body: &[u8]) {
            out.extend(START);
            let mut n = name.as_bytes().to_vec();
            n.resize(19, b' ');
            n.push(0);
            out.extend(n);
            out.extend(body);
            out.extend(END);
        }
        fn s(v: &mut Vec<u8>, t: &str) {
            v.extend(&(t.len() as u32 + 1).to_le_bytes());
            v.extend(t.as_bytes());
            v.push(0);
        }
        let mut f = b"RNS016\0\0\0".to_vec();
        let mut h = Vec::new();
        s(&mut h, "Little");
        s(&mut h, "Someone");
        chunk(&mut f, "Header V01", &h);
        let mut p = Vec::new();
        p.extend(&1u32.to_le_bytes());
        p.extend(&0u32.to_le_bytes());
        p.extend(&0u32.to_le_bytes());
        s(&mut p, "One");
        p.extend(&[1, 1, 0, 0]);
        p.extend(&1u32.to_le_bytes());
        p.extend(&8u32.to_le_bytes());
        p.push(0);
        for l in 0..8u8 {
            // D-4 (50) of instrument 0 at full volume on line 1.
            p.extend(if l == 1 { [50, 0, 0x40, 255] } else { [121, 255, 255, 255] });
        }
        p.extend(&[0u8; 16]);
        chunk(&mut f, "PatternEditor V02", &p);
        let mut q = vec![1u8, 0, 0];
        q.resize(258, 0);
        chunk(&mut f, "Sequencer V00", &q);
        let mut pl = Vec::new();
        pl.extend(&1.0f32.to_le_bytes());
        pl.extend(&44100u32.to_le_bytes());
        pl.extend(&0u32.to_le_bytes());
        pl.extend(&120u32.to_le_bytes());
        pl.extend(&6u32.to_le_bytes());
        chunk(&mut f, "Player V01", &pl);
        let mut ins = vec![0u8; 12];
        s(&mut ins, "square");
        ins.extend(&[0u8; 120]);
        ins.extend(&0u32.to_le_bytes());
        ins.push(1);
        ins.extend(&[0u8; 15]);
        s(&mut ins, "square");
        ins.extend(&[1, 0, 0, 0, 0]);
        ins.extend(&1.0f32.to_le_bytes());
        ins.extend(&100u32.to_le_bytes());
        ins.extend(&[1, 0, 48, 0, 16, 0, 1]);
        ins.extend(&0u32.to_le_bytes());
        ins.extend(&100u32.to_le_bytes());
        ins.extend(&1.0f32.to_le_bytes());
        ins.extend(&[0, 0]);
        chunk(&mut f, "Instruments V04", &ins);
        // A 441 Hz square: +16000 for 50 points, -16000 for 50, as deltas.
        let mut data = Vec::new();
        let mut last = 0i16;
        for i in 0..100 {
            let v: i16 = if i < 50 { 16000 } else { -16000 };
            data.extend(&v.wrapping_sub(last).to_le_bytes());
            last = v;
        }
        chunk(&mut f, "SampleData V00", &data);
        f
    }

    #[test]
    fn a_renoise_1_song_reads_and_plays_its_note() {
        let s = read(&little_rns()).expect("read");
        assert_eq!((s.name.as_str(), s.artist.as_str()), ("Little", "Someone"));
        assert_eq!((s.bpm, s.tpl, s.sequence.len()), (120.0, 6, 1));
        let line = &s.patterns[0].tracks[0].lines[0];
        assert_eq!((line.0, line.1.notes[0].note, line.1.notes[0].vol), (1, 50, Col::Set(0x80)));
        assert_eq!(s.instruments[0].samples[0].audio.frames(), 100);
        assert_eq!(s.instruments[0].samples[0].audio.channels[0][60], -16000.0 / 32768.0);
        // Speed timing: a line is 6 ticks of 2.5/120 s.
        let out = super::super::render_stereo(Arc::new(s), 0.5);
        let first = out.chunks(2).position(|f| f[0].abs() > 0.01).expect("sound");
        assert!((5400..5700).contains(&first), "starts at {first}");
    }

    /// Every .rns in RNS_OLD reads: what each holds.
    #[test]
    #[ignore]
    fn read_all_rns() {
        let dir = std::env::var("RNS_OLD").expect("RNS_OLD");
        let mut names: Vec<_> = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.path()).collect();
        names.sort();
        let (mut ok, mut total) = (0, 0);
        for p in names {
            let d = std::fs::read(&p).unwrap();
            total += 1;
            let name = p.file_name().unwrap().to_string_lossy().to_string();
            let Some(s) = read(&d) else {
                println!("{name}: not read");
                continue;
            };
            ok += 1;
            let samples: usize = s.instruments.iter().map(|i| i.samples.len()).sum();
            let silent = s.instruments.iter().flat_map(|i| &i.samples).filter(|x| x.audio.frames() == 0).count();
            let envs = s.instruments.iter().filter(|i| i.envelopes.is_some()).count();
            let autos: usize = s.patterns.iter().flat_map(|p| &p.tracks).map(|t| t.automation.len()).sum();
            let devs: Vec<String> = s.tracks.iter().flat_map(|t| t.devices.iter().skip(1)).map(|d| d.kind.clone()).collect();
            let unknown = devs.iter().filter(|k| k.starts_with("Rns:")).count();
            println!(
                "{name}: {} tracks {} patterns {} positions {samples} samples ({silent} empty) {envs} envelope sets {autos} automations {} devices ({unknown} silent) bpm {} tpl {}",
                s.tracks.len(),
                s.patterns.len(),
                s.sequence.len(),
                devs.len(),
                s.bpm,
                s.tpl
            );
        }
        println!("{ok} of {total} read");
    }

    /// Renders every .rns in RNS_OLD (RNS_ONLY a name part, RNS_SECS) and
    /// prints its levels; WAVs to RNS_OUT when set.
    #[test]
    #[ignore]
    fn render_all_rns() {
        let dir = std::env::var("RNS_OLD").expect("RNS_OLD");
        let only = std::env::var("RNS_ONLY").ok();
        let out = std::env::var("RNS_OUT").ok();
        let secs: f64 = std::env::var("RNS_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(30.0);
        let mut names: Vec<_> = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.path()).collect();
        names.sort();
        for p in names {
            let name = p.file_stem().unwrap().to_string_lossy().to_string();
            if only.as_ref().is_some_and(|o| !name.contains(o.as_str())) {
                continue;
            }
            let Some(s) = read(&std::fs::read(&p).unwrap()) else { continue };
            let t0 = std::time::Instant::now();
            let data = super::super::render_stereo(Arc::new(s), secs);
            let el = t0.elapsed().as_secs_f64();
            let peak = data.iter().fold(0.0f32, |a, &x| a.max(x.abs()));
            let rms = (data.iter().map(|x| x * x).sum::<f32>() / data.len().max(1) as f32).sqrt();
            let nan = data.iter().filter(|x| !x.is_finite()).count();
            let silent = data.chunks(4410 * 2).filter(|c| c.iter().all(|x| x.abs() < 1e-4)).count();
            println!(
                "{name}: {:.1}s in {el:.2}s, peak {peak:.3} rms {rms:.4} silent {silent}/{} nan {nan}",
                data.len() as f64 / 88200.0,
                data.len() / 8820 + 1
            );
            if let Some(o) = &out {
                let mut w = Vec::new();
                let n = data.len() * 2;
                w.extend(b"RIFF");
                w.extend(&((36 + n) as u32).to_le_bytes());
                w.extend(b"WAVEfmt ");
                w.extend(&16u32.to_le_bytes());
                w.extend(&[1, 0, 2, 0]);
                w.extend(&44100u32.to_le_bytes());
                w.extend(&(44100u32 * 4).to_le_bytes());
                w.extend(&[4, 0, 16, 0]);
                w.extend(b"data");
                w.extend(&(n as u32).to_le_bytes());
                for &x in &data {
                    w.extend(&((x.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
                }
                std::fs::write(std::path::Path::new(o).join(format!("{name}.wav")), w).unwrap();
            }
        }
    }

    /// Each track's peak and devices over RNS_SECS of the .rns RNS_ONLY names (in RNS_OLD).
    #[test]
    #[ignore]
    fn peaks_rns() {
        let dir = std::env::var("RNS_OLD").expect("RNS_OLD");
        let only = std::env::var("RNS_ONLY").expect("RNS_ONLY");
        let secs: f64 = std::env::var("RNS_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(30.0);
        let p = std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .find(|p| p.to_string_lossy().contains(only.as_str()))
            .expect("song");
        let s = Arc::new(read(&std::fs::read(&p).unwrap()).unwrap());
        let mut e = super::super::play::Engine::new(s.clone());
        e.first();
        let mut peaks = vec![0.0f32; s.tracks.len()];
        let mut t = 0.0;
        while t < secs {
            e.make(true);
            for (k, p) in e.track_peaks().into_iter().enumerate() {
                peaks[k] = peaks[k].max(p);
            }
            t += e.frames as f64 / 44100.0;
            e.advance();
        }
        let p0 = &s.patterns[s.sequence[0].pattern];
        for (t, pt) in p0.tracks.iter().enumerate() {
            println!(
                "first pattern track {t}: {} lines, first {:?}",
                pt.lines.len(),
                pt.lines.first().map(|(l, x)| (
                    l,
                    x.notes.iter().map(|c| (c.note, c.instr, c.vol)).collect::<Vec<_>>(),
                    x.fx.iter().map(|f| (f.num, f.val)).collect::<Vec<_>>()
                ))
            );
        }
        for (k, tr) in s.tracks.iter().enumerate() {
            let devs: Vec<String> = tr
                .devices
                .iter()
                .map(|d| {
                    format!(
                        "{}{} {:?}",
                        if d.active { "" } else { "(off) " },
                        d.kind,
                        d.params.iter().map(|(n, v)| format!("{n}={v}")).collect::<Vec<_>>()
                    )
                })
                .collect();
            println!("{k:2} {:?} {:20} peak {:.4} {:?}", tr.kind, tr.name, peaks[k], devs);
        }
        for (k, i) in s.instruments.iter().enumerate().filter(|(_, i)| !i.samples.is_empty()) {
            if let Some(e) = &i.envelopes {
                println!("    vol env {:?}", e.volume);
            }
            println!(
                "ins {k} {:?} vol {:?} env {:?}",
                i.name,
                i.samples.iter().map(|x| (x.volume, x.audio.frames())).collect::<Vec<_>>(),
                i.envelopes.as_ref().map(|e| (e.volume.on, e.volume.points.len(), e.volume.length, e.volume.sustain))
            );
        }
    }
}
