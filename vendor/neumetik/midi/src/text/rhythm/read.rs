// Copyright 2026 ansiapps. Neumetik MIDI, MIT licensed: see LICENSE.

//! Reading rhythm in every notation (`super::Notation`) into a
//! [`Pattern`]. [`parse`] takes text and a notation (or finds it with
//! [`detect`]); [`parse_bytes`] takes a file (ABC, MusicXML, `**kern`,
//! MIDI...) through Neumetik.
//!
//! A text may start with directives, a line each, that tell what the
//! notation can't:
//!
//! | Directive | Meaning |
//! |---|---|
//! | `@time 7/8` | the meter (else 4/4, or what the cell count says) |
//! | `@bars 2` | the bars the cells make (else `|` marks them, see below) |
//! | `@steps 16` | cells a bar, for notations that have none of their own |
//! | `@per 4` | cells a pulse, for bols and stickings |
//! | `@lane snare` | the drum a notation with no drum in it plays (else claves) |
//! | `@title Name` | the pattern's name |
//! | `@from 2` | the bar a score is read from |
//! | `@bols konnakol` | read bols as konnakol (else tabla, or found by the words) |
//!
//! In a grid or tab, `|` marks bars, unless there are exactly as many of
//! them as the meter has beats (then they mark beats). `@bars` settles it.

use super::json;
use super::names;
use super::{level_of, Notation, Pattern, ACCENT, HIT, SOFT};

/// What a text says about how to read it, set by `@` lines or the caller.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Opts {
    /// 0 when not said.
    pub beats: u8,
    pub unit: u8,
    pub bars: u32,
    pub steps: u32,
    pub per: u32,
    /// The drum a notation with none of its own plays.
    pub lane: u8,
    pub from: u32,
    pub title: String,
    pub konnakol: bool,
}

impl Default for Opts {
    fn default() -> Opts {
        Opts { beats: 0, unit: 0, bars: 0, steps: 0, per: 0, lane: 75, from: 0, title: String::new(), konnakol: false }
    }
}

impl Opts {
    pub fn meter_set(&self) -> bool {
        self.beats > 0 && self.unit > 0
    }

    pub fn meter(&self) -> (u8, u8) {
        if self.meter_set() {
            (self.beats, self.unit)
        } else {
            (4, 4)
        }
    }
}

type R<T> = Result<T, String>;

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

fn lcm(a: u64, b: u64) -> u64 {
    if a == 0 || b == 0 {
        a.max(b)
    } else {
        a / gcd(a, b) * b
    }
}

/// A fraction of a whole note or a pulse, in lowest terms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Fr(u64, u64);

impl Fr {
    fn new(n: u64, d: u64) -> Fr {
        let g = gcd(n, d).max(1);
        Fr(n / g, d / g)
    }

    fn add(self, o: Fr) -> Fr {
        Fr::new(self.0 * o.1 + o.0 * self.1, self.1 * o.1)
    }

    fn mul(self, o: Fr) -> Fr {
        Fr::new(self.0 * o.0, self.1 * o.1)
    }
}

/// The meter a count of cells in a bar suggests, when none was said.
fn guess_meter(steps: u32) -> (u8, u8) {
    match steps {
        12 => (12, 8),
        6 => (6, 8),
        9 => (9, 8),
        7 | 14 => (7, 8),
        3 => (3, 4),
        5 => (5, 4),
        _ => (4, 4),
    }
}

/// Splits the `@` lines off a text and applies them.
pub fn directives(text: &str, base: &Opts) -> R<(Opts, String)> {
    let mut o = base.clone();
    let mut body = String::new();
    for line in text.lines() {
        let t = line.trim();
        if let Some(d) = t.strip_prefix('@') {
            let (k, v) = d.split_once(char::is_whitespace).map_or((d, ""), |(k, v)| (k, v.trim()));
            let num = |what: &str| v.parse::<u32>().map_err(|_| format!("@{what} wants a number, not '{v}'"));
            match k.to_ascii_lowercase().as_str() {
                "time" | "meter" => {
                    let (b, u) = v.split_once('/').ok_or("@time wants a meter like 7/8")?;
                    let (b, u): (u8, u8) = (b.trim().parse().map_err(|_| "@time: the beats are not a number")?, u.trim().parse().map_err(|_| "@time: the unit is not a number")?);
                    if b == 0 || !matches!(u, 1 | 2 | 4 | 8 | 16 | 32) {
                        return Err(format!("@time {b}/{u} is not a meter (the unit is 1, 2, 4, 8, 16 or 32)"));
                    }
                    (o.beats, o.unit) = (b, u);
                }
                "bars" => o.bars = num("bars")?.clamp(1, 64),
                "steps" => o.steps = num("steps")?.clamp(1, 192),
                "per" => o.per = num("per")?.clamp(1, 48),
                "from" => o.from = num("from")?,
                "title" | "name" => o.title = v.to_string(),
                "lane" | "drum" => o.lane = names::key_from_name(v).ok_or_else(|| format!("@lane: '{v}' is not a drum"))?,
                "bols" => o.konnakol = matches!(v.to_ascii_lowercase().as_str(), "konnakol" | "solkattu"),
                other => return Err(format!("unknown directive @{other}")),
            }
        } else {
            body.push_str(line);
            body.push('\n');
        }
    }
    Ok((o, body))
}

/// Reads a text in a notation, or in the one [`detect`] finds.
pub fn parse(text: &str, notation: Option<Notation>, base: &Opts) -> R<Pattern> {
    let (o, body) = directives(text, base)?;
    let n = match notation {
        Some(n) => n,
        None => detect(&body).ok_or("this doesn't look like any rhythm notation Neumetik reads")?,
    };
    let mut p = match n {
        Notation::Lanes => lanes(&body, &o)?,
        Notation::Onsets => onsets(&body, &o)?,
        Notation::Intervals => intervals(&body, &o)?,
        Notation::Euclid => euclid(&body, &o)?,
        Notation::Bols => bols(&body, &o)?,
        Notation::Syllables => syllables(&body, &o)?,
        Notation::Counts => counts(&body, &o)?,
        Notation::Values => values(&body, &o)?,
        Notation::Lilypond => lilypond(&body, &o)?,
        Notation::Sticking => sticking(&body, &o)?,
        Notation::Drumscript => drumscript(&body, &o)?,
        Notation::Hydrogen => super::hydrogen::read(&body, &o)?,
        Notation::Json => json::read(&body, &o)?,
        Notation::Mini => mini(&body, &o)?,
        Notation::Score => parse_bytes(body.as_bytes(), None, &o)?,
    };
    if p.title.is_empty() {
        p.title = o.title.clone();
    }
    p.tidy();
    if p.hits() == 0 {
        return Err("there is nothing in it: no hits".into());
    }
    Ok(p)
}

/// Reads a file's bytes: MIDI, or any text format Neumetik reads (ABC,
/// MusicXML, `**kern`, MML and the rest), by its extension or contents.
pub fn parse_bytes(bytes: &[u8], extension: Option<&str>, base: &Opts) -> R<Pattern> {
    let (o, body);
    let bytes = if bytes.first() == Some(&b'@') {
        let (oo, b) = directives(&String::from_utf8_lossy(bytes), base)?;
        o = oo;
        body = b;
        body.as_bytes()
    } else {
        o = base.clone();
        bytes
    };
    let score = if bytes.starts_with(b"MThd") || bytes.starts_with(b"RIFF") {
        crate::Score::from_midi(bytes).map_err(|e| format!("not a MIDI file: {e:?}"))?
    } else {
        crate::text::read(bytes, extension).map_err(|e| format!("{e}"))?.0
    };
    let mut p = from_score(&score, &o)?;
    p.tidy();
    if p.hits() == 0 {
        return Err("there are no notes in it".into());
    }
    Ok(p)
}

// ---------------------------------------------------------------------
// Detecting

const CELL_CHARS: &str = "xXoO#@gGfdb.-_~|0123456789 \t\u{d7}\u{b7}\u{2022}\u{25cf}\u{25cb}";

fn is_counting_word(t: &str) -> bool {
    matches!(t.to_ascii_lowercase().as_str(), "e" | "&" | "a" | "+" | "and" | "ti" | "ta" | "ka" | "di" | "mi" | "la" | "li" | "trip" | "let" | "ee" | "an" | "ah") || t.parse::<u32>().is_ok_and(|n| n <= 16)
}

/// The notation a text looks like, or None.
pub fn detect(text: &str) -> Option<Notation> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    if t.starts_with('{') && (t.contains("pattern_grid") || t.contains("time_signature")) {
        return Some(Notation::Json);
    }
    if t.contains("\\drummode") || t.contains("DrumStaff") || (t.contains("\\time") && t.contains('{')) {
        return Some(Notation::Lilypond);
    }
    if t.starts_with('<') && (t.contains("<drumkit_pattern") || (t.contains("<patternList") && t.contains("<noteList"))) {
        return Some(Notation::Hydrogen);
    }
    if t.starts_with("<?xml") || t.starts_with("<score-") || t.contains("**kern") || t.starts_with("MThd") || (t.lines().any(|l| l.trim_start().starts_with("X:")) && t.lines().any(|l| l.trim_start().starts_with("K:"))) {
        return Some(Notation::Score);
    }
    if looks_like_drumscript(t) {
        return Some(Notation::Drumscript);
    }
    let first = t.lines().find(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))?.trim();
    let lower = first.to_ascii_lowercase();
    let after_label = |l: &str| -> String { l.split_once(':').filter(|(a, _)| a.chars().all(|c| c.is_ascii_alphanumeric() || c == ' ' || c == '-')).map_or(l.to_string(), |(_, b)| b.trim().to_string()) };
    let body = after_label(&lower);
    if body.starts_with("e(") || body.starts_with("euclid") || body.starts_with("bjorklund") || is_euclid_pair(&body) {
        return Some(Notation::Euclid);
    }
    if lower.starts_with("tala") || lower.starts_with("konnakol") || lower.starts_with("solkattu") || bol_share(t) >= 0.6 {
        return Some(Notation::Bols);
    }
    // Rows of cells (a label then a bar line or colon, or a bare row).
    let lane_rows = t.lines().filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#')).filter(|l| looks_like_lane_row(l)).count();
    let rows = t.lines().filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#')).count();
    if lane_rows > 0 && lane_rows * 2 >= rows {
        return Some(Notation::Lanes);
    }
    if t.contains('{') || t.contains("~") || t.contains('[') || t.contains('<') || t.contains('*') || t.contains("s(") || t.contains("sound(") || t.contains("note(") || t.contains("stack(") || t.contains("$:") {
        if body.contains('{') && body.contains('}') && body.contains('/') && body.chars().filter(|c| c.is_ascii_alphabetic()).count() == 0 {
            return Some(Notation::Onsets);
        }
        return Some(Notation::Mini);
    }
    let words: Vec<&str> = body.split(|c: char| c.is_whitespace() || c == ',').filter(|w| !w.is_empty()).collect();
    if lower.starts_with("ioi") || lower.starts_with("intervals") || lower.starts_with("additive") || (body.contains('+') && body.chars().all(|c| c.is_ascii_digit() || c == '+' || c == ' ' || c == '-')) {
        return Some(Notation::Intervals);
    }
    if body.contains('/') && body.split('/').next().is_some_and(|a| a.chars().all(|c| c.is_ascii_digit() || c == ' ' || c == ',' || c == '!' || c == '{')) {
        return Some(Notation::Onsets);
    }
    if !words.is_empty() && words.iter().all(|w| w.chars().all(|c| "RLrlK()>.-/|".contains(c))) && words.iter().any(|w| w.chars().any(|c| "RLrl".contains(c))) {
        return Some(Notation::Sticking);
    }
    if !words.is_empty() && words.iter().all(|w| is_counting_word(w)) && words.iter().any(|w| w.parse::<u32>().is_ok()) && words.iter().any(|w| !w.chars().all(|c| c.is_ascii_digit())) {
        return Some(Notation::Counts);
    }
    let spaced = body.replace(['(', ')', '[', ']'], " ");
    let vwords: Vec<&str> = spaced.split_whitespace().collect();
    if !vwords.is_empty() && vwords.iter().all(|w| value_token(w).is_some() || *w == "|" || *w == "_" || w.parse::<u32>().is_ok_and(|n| (3..=13).contains(&n))) && vwords.iter().any(|w| value_token(w).is_some_and(|_| !w.chars().all(|c| c.is_ascii_digit()))) {
        return Some(Notation::Values);
    }
    if !words.is_empty() && words.iter().all(|w| syllable_group(w).is_some() || *w == "|") {
        return Some(Notation::Syllables);
    }
    if !words.is_empty() && words.iter().all(|w| w.parse::<u32>().is_ok_and(|n| n < 200) || w.trim_end_matches('!').parse::<u32>().is_ok()) {
        return Some(Notation::Onsets);
    }
    None
}

