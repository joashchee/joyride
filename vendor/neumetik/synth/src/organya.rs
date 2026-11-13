// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! Organya (Studio Pixel, 1999; Organya Twei by Rxo Inverse): the music
//! of Cave Story. Eight melody tracks, each one of 100 single-cycle
//! waveforms, and eight drum tracks, each one of 42 drum sounds; a song
//! is each track's notes placed on a timeline of ticks, `wait` ms apart,
//! looping from its end back to its repeat point.
//!
//! Organya played its sounds through DirectSound, so what's heard is
//! DirectSound's: a melody track has two buffers an octave (a cycle of
//! 256 to 8 points, or for pizzicato 4 to 32 cycles played once), taking
//! turns note by note so the one let go finishes its pass instead of
//! clicking; volume and pan are DirectSound's, in hundredths of a decibel;
//! each buffer is stepped at its frequency and resampled by 4-point
//! Lagrange interpolation, as Windows Vista and later mix it (organya.h's
//! finding). The same as organya.h, set to the editor's volume and tuning,
//! on 374 of 377 songs; the other three are where organya.h isn't the
//! editor (a pizzicato note it never lets go, a tick a sample late).
//!
//! This follows the editor's own source (Organya Twei 2.1.1,
//! github.com/shbow/organya: OrgPlay.cpp, Sound.cpp, OrgFile.cpp), and its
//! waveforms and drums are the editor's (in `data/organya`), under this
//! licence:
//!
//! > Copyright (c) 2018, Rxo Inverse , Studio Pixel
//! > All rights reserved.
//! >
//! > Redistribution and use in source and binary forms, with or without
//! > modification, are permitted provided that the following conditions are met:
//! >
//! > * Redistributions of source code must retain the above copyright notice, this
//! >   list of conditions and the following disclaimer.
//! >
//! > * Redistributions in binary form must reproduce the above copyright notice,
//! >   this list of conditions and the following disclaimer in the documentation
//! >   and/or other materials provided with the distribution.
//! >
//! > * Neither the name of the copyright holder nor the names of its
//! >   contributors may be used to endorse or promote products derived from
//! >   this software without specific prior written permission.
//! >
//! > THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
//! > AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//! > IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
//! > DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
//! > FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
//! > DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
//! > SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
//! > CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
//! > OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
//! > OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

use std::sync::{Arc, OnceLock};

use crate::replay::{Bufs, Driver, Patch, PatchBytes, Tune};

pub const MELODY: usize = 8;
pub const TRACKS: usize = 16;
/// "Org-0n" and the song's header: wait, line, dot, repeat, end, and each
/// track's frequency, wave, pizzicato and note count.
const HEADER: usize = 6 + 12 + TRACKS * 6;
/// A note's key, volume or pan left as it was.
pub const DUMMY: u8 = 255;
/// A buffer's sample at full scale and 0 dB, at the Tracker's level.
const LEVEL: f32 = 0.4;
/// DirectSound's frequency range; a frequency outside it isn't set.
const FREQ_MIN: f64 = 100.0;
const FREQ_MAX: f64 = 200_000.0;
/// The format the editor makes its buffers in (8-bit mono).
const BASE_HZ: u32 = 22_050;

/// The drums, in the order a drum track's wave number picks them.
pub const DRUMS: [&str; 42] = [
    "Bass01", "Bass02", "Snare01", "Snare02", "Tom01", "HiClose", "HiOpen", "Crash", "Per01", "Per02", "Bass03", "Tom02", "Bass04", "Bass05", "Snare03", "Snare04", "HiClose02",
    "HiOpen02", "HiClose03", "HiOpen03", "Crash02", "RevSym01", "Ride01", "Tom03", "Tom04", "OrcDrm01", "Bell", "Cat", "Bass06", "Bass07", "Snare05", "Snare06", "Snare07",
    "Tom05", "HiOpen04", "HiClose04", "Clap01", "Pesi01", "Quick01", "Bass08", "Snare08", "HiClose05",
];

