// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! Renoise's replayer: the sequence, lines and ticks (Renoise 1's speed
//! timing, or 2.0's lines a beat and ticks a line, the groove), each note
//! column's voices, the volume, panning, delay and effect columns as the
//! song's version means them, graphical automation, and each tick's audio
//! through the tracks' device chains into groups, send tracks and the
//! master.

use std::sync::Arc;

use super::dsp::{self, Ctx, Params, Proc};
use super::env::{Mods, OldEnv, Rng};
use super::filter::{self, VoiceFilter, RATE};
use super::params;
use super::song::{Col, Device, Interp, Nna, NoteCell, PatternTrack, Song, TrackKind, NONE, OFF};
use super::voice::Voice;

/// At most this many voices sound at once; the oldest give way.
const MAX_VOICES: usize = 256;
/// At most this many notes let go of (or continued past) linger from one
/// note column; the oldest give way.
const LINGER: usize = 4;
/// Frames between updates of a voice's envelopes and pitch.
const CONTROL: usize = 64;

/// A device in a track's chain, as it plays.
pub(crate) struct Dev {
    pub def: Device,
    pub active: bool,
    pub values: Vec<f32>,
    pub proc: Option<Box<dyn Proc>>,
    /// A Doofer's devices.
    pub inner: Vec<Dev>,
    /// A meta device's state: its LFO's phase, its follower's level.
    phase: f32,
    level: f32,
    random: f32,
    /// A "reset" asked of a device without a processor (the LFO).
    reset: Option<f32>,
}

impl Dev {
    fn new(def: &Device) -> Dev {
        Dev {
            def: def.clone(),
            active: def.active,
            values: def.params.iter().map(|&(_, v)| v).collect(),
            proc: dsp::make(&def.kind),
            inner: def.inner.iter().map(Dev::new).collect(),
            phase: 0.0,
            level: 0.0,
            random: 0.0,
            reset: None,
        }
    }

    fn get(&self, name: &str, fallback: f32) -> f32 {
        self.def.params.iter().position(|(n, _)| n == name).and_then(|i| self.values.get(i).copied()).unwrap_or(fallback)
    }

    fn set_norm(&mut self, param: usize, norm: f32, doc: u32) {
        if let Some((name, _)) = self.def.params.get(param) {
            let v = params::value(&self.def.kind, name, doc, norm);
            self.values[param] = v;
        } else if let Some(p) = self.proc.as_mut() {
            // Past the saved parameters: the hidden "reset".
            p.reset(norm);
        } else {
            self.reset = Some(norm);
        }
    }

    fn is_mixer(&self) -> bool {
        self.def.kind.starts_with("Sequencer") && self.def.kind.ends_with("Device") || self.def.kind.ends_with("TrackMixerDevice") || self.def.kind == "SequencerTrackDevice"
    }
}

/// A note column's state.
#[derive(Clone, Copy, Debug, Default)]
struct Column {
    instr: Option<usize>,
    note: Option<u8>,
    /// 0..1, the column's volume (velocity) for its note.
    vol: f32,
    pan: Option<f32>,
}

/// An effect running on a track or a column for the line.
#[derive(Clone, Copy, Debug, Default)]
struct Fx {
    arp: Option<(u8, u8)>,
    /// Semitones a line.
    slide: f32,
    glide: Option<f32>,
    vibrato: Option<(u8, u8)>,
    tremolo: Option<(u8, u8)>,
    autopan: Option<(u8, u8)>,
    /// Volume change a line (fractions of full).
    fade: f32,
    pan_slide: f32,
    cut_vol: Option<(u8, u8)>,
    retrig: Option<(u8, u8)>,
    /// Retriggers that don't restart the sample (the columns' Rx).
    soft_retrig: Option<u8>,
    cut: Option<u8>,
}

/// A note to start (or stop) within a tick.
#[derive(Clone, Copy, Debug)]
struct Event {
    tick: u32,
    offset: usize,
    track: usize,
    column: usize,
    cell: NoteCell,
    /// From the line's effects: a sample offset (0..255), backwards, glide.
    offset_cmd: Option<u8>,
    backwards: Option<bool>,
    glide: bool,
}

struct TrackRun {
    kind: TrackKind,
    chain: Vec<Dev>,
    columns: Vec<Column>,
    line_fx: Fx,
    column_fx: Vec<Fx>,
    /// The M command's gain.
    channel_vol: f32,
    /// Where it goes: a group's track, or None for the master.
    to: Option<usize>,
    buf: [Vec<f32>; 2],
    on: bool,
    /// Tick phases for vibrato, tremolo and autopan.
    phase: [f32; 3],
    /// Ticks since the line started (for the effects counting them).
    retrig_count: u32,
    /// The last note started on the track: note, velocity (0..1), instrument.
    last_note: Option<(u8, f32, usize)>,
    /// The last value each slide command had (its 00 repeats it): the
    /// track's, then each note column's, by the command's letter.
    memory: [[u8; 26]; 13],
}

/// The replayer.
pub struct Engine {
    pub song: Arc<Song>,
    doc: u32,
    pub bpm: f32,
    pub lpb: u32,
    pub tpl: u32,
    groove: bool,
    pub entry: usize,
    pub line: usize,
    pub tick: u32,
    /// Ticks this line has (TPL, more with a line delay).
    line_ticks: u32,
    break_to: Option<usize>,
    pause_lines: u32,
    pub stopped: bool,
    pub looped: bool,
    tracks: Vec<TrackRun>,
    voices: Vec<Voice>,
    serial: u64,
    rng: Rng,
    frac: f64,
    pub frames: usize,
    events: Vec<Event>,
    sends: Vec<usize>,
    master: Option<usize>,
    /// The channels the host sees: the song's (sequencer) tracks.
    pub outputs: Vec<usize>,
    pub out: Vec<[Vec<f32>; 2]>,
    master_buf: [Vec<f32>; 2],
    dc: [f32; 4],
    /// Renoise 3's sample effect chains in use: (track, instrument, chain).
    chains: Vec<InsChain>,
    phrases: Vec<PhraseRun>,
}

/// A phrase playing on a note column: the instrument's own lines, at the
/// phrase's lines a beat, transposed by the note that started it.
struct PhraseRun {
    track: usize,
    column: usize,
    instr: usize,
    phrase: usize,
    transpose: i32,
    /// Where it is, in its lines.
    pos: f64,
    /// The last line it played.
    line: usize,
    vel: f32,
}

/// The voice column a phrase's note column plays in.
fn phrase_column(column: usize, pc: usize) -> usize {
    100 + column * 16 + pc
}

/// A sample effect chain as it plays on a track.
struct InsChain {
    track: usize,
    instr: usize,
    chain: usize,
    devs: Vec<Dev>,
    buf: [Vec<f32>; 2],
}

fn note_gain_db(v: u8, lo: f32) -> f32 {
    // 00 = lo dB, FF = +3 dB.
    let db = lo + (3.0 - lo) * f32::from(v) / 255.0;
    10f32.powf(db / 20.0)
}

fn balance(pan: f32) -> (f32, f32) {
    let p = pan.clamp(0.0, 1.0);
    ((2.0 * (1.0 - p)).min(1.0), (2.0 * p).min(1.0))
}

