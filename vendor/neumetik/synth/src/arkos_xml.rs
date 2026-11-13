// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! Arkos Tracker's XML songs (.aks), read into `arkos::Song` as Arkos
//! Tracker 3's importers read them (MIT, see `arkos.rs`): Arkos Tracker
//! 1's ("ArkosTrackerSong 1.0a"), Arkos Tracker 2's ("aks:song", format
//! 1.0) and Arkos Tracker 3's (format 3.0). The XML is read by a small
//! parser of its own; a zipped or gzipped .aks is unpacked by the caller.

use std::sync::Arc;

pub(crate) use crate::xml::{base64, int_value, Node};
use crate::xml::{hex_value, parse_xml};

use crate::arkos::{self, Cell, Effect, Expression, Instrument, InstrumentCell, Link, Loop, Pattern, Position, Sample, Song, Subsong, XmlBuilt, RST_NOTE, TRACK_ROWS};

/// The note a sample plays at as a digidrum unless it says (C-6).
pub(crate) const DIGI_NOTE: i32 = 72;


pub(crate) fn parse(d: &[u8]) -> Option<Song> {
    let head = String::from_utf8_lossy(&d[..d.len().min(600)]);
    if head.contains("<Version>ArkosTrackerSong 1.0a</Version>") || head.contains("<Version>Arkos Tracker 1.0</Version>") {
        return parse_at1(&parse_xml(d)?, d);
    }
    let root = parse_xml(d)?;
    match root.name.as_str() {
        "aks:song" if root.string("aks:formatVersion") == "1.0" => parse_at2(&root, d),
        "song" if root.string("formatVersion") == "3.0" => crate::arkos_at3::parse(&root, d),
        _ => None,
    }
}

/// Whether the bytes are an Arkos Tracker XML song (unpacked).
pub fn probe(d: &[u8]) -> bool {
    let head = String::from_utf8_lossy(&d[..d.len().min(600)]);
    head.contains("<Version>ArkosTrackerSong 1.0a</Version>") || head.contains("<Version>Arkos Tracker 1.0</Version>") || (head.contains("<aks:song") && head.contains("<aks:formatVersion>1.0<")) || (head.contains("<song") && head.contains("<formatVersion>3.0<"))
}

/// How many hex digits an effect's value has, as Arkos Tracker counts them.
pub(crate) fn digits(e: Effect) -> u32 {
    use Effect::*;
    match e {
        Volume | Reset => 1,
        PitchTable | ArpeggioTable | Arpeggio3Notes | ForceArpeggioSpeed | ForcePitchTableSpeed | ForceInstrumentSpeed | Legato => 2,
        _ => 3,
    }
}

/// An effect's value from its raw three-digit value.
pub(crate) fn logical(e: Effect, raw: i32) -> i32 {
    let (shift, mask) = match digits(e) {
        1 => (8, 0xF),
        2 => (4, 0xFF),
        _ => (0, 0xFFF),
    };
    (raw >> shift) & mask
}

pub(crate) fn effect_from_name(s: &str) -> Option<Effect> {
    use Effect::*;
    Some(match s {
        "arpeggio3Notes" => Arpeggio3Notes,
        "arpeggio4Notes" => Arpeggio4Notes,
        "arpeggioTable" => ArpeggioTable,
        "forceArpeggioSpeed" => ForceArpeggioSpeed,
        "forceInstrumentSpeed" => ForceInstrumentSpeed,
        "forcePitchTableSpeed" => ForcePitchTableSpeed,
        "pitchDown" => PitchDown,
        "pitchUp" => PitchUp,
        "fastPitchDown" => FastPitchDown,
        "fastPitchUp" => FastPitchUp,
        "pitchGlide" => PitchGlide,
        "pitchTable" => PitchTable,
        "reset" => Reset,
        "volume" => Volume,
        "volumeIn" => VolumeIn,
        "volumeOut" => VolumeOut,
        _ => return None,
    })
}

