// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! StoneTracker's player, as its own player plays (measured from it,
//! run as a black box on an emulated Paula and CIA):
//!
//! - A tick every `floor(1773447 / BPM) + 1` of the CIA's 709,379 a
//!   second (the first at 125 BPM: a song's own BPM counts from its
//!   second); the line's effects on its first tick, the rest on the
//!   others, ProTracker's way with StoneTracker's differences (no effect
//!   or vibrato position memory to reset, slides in 16 bits, the FX
//!   track's effects sent to the tracks it names).
//! - Periods `856 * 2^-((note - 1) * 8 + finetune) / 96`, rounded, never
//!   below 113; finetunes in eighths of a semitone, -16 to 15.
//! - A track Paula plays itself has its sample copied into chip memory a
//!   chunk at a time (about 16,000 colour clocks of it, by the period),
//!   the next chunk as Paula takes up the one before; a note stops the
//!   channel at the tick and starts it 769 CIA counts later. A sample
//!   given without a note follows when the playing one's loop ends.
//! - The mixed tracks are stepped through at period 128 (16.16 steps,
//!   `(128 << 16) / period`), each point scaled by the player's volume
//!   curve (louder than linear below full volume) and summed into blocks
//!   of 250, clipped to a byte; a block is mixed three points before the
//!   one ahead of it ends, so a mixed track's change is heard a block or
//!   two after its tick.

use std::collections::VecDeque;
use std::sync::Arc;

use super::{Line, Sample, SongDef};
use crate::paula::CLOCK;
use crate::replay::{Bufs, Driver};

/// Colour clocks the CIA counts one in.
const ECLOCK: u64 = 5;
/// The player's wait between stopping a channel and starting it again.
const DMA_WAIT: u64 = 769 * ECLOCK;
/// The mixer's period, and the points in its blocks.
const MIX_PERIOD: u32 = 128;
const MIX_POINTS: usize = 250;
/// The lowest period the player sets.
const PERIOD_MIN: i32 = 113;
/// The highest (the lowest note at the lowest finetune).
const PERIOD_MAX: i32 = 961;

/// The mixer's gain at each volume, over 2^14: louder than linear below
/// full volume (half volume about 0.55), measured from the player's
/// output (its curve isn't a plain power law).
const MIX_GAIN: [i32; 65] = [
    0, 443, 806, 1144, 1471, 1796, 2089, 2402, 2682, 2992, 3251, 3548, 3844, 4097, 4370, 4647, 4933, 5185, 5419, 5728, 5958, 6211, 6464, 6727, 7002, 7215, 7510, 7737, 7975, 8193, 8466, 8724, 8992,
    9170, 9453, 9738, 9935, 10136, 10445, 10658, 10874, 11091, 11317, 11544, 11777, 12015, 12256, 12504, 12760, 13018, 13280, 13406, 13684, 13957, 14101, 14386, 14530, 14824, 14972, 15274, 15430,
    15742, 15898, 16221, 16384,
];

/// ProTracker's vibrato and tremolo sine.
const SINE: [i32; 32] = [
    0, 24, 49, 74, 97, 120, 141, 161, 180, 197, 212, 224, 235, 244, 250, 253, 255, 253, 250, 244, 235, 224, 212, 197, 180, 161, 141, 120, 97, 74, 49, 24,
];

/// A note's period at a finetune.
pub fn period(note: i32, finetune: i32) -> i32 {
    let x = (note - 1) * 8 + finetune;
    ((856.0 * (-f64::from(x) / 96.0).exp2()).round() as i32).max(PERIOD_MIN)
}

/// What the player needs of a module.
#[derive(Debug)]
pub struct Shared {
    pub songs: Vec<SongDef>,
    pub patterns: Vec<Vec<Line>>,
    pub fx_patterns: Vec<Vec<Line>>,
    pub lines: usize,
    pub samples: Vec<Sample>,
    pub data: Arc<Vec<i8>>,
}

/// Where a channel's points come from.
#[derive(Clone, Debug, Default)]
enum Source {
    #[default]
    Silence,
    /// The player's chunks (a native track's sample, copied a chunk at a
    /// time into chip memory): the one playing, where in it, and the one
    /// Paula has taken up to play next.
    Chunks { cur: Vec<i8>, idx: usize, next: Option<Vec<i8>> },
    /// The mixer's blocks.
    Mixed,
}

/// A stretch of the bank: where reading is, where it ends, and the loop after.
#[derive(Clone, Copy, Debug, Default)]
struct Part {
    pos: usize,
    end: usize,
    looped: Option<(usize, usize)>,
}

/// The silent buffer a channel is left on once its sample has run out.
const SILENT_WORDS: usize = 0x73;

/// The player's copying of a native track's sample into Paula's chunks.
#[derive(Clone, Debug, Default)]
struct Feed {
    part: Part,
    /// The sample (and its loop) has run out.
    ended: bool,
    /// What follows when the part ends instead of its loop: another
    /// sample (an instrument given without a note), or nothing.
    swap: Option<Option<Part>>,
    /// The chunks' interrupt is on (the player copies one each time Paula takes one up).
    irq: bool,
    /// The chunk Paula plays next.
    latch: Vec<i8>,
}

/// Words in a chunk at a period (each lasts about 16,000 colour clocks).
fn chunk_words(period: u32) -> usize {
    let p = period.max(1) as usize;
    (64368 + p) / (8 * p)
}

impl Feed {
    /// The part runs out: its swap, its loop, or the end.
    fn part_ended(&mut self) {
        match self.swap.take() {
            Some(Some(p)) => self.part = p,
            Some(None) => self.ended = true,
            None => match self.part.looped {
                Some((ls, ll)) => {
                    self.part = Part {
                        pos: ls,
                        end: ls + ll,
                        looped: self.part.looped,
                    }
                }
                None => self.ended = true,
            },
        }
        if !self.ended && self.part.pos >= self.part.end {
            self.ended = true;
        }
    }