/// The drums' WAV files, as the names above (HiClose02 and HiOpen02 are
/// the files HiClose2 and HiOpen2).
const DRUM_FILES: [&[u8]; 42] = [
    include_bytes!("../data/organya/Bass01.wav"),
    include_bytes!("../data/organya/Bass02.wav"),
    include_bytes!("../data/organya/Snare01.wav"),
    include_bytes!("../data/organya/Snare02.wav"),
    include_bytes!("../data/organya/Tom01.wav"),
    include_bytes!("../data/organya/HiClose.wav"),
    include_bytes!("../data/organya/HiOpen.wav"),
    include_bytes!("../data/organya/Crash.wav"),
    include_bytes!("../data/organya/Per01.wav"),
    include_bytes!("../data/organya/Per02.wav"),
    include_bytes!("../data/organya/Bass03.wav"),
    include_bytes!("../data/organya/Tom02.wav"),
    include_bytes!("../data/organya/Bass04.wav"),
    include_bytes!("../data/organya/Bass05.wav"),
    include_bytes!("../data/organya/Snare03.wav"),
    include_bytes!("../data/organya/Snare04.wav"),
    include_bytes!("../data/organya/HiClose2.wav"),
    include_bytes!("../data/organya/HiOpen2.wav"),
    include_bytes!("../data/organya/HiClose03.wav"),
    include_bytes!("../data/organya/HiOpen03.wav"),
    include_bytes!("../data/organya/Crash02.wav"),
    include_bytes!("../data/organya/RevSym01.wav"),
    include_bytes!("../data/organya/Ride01.wav"),
    include_bytes!("../data/organya/Tom03.wav"),
    include_bytes!("../data/organya/Tom04.wav"),
    include_bytes!("../data/organya/OrcDrm01.wav"),
    include_bytes!("../data/organya/Bell.wav"),
    include_bytes!("../data/organya/Cat.wav"),
    include_bytes!("../data/organya/Bass06.wav"),
    include_bytes!("../data/organya/Bass07.wav"),
    include_bytes!("../data/organya/Snare05.wav"),
    include_bytes!("../data/organya/Snare06.wav"),
    include_bytes!("../data/organya/Snare07.wav"),
    include_bytes!("../data/organya/Tom05.wav"),
    include_bytes!("../data/organya/HiOpen04.wav"),
    include_bytes!("../data/organya/HiClose04.wav"),
    include_bytes!("../data/organya/Clap01.wav"),
    include_bytes!("../data/organya/Pesi01.wav"),
    include_bytes!("../data/organya/Quick01.wav"),
    include_bytes!("../data/organya/Bass08.wav"),
    include_bytes!("../data/organya/Snare08.wav"),
    include_bytes!("../data/organya/HiClose05.wav"),
];

/// The 100 waveforms, 256 signed points each.
const WAVES: &[u8; 25_600] = include_bytes!("../data/organya/Wave.dat");

/// Each octave's buffer: its cycle's points, the octave's multiplier (in
/// eighths) and, for pizzicato, how many cycles it holds.
const OCTAVES: [(usize, f64, usize); 8] = [(256, 1.0, 4), (256, 2.0, 8), (128, 4.0, 12), (128, 8.0, 16), (64, 16.0, 20), (32, 32.0, 24), (16, 64.0, 28), (8, 128.0, 32)];
/// C4 to B4 in Hz (the editor's table since 2.1.0).
const FREQS: [f64; 12] = [
    261.62556530060, 277.18263097687, 293.66476791741, 311.12698372208, 329.62755691287, 349.22823143300, 369.99442271163, 391.99543598175, 415.30469757995, 440.00000000000,
    466.16376151809, 493.88330125612,
];
/// Pans 0 (left) to 12 (right), 6 the middle, on DirectSound's 0 to 512.
pub const PANS: [i16; 13] = [0, 43, 86, 129, 172, 215, 256, 297, 340, 383, 426, 469, 512];

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Note {
    /// Its tick.
    pub x: i32,
    /// Its key (0 the octave of C1 to 95), or `DUMMY` for a change of volume or pan alone.
    pub key: u8,
    /// Ticks it's held (melody).
    pub length: u8,
    /// 0 to 254, or `DUMMY`.
    pub volume: u8,
    /// 0 to 12, or `DUMMY`.
    pub pan: u8,
}

