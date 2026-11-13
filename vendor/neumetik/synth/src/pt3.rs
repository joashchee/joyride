// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! Pro Tracker 3 (.pt3; ZX Spectrum, GDC 1997 and on, and Vortex Tracker
//! II's modules): three AY channels, each a track of a pattern, the
//! patterns in a list of positions; notes on 96 periods (one of four
//! tables, as each version of the editor had them), samples of 4-byte
//! lines (a tone offset, a volume, noise or an envelope offset, masks,
//! each offset kept or not, the volume sliding), ornaments of semitone
//! offsets, an envelope, and the effects: glissando, tone portamento,
//! sample and ornament offsets, vibrato (sound on and off), envelope
//! slide and tempo.
//!
//! Written from the format's published description (Alone Coder's
//! "Формат модуля Pro Tracker v3.7x", ptdoc.txt, with Sergey Bulba's
//! additions, in his PT3Docs) and the editor's own notes, its details
//! checked against what a player does, frame by frame, as a black box.
//! The note and volume tables are the format's: as players set them for
//! each version and table number. Played on an AY (`ay.rs`) at the ZX
//! Spectrum 128's 1.7734 MHz, a frame each 1/50 s.

use std::sync::Arc;

use crate::ay::Ay;
use crate::replay::{Bufs, Driver, Patch, PatchBytes, Tune};

/// The ZX Spectrum 128's AY clock.
const AY_CLOCK: f64 = 1_773_400.0;
/// Its frame: 70,908 of the 3.5469 MHz CPU's cycles.
const FRAME: f64 = 70_908.0 / 3_546_900.0;

