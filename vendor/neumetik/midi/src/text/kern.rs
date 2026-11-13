// Copyright 2026 ansiapps. Neumetik MIDI, MIT licensed: see LICENSE.

//! **Humdrum `**kern`**, from the Humdrum Toolkit's reference manual:
//! spines (tab-separated columns), each `**kern` spine a part; spines
//! splitting and joining (`*^`, `*v`), exchanged (`*x`), added (`*+`) and
//! ended (`*-`); notes as a duration (the reciprocal: `4` a quarter, `3`
//! a triplet half, `0` a breve, `3%2` two thirds of a whole) with dots,
//! then the pitch (`c` middle C, `cc` an octave up, `C` an octave down)
//! with `#`, `-` and `n`; `r` rests; chords as tokens in one field; ties
//! (`[`, `_`, `]`); `*MM` tempo, `*M` meter, `*k[]` key, `*I` instruments,
//! and section expansion lists (`*>[A,A,B]`). Grace notes (`q`, `Q`) take
//! no time and aren't played; other spines (`**dynam`, `**text`...) are
//! skipped.
//!
//! Written ([`write`]) from [`super::notation`]: a spine for each voice
//! of each part (the last part leftmost, as Humdrum has it), each
//! instrument by name and code, `*MM`, `*M` and `*k[]` where they change,
//! numbered bars, chords, ties, and lengths as reciprocals (`12` a
//! triplet eighth, `3%2` when no reciprocal fits). Velocity isn't kept.

use super::{bad, Builder, Frac};
use crate::score::Score;
use crate::Error;

#[derive(Debug, Clone)]
struct Spine {
    kern: bool,
    part: Option<usize>,
    /// When its last note or rest ends.
    end: Frac,
    /// Tied notes: key, note index.
    ties: Vec<(i32, usize)>,
}