#[derive(Clone, Debug, Default)]
pub struct Track {
    /// The frequency's offset plus 1000 (melody).
    pub freq: u16,
    /// Its waveform (melody, 0 to 99) or drum (0 to 41).
    pub wave: u8,
    /// Each note played once rather than held (Org-02 and later).
    pub pizzicato: bool,
    pub notes: Vec<Note>,
}

#[derive(Clone, Debug)]
pub struct Song {
    /// 1 to 3, from "Org-0n".
    pub version: u8,
    /// Ms a tick.
    pub wait: u16,
    /// Beats a measure and ticks a beat, as the editor's grid shows them.
    pub line: u8,
    pub dot: u8,
    pub repeat: i32,
    pub end: i32,
    pub tracks: Vec<Track>,
    file: Arc<Vec<u8>>,
}

pub fn probe(d: &[u8]) -> bool {
    d.len() >= HEADER && &d[..5] == b"Org-0" && matches!(d[5], b'1'..=b'3')
}

impl Song {
    pub fn parse(d: &[u8]) -> Option<Song> {
        if !probe(d) {
            return None;
        }
        let version = d[5] - b'0';
        let u16_at = |at: usize| u16::from_le_bytes([d[at], d[at + 1]]);
        let i32_at = |at: usize| i32::from_le_bytes([d[at], d[at + 1], d[at + 2], d[at + 3]]);
        let mut tracks = Vec::with_capacity(TRACKS);
        let mut at = HEADER;
        let mut counts = [0usize; TRACKS];
        for (t, count) in counts.iter_mut().enumerate() {
            let h = 18 + t * 6;
            *count = usize::from(u16_at(h + 4));
            // Org-01 had no pizzicato; its byte is ignored.
            tracks.push(Track { freq: u16_at(h), wave: d[h + 2], pizzicato: version >= 2 && d[h + 3] != 0, notes: Vec::new() });
        }
        for (t, &n) in counts.iter().enumerate() {
            let cols = d.get(at..at + n * 8)?;
            tracks[t].notes = (0..n)
                .map(|i| Note {
                    x: i32::from_le_bytes(cols[i * 4..i * 4 + 4].try_into().expect("four bytes")),
                    key: cols[n * 4 + i],
                    length: cols[n * 5 + i],
                    volume: cols[n * 6 + i],
                    pan: cols[n * 7 + i],
                })
                .collect();
            at += n * 8;
        }
        Some(Song { version, wait: u16_at(6), line: d[8], dot: d[9], repeat: i32_at(10), end: i32_at(14), tracks, file: Arc::new(d.to_vec()) })
    }

    /// Ticks a measure, as the editor's grid has them (16 when unset, 256 at most).
    pub fn measure(&self) -> usize {
        match usize::from(self.line) * usize::from(self.dot) {
            0 => 16,
            n => n.min(256),
        }
    }

    /// The drum a drum track plays, if it's one the editor has.
    pub fn drum_name(&self, track: usize) -> Option<&'static str> {
        DRUMS.get(usize::from(self.tracks[track].wave)).copied()
    }
}

impl Tune for Song {
    fn format(&self) -> &'static str {
        "org"
    }
    fn file(&self) -> &[u8] {
        &self.file
    }
    fn driver(&self, _song: usize) -> Box<dyn Driver> {
        Box::new(Player::new(Arc::new(self.clone())))
    }
    fn length(&self, _song: usize) -> Option<f64> {
        Some(f64::from(self.end.max(1)) * f64::from(self.wait.max(1)) / 1000.0)
    }

    /// Each track that plays: a melody track's waveform (one of the
    /// editor's 100), its frequency offset and pizzicato, or a drum
    /// track's drum. The waveforms and drums are built in, so their
    /// numbers say them.
    fn patches(&self) -> Vec<Patch> {
        let mut out = Vec::new();
        for (t, track) in self.tracks.iter().enumerate() {
            if track.notes.is_empty() {
                continue;
            }
            let mut b = PatchBytes::default();
            let name = if t < 8 {
                b.u8(0).u8(track.wave).u16(track.freq).u8(u8::from(track.pizzicato));
                format!("Wave {:02}", track.wave)
            } else {
                b.u8(1).u8(track.wave);
                format!("Drum {:02}", track.wave)
            };
            out.push(b.patch(t + 1, &name));
        }
        out
    }
}