pub(crate) fn link_from_name(s: &str) -> Option<Link> {
    Some(match s {
        "softOnly" => Link::SoftOnly,
        "hardOnly" => Link::HardOnly,
        "hardToSoft" => Link::HardToSoft,
        "softToHard" => Link::SoftToHard,
        "softAndHard" => Link::SoftAndHard,
        "noSoftNoHard" => Link::NoSoftNoHard,
        _ => return None,
    })
}

/// Sets an effect in slot `index` of a cell (slots fill up to four).
pub(crate) fn set_effect(cell: &mut Cell, index: usize, e: Effect, value: i32) {
    if index >= 4 {
        return;
    }
    if cell.effects.len() <= index {
        cell.effects.resize(index + 1, None);
    }
    cell.effects[index] = Some((e, value));
}

/// An instrument cell from its software and hardware values, primary
/// and secondary as the link makes them.
#[allow(clippy::too_many_arguments)]
pub(crate) fn instrument_cell(link: Link, volume: i32, noise: i32, ratio: i32, envelope: i32, retrig: bool, soft: (i32, i32, i32), hard: (i32, i32, i32)) -> InstrumentCell {
    let (p, s) = match link {
        Link::HardOnly | Link::HardToSoft => (hard, soft),
        _ => (soft, hard),
    };
    InstrumentCell { link, volume, noise, primary_period: p.0, primary_arpeggio: p.1, primary_pitch: p.2, ratio, envelope, secondary_period: s.0, secondary_arpeggio: s.1, secondary_pitch: s.2, retrig }
}

/// Puts a track's cell, growing the subsong's tracks as needed.
pub(crate) fn track_mut(sub: &mut Subsong, t: usize) -> &mut Vec<Cell> {
    if sub.tracks.len() <= t {
        sub.tracks.resize(t + 1, Vec::new());
    }
    let tr = &mut sub.tracks[t];
    if tr.len() < TRACK_ROWS {
        tr.resize(TRACK_ROWS, Cell::default());
    }
    tr
}

pub(crate) fn special_mut(sub: &mut Subsong, speed: bool, t: usize) -> &mut Vec<Option<i32>> {
    let v = if speed { &mut sub.speed_tracks } else { &mut sub.event_tracks };
    if v.len() <= t {
        v.resize(t + 1, Vec::new());
    }
    let tr = &mut v[t];
    if tr.len() < TRACK_ROWS {
        tr.resize(TRACK_ROWS, None);
    }
    tr
}

