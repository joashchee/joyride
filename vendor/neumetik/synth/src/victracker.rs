// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! VIC-TRACKER (Daniel Kahlin, 1994-2004): a tracker for the Commodore
//! VIC-20, its songs (.vt, "T1"; "T0" from version 1.0) played on the VIC
//! chip's three square voices and its noise. Five pattern lists, one a
//! voice and a fifth for the volume and tempo, pick 32-step patterns of a
//! note and a parameter each step; positions play as long as the length
//! table says. Sixteen sounds (a duration, a frequency offset, a glide and
//! an arpeggio) and sixteen arpeggios (three modes: semitones, absolute
//! notes, or raw frequency steps) shape the notes. A module holds up to
//! fourteen songs, each a start, end and repeat position and a speed; the
//! player is ticked by the VIC's timer at the speed the song was written
//! for (PAL or NTSC, once to four times a frame).
//!
//! Ported from VIC-TRACKER 2.0's own player (musiced/player.asm, as the
//! editor plays a song) and its file format description, released under
//! the BSD license: every step in the same 8-bit arithmetic and order,
//! and where it reads past its frequency table, the player's own state
//! laid out as it lays it out. The same chip registers as that player
//! assembled and run on a 6502, every tick, through five minutes of all
//! Modland's songs. Its notice:
//!
//! > Copyright (c) 1994, 2001, 2003, 2004, Daniel Kahlin <daniel@kahlin.net>
//! > All rights reserved.
//! >
//! > Redistribution and use in source and binary forms, with or without
//! > modification, are permitted provided that the following conditions
//! > are met:
//! > 1. Redistributions of source code must retain the above copyright
//! >    notice, this list of conditions and the following disclaimer.
//! > 2. Redistributions in binary form must reproduce the above copyright
//! >    notice, this list of conditions and the following disclaimer in the
//! >    documentation and/or other materials provided with the distribution.
//! > 3. Neither the names of the copyright holders nor the names of their
//! >    contributors may be used to endorse or promote products derived
//! >    from this software without specific prior written permission.
//! >
//! > THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
//! > "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
//! > LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
//! > A PARTICULAR PURPOSE ARE DISCLAIMED.  IN NO EVENT SHALL THE REGENTS OR
//! > CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL,
//! > EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO,
//! > PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR
//! > PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF
//! > LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING
//! > NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS
//! > SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

use std::sync::Arc;

use crate::replay::{Bufs, Driver, Patch, PatchBytes, Tune};
use crate::vic::{self, Vic};

// The T1 layout, from just past the file's load address ($3300).
const SONGS: usize = 8;
const ARPEGGIOS: usize = 64;
const ARP_CONF: usize = 320;
const TITLE: usize = 412;
const AUTHOR: usize = 428;
const YEAR: usize = 444;
const SOUNDS: usize = 448;
const LENGTHS: usize = 704;
const TABS: usize = 960;
const PATTERNS: usize = 2240;
const SIZE: usize = PATTERNS + 0x40 * 128;
/// The load address the editor saves.
const LOAD: [u8; 2] = [0x00, 0x33];
/// Voices: bass, alto, soprano, noise, and volume/tempo.
pub const VOICES: usize = 5;
pub const VOICE_NAMES: [&str; VOICES] = ["Bass", "Alto", "Soprano", "Noise", "Volume"];

// The VIA timer's count between interrupts, by the machine's video standard.
const SCREENTIME_PAL: u32 = 22150;
const SCREENTIME_NTSC: u32 = 16963;

/// The note a two-digit editor note stands for (octave, then 0-B), past
/// which the player reads its arpeggio index and frequency table.
const CONV_TAB: [u8; 80] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 0, 0, 0, 0, //
    13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 0, 0, 0, 0, //
    25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 0, 0, //
    37, 38, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];
/// Each note's register value (with the gate), 0 for none.
const FREQ_TAB: [u8; 39] = [
    0, 135, 143, 147, 151, 159, 163, 167, 175, 179, 183, 187, 191, //
    195, 199, 201, 203, 207, 209, 212, 215, 217, 219, 221, 223, //
    225, 227, 228, 229, 231, 232, 233, 235, 236, 237, 238, 239, //
    240, 241,
];

