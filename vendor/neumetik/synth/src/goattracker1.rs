// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.
//
// GoatTracker 1.5's standard play routine (player1.s, Lasse Öörni),
// followed here from its source, which its author gave away apart from the
// editor's GPL: "This playroutine source code does not fall under the GPL
// license! Use it freely for any purpose, commercial or noncommercial." The
// song's file is read from GoatTracker's published description of it (its
// readme's "File/data formats description").

//! GoatTracker 1's songs (.sng, "GTS!"): C64 music of three SID channels,
//! each an order list of patterns of notes and commands (arpeggio,
//! portamento, vibrato, filter, sustain/release, tempo), on 31 instruments
//! with their own wave tables, pulse sweeps between limits and a filter
//! table of steps; up to 32 subtunes. Played by a port of the editor's
//! standard play routine, a frame (1/50 s) at a time, on the SID. The song
//! is first laid out as the routine reads it (a page each for instruments,
//! waves, notes, filters), as the editor's packer lays it out.

use std::sync::Arc;

use crate::goattracker::{follow, Frames, Player, Timeline};
use crate::replay::{Driver, Patch, PatchBytes, Tune};

/// The routine's frequency table, high bytes then low (C-0 to B-7).
const FREQ_HI: [u8; 96] = [
    0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x03, 0x03, 0x03, 0x03, 0x03, 0x04, 0x04, 0x04, 0x04, 0x05, 0x05, 0x05, 0x06, 0x06,
    0x06, 0x07, 0x07, 0x08, 0x08, 0x09, 0x09, 0x0a, 0x0a, 0x0b, 0x0c, 0x0d, 0x0d, 0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x17, 0x18, 0x1a, 0x1b, 0x1d, 0x1f, 0x20, 0x22, 0x24, 0x27, 0x29,
    0x2b, 0x2e, 0x31, 0x34, 0x37, 0x3a, 0x3e, 0x41, 0x45, 0x49, 0x4e, 0x52, 0x57, 0x5c, 0x62, 0x68, 0x6e, 0x75, 0x7c, 0x83, 0x8b, 0x93, 0x9c, 0xa5, 0xaf, 0xb9, 0xc4, 0xd0, 0xdd, 0xea, 0xf8, 0xff,
];
const FREQ_LO: [u8; 96] = [
    0x17, 0x27, 0x39, 0x4b, 0x5f, 0x74, 0x8a, 0xa1, 0xba, 0xd4, 0xf0, 0x0e, 0x2d, 0x4e, 0x71, 0x96, 0xbe, 0xe8, 0x14, 0x43, 0x74, 0xa9, 0xe1, 0x1c, 0x5a, 0x9c, 0xe2, 0x2d, 0x7c, 0xcf, 0x28, 0x85,
    0xe8, 0x52, 0xc1, 0x37, 0xb4, 0x39, 0xc5, 0x5a, 0xf7, 0x9e, 0x4f, 0x0a, 0xd1, 0xa3, 0x82, 0x6e, 0x68, 0x71, 0x8a, 0xb3, 0xee, 0x3c, 0x9e, 0x15, 0xa2, 0x46, 0x04, 0xdc, 0xd0, 0xe2, 0x14, 0x67,
    0xdd, 0x79, 0x3c, 0x29, 0x44, 0x8d, 0x08, 0xb8, 0xa1, 0xc5, 0x28, 0xcd, 0xba, 0xf1, 0x78, 0x53, 0x87, 0x1a, 0x10, 0x71, 0x42, 0x89, 0x4f, 0x9b, 0x74, 0xe2, 0xf0, 0xa6, 0x0e, 0x33, 0x20, 0xff,
];

// Past the low table the routine reads what follows it (nothing, as this
// lays it out); past the high table, the low table.
fn freq_lo(i: u8) -> u8 {
    FREQ_LO.get(usize::from(i)).copied().unwrap_or(0)
}

fn freq_hi(i: u8) -> u8 {
    let i = usize::from(i);
    if i < 96 {
        FREQ_HI[i]
    } else {
        FREQ_LO.get(i - 96).copied().unwrap_or(0)
    }
}