fn parse_at1(root: &Node, file: &[u8]) -> Option<Song> {
    const RST: i32 = 144;
    const EMPTY_NOTE: i32 = 145;
    const EMPTY_VOLUME: i32 = 255;
    let title = root.string("Name");
    let author = root.string("Author");
    let comments = root.string("Comments");
    let mut sub = arkos::new_subsong("Main");
    sub.loop_start = root.int("LoopStart").max(0) as usize;
    sub.end = root.int("LoopEnd").max(0) as usize;
    let freq = root.string("Frequency");
    sub.replay_hz = freq.strip_prefix("freq").and_then(|f| f.find("hz").filter(|&i| i >= 1).map(|i| int_value(&f[..i]))).filter(|&v| v > 0).map_or(50.0, f64::from);
    sub.psgs = vec![arkos::Psg::new(root.int_or("MasterFrequency", arkos::CPC_HZ), 440.0)];
    sub.digi_channel = root.int("SamplesChannel").clamp(0, 2) as usize;
    sub.initial_speed = root.int_or("BeginningSpeed", 6);
    let tracks = root.child("TrackArray")?;
    for (ti, wrapper) in tracks.children("Track").enumerate() {
        if wrapper.children.is_empty() {
            continue;
        }
        let mut ctx_volume: Option<i32> = None;
        let (mut up, mut down): (Option<i32>, Option<i32>) = (None, None);
        let Some(track) = wrapper.child("track") else { continue };
        let cells: Vec<&Node> = track.children("Cell").collect();
        let tr = track_mut(&mut sub, ti);
        for (ci, c) in cells.iter().enumerate().take(TRACK_ROWS) {
            let mut slot = 0;
            let note = c.int_or("Note", EMPTY_NOTE);
            let ins = c.int("Instrument");
            let volume = c.int("Volume");
            let pitch = c.int("Pitch");
            let cell = &mut tr[ci];
            if note != EMPTY_NOTE {
                up = None;
                down = None;
                if note == RST {
                    cell.note = Some(RST_NOTE);
                    cell.instrument = Some(0);
                } else {
                    cell.note = Some(note.clamp(0, 255) as u8);
                    cell.instrument = Some(ins.clamp(0, 255) as u8);
                }
            }
            if volume != EMPTY_VOLUME && ctx_volume != Some(volume) {
                ctx_volume = Some(volume);
                set_effect(cell, slot, Effect::Volume, logical(Effect::Volume, volume << 8));
                slot += 1;
            }
            if pitch != 0 {
                let v = pitch.abs() * 0x20;
                let (e, mine, other) = if pitch > 0 { (Effect::PitchUp, &mut up, &mut down) } else { (Effect::PitchDown, &mut down, &mut up) };
                *other = None;
                if *mine != Some(v) {
                    *mine = Some(v);
                    set_effect(cell, slot, e, logical(e, v));
                }
            }
        }
    }
    if let Some(specials) = root.child("SpecialTracksArray") {
        for (ti, wrapper) in specials.children("SpecialTrack").enumerate() {
            if wrapper.children.is_empty() {
                continue;
            }
            let Some(cells) = wrapper.child("SpecialCells") else { continue };
            let list: Vec<(i32, i32)> = cells.children("SpecialCell").map(|c| (c.int("Effect"), c.int("Value"))).collect();
            for (ci, (e, v)) in list.into_iter().enumerate().take(TRACK_ROWS) {
                if v > 0 && (e == 1 || e == 2) {
                    special_mut(&mut sub, e == 1, ti)[ci] = Some(v);
                }
            }
        }
    }
    for p in root.child("PatternsList")?.children("Pattern") {
        let tracks: Vec<usize> = (1..=3).map(|k| p.int(&format!("Track{k}Number")).max(0) as usize).collect();
        let trans: Vec<i32> = (1..=3).map(|k| p.int(&format!("Transposition{k}"))).collect();
        let special = p.int("SpecialTrackNumber").max(0) as usize;
        let pattern = sub.patterns.len();
        sub.patterns.push(Pattern { tracks, speed_track: special, event_track: special });
        sub.positions.push(Position { pattern, height: p.int("Height").clamp(1, TRACK_ROWS as i32) as usize, transpositions: trans });
    }
    let mut b = XmlBuilt::new();
    if let Some(list) = root.child("InstrumentsList") {
        for (ii, ins) in list.children("Instrument").enumerate() {
            if ii == 0 {
                continue;
            }
            let end = ins.int("LoopEnd");
            let mut instrument = Instrument { name: ins.string("Name"), speed: ins.int("Speed"), retrig: ins.bool("Retrig"), main_loop: Loop { start: ins.int("LoopStart"), end, looping: ins.bool("Loop") }, cells: Vec::new(), ..Default::default() };
            if let Some(items) = ins.child("InstrumentItems") {
                for (k, c) in items.children("InstrumentItem").enumerate() {
                    if k as i32 <= end {
                        instrument.cells.push(at1_cell(c));
                    }
                }
            }
            b.instrument(ii, instrument);
        }
    }
    arkos::finish_subsong(&mut sub);
    Some(Song::from_parts(title, author.clone(), author, comments, "aks1", vec![sub], b, file))
}

fn at1_cell(c: &Node) -> InstrumentCell {
    let sound = c.bool("IsSound");
    let link = if !c.bool("IsHard") {
        if sound { Link::SoftOnly } else { Link::NoSoftNoHard }
    } else {
        match c.int("LinkValue") {
            1 => {
                if sound { Link::SoftToHard } else { Link::SoftOnly }
            }
            2 => {
                if sound { Link::HardToSoft } else { Link::HardOnly }
            }
            _ => {
                if sound { Link::SoftAndHard } else { Link::HardOnly }
            }
        }
    };
    let soft = (c.int("SoundFrequency"), c.int("SoundArpeggio"), c.int("SoundPitch"));
    let hard = (c.int("HardwareFrequency"), c.int("HardwareArpeggio"), c.int("HardwarePitch"));
    instrument_cell(link, c.int("Volume"), c.int("Noise"), c.int("Shift"), c.int("HardwareEnvelope").clamp(8, 15), c.bool("IsRetrig"), soft, hard)
}