/// Whether a text is drumscript: every line a meter, a block's edge, or
/// words that are drums with lengths (`bd4`, `[hh8] * 8`, `r4`) or blocks,
/// and a `---` mapping at the end if there is one.
fn looks_like_drumscript(t: &str) -> bool {
    let body = t.split("\n---").next().unwrap_or(t);
    let mut notes = 0;
    let mut heads: Vec<String> = Vec::new();
    for l in body.lines() {
        let l = l.split('#').next().unwrap_or("").trim();
        if let Some((h, _)) = l.split_once('(').filter(|(h, _)| ds_is_name(h.trim())) {
            heads.push(h.trim().to_ascii_lowercase());
        }
    }
    for l in body.lines() {
        let l = l.split('#').next().unwrap_or("").trim();
        let l = l.split_once('(').filter(|(h, _)| ds_is_name(h.trim())).map_or(l, |(_, r)| r).trim().trim_end_matches(')').trim();
        if l.is_empty() || l == "---" {
            continue;
        }
        if l.split_once('/').is_some_and(|(a, b)| a.trim().parse::<u8>().is_ok() && b.trim().parse::<u8>().is_ok()) {
            continue;
        }
        for w in ds_words(l) {
            if matches!(w.as_str(), "[" | "]" | "*") || w.parse::<u32>().is_ok() || heads.contains(&w.to_ascii_lowercase()) {
                continue;
            }
            match ds_note_word(&w) {
                Some((n, _)) if n.eq_ignore_ascii_case("r") => {}
                Some((n, _)) if ds_key(n, &[]).is_some() => notes += 1,
                _ => return false,
            }
        }
    }
    notes > 0
}

fn is_euclid_pair(s: &str) -> bool {
    let t = s.trim().trim_start_matches('(').trim_end_matches(')');
    let parts: Vec<&str> = t.split(',').map(str::trim).collect();
    (parts.len() == 2 || parts.len() == 3) && parts.iter().all(|p| p.parse::<i64>().is_ok()) && s.trim().starts_with('(')
}

fn looks_like_lane_row(l: &str) -> bool {
    let l = l.trim();
    let sep = l.find(['|', ':', '=']);
    let (label, rest) = match sep {
        Some(i) if i > 0 => (l[..i].trim(), &l[i + 1..]),
        Some(_) => ("", &l[1..]),
        None => ("", l),
    };
    let rest = rest.trim();
    if rest.is_empty() {
        return false;
    }
    let cells_ok = rest.chars().all(|c| CELL_CHARS.contains(c)) || is_hex_text(rest);
    if !cells_ok {
        return false;
    }
    // A row of digits and spaces on its own is an onset list, not cells.
    let only_digits = rest.chars().all(|c| c.is_ascii_digit() || c.is_whitespace() || c == '|');
    if label.is_empty() {
        return rest.chars().any(|c| "xXoO#@\u{d7}\u{2022}\u{25cf}\u{25cb}".contains(c)) && !only_digits;
    }
    if only_digits && rest.split_whitespace().any(|w| w.len() > 1 && w.chars().any(|c| c > '1')) {
        return false;
    }
    if only_digits && rest.split_whitespace().count() > 1 && rest.split_whitespace().any(|w| w.parse::<u32>().is_ok_and(|n| n > 9)) {
        return false;
    }
    names::key_from_name(label).is_some() || rest.chars().any(|c| "xXoO#@\u{d7}".contains(c)) || is_counting_header(label, rest)
}

fn is_hex_text(s: &str) -> bool {
    let t = s.trim();
    ((t.starts_with("0x") || t.starts_with("0X")) && t.len() > 2 && t[2..].chars().all(|c| c.is_ascii_hexdigit() || c == ' '))
        || (t.len() > 2 && (t.ends_with('h') || t.ends_with('H')) && t[..t.len() - 1].chars().all(|c| c.is_ascii_hexdigit()) && t[..t.len() - 1].chars().any(|c| c.is_ascii_alphabetic() || c > '1'))
}

fn is_counting_header(label: &str, rest: &str) -> bool {
    let toks: Vec<&str> = rest.split_whitespace().collect();
    names::key_from_name(label).is_none() && toks.len() >= 2 && toks.iter().all(|t| is_counting_word(t))
}

// ---------------------------------------------------------------------
// Lanes: drum grids, drum tabs, step strings, box notation

fn norm_cell(c: char) -> char {
    match c {
        '\u{d7}' | '\u{2022}' | '\u{25cf}' => 'x',
        '\u{25cb}' => 'o',
        '\u{b7}' => '.',
        c => c,
    }
}

fn cell_level(c: char, tab: bool) -> Option<u8> {
    Some(match c {
        'X' | '#' | '@' | 'A' => ACCENT,
        'O' => ACCENT,
        'x' => HIT,
        'o' => {
            if tab {
                HIT
            } else {
                SOFT
            }
        }
        'g' | 'G' => SOFT,
        'f' | 'd' | 'b' => HIT,
        '.' | '-' | '_' | '~' | ' ' | '\t' | '0' => 0,
        _ => return None,
    })
}

/// One row's cells as bar segments: a cell a number 0 to 3.
fn decode_cells(raw: &str, tab: bool) -> R<Vec<Vec<u8>>> {
    let raw = raw.trim();
    // Hexadecimal: 0x8888, or 8888h.
    let hex = raw.strip_prefix("0x").or_else(|| raw.strip_prefix("0X")).or_else(|| raw.strip_suffix('h').or_else(|| raw.strip_suffix('H')).filter(|s| s.chars().all(|c| c.is_ascii_hexdigit()) && s.chars().any(|c| c.is_ascii_alphabetic() || c > '1')));
    if let Some(h) = hex {
        let h: String = h.chars().filter(|c| !c.is_whitespace() && *c != '_').collect();
        if h.is_empty() || !h.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(format!("'{raw}' is not hexadecimal"));
        }
        let mut cells = Vec::new();
        for c in h.chars() {
            let v = c.to_digit(16).unwrap_or(0);
            for b in (0..4).rev() {
                cells.push(if v >> b & 1 == 1 { HIT } else { 0 });
            }
        }
        return Ok(vec![cells]);
    }
    let mut segs: Vec<Vec<u8>> = Vec::new();
    for seg in raw.split('|') {
        let seg: String = seg.chars().filter(|c| !c.is_whitespace()).map(norm_cell).collect();
        if seg.is_empty() {
            continue;
        }
        let digits = seg.chars().all(|c| c.is_ascii_digit());
        let binary = seg.chars().all(|c| c == '0' || c == '1');
        let cells: Vec<u8> = if binary && seg.len() > 1 {
            seg.chars().map(|c| if c == '1' { HIT } else { 0 }).collect()
        } else if digits && seg.len() > 1 {
            // Velocity digits: 1 to 3 soft, 4 to 7 a hit, 8 and 9 an accent.
            seg.chars().map(|c| match c { '0' => 0, '1'..='3' => SOFT, '4'..='7' => HIT, _ => ACCENT }).collect()
        } else {
            let mut v = Vec::with_capacity(seg.len());
            for c in seg.chars() {
                match cell_level(c, tab) {
                    Some(l) => v.push(l),
                    None => return Err(format!("'{c}' is not a mark a grid uses (x X o . - are)")),
                }
            }
            v
        };
        segs.push(cells);
    }
    if segs.is_empty() {
        return Err("a row with no cells".into());
    }
    Ok(segs)
}

struct Row {
    key: Option<u8>,
    segs: Vec<Vec<u8>>,
}

/// Drum grids, drum tabs, step strings (binary, velocity digits,
/// hexadecimal) and box notation: a row a drum.
pub fn lanes(text: &str, o: &Opts) -> R<Pattern> {
    let mut rows: Vec<Row> = Vec::new();
    let mut header: Option<usize> = None;
    for line in text.lines() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') || l.starts_with("//") {
            continue;
        }
        let sep = l.find(['|', ':', '=']).filter(|i| *i > 0);
        let (label, rest, tab) = match sep {
            Some(i) => {
                let lab = l[..i].trim();
                let tab = l.as_bytes()[i] == b'|' && lab.len() == i && l.trim_end().ends_with('|');
                (Some(lab), l[i + 1..].trim(), tab)
            }
            None => (None, l.trim_start_matches('|'), false),
        };
        if let Some(lab) = label {
            let key = names::key_from_name(lab);
            if key.is_none() {
                if is_counting_header(lab, rest) {
                    header = Some(rest.split_whitespace().count());
                    continue;
                }
                return Err(format!("'{lab}' is not a drum Neumetik knows (try kick, snare, hat, openhat, ride, crash, tom...)"));
            }
            rows.push(Row { key, segs: decode_cells(rest, tab)? });
        } else if rest.split_whitespace().all(is_counting_word) && rest.split_whitespace().count() >= 2 {
            header = Some(rest.split_whitespace().count());
        } else {
            rows.push(Row { key: None, segs: decode_cells(rest, false)? });
        }
    }
    if rows.is_empty() {
        return Err("no row of cells found".into());
    }
    // How many bars: the `|`s, unless there are as many as beats.
    let (mut beats, mut unit) = o.meter();
    let segs = rows[0].segs.len();
    let total: usize = rows[0].segs.iter().map(Vec::len).sum();
    if !o.meter_set() {
        if let Some(h) = header {
            // A counting header: "1 e & a 2 e & a" is 2 beats of 4.
            let beats_in = rows_header_beats(text);
            if beats_in > 0 && h % beats_in == 0 {
                (beats, unit) = (beats_in as u8, 4);
            }
        }
    }
    let bars = if o.bars > 0 {
        o.bars as usize
    } else if segs == 1 || segs == usize::from(beats) {
        1
    } else if rows[0].segs.iter().all(|s| s.len() == rows[0].segs[0].len()) {
        segs
    } else {
        1
    };
    if total % bars != 0 {
        return Err(format!("{total} cells do not make {bars} equal bars"));
    }
    let steps = total / bars;
    if !o.meter_set() && header.is_none() {
        (beats, unit) = guess_meter(steps as u32);
    }
    if o.steps > 0 && o.steps as usize != steps {
        return Err(format!("@steps {} but the rows have {steps} cells a bar", o.steps));
    }
    let mut p = Pattern::new(beats, unit, bars as u32, steps as u32);
    for r in &rows {
        let n: usize = r.segs.iter().map(Vec::len).sum();
        if n != total {
            let name = r.key.map_or("a row".into(), names::name);
            return Err(format!("{name} has {n} cells, the first row {total}"));
        }
        let key = r.key.unwrap_or(o.lane);
        let flat: Vec<u8> = r.segs.concat();
        for (i, l) in flat.iter().enumerate() {
            if *l != 0 {
                p.put(key, i as i64, *l);
            }
        }
    }
    Ok(p)
}