const KEYOFF: u8 = 0x5e;
const NOCMD: u8 = 0x60;
const FIRSTPACKEDREST: u8 = 0xc0;
const ENDPATT: u8 = 0xff;
const REPEAT: u8 = 0xd0;
const TRANSDOWN: u8 = 0xe0;
const LOOPSONG: u8 = 0xff;

/// The editor's defaults the packer puts in the routine: the hard restart
/// two frames before a row, its attack/decay and sustain/release ("0F00").
pub const GATETIMER: u8 = 2;
pub const HR_AD: u8 = 0x0f;
pub const HR_SR: u8 = 0x00;

#[derive(Clone, Copy, Default)]
struct Ins {
    ad: u8,
    sr: u8,
    pulse: u8,
    pulsespd: u8,
    pulselow: u8,
    pulsehigh: u8,
    filter: u8,
}

/// A GoatTracker 1 song.
#[derive(Clone)]
pub struct Song {
    file: Arc<Vec<u8>>,
    pub name: String,
    pub author: String,
    pub copyright: String,
    subtunes: usize,
    pub orders: Vec<Vec<u8>>,
    ins: Vec<Ins>,
    waves: Vec<Vec<(u8, u8)>>,
    pub ins_names: Vec<String>,
    /// The patterns' rows: note, instrument and command, data.
    pub patterns: Vec<Vec<[u8; 3]>>,
    filters: [u8; 256],
}

pub fn probe(d: &[u8]) -> bool {
    d.starts_with(b"GTS!") && Song::parse(d).is_some()
}

fn text(b: &[u8]) -> String {
    let b = &b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())];
    b.iter().map(|&c| if (0x20..0x7f).contains(&c) { c as char } else { ' ' }).collect::<String>().trim().to_string()
}

impl Song {
    pub fn parse(d: &[u8]) -> Option<Song> {
        if !d.starts_with(b"GTS!") || d.len() < 101 {
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
        let mut ins = vec![Ins::default()];
        let mut waves = vec![Vec::new()];
        let mut ins_names = Vec::new();
        for _ in 0..31 {
            let b = take(24)?;
            ins.push(Ins {
                ad: b[0],
                sr: b[1],
                pulse: b[2],
                pulsespd: b[3],
                pulselow: b[4],
                pulsehigh: b[5],
                filter: b[6],
            });
            ins_names.push(text(&b[8..24]));
            let n = usize::from(b[7]);
            let w = take(n)?;
            waves.push(w.chunks_exact(2).map(|p| (p[0], p[1])).collect());
        }
        let n = usize::from(take(1)?[0]);
        if n == 0 {
            return None;
        }
        let mut patterns = Vec::new();
        for _ in 0..n {
            let m = usize::from(take(1)?[0]);
            let b = take(m)?;
            patterns.push(b.chunks_exact(3).map(|r| [r[0], r[1], r[2]]).collect());
        }
        // Songs before 1.4 have no filter table.
        let mut filters = [0u8; 256];
        if let Some(f) = d.get(at..at + 256) {
            filters.copy_from_slice(f);
        }
        Some(Song {
            file: Arc::new(d.to_vec()),
            name: text(&d[4..36]),
            author: text(&d[36..68]),
            copyright: text(&d[68..100]),
            subtunes,
            orders,
            ins,
            waves,
            ins_names,
            patterns,
            filters,
        })
    }

    pub fn subtunes(&self) -> usize {
        self.subtunes
    }

    /// Its notes laid out by channel 1's rows (as GoatTracker 2's are).
    pub fn timeline(&self, subtune: usize, max_frames: usize) -> Timeline {
        let mut r = Routine::new(self);
        r.init(subtune as u8);
        follow(r, max_frames)
    }
}

impl Tune for Song {
    fn format(&self) -> &'static str {
        "gt1"
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

