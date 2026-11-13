// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.
//
// GoatTracker 2's play routine (player.s, Lasse Öörni), followed here from
// its source, which its author gave away apart from the editor's GPL: "This
// playroutine source code does not fall under the GPL license! Use it, or
// song binaries created from it freely for any purpose, commercial or
// noncommercial." The song's file is read from GoatTracker 2's published
// description of it (its readme's "File/data formats description").

//! GoatTracker 2's songs (.sng, "GTS5"): C64 music of three SID channels,
//! each an order list of patterns (transposed, repeated) whose rows play
//! instruments driven by wave, pulse and filter tables, with portamento,
//! vibrato and tempo commands; up to 32 subtunes. Played by a port of the
//! editor's own play routine, a frame (1/50 s) at a time, on the SID
//! (`sid.rs`). The song is first laid out as the routine reads it (its
//! packed patterns, order lists and tables), as the editor's relocator lays
//! it out.

use std::sync::Arc;

use crate::replay::{Bufs, Driver, Patch, PatchBytes, Tune};
use crate::sid::{Sid, PAL_CLOCK};

/// The frequencies of C-0 to B-7 on a PAL C64's SID.
const FREQ: [u16; 96] = [
    0x0117, 0x0127, 0x0139, 0x014b, 0x015f, 0x0174, 0x018a, 0x01a1, 0x01ba, 0x01d4, 0x01f0, 0x020e, 0x022d, 0x024e, 0x0271, 0x0296, 0x02be, 0x02e8, 0x0314, 0x0343, 0x0374, 0x03a9, 0x03e1, 0x041c,
    0x045a, 0x049c, 0x04e2, 0x052d, 0x057c, 0x05cf, 0x0628, 0x0685, 0x06e8, 0x0752, 0x07c1, 0x0837, 0x08b4, 0x0939, 0x09c5, 0x0a5a, 0x0af7, 0x0b9e, 0x0c4f, 0x0d0a, 0x0dd1, 0x0ea3, 0x0f82, 0x106e,
    0x1168, 0x1271, 0x138a, 0x14b3, 0x15ee, 0x173c, 0x189e, 0x1a15, 0x1ba2, 0x1d46, 0x1f04, 0x20dc, 0x22d0, 0x24e2, 0x2714, 0x2967, 0x2bdd, 0x2e79, 0x313c, 0x3429, 0x3744, 0x3a8d, 0x3e08, 0x41b8,
    0x45a1, 0x49c5, 0x4e28, 0x52cd, 0x57ba, 0x5cf1, 0x6278, 0x6853, 0x6e87, 0x751a, 0x7c10, 0x8371, 0x8b42, 0x9389, 0x9c4f, 0xa59b, 0xaf74, 0xb9e2, 0xc4f0, 0xd0a6, 0xdd0e, 0xea33, 0xf820, 0xffff,
];

// Pattern data as the routine reads it.
const ENDPATT: u8 = 0x00;
const FX: u8 = 0x40;
const FXONLY: u8 = 0x50;
const NOTE: u8 = 0x60;
const REST: u8 = 0xbd;
const FIRSTPACKEDREST: u8 = 0xc0;
const REPEAT: u8 = 0xd0;
const TRANSDOWN: u8 = 0xe0;
const TRANS: u8 = 0xf0;
const LOOPSONG: u8 = 0xff;
const TONEPORTA: u8 = 3;

/// The hard restart's attack/decay and sustain/release (the relocator's
/// default, "0F00").
const ADPARAM: u8 = 0x0f;
const SRPARAM: u8 = 0x00;

#[derive(Clone, Copy, Default)]
struct Ins {
    ad: u8,
    sr: u8,
    wave: u8,
    pulse: u8,
    filt: u8,
    vibparam: u8,
    vibdelay: u8,
    gatetimer: u8,
    firstwave: u8,
}

/// A GoatTracker 2 song.
#[derive(Clone)]
pub struct Song {
    file: Arc<Vec<u8>>,
    pub name: String,
    pub author: String,
    pub copyright: String,
    subtunes: usize,
    /// Each subtune's order lists (subtune * 3 + channel), as in the file.
    pub orders: Vec<Vec<u8>>,
    ins: Vec<Ins>,
    pub ins_names: Vec<String>,
    tables: [(Vec<u8>, Vec<u8>); 4],
    /// The patterns' rows: note, instrument, command, data.
    pub patterns: Vec<Vec<[u8; 4]>>,
}

pub fn probe(d: &[u8]) -> bool {
    (d.starts_with(b"GTS5") || d.starts_with(b"GTS2")) && Song::parse(d).is_some()
}

fn text(b: &[u8]) -> String {
    let b = &b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())];
    b.iter().map(|&c| if (0x20..0x7f).contains(&c) { c as char } else { ' ' }).collect::<String>().trim().to_string()
}

impl Song {
    pub fn parse(d: &[u8]) -> Option<Song> {
        // GTS2: the format before the speed table (v2.2) and finer pulse speeds (v2.4).
        let old = d.starts_with(b"GTS2");
        if !(d.starts_with(b"GTS5") || old) || d.len() < 101 {
            return None;
        }
        let mut at = 101usize;
        let mut take = |n: usize| -> Option<&[u8]> {
            let s = d.get(at..at + n)?;
            at += n;
            Some(s)
        };
        let subtunes = usize::from(d[100]);
        if subtunes == 0 || subtunes > 32 {
            return None;
        }
        let mut orders = Vec::new();
        for _ in 0..subtunes * 3 {
            let n = usize::from(take(1)?[0]);
            orders.push(take(n + 1)?.to_vec());
        }
        let n = usize::from(take(1)?[0]);
        if n > 63 {
            return None;
        }
        let mut ins = vec![Ins::default()];
        let mut ins_names = Vec::new();
        for _ in 0..n {
            let b = take(25)?;
            ins.push(Ins {
                ad: b[0],
                sr: b[1],
                wave: b[2],
                pulse: b[3],
                filt: b[4],
                vibparam: b[5],
                vibdelay: b[6],
                gatetimer: b[7],
                firstwave: b[8],
            });
            ins_names.push(text(&b[9..25]));
        }
        let mut table = || -> Option<(Vec<u8>, Vec<u8>)> {
            let n = usize::from(take(1)?[0]);
            let l = take(n)?.to_vec();
            let r = take(n)?.to_vec();
            Some((l, r))
        };
        let tables = if old {
            [table()?, table()?, table()?, (Vec::new(), Vec::new())]
        } else {
            [table()?, table()?, table()?, table()?]
        };
        let n = usize::from(take(1)?[0]);
        let mut patterns = Vec::new();
        for _ in 0..n {
            let rows = usize::from(take(1)?[0]);
            let b = take(rows * 4)?;
            patterns.push(b.chunks_exact(4).map(|r| [r[0], r[1], r[2], r[3]]).collect());
        }
        let mut song = Song {
            file: Arc::new(d.to_vec()),
            name: text(&d[4..36]),
            author: text(&d[36..68]),
            copyright: text(&d[68..100]),
            subtunes,
            orders,
            ins,
            ins_names,
            tables,
            patterns,
        };
        if old {
            song.update();
        }
        Some(song)
    }