/// A VIC-TRACKER module.
#[derive(Clone, Debug)]
pub struct Song {
    pub title: String,
    pub author: String,
    pub year: String,
    /// The interrupt speed: 0 legacy, 1-4 PAL once to four times a frame,
    /// 5-8 NTSC, 9-10 an external clock (played as PAL).
    pub play_mode: u8,
    pub version: (u8, u8),
    /// The module's memory as the editor holds it (a T0 converted).
    mem: Vec<u8>,
    songs: usize,
    file: Arc<Vec<u8>>,
}

pub fn probe(d: &[u8]) -> bool {
    d.len() > 8 && d[..2] == LOAD && d[2] == b'T' && matches!(d[3], b'0' | b'1')
}

/// One step of a voice's pattern: its note byte and parameter.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Step {
    pub note: u8,
    pub param: u8,
}

impl Song {
    pub fn parse(d: &[u8]) -> Option<Song> {
        if !probe(d) {
            return None;
        }
        let mut mem = vec![0u8; SIZE];
        let n = (d.len() - 2).min(SIZE);
        mem[..n].copy_from_slice(&d[2..2 + n]);
        if mem[1] == b'0' {
            convert_t0(&mut mem);
        }
        let text = |at: usize, n: usize| {
            let s = &mem[at..at + n];
            let end = s.iter().position(|&c| c == 0).unwrap_or(n);
            s[..end].iter().map(|&c| petscii(c)).collect::<String>().trim().to_string()
        };
        let (title, author, year) = (text(TITLE, 16), text(AUTHOR, 16), text(YEAR, 4));
        let songs = usize::from(mem[7]).clamp(1, 14);
        Some(Song { title, author, year, play_mode: mem[4], version: (mem[2], mem[3]), songs, mem, file: Arc::new(d.to_vec()) })
    }

    /// How many songs the module has.
    pub fn song_count(&self) -> usize {
        self.songs
    }

    /// Song `s`'s start, end and repeat positions and start speed byte.
    pub fn song(&self, s: usize) -> (u8, u8, u8, u8) {
        let at = SONGS + (s & 15) * 4;
        (self.mem[at], self.mem[at + 1], self.mem[at + 2], self.mem[at + 3])
    }

    /// The pattern voice `v` plays at position `pos`.
    pub fn pattern(&self, v: usize, pos: u8) -> u8 {
        self.mem[TABS + v * 256 + usize::from(pos)] & 0x7F
    }

    /// The steps position `pos` plays (its length table entry plus one).
    pub fn length(&self, pos: u8) -> usize {
        usize::from(self.mem[LENGTHS + usize::from(pos)]) + 1
    }

    /// Step `i` of voice `v` at position `pos`, as the player reads it
    /// (past a pattern's 32 steps, into the next).
    pub fn step(&self, v: usize, pos: u8, i: usize) -> Step {
        let at = PATTERNS + usize::from(self.pattern(v, pos)) * 0x40 + ((i * 2) & 0xFF);
        Step { note: self.mem.get(at).copied().unwrap_or(0), param: self.mem.get(at + 1).copied().unwrap_or(0) }
    }

    /// Sound `s`'s duration, frequency offset, glide and arpeggio bytes.
    pub fn sound(&self, s: usize) -> [u8; 4] {
        let at = SOUNDS + (s & 15) * 8;
        [self.mem[at], self.mem[at + 1], self.mem[at + 2], self.mem[at + 3]]
    }

    /// The note (1 the editor's C-1, 38 its C#4) of a voice's note byte;
    /// None for a tie, a rest or the noise and volume voices.
    pub fn note_of(v: usize, b: u8) -> Option<u8> {
        if v >= 3 || b == 0x80 {
            return None;
        }
        Some(CONV_TAB[usize::from(b & 0x7F).min(79)]).filter(|&n| n != 0)
    }

