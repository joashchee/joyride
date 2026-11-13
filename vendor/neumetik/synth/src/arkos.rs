// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! Arkos Tracker's songs (Julien Nevo, Targhan/Arkos): Amstrad CPC (and
//! other AY) music of three channels a PSG, read from STarKos (.sks,
//! "STK1.0SONG"), Arkos Tracker 1 (.aks, "ArkosTrackerSong" XML) and
//! Arkos Tracker 2 and 3 (.aks, "aks:song" XML). Positions each pick a
//! pattern (a track a channel, transposed, plus a speed and an event
//! track) and a height; each cell a note, an instrument and up to four
//! effects (volume and its slides, pitch slides and glides, arpeggios
//! inline or from tables, pitch tables, a reset, forced speeds). An
//! instrument is a list of cells, each sounding the software period, the
//! hardware envelope or both, linked one from the other by a ratio, with
//! a volume, a noise, arpeggios and pitches; arpeggio and pitch tables
//! loop at their own speed. Played on the AY (Ayumi's, ported) at the
//! song's PSG clock, a frame at its replay rate.
//!
//! Ported from Arkos Tracker 3, whose source its author released under
//! the MIT License: its song player (SongPlayer, ChannelPlayer), its
//! song model's rules, and its importers for STarKos, Arkos Tracker 1 and
//! Arkos Tracker 2's format. Its notice:
//!
//! > MIT License
//! >
//! > Copyright (c) 2016-2024 Julien Nevo (contact@julien-nevo.com)
//! >
//! > Permission is hereby granted, free of charge, to any person obtaining
//! > a copy of this software and associated documentation files (the
//! > "Software"), to deal in the Software without restriction, including
//! > without limitation the rights to use, copy, modify, merge, publish,
//! > distribute, sublicense, and/or sell copies of the Software, and to
//! > permit persons to whom the Software is furnished to do so, subject to
//! > the following conditions:
//! >
//! > The above copyright notice and this permission notice shall be
//! > included in all copies or substantial portions of the Software.
//! >
//! > THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
//! > EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
//! > MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
//! > NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE
//! > LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
//! > OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION
//! > WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

use std::sync::Arc;

use crate::ay::Ay;
use crate::replay::{Bufs, Driver, Patch, PatchBytes, Tune};

/// The highest note (C-0, note 0, is MIDI's 24).
pub const NOTE_MAX: i32 = 127;
/// The note a reset (RST) is written as, with instrument 0.
pub const RST_NOTE: u8 = 48;
/// Rows a track holds at most.
pub const TRACK_ROWS: usize = 128;
const MAX_VOLUME: i32 = 15;
const MAX_SOFTWARE_PERIOD: i32 = 0xFFF;
const MAX_HARDWARE_PERIOD: i32 = 0xFFFF;
const DEFAULT_RATIO: i32 = 4;
const DEFAULT_ENVELOPE: i32 = 8;
/// The CPC's PSG clock.
pub const CPC_HZ: i32 = 1_000_000;

/// A cell's effects, as Arkos Tracker numbers them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    PitchUp,
    PitchDown,
    PitchGlide,
    PitchTable,
    Volume,
    VolumeIn,
    VolumeOut,
    ArpeggioTable,
    Arpeggio3Notes,
    Arpeggio4Notes,
    Reset,
    ForceInstrumentSpeed,
    ForceArpeggioSpeed,
    ForcePitchTableSpeed,
    FastPitchUp,
    FastPitchDown,
    Legato,
}

/// How an instrument cell's software (the tone) and hardware (the
/// envelope) parts are made, one from the other or each on its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Link {
    #[default]
    NoSoftNoHard,
    SoftOnly,
    SoftToHard,
    HardOnly,
    HardToSoft,
    SoftAndHard,
}

impl Link {
    fn is_software(self) -> bool {
        matches!(self, Link::NoSoftNoHard | Link::SoftOnly)
    }
}

/// One step of an instrument, as the editor keeps it: "primary" is the
/// software part unless the link makes the hardware the source.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InstrumentCell {
    pub link: Link,
    pub volume: i32,
    pub noise: i32,
    pub primary_period: i32,
    pub primary_arpeggio: i32,
    pub primary_pitch: i32,
    pub ratio: i32,
    pub envelope: i32,
    pub secondary_period: i32,
    pub secondary_arpeggio: i32,
    pub secondary_pitch: i32,
    pub retrig: bool,
}

impl Default for InstrumentCell {
    fn default() -> InstrumentCell {
        InstrumentCell { link: Link::NoSoftNoHard, volume: 0, noise: 0, primary_period: 0, primary_arpeggio: 0, primary_pitch: 0, ratio: DEFAULT_RATIO, envelope: DEFAULT_ENVELOPE, secondary_period: 0, secondary_arpeggio: 0, secondary_pitch: 0, retrig: false }
    }
}

/// An instrument cell as the player reads it.
#[derive(Clone, Copy, Debug, Default)]
struct LowCell {
    link: Link,
    volume: i32,
    noise: i32,
    software_period: i32,
    software_arpeggio: i32,
    software_pitch: i32,
    ratio: i32,
    envelope: i32,
    hardware_period: i32,
    hardware_arpeggio: i32,
    hardware_pitch: i32,
    retrig: bool,
}

/// An arpeggio as its note in the octave and its octave, kept within ten
/// octaves either way, as the editor corrects it.
fn correct_arpeggio(a: i32) -> i32 {
    let octave = if a >= 0 { a / 12 } else { -((-a - 1) / 12) - 1 };
    let note = a.rem_euclid(12);
    note + octave.clamp(-10, 10) * 12
}

impl InstrumentCell {
    /// The cell with each value within its range, as the editor corrects a loaded song.
    fn corrected(&self) -> InstrumentCell {
        let primary_soft = matches!(self.link, Link::NoSoftNoHard | Link::SoftOnly | Link::SoftToHard | Link::SoftAndHard);
        let (pmax, smax) = if primary_soft { (MAX_SOFTWARE_PERIOD, MAX_HARDWARE_PERIOD) } else { (MAX_HARDWARE_PERIOD, MAX_SOFTWARE_PERIOD) };
        InstrumentCell {
            link: self.link,
            volume: self.volume.clamp(0, MAX_VOLUME),
            noise: self.noise.clamp(0, 31),
            primary_period: self.primary_period.clamp(0, pmax),
            primary_arpeggio: correct_arpeggio(self.primary_arpeggio),
            primary_pitch: self.primary_pitch.clamp(-0xFFF, 0xFFF),
            ratio: self.ratio.clamp(0, 7),
            envelope: self.envelope.clamp(0, 15),
            secondary_period: self.secondary_period.clamp(0, smax),
            secondary_arpeggio: correct_arpeggio(self.secondary_arpeggio),
            secondary_pitch: self.secondary_pitch.clamp(-0xFFF, 0xFFF),
            retrig: self.retrig,
        }
    }

    fn low(&self) -> LowCell {
        let c = self;
        let (sp, sa, spi, hp, ha, hpi) = if c.link.is_software() || matches!(c.link, Link::SoftToHard | Link::SoftAndHard) {
            (c.primary_period.clamp(0, MAX_SOFTWARE_PERIOD), c.primary_arpeggio, c.primary_pitch, c.secondary_period.clamp(0, MAX_HARDWARE_PERIOD), c.secondary_arpeggio, c.secondary_pitch)
        } else {
            (c.secondary_period.clamp(0, MAX_SOFTWARE_PERIOD), c.secondary_arpeggio, c.secondary_pitch, c.primary_period.clamp(0, MAX_HARDWARE_PERIOD), c.primary_arpeggio, c.primary_pitch)
        };
        LowCell { link: c.link, volume: c.volume, noise: c.noise, software_period: sp, software_arpeggio: sa, software_pitch: spi, ratio: c.ratio, envelope: c.envelope, hardware_period: hp, hardware_arpeggio: ha, hardware_pitch: hpi, retrig: c.retrig }
    }
}

/// A PSG of a subsong: its clock, A's frequency, whether it's a YM2149,
/// the rate its samples play at, and where its channels sit in stereo.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Psg {
    pub hz: i32,
    pub reference: f64,
    pub ym: bool,
    pub sample_hz: i32,
    pub pan: [f32; 3],
}

impl Psg {
    pub fn new(hz: i32, reference: f64) -> Psg {
        Psg { hz, reference, ym: false, sample_hz: 8000, pan: [0.2, 0.5, 0.8] }
    }

    /// The channels' places from Arkos Tracker's mixing output ("ABC",
    /// "ACB"... or all "left", "center" or "right").
    pub fn pan_from(s: &str) -> [f32; 3] {
        match s {
            "left" => [0.2; 3],
            "center" => [0.5; 3],
            "right" => [0.8; 3],
            _ if s.len() == 3 => {
                let mut pan = [0.2, 0.5, 0.8];
                for (k, ch) in s.bytes().enumerate() {
                    if let Some(c) = (ch as usize).checked_sub(b'A' as usize).filter(|&c| c < 3) {
                        pan[c] = [0.2, 0.5, 0.8][k];
                    }
                }
                pan
            }
            _ => [0.2, 0.5, 0.8],
        }
    }
}

/// A loop over steps: from `start` to `end` included, round again if `looping`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Loop {
    pub start: i32,
    pub end: i32,
    pub looping: bool,
}

impl Loop {
    fn checked(self, length: i32) -> Loop {
        let end = self.end.clamp(0, (length - 1).max(0));
        Loop { start: self.start.clamp(0, end), end, looping: self.looping }
    }
}