    // An old song's speeds as the speed table holds them since v2.2, and
    // its pulse speeds doubled (v2.4), as GoatTracker 2 converts it.
    fn update(&mut self) {
        let mut speed: Vec<(u8, u8)> = Vec::new();
        let mut entry = |v: (u8, u8)| -> u8 {
            let i = speed.iter().position(|&e| e == v).unwrap_or_else(|| {
                speed.push(v);
                speed.len() - 1
            });
            (i + 1) as u8
        };
        let porta = |p: u8| ((u16::from(p) * 4) >> 8, (u16::from(p) * 4) as u8);
        for pat in self.patterns.iter_mut() {
            for r in pat.iter_mut() {
                let p = r[3];
                r[3] = match r[2] & 0x0f {
                    1 | 2 if p != 0 => {
                        let (h, l) = porta(p);
                        entry((h as u8, l))
                    }
                    3 if p != 0 => {
                        let (h, l) = porta(p);
                        entry((h as u8, l))
                    }
                    4 if p != 0 => entry((p >> 4, (p & 0x0f) << 4)),
                    0x0e if p != 0 => entry((p >> 4, p & 0x0f)),
                    _ => p,
                };
            }
        }
        for i in self.ins.iter_mut() {
            // Its delay came before its parameter.
            std::mem::swap(&mut i.vibparam, &mut i.vibdelay);
            // A first frame wave of 0 was legato (no hard restart or gate
            // off), before the gate timer's flags (v2.59).
            if i.firstwave == 0 {
                i.gatetimer |= 0x40;
            }
            // Its high bit was no hard restart.
            if i.firstwave & 0x80 != 0 {
                i.firstwave &= 0x7f;
                i.gatetimer |= 0x80;
            }
            if i.vibparam != 0 {
                i.vibparam = entry((i.vibparam >> 4, (i.vibparam & 0x0f) << 4));
            }
        }
        let (pl, pr) = &mut self.tables[1];
        for (l, r) in pl.iter().zip(pr.iter_mut()) {
            if (1..0x80).contains(l) {
                *r = (i16::from(*r as i8) * 2).clamp(-128, 127) as u8;
            }
        }
        self.tables[3] = (speed.iter().map(|e| e.0).collect(), speed.iter().map(|e| e.1).collect());
    }

    pub fn subtunes(&self) -> usize {
        self.subtunes
    }

    /// Its notes laid out by channel 1's rows, played without the SID to
    /// where channel 1's order list comes round: each channel's
    /// (pattern, row) and note at each of its rows.
    pub fn timeline(&self, subtune: usize, max_frames: usize) -> Timeline {
        let mut r = Routine::new(self);
        r.init(subtune as u8);
        follow(r, max_frames)
    }
}

/// A GoatTracker play routine, a frame at a time (2's or 1's).
pub trait Frames: Send {
    fn frame(&mut self);
    /// The SID's registers written in the frame, in order.
    fn writes(&self) -> &[(u8, u8)];
    /// What channel `c` did in the frame: the row it plays from (its
    /// pattern and index), a note begun, a key off, its order list come
    /// round.
    fn shown(&self, c: usize) -> (Option<(u8, u8)>, Option<u8>, bool, bool);
    /// Channel 1's tempo (ticks a row, less one).
    fn tempo(&self) -> u8;
}

impl Frames for Routine {
    fn frame(&mut self) {
        self.play();
    }
    fn writes(&self) -> &[(u8, u8)] {
        &self.writes
    }
    fn shown(&self, c: usize) -> (Option<(u8, u8)>, Option<u8>, bool, bool) {
        let s = self.shown[c];
        (s.at, s.note, s.off, s.looped)
    }
    fn tempo(&self) -> u8 {
        self.c[0].tempo
    }
}

/// A song's notes laid out by channel 1's rows, played without the SID to
/// where channel 1's order list comes round.
pub fn follow<R: Frames>(mut r: R, max_frames: usize) -> Timeline {
    let mut tl = Timeline::default();
    let mut row = None;
    for f in 0..max_frames {
        r.frame();
        if r.shown(0).3 && f > 0 {
            tl.looped = true;
            break;
        }
        if let Some(at) = r.shown(0).0 {
            tl.rows.push(at);
            row = Some(tl.rows.len() - 1);
        }
        for c in 0..3 {
            let Some(i) = row else { continue };
            let (_, note, off, _) = r.shown(c);
            if let Some(n) = note {
                tl.notes.push((i, c, Some(n)));
            } else if off {
                tl.notes.push((i, c, None));
            }
        }
        tl.frames = f + 1;
    }
    if tl.looped {
        // Where it comes back to: the first row after the loop.
        for _ in 0..50 * 60 {
            if let Some(at) = r.shown(0).0 {
                tl.restart_row = tl.rows.iter().position(|&x| x == at);
                break;
            }
            r.frame();
        }
    }
    tl
}

/// A song's notes laid out by channel 1's rows.
#[derive(Default)]
pub struct Timeline {
    /// Each of channel 1's rows: its pattern and index there.
    pub rows: Vec<(u8, u8)>,
    /// (row, channel, note; None a key off).
    pub notes: Vec<(usize, usize, Option<u8>)>,
    pub frames: usize,
    pub looped: bool,
    /// The row it loops back to.
    pub restart_row: Option<usize>,
}

impl Timeline {
    /// Where each pattern shown starts: a new one each time channel 1's row
    /// isn't the next of the one before.
    pub fn pattern_starts(&self) -> Vec<usize> {
        let mut starts = Vec::new();
        let mut prev: Option<(u8, u8)> = None;
        for (i, &(p, r)) in self.rows.iter().enumerate() {
            if prev.is_none_or(|(pp, pr)| r == 0 || p != pp || r != pr.wrapping_add(1)) {
                starts.push(i);
            }
            prev = Some((p, r));
        }
        starts
    }
}

/// A PAL C64 frame in seconds (312 lines of 63 cycles).
pub const FRAME: f64 = 19_656.0 / PAL_CLOCK as f64;