    /// The frequency in hertz a voice's register value plays at.
    pub fn hz(&self, v: usize, reg: u8) -> f64 {
        let div = [256.0, 128.0, 64.0, 128.0][v.min(3)];
        self.clock() / (div * f64::from(128 - u16::from(reg & 0x7F)) + 2.0)
    }

    /// Each note's register value.
    pub fn freq_tab() -> &'static [u8] {
        &FREQ_TAB
    }

    fn ntsc(&self) -> bool {
        (5..=8).contains(&self.play_mode)
    }

    fn clock(&self) -> f64 {
        if self.ntsc() { vic::NTSC_HZ } else { vic::PAL_HZ }
    }

    /// Ticks a frame and the cycles between frames.
    fn timing(&self) -> (u32, u32) {
        let times = match self.play_mode {
            2 | 6 => 2,
            3 | 7 => 3,
            4 | 8 => 4,
            _ => 1,
        };
        let screen = match self.play_mode {
            5..=8 => SCREENTIME_NTSC,
            _ => SCREENTIME_PAL,
        };
        (times, screen)
    }

    /// The speed the editor's player says a song's time is, in ticks a second.
    pub fn tick_hz(&self) -> f64 {
        let (times, screen) = self.timing();
        self.clock() / f64::from(screen + 2) * f64::from(times)
    }
}

/// The VIC's screen codes as the editor types them, as ASCII.
fn petscii(c: u8) -> char {
    match c {
        0x20..=0x40 | 0x5B | 0x5D => c as char,
        0x41..=0x5A => c as char,
        0xC1..=0xDA => (c - 0x80) as char,
        _ => ' ',
    }
}

/// A version 1.0 module made T1, as the editor's loader makes it.
fn convert_t0(mem: &mut [u8]) {
    // Song 0's speed moves past its new repeat position, which is its start.
    mem[SONGS + 3] = mem[SONGS + 2];
    mem[SONGS + 2] = mem[SONGS];
    // Room for T1's extra data after the arpeggios.
    let gap = TABS - ARP_CONF;
    mem.copy_within(ARP_CONF..SIZE - gap, ARP_CONF + gap);
    // Its loop clears 256 bytes from each of the two.
    mem[ARP_CONF..ARP_CONF + 256].fill(0);
    mem[SOUNDS..SOUNDS + 256].fill(0);
    mem[LENGTHS..LENGTHS + 256].fill(0x1F);
    // Eight-step arpeggios become sixteen, the last eight blank.
    for k in (0..16).rev() {
        let mut old = [0u8; 8];
        old.copy_from_slice(&mem[ARPEGGIOS + k * 8..ARPEGGIOS + k * 8 + 8]);
        let at = ARPEGGIOS + k * 16;
        mem[at..at + 8].copy_from_slice(&old);
        mem[at + 8..at + 16].fill(0);
    }
    for k in 0..16 {
        mem[ARP_CONF + k * 2] = 0xF0;
        mem[ARP_CONF + k * 2 + 1] = 0x07;
    }
    for k in 0..16 {
        mem[SOUNDS + k * 8 + 1] = 0xFC;
    }
}

impl Tune for Song {
    fn format(&self) -> &'static str {
        "vt"
    }
    fn file(&self) -> &[u8] {
        &self.file
    }
    fn songs(&self) -> usize {
        self.songs
    }
    fn driver(&self, song: usize) -> Box<dyn Driver> {
        Box::new(Player::new(Arc::new(self.clone()), song))
    }

    /// Each of the 16 sounds that isn't left as a new module has it: its
    /// duration, frequency offset, glide and arpeggio bytes, and the
    /// arpeggio it plays (its speed and loop, then its 16 steps).
    fn patches(&self) -> Vec<Patch> {
        let mut out = Vec::new();
        for k in 0..16 {
            let sound = self.sound(k);
            if sound == [0, 0, 0, 0] || sound == [0, 0xFC, 0, 0] {
                continue;
            }
            let mut b = PatchBytes::default();
            b.bytes(&[sound[0], sound[1], sound[2], sound[3] & 0x80]);
            if sound[3] & 0x80 != 0 {
                let n = usize::from(sound[3] & 0x0F);
                b.bytes(&self.mem[ARP_CONF + n * 2..ARP_CONF + n * 2 + 2]).bytes(&self.mem[ARPEGGIOS + n * 16..ARPEGGIOS + n * 16 + 16]);
            }
            out.push(b.patch(k + 1, ""));
        }
        out
    }
}