const T0_OLD: [u16; 96] = [
    3105, 2931, 2766, 2611, 2464, 2326, 2195, 2072, 1956, 1846, 1742, 1645, 1552, 1465, 1383, 1305, 1232, 1163, 1097, 1036, 978, 923, 871, 822, 776, 732, 691, 652, 616, 581, 548, 518, 489, 461, 435,
    411, 388, 366, 345, 326, 308, 290, 274, 259, 244, 230, 217, 205, 194, 183, 172, 163, 154, 145, 137, 129, 122, 115, 108, 102, 97, 91, 86, 81, 77, 72, 68, 64, 61, 57, 54, 51, 48, 45, 43, 40, 38,
    36, 34, 32, 30, 28, 27, 25, 24, 22, 21, 20, 19, 18, 17, 16, 15, 14, 13, 12,
];
const T0_NEW: [u16; 96] = [
    3106, 2931, 2767, 2611, 2465, 2327, 2196, 2073, 1956, 1847, 1743, 1645, 1553, 1466, 1383, 1306, 1232, 1163, 1098, 1036, 978, 923, 871, 823, 776, 733, 692, 653, 616, 582, 549, 518, 489, 462, 436,
    411, 388, 366, 346, 326, 308, 291, 274, 259, 245, 231, 218, 206, 194, 183, 173, 163, 154, 145, 137, 130, 122, 115, 109, 103, 97, 92, 86, 82, 77, 73, 69, 65, 61, 58, 54, 51, 49, 46, 43, 41, 39,
    36, 34, 32, 31, 29, 27, 26, 24, 23, 22, 20, 19, 18, 17, 16, 15, 14, 13, 12,
];
const T1: [u16; 96] = [
    3832, 3600, 3424, 3200, 3032, 2856, 2696, 2544, 2400, 2272, 2136, 2016, 1916, 1800, 1712, 1600, 1516, 1428, 1348, 1272, 1200, 1136, 1068, 1021, 958, 900, 856, 800, 758, 714, 674, 636, 600, 568,
    534, 504, 479, 450, 428, 400, 379, 357, 337, 318, 300, 284, 266, 252, 239, 225, 214, 200, 189, 178, 168, 159, 150, 142, 133, 126, 119, 112, 107, 100, 94, 89, 84, 79, 75, 71, 66, 63, 59, 56, 53,
    50, 47, 44, 42, 39, 37, 35, 33, 31, 29, 28, 26, 25, 23, 22, 21, 19, 18, 17, 16, 15,
];
const T2_OLD: [u16; 96] = [
    3390, 3200, 3020, 2850, 2690, 2540, 2396, 2262, 2136, 2016, 1902, 1796, 1695, 1600, 1510, 1425, 1345, 1270, 1198, 1131, 1068, 1008, 951, 898, 847, 800, 755, 712, 673, 635, 599, 566, 534, 504,
    476, 449, 424, 400, 377, 356, 336, 317, 300, 283, 267, 252, 238, 224, 212, 200, 189, 178, 168, 159, 150, 141, 133, 126, 119, 112, 106, 100, 94, 89, 84, 80, 75, 71, 67, 63, 60, 56, 53, 50, 47, 45,
    42, 40, 38, 36, 34, 32, 30, 29, 27, 26, 25, 24, 21, 20, 19, 18, 17, 16, 15, 14,
];
const T2_NEW: [u16; 96] = [
    3344, 3157, 2980, 2812, 2655, 2506, 2365, 2232, 2107, 1989, 1877, 1772, 1672, 1578, 1490, 1406, 1327, 1253, 1182, 1116, 1053, 994, 939, 886, 836, 789, 745, 703, 664, 626, 591, 558, 527, 497, 469,
    443, 418, 395, 372, 352, 332, 313, 296, 279, 263, 249, 235, 221, 209, 197, 186, 176, 166, 157, 148, 140, 132, 124, 117, 111, 105, 99, 93, 88, 83, 78, 74, 70, 66, 62, 59, 55, 52, 49, 47, 44, 41,
    39, 37, 35, 33, 31, 29, 28, 26, 25, 23, 22, 21, 20, 18, 17, 16, 15, 14, 13,
];
const T3_OLD: [u16; 96] = [
    3290, 3106, 2931, 2767, 2611, 2465, 2327, 2196, 2073, 1956, 1847, 1743, 1645, 1553, 1466, 1383, 1306, 1232, 1163, 1098, 1036, 978, 923, 871, 823, 776, 733, 692, 653, 616, 582, 549, 518, 489, 462,
    436, 411, 388, 366, 346, 326, 308, 291, 275, 259, 245, 231, 218, 206, 194, 183, 173, 163, 154, 145, 137, 130, 122, 115, 109, 103, 97, 92, 86, 82, 77, 73, 69, 65, 61, 58, 54, 51, 49, 46, 43, 41,
    39, 36, 34, 32, 31, 29, 27, 26, 24, 23, 22, 20, 19, 18, 17, 16, 15, 14, 13,
];
const T3_NEW: [u16; 96] = [
    3290, 3106, 2931, 2767, 2611, 2465, 2327, 2196, 2073, 1956, 1847, 1743, 1645, 1553, 1466, 1383, 1306, 1232, 1163, 1098, 1036, 978, 923, 871, 823, 776, 733, 692, 653, 616, 582, 549, 518, 489, 462,
    436, 411, 388, 366, 346, 326, 308, 291, 274, 259, 245, 231, 218, 206, 194, 183, 173, 163, 154, 145, 137, 130, 122, 115, 109, 103, 97, 92, 86, 82, 77, 73, 69, 65, 61, 58, 54, 51, 49, 46, 43, 41,
    39, 36, 34, 32, 31, 29, 27, 26, 24, 23, 22, 20, 19, 18, 17, 16, 15, 14, 13,
];
const VOL_OLD: [[u8; 16]; 16] = [
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1],
    [0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2],
    [0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3],
    [0, 0, 0, 0, 1, 1, 1, 2, 2, 2, 3, 3, 3, 4, 4, 4],
    [0, 0, 0, 1, 1, 1, 2, 2, 3, 3, 3, 4, 4, 4, 5, 5],
    [0, 0, 0, 1, 1, 2, 2, 3, 3, 3, 4, 4, 5, 5, 6, 6],
    [0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7],
    [0, 0, 1, 1, 2, 2, 3, 3, 4, 5, 5, 6, 6, 7, 7, 8],
    [0, 0, 1, 1, 2, 3, 3, 4, 5, 5, 6, 6, 7, 8, 8, 9],
    [0, 0, 1, 2, 2, 3, 4, 4, 5, 6, 6, 7, 8, 8, 9, 10],
    [0, 0, 1, 2, 3, 3, 4, 5, 6, 6, 7, 8, 9, 9, 10, 11],
    [0, 0, 1, 2, 3, 4, 4, 5, 6, 7, 8, 8, 9, 10, 11, 12],
    [0, 0, 1, 2, 3, 4, 5, 6, 7, 7, 8, 9, 10, 11, 12, 13],
    [0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14],
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
];

/// The volume table of 3.5 and on: a channel's volume times the sample's
/// amplitude, over 15, rounded.
fn vol_new(v: usize, a: usize) -> u8 {
    ((v * a + 7) / 15) as u8
}

