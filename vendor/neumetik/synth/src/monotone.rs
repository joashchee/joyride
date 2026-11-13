// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! MONOTONE (Jim Leonard): a DOS tracker for the PC speaker. Up to four
//! tracks of 64-row patterns, each cell a note (A0 to C9, equal
//! temperament) and one of eight effects; a player ticked at 60 Hz turns
//! each track into a frequency, and the speaker, which sounds one square
//! wave, gives each tick to the next sounding track in turn (the first
//! again on every new row), holding the last while the track whose turn
//! it is is silent. This is the editor's own output (its "8253 PIT", not
//! equal-share), as MONOTONE's source (released under the Unlicense)
//! describes it; the source was read for what it does, and its tables are
//! computed here as it computes them.

use std::sync::Arc;

use crate::replay::{Bufs, Driver, Tune};

/// The PIT's clock: the speaker's square wave is this divided by a whole number.
const PIT_HZ: f64 = 1_193_182.0;
/// The PIT divisor that ticks the player at 60 a second.
const TICK_DIVISOR: f64 = 19_886.0;
/// Header: "MONOTONE" (a Pascal string[8]), title and comment (string[40]),
/// version, patterns, tracks, cell size, 256 orders.
const HEADER: usize = 9 + 41 + 41 + 4 + 256;
const ROWS: usize = 64;
/// Notes 1 (A0) to 100 (C9); 127 stops the track.
const NOTE_MAX: u8 = 100;
const NOTE_END: u8 = 127;
/// Steps between notes in the frequency table (a step is 1/96 octave).
const IBN: usize = 8;
/// The speaker sounds this many tracks at most.
const VOICES: usize = 4;
/// What the speaker is set to for silence: above hearing.
const SILENT: u16 = 0xFFFF;
/// A square wave at full swing, each side.
const LEVEL: f32 = 0.25;

/// One cell: its note (0 none) and effect (as shown: 0-4, B, D or F)
/// with its parameter (0-63, or two of 0-7 for 0 and 4).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Cell {
    pub note: u8,
    pub effect: u8,
    pub param: u8,
    /// Whether an effect is written (0 00 is none).
    pub raw: bool,
}

impl Cell {
    fn read(w: u16) -> Cell {
        const SHOWN: [u8; 8] = [0x0, 0x1, 0x2, 0x3, 0x4, 0xB, 0xD, 0xF];
        let e = (w >> 6) & 7;
        let d = (w & 0x3F) as u8;
        let effect = SHOWN[usize::from(e)];
        // 0xy and 4xy keep x and y in three bits each.
        let param = if matches!(effect, 0x0 | 0x4) { (d >> 3) << 4 | (d & 7) } else { d };
        Cell { note: (w >> 9) as u8, effect, param, raw: w & 0x1FF != 0 }
    }
}

#[derive(Clone, Debug)]
pub struct Song {
    pub title: String,
    pub comment: String,
    pub tracks: usize,
    /// Each pattern's rows, a cell a track.
    pub patterns: Vec<Vec<Cell>>,
    /// The order list, to the first 0xFF.
    pub orders: Vec<u8>,
    /// All 256 orders, as the player reads them.
    all_orders: [u8; 256],
    file: Arc<Vec<u8>>,
}

pub fn probe(d: &[u8]) -> bool {
    d.len() >= HEADER && d[0] == 8 && &d[1..9] == b"MONOTONE" && d[91] == 1 && (1..=12).contains(&d[93])
}

impl Song {
    pub fn parse(d: &[u8]) -> Option<Song> {
        if !probe(d) {
            return None;
        }
        let pstr = |at: usize, max: usize| {
            let n = usize::from(d[at]).min(max);
            d[at + 1..at + 1 + n].iter().map(|&c| if (32..127).contains(&c) { c as char } else { ' ' }).collect::<String>().trim().to_string()
        };
        let (npat, tracks) = (usize::from(d[92]), usize::from(d[93]));
        let all_orders: [u8; 256] = d[95..HEADER].try_into().ok()?;
        let orders: Vec<u8> = all_orders.iter().copied().take_while(|&o| o != 0xFF).collect();
        if orders.is_empty() {
            return None;
        }
        let size = tracks * ROWS * 2;
        let mut patterns = Vec::with_capacity(npat);
        for p in 0..npat {
            // A short file's last patterns are blank, as the editor leaves them.
            let at = HEADER + p * size;
            let cells = (0..tracks * ROWS)
                .map(|i| d.get(at + i * 2..at + i * 2 + 2).map_or(Cell::default(), |b| Cell::read(u16::from_le_bytes([b[0], b[1]]))))
                .collect();
            patterns.push(cells);
        }
        Some(Song { title: pstr(9, 40), comment: pstr(50, 40), tracks, patterns, orders, all_orders, file: Arc::new(d.to_vec()) })
    }