/// The parts of an instrument cell an auto-spread loop repeats.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    SoundType,
    Envelope,
    Noise,
    PrimaryPeriod,
    PrimaryArpeggioNote,
    PrimaryArpeggioOctave,
    PrimaryPitch,
    SecondaryPeriod,
    SecondaryArpeggioNote,
    SecondaryArpeggioOctave,
    SecondaryPitch,
}

impl Section {
    pub fn from_name(s: &str) -> Option<Section> {
        use Section::*;
        Some(match s {
            "soundType" => SoundType,
            "envelope" => Envelope,
            "noise" => Noise,
            "primaryPeriod" => PrimaryPeriod,
            "primaryArpeggioNoteInOctave" => PrimaryArpeggioNote,
            "primaryArpeggioOctave" => PrimaryArpeggioOctave,
            "primaryPitch" => PrimaryPitch,
            "secondaryPeriod" => SecondaryPeriod,
            "secondaryArpeggioNoteInOctave" => SecondaryArpeggioNote,
            "secondaryArpeggioOctave" => SecondaryArpeggioOctave,
            "secondaryPitch" => SecondaryPitch,
            _ => return None,
        })
    }
}

/// A sample instrument's sound: 8-bit unsigned, looping or not.
#[derive(Clone, Debug, Default)]
pub struct Sample {
    pub data: Arc<Vec<u8>>,
    pub sample_loop: Loop,
    pub amplification: f32,
    /// The note it plays at as a digidrum (an event).
    pub digi_note: i32,
}

#[derive(Clone, Debug, Default)]
pub struct Instrument {
    pub name: String,
    pub speed: i32,
    pub retrig: bool,
    pub main_loop: Loop,
    pub cells: Vec<InstrumentCell>,
    /// Auto-spread loops: past a loop's end, its section repeats it.
    pub spread: Vec<(Section, Loop)>,
    /// A sample instrument's sound (it has no PSG cells then).
    pub sample: Option<Sample>,
}

impl Instrument {
    /// Instrument 0: one silent step, as slow as can be.
    fn empty() -> Instrument {
        Instrument { name: "Empty".into(), speed: 255, retrig: false, main_loop: Loop { start: 0, end: 0, looping: true }, cells: vec![InstrumentCell::default()], ..Default::default() }
    }

    fn correct(&mut self) {
        if let Some(s) = self.sample.as_mut() {
            let len = s.data.len().max(1) as i32;
            s.sample_loop = s.sample_loop.checked(len);
            return;
        }
        if self.cells.is_empty() {
            self.cells.push(InstrumentCell::default());
        }
        self.speed = self.speed.clamp(0, 255);
        let len = self.cells.len() as i32;
        self.main_loop = self.main_loop.checked(len);
        for (_, l) in self.spread.iter_mut() {
            *l = l.checked(len);
        }
        for c in self.cells.iter_mut() {
            *c = c.corrected();
        }
    }

    /// Cell `i` as the player reads it, its auto-spread sections applied.
    fn cell_at(&self, i: usize) -> Option<InstrumentCell> {
        let original = *self.cells.get(i)?;
        if self.spread.is_empty() {
            return Some(original);
        }
        let from = |section: Section| -> InstrumentCell {
            match self.spread.iter().find(|s| s.0 == section) {
                Some((_, l)) if l.looping && i as i32 > l.end => {
                    let len = l.end - l.start + 1;
                    let t = (i as i32 - l.start) % len + l.start;
                    self.cells.get(t as usize).copied().unwrap_or(original)
                }
                _ => original,
            }
        };
        let split = |a: i32| (a.rem_euclid(12), if a >= 0 { a / 12 } else { -((-a - 1) / 12) - 1 });
        let (env, pan, pao, san, sao) = (from(Section::Envelope), from(Section::PrimaryArpeggioNote), from(Section::PrimaryArpeggioOctave), from(Section::SecondaryArpeggioNote), from(Section::SecondaryArpeggioOctave));
        Some(InstrumentCell {
            link: from(Section::SoundType).link,
            volume: env.volume,
            noise: from(Section::Noise).noise,
            primary_period: from(Section::PrimaryPeriod).primary_period,
            primary_arpeggio: split(pan.primary_arpeggio).0 + split(pao.primary_arpeggio).1 * 12,
            primary_pitch: from(Section::PrimaryPitch).primary_pitch,
            ratio: env.ratio,
            envelope: env.envelope,
            secondary_period: from(Section::SecondaryPeriod).secondary_period,
            secondary_arpeggio: split(san.secondary_arpeggio).0 + split(sao.secondary_arpeggio).1 * 12,
            secondary_pitch: from(Section::SecondaryPitch).secondary_pitch,
            retrig: env.retrig,
        })
    }
}

/// An arpeggio or pitch table: its values, looping from `loop_start` to `end` at `speed`.
#[derive(Clone, Debug, PartialEq)]
pub struct Expression {
    pub values: Vec<i32>,
    pub loop_start: i32,
    pub end: i32,
    pub speed: i32,
}

impl Expression {
    fn empty() -> Expression {
        Expression { values: vec![0], loop_start: 0, end: 0, speed: 0 }
    }

    fn value(&self, i: i32) -> i32 {
        usize::try_from(i).ok().and_then(|i| self.values.get(i)).copied().unwrap_or(0)
    }

    fn correct(&mut self) {
        if self.values.is_empty() {
            self.values.push(0);
        }
        let l = Loop { start: self.loop_start, end: self.end, looping: true }.checked(self.values.len() as i32);
        (self.loop_start, self.end) = (l.start, l.end);
        self.speed = self.speed.clamp(0, 255);
    }
}

/// A cell of a track.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Cell {
    pub note: Option<u8>,
    pub instrument: Option<u8>,
    /// Its effect slots (up to four), each an effect and its value.
    pub effects: Vec<Option<(Effect, i32)>>,
}

impl Cell {
    pub fn is_empty(&self) -> bool {
        self.note.is_none() && self.instrument.is_none() && self.effects.iter().all(|e| e.is_none())
    }

    fn add_effect(&mut self, e: Effect, v: i32) {
        if self.effects.len() < 4 {
            self.effects.push(Some((e, v)));
        }
    }
}

pub type Track = Vec<Cell>;
/// A speed or event track: a value on some rows.
pub type SpecialTrack = Vec<Option<i32>>;

#[derive(Clone, Debug, Default)]
pub struct Pattern {
    pub tracks: Vec<usize>,
    pub speed_track: usize,
    pub event_track: usize,
}

#[derive(Clone, Debug, Default)]
pub struct Position {
    pub pattern: usize,
    pub height: usize,
    pub transpositions: Vec<i32>,
}

#[derive(Clone, Debug)]
pub struct Subsong {
    pub name: String,
    pub initial_speed: i32,
    pub replay_hz: f64,
    pub digi_channel: usize,
    /// The PSGs, three channels each.
    pub psgs: Vec<Psg>,
    pub loop_start: usize,
    pub end: usize,
    pub positions: Vec<Position>,
    pub patterns: Vec<Pattern>,
    pub tracks: Vec<Track>,
    pub speed_tracks: Vec<SpecialTrack>,
    pub event_tracks: Vec<SpecialTrack>,
}

impl Subsong {
    fn new(name: &str) -> Subsong {
        Subsong { name: name.into(), initial_speed: 6, replay_hz: 50.0, digi_channel: 1, psgs: vec![Psg::new(CPC_HZ, 440.0)], loop_start: 0, end: 0, positions: Vec::new(), patterns: Vec::new(), tracks: Vec::new(), speed_tracks: Vec::new(), event_tracks: Vec::new() }
    }

    /// The clock and A's frequency of the PSG channel `c` is on.
    pub fn psg(&self, c: usize) -> (i32, f64) {
        self.psgs.get(c / 3).map_or((CPC_HZ, 440.0), |p| (p.hz, p.reference))
    }

    pub fn channels(&self) -> usize {
        self.psgs.len().max(1) * 3
    }

    pub fn position_height(&self, p: usize) -> usize {
        self.positions.get(p).map_or(64, |p| p.height)
    }

    fn pattern_of(&self, p: usize) -> Option<&Pattern> {
        self.positions.get(p).and_then(|p| self.patterns.get(p.pattern))
    }

    /// The cell at a position, row and channel (empty where there's none).
    pub fn cell(&self, p: usize, row: usize, c: usize) -> Option<&Cell> {
        let t = *self.pattern_of(p)?.tracks.get(c)?;
        self.tracks.get(t)?.get(row).filter(|c| !c.is_empty())
    }

    /// A speed (or event) track's value at a position and row.
    pub fn special(&self, speed: bool, p: usize, row: usize) -> Option<i32> {
        let pat = self.pattern_of(p)?;
        let tracks = if speed { &self.speed_tracks } else { &self.event_tracks };
        *tracks.get(if speed { pat.speed_track } else { pat.event_track })?.get(row)?
    }

    fn latest_special(&self, speed: bool, p: usize, row: usize) -> Option<i32> {
        (0..=row).rev().find_map(|r| self.special(speed, p, r))
    }

    /// The speed at a position and row, as the editor works it out to play from there.
    fn speed_at(&self, p: usize, row: usize) -> i32 {
        let mut speed = self.initial_speed;
        for q in 0..=p.min(self.positions.len().saturating_sub(1)) {
            let r = if q == p { row } else { self.position_height(q).saturating_sub(1) };
            if let Some(s) = self.latest_special(true, q, r) {
                speed = s;
            }
        }
        speed
    }
}

/// An Arkos Tracker song.
#[derive(Clone, Debug)]
pub struct Song {
    pub title: String,
    pub author: String,
    pub composer: String,
    pub comments: String,
    /// What it was read from: "sks", "aks1" or "aks2".
    pub kind: &'static str,
    pub subsongs: Vec<Subsong>,
    pub instruments: Vec<Instrument>,
    pub arpeggios: Vec<Expression>,
    pub pitches: Vec<Expression>,
    file: Arc<Vec<u8>>,
}