// Parameters' commands (the high nibble).
const PORTUP: u8 = 0x10;
const PORTDOWN: u8 = 0x20;
const ARPEGGIO: u8 = 0x30;
const PORTUPSLOW: u8 = 0x50;
const PORTDOWNSLOW: u8 = 0x60;
const SETUSERFLAG: u8 = 0x70;
const SETSOUND: u8 = 0x80;
const CUTNOTE: u8 = 0xC0;
const DELAYNOTE: u8 = 0xD0;

/// The editor's player: its variables, a byte each per voice.
#[derive(Clone, Default)]
struct State {
    speed: u8,
    count: u8,
    user_flag: u8,
    step: u8,
    pattern_pos: u8,
    start_speed: u8,
    start_step: u8,
    end_step: u8,
    repeat_step: u8,
    note: [u8; VOICES],
    param: [u8; VOICES],
    next_note: [u8; VOICES],
    next_param: [u8; VOICES],
    delay_count: [u8; VOICES],
    duration_count: [u8; VOICES],
    this_note: [u8; VOICES],
    this_param: [u8; VOICES],
    effective_note: [u8; VOICES],
    freq_offset_low: [u8; VOICES],
    freq_offset_high: [u8; VOICES],
    arp_mode: [u8; VOICES],
    arp_step: [u8; VOICES],
    arp_offset: [u8; VOICES],
    arp_count: [u8; VOICES],
    sound: [u8; VOICES],
    /// $900A to $900E.
    regs: [u8; 5],
}

struct Player {
    song: Arc<Song>,
    s: State,
    vic: Vic,
    /// The position and step each voice's next note came from, and the
    /// one playing.
    fetched: (u8, u8),
    shown: (usize, usize),
    seen: Vec<bool>,
    done: bool,
    /// Which of a frame's interrupts comes next, and the cycles each lasts.
    sub: u32,
    cycles: u32,
    voice_bufs: [Vec<f32>; 4],
}

impl Player {
    fn new(song: Arc<Song>, n: usize) -> Player {
        let vic = Vic::new(song.clock());
        let mut p = Player { song, s: State::default(), vic, fetched: (0, 0), shown: (0, 0), seen: vec![false; 256], done: false, sub: 0, cycles: 0, voice_bufs: Default::default() };
        p.init(n);
        p
    }

    fn mem(&self, at: usize) -> u8 {
        self.song.mem.get(at).copied().unwrap_or(0)
    }

    /// pl_FreqTab[y], and past it what the player keeps there: its
    /// pattern list pointers, then its voice variables.
    fn freq(&self, y: u8) -> u8 {
        let y = usize::from(y);
        if y < FREQ_TAB.len() {
            return FREQ_TAB[y];
        }
        let i = y - FREQ_TAB.len();
        let s = &self.s;
        let (arr, v) = (i / VOICES, i % VOICES);
        let tab = 0x3300 + TABS + v * 256;
        match arr {
            0 => (tab & 0xFF) as u8,
            1 => (tab >> 8) as u8,
            2 => s.note[v],
            3 => s.param[v],
            4 => s.next_note[v],
            5 => s.next_param[v],
            6 => s.delay_count[v],
            7 => s.duration_count[v],
            8 => s.this_note[v],
            9 => s.this_param[v],
            10 => s.effective_note[v],
            11 => s.freq_offset_low[v],
            12 => s.freq_offset_high[v],
            13 => s.arp_mode[v],
            14 => s.arp_step[v],
            15 => s.arp_offset[v],
            16 => s.arp_count[v],
            17 => s.sound[v],
            // The player's code: none of its notes reach here.
            _ => 0,
        }
    }

    fn write(&mut self, reg: usize, v: u8) {
        self.s.regs[reg] = v;
        self.vic.write(0x0A + reg, v);
    }