    /// Each instrument with a wave table: its settings, its wave table and
    /// its filter's four bytes.
    fn patches(&self) -> Vec<Patch> {
        let mut out = Vec::new();
        for (k, i) in self.ins.iter().enumerate().skip(1) {
            let Some(w) = self.waves.get(k).filter(|w| !w.is_empty()) else { continue };
            // A new instrument (no envelope, pulse or filter) is the editor's blank.
            if [i.ad, i.sr, i.pulse, i.pulsespd, i.pulselow, i.pulsehigh, i.filter].iter().all(|&v| v == 0) {
                continue;
            }
            let mut b = PatchBytes::default();
            for v in [i.ad, i.sr, i.pulse, i.pulsespd, i.pulselow, i.pulsehigh, i.filter] {
                b.u8(v);
            }
            b.u16(w.len() as u16);
            for &(l, r) in w {
                b.u8(l).u8(r);
            }
            if i.filter != 0 {
                let at = usize::from(i.filter) * 4;
                b.bytes(self.filters.get(at..at + 4).unwrap_or_default());
            }
            out.push(b.patch(k, self.ins_names.get(k - 1).map_or("", |n| n.as_str())));
        }
        out
    }
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

// ---- The song as the routine reads it ----

fn swap(v: u8) -> u8 {
    v.rotate_left(4)
}

// A tempo command's parameter as the packer keeps it (a tick fewer).
fn tempo_param(p: u8) -> u8 {
    match p {
        0 => 0,
        0x01..=0x7f => p - 1,
        0xef..=0xff => p,
        0x80 => 0x80,
        _ => p - 1,
    }
}

/// The routine's data: its pages (instruments, waves, notes, filters), its
/// order lists and packed patterns.
#[derive(Clone)]
pub struct Image {
    pub ins: [u8; 256],
    pub wavetbl: [u8; 256],
    pub notetbl: [u8; 256],
    pub filttbl: [u8; 256],
    pub songs: Vec<Vec<u8>>,
    pub patts: Vec<Vec<u8>>,
}

impl Image {
    pub fn new(s: &Song) -> Image {
        let mut im = Image {
            ins: [0; 256],
            wavetbl: [0; 256],
            notetbl: [0; 256],
            filttbl: s.filters,
            songs: s.orders.clone(),
            patts: Vec::new(),
        };
        // Filter numbers as the offsets of their four bytes; filter 0's
        // last two the funktempo's tempos, a tick fewer.
        for f in 0..64 {
            if f == 0 {
                for k in 2..4 {
                    let v = im.filttbl[k];
                    im.filttbl[k] = v.saturating_sub(1);
                }
            } else {
                im.filttbl[f * 4 + 3] = im.filttbl[f * 4 + 3].wrapping_mul(4);
            }
        }
        let mut next = 1usize;
        for i in 1..32 {
            let ins = s.ins[i];
            let w = &s.waves[i];
            let mut start = 0u8;
            if !w.is_empty() && next + w.len() < 256 {
                start = next as u8;
                for (k, &(wv, n)) in w.iter().enumerate() {
                    im.wavetbl[next + k] = wv;
                    im.notetbl[next + k] = n;
                }
                next += w.len();
                if w.last().is_none_or(|e| e.0 != 0xff) && next < 255 {
                    im.wavetbl[next] = 0xff;
                    im.notetbl[next] = 0;
                    next += 1;
                }
            }
            let b = &mut im.ins[i * 8..i * 8 + 8];
            b[0] = ins.ad;
            b[1] = ins.sr;
            b[2] = swap(ins.pulse & 0xfe);
            // The pulse width's low bit (no hard restart) as the speed's.
            b[3] = (ins.pulsespd & 0xfe) | (ins.pulse & 1);
            b[4] = swap(ins.pulselow);
            b[5] = swap(ins.pulsehigh);
            b[6] = ins.filter.wrapping_mul(4);
            b[7] = start;
        }
        for p in &s.patterns {
            let mut out = Vec::new();
            for r in p.iter().take_while(|r| r[0] != ENDPATT) {
                let (note, ic) = (r[0], r[1]);
                let param = match ic & 7 {
                    5 => r[2].wrapping_mul(4),
                    7 => tempo_param(r[2]),
                    _ => r[2],
                };
                if ic == 0 && param == 0 {
                    out.push(note.wrapping_add(NOCMD));
                } else {
                    out.extend_from_slice(&[note, ic, param]);
                }
            }
            out.push(ENDPATT);
            im.patts.push(out);
        }
        im
    }
}

// ---- The play routine ----

#[derive(Clone, Copy, Default)]
struct Chn {
    wave: u8,
    waveptr: u8,
    packrest: u8,
    repeat: u8,
    trans: u8,
    songptr: u8,
    pattptr: u8,
    freqlo: u8,
    freqhi: u8,
    pulse: u8,
    pulsedir: u8,
    gate: u8,
    newfx: u8,
    newfxparam: u8,
    fx: u8,
    fxparam: u8,
    note: u8,
    newnote: u8,
    instnum: u8,
    arpcount: u8,
    songnum: u8,
    pattnum: u8,
    tick: u8,
    tempo: u8,
}

/// What a channel did in the frame just played (for the screen).
#[derive(Clone, Copy, Default)]
pub struct Shown {
    /// The row it plays from this frame (its pattern and index).
    pub at: Option<(u8, u8)>,
    pub note: Option<u8>,
    pub off: bool,
    pub looped: bool,
}

#[derive(Clone)]
pub struct Routine {
    pub im: Image,
    c: [Chn; 3],
    chnloop: u8,
    filttime: u8,
    filtstep: u8,
    filtcutoffadd: u8,
    d416: u8,
    d417: u8,
    d418: u8,
    volume: u8,
    temp1: u8,
    temp2: u8,
    tick_templo: u8,
    tick_temphi: u8,
    notenum: u8,
    pub timecounter: u8,
    pub regs: [u8; 25],
    pub writes: Vec<(u8, u8)>,
    pub shown: [Shown; 3],
    fetched: [(u8, u8); 3],
    rowidx: [u8; 3],
}

enum Go {
    Tick0Done,
    WaveExec(u8),
    PulseExec,
    Sequencer,
    GetNewNotes,
    Rest,
    LoadRegs,
    NextChn,
    ArpFreq(u8),
    ArpFreqResetVib(u8),
    FreqAdd(bool),
    FreqSub(bool),
    Arpeggio(u8),
}

fn adc(a: u8, b: u8, c: bool) -> (u8, bool) {
    let r = u16::from(a) + u16::from(b) + u16::from(c);
    (r as u8, r > 0xff)
}

fn sbc(a: u8, b: u8, c: bool) -> (u8, bool) {
    let r = i16::from(a) - i16::from(b) - i16::from(!c);
    (r as u8, r >= 0)
}

impl Routine {
    pub fn new(song: &Song) -> Routine {
        let mut c = [Chn::default(); 3];
        for ch in c.iter_mut() {
            ch.instnum = 8;
        }
        Routine {
            im: Image::new(song),
            c,
            chnloop: 0,
            filttime: 0,
            filtstep: 0,
            filtcutoffadd: 0,
            d416: 0,
            d417: 0,
            d418: 0x0f,
            volume: 0xff,
            temp1: 0,
            temp2: 0,
            tick_templo: 0,
            tick_temphi: 0,
            notenum: 0,
            timecounter: 0,
            regs: [0; 25],
            writes: Vec::new(),
            shown: [Shown::default(); 3],
            fetched: [(0, 0); 3],
            rowidx: [0; 3],
        }
    }