pub fn probe_sks(d: &[u8]) -> bool {
    strip_amsdos(d).starts_with(b"STK1.0SONG")
}

/// The data past an Amsdos header (128 bytes, its checksum right), if there is one.
fn strip_amsdos(d: &[u8]) -> &[u8] {
    if d.len() >= 128 {
        let sum: u32 = d[..67].iter().map(|&b| u32::from(b)).sum();
        if sum & 0xFFFF == u32::from(u16::from_le_bytes([d[67], d[68]])) && sum != 0 {
            return &d[128..];
        }
    }
    d
}

/// Builds a song as Arkos Tracker's SongBuilder does: instrument and
/// expression 0 made first, holes filled, everything corrected.
struct Builder {
    instruments: Vec<Option<Instrument>>,
    arpeggios: Vec<Option<Expression>>,
    pitches: Vec<Option<Expression>>,
}

impl Builder {
    fn new() -> Builder {
        Builder { instruments: vec![Some(Instrument::empty())], arpeggios: vec![Some(Expression::empty())], pitches: vec![Some(Expression::empty())] }
    }

    fn put<T>(v: &mut Vec<Option<T>>, i: usize, x: T) -> bool {
        if i >= v.len() {
            v.resize_with(i + 1, || None);
        }
        if v[i].is_some() {
            return false;
        }
        v[i] = Some(x);
        true
    }

    fn instrument(&mut self, i: usize, ins: Instrument) {
        // Instrument 0 can't be replaced.
        if i > 0 {
            Builder::put(&mut self.instruments, i, ins);
        }
    }

    fn expression(&mut self, arpeggio: bool, i: usize, e: Expression) {
        if i > 0 {
            Builder::put(if arpeggio { &mut self.arpeggios } else { &mut self.pitches }, i, e);
        }
    }

    fn finish(self) -> (Vec<Instrument>, Vec<Expression>, Vec<Expression>) {
        let instruments = self
            .instruments
            .into_iter()
            .map(|i| {
                let mut i = i.unwrap_or_else(Instrument::empty);
                i.correct();
                i
            })
            .collect();
        let exprs = |v: Vec<Option<Expression>>| {
            v.into_iter()
                .map(|e| {
                    let mut e = e.unwrap_or_else(Expression::empty);
                    e.correct();
                    e
                })
                .collect()
        };
        (instruments, exprs(self.arpeggios), exprs(self.pitches))
    }
}

/// A subsong's tracks as they're declared, any holes left empty.
fn place<T: Default + Clone>(v: &mut Vec<T>, i: usize, x: T) {
    if i >= v.len() {
        v.resize(i + 1, T::default());
    }
    v[i] = x;
}

/// The trailing effects the importers keep, so that an effect already
/// running isn't written again (Arkos Tracker's TrailingEffectContext).
#[derive(Default)]
struct Trailing {
    volume: Option<i32>,
    pitch_up: Option<i32>,
    pitch_down: Option<i32>,
}

#[derive(Clone, Copy, PartialEq)]
enum Trail {
    None,
    PitchUp,
    PitchDown,
    Volume,
}

impl Trailing {
    fn slot(&mut self, t: Trail) -> &mut Option<i32> {
        match t {
            Trail::PitchUp => &mut self.pitch_up,
            Trail::PitchDown => &mut self.pitch_down,
            _ => &mut self.volume,
        }
    }

    fn on(&mut self, t: Trail, v: i32) -> (Trail, i32) {
        let s = self.slot(t);
        if *s == Some(v) {
            return (Trail::None, 0);
        }
        *s = Some(v);
        (t, v)
    }

    fn declare(&mut self, t: Trail, v: i32) -> (Trail, i32) {
        match t {
            Trail::None => {
                if self.pitch_up.take().is_some() {
                    return (Trail::PitchUp, 0);
                }
                if self.pitch_down.take().is_some() {
                    return (Trail::PitchDown, 0);
                }
                (Trail::None, 0)
            }
            Trail::PitchUp => {
                self.pitch_down = None;
                self.on(t, v)
            }
            Trail::PitchDown => {
                self.pitch_up = None;
                self.on(t, v)
            }
            _ => self.on(t, v),
        }
    }

    fn note(&mut self) {
        self.pitch_up = None;
        self.pitch_down = None;
    }
}

struct Reader<'a> {
    d: &'a [u8],
    at: usize,
    ok: bool,
}

impl Reader<'_> {
    fn u8(&mut self) -> u8 {
        let v = self.d.get(self.at).copied();
        self.at += 1;
        self.ok &= v.is_some();
        v.unwrap_or(0)
    }
    fn i8(&mut self) -> i32 {
        i32::from(self.u8() as i8)
    }
    fn u16(&mut self) -> u16 {
        u16::from(self.u8()) | u16::from(self.u8()) << 8
    }
    fn i16(&mut self) -> i32 {
        i32::from(self.u16() as i16)
    }
    fn text(&self, at: usize, n: usize) -> String {
        let s = self.d.get(at..at + n).unwrap_or(&[]);
        s.iter().map(|&c| if (32..127).contains(&c) { c as char } else { ' ' }).collect::<String>().trim().to_string()
    }
}

impl Song {
    /// Reads a STarKos song.
    pub fn parse_sks(file: &[u8]) -> Option<Song> {
        let d = strip_amsdos(file);
        if !d.starts_with(b"STK1.0SONG") || d.len() < 60 {
            return None;
        }
        let mut r = Reader { d, at: 52, ok: true };
        let author = r.text(10, 10);
        let comments = r.text(20, 32);
        let digi = i32::from(r.u8()) - 1;
        let end = usize::from(r.u8());
        let loop_start = usize::from(r.u8());
        r.u8();
        let speed = i32::from(r.u8());
        let freq = r.u8();
        let replay_hz = match freq {
            0 => 12.5,
            1 => 25.0,
            3 => 100.0,
            4 => 150.0,
            5 => 300.0,
            _ => 50.0,
        };
        let mut sub = Subsong::new("Main");
        sub.initial_speed = speed;
        sub.replay_hz = replay_hz;
        sub.digi_channel = digi.clamp(0, 2) as usize;
        sub.loop_start = loop_start;
        sub.end = end;
        let mut b = Builder::new();
        r.at = 58;
        // The linker: positions, each its three tracks and transpositions, height and special track.
        let last = usize::from(r.u8());
        r.u8();
        let mut known: Vec<(Vec<usize>, usize)> = Vec::new();
        for _ in 0..=last {
            let mut tracks = Vec::new();
            let mut trans = Vec::new();
            for _ in 0..3 {
                let b1 = r.u8();
                let b2 = r.u8();
                tracks.push(usize::from(b1) | usize::from(b2 & 1) << 8);
                trans.push(i32::from(b2 as i8) >> 1);
            }
            let height = usize::from(r.u8());
            let special = usize::from(r.u8());
            if height > 127 || !r.ok {
                return None;
            }
            let key = (tracks.clone(), special);
            let pattern = match known.iter().position(|k| *k == key) {
                Some(i) => i,
                None => {
                    known.push(key);
                    sub.patterns.push(Pattern { tracks, speed_track: special, event_track: special });
                    known.len() - 1
                }
            };
            sub.positions.push(Position { pattern, height: height + 1, transpositions: trans });
        }
        // Instruments.
        loop {
            let index = r.u16();
            if index == 0xFFFF || !r.ok {
                break;
            }
            r.at += 4;
            let speed = i32::from(r.u8());
            let retrig = r.u8() != 0;
            let end = i32::from(r.u8());
            let loop_start = i32::from(r.u8());
            let looping = r.u8() != 0;
            let name = r.text(r.at, 8);
            r.at += 8;
            let mut ins = Instrument { name, speed, retrig, main_loop: Loop { start: loop_start, end, looping }, cells: Vec::new(), ..Default::default() };
            for _ in 0..=end {
                ins.cells.push(sks_instrument_cell(&mut r));
            }
            if !r.ok {
                return None;
            }
            b.instrument(usize::from(index), ins);
        }
        // Special tracks: a speed and an event track each.
        loop {
            let index = r.u16();
            if index == 0xFFFF || !r.ok {
                break;
            }
            let mut speed_t = vec![None; TRACK_ROWS];
            let mut event_t = vec![None; TRACK_ROWS];
            r.u8();
            let mut line = 0;
            while line < TRACK_ROWS {
                let x = r.u8();
                if x == 0xFF || !r.ok {
                    break;
                }
                if x & 0x80 != 0 {
                    line += usize::from(x & 0x7F) + 1;
                } else {
                    let v = i32::from(x & 0x1F);
                    if x & 0x40 != 0 {
                        event_t[line] = Some(v);
                    } else {
                        speed_t[line] = Some(v);
                    }
                    line += 1;
                }
            }
            place(&mut sub.speed_tracks, usize::from(index), speed_t);
            place(&mut sub.event_tracks, usize::from(index), event_t);
        }
        // Tracks.
        loop {
            let index = r.u16();
            if index == 0xFFFF || !r.ok {
                break;
            }
            r.at += 2;
            let track = sks_track(&mut r);
            place(&mut sub.tracks, usize::from(index), track);
        }
        sks_fix(&mut sub);
        let (instruments, arpeggios, pitches) = b.finish();
        Some(Song { title: "Untitled".into(), author, composer: String::new(), comments, kind: "sks", subsongs: vec![sub], instruments, arpeggios, pitches, file: Arc::new(file.to_vec()) })
    }
}

