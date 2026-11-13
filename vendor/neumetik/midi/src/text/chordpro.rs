// Copyright 2026 ansiapps. Neumetik MIDI, MIT licensed: see LICENSE.

//! **ChordPro** (chordpro.org, version 6), read: lyrics with `[chords]`
//! in them, and directives in braces. The music is the chords, voiced and
//! played in a simple pattern: a piano on every beat, a bass on the first
//! and the middle of each bar. Their timing comes from `{tempo}` and
//! `{time}`, grid sections (`{start_of_grid}`: `| G . . . | C . D . |`,
//! each cell a share of its bar, `.` holding the chord, `/` playing it
//! again, `N.C.` silence, `%` the bar before), bar lines among the lyrics (the chords
//! between two sharing their bar), else a chord a bar. `{chorus}` plays
//! the last chorus again; `{capo}` and `{transpose}` sound the chords
//! higher. The chord symbols and lyrics are kept as text events.
//!
//! Written ([`write`]): the score's own chord symbols, else chords
//! recognised from its notes a beat at a time ([`recognise`]); as lyric
//! lines with the chords in them and bar lines when it has lyrics, else
//! as a grid, so the timing is kept to the beat.

use super::{bad, Builder, Frac};
use crate::score::Score;
use crate::Error;

/// A chord symbol's notes: the root (0 is C) and intervals above it,
/// and a bass (`C/E`) if it's not the root.
pub fn chord(symbol: &str) -> Option<(i32, Vec<i32>, Option<i32>)> {
    let s = symbol.trim();
    let (main, bass) = match s.split_once('/') {
        Some((m, b)) if !b.is_empty() && b.chars().next().is_some_and(|c| "ABCDEFG".contains(c)) => (m, Some(b)),
        _ => (s, None),
    };
    let note = |t: &str| -> Option<(i32, usize)> {
        let mut chars = t.chars();
        let letter = chars.next()?;
        if !"ABCDEFG".contains(letter) {
            return None;
        }
        let mut k = super::letter_key(letter)? - 60;
        let mut used = 1;
        for c in chars {
            match c {
                '#' | '♯' => k += 1,
                'b' | '♭' => k -= 1,
                _ => break,
            }
            used += c.len_utf8();
        }
        Some((k.rem_euclid(12), used))
    };
    let (root, used) = note(main)?;
    let q = &main[used..];
    let mut tones: Vec<i32> = vec![0, 4, 7];
    let mut rest = q;
    let mut seventh_major = false;
    let take = |rest: &mut &str, p: &str| -> bool {
        if let Some(r) = rest.strip_prefix(p) {
            *rest = r;
            true
        } else {
            false
        }
    };
    if take(&mut rest, "maj") || take(&mut rest, "Maj") || take(&mut rest, "M") || take(&mut rest, "Δ") || take(&mut rest, "^") {
        seventh_major = true;
        if rest.is_empty() {
            // "Cmaj" or "CM": a major chord.
            seventh_major = false;
        }
    } else if take(&mut rest, "min") || take(&mut rest, "mi") || take(&mut rest, "m") || take(&mut rest, "-") {
        tones = vec![0, 3, 7];
        if take(&mut rest, "maj") || take(&mut rest, "Maj") || take(&mut rest, "M") || take(&mut rest, "(maj") {
            seventh_major = true;
        }
    } else if take(&mut rest, "dim") || take(&mut rest, "o") || take(&mut rest, "°") {
        tones = vec![0, 3, 6];
        if take(&mut rest, "7") {
            tones.push(9);
        }
    } else if take(&mut rest, "ø") {
        tones = vec![0, 3, 6, 10];
    } else if take(&mut rest, "aug") || take(&mut rest, "+") {
        tones = vec![0, 4, 8];
    }
    let mut guard = 0;
    while !rest.is_empty() && guard < 16 {
        guard += 1;
        let r = rest.trim_start_matches(['(', ')', ' ', ',']);
        if r.is_empty() {
            break;
        }
        rest = r;
        let ext = |n: i32, tones: &mut Vec<i32>, maj: bool| {
            let seventh = if maj { 11 } else { 10 };
            match n {
                6 => tones.push(9),
                7 => tones.push(seventh),
                9 => tones.extend([seventh, 14]),
                11 => tones.extend([seventh, 14, 17]),
                13 => tones.extend([seventh, 14, 21]),
                2 => tones.push(14),
                4 => tones.push(17),
                _ => {}
            }
        };
        if take(&mut rest, "sus2") {
            tones.retain(|&t| t != 3 && t != 4);
            tones.push(2);
        } else if take(&mut rest, "sus4") || take(&mut rest, "sus") {
            tones.retain(|&t| t != 3 && t != 4);
            tones.push(5);
        } else if take(&mut rest, "add") {
            let n: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            rest = &rest[n.len()..];
            match n.as_str() {
                "9" | "2" => tones.push(14),
                "11" | "4" => tones.push(17),
                "13" | "6" => tones.push(21),
                _ => {}
            }
        } else if take(&mut rest, "b5") || take(&mut rest, "-5") {
            tones.retain(|&t| t != 7);
            tones.push(6);
        } else if take(&mut rest, "#5") || take(&mut rest, "+5") {
            tones.retain(|&t| t != 7);
            tones.push(8);
        } else if take(&mut rest, "b9") {
            tones.push(13);
        } else if take(&mut rest, "#9") {
            tones.push(15);
        } else if take(&mut rest, "#11") {
            tones.push(18);
        } else if take(&mut rest, "b13") {
            tones.push(20);
        } else if take(&mut rest, "5") && tones == [0, 4, 7] {
            tones = vec![0, 7];
        } else if take(&mut rest, "maj7") || take(&mut rest, "M7") {
            tones.push(11);
        } else if rest.starts_with(|c: char| c.is_ascii_digit()) {
            let n: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            rest = &rest[n.len()..];
            ext(n.parse().unwrap_or(0), &mut tones, seventh_major);
        } else {
            return None;
        }
    }
    if seventh_major && !tones.contains(&11) && !tones.iter().any(|&t| t >= 14) {
        tones.push(11);
    }
    tones.sort_unstable();
    tones.dedup();
    let bass = match bass {
        Some(b) => Some(note(b)?.0),
        None => None,
    };
    Some((root, tones, bass))
}

