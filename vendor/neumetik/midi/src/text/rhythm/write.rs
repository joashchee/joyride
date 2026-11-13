// Copyright 2026 ansiapps. Neumetik MIDI, MIT licensed: see LICENSE.

//! A pattern shown in a notation: as text lines for the screen, or as
//! text to save. Every line is ASCII (the VGA font's), and a rendering
//! of one bar is at most 64 columns wide where it can be: a pattern that
//! is wider is shown a cell to a column.

use super::names;
use super::read::Opts;
use super::{Lane, Pattern, ACCENT, HIT, REST, SOFT};
use crate::score::{Note as ScoreNote, Part, DRUMS};
use crate::Score;

/// The formats a pattern is written in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Grid,
    Tab,
    Staff,
    Onsets,
    Intervals,
    Hex,
    Euclid,
    Tala,
    Lilypond,
    Drumscript,
    Hydrogen,
    Mini,
    Abc,
    MusicXml,
    Kern,
}

pub const FORMATS: [(Format, &str, &str); 15] = [
    (Format::Grid, "grid", "Drum grid"),
    (Format::Tab, "tab", "Drum tab"),
    (Format::Staff, "staff", "Percussion staff"),
    (Format::Onsets, "onsets", "Onset sets"),
    (Format::Intervals, "intervals", "Intervals"),
    (Format::Hex, "hex", "Step hex"),
    (Format::Euclid, "euclid", "Euclidean"),
    (Format::Tala, "tala", "Bols and tala"),
    (Format::Lilypond, "lilypond", "LilyPond drums"),
    (Format::Drumscript, "drumscript", "Drumscript"),
    (Format::Hydrogen, "hydrogen", "Hydrogen pattern"),
    (Format::Mini, "mini", "Strudel / Tidal"),
    (Format::Abc, "abc", "ABC"),
    (Format::MusicXml, "musicxml", "MusicXML"),
    (Format::Kern, "kern", "Humdrum kern"),
];

impl Format {
    pub fn by_name(name: &str) -> Option<Format> {
        let n = name.trim().to_ascii_lowercase();
        FORMATS.iter().find(|f| f.1 == n).map(|f| f.0)
    }

    pub fn name(self) -> &'static str {
        FORMATS.iter().find(|f| f.0 == self).map_or("", |f| f.1)
    }
}

/// The pattern in a format. `bar` picks one screen page of a grid or a staff
/// (see [`pages`]; past the last, all of them in turn); the others show the
/// whole cycle.
pub fn render(p: &Pattern, f: Format, bar: u32) -> Result<String, String> {
    Ok(match f {
        Format::Grid => grid(p, bar),
        Format::Tab => tab(p),
        Format::Staff => staff(p, bar),
        Format::Onsets => onsets(p),
        Format::Intervals => intervals(p),
        Format::Hex => hex(p),
        Format::Euclid => euclid_text(p),
        Format::Tala => tala(p),
        Format::Lilypond => lilypond(p)?,
        Format::Drumscript => drumscript(p)?,
        Format::Hydrogen => super::hydrogen::write(p)?,
        Format::Mini | Format::Abc | Format::MusicXml | Format::Kern => {
            let s = to_score(p, 120);
            let ext = match f {
                Format::Mini => "mini",
                Format::Abc => "abc",
                Format::MusicXml => "musicxml",
                _ => "krn",
            };
            crate::text::write(&s, if p.title.is_empty() { "Rhythm" } else { &p.title }, ext).ok_or_else(|| format!("Neumetik has no writer for {}", f.name()))?
        }
    })
}

/// A pattern as a score of one drum part, to write in Neumetik's formats
/// or play (`Score::to_midi`).
pub fn to_score(p: &Pattern, bpm: u16) -> Score {
    let mut s = Score::new();
    s.ppq = super::PPQ as u16;
    s.bpm = f64::from(bpm);
    s.beats = p.beats;
    s.unit = p.unit;
    s.bars = p.bars;
    let notes = p.notes().into_iter().map(|(at, len, key, vel)| ScoreNote { at, length: len, key, velocity: vel }).collect();
    s.parts = vec![Part { name: "Drums".into(), channel: DRUMS, bank: 0, program: 0, notes, kept: vec![] }];
    s
}