impl Engine {
    pub fn new(song: Arc<Song>) -> Engine {
        let doc = song.doc;
        let mut tracks: Vec<TrackRun> = song
            .tracks
            .iter()
            .map(|t| TrackRun {
                kind: t.kind,
                chain: t.devices.iter().map(Dev::new).collect(),
                columns: vec![Column { vol: 1.0, ..Default::default() }; 12],
                line_fx: Fx::default(),
                column_fx: vec![Fx::default(); 12],
                channel_vol: 1.0,
                to: t.group,
                buf: [Vec::new(), Vec::new()],
                on: t.on,
                phase: [0.0; 3],
                retrig_count: 0,
                last_note: None,
                memory: [[0; 26]; 13],
            })
            .collect();
        let sends: Vec<usize> = (0..tracks.len()).filter(|&t| tracks[t].kind == TrackKind::Send).collect();
        let master = (0..tracks.len()).find(|&t| tracks[t].kind == TrackKind::Master);
        for t in tracks.iter_mut() {
            if matches!(t.kind, TrackKind::Send | TrackKind::Master) {
                t.to = None;
            }
        }
        let outputs: Vec<usize> = (0..tracks.len()).filter(|&t| tracks[t].kind == TrackKind::Normal).collect();
        let out = outputs.iter().map(|_| [Vec::new(), Vec::new()]).collect();
        let mut e = Engine {
            bpm: song.bpm,
            lpb: song.lpb,
            tpl: song.tpl,
            groove: song.shuffle.is_some(),
            doc,
            song,
            entry: 0,
            line: 0,
            tick: 0,
            line_ticks: 1,
            break_to: None,
            pause_lines: 0,
            stopped: false,
            looped: false,
            tracks,
            voices: Vec::new(),
            serial: 0,
            rng: Rng(0x2545_f491),
            frac: 0.0,
            frames: 0,
            events: Vec::new(),
            sends,
            master,
            outputs,
            out,
            master_buf: [Vec::new(), Vec::new()],
            dc: [0.0; 4],
            chains: Vec::new(),
            phrases: Vec::new(),
        };
        e.line_ticks = e.tpl;
        e.start_line();
        e
    }

    fn beat_seconds(&self) -> f32 {
        60.0 / self.bpm.max(1.0)
    }

    fn line_seconds(&self) -> f32 {
        let base = if self.song.speed_timing {
            2.5 / self.bpm.max(1.0) * self.tpl as f32
        } else {
            60.0 / (self.bpm.max(1.0) * self.lpb as f32)
        };
        base * self.groove_factor(self.line)
    }

    /// A line's length relative to the beat's lines, with the groove.
    fn groove_factor(&self, line: usize) -> f32 {
        let Some(g) = self.song.shuffle.filter(|_| self.groove) else { return 1.0 };
        let lpb = if self.song.speed_timing { 4 } else { self.lpb as usize };
        let unit = (lpb / 4).max(1);
        let s = (line / unit) % 8;
        let a = g[s / 2].clamp(0.0, 0.99);
        if s % 2 == 0 {
            1.0 + a
        } else {
            1.0 - a
        }
    }

    pub fn tick_seconds(&self) -> f64 {
        f64::from(self.line_seconds()) / f64::from(self.tpl.max(1))
    }

    fn ctx(&self) -> Ctx {
        Ctx {
            rate: RATE,
            bpm: self.bpm,
            line: if self.song.speed_timing {
                2.5 / self.bpm * self.tpl as f32
            } else {
                60.0 / (self.bpm * self.lpb as f32)
            },
        }
    }

    fn pattern(&self) -> usize {
        self.song.sequence.get(self.entry).map_or(0, |e| e.pattern)
    }

    fn pattern_lines(&self) -> usize {
        self.song.patterns.get(self.pattern()).map_or(64, |p| p.lines)
    }

    fn track_muted(&self, t: usize) -> bool {
        !self.tracks[t].on || self.song.sequence.get(self.entry).is_some_and(|e| e.muted.contains(&t))
    }

    // ---------------------------------------------------------------
    // Lines and ticks.

    /// Reads the line at the current position: global commands, notes,
    /// effects.
    fn start_line(&mut self) {
        self.line_ticks = self.tpl.max(1);
        // Notes of the last line that never came (its pattern broke off).
        self.events.clear();
        let song = self.song.clone();
        let pat = self.pattern();
        let line = self.line;
        let ntracks = self.tracks.len();
        // Global commands first (they decide the line's timing).
        for t in 0..ntracks {
            let Some(pt) = song.pattern_track(pat, t) else { continue };
            let Some(l) = line_of(pt, line) else { continue };
            for fx in &l.fx {
                self.global(fx.num, fx.val);
            }
        }
        for t in 0..ntracks {
            let tr = &mut self.tracks[t];
            tr.line_fx = Fx::default();
            for c in tr.column_fx.iter_mut() {
                *c = Fx::default();
            }
            tr.retrig_count = 0;
            let Some(pt) = song.pattern_track(pat, t) else { continue };
            let Some(l) = line_of(pt, line) else { continue };
            let muted = self.track_muted(t);
            // The track's effects for the line.
            let mut delay_ticks = 0u32;
            let mut maybe = 255u8;
            let mut offset_cmd = None;
            let mut backwards = None;
            let mut glide = false;
            for fx in &l.fx {
                let r = self.track_fx(t, None, fx.num, fx.val);
                delay_ticks = delay_ticks.max(r.delay);
                if let Some(m) = r.maybe {
                    maybe = m;
                }
                offset_cmd = r.offset.or(offset_cmd);
                backwards = r.backwards.or(backwards);
                glide |= r.glide;
            }
            if muted {
                continue;
            }
            if maybe < 255 && (self.rng.next() * 0.5 + 0.5) * 255.0 > f32::from(maybe) {
                continue;
            }
            for (c, cell) in l.notes.iter().enumerate() {
                if c >= 12 || song.tracks[t].column_off.get(c).copied().unwrap_or(false) {
                    continue;
                }
                let mut ticks = delay_ticks;
                let mut col_offset = offset_cmd;
                let mut col_back = backwards;
                let mut col_glide = glide;
                // The volume and panning columns' commands.
                for (k, col) in [cell.vol, cell.pan].iter().enumerate() {
                    if let Col::Cmd(cmd, d) = *col {
                        match cmd {
                            b'Q' => ticks = ticks.max(u32::from(d)),
                            b'G' => {
                                col_glide = true;
                                self.tracks[t].column_fx[c].glide = Some(if d == 15 { 1000.0 } else { f32::from(d) });
                            }
                            b'B' => col_back = Some(d == 0),
                            b'Y' => {
                                if (self.rng.next() * 0.5 + 0.5) * 15.0 > f32::from(d) {
                                    ticks = u32::MAX;
                                }
                            }
                            _ => self.column_cmd(t, c, cmd, d, k == 1),
                        }
                    }
                }
                if let Some((num, val)) = cell.fx {
                    let r = self.track_fx(t, Some(c), num, val);
                    ticks = ticks.max(r.delay);
                    col_offset = r.offset.or(col_offset);
                    col_back = r.backwards.or(col_back);
                    col_glide |= r.glide;
                }
                if ticks == u32::MAX || (cell.note == NONE && cell.instr == NONE && !matches!(cell.vol, Col::Set(_)) && !matches!(cell.pan, Col::Set(_))) {
                    continue;
                }
                // The delay column: 1/256ths of a line.
                let tick_frames = self.tick_seconds() * f64::from(RATE);
                let line_frames = tick_frames * f64::from(self.tpl);
                let at = f64::from(ticks) * tick_frames + f64::from(cell.delay) / 256.0 * line_frames;
                // Delayed past the line's end, a note comes on its last tick.
                let tick = (at / tick_frames.max(1.0)).floor().min(f64::from(self.line_ticks - 1));
                let offset = ((at - tick * tick_frames).max(0.0) as usize).min(tick_frames as usize);
                self.events.push(Event {
                    tick: tick as u32,
                    offset,
                    track: t,
                    column: c,
                    cell: *cell,
                    offset_cmd: col_offset,
                    backwards: col_back,
                    glide: col_glide,
                });
            }
        }
        self.automate(0.0);
    }