    /// init: the subtune to start on the next frame.
    pub fn init(&mut self, subtune: u8) {
        self.chnloop = subtune.wrapping_mul(3);
    }

    fn sid(&mut self, reg: u8, v: u8) {
        self.regs[usize::from(reg)] = v;
        self.writes.push((reg, v));
    }

    fn ins(&self, x: usize, k: usize) -> u8 {
        self.im.ins[(usize::from(self.c[x].instnum) + k) & 0xff]
    }

    fn filt(&self, i: usize) -> u8 {
        self.im.filttbl[i & 0xff]
    }

    /// play: one frame.
    pub fn play(&mut self) {
        self.writes.clear();
        self.shown = [Shown::default(); 3];
        if self.filttime != 0 {
            self.filttime -= 1;
            self.d416 = self.d416.wrapping_add(self.filtcutoffadd);
        } else if self.filtstep != 0 {
            let a = self.filtstep;
            self.setfiltersub(a);
        }
        let (a, b, c) = (self.d416, self.d417, self.d418 & self.volume);
        self.sid(0x16, a);
        self.sid(0x17, b);
        self.sid(0x18, c);
        let mut x = 0usize;
        loop {
            let go = if self.chnloop & 0x80 == 0 {
                self.c[x].songnum = self.chnloop;
                self.chnloop = self.chnloop.wrapping_add(1);
                if x == 0 {
                    for ch in self.c.iter_mut() {
                        (ch.wave, ch.waveptr, ch.packrest, ch.repeat, ch.trans, ch.songptr, ch.pattptr) = (0, 0, 0, 0, 0, 0, 0);
                    }
                    self.sid(0x15, 0);
                    self.setfiltersub(0);
                }
                self.c[x].tempo = 5;
                self.c[x].tick = GATETIMER + 2;
                self.c[x].pattptr = ENDPATT;
                self.c[x].newnote = ENDPATT;
                Go::LoadRegs
            } else {
                self.noinit(x)
            };
            self.run(x, go);
            if x >= 2 {
                break;
            }
            x += 1;
        }
        self.chnloop = 0xff;
    }