fn rows_header_beats(text: &str) -> usize {
    for line in text.lines() {
        let l = line.trim();
        let rest = l.split_once(['|', ':']).map_or(l, |(_, r)| r);
        let toks: Vec<&str> = rest.split_whitespace().collect();
        if toks.len() >= 2 && toks.iter().all(|t| is_counting_word(t)) && toks.iter().any(|t| t.parse::<u32>().is_ok()) {
            return toks.iter().filter_map(|t| t.parse::<usize>().ok()).max().unwrap_or(0);
        }
    }
    0
}

// ---------------------------------------------------------------------
// Onsets, intervals, Euclid

fn strip_label(line: &str) -> (Option<u8>, &str) {
    if let Some((a, b)) = line.split_once(':') {
        let a = a.trim();
        if let Some(k) = names::key_from_name(a) {
            return (Some(k), b);
        }
    }
    (None, line)
}

/// A cycle length for a set of onsets: the first of the usual lengths
/// that holds the last onset.
fn usual_length(max: u32) -> u32 {
    [4u32, 8, 12, 16, 24, 32, 48, 64, 96, 128, 192].into_iter().find(|n| *n > max).unwrap_or(max + 1)
}

fn finish_cycle(lanes: Vec<(u8, Vec<(u32, u8)>)>, n: u32, o: &Opts) -> R<Pattern> {
    let bars = o.bars.max(1);
    if n % bars != 0 {
        return Err(format!("{n} steps do not make {bars} equal bars"));
    }
    let steps = n / bars;
    let (b, u) = if o.meter_set() { o.meter() } else { guess_meter(steps) };
    let mut p = Pattern::new(b, u, bars, steps);
    for (key, hits) in lanes {
        for (at, level) in hits {
            if at >= n {
                return Err(format!("an onset at {at} is past the cycle's {n} steps"));
            }
            p.put(key, i64::from(at), level);
        }
    }
    Ok(p)
}

/// Onset sets: `{0,3,6,10,12}/16`, `kick: 0 4 8 12 / 16`, `4! 10`.
pub fn onsets(text: &str, o: &Opts) -> R<Pattern> {
    let mut lanes: Vec<(u8, Vec<(u32, u8)>)> = Vec::new();
    let mut len: Option<u32> = None;
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let (key, rest) = strip_label(line);
        let (set, n) = match rest.rsplit_once('/') {
            Some((a, b)) => (a, Some(b.trim().parse::<u32>().map_err(|_| format!("'{}' is not a cycle length", b.trim()))?)),
            None => (rest, None),
        };
        if let Some(n) = n {
            if len.is_some_and(|l| l != n) {
                return Err("the rows have different cycle lengths".into());
            }
            len = Some(n);
        }
        let mut hits = Vec::new();
        for tok in set.split(|c: char| c.is_whitespace() || "{}[],".contains(c)).filter(|t| !t.is_empty()) {
            let (num, level) = match tok.strip_suffix('!') {
                Some(t) => (t, ACCENT),
                None => (tok.strip_suffix('?').unwrap_or(tok), if tok.ends_with('?') { SOFT } else { HIT }),
            };
            hits.push((num.parse::<u32>().map_err(|_| format!("'{tok}' is not an onset"))?, level));
        }
        lanes.push((key.unwrap_or(o.lane), hits));
    }
    if lanes.is_empty() {
        return Err("no onsets found".into());
    }
    let max = lanes.iter().flat_map(|l| l.1.iter().map(|h| h.0)).max().unwrap_or(0);
    let n = len.unwrap_or_else(|| if o.steps > 0 { o.steps * o.bars.max(1) } else { usual_length(max) });
    finish_cycle(lanes, n, o)
}

/// Inter-onset intervals and additive meters: `3+3+4+2+4`.
pub fn intervals(text: &str, o: &Opts) -> R<Pattern> {
    let mut lanes = Vec::new();
    let mut len: Option<u32> = None;
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let (key, rest) = strip_label(line);
        let rest = rest.trim_start_matches(|c: char| c.is_ascii_alphabetic() || c == ':' || c == ' ');
        let nums: Vec<u32> = rest.split(|c: char| !c.is_ascii_digit()).filter(|t| !t.is_empty()).map(|t| t.parse().unwrap_or(0)).collect();
        if nums.is_empty() || nums.contains(&0) {
            return Err(format!("'{line}' is not a list of intervals (3+3+4+2+4)"));
        }
        let n: u32 = nums.iter().sum();
        if len.is_some_and(|l| l != n) {
            return Err("the rows add up to different cycle lengths".into());
        }
        len = Some(n);
        let mut at = 0;
        let mut hits = Vec::new();
        for (i, d) in nums.iter().enumerate() {
            hits.push((at, if i == 0 { ACCENT } else { HIT }));
            at += d;
        }
        lanes.push((key.unwrap_or(o.lane), hits));
    }
    if lanes.is_empty() {
        return Err("no intervals found".into());
    }
    finish_cycle(lanes, len.unwrap_or(1), o)
}

/// Bjorklund's Euclidean rhythms: `E(5,8)`, `E(3,8,2)`, `(3,8)`, `euclid 5 8 rot 2`.
pub fn euclid(text: &str, o: &Opts) -> R<Pattern> {
    let mut lanes = Vec::new();
    let mut len: Option<u32> = None;
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let (key, rest) = strip_label(line);
        let nums: Vec<i64> = rest.split(|c: char| !(c.is_ascii_digit() || c == '-')).filter(|t| !t.is_empty() && *t != "-").filter_map(|t| t.parse().ok()).collect();
        if nums.len() < 2 || nums.len() > 3 {
            return Err(format!("'{line}' wants hits and steps, E(5,8), and maybe a rotation"));
        }
        let (k, n, r) = (nums[0], nums[1], nums.get(2).copied().unwrap_or(0));
        if k < 0 || n < 1 || k > n || n > 192 {
            return Err(format!("E({k},{n}) is not possible: the hits fit in the steps"));
        }
        if len.is_some_and(|l| i64::from(l) != n) {
            return Err("the rows have different step counts".into());
        }
        len = Some(n as u32);
        let hits = crate::text::mini::euclid(k as usize, n as usize, r).iter().enumerate().filter(|(_, h)| **h).map(|(i, _)| (i as u32, HIT)).collect();
        lanes.push((key.unwrap_or(o.lane), hits));
    }
    if lanes.is_empty() {
        return Err("no Euclidean rhythm found".into());
    }
    finish_cycle(lanes, len.unwrap_or(1), o)
}

// ---------------------------------------------------------------------
// Counting

/// Counting: `1 e & a 2 & 3 4 e`: the hits at the counts named.
pub fn counts(text: &str, o: &Opts) -> R<Pattern> {
    let mut hits: Vec<(u32, u32, u32)> = Vec::new(); // (bar, beat, sixteenth-in-beat)
    let (mut bar, mut beat, mut last) = (0u32, 0u32, 0u32);
    let mut spb = 1;
    let (key, rest) = strip_label(text.trim());
    let mut maxbeat = 0;
    for tok in rest.split(|c: char| c.is_whitespace() || c == ',').filter(|t| !t.is_empty()) {
        match tok.to_ascii_lowercase().as_str() {
            "e" | "ee" => { spb = spb.max(4); hits.push((bar, beat, 1)); }
            "&" | "+" | "and" => { spb = spb.max(2); hits.push((bar, beat, 2)); }
            "a" | "ah" | "an" => { spb = spb.max(4); hits.push((bar, beat, 3)); }
            "|" => { bar += 1; beat = 0; last = 0; }
            n => {
                let v: u32 = n.parse().map_err(|_| format!("'{tok}' is not a count (1 2 3 4 e & a are)"))?;
                if v == 0 || v > 16 {
                    return Err(format!("beat {v} is out of range"));
                }
                if v <= last {
                    bar += 1;
                }
                (beat, last) = (v - 1, v);
                maxbeat = maxbeat.max(v);
                hits.push((bar, beat, 0));
            }
        }
    }
    // Normalise "e" and "a" (sixteenths) against "&" (eighths).
    let spb = if spb == 1 { 1 } else if spb == 2 { 2 } else { 4 };
    let (beats, unit) = if o.meter_set() { o.meter() } else { (maxbeat.max(1) as u8, 4) };
    let bars = bar + 1;
    let mut p = Pattern::new(beats, unit, bars, u32::from(beats) * spb);
    for (b, bt, s) in hits {
        let sub = if spb == 4 { s } else if spb == 2 { s / 2 } else { 0 };
        p.put(key.unwrap_or(o.lane), i64::from(b * u32::from(beats) * spb + bt * spb + sub), HIT);
    }
    Ok(p)
}

// ---------------------------------------------------------------------
// Bols: tala, tabla, konnakol

/// A tala: (name, theka with `|` between vibhags, the marks a vibhag, the meter).
pub const TALAS: [(&str, &str, &str, (u8, u8)); 8] = [
    ("teentaal", "Dha Dhin Dhin Dha | Dha Dhin Dhin Dha | Dha Tin Tin Ta | Ta Dhin Dhin Dha", "X 2 0 3", (4, 4)),
    ("jhaptaal", "Dhin Na | Dhin Dhin Na | Tin Na | Dhin Dhin Na", "X 2 0 3", (5, 4)),
    ("ektaal", "Dhin Dhin | Dha-Ge Ti-Ra-Ki-Ta | Tu Na | Kat Ta | Dha-Ge Ti-Ra-Ki-Ta | Dhin Na", "X 0 2 0 3 4", (6, 4)),
    ("rupak", "Tin Tin Na | Dhi Na | Dhi Na", "0 2 3", (7, 4)),
    ("dadra", "Dha Dhi Na | Dha Tu Na", "X 0", (3, 4)),
    ("keherwa", "Dha Ge Na Ti | Na Ka Dhi Na", "X 0", (4, 4)),
    ("dhamar", "Ka Dhi Ta Dhi Ta | Dha - | Ga Ti Ta | Ti Ta Ti Ta", "X 2 0 3", (7, 4)),
    ("deepchandi", "Dha Dhin - | Dha Dha Tin - | Ta Tin - | Dha Dha Dhin -", "X 2 0 3", (7, 4)),
];