    /// A global command (they apply at the line's start).
    fn global(&mut self, num: [u8; 2], val: u8) {
        let doc = self.doc;
        if doc >= 37 {
            if num[0] != b'Z' {
                return;
            }
            match num[1] {
                b'T' => {
                    if val == 0 {
                        self.stopped = true;
                    } else {
                        self.bpm = f32::from(val.max(20));
                    }
                }
                b'L' => {
                    if val == 0 {
                        self.stopped = true;
                    } else {
                        self.lpb = u32::from(val);
                    }
                }
                b'K' => self.tpl = u32::from(val.clamp(1, 16)),
                b'G' => self.groove = val != 0,
                b'B' => self.break_to = Some(usize::from(val)),
                b'D' => self.pause_lines = u32::from(val),
                _ => {}
            }
            return;
        }
        if num[0] != b'F' {
            return;
        }
        match (num[1], doc <= 10) {
            // Renoise 1's manual gives F0 20 to FF; below that, nothing.
            (b'0', true) if (1..0x20).contains(&val) => {}
            (b'0', _) => {
                if val == 0 {
                    self.stopped = true;
                } else {
                    self.bpm = f32::from(val.max(20));
                }
            }
            (b'1', true) => {
                if val == 0 {
                    self.stopped = true;
                } else {
                    self.tpl = u32::from(val.min(32));
                }
            }
            (b'1', false) => {
                if val == 0 {
                    self.stopped = true;
                } else {
                    self.lpb = u32::from(val);
                }
            }
            (b'2', false) => self.tpl = u32::from(val.clamp(1, 16)),
            (b'4', false) => self.groove = val != 0,
            (b'B', _) => self.break_to = Some(usize::from(val)),
            (b'C', _) => {
                if let Some(m) = self.master {
                    let g = note_gain_db(val, -60.0);
                    if let Some(d) = self.tracks[m].chain.iter_mut().find(|d| d.is_mixer()) {
                        if let Some(i) = d.def.params.iter().position(|(n, _)| n == "Volume") {
                            d.values[i] = if val == 0 { 0.0 } else { g };
                        }
                    }
                }
            }
            (b'D', _) => self.pause_lines = u32::from(val),
            _ => {}
        }
    }

    /// A track (or note column's) effect command: sets the line's effect,
    /// or changes a device; says what notes on the line should do.
    fn track_fx(&mut self, t: usize, column: Option<usize>, num: [u8; 2], val: u8) -> LineFx {
        let mut r = LineFx::default();
        let doc = self.doc;
        let cmd = if doc >= 37 {
            if num[0] == b'0' {
                num[1]
            } else if num[0].is_ascii_hexdigit() && num[0] != b'0' {
                // A device: x = device, y = parameter (0 its on switch).
                let x = (num[0] as char).to_digit(16).unwrap_or(0) as usize;
                let y = (num[1] as char).to_digit(16).unwrap_or(0) as usize;
                self.device_cmd(t, x, if y == 0 { None } else { Some(y - 1) }, val);
                return r;
            } else {
                return r;
            }
        } else {
            if num[0] != b'0' {
                if num[0] != b'F' {
                    let x = (num[0] as char).to_digit(16).unwrap_or(0) as usize;
                    let y = (num[1] as char).to_digit(16).unwrap_or(0) as usize;
                    self.device_cmd(t, x, if y == 15 { None } else { Some(y) }, val);
                } else if num[1] == b'F' && val == 0 {
                    self.stop_track(t);
                }
                return r;
            }
            // Renoise 1 and 2's sample commands, as Renoise 3's letters.
            match num[1] {
                b'0' => b'A',
                b'1' => b'U',
                b'2' => b'D',
                b'3' => b'M',
                b'4' => b'C',
                b'5' => b'G',
                b'6' => b'I',
                b'7' => b'O',
                b'8' => b'P',
                b'9' => b'S',
                b'A' => b'W',
                b'B' => b'B',
                b'C' => b'L',
                b'D' => b'Q',
                b'E' => b'R',
                b'F' => b'V',
                _ => return r,
            }
        };
        // Slides, glides and the oscillating effects repeat their last value for 00.
        let mut val = val;
        if matches!(cmd, b'U' | b'D' | b'G' | b'V' | b'I' | b'O' | b'T' | b'N') {
            let slot = &mut self.tracks[t].memory[column.map_or(0, |c| c + 1)][usize::from(cmd - b'A')];
            if val == 0 {
                val = *slot;
            } else {
                *slot = val;
            }
        }
        let (hi, lo) = (val >> 4, val & 15);
        // Slides' units: 1/16 semitone, and a volume step of 1/256 of full
        // (1/64 in Renoise 1's 00-40 volume scale).
        let vol_unit = if doc <= 10 { 64.0 } else { 256.0 };
        let fx = match column {
            Some(c) => &mut self.tracks[t].column_fx[c],
            None => &mut self.tracks[t].line_fx,
        };
        match cmd {
            b'A' => {
                if val != 0 {
                    fx.arp = Some((hi, lo));
                }
            }
            b'U' => fx.slide += f32::from(val) / 16.0,
            b'D' => fx.slide -= f32::from(val) / 16.0,
            b'G' => {
                fx.glide = Some(if val == 0xff { 1000.0 } else { f32::from(val) / 16.0 });
                r.glide = true;
            }
            b'V' => fx.vibrato = Some((hi, lo)),
            b'T' => fx.tremolo = Some((hi, lo)),
            b'N' => fx.autopan = Some((hi, lo)),
            b'I' => fx.fade += f32::from(val) / vol_unit,
            b'O' => fx.fade -= f32::from(val) / vol_unit,
            b'C' => fx.cut_vol = Some((hi, lo)),
            b'R' => {
                if lo > 0 {
                    fx.retrig = Some((hi, lo));
                }
            }
            b'S' => r.offset = Some(val),
            b'B' => {
                r.backwards = Some(val == 0);
                let back = val == 0;
                for v in self.voices.iter_mut().filter(|v| v.track == t && column.is_none_or(|c| v.column == c)) {
                    v.back = back;
                }
            }
            b'Q' => r.delay = u32::from(val),
            b'Y' => r.maybe = Some(val),
            b'M' => self.tracks[t].channel_vol = note_gain_db(val, -60.0),
            b'L' => self.mixer_set(t, "Volume", f32::from(val) / 255.0),
            b'P' => self.mixer_set(t, "Panning", f32::from(val) / 255.0),
            b'W' => self.mixer_set(t, "Surround", f32::from(val) / 255.0),
            b'X' => {
                if val == 0 {
                    self.stop_track(t);
                }
            }
            b'E' => {
                for v in self.voices.iter_mut().filter(|v| v.track == t) {
                    if let Some(e) = v.env.as_mut() {
                        e.seek(f32::from(val));
                    }
                }
            }
            _ => {}
        }
        r
    }

    fn column_cmd(&mut self, t: usize, c: usize, cmd: u8, d: u8, panning: bool) {
        let doc = self.doc;
        let fx = &mut self.tracks[t].column_fx[c];
        let step = f32::from(d) * 16.0 / if doc <= 10 { 64.0 } else { 256.0 };
        match cmd {
            b'I' => fx.fade += step,
            b'O' => fx.fade -= step,
            b'J' if panning => fx.pan_slide -= f32::from(d) / 128.0,
            b'K' if panning => fx.pan_slide += f32::from(d) / 128.0,
            b'U' => fx.slide += f32::from(d),
            b'D' => fx.slide -= f32::from(d),
            b'C' => fx.cut = Some(d),
            b'R' if d > 0 => fx.soft_retrig = Some(d),
            _ => {}
        }
    }

    fn mixer_set(&mut self, t: usize, name: &str, norm: f32) {
        let doc = self.doc;
        if let Some(d) = self.tracks[t].chain.iter_mut().find(|d| d.is_mixer()) {
            if let Some(i) = d.def.params.iter().position(|(n, _)| n == name) {
                d.set_norm(i, norm, doc);
            }
        }
    }

    /// A device command: device `x` of the track's chain, parameter `p`
    /// (None: on or off).
    fn device_cmd(&mut self, t: usize, x: usize, p: Option<usize>, val: u8) {
        let doc = self.doc;
        let Some(d) = self.tracks[t].chain.get_mut(x) else { return };
        match p {
            None => d.active = val != 0,
            Some(p) => d.set_norm(p, f32::from(val) / 255.0, doc),
        }
    }

    fn stop_track(&mut self, t: usize) {
        for v in self.voices.iter_mut().filter(|v| v.track == t) {
            v.cutting = true;
        }
        self.tracks[t].line_fx = Fx::default();
    }

    /// The pattern's graphical automation at `frac` of the line.
    fn automate(&mut self, frac: f32) {
        let song = self.song.clone();
        let pat = self.pattern();
        let at = self.line as f64 + f64::from(frac);
        let doc = self.doc;
        for t in 0..self.tracks.len() {
            let Some(pt) = song.pattern_track(pat, t) else { continue };
            for a in &pt.automation {
                let Some(v) = automation_at(&a.points, a.interp, at) else { continue };
                if let Some(d) = self.tracks[t].chain.get_mut(a.device) {
                    if a.param < d.values.len() {
                        d.set_norm(a.param, v, doc);
                    }
                }
            }
        }
    }