    fn set_volume(&mut self, a: u8) {
        let v = (self.s.regs[4] & 0xF0) | (a & 0x0F);
        self.write(4, v);
    }

    /// pl_Init.
    fn init(&mut self, n: usize) {
        // pl_UnInit.
        for r in 0..4 {
            self.write(r, 0);
        }
        self.set_volume(0);
        let (start, end, repeat, speed) = self.song.song(n);
        // Every voice variable starts at 0.
        self.s = State { speed: speed & 0x3F, step: start, start_speed: speed, start_step: start, end_step: end, repeat_step: repeat, regs: self.s.regs, ..State::default() };
        for x in (0..VOICES).rev() {
            self.get_voice(x);
        }
        self.update_position();
        self.cycles = self.song.timing().1;
    }

    /// pl_GetVoice: the step after the one about to play.
    fn get_voice(&mut self, x: usize) {
        let song = &self.song;
        let s = &mut self.s;
        let pat = usize::from(song.mem[TABS + x * 256 + usize::from(s.step)] & 0x7F);
        let base = PATTERNS + pat * 0x40;
        s.note[x] = s.next_note[x];
        s.param[x] = s.next_param[x];
        let y = usize::from(s.pattern_pos.wrapping_mul(2));
        let note = song.mem.get(base + y).copied().unwrap_or(0);
        s.next_param[x] = song.mem.get(base + ((y + 1) & 0xFF)).copied().unwrap_or(0);
        s.next_note[x] = if x >= 3 || note == 0x80 { note } else { CONV_TAB.get(usize::from(note & 0x7F)).copied().unwrap_or_else(|| conv_past(note & 0x7F)) | (note & 0x80) };
        if x == 0 {
            self.shown = (usize::from(self.fetched.0), usize::from(self.fetched.1));
            self.fetched = (self.s.step, self.s.pattern_pos);
        }
    }

    /// pl_UpdatePosition.
    fn update_position(&mut self) {
        let s = &mut self.s;
        let y = s.step;
        let a = s.pattern_pos;
        s.pattern_pos = s.pattern_pos.wrapping_add(1);
        if a != self.song.mem[LENGTHS + usize::from(y)] {
            return;
        }
        s.pattern_pos = 0;
        s.step = s.step.wrapping_add(1);
        if y != s.end_step {
            return;
        }
        s.step = if s.start_speed & 0xC0 == 0 { s.start_step } else { s.repeat_step };
    }

    /// pl_Play.
    fn play(&mut self) {
        self.s.count = self.s.count.wrapping_sub(1);
        if self.s.count & 0x80 != 0 {
            for x in (0..VOICES).rev() {
                self.get_voice(x);
                self.pre_check_effects(x);
                self.retrig(x);
                self.post_check_effects(x);
            }
            self.s.count = self.s.speed;
            self.update_position();
            let pos = self.shown.0;
            if self.shown.1 == 0 && std::mem::replace(&mut self.seen[pos], true) {
                self.done = true;
            }
        }
        for x in (0..VOICES).rev() {
            self.pre_update_voice(x);
            self.play_voice(x);
            self.post_update_voice(x);
        }
    }

    fn pre_check_effects(&mut self, x: usize) {
        let s = &mut self.s;
        if x == 4 {
            let a = s.note[x] & 0xF0;
            if a != 0 {
                s.speed = a >> 4;
            }
            return;
        }
        if s.param[x] & 0xF0 == SETSOUND {
            s.sound[x] = (s.param[x] & 0x0F) << 3;
        }
    }

    fn snd(&self, x: usize, off: usize) -> u8 {
        self.mem(SOUNDS + usize::from(self.s.sound[x]) + off)
    }