/// Makes every track a subsong's patterns name exist, full height.
fn sks_fix(sub: &mut Subsong) {
    let (mut nt, mut ns) = (sub.tracks.len(), sub.speed_tracks.len().max(sub.event_tracks.len()));
    for p in &sub.patterns {
        nt = nt.max(p.tracks.iter().copied().max().map_or(0, |m| m + 1));
        ns = ns.max(p.speed_track.max(p.event_track) + 1);
    }
    sub.tracks.resize(nt, Vec::new());
    for t in sub.tracks.iter_mut() {
        t.resize(TRACK_ROWS, Cell::default());
    }
    sub.speed_tracks.resize(ns, Vec::new());
    sub.event_tracks.resize(ns, Vec::new());
    for t in sub.speed_tracks.iter_mut().chain(sub.event_tracks.iter_mut()) {
        t.resize(TRACK_ROWS, None);
    }
    if sub.positions.is_empty() {
        sub.positions.push(Position { pattern: 0, height: 64, transpositions: vec![0; 3] });
        sub.patterns.push(Pattern { tracks: vec![0, 0, 0], speed_track: 0, event_track: 0 });
        sub.tracks.resize(1, vec![Cell::default(); TRACK_ROWS]);
        sub.speed_tracks.resize(1, vec![None; TRACK_ROWS]);
        sub.event_tracks.resize(1, vec![None; TRACK_ROWS]);
    }
    sub.end = sub.end.min(sub.positions.len() - 1);
    sub.loop_start = sub.loop_start.min(sub.end);
}

fn sks_instrument_cell(r: &mut Reader) -> InstrumentCell {
    let mut c = InstrumentCell::default();
    let first = r.u8();
    if first & 0x80 == 0 {
        // Software.
        let mut manual = false;
        let mut sound = true;
        if first & 0x10 == 0 {
            if first & 0x0F == 0 {
                c.link = Link::NoSoftNoHard;
            } else {
                c.link = Link::SoftOnly;
                c.volume = i32::from(first & 0x0F);
            }
        } else {
            let second = r.u8();
            let noise = i32::from(second & 0x1F);
            if noise != 0 {
                c.noise = noise;
            }
            sound = second & 0x20 != 0;
            c.volume = if sound || noise != 0 { i32::from(first & 0x0F) } else { 0 };
            manual = second & 0x40 != 0;
        }
        if manual {
            c.primary_period = i32::from(r.u16());
        } else {
            c.primary_period = 0;
            if first & 0x20 != 0 {
                c.primary_arpeggio = r.i8();
            }
            if first & 0x40 != 0 {
                c.primary_pitch = -r.i16();
            }
        }
        c.link = if sound { Link::SoftOnly } else { Link::NoSoftNoHard };
        return c;
    }
    // Hardware.
    let second = r.u8();
    let mut link = Link::SoftToHard;
    let manual_soft = first & 0x10 != 0;
    let manual_hard = first & 0x20 != 0;
    c.retrig = first & 0x40 != 0;
    c.noise = if first & 0x08 != 0 { i32::from(r.u8()) } else { 0 };
    let sound = first & 0x01 == 0;
    c.envelope = i32::from(second & 3) * 2 + 8;
    let mut soft_arp = 0;
    if !manual_soft {
        if first & 0x02 != 0 {
            soft_arp = r.i8();
            c.primary_arpeggio = soft_arp;
        }
        if first & 0x04 != 0 {
            c.primary_pitch = -r.i16();
        }
    } else {
        c.primary_period = i32::from(r.u16());
    }
    let mut ratio = 0;
    if !manual_hard {
        ratio = 7 - i32::from((second >> 2) & 7);
        c.ratio = ratio;
    } else {
        link = Link::SoftAndHard;
        c.secondary_period = i32::from(r.u16());
    }
    if sound && !manual_hard && second & 0x80 != 0 {
        link = Link::HardToSoft;
        c.primary_arpeggio = ratio * 12 + soft_arp;
        c.secondary_arpeggio = 0;
    }
    if !sound {
        link = Link::HardOnly;
    }
    if second & 0x40 != 0 {
        let fine = r.i8();
        if link == Link::SoftToHard {
            c.primary_pitch = fine;
        } else if link == Link::HardToSoft {
            c.secondary_pitch = fine;
        }
    }
    c.link = link;
    c
}

fn sks_track(r: &mut Reader) -> Track {
    const VOLUME_ONLY: u8 = 96;
    const PITCH_ONLY: u8 = 97;
    const VOLUME_AND_PITCH: u8 = 98;
    const RESET: u8 = 99;
    const DIGIDRUM: u8 = 100;
    let mut t = vec![Cell::default(); TRACK_ROWS];
    let mut ctx = Trailing::default();
    let mut line = 0;
    let mut instrument = 0u8;
    while line < TRACK_ROWS {
        let x = r.u8();
        if x == 0xFF || !r.ok {
            break;
        }
        if x & 0x80 != 0 {
            let (e, _) = ctx.declare(Trail::None, 0);
            if matches!(e, Trail::PitchUp | Trail::PitchDown) {
                t[line].add_effect(if e == Trail::PitchUp { Effect::FastPitchUp } else { Effect::FastPitchDown }, 0);
            }
            line += usize::from(x & 0x7F) + 1;
            continue;
        }
        let cell = &mut t[line];
        let (mut read_ins, mut read_vol, mut read_pitch, mut read_note, mut read_flags) = (false, false, false, false, true);
        match x {
            RESET => {
                cell.note = Some(RST_NOTE);
                cell.instrument = Some(0);
            }
            DIGIDRUM => r.at += 1,
            VOLUME_ONLY => read_vol = true,
            PITCH_ONLY => {
                read_pitch = true;
                read_flags = false;
            }
            VOLUME_AND_PITCH => {
                read_vol = true;
                read_pitch = true;
            }
            _ => {
                read_note = true;
                read_ins = true;
            }
        }
        if read_note || read_pitch || read_vol {
            if read_note {
                ctx.note();
                cell.note = Some(x);
            }
            let mut flags = 0u8;
            if read_flags {
                flags = r.u8();
                read_vol |= flags & 0x40 == 0;
                read_pitch |= flags & 0x10 != 0;
            }
            if read_vol {
                let v = 0xF - i32::from(flags & 0xF);
                let (e, v) = ctx.declare(Trail::Volume, v);
                if e == Trail::Volume {
                    cell.add_effect(Effect::Volume, v);
                }
            }
            if read_ins {
                if flags & 0x20 == 0 {
                    instrument = r.u8();
                }
                cell.instrument = Some(instrument);
            }
            if read_pitch {
                let mut pitch = r.i8() * 8;
                let (effect, trail) = if pitch >= 0 {
                    (Effect::FastPitchDown, Trail::PitchDown)
                } else {
                    pitch = -pitch;
                    (Effect::FastPitchUp, Trail::PitchUp)
                };
                let (e, v) = ctx.declare(trail, pitch & 0xFFF);
                if matches!(e, Trail::PitchUp | Trail::PitchDown) {
                    cell.add_effect(effect, v);
                }
            } else if !read_note && !read_ins {
                let (e, _) = ctx.declare(Trail::None, 0);
                if matches!(e, Trail::PitchUp | Trail::PitchDown) {
                    cell.add_effect(if e == Trail::PitchUp { Effect::FastPitchUp } else { Effect::FastPitchDown }, 0);
                }
            }
        }
        line += 1;
    }
    t
}

impl Tune for Song {
    fn format(&self) -> &'static str {
        "arkos"
    }
    fn file(&self) -> &[u8] {
        &self.file
    }
    fn songs(&self) -> usize {
        self.subsongs.len().max(1)
    }
    fn driver(&self, song: usize) -> Box<dyn Driver> {
        Box::new(Player::new(Arc::new(self.clone()), song))
    }

    /// Each instrument but the empty one: its speed, retrig, loops and
    /// cells (each its link and values), or a sample instrument's sound.
    fn patches(&self) -> Vec<Patch> {
        let mut out = Vec::new();
        for (k, i) in self.instruments.iter().enumerate().skip(1) {
            // A new instrument's silent step (the editor's "Empty") makes no sound.
            if i.sample.is_none() && i.cells.iter().all(|c| c.volume == 0 && c.link == Link::NoSoftNoHard) {
                continue;
            }
            let mut b = PatchBytes::default();
            let lp = |b: &mut PatchBytes, l: &Loop| {
                b.i32(l.start).i32(l.end).u8(u8::from(l.looping));
            };
            if let Some(smp) = &i.sample {
                b.u8(1);
                lp(&mut b, &smp.sample_loop);
                b.f32(smp.amplification).i32(smp.digi_note).bytes(&smp.data);
            } else {
                b.u8(0).i32(i.speed).u8(u8::from(i.retrig));
                lp(&mut b, &i.main_loop);
                b.u16(i.cells.len() as u16);
                for c in &i.cells {
                    b.u8(c.link as u8);
                    for v in [c.volume, c.noise, c.primary_period, c.primary_arpeggio, c.primary_pitch, c.ratio, c.envelope, c.secondary_period, c.secondary_arpeggio, c.secondary_pitch] {
                        b.i32(v);
                    }
                    b.u8(u8::from(c.retrig));
                }
                b.u16(i.spread.len() as u16);
                for (section, l) in &i.spread {
                    b.u8(*section as u8);
                    lp(&mut b, l);
                }
            }
            out.push(b.patch(k, &i.name));
        }
        out
    }
}

impl Song {
    /// Reads any of the song formats Arkos Tracker's player plays.
    pub fn parse(d: &[u8]) -> Option<Song> {
        if probe_sks(d) {
            return Song::parse_sks(d);
        }
        crate::arkos_xml::parse(d)
    }