    /// Moves to the next line (or entry), with breaks, pauses and the
    /// song's end.
    fn next_line(&mut self) {
        if self.pause_lines > 0 {
            self.pause_lines -= 1;
            self.start_line_effects_only();
            return;
        }
        let lines = self.pattern_lines();
        if let Some(to) = self.break_to.take() {
            self.next_entry();
            self.line = to.min(self.pattern_lines().saturating_sub(1));
        } else {
            self.line += 1;
            if self.line >= lines {
                self.line = 0;
                self.next_entry();
            }
        }
        self.start_line();
    }

    fn start_line_effects_only(&mut self) {
        for t in self.tracks.iter_mut() {
            t.retrig_count = 0;
        }
    }

    fn next_entry(&mut self) {
        self.entry += 1;
        if self.entry >= self.song.sequence.len() {
            self.entry = 0;
            self.looped = true;
        }
    }

    /// One tick: the sequence moves on, the tick's events are set out.
    pub fn advance(&mut self) {
        self.tick += 1;
        if self.tick >= self.line_ticks {
            self.tick = 0;
            self.next_line();
        } else {
            self.automate(self.tick as f32 / self.tpl.max(1) as f32);
        }
        let secs = self.tick_seconds() * f64::from(RATE) + self.frac;
        self.frames = secs.floor().max(1.0) as usize;
        self.frac = secs - self.frames as f64;
    }

    /// The first tick (call once before rendering the song's start).
    pub fn first(&mut self) {
        let secs = self.tick_seconds() * f64::from(RATE);
        self.frames = secs.floor().max(1.0) as usize;
        self.frac = secs - self.frames as f64;
    }

    /// How many voices are sounding.
    #[cfg(test)]
    pub fn voice_count(&self) -> usize {
        self.voices.len()
    }

    /// Each track's peak in the tick just made (after its chain).
    #[cfg(test)]
    pub fn track_peaks(&self) -> Vec<f32> {
        self.tracks.iter().map(|t| t.buf.iter().flat_map(|b| b.iter()).fold(0.0f32, |a, &x| a.max(x.abs()))).collect()
    }

    /// The master's output for the tick just made.
    pub fn master_out(&self) -> &[Vec<f32>; 2] {
        &self.master_buf
    }

    // ---------------------------------------------------------------
    // Notes.

    fn note_on(&mut self, e: &Event) {
        let song = self.song.clone();
        let t = e.track;
        let c = e.column;
        let cell = e.cell;
        let col = &mut self.tracks[t].columns[c];
        if cell.instr != NONE {
            col.instr = Some(usize::from(cell.instr));
        }
        if let Col::Set(v) = cell.vol {
            col.vol = f32::from(v.min(0x80)) / 128.0;
            for v in self.voices.iter_mut().filter(|v| v.track == t && v.column == c) {
                v.vol = f32::from(cell.vol_value()) / 128.0;
            }
        }
        if let Col::Set(p) = cell.pan {
            let p = f32::from(p.min(0x80)) / 128.0;
            self.tracks[t].columns[c].pan = Some(p);
            for v in self.voices.iter_mut().filter(|v| v.track == t && v.column == c) {
                v.pan = p;
            }
        }
        if cell.note == OFF {
            self.note_off(t, c);
            return;
        }
        if cell.note == NONE {
            // An instrument alone resets the playing note's volume.
            if cell.instr != NONE && !matches!(cell.vol, Col::Set(_)) {
                let vol = 1.0;
                self.tracks[t].columns[c].vol = vol;
                for v in self.voices.iter_mut().filter(|v| v.track == t && v.column == c) {
                    v.vol = vol;
                }
            }
            return;
        }
        let note = cell.note;
        let col = self.tracks[t].columns[c];
        let Some(ii) = col.instr else { return };
        let Some(ins) = song.instruments.get(ii) else { return };
        // A glide moves the playing note instead of starting one.
        if e.glide {
            let target = note as f32;
            let mut any = false;
            for v in self.voices.iter_mut().filter(|v| v.track == t && v.column == c && !v.released && !v.dead) {
                v.glide_to = Some(v.semis_for(target));
                any = true;
            }
            if any {
                self.tracks[t].columns[c].note = Some(note);
                return;
            }
        }
        let vel = if matches!(cell.vol, Col::Set(_)) { col.vol } else { 1.0 };
        if !matches!(cell.vol, Col::Set(_)) {
            self.tracks[t].columns[c].vol = 1.0;
        }
        // The column's playing notes give way, as their samples say.
        for v in self.voices.iter_mut().filter(|v| v.track == t && v.column == c && !v.dead) {
            let nna = song.instruments.get(v.instr).and_then(|i| i.samples.get(v.sample)).map_or(Nna::Cut, |s| s.nna);
            match nna {
                Nna::Cut => v.cutting = true,
                Nna::Off => {
                    v.released = true;
                    if !v.holds_release() {
                        v.cutting = true;
                    }
                    v.column = usize::MAX;
                }
                Nna::Continue => v.column = usize::MAX,
            }
        }
        self.tracks[t].columns[c].note = Some(note);
        self.tracks[t].last_note = Some((note, vel, ii));
        // An instrument's phrase plays instead, when its keymap has one there.
        let phrase = match ins.phrase_mode {
            1 => ins.phrases.iter().position(|ph| (ph.keys.0..=ph.keys.1).contains(&note)).map(|k| (k, ins.phrases[k].base_note)),
            // Selective: the selected phrase, from C-4.
            2 => ins.phrase_selected.filter(|&k| k < ins.phrases.len()).map(|k| (k, 48)),
            _ => None,
        };
        if let Some((k, base)) = phrase {
            self.phrases.retain(|r| !(r.track == t && r.column == c));
            for v in self.voices.iter_mut().filter(|v| v.track == t && v.column >= 100 && v.column != usize::MAX && v.home == c && !v.dead) {
                v.cutting = true;
            }
            self.phrases.push(PhraseRun {
                track: t,
                column: c,
                instr: ii,
                phrase: k,
                transpose: i32::from(note) - base,
                pos: -1e-9,
                line: usize::MAX,
                vel,
            });
            return;
        }
        self.spawn(t, c, c, ii, note, vel, e.offset_cmd, e.backwards, e.offset, col.pan, false);
        // A column's old notes linger, but only a few.
        let mut old: Vec<(u64, usize)> = self
            .voices
            .iter()
            .enumerate()
            .filter(|(_, v)| v.track == t && v.column == usize::MAX && v.home == c && !v.cutting)
            .map(|(i, v)| (v.serial, i))
            .collect();
        if old.len() > LINGER {
            old.sort();
            for &(_, i) in &old[..old.len() - LINGER] {
                self.voices[i].cutting = true;
            }
        }
        // Too many voices: the oldest released ones go first.
        if self.voices.len() > MAX_VOICES {
            self.voices.sort_by_key(|v| (!v.released, v.serial));
            let n = self.voices.len() - MAX_VOICES;
            self.voices.drain(..n);
        }
    }