    /// The next chunk at `period`: the sample's points (zeros once it's
    /// run out), or the silent buffer, with the interrupt off, after.
    fn chunk(&mut self, data: &[i8], period: u32) -> Vec<i8> {
        if self.ended {
            self.irq = false;
            return vec![0; 2 * SILENT_WORDS];
        }
        let n = 2 * chunk_words(period);
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            if self.ended {
                out.push(0);
                continue;
            }
            out.push(data.get(self.part.pos).copied().unwrap_or(0));
            self.part.pos += 1;
            if self.part.pos >= self.part.end {
                self.part_ended();
            }
        }
        out
    }
}

/// One of Paula's channels: a point held a period at a time.
#[derive(Clone, Debug, Default)]
struct Chan {
    on: bool,
    period: u32,
    volume: u32,
    out: i8,
    /// When its next point starts.
    next: u64,
    src: Source,
    /// For a mixed channel: each of its tracks' share of the point playing.
    parts: [i16; 3],
}

/// A block of the mixer's: its points and each track's share.
#[derive(Clone, Debug)]
struct Block {
    points: Vec<i8>,
    parts: Vec<[i16; 3]>,
}

/// A track's state.
#[derive(Clone, Debug, Default)]
struct Track {
    /// 0-based sample, when one's been given.
    sample: Option<usize>,
    note: i32,
    finetune: i32,
    /// The period effects work on, and the one written this tick (the
    /// player keeps them in 16 bits, so a slide up can wrap).
    period: u16,
    out_period: u16,
    /// 0 to 64, and the one written: kept until a change of volume (or
    /// the tremolo) writes another.
    volume: i32,
    out_volume: i32,
    /// The track's own volume (Set/Add/Sub Abs Volume), 0 to 64.
    abs_volume: i32,
    // Effect memories.
    porta_target: u16,
    porta_speed: u16,
    glissando: bool,
    vib_speed: i32,
    vib_depth: i32,
    vib_pos: i32,
    vib_wave: u8,
    trem_speed: i32,
    trem_depth: i32,
    trem_pos: i32,
    trem_wave: u8,
    /// Where Retrig Note restarts the sample (the line's Sample Offset).
    retrig_offset: usize,
    loop_line: usize,
    loop_count: i32,
    /// The line's effects, for the ticks after its first.
    fx: Vec<(u8, u8)>,
    /// A note held back (Note Delay, or Next/Previous Note with a delay):
    /// the tick it starts at, silent till then, and the note it plays.
    delayed: Option<(u32, i32)>,
    /// Set Note's change due at a tick (no restart).
    pending_note: Option<(i32, u32)>,
    /// The sample offset a held-back note starts from.
    delayed_offset: usize,
    /// What a mixed track's part is followed by instead of its loop (see [`Feed::swap`]).
    mswap: Option<Option<Part>>,
    /// A mixed track's held-back note with a new sample: the voice stops on the line's next tick.
    mstop: bool,
    /// No note has started on the track yet (the player's quirk: a
    /// held-back note of a sample with no loop, as the track's first,
    /// starts at volume 0).
    fresh: bool,
    /// For a mixed track: where it is in its sample (16.16), and its part.
    mpos: u64,
    mend: usize,
    mloop: Option<(usize, usize)>,
    mon: bool,
}

/// What a tick asks of Paula, at its time.
#[derive(Clone, Debug)]
enum Event {
    /// A native track's channel starts its chunks (after the DMA wait).
    DmaOn { chan: usize },
    /// Its second chunk is copied and its interrupt turned on (a wait later).
    Arm { chan: usize },
    /// The mixed channels start (the first tick).
    StartMixed,
}

/// The player at a song of a module.
pub struct Player {
    m: Arc<Shared>,
    song: usize,
    voices: usize,
    /// Colour clocks: now, and when the next tick falls.
    now: u64,
    tick_len: u64,
    bpm: u32,
    speed: u32,
    tick: u32,
    position: usize,
    line: usize,
    /// Where the next line is read from (after a jump or break).
    next: Option<(usize, usize)>,
    line_delay: u32,
    stopped: bool,
    done: bool,
    visited: Vec<bool>,
    tracks: Vec<Track>,
    /// The tracks the FX track sends its effects to, bit 7 track 1.
    fx_mask: u8,
    chans: [Chan; 4],
    feeds: [Feed; 4],
    events: Vec<(u64, Event)>,
    /// The mixer: its channels' tracks, blocks queued for each, the time
    /// the next block is mixed, and which it is.
    groups: [Vec<usize>; 2],
    blocks: [VecDeque<Block>; 2],
    mix_start: Option<u64>,
    mixed: u64,
    /// Each channel's points, for comparing with the player's (tests).
    pub streams: Option<[Vec<(i8, u16, u8)>; 4]>,
    first: bool,
    started: bool,
    /// For rendering: when the tick ends, and the next frame's time.
    tick_end: u64,
    frame_time: f64,
}