fn parse_at2(root: &Node, file: &[u8]) -> Option<Song> {
    let mut b = XmlBuilt::new();
    if let Some(list) = root.child("aks:fmInstruments") {
        for ins in list.children("aks:fmInstrument") {
            let index = ins.int_or("aks:number", -1);
            if index < 1 {
                return None;
            }
            let mut instrument = Instrument {
                name: ins.string("aks:title"),
                speed: ins.int("aks:speed"),
                retrig: ins.bool("aks:isRetrig"),
                main_loop: Loop { start: ins.int("aks:loopStartIndex"), end: ins.int("aks:endIndex"), looping: ins.bool("aks:isLooping") },
                ..Default::default()
            };
            for c in ins.children("aks:fmInstrumentCell") {
                let link = link_from_name(&c.string("aks:link")).unwrap_or(Link::SoftOnly);
                let soft = (c.int("aks:softwarePeriod"), c.int("aks:softwareArpeggio"), c.int("aks:softwarePitch"));
                let hard = (c.int("aks:hardwarePeriod"), c.int("aks:hardwareArpeggio"), c.int("aks:hardwarePitch"));
                instrument.cells.push(instrument_cell(link, c.int("aks:volume"), c.int("aks:noise"), c.int("aks:ratio"), c.int("aks:hardwareCurve"), c.bool("aks:isRetrig"), soft, hard));
            }
            b.instrument(index as usize, instrument);
        }
    }
    if let Some(list) = root.child("aks:sampleInstruments") {
        for ins in list.children("aks:sampleInstrument") {
            let index = ins.int_or("aks:number", -1);
            if index < 1 {
                return None;
            }
            let data = base64(&ins.string("aks:sampleData8bitsUnsignedBase64"));
            let sample_loop = Loop { start: ins.int("aks:loopStartIndex"), end: ins.int("aks:endIndex"), looping: ins.bool("aks:isLooping") };
            let sample = Sample { data: Arc::new(data), sample_loop, amplification: ins.float_or("aks:volumeRatio", 1.0) as f32, digi_note: DIGI_NOTE };
            b.instrument(index as usize, Instrument { name: ins.string("aks:title"), sample: Some(sample), ..Default::default() });
        }
    }
    for (arp, list, item) in [(true, "aks:arpeggios", "aks:arpeggio"), (false, "aks:pitchs", "aks:pitch")] {
        let Some(list) = root.child(list) else { continue };
        for e in list.children(item) {
            let index = e.int_or("aks:index", -1);
            if index < 1 {
                return None;
            }
            let values: Vec<i32> = if arp { e.children("aks:arpeggioCell").map(|c| c.int("aks:octave") * 12 + c.int("aks:note")).collect() } else { e.children("aks:value").map(|v| int_value(&v.all_text())).collect() };
            if values.is_empty() {
                continue;
            }
            b.expression(arp, index as usize, Expression { values, loop_start: e.int("aks:loopStartIndex"), end: e.int("aks:endIndex"), speed: e.int("aks:speed") });
        }
    }
    let mut subsongs = Vec::new();
    for s in root.child("aks:subsongs")?.children("aks:subsong") {
        let mut sub = arkos::new_subsong(&s.string("aks:title"));
        sub.initial_speed = s.int("aks:initialSpeed");
        sub.end = s.int("aks:endIndex").max(0) as usize;
        sub.loop_start = s.int("aks:loopStartIndex").max(0) as usize;
        sub.replay_hz = s.float_or("aks:replayFrequency", 50.0);
        sub.digi_channel = s.int("aks:digiChannel").max(0) as usize;
        sub.psgs = s
            .children("aks:psgMetadata")
            .map(|p| arkos::Psg {
                hz: p.int_or("aks:psgFrequency", arkos::CPC_HZ),
                reference: p.float_or("aks:referenceFrequency", 440.0),
                ym: p.string("aks:type") == "ym",
                sample_hz: p.int_or("aks:samplePlayerFrequency", 8000),
                pan: arkos::Psg::pan_from(&p.string("aks:mixingOutput")),
            })
            .collect();
        if sub.psgs.is_empty() {
            return None;
        }
        if let Some(tracks) = s.child("aks:tracks") {
            for t in tracks.children("aks:track") {
                let ti = t.int_or("aks:number", -1);
                if ti < 0 {
                    return None;
                }
                let tr = track_mut(&mut sub, ti as usize);
                for c in t.children("aks:cell") {
                    let ci = c.int("aks:index");
                    let Some(cell) = usize::try_from(ci).ok().and_then(|i| tr.get_mut(i)) else { continue };
                    let note = c.int_or("aks:note", -1);
                    let ins = c.int_or("aks:instrument", -1);
                    if note >= 0 {
                        cell.note = Some(note.min(255) as u8);
                    }
                    if ins >= 0 {
                        cell.instrument = Some(ins.min(255) as u8);
                    }
                    for ev in c.children("aks:effectAndValue") {
                        let (Some(e), Some(raw)) = (effect_from_name(&ev.string("aks:effect")), hex_value(&ev.string("aks:hexValue"))) else { continue };
                        if note >= 0 && ins >= 0 && e == Effect::PitchGlide {
                            cell.instrument = None;
                        }
                        set_effect(cell, ev.int("aks:index").max(0) as usize, e, logical(e, raw));
                    }
                }
            }
        }
        for (speed, list, item, cell_tag) in [(true, "aks:speedTracks", "aks:speedTrack", "aks:speedCell"), (false, "aks:eventTracks", "aks:eventTrack", "aks:eventCell")] {
            let Some(list) = s.child(list) else { continue };
            for t in list.children(item) {
                let ti = t.int_or("aks:number", -1);
                if ti < 0 {
                    return None;
                }
                let tr = special_mut(&mut sub, speed, ti as usize);
                for c in t.children(cell_tag) {
                    if let Some(x) = usize::try_from(c.int("aks:index")).ok().and_then(|i| tr.get_mut(i)) {
                        *x = Some(c.int("aks:value"));
                    }
                }
            }
        }
        if let Some(patterns) = s.child("aks:patterns") {
            for p in patterns.children("aks:pattern") {
                let cells: Vec<&Node> = p.children("aks:patternCell").collect();
                let tracks: Vec<usize> = cells.iter().map(|c| c.int("aks:trackNumber").max(0) as usize).collect();
                let trans: Vec<i32> = cells.iter().map(|c| c.int("aks:transposition")).collect();
                let pattern = sub.patterns.len();
                sub.patterns.push(Pattern { tracks, speed_track: p.int("aks:speedTrackNumber").max(0) as usize, event_track: p.int("aks:eventTrackNumber").max(0) as usize });
                sub.positions.push(Position { pattern, height: p.int("aks:height").clamp(1, TRACK_ROWS as i32) as usize, transpositions: trans });
            }
        }
        arkos::finish_subsong(&mut sub);
        subsongs.push(sub);
    }
    if subsongs.is_empty() {
        return None;
    }
    Some(Song::from_parts(root.string("aks:title"), root.string("aks:author"), root.string("aks:composer"), root.string("aks:comment"), "aks2", subsongs, b, file))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml() {
        let n = parse_xml(br#"<?xml version="1.0"?><!-- c --><a x="1 &amp; 2"><b>4&lt;5</b><c/><b> 7 </b></a>"#).unwrap();
        assert_eq!(n.name, "a");
        assert_eq!(n.attrs, vec![("x".to_string(), "1 & 2".to_string())]);
        assert_eq!(n.string("b"), "4<5");
        assert_eq!(n.children("b").count(), 2);
        assert_eq!(int_value(" -12x"), -12);
        assert_eq!(hex_value("#f00"), Some(0xF00));
        assert_eq!(logical(Effect::Volume, 0xF00), 0xF);
        assert_eq!(logical(Effect::ArpeggioTable, 0x120), 0x12);
        assert_eq!(base64("aGVsbG8="), b"hello");
    }
}