    /// Starts the voices a note plays: every key zone of the instrument the
    /// note and velocity fall in (layers sound together).
    #[allow(clippy::too_many_arguments)]
    fn spawn(&mut self, t: usize, column: usize, home: usize, ii: usize, note: u8, vel: f32, offset_cmd: Option<u8>, backwards: Option<bool>, wait: usize, pan: Option<f32>, off_layer: bool) {
        let song = self.song.clone();
        let Some(ins) = song.instruments.get(ii) else { return };
        let vel127 = (vel * 127.0).round().clamp(0.0, 127.0) as u8;
        let zones: Vec<_> = ins
            .zones
            .iter()
            .filter(|z| z.off_layer == off_layer && (z.notes.0..=z.notes.1).contains(&note) && (z.vels.0..=z.vels.1).contains(&vel127))
            .copied()
            .collect();
        for z in zones {
            let Some(s) = ins.samples.get(z.sample) else { continue };
            if s.audio.frames() == 0 {
                continue;
            }
            // Mute groups cut each other in the track.
            if s.mute_group >= 0 {
                for v in self.voices.iter_mut().filter(|v| v.track == t && v.instr == ii && v.mute_group == s.mute_group) {
                    v.cutting = true;
                }
            }
            let key = if z.key_to_pitch { i32::from(note) } else { z.base_note };
            let semis = (key - z.base_note + s.transpose + ins.transpose) as f32 + s.finetune as f32 / 128.0;
            let frames = s.audio.frames();
            let rate_ratio = f64::from(s.audio.rate) / f64::from(RATE);
            let base_step = if let Some(lines) = s.beat_sync {
                frames as f64 / (f64::from(lines) * f64::from(self.ctx().line) * f64::from(RATE))
            } else {
                rate_ratio
            };
            let (mut start, mut end) = (0.0, frames as f64);
            if s.alias && s.slices.len() == 2 {
                start = s.slices[0] as f64;
                end = (s.slices[1] as f64).min(frames as f64);
            }
            if let Some(o) = offset_cmd {
                if !s.slices.is_empty() && !s.alias && usize::from(o) > 0 {
                    // A sliced sample's Sxx plays slice xx.
                    if let Some(&at) = s.slices.get(usize::from(o) - 1) {
                        start = at as f64;
                    }
                } else if self.song.amiga_offsets {
                    start += f64::from(o) * 256.0;
                } else {
                    start += (end - start) * f64::from(o) / 256.0;
                }
            }
            let vol_gain = if z.vel_to_volume { vel } else { 1.0 };
            let mut filter = VoiceFilter::new(filter::Kind::None);
            let env = ins.envelopes.as_ref().map(|e| {
                let oe = OldEnv::new(e.clone(), self.serial as u32 ^ 0x9e37);
                if let Some(ft) = oe.filter_type() {
                    filter = VoiceFilter::new(filter::kind(2, ft));
                }
                oe
            });
            let mods = (!ins.mod_sets.is_empty()).then(|| {
                let set = &ins.mod_sets[(s.mod_set.max(0) as usize).min(ins.mod_sets.len() - 1)];
                filter = VoiceFilter::new(filter::kind(set.filter_version, set.filter_type));
                Mods::new(set, note, vel, self.serial as u32 ^ 0x51ed)
            });
            let looping = s.loop_mode != super::song::Loop::Off && s.loop_end > s.loop_start;
            self.serial += 1;
            let mut v = Voice {
                track: t,
                column,
                instr: ii,
                sample: z.sample,
                audio: s.audio.clone(),
                interp: s.interp,
                loop_mode: if looping { s.loop_mode } else { super::song::Loop::Off },
                loop_start: s.loop_start as f64,
                loop_end: (s.loop_end as f64).min(frames as f64),
                end,
                start,
                loop_release: s.loop_release,
                pos: start.min(end - 1.0).max(0.0),
                back: backwards.unwrap_or(false),
                looping_back: false,
                in_loop: false,
                wrapped: false,
                semis,
                base_step,
                bend: 0.0,
                glide_to: None,
                vol: vol_gain,
                sample_vol: s.volume * ins.volume,
                pan: pan.unwrap_or(0.5),
                sample_pan: s.pan,
                released: false,
                fade: 1.0,
                cutting: false,
                env,
                mods,
                filter,
                chain: s.chain,
                dead: false,
                gain: (0.0, 0.0),
                started: false,
                oneshot: s.oneshot,
                mute_group: s.mute_group,
                serial: self.serial,
                wait,
                note: f32::from(note),
                home,
                arp: 0.0,
                vib: 0.0,
                trem: 1.0,
                auto_pan: 0.0,
            };
            if v.back && v.pos <= start {
                v.pos = end - 1.0;
            }
            self.voices.push(v);
        }
    }

    fn note_off(&mut self, t: usize, c: usize) {
        let song = self.song.clone();
        // A phrase stops, its notes let go.
        self.phrases.retain(|r| !(r.track == t && r.column == c));
        for v in self.voices.iter_mut().filter(|v| v.track == t && v.column >= 100 && v.column != usize::MAX && v.home == c && !v.dead) {
            v.released = true;
            if !v.holds_release() {
                v.cutting = true;
            }
        }
        for v in self.voices.iter_mut().filter(|v| v.track == t && v.column == c && !v.dead) {
            if v.oneshot {
                continue;
            }
            v.released = true;
            if !v.holds_release() {
                v.cutting = true;
            }
        }
        // Renoise 3's note-off layer: its samples sound as the note is let go.
        let col = self.tracks[t].columns[c];
        if let (Some(ii), Some(note)) = (col.instr, col.note) {
            if song
                .instruments
                .get(ii)
                .is_some_and(|ins| ins.zones.iter().any(|z| z.off_layer && (z.notes.0..=z.notes.1).contains(&note)))
            {
                self.spawn(t, usize::MAX, c, ii, note, col.vol, None, None, 0, col.pan, true);
            }
        }
    }

    /// The phrases playing: their lines that fall in this tick.
    fn run_phrases(&mut self, frames: usize) {
        if self.phrases.is_empty() {
            return;
        }
        let song = self.song.clone();
        let mut starts = Vec::new();
        for r in self.phrases.iter_mut() {
            let Some(ph) = song.instruments.get(r.instr).and_then(|i| i.phrases.get(r.phrase)) else {
                continue;
            };
            let line_frames = f64::from(RATE) * 60.0 / (f64::from(self.bpm) * f64::from(ph.lpb.max(1)));
            let from = r.pos;
            r.pos += frames as f64 / line_frames;
            // Each line it reaches in the tick, at its place in the tick.
            let mut l = from.ceil().max(0.0) as usize;
            while (l as f64) < r.pos {
                let mut at = l;
                if at >= ph.length {
                    if ph.looping {
                        let (ls, le) = (
                            ph.loop_range.0.min(ph.length - 1),
                            if ph.loop_range.1 > ph.loop_range.0 { ph.loop_range.1.min(ph.length) } else { ph.length },
                        );
                        let span = (le - ls).max(1);
                        at = ls + (at - ls) % span;
                    } else {
                        break;
                    }
                }
                let offset = (((l as f64) - from) * line_frames).max(0.0) as usize;
                if let Ok(i) = ph.lines.binary_search_by_key(&at, |(k, _)| *k) {
                    starts.push((r.track, r.column, r.instr, r.transpose, r.vel, offset.min(frames.saturating_sub(1)), ph.lines[i].1.clone()));
                }
                r.line = at;
                l += 1;
            }
        }
        for (t, c, ii, transpose, vel, offset, line) in starts {
            for (pc, cell) in line.notes.iter().enumerate() {
                let id = phrase_column(c, pc);
                if cell.note == NONE {
                    continue;
                }
                for v in self.voices.iter_mut().filter(|v| v.track == t && v.column == id && !v.dead) {
                    if cell.note == OFF {
                        v.released = true;
                        if !v.holds_release() {
                            v.cutting = true;
                        }
                    } else {
                        v.cutting = true;
                    }
                }
                if cell.note == OFF {
                    continue;
                }
                let note = (i32::from(cell.note) + transpose).clamp(0, 119) as u8;
                let cv = if let Col::Set(x) = cell.vol { f32::from(x.min(0x80)) / 128.0 } else { 1.0 };
                let pan = if let Col::Set(p) = cell.pan { Some(f32::from(p.min(0x80)) / 128.0) } else { None };
                let ins = if cell.instr != NONE { usize::from(cell.instr) } else { ii };
                let ins = if song.instruments.get(ins).is_some_and(|i| !i.samples.is_empty()) { ins } else { ii };
                self.spawn(t, id, c, ins, note, vel * cv, None, None, offset, pan, false);
            }
        }
    }

    // ---------------------------------------------------------------
    // Meta devices.

