// Copyright 2026 ansiapps. Neumetik MIDI, MIT licensed: see LICENSE.

//! **Rhythm notation**: a drum or rhythm pattern, however it was written.
//!
//! One shape, [`Pattern`]: a cycle of bars, each a row of cells (steps),
//! a lane of cells for each drum (a General MIDI key), a cell a level
//! (rest, soft, hit, accent). Every notation the reference kit
//! (`docs/rhythm-patterns-reference-kit.md`) names, and the others
//! rhythm is written in, is read into it by [`read`] and shown from it
//! by [`write`]. (TinyNTT's pattern corpus, tempos, autofill and step
//! sequencer are built on it, in `../tinyntt`.)
//!
//! Reading, by notation ([`Notation`]):
//! - **drum grid** and **drum tab** (a row a drum), **step strings** (`1`
//!   and `0`, velocity digits, hexadecimal) and Toussaint's **box
//!   notation** (a row of marks and dots): [`Notation::Lanes`];
//! - **onset sets** (`{0,3,6,10,12}/16`), **inter-onset intervals** and
//!   additive meters (`3+3+4+2+4`), **Euclidean rhythms** (`E(5,8)`);
//! - **tala** and other **bols** (`dha dhin dhin dha | ...`, tabla,
//!   konnakol and solkattu syllables), **takadimi and Kodaly syllables**,
//!   **counting** (`1 e & a 2`), **note values** (`q e e q`, `h q q`),
//!   LilyPond's **drum mode**, **drumscript** (the CC0 `bd4 sn4` DSL), **Hydrogen** patterns and songs (XML), **stickings** (`R L R R L R L L`, flams
//!   and drags);
//! - the kit's **JSON** (`pattern_grid`, `instrument_map`);
//! - **Strudel and Tidal mini-notation**, **ABC**, **MusicXML**,
//!   Humdrum `**kern`, **MIDI** and the other formats Neumetik reads
//!   (this crate's `text`): their drum part, or the rhythm of their
//!   notes.
//!
//! Standard library only; the other formats are read by this crate's other
//! readers, through [`crate::Score`].

pub mod hydrogen;
pub mod json;
pub mod read;
pub mod write;

/// Ticks a quarter note is in a pattern's notes (`Pattern::notes`, `write::to_score`).
pub const PPQ: u32 = 96;
/// The bars a song loops over: a pattern `fits` when its cycle comes round in them.
pub const LOOP_BARS: u32 = 8;

/// A cell with nothing.
pub const REST: u8 = 0;
/// A soft hit: a ghost note.
pub const SOFT: u8 = 1;
/// A hit.
pub const HIT: u8 = 2;
/// An accent.
pub const ACCENT: u8 = 3;

/// MIDI velocity of a cell's level.
pub fn velocity(level: u8) -> u8 {
    match level {
        SOFT => 62,
        HIT => 98,
        ACCENT => 118,
        _ => 0,
    }
}

/// The level a MIDI velocity is read as.
pub fn level_of(vel: u8) -> u8 {
    match vel {
        0 => REST,
        1..=75 => SOFT,
        76..=108 => HIT,
        _ => ACCENT,
    }
}

/// One drum's row of cells, all the cycle's bars end to end.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lane {
    /// The General MIDI drum key (27 to 87).
    pub key: u8,
    pub cells: Vec<u8>,
}

/// A rhythm: `bars` bars of `beats` of `unit` (4 a quarter, 8 an eighth),
/// each of `steps` equal cells.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pattern {
    pub title: String,
    pub beats: u8,
    pub unit: u8,
    pub bars: u32,
    pub steps: u32,
    pub lanes: Vec<Lane>,
    /// What each cell was called in the source, for display: a bol, a
    /// syllable, a sticking letter. Empty when the notation has none, else
    /// a string a cell of the whole cycle.
    pub bols: Vec<String>,
    /// A tala's marks a cell (`X` the sam, `2` `3` a clap, `0` a wave), or
    /// blanks. Empty when the pattern has none.
    pub marks: Vec<char>,
}

impl Pattern {
    pub fn new(beats: u8, unit: u8, bars: u32, steps: u32) -> Pattern {
        Pattern { title: String::new(), beats: beats.max(1), unit: unit.max(1), bars: bars.max(1), steps: steps.max(1), lanes: Vec::new(), bols: Vec::new(), marks: Vec::new() }
    }

    /// Ticks (at [`PPQ`]) a bar lasts.
    pub fn bar_ticks(&self) -> u32 {
        PPQ * 4 * u32::from(self.beats) / u32::from(self.unit)
    }