/// Whether a text looks like ChordPro: `{directives}`, or lyrics with
/// `[chords]` in them.
pub fn looks_like(text: &str) -> bool {
    let mut chorded = 0;
    for line in text.lines() {
        let l = line.trim();
        if l.starts_with('{') && l.ends_with('}') && l.contains(':') {
            return true;
        }
        let mut rest = l;
        let mut chords = 0;
        let mut words = String::new();
        while let Some(i) = rest.find('[') {
            words.push_str(&rest[..i]);
            let Some(e) = rest[i..].find(']') else { break };
            let sym = &rest[i + 1..i + e];
            if chord(sym).is_none() {
                return false;
            }
            chords += 1;
            rest = &rest[i + e + 1..];
        }
        words.push_str(rest);
        if chords > 0 && words.split_whitespace().any(|w| w.chars().filter(|c| c.is_alphabetic()).count() >= 2) {
            chorded += 1;
        }
    }
    chorded >= 1
}

/// A chord's place in the music: when, how long, its symbol.
type Placed = (Frac, Frac, String);

/// One section's chords and lyrics, as they're laid out from its lines.
#[derive(Default, Clone)]
struct Section {
    chords: Vec<Placed>,
    lyrics: Vec<(Frac, String)>,
    length: Frac,
}

/// A ChordPro song as a score and its title.
pub fn read(text: &str) -> Result<(Score, String), Error> {
    let mut title = String::new();
    let mut bpm = 100.0;
    let (mut beats, mut unit) = (4u8, 4u8);
    let mut key: Option<(i8, bool)> = None;
    let mut shift = 0i32;
    let mut grid = false;
    let mut in_chorus = false;
    let mut chorus = Section::default();
    let mut last_chorus = Section::default();
    let mut now = Frac::ZERO;
    let mut song = Section::default();
    let mut tab = false;
    let mut at = 0;
    for raw in text.split_inclusive('\n') {
        let line_at = at;
        at += raw.len();
        let line = raw.trim_end_matches(['\n', '\r']);
        let t = line.trim();
        if t.starts_with('#') {
            continue;
        }
        let bar = Frac::new(i64::from(beats), i64::from(unit));
        if t.starts_with('{') {
            let end = t.find('}').ok_or_else(|| bad(text, line_at, "a directive in { } that doesn't end"))?;
            let inner = &t[1..end];
            let (name, value) = match inner.split_once([':', ' ']) {
                Some((n, v)) => (n.trim().to_ascii_lowercase(), v.trim().to_string()),
                None => (inner.trim().to_ascii_lowercase(), String::new()),
            };
            match name.as_str() {
                "title" | "t" => {
                    if title.is_empty() {
                        title = value;
                    }
                }
                "tempo" => {
                    if let Ok(v) = value.parse::<f64>() {
                        bpm = v;
                    }
                }
                "time" => {
                    if let Some((n, d)) = value.split_once('/') {
                        if let (Ok(n), Ok(d)) = (n.trim().parse::<u8>(), d.trim().parse::<u8>()) {
                            if n > 0 && d.is_power_of_two() {
                                (beats, unit) = (n, d);
                            }
                        }
                    }
                }
                "key" => key = super::key_signature(&value),
                "capo" => shift += value.parse::<i32>().unwrap_or(0),
                "transpose" => shift += value.parse::<i32>().unwrap_or(0),
                "start_of_grid" | "sog" => grid = true,
                "end_of_grid" | "eog" => grid = false,
                "start_of_tab" | "sot" => tab = true,
                "end_of_tab" | "eot" => tab = false,
                "start_of_chorus" | "soc" => {
                    in_chorus = true;
                    chorus = Section::default();
                }
                "end_of_chorus" | "eoc" => {
                    in_chorus = false;
                    chorus.length = now.sub(chorus.chords.first().map(|c| c.0).unwrap_or(now));
                    last_chorus = chorus.clone();
                }
                "chorus" => {
                    // The last chorus again, from here.
                    if let Some(first) = last_chorus.chords.first().map(|c| c.0) {
                        for (s, l, c) in &last_chorus.chords {
                            song.chords.push((now.add(s.sub(first)), *l, c.clone()));
                        }
                        for (s, w) in &last_chorus.lyrics {
                            song.lyrics.push((now.add(s.sub(first)), w.clone()));
                        }
                        now = now.add(last_chorus.length);
                    }
                }
                _ => {}
            }
            continue;
        }
        if tab || t.is_empty() {
            continue;
        }
        let mut placed: Vec<Placed> = vec![];
        let mut words: Vec<(Frac, String)> = vec![];
        if grid {
            // | G . . . | C . D . |
            let mut cells_bar: Vec<Vec<&str>> = vec![];
            let mut cur: Vec<&str> = vec![];
            let mut started = false;
            for tok in t.split_whitespace() {
                if tok.starts_with('|') || tok.ends_with('|') || tok == "||" || tok.starts_with(":|") {
                    if started && !cur.is_empty() {
                        cells_bar.push(std::mem::take(&mut cur));
                    }
                    started = true;
                    continue;
                }
                if !started {
                    // Text before the first bar line is a label.
                    continue;
                }
                cur.push(tok);
            }
            if !cur.is_empty() && started {
                // Text after the last bar line is a comment.
            }
            let mut last: Option<String> = song.chords.last().map(|c| c.2.clone());
            let mut silent = false;
            let mut previous_bar: Vec<&str> = vec![];
            for cells in cells_bar {
                let cells = if cells == ["%"] { previous_bar.clone() } else { cells };
                let share = bar.div(Frac::int(cells.len().max(1) as i64));
                for (i, c) in cells.iter().enumerate() {
                    let when = now.add(share.mul(Frac::int(i as i64)));
                    match *c {
                        "." | "~" => {
                            if let Some(p) = placed.last_mut().filter(|_| !silent) {
                                p.1 = p.1.add(share);
                            }
                        }
                        "N.C." | "n.c." | "NC" => silent = true,
                        "/" => {
                            if let Some(l) = &last {
                                placed.push((when, share, l.clone()));
                                silent = false;
                            }
                        }
                        sym => {
                            silent = false;
                            if chord(sym).is_none() {
                                return Err(bad(text, line_at + line.find(sym).unwrap_or(0), "a chord that can't be read"));
                            }
                            placed.push((when, share, sym.to_string()));
                            last = Some(sym.to_string());
                        }
                    }
                }
                now = now.add(bar);
                previous_bar = cells;
            }
        } else if t.contains('[') {
            // Lyrics with chords; bar lines among them share a bar.
            let bars_marked = t.contains('|');
            let mut segments: Vec<Vec<(String, String)>> = vec![vec![]];
            let mut rest = t;
            let mut lead = String::new();
            let mut offset = line_at + (line.len() - line.trim_start().len());
            while let Some(i) = rest.find(['[', '|']) {
                let before = &rest[..i];
                match segments.last_mut().and_then(|s| s.last_mut()) {
                    Some(last) => last.1.push_str(before),
                    None => lead.push_str(before),
                }
                if rest.as_bytes()[i] == b'|' {
                    segments.push(vec![]);
                    rest = &rest[i + 1..];
                    offset += i + 1;
                    continue;
                }
                let Some(e) = rest[i..].find(']') else { return Err(bad(text, offset + i, "a chord in [ ] that doesn't end")) };
                let sym = rest[i + 1..i + e].trim();
                if chord(sym).is_none() && !sym.starts_with('*') {
                    return Err(bad(text, offset + i + 1, "a chord that can't be read"));
                }
                if !sym.starts_with('*') {
                    if let Some(s) = segments.last_mut() { s.push((sym.to_string(), String::new())) }
                }
                rest = &rest[i + e + 1..];
                offset += i + e + 1;
            }
            match segments.last_mut().and_then(|s| s.last_mut()) {
                Some(last) => last.1.push_str(rest),
                None => lead.push_str(rest),
            }
            let mut first_word = true;
            for seg in segments.iter().filter(|s| !s.is_empty() || !bars_marked) {
                if seg.is_empty() {
                    continue;
                }
                if bars_marked {
                    let share = bar.div(Frac::int(seg.len() as i64));
                    for (i, (sym, w)) in seg.iter().enumerate() {
                        let when = now.add(share.mul(Frac::int(i as i64)));
                        placed.push((when, share, sym.clone()));
                        let w = if first_word { format!("{lead}{w}") } else { w.clone() };
                        first_word = false;
                        if !w.trim().is_empty() {
                            words.push((when, w.trim().to_string()));
                        }
                    }
                    now = now.add(bar);
                } else {
                    for (sym, w) in seg {
                        placed.push((now, bar, sym.clone()));
                        let w = if first_word { format!("{lead}{w}") } else { w.clone() };
                        first_word = false;
                        if !w.trim().is_empty() {
                            words.push((now, w.trim().to_string()));
                        }
                        now = now.add(bar);
                    }
                }
            }
        } else {
            // A line of lyrics with no chords: its words go with the music
            // already there; no time passes.
            continue;
        }
        if in_chorus {
            chorus.chords.extend(placed.iter().cloned());
            chorus.lyrics.extend(words.iter().cloned());
        }
        song.chords.extend(placed);
        song.lyrics.extend(words);
    }
    if song.chords.is_empty() {
        return Err(bad(text, 0, "there are no chords in it to play"));
    }
    let mut b = Builder::new();
    b.title = title.clone();
    b.tempo(Frac::ZERO, bpm);
    b.meter(Frac::ZERO, beats, unit);
    if let Some((s, m)) = key {
        b.key(Frac::ZERO, s, m);
    }
    let piano = b.part("Chords", 0, false);
    let bass = b.part("Bass", 32, false);
    let beat = Frac::new(1, i64::from(unit));
    let bar = Frac::new(i64::from(beats), i64::from(unit));
    let half = if beats % 2 == 0 && beats >= 4 { bar.scale(1, 2) } else { bar };
    for (start, len, sym) in &song.chords {
        b.chord(*start, sym);
        let Some((root, tones, bass_note)) = chord(sym) else { continue };
        let root = root + shift;
        // Piano: the chord around middle C, a hit a beat.
        let voicing: Vec<i32> = tones.iter().map(|t| 48 + (root + t).rem_euclid(12) + if (root + t).rem_euclid(12) < 5 { 12 } else { 0 } + if *t >= 12 { 12 } else { 0 }).collect();
        let mut t = *start;
        let end = start.add(*len);
        while t < end {
            let l = beat.min(end.sub(t));
            for &k in &voicing {
                b.note(piano, t, l.scale(9, 10), k, 70);
            }
            t = t.add(beat);
        }
        // Bass: on the bar's first beat and its middle.
        let bk = 36 + (bass_note.map(|x| x + shift).unwrap_or(root)).rem_euclid(12);
        let mut t = *start;
        while t < end {
            let l = half.min(end.sub(t));
            b.note(bass, t, l.scale(9, 10), bk, 90);
            t = t.add(half);
        }
    }
    for (when, w) in &song.lyrics {
        b.lyric(piano, *when, w);
    }
    b.end_at(now);
    Ok((b.finish(), title))
}