/// The drums' samples, from 8-bit unsigned to ±1.
fn drums() -> &'static [Vec<f32>] {
    static D: OnceLock<Vec<Vec<f32>>> = OnceLock::new();
    D.get_or_init(|| {
        DRUM_FILES
            .iter()
            .map(|f| {
                // The editor reads the data's size at 0x36 and the data at 0x3A.
                let n = u32::from_le_bytes(f[0x36..0x3A].try_into().expect("four bytes")) as usize;
                f[0x3A..(0x3A + n).min(f.len())].iter().map(|&b| (f32::from(b) - 128.0) / 128.0).collect()
            })
            .collect()
    })
}

/// A melody track's buffers' data, an octave each: the waveform's cycle
/// at the octave's points (every 256/points-th point), repeated for
/// pizzicato.
fn melody_data(wave: u8, pizzicato: bool) -> Vec<Arc<[f32]>> {
    let w = &WAVES[usize::from(wave) * 256..usize::from(wave) * 256 + 256];
    OCTAVES
        .iter()
        .map(|&(size, _, cycles)| {
            let n = if pizzicato { size * cycles } else { size };
            (0..n).map(|i| f32::from(w[(i * (256 / size)) % 256] as i8) / 128.0).collect()
        })
        .collect()
}

/// A DirectSound buffer: its data played at its frequency, looping or
/// once, at its volume and pan, resampled by 4-point Lagrange.
#[derive(Clone)]
struct Buffer {
    data: Arc<[f32]>,
    playing: bool,
    looping: bool,
    pos: usize,
    freq: f64,
    gain: (f32, f32),
    volume: i32,
    pan: i32,
    /// The last four points read, and how far between the middle two.
    hist: [f32; 4],
    frac: f32,
    /// Points still to read after it stops, to let the history empty.
    tail: u8,
}

impl Buffer {
    fn new(data: Arc<[f32]>) -> Buffer {
        Buffer { data, playing: false, looping: false, pos: 0, freq: f64::from(BASE_HZ), gain: (1.0, 1.0), volume: 0, pan: 0, hist: [0.0; 4], frac: 0.0, tail: 0 }
    }

    /// Play: from the top if stopped (between points afresh once what it
    /// played last has died away), else on from where it is.
    fn play(&mut self, looping: bool) {
        if self.data.is_empty() {
            return;
        }
        if !self.playing {
            self.pos = 0;
            if self.tail == 0 {
                self.frac = 0.0;
            }
        }
        self.playing = true;
        self.looping = looping;
    }

    /// Stop, and back to the top (SetCurrentPosition(0): between points afresh).
    fn stop(&mut self) {
        if self.playing {
            self.tail = 4;
        }
        self.playing = false;
        self.pos = 0;
        self.frac = 0.0;
    }

    /// SetFrequency (the editor's double cast to a DWORD); out of range, it's not set.
    fn set_freq(&mut self, hz: f64) {
        let hz = hz.trunc();
        if (FREQ_MIN..=FREQ_MAX).contains(&hz) {
            self.freq = hz;
        }
    }

    /// SetVolume and SetPan, in hundredths of a decibel: the pan
    /// attenuates the far side.
    fn set_volume(&mut self, v: i32) {
        self.volume = v.clamp(-10_000, 0);
        self.regain();
    }

    fn set_pan(&mut self, p: i32) {
        self.pan = p.clamp(-10_000, 10_000);
        self.regain();
    }