impl Player {
    pub fn new(m: Arc<Shared>, song: usize) -> Player {
        let def = m.songs.get(song).cloned().unwrap_or_default();
        let voices = def.voices.clamp(4, 8);
        let mut groups: [Vec<usize>; 2] = [Vec::new(), Vec::new()];
        if voices > 4 {
            // Paula's third channel: tracks 3, 6, 7; its fourth: 4, 5, 8 (track 3 alone plays itself).
            for t in [2, 5, 6] {
                if t < voices && voices > 5 {
                    groups[0].push(t);
                }
            }
            for t in [3, 4, 7] {
                if t < voices {
                    groups[1].push(t);
                }
            }
        }
        let bpm = u32::from(def.bpm).max(32);
        let positions = def.positions();
        let mut p = Player {
            m,
            song,
            voices,
            now: 0,
            // The timer starts at 125 BPM: the song's own counts from its second tick.
            tick_len: tick_len(125),
            bpm,
            speed: 6,
            tick: 0,
            position: 0,
            line: 0,
            next: None,
            line_delay: 0,
            stopped: positions == 0,
            done: false,
            visited: vec![false; positions.max(1)],
            tracks: vec![
                Track {
                    abs_volume: 64,
                    fresh: true,
                    ..Track::default()
                };
                voices
            ],
            fx_mask: 0xFF,
            chans: Default::default(),
            feeds: Default::default(),
            events: Vec::new(),
            groups,
            blocks: Default::default(),
            mix_start: None,
            mixed: 0,
            streams: None,
            first: true,
            started: false,
            tick_end: 0,
            frame_time: 0.0,
        };
        for g in 0..2 {
            if !p.groups[g].is_empty() {
                p.chans[2 + g].src = Source::Mixed;
            }
        }
        p
    }

    fn def(&self) -> &SongDef {
        &self.m.songs[self.song.min(self.m.songs.len().saturating_sub(1))]
    }

    /// The Paula channel a track plays on, when it plays on its own.
    fn native(&self, t: usize) -> Option<usize> {
        (t < 4 && !self.groups.iter().any(|g| g.contains(&t))).then_some(t)
    }

    fn line_at(&self, t: usize) -> Line {
        let def = self.def();
        let pat = def.tracks.get(t).and_then(|l| l.get(self.position)).copied();
        pat.and_then(|p| self.m.patterns.get(usize::from(p))).and_then(|p| p.get(self.line)).cloned().unwrap_or_default()
    }

    fn fx_line(&self) -> Line {
        let def = self.def();
        let pat = match &def.fx {
            Some(l) => l.get(self.position).copied(),
            None => Some(0),
        };
        pat.and_then(|p| self.m.fx_patterns.get(usize::from(p))).and_then(|p| p.get(self.line)).cloned().unwrap_or_default()
    }

    /// One tick, at `now`.
    fn step(&mut self) {
        // Paula's points due now play before the tick's interrupt does anything.
        let now = self.now;
        for c in 0..4 {
            if self.chans[c].on && self.chans[c].next == now {
                self.point(c, now);
            }
        }
        if self.stopped {
            return;
        }
        if self.first {
            self.first = false;
            self.tick_len = tick_len(self.bpm);
            if self.groups.iter().any(|g| !g.is_empty()) {
                self.events.push((self.now + DMA_WAIT, Event::StartMixed));
            }
        }
        let first = self.now == 0 && self.tick == 0 && self.position == 0 && self.line == 0 && !self.started;
        if self.tick == 0 {
            if self.line_delay > 0 {
                self.line_delay -= 1;
                self.tick_effects(false);
            } else {
                self.read_line();
            }
        } else {
            self.tick_effects(true);
        }
        if first {
            self.started = true;
            // The first tick starts every channel Paula plays itself: a
            // held-back note's sample (at volume 0 till it starts), or silence.
            for c in 0..4 {
                if self.native(c).is_none() || self.events.iter().any(|e| matches!(e.1, Event::DmaOn { chan } if chan == c)) {
                    continue;
                }
                let held = self.tracks[c].delayed.and(self.tracks[c].sample).and_then(|s| self.m.samples.get(s)).cloned();
                match held {
                    // A held-back note's sample: with a loop, it plays (silent) from now.
                    Some(smp) if smp.loop_len > 2 => {
                        let off = self.tracks[c].delayed_offset;
                        self.start_feed(c, native_part(&smp, off), self.tracks[c].period.into());
                    }
                    Some(_) => {}
                    None => {
                        self.feeds[c] = Feed { ended: true, ..Feed::default() };
                        self.feeds[c].latch = vec![0; 2 * SILENT_WORDS];
                        self.events.push((self.now + DMA_WAIT, Event::DmaOn { chan: c }));
                    }
                }
            }
        }
        self.output();
        // The mixer's first block (from the first tick, as the channels
        // start) is silence: the first mixed is the next.
        if self.mix_start.is_none() && self.groups.iter().any(|g| !g.is_empty()) {
            self.mix_start = Some(self.now + DMA_WAIT);
            for g in 0..2 {
                if !self.groups[g].is_empty() {
                    self.blocks[g].push_back(Block {
                        points: vec![0; MIX_POINTS],
                        parts: vec![[0; 3]; MIX_POINTS],
                    });
                }
            }
            self.mixed = 1;
        }
        self.tick += 1;
        if self.tick >= self.speed.max(1) {
            self.tick = 0;
            if self.line_delay == 0 {
                self.advance();
            }
        }
    }

    fn advance(&mut self) {
        let positions = self.def().positions();
        let (pos, line) = match self.next.take() {
            Some(n) => n,
            None if self.line + 1 >= self.m.lines => (self.position + 1, 0),
            None => (self.position, self.line + 1),
        };
        let pos = if pos >= positions {
            self.done = true;
            0
        } else {
            pos
        };
        if pos != self.position || line < self.line {
            if self.visited.get(pos).copied().unwrap_or(false) && (pos != self.position || line <= self.line) {
                self.done = true;
            }
        }
        if let Some(v) = self.visited.get_mut(pos) {
            *v = true;
        }
        self.position = pos;
        self.line = line.min(self.m.lines.saturating_sub(1));
    }