impl Tune for Song {
    fn format(&self) -> &'static str {
        "gt2"
    }
    fn file(&self) -> &[u8] {
        &self.file
    }
    fn songs(&self) -> usize {
        self.subtunes
    }
    fn driver(&self, song: usize) -> Box<dyn Driver> {
        let mut r = Routine::new(self);
        r.init(song as u8);
        Box::new(Player::new(r))
    }

    /// Each instrument: its ADSR, vibrato delay, gate timer and first
    /// wave, then its wave, pulse and filter programs (each followed from
    /// where it points through its jumps, the rows renumbered from 1 in
    /// the patch) and its vibrato's speed entry.
    fn patches(&self) -> Vec<Patch> {
        let mut out = Vec::new();
        for (k, i) in self.ins.iter().enumerate().skip(1) {
            if i.wave == 0 {
                continue;
            }
            let mut b = PatchBytes::default();
            b.u8(i.ad).u8(i.sr).u8(i.vibdelay).u8(i.gatetimer).u8(i.firstwave);
            for (t, at) in [(0, i.wave), (1, i.pulse), (2, i.filt)] {
                let rows = chase(&self.tables[t], at);
                b.u16(rows.len() as u16);
                for (l, r) in rows {
                    b.u8(l).u8(r);
                }
            }
            let (sl, sr) = &self.tables[3];
            let v = usize::from(i.vibparam).wrapping_sub(1);
            b.u8(sl.get(v).copied().unwrap_or(0)).u8(sr.get(v).copied().unwrap_or(0));
            out.push(b.patch(k, self.ins_names.get(k - 1).map_or("", |n| n.as_str())));
        }
        out
    }
}

/// A table's program from row `start` (from 1; 0 none) as far as it
/// goes: its rows in the order they're reached, each jump (`FF`, to the
/// row on its right) followed and pointed at its row here, from 1.
pub(crate) fn chase(table: &(Vec<u8>, Vec<u8>), start: u8) -> Vec<(u8, u8)> {
    let (left, right) = table;
    let mut rows: Vec<(u8, u8)> = Vec::new();
    let mut place: Vec<(usize, usize)> = Vec::new();
    let mut jumps: Vec<(usize, usize)> = Vec::new();
    let mut at = usize::from(start);
    while at >= 1 && at <= left.len() && rows.len() < 256 && !place.iter().any(|&(o, _)| o == at) {
        place.push((at, rows.len()));
        let (l, r) = (left[at - 1], right.get(at - 1).copied().unwrap_or(0));
        rows.push((l, r));
        if l == 0xFF {
            jumps.push((rows.len() - 1, usize::from(r)));
            at = usize::from(r);
        } else {
            at += 1;
        }
    }
    for (row, to) in jumps {
        rows[row].1 = place.iter().find(|&&(o, _)| o == to).map_or(0, |&(_, p)| p as u8 + 1);
    }
    rows
}

/// A song played on the SID.
pub struct Player<R> {
    r: R,
    sid: Sid,
    rate: u32,
    // SID clocks still owed to the output (fractional).
    owed: f64,
    shown: (usize, usize),
    done: bool,
    // As the timeline lays the song out: patterns shown, where it loops
    // back to, and the row before (to tell a new one).
    patterns: usize,
    restart: usize,
    at: Option<usize>,
    prev: Option<(u8, u8)>,
    row: usize,
}

impl<R: Frames + Clone> Player<R> {
    /// A player of the routine `r`, started on its song.
    pub fn new(r: R) -> Player<R> {
        let tl = follow(r.clone(), 50 * 60 * 15);
        let starts = tl.pattern_starts();
        let restart = tl.restart_row.map_or(0, |row| starts.iter().rposition(|&s| s <= row).unwrap_or(0));
        Player {
            r,
            sid: Sid::new(48_000),
            rate: 0,
            owed: 0.0,
            shown: (0, 0),
            done: false,
            patterns: starts.len().max(1),
            restart,
            at: None,
            prev: None,
            row: 0,
        }
    }
}

impl<R: Frames> Driver for Player<R> {
    fn channels(&self) -> usize {
        3
    }

    fn tick(&mut self) {
        self.r.frame();
        for &(reg, v) in self.r.writes() {
            self.sid.write(reg, v);
        }
        let (at, _, _, looped) = self.r.shown(0);
        if looped {
            self.done = true;
        }
        if let Some((p, r)) = at {
            let new = self.prev.is_none_or(|(pp, pr)| r == 0 || p != pp || r != pr.wrapping_add(1));
            self.prev = Some((p, r));
            if new {
                let next = self.at.map_or(0, |a| a + 1);
                self.at = Some(if next >= self.patterns { self.restart } else { next });
                self.row = 0;
            } else {
                self.row += 1;
            }
            self.shown = (self.at.unwrap_or(0), self.row);
        }
    }

    fn tick_seconds(&self) -> f64 {
        FRAME
    }

    fn render(&mut self, rate: u32, frames: usize, out: &mut Bufs) {
        if rate != self.rate {
            self.rate = rate;
            let regs = self.sid.regs;
            self.sid = Sid::new(rate);
            for (i, &v) in regs.iter().enumerate().take(25) {
                self.sid.write(i as u8, v);
            }
        }
        let per = f64::from(PAL_CLOCK) / f64::from(rate.max(1));
        for i in 0..frames {
            self.owed += per;
            let n = self.owed as usize;
            self.owed -= n as f64;
            let mut acc = [0f32; 3];
            for _ in 0..n {
                let o = self.sid.clock();
                for c in 0..3 {
                    acc[c] += o[c];
                }
            }
            for c in 0..3 {
                let v = if n > 0 { acc[c] / n as f32 } else { 0.0 } * LEVEL;
                out.left[c][i] = v;
                out.right[c][i] = v;
            }
        }
    }

    fn position(&self) -> (usize, usize) {
        self.shown
    }

    fn speed(&self) -> (u32, u32) {
        let t = u32::from(self.r.tempo()).max(1) + 1;
        (t, 125)
    }

    fn take_done(&mut self) -> bool {
        std::mem::take(&mut self.done)
    }
}

/// A voice at full volume reaches about 0.25.
const LEVEL: f32 = 2.0;

// ---- The song as the play routine reads it ----

#[derive(Clone)]
struct Packed {
    songtbl: Vec<Vec<u8>>,
    patt: Vec<Vec<u8>>,
    ins: Vec<Ins>,
    hr: Vec<bool>,
    legato: Vec<bool>,
    wavetbl: Vec<u8>,
    notetbl: Vec<u8>,
    pulsetime: Vec<u8>,
    pulsespd: Vec<u8>,
    filttime: Vec<u8>,
    filtspd: Vec<u8>,
    speedleft: Vec<u8>,
    speedright: Vec<u8>,
    defaulttempo: u8,
    /// Every pulse speed a multiple of 16: the relocator then keeps pulse
    /// widths as one byte (bits 4-11), written to both registers.
    simple_pulse: bool,
    /// The instrument the routine starts each channel on: its first, as the
    /// relocator orders them (with hard restart, then without, then legato;
    /// the unused left out).
    first_ins: u8,
}

// A tempo command's parameter as the routine keeps it (a tick fewer).
fn tempo_param(p: u8) -> u8 {
    match p {
        0 | 1 | 2 => p,
        0x80..=0x82 => p,
        _ => p - 1,
    }
}