    /// The meta devices: each sets its target's parameter for the tick.
    fn run_meta(&mut self, dt: f32) {
        let doc = self.doc;
        let mut writes: Vec<(usize, usize, usize, f32)> = Vec::new();
        for t in 0..self.tracks.len() {
            let last = self.tracks[t].last_note;
            for di in 0..self.tracks[t].chain.len() {
                let d = &mut self.tracks[t].chain[di];
                if !d.active {
                    continue;
                }
                // Renoise 3 counts the target's parameters from 1, before it from 0.
                let first = if doc >= 37 { 1 } else { 0 };
                let dest = |d: &Dev, prefix: &str| -> Option<(usize, usize, usize)> {
                    let tr = d.get(&format!("{prefix}DestTrack"), -1.0).round() as i32;
                    let e = d.get(&format!("{prefix}DestEffect"), -1.0).round() as i32;
                    let p = d.get(&format!("{prefix}DestParameter"), -1.0).round() as i32 - first;
                    (e >= 0 && p >= 0).then_some((if tr < 0 { t } else { tr as usize }, e as usize, p as usize))
                };
                match d.def.kind.as_str() {
                    "LfoDevice" => {
                        if let Some(at) = d.reset.take() {
                            d.phase = at;
                        }
                        let freq = d.get("Frequency", 1.0);
                        let before = d.phase;
                        d.phase += freq * dt;
                        let kind = d.get("Type", 0.0).round() as i32;
                        if d.phase.floor() != before.floor() {
                            self.rng.next();
                            d.random = self.rng.next() * 0.5 + 0.5;
                        }
                        let p = d.phase.fract();
                        let u = match kind {
                            1 => 1.0 - p,
                            2 => {
                                if p < 0.5 {
                                    1.0
                                } else {
                                    0.0
                                }
                            }
                            3 => d.random,
                            4 => {
                                let e = d.def.env.as_ref();
                                let one = d.def.flag("CustomEnvelopeOneShot");
                                let pos = if one { d.phase.min(1.0) } else { p };
                                e.map_or(0.5, |e| {
                                    let pts: Vec<(f32, f32)> = e.points.iter().map(|&(t, v)| (t as f32, v)).collect();
                                    super::env::curve(&pts, pos * e.length.saturating_sub(1).max(1) as f32, e.interp.unwrap_or(Interp::Curve), 0.5)
                                })
                            }
                            _ => 0.5 + 0.5 * (p * std::f32::consts::TAU).sin(),
                        };
                        let v = d.get("Offset", 0.0) + d.get("Amplitude", 1.0) * u;
                        if let Some((tt, e, pp)) = dest(d, "") {
                            writes.push((tt, e, pp, v));
                        }
                    }
                    "HydraDevice" => {
                        let x = d.get("InputValue", 0.0);
                        for k in 1..=9 {
                            let pre = format!("Out{k}");
                            if let Some((tt, e, pp)) = dest(d, &pre) {
                                let (lo, hi) = (d.get(&format!("{pre}Min"), 0.0), d.get(&format!("{pre}Max"), 1.0));
                                let v = lo + (hi - lo) * scale(d.def.text(&format!("{pre}Scaling")), x);
                                writes.push((tt, e, pp, v));
                            }
                        }
                    }
                    "SignalFollowerDevice" => {
                        if let Some((tt, e, pp)) = dest(d, "") {
                            let (lo, hi) = (d.get("DestMin", 0.0), d.get("DestMax", 1.0));
                            let x = (d.level * d.get("Sensitivity", 1.0)).clamp(0.0, 1.0);
                            let v = lo + (hi - lo) * scale(d.def.text("DestScaling"), x) + d.get("DestOffset", 0.0);
                            writes.push((tt, e, pp, v));
                        }
                    }
                    "KeyTrackingDevice" | "VelocityDevice" => {
                        let key = d.def.kind == "KeyTrackingDevice";
                        let src: i32 = d.def.text("SrcInstrument").and_then(|s| s.parse().ok()).unwrap_or(-1);
                        if let (Some((note, vel, ins)), Some((tt, e, pp))) = (last, dest(d, "")) {
                            if src < 0 || src as usize == ins {
                                let x = if key {
                                    let lo: f32 = d.def.text("KeyTrackingMin").and_then(|s| s.parse().ok()).unwrap_or(0.0);
                                    let hi: f32 = d.def.text("KeyTrackingMax").and_then(|s| s.parse().ok()).unwrap_or(119.0);
                                    ((f32::from(note) - lo) / (hi - lo).max(1.0)).clamp(0.0, 1.0)
                                } else {
                                    let lo: f32 = d.def.text("VelocityMin").and_then(|s| s.parse().ok()).unwrap_or(0.0);
                                    let hi: f32 = d.def.text("VelocityMax").and_then(|s| s.parse().ok()).unwrap_or(127.0);
                                    ((vel * 127.0 - lo) / (hi - lo).max(1.0)).clamp(0.0, 1.0)
                                };
                                let scaling = if key {
                                    d.def.text("DestScaling")
                                } else {
                                    d.def.text("VelocityScaling").or(d.def.text("DestScaling"))
                                };
                                let (lo, hi) = (d.get("DestMin", 0.0), d.get("DestMax", 1.0));
                                writes.push((tt, e, pp, lo + (hi - lo) * scale(scaling, x)));
                            }
                        }
                    }
                    "MetaMixerDevice" => {
                        if let Some((tt, e, pp)) = dest(d, "") {
                            let ws = [d.get("WeightA", 0.5), d.get("WeightB", 0.5), d.get("WeightC", 0.5)];
                            let xs = [d.get("InputA", 0.0), d.get("InputB", 0.0), d.get("InputC", 0.0)];
                            let sum: f32 = ws.iter().sum();
                            let x = if sum > 0.0 { ws.iter().zip(&xs).map(|(w, x)| w * x).sum::<f32>() / sum } else { 0.0 };
                            let (lo, hi) = (d.get("DestMin", 0.0), d.get("DestMax", 1.0));
                            writes.push((tt, e, pp, lo + (hi - lo) * scale(d.def.text("DestScaling"), x)));
                        }
                    }
                    _ => {}
                }
            }
        }
        for (t, e, p, v) in writes {
            if let Some(d) = self.tracks.get_mut(t).and_then(|tr| tr.chain.get_mut(e)) {
                d.set_norm(p, v.clamp(0.0, 1.0), doc);
            }
        }
    }

    // ---------------------------------------------------------------
    // Audio.

    /// Applies the line's running effects for this tick.
    fn tick_effects(&mut self) {
        let tick = self.tick;
        let tpl = self.tpl.max(1) as f32;
        for t in 0..self.tracks.len() {
            let lf = self.tracks[t].line_fx;
            let cfx = self.tracks[t].column_fx.clone();
            let ph = self.tracks[t].phase;
            for v in self.voices.iter_mut().filter(|v| v.track == t && !v.dead) {
                let col = if v.column < 12 { cfx[v.column] } else { Fx::default() };
                for fx in [lf, col] {
                    if fx.slide != 0.0 {
                        v.bend += fx.slide / tpl;
                    }
                    if let Some(g) = fx.glide {
                        if let Some(to) = v.glide_to {
                            let now = v.semis + v.bend;
                            let step = g / tpl;
                            if (to - now).abs() <= step {
                                v.bend = to - v.semis;
                                v.glide_to = None;
                            } else {
                                v.bend += step * (to - now).signum();
                            }
                        }
                    }
                    if fx.fade != 0.0 {
                        v.vol = (v.vol + fx.fade / tpl).clamp(0.0, 1.0);
                    }
                    if fx.pan_slide != 0.0 {
                        v.pan = (v.pan + fx.pan_slide / tpl).clamp(0.0, 1.0);
                    }
                    if let Some((x, y)) = fx.cut_vol {
                        if tick == u32::from(y) {
                            v.vol *= f32::from(x) / 15.0;
                        }
                    }
                    if let Some(c) = fx.cut {
                        if tick == u32::from(c) {
                            v.cutting = true;
                        }
                    }
                    if let Some((x, y)) = fx.retrig {
                        if tick > 0 && tick % u32::from(y) == 0 {
                            v.pos = v.start;
                            v.in_loop = false;
                            v.wrapped = false;
                            v.vol = retrig_volume(v.vol, x);
                        }
                    }
                    if let Some(d) = fx.soft_retrig {
                        if tick > 0 && tick % u32::from(d) == 0 {
                            v.restart_envelopes();
                        }
                    }
                }
                // Arpeggio, vibrato, tremolo, autopan: this tick's offsets.
                let arp = lf.arp.or(col.arp);
                v.arp = arp.map_or(0.0, |(x, y)| [0.0, f32::from(x), f32::from(y)][(tick % 3) as usize]);
                v.vib = lf.vibrato.or(col.vibrato).map_or(0.0, |(_, d)| (ph[0] * std::f32::consts::TAU).sin() * f32::from(d) / 8.0);
                v.trem = lf
                    .tremolo
                    .or(col.tremolo)
                    .map_or(1.0, |(_, d)| 1.0 - f32::from(d) / 15.0 * 0.5 * (1.0 - (ph[1] * std::f32::consts::TAU).sin()));
                v.auto_pan = lf.autopan.or(col.autopan).map_or(0.0, |(_, d)| (ph[2] * std::f32::consts::TAU).sin() * f32::from(d) / 15.0 * 0.5);
            }
            // Effect phases: speed x is x/4 cycles a line.
            let tr = &mut self.tracks[t];
            for (k, fx) in [lf.vibrato, lf.tremolo, lf.autopan].iter().enumerate() {
                let any = fx.or(cfx.iter().find_map(|c| [c.vibrato, c.tremolo, c.autopan][k]));
                if let Some((s, _)) = any {
                    tr.phase[k] = (tr.phase[k] + f32::from(s) / 4.0 / tpl).fract();
                }
            }
        }
    }