#[derive(Clone, Debug)]
pub struct Song {
    file: Arc<Vec<u8>>,
    /// The editor's subversion (3.x's x; Vortex Tracker's as 6).
    pub version: u8,
    pub table: u8,
    pub name: String,
    pub author: String,
    pub tempo: u8,
    pub loop_pos: usize,
    /// The positions' patterns.
    pub positions: Vec<usize>,
    pattern_table: usize,
}

fn le16(d: &[u8], i: usize) -> usize {
    d.get(i..i + 2).map_or(0, |b| usize::from(u16::from_le_bytes([b[0], b[1]])))
}

pub fn probe(d: &[u8]) -> bool {
    d.len() > 202 && (d.starts_with(b"ProTracker 3.") || d.starts_with(b"Vortex Tracker II")) && d[99] < 4
}

fn text(b: &[u8]) -> String {
    b.iter().map(|&x| if (0x20..0x7F).contains(&x) { x as char } else { ' ' }).collect::<String>().trim().to_string()
}

impl Song {
    pub fn parse(d: &[u8]) -> Option<Song> {
        if !probe(d) {
            return None;
        }
        let version = if d.starts_with(b"ProTracker 3.") {
            let c = d[13];
            if c.is_ascii_digit() {
                c - b'0'
            } else {
                6
            }
        } else {
            6
        };
        let mut positions = Vec::new();
        let mut i = 201;
        while let Some(&b) = d.get(i) {
            if b == 0xFF {
                break;
            }
            positions.push(usize::from(b / 3));
            i += 1;
        }
        if positions.is_empty() || d.get(i).is_none() {
            return None;
        }
        Some(Song {
            file: Arc::new(d.to_vec()),
            version,
            table: d[99],
            name: text(&d[30..62]),
            author: text(&d[66..98]),
            tempo: d[100],
            loop_pos: usize::from(d[102]).min(positions.len() - 1),
            positions,
            pattern_table: le16(d, 103),
        })
    }

    fn note_table(&self) -> &'static [u16; 96] {
        let old = self.version <= 3;
        match self.table {
            0 if old => &T0_OLD,
            0 => &T0_NEW,
            1 => &T1,
            2 if old => &T2_OLD,
            2 => &T2_NEW,
            _ if old => &T3_OLD,
            _ => &T3_NEW,
        }
    }

    fn volume(&self, v: usize, a: usize) -> u8 {
        if self.version <= 4 {
            VOL_OLD[v & 15][a & 15]
        } else {
            vol_new(v & 15, a & 15)
        }
    }

    fn b(&self, i: usize) -> u8 {
        self.file.get(i).copied().unwrap_or(0)
    }

    /// Where pattern `p`'s track for channel `c` starts.
    fn track(&self, p: usize, c: usize) -> usize {
        le16(&self.file, self.pattern_table + p * 6 + c * 2)
    }

    fn sample(&self, n: usize) -> usize {
        le16(&self.file, 105 + (n & 31) * 2)
    }

    fn ornament(&self, n: usize) -> usize {
        le16(&self.file, 169 + (n & 15) * 2)
    }
}

/// A row's cell of one channel, for the screen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cell {
    /// The note (0 the table's first, C-1), or 0xFF for a stop.
    pub note: Option<u8>,
    pub sample: Option<u8>,
    pub ornament: Option<u8>,
    pub volume: Option<u8>,
}