fn pack_orderlist(o: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    // Where each entry of the file's list went.
    let mut map = vec![0usize; o.len() + 1];
    let mut i = 0;
    let mut group_start = None;
    let mut repeat: Option<u8> = None;
    while i < o.len() {
        let v = o[i];
        if group_start.is_none() {
            group_start = Some(out.len());
        }
        map[i] = group_start.unwrap();
        match v {
            0xff => {
                map[i] = out.len();
                out.push(LOOPSONG);
                let r = usize::from(o.get(i + 1).copied().unwrap_or(0));
                out.push(r as u8);
                // Fixed up below.
                break;
            }
            0xe0..=0xfe => out.push(v),
            // The repeat goes after its pattern, as the routine reads it.
            0xd0..=0xdf => repeat = Some(v),
            _ => {
                out.push(v);
                if let Some(r) = repeat {
                    out.push(r);
                }
                repeat = None;
                group_start = None;
            }
        }
        i += 1;
    }
    if let Some(p) = out.iter().position(|&b| b == LOOPSONG) {
        if let Some(r) = out.get(p + 1).copied() {
            out[p + 1] = map.get(usize::from(r)).copied().unwrap_or(0) as u8;
        }
    } else {
        out.push(LOOPSONG);
        out.push(0);
    }
    out
}

fn pack_pattern(rows: &[[u8; 4]]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut fx: Option<(u8, u8)> = None;
    for r in rows {
        let (note, ins, cmd, param) = (r[0], r[1] & 0x3f, r[2] & 0x0f, r[3]);
        if note == 0xff {
            break;
        }
        let param = if cmd == 0 {
            0
        } else if cmd == 0x0f {
            tempo_param(param)
        } else {
            param
        };
        let fx_new = fx != Some((cmd, param));
        if ins != 0 {
            out.push(ins);
        }
        let note = if (NOTE..=0xbf).contains(&note) { note } else { REST };
        if fx_new {
            fx = Some((cmd, param));
            if note == REST && ins == 0 {
                out.push(FXONLY | cmd);
                if cmd != 0 {
                    out.push(param);
                }
                continue;
            }
            out.push(FX | cmd);
            if cmd != 0 {
                out.push(param);
            }
        }
        out.push(note);
    }
    out.push(ENDPATT);
    out
}

impl Packed {
    fn new(s: &Song) -> Packed {
        let (wl, wr) = &s.tables[0];
        let mut wavetbl = Vec::new();
        let mut notetbl = Vec::new();
        for (i, &l) in wl.iter().enumerate() {
            let r = wr.get(i).copied().unwrap_or(0);
            let (sl, sr) = match l {
                0xff => (0xff, r),
                0xf0..=0xfe => (
                    l,
                    if l == 0xff {
                        r
                    } else if l & 0x0f == 0x0f {
                        tempo_param(r)
                    } else {
                        r
                    },
                ),
                _ => {
                    let sl = match l {
                        0x00..=0x0f => l,
                        0x10..=0xdf => l + 0x10,
                        _ => l - 0xd0,
                    };
                    let sr = match r {
                        0x80 => 0,
                        0x00..=0x7f => 0x80 | r,
                        _ => r & 0x7f,
                    };
                    (sl, sr)
                }
            };
            wavetbl.push(sl);
            notetbl.push(sr);
        }
        let (fl, fr) = &s.tables[2];
        let filttime = fl.iter().map(|&l| if (0x80..0xff).contains(&l) { 0x80 | ((l & 0x70) >> 1) } else { l }).collect();
        let mut ins = s.ins.clone();
        let hr = ins.iter().map(|i| i.gatetimer & 0xc0 == 0).collect();
        let legato = ins.iter().map(|i| i.gatetimer & 0x40 != 0).collect();
        for i in ins.iter_mut() {
            i.gatetimer &= 0x3f;
            // A vibrato delay of 0 is no vibrato.
            if i.vibdelay == 0 {
                i.vibparam = 0;
            } else {
                // Counted down to 0 before it starts: a tick fewer.
                i.vibdelay -= 1;
            }
        }
        ins.resize(64, Ins::default());
        let defaulttempo = match s.ins.get(63).map(|i| i.ad) {
            Some(t) if t >= 2 => t - 1,
            _ => 5,
        };
        let mut used = [false; 64];
        for p in &s.patterns {
            for r in p.iter().take_while(|r| r[0] != 0xff) {
                used[usize::from(r[1] & 0x3f)] = true;
            }
        }
        let class = |i: &Ins| {
            if i.gatetimer & 0x40 != 0 {
                2
            } else if i.gatetimer & 0x80 != 0 {
                1
            } else {
                0
            }
        };
        let first_ins = (1..s.ins.len()).filter(|&i| used[i]).min_by_key(|&i| (class(&s.ins[i]), i)).unwrap_or(1) as u8;
        let (pl, pr) = &s.tables[1];
        let simple_pulse = pl.iter().zip(pr.iter()).all(|(&l, &r)| !(1..0x80).contains(&l) || r & 0x0f == 0);
        let mut pulsespd = pr.clone();
        if simple_pulse {
            for (i, v) in pulsespd.iter_mut().enumerate() {
                let l = pl[i];
                if (0x80..0xff).contains(&l) {
                    *v = (l & 0x0f) | (*v & 0xf0);
                } else if (1..0x80).contains(&l) && *v & 0x80 != 0 {
                    *v -= 1;
                }
            }
        }
        Packed {
            simple_pulse,
            first_ins,
            songtbl: s.orders.iter().map(|o| pack_orderlist(o)).collect(),
            patt: s.patterns.iter().map(|p| pack_pattern(p)).collect(),
            ins,
            hr,
            legato,
            wavetbl,
            notetbl,
            pulsetime: pl.clone(),
            pulsespd,
            filttime,
            filtspd: fr.clone(),
            speedleft: s.tables[3].0.clone(),
            speedright: s.tables[3].1.clone(),
            defaulttempo,
        }
    }
}

fn at(t: &[u8], i: isize) -> u8 {
    if i < 0 {
        0
    } else {
        t.get(i as usize).copied().unwrap_or(0)
    }
}

// ---- The play routine ----

#[derive(Clone, Copy, Default)]
struct Chn {
    songptr: u8,
    trans: u8,
    repeat: u8,
    pattptr: u8,
    packedrest: u8,
    newfx: u8,
    newparam: u8,
    fx: u8,
    param: u8,
    newnote: u8,
    waveptr: u8,
    wave: u8,
    pulseptr: u8,
    pulsetime: u8,
    songnum: u8,
    pattnum: u8,
    tempo: u8,
    counter: u8,
    note: u8,
    instr: u8,
    gate: u8,
    vibtime: u8,
    vibdelay: u8,
    wavetime: u8,
    freqlo: u8,
    freqhi: u8,
    pulselo: u8,
    pulsehi: u8,
    gatetimer: u8,
    lastnote: u8,
}