    /// Pattern `p`'s cell on `row` of `track` (blank past the patterns the file has).
    pub fn cell(&self, p: usize, row: usize, track: usize) -> Cell {
        self.patterns.get(p).and_then(|c| c.get(row * self.tracks + track)).copied().unwrap_or_default()
    }

    /// Ticks a row at the start: one a track, at least 4.
    pub fn start_speed(&self) -> u8 {
        self.tracks.max(4) as u8
    }
}

impl Tune for Song {
    fn format(&self) -> &'static str {
        "mon"
    }
    fn file(&self) -> &[u8] {
        &self.file
    }
    fn driver(&self, _song: usize) -> Box<dyn Driver> {
        Box::new(Player::new(Arc::new(self.clone())))
    }
}

/// The frequency of each 1/96 octave from below A0 (index 8) up, rounded
/// to whole Hz as MONOTONE's table is: A0 is 27.5 Hz, the bottom at least
/// 19, built by multiplying up a step at a time. Room is left above C9
/// for arpeggios and vibratos that go past it.
fn note_table() -> Vec<u16> {
    let n = (usize::from(NOTE_END) + 8) * IBN;
    let step = (2f64.ln() / 96.0).exp();
    let mut t = vec![440u16; n + 1];
    let mut hz = 27.5;
    for i in (1..IBN).rev() {
        hz /= step;
        t[i] = hz.max(19.0).round() as u16;
    }
    let mut hz = 27.5;
    t[IBN] = 28;
    for v in t.iter_mut().skip(IBN + 1) {
        hz *= step;
        *v = hz.round().min(65535.0) as u16;
    }
    t
}

/// The vibrato's table: a wave of a sine, ±64, over 32 steps.
fn vib_table() -> [i8; 32] {
    std::array::from_fn(|b| (64.0 * (b as f64 * std::f64::consts::PI / 32.0 * 2.0).sin()).round() as i8)
}

#[derive(Clone, Copy, Default)]
struct Track {
    // As the speaker sees it.
    active: bool,
    interval: usize,
    freq: u16,
    // The row's cell.
    note: u8,
    effect: u8,
    p1: u8,
    p2: u8,
    raw: bool,
    last_note: u8,
    porta_stop: u16,
    porta_delta: u8,
    vib_speed: u8,
    vib_depth: u8,
    vib_index: u8,
}

struct Player {
    song: Arc<Song>,
    hz: Vec<u16>,
    vib: [i8; 32],
    voices: usize,
    tr: [Track; VOICES],
    tempo: u8,
    count: u8,
    order: u8,
    pattern: u8,
    row: i32,
    started: bool,
    stopped: bool,
    /// The speaker: what's set now, the track it's for, what's set next.
    sounding: (u16, usize),
    next: (u16, usize),
    dominant: usize,
    phase: f64,
    shown: (usize, usize),
    seen: Vec<bool>,
    done: bool,
}

impl Player {
    fn new(song: Arc<Song>) -> Player {
        let voices = song.tracks.min(VOICES);
        let tempo = song.start_speed();
        let pattern = song.all_orders[0];
        Player {
            song,
            hz: note_table(),
            vib: vib_table(),
            voices,
            tr: [Track::default(); VOICES],
            tempo,
            count: 0,
            order: 0,
            pattern,
            row: 0,
            started: false,
            stopped: false,
            sounding: (SILENT, 0),
            next: (SILENT, 0),
            dominant: 0,
            phase: 0.0,
            shown: (0, 0),
            seen: vec![false; 256],
            done: false,
        }
    }

    fn hz_at(&self, i: usize) -> u16 {
        self.hz[i.min(self.hz.len() - 1)]
    }

    /// The speaker's interrupt: sets what was worked out last time, then
    /// the next track's turn (MONOTONE's PC speaker output, not equal-share).
    fn speaker(&mut self) {
        self.sounding = self.next;
        let t = &self.tr[self.dominant];
        if t.active {
            self.next = (t.freq, self.dominant);
        } else if !self.tr[..self.voices].iter().any(|t| t.active) {
            self.next = (SILENT, self.dominant);
        }
        self.dominant += 1;
        if self.dominant >= self.voices || self.count == 0 {
            self.dominant = 0;
        }
    }

