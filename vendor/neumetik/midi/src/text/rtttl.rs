// Copyright 2026 ansiapps. Neumetik MIDI, MIT licensed: see LICENSE.

//! **RTTTL**, Nokia's Ring Tone Text Transfer Language: one line, a name,
//! defaults and the notes.
//!
//! ```text
//! Tune:d=4,o=5,b=125:8c,8d,e.,p,c6,8a#,2g
//! ```
//!
//! `d` is the length (1 a whole note to 32), `o` the octave (A in octave
//! 4 is 440 Hz, so `c4` is middle C), `b` quarter notes a minute. A note
//! is its length, the letter (or `p`, a rest; `h` is B), `#`, a dot, and
//! its octave; the dot may come after the octave too. Read and written.

use super::{bad, letter_key, Builder, Frac};
use crate::score::Score;
use crate::Error;

/// Whether the text looks like RTTTL: a name, a colon, `d=`/`o=`/`b=`
/// settings and a colon.
pub fn looks_like(text: &str) -> bool {
    let line = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let mut parts = line.splitn(3, ':');
    let (_, settings, notes) = (parts.next(), parts.next(), parts.next());
    match (settings, notes) {
        (Some(s), Some(_)) => {
            let s = s.trim();
            s.is_empty() || s.split(',').all(|kv| matches!(kv.trim().split_once('='), Some((k, _)) if matches!(k.trim(), "d" | "o" | "b" | "l" | "s")))
        }
        _ => false,
    }
}

/// An RTTTL tune as a score and its name.
pub fn read(text: &str) -> Result<(Score, String), Error> {
    let start = text.len() - text.trim_start().len();
    let first = text[start..].find(':').map(|i| i + start).ok_or_else(|| bad(text, start, "an RTTTL tune starts with its name and a colon"))?;
    let second = text[first + 1..].find(':').map(|i| i + first + 1).ok_or_else(|| bad(text, first + 1, "the settings (d=, o=, b=) end with a colon"))?;
    let name = text[start..first].trim().to_string();
    let (mut d, mut o, mut b) = (4i64, 6i32, 63.0f64);
    let mut at = first + 1;
    for kv in text[first + 1..second].split(',') {
        let here = at;
        at += kv.len() + 1;
        if kv.trim().is_empty() {
            continue;
        }
        let Some((k, v)) = kv.split_once('=') else { return Err(bad(text, here, "a setting is a letter, =, and a number")) };
        let v: f64 = v.trim().parse().map_err(|_| bad(text, here, "a setting's value is a number"))?;
        match k.trim() {
            "d" => d = v as i64,
            "o" => o = v as i32,
            "b" => b = v,
            _ => {}
        }
    }
    if !matches!(d, 1 | 2 | 4 | 8 | 16 | 32 | 64) {
        d = 4;
    }
    let mut builder = Builder::new();
    builder.title = name.clone();
    builder.tempo(Frac::ZERO, b);
    let part = builder.part(if name.is_empty() { "Tune" } else { &name }, 80, false);
    let mut now = Frac::ZERO;
    let mut offset = second + 1;
    let body = &text[second + 1..];
    for token in body.split(',') {
        let here = offset;
        offset += token.len() + 1;
        let t = token.trim();
        if t.is_empty() {
            continue;
        }
        let bytes = t.as_bytes();
        let mut i = 0;
        let mut length = d;
        let digits = bytes.iter().take_while(|c| c.is_ascii_digit()).count();
        if digits > 0 {
            length = t[..digits].parse().unwrap_or(d);
            i = digits;
        }
        let letter = *bytes.get(i).ok_or_else(|| bad(text, here, "a note's letter is missing"))? as char;
        i += 1;
        let mut key = match letter.to_ascii_lowercase() {
            'p' => None,
            'h' => Some(71),
            l => Some(letter_key(l).ok_or_else(|| bad(text, here, "a note is a to g, h, or p for a rest"))?),
        };
        if bytes.get(i) == Some(&b'#') {
            key = key.map(|k| k + 1);
            i += 1;
        }
        let mut dotted = false;
        if bytes.get(i) == Some(&b'.') {
            dotted = true;
            i += 1;
        }
        let mut octave = o;
        if let Some(c) = bytes.get(i).filter(|c| c.is_ascii_digit()) {
            octave = i32::from(c - b'0');
            i += 1;
        }
        if bytes.get(i) == Some(&b'.') {
            dotted = true;
            i += 1;
        }
        if i != bytes.len() {
            return Err(bad(text, here + (token.len() - token.trim_start().len()) + i, "something after a note that RTTTL doesn't have"));
        }
        if !matches!(length, 1 | 2 | 4 | 8 | 16 | 32 | 64) {
            return Err(bad(text, here, "a length is 1, 2, 4, 8, 16 or 32"));
        }
        let mut len = Frac::new(1, length);
        if dotted {
            len = len.scale(3, 2);
        }
        if let Some(k) = key {
            builder.note(part, now, len, k + 12 * (octave - 4), 100);
        }
        now = now.add(len);
    }
    builder.end_at(now);
    Ok((builder.finish(), name))
}