/// A `**kern` file as a score and its title (`!!!OTL`).
pub fn read(text: &str) -> Result<(Score, String), Error> {
    // Lines with their offsets.
    let mut lines: Vec<(usize, &str)> = vec![];
    let mut at = 0;
    for l in text.split_inclusive('\n') {
        lines.push((at, l.trim_end_matches(['\n', '\r'])));
        at += l.len();
    }
    let mut title = String::new();
    for (_, l) in &lines {
        if let Some(t) = l.strip_prefix("!!!OTL") {
            if title.is_empty() {
                title = t.split_once(':').map(|x| x.1).unwrap_or(t).trim().to_string();
            }
        }
    }
    let lines = expand(lines);
    let mut b = Builder::new();
    b.title = title.clone();
    b.tempo(Frac::ZERO, 120.0);
    let mut spines: Vec<Spine> = vec![];
    let mut now = Frac::ZERO;
    let mut started = false;
    for (off, line) in lines {
        if line.is_empty() || line.starts_with("!!") {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        let field_at = |i: usize| off + fields[..i].iter().map(|f| f.len() + 1).sum::<usize>();
        if !started {
            if !fields[0].starts_with("**") {
                return Err(bad(text, off, "a Humdrum file starts with its spines' kinds, like **kern"));
            }
            for f in &fields {
                let kern = *f == "**kern";
                let part = kern.then(|| b.part(&format!("Spine {}", b.parts.len() + 1), 0, false));
                spines.push(Spine { kern, part, end: Frac::ZERO, ties: vec![] });
            }
            started = true;
            continue;
        }
        if fields.len() != spines.len() {
            return Err(bad(text, off, "a line with a different number of spines than the one before"));
        }
        let first = fields[0];
        if first.starts_with('!') {
            continue;
        }
        if first.starts_with('*') {
            // Interpretations, and spine changes.
            for (i, f) in fields.iter().enumerate() {
                let s = &spines[i];
                if !s.kern {
                    continue;
                }
                let part = s.part.unwrap_or(0);
                if let Some(t) = f.strip_prefix("*MM") {
                    if let Ok(t) = t.parse::<f64>() {
                        b.tempo(now, t);
                    }
                } else if let Some(m) = f.strip_prefix("*M") {
                    if let Some((n, d)) = m.split_once('/') {
                        if let (Ok(n), Ok(d)) = (n.parse::<u8>(), d.parse::<u8>()) {
                            b.meter(now, n, d);
                        }
                    }
                } else if let Some(k) = f.strip_prefix("*k[").and_then(|k| k.strip_suffix(']')) {
                    let sharps = k.matches('#').count() as i8 - k.matches('-').count() as i8;
                    b.key(now, sharps, false);
                } else if let Some(name) = f.strip_prefix("*I\"") {
                    b.parts[part].name = name.to_string();
                    if let Some(p) = super::program(name) {
                        b.parts[part].program = p;
                    }
                } else if let Some(code) = f.strip_prefix("*I") {
                    // Lower case: an instrument (`*IC` is a class, `*IG` a group).
                    if code.starts_with(|c: char| c.is_ascii_lowercase()) {
                        if let Some(p) = super::program(code) {
                            b.parts[part].program = p;
                            if b.parts[part].name.starts_with("Spine") {
                                b.parts[part].name = super::GM_NAMES[usize::from(p)].to_string();
                            }
                        }
                        if code == "perc" || code == "drums" {
                            b.parts[part].drums = true;
                        }
                    }
                }
            }
            if fields.iter().any(|f| matches!(*f, "*^" | "*v" | "*x" | "*-" | "*+") || f.starts_with("**")) {
                let mut next: Vec<Spine> = vec![];
                let mut i = 0;
                while i < fields.len() {
                    let f = fields[i];
                    match f {
                        "*^" => {
                            next.push(spines[i].clone());
                            next.push(Spine { ties: vec![], ..spines[i].clone() });
                        }
                        "*v" => {
                            // Joined with every *v next to it.
                            let mut j = i;
                            let mut end = spines[i].end;
                            while j + 1 < fields.len() && fields[j + 1] == "*v" {
                                j += 1;
                                end = end.max(spines[j].end);
                            }
                            let mut s = spines[i].clone();
                            s.end = end;
                            for joined in &spines[i + 1..=j] {
                                s.ties.extend(joined.ties.clone());
                            }
                            next.push(s);
                            i = j;
                        }
                        "*x" => {
                            if i + 1 < fields.len() && fields[i + 1] == "*x" {
                                next.push(spines[i + 1].clone());
                                next.push(spines[i].clone());
                                i += 1;
                            } else {
                                next.push(spines[i].clone());
                            }
                        }
                        "*-" => {}
                        "*+" => {
                            next.push(spines[i].clone());
                            next.push(Spine { kern: false, part: None, end: now, ties: vec![] });
                        }
                        f if f.starts_with("**") => {
                            let kern = f == "**kern";
                            let part = kern.then(|| b.part(&format!("Spine {}", b.parts.len() + 1), 0, false));
                            next.push(Spine { kern, part, end: now, ties: vec![] });
                        }
                        _ => next.push(spines[i].clone()),
                    }
                    i += 1;
                }
                spines = next;
                if spines.is_empty() {
                    break;
                }
            }
            continue;
        }
        if first.starts_with('=') {
            continue;
        }
        // Data: each kern field's notes start now.
        for (i, f) in fields.iter().enumerate() {
            if !spines[i].kern || *f == "." {
                continue;
            }
            let part = spines[i].part.unwrap_or(0);
            let mut longest: Option<Frac> = None;
            let mut ties = std::mem::take(&mut spines[i].ties);
            for token in f.split(' ').filter(|t| !t.is_empty() && *t != ".") {
                let n = note(token).ok_or_else(|| bad(text, field_at(i), "a **kern note's duration and pitch can't be read"))?;
                let Some((len, key, tie)) = n else { continue };
                longest = Some(longest.map_or(len, |l: Frac| l.min(len)));
                let Some(key) = key else { continue };
                match tie {
                    Tie::Middle | Tie::End => {
                        if let Some(p) = ties.iter().position(|t| t.0 == key) {
                            let (_, idx) = ties.remove(p);
                            let note = &mut b.parts[part].notes[idx];
                            note.1 = now.add(len).sub(note.0);
                            if tie == Tie::Middle {
                                ties.push((key, idx));
                            }
                            continue;
                        }
                    }
                    _ => {}
                }
                let before = b.parts[part].notes.len();
                b.note(part, now, len, key, 80);
                if tie == Tie::Start && b.parts[part].notes.len() > before {
                    ties.push((key, before));
                }
            }
            spines[i].ties = ties;
            if let Some(l) = longest {
                spines[i].end = now.add(l);
            }
        }
        // The next line is when the soonest sounding note or rest ends.
        let next = spines.iter().filter(|s| s.kern && s.end > now).map(|s| s.end).min();
        if let Some(n) = next {
            now = n;
        }
        b.end_at(now);
    }
    if !started {
        return Err(bad(text, 0, "a Humdrum file starts with its spines' kinds, like **kern"));
    }
    let end = spines.iter().map(|s| s.end).max().unwrap_or(now);
    b.end_at(end);
    Ok((b.finish(), title))
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Tie {
    None,
    Start,
    Middle,
    End,
}

/// One token: `Ok(None)` for a grace note (no time), else its length, the
/// key (none for a rest) and its tie.
#[allow(clippy::type_complexity)]
fn note(token: &str) -> Option<Option<(Frac, Option<i32>, Tie)>> {
    if token.contains('q') || token.contains('Q') {
        return Some(None);
    }
    let s = token.as_bytes();
    let mut tie = Tie::None;
    if token.contains('[') {
        tie = Tie::Start;
    }
    if token.contains('_') {
        tie = Tie::Middle;
    }
    if token.contains(']') {
        tie = Tie::End;
    }
    // The duration: digits, an optional %digits, dots.
    let mut i = s.iter().position(|c| c.is_ascii_digit())?;
    let st = i;
    while i < s.len() && s[i].is_ascii_digit() {
        i += 1;
    }
    let digits = &token[st..i];
    let mut len = match digits {
        "0" => Frac::int(2),
        "00" => Frac::int(4),
        "000" => Frac::int(8),
        d => Frac::new(1, d.parse().ok().filter(|&n: &i64| n > 0)?),
    };
    if s.get(i) == Some(&b'%') {
        i += 1;
        let st2 = i;
        while i < s.len() && s[i].is_ascii_digit() {
            i += 1;
        }
        let m: i64 = token[st2..i].parse().ok()?;
        let n: i64 = digits.parse().ok()?;
        len = Frac::new(m, n.max(1));
    }
    let dots = token.matches('.').count() as u32;
    let mut add = len;
    for _ in 0..dots.min(4) {
        add = add.scale(1, 2);
        len = len.add(add);
    }
    if token.contains('r') {
        return Some(Some((len, None, tie)));
    }
    // The pitch: a run of one letter.
    let letter = token.chars().find(|c| "abcdefgABCDEFG".contains(*c))?;
    let count = token.chars().filter(|&c| c == letter).count() as i32;
    let base = super::letter_key(letter)?;
    let octave = if letter.is_ascii_lowercase() { count - 1 } else { -count };
    let sharps = token.matches('#').count() as i32;
    let flats = token.matches('-').count() as i32;
    Some(Some((len, Some(base + 12 * octave + sharps - flats), tie)))
}

/// Section expansion: with a `*>[A,B,A]` list, the sections (from each
/// `*>A` line to the next label) in that order; without, as written.
fn expand(lines: Vec<(usize, &str)>) -> Vec<(usize, &str)> {
    let list = lines.iter().find_map(|(_, l)| {
        let f = l.split('\t').next()?;
        let inner = f.strip_prefix("*>")?.strip_prefix('[')?.strip_suffix(']')?;
        // Only the list without a name (the default expansion).
        Some(inner.split(',').map(|s| s.trim().to_string()).collect::<Vec<_>>())
    });
    let Some(list) = list else { return lines };
    let label = |l: &str| -> Option<String> {
        let f = l.split('\t').next()?;
        let name = f.strip_prefix("*>")?;
        (!name.starts_with('[') && !name.contains('[')).then(|| name.to_string())
    };
    let first = lines.iter().position(|(_, l)| label(l).is_some()).unwrap_or(lines.len());
    let mut sections: Vec<(String, Vec<(usize, &str)>)> = vec![];
    for &(o, l) in &lines[first..] {
        match label(l) {
            Some(name) => sections.push((name, vec![])),
            None => {
                if let Some(s) = sections.last_mut() {
                    s.1.push((o, l));
                }
            }
        }
    }
    let mut out: Vec<(usize, &str)> = lines[..first].iter().filter(|(_, l)| !l.split('\t').next().unwrap_or("").starts_with("*>")).copied().collect();
    // A section's spine terminators (*-) only at the very end.
    let mut tail = vec![];
    for (n, name) in list.iter().enumerate() {
        if let Some(s) = sections.iter().find(|s| &s.0 == name) {
            for &(o, l) in &s.1 {
                if l.split('\t').all(|f| f == "*-") {
                    if n + 1 == list.len() || tail.is_empty() {
                        tail = vec![(o, l)];
                    }
                    continue;
                }
                out.push((o, l));
            }
        }
    }
    out.extend(tail);
    out
}

/// A length as a `**kern` reciprocal: `4`, `8.`, `12`, `0`, or `3%2`.
fn recip(len: Frac) -> String {
    let (n, d) = (len.num(), len.den());
    match (n, d) {
        (2, 1) => "0".into(),
        (4, 1) => "00".into(),
        (1, d) => d.to_string(),
        (3, d) if d % 2 == 0 => format!("{}.", d / 2),
        (3, 1) => "0.".into(),
        (7, d) if d % 4 == 0 => format!("{}..", d / 4),
        (n, d) => format!("{d}%{n}"),
    }
}

/// A score as Humdrum `**kern`, titled.
pub fn write(score: &Score, title: &str) -> String {
    use super::notation::{notate, spell, Item};
    let n = notate(score);
    // Spines: (part, voice), the last part leftmost.
    let mut spines: Vec<(usize, usize)> = vec![];
    for (pi, p) in n.parts.iter().enumerate().rev() {
        for v in 0..p.voices.len() {
            spines.push((pi, v));
        }
    }
    let row = |cells: Vec<String>| cells.join("\t") + "\n";
    let all = |s: &str| row(vec![s.to_string(); spines.len()]);
    let mut o = String::new();
    if !title.is_empty() {
        o.push_str(&format!("!!!OTL: {}\n", super::notation::one_line(title)));
    }
    o.push_str(&all("**kern"));
    o.push_str(&row(spines
        .iter()
        .map(|&(pi, _)| {
            // A name the reader would take for another instrument isn't written.
            let p = &n.parts[pi];
            let name = if p.name.is_empty() { super::GM_NAMES[usize::from(p.program & 127)] } else { p.name.as_str() };
            // (For an instrument, the code on the next line sets it again.)
            let other = p.drums && super::program(name).is_some();
            if other {
                "*".to_string()
            } else {
                format!("*I\"{}", name.replace('\t', " "))
            }
        })
        .collect()));
    o.push_str(&row(spines
        .iter()
        .map(|&(pi, _)| {
            let p = &n.parts[pi];
            if p.drums {
                "*Iperc".to_string()
            } else {
                let code: String = super::GM_NAMES[usize::from(p.program & 127)].chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_ascii_lowercase();
                format!("*I{code}")
            }
        })
        .collect()));
    let key = |sharps: i8| {
        const SHARPS: [&str; 7] = ["f#", "c#", "g#", "d#", "a#", "e#", "b#"];
        const FLATS: [&str; 7] = ["b-", "e-", "a-", "d-", "g-", "c-", "f-"];
        let list: String = if sharps >= 0 { SHARPS[..sharps as usize].concat() } else { FLATS[..(-sharps) as usize].concat() };
        format!("*k[{list}]")
    };
    let token = |it: &Item, sharps: i8| -> String {
        let r = recip(it.len);
        if it.rest() {
            return format!("{r}r");
        }
        it.keys
            .iter()
            .map(|&k| {
                let sp = spell(k, sharps);
                let letter = sp.letter();
                let mut t = String::new();
                if it.tie_to && !it.tie_from {
                    t.push('[');
                }
                t.push_str(&r);
                let (c, count) = if sp.octave >= 4 { (letter.to_ascii_lowercase(), sp.octave - 3) } else { (letter, 4 - sp.octave) };
                for _ in 0..count.max(1) {
                    t.push(c);
                }
                for _ in 0..sp.alter.max(0) {
                    t.push('#');
                }
                for _ in 0..(-sp.alter).max(0) {
                    t.push('-');
                }
                if it.tie_from && it.tie_to {
                    t.push('_');
                } else if it.tie_from {
                    t.push(']');
                }
                t
            })
            .collect::<Vec<_>>()
            .join(" ")
    };
    for (bi, bar) in n.bars.iter().enumerate() {
        let prev = bi.checked_sub(1).map(|i| &n.bars[i]);
        if bi > 0 {
            o.push_str(&all(&format!("={}", bi + 1)));
        }
        if let Some(t) = bar.tempo {
            o.push_str(&all(&format!("*MM{}", (t * 100.0).round() / 100.0)));
        }
        if prev.map_or(true, |p| (p.beats, p.unit) != (bar.beats, bar.unit)) {
            o.push_str(&all(&format!("*M{}/{}", bar.beats, bar.unit)));
        }
        if prev.map_or(true, |p| p.sharps != bar.sharps) {
            o.push_str(&all(&key(bar.sharps)));
        }
        let mut times: Vec<Frac> = spines.iter().flat_map(|&(pi, v)| n.parts[pi].voices[v][bi].iter().map(|i| i.at)).collect();
        times.sort();
        times.dedup();
        for t in times {
            o.push_str(&row(spines
                .iter()
                .map(|&(pi, v)| n.parts[pi].voices[v][bi].iter().find(|i| i.at == t).map_or_else(|| ".".to_string(), |i| token(i, bar.sharps)))
                .collect()));
        }
    }
    o.push_str(&all("=="));
    o.push_str(&all("*-"));
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spines_split_and_join() {
        let kern = "!!!OTL: Little Chorale\n**kern\t**kern\t**text\n*ICklav\t*Iflt\t*\n*MM100\t*MM100\t*\n*M4/4\t*M4/4\t*\n4C\t4c\tHal-\n*\t*^\t*\n4D\t8e\t4g\tle-\n.\t8f\t.\t.\n2E 2G\t[2e\t2a\tlu\n*\t*\t.\t*\n=1\t=1\t=1\t=1\n*\t*v\t*v\t*\n4F\t4e]\tjah\n*-\t*-\t*-\n";
        let (score, title) = read(&kern.replace("*\t*\t.\t*\n", "")).unwrap();
        assert_eq!(title, "Little Chorale");
        assert_eq!(score.bpm, 100.0);
        let low: Vec<(u32, u32, u8)> = score.parts[0].notes.iter().map(|n| (n.at, n.length, n.key)).collect();
        assert_eq!(low, vec![(0, 480, 48), (480, 480, 50), (960, 960, 52), (960, 960, 55), (1920, 480, 53)]);
        let high: Vec<(u32, u32, u8)> = score.parts[1].notes.iter().map(|n| (n.at, n.length, n.key)).collect();
        assert_eq!(high, vec![(0, 480, 60), (480, 240, 64), (480, 480, 67), (720, 240, 65), (960, 1440, 64), (960, 960, 69)]);
        assert_eq!(score.parts[1].program, 73);
    }

    #[test]
    fn durations_and_pitches() {
        assert_eq!(note("4.cc#"), Some(Some((Frac::new(3, 8), Some(73), Tie::None))));
        assert_eq!(note("3BB-"), Some(Some((Frac::new(1, 3), Some(46), Tie::None))));
        assert_eq!(note("0r"), Some(Some((Frac::int(2), None, Tie::None))));
        assert_eq!(note("3%2e"), Some(Some((Frac::new(2, 3), Some(64), Tie::None))));
        assert_eq!(note("8qc"), Some(None));
        assert_eq!(note("xyz"), None);
    }

    #[test]
    fn expansion() {
        let kern = "**kern\n*>[A,A,B]\n*>A\n4c\n*>B\n4d\n*-\n";
        let (score, _) = read(kern).unwrap();
        assert_eq!(score.parts[0].notes.iter().map(|n| (n.at, n.key)).collect::<Vec<_>>(), vec![(0, 60), (480, 60), (960, 62)]);
    }

    #[test]
    fn errors() {
        assert!(matches!(read("4c\n"), Err(Error::BadText { line: 1, .. })));
        assert!(matches!(read("**kern\n4c\t4d\n"), Err(Error::BadText { line: 2, .. })));
        assert!(matches!(read("**kern\n4z\n"), Err(Error::BadText { line: 2, .. })));
    }
}