    fn run(&mut self, x: usize, mut go: Go) {
        loop {
            go = match go {
                Go::NextChn => return,
                Go::Tick0Done => self.tick0done(x),
                Go::WaveExec(y) => self.waveexec(x, y),
                Go::PulseExec => self.pulseexec(x),
                Go::Sequencer => self.sequencer(x),
                Go::GetNewNotes => self.getnewnotes(x),
                Go::Rest | Go::LoadRegs => {
                    let (lo, hi, w) = (self.c[x].freqlo, self.c[x].freqhi, self.c[x].wave & self.c[x].gate);
                    let b = x as u8 * 7;
                    self.sid(b, lo);
                    self.sid(b + 1, hi);
                    self.sid(b + 4, w);
                    Go::NextChn
                }
                Go::ArpFreqResetVib(y) => {
                    self.c[x].arpcount = 0;
                    Go::ArpFreq(y)
                }
                Go::ArpFreq(y) => {
                    self.c[x].freqlo = freq_lo(y);
                    self.c[x].freqhi = freq_hi(y);
                    Go::PulseExec
                }
                Go::FreqAdd(c) => {
                    let (lo, c) = adc(self.c[x].freqlo, self.temp1, c);
                    self.c[x].freqlo = lo;
                    self.c[x].freqhi = adc(self.c[x].freqhi, self.temp2, c).0;
                    Go::PulseExec
                }
                Go::FreqSub(c) => {
                    let (lo, c) = sbc(self.c[x].freqlo, self.temp1, c);
                    self.c[x].freqlo = lo;
                    self.c[x].freqhi = sbc(self.c[x].freqhi, self.temp2, c).0;
                    Go::PulseExec
                }
                Go::Arpeggio(a) => self.arpeggio(x, a),
            };
        }
    }

    // mt_setfiltersub.
    fn setfiltersub(&mut self, y: u8) {
        let i = usize::from(y);
        let a;
        if self.filt(i) == 0 {
            self.filtcutoffadd = self.filt(i + 2);
            a = self.filt(i + 1);
        } else {
            self.d417 = self.filt(i);
            self.d418 = self.filt(i + 1);
            if self.filt(i + 2) != 0 {
                self.d416 = self.filt(i + 2);
            }
            a = 0;
        }
        self.filttime = a;
        self.filtstep = if y == 0 { 0 } else { self.filt(i + 3) };
    }

    // mt_noinit: ticks 1 to n.
    fn noinit(&mut self, x: usize) -> Go {
        self.c[x].tick = self.c[x].tick.wrapping_sub(1);
        if self.c[x].tick == 0 {
            return self.tick0(x);
        }
        if self.c[x].tick & 0x80 != 0 {
            let mut a = self.c[x].tempo;
            if a < 2 {
                self.c[x].tempo = a ^ 1;
                a = self.filt(2 + usize::from(a));
            }
            self.c[x].tick = a;
        }
        let y = self.c[x].waveptr;
        if y != 0 {
            return Go::WaveExec(y);
        }
        self.temp2 = 0;
        let a = self.c[x].fxparam;
        match self.c[x].fx {
            0 => {
                if a == 0 {
                    Go::PulseExec
                } else {
                    Go::Arpeggio(a)
                }
            }
            1 => {
                self.makespeed(a);
                Go::FreqAdd(false)
            }
            2 => {
                self.makespeed(a);
                Go::FreqSub(true)
            }
            3 => self.tickntoneport(x, a),
            4 => {
                self.temp1 = a & 0xf0;
                let cmp = a & 0x0f;
                let mut v = self.c[x].arpcount;
                if v & 0x80 == 0 && v > cmp {
                    v ^= 0xff;
                }
                v = v.wrapping_add(2);
                self.c[x].arpcount = v;
                if v & 1 == 0 {
                    Go::FreqAdd(false)
                } else {
                    Go::FreqSub(true)
                }
            }
            _ => Go::PulseExec,
        }
    }

