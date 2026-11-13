// Copyright 2026 ansiapps. Neumetik MIDI, MIT licensed: see LICENSE.

//! **MusicXML** (W3C Music Notation Community Group, versions 1.0 to
//! 4.0), read: `.musicxml` and `.xml`, and `.mxl`, the zip holding one.
//! Part-wise and time-wise scores; each part's instrument from its
//! `<midi-instrument>` (else its name), percussion by `<midi-unpitched>`;
//! chords, voices (`<backup>`, `<forward>`), ties, tuplets (by their
//! durations), transposing instruments sounding as written for them;
//! repeats and endings played out; tempo from `<sound tempo>` or a
//! metronome mark; dynamics from `<sound dynamics>`, a note's own
//! `dynamics`, or the marks (`p`, `mf`, `ff`...) as velocity; `<harmony>`
//! as chord symbols; the first verse's lyrics. Grace notes and cue notes
//! aren't played (a cue note keeps its time, silent), as the standard says.
//!
//! Written ([`write`]) from [`super::notation`]: a part-wise 4.0 score,
//! each part's instrument, voices, ties, tuplets, keys spelled by the key
//! signature, tempo, velocity as each note's `dynamics`, chord symbols as
//! `<harmony>`, lyrics, and markers as rehearsal marks.

use super::xml::{self, Element};
use super::{bad, Builder, Frac};
use crate::score::Score;
use crate::Error;

/// A compressed MusicXML file's score text: the root file its
/// `META-INF/container.xml` names, else its first `.musicxml` or `.xml`.
pub fn unzip(bytes: &[u8]) -> Result<String, Error> {
    let files = super::inflate::unzip(bytes, 64 << 20)?;
    let text = |b: &[u8]| String::from_utf8_lossy(b).trim_start_matches('\u{feff}').to_string();
    let root = files.iter().find(|f| f.0 == "META-INF/container.xml").and_then(|c| {
        let doc = xml::parse(&text(&c.1)).ok()?;
        let mut path = None;
        doc.walk(&mut |e| {
            if e.name == "rootfile" && path.is_none() {
                path = e.attr("full-path").map(|s| s.to_string());
            }
        });
        path
    });
    let file = match root {
        Some(p) => files.iter().find(|f| f.0 == p),
        None => files.iter().find(|f| !f.0.starts_with("META-INF") && (f.0.ends_with(".musicxml") || f.0.ends_with(".xml"))),
    };
    file.map(|f| text(&f.1)).ok_or(Error::BadText { line: 1, column: 1, why: "the compressed file holds no MusicXML score" })
}

/// One `<midi-instrument>`: channel (0-based), program, unpitched key.
#[derive(Debug, Clone, Default)]
struct Instrument {
    id: String,
    channel: Option<u8>,
    program: Option<u8>,
    unpitched: Option<u8>,
    volume: Option<f64>,
}

struct PartInfo {
    id: String,
    name: String,
    instruments: Vec<Instrument>,
}

const DYNAMICS: [(&str, f64); 12] = [("pppppp", 10.0), ("ppppp", 15.0), ("pppp", 20.0), ("ppp", 30.0), ("pp", 45.0), ("p", 60.0), ("mp", 75.0), ("mf", 90.0), ("f", 105.0), ("ff", 120.0), ("fff", 127.0), ("ffff", 127.0)];