    /// The tick's audio (or, when not `audible`, its passing).
    pub fn make(&mut self, audible: bool) {
        let n = self.frames;
        let ctx = self.ctx();
        self.run_meta(n as f32 / RATE);
        self.tick_effects();
        self.run_phrases(n);
        for b in self.tracks.iter_mut().flat_map(|t| t.buf.iter_mut()).chain(self.chains.iter_mut().flat_map(|c| c.buf.iter_mut())) {
            b.clear();
            b.resize(n, 0.0);
        }
        // This tick's events, in order through the block.
        let mut evs: Vec<Event> = Vec::new();
        let tick = self.tick;
        self.events.retain(|e| {
            if e.tick == tick {
                evs.push(*e);
                false
            } else {
                true
            }
        });
        evs.sort_by_key(|e| e.offset);
        let mut at = 0;
        for e in evs {
            let o = e.offset.min(n);
            self.render_voices(at, o, audible, &ctx);
            at = o;
            let mut e = e;
            e.offset = 0;
            self.note_on(&e);
        }
        self.render_voices(at, n, audible, &ctx);
        self.voices.retain(|v| !v.dead);
        if audible {
            self.run_sample_chains(&ctx);
            self.mix(&ctx);
        }
    }

    /// The voices' frames `from..to` of the tick, into their tracks.
    fn render_voices(&mut self, from: usize, to: usize, audible: bool, ctx: &Ctx) {
        if to <= from {
            return;
        }
        let beat = self.beat_seconds();
        let env_ticks_per_frame = beat.recip() * 24.0 / RATE;
        let headroom = self.song.headroom;
        for vi in 0..self.voices.len() {
            let mut k = from;
            while k < to && !self.voices[vi].dead {
                let m = (to - k).min(CONTROL);
                let v = &mut self.voices[vi];
                let t = v.track;
                // Modulation for the chunk.
                let mut mo = super::env::Out::default();
                if let Some(e) = v.env.as_mut() {
                    mo = e.step(m as f32 * env_ticks_per_frame, v.released);
                    if mo.done {
                        v.cutting = true;
                    }
                    if !e.volume_on() && v.released {
                        v.cutting = true;
                    }
                }
                if let Some(md) = v.mods.as_mut() {
                    mo = md.step(m as f32 / RATE, beat, v.released);
                    if mo.done {
                        v.cutting = true;
                    }
                }
                let ch_vol = self.tracks[t].channel_vol;
                // Slides stop at Renoise's ten octaves either way.
                v.bend = v.bend.clamp(-120.0, 120.0);
                let semis = (v.semis + v.bend + v.arp + v.vib + mo.pitch).clamp(-120.0, 120.0);
                let step = v.base_step * 2f64.powf(f64::from(semis) / 12.0);
                let vol = v.vol * v.sample_vol * mo.volume * v.trem * ch_vol * headroom;
                let pan = (v.pan + (v.sample_pan - 0.5) + mo.pan * 0.5 + v.auto_pan).clamp(0.0, 1.0);
                let (gl, gr) = balance(pan);
                v.filter.cutoff = mo.cutoff;
                v.filter.reso = mo.reso;
                v.filter.drive = mo.drive;
                if audible {
                    let chain = usize::try_from(v.chain).ok().filter(|&c| self.song.instruments.get(v.instr).is_some_and(|i| c < i.chains.len()));
                    let buf = match chain {
                        Some(c) => {
                            let key = (t, v.instr, c);
                            let at = match self.chains.iter().position(|x| (x.track, x.instr, x.chain) == key) {
                                Some(at) => at,
                                None => {
                                    let devs = self.song.instruments[v.instr].chains[c].iter().map(Dev::new).collect();
                                    let n = self.frames;
                                    self.chains.push(InsChain {
                                        track: t,
                                        instr: v.instr,
                                        chain: c,
                                        devs,
                                        buf: [vec![0.0; n], vec![0.0; n]],
                                    });
                                    self.chains.len() - 1
                                }
                            };
                            &mut self.chains[at].buf
                        }
                        None => &mut self.tracks[t].buf,
                    };
                    let (l, r) = buf.split_at_mut(1);
                    v.render((&mut l[0][k..k + m], &mut r[0][k..k + m]), (vol * gl, vol * gr), step);
                } else {
                    v.skip(m, step);
                }
                k += m;
            }
        }
        let _ = ctx;
    }

    /// Renoise 3's sample effect chains, each into its track.
    fn run_sample_chains(&mut self, ctx: &Ctx) {
        let n = self.frames;
        for ch in self.chains.iter_mut() {
            let mut post = (1.0f32, 0.5f32);
            let [l, r] = &mut ch.buf;
            for d in ch.devs.iter_mut().filter(|d| d.active) {
                if d.def.kind == "SampleMixerDevice" {
                    let (gl, gr) = balance(d.get("Panning", 0.5));
                    let vol = d.get("Volume", 1.0);
                    post = (d.get("PostVolume", 1.0), d.get("PostPanning", 0.5));
                    for i in 0..n {
                        l[i] *= vol * gl;
                        r[i] *= vol * gr;
                    }
                } else if let Some(p) = d.proc.as_mut() {
                    let params = Params { def: &d.def, values: &d.values };
                    p.process(&params, l, r, ctx);
                }
            }
            let (gl, gr) = balance(post.1);
            let tb = &mut self.tracks[ch.track].buf;
            for i in 0..n {
                tb[0][i] += l[i] * post.0 * gl;
                tb[1][i] += r[i] * post.0 * gr;
            }
        }
    }