/// The drums a bol plays: tabla's right hand on the high congas and
/// bongos, its left on the low ones (Neumetik's own mapping).
fn tabla_keys(bol: &str) -> Option<&'static [u8]> {
    Some(match bol {
        "dha" => &[63, 64],
        "dhin" | "dhun" => &[60, 64],
        "dhi" | "dhe" | "dhit" | "dhir" => &[62, 64],
        "na" | "ta" => &[63],
        "tin" | "tun" => &[60],
        "te" | "ti" | "ra" | "ri" | "ki" | "tu" | "tit" | "tak" | "re" | "kit" | "tra" => &[62],
        "ke" | "ka" | "kat" | "kha" => &[61],
        "ge" | "ghe" | "ga" | "gi" | "gha" => &[64],
        _ => return None,
    })
}

/// Konnakol and solkattu syllables, on the same drums (Neumetik's own).
fn konnakol_keys(bol: &str) -> Option<&'static [u8]> {
    Some(match bol {
        "ta" | "na" | "lan" => &[63],
        "ka" | "ki" | "ke" | "te" | "ri" | "re" => &[62],
        "di" | "ju" | "ma" | "dhi" => &[60],
        "mi" | "nu" | "ri2" => &[61],
        "dhom" | "tom" | "gu" | "thom" | "ge" | "gi" | "dhin" | "dha" => &[64],
        _ => return None,
    })
}

fn bol_share(text: &str) -> f64 {
    let words: Vec<String> = text.split(|c: char| !c.is_ascii_alphabetic()).filter(|w| w.len() >= 2).map(|w| w.to_ascii_lowercase()).collect();
    if words.len() < 4 {
        return 0.0;
    }
    let bols = words.iter().filter(|w| tabla_keys(w).is_some_and(|_| !matches!(w.as_str(), "ta" | "ka" | "na" | "ra" | "ti" | "te" | "ki" | "tu")) || matches!(w.as_str(), "dhin" | "dha" | "tirakita" | "dhage")).count();
    bols as f64 / words.len() as f64
}

/// Splits a bol token into its syllables: `Dha-Ge`, `TiRaKiTa`.
fn syllables_of(tok: &str) -> Vec<String> {
    let mut out = Vec::new();
    for part in tok.split('-').filter(|p| !p.is_empty()) {
        let mut cur = String::new();
        for c in part.chars() {
            if c.is_ascii_uppercase() && !cur.is_empty() && cur.chars().any(|x| x.is_ascii_lowercase()) {
                out.push(std::mem::take(&mut cur));
            }
            cur.push(c);
        }
        if !cur.is_empty() {
            out.push(cur);
        }
    }
    // A word of syllables run together in lower case.
    let mut v = Vec::new();
    for s in out {
        match s.to_ascii_lowercase().as_str() {
            "tirakita" => v.extend(["ti", "ra", "ki", "ta"].map(String::from)),
            "dhage" => v.extend(["dha", "ge"].map(String::from)),
            "terekete" => v.extend(["te", "re", "ke", "te"].map(String::from)),
            _ => v.push(s),
        }
    }
    v
}

fn is_rest_token(t: &str) -> bool {
    matches!(t, "-" | "," | "." | "~" | "S" | "s" | "_")
}

/// Tala, tabla bols and konnakol: a syllable a pulse, `|` between groups,
/// `tala NAME` for a known one, a line of marks (`X 2 0 3`) under it.
pub fn bols(text: &str, o: &Opts) -> R<Pattern> {
    let mut o = o.clone();
    let mut theka = String::new();
    let mut marks_line = String::new();
    let mut named: Option<usize> = None;
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let lower = line.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("tala") {
            let name = rest.trim_start_matches([':', ' ']).trim();
            let i = TALAS.iter().position(|t| t.0 == name || (name == "trital" && t.0 == "teentaal") || (name == "roopak" && t.0 == "rupak")).ok_or_else(|| {
                let known: Vec<&str> = TALAS.iter().map(|t| t.0).collect();
                format!("'{name}' is not a tala Neumetik has ({})", known.join(", "))
            })?;
            named = Some(i);
            continue;
        }
        if lower.starts_with("konnakol") || lower.starts_with("solkattu") {
            o.konnakol = true;
            continue;
        }
        let toks: Vec<&str> = line.split_whitespace().collect();
        if toks.iter().all(|t| *t == "|" || t.chars().all(|c| c == 'X' || c.is_ascii_digit() || c == '|')) && toks.iter().any(|t| *t != "|") {
            marks_line = line.to_string();
        } else {
            theka.push_str(line);
            theka.push(' ');
        }
    }
    let mut table_meter = None;
    if let Some(i) = named {
        let t = &TALAS[i];
        if theka.trim().is_empty() {
            theka = t.1.to_string();
        }
        if marks_line.is_empty() {
            marks_line = t.2.to_string();
        }
        table_meter = Some(t.3);
    }
    // Groups (vibhags) of pulses.
    let mut groups: Vec<Vec<String>> = vec![Vec::new()];
    for tok in theka.split_whitespace() {
        if tok == "|" {
            groups.push(Vec::new());
        } else {
            for piece in tok.split('|') {
                if piece.is_empty() {
                    groups.push(Vec::new());
                } else {
                    groups.last_mut().expect("never empty").push(piece.to_string());
                }
            }
        }
    }
    groups.retain(|g| !g.is_empty());
    let pulses: usize = groups.iter().map(Vec::len).sum();
    if pulses == 0 {
        return Err("no bols found".into());
    }
    let konnakol = o.konnakol || groups.iter().flatten().any(|t| matches!(t.to_ascii_lowercase().as_str(), "di" | "mi" | "ju" | "nu" | "ma" | "dhom" | "lan" | "gu"));
    let (beats, unit) = if o.meter_set() {
        o.meter()
    } else if let Some(m) = table_meter {
        m
    } else if konnakol || pulses % 4 == 0 || pulses > 16 {
        (4, 4)
    } else {
        (pulses as u8, 4)
    };
    // A pulse is `per` cells; a bar is a beat of sixteenths (an eighth a beat in 8) a beat.
    let per = if o.per > 0 { o.per } else if konnakol { 1 } else if unit == 8 { 2 } else { 4 } as usize;
    let steps = if o.steps > 0 { o.steps as usize } else if unit == 8 { usize::from(beats) * 2 } else { usize::from(beats) * 4 };
    if steps % per != 0 {
        return Err(format!("a bar of {steps} cells does not hold whole pulses of {per}: say @steps or @per"));
    }
    let bars = pulses.div_ceil(steps / per).max(1);
    let mut p = Pattern::new(beats, unit, bars as u32, steps as u32);
    let total = p.len();
    p.bols = vec![String::new(); total];
    let marks: Vec<char> = marks_line.split_whitespace().filter(|t| *t != "|").map(|t| t.chars().next().unwrap_or(' ')).collect();
    if !marks.is_empty() {
        p.marks = vec![' '; total];
    }
    let mut pulse = 0usize;
    for (gi, g) in groups.iter().enumerate() {
        for (ti, tok) in g.iter().enumerate() {
            let base = pulse * per;
            if base >= total {
                break;
            }
            if let Some(m) = marks.get(gi) {
                if ti == 0 {
                    p.marks[base] = *m;
                }
            }
            if is_rest_token(tok) {
                pulse += 1;
                continue;
            }
            let syl = syllables_of(tok);
            if syl.is_empty() {
                pulse += 1;
                continue;
            }
            if per % syl.len() != 0 {
                return Err(format!("'{tok}' has {} syllables in a pulse of {per} cells: say @per {}", syl.len(), per * syl.len()));
            }
            let gap = per / syl.len();
            for (si, s) in syl.iter().enumerate() {
                let low = s.to_ascii_lowercase();
                let keys = if konnakol { konnakol_keys(&low).or_else(|| tabla_keys(&low)) } else { tabla_keys(&low).or_else(|| konnakol_keys(&low)) };
                let keys = keys.ok_or_else(|| format!("'{s}' is not a bol Neumetik knows"))?;
                let cell = base + si * gap;
                let level = if (gi == 0 && ti == 0 && si == 0) || (konnakol && ti == 0 && si == 0 && g.len() > 1) { ACCENT } else { HIT };
                for k in keys {
                    p.put(*k, cell as i64, level);
                }
                p.bols[cell] = s.clone();
            }
            pulse += 1;
        }
    }
    Ok(p)
}

// ---------------------------------------------------------------------
// Takadimi, Kodaly: a word a beat

/// A word's hits as fractions of a pulse, and the pulses it spans.
fn syllable_group(g: &str) -> Option<(Vec<Fr>, u64)> {
    let g = g.to_ascii_lowercase();
    if matches!(g.as_str(), "z" | "sh" | "rest" | "-" | ".") {
        return Some((vec![], 1));
    }
    let special: Option<(&[(u64, u64)], u64)> = match g.as_str() {
        "syn-co-pa" | "syncopa" => Some((&[(0, 1), (1, 2), (3, 2)], 2)),
        "tim-ka" => Some((&[(0, 1), (3, 4)], 1)),
        "ka-tim" => Some((&[(0, 1), (1, 4)], 1)),
        "ti-ka-ti" => Some((&[(0, 1), (1, 4), (1, 2)], 1)),
        "ti-ti-ka" => Some((&[(0, 1), (1, 2), (3, 4)], 1)),
        "tam-ti" => Some((&[(0, 1), (3, 4)], 1)),
        _ => None,
    };
    if let Some((h, span)) = special {
        return Some((h.iter().map(|(n, d)| Fr::new(*n, *d)).collect(), span));
    }
    let parts: Vec<&str> = g.split('-').collect();
    if parts.len() > 1 && parts[1..].iter().all(|p| *p == "a") && parts[0] != "a" {
        return Some((vec![Fr::new(0, 1)], parts.len() as u64));
    }
    const OK: [&str; 13] = ["ta", "ka", "di", "mi", "ki", "da", "ti", "tri", "o", "la", "ke", "te", "tu"];
    if !parts.is_empty() && parts.len() <= 12 && parts.iter().all(|p| OK.contains(p)) {
        let n = parts.len() as u64;
        return Some(((0..n).map(|i| Fr::new(i, n)).collect(), 1));
    }
    None
}

/// Takadimi and Kodaly syllables: a word a beat (`ta`, `ti-ti`, `ta-ka-di-mi`,
/// `ta-a`, `syn-co-pa`, `z` for a rest); in 6/8 and the like a word is a dotted
/// beat of three.
pub fn syllables(text: &str, o: &Opts) -> R<Pattern> {
    let (beats, unit) = o.meter();
    let compound = unit == 8 && beats % 3 == 0 && beats >= 6;
    let ppb: u64 = if compound { u64::from(beats / 3) } else { u64::from(beats) };
    let (key, rest) = strip_label(text.trim());
    let mut hits: Vec<(u64, Fr, String)> = Vec::new(); // (pulse, fraction, syllable)
    let mut pulse = 0u64;
    let mut den = 1u64;
    for w in rest.split_whitespace() {
        if w == "|" {
            continue;
        }
        let (offs, span) = syllable_group(w).ok_or_else(|| format!("'{w}' is not a rhythm syllable (ta, ti-ti, ta-ka-di-mi, ta-a, syn-co-pa, z are)"))?;
        let names: Vec<&str> = w.split('-').collect();
        for (i, f) in offs.iter().enumerate() {
            den = lcm(den, f.1);
            hits.push((pulse, *f, names.get(i).copied().unwrap_or("").to_string()));
        }
        pulse += span;
    }
    if hits.is_empty() {
        return Err("only rests".into());
    }
    if den > 48 {
        return Err("those divisions need too fine a grid".into());
    }
    let bars = pulse.div_ceil(ppb).max(1);
    let steps = den * ppb;
    let mut p = Pattern::new(beats, unit, bars as u32, steps as u32);
    p.bols = vec![String::new(); p.len()];
    for (pl, f, s) in hits {
        let cell = pl * den + f.0 * den / f.1;
        p.put(key.unwrap_or(o.lane), cell as i64, HIT);
        p.bols[cell as usize] = s;
    }
    Ok(p)
}