    fn retrig(&mut self, x: usize) {
        self.s.this_param[x] = self.s.param[x];
        let note = self.s.note[x];
        if note == 0 {
            return;
        }
        if note != 0x80 {
            self.s.this_note[x] = note;
            if note & 0x80 == 0 {
                let s = &mut self.s;
                s.delay_count[x] = 0;
                s.arp_mode[x] = 0;
                s.arp_step[x] = 0;
                s.arp_offset[x] = 0;
                s.arp_count[x] = 0;
                s.freq_offset_high[x] = 0;
                s.freq_offset_low[x] = 0x80;
                let foffs = self.snd(x, 1);
                if foffs != 0 {
                    self.portamento_signed(x, foffs);
                }
                let param = self.s.param[x];
                let arp = if param & 0xF0 == ARPEGGIO {
                    Some(param)
                } else {
                    let a = self.snd(x, 3);
                    (a & 0x80 != 0).then_some(a)
                };
                if let Some(a) = arp {
                    let n = usize::from(a & 0x0F);
                    self.s.arp_offset[x] = self.mem(ARPEGGIOS + n * 16);
                    self.s.arp_mode[x] = self.mem(ARP_CONF + n * 2) & 0xF0;
                }
            }
        }
        let dur = if x == 4 || self.s.next_note[x] & 0x80 != 0 { 0 } else { self.snd(x, 0) };
        self.s.duration_count[x] = if dur == 0 { self.s.speed.wrapping_add(1) } else { dur };
    }

    fn post_check_effects(&mut self, x: usize) {
        let s = &mut self.s;
        let p = s.param[x];
        match p & 0xF0 {
            SETUSERFLAG => s.user_flag = p & 0x0F,
            DELAYNOTE => s.delay_count[x] = p & 0x0F,
            CUTNOTE if x != 4 => s.duration_count[x] = p & 0x0F,
            _ => {}
        }
    }

    fn pre_update_voice(&mut self, x: usize) {
        let s = &mut self.s;
        if s.delay_count[x] != 0 {
            s.delay_count[x] -= 1;
        } else if s.duration_count[x] != 0 {
            s.duration_count[x] -= 1;
            s.effective_note[x] = s.this_note[x] & 0x7F;
            return;
        }
        s.effective_note[x] = 0;
        s.freq_offset_low[x] = 0;
        s.freq_offset_high[x] = 0;
        s.arp_step[x] = 0;
        s.arp_offset[x] = 0;
        s.arp_count[x] = 0;
    }

    fn play_voice(&mut self, x: usize) {
        if x == 4 {
            let a = self.s.effective_note[4];
            self.set_volume(a);
            return;
        }
        let v = self.voice_value(x);
        self.write(x, v);
    }

    /// The register value voice `x` (0-3) is set to.
    fn voice_value(&self, x: usize) -> u8 {
        let s = &self.s;
        let noise = x == 3;
        if s.arp_mode[x] == 0xF0 {
            let t1 = s.freq_offset_high[x];
            let a = s.effective_note[x] & 0x7F;
            if noise {
                if a == 0 {
                    return 0;
                }
                return a.wrapping_add(s.arp_offset[x]).wrapping_add(t1) | 0x80;
            }
            let f = self.freq(a.wrapping_add(s.arp_offset[x]));
            if f == 0 {
                return 0;
            }
            return f.wrapping_add(t1) | 0x80;
        }
        let mut t2 = s.effective_note[x] & 0x7F;
        if t2 == 0 {
            return 0;
        }
        let mut t1 = s.freq_offset_high[x];
        let t3 = s.arp_offset[x];
        if t3 != 0 {
            if t3 & 0x80 != 0 {
                t1 = 0;
            }
            if s.arp_mode[x] == 0x10 {
                let a = if noise { t2 } else { self.freq(t2) };
                if a == 0 && !noise {
                    return 0;
                }
                return a.wrapping_add(t3).wrapping_add(t1) | 0x80;
            }
            if t3 & 0x40 != 0 {
                t2 = CONV_TAB[usize::from(t3 & 0x3F)];
            } else {
                // Six bits, sign extended.
                let off = ((t3 << 2) as i8 >> 2) as u8;
                t2 = t2.wrapping_add(off);
            }
        }
        let a = if noise { t2 } else { self.freq(t2) };
        if a == 0 && !noise {
            return 0;
        }
        a.wrapping_add(t1) | 0x80
    }