    /// A bar's cells.
    pub fn cells_per_bar(&self) -> usize {
        self.steps as usize
    }

    /// The cycle's cells.
    pub fn len(&self) -> usize {
        (self.bars * self.steps) as usize
    }

    pub fn is_empty(&self) -> bool {
        self.hits() == 0
    }

    /// Whether a cell lands on a whole tick (else it is rounded).
    pub fn exact(&self) -> bool {
        self.bar_ticks() % self.steps == 0
    }

    /// The lane of a drum, made if it isn't there.
    pub fn lane(&mut self, key: u8) -> &mut Lane {
        let n = self.len();
        if let Some(i) = self.lanes.iter().position(|l| l.key == key) {
            return &mut self.lanes[i];
        }
        self.lanes.push(Lane { key, cells: vec![REST; n] });
        self.lanes.last_mut().expect("just pushed")
    }

    /// Puts a level on a drum's cell (the higher level when one is
    /// there); a cell past the cycle wraps round, as the loop does.
    pub fn put(&mut self, key: u8, cell: i64, level: u8) {
        let n = self.len() as i64;
        if n == 0 {
            return;
        }
        let c = cell.rem_euclid(n) as usize;
        let lane = self.lane(key);
        lane.cells[c] = lane.cells[c].max(level);
    }

    /// The hits in the whole cycle.
    pub fn hits(&self) -> usize {
        self.lanes.iter().map(|l| l.cells.iter().filter(|c| **c != REST).count()).sum()
    }

    /// The lanes with something in them, in the order a drummer's chart
    /// reads them (cymbals first, the kick last).
    pub fn shown(&self) -> Vec<&Lane> {
        let mut v: Vec<&Lane> = self.lanes.iter().filter(|l| l.cells.iter().any(|c| *c != REST)).collect();
        v.sort_by_key(|l| (names::rank(l.key), l.key));
        v
    }

    /// Drops the lanes that have nothing in them and sorts the rest.
    pub fn tidy(&mut self) {
        self.lanes.retain(|l| l.cells.iter().any(|c| *c != REST));
        self.lanes.sort_by_key(|l| (names::rank(l.key), l.key));
    }

    /// The pattern's notes, a cycle from tick 0: (start, length, key,
    /// velocity), each inside its bar.
    pub fn notes(&self) -> Vec<(u32, u32, u8, u8)> {
        let (bar, steps) = (self.bar_ticks(), self.steps);
        let at = |c: u32| -> u32 {
            let (b, s) = (c / steps, c % steps);
            b * bar + (u64::from(s) * u64::from(bar) / u64::from(steps)) as u32
        };
        let mut out = Vec::new();
        for lane in &self.lanes {
            for (c, level) in lane.cells.iter().enumerate() {
                if *level == REST {
                    continue;
                }
                let c = c as u32;
                let start = at(c);
                let next = if (c + 1) % steps == 0 { (c / steps + 1) * bar } else { at(c + 1) };
                let len = next.saturating_sub(start).saturating_sub(4).max(6).min(next.saturating_sub(start).max(1));
                out.push((start, len, lane.key, velocity(*level)));
            }
        }
        out.sort_by_key(|n| (n.0, n.2));
        out
    }

    /// The smallest number of bars the pattern repeats in (a pattern
    /// of four bars made of one twice over is two).
    pub fn period_bars(&self) -> u32 {
        let per = self.steps as usize;
        for b in 1..self.bars {
            if self.bars % b == 0 && self.lanes.iter().all(|l| (per * b as usize..l.cells.len()).all(|i| l.cells[i] == l.cells[i - per * b as usize])) {
                return b;
            }
        }
        self.bars
    }

    /// Whether the cycle holds the same bar over and over, so that the
    /// bars can be said to be fewer.
    pub fn trim_repeats(&mut self) {
        let b = self.period_bars();
        if b < self.bars {
            let n = (b * self.steps) as usize;
            for l in &mut self.lanes {
                l.cells.truncate(n);
            }
            self.bols.truncate(n);
            self.marks.truncate(n);
            self.bars = b;
        }
    }

    /// The meter as written: `7/8`.
    pub fn meter(&self) -> String {
        format!("{}/{}", self.beats, self.unit)
    }

    /// One line about the pattern: meter, bars, cells, hits, drums.
    pub fn describe(&self) -> String {
        format!("{} {} bar{} of {} steps, {} hits on {} drums", self.meter(), self.bars, if self.bars == 1 { "" } else { "s" }, self.steps, self.hits(), self.shown().len())
    }