/// Chord kinds by their intervals, simplest first (a tie goes to the
/// simpler).
const KINDS: [(&str, &[usize]); 14] = [
    ("", &[0, 4, 7]),
    ("m", &[0, 3, 7]),
    ("7", &[0, 4, 7, 10]),
    ("m7", &[0, 3, 7, 10]),
    ("maj7", &[0, 4, 7, 11]),
    ("5", &[0, 7]),
    ("sus4", &[0, 5, 7]),
    ("sus2", &[0, 2, 7]),
    ("dim", &[0, 3, 6]),
    ("aug", &[0, 4, 8]),
    ("6", &[0, 4, 7, 9]),
    ("m6", &[0, 3, 7, 9]),
    ("dim7", &[0, 3, 6, 9]),
    ("m7b5", &[0, 3, 6, 10]),
];

/// A pitch class named in a key signature (`F#` or `Gb`).
fn pc_name(pc: usize, sharps: i8) -> String {
    let sp = super::notation::spell(60 + pc as u8, sharps);
    format!("{}{}", sp.letter(), match sp.alter {
        1 => "#",
        2 => "##",
        -1 => "b",
        -2 => "bb",
        _ => "",
    })
}

/// The chord a beat's notes make: each pitch class's weight (how long
/// and loud it sounds), and the lowest key. Its tones count for it, any
/// other note half against it, and the lowest note being the root a
/// little more; an inversion is written over its bass (`C/E`). `None`
/// when nothing sounds.
pub fn recognise(weights: [f64; 12], lowest: Option<u8>, sharps: i8) -> Option<String> {
    let total: f64 = weights.iter().sum();
    if total <= 0.0 {
        return None;
    }
    let bass = lowest.map(|k| usize::from(k) % 12);
    let mut best: Option<(f64, usize, usize)> = None;
    for (ki, (_, tones)) in KINDS.iter().enumerate() {
        for root in 0..12 {
            let mut score = 0.0;
            for (pc, w) in weights.iter().enumerate() {
                if tones.contains(&((pc + 12 - root) % 12)) {
                    score += w;
                } else {
                    score -= w * 0.5;
                }
            }
            // Tones the chord has that don't sound count a little against it.
            score -= tones.iter().filter(|&&t| weights[(root + t) % 12] <= 0.0).count() as f64 * total * 0.2;
            if bass == Some(root) {
                score += total * 0.1;
            }
            if best.map_or(true, |b| score > b.0 + 1e-9) {
                best = Some((score, ki, root));
            }
        }
    }
    let (_, ki, root) = best?;
    let mut name = format!("{}{}", pc_name(root, sharps), KINDS[ki].0);
    if let Some(b) = bass.filter(|&b| b != root && KINDS[ki].1.contains(&((b + 12 - root) % 12))) {
        name.push('/');
        name.push_str(&pc_name(b, sharps));
    }
    Some(name)
}