    /// A line's first tick: the FX track's line and every track's.
    fn read_line(&mut self) {
        if let Some(v) = self.visited.get_mut(self.position) {
            *v = true;
        }
        let fxl = self.fx_line();
        let mut lines: Vec<Line> = (0..self.voices).map(|t| self.line_at(t)).collect();
        let mut flow_taken = false;
        // The FX track's commands for everyone, then those it sends to its tracks.
        let mut sent: Vec<(u8, u8)> = Vec::new();
        for &(e, p) in &fxl.fx {
            match e {
                0x20 => self.fx_mask = p,
                0x21 => self.fx_mask |= p,
                0x22 => self.fx_mask &= !p,
                _ if is_global(e) => self.global(e, p, &mut flow_taken),
                _ => sent.push((e, p)),
            }
        }
        for t in 0..self.voices {
            let mut l = std::mem::take(&mut lines[t]);
            if self.fx_mask & (0x80 >> t) != 0 && !sent.is_empty() {
                let mut fx = sent.clone();
                fx.extend(l.fx.drain(..));
                l.fx = fx;
            }
            for &(e, p) in &l.fx {
                if is_global(e) {
                    self.global(e, p, &mut flow_taken);
                }
            }
            self.track_line(t, l);
        }
    }

    /// The commands that work on the song, not a track.
    fn global(&mut self, e: u8, p: u8, flow_taken: &mut bool) {
        match e {
            0x0B if !*flow_taken => {
                *flow_taken = true;
                self.next = Some((usize::from(p), 0));
            }
            0x0D if !*flow_taken => {
                *flow_taken = true;
                self.next = Some((self.position + 1, usize::from(p)));
            }
            0x0F => match p {
                0 => self.stopped = true,
                1..=31 => self.speed = u32::from(p),
                _ => {
                    self.bpm = u32::from(p);
                    self.tick_len = tick_len(self.bpm);
                }
            },
            0x1E if self.line_delay == 0 => self.line_delay = u32::from(p),
            // The song jumps (29 to 2C) do nothing in StoneTracker's own player.
            _ => {}
        }
    }

    /// A track's line on its first tick.
    fn track_line(&mut self, t: usize, l: Line) {
        let has = |e: u8| l.fx.iter().any(|f| f.0 == e);
        let param = |e: u8| l.fx.iter().find(|f| f.0 == e).map(|f| f.1);
        {
            let tr = &mut self.tracks[t];
            tr.fx = l.fx.clone();
            tr.pending_note = None;
            tr.delayed = None;
            tr.mstop = false;
            tr.out_period = tr.period;
        }
        let was = self.tracks[t].sample;
        if l.inst != 0 {
            let s = usize::from(l.inst) - 1;
            if let Some(smp) = self.m.samples.get(s) {
                let tr = &mut self.tracks[t];
                tr.sample = Some(s);
                tr.volume = i32::from(smp.volume);
                tr.out_volume = tr.volume;
                tr.finetune = i32::from(smp.finetune);
            }
        }
        // A finetune on the line counts for its note (one out of range is ignored).
        for &(e, p) in &l.fx {
            if e == 0x15 && (-16..=15).contains(&(p as i8)) {
                self.tracks[t].finetune = i32::from(p as i8);
            }
        }
        // Sample Offset has no memory: 00 starts from the start.
        let offset = |_: &mut Track| param(0x09).map_or(0, |o| usize::from(o) << 8);
        if l.note != 0 {
            let note = i32::from(l.note);
            let target = period(note, self.tracks[t].finetune) as u16;
            if has(0x03) || has(0x05) {
                let tr = &mut self.tracks[t];
                tr.porta_target = target;
                tr.note = note;
            } else if let Some(p) = param(0x0E) {
                let tr = &mut self.tracks[t];
                if p == 0 {
                    tr.note = note;
                    tr.period = target;
                } else {
                    tr.pending_note = Some((note, u32::from(p)));
                }
            } else {
                let delay = param(0x1D).map_or(0, u32::from);
                let tr = &mut self.tracks[t];
                tr.note = note;
                tr.period = target;
                if delay > 0 && delay < self.speed {
                    // Silent till its tick.
                    tr.delayed = Some((delay, note));
                    tr.delayed_offset = offset(tr);
                    tr.out_volume = 0;
                } else if param(0x1C) == Some(0) {
                    tr.volume = 0;
                    tr.out_volume = 0;
                } else {
                    tr.fresh = false;
                    let off = offset(tr);
                    self.trigger(t, off);
                }
            }
        }
        // A new sample with no note started (or a held-back note's): its
        // loop follows when the part playing ends (nothing, when it has
        // none), as Paula's loop registers would on ProTracker.
        let now_sample = self.tracks[t].sample;
        if l.inst != 0 && now_sample != was && (l.note == 0 || self.tracks[t].delayed.is_some()) {
            let native = self.native(t).is_some();
            let swap = now_sample
                .and_then(|s| self.m.samples.get(s))
                .map(|smp| if native { native_part(smp, usize::MAX) } else { part_of(smp, usize::MAX) })
                .filter(|p| p.pos < p.end);
            match self.native(t) {
                Some(c) => self.feeds[c].swap = Some(swap),
                // The mixer: a held-back note's new sample leaves silence
                // (and starts at once, when the track was silent).
                None if l.note != 0 => self.tracks[t].mstop = true,
                None => self.tracks[t].mswap = Some(swap),
            }
        }
        // A mixed track that's silent starts a held-back note's sample at once.
        if self.native(t).is_none() && self.tracks[t].delayed.is_some() && !self.tracks[t].mon {
            let off = self.tracks[t].delayed_offset;
            self.trigger(t, off);
        }
        let no_note = l.note == 0;
        for &(e, p) in &l.fx {
            let (x, y) = (i32::from(p >> 4), i32::from(p & 15));
            let tr = &mut self.tracks[t];
            match e {
                0x03 | 0x05 => {
                    if e == 0x03 && p != 0 {
                        tr.porta_speed = u16::from(p);
                    }
                    // With no note on the line, the slide starts at once.
                    if no_note {
                        tone_porta(tr);
                        if e == 0x05 {
                            vol_slide(tr, x, y);
                        }
                    }
                }
                0x04 => {
                    if x != 0 {
                        tr.vib_speed = x;
                    }
                    if y != 0 {
                        tr.vib_depth = y;
                    }
                }
                0x07 => {
                    if x != 0 {
                        tr.trem_speed = x;
                    }
                    if y != 0 {
                        tr.trem_depth = y;
                    }
                }
                0x0C => set_volume(tr, i32::from(p).min(64)),
                0x30 | 0x31 => {
                    let note = if e == 0x30 { (tr.note + 1).min(36) } else { (tr.note - 1).max(1) };
                    if p == 0 {
                        tr.note = note;
                        tr.period = period(note, tr.finetune) as u16;
                        tr.out_period = tr.period;
                        self.trigger(t, 0);
                    } else if u32::from(p) < self.speed {
                        tr.delayed = Some((u32::from(p), note));
                        tr.delayed_offset = 0;
                        tr.out_volume = 0;
                    }
                }
                0x32 | 0x33 => {
                    let note = if e == 0x32 { (tr.note + 1).min(36) } else { (tr.note - 1).max(1) };
                    tr.note = note;
                    tr.porta_target = period(note, tr.finetune) as u16;
                    if p != 0 {
                        tr.porta_speed = u16::from(p);
                    }
                }
                0x11 => slide_up(tr, p),
                0x12 => slide_down(tr, p),
                0x13 => tr.glissando = p & 15 != 0,
                0x14 => tr.vib_wave = p & 15,
                0x16 => {
                    if p == 0 {
                        tr.loop_line = self.line;
                    } else if tr.loop_count == 0 {
                        tr.loop_count = i32::from(p);
                        self.next = Some((self.position, tr.loop_line));
                    } else {
                        tr.loop_count -= 1;
                        if tr.loop_count > 0 {
                            self.next = Some((self.position, tr.loop_line));
                        }
                    }
                }
                0x17 => tr.trem_wave = p & 15,
                0x1A => set_volume(tr, (tr.volume + i32::from(p)).min(64)),
                0x1B => set_volume(tr, (tr.volume - i32::from(p)).max(0)),
                0x1C if p == 0 => set_volume(tr, 0),
                // A held-back note is silent from here (a volume set after it is heard).
                0x1D if tr.delayed.is_some() => tr.out_volume = 0,
                0x23 => tr.abs_volume = i32::from(p).min(64),
                0x24 | 0x26 => tr.abs_volume = (tr.abs_volume + i32::from(p)).min(64),
                0x25 | 0x27 => tr.abs_volume = (tr.abs_volume - i32::from(p)).max(0),
                _ => {}
            }
        }
        let tr = &mut self.tracks[t];
        tr.retrig_offset = param(0x09).map_or(0, |o| usize::from(o) << 8);
        tr.out_period = tr.period;
        if tr.glissando && l.fx.iter().any(|f| matches!(f.0, 0x03 | 0x05 | 0x32 | 0x33)) {
            tr.out_period = glissando(tr.out_period, tr.finetune);
        }
    }