fn mark(level: u8, head: char, tab: bool) -> char {
    match level {
        REST => '.',
        SOFT => {
            if tab {
                'g'
            } else {
                'o'
            }
        }
        HIT => {
            if tab {
                head
            } else {
                'x'
            }
        }
        _ => {
            if tab {
                head.to_ascii_uppercase()
            } else {
                'X'
            }
        }
    }
}

fn short(key: u8) -> String {
    names::display(key)
}

/// The counting that goes over a bar's cells: `1 e & a 2 e & a`.
fn counting(p: &Pattern) -> Vec<String> {
    let (steps, beats) = (p.steps as usize, usize::from(p.beats));
    let compound = p.unit == 8 && beats % 3 == 0 && beats >= 6 && steps == beats;
    if compound {
        return (0..steps).map(|i| ["1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15"][i / 3].to_string()).enumerate().map(|(i, s)| if i % 3 == 0 { s } else if i % 3 == 1 { "la".into() } else { "li".into() }).collect();
    }
    if steps % beats != 0 {
        return (1..=steps).map(|i| i.to_string()).collect();
    }
    let spb = steps / beats;
    (0..steps)
        .map(|i| {
            let (b, s) = (i / spb + 1, i % spb);
            match (spb, s) {
                (_, 0) => b.to_string(),
                (2, 1) => "&".into(),
                (3, 1) => "la".into(),
                (3, 2) => "li".into(),
                (4, 1) => "e".into(),
                (4, 2) => "&".into(),
                (4, 3) => "a".into(),
                _ => ".".into(),
            }
        })
        .collect()
}

/// How a bar is cut into screen pages: (cells a page, columns a cell).
/// A bar of up to 27 cells is one page of two columns a cell; a longer
/// one is cut into pages of up to 48 cells of a column each, so that
/// nothing is wider than 64 columns and nothing scrolls.
pub fn page_plan(steps: u32) -> (u32, usize) {
    if steps <= 27 {
        (steps, 2)
    } else {
        let parts = steps.div_ceil(48);
        (steps.div_ceil(parts), 1)
    }
}

/// How many screen pages a grid or staff of the pattern is (a bar, or
/// a part of a long one, a page).
pub fn pages(p: &Pattern) -> u32 {
    p.bars * p.steps.div_ceil(page_plan(p.steps).0)
}

/// The (bar, first cell, last cell + 1) of the pages `page` picks: one, or
/// all of them for a `page` past the last.
fn page_cuts(p: &Pattern, page: u32) -> Vec<(u32, usize, usize)> {
    let (per, _) = page_plan(p.steps);
    let parts = p.steps.div_ceil(per);
    let one = |n: u32| {
        let (b, s) = (n / parts, n % parts);
        (b, (s * per) as usize, (((s + 1) * per).min(p.steps)) as usize)
    };
    if page < pages(p) {
        vec![one(page)]
    } else {
        (0..pages(p)).map(one).collect()
    }
}

fn page_title(p: &Pattern, bar: u32, from: usize, to: usize) -> Option<String> {
    let (per, _) = page_plan(p.steps);
    let parts = p.steps.div_ceil(per);
    if p.bars == 1 && parts == 1 {
        return None;
    }
    let part = if parts > 1 { format!(", cells {} to {}", from + 1, to) } else { String::new() };
    Some(format!("Bar {}/{}{part}", bar + 1, p.bars))
}