/// What a channel did in the frame just played (for the screen).
#[derive(Clone, Copy, Default)]
pub struct Shown {
    /// A row began (its tick 0).
    pub row: bool,
    /// A note began: its note (0 is C-0).
    pub note: Option<u8>,
    /// Its gate was cleared from the pattern (a key off).
    pub off: bool,
    /// The row it plays from this frame (fetched before): its pattern and
    /// index, when a row began.
    pub at: Option<(u8, u8)>,
    /// Its order list came round to its restart (this frame's sequencer).
    pub looped: bool,
}

/// The play routine's state, a frame at a time.
#[derive(Clone)]
pub struct Routine {
    p: Packed,
    c: [Chn; 3],
    initsongnum: Option<u8>,
    filtstep: u8,
    filttime: u8,
    filtcutoff: u8,
    filtctrl: u8,
    filttype: u8,
    masterfader: u8,
    funktempo: [u8; 2],
    temp1: u8,
    temp2: u8,
    effectnum: u8,
    // The row each channel fetched last (pattern, index), and the next index.
    fetched: [(u8, u8); 3],
    rowidx: [u8; 3],
    /// The SID's registers as the routine last wrote them.
    pub regs: [u8; 25],
    /// Each register written this frame, in order.
    pub writes: Vec<(u8, u8)>,
    pub shown: [Shown; 3],
    /// A note past B-7 was looked up (the relocated player reads whatever
    /// follows its table there; this reads 0).
    pub out_of_range: bool,
}

// Where the routine goes next (its labels).
enum Go {
    Done,
    LoadRegs,
    LoadRegsWaveOnly,
    WaveExec,
    WaveDone,
    GateTimer(u8),
    GetNewNote,
    WaveNoteAbs(u8),
    Rts,
}

fn adc(a: u8, b: u8, c: bool) -> (u8, bool) {
    let r = u16::from(a) + u16::from(b) + u16::from(c);
    (r as u8, r > 0xff)
}

fn sbc(a: u8, b: u8, c: bool) -> (u8, bool) {
    let r = i16::from(a) - i16::from(b) - i16::from(!c);
    (r as u8, r >= 0)
}

fn freq(n: isize) -> u16 {
    if (0..96).contains(&n) {
        FREQ[n as usize]
    } else {
        0
    }
}

impl Routine {
    pub fn new(song: &Song) -> Routine {
        let mut c = [Chn::default(); 3];
        for (i, ch) in c.iter_mut().enumerate() {
            ch.songnum = i as u8;
            ch.gate = 0xfe;
        }
        let p = Packed::new(song);
        for ch in c.iter_mut() {
            ch.instr = p.first_ins;
        }
        Routine {
            p,
            c,
            initsongnum: Some(0),
            filtstep: 0,
            filttime: 0,
            filtcutoff: 0,
            filtctrl: 0,
            filttype: 0,
            masterfader: 0x0f,
            funktempo: [8, 5],
            temp1: 0,
            temp2: 0,
            effectnum: 0,
            fetched: [(0, 0); 3],
            rowidx: [0; 3],
            regs: [0; 25],
            writes: Vec::new(),
            shown: [Shown::default(); 3],
            out_of_range: false,
        }
    }

    /// mt_init: the subtune to start on the next frame.
    pub fn init(&mut self, subtune: u8) {
        self.initsongnum = Some(subtune.wrapping_mul(3));
    }

    fn sid(&mut self, reg: u8, v: u8) {
        self.regs[usize::from(reg)] = v;
        self.writes.push((reg, v));
    }

    fn ins(&self, x: usize) -> Ins {
        self.p.ins.get(usize::from(self.c[x].instr)).copied().unwrap_or_default()
    }

    /// mt_play: one frame.
    pub fn play(&mut self) {
        self.writes.clear();
        for (sh, ch) in self.shown.iter_mut().zip(self.c.iter()) {
            let _ = ch;
            *sh = Shown::default();
        }
        if let Some(y) = self.initsongnum.take() {
            for ch in self.c.iter_mut() {
                (ch.songptr, ch.trans, ch.repeat, ch.pattptr, ch.packedrest, ch.newfx, ch.newparam) = (0, 0, 0, 0, 0, 0, 0);
                (ch.fx, ch.param, ch.newnote, ch.waveptr, ch.wave, ch.pulseptr, ch.pulsetime) = (0, 0, 0, 0, 0, 0, 0);
            }
            self.sid(0x15, 0);
            self.filtctrl = 0;
            self.filtstep = 0;
            for x in 0..3 {
                self.c[x].songnum = y.wrapping_add(x as u8);
                self.c[x].tempo = self.p.defaulttempo;
                self.c[x].counter = 1;
                self.c[x].instr = self.p.first_ins;
                let w = self.c[x].wave & self.c[x].gate;
                self.sid(x as u8 * 7 + 4, w);
            }
            return;
        }
        self.filter();
        for x in 0..3 {
            self.execchn(x);
        }
    }

    // mt_filtstep to mt_masterfader.
    fn filter(&mut self) {
        let y = self.filtstep;
        let mut cutoff = None;
        if y != 0 {
            if self.filttime != 0 {
                cutoff = self.filtmod(y);
            } else {
                let a = at(&self.p.filttime, y as isize - 1);
                if a != 0 && a < 0x80 {
                    self.filttime = a;
                    cutoff = self.filtmod(y);
                } else {
                    let mut y = y;
                    let mut set = a == 0;
                    if a >= 0x80 {
                        self.filttype = a << 1;
                        self.filtctrl = at(&self.p.filtspd, y as isize - 1);
                        let next = at(&self.p.filttime, y as isize);
                        if next != 0 {
                            self.next_filt_step(y, next);
                        } else {
                            y = y.wrapping_add(1);
                            set = true;
                        }
                    }
                    if set {
                        self.filtcutoff = at(&self.p.filtspd, y as isize - 1);
                        let next = at(&self.p.filttime, y as isize);
                        self.next_filt_step(y, next);
                    }
                }
            }
        }
        let v = cutoff.unwrap_or(self.filtcutoff);
        self.sid(0x16, v);
        let v = self.filtctrl;
        self.sid(0x17, v);
        let v = self.filttype | self.masterfader;
        self.sid(0x18, v);
    }

    // mt_filtmod: the cutoff, stored straight away, while the step lasts.
    fn filtmod(&mut self, y: u8) -> Option<u8> {
        self.filtcutoff = at(&self.p.filtspd, y as isize - 1).wrapping_add(self.filtcutoff);
        self.filttime = self.filttime.wrapping_sub(1);
        if self.filttime != 0 {
            return Some(self.filtcutoff);
        }
        let next = at(&self.p.filttime, y as isize);
        self.next_filt_step(y, next);
        None
    }

    fn next_filt_step(&mut self, y: u8, next: u8) {
        let y1 = y.wrapping_add(1);
        self.filtstep = if next == 0xff { at(&self.p.filtspd, y1 as isize - 1) } else { y1 };
    }