    fn stop(&mut self) {
        self.stopped = true;
        self.done = true;
        for t in self.tr.iter_mut() {
            t.active = false;
        }
        self.count = 0;
    }

    fn enter_order(&mut self) {
        if std::mem::replace(&mut self.seen[usize::from(self.order)], true) {
            self.done = true;
        }
    }

    /// The player's tick (MONOTONE's MTV1 player, playing the song from its top).
    fn calc(&mut self) {
        if self.stopped {
            return;
        }
        if !self.started {
            // Its first tick only starts the song; the first row comes next.
            self.started = true;
            self.count = self.tempo;
            self.enter_order();
            return;
        }
        self.count = self.count.wrapping_add(1);
        if self.count >= self.tempo {
            self.count = 0;
            if self.row >= ROWS as i32 {
                self.order = self.order.wrapping_add(1);
                if self.song.all_orders[usize::from(self.order)] == 0xFF {
                    self.order = self.order.wrapping_sub(1);
                    self.stop();
                    return;
                }
                self.pattern = self.song.all_orders[usize::from(self.order)];
                self.row = 0;
                self.enter_order();
            }
            self.shown = (usize::from(self.order), self.row.max(0) as usize);
            self.row_tick();
            self.row += 1;
        } else {
            self.effects();
        }
        // A pattern break on the last order goes past the song: it ends.
        if self.song.all_orders[usize::from(self.order)] == 0xFF {
            self.order = self.order.wrapping_sub(1);
            self.stop();
        }
    }

    fn row_tick(&mut self) {
        let (pat, row) = (usize::from(self.pattern), self.row.clamp(0, ROWS as i32 - 1) as usize);
        for c in 0..self.voices {
            let cell = self.song.cell(pat, row, c);
            let t = &mut self.tr[c];
            t.note = cell.note;
            t.effect = cell.effect;
            t.raw = cell.raw;
            (t.p1, t.p2) = if matches!(cell.effect, 0x0 | 0x4) { (cell.param >> 4, cell.param & 7) } else { (cell.param, 0) };
        }
        for c in 0..self.voices {
            let t = self.tr[c];
            if t.note == NOTE_END {
                self.tr[c].active = false;
            } else if (1..=NOTE_MAX).contains(&t.note) && t.effect != 0x3 {
                let iv = usize::from(t.note) * IBN;
                let f = self.hz_at(iv);
                let t = &mut self.tr[c];
                t.active = true;
                t.interval = iv;
                t.freq = f;
                t.last_note = t.note;
                t.vib_index = 0;
            }
        }
        let (mut broke, mut jumped) = (false, false);
        for c in 0..self.voices {
            let t = self.tr[c];
            if !t.raw {
                continue;
            }
            match t.effect {
                0x3 => {
                    if (1..=NOTE_MAX).contains(&t.note) {
                        self.tr[c].porta_stop = self.hz_at(usize::from(t.note) * IBN);
                    }
                    if t.p1 != 0 {
                        self.tr[c].porta_delta = t.p1;
                    }
                }
                0x4 => {
                    let t = &mut self.tr[c];
                    if t.p1 != 0 {
                        t.vib_speed = t.p1;
                    }
                    if t.p2 != 0 {
                        t.vib_depth = t.p2;
                    }
                    t.vib_index = t.vib_index.wrapping_add(t.vib_speed) & 31;
                }
                0xB => {
                    self.order = t.p1;
                    self.row = -1;
                    jumped = true;
                    self.pattern = self.song.all_orders[usize::from(self.order)];
                }
                0xD => {
                    self.row = i32::from(t.p1) - 1;
                    if self.row >= ROWS as i32 {
                        self.row = -1;
                    }
                    if !broke && !jumped {
                        self.order = self.order.wrapping_add(1);
                    }
                    broke = true;
                    let o = self.song.all_orders[usize::from(self.order)];
                    if o != 0xFF {
                        self.pattern = o;
                    }
                }
                0xF => self.tempo = t.p1,
                _ => {}
            }
        }
        if broke || jumped {
            self.enter_order();
        }
    }