impl Song {
    /// A pattern's rows: its length (where channel A's track ends, at
    /// most 256) and each row's cells, each channel's lines laid out by
    /// how many rows each holds.
    pub fn pattern(&self, p: usize) -> Vec<[Cell; 3]> {
        let mut addr = [self.track(p, 0), self.track(p, 1), self.track(p, 2)];
        let mut skip = [1u8; 3];
        let mut counter = [1u8; 3];
        let mut rows = Vec::new();
        'rows: while rows.len() < 256 {
            let mut row = [Cell::default(); 3];
            for c in 0..3 {
                counter[c] = counter[c].wrapping_sub(1);
                if counter[c] != 0 {
                    continue;
                }
                if c == 0 && self.b(addr[0]) == 0 {
                    break 'rows;
                }
                let (cell, next, sk) = self.line(addr[c], skip[c]);
                row[c] = cell;
                addr[c] = next;
                skip[c] = sk;
                counter[c] = sk;
            }
            rows.push(row);
        }
        rows
    }

    /// One line of a track: its cell, where the next starts, and how many
    /// rows it holds.
    fn line(&self, mut a: usize, mut skip: u8) -> (Cell, usize, u8) {
        let mut cell = Cell::default();
        let mut effects = 0usize;
        for _ in 0..256 {
            let v = self.b(a);
            a += 1;
            match v {
                0xF0..=0xFF => {
                    cell.ornament = Some(v - 0xF0);
                    cell.sample = Some(self.b(a) / 2);
                    a += 1;
                }
                0xD1..=0xEF => cell.sample = Some(v - 0xD0),
                0xD0 => break,
                0xC1..=0xCF => cell.volume = Some(v - 0xC0),
                0xC0 => {
                    cell.note = Some(0xFF);
                    break;
                }
                0xB2..=0xBF => a += 2,
                0xB1 => {
                    skip = self.b(a);
                    a += 1;
                }
                0x50..=0xAF => {
                    cell.note = Some(v - 0x50);
                    break;
                }
                0x40..=0x4F => cell.ornament = Some(v - 0x40),
                0x11..=0x1F => {
                    cell.sample = Some(self.b(a + 2) / 2);
                    a += 3;
                }
                0x10 => {
                    cell.sample = Some(self.b(a) / 2);
                    a += 1;
                }
                0x01..=0x09 => effects += [0, 3, 5, 1, 1, 2, 0, 0, 3, 1][usize::from(v)],
                _ => {}
            }
        }
        (cell, a + effects, skip)
    }
}

impl Tune for Song {
    fn format(&self) -> &'static str {
        "pt3"
    }
    fn file(&self) -> &[u8] {
        &self.file
    }
    fn driver(&self, _song: usize) -> Box<dyn Driver> {
        Box::new(Player::new(Arc::new(self.clone())))
    }

    /// Each sample (an AY instrument: its loop, then lines of tone, noise,
    /// volume and envelope, 4 bytes each), with the editor's version,
    /// which picks its volume table. Ornaments go with the notes, not
    /// with a sample.
    fn patches(&self) -> Vec<Patch> {
        let mut out = Vec::new();
        for n in 1..32 {
            let at = self.sample(n);
            if at == 0 {
                continue;
            }
            let len = usize::from(self.b(at + 1));
            let Some(lines) = self.file.get(at + 2..at + 2 + len * 4) else { continue };
            if lines.iter().all(|&x| x == 0) {
                continue;
            }
            let mut b = PatchBytes::default();
            b.u8(u8::from(self.version >= 5)).u8(self.b(at)).bytes(lines);
            out.push(b.patch(n, ""));
        }
        out
    }
}

#[derive(Clone, Copy, Default)]
struct Chan {
    addr: usize,
    orn: usize,
    smp: usize,
    ton: u16,
    orn_loop: u8,
    orn_len: u8,
    orn_pos: u8,
    smp_loop: u8,
    smp_len: u8,
    smp_pos: u8,
    volume: u8,
    skip_notes: u8,
    note: u8,
    slide_to_note: u8,
    amplitude: u8,
    env_on: bool,
    enabled: bool,
    simple_gliss: bool,
    amp_sliding: i8,
    noise_sliding: i8,
    env_sliding: i8,
    ton_slide_count: i16,
    cur_onoff: i16,
    onoff_delay: i16,
    offon_delay: i16,
    ton_slide_delay: i16,
    ton_sliding: i16,
    ton_accumulator: i16,
    ton_slide_step: i16,
    ton_delta: i16,
    skip_counter: u8,
}

pub struct Player {
    song: Arc<Song>,
    ch: [Chan; 3],
    delay: u8,
    delay_counter: u8,
    position: usize,
    env_base: u16,
    cur_env_slide: i16,
    env_slide_add: i16,
    cur_env_delay: i16,
    env_delay: i16,
    noise_base: u8,
    add_to_noise: u8,
    add_to_env: i16,
    /// The chip's registers after the last frame, and whether the
    /// envelope's shape was written in it.
    pub regs: [u8; 14],
    pub shape_written: bool,
    ay: Ay,
    rows: usize,
    shown: (usize, usize),
    done: bool,
    ended: bool,
}