/// A MusicXML score as a score and its title.
pub fn read(text: &str) -> Result<(Score, String), Error> {
    let doc = xml::parse(text)?;
    let timewise = match doc.name.as_str() {
        "score-partwise" => false,
        "score-timewise" => true,
        _ => return Err(bad(text, doc.offset, "a MusicXML score is a <score-partwise> or <score-timewise>")),
    };
    let title = doc
        .child("work")
        .and_then(|w| w.get("work-title"))
        .filter(|t| !t.is_empty())
        .or_else(|| doc.get("movement-title"))
        .unwrap_or_default();
    // The part list: names and instruments.
    let mut infos: Vec<PartInfo> = vec![];
    if let Some(list) = doc.child("part-list") {
        for sp in list.all("score-part") {
            let mut instruments = vec![];
            for mi in sp.all("midi-instrument") {
                instruments.push(Instrument {
                    id: mi.attr("id").unwrap_or("").to_string(),
                    channel: mi.num("midi-channel").map(|c| (c as i64 - 1).clamp(0, 15) as u8),
                    program: mi.num("midi-program").map(|p| (p as i64 - 1).clamp(0, 127) as u8),
                    unpitched: mi.num("midi-unpitched").map(|p| (p as i64 - 1).clamp(0, 127) as u8),
                    volume: mi.num("volume"),
                });
            }
            let name = sp.get("part-name").filter(|n| !n.is_empty()).or_else(|| sp.child("score-instrument").and_then(|s| s.get("instrument-name"))).unwrap_or_default();
            infos.push(PartInfo { id: sp.attr("id").unwrap_or("").to_string(), name, instruments });
        }
    }
    // Each part's measures, as the elements holding their music.
    let mut parts: Vec<(String, Vec<&Element>)> = vec![];
    if timewise {
        for m in doc.all("measure") {
            for p in m.all("part") {
                let id = p.attr("id").unwrap_or("").to_string();
                match parts.iter_mut().find(|x| x.0 == id) {
                    Some(x) => x.1.push(p),
                    None => parts.push((id, vec![p])),
                }
            }
        }
    } else {
        for p in doc.all("part") {
            parts.push((p.attr("id").unwrap_or("").to_string(), p.all("measure").collect()));
        }
    }
    if parts.is_empty() {
        return Err(bad(text, doc.offset, "the score has no parts"));
    }
    let count = parts.iter().map(|p| p.1.len()).max().unwrap_or(0);
    // Each measure's length (the longest any part takes), in whole notes.
    let mut lengths = vec![Frac::ZERO; count];
    for (_, measures) in &parts {
        let mut divisions = 1i64;
        for (i, m) in measures.iter().enumerate() {
            let mut cursor = 0i64;
            let mut most = 0i64;
            let mut last = 0i64;
            for e in m.elements() {
                match e.name.as_str() {
                    "attributes" => {
                        if let Some(d) = e.num("divisions") {
                            divisions = (d as i64).max(1);
                        }
                    }
                    "note" => {
                        // Grace notes take no time; cue notes take theirs, silently.
                        if e.child("grace").is_some() {
                            continue;
                        }
                        let d = e.num("duration").unwrap_or(0.0) as i64;
                        if e.child("chord").is_some() {
                            most = most.max(last + d);
                            continue;
                        }
                        last = cursor;
                        cursor += d;
                    }
                    "backup" => cursor -= e.num("duration").unwrap_or(0.0) as i64,
                    "forward" => cursor += e.num("duration").unwrap_or(0.0) as i64,
                    _ => {}
                }
                most = most.max(cursor);
            }
            lengths[i] = lengths[i].max(Frac::new(most, divisions * 4));
        }
    }
    // The order measures play in, from the first part's repeats and endings.
    let order = play_order(&parts[0].1);
    let mut starts = vec![];
    let mut now = Frac::ZERO;
    for &i in &order {
        starts.push(now);
        now = now.add(lengths[i]);
    }
    let mut b = Builder::new();
    b.title = title.clone();
    b.tempo(Frac::ZERO, 120.0);
    b.end_at(now);
    // `<tie>` is what sounds, `<tied>` what's drawn: a file with any
    // `<tie>` is played by those alone.
    let mut sounding_ties = false;
    doc.walk(&mut |e| sounding_ties |= e.name == "tie");
    for (n, (id, measures)) in parts.iter().enumerate() {
        let info = infos.iter().find(|i| &i.id == id);
        part(&mut b, info, measures, &order, &starts, n == 0, sounding_ties);
    }
    Ok((b.finish(), title))
}

/// The measures in the order they're played: repeats (`<repeat>` in a
/// `<barline>`, `times` or twice) and endings.
fn play_order(measures: &[&Element]) -> Vec<usize> {
    // Each measure's repeats and ending numbers.
    let mut forward = vec![false; measures.len()];
    let mut backward = vec![0u32; measures.len()];
    let mut endings: Vec<Vec<u32>> = vec![vec![]; measures.len()];
    let mut current: Vec<u32> = vec![];
    for (i, m) in measures.iter().enumerate() {
        let mut stop_after = false;
        for bl in m.all("barline") {
            if let Some(r) = bl.child("repeat") {
                match r.attr("direction") {
                    Some("forward") => forward[i] = true,
                    Some("backward") => backward[i] = r.attr("times").and_then(|t| t.parse().ok()).unwrap_or(2).clamp(2, 16),
                    _ => {}
                }
            }
            if let Some(e) = bl.child("ending") {
                let numbers: Vec<u32> = e.attr("number").unwrap_or("").split([',', ' ']).filter_map(|n| n.trim().parse().ok()).collect();
                match e.attr("type") {
                    Some("start") => current = numbers,
                    Some("stop") | Some("discontinue") => stop_after = true,
                    _ => {}
                }
            }
        }
        endings[i] = current.clone();
        if stop_after {
            current.clear();
        }
    }
    let mut order = vec![];
    let (mut i, mut start, mut pass) = (0usize, 0usize, 1u32);
    let mut in_ending = false;
    let mut jumped = false;
    while i < measures.len() && order.len() < 1 << 16 {
        if forward[i] && !jumped {
            start = i;
            pass = 1;
        }
        jumped = false;
        if !endings[i].is_empty() {
            if !endings[i].contains(&pass) {
                i += 1;
                continue;
            }
            in_ending = true;
        } else if in_ending {
            in_ending = false;
            pass = 1;
        }
        order.push(i);
        if backward[i] > 0 {
            if pass < backward[i] {
                pass += 1;
                i = start;
                jumped = true;
                in_ending = false;
                continue;
            }
            pass = 1;
            start = i + 1;
        }
        i += 1;
    }
    order
}