    /// Whether it fits a song's bars: the same bar length in ticks, and a
    /// cycle that comes round in a song's eight bars.
    pub fn fits(&self, bar_ticks: u32) -> bool {
        self.bar_ticks() == bar_ticks && self.bars > 0 && LOOP_BARS % self.bars == 0
    }
}

/// The notations a pattern is read in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Notation {
    /// A row a drum: drum grids, drum tabs, step strings, box notation.
    Lanes,
    /// Where the hits fall: `{0,3,6,10,12}/16`.
    Onsets,
    /// How long between hits: `3+3+4+2+4`.
    Intervals,
    /// Bjorklund's: `E(5,8)`.
    Euclid,
    /// Tala, tabla bols, konnakol and solkattu: a syllable a pulse.
    Bols,
    /// Takadimi and Kodaly syllables: a word a beat.
    Syllables,
    /// Counting: `1 e & a 2`.
    Counts,
    /// Note values: `q e e q`.
    Values,
    /// LilyPond drum mode.
    Lilypond,
    /// Stickings and rudiments: `R L R R`.
    Sticking,
    /// Drumscript (`bd4 sn4`, blocks, `[hh8] * 8`).
    Drumscript,
    /// Hydrogen's patterns and songs (XML).
    Hydrogen,
    /// The reference kit's JSON.
    Json,
    /// Strudel and Tidal mini-notation, by Neumetik.
    Mini,
    /// A score Neumetik reads (ABC, MusicXML, `**kern`, MIDI...).
    Score,
}

/// Every notation read, with the name it is asked for by.
pub const NOTATIONS: [(Notation, &str, &str); 15] = [
    (Notation::Lanes, "grid", "Drum grid, drum tab, step strings, box notation"),
    (Notation::Onsets, "onsets", "Onset sets {0,3,6,10,12}/16"),
    (Notation::Intervals, "intervals", "Inter-onset intervals and additive meters 3+3+4+2+4"),
    (Notation::Euclid, "euclid", "Euclidean rhythms E(5,8)"),
    (Notation::Bols, "bols", "Tala, tabla bols, konnakol and solkattu"),
    (Notation::Syllables, "syllables", "Takadimi and Kodaly syllables"),
    (Notation::Counts, "counts", "Counting: 1 e & a 2"),
    (Notation::Values, "values", "Note values: q e e q"),
    (Notation::Lilypond, "lilypond", "LilyPond drum mode"),
    (Notation::Sticking, "sticking", "Stickings and rudiments"),
    (Notation::Drumscript, "drumscript", "Drumscript: bd4 sn4, blocks, [hh8] * 8"),
    (Notation::Hydrogen, "hydrogen", "Hydrogen .h2pattern and .h2song"),
    (Notation::Json, "json", "The reference kit's JSON"),
    (Notation::Mini, "mini", "Strudel and Tidal mini-notation"),
    (Notation::Score, "score", "ABC, MusicXML, **kern, MIDI and the rest Neumetik reads"),
];

impl Notation {
    /// The name a notation is asked for by.
    pub fn name(self) -> &'static str {
        NOTATIONS.iter().find(|n| n.0 == self).map_or("", |n| n.1)
    }

    pub fn by_name(name: &str) -> Option<Notation> {
        let n = name.trim().to_ascii_lowercase();
        NOTATIONS.iter().find(|x| x.1 == n).map(|x| x.0)
    }
}

pub mod names {
    //! The drums by name, and where each sits on a percussion staff.