    fn regain(&mut self) {
        let db = |h: i32| 10f32.powf(h as f32 / 2000.0);
        let g = db(self.volume);
        self.gain = if self.pan < 0 { (g, g * db(self.pan)) } else { (g * db(-self.pan), g) };
    }

    fn sounding(&self) -> bool {
        self.playing || self.tail > 0
    }

    fn read(&mut self) {
        let s = if self.playing {
            let s = self.data[self.pos];
            self.pos += 1;
            if self.pos >= self.data.len() {
                self.pos = 0;
                if !self.looping {
                    self.playing = false;
                    self.tail = 4;
                }
            }
            s
        } else {
            self.tail = self.tail.saturating_sub(1);
            0.0
        };
        self.hist = [self.hist[1], self.hist[2], self.hist[3], s];
    }

    /// Adds the next `l.len()` frames at `rate`.
    fn mix(&mut self, rate: f64, l: &mut [f32], r: &mut [f32]) {
        // Stepped in f32, as organya.h (Zlib) steps it: a note held for a
        // whole number of cycles then lets go a point before the loop, as there.
        let step = self.freq as f32 / rate as f32;
        for (l, r) in l.iter_mut().zip(r.iter_mut()) {
            if !self.sounding() {
                return;
            }
            // Lagrange through the points at -1, 0, 1 and 2, at t between 0 and 1.
            let t = self.frac;
            let [a, b, c, d] = self.hist;
            let s = -a * t * (t - 1.0) * (t - 2.0) / 6.0 + b * (t + 1.0) * (t - 1.0) * (t - 2.0) / 2.0 - c * (t + 1.0) * t * (t - 2.0) / 2.0 + d * (t + 1.0) * t * (t - 1.0) / 6.0;
            *l += s * self.gain.0 * LEVEL;
            *r += s * self.gain.1 * LEVEL;
            self.frac += step;
            let k = self.frac as u32;
            self.frac %= 1.0;
            for _ in 0..k {
                self.read();
            }
        }
    }
}

/// A melody track's buffers (two an octave), the key sounding (or none)
/// and which of the two is in use.
struct Melody {
    bufs: Vec<[Buffer; 2]>,
    old_key: u8,
    twin: usize,
    pizzicato: bool,
    freq: u16,
    /// Ticks the note has left.
    left: i32,
}

impl Melody {
    fn cur(&mut self) -> &mut Buffer {
        let o = usize::from(self.old_key / 12);
        &mut self.bufs[o][self.twin]
    }

    /// ChangeOrganFrequency: every buffer the track has, to the key's note in its octave.
    fn tune(&mut self, key: u8) {
        let base = FREQS[usize::from(key % 12)];
        for (o, pair) in self.bufs.iter_mut().enumerate() {
            let (size, mul, _) = OCTAVES[o];
            let hz = size as f64 * base * mul / 8.0 + (f64::from(self.freq) - 1000.0);
            for b in pair.iter_mut() {
                b.set_freq(hz);
            }
        }
    }

    /// PlayOrganObject (and its pizzicato twin) with a key: the note let
    /// go finishes its pass in its buffer while the next starts in the
    /// other.
    fn note_on(&mut self, key: u8) {
        if self.bufs.is_empty() || key >= 96 {
            return;
        }
        let once = self.pizzicato;
        if self.old_key == DUMMY {
            self.tune(key);
            self.bufs[usize::from(key / 12)][self.twin].play(!once);
        } else {
            if !once {
                self.cur().play(false);
            }
            self.twin ^= 1;
            if self.old_key != key {
                self.tune(key);
            }
            self.bufs[usize::from(key / 12)][self.twin].play(!once);
        }
        self.old_key = key;
    }

    /// Mode 2: the note let go, to finish its pass.
    fn note_off(&mut self) {
        if self.bufs.is_empty() || self.old_key == DUMMY {
            return;
        }
        if !self.pizzicato {
            self.cur().play(false);
        }
        self.old_key = DUMMY;
    }
}