    // mt_execchn.
    fn execchn(&mut self, x: usize) {
        self.c[x].counter = self.c[x].counter.wrapping_sub(1);
        let mut go = if self.c[x].counter == 0 {
            self.shown[x].row = true;
            self.shown[x].at = Some(self.fetched[x]);
            self.tick0(x)
        } else {
            if self.c[x].counter & 0x80 != 0 {
                let mut a = self.c[x].tempo;
                if a < 2 {
                    self.c[x].tempo = a ^ 1;
                    // The compare left carry clear: a tick fewer.
                    a = self.funktempo[usize::from(a)].wrapping_sub(1);
                }
                self.c[x].counter = a;
            }
            Go::WaveExec
        };
        loop {
            go = match go {
                Go::Rts => return,
                Go::WaveExec => self.waveexec(x),
                Go::WaveDone => self.wavedone(x),
                Go::WaveNoteAbs(n) => {
                    self.wavenoteabs(x, n);
                    Go::Done
                }
                Go::Done => Go::GateTimer(self.c[x].counter),
                Go::GateTimer(a) => {
                    if a == self.c[x].gatetimer {
                        Go::GetNewNote
                    } else {
                        self.pulseexec(x, a);
                        Go::LoadRegs
                    }
                }
                Go::GetNewNote => self.getnewnote(x),
                Go::LoadRegs => {
                    let (lo, hi) = (self.c[x].freqlo, self.c[x].freqhi);
                    self.sid(x as u8 * 7, lo);
                    self.sid(x as u8 * 7 + 1, hi);
                    Go::LoadRegsWaveOnly
                }
                Go::LoadRegsWaveOnly => {
                    let w = self.c[x].wave & self.c[x].gate;
                    self.sid(x as u8 * 7 + 4, w);
                    Go::Rts
                }
            };
        }
    }

    // mt_tick0 to mt_nonewnoteinit.
    fn tick0(&mut self, x: usize) -> Go {
        if self.c[x].pattptr == 0 {
            self.sequencer(x);
        }
        let ins = self.ins(x);
        self.c[x].gatetimer = ins.gatetimer;
        let newfx = self.c[x].newfx;
        if self.c[x].newnote != 0 {
            let ch = &mut self.c[x];
            ch.note = ch.newnote.wrapping_sub(NOTE);
            self.shown[x].note = Some(ch.note);
            ch.fx = 0;
            ch.newnote = 0;
            ch.vibdelay = ins.vibdelay;
            ch.param = ins.vibparam;
            if newfx != TONEPORTA {
                let fw = ins.firstwave;
                if fw != 0 {
                    if fw >= 0xfe {
                        ch.gate = fw;
                    } else {
                        ch.wave = fw;
                        ch.gate = 0xff;
                    }
                }
                if ins.pulse != 0 {
                    ch.pulseptr = ins.pulse;
                    ch.pulsetime = 0;
                }
                if ins.filt != 0 {
                    self.filtstep = ins.filt;
                    self.filttime = 0;
                }
                self.c[x].waveptr = ins.wave;
                self.sid(x as u8 * 7 + 6, ins.sr);
                self.sid(x as u8 * 7 + 5, ins.ad);
                let a = self.c[x].newparam;
                self.tick0fx(x, newfx, a);
                return Go::LoadRegsWaveOnly;
            }
        }
        let a = self.c[x].newparam;
        self.tick0fx(x, newfx, a);
        Go::WaveExec
    }

    // mt_sequencer.
    fn sequencer(&mut self, x: usize) {
        let list = self.p.songtbl.get(usize::from(self.c[x].songnum)).cloned().unwrap_or_default();
        let get = |y: u8| list.get(usize::from(y)).copied().unwrap_or(0);
        let mut y = self.c[x].songptr;
        let mut a = get(y);
        if a >= LOOPSONG {
            self.shown[x].looped = true;
            y = get(y.wrapping_add(1));
            a = get(y);
        }
        if a >= TRANSDOWN {
            self.c[x].trans = a.wrapping_sub(TRANS);
            y = y.wrapping_add(1);
            a = get(y);
        }
        if a >= REPEAT {
            let k = a - REPEAT;
            self.c[x].repeat = self.c[x].repeat.wrapping_add(1);
            if k != self.c[x].repeat {
                return;
            }
            self.c[x].repeat = 0;
        } else {
            self.c[x].pattnum = a;
        }
        self.c[x].songptr = y.wrapping_add(1);
    }

    // The tick-0 commands (mt_tick0_0 to mt_tick0_f), `a` the parameter.
    fn tick0fx(&mut self, x: usize, fx: u8, a: u8) {
        match fx & 0x0f {
            0 => {
                let v = self.ins(x).vibparam;
                self.tick0_34(x, v);
            }
            1 | 2 => {
                self.c[x].vibtime = 0;
                self.tick0_34(x, a);
            }
            3 | 4 => self.tick0_34(x, a),
            5 => self.sid(x as u8 * 7 + 5, a),
            6 => self.sid(x as u8 * 7 + 6, a),
            7 => self.c[x].wave = a,
            8 => {
                self.c[x].waveptr = a;
                self.c[x].wavetime = 0;
            }
            9 => {
                self.c[x].pulseptr = a;
                self.c[x].pulsetime = 0;
            }
            0xa => {
                self.filttime = 0;
                self.filtstep = a;
            }
            0xb => {
                self.filtctrl = a;
                if a == 0 {
                    self.filtstep = 0;
                }
            }
            0xc => self.filtcutoff = a,
            0xd => {
                // From $10 a timing mark, kept in the player's memory only.
                if a < 0x10 {
                    self.masterfader = a;
                }
            }
            0xe => {
                let y = a as isize;
                self.funktempo = [at(&self.p.speedleft, y - 1), at(&self.p.speedright, y - 1)];
                self.set_global_tempo(0);
            }
            _ => {
                if a & 0x80 != 0 {
                    self.c[x].tempo = a & 0x7f;
                } else {
                    self.set_global_tempo(a);
                }
            }
        }
    }

    fn set_global_tempo(&mut self, a: u8) {
        for ch in self.c.iter_mut() {
            ch.tempo = a;
        }
    }

    fn tick0_34(&mut self, x: usize, a: u8) {
        self.c[x].param = a;
        self.c[x].fx = self.c[x].newfx;
    }