    /// (key, name, tab label, other names, staff position, notehead).
    /// A staff position counts half-lines up from the bottom line (0 the
    /// bottom line, 1 the first space, 8 the top line, 9 the space above
    /// it, 10 the line above that, -1 the space below), after common
    /// drum-kit charts: its own table, not MuseScore's. `x` is a
    /// cross notehead (cymbals, hats), `o` an ordinary one. `None` is a
    /// drum with no place on a kit's staff: it is shown on a row of its own.
    pub const KIT: [(u8, &str, &str, &str, Option<i8>, char); 47] = [
        (35, "kick2", "B2", "acousticbassdrum bd2 bassdrum2", Some(1), 'o'),
        (36, "kick", "BD", "bd bassdrum bassdrum1 kickdrum k", Some(1), 'o'),
        (37, "rim", "RS", "sidestick stick rimshot rs ss", Some(5), 'x'),
        (38, "snare", "SD", "sd sn snaredrum acousticsnare s", Some(5), 'o'),
        (39, "clap", "CP", "handclap cp clp hc", Some(6), 'x'),
        (40, "snare2", "S2", "electricsnare sd2 sn2", Some(5), 'o'),
        (41, "lowfloortom", "FL", "floortomlow ftl flrl tomf2", Some(0), 'o'),
        (42, "hat", "HH", "hh closedhat closedhihat chh ch hihat hihatclosed", Some(9), 'x'),
        (43, "highfloortom", "FH", "floortomhigh fth flrh tomf floortom", Some(2), 'o'),
        (44, "pedalhat", "PH", "phh hhp pedalhihat footchick foothat", Some(-1), 'x'),
        (45, "lowtom", "TL", "tomlow tl toml", Some(3), 'o'),
        (46, "openhat", "OH", "ohh oh openhihat hho hihatopen", Some(9), 'x'),
        (47, "lowmidtom", "TM", "tomlowmid tlm tomml", Some(4), 'o'),
        (48, "himidtom", "T2", "highmidtom tomhm tommh", Some(6), 'o'),
        (49, "crash", "CR", "cr cc crashcymbal cymc crash1", Some(10), 'x'),
        (50, "hightom", "T1", "tomhigh th tomh ht", Some(7), 'o'),
        (51, "ride", "RD", "rd ridecymbal cymr ride1", Some(8), 'x'),
        (52, "china", "CN", "chinese chn cymch", Some(11), 'x'),
        (53, "ridebell", "RB", "bell rb", Some(8), 'x'),
        (54, "tambourine", "TB", "tamb tb", Some(9), 'x'),
        (55, "splash", "SP", "spl cymsp", Some(11), 'x'),
        (56, "cowbell", "CB", "cow cb", None, 'x'),
        (57, "crash2", "C2", "cr2 cc2", Some(10), 'x'),
        (58, "vibraslap", "VS", "vib", None, 'x'),
        (59, "ride2", "R2", "rd2", Some(8), 'x'),
        (60, "highbongo", "BH", "bongohigh hibongo boh bongoh bongo", None, 'o'),
        (61, "lowbongo", "BL", "bongolow lobongo bol", None, 'o'),
        (62, "mutehighconga", "CM", "congamute cghm congahighmute", None, 'o'),
        (63, "openhighconga", "CO", "congaopen cgho congahigh conga", None, 'o'),
        (64, "lowconga", "CG", "congalow cgl", None, 'o'),
        (65, "hightimbale", "MH", "timbaleh tmbh timbale", None, 'o'),
        (66, "lowtimbale", "ML", "timbalel tmbl", None, 'o'),
        (67, "highagogo", "AH", "agogoh agh agogo", None, 'x'),
        (68, "lowagogo", "AL", "agogol agl", None, 'x'),
        (69, "cabasa", "CA", "cab", None, 'x'),
        (70, "maracas", "MA", "shaker mar sh", None, 'x'),
        (71, "shortwhistle", "WS", "whistleshort whs", None, 'x'),
        (72, "longwhistle", "WL", "whistlelong whl", None, 'x'),
        (73, "shortguiro", "GS", "guiroshort guis", None, 'x'),
        (74, "longguiro", "GL", "guirolong guil guiro", None, 'x'),
        (75, "claves", "CV", "clave cl", None, 'x'),
        (76, "highwoodblock", "WH", "woodblockhigh wbh woodblock", None, 'x'),
        (77, "lowwoodblock", "WB", "woodblocklow wbl", None, 'x'),
        (78, "mutecuica", "UM", "cuicamute cuim", None, 'o'),
        (79, "opencuica", "UO", "cuicaopen cuio cuica", None, 'o'),
        (80, "mutetriangle", "TR", "trianglemute trim", None, 'x'),
        (81, "opentriangle", "TO", "triangleopen trio triangle tri", None, 'x'),
    ];

    fn squash(name: &str) -> String {
        name.chars().filter(|c| c.is_ascii_alphanumeric()).map(|c| c.to_ascii_lowercase()).collect()
    }

    /// A drum's key by any of its names (`kick`, `BD`, `hi-hat`, `36`,
    /// `n42`), or None.
    pub fn key_from_name(name: &str) -> Option<u8> {
        let n = squash(name);
        if n.is_empty() {
            return None;
        }
        if let Ok(k) = n.trim_start_matches('n').parse::<u8>() {
            return (27..=87).contains(&k).then_some(k);
        }
        for (key, canon, label, others, _, _) in KIT.iter() {
            if *canon == n || label.to_ascii_lowercase() == n || others.split(' ').any(|o| o == n) {
                return Some(*key);
            }
        }
        // A few written with spaces and hyphens: "closed hi-hat" squashes to
        // "closedhihat" above; these fold the rest.
        match n.as_str() {
            "hihatopen" | "openhihat" => Some(46),
            "bassdrum" => Some(36),
            _ => None,
        }
    }