/// A drum grid, a screen page at a time (`page` past the last shows all):
/// a counting header, a row a drum, one cell a column pair (or a column
/// for a long bar). X accent, x hit, o soft, . nothing.
pub fn grid(p: &Pattern, page: u32) -> String {
    let mut out = Vec::new();
    let lw = p.shown().iter().map(|l| short(l.key).len()).max().unwrap_or(4).max(4);
    let (_, w) = page_plan(p.steps);
    let count = counting(p);
    for (b, from, to) in page_cuts(p, page) {
        if let Some(t) = page_title(p, b, from, to) {
            out.push(t);
        }
        let mut head = format!("{:lw$} |", "", lw = lw);
        for c in &count[from..to] {
            head.push_str(&format!("{:<w$}", &c[..c.len().min(w)], w = w));
        }
        out.push(head.trim_end().to_string());
        for l in p.shown() {
            let mut row = format!("{:lw$} |", short(l.key), lw = lw);
            for s in from..to {
                row.push(mark(l.cells[(b * p.steps) as usize + s], 'x', false));
                if w == 2 {
                    row.push(' ');
                }
            }
            out.push(row.trim_end().to_string());
        }
    }
    out.join("\n")
}

/// An ASCII drum tab: two letters a drum, `|` between bars; x and o
/// for a hit by the drum's notehead, X and O an accent, g a ghost.
pub fn tab(p: &Pattern) -> String {
    let mut out = Vec::new();
    for l in p.shown() {
        let mut row = format!("{:<2}|", names::label(l.key));
        for b in 0..p.bars as usize {
            for s in 0..p.steps as usize {
                let c = match l.cells[b * p.steps as usize + s] {
                    REST => '-',
                    lvl => mark(lvl, names::head(l.key), true),
                };
                row.push(c);
            }
            row.push('|');
        }
        out.push(row);
    }
    out.join("\n")
}

fn lane_onsets(l: &Lane) -> Vec<(usize, u8)> {
    l.cells.iter().enumerate().filter(|(_, c)| **c != REST).map(|(i, c)| (i, *c)).collect()
}

