// Copyright 2026 ansiapps. Neumetik MIDI, MIT licensed: see LICENSE.

//! The reference kit's JSON (`docs/rhythm-patterns-reference-kit.md`,
//! "Suggested normalised schema"): read from `pattern_grid`,
//! `time_signature` and `instrument_map`; written with the schema's
//! fields. A small JSON reader of its own (standard library only).

use super::names;
use super::read::Opts;
use super::{level_of, velocity, Pattern, ACCENT, HIT, REST};

#[derive(Clone, Debug, PartialEq)]
pub enum J {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
}

impl J {
    pub fn get(&self, key: &str) -> Option<&J> {
        match self {
            J::Obj(v) => v.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn str(&self) -> Option<&str> {
        match self {
            J::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn num(&self) -> Option<f64> {
        match self {
            J::Num(n) => Some(*n),
            J::Bool(b) => Some(f64::from(u8::from(*b))),
            _ => None,
        }
    }
}

struct P<'a> {
    s: &'a [u8],
    i: usize,
}

impl P<'_> {
    fn ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }

    fn err<T>(&self, why: &str) -> Result<T, String> {
        let line = self.s[..self.i.min(self.s.len())].iter().filter(|b| **b == b'\n').count() + 1;
        Err(format!("JSON, line {line}: {why}"))
    }

    fn value(&mut self, depth: usize) -> Result<J, String> {
        if depth > 32 {
            return self.err("nested too deep");
        }
        self.ws();
        match self.s.get(self.i) {
            None => self.err("ends too soon"),
            Some(b'{') => {
                self.i += 1;
                let mut v = Vec::new();
                loop {
                    self.ws();
                    if self.s.get(self.i) == Some(&b'}') {
                        self.i += 1;
                        return Ok(J::Obj(v));
                    }
                    if !v.is_empty() {
                        if self.s.get(self.i) != Some(&b',') {
                            return self.err("wants , or }");
                        }
                        self.i += 1;
                        self.ws();
                    }
                    let k = match self.value(depth + 1)? {
                        J::Str(k) => k,
                        _ => return self.err("a key is a string"),
                    };
                    self.ws();
                    if self.s.get(self.i) != Some(&b':') {
                        return self.err("wants :");
                    }
                    self.i += 1;
                    v.push((k, self.value(depth + 1)?));
                }
            }
            Some(b'[') => {
                self.i += 1;
                let mut v = Vec::new();
                loop {
                    self.ws();
                    if self.s.get(self.i) == Some(&b']') {
                        self.i += 1;
                        return Ok(J::Arr(v));
                    }
                    if !v.is_empty() {
                        if self.s.get(self.i) != Some(&b',') {
                            return self.err("wants , or ]");
                        }
                        self.i += 1;
                    }
                    v.push(self.value(depth + 1)?);
                }
            }
            Some(b'"') => {
                self.i += 1;
                let mut out = Vec::new();
                loop {
                    match self.s.get(self.i) {
                        None => return self.err("a string is not closed"),
                        Some(b'"') => {
                            self.i += 1;
                            return Ok(J::Str(String::from_utf8_lossy(&out).into_owned()));
                        }
                        Some(b'\\') => {
                            self.i += 1;
                            let c = self.s.get(self.i).copied().unwrap_or(b'\\');
                            match c {
                                b'n' => out.push(b'\n'),
                                b't' => out.push(b'\t'),
                                b'u' => {
                                    let h = std::str::from_utf8(self.s.get(self.i + 1..self.i + 5).unwrap_or(b"")).unwrap_or("");
                                    let ch = u32::from_str_radix(h, 16).ok().and_then(char::from_u32).unwrap_or('?');
                                    let mut b = [0u8; 4];
                                    out.extend_from_slice(ch.encode_utf8(&mut b).as_bytes());
                                    self.i += 4;
                                }
                                c => out.push(c),
                            }
                            self.i += 1;
                        }
                        Some(c) => {
                            out.push(*c);
                            self.i += 1;
                        }
                    }
                }
            }
            Some(b't') if self.s[self.i..].starts_with(b"true") => {
                self.i += 4;
                Ok(J::Bool(true))
            }
            Some(b'f') if self.s[self.i..].starts_with(b"false") => {
                self.i += 5;
                Ok(J::Bool(false))
            }
            Some(b'n') if self.s[self.i..].starts_with(b"null") => {
                self.i += 4;
                Ok(J::Null)
            }
            Some(c) if c.is_ascii_digit() || *c == b'-' => {
                let st = self.i;
                while self.i < self.s.len() && (self.s[self.i].is_ascii_digit() || b"-+.eE".contains(&self.s[self.i])) {
                    self.i += 1;
                }
                std::str::from_utf8(&self.s[st..self.i]).ok().and_then(|t| t.parse().ok()).map(J::Num).ok_or_else(|| "JSON: a bad number".to_string())
            }
            Some(_) => self.err("is not JSON"),
        }
    }
}

pub fn parse_json(text: &str) -> Result<J, String> {
    let mut p = P { s: text.as_bytes(), i: 0 };
    let v = p.value(0)?;
    p.ws();
    if p.i != p.s.len() {
        return p.err("has more after the value");
    }
    Ok(v)
}

/// The kit's schema as a pattern.
pub fn read(text: &str, o: &Opts) -> Result<Pattern, String> {
    let j = parse_json(text)?;
    let (mut beats, mut unit) = o.meter();
    if let Some(ts) = j.get("time_signature").and_then(J::str) {
        let (b, u) = ts.split_once('/').ok_or("time_signature wants a meter like 4/4")?;
        beats = b.trim().parse().map_err(|_| "time_signature: the beats are not a number")?;
        unit = u.trim().parse().map_err(|_| "time_signature: the unit is not a number")?;
        if beats == 0 || unit == 0 {
            return Err("time_signature has a zero".into());
        }
    }
    let grid = j.get("pattern_grid").ok_or("no pattern_grid")?;
    let levels = j.get("pattern_levels");
    let J::Obj(rows) = grid else { return Err("pattern_grid is an object of arrays".into()) };
    let map = j.get("instrument_map");
    let resolution = grid.get("resolution").and_then(J::num).map(|r| r as u64);
    let mut lanes: Vec<(u8, Vec<u8>)> = Vec::new();
    for (name, v) in rows {
        let J::Arr(cells) = v else { continue };
        let key = map.and_then(|m| m.get(name)).and_then(J::num).map(|k| k as u8).or_else(|| names::key_from_name(name)).ok_or_else(|| format!("'{name}' is not a drum Neumetik knows: add it to instrument_map"))?;
        let lv = levels.and_then(|l| l.get(name));
        let mut out = Vec::with_capacity(cells.len());
        for (i, c) in cells.iter().enumerate() {
            c.num().ok_or_else(|| format!("{name}[{i}] is not a number"))?;
            let n = lv.and_then(|l| match l {
                J::Arr(a) => a.get(i).and_then(J::num),
                _ => None,
            });
            let base = c.num().unwrap_or(0.0);
            out.push(match n {
                Some(vel) if base > 0.0 => level_of(vel as u8).max(1),
                _ if base <= 0.0 => REST,
                _ if base < 1.5 => HIT,
                _ if base < 2.5 => ACCENT,
                _ => level_of(base as u8).max(1),
            });
        }
        lanes.push((key, out));
    }
    let total = lanes.first().map_or(0, |l| l.1.len());
    if total == 0 {
        return Err("pattern_grid has no rows".into());
    }
    if let Some(l) = lanes.iter().find(|l| l.1.len() != total) {
        return Err(format!("a row has {} cells, the first {total}", l.1.len()));
    }
    // Cells a bar: the resolution is cells a whole note.
    let per_bar = match resolution {
        Some(r) if r > 0 && (r * u64::from(beats)) % u64::from(unit) == 0 => (r * u64::from(beats) / u64::from(unit)) as usize,
        _ => total,
    };
    let bars = j.get("bar_count").and_then(J::num).map(|b| b as usize).filter(|b| *b > 0 && total % *b == 0).unwrap_or(total / per_bar.max(1));
    if per_bar == 0 || total % per_bar != 0 {
        return Err(format!("{total} cells do not make whole bars of {per_bar}"));
    }
    let _ = bars;
    let mut p = Pattern::new(beats, unit, (total / per_bar) as u32, per_bar as u32);
    if let Some(t) = j.get("title").and_then(J::str) {
        p.title = t.to_string();
    }
    for (key, cells) in lanes {
        for (i, l) in cells.iter().enumerate() {
            if *l != REST {
                p.put(key, i as i64, *l);
            }
        }
    }
    Ok(p)
}

/// What a corpus entry says about a pattern, for the kit's fields.
pub struct Meta<'a> {
    pub id: &'a str,
    pub style: &'a str,
    pub tradition: &'a str,
    pub tags: &'a str,
    pub difficulty: u8,
    pub notes: &'a str,
    pub licence: &'a str,
    pub attribution: &'a str,
    /// (slowest, suggested, fastest) beats a minute.
    pub tempo: (u16, u16, u16),
}