/// A score as ChordPro, titled.
pub fn write(score: &Score, title: &str) -> String {
    use crate::score::DRUMS;
    let n = super::notation::notate(score);
    let whole = Frac::int(4 * i64::from(score.ppq.max(1)));
    let at = |t: u32| Frac::int(i64::from(t)).div(whole);
    // Each beat's chord.
    let mut cells: Vec<Vec<(Frac, Option<String>)>> = vec![];
    for bar in &n.bars {
        let beat = Frac::new(1, i64::from(bar.unit.max(1)));
        let mut row = vec![];
        let mut t = bar.start;
        let end = bar.start.add(bar.len);
        while t < end {
            let e = t.add(beat).min(end);
            let mut w = [0.0; 12];
            let mut lowest: Option<u8> = None;
            for p in score.parts.iter().filter(|p| p.channel != DRUMS) {
                for x in &p.notes {
                    let (s0, e0) = (at(x.at), at(x.at + x.length));
                    let overlap = e0.min(e).sub(s0.max(t));
                    if overlap.is_positive() {
                        w[usize::from(x.key % 12)] += overlap.f64() * f64::from(x.velocity);
                        lowest = Some(lowest.map_or(x.key, |l| l.min(x.key)));
                    }
                }
            }
            let sym = if lowest.is_none() {
                None
            } else if !n.chords.is_empty() {
                // The score's own: the last one starting by the beat's end.
                n.chords.iter().rfind(|c| c.0 < e).map(|c| c.1.clone())
            } else {
                recognise(w, lowest, bar.sharps)
            };
            row.push((t, sym));
            t = e;
        }
        cells.push(row);
    }
    let mut o = String::new();
    if !title.is_empty() {
        o.push_str(&format!("{{title: {}}}\n", super::notation::one_line(title)));
    }
    let first = &n.bars[0];
    o.push_str(&format!("{{tempo: {}}}\n", (first.tempo.unwrap_or(score.bpm) * 100.0).round() / 100.0));
    o.push_str(&format!("{{time: {}/{}}}\n", first.beats, first.unit));
    let major = ["Cb", "Gb", "Db", "Ab", "Eb", "Bb", "F", "C", "G", "D", "A", "E", "B", "F#", "C#"];
    let k = major[(i32::from(first.sharps) + 7) as usize];
    o.push_str(&format!("{{key: {}}}\n", if first.minor { format!("{}m", ["Ab", "Eb", "Bb", "F", "C", "G", "D", "A", "E", "B", "F#", "C#", "G#", "D#", "A#"][(i32::from(first.sharps) + 7) as usize]) } else { k.to_string() }));
    // Lyrics, from the part with the most.
    let lyrics: Vec<(Frac, String)> = score
        .parts
        .iter()
        .map(|p| p.kept.iter().filter(|k| k.bytes.len() > 2 && k.bytes[0] == 0xff && k.bytes[1] == 0x05).map(|k| (at(k.at), String::from_utf8_lossy(meta_data(&k.bytes)).into_owned())).collect::<Vec<_>>())
        .max_by_key(|l| l.len())
        .unwrap_or_default();
    if lyrics.is_empty() {
        o.push_str("{start_of_grid}\n");
        let mut line = String::new();
        let mut prev: Option<Option<String>> = None;
        let mut in_line = 0;
        for (bi, bar) in n.bars.iter().enumerate() {
            if bi > 0 && (n.bars[bi - 1].beats, n.bars[bi - 1].unit) != (bar.beats, bar.unit) {
                if !line.is_empty() {
                    o.push_str(line.trim_end());
                    o.push_str(" |\n");
                    line.clear();
                    in_line = 0;
                    prev = None;
                }
                o.push_str(&format!("{{time: {}/{}}}\n", bar.beats, bar.unit));
            }
            line.push_str("| ");
            for (_, sym) in &cells[bi] {
                let same = prev.as_ref() == Some(sym);
                line.push_str(&match (same, sym) {
                    (true, _) => ".".to_string(),
                    (false, None) => "N.C.".to_string(),
                    (false, Some(c)) => c.clone(),
                });
                line.push(' ');
                prev = Some(sym.clone());
            }
            in_line += 1;
            if in_line == 4 {
                o.push_str(&line);
                o.push_str("|\n");
                line.clear();
                in_line = 0;
                prev = None;
            }
        }
        if !line.is_empty() {
            o.push_str(&line);
            o.push_str("|\n");
        }
        o.push_str("{end_of_grid}\n");
        return o;
    }
    // Lyric lines: a bar's syllables with the chords that change in it,
    // bars between bar lines, four bars a line.
    let mut line = String::new();
    let mut last: Option<String> = None;
    for (bi, bar) in n.bars.iter().enumerate() {
        let end = bar.start.add(bar.len);
        let mut words: Vec<(Frac, &str)> = lyrics.iter().filter(|l| l.0 >= bar.start && l.0 < end).map(|l| (l.0, l.1.as_str())).collect();
        words.sort_by_key(|w| w.0);
        let mut bar_text = String::new();
        let mut cell_iter = cells[bi].iter().peekable();
        let mut pending: Vec<String> = vec![];
        let mut take_chords = |upto: Frac, pending: &mut Vec<String>, last: &mut Option<String>| {
            while let Some((t, sym)) = cell_iter.peek() {
                if *t > upto {
                    break;
                }
                if let Some(c) = sym {
                    if last.as_ref() != Some(c) {
                        pending.push(c.clone());
                        *last = Some(c.clone());
                    }
                }
                cell_iter.next();
            }
        };
        for (t, w) in &words {
            take_chords(*t, &mut pending, &mut last);
            for c in pending.drain(..) {
                bar_text.push_str(&format!("[{c}]"));
            }
            match w.strip_suffix('-') {
                Some(stem) => bar_text.push_str(stem),
                None => {
                    bar_text.push_str(w);
                    bar_text.push(' ');
                }
            }
        }
        take_chords(end, &mut pending, &mut last);
        for c in pending.drain(..) {
            bar_text.push_str(&format!("[{c}] "));
        }
        line.push_str(bar_text.trim_end());
        if bi % 4 == 3 || bi + 1 == n.bars.len() {
            o.push_str(line.trim());
            o.push('\n');
            line.clear();
        } else {
            line.push_str(" | ");
        }
    }
    o
}