    fn effects(&mut self) {
        const MIN_HZ: i32 = 20;
        const MAX_HZ: i32 = 65535 - 63;
        for c in 0..self.voices {
            let t = self.tr[c];
            if !t.raw {
                continue;
            }
            match t.effect {
                0x0 => {
                    let add = match self.count % 3 {
                        0 => 0,
                        1 => t.p1,
                        _ => t.p2,
                    };
                    let iv = usize::from(t.last_note + add) * IBN;
                    let f = self.hz_at(iv);
                    (self.tr[c].interval, self.tr[c].freq) = (iv, f);
                }
                0x1 => self.tr[c].freq = (i32::from(t.freq) + i32::from(t.p1)).min(MAX_HZ) as u16,
                0x2 => self.tr[c].freq = (i32::from(t.freq) - i32::from(t.p1)).max(MIN_HZ) as u16,
                0x3 => {
                    let t = &mut self.tr[c];
                    if t.freq < t.porta_stop {
                        t.freq = t.freq.wrapping_add(u16::from(t.porta_delta)).min(t.porta_stop);
                    }
                    if t.freq > t.porta_stop {
                        t.freq = t.freq.wrapping_sub(u16::from(t.porta_delta)).max(t.porta_stop);
                    }
                }
                0x4 => {
                    // Pascal's div: toward zero.
                    let off = i32::from(self.vib[usize::from(t.vib_index)]) * i32::from(t.vib_depth) / 64;
                    let f = self.hz_at((t.interval as i32 + off).max(0) as usize);
                    let t = &mut self.tr[c];
                    t.freq = f;
                    t.vib_index = t.vib_index.wrapping_add(t.vib_speed) & 31;
                }
                _ => {}
            }
        }
    }
}

/// A band-limited step's correction (polyBLEP) at phase `t` for a step of `dt`.
fn blep(t: f64, dt: f64) -> f64 {
    if t < dt {
        let x = t / dt;
        2.0 * x - x * x - 1.0
    } else if t > 1.0 - dt {
        let x = (t - 1.0) / dt;
        x * x + 2.0 * x + 1.0
    } else {
        0.0
    }
}

impl Driver for Player {
    fn channels(&self) -> usize {
        // Tracks past the fourth are shown but never sound.
        self.song.tracks
    }

    fn tick(&mut self) {
        self.speaker();
        self.calc();
    }

    fn tick_seconds(&self) -> f64 {
        TICK_DIVISOR / PIT_HZ
    }

    fn render(&mut self, rate: u32, frames: usize, out: &mut Bufs) {
        for c in 0..self.song.tracks {
            out.left[c][..frames].fill(0.0);
            out.right[c][..frames].fill(0.0);
        }
        let (hz, owner) = self.sounding;
        if hz == SILENT || hz == 0 || owner >= self.voices {
            return;
        }
        // The PIT divides its clock by a whole number.
        let f = PIT_HZ / (PIT_HZ / f64::from(hz)).floor().max(1.0);
        let dt = f / f64::from(rate.max(1));
        if dt >= 0.5 {
            return;
        }
        for i in 0..frames {
            let p = self.phase;
            let mut v = if p < 0.5 { 1.0 } else { -1.0 };
            v += blep(p, dt);
            v -= blep((p + 0.5).fract(), dt);
            let s = v as f32 * LEVEL;
            out.left[owner][i] = s;
            out.right[owner][i] = s;
            self.phase = (p + dt).fract();
        }
    }

    fn position(&self) -> (usize, usize) {
        self.shown
    }

    fn speed(&self) -> (u32, u32) {
        (u32::from(self.tempo.max(1)), 150)
    }

    fn take_done(&mut self) -> bool {
        std::mem::take(&mut self.done)
    }

    fn playing(&self) -> bool {
        !self.stopped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables() {
        let t = note_table();
        assert_eq!(t[IBN], 28);
        // A4 (note 49) is 440 Hz; C9 about 8372.
        assert_eq!(t[49 * IBN], 440);
        assert_eq!(t[100 * IBN], 8372);
        let v = vib_table();
        assert_eq!((v[0], v[8], v[16], v[24]), (0, 64, 0, -64));
    }

    #[test]
    fn cells() {
        // Note 40, effect 4 (vibrato) x 3 y 5.
        let c = Cell::read(40 << 9 | 4 << 6 | 3 << 3 | 5);
        assert_eq!((c.note, c.effect, c.param, c.raw), (40, 4, 0x35, true));
        let c = Cell::read(7 << 6 | 6);
        assert_eq!((c.note, c.effect, c.param), (0, 0xF, 6));
    }
}