    /// The tracks' chains, their routing, the master. Each track's channel
    /// is what it sends straight to the master, and a share (by how loud
    /// it went in) of what groups, send tracks and the master's own chain
    /// make of the whole.
    fn mix(&mut self, ctx: &Ctx) {
        let n = self.frames;
        let order: Vec<usize> = (0..self.tracks.len())
            .filter(|&t| matches!(self.tracks[t].kind, TrackKind::Normal | TrackKind::Group))
            .chain(self.sends.iter().copied())
            .collect();
        for b in self.master_buf.iter_mut() {
            b.clear();
            b.resize(n, 0.0);
        }
        for o in self.out.iter_mut() {
            for b in o.iter_mut() {
                b.clear();
                b.resize(n, 0.0);
            }
        }
        let mut energy = vec![0.0f32; self.outputs.len()];
        for &t in &order {
            let muted = self.track_muted(t);
            let mut buf = std::mem::take(&mut self.tracks[t].buf);
            if muted {
                for b in buf.iter_mut() {
                    b.fill(0.0);
                }
            }
            let slot = self.outputs.iter().position(|&x| x == t);
            if let Some(k) = slot {
                energy[k] = buf[0].iter().chain(buf[1].iter()).map(|x| x * x).sum();
            }
            self.run_chain(t, &mut buf, ctx);
            match self.tracks[t].to {
                Some(g) => {
                    let gb = &mut self.tracks[g].buf;
                    for c in 0..2 {
                        for i in 0..n {
                            gb[c][i] += buf[c][i];
                        }
                    }
                }
                None => {
                    for c in 0..2 {
                        for i in 0..n {
                            self.master_buf[c][i] += buf[c][i];
                        }
                    }
                    if let Some(k) = slot {
                        self.out[k] = [buf[0].clone(), buf[1].clone()];
                    }
                }
            }
            self.tracks[t].buf = buf;
        }
        if let Some(m) = self.master {
            let mut buf = std::mem::take(&mut self.master_buf);
            self.run_chain(m, &mut buf, ctx);
            let dev = self.tracks[m].chain.iter().find(|d| d.is_mixer());
            let (soft, dc) = dev.map_or((false, false), |d| (d.def.flag("SoftClipping"), d.def.flag("AutoDcCorrection")));
            for i in 0..n {
                for c in 0..2 {
                    let mut x = buf[c][i];
                    if dc {
                        // A gentle DC filter.
                        self.dc[c] = 0.9995 * self.dc[c] + x - self.dc[c + 2];
                        self.dc[c + 2] = x;
                        x = self.dc[c];
                    }
                    if soft {
                        x = x.tanh();
                    }
                    buf[c][i] = x;
                }
            }
            self.master_buf = buf;
        }
        // What's left of the master once each track's own part is taken,
        // shared out by how loud each went in.
        let total: f32 = energy.iter().sum();
        let k = self.out.len();
        let mut rest = self.master_buf.clone();
        for o in &self.out {
            for c in 0..2 {
                for i in 0..n {
                    rest[c][i] -= o[c][i];
                }
            }
        }
        for (j, o) in self.out.iter_mut().enumerate() {
            let share = if total > 0.0 { energy[j] / total } else { 1.0 / k.max(1) as f32 };
            for c in 0..2 {
                for i in 0..n {
                    o[c][i] += rest[c][i] * share;
                }
            }
        }
    }

    /// A track's chain over its block: the pre mixer, devices, sends, the
    /// post mixer.
    fn run_chain(&mut self, t: usize, buf: &mut [Vec<f32>; 2], ctx: &Ctx) {
        let n = buf[0].len();
        let mut post = (1.0f32, 0.5f32);
        let sends = self.sends.clone();
        let doc = self.doc;
        for di in 0..self.tracks[t].chain.len() {
            let d = &mut self.tracks[t].chain[di];
            if !d.active {
                continue;
            }
            let kind = d.def.kind.clone();
            if d.is_mixer() {
                let (vol, pan) = (d.get("Volume", 1.0), d.get("Panning", 0.5));
                post = (d.get("PostVolume", 1.0), d.get("PostPanning", 0.5));
                let (gl, gr) = balance(pan);
                for i in 0..n {
                    buf[0][i] *= vol * gl;
                    buf[1][i] *= vol * gr;
                }
                continue;
            }
            match kind.as_str() {
                "SendDevice" | "SequencerTrackSendDevice" | "Device" => {
                    let amount = d.get("SendAmount", 1.0);
                    let dest = d.get("DestSendTrack", 0.0).round() as usize;
                    let mute = d.def.flag("MuteSource");
                    let pan = d.get("SendPan", 0.5);
                    let (pl, pr) = balance(pan);
                    if let Some(&s) = sends.get(dest) {
                        if s != t {
                            let sb = &mut self.tracks[s].buf;
                            if sb[0].len() == n {
                                for i in 0..n {
                                    sb[0][i] += buf[0][i] * amount * pl;
                                    sb[1][i] += buf[1][i] * amount * pr;
                                }
                            }
                        }
                    }
                    if mute {
                        buf[0].fill(0.0);
                        buf[1].fill(0.0);
                    }
                }
                "GainerDevice" | "GainerDocument" => {
                    let (vol, pan) = (d.get("Volume", 1.0), d.get("Panning", 0.5));
                    let (gl, gr) = balance(pan);
                    let il = if d.def.flag("LPhaseInvert") { -1.0 } else { 1.0 };
                    let ir = if d.def.flag("RPhaseInvert") { -1.0 } else { 1.0 };
                    for i in 0..n {
                        buf[0][i] *= vol * gl * il;
                        buf[1][i] *= vol * gr * ir;
                    }
                }
                "SignalFollowerDevice" => {
                    // Its level: the block's peak, attacked and released.
                    let peak = buf[0].iter().chain(buf[1].iter()).fold(0.0f32, |a, &x| a.max(x.abs()));
                    let secs = n as f32 / RATE;
                    let tau = if peak > d.level { d.get("Attack", 0.01).max(1e-4) } else { d.get("Release", 0.2).max(1e-4) };
                    d.level += (peak - d.level) * (1.0 - (-secs / tau).exp());
                }
                "DooferDevice" => {
                    for inner in d.inner.iter_mut().filter(|x| x.active) {
                        if let Some(p) = inner.proc.as_mut() {
                            let params = Params {
                                def: &inner.def,
                                values: &inner.values,
                            };
                            let (l, r) = buf.split_at_mut(1);
                            p.process(&params, &mut l[0], &mut r[0], ctx);
                        }
                    }
                }
                _ => {
                    if let Some(p) = d.proc.as_mut() {
                        let params = Params { def: &d.def, values: &d.values };
                        let (l, r) = buf.split_at_mut(1);
                        p.process(&params, &mut l[0], &mut r[0], ctx);
                    }
                }
            }
            let _ = doc;
        }
        let (gl, gr) = balance(post.1);
        for i in 0..n {
            buf[0][i] *= post.0 * gl;
            buf[1][i] *= post.0 * gr;
        }
    }
}

/// What a line's effects ask of its notes.
#[derive(Clone, Copy, Debug, Default)]
struct LineFx {
    delay: u32,
    maybe: Option<u8>,
    offset: Option<u8>,
    backwards: Option<bool>,
    glide: bool,
}

fn line_of(pt: &PatternTrack, line: usize) -> Option<&super::song::Line> {
    pt.lines.binary_search_by_key(&line, |(at, _)| *at).ok().map(|i| &pt.lines[i].1)
}

/// An automation's value at `at` lines.
fn automation_at(points: &[(f64, f32)], interp: Interp, at: f64) -> Option<f32> {
    if points.is_empty() {
        return None;
    }
    if at <= points[0].0 {
        return Some(points[0].1);
    }
    for w in points.windows(2) {
        let ((t0, v0), (t1, v1)) = (w[0], w[1]);
        if at < t1 {
            let f = if t1 > t0 { ((at - t0) / (t1 - t0)) as f32 } else { 1.0 };
            return Some(match interp {
                Interp::Points => v0,
                Interp::Linear => v0 + (v1 - v0) * f,
                Interp::Curve => v0 + (v1 - v0) * (f * f * (3.0 - 2.0 * f)),
            });
        }
    }
    Some(points[points.len() - 1].1)
}

/// A meta device's scaling of 0..1.
fn scale(name: Option<&str>, x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    match name.unwrap_or("Linear") {
        "Log Fast" => x.powf(0.25),
        "Log Slow" => x.sqrt(),
        "Exp Slow" => x * x,
        "Exp Fast" => x.powi(4),
        _ => x,
    }
}

/// A retrigger's volume change (the R command's x).
fn retrig_volume(v: f32, x: u8) -> f32 {
    let r = match x {
        1 => v - 0.03,
        2 => v - 0.06,
        3 => v - 0.12,
        4 => v - 0.25,
        5 => v - 0.5,
        6 => v * 2.0 / 3.0,
        7 => v * 0.5,
        9 => v + 0.03,
        10 => v + 0.06,
        11 => v + 0.12,
        12 => v + 0.25,
        13 => v + 0.5,
        14 => v * 1.5,
        15 => v * 2.0,
        _ => v,
    };
    r.clamp(0.0, 1.0)
}

impl NoteCell {
    fn vol_value(&self) -> u8 {
        match self.vol {
            Col::Set(v) => v.min(0x80),
            _ => 0x80,
        }
    }
}