/// Onset sets, a line a drum: `kick: {0,6,10}/16` (`!` an accent, `?` soft).
pub fn onsets(p: &Pattern) -> String {
    let n = p.len();
    p.shown()
        .iter()
        .map(|l| {
            let list: Vec<String> = lane_onsets(l).into_iter().map(|(i, c)| format!("{i}{}", if c == ACCENT { "!" } else if c == SOFT { "?" } else { "" })).collect();
            format!("{}: {{{}}}/{n}", names::name(l.key), list.join(","))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The gaps between a drum's hits, round the cycle: `kick: 6+4+6`. A
/// drum whose first hit is not the first cell says where it starts.
pub fn intervals(p: &Pattern) -> String {
    let n = p.len();
    p.shown()
        .iter()
        .map(|l| {
            let on = lane_onsets(l);
            let first = on[0].0;
            let mut gaps: Vec<usize> = on.windows(2).map(|w| w[1].0 - w[0].0).collect();
            gaps.push(n - on[on.len() - 1].0 + first);
            let list: Vec<String> = gaps.iter().map(usize::to_string).collect();
            format!("{}: {}{}", names::name(l.key), list.join("+"), if first > 0 { format!(" (from {first} of {n})") } else { String::new() })
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// A drum's cells a bar as hexadecimal (a hit a bit), where a bar is a
/// whole number of nibbles; else as 0s and 1s.
pub fn hex(p: &Pattern) -> String {
    p.shown()
        .iter()
        .map(|l| {
            let bars: Vec<String> = (0..p.bars as usize)
                .map(|b| {
                    let cells = &l.cells[b * p.steps as usize..(b + 1) * p.steps as usize];
                    if cells.len() % 4 == 0 {
                        let digits: String = cells.chunks(4).map(|c| char::from_digit(c.iter().fold(0u32, |a, x| a << 1 | u32::from(*x != REST)), 16).unwrap_or('0')).collect();
                        format!("0x{digits}")
                    } else {
                        cells.iter().map(|c| if *c == REST { '0' } else { '1' }).collect()
                    }
                })
                .collect();
            format!("{}: {}", names::name(l.key), bars.join(" "))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The Euclidean rhythm a drum's cycle is, if it is one.
pub fn euclid_of(l: &Lane) -> Option<(usize, usize, usize)> {
    let n = l.cells.len();
    let k = l.cells.iter().filter(|c| **c != REST).count();
    if k == 0 || k > n {
        return None;
    }
    let bools: Vec<bool> = l.cells.iter().map(|c| *c != REST).collect();
    (0..n).find(|r| crate::text::mini::euclid(k, n, *r as i64) == bools).map(|r| (k, n, r))
}

/// The drums that are Euclidean rhythms, named: `kick: E(3,8)`.
pub fn euclid_text(p: &Pattern) -> String {
    let rows: Vec<String> = p.shown().iter().map(|l| match euclid_of(l) {
        Some((k, n, 0)) => format!("{}: E({k},{n})", names::name(l.key)),
        Some((k, n, r)) => format!("{}: E({k},{n},{r})", names::name(l.key)),
        None => format!("{}: not Euclidean", names::name(l.key)),
    }).collect();
    rows.join("\n")
}

/// Bols under their marks, a pulse a column, a row of up to eight pulses
/// at a time, `|` where a vibhag (a group) starts. Without bols or
/// marks, a plain count of pulses.
pub fn tala(p: &Pattern) -> String {
    let per = (p.steps as usize / usize::from(p.beats)).max(1);
    let per = if p.unit == 8 { per.max(1) } else { per };
    let total = p.len();
    let pulses: Vec<usize> = (0..total).step_by(per).collect();
    let mut out = Vec::new();
    let has_bols = p.bols.iter().any(|b| !b.is_empty());
    let mut row_no = 0;
    for chunk in pulses.chunks(8) {
        let mut marks = String::new();
        let mut bols = String::new();
        let mut count = String::new();
        for at in chunk {
            let m = p.marks.get(*at).copied().filter(|c| *c != ' ');
            let sep = if m.is_some() { '|' } else { ' ' };
            marks.push(sep);
            bols.push(sep);
            count.push(sep);
            let b: String = (0..per).filter_map(|i| p.bols.get(at + i).filter(|b| !b.is_empty()).map(|b| b.to_string())).collect::<Vec<_>>().join("-");
            let b = if b.is_empty() {
                let hit = p.lanes.iter().any(|l| (0..per).any(|i| l.cells.get(at + i).is_some_and(|c| *c != REST)));
                if hit { "*".to_string() } else { "-".to_string() }
            } else {
                b
            };
            marks.push_str(&format!("{:<6}", m.map(|c| c.to_string()).unwrap_or_default()));
            bols.push_str(&format!("{b:<6}"));
            count.push_str(&format!("{:<6}", pulses.iter().position(|x| x == at).map_or(0, |i| i + 1)));
        }
        let _ = has_bols;
        if row_no == 0 && p.marks.iter().any(|c| *c != ' ') || p.marks.iter().any(|c| *c != ' ') {
            out.push(marks.trim_end().to_string());
        }
        out.push(bols.trim_end().to_string());
        out.push(count.trim_end().to_string());
        row_no += 1;
    }
    out.join("\n")
}

/// A percussion staff, a screen page at a time (see [`grid`]): five lines, the drums
/// by their place on a kit's staff (`names::KIT`), a cross for a cymbal
/// or hat (x, X accent), a round head for a drum (o, O accent), . a
/// ghost note; a drum with no place on the kit's staff gets a row of
/// its own under it, named.
pub fn staff(p: &Pattern, page: u32) -> String {
    let w = page_plan(p.steps).1;
    let mut out = Vec::new();
    for (b, from, to) in page_cuts(p, page) {
        if let Some(t) = page_title(p, b, from, to) {
            out.push(t);
        }
        let lanes = p.shown();
        let at = |l: &Lane, s: usize| l.cells[b as usize * p.steps as usize + s];
        let on_staff: Vec<&&Lane> = lanes.iter().filter(|l| names::staff(l.key).is_some()).collect();
        if !on_staff.is_empty() {
            let places: Vec<i8> = on_staff.iter().filter_map(|l| names::staff(l.key)).map(|s| s.0).collect();
            let (hi, lo) = (places.iter().copied().max().unwrap_or(8).max(8), places.iter().copied().min().unwrap_or(0).min(0));
            for pos in (lo..=hi).rev() {
                // The five lines are 0, 2, 4, 6 and 8; a note off them has a short
                // ledger line, drawn only where it sounds.
                let line = (0..=8).contains(&pos) && pos % 2 == 0;
                let ledger = !(0..=8).contains(&pos) && pos % 2 == 0;
                let mut row = String::new();
                for s in from..to {
                    let here = on_staff.iter().find_map(|l| {
                        let (pp, head) = names::staff(l.key)?;
                        (pp == pos && at(l, s) != REST).then_some((head, at(l, s)))
                    });
                    let fill = if line || (ledger && here.is_some()) { '-' } else { ' ' };
                    let c = match here {
                        Some((head, ACCENT)) => head.to_ascii_uppercase(),
                        Some((_, SOFT)) => '.',
                        Some((head, _)) => head,
                        None => fill,
                    };
                    row.push(c);
                    if w == 2 {
                        row.push(fill);
                    }
                }
                out.push(format!("{}{}", if line { "-" } else { " " }, row.trim_end()) + if line { "|" } else { "" });
            }
        }
        for l in lanes.iter().filter(|l| names::staff(l.key).is_none()) {
            let mut row = String::new();
            for s in from..to {
                row.push(match at(l, s) {
                    REST => '-',
                    ACCENT => names::head(l.key).to_ascii_uppercase(),
                    SOFT => '.',
                    _ => names::head(l.key),
                });
                if w == 2 {
                    row.push('-');
                }
            }
            out.push(format!("-{row}| {}", short(l.key)));
        }
    }
    out.join("\n")
}

// ---- LilyPond ----

fn lily_name(key: u8) -> &'static str {
    match key {
        35 => "bda",
        36 => "bd",
        37 => "ss",
        38 => "sn",
        39 => "hc",
        40 => "sne",
        41 => "tomfl",
        42 => "hh",
        43 => "tomfh",
        44 => "hhp",
        45 => "toml",
        46 => "hho",
        47 => "tomml",
        48 => "tommh",
        49 => "cymc",
        50 => "tomh",
        51 => "cymr",
        52 => "cymch",
        53 => "rb",
        54 => "tamb",
        55 => "cyms",
        56 => "cb",
        57 => "cymcb",
        58 => "vibs",
        59 => "cymrb",
        60 => "boh",
        61 => "bol",
        62 => "cghm",
        63 => "cgho",
        64 => "cgl",
        65 => "timh",
        66 => "timl",
        67 => "agh",
        68 => "agl",
        69 => "cab",
        70 => "mar",
        71 => "whs",
        72 => "whl",
        73 => "guis",
        74 => "guil",
        75 => "cl",
        76 => "wbh",
        77 => "wbl",
        78 => "cuim",
        79 => "cuio",
        80 => "trim",
        _ => "tri",
    }
}

/// LilyPond drum mode: every cell a note of its length, rests between,
/// chords where drums sound together; triplet cells in `\tuplet 3/2`.
/// Only for cells that are a power-of-two note (or a triplet of one).
pub fn lilypond(p: &Pattern) -> Result<String, String> {
    // A cell's length: beats/unit/steps of a whole note.
    let (num, den) = (u64::from(p.beats), u64::from(p.unit) * u64::from(p.steps));
    let g = {
        let (mut a, mut b) = (num, den);
        while b != 0 {
            (a, b) = (b, a % b);
        }
        a
    };
    let (n, d) = (num / g, den / g);
    let (dur, tuplet) = if n == 1 && d.is_power_of_two() {
        (d, false)
    } else if n == 1 && d % 3 == 0 && (d / 3).is_power_of_two() {
        (d / 3 * 2, true)
    } else {
        return Err(format!("a cell of {n}/{d} of a whole note has no LilyPond duration"));
    };
    let mut out = format!("\\drummode {{\n  \\time {}/{}\n", p.beats, p.unit);
    let lanes = p.shown();
    for b in 0..p.bars as usize {
        let mut line = String::from("  ");
        let mut cells: Vec<String> = Vec::new();
        for s in 0..p.steps as usize {
            let at = b * p.steps as usize + s;
            let hits: Vec<(&Lane, u8)> = lanes.iter().filter(|l| l.cells[at] != REST).map(|l| (*l, l.cells[at])).collect();
            let accent = hits.iter().any(|h| h.1 == ACCENT);
            cells.push(match hits.len() {
                0 => format!("r{dur}"),
                1 => format!("{}{dur}{}", lily_name(hits[0].0.key), if accent { "->" } else { "" }),
                _ => format!("<{}>{dur}{}", hits.iter().map(|h| lily_name(h.0.key)).collect::<Vec<_>>().join(" "), if accent { "->" } else { "" }),
            });
        }
        if tuplet {
            for g in cells.chunks(3) {
                line.push_str(&format!("\\tuplet 3/2 {{ {} }} ", g.join(" ")));
            }
        } else {
            line.push_str(&cells.join(" "));
        }
        out.push_str(line.trim_end());
        out.push_str(" |\n");
    }
    out.push_str("}\n");
    Ok(out)
}

/// A key as a note name for drumscript's mapping: `c2` is 36, sharps as
/// LilyPond writes them (`fis2`).
fn note_name(key: u8) -> String {
    const PC: [&str; 12] = ["c", "cis", "d", "dis", "e", "f", "fis", "g", "gis", "a", "ais", "b"];
    format!("{}{}", PC[usize::from(key % 12)], i32::from(key / 12) - 1)
}

/// Drumscript: a line a drum (a voice), a block a distinct bar when there
/// are several, `[hh8] * 8` for runs, and the `---` mapping to General MIDI
/// keys. Drumscript has no soft hit or accent, so every hit is the same
/// there; only for cells that are a power-of-two note (drumscript has no
/// tuplets).
pub fn drumscript(p: &Pattern) -> Result<String, String> {
    let (num, den) = (u64::from(p.beats), u64::from(p.unit) * u64::from(p.steps));
    let g = {
        let (mut a, mut b) = (num, den);
        while b != 0 {
            (a, b) = (b, a % b);
        }
        a
    };
    let (n, d) = (num / g, den / g);
    if n != 1 || !d.is_power_of_two() || d > 64 {
        return Err(format!("a cell of {n}/{d} of a whole note has no drumscript length"));
    }
    let lanes = p.shown();
    let steps = p.steps as usize;
    let bar_lines = |b: usize| -> Vec<String> {
        lanes
            .iter()
            .map(|l| {
                let toks: Vec<String> = (0..steps).map(|s| if l.cells[b * steps + s] == REST { format!("r{d}") } else { format!("{}{d}", lily_name(l.key)) }).collect();
                let mut parts: Vec<String> = Vec::new();
                let mut i = 0;
                while i < toks.len() {
                    let run = toks[i..].iter().take_while(|t| **t == toks[i]).count();
                    if run >= 3 {
                        parts.push(format!("[{}] * {run}", toks[i]));
                    } else {
                        parts.extend(std::iter::repeat(toks[i].clone()).take(run));
                    }
                    i += run;
                }
                parts.join(" ")
            })
            .collect()
    };
    let mut out = String::new();
    if p.bars == 1 {
        if (p.beats, p.unit) != (4, 4) {
            out.push_str(&format!("{}/{}\n", p.beats, p.unit));
        }
        for l in bar_lines(0) {
            out.push_str(&l);
            out.push('\n');
        }
    } else {
        let mut distinct: Vec<Vec<String>> = Vec::new();
        let mut seq: Vec<(usize, u32)> = Vec::new();
        for b in 0..p.bars as usize {
            let lines = bar_lines(b);
            let id = distinct.iter().position(|x| *x == lines).unwrap_or_else(|| {
                distinct.push(lines);
                distinct.len() - 1
            });
            match seq.last_mut() {
                Some(last) if last.0 == id => last.1 += 1,
                _ => seq.push((id, 1)),
            }
        }
        let name = |i: usize| -> String { format!("bar-{}", if i < 26 { ((b'a' + i as u8) as char).to_string() } else { (i + 1).to_string() }) };
        for (i, lines) in distinct.iter().enumerate() {
            out.push_str(&format!("{} (\n", name(i)));
            for l in lines {
                out.push_str(&format!("  {l}\n"));
            }
            out.push_str(")\n\n");
        }
        out.push_str(&format!("{}/{}\n", p.beats, p.unit));
        for (id, times) in seq {
            if times > 1 {
                out.push_str(&format!("{} * {times}\n", name(id)));
            } else {
                out.push_str(&format!("{}\n", name(id)));
            }
        }
    }
    out.push_str("---\n");
    for l in &lanes {
        out.push_str(&format!("{} {}\n", lily_name(l.key), note_name(l.key)));
    }
    Ok(out)
}

/// Reads a text and writes it in another notation: `convert(text, None,
/// Format::Staff)`.
pub fn convert(text: &str, from: Option<super::Notation>, to: Format) -> Result<String, String> {
    let p = super::read::parse(text, from, &Opts::default())?;
    render(&p, to, u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::super::read::{parse, Opts};
    use super::*;

    fn rock() -> Pattern {
        parse("kick: X-------x-x-----\nsnare: ----X-------X---\nhat: x-x-x-x-x-x-x-x-\nopenhat: ..............x.", None, &Opts::default()).expect("rock")
    }

    #[test]
    fn a_grid_fits_the_screen_and_counts() {
        let g = grid(&rock(), 0);
        assert!(g.lines().all(|l| l.len() <= 64 && l.is_ascii()), "{g}");
        assert!(g.lines().next().unwrap_or("").contains("1 e & a 2"), "{g}");
        assert!(g.contains("kick") && g.contains("X . . . . . . . x"), "{g}");
    }

    #[test]
    fn every_text_notation_reads_back_to_the_same_pattern() {
        let p = rock();
        for (f, from) in [(Format::Tab, super::super::Notation::Lanes), (Format::Onsets, super::super::Notation::Onsets), (Format::Hex, super::super::Notation::Lanes), (Format::Grid, super::super::Notation::Lanes)] {
            let text = render(&p, f, u32::MAX).expect("renders");
            let text = if f == Format::Grid { text.lines().skip(1).map(|l| l.replace(" |", ":")).collect::<Vec<_>>().join("\n") } else { text };
            let mut q = parse(&text, Some(from), &Opts::default()).unwrap_or_else(|e| panic!("{f:?}: {e}\n{text}"));
            q.tidy();
            let mut a = p.clone();
            a.tidy();
            if f == Format::Hex {
                // Hexadecimal keeps where, not how loud.
                for l in a.lanes.iter_mut() {
                    l.cells.iter_mut().for_each(|c| *c = if *c == 0 { 0 } else { 2 });
                }
            }
            assert_eq!(q.lanes, a.lanes, "{f:?}\n{text}");
        }
    }

    #[test]
    fn lilypond_and_mini_read_back() {
        let p = rock();
        let ly = lilypond(&p).expect("lilypond");
        let q = parse(&ly, None, &Opts::default()).unwrap_or_else(|e| panic!("{e}\n{ly}"));
        assert_eq!(q.lanes.len(), p.lanes.len(), "{ly}");
        for l in &p.lanes {
            // A chord's accent is the whole chord's, so only where is compared.
            let wh = |c: &Vec<u8>| c.iter().map(|x| *x != 0).collect::<Vec<bool>>();
            assert_eq!(q.lanes.iter().find(|x| x.key == l.key).map(|x| wh(&x.cells)), Some(wh(&l.cells)), "{ly}");
        }
        let mini = render(&p, Format::Mini, 0).expect("mini");
        let r = parse(&mini, None, &Opts::default()).unwrap_or_else(|e| panic!("{e}\n{mini}"));
        assert_eq!(r.hits(), p.hits(), "{mini}");
    }

    #[test]
    fn the_staff_puts_the_kick_low_and_the_hat_high() {
        let s = staff(&rock(), 0);
        let rows: Vec<&str> = s.lines().collect();
        let hat = rows.iter().position(|r| r.contains('x')).expect("hat");
        let kick = rows.iter().position(|r| r.contains('O')).expect("kick");
        assert!(hat < kick, "{s}");
        assert!(rows.iter().all(|r| r.len() <= 64 && r.is_ascii()), "{s}");
    }

    #[test]
    fn euclid_is_found() {
        let p = parse("E(5,8,2)", None, &Opts::default()).expect("e");
        assert_eq!(euclid_text(&p), "claves: E(5,8,2)");
        assert!(render(&p, Format::Intervals, 0).expect("i").contains("claves:"));
    }

    #[test]
    fn neumetiks_other_writers_take_a_pattern() {
        let p = rock();
        for f in [Format::Abc, Format::MusicXml, Format::Kern] {
            let t = render(&p, f, 0);
            assert!(t.as_ref().is_ok_and(|t| t.len() > 20) || t.is_err(), "{f:?}");
        }
        let midi = to_score(&p, 100).to_midi();
        assert!(midi.starts_with(b"MThd"));
        let back = super::super::read::parse_bytes(&midi, None, &Opts::default()).expect("midi back");
        assert_eq!(back.hits(), p.hits());
    }

    #[test]
    fn drumscript_reads_back() {
        let p = rock();
        let ds = drumscript(&p).expect("drumscript");
        assert!(ds.contains("---") && ds.contains("kick") == false && ds.contains("bd c2"), "{ds}");
        let q = parse(&ds, None, &Opts::default()).unwrap_or_else(|e| panic!("{e}\n{ds}"));
        // The same hits at the same times (the reader keeps the coarsest grid they sit on).
        let wh = |x: &Pattern| x.notes().into_iter().map(|n| (n.0, n.2)).collect::<Vec<_>>();
        assert_eq!((q.beats, q.unit, q.bars), (p.beats, p.unit, p.bars), "{ds}");
        assert_eq!(wh(&q), wh(&p), "{ds}");
        // Several bars come out as blocks and a sequence, and read back.
        let two = parse("kick: x...x...x...x...|x...x...x.x.x...\nhat: x-x-x-x-x-x-x-x-|x-x-x-x-x-x-x-x-", None, &Opts::default()).expect("two bars");
        let ds2 = drumscript(&two).expect("drumscript");
        assert!(ds2.contains("bar-a (") && ds2.contains("bar-b ("), "{ds2}");
        let back = parse(&ds2, None, &Opts::default()).unwrap_or_else(|e| panic!("{e}\n{ds2}"));
        assert_eq!((back.bars, wh(&back)), (two.bars, wh(&two)), "{ds2}");
        // A triplet cell has no drumscript length.
        let trip = parse("@time 4/4\nkick: x..x..x..x..", Some(super::super::Notation::Lanes), &Opts::default()).expect("12 steps");
        assert!(drumscript(&trip).is_err());
    }

    #[test]
    fn a_tala_shows_its_marks() {
        let p = parse("tala teentaal", None, &Opts::default()).expect("teentaal");
        let t = tala(&p);
        assert!(t.contains("Dhin") && t.contains('X') && t.lines().all(|l| l.len() <= 64), "{t}");
    }
}