    fn makespeed(&mut self, a: u8) {
        let v = u16::from(a) << 2;
        self.temp1 = v as u8;
        self.temp2 = (self.temp2 << 2) | (v >> 8) as u8;
    }

    // mt_tickntoneport.
    fn tickntoneport(&mut self, x: usize, a: u8) -> Go {
        let y = self.c[x].note;
        if a == 0 {
            self.c[x].fxparam = 0;
            return Go::ArpFreqResetVib(y);
        }
        self.makespeed(a);
        let (lo, c) = sbc(freq_lo(y), self.c[x].freqlo, true);
        let (hi, c) = sbc(freq_hi(y), self.c[x].freqhi, c);
        self.tick_templo = lo;
        self.tick_temphi = hi;
        if hi & 0x80 != 0 {
            let (_, c) = adc(lo, self.temp1, c);
            let (r, _) = adc(hi, self.temp2, c);
            if r & 0x80 == 0 {
                self.c[x].fxparam = 0;
                return Go::ArpFreqResetVib(y);
            }
            return Go::FreqSub(true);
        }
        let reached = if self.temp2 != hi { self.temp2 >= hi } else { self.temp1 >= lo };
        if reached {
            self.c[x].fxparam = 0;
            return Go::ArpFreqResetVib(y);
        }
        Go::FreqAdd(false)
    }

    // mt_arpeggio, `a` its parameter as the routine has it in A (its top bit half speed).
    fn arpeggio(&mut self, x: usize, a: u8) -> Go {
        let c = a & 0x80 != 0;
        let old = self.c[x].arpcount;
        let (mut n, _) = adc(old, 1, c);
        if n >= 6 {
            n = 0;
        }
        self.c[x].arpcount = n;
        let p = self.c[x].fxparam;
        let off = match old >> 1 {
            0 => (p & 0x70) >> 4,
            1 => p & 0x0f,
            _ => 0,
        };
        Go::ArpFreq(off.wrapping_add(self.c[x].note))
    }

    // mt_tick0.
    fn tick0(&mut self, x: usize) -> Go {
        self.shown[x].at = Some(self.fetched[x]);
        let a = self.c[x].newfx;
        if a & 0xf8 != 0 {
            self.c[x].instnum = a & 0xf8;
        }
        let fx = a & 7;
        self.c[x].fx = fx;
        let p = self.c[x].newfxparam;
        self.c[x].fxparam = p;
        match fx {
            0 => {
                if p == 0 || self.c[x].newnote & 0x80 == 0 {
                    return Go::Tick0Done;
                }
                if self.c[x].waveptr == 0 {
                    // A is the (no) new note here: half speed.
                    return Go::Arpeggio(self.c[x].newnote);
                }
                Go::Tick0Done
            }
            3 => {
                if self.c[x].newnote & 0x80 == 0 {
                    self.c[x].note = self.c[x].newnote;
                    self.c[x].newnote = 0xff;
                }
                Go::Tick0Done
            }
            5 => {
                self.setfiltersub(p);
                Go::Tick0Done
            }
            6 => {
                self.sid(x as u8 * 7 + 6, p);
                Go::Tick0Done
            }
            7 => {
                if p & 0x80 == 0 {
                    for ch in self.c.iter_mut() {
                        ch.tempo = p;
                    }
                } else if p == 0xef {
                    self.timecounter = self.timecounter.wrapping_add(1);
                } else if p > 0xef {
                    self.volume = p;
                } else {
                    self.c[x].tempo = p & 0x7f;
                }
                Go::Tick0Done
            }
            _ => Go::Tick0Done,
        }
    }