    pub(crate) fn from_parts(title: String, author: String, composer: String, comments: String, kind: &'static str, subsongs: Vec<Subsong>, b: XmlBuilt, file: &[u8]) -> Song {
        let (instruments, arpeggios, pitches) = b.0.finish();
        Song { title, author, composer, comments, kind, subsongs, instruments, arpeggios, pitches, file: Arc::new(file.to_vec()) }
    }
}

/// The instruments and tables an XML importer gathered.
pub(crate) struct XmlBuilt(Builder);

impl XmlBuilt {
    pub(crate) fn new() -> XmlBuilt {
        XmlBuilt(Builder::new())
    }
    pub(crate) fn instrument(&mut self, i: usize, ins: Instrument) {
        self.0.instrument(i, ins);
    }
    pub(crate) fn expression(&mut self, arpeggio: bool, i: usize, e: Expression) {
        self.0.expression(arpeggio, i, e);
    }
    /// Every instrument and expression at once, from index 0 (Arkos Tracker 3's own songs).
    pub(crate) fn replace(&mut self, instruments: Vec<Instrument>, arpeggios: Vec<Expression>, pitches: Vec<Expression>) {
        if !instruments.is_empty() {
            self.0.instruments = instruments.into_iter().map(Some).collect();
        }
        if !arpeggios.is_empty() {
            self.0.arpeggios = arpeggios.into_iter().map(Some).collect();
        }
        if !pitches.is_empty() {
            self.0.pitches = pitches.into_iter().map(Some).collect();
        }
    }
}

pub(crate) fn finish_subsong(sub: &mut Subsong) {
    sks_fix(sub);
}

pub(crate) fn new_subsong(name: &str) -> Subsong {
    Subsong::new(name)
}

// ---- The player ----------------------------------------------------------

/// Arkos Tracker's fixed-point number: a 16-bit whole part and an 8-bit fraction.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Fp {
    int: u16,
    dec: i32,
}

impl Fp {
    fn new(int: u16) -> Fp {
        Fp { int, dec: 0 }
    }
    fn from_digits(d: i32) -> Fp {
        let d = d as u32;
        Fp { int: ((d >> 8) & 0xFF) as u16, dec: (d & 0xFF) as i32 }
    }
    fn add(self, o: Fp) -> Fp {
        let mut int = self.int.wrapping_add(o.int);
        let dec = self.dec + o.dec;
        int = int.wrapping_add(((dec as u32) / 0x100) as u16);
        Fp { int, dec: ((dec as u32) & 0xFF) as i32 }
    }
    fn sub(self, o: Fp) -> Fp {
        let mut int = self.int.wrapping_sub(o.int);
        let dec = self.dec - o.dec;
        int = int.wrapping_add(((dec as u32) / 0x100) as u16);
        Fp { int, dec: ((dec as u32) & 0xFF) as i32 }
    }
    fn negate(self) -> Fp {
        Fp::default().sub(self)
    }
    fn correct_to(&mut self, max: u16) {
        if self.int & 0x8000 != 0 {
            *self = Fp::default();
        } else if self.int > max {
            *self = Fp::new(max);
        }
    }
}

fn restrict_loop(n: i32, high: i32) -> i32 {
    if n > high {
        n % (high + 1)
    } else if n < 0 {
        let p = -n % (high + 1);
        if p == 0 { 0 } else { high + 1 - p }
    } else {
        n
    }
}

fn correct_note(n: i32) -> i32 {
    restrict_loop(n, NOTE_MAX)
}

/// The PSG period of a note (0 to 127, C-0 first) for a PSG clock and A's frequency.
pub fn period(psg_hz: i32, reference_hz: f64, note: i32) -> i32 {
    let note = note.clamp(0, NOTE_MAX);
    let divider = f64::from(psg_hz) / 8.0;
    let octave = note / 12 - 3;
    let in_octave = note % 12 + 1;
    let f = reference_hz * 2f64.powf(f64::from(octave) + (f64::from(in_octave) - 10.0) / 12.0);
    (divider / f).round() as i32
}

/// The frequency a sample plays a note at (A's frequency at note 81, as
/// Arkos Tracker's tempered scale for samples has it).
fn sample_frequency(note: i32, reference: f64) -> f64 {
    reference / 32.0 * 2f64.powf((f64::from(note) - 12.0) / 12.0)
}

/// What a channel asks of its sample player this frame.
#[derive(Clone, Copy, Debug)]
struct SampleInfo {
    instrument: usize,
    from_start: bool,
    volume: i32,
    pitch: f64,
    reference: f64,
    /// A digidrum: it plays to its end over whatever the channel plays, once.
    high: bool,
}

/// A channel's sample player (Arkos Tracker's PsgStreamGenerator's).
#[derive(Clone, Copy, Debug, Default)]
struct Voice {
    info: Option<SampleInfo>,
    playing: bool,
    index: f64,
}

/// A channel's state (Arkos Tracker's ChannelPlayer).
#[derive(Clone)]
struct Channel {
    base_note: i32,
    track_note: i32,
    noise: i32,
    sound_open: bool,
    track_pitch: Fp,
    track_pitch_slide: Fp,
    track_volume: Fp,
    track_volume_slide: Fp,
    glide: bool,
    glide_goal: u16,
    glide_final_pitch: u16,
    glide_initial: u16,
    glide_increasing: bool,
    pitch_from_table: i32,
    instrument: Option<usize>,
    volume: i32,
    ins_line: i32,
    ins_tick: i32,
    forced_ins_speed: Option<i32>,
    envelope: i32,
    retrig: bool,
    hardware_period: i32,
    software_period: i32,
    inline_arp: Expression,
    use_inline: bool,
    arp_table: Option<usize>,
    arp_step: i32,
    arp_tick: i32,
    arp_speed: i32,
    arp_end: i32,
    arp_loop: i32,
    forced_arp_speed: i32,
    pitch_table: Option<usize>,
    pitch_step: i32,
    pitch_tick: i32,
    pitch_speed: i32,
    pitch_end: i32,
    pitch_loop: i32,
    forced_pitch_speed: i32,
    /// A note was read this frame.
    new_note: bool,
    /// The sample instrument playing, if any, and whether it starts again.
    sample: Option<usize>,
    sample_restarted: bool,
}

impl Channel {
    fn new() -> Channel {
        Channel {
            base_note: 0,
            track_note: 0,
            noise: 0,
            sound_open: false,
            track_pitch: Fp::default(),
            track_pitch_slide: Fp::default(),
            track_volume: Fp::new(15),
            track_volume_slide: Fp::default(),
            glide: false,
            glide_goal: 0,
            glide_final_pitch: 0,
            glide_initial: 0,
            glide_increasing: false,
            pitch_from_table: 0,
            instrument: None,
            volume: 0,
            ins_line: 0,
            ins_tick: 0,
            forced_ins_speed: None,
            envelope: DEFAULT_ENVELOPE,
            retrig: false,
            hardware_period: 0,
            software_period: 0,
            inline_arp: Expression::empty(),
            use_inline: false,
            arp_table: None,
            arp_step: 0,
            arp_tick: 0,
            arp_speed: 0,
            arp_end: 0,
            arp_loop: 0,
            forced_arp_speed: -1,
            pitch_table: None,
            pitch_step: 0,
            pitch_tick: 0,
            pitch_speed: 0,
            pitch_end: 0,
            pitch_loop: 0,
            forced_pitch_speed: -1,
            new_note: false,
            sample: None,
            sample_restarted: false,
        }
    }
}

/// What a channel sets on the PSG.
#[derive(Clone, Copy, Default)]
struct ChannelOut {
    volume: i32,
    noise: i32,
    sound: bool,
    software_period: i32,
    hardware_period: i32,
    envelope: i32,
    retrig: bool,
}

/// An effect's slots with those Arkos Tracker would refuse taken out (CellEffectsChecker).
fn checked_effects(cell: &Cell) -> Vec<(Effect, i32)> {
    use Effect::*;
    let note = cell.note.is_some();
    let ins = cell.instrument.is_some();
    let slots: Vec<(usize, Effect, i32)> = cell.effects.iter().enumerate().filter_map(|(i, e)| e.map(|(e, v)| (i, e, v))).collect();
    let reset_at = slots.iter().find(|s| s.1 == Reset).map(|s| s.0);
    let mut bad = vec![false; cell.effects.len()];
    if let Some(r) = reset_at {
        for s in &slots {
            if s.0 < r {
                bad[s.0] = true;
            }
        }
    }
    let (mut vol, mut volio, mut pitch, mut ptable, mut arp, mut reset, mut legato, mut farp, mut fins, mut fpitch) = (0, 0, 0, 0, 0, 0, 0, 0, 0, 0);
    for &(i, e, v) in &slots {
        let zero = v == 0;
        let reset_found = reset > 0;
        let zr = reset_found && zero;
        let glide = e == PitchGlide;
        let err = match e {
            Legato => {
                legato += 1;
                legato > 1
            }
            Reset => {
                reset += 1;
                reset > 1
            }
            Volume => {
                vol += 1;
                vol > 1 || volio > 0 || reset > 0
            }
            VolumeIn | VolumeOut => {
                volio += 1;
                volio > 1 || zr
            }
            Arpeggio3Notes | Arpeggio4Notes | ArpeggioTable => {
                arp += 1;
                arp > 1 || farp > 0 || zr
            }
            PitchDown | PitchUp | FastPitchDown | FastPitchUp | PitchGlide => {
                pitch += 1;
                pitch > 1 || zr || (reset_found && glide) || (note && ins && glide) || (note && ins && zero) || (note && !ins && zero && glide)
            }
            PitchTable => {
                ptable += 1;
                ptable > 1 || fpitch > 0 || zr
            }
            ForceArpeggioSpeed => {
                farp += 1;
                farp > 1 || zr
            }
            ForceInstrumentSpeed => {
                fins += 1;
                fins > 1 || zr
            }
            ForcePitchTableSpeed => {
                fpitch += 1;
                fpitch > 1 || zr
            }
        };
        if err {
            bad[i] = true;
        }
    }
    slots.into_iter().filter(|s| !bad[s.0]).map(|s| (s.1, s.2)).collect()
}