fn dynamics_of(e: &Element) -> Option<f64> {
    e.elements().find_map(|d| DYNAMICS.iter().find(|x| x.0 == d.name).map(|x| x.1))
}

/// One part's notes into the builder.
fn part(b: &mut Builder, info: Option<&PartInfo>, measures: &[&Element], order: &[usize], starts: &[Frac], first: bool, sounding_ties: bool) {
    let name = info.map(|i| i.name.clone()).unwrap_or_default();
    let instruments = info.map(|i| i.instruments.clone()).unwrap_or_default();
    let main = instruments.first().cloned().unwrap_or_default();
    let drums = main.channel == Some(9) || instruments.iter().any(|i| i.unpitched.is_some()) && main.program.is_none();
    let program = main.program.or_else(|| super::program(&name)).unwrap_or(0);
    let index = b.part(&name, program, drums);
    if let Some(c) = main.channel.filter(|&c| c != 9) {
        b.parts[index].channel = Some(c);
    }
    if let Some(v) = main.volume {
        b.control(index, Frac::ZERO, 7, (v * 127.0 / 100.0).round().clamp(0.0, 127.0) as u8);
    }
    let mut divisions = 1i64;
    let mut transpose = 0i32;
    let mut velocity = 80.0f64;
    // Tied notes: key, voice, note index.
    let mut open: Vec<(i32, String, usize)> = vec![];
    for (k, &mi) in order.iter().enumerate() {
        let Some(m) = measures.get(mi) else { continue };
        let start = starts[k];
        let mut cursor = 0i64;
        let mut last_start = 0i64;
        let mut chord_voice = String::new();
        let at = |c: i64, d: i64| start.add(Frac::new(c, d * 4));
        for e in m.elements() {
            match e.name.as_str() {
                "attributes" => {
                    if let Some(d) = e.num("divisions") {
                        divisions = (d as i64).max(1);
                    }
                    if let Some(t) = e.child("transpose") {
                        transpose = t.num("chromatic").unwrap_or(0.0) as i32 + 12 * t.num("octave-change").unwrap_or(0.0) as i32;
                    }
                    if first {
                        if let Some(time) = e.child("time") {
                            let beats: u32 = time.get("beats").unwrap_or_default().split('+').filter_map(|x| x.trim().parse::<u32>().ok()).sum();
                            let unit = time.num("beat-type").unwrap_or(4.0) as u32;
                            if beats > 0 && beats < 256 && unit.is_power_of_two() {
                                b.meter(at(cursor, divisions), beats as u8, unit as u8);
                            }
                        }
                        if let Some(key) = e.child("key") {
                            if let Some(f) = key.num("fifths") {
                                b.key(at(cursor, divisions), f as i8, key.get("mode").as_deref() == Some("minor"));
                            }
                        }
                    }
                }
                "direction" | "sound" => {
                    let sound = if e.name == "sound" { Some(e) } else { e.child("sound") };
                    if let Some(s) = sound {
                        if let Some(t) = s.attr("tempo").and_then(|t| t.parse::<f64>().ok()) {
                            b.tempo(at(cursor, divisions), t);
                        }
                        if let Some(d) = s.attr("dynamics").and_then(|t| t.parse::<f64>().ok()) {
                            velocity = d * 0.9;
                        }
                    }
                    if e.name == "direction" {
                        for dt in e.all("direction-type") {
                            if let Some(d) = dt.child("dynamics").and_then(dynamics_of) {
                                if sound.and_then(|s| s.attr("dynamics")).is_none() {
                                    velocity = d;
                                }
                            }
                            if let Some(mm) = dt.child("metronome") {
                                if sound.and_then(|s| s.attr("tempo")).is_none() {
                                    if let (Some(unit), Some(pm)) = (mm.get("beat-unit"), mm.num("per-minute")) {
                                        let mut q = match unit.as_str() {
                                            "whole" => 4.0,
                                            "half" => 2.0,
                                            "quarter" => 1.0,
                                            "eighth" => 0.5,
                                            "16th" => 0.25,
                                            _ => 1.0,
                                        };
                                        if mm.child("beat-unit-dot").is_some() {
                                            q *= 1.5;
                                        }
                                        if first {
                                            b.tempo(at(cursor, divisions), pm * q);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                "harmony" => {
                    if let Some(sym) = harmony(e) {
                        let off = e.num("offset").unwrap_or(0.0) as i64;
                        b.chord(at(cursor + off, divisions), &sym);
                    }
                }
                "backup" => cursor = (cursor - e.num("duration").unwrap_or(0.0) as i64).max(0),
                "forward" => cursor += e.num("duration").unwrap_or(0.0) as i64,
                "note" => {
                    if e.child("grace").is_some() {
                        continue;
                    }
                    let dur = e.num("duration").unwrap_or(0.0) as i64;
                    let chord = e.child("chord").is_some();
                    let onset = if chord { last_start } else { cursor };
                    if !chord {
                        last_start = cursor;
                        cursor += dur;
                    }
                    if e.child("rest").is_some() || e.child("cue").is_some() {
                        continue;
                    }
                    let inst = e.child("instrument").and_then(|i| i.attr("id")).and_then(|id| instruments.iter().find(|x| x.id == id));
                    let key = if let Some(p) = e.child("pitch") {
                        let step = p.get("step").unwrap_or_default();
                        let Some(base) = step.chars().next().and_then(super::letter_key) else { continue };
                        let alter = p.num("alter").unwrap_or(0.0).round() as i32;
                        let octave = p.num("octave").unwrap_or(4.0) as i32;
                        base + 12 * (octave - 4) + alter + transpose
                    } else if let Some(u) = e.child("unpitched") {
                        match inst.or(instruments.first()).and_then(|i| i.unpitched) {
                            Some(k) => i32::from(k),
                            None => {
                                let step = u.get("display-step").unwrap_or_default();
                                let Some(base) = step.chars().next().and_then(super::letter_key) else { continue };
                                base + 12 * (u.num("display-octave").unwrap_or(4.0) as i32 - 4)
                            }
                        }
                    } else {
                        continue;
                    };
                    let mut vel = velocity;
                    if let Some(d) = e.attr("dynamics").and_then(|d| d.parse::<f64>().ok()) {
                        vel = d * 0.9;
                    }
                    let vel = vel.round().clamp(1.0, 127.0) as u8;
                    // A chord's note without a voice is its chord's.
                    let voice = match e.get("voice") {
                        Some(v) => v,
                        None if chord => chord_voice.clone(),
                        None => String::new(),
                    };
                    chord_voice = voice.clone();
                    let ties: Vec<&str> = if sounding_ties {
                        e.all("tie").filter_map(|t| t.attr("type")).collect()
                    } else {
                        e.child("notations").into_iter().flat_map(|n| n.all("tied").filter_map(|t| t.attr("type"))).collect()
                    };
                    let (starts_tie, stops_tie) = (ties.contains(&"start"), ties.contains(&"stop"));
                    let from = at(onset, divisions);
                    let len = Frac::new(dur, divisions * 4);
                    if stops_tie {
                        if let Some(pos) = open.iter().rposition(|o| o.0 == key && o.1 == voice).or_else(|| if voice.is_empty() { open.iter().rposition(|o| o.0 == key) } else { None }) {
                            let (_, _, idx) = open.remove(pos);
                            let n = &mut b.parts[index].notes[idx];
                            n.1 = from.add(len).sub(n.0);
                            if starts_tie {
                                open.push((key, voice, idx));
                            }
                            continue;
                        }
                    }
                    let before = b.parts[index].notes.len();
                    b.note(index, from, len, key, vel);
                    if starts_tie && b.parts[index].notes.len() > before {
                        open.push((key, voice.clone(), before));
                    }
                    if let Some(l) = e.child("lyric").filter(|l| l.attr("number").map_or(true, |n| n == "1")) {
                        if let Some(t) = l.get("text") {
                            let hyphen = matches!(l.get("syllabic").as_deref(), Some("begin" | "middle"));
                            b.lyric(index, from, &if hyphen { format!("{t}-") } else { t });
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

/// A `<harmony>`'s chord symbol: its root, the kind's text (or a short
/// name for the kind), and a bass.
fn harmony(e: &Element) -> Option<String> {
    let root = e.child("root")?;
    let alter = |x: Option<f64>| match x.map(|a| a.round() as i32) {
        Some(1) => "#",
        Some(-1) => "b",
        Some(2) => "##",
        Some(-2) => "bb",
        _ => "",
    };
    let mut s = format!("{}{}", root.get("root-step")?, alter(root.num("root-alter")));
    if let Some(kind) = e.child("kind") {
        let text = kind.attr("text").map(|t| t.to_string()).unwrap_or_else(|| {
            match kind.text().as_str() {
                "major" => "",
                "minor" => "m",
                "augmented" => "aug",
                "diminished" => "dim",
                "dominant" => "7",
                "major-seventh" => "maj7",
                "minor-seventh" => "m7",
                "diminished-seventh" => "dim7",
                "augmented-seventh" => "aug7",
                "half-diminished" => "m7b5",
                "major-minor" => "m(maj7)",
                "major-sixth" => "6",
                "minor-sixth" => "m6",
                "dominant-ninth" => "9",
                "major-ninth" => "maj9",
                "minor-ninth" => "m9",
                "dominant-11th" => "11",
                "dominant-13th" => "13",
                "suspended-second" => "sus2",
                "suspended-fourth" => "sus4",
                "power" => "5",
                _ => "",
            }
            .to_string()
        });
        s.push_str(&text);
    }
    if let Some(bass) = e.child("bass") {
        if let Some(step) = bass.get("bass-step") {
            s.push('/');
            s.push_str(&step);
            s.push_str(alter(bass.num("bass-alter")));
        }
    }
    Some(s)
}

/// Text safe in an XML element or attribute.
fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// A score as MusicXML 4.0 (part-wise), titled.
pub fn write(score: &Score, title: &str) -> String {
    use super::notation::{notate, spell, type_name};
    let n = notate(score);
    // Divisions a quarter note that every length divides into.
    let mut divisions = 1i64;
    for it in n.parts.iter().flat_map(|p| p.voices.iter().flatten().flatten()) {
        let q = it.len.mul(Frac::int(4));
        divisions = divisions / gcd(divisions, q.den()) * q.den();
        if divisions > 1 << 24 {
            divisions = 1 << 24;
            break;
        }
    }
    for b in &n.bars {
        let q = b.len.mul(Frac::int(4));
        divisions = (divisions / gcd(divisions, q.den()) * q.den()).min(1 << 24);
    }
    let dur = |f: Frac| f.mul(Frac::int(4 * divisions)).f64().round() as i64;
    let mut o = String::new();
    o.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"no\"?>\n");
    o.push_str("<!DOCTYPE score-partwise PUBLIC \"-//Recordare//DTD MusicXML 4.0 Partwise//EN\" \"http://www.musicxml.org/dtds/partwise.dtd\">\n");
    o.push_str("<score-partwise version=\"4.0\">\n");
    if !title.is_empty() {
        o.push_str(&format!("  <work><work-title>{}</work-title></work>\n", esc(title)));
    }
    o.push_str("  <identification><encoding><software>Neumetik</software></encoding></identification>\n  <part-list>\n");
    for (i, p) in n.parts.iter().enumerate() {
        let id = i + 1;
        let name = if p.name.is_empty() { if p.drums { "Drums".to_string() } else { super::GM_NAMES[usize::from(p.program & 127)].to_string() } } else { p.name.clone() };
        o.push_str(&format!("    <score-part id=\"P{id}\">\n      <part-name>{}</part-name>\n", esc(&name)));
        o.push_str(&format!("      <score-instrument id=\"P{id}-I1\"><instrument-name>{}</instrument-name></score-instrument>\n", esc(&name)));
        o.push_str(&format!("      <midi-instrument id=\"P{id}-I1\"><midi-channel>{}</midi-channel>", p.channel + 1));
        o.push_str(&format!("<midi-program>{}</midi-program>", p.program + 1));
        o.push_str("</midi-instrument>\n    </score-part>\n");
    }
    o.push_str("  </part-list>\n");
    for (pi, p) in n.parts.iter().enumerate() {
        o.push_str(&format!("  <part id=\"P{}\">\n", pi + 1));
        for (bi, bar) in n.bars.iter().enumerate() {
            o.push_str(&format!("    <measure number=\"{}\">\n", bi + 1));
            let prev = bi.checked_sub(1).map(|i| &n.bars[i]);
            let key_changed = prev.map_or(true, |b| (b.sharps, b.minor) != (bar.sharps, bar.minor));
            let time_changed = prev.map_or(true, |b| (b.beats, b.unit, b.len) != (bar.beats, bar.unit, bar.len));
            if bi == 0 || key_changed || time_changed {
                o.push_str("      <attributes>\n");
                if bi == 0 {
                    o.push_str(&format!("        <divisions>{divisions}</divisions>\n"));
                }
                if key_changed {
                    o.push_str(&format!("        <key><fifths>{}</fifths><mode>{}</mode></key>\n", bar.sharps, if bar.minor { "minor" } else { "major" }));
                }
                if time_changed {
                    // A short bar (a pickup, or cut by a meter change) is
                    // written in the meter it's cut from; its notes say how long it is.
                    o.push_str(&format!("        <time><beats>{}</beats><beat-type>{}</beat-type></time>\n", bar.beats, bar.unit));
                }
                if bi == 0 {
                    let clef = if p.drums { "<sign>percussion</sign>" } else if p.middle() < 57 { "<sign>F</sign><line>4</line>" } else { "<sign>G</sign><line>2</line>" };
                    o.push_str(&format!("        <clef>{clef}</clef>\n"));
                }
                o.push_str("      </attributes>\n");
            }
            if pi == 0 {
                if let Some(t) = bar.tempo {
                    let shown = (t * 100.0).round() / 100.0;
                    o.push_str(&format!("      <direction placement=\"above\"><direction-type><metronome><beat-unit>quarter</beat-unit><per-minute>{shown}</per-minute></metronome></direction-type><sound tempo=\"{shown}\"/></direction>\n"));
                }
                let end = bar.start.add(bar.len);
                for (t, text) in n.markers.iter().filter(|m| m.0 >= bar.start && m.0 < end) {
                    o.push_str(&format!("      <direction placement=\"above\"><direction-type><rehearsal>{}</rehearsal></direction-type>", esc(text)));
                    if *t > bar.start {
                        o.push_str(&format!("<offset>{}</offset>", dur(t.sub(bar.start))));
                    }
                    o.push_str("</direction>\n");
                }
            }
            // Chord symbols go with the first part, before the voice that's
            // at their time.
            let mut chords: Vec<(Frac, &str)> = if pi == 0 { n.chords.iter().filter(|c| c.0 >= bar.start && c.0 < bar.start.add(bar.len)).map(|c| (c.0.sub(bar.start), c.1.as_str())).collect() } else { vec![] };
            let voices: Vec<usize> = (0..p.voices.len()).filter(|&v| v == 0 || p.voices[v][bi].iter().any(|i| !i.rest())).collect();
            for (k, &v) in voices.iter().enumerate() {
                if k > 0 {
                    o.push_str(&format!("      <backup><duration>{}</duration></backup>\n", dur(bar.len)));
                }
                // Accidentals shown: by letter and octave, as the bar has them so far.
                let sig = super::signature_accidentals(bar.sharps);
                let mut shown: Vec<((u8, i8), i8)> = vec![];
                for it in &p.voices[v][bi] {
                    if v == voices[0] {
                        while let Some(i) = chords.iter().position(|c| c.0 <= it.at) {
                            let (_, sym) = chords.remove(i);
                            o.push_str(&harmony_xml(sym));
                        }
                    }
                    let keys: Vec<Option<u8>> = if it.rest() { vec![None] } else { it.keys.iter().map(|&k| Some(k)).collect() };
                    for (j, key) in keys.iter().enumerate() {
                        if it.rest() {
                            o.push_str("      <note>");
                        } else {
                            o.push_str(&format!("      <note dynamics=\"{:.2}\">", f64::from(it.velocities.get(j).copied().unwrap_or(it.velocity)) / 0.9));
                        }
                        if j > 0 {
                            o.push_str("<chord/>");
                        }
                        let mut accidental = None;
                        match key {
                            None => o.push_str(if it.len == bar.len && it.at == Frac::ZERO { "<rest measure=\"yes\"/>" } else { "<rest/>" }),
                            Some(k) => {
                                let sp = spell(*k, bar.sharps);
                                let expected = shown.iter().find(|s| s.0 == (sp.step, sp.octave)).map(|s| s.1).unwrap_or(sig[usize::from(sp.step)] as i8);
                                if sp.alter != expected && !it.tie_from {
                                    accidental = Some(match sp.alter {
                                        2 => "double-sharp",
                                        1 => "sharp",
                                        -1 => "flat",
                                        -2 => "flat-flat",
                                        _ => "natural",
                                    });
                                }
                                shown.retain(|s| s.0 != (sp.step, sp.octave));
                                shown.push(((sp.step, sp.octave), sp.alter));
                                o.push_str(&format!("<pitch><step>{}</step>", sp.letter()));
                                if sp.alter != 0 {
                                    o.push_str(&format!("<alter>{}</alter>", sp.alter));
                                }
                                o.push_str(&format!("<octave>{}</octave></pitch>", sp.octave));
                            }
                        }
                        o.push_str(&format!("<duration>{}</duration>", dur(it.len)));
                        if it.tie_from {
                            o.push_str("<tie type=\"stop\"/>");
                        }
                        if it.tie_to {
                            o.push_str("<tie type=\"start\"/>");
                        }
                        o.push_str(&format!("<voice>{}</voice>", v + 1));
                        if !(it.rest() && it.len == bar.len && it.at == Frac::ZERO) {
                            o.push_str(&format!("<type>{}</type>", type_name(it.base)));
                            for _ in 0..it.dots {
                                o.push_str("<dot/>");
                            }
                        }
                        if let Some(a) = accidental {
                            o.push_str(&format!("<accidental>{a}</accidental>"));
                        }
                        if let Some(t) = it.tuplet {
                            o.push_str(&format!("<time-modification><actual-notes>{}</actual-notes><normal-notes>{}</normal-notes></time-modification>", t.actual, t.normal));
                        }
                        let mut notations = String::new();
                        if it.tie_from {
                            notations.push_str("<tied type=\"stop\"/>");
                        }
                        if it.tie_to {
                            notations.push_str("<tied type=\"start\"/>");
                        }
                        if let Some(t) = it.tuplet.filter(|_| j == 0) {
                            if t.first {
                                notations.push_str("<tuplet type=\"start\" bracket=\"yes\"/>");
                            }
                            if t.last {
                                notations.push_str("<tuplet type=\"stop\"/>");
                            }
                        }
                        if !notations.is_empty() {
                            o.push_str(&format!("<notations>{notations}</notations>"));
                        }
                        if let (0, Some(l)) = (j, &it.lyric) {
                            let (text, syllabic) = match l.strip_suffix('-') {
                                Some(t) => (t, "begin"),
                                None => (l.as_str(), "single"),
                            };
                            o.push_str(&format!("<lyric number=\"1\"><syllabic>{syllabic}</syllabic><text>{}</text></lyric>", esc(text)));
                        }
                        o.push_str("</note>\n");
                    }
                }
            }
            if bi + 1 == n.bars.len() {
                o.push_str("      <barline location=\"right\"><bar-style>light-heavy</bar-style></barline>\n");
            }
            o.push_str("    </measure>\n");
        }
        o.push_str("  </part>\n");
    }
    o.push_str("</score-partwise>\n");
    o
}

fn gcd(mut a: i64, mut b: i64) -> i64 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a.abs().max(1)
}

/// A chord symbol as `<harmony>`: its root, and the rest kept as the
/// kind's text, so it reads back as written.
fn harmony_xml(sym: &str) -> String {
    let mut chars = sym.chars();
    let Some(step) = chars.next().filter(|c| "ABCDEFG".contains(*c)) else {
        return format!("      <direction><direction-type><words>{}</words></direction-type></direction>\n", esc(sym));
    };
    let rest: String = chars.collect();
    let (alter, rest) = if let Some(r) = rest.strip_prefix('#') {
        (1, r)
    } else if let Some(r) = rest.strip_prefix('b') {
        (-1, r)
    } else {
        (0, rest.as_str())
    };
    let mut s = format!("      <harmony><root><root-step>{step}</root-step>");
    if alter != 0 {
        s.push_str(&format!("<root-alter>{alter}</root-alter>"));
    }
    s.push_str(&format!("</root><kind text=\"{}\">other</kind></harmony>\n", esc(rest)));
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCORE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE score-partwise PUBLIC "-//Recordare//DTD MusicXML 4.0 Partwise//EN" "http://www.musicxml.org/dtds/partwise.dtd">
<score-partwise version="4.0">
  <work><work-title>Little Piece</work-title></work>
  <part-list>
    <score-part id="P1"><part-name>Flute</part-name></score-part>
    <score-part id="P2"><part-name>Clarinet in Bb</part-name>
      <midi-instrument id="P2-I1"><midi-channel>3</midi-channel><midi-program>72</midi-program></midi-instrument></score-part>
  </part-list>
  <part id="P1">
    <measure number="1">
      <barline location="left"><repeat direction="forward"/></barline>
      <attributes><divisions>2</divisions><key><fifths>1</fifths></key><time><beats>2</beats><beat-type>4</beat-type></time></attributes>
      <direction><sound tempo="90" dynamics="100"/></direction>
      <harmony><root><root-step>G</root-step></root><kind>major</kind></harmony>
      <note><pitch><step>G</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><lyric number="1"><syllabic>begin</syllabic><text>Hap</text></lyric></note>
      <note><pitch><step>B</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice></note>
      <note><chord/><pitch><step>D</step><octave>5</octave></pitch><duration>2</duration><voice>1</voice></note>
    </measure>
    <measure number="2">
      <barline location="left"><ending number="1" type="start"/></barline>
      <note><pitch><step>F</step><alter>1</alter><octave>4</octave></pitch><duration>4</duration><tie type="start"/></note>
      <barline location="right"><ending number="1" type="stop"/><repeat direction="backward"/></barline>
    </measure>
    <measure number="3">
      <barline location="left"><ending number="2" type="start"/></barline>
      <note><grace/><pitch><step>A</step><octave>4</octave></pitch></note>
      <note><pitch><step>G</step><octave>4</octave></pitch><duration>1</duration></note>
      <note><rest/><duration>1</duration></note>
      <backup><duration>2</duration></backup>
      <note><pitch><step>E</step><octave>4</octave></pitch><duration>2</duration><voice>2</voice></note>
      <forward><duration>2</duration></forward>
      <barline location="right"><ending number="2" type="discontinue"/></barline>
    </measure>
  </part>
  <part id="P2">
    <measure number="1">
      <attributes><divisions>1</divisions><transpose><diatonic>-1</diatonic><chromatic>-2</chromatic></transpose></attributes>
      <note><pitch><step>D</step><octave>5</octave></pitch><duration>2</duration></note>
    </measure>
    <measure number="2"><note><pitch><step>E</step><octave>5</octave></pitch><duration>2</duration></note></measure>
    <measure number="3"><note><pitch><step>F</step><octave>5</octave></pitch><duration>2</duration></note></measure>
  </part>
</score-partwise>"#;

    #[test]
    fn reads_a_score() {
        let (score, title) = read(SCORE).unwrap();
        assert_eq!(title, "Little Piece");
        assert_eq!((score.bpm, score.beats, score.unit), (90.0, 2, 4));
        let flute: Vec<(u32, u32, u8, u8)> = score.parts[0].notes.iter().map(|n| (n.at, n.length, n.key, n.velocity)).collect();
        // Measures play 1, 2, 1, 3.
        assert_eq!(
            flute,
            vec![(0, 480, 67, 90), (480, 480, 71, 90), (480, 480, 74, 90), (960, 960, 66, 90), (1920, 480, 67, 90), (2400, 480, 71, 90), (2400, 480, 74, 90), (2880, 480, 64, 90), (2880, 240, 67, 90)]
        );
        assert_eq!(score.parts[0].program, 73, "a flute, by its name");
        let clarinet = &score.parts[1];
        assert_eq!((clarinet.channel, clarinet.program), (2, 71));
        // Written D sounds C.
        assert_eq!(clarinet.notes.iter().map(|n| n.key).collect::<Vec<_>>(), vec![72, 74, 72, 75]);
        assert!(score.kept.iter().any(|k| k.bytes == crate::text::meta(0x01, b"G")));
        assert!(score.parts[0].kept.iter().any(|k| k.bytes == crate::text::meta(0x05, b"Hap-")));
        assert_eq!(score.bars, 4);
    }

    #[test]
    fn timewise() {
        let tw = r#"<score-timewise><part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list>
            <measure number="1"><part id="P1"><attributes><divisions>1</divisions></attributes>
            <note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration></note></part></measure>
            <measure number="2"><part id="P1"><note><pitch><step>D</step><octave>4</octave></pitch><duration>4</duration></note></part></measure></score-timewise>"#;
        let (score, _) = read(tw).unwrap();
        assert_eq!(score.parts[0].notes.iter().map(|n| (n.at, n.key)).collect::<Vec<_>>(), vec![(0, 60), (1920, 62)]);
    }

    #[test]
    fn errors() {
        assert!(matches!(read("<html></html>"), Err(Error::BadText { .. })));
        assert!(matches!(read("<score-partwise><part></score-partwise>"), Err(Error::BadText { line: 1, .. })));
    }
}