impl Player {
    pub fn new(song: Arc<Song>) -> Player {
        let mut p = Player {
            ch: [Chan::default(); 3],
            delay: song.tempo,
            delay_counter: 1,
            position: 0,
            env_base: 0,
            cur_env_slide: 0,
            env_slide_add: 0,
            cur_env_delay: 0,
            env_delay: 0,
            noise_base: 0,
            add_to_noise: 0,
            add_to_env: 0,
            regs: [0; 14],
            shape_written: false,
            ay: Ay::new(false, AY_CLOCK, 44100),
            rows: 0,
            shown: (0, 0),
            done: false,
            ended: false,
            song,
        };
        let pat = p.song.positions[0];
        for c in 0..3 {
            let ch = &mut p.ch[c];
            ch.addr = p.song.track(pat, c);
            ch.volume = 15;
            ch.skip_counter = 1;
        }
        for c in 0..3 {
            p.set_sample(c, 1);
            p.set_ornament(c, 0);
        }
        p
    }

    fn set_sample(&mut self, c: usize, n: usize) {
        let s = self.song.sample(n);
        let ch = &mut self.ch[c];
        ch.smp = s + 2;
        ch.smp_loop = self.song.b(s);
        ch.smp_len = self.song.b(s + 1);
    }

    fn set_ornament(&mut self, c: usize, n: usize) {
        let o = self.song.ornament(n);
        let ch = &mut self.ch[c];
        ch.orn = o + 2;
        ch.orn_loop = self.song.b(o);
        ch.orn_len = self.song.b(o + 1);
        ch.orn_pos = 0;
    }