/// A meta event's data.
fn meta_data(bytes: &[u8]) -> &[u8] {
    let mut i = 2;
    while i < bytes.len() && bytes[i] & 0x80 != 0 {
        i += 1;
    }
    bytes.get(i + 1..).unwrap_or(&[])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chords() {
        assert_eq!(chord("C"), Some((0, vec![0, 4, 7], None)));
        assert_eq!(chord("Am7"), Some((9, vec![0, 3, 7, 10], None)));
        assert_eq!(chord("F#m7b5"), Some((6, vec![0, 3, 6, 10], None)));
        assert_eq!(chord("Bbmaj7"), Some((10, vec![0, 4, 7, 11], None)));
        assert_eq!(chord("Dsus4"), Some((2, vec![0, 5, 7], None)));
        assert_eq!(chord("G7sus4").map(|c| c.1), Some(vec![0, 5, 7, 10]));
        assert_eq!(chord("Cadd9").map(|c| c.1), Some(vec![0, 4, 7, 14]));
        assert_eq!(chord("C/E"), Some((0, vec![0, 4, 7], Some(4))));
        assert_eq!(chord("Edim7").map(|c| c.1), Some(vec![0, 3, 6, 9]));
        assert_eq!(chord("A5").map(|c| c.1), Some(vec![0, 7]));
        assert_eq!(chord("Hello"), None);
        assert_eq!(chord("Cxyz"), None);
    }

    #[test]
    fn a_song() {
        let song = "{title: Little Song}\n{tempo: 90}\n{key: G}\n\n{start_of_chorus}\n[G]Sing a [C]song of [D]joy\n{end_of_chorus}\n\nA line with no chords\n{chorus}\n";
        assert!(looks_like(song));
        let (score, title) = read(song).unwrap();
        assert_eq!((title.as_str(), score.bpm), ("Little Song", 90.0));
        // Three chords a bar each, then the chorus again: six bars.
        assert_eq!(score.bars, 6);
        let bass: Vec<(u32, u8)> = score.parts[1].notes.iter().map(|n| (n.at, n.key)).collect();
        assert_eq!(bass[..4], [(0, 43), (960, 43), (1920, 36), (2880, 36)]);
        assert_eq!(bass[6], (5760, 43));
        // A piano chord on every beat.
        assert_eq!(score.parts[0].notes.iter().filter(|n| n.at == 480).count(), 3);
        assert!(score.parts[0].kept.iter().any(|k| k.bytes == crate::text::meta(0x05, b"Sing a")));
    }

    #[test]
    fn grids_and_bars() {
        let song = "{time: 3/4}\n{start_of_grid}\n| G . D | % | C / . |\n{end_of_grid}\n[Am]Two [E]chords | [F]one\n";
        let (score, _) = read(song).unwrap();
        let symbols: Vec<(u32, Vec<u8>)> = score.kept.iter().filter(|k| k.bytes[1] == 0x01).map(|k| (k.at, k.bytes[3..].to_vec())).collect();
        let s = |x: &str| x.as_bytes().to_vec();
        assert_eq!(symbols, vec![(0, s("G")), (960, s("D")), (1440, s("G")), (2400, s("D")), (2880, s("C")), (3360, s("C")), (4320, s("Am")), (5040, s("E")), (5760, s("F"))]);
        assert_eq!(score.bars, 5);
    }

    #[test]
    fn errors() {
        assert!(matches!(read("[Xq]la la"), Err(Error::BadText { line: 1, column: 2, .. })));
        assert!(read("just words").is_err());
        assert!(!looks_like("[c3 e3] g3"));
    }

    #[test]
    fn chords_are_recognised_from_their_notes() {
        let w = |keys: &[u8]| {
            let mut w = [0.0; 12];
            for &k in keys {
                w[usize::from(k % 12)] += 1.0;
            }
            w
        };
        assert_eq!(recognise(w(&[48, 64, 67, 72]), Some(48), 0).as_deref(), Some("C"));
        assert_eq!(recognise(w(&[45, 60, 64, 69]), Some(45), 0).as_deref(), Some("Am"));
        assert_eq!(recognise(w(&[43, 59, 62, 65]), Some(43), 0).as_deref(), Some("G7"));
        assert_eq!(recognise(w(&[52, 60, 67]), Some(52), 0).as_deref(), Some("C/E"));
        assert_eq!(recognise(w(&[46, 62, 65]), Some(46), -2).as_deref(), Some("Bb"));
        assert_eq!(recognise([0.0; 12], None, 0), None);
    }

    #[test]
    fn a_song_written_reads_back_with_its_chords_on_the_beat() {
        // C for a bar, G and Am for two beats each, silence, F.
        let song = "{title: Four}\n{time: 4/4}\n{start_of_grid}\n| C . . . | G . Am . | N.C. . . . | F . . . |\n{end_of_grid}\n";
        let (score, title) = read(song).expect("reads");
        assert_eq!(title, "Four");
        let back = write(&score, &title);
        assert!(back.contains("| C . . . | G . Am . | N.C. . . . | F . . . |"), "{back}");
        let (again, _) = read(&back).expect("reads again");
        let notes = |s: &Score| s.parts.iter().map(|p| p.notes.iter().map(|n| (n.at, n.key)).collect::<Vec<_>>()).collect::<Vec<_>>();
        assert_eq!(notes(&again), notes(&score));
    }

    #[test]
    fn a_score_without_chord_symbols_gets_them_from_its_notes() {
        let (score, _) = crate::text::abc::read("X:1\nM:4/4\nL:1/4\nK:C\n[CEG]4 | [G,B,DF]2 [A,CE]2 |]\n").expect("reads");
        let text = write(&score, "");
        assert!(text.contains("| C . . . | G7 . Am . |"), "{text}");
    }

    #[test]
    fn lyrics_are_written_with_their_chords() {
        let (score, _) = read("{time: 4/4}\n[C]Hel-lo [G]world | [F]good-bye\n").expect("reads");
        let text = write(&score, "");
        assert!(text.contains("[C]Hel-lo [G]world | [F]good-bye\n"), "{text}");
        let (again, _) = read(&text).expect("reads again");
        assert_eq!(again.parts.iter().map(|p| p.notes.len()).sum::<usize>(), score.parts.iter().map(|p| p.notes.len()).sum::<usize>());
    }
}