struct Player {
    song: Arc<Song>,
    melody: Vec<Melody>,
    drums: Vec<Option<Buffer>>,
    /// The tick, and each track's next note.
    x: i32,
    next: [usize; TRACKS],
    shown: (usize, usize),
    done: bool,
}

impl Player {
    fn new(song: Arc<Song>) -> Player {
        let melody = song.tracks[..MELODY]
            .iter()
            .map(|t| {
                // A waveform past 99 isn't made: the track is silent.
                let bufs = if t.wave < 100 { melody_data(t.wave, t.pizzicato).into_iter().map(|d| [Buffer::new(d.clone()), Buffer::new(d)]).collect() } else { Vec::new() };
                Melody { bufs, old_key: DUMMY, twin: 0, pizzicato: t.pizzicato, freq: t.freq, left: 0 }
            })
            .collect();
        let drums = song.tracks[MELODY..].iter().map(|t| drums().get(usize::from(t.wave)).map(|d| Buffer::new(d.as_slice().into()))).collect();
        let mut p = Player { song, melody, drums, x: 0, next: [0; TRACKS], shown: (0, 0), done: false };
        p.seek(0);
        p
    }

    /// SetPlayPointer: each track's next note the first at or after `x`.
    fn seek(&mut self, x: i32) {
        for (t, n) in self.next.iter_mut().enumerate() {
            *n = self.song.tracks[t].notes.iter().position(|n| n.x >= x).unwrap_or(usize::MAX);
        }
        self.x = x;
    }

    /// The note due now on track `t`, moving on past it.
    fn due(&mut self, t: usize) -> Option<Note> {
        let n = *self.song.tracks[t].notes.get(self.next[t])?;
        // As the editor, a note only plays on its own tick: one out of order holds the track up.
        if n.x != self.x {
            return None;
        }
        self.next[t] += 1;
        Some(n)
    }

    /// PlayData: a tick.
    fn play_data(&mut self) {
        let m = self.song.measure() as i32;
        self.shown = ((self.x.max(0) / m) as usize, (self.x.max(0) % m) as usize);
        for t in 0..MELODY {
            if let Some(n) = self.due(t) {
                let mel = &mut self.melody[t];
                if n.key != DUMMY {
                    mel.note_on(n.key);
                    mel.left = i32::from(n.length);
                }
                if !mel.bufs.is_empty() && mel.old_key != DUMMY {
                    if n.pan != DUMMY {
                        let p = (i32::from(PANS[usize::from(n.pan.min(12))]) - 256) * 10;
                        mel.cur().set_pan(p);
                    }
                    if n.volume != DUMMY {
                        let v = (i32::from(n.volume) - 255) * 8;
                        mel.cur().set_volume(v);
                    }
                }
            }
            let mel = &mut self.melody[t];
            if mel.left == 0 {
                mel.note_off();
            }
            if mel.left > 0 {
                mel.left -= 1;
            }
        }
        for t in MELODY..TRACKS {
            if let Some(n) = self.due(t) {
                if let Some(b) = &mut self.drums[t - MELODY] {
                    if n.key != DUMMY {
                        b.stop();
                        b.set_freq(f64::from(n.key) * 800.0 + 100.0);
                        b.play(false);
                    }
                    if n.pan != DUMMY {
                        b.set_pan((i32::from(PANS[usize::from(n.pan.min(12))]) - 256) * 10);
                    }
                    if n.volume != DUMMY {
                        b.set_volume((i32::from(n.volume) - 255) * 8);
                    }
                }
            }
        }
        self.x += 1;
        if self.x >= self.song.end {
            let r = self.song.repeat;
            self.seek(r);
            self.done = true;
        }
    }
}

impl Driver for Player {
    fn channels(&self) -> usize {
        TRACKS
    }

    fn tick(&mut self) {
        self.play_data();
    }

    fn tick_seconds(&self) -> f64 {
        f64::from(self.song.wait.max(1)) / 1000.0
    }