/// A part's top line (the highest note sounding at each start), as
/// (start, length, key) in ticks.
pub(crate) fn top_line(score: &Score) -> Vec<(u32, u32, u8)> {
    let Some(part) = score.parts.iter().find(|p| p.channel != crate::score::DRUMS && !p.notes.is_empty()) else { return vec![] };
    let mut notes = part.notes.clone();
    notes.sort_by_key(|n| (n.at, std::cmp::Reverse(n.key)));
    let mut line: Vec<(u32, u32, u8)> = vec![];
    for n in notes {
        if let Some(last) = line.last_mut() {
            if n.at == last.0 {
                continue;
            }
            if n.at < last.0 + last.1 {
                last.1 = n.at - last.0;
            }
        }
        line.push((n.at, n.length, n.key));
    }
    line
}

/// The score's top line as RTTTL: its first part's highest notes, each
/// length the nearest RTTTL has (a whole note to a 32nd, dotted or not).
pub fn write(score: &Score, name: &str) -> String {
    let name: String = name.chars().filter(|c| *c != ':' && *c != ',').take(64).collect();
    let q = f64::from(score.ppq);
    let bpm = score.bpm.round().clamp(25.0, 900.0) as u32;
    let mut out = format!("{}:d=4,o=5,b={bpm}:", if name.is_empty() { "Tune" } else { &name });
    let mut items: Vec<String> = vec![];
    let mut now = 0u32;
    let lengths = |ticks: u32| -> Vec<String> {
        // Whole notes, then nearest lengths until the time's filled.
        let mut left = f64::from(ticks) / (q * 4.0);
        let mut out = vec![];
        let choices: Vec<(f64, String)> = [1, 2, 4, 8, 16, 32]
            .iter()
            .flat_map(|&d| [(1.0 / d as f64 * 1.5, format!("{d}.")), (1.0 / d as f64, format!("{d}"))])
            .filter(|c| c.1 != "32.")
            .collect();
        while left > 1.0 / 64.0 && out.len() < 64 {
            let pick = choices.iter().find(|c| c.0 <= left + 1.0 / 128.0).unwrap_or(&choices[choices.len() - 1]);
            out.push(pick.1.clone());
            left -= pick.0;
        }
        out
    };
    let push = |items: &mut Vec<String>, len: &str, what: &str| {
        let (d, dot) = len.split_once('.').map(|(d, _)| (d, ".")).unwrap_or((len, ""));
        let d = if d == "4" { "" } else { d };
        items.push(format!("{d}{what}{dot}"));
    };
    for (at, len, key) in top_line(score) {
        if at > now {
            for l in lengths(at - now) {
                push(&mut items, &l, "p");
            }
        }
        let mut k = i32::from(key);
        while k < 60 {
            k += 12;
        }
        while k > 107 {
            k -= 12;
        }
        let names = ["c", "c#", "d", "d#", "e", "f", "f#", "g", "g#", "a", "a#", "b"];
        let octave = (k / 12 - 1).clamp(4, 7);
        let note = format!("{}{}", names[(k % 12) as usize], if octave == 5 { String::new() } else { octave.to_string() });
        let ls = lengths(len);
        if ls.is_empty() {
            continue;
        }
        // RTTTL has no ties: the first length sounds, the rest rest.
        for (i, l) in ls.iter().enumerate() {
            push(&mut items, l, if i == 0 { &note } else { "p" });
        }
        now = at + len;
    }
    out.push_str(&items.join(","));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::score::Note;

    #[test]
    fn reads_a_tune() {
        let (score, name) = read("Tune:d=4,o=5,b=120:8c,8d,e.,p,c6,8a#,2g,16f#.4").unwrap();
        assert_eq!(name, "Tune");
        assert_eq!(score.bpm, 120.0);
        let n = &score.parts[0].notes;
        assert_eq!(n[0], Note { at: 0, length: 240, key: 72, velocity: 100 });
        assert_eq!(n[2], Note { at: 480, length: 720, key: 76, velocity: 100 });
        assert_eq!(n[3], Note { at: 480 + 720 + 480, length: 480, key: 84, velocity: 100 });
        assert_eq!(n[4].key, 82);
        assert_eq!(n[6], Note { at: 3360, length: 180, key: 66, velocity: 100 });
    }

    #[test]
    fn defaults_and_errors() {
        let (score, _) = read("x::c,d").unwrap();
        assert_eq!((score.bpm, score.parts[0].notes[0].key), (63.0, 84));
        assert_eq!(read("x:d=4:q"), Err(Error::BadText { line: 1, column: 7, why: "a note is a to g, h, or p for a rest" }));
        assert!(looks_like("Simpsons:d=4,o=5,b=160:c.6,e6"));
        assert!(!looks_like("http://example.com"));
    }

    #[test]
    fn writes_what_it_reads() {
        let tune = "Tune:d=4,o=5,b=120:8c,8d,e.,p,c6,8a#,2g,16f#.4";
        let (score, _) = read(tune).unwrap();
        let text = write(&score, "Tune");
        assert_eq!(text, "Tune:d=4,o=5,b=120:8c,8d,e.,p,c6,8a#,2g,16f#4.");
        let (again, _) = read(&text).unwrap();
        assert_eq!(again.parts[0].notes, score.parts[0].notes);
    }
}