    // mt_waveexec.
    fn waveexec(&mut self, x: usize) -> Go {
        let y = self.c[x].waveptr;
        if y == 0 {
            return Go::WaveDone;
        }
        let a = at(&self.p.wavetbl, y as isize - 1);
        if a < 0x10 {
            if a != self.c[x].wavetime {
                self.c[x].wavetime = self.c[x].wavetime.wrapping_add(1);
                return Go::WaveDone;
            }
        } else if a - 0x10 < 0xe0 {
            self.c[x].wave = a - 0x10;
        }
        let next = at(&self.p.wavetbl, y as isize);
        let y = y.wrapping_add(1);
        self.c[x].waveptr = if next == 0xff { at(&self.p.notetbl, y as isize - 1) } else { y };
        self.c[x].wavetime = 0;
        let cur = at(&self.p.wavetbl, y as isize - 2);
        if cur >= 0xe0 {
            return self.execwavecmd(x, cur & 0x0f, at(&self.p.notetbl, y as isize - 2));
        }
        let n = at(&self.p.notetbl, y as isize - 2);
        if n == 0 {
            return Go::WaveDone;
        }
        // mt_wavefreq (carry clear from the compare before).
        Go::WaveNoteAbs(if n & 0x80 != 0 { n.wrapping_add(self.c[x].note) & 0x7f } else { n })
    }

    // mt_wavenoteabs.
    fn wavenoteabs(&mut self, x: usize, n: u8) {
        if n >= 96 {
            self.out_of_range = true;
        }
        self.c[x].lastnote = n;
        self.c[x].vibtime = 0;
        let f = freq(n as isize);
        self.c[x].freqlo = f as u8;
        self.c[x].freqhi = (f >> 8) as u8;
    }

    // mt_wavedone: the continuous effects.
    fn wavedone(&mut self, x: usize) -> Go {
        if self.c[x].counter == 0 {
            return Go::GateTimer(0);
        }
        let fx = self.c[x].fx;
        self.effectnum = fx;
        let y = self.c[x].param;
        self.effect(x, fx, y)
    }

    // mt_execwavecmd.
    fn execwavecmd(&mut self, x: usize, c: u8, param: u8) -> Go {
        if c < 5 {
            self.effectnum = c;
            return self.effect(x, c, param);
        }
        self.tick0fx(x, c, param);
        Go::Done
    }

    // mt_setspeedparam, then the effect (mt_effect_*): whether the speed
    // was zero (the routine's zero flag as it jumps).
    fn effect(&mut self, x: usize, which: u8, y: u8) -> Go {
        let left = at(&self.p.speedleft, y as isize - 1);
        let right = at(&self.p.speedright, y as isize - 1);
        let zero;
        if left & 0x80 == 0 {
            self.temp2 = left;
            self.temp1 = right;
            zero = right == 0;
        } else {
            let n = self.c[x].lastnote as isize;
            if n + 1 >= 96 {
                self.out_of_range = true;
            }
            let (f1, f0) = (freq(n + 1), freq(n));
            let (lo, c) = sbc(f1 as u8, f0 as u8, true);
            let (mut hi, _) = sbc((f1 >> 8) as u8, (f0 >> 8) as u8, c);
            let mut lo = lo;
            for _ in 0..right {
                let b = hi & 1;
                hi >>= 1;
                lo = (lo >> 1) | (b << 7);
            }
            self.temp1 = lo;
            self.temp2 = hi;
            zero = y == 0;
        }
        match which {
            0 => {
                if zero {
                    return Go::Done;
                }
                if self.c[x].vibdelay != 0 {
                    self.c[x].vibdelay -= 1;
                    return Go::Done;
                }
                self.vibrato(x, y)
            }
            1 | 2 => self.porta(x),
            3 => {
                if y == 0 {
                    let n = self.c[x].note;
                    return Go::WaveNoteAbs(n);
                }
                self.porta(x)
            }
            _ => self.vibrato(x, y),
        }
    }

    // mt_effect_4.
    fn vibrato(&mut self, x: usize, y: u8) -> Go {
        let left = at(&self.p.speedleft, y as isize - 1);
        if left & 0x80 == 0 {
            self.temp2 = 0;
        }
        let speed = left & 0x7f;
        let mut a = self.c[x].vibtime;
        if a & 0x80 == 0 && a > speed {
            a ^= 0xff;
        }
        a = a.wrapping_add(2);
        self.c[x].vibtime = a;
        if a & 1 == 0 {
            self.freqadd(x, false)
        } else {
            self.freqsub(x, true)
        }
    }

    // mt_effect_12 and the toneportamento.
    fn porta(&mut self, x: usize) -> Go {
        match self.effectnum {
            0 | 1 => return self.freqadd(x, false),
            2 => return self.freqsub(x, true),
            _ => {}
        }
        let n = self.c[x].note as isize;
        if n >= 96 {
            self.out_of_range = true;
        }
        let t = freq(n);
        let (lo, c) = sbc(self.c[x].freqlo, t as u8, true);
        let (hi, c) = sbc(self.c[x].freqhi, (t >> 8) as u8, c);
        if c {
            let (_, c) = sbc(lo, self.temp1, c);
            let (r, c) = sbc(hi, self.temp2, c);
            if r & 0x80 != 0 {
                let n = self.c[x].note;
                return Go::WaveNoteAbs(n);
            }
            self.freqsub(x, c)
        } else {
            let (_, c) = adc(lo, self.temp1, c);
            let (r, c) = adc(hi, self.temp2, c);
            if r & 0x80 == 0 {
                let n = self.c[x].note;
                return Go::WaveNoteAbs(n);
            }
            self.freqadd(x, c)
        }
    }

    fn freqadd(&mut self, x: usize, c: bool) -> Go {
        let (lo, c) = adc(self.c[x].freqlo, self.temp1, c);
        self.c[x].freqlo = lo;
        let (hi, _) = adc(self.c[x].freqhi, self.temp2, c);
        self.c[x].freqhi = hi;
        Go::Done
    }

    fn freqsub(&mut self, x: usize, c: bool) -> Go {
        let (lo, c) = sbc(self.c[x].freqlo, self.temp1, c);
        self.c[x].freqlo = lo;
        let (hi, _) = sbc(self.c[x].freqhi, self.temp2, c);
        self.c[x].freqhi = hi;
        Go::Done
    }

    // mt_pulseexec, `a` the counter (0 on tick 0).
    fn pulseexec(&mut self, x: usize, a: u8) {
        let y = self.c[x].pulseptr;
        if y == 0 || a | self.c[x].pattptr == 0 {
            return;
        }
        let simple = self.p.simple_pulse;
        let mut step = true;
        if self.c[x].pulsetime == 0 {
            let l = at(&self.p.pulsetime, y as isize - 1);
            if l < 0x80 {
                self.c[x].pulsetime = l;
            } else {
                if !simple {
                    self.c[x].pulsehi = l;
                }
                self.c[x].pulselo = at(&self.p.pulsespd, y as isize - 1);
                step = false;
            }
        }
        if step {
            let s = at(&self.p.pulsespd, y as isize - 1);
            let ch = &mut self.c[x];
            if simple {
                let (v, c) = adc(ch.pulselo, s, false);
                ch.pulselo = v.wrapping_add(u8::from(c));
            } else {
                if s & 0x80 != 0 {
                    ch.pulsehi = ch.pulsehi.wrapping_sub(1);
                }
                let (lo, c) = adc(ch.pulselo, s, false);
                ch.pulselo = lo;
                if c {
                    ch.pulsehi = ch.pulsehi.wrapping_add(1);
                }
            }
            ch.pulsetime = ch.pulsetime.wrapping_sub(1);
            if ch.pulsetime != 0 {
                self.write_pulse(x);
                return;
            }
        }
        let next = at(&self.p.pulsetime, y as isize);
        let y1 = y.wrapping_add(1);
        self.c[x].pulseptr = if next == 0xff { at(&self.p.pulsespd, y1 as isize - 1) } else { y1 };
        self.write_pulse(x);
    }