    /// The channel's track read to the end of its next line.
    fn interpret(&mut self, c: usize) {
        let song = self.song.clone();
        let table = song.note_table();
        let pr_note = self.ch[c].note;
        let pr_sliding = self.ch[c].ton_sliding;
        let mut counter = 0u8;
        let (mut f1, mut f2, mut f3, mut f4, mut f5, mut f8, mut f9) = (0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8);
        let mut a = self.ch[c].addr;
        let mut guard = 0;
        loop {
            guard += 1;
            if guard > 4096 {
                break;
            }
            let v = song.b(a);
            match v {
                0xF0..=0xFF => {
                    self.set_ornament(c, usize::from(v - 0xF0));
                    a += 1;
                    self.set_sample(c, usize::from(song.b(a) / 2));
                    self.ch[c].env_on = false;
                    self.ch[c].orn_pos = 0;
                }
                0xD1..=0xEF => self.set_sample(c, usize::from(v - 0xD0)),
                0xD0 => {
                    a += 1;
                    break;
                }
                0xC1..=0xCF => self.ch[c].volume = v - 0xC0,
                0xC0 => {
                    let ch = &mut self.ch[c];
                    ch.smp_pos = 0;
                    ch.amp_sliding = 0;
                    ch.noise_sliding = 0;
                    ch.env_sliding = 0;
                    ch.orn_pos = 0;
                    ch.ton_slide_count = 0;
                    ch.ton_sliding = 0;
                    ch.ton_accumulator = 0;
                    ch.cur_onoff = 0;
                    ch.enabled = false;
                    a += 1;
                    break;
                }
                0xB2..=0xBF => {
                    self.ch[c].env_on = true;
                    self.set_env_shape(v - 0xB1);
                    self.env_base = (u16::from(song.b(a + 1)) << 8) | u16::from(song.b(a + 2));
                    a += 2;
                    self.ch[c].orn_pos = 0;
                    self.cur_env_slide = 0;
                    self.cur_env_delay = 0;
                }
                0xB1 => {
                    a += 1;
                    self.ch[c].skip_notes = song.b(a);
                }
                0xB0 => {
                    self.ch[c].env_on = false;
                    self.ch[c].orn_pos = 0;
                }
                0x50..=0xAF => {
                    let ch = &mut self.ch[c];
                    ch.note = v - 0x50;
                    ch.smp_pos = 0;
                    ch.amp_sliding = 0;
                    ch.noise_sliding = 0;
                    ch.env_sliding = 0;
                    ch.orn_pos = 0;
                    ch.ton_slide_count = 0;
                    ch.ton_sliding = 0;
                    ch.ton_accumulator = 0;
                    ch.cur_onoff = 0;
                    ch.enabled = true;
                    a += 1;
                    break;
                }
                0x40..=0x4F => {
                    self.set_ornament(c, usize::from(v - 0x40));
                }
                0x20..=0x3F => self.noise_base = v - 0x20,
                0x10..=0x1F => {
                    if v == 0x10 {
                        self.ch[c].env_on = false;
                    } else {
                        self.set_env_shape(v - 0x10);
                        self.env_base = (u16::from(song.b(a + 1)) << 8) | u16::from(song.b(a + 2));
                        a += 2;
                        self.ch[c].env_on = true;
                        self.cur_env_slide = 0;
                        self.cur_env_delay = 0;
                    }
                    a += 1;
                    self.set_sample(c, usize::from(song.b(a) / 2));
                    self.ch[c].orn_pos = 0;
                }
                0x09 => {
                    counter += 1;
                    f9 = counter;
                }
                0x08 => {
                    counter += 1;
                    f8 = counter;
                }
                0x05 => {
                    counter += 1;
                    f5 = counter;
                }
                0x04 => {
                    counter += 1;
                    f4 = counter;
                }
                0x03 => {
                    counter += 1;
                    f3 = counter;
                }
                0x02 => {
                    counter += 1;
                    f2 = counter;
                }
                0x01 => {
                    counter += 1;
                    f1 = counter;
                }
                _ => {}
            }
            a += 1;
        }
        // The effects' parameters, after the line, the last effect's first.
        while counter > 0 {
            if counter == f1 {
                let ch = &mut self.ch[c];
                ch.ton_slide_delay = i16::from(song.b(a));
                ch.ton_slide_count = ch.ton_slide_delay;
                ch.ton_slide_step = le16(&song.file, a + 1) as u16 as i16;
                a += 3;
                ch.simple_gliss = true;
                ch.cur_onoff = 0;
                if ch.ton_slide_count == 0 && song.version >= 7 {
                    ch.ton_slide_count += 1;
                }
            } else if counter == f2 {
                let ch = &mut self.ch[c];
                ch.simple_gliss = false;
                ch.cur_onoff = 0;
                ch.ton_slide_delay = i16::from(song.b(a));
                ch.ton_slide_count = ch.ton_slide_delay;
                a += 3;
                ch.ton_slide_step = (le16(&song.file, a) as u16 as i16).wrapping_abs();
                a += 2;
                ch.ton_delta = (table[usize::from(ch.note).min(95)] as i16).wrapping_sub(table[usize::from(pr_note).min(95)] as i16);
                ch.slide_to_note = ch.note;
                ch.note = pr_note;
                if song.version >= 6 {
                    ch.ton_sliding = pr_sliding;
                }
                if ch.ton_delta.wrapping_sub(ch.ton_sliding) < 0 {
                    ch.ton_slide_step = ch.ton_slide_step.wrapping_neg();
                }
            } else if counter == f3 {
                self.ch[c].smp_pos = song.b(a);
                a += 1;
            } else if counter == f4 {
                self.ch[c].orn_pos = song.b(a);
                a += 1;
            } else if counter == f5 {
                let ch = &mut self.ch[c];
                ch.onoff_delay = i16::from(song.b(a));
                ch.offon_delay = i16::from(song.b(a + 1));
                ch.cur_onoff = ch.onoff_delay;
                a += 2;
                ch.ton_slide_count = 0;
                ch.ton_sliding = 0;
            } else if counter == f8 {
                self.env_delay = i16::from(song.b(a));
                self.cur_env_delay = self.env_delay;
                self.env_slide_add = le16(&song.file, a + 1) as u16 as i16;
                a += 3;
            } else if counter == f9 {
                self.delay = song.b(a);
                a += 1;
            }
            counter -= 1;
        }
        let ch = &mut self.ch[c];
        ch.addr = a;
        ch.skip_counter = ch.skip_notes;
    }

    fn set_env_shape(&mut self, shape: u8) {
        self.regs[13] = shape;
        self.shape_written = true;
    }