    fn post_update_voice(&mut self, x: usize) {
        // Portamento.
        let p = self.s.this_param[x];
        let t1 = p & 0x0F;
        match p & 0xF0 {
            PORTUP => self.port_up(x, port_fast(t1)),
            PORTDOWN => self.port_down(x, port_fast(t1)),
            PORTUPSLOW => self.port_up(x, port_slow(t1)),
            PORTDOWNSLOW => self.port_down(x, port_slow(t1)),
            _ => {
                let glide = self.snd(x, 2);
                if glide != 0 {
                    self.portamento_signed(x, glide);
                }
            }
        }
        // Arpeggio.
        let p = self.s.this_param[x];
        if p & 0xF0 == ARPEGGIO {
            self.arpeggio(x, p & 0x0F);
        } else {
            let a = self.snd(x, 3);
            if a & 0x80 != 0 {
                self.arpeggio(x, a & 0x0F);
            }
        }
    }

    fn port_up(&mut self, x: usize, (hi, lo): (u8, u8)) {
        let s = &mut self.s;
        let v = u16::from_le_bytes([s.freq_offset_low[x], s.freq_offset_high[x]]).wrapping_add(u16::from_le_bytes([lo, hi]));
        [s.freq_offset_low[x], s.freq_offset_high[x]] = v.to_le_bytes();
    }

    fn port_down(&mut self, x: usize, (hi, lo): (u8, u8)) {
        let s = &mut self.s;
        let v = u16::from_le_bytes([s.freq_offset_low[x], s.freq_offset_high[x]]).wrapping_sub(u16::from_le_bytes([lo, hi]));
        [s.freq_offset_low[x], s.freq_offset_high[x]] = v.to_le_bytes();
    }

    /// pl_DoPortamentoSigned: a slow portamento up or down by a signed byte.
    fn portamento_signed(&mut self, x: usize, a: u8) {
        if a & 0x80 == 0 {
            self.port_up(x, port_slow(a));
        } else {
            self.port_down(x, port_slow(a.wrapping_neg()));
        }
    }

    fn arpeggio(&mut self, x: usize, n: u8) {
        let base = n.wrapping_mul(16);
        let y = usize::from(n) * 2;
        let speed = self.mem(ARP_CONF + y) & 0x0F;
        let s = &self.s;
        if speed != s.arp_count[x] {
            self.s.arp_count[x] = s.arp_count[x].wrapping_add(1);
            return;
        }
        self.s.arp_count[x] = 0;
        let conf = self.mem(ARP_CONF + y + 1);
        self.s.arp_step[x] = if conf & 0x0F == self.s.arp_step[x] { conf >> 4 } else { self.s.arp_step[x].wrapping_add(1) };
        self.s.arp_offset[x] = self.mem(ARPEGGIOS + usize::from(self.s.arp_step[x].wrapping_add(base)));
    }
}

/// pl_ConvTab read past its end: the arpeggio index, then the frequency table.
fn conv_past(i: u8) -> u8 {
    let i = usize::from(i);
    if i < 96 { ((i - 80) as u8) << 4 } else { FREQ_TAB.get(i - 96).copied().unwrap_or(0) }
}

/// A fast portamento's step from its parameter: half of it a tick, as (high, low).
fn port_fast(t: u8) -> (u8, u8) {
    (t >> 1, (t & 1) << 7)
}

/// A slow portamento's: an eighth of it.
fn port_slow(t: u8) -> (u8, u8) {
    (t >> 3, t << 5)
}

impl Driver for Player {
    fn channels(&self) -> usize {
        VOICES
    }

    fn tick(&mut self) {
        let (times, screen) = self.song.timing();
        // The timer runs a frame's first tick, then one every share of it.
        let share = screen / times;
        self.cycles = if self.sub + 1 < times { share } else { screen + 2 - share * (times - 1) };
        self.sub = (self.sub + 1) % times;
        self.play();
    }

    fn tick_seconds(&self) -> f64 {
        f64::from(self.cycles) / self.song.clock()
    }