fn esc(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            c if (c as u32) < 32 => o.push(' '),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// The kit's `meter_classification` for a meter.
pub fn meter_class(beats: u8, unit: u8) -> &'static str {
    match (beats, unit) {
        (2, 4) => "simple_duple",
        (3, 4) | (3, 8) => "simple_triple",
        (4, 4) => "simple_quadruple",
        (6, 8) => "compound_duple",
        (9, 8) => "compound_triple",
        (12, 8) => "compound_quadruple",
        (5, _) | (7, _) | (10, _) | (11, _) | (13, _) => "asymmetric_additive",
        (6, 4) => "simple_sextuple",
        _ => "other",
    }
}

/// The kit's `subdivision` for a pattern: the note each cell is.
pub fn subdivision(p: &Pattern) -> String {
    let per_whole = u64::from(p.steps) * u64::from(p.unit) / u64::from(p.beats);
    match per_whole {
        1 => "whole".into(),
        2 => "half".into(),
        4 => "quarter".into(),
        8 => "8th".into(),
        16 => "16th".into(),
        32 => "32nd".into(),
        12 => "8th triplet".into(),
        24 => "16th triplet".into(),
        n => format!("{n} a whole note"),
    }
}

/// A pattern as the kit's JSON, with its metadata.
pub fn write(p: &Pattern, title: &str, tempo: u16, m: &Meta) -> String {
    let lanes = p.shown();
    let mut out = String::from("{\n");
    let mut field = |k: &str, v: String| out.push_str(&format!("  \"{k}\": {v},\n"));
    field("id", esc(m.id));
    field("title", esc(title));
    field("style", esc(m.style));
    field("tradition", esc(m.tradition));
    field("time_signature", esc(&p.meter()));
    field("meter_classification", esc(meter_class(p.beats, p.unit)));
    let _ = tempo;
    field("tempo_bpm", m.tempo.1.to_string());
    field("tempo_range_bpm", format!("[{}, {}]", m.tempo.0, m.tempo.2));
    field("subdivision", esc(&subdivision(p)));
    field("bar_count", p.bars.to_string());
    field("instruments", format!("[{}]", lanes.iter().map(|l| esc(&names::name(l.key))).collect::<Vec<_>>().join(", ")));
    field("instrument_map", format!("{{{}}}", lanes.iter().map(|l| format!("{}: {}", esc(&names::name(l.key)), l.key)).collect::<Vec<_>>().join(", ")));
    let steps_whole = u64::from(p.steps) * u64::from(p.unit) / u64::from(p.beats);
    let rows: Vec<String> = lanes.iter().map(|l| format!("    {}: [{}]", esc(&names::name(l.key)), l.cells.iter().map(|c| if *c == REST { "0" } else { "1" }).collect::<Vec<_>>().join(","))).collect();
    field("pattern_grid", format!("{{\n    \"resolution\": {steps_whole},\n{}\n  }}", rows.join(",\n")));
    let vels: Vec<String> = lanes.iter().map(|l| format!("    {}: [{}]", esc(&names::name(l.key)), l.cells.iter().map(|c| velocity(*c).to_string()).collect::<Vec<_>>().join(","))).collect();
    field("pattern_levels", format!("{{\n{}\n  }}", vels.join(",\n")));
    field("source_url", "\"\"".into());
    field("source_licence", esc(m.licence));
    field("attribution", esc(m.attribution));
    field("tags", format!("[{}]", m.tags.split(' ').filter(|t| !t.is_empty()).map(esc).collect::<Vec<_>>().join(", ")));
    field("difficulty", m.difficulty.to_string());
    out.push_str(&format!("  \"notes\": {}\n}}\n", esc(m.notes)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_reads_the_usual_things() {
        let j = parse_json(r#"{"a": [1, 2.5, -3e1, true, null], "b": {"c": "x\"yA"}}"#).expect("json");
        assert_eq!(j.get("a"), Some(&J::Arr(vec![J::Num(1.0), J::Num(2.5), J::Num(-30.0), J::Bool(true), J::Null])));
        assert_eq!(j.get("b").and_then(|b| b.get("c")).and_then(J::str), Some("x\"yA"));
        assert!(parse_json("{").is_err());
        assert!(parse_json("{} x").is_err());
    }

    #[test]
    fn a_pattern_goes_out_as_the_kit_s_json_and_comes_back() {
        let mut p = Pattern::new(7, 8, 1, 14);
        for c in [0, 4, 8] {
            p.put(36, c, ACCENT);
        }
        p.put(38, 6, super::super::SOFT);
        p.put(42, 2, HIT);
        let m = Meta { id: "t", style: "Rock", tradition: "x", tags: "a b", difficulty: 1, notes: "n \"q\"", licence: "CC0", attribution: "Original pattern", tempo: (90, 100, 120) };
        let text = write(&p, "Test", 100, &m);
        assert!(text.contains("\"time_signature\": \"7/8\"") && text.contains("asymmetric_additive") && text.contains("\"resolution\": 16"));
        let q = read(&text, &Opts::default()).expect("reads back");
        assert_eq!((q.beats, q.unit, q.steps, q.bars), (7, 8, 14, 1));
        let mut a = p.clone();
        a.tidy();
        let mut b = q;
        b.tidy();
        assert_eq!(a.lanes, b.lanes);
    }
}