struct Player {
    song: Arc<Song>,
    sub: usize,
    channels: Vec<Channel>,
    /// The PSGs' registers, kept from frame to frame.
    regs: Vec<[u8; 14]>,
    /// Whether R13 was written this frame (a retrig or a new shape).
    shape_written: Vec<bool>,
    /// The registers every PSG is filled in, and their retrig flag,
    /// which (as in Arkos Tracker) stays until a hardware sound sets it
    /// again; each PSG's R13 last frame.
    shared: [u8; 14],
    shared_retrig: bool,
    last_shape: Vec<i32>,
    ays: Vec<Ay>,
    voices: Vec<Voice>,
    position: usize,
    line: usize,
    tick: i32,
    speed: i32,
    first: bool,
    shown: (usize, usize),
    done: bool,
}

impl Player {
    fn new(song: Arc<Song>, sub: usize) -> Player {
        let sub = sub.min(song.subsongs.len().saturating_sub(1));
        let s = &song.subsongs[sub];
        let psgs = s.channels() / 3;
        let speed = s.speed_at(0, 0);
        let ays = (0..psgs).map(|k| {
            let p = s.psgs.get(k).copied().unwrap_or(Psg::new(CPC_HZ, 440.0));
            Ay::new(p.ym, f64::from(p.hz), 44100)
        }).collect();
        Player { voices: vec![Voice::default(); psgs * 3], channels: vec![Channel::new(); psgs * 3], regs: vec![empty_regs(); psgs], shape_written: vec![false; psgs], shared: empty_regs(), shared_retrig: false, last_shape: vec![16; psgs], ays, position: 0, line: 0, tick: 0, speed, first: true, shown: (0, 0), done: false, song, sub }
    }

    fn subsong(&self) -> &Subsong {
        &self.song.subsongs[self.sub]
    }

    /// One frame (SongPlayer::getNextRegisters, then advanceInSong).
    fn frame(&mut self) {
        let song = self.song.clone();
        let sub = &song.subsongs[self.sub];
        let first_frame = std::mem::replace(&mut self.first, false);
        if first_frame {
            // Playing starts with every sound stopped.
            for c in self.channels.iter_mut() {
                c.instrument = None;
                c.arp_table = None;
                c.pitch_table = None;
                c.track_volume = Fp::new(15);
                c.track_volume_slide = Fp::default();
                reset_inline_arpeggio(c);
            }
            self.shared = empty_regs();
            self.shared_retrig = false;
        }
        let first_tick = self.tick == 0;
        let within = self.tick < self.speed;
        let mut posted: Vec<Option<Cell>> = vec![None; self.channels.len()];
        let mut event = None;
        if first_tick {
            event = sub.special(false, self.position, self.line);
            self.shown = (self.position, self.line);
            if let Some(s) = sub.special(true, self.position, self.line) {
                self.speed = s;
            }
            for (c, p) in posted.iter_mut().enumerate() {
                *p = sub.cell(self.position, self.line, c).cloned();
            }
        }
        // Arkos Tracker fills one set of registers for every PSG in turn,
        // each PSG's taken as it stands after its channels: what a channel
        // leaves set (the noise, the envelope) carries into the next PSG.
        for c in 0..self.channels.len() {
            let trans = sub.positions.get(self.position).and_then(|p| p.transpositions.get(c)).copied().unwrap_or(0);
            let out = self.play_stream(c, posted[c].take(), trans, within, sub.psg(c));
            self.sample_info(&song, c, sub, event, first_frame);
            let (psg, ch) = (c / 3, c % 3);
            let r = &mut self.shared;
            r[ch * 2] = (out.software_period & 0xFF) as u8;
            r[ch * 2 + 1] = ((out.software_period >> 8) & 0xFF) as u8;
            if out.noise > 0 {
                r[6] = out.noise as u8;
            }
            set_bit(&mut r[7], ch + 3, out.noise > 0);
            set_bit(&mut r[7], ch, out.sound);
            r[8 + ch] = out.volume as u8;
            if out.volume == 16 {
                // The hardware envelope (set for hardware and soft-and-hard sounds).
                r[11] = (out.hardware_period & 0xFF) as u8;
                r[12] = ((out.hardware_period >> 8) & 0xFF) as u8;
                r[13] = out.envelope as u8;
                self.shared_retrig = out.retrig;
            }
            if ch == 2 {
                self.regs[psg] = self.shared;
                // R13 is written on a retrig or when it changes.
                let shape = i32::from(self.shared[13]);
                self.shape_written[psg] = self.shared_retrig || shape != self.last_shape[psg];
                self.last_shape[psg] = shape;
            }
        }
        self.advance(&song);
    }

    /// What channel `ci`'s sample player is told this frame: its own
    /// sample instrument, or the digidrum an event plays on the digi channel.
    fn sample_info(&mut self, song: &Song, ci: usize, sub: &Subsong, event: Option<i32>, _first: bool) {
        let c = &mut self.channels[ci];
        let reference = sub.psg(ci).1;
        let restarted = std::mem::take(&mut c.sample_restarted);
        let mut info = c.sample.map(|instrument| SampleInfo {
            instrument,
            from_start: restarted,
            volume: i32::from(c.track_volume.int),
            pitch: sample_frequency(c.track_note, reference) - f64::from(c.track_pitch.int as i16),
            reference,
            high: false,
        });
        if let Some(e) = event.filter(|_| ci == sub.digi_channel) {
            if let Some(smp) = usize::try_from(e).ok().and_then(|i| song.instruments.get(i)).and_then(|i| i.sample.as_ref()) {
                info = Some(SampleInfo { instrument: e as usize, from_start: true, volume: 15, pitch: sample_frequency(smp.digi_note, reference), reference, high: true });
            }
        }
        let v = &mut self.voices[ci];
        match info {
            None => {
                // A PSG note after a sample note is heard; a digidrum plays on.
                if v.playing && !v.info.is_some_and(|i| i.high) {
                    v.playing = false;
                }
            }
            Some(new) => {
                if v.playing && v.info.is_some_and(|i| i.high) && !new.high {
                    return;
                }
                let present = v.info.is_some();
                v.info = Some(new);
                if !present || new.from_start {
                    v.index = 0.0;
                }
                if new.from_start {
                    v.playing = true;
                }
            }
        }
    }

    fn advance(&mut self, song: &Song) {
        let sub = &song.subsongs[self.sub];
        self.tick += 1;
        if self.tick >= self.speed {
            self.tick = 0;
            self.line += 1;
            if self.line >= sub.position_height(self.position) {
                self.line = 0;
                self.position += 1;
            }
            if self.position > sub.end {
                self.position = sub.loop_start;
                self.line = 0;
                self.done = true;
            }
        }
    }

    fn play_stream(&mut self, ci: usize, cell: Option<Cell>, trans: i32, within: bool, psg: (i32, f64)) -> ChannelOut {
        let song = self.song.clone();
        let c = &mut self.channels[ci];
        c.new_note = false;
        if let Some(cell) = cell {
            read_cell(&song, psg, c, &cell, trans);
        }
        // Trailing effects.
        let arp = read_arpeggio(&song, c);
        c.track_note = correct_note(c.base_note + arp);
        if within {
            if c.glide {
                if c.glide_increasing {
                    c.track_pitch = c.track_pitch.add(c.track_pitch_slide);
                } else {
                    c.track_pitch = c.track_pitch.sub(c.track_pitch_slide);
                }
                let now = c.glide_initial.wrapping_add(c.track_pitch.int);
                if now == c.glide_goal || (c.glide_goal > now && !c.glide_increasing) || (c.glide_goal < now && c.glide_increasing) {
                    c.track_pitch = Fp::new(c.glide_final_pitch);
                    c.glide = false;
                    c.track_pitch_slide = Fp::default();
                }
            } else {
                c.track_pitch = c.track_pitch.add(c.track_pitch_slide);
            }
            c.track_volume = c.track_volume.add(c.track_volume_slide);
            c.track_volume.correct_to(MAX_VOLUME as u16);
        }
        c.pitch_from_table = read_pitch(&song, c);
        play_instrument(&song, psg, c)
    }
}

/// The level a playing sample puts on its channel now (0 to 31), and the
/// sample moved on by one output frame at `rate`.
fn sample_level(song: &Song, v: &mut Voice, sample_hz: i32, rate: u32) -> Option<u8> {
    if !v.playing {
        return None;
    }
    let info = v.info?;
    let Some(smp) = song.instruments.get(info.instrument).and_then(|i| i.sample.as_ref()) else {
        v.playing = false;
        return None;
    };
    let data = &smp.data;
    let Some(&raw) = data.get(v.index as usize) else {
        v.playing = false;
        return None;
    };
    let mut x = i32::from(raw);
    if smp.amplification != 1.0 {
        x = (((x as f32 - 128.0) * smp.amplification + 128.0) as i32).clamp(0, 255);
    }
    x = x * info.volume / 15;
    let v4 = (x.max(0) as u32 >> 4) as i32;
    let level = match v4 {
        0 => 0,
        16.. => 32,
        v => v * 2 + 1,
    };
    let step = f64::from(sample_hz) * (info.pitch / info.reference) / f64::from(rate.max(1));
    let past = data.len().min((smp.sample_loop.end + 1).max(0) as usize);
    let mut next = v.index + step;
    if next as usize >= past {
        next = f64::from(smp.sample_loop.start);
        v.playing = smp.sample_loop.looping && !info.high;
    }
    v.index = next;
    Some(level.min(31) as u8)
}

