// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! Arkos Tracker 3's own songs (.aks, "song" format 3.0), read into
//! `arkos::Song` as Arkos Tracker 3 reads them (MIT, see `arkos.rs`).
//! Its SID effects (the Atari ST's timers) aren't played: those cells
//! sound as their plain PSG part.

use std::sync::Arc;

use crate::arkos::{self, Expression, Instrument, InstrumentCell, Loop, Pattern, Position, Psg, Sample, Section, Song, XmlBuilt, TRACK_ROWS};
use crate::arkos_xml::{base64, effect_from_name, int_value, set_effect, special_mut, track_mut, Node, DIGI_NOTE};

fn link_from_name(s: &str) -> Option<arkos::Link> {
    use arkos::Link::*;
    Some(match s {
        "softwareOnly" => SoftOnly,
        "hardwareOnly" => HardOnly,
        "hardwareToSoftware" => HardToSoft,
        "softwareToHardware" => SoftToHard,
        "softwareAndHardware" => SoftAndHard,
        "noSoftwareNoHardware" => NoSoftNoHard,
        _ => return None,
    })
}

fn instrument(n: &Node) -> Option<Instrument> {
    let name = n.string("name");
    match n.string("type").as_str() {
        "psg" => {
            let mut ins = Instrument {
                name,
                speed: n.int("speed"),
                retrig: n.bool("isRetrig"),
                main_loop: Loop { start: n.int("loopStartIndex"), end: n.int("endIndex"), looping: n.bool("isLooping") },
                ..Default::default()
            };
            for a in n.children("autoSpread") {
                if let Some(section) = Section::from_name(&a.string("psgSection")) {
                    ins.spread.push((section, Loop { start: a.int("loopStartIndex"), end: a.int("endIndex"), looping: a.bool("isLooping") }));
                }
            }
            for c in n.child("cells")?.children("cell") {
                let link = link_from_name(&c.string("link"))?;
                ins.cells.push(InstrumentCell {
                    link,
                    volume: c.int("volume"),
                    noise: c.int("noise"),
                    primary_period: c.int("primaryPeriod"),
                    primary_arpeggio: c.int("primaryArpeggioNoteInOctave") + c.int("primaryArpeggioOctave") * 12,
                    primary_pitch: c.int("primaryPitch"),
                    ratio: c.int_or("ratio", 4),
                    envelope: c.int_or("hardwareEnvelope", 8),
                    secondary_period: c.int("secondaryPeriod"),
                    secondary_arpeggio: c.int("secondaryArpeggioNoteInOctave") + c.int("secondaryArpeggioOctave") * 12,
                    secondary_pitch: c.int("secondaryPitch"),
                    retrig: c.bool("isRetrig"),
                });
            }
            Some(ins)
        }
        "sample" => {
            let (start, end) = (n.int_or("loopStartIndex", -1), n.int_or("endIndex", -1));
            if start < 0 || end < 0 || start > end {
                return None;
            }
            let sample = Sample {
                data: Arc::new(base64(&n.string("sampleUnsigned8BitsBase64"))),
                sample_loop: Loop { start, end, looping: n.bool("isLooping") },
                amplification: n.float_or("amplificationRatio", 1.0) as f32,
                digi_note: n.int_or("digiNote", DIGI_NOTE),
            };
            Some(Instrument { name, sample: Some(sample), ..Default::default() })
        }
        _ => None,
    }
}

/// An expression with its shift (leading zeros) laid into its values.
fn expression(n: &Node) -> Option<Expression> {
    let shift = n.int("shift").clamp(0, 255);
    let mut values = vec![0; shift as usize];
    values.extend(n.child("values")?.children("value").map(|v| int_value(&v.all_text())));
    Some(Expression { values, loop_start: n.int("loopStartIndex") + shift, end: n.int("endIndex") + shift, speed: n.int("speed") })
}

/// A pattern's track (or special track) index: the linked track when there is one.
fn used_index(n: Option<&Node>) -> usize {
    let Some(n) = n else { return 0 };
    let linked = n.int_or("linkedTrackIndex", -1);
    (if linked >= 0 { linked } else { n.int("trackIndex") }).max(0) as usize
}