    fn write_pulse(&mut self, x: usize) {
        let lo = self.c[x].pulselo;
        let hi = if self.p.simple_pulse { lo } else { self.c[x].pulsehi };
        self.sid(x as u8 * 7 + 2, lo);
        self.sid(x as u8 * 7 + 3, hi);
    }

    // mt_getnewnote.
    fn getnewnote(&mut self, x: usize) -> Go {
        let pn = usize::from(self.c[x].pattnum);
        let byte = |s: &Self, y: u8| s.p.patt.get(pn).and_then(|p| p.get(usize::from(y))).copied().unwrap_or(0);
        let mut y = self.c[x].pattptr;
        if y == 0 && self.c[x].packedrest == 0 {
            self.rowidx[x] = 0;
        }
        self.fetched[x] = (self.c[x].pattnum, self.rowidx[x]);
        self.rowidx[x] = self.rowidx[x].wrapping_add(1);
        let mut a = byte(self, y);
        if a < FX {
            self.c[x].instr = a;
            y = y.wrapping_add(1);
            a = byte(self, y);
            if a >= NOTE {
                return self.note(x, y, a);
            }
        } else if a >= FIRSTPACKEDREST {
            let mut v = self.c[x].packedrest;
            if v == 0 {
                v = a;
            }
            v = v.wrapping_add(1);
            self.c[x].packedrest = v;
            return if v == 0 { self.rest(x, y) } else { Go::LoadRegs };
        } else if a >= NOTE {
            return self.note(x, y, a);
        }
        let only = a >= FXONLY;
        let f = a & 0x0f;
        self.c[x].newfx = f;
        if f != 0 {
            y = y.wrapping_add(1);
            self.c[x].newparam = byte(self, y);
        }
        if only {
            return self.rest(x, y);
        }
        y = y.wrapping_add(1);
        let a = byte(self, y);
        self.note(x, y, a)
    }

    // mt_note.
    fn note(&mut self, x: usize, y: u8, a: u8) -> Go {
        if a > REST {
            self.c[x].gate = a | 0xf0;
            if self.c[x].gate == 0xfe {
                self.shown[x].off = true;
            }
            return self.rest(x, y);
        }
        if a == REST {
            return self.rest(x, y);
        }
        self.c[x].newnote = a.wrapping_add(self.c[x].trans);
        if self.c[x].newfx == TONEPORTA {
            return self.rest(x, y);
        }
        let i = usize::from(self.c[x].instr);
        if self.p.legato.get(i).copied().unwrap_or(false) {
            return self.rest(x, y);
        }
        if self.p.hr.get(i).copied().unwrap_or(false) {
            self.sid(x as u8 * 7 + 6, SRPARAM);
            self.sid(x as u8 * 7 + 5, ADPARAM);
        }
        self.c[x].gate = 0xfe;
        self.rest(x, y)
    }

    // mt_rest: on to the next row, or the pattern's end.
    fn rest(&mut self, x: usize, y: u8) -> Go {
        let y = y.wrapping_add(1);
        let pn = usize::from(self.c[x].pattnum);
        let next = self.p.patt.get(pn).and_then(|p| p.get(usize::from(y))).copied().unwrap_or(0);
        self.c[x].pattptr = if next == ENDPATT { 0 } else { y };
        Go::LoadRegs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The SID's registers each frame as GoatTracker 2's relocator packs a
    /// song and its play routine plays it (on a 6502, as a black box):
    /// GT_DIR the songs, GT_REF the logs (<name>.log, a line a frame).
    #[test]
    #[ignore]
    fn the_same_as_the_relocated_player() {
        let (Ok(dir), Ok(rf)) = (std::env::var("GT_DIR"), std::env::var("GT_REF")) else { return };
        let only = std::env::var("GT_ONLY").ok();
        let show = std::env::var("GT_SHOW").is_ok();
        let mut paths: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).collect();
        paths.sort();
        let (mut same, mut total, mut oor) = (0, 0, 0);
        for path in paths {
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            if only.as_ref().is_some_and(|o| !name.contains(o.as_str())) {
                continue;
            }
            let Ok(log) = std::fs::read_to_string(format!("{rf}/{name}.log")) else { continue };
            total += 1;
            let Some(song) = Song::parse(&std::fs::read(&path).unwrap()) else {
                eprintln!("{name}: not read");
                continue;
            };
            let mut r = Routine::new(&song);
            r.init(0);
            let mut bad = None;
            for (f, line) in log.lines().enumerate() {
                r.play();
                let ours: String = r.regs.iter().map(|b| format!("{b:02x}")).collect();
                if ours != line {
                    bad = Some((f, ours, line.to_string()));
                    break;
                }
            }
            match bad {
                None => same += 1,
                Some(_) if r.out_of_range => {
                    oor += 1;
                    eprintln!("{name}: past the note table");
                }
                Some((f, ours, want)) => {
                    eprintln!("{name}: frame {f}");
                    if show {
                        eprintln!("  ours {ours}\n  want {want}");
                        let mut r = Routine::new(&song);
                        r.init(0);
                        let lines: Vec<&str> = log.lines().collect();
                        for g in 0..f + 4 {
                            r.play();
                            if g + 6 >= f {
                                let o: String = r.regs.iter().map(|b| format!("{b:02x}")).collect();
                                eprintln!("  {g:5} {o} {}", lines.get(g).unwrap_or(&""));
                                if let Ok(c) = std::env::var("GT_CH") {
                                    let ch = r.c[c.parse::<usize>().unwrap()];
                                    eprintln!(
                                        "        row {:?} note {:02x} last {:02x} fx {} param {:02x} newfx {} newparam {:02x} waveptr {:02x} cnt {} tempo {} pattnum {} pattptr {}",
                                        r.fetched[c.parse::<usize>().unwrap()],
                                        ch.note,
                                        ch.lastnote,
                                        ch.fx,
                                        ch.param,
                                        ch.newfx,
                                        ch.newparam,
                                        ch.waveptr,
                                        ch.counter,
                                        ch.tempo,
                                        ch.pattnum,
                                        ch.pattptr
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        eprintln!("{same}/{total} the same, {oor} more until a note past the table");
        assert_eq!(same + oor, total);
    }
}