// ---------------------------------------------------------------------
// Note values: q e e q, h q q

/// A note value token as a fraction of a whole note, and whether it is a rest.
fn value_token(tok: &str) -> Option<(Fr, bool)> {
    let (rest, body) = match tok.strip_prefix(['r', 'R']) {
        Some(b) if !b.is_empty() => (true, b),
        _ => (false, tok),
    };
    let dots = body.chars().rev().take_while(|c| *c == '.').count();
    let core = &body[..body.len() - dots];
    let base = match core.to_ascii_lowercase().as_str() {
        "w" | "1" => Fr(1, 1),
        "h" | "2" => Fr(1, 2),
        "q" | "4" => Fr(1, 4),
        "e" | "8" => Fr(1, 8),
        "s" | "16" => Fr(1, 16),
        "t" | "32" => Fr(1, 32),
        "x" | "64" => Fr(1, 64),
        c if c.contains('/') => {
            let (n, d) = c.split_once('/')?;
            let (n, d): (u64, u64) = (n.parse().ok()?, d.parse().ok()?);
            if n == 0 || d == 0 {
                return None;
            }
            Fr::new(n, d)
        }
        _ => return None,
    };
    let mut v = base;
    let mut add = base;
    for _ in 0..dots {
        add = add.mul(Fr(1, 2));
        v = v.add(add);
    }
    Some((v, rest))
}

/// Note values: `q e e q`, `q. e h`, `rq` a rest, `3(e e e)` a triplet, `_`
/// a tie to the note before.
pub fn values(text: &str, o: &Opts) -> R<Pattern> {
    let (key, rest) = strip_label(text.trim());
    let spaced = rest.replace('(', " ( ").replace(')', " ) ").replace('[', " ( ").replace(']', " ) ");
    let toks: Vec<&str> = spaced.split_whitespace().collect();
    let mut events: Vec<(Fr, Fr, bool)> = Vec::new(); // (start, length, rest)
    let mut at = Fr(0, 1);
    let mut scale: Vec<Fr> = vec![Fr(1, 1)];
    let mut pend: Option<Fr> = None;
    let mut tie = false;
    for (i, tok) in toks.iter().enumerate() {
        match *tok {
            "(" => {
                let top = *scale.last().expect("never empty");
                scale.push(top.mul(pend.take().unwrap_or(Fr(1, 1))));
            }
            ")" => {
                if scale.len() > 1 {
                    scale.pop();
                }
            }
            "_" | "~" => tie = true,
            "|" => {}
            t => {
                let group = t.parse::<u64>().ok().filter(|n| (3..=13).contains(n) && !n.is_power_of_two() && toks.get(i + 1) == Some(&"("));
                if let Some(n) = group {
                    // n notes in the time of the power of two below.
                    let m = (1..n).rev().find(|m| m.is_power_of_two()).unwrap_or(2);
                    pend = Some(Fr::new(m, n));
                } else if let Some((v, is_rest)) = value_token(t) {
                    let len = v.mul(*scale.last().expect("never empty"));
                    match events.last_mut() {
                        Some(last) if tie && !is_rest => {
                            last.1 = last.1.add(len);
                        }
                        _ => events.push((at, len, is_rest)),
                    }
                    tie = false;
                    at = at.add(len);
                } else {
                    return Err(format!("'{t}' is not a note value (w h q e s t, dots, r for a rest, 3( ) for a triplet)"));
                }
            }
        }
    }
    let (beats, unit) = o.meter();
    let bar = Fr::new(u64::from(beats), u64::from(unit));
    let mut den = bar.1;
    for (s, _, r) in &events {
        if !r {
            den = lcm(den, s.1);
        }
    }
    if den > 192 {
        return Err("those note values need too fine a grid".into());
    }
    // Whole bars the music takes.
    let bars = ((at.0 * bar.1).div_ceil(at.1 * bar.0)).max(1);
    let steps = bar.0 * den / bar.1;
    let mut p = Pattern::new(beats, unit, bars as u32, steps as u32);
    for (s, _, r) in events {
        if !r {
            p.put(key.unwrap_or(o.lane), (s.0 * den / s.1) as i64, HIT);
        }
    }
    Ok(p)
}

// ---------------------------------------------------------------------
// LilyPond drum mode

/// LilyPond's drum names (the common ones), as General MIDI keys.
fn lily_key(name: &str) -> Option<u8> {
    Some(match name {
        "bd" | "bda" | "bassdrum" | "acousticbassdrum" => 36,
        "sn" | "sna" | "snare" | "acousticsnare" => 38,
        "sne" | "electricsnare" => 40,
        "ss" | "ssh" | "ssl" | "sidestick" => 37,
        "hc" | "handclap" => 39,
        "hh" | "hhc" | "closedhihat" | "hihat" => 42,
        "hho" | "openhihat" | "hhho" => 46,
        "hhp" | "pedalhihat" => 44,
        "cymc" | "cymca" | "crashcymbal" => 49,
        "cymr" | "cymra" | "ridecymbal" => 51,
        "rb" | "ridebell" => 53,
        "cymch" | "chinesecymbal" => 52,
        "cyms" | "splashcymbal" => 55,
        "tomh" | "hightom" => 50,
        "tommh" | "himidtom" => 48,
        "tomml" | "lowmidtom" => 47,
        "toml" | "lowtom" => 45,
        "tomfh" | "highfloortom" => 43,
        "tomfl" | "lowfloortom" => 41,
        "cb" | "cowbell" => 56,
        "tamb" | "tambourine" => 54,
        "vibs" | "vibraslap" => 58,
        "boh" | "bohm" | "bongo" => 60,
        "bol" | "bolm" => 61,
        "cghm" | "mutehighconga" => 62,
        "cgho" | "openhighconga" => 63,
        "cgl" | "cglm" | "lowconga" => 64,
        "timh" | "hightimbale" => 65,
        "timl" | "lowtimbale" => 66,
        "agh" | "highagogo" => 67,
        "agl" | "lowagogo" => 68,
        "cab" | "cabasa" => 69,
        "mar" | "maracas" => 70,
        "whs" | "shortwhistle" => 71,
        "whl" | "longwhistle" => 72,
        "guis" | "shortguiro" => 73,
        "guil" | "guiro" | "longguiro" => 74,
        "cl" | "claves" => 75,
        "wbh" | "highwoodblock" => 76,
        "wbl" | "lowwoodblock" => 77,
        "cuim" | "mutecuica" => 78,
        "cuio" | "opencuica" => 79,
        "trim" | "mutetriangle" => 80,
        "tri" | "trio" | "opentriangle" => 81,
        _ => return None,
    })
}

/// A LilyPond duration: 1 2 4 8 16 32 64 with dots, and `*n/m`.
fn lily_dur(tok: &str) -> Option<Fr> {
    let (head, mult) = tok.split_once('*').map_or((tok, None), |(a, b)| (a, Some(b)));
    let dots = head.chars().rev().take_while(|c| *c == '.').count();
    let n: u64 = head[..head.len() - dots].parse().ok()?;
    if !matches!(n, 1 | 2 | 4 | 8 | 16 | 32 | 64 | 128) {
        return None;
    }
    let base = Fr(1, n);
    let (mut v, mut add) = (base, base);
    for _ in 0..dots {
        add = add.mul(Fr(1, 2));
        v = v.add(add);
    }
    if let Some(m) = mult {
        let (a, b) = m.split_once('/').map_or((m, "1"), |(a, b)| (a, b));
        v = v.mul(Fr::new(a.parse().ok()?, b.parse().ok()?));
    }
    Some(v)
}