    // mt_tick0done: a new note's start, or the wave table.
    fn tick0done(&mut self, x: usize) -> Go {
        let a = self.c[x].newnote;
        if a & 0x80 != 0 {
            let y = self.c[x].waveptr;
            return if y == 0 { Go::PulseExec } else { Go::WaveExec(y) };
        }
        self.shown[x].note = Some(a);
        self.c[x].note = a;
        self.c[x].newnote = 0xff;
        self.c[x].gate = 0xff;
        let pulse = self.ins(x, 2);
        let b = x as u8 * 7;
        if pulse != 0 {
            self.c[x].pulsedir = pulse & 0xf0;
            self.sid(b + 2, pulse & 0xf0);
            self.c[x].pulse = pulse & 0x0f;
            self.sid(b + 3, pulse & 0x0f);
        }
        self.c[x].waveptr = self.ins(x, 7);
        let ad = self.ins(x, 0);
        self.sid(b + 5, ad);
        if self.c[x].fx != 6 {
            let sr = self.ins(x, 1);
            self.sid(b + 6, sr);
        }
        self.c[x].wave = 0x09;
        self.sid(b + 4, 0x09);
        let f = self.ins(x, 6);
        if f != 0 {
            self.setfiltersub(f);
        }
        Go::NextChn
    }

    // mt_waveexec.
    fn waveexec(&mut self, x: usize, y: u8) -> Go {
        let a = self.im.wavetbl[usize::from(y)];
        if a < 8 {
            if a != self.c[x].arpcount {
                self.c[x].arpcount = self.c[x].arpcount.wrapping_add(1);
                if self.c[x].arpcount != 0 {
                    return Go::PulseExec;
                }
                self.c[x].wave = a;
            }
        } else {
            self.c[x].wave = a;
        }
        let mut n = self.im.notetbl[usize::from(y)];
        if n & 0x80 == 0 {
            n = n.wrapping_add(self.c[x].note);
        }
        self.temp1 = n & 0x7f;
        let next = self.im.wavetbl[usize::from(y.wrapping_add(1))];
        let mut ptr = y.wrapping_add(1);
        if next == 0xff {
            let l = self.im.notetbl[usize::from(y.wrapping_add(1))];
            if l == 0 {
                ptr = 0;
            } else {
                let (v, c) = adc(l, self.ins(x, 7), true);
                let (v, _) = adc(v, 0xfe, c);
                // (0 falls through to the next row, its index the instrument's.)
                ptr = if v != 0 { v } else { self.c[x].instnum.wrapping_add(1) };
            }
        }
        self.c[x].waveptr = ptr;
        Go::ArpFreqResetVib(self.temp1)
    }

    // mt_pulseexec.
    fn pulseexec(&mut self, x: usize) -> Go {
        if self.c[x].tick == GATETIMER {
            return Go::GetNewNotes;
        }
        if self.c[x].pattptr == ENDPATT {
            return Go::Sequencer;
        }
        let spd = self.ins(x, 3) & 0xfe;
        if spd == 0 {
            return Go::LoadRegs;
        }
        let dir = self.c[x].pulsedir;
        let base = dir & 0xfe;
        let (lo, c) = if dir & 1 == 0 {
            let (lo, c) = adc(base, spd, false);
            let (p, _) = adc(self.c[x].pulse, 0, c);
            self.c[x].pulse = p;
            (lo, p >= self.ins(x, 5))
        } else {
            let (lo, c) = sbc(base, spd, true);
            let (p, _) = sbc(self.c[x].pulse, 0, c);
            self.c[x].pulse = p;
            (lo, p >= self.ins(x, 4))
        };
        let b = x as u8 * 7;
        let p = self.c[x].pulse;
        self.sid(b + 3, p);
        let lo = lo.wrapping_add(u8::from(c));
        self.c[x].pulsedir = lo;
        self.sid(b + 2, lo);
        Go::LoadRegs
    }