    /// A native channel starts a part: stopped now, its first chunk
    /// copied at `period`, started after the DMA wait, its second chunk
    /// and interrupt a wait after that.
    fn start_feed(&mut self, c: usize, part: Part, period: u32) {
        self.chans[c].on = false;
        self.events.retain(|e| !matches!(e.1, Event::DmaOn { chan } | Event::Arm { chan } if chan == c));
        let f = &mut self.feeds[c];
        *f = Feed {
            part,
            ended: part.pos >= part.end,
            ..Feed::default()
        };
        let data = self.m.data.clone();
        f.latch = f.chunk(&data, period);
        self.events.push((self.now + DMA_WAIT, Event::DmaOn { chan: c }));
        self.events.push((self.now + 2 * DMA_WAIT, Event::Arm { chan: c }));
    }

    /// The track's sample starts (from `offset`).
    fn trigger(&mut self, t: usize, offset: usize) {
        let Some(s) = self.tracks[t].sample else { return };
        let Some(smp) = self.m.samples.get(s).cloned() else { return };
        let part = part_of(&smp, offset);
        let (pos, end, looped) = (part.pos, part.end, part.looped);
        if let Some(c) = self.native(t) {
            self.start_feed(c, native_part(&smp, offset), u32::from(self.tracks[t].period));
        } else {
            let tr = &mut self.tracks[t];
            tr.mswap = None;
            tr.mpos = (pos as u64) << 16;
            tr.mend = end;
            tr.mloop = looped;
            tr.mon = pos < end || looped.is_some();
        }
    }