/// LilyPond drum mode: `\drummode { bd4 hh8 hh sn4 <bd hh>8 r }`, with
/// `\time`, `\tuplet` and `\times`, `\repeat unfold`, chords `< >`, rests
/// `r` and spacers `s4`. Written from LilyPond's published notation
/// reference; not run against LilyPond.
pub fn lilypond(text: &str, o: &Opts) -> R<Pattern> {
    // Tokens: split at spaces, and at braces, angle brackets, bars.
    let mut src = String::new();
    for line in text.lines() {
        src.push_str(line.split('%').next().unwrap_or(""));
        src.push(' ');
    }
    let spaced = src.replace("->", " \u{1} ").replace('{', " { ").replace('}', " } ").replace('<', " < ").replace('>', " > ").replace('|', " ");
    let toks: Vec<String> = spaced.split_whitespace().map(String::from).collect();
    struct Ev {
        at: Fr,
        key: u8,
        accent: bool,
    }
    let mut evs: Vec<Ev> = Vec::new();
    let mut at = Fr(0, 1);
    let mut dur = Fr(1, 4);
    let (mut beats, mut unit) = o.meter();
    // Brace stack: (scale, repeat count, tokens start) handled recursively below.
    fn run(toks: &[String], i: &mut usize, at: &mut Fr, dur: &mut Fr, scale: Fr, evs: &mut Vec<Ev>, grid: &mut u64, meter: &mut (u8, u8), depth: usize) -> R<()> {
        if depth > 8 {
            return Err("too many nested groups".into());
        }
        while *i < toks.len() {
            let t = toks[*i].clone();
            *i += 1;
            match t.as_str() {
                "}" => return Ok(()),
                "{" => run(toks, i, at, dur, scale, evs, grid, meter, depth + 1)?,
                "\\drummode" | "\\new" | "\\drums" | "DrumStaff" | "DrumVoice" | "\\context" | "\\relative" | "\\global" | "\\drumsOn" | "\\stemUp" | "\\stemDown" | "\\voiceOne" | "\\voiceTwo" | "\\bar" => {}
                "\\time" => {
                    let v = toks.get(*i).ok_or("\\time wants a meter")?.clone();
                    *i += 1;
                    let (b, u) = v.split_once('/').ok_or("\\time wants a meter like 7/8")?;
                    *meter = (b.parse().map_err(|_| "bad \\time")?, u.parse().map_err(|_| "bad \\time")?);
                }
                "\\tempo" | "\\clef" | "\\key" | "\\set" | "\\override" | "\\revert" => {
                    // Skip its arguments up to the next token that starts a note or a brace.
                    while *i < toks.len() && !toks[*i].starts_with('\\') && toks[*i] != "{" && toks[*i] != "}" && lily_key(note_name(&toks[*i])).is_none() && toks[*i] != "<" {
                        *i += 1;
                    }
                }
                "\\tuplet" | "\\times" | "\\scaleDurations" => {
                    let v = toks.get(*i).ok_or("a tuplet wants a fraction")?.clone();
                    *i += 1;
                    let (a, b) = v.split_once('/').ok_or("a tuplet wants a fraction like 3/2")?;
                    let (a, b): (u64, u64) = (a.parse().map_err(|_| "bad tuplet")?, b.parse().map_err(|_| "bad tuplet")?);
                    // \tuplet 3/2 { } plays 3 in the time of 2; \times 2/3 { } the same.
                    let f = if t == "\\tuplet" { Fr::new(b, a) } else { Fr::new(a, b) };
                    if toks.get(*i).map(String::as_str) != Some("{") {
                        return Err("a tuplet wants { }".into());
                    }
                    *i += 1;
                    run(toks, i, at, dur, scale.mul(f), evs, grid, meter, depth + 1)?;
                }
                "\\repeat" => {
                    let kind = toks.get(*i).cloned().unwrap_or_default();
                    let n: usize = toks.get(*i + 1).and_then(|n| n.parse().ok()).ok_or("\\repeat wants a count")?;
                    *i += 2;
                    if !matches!(kind.as_str(), "unfold" | "percent" | "volta") {
                        return Err(format!("\\repeat {kind} is not read"));
                    }
                    if toks.get(*i).map(String::as_str) != Some("{") {
                        return Err("\\repeat wants { }".into());
                    }
                    *i += 1;
                    let start = *i;
                    for rep in 0..n.min(64) {
                        *i = start;
                        run(toks, i, at, dur, scale, evs, grid, meter, depth + 1)?;
                        let _ = rep;
                    }
                    if n == 0 {
                        // skip the block
                        let mut d = 1;
                        while *i < toks.len() && d > 0 {
                            if toks[*i] == "{" {
                                d += 1;
                            } else if toks[*i] == "}" {
                                d -= 1;
                            }
                            *i += 1;
                        }
                    }
                }
                "<" => {
                    let mut keys = Vec::new();
                    while *i < toks.len() && toks[*i] != ">" {
                        let n = note_name(&toks[*i]);
                        keys.push(lily_key(n).ok_or_else(|| format!("'{n}' is not a drum LilyPond names"))?);
                        *i += 1;
                    }
                    *i += 1;
                    // The duration follows the `>` straight on (a token `>8` was split).
                    let mut chord_accent = false;
                    if let Some(tok) = toks.get(*i) {
                        let (d, acc) = match tok.strip_suffix("->") {
                            Some(d) => (d, true),
                            None => (tok.as_str(), false),
                        };
                        if let Some(d) = lily_dur(d) {
                            *dur = d;
                            chord_accent = acc;
                            *i += 1;
                        }
                    }
                    for k in keys {
                        evs.push(Ev { at: *at, key: k, accent: chord_accent });
                    }
                    *at = at.add(dur.mul(scale));
                    *grid = lcm(*grid, at.1);
                }
                "~" | "(" | ")" | "\\(" | "\\)" | "[" | "]" | "\\accent" | "\\ghost" | "\\laissezVibrer" => {}
                "\u{1}" | "-^" => {
                    if let Some(last) = evs.last().map(|e| e.at) {
                        for e in evs.iter_mut().rev().take_while(|e| e.at == last) {
                            e.accent = true;
                        }
                    }
                }
                other if other.starts_with('\\') => return Err(format!("'{other}' is not read in drum mode")),
                other => {
                    let name = note_name(other);
                    let tail = &other[name.len()..];
                    let (d, accent) = match tail.strip_suffix("->") {
                        Some(d) => (d, true),
                        None => (tail, false),
                    };
                    if !d.is_empty() {
                        *dur = lily_dur(d).ok_or_else(|| format!("'{other}' has a duration LilyPond doesn't"))?;
                    }
                    let len = dur.mul(scale);
                    if matches!(name, "r" | "R" | "s") {
                        // A rest, a whole-bar rest or a spacer.
                        *at = at.add(len);
                        *grid = lcm(*grid, at.1);
                    } else {
                        let k = lily_key(name).ok_or_else(|| format!("'{name}' is not a drum LilyPond names (bd sn hh hho cymc tomh...)"))?;
                        evs.push(Ev { at: *at, key: k, accent });
                        *at = at.add(len);
                        *grid = lcm(*grid, at.1);
                    }
                }
            }
        }
        Ok(())
    }
    // A name is the letters of a token; the rest is its duration.
    fn note_name(t: &str) -> &str {
        let end = t.find(|c: char| c.is_ascii_digit() || c == '-' || c == '*' || c == '.').unwrap_or(t.len());
        &t[..end]
    }
    let mut i = 0;
    let mut m = (beats, unit);
    let mut grid = 1u64;
    run(&toks, &mut i, &mut at, &mut dur, Fr(1, 1), &mut evs, &mut grid, &mut m, 0)?;
    if o.meter_set() {
        m = o.meter();
    }
    (beats, unit) = m;
    let bar = Fr::new(u64::from(beats), u64::from(unit));
    let mut den = lcm(bar.1, grid);
    for e in &evs {
        den = lcm(den, e.at.1);
    }
    if den > 192 {
        return Err("those durations need too fine a grid".into());
    }
    let bars = ((at.0 * bar.1).div_ceil(at.1 * bar.0)).max(1);
    let steps = bar.0 * den / bar.1;
    let mut p = Pattern::new(beats, unit, bars as u32, steps as u32);
    for e in evs {
        p.put(e.key, (e.at.0 * den / e.at.1) as i64, if e.accent { ACCENT } else { HIT });
    }
    Ok(p)
}

// ---------------------------------------------------------------------
// Stickings and rudiments

/// Stickings: `R L R R L R L L`, `R> L L R`, `(l)R` a flam, `(ll)R` a drag,
/// `.` a rest, `K` the kick. The right hand plays the snare and the left the
/// second snare (so that a sticking is heard), or both play `@lane`'s drum. A stroke is a sixteenth (`@per 4` strokes a
/// beat), cut into four cells so that a grace lands before it; the
/// sequence is tiled to fill whole bars (or padded with rests).
pub fn sticking(text: &str, o: &Opts) -> R<Pattern> {
    let (beats, unit) = o.meter();
    let per_beat = if o.per > 0 { o.per } else { 4 } as usize;
    const SUB: usize = 4;
    struct St {
        key: u8,
        level: u8,
        graces: usize,
        /// The hand of the graces (`L` or `R`).
        grace_hand: char,
        mark: String,
    }
    let mut seq: Vec<Option<St>> = Vec::new();
    let mut graces = 0usize;
    let mut grace_hand = 'R';
    let mut chars = text.chars().filter(|c| !c.is_whitespace() && *c != '/' && *c != '|').peekable();
    while let Some(c) = chars.next() {
        match c {
            '(' => {
                let mut n = 0;
                for g in chars.by_ref() {
                    if g == ')' {
                        break;
                    }
                    if !"RLrl".contains(g) {
                        return Err(format!("'{g}' is not a grace note (r or l)"));
                    }
                    grace_hand = g.to_ascii_uppercase();
                    n += 1;
                }
                graces = n;
            }
            'R' | 'L' | 'r' | 'l' | 'K' => {
                let accent = chars.peek() == Some(&'>');
                if accent {
                    chars.next();
                }
                seq.push(Some(St { key: if c == 'K' { 36 } else { o.lane.clamp(27, 87) }, level: if accent { ACCENT } else { HIT }, graces, grace_hand, mark: c.to_ascii_uppercase().to_string() }));
                graces = 0;
            }
            '.' | '-' => seq.push(None),
            other => return Err(format!("'{other}' is not a sticking (R L r l K ( ) > . are)")),
        }
    }
    if seq.is_empty() {
        return Err("no strokes found".into());
    }
    // The default drums: the snare for the right hand, the second snare for
    // the left, so a sticking can be told by ear; `@lane` puts both on one.
    let (right, left) = if o.lane == 75 { (38, 40) } else { (o.lane, o.lane) };
    // Strokes a bar: `per_beat` a quarter note.
    let per_bar = (per_beat * usize::from(beats) * 4 / usize::from(unit.max(1))).max(1);
    // Tile to whole bars when it comes out in eight copies or fewer, else pad.
    let len = seq.len();
    let tiles = (1..=8usize).find(|k| (len * k) % per_bar == 0).unwrap_or(0);
    let total = if tiles > 0 { len * tiles } else { len.div_ceil(per_bar) * per_bar };
    let bars = total / per_bar;
    let mut p = Pattern::new(beats, unit, bars as u32, (per_bar * SUB) as u32);
    p.bols = vec![String::new(); p.len()];
    for i in 0..total {
        if let Some(Some(s)) = seq.get(i % len).filter(|_| tiles > 0 || i < len) {
            let cell = (i * SUB) as i64;
            let key = if s.key == 36 { 36 } else if s.mark == "L" { left } else { right };
            p.put(key, cell, s.level);
            p.bols[cell as usize] = s.mark.clone();
            let gkey = if s.key == 36 { 36 } else if s.grace_hand == 'L' { left } else { right };
            for g in 0..s.graces {
                // The graces before the stroke, a cell apart (about 30 ms).
                p.put(gkey, cell - (g as i64 + 1), SOFT);
            }
        }
    }
    Ok(p)
}

// ---------------------------------------------------------------------
// Mini-notation and scores, by Neumetik

fn mini(text: &str, o: &Opts) -> R<Pattern> {
    let (score, title) = crate::text::mini::read(text).map_err(|e| format!("{e}"))?;
    let mut p = from_score(&score, o)?;
    if p.title.is_empty() {
        p.title = title;
    }
    Ok(p)
}

/// The drum part of a score (or, with none, all its notes on one drum) as
/// a pattern, quantized to the smallest grid its notes sit on.
pub fn from_score(score: &crate::Score, o: &Opts) -> R<Pattern> {
    let drums: Vec<&crate::score::Part> = score.parts.iter().filter(|p| p.channel == crate::score::DRUMS && !p.notes.is_empty()).collect();
    let (beats, unit) = if o.meter_set() { o.meter() } else { (score.beats.max(1), score.unit.max(1)) };
    let ppq = u32::from(score.ppq.max(1));
    let tpb = (ppq * 4 / u32::from(unit)).max(1);
    let bar = tpb * u32::from(beats);
    let from = o.from;
    let count = if o.bars > 0 { o.bars } else { score.bars.saturating_sub(from).clamp(1, 8) };
    let (t0, t1) = (u64::from(from) * u64::from(bar), u64::from(from + count) * u64::from(bar));
    let mut notes: Vec<(u32, u8, u8)> = Vec::new();
    if drums.is_empty() {
        for part in &score.parts {
            for n in &part.notes {
                notes.push((n.at, o.lane, n.velocity));
            }
        }
    } else {
        for part in &drums {
            for n in &part.notes {
                notes.push((n.at, n.key, n.velocity));
            }
        }
    }
    notes.retain(|n| u64::from(n.0) >= t0 && u64::from(n.0) < t1);
    if notes.is_empty() {
        return Err("no notes in the bars asked for".into());
    }
    // The coarsest grid every onset sits on.
    let fits = |spb: u32, tol: f64| notes.iter().all(|n| {
        let rel = f64::from(n.0) - t0 as f64;
        let pos = rel * f64::from(spb) / f64::from(tpb);
        (pos - pos.round()).abs() <= tol
    });
    let spb = [2u32, 3, 4, 6, 8, 12, 16, 24].into_iter().find(|s| fits(*s, 0.02)).or_else(|| [2u32, 3, 4, 6, 8, 12, 16, 24].into_iter().find(|s| fits(*s, 0.2))).unwrap_or(4);
    let steps = u32::from(beats) * spb;
    let mut p = Pattern::new(beats, unit, count, steps);
    for (at, key, vel) in notes {
        let rel = f64::from(at) - t0 as f64;
        let cell = (rel * f64::from(spb) / f64::from(tpb)).round() as i64;
        p.put(key, cell, level_of(vel).max(SOFT));
    }
    Ok(p)
}