    /// A drum's name for a screen: eight letters at most.
    pub fn display(key: u8) -> String {
        match key {
            41 => "lofloor",
            43 => "hifloor",
            44 => "pedal",
            45 => "lotom",
            46 => "openhat",
            47 => "lomidtom",
            48 => "himidtom",
            50 => "hitom",
            53 => "ridebell",
            57 => "crash2",
            58 => "vibra",
            60 => "hibongo",
            61 => "lobongo",
            62 => "mutecga",
            63 => "opencga",
            65 => "hitimb",
            66 => "lotimb",
            67 => "hiagogo",
            68 => "loagogo",
            71 => "whistle",
            72 => "longwhs",
            73 => "guiro",
            74 => "longgui",
            76 => "hiwoodb",
            77 => "lowoodb",
            78 => "mutecui",
            79 => "opencui",
            80 => "mutetri",
            81 => "triangle",
            k => return name(k).chars().take(8).collect(),
        }
        .to_string()
    }

    /// A drum's name (`kick`), or `n` and its key for one with none.
    pub fn name(key: u8) -> String {
        KIT.iter().find(|k| k.0 == key).map_or_else(|| format!("n{key}"), |k| k.1.to_string())
    }

    /// The two letters a drum tab names it by.
    pub fn label(key: u8) -> String {
        KIT.iter().find(|k| k.0 == key).map_or_else(|| format!("{key}"), |k| k.2.to_string())
    }

    /// Where a drum sits on the staff, and its notehead.
    pub fn staff(key: u8) -> Option<(i8, char)> {
        KIT.iter().find(|k| k.0 == key).and_then(|k| k.4.map(|p| (p, k.5)))
    }

    /// A drum's notehead, for the ones with no staff place too.
    pub fn head(key: u8) -> char {
        KIT.iter().find(|k| k.0 == key).map_or('o', |k| k.5)
    }

    /// The order a chart shows drums in: highest on the staff first, the
    /// other percussion after the kit.
    pub fn rank(key: u8) -> i32 {
        match staff(key) {
            Some((p, _)) => 100 - i32::from(p) * 4 - i32::from(key == 46 || key == 53 || key == 57),
            None => 200 + i32::from(key),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_drum_has_a_name_that_finds_it_again() {
        for (key, canon, label, others, _, _) in names::KIT.iter() {
            assert_eq!(names::key_from_name(canon), Some(*key), "{canon}");
            assert_eq!(names::key_from_name(label), Some(*key), "{label}");
            for o in others.split(' ') {
                assert_eq!(names::key_from_name(o), Some(*key), "{o}");
            }
            assert_eq!(names::key_from_name(&key.to_string()), Some(*key));
        }
        assert_eq!(names::key_from_name("Closed Hi-Hat"), Some(42));
        assert_eq!(names::key_from_name("nonsense"), None);
        assert_eq!(names::KIT.iter().map(|k| k.0).collect::<Vec<_>>(), (35..=81).collect::<Vec<u8>>());
    }

    #[test]
    fn a_pattern_makes_notes_that_stay_in_their_bars() {
        let mut p = Pattern::new(7, 8, 2, 14);
        for c in 0..28 {
            p.put(38, c, ACCENT);
        }
        let notes = p.notes();
        assert_eq!(notes.len(), 28);
        assert!(p.exact());
        for (at, len, _, _) in notes {
            assert!(at / p.bar_ticks() == (at + len - 1) / p.bar_ticks());
        }
        let mut odd = Pattern::new(4, 4, 1, 20);
        for c in 0..20 {
            odd.put(38, c, HIT);
        }
        assert!(!odd.exact());
        for (at, len, _, _) in odd.notes() {
            assert!(at + len <= odd.bar_ticks());
        }
    }

    #[test]
    fn repeats_are_trimmed() {
        let mut p = Pattern::new(4, 4, 4, 4);
        for b in 0..4 {
            p.put(36, b * 4, HIT);
        }
        assert_eq!(p.period_bars(), 1);
        p.trim_repeats();
        assert_eq!((p.bars, p.len()), (1, 4));
    }
}