    fn render(&mut self, rate: u32, frames: usize, out: &mut Bufs) {
        let rate = f64::from(rate.max(1));
        for c in 0..TRACKS {
            out.left[c][..frames].fill(0.0);
            out.right[c][..frames].fill(0.0);
        }
        for (t, m) in self.melody.iter_mut().enumerate() {
            for pair in m.bufs.iter_mut() {
                for b in pair.iter_mut() {
                    b.mix(rate, &mut out.left[t][..frames], &mut out.right[t][..frames]);
                }
            }
        }
        for (t, b) in self.drums.iter_mut().enumerate() {
            if let Some(b) = b {
                b.mix(rate, &mut out.left[MELODY + t][..frames], &mut out.right[MELODY + t][..frames]);
            }
        }
    }

    fn position(&self) -> (usize, usize) {
        self.shown
    }

    fn speed(&self) -> (u32, u32) {
        // A beat is `dot` ticks.
        let beat = f64::from(self.song.wait.max(1)) * f64::from(self.song.dot.max(1));
        (1, (60_000.0 / beat).round().clamp(1.0, 999.0) as u32)
    }

    fn take_done(&mut self) -> bool {
        std::mem::take(&mut self.done)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data() {
        let d = drums();
        assert_eq!(d.len(), 42);
        assert_eq!(d[0].len(), 1299);
        // Every waveform's octave 3 is one 128-point cycle, every other point of it.
        let m = melody_data(0, false);
        assert_eq!(m[3].len(), 128);
        assert_eq!(m[3][1], f32::from(WAVES[2] as i8) / 128.0);
        assert_eq!(melody_data(5, true)[0].len(), 1024);
    }

    #[test]
    fn a440() {
        // Key 45 (A4) in octave 3: 128 points at 440 Hz.
        let mut m = Melody { bufs: melody_data(0, false).into_iter().map(|d| [Buffer::new(d.clone()), Buffer::new(d)]).collect(), old_key: DUMMY, twin: 0, pizzicato: false, freq: 1000, left: 0 };
        m.note_on(45);
        assert_eq!(m.bufs[3][0].freq, 56_320.0);
        assert!(m.bufs[3][0].playing && m.bufs[3][0].looping);
        // The next note takes the other buffer; the first plays out its pass.
        m.note_on(47);
        assert!(m.bufs[3][0].playing && !m.bufs[3][0].looping);
        assert!(m.bufs[3][1].playing && m.bufs[3][1].looping);
    }

    #[test]
    fn gains() {
        let mut b = Buffer::new(Arc::from(vec![0.0f32; 4]));
        b.set_volume((200 - 255) * 8);
        b.set_pan((PANS[0] as i32 - 256) * 10);
        // -4.4 dB, the right side 25.6 dB further down.
        assert!((b.gain.0 - 0.6026).abs() < 1e-3);
        assert!((b.gain.1 - 0.6026 * 0.0525).abs() < 1e-3);
    }

    /// Plays every song in ORG_DIR through, to look for panics and silence.
    #[test]
    #[ignore]
    fn corpus() {
        let dir = std::env::var("ORG_DIR").expect("ORG_DIR");
        let mut n = 0;
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            let d = std::fs::read(&p).unwrap();
            let Some(s) = Song::parse(&d) else {
                println!("unread: {}", p.display());
                continue;
            };
            let mut pl = Player::new(Arc::new(s));
            let mut bufs = Bufs { left: vec![vec![0.0; 4410]; TRACKS], right: vec![vec![0.0; 4410]; TRACKS] };
            let mut peak = 0f32;
            for _ in 0..200 {
                pl.tick();
                let f = (44_100.0 * pl.tick_seconds()) as usize;
                let f = f.min(4410);
                pl.render(44_100, f, &mut bufs);
                for c in 0..TRACKS {
                    peak = bufs.left[c][..f].iter().chain(&bufs.right[c][..f]).fold(peak, |a, &b| a.max(b.abs()));
                }
            }
            if peak == 0.0 {
                println!("silent: {}", p.display());
            }
            n += 1;
        }
        println!("{n} songs");
    }
}