    fn render(&mut self, rate: u32, frames: usize, out: &mut Bufs) {
        for b in self.voice_bufs.iter_mut() {
            b.clear();
            b.resize(frames, 0.0);
        }
        {
            let [a, b, c, d] = &mut self.voice_bufs;
            self.vic.render(rate, &mut [&mut a[..], &mut b[..], &mut c[..], &mut d[..]]);
        }
        for c in 0..VOICES {
            out.left[c][..frames].fill(0.0);
            out.right[c][..frames].fill(0.0);
        }
        // A voice at full volume swings ±0.15 on each side.
        for c in 0..4 {
            for i in 0..frames {
                let s = self.voice_bufs[c][i] * 0.15;
                out.left[c][i] = s;
                out.right[c][i] = s;
            }
        }
    }

    fn position(&self) -> (usize, usize) {
        self.shown
    }

    fn speed(&self) -> (u32, u32) {
        let bpm = self.song.tick_hz() * 60.0 / f64::from(u32::from(self.s.speed) + 1) / 4.0;
        (u32::from(self.s.speed) + 1, bpm.round() as u32)
    }

    fn take_done(&mut self) -> bool {
        std::mem::take(&mut self.done)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A module of one position: C-2 on the bass, volume 15 and speed 3 on the fifth voice.
    fn tiny() -> Vec<u8> {
        let mut m = vec![0u8; PATTERNS + 0x40 * 3];
        m[..2].copy_from_slice(b"T1");
        (m[2], m[4], m[7]) = (2, 1, 1);
        // Song 0: positions 0 to 0, speed 5.
        m[SONGS + 3] = 5;
        m[LENGTHS] = 0x1F;
        // The bass plays pattern 0, the volume voice pattern 1, the rest the empty 2.
        for v in 1..4 {
            m[TABS + v * 256] = 2;
        }
        m[TABS + 4 * 256] = 1;
        m[PATTERNS] = 0x20;
        m[PATTERNS + 0x40] = 0x3F;
        let mut f = LOAD.to_vec();
        f.extend_from_slice(&m);
        f
    }

    #[test]
    fn plays_a_note() {
        let song = Song::parse(&tiny()).unwrap();
        assert_eq!(song.song_count(), 1);
        let mut p = Player::new(Arc::new(song), 0);
        p.play();
        // C-2 is note 13 (195 with the gate), volume 15; the speed byte sets 3.
        assert_eq!(p.s.regs, [195, 0, 0, 0, 0x0F]);
        assert_eq!(p.s.speed, 3);
        assert_eq!(p.position(), (0, 0));
    }

    /// The registers each tick against VIC-TRACKER's own player assembled
    /// and run on a 6502 (`vtlog`): VT_DIR holds the songs (in folders),
    /// VT_REF the logs as `<name>.<song>.log`.
    #[test]
    #[ignore]
    fn against_reference() {
        let (Ok(dir), Ok(rf)) = (std::env::var("VT_DIR"), std::env::var("VT_REF")) else { return };
        let mut files = Vec::new();
        let mut stack = vec![std::path::PathBuf::from(dir)];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|x| x == "vt") {
                    files.push(p);
                }
            }
        }
        files.sort();
        let (mut same, mut total) = (0, 0);
        for f in files {
            let song = Song::parse(&std::fs::read(&f).unwrap()).unwrap();
            let name = f.file_stem().unwrap().to_string_lossy().to_string();
            for n in 0..song.songs {
                let Ok(log) = std::fs::read_to_string(format!("{rf}/{name}.{n}.log")) else { continue };
                total += 1;
                let mut p = Player::new(Arc::new(song.clone()), n);
                let mut bad = None;
                for (t, line) in log.lines().enumerate() {
                    p.play();
                    let ours: String = p.s.regs.iter().map(|r| format!("{r:02x}")).collect();
                    if ours != line {
                        bad = Some((t, ours, line.to_string()));
                        break;
                    }
                }
                match bad {
                    None => same += 1,
                    Some((t, o, r)) => println!("{name}.{n}: tick {t}: ours {o} ref {r}"),
                }
            }
        }
        println!("{same} of {total} the same");
        assert_eq!(same, total);
    }
}