    /// The channel's registers for this frame; its mixer bits into `mixer`.
    fn change_registers(&mut self, c: usize, mixer: &mut u8) {
        let song = self.song.clone();
        let table = song.note_table();
        let ch = &mut self.ch[c];
        if ch.enabled {
            let s = ch.smp + usize::from(ch.smp_pos) * 4;
            let (b0, b1) = (song.b(s), song.b(s + 1));
            let mut ton = (le16(&song.file, s + 2) as u16 as i16).wrapping_add(ch.ton_accumulator);
            if b1 & 0x40 != 0 {
                ch.ton_accumulator = ton;
            }
            let j = i16::from(ch.note) + i16::from(song.b(ch.orn + usize::from(ch.orn_pos)) as i8);
            let j = j.clamp(0, 95) as usize;
            ton = ton.wrapping_add(ch.ton_sliding).wrapping_add(table[j] as i16);
            // All 16 bits go to the chip, which takes 12.
            ch.ton = ton as u16;
            if ch.ton_slide_count > 0 {
                ch.ton_slide_count -= 1;
                if ch.ton_slide_count == 0 {
                    ch.ton_sliding = ch.ton_sliding.wrapping_add(ch.ton_slide_step);
                    ch.ton_slide_count = ch.ton_slide_delay;
                    if !ch.simple_gliss && ((ch.ton_slide_step < 0 && ch.ton_sliding <= ch.ton_delta) || (ch.ton_slide_step >= 0 && ch.ton_sliding >= ch.ton_delta)) {
                        ch.note = ch.slide_to_note;
                        ch.ton_slide_count = 0;
                        ch.ton_sliding = 0;
                    }
                }
            }
            let mut amp = i16::from(b1 & 0x0F);
            if b0 & 0x80 != 0 {
                if b0 & 0x40 != 0 {
                    if ch.amp_sliding < 15 {
                        ch.amp_sliding += 1;
                    }
                } else if ch.amp_sliding > -15 {
                    ch.amp_sliding -= 1;
                }
            }
            amp = (amp + i16::from(ch.amp_sliding)).clamp(0, 15);
            ch.amplitude = song.volume(usize::from(ch.volume), amp as usize);
            if b0 & 1 == 0 && ch.env_on {
                ch.amplitude |= 0x10;
            }
            if b1 & 0x80 == 0 {
                // The whole byte shifted: the volume slide's two bits too.
                let j = ((b0 >> 1) as i8).wrapping_add(ch.noise_sliding);
                if b1 & 0x20 != 0 {
                    ch.noise_sliding = j;
                }
                self.add_to_noise = j as u8;
            } else {
                let mut j = ((b0 >> 1) & 0x1F) as i8;
                if j & 0x10 != 0 {
                    j |= -0x10i8 as i8;
                }
                let j = j.wrapping_add(ch.env_sliding);
                if b1 & 0x20 != 0 {
                    ch.env_sliding = j;
                }
                self.add_to_env = self.add_to_env.wrapping_add(i16::from(j));
            }
            *mixer |= (b1 >> 1) & 0x48;
            // Round when the position reaches the length exactly (one set
            // past it by an offset runs on through memory).
            ch.smp_pos = ch.smp_pos.wrapping_add(1);
            if ch.smp_pos == ch.smp_len {
                ch.smp_pos = ch.smp_loop;
            }
            ch.orn_pos = ch.orn_pos.wrapping_add(1);
            if ch.orn_pos == ch.orn_len {
                ch.orn_pos = ch.orn_loop;
            }
        } else {
            ch.amplitude = 0;
        }
        *mixer >>= 1;
        if ch.cur_onoff > 0 {
            ch.cur_onoff -= 1;
            if ch.cur_onoff == 0 {
                ch.enabled = !ch.enabled;
                ch.cur_onoff = if ch.enabled { ch.onoff_delay } else { ch.offon_delay };
            }
        }
    }

    /// One frame (1/50 s): the tracks read on when it's time, the chip's
    /// registers made.
    pub fn frame(&mut self) {
        self.shape_written = false;
        self.delay_counter = self.delay_counter.wrapping_sub(1);
        if self.delay_counter == 0 {
            for c in 0..3 {
                self.ch[c].skip_counter = self.ch[c].skip_counter.wrapping_sub(1);
                if self.ch[c].skip_counter == 0 {
                    if c == 0 && self.song.b(self.ch[0].addr) == 0 {
                        self.next_position();
                    }
                    self.interpret(c);
                }
            }
            self.delay_counter = self.delay;
            self.rows += 1;
        }
        // The noise's offset is kept from frame to frame, set only by a
        // sample line with its noise on.
        self.add_to_env = 0;
        let mut mixer = 0u8;
        for c in 0..3 {
            self.change_registers(c, &mut mixer);
        }
        let r = &mut self.regs;
        for c in 0..3 {
            r[c * 2] = self.ch[c].ton as u8;
            r[c * 2 + 1] = (self.ch[c].ton >> 8) as u8;
            r[8 + c] = self.ch[c].amplitude;
        }
        r[7] = mixer;
        r[6] = self.noise_base.wrapping_add(self.add_to_noise);
        let env = (self.env_base as i16).wrapping_add(self.add_to_env).wrapping_add(self.cur_env_slide) as u16;
        r[11] = env as u8;
        r[12] = (env >> 8) as u8;
        if self.cur_env_delay > 0 {
            self.cur_env_delay -= 1;
            if self.cur_env_delay == 0 {
                self.cur_env_delay = self.env_delay;
                self.cur_env_slide = self.cur_env_slide.wrapping_add(self.env_slide_add);
            }
        }
    }