// ---------------------------------------------------------------------
// Drumscript

/// A note name as a MIDI key: `c2` (36), `fis2`, `f#2`, `bb1`; middle C is
/// `c4`. Drumscript's mapping section writes the drums' keys this way.
fn ds_note(s: &str) -> Option<u8> {
    let s = s.to_ascii_lowercase();
    let mut ch = s.chars();
    let letter = ch.next()?;
    let pc: i32 = match letter {
        'c' => 0,
        'd' => 2,
        'e' => 4,
        'f' => 5,
        'g' => 7,
        'a' => 9,
        'b' => 11,
        _ => return None,
    };
    let rest = ch.as_str();
    let split = rest.find(|c: char| c.is_ascii_digit() || c == '-').unwrap_or(rest.len());
    let (acc, oct) = rest.split_at(split);
    let shift = match acc {
        "" => 0,
        "#" | "is" => 1,
        "b" | "es" | "flat" => -1,
        "s" => {
            if letter == 'e' || letter == 'a' {
                -1
            } else {
                1
            }
        }
        _ => return None,
    };
    let oct: i32 = oct.parse().ok()?;
    u8::try_from((oct + 1) * 12 + pc + shift).ok().filter(|k| *k < 128)
}

/// Splits a line into words, with `[`, `]` and `*` words of their own.
fn ds_words(line: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    for c in line.chars() {
        if c.is_whitespace() || matches!(c, '[' | ']' | '*') {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            if !c.is_whitespace() {
                out.push(c.to_string());
            }
        } else {
            cur.push(c);
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn ds_is_name(s: &str) -> bool {
    !s.is_empty() && s.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// A word `hh8`, `bd4.`, `r4`: its drum name and its length.
fn ds_note_word(w: &str) -> Option<(&str, Fr)> {
    let cut = w.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.').len();
    let (name, dur) = w.split_at(cut);
    if name.is_empty() || dur.is_empty() || !ds_is_name(name) {
        return None;
    }
    Some((name, lily_dur(dur)?))
}

fn ds_key(name: &str, map: &[(String, u8)]) -> Option<u8> {
    let n = name.to_ascii_lowercase();
    lily_key(&n).or_else(|| names::key_from_name(&n)).or_else(|| map.iter().find(|m| m.0 == n).map(|m| m.1).filter(|k| (27..=87).contains(k)))
}

/// One voice's words: its hits (start, key) and its length.
fn ds_voice(words: &[String], map: &[(String, u8)]) -> R<(Vec<(Fr, u8)>, Fr)> {
    fn items(words: &[String], i: &mut usize, nested: bool, map: &[(String, u8)]) -> R<(Vec<(Fr, u8)>, Fr)> {
        let (mut ev, mut at) = (Vec::new(), Fr(0, 1));
        let mut last: Option<(Vec<(Fr, u8)>, Fr)> = None;
        while *i < words.len() {
            let w = words[*i].as_str();
            *i += 1;
            let item: (Vec<(Fr, u8)>, Fr) = match w {
                "]" if nested => return Ok((ev, at)),
                "]" => return Err("a ] with no [ before it".into()),
                "[" => items(words, i, true, map)?,
                "*" => {
                    let n: u64 = words.get(*i).and_then(|n| n.parse().ok()).filter(|n| (1..=512).contains(n)).ok_or("a * needs a count of 1 to 512 after it")?;
                    *i += 1;
                    let (lev, llen) = last.take().ok_or("a * has nothing before it to repeat")?;
                    for _ in 1..n {
                        ev.extend(lev.iter().map(|(s, k)| (at.add(*s), *k)));
                        at = at.add(llen);
                    }
                    last = Some((lev, llen));
                    continue;
                }
                w => {
                    let (name, len) = ds_note_word(w).ok_or_else(|| format!("'{w}' is not a drum with a length (hh8, bd4, sn16, r4 for a rest)"))?;
                    if name.eq_ignore_ascii_case("r") {
                        (Vec::new(), len)
                    } else {
                        let key = ds_key(name, map).ok_or_else(|| format!("'{name}' is not a drum Neumetik knows (bd sn hh tomh tomml toml tomfl ..., or name it under ---)"))?;
                        (vec![(Fr(0, 1), key)], len)
                    }
                }
            };
            ev.extend(item.0.iter().map(|(s, k)| (at.add(*s), *k)));
            at = at.add(item.1);
            last = Some(item);
        }
        if nested {
            Err("a [ with no ] after it".into())
        } else {
            Ok((ev, at))
        }
    }
    let mut i = 0;
    items(words, &mut i, false, map)
}

enum DsItem {
    Def(String, Vec<Vec<String>>),
    Meter(u8, u8),
    Line(Vec<String>),
    Break,
}

/// drumscript (the `drumscript` DSL, CC0): blocks `name ( ... )` of
/// voices (a line each, all starting together), `[hh8] * 8` repeats,
/// `4/4` meters, and a sequence of blocks `loop-a * 7`. Note lines outside a
/// block are a block of their own. The `---` mapping (`bd c2`) is read; a
/// drum Neumetik knows by name keeps its own key (drumscript calls the
/// mapping a backend detail), one it doesn't takes the mapping's.
/// A pattern has one meter, so a script that changes it is refused. Written
/// from the language's documents (`docs/language.md`, `README.md`); the
/// repository's compiler is empty, so no reference ran.
pub fn drumscript(text: &str, o: &Opts) -> R<Pattern> {
    let mut map: Vec<(String, u8)> = Vec::new();
    let mut items: Vec<DsItem> = Vec::new();
    let mut in_map = false;
    let mut open: Option<(String, Vec<Vec<String>>)> = None;
    for (n, raw) in text.lines().enumerate() {
        let line = raw.split('#').next().unwrap_or("").trim();
        let at = |m: String| format!("line {}: {m}", n + 1);
        if line == "---" {
            if open.is_some() {
                return Err(at("a block is not closed with )".into()));
            }
            in_map = true;
            continue;
        }
        if in_map {
            if line.is_empty() {
                continue;
            }
            let w: Vec<&str> = line.split_whitespace().collect();
            let key = (w.len() == 2).then(|| ds_note(w[1])).flatten().ok_or_else(|| at(format!("'{line}' is not a drum and a note (bd c2)")))?;
            map.push((w[0].to_ascii_lowercase(), key));
            continue;
        }
        if let Some((name, lines)) = open.as_mut() {
            let (body, closed) = match line.strip_suffix(')') {
                Some(b) => (b.trim(), true),
                None => (line, false),
            };
            if body.contains('(') {
                return Err(at("a block can't hold a block".into()));
            }
            if !body.is_empty() {
                lines.push(ds_words(body));
            }
            if closed {
                items.push(DsItem::Def(std::mem::take(name), std::mem::take(lines)));
                open = None;
            }
            continue;
        }
        if line.is_empty() {
            items.push(DsItem::Break);
        } else if let Some((head, rest)) = line.split_once('(').filter(|(h, _)| ds_is_name(h.trim())) {
            let name = head.trim().to_ascii_lowercase();
            let (body, closed) = match rest.trim().strip_suffix(')') {
                Some(b) => (b.trim(), true),
                None => (rest.trim(), false),
            };
            let lines = if body.is_empty() { Vec::new() } else { vec![ds_words(body)] };
            if closed {
                items.push(DsItem::Def(name, lines));
            } else {
                open = Some((name, lines));
            }
        } else if let Some((a, b)) = line.split_once('/').filter(|(a, b)| a.trim().parse::<u8>().is_ok() && b.trim().parse::<u8>().is_ok()) {
            let (beats, unit): (u8, u8) = (a.trim().parse().unwrap_or(4), b.trim().parse().unwrap_or(4));
            if beats == 0 || !matches!(unit, 1 | 2 | 4 | 8 | 16 | 32) {
                return Err(at(format!("{beats}/{unit} is not a meter")));
            }
            items.push(DsItem::Meter(beats, unit));
        } else {
            items.push(DsItem::Line(ds_words(line)));
        }
    }
    if open.is_some() {
        return Err("a block is not closed with )".into());
    }
    // The blocks, then the sequence.
    let mut blocks: Vec<(String, (Vec<(Fr, u8)>, Fr))> = Vec::new();
    for it in &items {
        if let DsItem::Def(name, lines) = it {
            let (mut ev, mut len) = (Vec::new(), Fr(0, 1));
            for l in lines {
                let (e, n) = ds_voice(l, &map).map_err(|e| format!("block {name}: {e}"))?;
                ev.extend(e);
                if n.0 * len.1 > len.0 * n.1 {
                    len = n;
                }
            }
            if blocks.iter().any(|b| b.0 == *name) {
                return Err(format!("block {name} is defined twice"));
            }
            blocks.push((name.clone(), (ev, len)));
        }
    }
    let (mut meter, mut used): ((u8, u8), Vec<(u8, u8)>) = (o.meter(), Vec::new());
    let (mut events, mut at) = (Vec::<(Fr, u8)>::new(), Fr(0, 1));
    let mut run: Vec<&Vec<String>> = Vec::new();
    let mut place = |blk: &(Vec<(Fr, u8)>, Fr), times: u64, meter: (u8, u8), used: &mut Vec<(u8, u8)>| -> R<()> {
        if blk.1 .0 == 0 {
            return Err("a block with no length".into());
        }
        if !used.contains(&meter) {
            used.push(meter);
        }
        for _ in 0..times {
            events.extend(blk.0.iter().map(|(s, k)| (at.add(*s), *k)));
            at = at.add(blk.1);
        }
        Ok(())
    };
    let flush = |run: &mut Vec<&Vec<String>>, meter: (u8, u8), used: &mut Vec<(u8, u8)>, place: &mut dyn FnMut(&(Vec<(Fr, u8)>, Fr), u64, (u8, u8), &mut Vec<(u8, u8)>) -> R<()>| -> R<()> {
        if run.is_empty() {
            return Ok(());
        }
        let (mut ev, mut len) = (Vec::new(), Fr(0, 1));
        for l in run.iter() {
            let (e, n) = ds_voice(l, &map)?;
            ev.extend(e);
            if n.0 * len.1 > len.0 * n.1 {
                len = n;
            }
        }
        run.clear();
        place(&(ev, len), 1, meter, used)
    };
    for it in &items {
        match it {
            DsItem::Def(..) => flush(&mut run, meter, &mut used, &mut place)?,
            DsItem::Break => flush(&mut run, meter, &mut used, &mut place)?,
            DsItem::Meter(b, u) => {
                flush(&mut run, meter, &mut used, &mut place)?;
                meter = (*b, *u);
            }
            DsItem::Line(w) => {
                let first = w.first().map(|s| s.to_ascii_lowercase()).unwrap_or_default();
                if blocks.iter().any(|b| b.0 == first) {
                    flush(&mut run, meter, &mut used, &mut place)?;
                    let mut i = 0;
                    while i < w.len() {
                        let name = w[i].to_ascii_lowercase();
                        i += 1;
                        let blk = blocks.iter().find(|b| b.0 == name).ok_or_else(|| format!("'{name}' is not a block defined here"))?;
                        let mut times = 1;
                        if w.get(i).map(String::as_str) == Some("*") {
                            times = w.get(i + 1).and_then(|n| n.parse::<u64>().ok()).filter(|n| (1..=512).contains(n)).ok_or("a * needs a count of 1 to 512 after it")?;
                            i += 2;
                        }
                        place(&blk.1, times, meter, &mut used)?;
                    }
                } else {
                    run.push(w);
                }
            }
        }
    }
    flush(&mut run, meter, &mut used, &mut place)?;
    if used.len() > 1 {
        let m: Vec<String> = used.iter().map(|m| format!("{}/{}", m.0, m.1)).collect();
        return Err(format!("this script changes meter ({}): a pattern has one meter, so read each meter's part on its own", m.join(", ")));
    }
    let (beats, unit) = used.first().copied().unwrap_or(meter);
    let bar = Fr::new(u64::from(beats), u64::from(unit));
    let mut den = bar.1;
    for (s, _) in &events {
        den = lcm(den, s.1);
    }
    if den > 192 {
        return Err("that needs too fine a grid".into());
    }
    let bars = (at.0 * bar.1).div_ceil(at.1 * bar.0).max(1);
    let steps = bar.0 * den / bar.1;
    let mut p = Pattern::new(beats, unit, bars as u32, steps as u32);
    for (s, key) in events {
        p.put(key, (s.0 * den / s.1) as i64, HIT);
    }
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cells(p: &Pattern, key: u8) -> String {
        p.lanes.iter().find(|l| l.key == key).map(|l| l.cells.iter().map(|c| ['.', 'o', 'x', 'X'][usize::from(*c)]).collect()).unwrap_or_default()
    }

    fn read(text: &str) -> Pattern {
        parse(text, None, &Opts::default()).unwrap_or_else(|e| panic!("{text}: {e}"))
    }

    #[test]
    fn a_drum_grid_with_a_counting_header() {
        let p = read("Instrument | 1 e & a 2 e & a 3 e & a 4 e & a\nKick       | x . . . . . x . . . x . . . . .\nSnare      | . . . . x . . . . . . . x . . .\nClosed hat | x . x . x . x . x . x . x . x .");
        assert_eq!((p.meter().as_str(), p.bars, p.steps), ("4/4", 1, 16));
        assert_eq!(cells(&p, 36), "x.....x...x.....");
        assert_eq!(cells(&p, 38), "....x.......x...");
        assert_eq!(cells(&p, 42), "x.x.x.x.x.x.x.x.");
    }

    #[test]
    fn a_drum_tab_two_bars() {
        let p = read("HH|x-x-x-x-|x-x-x-x-|\nSD|----o---|----o---|\nBD|o-------|o-o-----|");
        assert_eq!((p.bars, p.steps), (2, 8));
        assert_eq!(cells(&p, 38), "....x.......x...");
        assert_eq!(cells(&p, 36), "x.......x.x.....");
    }

    #[test]
    fn step_strings_binary_velocity_and_hex() {
        let p = read("kick: 1000100010001000\nsnare: 0x0808\nhat: 4040404040404040\n");
        assert_eq!(cells(&p, 36), "x...x...x...x...");
        assert_eq!(cells(&p, 38), "....x.......x...");
        assert_eq!(cells(&p, 42), "x.x.x.x.x.x.x.x.");
        let q = read("kick: 8888h");
        assert_eq!(cells(&q, 36), "x...x...x...x...");
    }

    #[test]
    fn box_notation_and_the_beats_rule() {
        let p = read("@time 4/4\n@lane claves\nx..x..x...x.x...");
        assert_eq!(cells(&p, 75), "x..x..x...x.x...");
        // Four groups in 4/4 are beats.
        let q = read("kick: x...|x...|x...|x...\n");
        assert_eq!((q.bars, q.steps), (1, 16));
    }

    #[test]
    fn onsets_intervals_and_euclid() {
        let a = read("{0,3,6,10,12}/16");
        assert_eq!(cells(&a, 75), "x..x..x...x.x...");
        let b = read("3+3+4+2+4");
        assert_eq!(cells(&b, 75), "X..x..x...x.x...");
        let c = read("E(3,8)");
        assert_eq!(cells(&c, 75), "x..x..x.");
        let d = read("E(5,8,2)");
        assert_eq!(d.hits(), 5);
        assert_eq!(read("kick: (3,8)").hits(), 3);
    }

    #[test]
    fn tala_theka_and_bols() {
        let p = read("tala teentaal");
        assert_eq!((p.meter().as_str(), p.bars, p.steps), ("4/4", 4, 16));
        assert_eq!(p.marks.iter().filter(|m| **m != ' ').count(), 4);
        assert_eq!(p.bols.iter().filter(|b| !b.is_empty()).count(), 16);
        assert_eq!(read("tala jhaptaal").bars, 2);
        assert_eq!(read("tala ektaal").hits() > 12, true);
        let q = read("Dha Dhin Dhin Dha | Dha Dhin Dhin Dha");
        assert_eq!((q.bars, q.steps), (2, 16));
        assert!(parse("Dha Zzz Dha Dha", Some(Notation::Bols), &Opts::default()).is_err());
    }

    #[test]
    fn syllables_counts_and_values() {
        let a = read("ta ti-ti ta ti-ti");
        assert_eq!((a.steps, cells(&a, 75).as_str()), (8, "x.xxx.xx"));
        let b = read("ta-ka-di-mi ta z ta");
        assert_eq!(cells(&b, 75), "xxxxx.......x...");
        let c = read("ta-a ta-ki-da z");
        assert_eq!((c.bars, c.steps), (1, 12));
        let d = read("1 & 2 e & a 3 4");
        assert_eq!(cells(&d, 75), "x.x.xxxxx...x...");
        let e = read("q q e e q");
        assert_eq!((e.steps, cells(&e, 75).as_str()), (8, "x.x.xxx."));
        let f = read("q 3(e e e) h");
        assert_eq!((f.steps, f.hits()), (12, 5));
        let g = read("q. e h");
        assert_eq!(cells(&g, 75), "x..xx...");
    }

    #[test]
    fn lilypond_drum_mode() {
        let p = read("\\drummode { bd4 hh8 hh sn4 <bd hh>8 r }");
        assert_eq!((p.bars, p.steps), (1, 8));
        assert_eq!(cells(&p, 36), "x.....x.");
        assert_eq!(cells(&p, 38), "....x...");
        assert_eq!(cells(&p, 42), "..xx..x.");
        let q = read("\\drummode { \\time 7/8 bd8 sn bd sn \\tuplet 3/2 { hh16 hh hh } hh8 }");
        assert_eq!((q.beats, q.unit), (7, 8));
        assert_eq!(read("\\drummode { \\repeat unfold 4 { bd4 } }").hits(), 4);
    }

    #[test]
    fn stickings_and_rudiments() {
        let p = read("R L R R L R L L");
        assert_eq!((p.bars, p.steps), (1, 64));
        assert_eq!(p.hits(), 16);
        let f = read("(l)R (r)L R L");
        assert!(f.hits() > 4);
        assert_eq!(read("RLRR LRLL").lanes[0].key, 38);
    }

    #[test]
    fn the_kit_s_json() {
        let p = read(r#"{"time_signature":"4/4","pattern_grid":{"resolution":16,"kick":[1,0,0,0,0,0,1,0,0,0,1,0,0,0,0,0],"snare":[0,0,0,0,1,0,0,0,0,0,0,0,1,0,0,0]},"instrument_map":{"kick":36,"snare":38}}"#);
        assert_eq!(cells(&p, 36), "x.....x...x.....");
        assert_eq!(cells(&p, 38), "....x.......x...");
    }

    #[test]
    fn mini_notation_by_neumetik() {
        let p = read("s(\"bd ~ sd ~ bd ~ sd ~\")");
        assert_eq!(cells(&p, 36), "x...x...");
        assert_eq!(cells(&p, 38), "..x...x.".replace("..x...x.", "..x...x."));
    }

    #[test]
    fn drumscript_blocks_and_sequences() {
        let basic = read("hh8 hh8 hh8 hh8 hh8 hh8 hh8 hh8\nbd4     sn4     bd4     sn4\n---\nbd c2\nsn d2\nhh f2");
        assert_eq!((basic.bars, basic.steps), (1, 8));
        assert_eq!(cells(&basic, 42), "xxxxxxxx");
        assert_eq!(cells(&basic, 36), "x...x...");
        let src = "loop-a (\n  [hh8] * 8\n  bd4 sn4 bd4 sn4\n)\n\nfill-a (\n  tomh16 tomml16 toml16 tomfl16\n)\n\n4/4\nloop-a * 7\nfill-a\n";
        let p = read(src);
        assert_eq!((p.bars, p.steps), (8, 16));
        assert_eq!(p.hits(), 7 * 12 + 4);
        assert_eq!(cells(&p, 50).chars().skip(7 * 16).take(16).collect::<String>(), "x...............");
        assert_eq!(cells(&p, 41).chars().skip(7 * 16).take(16).collect::<String>(), "...x............");
        assert_eq!(detect(src), Some(Notation::Drumscript));
    }

    #[test]
    fn drumscript_odds_and_ends() {
        // A rest, a dot, a repeated note, a one-line block, the mapping naming an unknown drum.
        let p = read("bd4. r8 bd4 * 2\n");
        assert_eq!((p.beats, p.unit, p.steps), (4, 4, 4));
        assert_eq!(cells(&p, 36), "x.xx");
        let q = read("3/4\ng (bd4 sn4 bd4)\ng * 2");
        assert_eq!((q.beats, q.unit, q.bars, q.steps), (3, 4, 2, 3));
        let m = parse("zap8 r8\n---\nzap cis2", Some(Notation::Drumscript), &Opts::default()).expect("mapped drum");
        assert_eq!(cells(&m, 37), "x");
        assert_eq!(ds_note("c2"), Some(36));
        assert_eq!(ds_note("fis2"), Some(42));
        assert_eq!(ds_note("bb1"), Some(34));
        assert_eq!(ds_note("es3"), Some(51));
        let e = |t: &str| parse(t, Some(Notation::Drumscript), &Opts::default()).unwrap_err();
        assert!(e("4/4\ng (bd4)\ng\n3/4\ng").contains("changes meter"));
        assert!(e("bd").contains("not a drum with a length"));
        assert!(e("nope4").contains("nope"));
        assert!(e("[bd4").contains("no ]"));
        assert!(e("a (bd4").contains("not closed"));
        // Mini-notation and the others are not taken for it.
        assert_ne!(detect("s(\"bd ~ sd ~\")"), Some(Notation::Drumscript));
        assert_ne!(detect("kick: x...x..."), Some(Notation::Drumscript));
        assert_ne!(detect("q e e q"), Some(Notation::Drumscript));
    }

    #[test]
    fn mistakes_say_what_is_wrong() {
        assert!(parse("Kick | x...\nSnare | x..", None, &Opts::default()).unwrap_err().contains("cells"));
        assert!(parse("Banjo: x...", Some(Notation::Lanes), &Opts::default()).unwrap_err().contains("Banjo"));
        assert!(parse("@time 4/3\nx...", None, &Opts::default()).is_err());
        assert!(parse("", None, &Opts::default()).is_err());
        assert!(parse("qwerty zxcv", None, &Opts::default()).is_err());
    }
}