fn empty_regs() -> [u8; 14] {
    let mut r = [0u8; 14];
    r[7] = 0b11_1111;
    r[13] = 8;
    r
}

fn set_bit(r: &mut u8, bit: usize, open: bool) {
    if open {
        *r &= !(1 << bit);
    } else {
        *r |= 1 << bit;
    }
}

fn reset_inline_arpeggio(c: &mut Channel) {
    c.inline_arp = Expression::empty();
    c.arp_speed = 0;
    c.arp_step = 0;
    c.arp_tick = 0;
}

fn update_arp_meta(song: &Song, c: &mut Channel, reset_speed: bool) {
    if reset_speed {
        c.forced_arp_speed = -1;
    }
    let e = if c.use_inline { Some(&c.inline_arp) } else { c.arp_table.and_then(|i| song.arpeggios.get(i)) };
    (c.arp_loop, c.arp_end, c.arp_speed) = e.map_or((0, 0, 0), |e| (e.loop_start, e.end, e.speed));
    if c.forced_arp_speed >= 0 {
        c.arp_speed = c.forced_arp_speed;
    }
}

fn update_pitch_meta(song: &Song, c: &mut Channel, reset_speed: bool) {
    if reset_speed {
        c.forced_pitch_speed = -1;
    }
    let e = c.pitch_table.and_then(|i| song.pitches.get(i));
    (c.pitch_loop, c.pitch_end, c.pitch_speed) = e.map_or((0, 0, 0), |e| (e.loop_start, e.end, e.speed));
    if c.forced_pitch_speed >= 0 {
        c.pitch_speed = c.forced_pitch_speed;
    }
}

fn read_arpeggio(song: &Song, c: &mut Channel) -> i32 {
    update_arp_meta(song, c, false);
    if c.arp_step > c.arp_end {
        c.arp_step = 0;
    }
    let v = if c.use_inline { c.inline_arp.value(c.arp_step) } else { c.arp_table.and_then(|i| song.arpeggios.get(i)).map_or(0, |e| e.value(c.arp_step)) };
    c.arp_tick += 1;
    if c.arp_tick > c.arp_speed {
        c.arp_tick = 0;
        c.arp_step += 1;
        if c.arp_step > c.arp_end {
            c.arp_step = c.arp_loop;
        }
    }
    v
}

fn read_pitch(song: &Song, c: &mut Channel) -> i32 {
    update_pitch_meta(song, c, false);
    if c.pitch_step > c.pitch_end {
        c.pitch_step = 0;
    }
    let v = c.pitch_table.and_then(|i| song.pitches.get(i)).map_or(0, |e| e.value(c.pitch_step));
    c.pitch_tick += 1;
    if c.pitch_tick > c.pitch_speed {
        c.pitch_tick = 0;
        c.pitch_step += 1;
        if c.pitch_step > c.pitch_end {
            c.pitch_step = c.pitch_loop;
        }
    }
    v
}

/// The effect's value as a 3-digit number's nibbles.
fn nibble(v: i32, n: u32) -> i32 {
    (v >> (4 * n)) & 0xF
}

fn reset_effects(song: &Song, c: &mut Channel, inverted_volume: i32) {
    let _ = song;
    c.track_volume = Fp::new((MAX_VOLUME - (inverted_volume & 0xF)) as u16);
    c.track_volume_slide = Fp::default();
    c.track_pitch_slide = Fp::default();
    reset_inline_arpeggio(c);
    c.arp_table = Some(0);
    c.use_inline = false;
    c.pitch_table = Some(0);
    c.pitch_step = 0;
    c.pitch_tick = 0;
    c.pitch_speed = 0;
    c.pitch_end = 0;
    c.pitch_loop = 0;
}

fn read_cell(song: &Song, (psg_hz, ref_hz): (i32, f64), c: &mut Channel, cell: &Cell, trans: i32) {
    let effects = checked_effects(cell);
    let glide = effects.iter().find(|e| e.0 == Effect::PitchGlide).map(|e| e.1);
    if let Some(g) = glide {
        c.track_pitch_slide = Fp::from_digits(g);
        c.glide = true;
    }
    if let Some(n) = cell.note {
        let note = i32::from(n) + trans;
        c.new_note = true;
        if glide.is_some() {
            let goal = period(psg_hz, ref_hz, note);
            let base = period(psg_hz, ref_hz, c.base_note);
            c.glide_final_pitch = (goal - base) as u16;
            c.glide_goal = goal as u16;
            let pitched = (base as u32).wrapping_add(u32::from(c.track_pitch.int)) as u16;
            c.glide_initial = base as u16;
            c.glide_increasing = goal >= i32::from(pitched);
        } else {
            c.base_note = note;
            c.track_pitch = Fp::default();
            c.track_pitch_slide = Fp::default();
            c.glide = false;
            if let Some(ins) = cell.instrument {
                c.track_volume_slide = Fp::default();
                update_arp_meta(song, c, true);
                c.arp_step = 0;
                c.arp_tick = 0;
                update_pitch_meta(song, c, true);
                c.pitch_step = 0;
                c.pitch_tick = 0;
                c.ins_line = 0;
                c.ins_tick = 0;
                c.instrument = song.instruments.get(usize::from(ins)).map(|_| usize::from(ins));
                if c.instrument.and_then(|i| song.instruments.get(i)).is_some_and(|i| i.sample.is_some()) {
                    // A sample silences the channel's PSG sound.
                    c.sample_restarted = true;
                    c.noise = 0;
                    c.sound_open = false;
                    c.volume = 0;
                } else {
                    c.forced_ins_speed = None;
                }
            }
        }
    }
    for (e, v) in effects {
        match e {
            Effect::Volume => {
                c.track_volume = Fp::new(v as u16);
                c.track_volume_slide = Fp::default();
            }
            Effect::VolumeIn => c.track_volume_slide = Fp::from_digits(v),
            Effect::VolumeOut => c.track_volume_slide = Fp::from_digits(v).negate(),
            Effect::PitchUp => {
                c.track_pitch_slide = Fp::from_digits(v).negate();
                c.glide = false;
            }
            Effect::FastPitchUp => {
                c.track_pitch_slide = Fp::from_digits(v << 4).negate();
                c.glide = false;
            }
            Effect::PitchDown => {
                c.track_pitch_slide = Fp::from_digits(v);
                c.glide = false;
            }
            Effect::FastPitchDown => {
                c.track_pitch_slide = Fp::from_digits(v << 4);
                c.glide = false;
            }
            Effect::PitchTable => {
                c.pitch_table = Some(v as usize).filter(|&i| i < song.pitches.len());
                c.pitch_step = 0;
                c.pitch_tick = 0;
                update_pitch_meta(song, c, true);
            }
            Effect::Reset => reset_effects(song, c, v),
            Effect::ForceInstrumentSpeed => c.forced_ins_speed = Some(v),
            Effect::ForceArpeggioSpeed => c.forced_arp_speed = v,
            Effect::ForcePitchTableSpeed => c.forced_pitch_speed = v,
            Effect::Arpeggio3Notes => {
                reset_inline_arpeggio(c);
                c.inline_arp = Expression { values: vec![0, nibble(v, 1), nibble(v, 0)], loop_start: 0, end: 2, speed: 0 };
                c.use_inline = true;
                c.forced_arp_speed = -1;
            }
            Effect::Arpeggio4Notes => {
                reset_inline_arpeggio(c);
                c.inline_arp = Expression { values: vec![0, nibble(v, 2), nibble(v, 1), nibble(v, 0)], loop_start: 0, end: 3, speed: 0 };
                c.use_inline = true;
                c.forced_arp_speed = -1;
            }
            Effect::ArpeggioTable => {
                c.use_inline = false;
                c.arp_table = Some(v as usize).filter(|&i| i < song.arpeggios.len());
                c.arp_step = 0;
                c.arp_tick = 0;
                update_arp_meta(song, c, true);
            }
            Effect::PitchGlide | Effect::Legato => {}
        }
    }
}