    /// The effects on the ticks after a line's first (and on its first
    /// again while a Position Delay holds it, `later` false).
    fn tick_effects(&mut self, later: bool) {
        let tick = self.tick;
        for t in 0..self.voices {
            if !later {
                continue;
            }
            self.tracks[t].out_period = self.tracks[t].period;
            if std::mem::take(&mut self.tracks[t].mstop) && self.tracks[t].delayed.is_some() {
                self.tracks[t].mon = false;
            }
            if let Some((at, note)) = self.tracks[t].delayed {
                if tick == at {
                    let tr = &mut self.tracks[t];
                    tr.delayed = None;
                    tr.note = note;
                    tr.period = period(note, tr.finetune) as u16;
                    tr.out_period = tr.period;
                    let looped = tr.sample.and_then(|s| self.m.samples.get(s)).is_some_and(|s| s.loop_len > 2);
                    if tr.fresh && !looped && t < 4 && !self.groups.iter().any(|g| g.contains(&t)) {
                        tr.out_volume = 0;
                    } else {
                        tr.fresh = false;
                        tr.out_volume = tr.volume;
                    }
                    let off = tr.delayed_offset;
                    self.trigger(t, off);
                }
            }
            if let Some((note, at)) = self.tracks[t].pending_note {
                if tick == at {
                    let tr = &mut self.tracks[t];
                    tr.pending_note = None;
                    tr.note = note;
                    tr.period = period(note, tr.finetune) as u16;
                    tr.out_period = tr.period;
                }
            }
            let fx = self.tracks[t].fx.clone();
            let mut porta = false;
            for (e, p) in fx {
                let (x, y) = (i32::from(p >> 4), i32::from(p & 15));
                let tr = &mut self.tracks[t];
                match e {
                    0x00 if p != 0 => {
                        let k = match tick % 3 {
                            0 => 0,
                            1 => x,
                            _ => y,
                        };
                        tr.out_period = if k == 0 { tr.period } else { period(tr.note + k, tr.finetune) as u16 };
                    }
                    0x01 => {
                        slide_up(tr, p);
                        tr.out_period = tr.period;
                    }
                    0x02 => {
                        slide_down(tr, p);
                        tr.out_period = tr.period;
                    }
                    0x03 | 0x32 | 0x33 | 0x05 => {
                        tone_porta(tr);
                        tr.out_period = tr.period;
                        porta = true;
                        if e == 0x05 {
                            vol_slide(tr, x, y);
                        }
                    }
                    0x04 | 0x06 => {
                        if e == 0x06 {
                            vol_slide(tr, x, y);
                        }
                        if tr.vib_wave <= 2 {
                            let d = (wave(tr.vib_wave, tr.vib_pos) * tr.vib_depth) >> 7;
                            tr.out_period = (i32::from(tr.period) + d) as u16;
                        }
                        tr.vib_pos += tr.vib_speed;
                    }
                    0x07 => {
                        if tr.trem_wave <= 2 {
                            let d = (wave(tr.trem_wave, tr.trem_pos) * tr.trem_depth) >> 6;
                            tr.out_volume = (tr.volume + d).clamp(0, 64);
                        }
                        tr.trem_pos += tr.trem_speed;
                    }
                    0x0A => vol_slide(tr, x, y),
                    0x19 if p != 0 && tick % u32::from(p) == 0 => {
                        let off = tr.retrig_offset;
                        self.trigger(t, off);
                    }
                    0x1C if u32::from(p) == tick => set_volume(tr, 0),
                    0x24 => tr.abs_volume = (tr.abs_volume + i32::from(p)).min(64),
                    0x25 => tr.abs_volume = (tr.abs_volume - i32::from(p)).max(0),
                    _ => {}
                }
            }
            let tr = &mut self.tracks[t];
            if porta && tr.glissando {
                tr.out_period = glissando(tr.out_period, tr.finetune);
            }
        }
    }

    /// Each track's period and volume played this tick, to Paula (or the mixer).
    fn output(&mut self) {
        for t in 0..self.voices {
            let tr = &self.tracks[t];
            // The track volume scales it, rounded.
            let vol = ((tr.out_volume * tr.abs_volume + 32) >> 6) as u32;
            if let Some(c) = self.native(t) {
                self.chans[c].period = u32::from(tr.out_period);
                self.chans[c].volume = vol;
            }
        }
    }

    /// The next block of each mixed channel, from its tracks as they are now.
    fn mix_block(&mut self) {
        for g in 0..2 {
            if self.groups[g].is_empty() {
                continue;
            }
            let mut points = Vec::with_capacity(MIX_POINTS);
            let mut parts = Vec::with_capacity(MIX_POINTS);
            let data = self.m.data.clone();
            let members = self.groups[g].clone();
            let steps: Vec<u64> = members.iter().map(|&t| (u64::from(MIX_PERIOD) << 16) / self.tracks[t].out_period.max(1) as u64).collect();
            for _ in 0..MIX_POINTS {
                let mut part = [0i16; 3];
                let mut sum = 0i32;
                for (k, &t) in members.iter().enumerate() {
                    let tr = &mut self.tracks[t];
                    if !tr.mon {
                        continue;
                    }
                    let at = (tr.mpos >> 16) as usize;
                    let x = i32::from(data.get(at).copied().unwrap_or(0));
                    let v = (x * MIX_GAIN[((tr.out_volume * tr.abs_volume + 32) >> 6).clamp(0, 64) as usize]) >> 14;
                    part[k] = v as i16;
                    sum += v;
                    tr.mpos += steps[k];
                    while (tr.mpos >> 16) as usize >= tr.mend {
                        if let Some(sw) = tr.mswap.take() {
                            match sw {
                                Some(p) => {
                                    let over = tr.mpos - ((tr.mend as u64) << 16);
                                    tr.mpos = ((p.pos as u64) << 16) + over;
                                    tr.mend = p.end;
                                    tr.mloop = p.looped;
                                    continue;
                                }
                                None => {
                                    tr.mon = false;
                                    break;
                                }
                            }
                        }
                        match tr.mloop {
                            Some((ls, ll)) => {
                                // On to the loop's start (the sample may run past the loop's end first).
                                let over = tr.mpos - ((tr.mend as u64) << 16);
                                tr.mpos = ((ls as u64) << 16) + over;
                                tr.mend = ls + ll;
                            }
                            None => {
                                tr.mon = false;
                                break;
                            }
                        }
                    }
                }
                points.push(sum.clamp(-128, 127) as i8);
                parts.push(part);
            }
            self.blocks[g].push_back(Block { points, parts });
        }
        self.mixed += 1;
    }

    /// When the next block is mixed: the first as the mixer starts, then
    /// each as its channel fetches the last word of the one before (three
    /// points before it ends).
    fn next_mix_time(&self) -> Option<u64> {
        let start = self.mix_start?;
        let block = (MIX_POINTS as u64) * u64::from(MIX_PERIOD);
        Some(match self.mixed {
            0 => self.now,
            1 => start,
            m => start + (m - 1) * block - 3 * u64::from(MIX_PERIOD),
        })
    }