    // mt_sequencer.
    fn sequencer(&mut self, x: usize) -> Go {
        if self.c[x].repeat != 0 {
            self.c[x].repeat -= 1;
        } else {
            let list = self.im.songs.get(usize::from(self.c[x].songnum)).cloned().unwrap_or_default();
            let get = |y: u8| list.get(usize::from(y)).copied().unwrap_or(0);
            let mut y = self.c[x].songptr;
            for _ in 0..0x400 {
                let a = get(y);
                y = y.wrapping_add(1);
                if a < REPEAT {
                    self.c[x].pattnum = a;
                    self.c[x].songptr = y;
                    break;
                } else if a < TRANSDOWN {
                    self.c[x].repeat = a - REPEAT;
                } else if a == LOOPSONG {
                    self.shown[x].looped = true;
                    y = get(y);
                } else {
                    self.c[x].trans = a.wrapping_sub(0xf0);
                }
            }
        }
        self.c[x].pattptr = self.c[x].pattptr.wrapping_add(1);
        Go::LoadRegs
    }

    // mt_getnewnotes.
    fn getnewnotes(&mut self, x: usize) -> Go {
        let pn = usize::from(self.c[x].pattnum);
        let byte = |s: &Self, y: u8| s.im.patts.get(pn).and_then(|p| p.get(usize::from(y))).copied().unwrap_or(ENDPATT);
        let mut y = self.c[x].pattptr;
        if y == 0 && self.c[x].packrest == 0 {
            self.rowidx[x] = 0;
        }
        self.fetched[x] = (self.c[x].pattnum, self.rowidx[x]);
        self.rowidx[x] = self.rowidx[x].wrapping_add(1);
        let a = byte(self, y);
        y = y.wrapping_add(1);
        if a < NOCMD {
            self.notenum = a;
            self.c[x].newfx = byte(self, y);
            y = y.wrapping_add(1);
            self.c[x].newfxparam = byte(self, y);
            y = y.wrapping_add(1);
        } else if a >= FIRSTPACKEDREST {
            if self.c[x].packrest == 0 {
                self.c[x].packrest = a;
            }
            self.c[x].packrest = self.c[x].packrest.wrapping_add(1);
            if self.c[x].packrest == 0 {
                self.c[x].pattptr = self.c[x].pattptr.wrapping_add(1);
            }
            return Go::Rest;
        } else {
            self.notenum = a - NOCMD;
        }
        self.c[x].pattptr = if byte(self, y) == ENDPATT { ENDPATT } else { y };
        let a = self.notenum;
        if a == KEYOFF {
            self.shown[x].off = true;
            self.c[x].gate = 0xfe;
            return Go::Rest;
        }
        if a > KEYOFF {
            return Go::Rest;
        }
        self.c[x].newnote = a.wrapping_add(self.c[x].trans);
        if self.c[x].newfx & 7 == 3 {
            return Go::Rest;
        }
        if self.ins(x, 3) & 1 == 0 {
            let b = x as u8 * 7;
            self.sid(b + 5, HR_AD);
            self.sid(b + 6, HR_SR);
        }
        self.c[x].gate = 0xfe;
        Go::Rest
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The SID's registers each frame as GoatTracker 1.5's own player1.bin
    /// plays the song laid out here (on a 6502, as a black box): GT1_DIR the
    /// songs, GT1_REF the logs (<name>.log, a line a frame).
    #[test]
    #[ignore]
    fn the_same_as_the_player() {
        let (Ok(dir), Ok(rf)) = (std::env::var("GT1_DIR"), std::env::var("GT1_REF")) else { return };
        let only = std::env::var("GT1_ONLY").ok();
        let show = std::env::var("GT1_SHOW").is_ok();
        let mut paths: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).collect();
        paths.sort();
        let (mut same, mut total) = (0, 0);
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
            let lines: Vec<&str> = log.lines().collect();
            let mut bad = None;
            for (f, line) in lines.iter().enumerate() {
                r.play();
                let ours: String = r.regs.iter().map(|b| format!("{b:02x}")).collect();
                if ours != *line {
                    bad = Some(f);
                    break;
                }
            }
            match bad {
                None => same += 1,
                Some(f) => {
                    eprintln!("{name}: frame {f}");
                    if show {
                        let mut r = Routine::new(&song);
                        r.init(0);
                        for g in 0..f + 3 {
                            r.play();
                            if g + 5 >= f {
                                let o: String = r.regs.iter().map(|b| format!("{b:02x}")).collect();
                                eprintln!("  {g:5} {o} {}", lines.get(g).unwrap_or(&""));
                            }
                        }
                    }
                }
            }
        }
        eprintln!("{same}/{total} the same");
        assert_eq!(same, total);
    }
}