pub(crate) fn parse(root: &Node, file: &[u8]) -> Option<Song> {
    let mut b = XmlBuilt::new();
    let instruments: Vec<Instrument> = root.child("instruments")?.children("instrument").map(instrument).collect::<Option<_>>()?;
    let arpeggios: Vec<Expression> = root.child("arpeggios").map(|a| a.children("expression").filter_map(expression).collect()).unwrap_or_default();
    let pitches: Vec<Expression> = root.child("pitches").map(|a| a.children("expression").filter_map(expression).collect()).unwrap_or_default();
    b.replace(instruments, arpeggios, pitches);
    let mut subsongs = Vec::new();
    for s in root.child("subsongs")?.children("subsong") {
        let mut sub = arkos::new_subsong(&s.string("title"));
        sub.initial_speed = s.int("initialSpeed");
        sub.digi_channel = s.int("digiChannel").max(0) as usize;
        sub.loop_start = s.int("loopStartPosition").max(0) as usize;
        sub.end = s.int("endPosition").max(0) as usize;
        sub.replay_hz = s.float_or("replayFrequencyHz", 50.0);
        sub.psgs = s
            .child("psgs")?
            .children("psg")
            .map(|p| {
                let hz = Some(p.int_or("frequencyHz", -1)).filter(|f| (5000..=8_000_000).contains(f)).unwrap_or(arkos::CPC_HZ);
                let reference = Some(p.float_or("referenceFrequencyHz", -1.0)).filter(|f| (100.0..=1000.0).contains(f)).unwrap_or(440.0);
                let sample_hz = Some(p.int("samplePlayerFrequencyHz")).filter(|f| (1000..=44100).contains(f)).unwrap_or(8000);
                Psg { hz, reference, ym: p.string("type") == "ym", sample_hz, pan: Psg::pan_from(&p.string("mixingOutput")) }
            })
            .collect();
        if sub.psgs.is_empty() {
            return None;
        }
        if let Some(tracks) = s.child("tracks") {
            for t in tracks.children("track") {
                let ti = t.int_or("index", -1);
                if ti < 0 {
                    return None;
                }
                let tr = track_mut(&mut sub, ti as usize);
                for c in t.children("cell") {
                    let Some(cell) = usize::try_from(c.int_or("index", -1)).ok().and_then(|i| tr.get_mut(i)) else { continue };
                    let note = c.int_or("note", -1);
                    if note >= 0 {
                        cell.note = Some(note.min(255) as u8);
                    }
                    let ins = c.int_or("instrument", -1);
                    if ins >= 0 {
                        cell.instrument = Some(ins.min(255) as u8);
                    }
                    for e in c.children("effect") {
                        let Some(effect) = effect_from_name(&e.string("name")) else { continue };
                        let index = e.int_or("index", -1);
                        if index >= 0 {
                            set_effect(cell, index as usize, effect, e.int("logicalValue"));
                        }
                    }
                }
            }
        }
        for (speed, list, item) in [(true, "speedTracks", "speedTrack"), (false, "eventTracks", "eventTrack")] {
            let Some(list) = s.child(list) else { continue };
            for t in list.children(item) {
                let ti = t.int_or("index", -1);
                if ti < 0 {
                    return None;
                }
                let tr = special_mut(&mut sub, speed, ti as usize);
                for c in t.children("cell") {
                    if let Some(x) = usize::try_from(c.int_or("index", -1)).ok().and_then(|i| tr.get_mut(i)) {
                        *x = Some(c.int("value"));
                    }
                }
            }
        }
        for p in s.child("positions")?.children("position") {
            let mut trans = vec![0; sub.psgs.len() * 3];
            if let Some(ts) = p.child("transpositions") {
                for t in ts.children("transposition") {
                    if let Some(x) = usize::try_from(t.int_or("channel", -1)).ok().and_then(|c| trans.get_mut(c)) {
                        *x = t.int("value");
                    }
                }
            }
            sub.positions.push(Position { pattern: p.int("patternIndex").max(0) as usize, height: p.int("height").clamp(1, TRACK_ROWS as i32) as usize, transpositions: trans });
        }
        for p in s.child("patterns")?.children("pattern") {
            let tracks = p.children("trackIndexes").map(|t| used_index(Some(t))).collect();
            sub.patterns.push(Pattern { tracks, speed_track: used_index(p.child("speedTrackIndex")), event_track: used_index(p.child("eventTrackIndex")) });
        }
        arkos::finish_subsong(&mut sub);
        subsongs.push(sub);
    }
    if subsongs.is_empty() {
        return None;
    }
    Some(Song::from_parts(root.string("title"), root.string("author"), root.string("composer"), root.string("comment"), "aks3", subsongs, b, file))
}