    /// Paula and the mixer run on to `until` (colour clocks).
    fn run_to(&mut self, until: u64) {
        loop {
            // The earliest thing due: a DMA start, a mix, a channel's next point.
            let mut t = until;
            for (at, _) in &self.events {
                t = t.min(*at);
            }
            if let Some(m) = self.next_mix_time() {
                t = t.min(m);
            }
            for c in &self.chans {
                if c.on {
                    t = t.min(c.next);
                }
            }
            if t >= until {
                break;
            }
            // Ties: Paula's points first, then the mixer (its interrupt), then DMA starts.
            let due: Vec<usize> = (0..4).filter(|&c| self.chans[c].on && self.chans[c].next == t).collect();
            for c in due {
                self.point(c, t);
            }
            if self.next_mix_time() == Some(t) && self.mix_start.is_some() && self.mixed > 0 {
                self.mix_block();
            }
            let mut fire = Vec::new();
            self.events.retain(|(at, e)| {
                if *at == t {
                    fire.push(e.clone());
                    false
                } else {
                    true
                }
            });
            for e in fire {
                match e {
                    Event::DmaOn { chan } => {
                        let c = &mut self.chans[chan];
                        c.src = Source::Chunks {
                            cur: self.feeds[chan].latch.clone(),
                            idx: 0,
                            next: None,
                        };
                        c.on = true;
                        self.point(chan, t);
                    }
                    Event::Arm { chan } => {
                        let data = self.m.data.clone();
                        let period = self.chans[chan].period;
                        let f = &mut self.feeds[chan];
                        f.irq = true;
                        f.latch = f.chunk(&data, period);
                    }
                    Event::StartMixed => {
                        for g in 0..2 {
                            if !self.groups[g].is_empty() {
                                let c = &mut self.chans[2 + g];
                                c.on = true;
                                c.period = MIX_PERIOD;
                                c.volume = 64;
                                self.point(2 + g, t);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Channel `c`'s next point starts at `t`.
    fn point(&mut self, c: usize, t: u64) {
        let data = self.m.data.clone();
        let ch = &mut self.chans[c];
        match &mut ch.src {
            Source::Silence => ch.out = 0,
            Source::Chunks { cur, idx, next } => {
                let f = &mut self.feeds[c];
                if *idx >= cur.len() {
                    *cur = next.take().unwrap_or_else(|| f.latch.clone());
                    *idx = 0;
                }
                ch.out = cur.get(*idx).copied().unwrap_or(0);
                *idx += 1;
                // Its last point: Paula takes up the chunk latched, and its
                // interrupt has the player copy the one after.
                if *idx == cur.len() {
                    *next = Some(f.latch.clone());
                    if f.irq {
                        f.latch = f.chunk(&data, ch.period);
                    }
                }
            }
            Source::Mixed => {
                let g = c - 2;
                let mut out = 0;
                let mut parts = [0i16; 3];
                if let Some(b) = self.blocks[g].front_mut() {
                    let i = MIX_POINTS - b.points.len().min(MIX_POINTS);
                    let _ = i;
                    if !b.points.is_empty() {
                        out = b.points.remove(0);
                        parts = b.parts.remove(0);
                    }
                    if b.points.is_empty() {
                        self.blocks[g].pop_front();
                    }
                }
                ch.out = out;
                ch.parts = parts;
            }
        }
        ch.next = t + u64::from(ch.period.max(1));
        if let Some(s) = self.streams.as_mut() {
            s[c].push((ch.out, ch.period as u16, ch.volume.min(255) as u8));
        }
    }

    /// A track's point now, left and right, at the Tracker's level.
    fn track_out(&self, t: usize) -> (f32, f32) {
        let (c, x) = match self.native(t) {
            Some(c) => (c, f32::from(self.chans[c].out) * self.chans[c].volume.min(64) as f32),
            None => {
                let g = usize::from(self.groups[1].contains(&t));
                let k = self.groups[g].iter().position(|&m| m == t).unwrap_or(0);
                let ch = &self.chans[2 + g];
                if !ch.on {
                    return (0.0, 0.0);
                }
                // Its share of the point, the clipping shared out with the others'.
                let sum: i32 = ch.parts.iter().map(|&p| i32::from(p)).sum();
                let mine = f32::from(ch.parts[k]);
                let x = if sum != i32::from(ch.out) && sum != 0 { mine * f32::from(ch.out) / sum as f32 } else { mine };
                (2 + g, x * 64.0)
            }
        };
        if !self.chans[c].on && self.native(t).is_none() {
            return (0.0, 0.0);
        }
        let v = x / 64.0 * 0.4 / 128.0;
        if matches!(c, 0 | 3) {
            (v, 0.0)
        } else {
            (0.0, v)
        }
    }

    /// Plays `secs` seconds, each channel's point times its volume every
    /// `n` colour clocks from the first tick (tests).
    pub fn run_grid(&mut self, secs: f64, n: u64) -> Vec<[i16; 4]> {
        let end = (secs * CLOCK) as u64;
        let mut out = Vec::new();
        let mut g = 0;
        while self.now < end {
            let next = self.now + self.tick_len;
            // A point at the tick's own time is the state before it.
            while g <= self.now {
                self.run_to(g);
                out.push(std::array::from_fn(|c| i16::from(self.chans[c].out) * self.chans[c].volume.min(64) as i16));
                g += n;
            }
            self.step();
            while g < next {
                self.run_to(g);
                out.push(std::array::from_fn(|c| i16::from(self.chans[c].out) * self.chans[c].volume.min(64) as i16));
                g += n;
            }
            self.run_to(next);
            self.now = next;
        }
        out
    }

    /// Each tick's (position, line, tick, period, volume) of Paula's channel `c` (tests).
    pub fn trace(&mut self, ticks: usize, c: usize) -> Vec<(usize, usize, u32, u32, u32)> {
        let mut out = Vec::new();
        for _ in 0..ticks {
            let (pos, line, tick) = (self.position, self.line, self.tick);
            let next = self.now + self.tick_len;
            self.step();
            out.push((pos, line, tick, self.chans[c].period, self.chans[c].volume));
            self.run_to(next);
            self.now = next;
        }
        out
    }

    /// Plays `secs` seconds, keeping each channel's points (tests).
    pub fn run_streams(&mut self, secs: f64) -> [Vec<(i8, u16, u8)>; 4] {
        self.streams = Some(Default::default());
        let end = (secs * CLOCK) as u64;
        while self.now < end {
            let next = self.now + self.tick_len;
            self.step();
            self.run_to(next);
            self.now = next;
        }
        self.streams.take().unwrap()
    }
}

/// A sample's part from an offset (past its end: its loop, or nothing).
fn part_of(smp: &Sample, offset: usize) -> Part {
    let looped = (smp.loop_len > 2).then_some((smp.loop_start, smp.loop_len));
    let end = smp.start + smp.len;
    if offset < smp.len {
        return Part { pos: smp.start + offset, end, looped };
    }
    match looped {
        Some((ls, ll)) => Part { pos: ls, end: ls + ll, looped },
        None => Part { pos: end, end, looped },
    }
}

/// The same for Paula's chunks, which the player copies a word at a
/// time: an odd length loses its last byte.
fn native_part(smp: &Sample, offset: usize) -> Part {
    let mut p = part_of(smp, offset);
    let first = p.pos < smp.start + smp.len && p.end == smp.start + smp.len;
    if first {
        p.end = smp.start + (smp.len & !1);
    }
    p.looped = p.looped.map(|(ls, ll)| (ls, ll & !1));
    if !first {
        p.end = p.pos.max(p.end.min(p.looped.map_or(p.end, |(ls, ll)| ls + ll)));
    }
    p
}

fn set_volume(tr: &mut Track, v: i32) {
    tr.volume = v;
    tr.out_volume = v;
}

fn vol_slide(tr: &mut Track, x: i32, y: i32) {
    set_volume(tr, if x != 0 { (tr.volume + x).min(64) } else { (tr.volume - y).max(0) });
}

/// A slide up: the period falls, wrapping below 0 (its floor is checked unsigned).
fn slide_up(tr: &mut Track, by: u8) {
    tr.period = tr.period.wrapping_sub(u16::from(by));
    if tr.period < PERIOD_MIN as u16 {
        tr.period = PERIOD_MIN as u16;
    }
}

fn slide_down(tr: &mut Track, by: u8) {
    tr.period = tr.period.wrapping_add(u16::from(by));
    if tr.period > PERIOD_MAX as u16 {
        tr.period = PERIOD_MAX as u16;
    }
}

fn tone_porta(tr: &mut Track) {
    if tr.porta_target == 0 {
        return;
    }
    if tr.period > tr.porta_target {
        tr.period = tr.period.saturating_sub(tr.porta_speed).max(tr.porta_target);
    } else if tr.period < tr.porta_target {
        tr.period = tr.period.saturating_add(tr.porta_speed).min(tr.porta_target);
    }
}

/// A period held to a semitone: the first of the finetune's notes at or below it.
fn glissando(p: u16, finetune: i32) -> u16 {
    (1..=36).map(|n| period(n, finetune) as u16).find(|&q| q <= p).unwrap_or(period(36, finetune) as u16)
}

/// A vibrato or tremolo wave at a position: 0 a sine, 1 a ramp, 2 a square.
fn wave(kind: u8, pos: i32) -> i32 {
    let i = (pos & 31) as usize;
    let neg = pos & 32 != 0;
    let v = match kind & 3 {
        0 => SINE[i],
        // The player's ramp: a sawtooth twice ProTracker's, 16 a step.
        1 => return if neg { (i as i32) * 16 - 512 } else { (i as i32) * 16 },
        2 => 255,
        _ => 0,
    };
    if neg {
        -v
    } else {
        v
    }
}

/// The effects that work on the song, whichever track they're on.
fn is_global(e: u8) -> bool {
    matches!(e, 0x0B | 0x0D | 0x0F | 0x1E | 0x28 | 0x29 | 0x2A | 0x2B | 0x2C)
}

/// A tick's length in colour clocks at a BPM: the CIA's latch, plus one.
fn tick_len(bpm: u32) -> u64 {
    (1_773_447 / u64::from(bpm.max(1)) + 1) * ECLOCK
}

impl Driver for Player {
    fn channels(&self) -> usize {
        self.voices
    }

    fn tick(&mut self) {
        // Paula catches up to the tick, which then falls.
        if self.tick_end > 0 {
            self.run_to(self.tick_end);
            self.now = self.tick_end;
        }
        // A BPM set now counts from the next tick: this one keeps its length.
        let len = self.tick_len;
        self.step();
        self.tick_end = self.now + len;
    }

    fn tick_seconds(&self) -> f64 {
        // The tick just played (a new BPM counts from the next).
        (self.tick_end - self.now) as f64 / CLOCK
    }

    fn render(&mut self, rate: u32, frames: usize, out: &mut Bufs) {
        let step = CLOCK / f64::from(rate.max(1));
        for i in 0..frames {
            // Paula as it is at the frame's time (never past the tick to come).
            let t = (self.frame_time.ceil() as u64).min(self.tick_end);
            self.frame_time += step;
            self.run_to(t);
            for tr in 0..self.voices {
                let (l, r) = self.track_out(tr);
                out.left[tr][i] = l;
                out.right[tr][i] = r;
            }
        }
    }

    fn position(&self) -> (usize, usize) {
        (self.position, self.line)
    }

    fn speed(&self) -> (u32, u32) {
        (self.speed, self.bpm)
    }

    fn take_done(&mut self) -> bool {
        std::mem::take(&mut self.done)
    }

    fn playing(&self) -> bool {
        !self.stopped
    }
}