    fn next_position(&mut self) {
        self.position += 1;
        if self.position >= self.song.positions.len() {
            self.position = self.song.loop_pos;
            self.done = true;
            self.ended = true;
        }
        let pat = self.song.positions[self.position];
        for c in 0..3 {
            self.ch[c].addr = self.song.track(pat, c);
        }
        self.noise_base = 0;
        self.rows = 0;
    }
}

impl Driver for Player {
    fn channels(&self) -> usize {
        3
    }

    fn tick(&mut self) {
        self.frame();
        self.shown = (self.position, self.rows.saturating_sub(1));
        let (r, w) = (self.regs, self.shape_written);
        self.ay.write_regs(&r, w);
    }

    fn tick_seconds(&self) -> f64 {
        FRAME
    }

    fn render(&mut self, rate: u32, frames: usize, out: &mut Bufs) {
        self.ay.set_rate(rate);
        // A, B and C as the Spectrum's ABC stereo places them.
        const PAN: [f32; 3] = [0.2, 0.5, 0.8];
        for i in 0..frames {
            self.ay.process(true);
            for c in 0..3 {
                let x = self.ay.out[c] as f32 * 0.6;
                out.left[c][i] = x * (1.0 - PAN[c]);
                out.right[c][i] = x * PAN[c];
            }
        }
    }

    fn position(&self) -> (usize, usize) {
        self.shown
    }

    fn speed(&self) -> (u32, u32) {
        (u32::from(self.delay), 125)
    }

    fn take_done(&mut self) -> bool {
        std::mem::take(&mut self.done)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_new_volume_table() {
        assert_eq!(vol_new(15, 15), 15);
        assert_eq!(vol_new(1, 8), 1);
        assert_eq!(vol_new(1, 7), 0);
    }

    /// The AY's registers each frame, as a player run as a black box logs
    /// them (pt3log): PT3_DIR the songs, PT3_REF the logs (<name>.log).
    #[test]
    #[ignore]
    fn the_same_as_a_player() {
        let (Ok(dir), Ok(rf)) = (std::env::var("PT3_DIR"), std::env::var("PT3_REF")) else { return };
        let only = std::env::var("PT3_ONLY").ok();
        let mut paths: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).collect();
        paths.sort();
        let (mut same, mut bad) = (0, 0);
        for path in paths {
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            if only.as_ref().is_some_and(|o| !name.contains(o.as_str())) {
                continue;
            }
            let Ok(log) = std::fs::read_to_string(format!("{rf}/{name}.log")) else { continue };
            let Some(song) = Song::parse(&std::fs::read(&path).unwrap()) else {
                eprintln!("{name}: not read");
                bad += 1;
                continue;
            };
            let mut p = Player::new(Arc::new(song));
            let mut first = None;
            for line in log.lines() {
                p.frame();
                let mut ours = line.split(' ').next().unwrap().to_string();
                for r in 0..14 {
                    ours += &format!(" {:02x}", p.regs[r]);
                }
                if p.shape_written {
                    ours += " E";
                }
                let theirs = line.replace(" END", "");
                if std::env::var("PT3_SHOW").is_ok() {
                    let c = &p.ch[2];
                    eprintln!(
                        "{}\n{ours}   smp {} pos {} len {} loop {} orn {} opos {} olen {} note {} acc {} sl {} cnt {} en {}",
                        theirs, c.smp, c.smp_pos, c.smp_len, c.smp_loop, c.orn, c.orn_pos, c.orn_len, c.note, c.ton_accumulator, c.ton_sliding, c.ton_slide_count, c.enabled
                    );
                }
                if ours != theirs {
                    first = Some(format!("{name}\n  ours   {ours}\n  theirs {theirs}"));
                    break;
                }
            }
            match first {
                None => same += 1,
                Some(f) => {
                    bad += 1;
                    if bad <= 12 {
                        eprintln!("{f}");
                    }
                }
            }
        }
        eprintln!("{same} the same, {bad} not");
        assert_eq!(bad, 0);
    }
}