fn play_instrument(song: &Song, (psg, refhz): (i32, f64), c: &mut Channel) -> ChannelOut {
    let ins = c.instrument.and_then(|i| song.instruments.get(i));
    if ins.is_some_and(|i| i.sample.is_some()) {
        // The sample plays on; the PSG part stays as the note left it.
        c.sample = c.instrument;
        return ChannelOut { volume: c.volume, noise: c.noise, sound: c.sound_open, software_period: c.software_period, hardware_period: c.hardware_period, envelope: c.envelope, retrig: c.retrig };
    }
    c.sample = None;
    let (main_loop, speed, ins_retrig, cell) = match ins {
        Some(i) => (i.main_loop, i.speed, i.retrig, i.cell_at(c.ins_line as usize).map(|x| x.low()).unwrap_or_default()),
        None => (Loop::default(), 0, false, LowCell { ratio: DEFAULT_RATIO, envelope: DEFAULT_ENVELOPE, ..Default::default() }),
    };
    c.retrig = c.new_note && c.ins_tick == 0 && ins_retrig;
    c.noise = cell.noise;
    let link = cell.link;
    c.sound_open = !matches!(link, Link::HardOnly | Link::NoSoftNoHard);
    if matches!(link, Link::NoSoftNoHard | Link::SoftOnly) {
        c.volume = cell.volume;
    } else {
        c.volume = 16;
        c.envelope = cell.envelope;
        if !c.retrig {
            c.retrig = cell.retrig && c.ins_tick == 0;
        }
    }
    let pitch = i32::from(c.track_pitch.int as i16);
    if matches!(link, Link::SoftOnly | Link::SoftAndHard | Link::SoftToHard) {
        if cell.software_period != 0 {
            c.software_period = cell.software_period;
        } else {
            let note = correct_note(c.track_note + cell.software_arpeggio);
            c.software_period = period(psg, refhz, note) + pitch - c.pitch_from_table - cell.software_pitch;
        }
    }
    if link == Link::SoftToHard {
        let r = cell.ratio;
        c.hardware_period = c.software_period >> r;
        if r > 0 && (c.software_period >> (r - 1)) & 1 != 0 {
            c.hardware_period += 1;
        }
        c.hardware_period -= cell.hardware_pitch;
    } else if matches!(link, Link::HardOnly | Link::HardToSoft | Link::SoftAndHard) {
        if cell.hardware_period != 0 {
            c.hardware_period = cell.hardware_period;
        } else {
            let note = c.track_note + cell.hardware_arpeggio;
            c.hardware_period = period(psg, refhz, note) + pitch - c.pitch_from_table - cell.hardware_pitch;
        }
    }
    if link == Link::HardToSoft {
        c.software_period = (c.hardware_period << cell.ratio) - cell.software_pitch;
    }
    c.software_period = restrict_loop(c.software_period, MAX_SOFTWARE_PERIOD);
    c.hardware_period = restrict_loop(c.hardware_period, MAX_HARDWARE_PERIOD);
    if c.volume <= MAX_VOLUME {
        c.volume = (c.volume - (MAX_VOLUME - i32::from(c.track_volume.int))).max(0);
    }
    // Advances in the instrument.
    let speed = c.forced_ins_speed.unwrap_or(speed);
    c.ins_tick += 1;
    if c.ins_tick > speed {
        c.ins_tick = 0;
        c.ins_line += 1;
        if c.ins_line > main_loop.end {
            if main_loop.looping {
                c.ins_line = main_loop.start;
            } else {
                c.ins_line = 0;
                c.instrument = None;
            }
        }
    }
    ChannelOut { volume: c.volume, noise: c.noise, sound: c.sound_open, software_period: c.software_period, hardware_period: c.hardware_period, envelope: c.envelope, retrig: c.retrig }
}

impl Driver for Player {
    fn channels(&self) -> usize {
        self.channels.len()
    }

    fn tick(&mut self) {
        self.shape_written.iter_mut().for_each(|w| *w = false);
        self.frame();
        for (p, ay) in self.ays.iter_mut().enumerate() {
            ay.write_regs(&self.regs[p], self.shape_written[p]);
        }
    }

    fn tick_seconds(&self) -> f64 {
        1.0 / self.subsong().replay_hz.max(1.0)
    }

    fn render(&mut self, rate: u32, frames: usize, out: &mut Bufs) {
        for ay in self.ays.iter_mut() {
            ay.set_rate(rate);
        }
        let song = self.song.clone();
        let sub = &song.subsongs[self.sub];
        // Several PSGs share the level one would have.
        let level = 0.6 / (self.ays.len().max(1) as f32).sqrt();
        for i in 0..frames {
            for (p, ay) in self.ays.iter_mut().enumerate() {
                let psg = sub.psgs.get(p).copied().unwrap_or(Psg::new(CPC_HZ, 440.0));
                for ch in 0..3 {
                    ay.sample[ch] = self.voices.get_mut(p * 3 + ch).and_then(|v| sample_level(&song, v, psg.sample_hz, rate));
                }
                ay.process(true);
                for ch in 0..3 {
                    let x = ay.out[ch] as f32 * level;
                    out.left[p * 3 + ch][i] = x * (1.0 - psg.pan[ch]);
                    out.right[p * 3 + ch][i] = x * psg.pan[ch];
                }
            }
        }
    }

    fn position(&self) -> (usize, usize) {
        self.shown
    }

    fn speed(&self) -> (u32, u32) {
        let hz = self.subsong().replay_hz;
        let bpm = hz * 60.0 / f64::from(self.speed.max(1)) / 4.0;
        (self.speed.max(1) as u32, bpm.round() as u32)
    }

    fn take_done(&mut self) -> bool {
        std::mem::take(&mut self.done)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn periods_and_numbers() {
        // A-4 (note 45: C-0, note 0, is MIDI's 24) at 440 Hz on the CPC's 1 MHz: 125000/440.
        assert_eq!(period(CPC_HZ, 440.0, 45), 284);
        assert_eq!(restrict_loop(-1, 0xFFF), 0xFFF);
        assert_eq!(restrict_loop(0x1001, 0xFFF), 1);
        assert_eq!(correct_arpeggio(-13), -13);
        assert_eq!(correct_arpeggio(200), 128);
        let a = Fp::from_digits(0x0180);
        assert_eq!(a.negate().add(a), Fp::default());
    }

    /// Registers from one PSG clock to another, as Arkos Tracker converts them.
    fn convert(r: &[u8; 14], from: i32, to: i32) -> [u8; 14] {
        let conv = |v: i32, max: i32| if v == 0 || from == to { v } else { ((f64::from(v) * (f64::from(to) / f64::from(from))) as i32).clamp(1, max) };
        let mut o = *r;
        for ch in 0..3 {
            let v = i32::from(r[ch * 2]) | i32::from(r[ch * 2 + 1]) << 8;
            let n = conv(v & 0xFFF, 0xFFF) | (v & 0xF000);
            (o[ch * 2], o[ch * 2 + 1]) = ((n & 0xFF) as u8, (n >> 8) as u8);
        }
        let h = conv(i32::from(r[11]) | i32::from(r[12]) << 8, 0xFFFF);
        (o[11], o[12]) = ((h & 0xFF) as u8, (h >> 8) as u8);
        o[6] = conv(i32::from(r[6] & 0x1F), 0x1F) as u8;
        o
    }

    /// Each frame's PSG registers against Arkos Tracker 3's own player
    /// (its SongToYm tool): ARKOS_DIR the songs, ARKOS_REF the dumps as
    /// `<name>.regs` (14 bytes a frame).
    #[test]
    #[ignore]
    fn against_reference() {
        let (Ok(dir), Ok(rf)) = (std::env::var("ARKOS_DIR"), std::env::var("ARKOS_REF")) else { return };
        let only = std::env::var("ARKOS_ONLY").ok();
        let mut files: Vec<std::path::PathBuf> = Vec::new();
        let mut stack = vec![std::path::PathBuf::from(dir)];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if !p.extension().is_some_and(|x| x == "src") {
                    files.push(p);
                }
            }
        }
        files.sort();
        let (mut same, mut total) = (0, 0);
        for f in files {
            let name = f.file_stem().unwrap().to_string_lossy().to_string();
            if only.as_ref().is_some_and(|o| !name.contains(o.as_str())) {
                continue;
            }
            let Ok(first) = std::fs::read(format!("{rf}/{name}.regs")) else { continue };
            // Each PSG's dump: the first, then `<name>.p2.regs` and on.
            let mut references = vec![first];
            while let Ok(r) = std::fs::read(format!("{rf}/{name}.p{}.regs", references.len() + 1)) {
                references.push(r);
            }
            let Some(song) = Song::parse(&std::fs::read(&f).unwrap()) else {
                println!("{name}: not read");
                total += 1;
                continue;
            };
            total += 1;
            let mut p = Player::new(Arc::new(song), 0);
            let mut bad = None;
            let show: usize = std::env::var("ARKOS_SHOW").ok().and_then(|s| s.parse().ok()).unwrap_or(0);
            let mut history: Vec<String> = Vec::new();
            let frames = references.iter().map(|r| r.len() / 14).min().unwrap_or(0);
            'frames: for t in 0..frames {
                p.frame();
                for (k, reference) in references.iter().enumerate() {
                    let want = &reference[t * 14..t * 14 + 14];
                    let Some(ours) = p.regs.get(k) else { break };
                    // The YM has every PSG's periods at the first PSG's clock.
                    let ours = &convert(ours, p.subsong().psg(k * 3).0, p.subsong().psg(0).0);
                    // YM6 keeps its effects in the registers' spare bits.
                    const MASK: [u8; 13] = [0xFF, 0x0F, 0xFF, 0x0F, 0xFF, 0x0F, 0x1F, 0x3F, 0x1F, 0x1F, 0x1F, 0xFF, 0xFF];
                    // A frame starting a digidrum has the digidrum's number in place of its channel's volume.
                    let digi = (want[3] >> 6 == 1).then(|| usize::from((want[3] >> 4) & 3).wrapping_sub(1));
                    let differs = (0..13).any(|r| digi.map(|d| d + 8) != Some(r) && ours[r] & MASK[r] != want[r] & MASK[r]);
                    // R13 is 255 when it isn't written.
                    let shape = if p.shape_written[k] { ours[13] & 0x0F } else { 0xFF };
                    if show > 0 {
                        history.push(format!("  {t} psg {k} pos {:?} ours {:02x?} w{} ref {:02x?}", p.shown, &ours[..], p.shape_written[k], want));
                    }
                    if differs || shape != want[13] {
                        for h in history.iter().rev().take(show).rev() {
                            println!("{h}");
                        }
                        bad = Some((t, ours.to_vec(), want.to_vec()));
                        break 'frames;
                    }
                }
            }
            match bad {
                None => same += 1,
                Some((t, o, w)) => println!("{name}: frame {t}: ours {o:02x?} ref {w:02x?}"),
            }
        }
        println!("{same} of {total} the same");
        assert_eq!(same, total);
    }
}
