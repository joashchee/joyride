// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! MGSDRV's songs (.mgs; Ain's MSX driver, 1992-): PSG (three channels),
//! SCC (five, wavetable) and MSX-MUSIC's OPLL (nine FM channels, or six and
//! the rhythm sounds), up to 17 tracks of compiled MML. Written clean, from
//! the format's published description (Mitsutaka Okazaki's mgs-format.md,
//! ISC) and MGSC's MML manual, its runtime behaviour matched against the
//! driver run as a black box (its chip registers each frame); none of the
//! driver's code was read.

/// Tracks: 1-3 PSG, 4-8 SCC, 9-17 FM (15-17 rhythm in rhythm mode).
pub const TRACKS: usize = 17;

/// The default pitches of octave 1, C to B: the PSG's and SCC's periods, the
/// FM's F-numbers (block 0), as the driver has them.
const PSG_TUNE: [u16; 12] = [0xd5d, 0xc9c, 0xbe7, 0xb3c, 0xa9b, 0xa02, 0x973, 0x8eb, 0x86b, 0x7f2, 0x780, 0x714];
const FM_TUNE: [u16; 12] = [0xac, 0xb6, 0xc2, 0xcd, 0xd9, 0xe6, 0xf4, 0x102, 0x111, 0x122, 0x133, 0x145];

/// An envelope (@e: a program of volumes; @r: attack, decay, sustain, release).
#[derive(Clone)]
enum Env {
    E { mode: u8, noise: u8, data: Vec<u8> },
    R { mode: u8, noise: u8, al: u8, ar: u8, dr: u8, sl: u8, sr: u8, rr: u8 },
}

impl Env {
    fn mode_noise(&self) -> (u8, u8) {
        match *self {
            Env::E { mode, noise, .. } | Env::R { mode, noise, .. } => (mode, noise),
        }
    }
}

#[derive(Clone)]
pub struct Song {
    file: Vec<u8>,
    envs: Vec<Option<Env>>,
    fm_voices: [[u8; 8]; 32],
    // Each voice's ROM voice (0 its own).
    rom: [u8; 32],
    waves: [[u8; 32]; 32],
    psg_tune: [u16; 12],
    fm_tune: [u16; 12],
    /// What track 0 defined, in order: (0 an FM voice, 2 an envelope, 3 an SCC wave; its number).
    defined: Vec<(u8, u8)>,
    /// The binary part: its header, then the tracks.
    data: Vec<u8>,
    pub title: String,
    /// "313" and the like, as a number.
    pub version: u16,
    pub opll_mode: u8,
    /// #machine_id.
    pub machine: u8,
    pub tempo: u16,
    tracks: [usize; 18],
}

impl Song {
    pub fn parse(d: &[u8]) -> Option<Song> {
        if !d.starts_with(b"MGS") || d.len() < 9 {
            return None;
        }
        let v = &d[3..6];
        let compressed = v[0] == b'A';
        let version = std::str::from_utf8(&v[1..3]).ok()?.parse::<u16>().ok()? + if compressed { 300 } else { u16::from(v[0] - b'0') * 100 };
        let eof = d.iter().position(|&b| b == 0x1a)?;
        let title: String = d[8..eof].iter().take_while(|&&b| b != 0x0d).map(|&b| if (0x20..0x7f).contains(&b) { b as char } else { ' ' }).collect();
        let bin = &d[eof + 1..];
        if bin.len() < 4 || bin[0] != 0 {
            return None;
        }
        if bin[1] & 0x80 != 0 {
            // Compressed (MGSARC): the song as it was.
            return Song::parse(&uncompress(d)?);
        }
        if bin.len() < 4 + 36 {
            return None;
        }
        let w = |i: usize| usize::from(bin[i]) | usize::from(bin[i + 1]) << 8;
        let mut tracks = [0usize; 18];
        for (t, x) in tracks.iter_mut().enumerate() {
            *x = w(4 + t * 2);
        }
        let mut song = Song {
            file: d.to_vec(),
            envs: vec![None; 32],
            fm_voices: [[0; 8]; 32],
            rom: std::array::from_fn(|i| if i < 15 { i as u8 + 1 } else { 0 }),
            waves: [[0; 32]; 32],
            psg_tune: PSG_TUNE,
            fm_tune: FM_TUNE,
            defined: Vec::new(),
            data: bin.to_vec(),
            title: title.trim().to_string(),
            version,
            opll_mode: bin[1] & 1,
            machine: (bin[1] >> 2) & 7,
            tempo: w(2) as u16,
            tracks,
        };
        song.definitions();
        Some(song)
    }

    // Track 0: the voices, envelopes, waves and tunings.
    fn definitions(&mut self) {
        let d = self.data.clone();
        let mut p = self.tracks[0];
        if p == 0 {
            return;
        }
        let get = |p: usize| d.get(p).copied();
        for _ in 0..4096 {
            let Some(c) = get(p) else { return };
            p += 1;
            match c {
                0x00 => {
                    let (Some(k), Some(v)) = (get(p), d.get(p + 1..p + 9)) else { return };
                    self.fm_voices[usize::from(k & 31)].copy_from_slice(v);
                    self.defined.push((0, k & 31));
                    p += 9;
                }
                0x01 => {
                    let (Some(n), Some(m)) = (get(p), get(p + 1)) else { return };
                    self.rom[usize::from(n & 31)] = m & 15;
                    p += 2;
                }
                0x02 => {
                    let (Some(k), Some(n)) = (get(p), get(p + 1)) else { return };
                    let (mode, noise) = ((n >> 5) & 3, n & 31);
                    if n & 0x80 != 0 {
                        let Some(r) = d.get(p + 2..p + 8) else { return };
                        self.defined.push((2, k & 31));
                        self.envs[usize::from(k & 31)] = Some(Env::R {
                            mode,
                            noise,
                            al: r[0],
                            ar: r[1],
                            dr: r[2],
                            sl: r[3],
                            sr: r[4],
                            rr: r[5],
                        });
                        p += 8;
                    } else {
                        let Some(l) = get(p + 2) else { return };
                        let Some(data) = d.get(p + 3..p + 3 + usize::from(l)) else { return };
                        self.defined.push((2, k & 31));
                        self.envs[usize::from(k & 31)] = Some(Env::E { mode, noise, data: data.to_vec() });
                        p += 3 + usize::from(l);
                    }
                }
                0x03 => {
                    let (Some(k), Some(w)) = (get(p), d.get(p + 1..p + 33)) else { return };
                    self.waves[usize::from(k & 31)].copy_from_slice(w);
                    self.defined.push((3, k & 31));
                    p += 33;
                }
                0x04 | 0x05 => {
                    let Some(t) = d.get(p..p + 24) else { return };
                    let tune: [u16; 12] = std::array::from_fn(|i| u16::from(t[i * 2]) | u16::from(t[i * 2 + 1]) << 8);
                    if c == 0x04 {
                        self.psg_tune = tune;
                    } else {
                        self.fm_tune = tune;
                    }
                    p += 24;
                }
                0x06 => {
                    p += 1;
                    while get(p).is_some_and(|b| b != 0) {
                        p += 1;
                    }
                    p += 1;
                }
                _ => return,
            }
        }
    }

    pub fn file(&self) -> &[u8] {
        &self.file
    }

    /// Its notes by sequencer step, followed without the chips, and how long
    /// it plays once through.
    pub fn timeline(&self) -> Timeline {
        let mut d = Driver::new(self);
        d.heard = Some(Vec::new());
        // Frames and steps at the first and second times round.
        let mut rounds: Vec<(u32, u64)> = Vec::new();
        let mut frames = 0u32;
        while frames < MAX_FRAMES && rounds.len() < 2 {
            d.frame();
            frames += 1;
            if d.take_round() {
                rounds.push((frames, d.steps));
                if !d.playing() {
                    break;
                }
            }
            if !d.playing() {
                break;
            }
        }
        let (once_frames, once_steps) = rounds.first().copied().unwrap_or((frames, d.steps));
        let restart = match rounds[..] {
            [(_, a), (_, b)] if b > a => Some(a.saturating_sub(b - a)),
            _ => None,
        };
        let mut events = d.heard.take().unwrap_or_default();
        events.retain(|e| e.0 <= once_steps);
        // The driver's start isn't an interrupt (the player's first tick is the second frame).
        Timeline {
            events,
            steps: once_steps,
            restart,
            seconds: f64::from(once_frames.saturating_sub(1)) * FRAME,
        }
    }

    fn legacy(&self) -> bool {
        self.version < 304
    }
}

// A song compressed by MGSARC ("MGSAxx"), as it was: an LZ coder much like
// LZE. Ported from mgsrc-js's uncompress.ts, ISC:
//
// Copyright (c) Mitsutaka Okazaki
//
// Permission to use, copy, modify, and/or distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
// ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
// ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
// OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
pub(crate) fn uncompress(inp: &[u8]) -> Option<Vec<u8>> {
    // The header up to the binary part's 0 marker and settings byte.
    let mut rp = inp.iter().position(|&b| b == 0)? + 1;
    let settings = *inp.get(rp)?;
    rp += 1;
    if settings & 0x80 == 0 {
        return None;
    }
    let mut out: Vec<u8> = inp[..rp].to_vec();
    out[3] = b'3';
    out[rp - 1] &= 0x7f;
    // (The data's size, which the decoder needn't.)
    rp += 2;
    let mut bits = 1u32;
    let mut cur = 0u8;
    let mut info = |rp: &mut usize| -> Option<u8> {
        let ret = cur >> 7;
        cur <<= 1;
        bits -= 1;
        if bits == 0 {
            bits = 8;
            cur = *inp.get(*rp)?;
            *rp += 1;
        }
        Some(ret)
    };
    info(&mut rp)?;
    loop {
        if out.len() > 0x10000 {
            return None;
        }
        if info(&mut rp)? == 1 {
            out.push(*inp.get(rp)?);
            rp += 1;
            continue;
        }
        let (size, offset) = if info(&mut rp)? == 1 {
            let (ll, hh) = (*inp.get(rp)?, *inp.get(rp + 1)?);
            rp += 2;
            let mut size = usize::from(hh >> 5);
            if size == 0 {
                size = usize::from(*inp.get(rp)?);
                rp += 1;
                if size == 0 {
                    break;
                }
            }
            (size, i32::from((u16::from(hh) << 8 | u16::from(ll) | 0xe000) as i16))
        } else {
            let size = usize::from(info(&mut rp)? << 1 | info(&mut rp)?);
            let ll = *inp.get(rp)?;
            rp += 1;
            (size, i32::from((0xff00 | u16::from(ll)) as i16))
        };
        for _ in 0..size + 2 {
            let from = usize::try_from(out.len() as i32 + offset).ok()?;
            out.push(*out.get(from)?);
        }
    }
    Some(out)
}

// The steps' frames for a tempo, as the compiler and driver make them: a
// whole note (192 steps) is 14400 / tempo frames (whole frames), each step's
// end rounded to the nearest frame.
fn tempo_table(tempo: u16) -> [u8; 192] {
    let mut t = [0u8; 192];
    let whole = 14400 / u32::from(tempo.max(1));
    let mut prev = 0;
    for (k, x) in t.iter_mut().enumerate() {
        let c = (2 * (k as u32 + 1) * whole + 192) / 384;
        *x = (c - prev).min(3) as u8;
        prev = c;
    }
    t
}

#[derive(Clone, Copy, PartialEq)]
enum Chip {
    Psg(u8),
    Scc(u8),
    Fm(u8),
    Rhythm,
}

// A running @e or @r envelope.
#[derive(Clone, Copy, Default)]
struct EnvRun {
    on: bool,
    adsr: bool,
    pos: usize,
    mark: usize,
    wait: u8,
    // @e: the volume, a ramp's start, target and frames.
    val: i32,
    ramp: bool,
    start: i32,
    target: i32,
    step: i32,
    k: i32,
    // @r: the level (0-255) and phase (0 attack, 1 decay, 2 sustain, 3 release).
    level: i32,
    phase: u8,
    // A key-off in the attack or decay, to release after them.
    off: bool,
}

#[derive(Clone, Copy, Default)]
struct Lfo {
    on: bool,
    delay: u8,
    depth: u8,
    speed: u8,
    coarse: i32,
    // Counting down: the delay, then each step's frames.
    dcnt: u8,
    scnt: u8,
    leg: u16,
    dir: i32,
    value: i32,
}

#[derive(Clone, Copy, Default)]
struct Track {
    ptr: usize,
    active: bool,
    len: u16,
    deflen: u16,
    octave: u8,
    vol: u8,
    q: u8,
    k: u8,
    keyed: bool,
    // Frames until the gate shuts (0: none).
    gatef: u16,
    // The release's countdown, and the volume now.
    rel: u8,
    outvol: u8,
    rest_off: bool,
    // An FM rest before any note: its key-off, written the frame after.
    fm_off: u8,
    note: u8,
    patch: u8,
    // The FM channel's ROM instrument, once a voice is chosen.
    fm_inst: Option<u8>,
    env: Option<usize>,
    run: EnvRun,
    detune: i32,
    fine: i32,
    // The detunes the note sounding took.
    n_det: i32,
    n_fine: i32,
    // Portamento: from, step (8.8) and frame.
    porta: bool,
    pfrom: i32,
    pto: i32,
    pstep: i32,
    pk: i32,
    porta_base: Option<u8>,
    porta_oct: u8,
    bend: u8,
    // The bend so far: new style in octave 1's periods, old style in periods.
    bnew: i32,
    bold: i32,
    // A new bend, in force from the next frame.
    bend_set: Option<u8>,
    bend_oct: u8,
    // The frame the track ended; it came round (an endless loop, or its end).
    ending: bool,
    round: bool,
    lfo_go: bool,
    lfo: Lfo,
    // The frame of the key-on (LFO, bend and portamento wait a frame).
    fresh: bool,
    // A rest since the last note; a release count to start again.
    rested: bool,
    rel_reset: bool,
    // The note's first frame (for the envelope), and a pitch to write next frame.
    fresh_env: bool,
    pitch_dirty: bool,
    // An envelope just chosen, to run from the next frame.
    env_wait: bool,
    played: bool,
    hw: bool,
    shape: u8,
    sus: bool,
    mode: u8,
    // Rhythm: each instrument's volume, cut on each hit (kf).
    rvol: [u8; 5],
    rcut: bool,
}

/// The chips' registers, as the driver leaves them each frame.
#[derive(Clone)]
pub struct Regs {
    pub psg: [u8; 16],
    pub scc_freq: [u8; 10],
    pub scc_vol: [u8; 5],
    pub scc_enable: u8,
    pub scc_wave: [[u8; 32]; 5],
    pub opll: [u8; 64],
    /// This frame: FM channels keyed on (anew: if on, they key off first), the
    /// drums struck, and whether the PSG's envelope shape was written (it
    /// restarts the envelope).
    pub rekey: u16,
    pub hits: u8,
    pub shape: bool,
}

impl Default for Regs {
    fn default() -> Regs {
        Regs {
            psg: [0; 16],
            scc_freq: [0; 10],
            scc_vol: [0; 5],
            scc_enable: 0,
            scc_wave: [[0; 32]; 5],
            opll: [0; 64],
            rekey: 0,
            hits: 0,
            shape: false,
        }
    }
}

#[derive(Clone)]
pub struct Driver {
    song: Song,
    // The song's binary, written to as it plays (its loops' counts).
    data: Vec<u8>,
    t: [Track; TRACKS],
    table: [u8; 192],
    pos: usize,
    wait: u8,
    pub regs: Regs,
    started: bool,
    span: i32,
    // r36's high nibble as the rhythm track's v leaves it.
    r36_hi: u8,
    // Register writes for the next frame: register, mask, value.
    pend: Vec<(usize, u8, u8)>,
    // Each patch's envelope (@e): SCC's, FM's.
    patch_env: [[Option<usize>; 32]; 2],
    frame_no: u32,
    /// Sequencer steps so far (192 a whole note).
    pub steps: u64,
    // Tracks that began with something to play.
    playing_at_start: [bool; TRACKS],
    scc_hi: [u8; 5],
    // What's played, for a timeline: step, channel (CHANNEL_NAMES), MIDI note (None a rest).
    heard: Option<Vec<(u64, usize, Option<u8>)>>,
    // Tracing commands (tests): track (0 all), first and last frame.
    #[cfg(test)]
    trace: Option<(usize, u32, u32)>,
}

impl Driver {
    pub fn new(song: &Song) -> Driver {
        let mut t = [Track::default(); TRACKS];
        for (i, tr) in t.iter_mut().enumerate() {
            let off = song.tracks[i + 1];
            tr.active = off != 0 && off < song.data.len();
            tr.ptr = off;
            tr.deflen = if song.legacy() { 24 } else { 48 };
            tr.octave = 4;
            tr.bend_oct = 4;
            tr.q = 8;
            tr.len = 1;
            tr.mode = 1;
            tr.rvol = [15; 5];
            // SCC and FM tracks start on patch 2 (it's what @e takes before any @).
            if i >= 3 {
                tr.patch = 2;
            }
        }
        let table = if song.legacy() { [1u8; 192] } else { tempo_table(if song.tempo == 0 { 120 } else { song.tempo }) };
        let mut regs = Regs::default();
        regs.psg[7] = 0xb8;
        regs.psg[11] = 0xff;
        regs.scc_enable = 0x1f;
        for r in 0x30..0x39 {
            regs.opll[r] = 0xff;
        }
        // A song for the OPLL's drums starts it in rhythm mode, the drums'
        // pitches (channels 7 to 9) as MSX-MUSIC has them; an FM song leaves
        // it in melody mode.
        if song.opll_mode == 1 {
            regs.opll[0x0e] = 0x20;
            regs.opll[0x16..0x19].copy_from_slice(&[0x20, 0x50, 0xc0]);
            regs.opll[0x26..0x29].copy_from_slice(&[0x05, 0x05, 0x01]);
        }
        let span = i32::from(song.fm_tune[0]) + 1;
        let starts: [bool; TRACKS] = std::array::from_fn(|i| t[i].active);
        Driver {
            song: song.clone(),
            data: song.data.clone(),
            t,
            table,
            pos: 0,
            wait: 0,
            regs,
            started: false,
            span,
            r36_hi: 0,
            pend: Vec::new(),
            patch_env: [[None; 32]; 2],
            frame_no: 0,
            steps: 0,
            playing_at_start: starts,
            scc_hi: [8; 5],
            heard: None,
            #[cfg(test)]
            trace: std::env::var("MGS_TRACE").ok().and_then(|v| {
                let n: Vec<u32> = v.split(',').filter_map(|x| x.parse().ok()).collect();
                (n.len() == 3).then(|| (n[0] as usize, n[1], n[2]))
            }),
        }
    }

    fn chip(&self, i: usize) -> Chip {
        match i {
            0..=2 => Chip::Psg(i as u8),
            3..=7 => Chip::Scc(i as u8 - 3),
            14 if self.song.opll_mode == 1 => Chip::Rhythm,
            _ => Chip::Fm(i as u8 - 8),
        }
    }

    // A track's channel in CHANNEL_NAMES.
    fn channel(&self, i: usize) -> usize {
        match self.chip(i) {
            Chip::Psg(c) => usize::from(c),
            Chip::Scc(c) => 3 + usize::from(c),
            Chip::Fm(c) => 8 + usize::from(c),
            Chip::Rhythm => 17,
        }
    }

    fn byte(&mut self, i: usize) -> u8 {
        let p = self.t[i].ptr;
        self.t[i].ptr += 1;
        self.data.get(p).copied().unwrap_or(0xff)
    }

    fn word(&mut self, i: usize) -> i16 {
        let lo = u16::from(self.byte(i));
        (lo | u16::from(self.byte(i)) << 8) as i16
    }

    /// Whether the song came round (every track through its endless loop,
    /// or ended) since this was last asked.
    pub fn take_round(&mut self) -> bool {
        let tracks: Vec<usize> = (0..TRACKS).filter(|&i| self.playing_at_start[i]).collect();
        if tracks.is_empty() || !tracks.iter().all(|&i| self.t[i].round) {
            return false;
        }
        for i in tracks {
            self.t[i].round = !self.t[i].active && self.t[i].round;
        }
        true
    }

    /// Whether any track still plays.
    pub fn playing(&self) -> bool {
        self.t.iter().any(|t| t.active)
    }

    /// One frame (1/60 s).
    pub fn frame(&mut self) {
        self.frame_no += 1;
        if !self.started {
            // The first call: the driver's start, before any interrupt.
            self.started = true;
            return;
        }
        if self.frame_no == 2 {
            // The first interrupt: FM songs leave rhythm mode, rhythm songs tune its channels.
            if self.song.opll_mode == 0 {
                self.regs.opll[0x0e] = 0;
            } else {
                self.regs.opll[0x16..0x19].copy_from_slice(&[0x20, 0x50, 0xc0]);
                self.regs.opll[0x26..0x29].copy_from_slice(&[0x05, 0x05, 0x01]);
            }
        }
        self.regs.rekey = 0;
        self.regs.hits = 0;
        self.regs.shape = false;
        for (r, m, v) in std::mem::take(&mut self.pend) {
            self.regs.opll[r] = (self.regs.opll[r] & !m) | v;
        }

        for i in 0..TRACKS {
            let tr = &mut self.t[i];
            if tr.gatef > 0 {
                tr.gatef -= 1;
                if tr.gatef == 0 && tr.keyed {
                    self.keyoff(i);
                }
            }
        }
        let mut guard = 0;
        while self.wait == 0 && guard < 192 {
            guard += 1;
            self.steps += 1;
            for i in 0..TRACKS {
                self.step(i);
            }
            self.wait = self.table[self.pos];
            self.pos = (self.pos + 1) % 192;
        }
        self.wait = self.wait.saturating_sub(1);
        for i in 0..TRACKS {
            self.per_frame(i);
        }
    }

    fn step(&mut self, i: usize) {
        if !self.t[i].active {
            return;
        }
        let tr = &mut self.t[i];

        tr.len = tr.len.saturating_sub(1);
        if tr.len > 0 {
            return;
        }
        self.read(i);
    }

    // A legacy length: ticks, 0xff adding the next byte.
    fn ticks(&mut self, i: usize) -> u16 {
        let mut n = 0u16;
        for _ in 0..256 {
            let b = self.byte(i);
            n = n.saturating_add(u16::from(b));
            if b != 0xff {
                break;
            }
        }
        n.max(1)
    }

    fn read(&mut self, i: usize) {
        let mut slur = false;
        let rhythm = self.chip(i) == Chip::Rhythm;
        let legacy = self.song.legacy();
        for _ in 0..4096 {
            let c = self.byte(i);
            #[cfg(test)]
            if self.trace.is_some_and(|(t, a, b)| (t == 0 || t == i + 1) && (a..=b).contains(&self.frame_no)) {
                let at = self.t[i].ptr - 1;
                eprintln!("    f{} t{} @{at}: {:02x?}", self.frame_no, i + 1, &self.data[at..(at + 6).min(self.data.len())]);
            }
            if rhythm {
                match c {
                    0x00..=0x1f if legacy => {
                        let len = self.ticks(i);
                        self.hit(i, c & 0x1f, len);
                        return;
                    }
                    0x20..=0x3f => {
                        let len = self.len_byte(i);
                        self.hit(i, c & 0x1f, len);
                        return;
                    }
                    0xa0..=0xbf => {
                        let len = self.t[i].deflen;
                        self.hit(i, c & 0x1f, len);
                        return;
                    }
                    0x45 => {
                        let n = self.byte(i);
                        self.t[i].rvol[usize::from(n >> 5).min(4)] = n & 15;
                        self.rhythm_vols(i);
                        continue;
                    }
                    0x47 => {
                        let n = self.byte(i);
                        let r = usize::from(n >> 5).min(4);
                        let d = i32::from(((n & 31) << 3) as i8 >> 3);
                        self.t[i].rvol[r] = (i32::from(self.t[i].rvol[r]) + d).clamp(0, 15) as u8;
                        self.rhythm_vols(i);
                        continue;
                    }
                    0x46 => {
                        let d = i32::from(self.byte(i) as i8);
                        for v in self.t[i].rvol.iter_mut() {
                            *v = (i32::from(*v) + d).clamp(0, 15) as u8;
                        }
                        self.t[i].vol = (i32::from(self.t[i].vol) + d).clamp(0, 15) as u8;
                        self.r36_hi = (15 - self.t[i].vol) << 4;
                        self.rhythm_vols(i);
                        continue;
                    }
                    0xc0..=0xcf => {
                        self.t[i].rvol = [c & 15; 5];
                        self.t[i].vol = c & 15;
                        self.r36_hi = (15 - (c & 15)) << 4;
                        self.rhythm_vols(i);
                        continue;
                    }
                    0x5a => {
                        let n = self.byte(i);
                        self.t[i].rcut = n == 1;
                        continue;
                    }
                    _ => {}
                }
            }
            match c {
                0x00..=0x0b if legacy => {
                    let len = self.ticks(i);
                    self.note(i, c & 0x0f, len, slur);
                    return;
                }
                0x10..=0x1b if legacy => {
                    let len = self.t[i].deflen;
                    self.note(i, c & 0x0f, len, slur);
                    return;
                }
                0x20..=0x2b | 0x30..=0x3b => {
                    let len = if c < 0x30 { self.len_byte(i) } else { self.t[i].deflen };
                    self.note(i, c & 0x0f, len, slur);
                    return;
                }
                0x0c if legacy => {
                    let len = self.ticks(i);
                    self.rest(i, len);
                    return;
                }
                0x1c if legacy => {
                    let len = self.t[i].deflen;
                    self.rest(i, len);
                    return;
                }
                0x2c | 0x3c => {
                    let len = if c == 0x2c { self.len_byte(i) } else { self.t[i].deflen };
                    self.rest(i, len);
                    return;
                }
                0x40 => slur = true,
                0x41 => {
                    self.byte(i);
                    self.byte(i);
                    let mut t = [0u8; 192];
                    for k in 0..48 {
                        let b = self.byte(i);
                        for j in 0..4 {
                            t[k * 4 + j] = (b >> (6 - 2 * j)) & 3;
                        }
                    }
                    self.table = t;
                }
                0x42 => self.t[i].deflen = self.len_byte(i),
                0x43 => self.t[i].deflen = (self.word(i) as u16).max(1),
                0x44 => self.t[i].q = self.byte(i),
                0x46 => {
                    let d = i32::from(self.byte(i) as i8);
                    self.t[i].vol = (i32::from(self.t[i].vol) + d).clamp(0, 15) as u8;
                }
                0x48 => self.t[i].k = self.byte(i),
                0x49 => {
                    let n = usize::from(self.byte(i) & 31);
                    let env = self.song.envs[n].as_ref().map(|_| n);
                    match self.chip(i) {
                        Chip::Psg(_) => {
                            self.t[i].env = env;
                            self.t[i].patch = n as u8;
                            self.env_mode(i);
                        }
                        ch => {
                            // Given to the patch in use, and every track that picks it.
                            let p = usize::from(self.t[i].patch & 31);
                            self.patch_env[usize::from(matches!(ch, Chip::Fm(_)))][p] = env;
                            self.start_env(i, env);
                            // The driver writes the instrument with a volume of 0
                            // for patches 0-6 and 15 up, 15 for patches 7-14.
                            if let Chip::Scc(c) = ch {
                                // The patch's wave, written again.
                                self.regs.scc_wave[usize::from(c.min(3))] = self.song.waves[p];
                            }
                            if let Chip::Fm(c) = ch {
                                let rom = self.fm_rom(i);
                                self.t[i].fm_inst = Some(rom);
                                let low = if (7..15).contains(&p) { 0x0f } else { 0 };
                                self.regs.opll[0x30 + usize::from(c)] = rom << 4 | low;
                            }
                        }
                    }
                }
                0x4a => {
                    if self.t[i].keyed {
                        self.keyoff(i);
                    }
                }
                0x4b => {
                    let n = self.byte(i);
                    self.t[i].hw = true;
                    self.t[i].shape = n & 15;
                }
                0x4c => {
                    let w = self.word(i) as u16;
                    self.regs.psg[11] = w as u8;
                    self.regs.psg[12] = (w >> 8) as u8;
                }
                0x4d => self.regs.psg[6] = self.byte(i) & 31,
                0x4e => self.t[i].octave = (self.t[i].octave + 1).min(8),
                0x4f => self.t[i].octave = self.t[i].octave.saturating_sub(1).max(1),
                0x50 => self.t[i].detune = i32::from(self.byte(i) as i8),
                0x51 => self.t[i].fine = i32::from(self.word(i)),
                0x52 => self.t[i].bend_set = Some(self.byte(i)),
                0x53 => {
                    self.t[i].porta_base = Some(self.byte(i) & 15);
                    self.t[i].porta_oct = self.t[i].octave - 1;
                }
                0x54 => {
                    let (n1, n2, n3, n4) = (self.byte(i), self.byte(i), self.byte(i), self.byte(i));
                    let l = &mut self.t[i].lfo;
                    l.on = true;
                    l.delay = n1;
                    l.depth = n2;
                    l.speed = n3;
                    l.coarse = i32::from(n4 as i8);
                    // Restarted, from its delay, counted from the next frame.
                    self.t[i].lfo_go = false;
                    {
                        let l = &mut self.t[i].lfo;
                        l.dcnt = l.delay;
                        l.scnt = 0;
                        l.leg = u16::from(l.depth).div_ceil(2);
                        l.dir = 1;
                        l.value = 0;
                    }
                }
                0x55 => self.t[i].lfo.coarse = i32::from(self.word(i)),
                0x57 => {
                    let n = self.byte(i);
                    let at = self.t[i].ptr + 1;
                    let off = self.word(i);
                    // The loop's count, kept in its end's spare byte.
                    let end = at.wrapping_add(off as usize);
                    if let Some(x) = self.data.get_mut(end + 1) {
                        *x = n;
                    }
                }
                0x58 => {
                    let at = self.t[i].ptr + 1;
                    let off = self.word(i);
                    let end = at.wrapping_add(off as usize);
                    if self.data.get(end + 1).copied() == Some(1) {
                        self.t[i].ptr = end + 4;
                    }
                }
                0x59 => {
                    let cnt_at = self.t[i].ptr;
                    self.byte(i);
                    let at = self.t[i].ptr + 1;
                    let off = self.word(i);
                    let c = self.data.get(cnt_at).copied().unwrap_or(0);
                    if c == 0 {
                        // An endless loop: the track comes round.
                        self.t[i].ptr = at.wrapping_add(off as usize);
                        self.t[i].round = true;
                    } else if c > 1 {
                        self.data[cnt_at] = c - 1;
                        self.t[i].ptr = at.wrapping_add(off as usize);
                    } else {
                        self.data[cnt_at] = 0;
                    }
                }
                0x5a => {
                    let n = self.byte(i);
                    self.t[i].lfo.on = n == 1;
                }
                0x5b => {
                    let n = self.byte(i);
                    self.t[i].sus = n == 1;
                    if let Chip::Fm(ch) = self.chip(i) {
                        // Written the frame after.
                        let ch = usize::from(ch);
                        let tr = self.t[i];
                        let _ = tr;
                        self.pend.push((0x20 + ch, 0x20, if n == 1 { 0x20 } else { 0 }));
                    }
                }
                0x5c => {
                    let (r, d) = (usize::from(self.byte(i)), self.byte(i));
                    self.write_reg(i, r, d);
                }
                0x5f => self.t[i].fine = i32::from(self.byte(i)),
                0x60 | 0x61 => {
                    self.byte(i);
                }
                0x80..=0x9f => {
                    let n = c - 0x80;
                    self.t[i].patch = n;
                    match self.chip(i) {
                        Chip::Psg(_) => {
                            self.t[i].env = self.song.envs[usize::from(n)].as_ref().map(|_| usize::from(n));
                            self.env_mode(i);
                        }
                        Chip::Scc(ch) => {
                            self.regs.scc_wave[usize::from(ch.min(3))] = self.song.waves[usize::from(n)];
                            self.start_env(i, self.patch_env[0][usize::from(n)]);
                        }
                        Chip::Fm(ch) => {
                            self.start_env(i, self.patch_env[1][usize::from(n)]);
                            let rom = self.song.rom[usize::from(n)];
                            self.t[i].fm_inst = Some(rom);
                            let r = &mut self.regs.opll[0x30 + usize::from(ch)];
                            *r = (rom << 4) | (*r & 0x0f);
                            if rom == 0 {
                                self.regs.opll[..8].copy_from_slice(&self.song.fm_voices[usize::from(n)]);
                            }
                        }
                        _ => {}
                    }
                }
                0xc0..=0xcf => {
                    self.t[i].vol = c & 0x0f;
                    self.t[i].hw = false;
                }
                0xd0..=0xdf => self.t[i].octave = (c & 0x0f) + 1,
                0xe0..=0xef => {
                    if let Chip::Psg(ch) = self.chip(i) {
                        let m = c & 3;
                        self.t[i].mode = m;
                        self.mixer(ch, m, true);
                    }
                }
                0xff => {
                    self.t[i].active = false;
                    if self.t[i].keyed {
                        self.keyoff(i);
                    }
                    // PSG and SCC fall silent at once; the drums are keyed off.
                    if rhythm {
                        self.regs.opll[0x0e] = 0x20;
                    }
                    self.t[i].outvol = 0;
                    self.t[i].run.on = false;
                    self.t[i].ending = true;
                    self.t[i].round = true;
                    return;
                }
                _ => {}
            }
        }
        self.t[i].active = false;
    }

    // An envelope chosen by @ or @e (SCC, FM).
    fn start_env(&mut self, i: usize, env: Option<usize>) {
        if let Chip::Scc(_) = self.chip(i) {
            // SCC's starts with the next note; FM's runs at once, rests or not.
            self.t[i].env = env;
            return;
        }
        let tr = &mut self.t[i];
        tr.env = env;
        tr.run = EnvRun {
            on: env.is_some(),
            ..EnvRun::default()
        };
        if let Some(Env::R { al, .. }) = env.and_then(|e| self.song.envs[e].as_ref()) {
            tr.run.adsr = true;
            tr.run.level = i32::from(*al);
        }
        tr.env_wait = true;
    }

    // The ROM instrument of the FM track's patch (0: its own voice).
    fn fm_rom(&self, i: usize) -> u8 {
        self.song.rom[usize::from(self.t[i].patch & 31)]
    }

    fn mixer(&mut self, ch: u8, m: u8, zero_silences: bool) {
        if m == 0 && !zero_silences {
            return;
        }
        let r = &mut self.regs.psg[7];
        *r |= 9 << ch;
        if m & 1 != 0 {
            *r &= !(1 << ch);
        }
        if m & 2 != 0 {
            *r &= !(8 << ch);
        }
    }

    // A PSG envelope's mode and noise, set when it's chosen.
    fn env_mode(&mut self, i: usize) {
        let (Chip::Psg(ch), Some(e)) = (self.chip(i), self.t[i].env) else { return };
        let Some(env) = self.song.envs[e].as_ref() else { return };
        let (m, n) = env.mode_noise();
        self.mixer(ch, m, false);
        if m >= 2 {
            self.regs.psg[6] = n;
        }
    }

    fn write_reg(&mut self, i: usize, r: usize, d: u8) {
        match self.chip(i) {
            Chip::Psg(_) => {
                if r < 16 {
                    self.regs.psg[r] = d;
                }
            }
            Chip::Fm(_) | Chip::Rhythm => {
                if r < 0x39 {
                    self.regs.opll[r] = d;
                }
            }
            Chip::Scc(_) => {}
        }
    }

    fn len_byte(&mut self, i: usize) -> u16 {
        let b = self.byte(i);
        if b == 0 {
            256
        } else {
            u16::from(b)
        }
    }

    fn frames(&self, len: u16) -> u16 {
        if self.song.legacy() {
            return len;
        }
        (0..usize::from(len)).map(|k| u32::from(self.table[(self.pos + k) % 192])).sum::<u32>().min(0xffff) as u16
    }

    fn hit(&mut self, i: usize, bits: u8, len: u16) {
        self.t[i].len = len;
        if bits == 0 {
            return;
        }
        if let Some(h) = &mut self.heard {
            // BD, HH, SD, TOM, CYM: bits 4, 0, 3, 2, 1.
            for (k, b) in [4, 0, 3, 2, 1].into_iter().enumerate() {
                if bits & 1 << b != 0 {
                    h.push((self.steps, 17 + k, Some(60)));
                }
            }
        }
        self.regs.hits |= bits;
        let o = &mut self.regs.opll;
        o[0x0e] = if self.t[i].rcut { 0x20 | bits } else { o[0x0e] | 0x20 | bits };
    }

    fn rhythm_vols(&mut self, i: usize) {
        let v = self.t[i].rvol;
        let o = &mut self.regs.opll;
        o[0x36] = self.r36_hi | (15 - v[0]);
        o[0x37] = (15 - v[4]) << 4 | (15 - v[1]);
        o[0x38] = (15 - v[2]) << 4 | (15 - v[3]);
    }

    fn note(&mut self, i: usize, n: u8, len: u16, slur: bool) {
        let frames = self.frames(len);
        let tr = &mut self.t[i];
        let tie = slur && tr.keyed;
        tr.len = len;
        let target = (tr.octave - 1) * 12 + n;
        if !tie {
            // Octave 1's C is MIDI's 24.
            let (step, ch) = (self.steps, self.channel(i));
            if let Some(h) = &mut self.heard {
                h.push((step, ch, Some(24 + target)));
            }
        }
        let tr = &mut self.t[i];
        // A note followed by & (slur or tie) isn't keyed off at its gate.
        let slurred = self.data.get(tr.ptr).copied() == Some(0x40);
        // The gate: q eighths of the note's frames, at least one.
        tr.gatef = if tr.q >= 8 || tr.q == 0 || slurred { 0 } else { (u32::from(frames) * u32::from(tr.q) / 8).clamp(1, 0xffff) as u16 };
        tr.rest_off = false;
        tr.porta = false;
        let base = tr.porta_base.take();
        tr.note = target;
        // The detunes as they are at the note: later \ and @\ wait for the next.
        tr.n_fine = tr.fine;
        tr.n_det = tr.detune;
        if let Some(b) = base {
            // From the base note (in the octave the slide began in) to this one,
            // over the note's frames.
            let from = self.t[i].porta_oct * 12 + b;
            let (a, z) = match self.chip(i) {
                Chip::Fm(_) => (self.fm_note(i, from), self.fm_note(i, target)),
                _ => (self.period(i, from), self.period(i, target)),
            };
            let tr = &mut self.t[i];
            tr.porta = true;
            tr.fresh = true;
            tr.pfrom = a;
            tr.pto = z;
            tr.pstep = i32::from(frames.max(1));
            tr.pk = 0;
        }
        let tr = &mut self.t[i];
        tr.played = true;
        tr.rested = false;
        if !tie {
            tr.keyed = true;
            tr.outvol = tr.vol;
            tr.rel = 0;
            tr.fresh = true;
            tr.bnew = 0;
            tr.bold = 0;
            let l = &mut tr.lfo;
            l.dcnt = l.delay;
            l.scnt = 0;
            l.leg = u16::from(l.depth).div_ceil(2);
            l.dir = 1;
            l.value = 0;
            tr.env_wait = false;
            if let Some(e) = tr.env {
                tr.run = EnvRun { on: true, ..EnvRun::default() };
                if let Some(Env::R { al, .. }) = self.song.envs[e] {
                    let r = &mut self.t[i].run;
                    r.adsr = true;
                    r.level = i32::from(al);
                }
            } else {
                tr.run.on = false;
            }
        }
        if tie && self.t[i].run.on {
            // A tie doesn't restart the envelope, but the note still steps it.
            if self.t[i].run.adsr {
                self.run_r(i);
            } else {
                self.run_e(i);
            }
        }
        self.keyon(i, tie);
    }

    fn rest(&mut self, i: usize, len: u16) {
        let (step, ch, keyed) = (self.steps, self.channel(i), self.t[i].keyed);
        if let Some(h) = &mut self.heard {
            if keyed {
                h.push((step, ch, None));
            }
        }
        let tr = &mut self.t[i];
        tr.len = len;
        tr.porta_base = None;
        if let Chip::Fm(c) = self.chip(i) {
            if !self.t[i].keyed && self.regs.opll[0x20 + usize::from(c)] & 0x0f == 0 {
                self.t[i].fm_off = 2;
            }
        }
        let fm = matches!(self.chip(i), Chip::Fm(_));
        let tr = &mut self.t[i];
        if !tr.keyed && tr.outvol > 0 && !fm && !tr.rested {
            // The first rest after a note keyed off by its gate starts the
            // release count again (after this frame's step).
            tr.rel_reset = true;
        }
        tr.rested = true;
        if tr.keyed {
            if matches!(self.chip(i), Chip::Fm(_)) {
                self.keyoff(i);
            } else {
                // PSG and SCC fall silent the frame after.
                self.t[i].rest_off = true;
            }
        }
    }

    // The PSG/SCC period of a note.
    fn period(&self, i: usize, note: u8) -> i32 {
        let tr = &self.t[i];
        let (oct, n) = (note / 12, usize::from(note % 12));
        ((i32::from(self.song.psg_tune[n]) + tr.n_fine + tr.bnew) >> oct) + tr.n_det + tr.bold
    }

    // The FM's linear pitch: block × span + the F-number above C's.
    fn fm_pitch(&self, note: u8) -> i32 {
        let (oct, n) = (i32::from(note / 12), usize::from(note % 12));
        oct * self.span + i32::from(self.song.fm_tune[n]) - i32::from(self.song.fm_tune[0])
    }

    fn fm_note(&self, i: usize, note: u8) -> i32 {
        let tr = &self.t[i];
        let p = self.fm_pitch(note);
        let next = self.fm_pitch(note.saturating_add(1));
        p + (((next - p) * tr.n_fine) >> 8) + tr.n_det
    }

    // One frame of the bend: new style (p < 128) in octave 1's periods, old
    // style ((p - 128) × (9 - octave) a frame) in the octave's.
    fn bend_step(&mut self, i: usize) {
        let tr = &mut self.t[i];
        // (Old style: by the octave the track was in last frame, not the note's.)
        let oct = i32::from(tr.bend_oct) - 1;
        if tr.bend < 128 {
            tr.bnew += 2 * i32::from(tr.bend);
        } else {
            tr.bold += i32::from(tr.bend - 128) * (8 - oct);
        }
    }

    // The note's pitch (period, or FM's linear pitch), sliding if it's a portamento.
    fn base_pitch(&self, i: usize) -> i32 {
        let tr = &self.t[i];
        if !tr.played {
            // Before any note, the pitch is 0x800: a period, or FM's block 4, F-number 0.
            if let Chip::Fm(_) = self.chip(i) {
                return 4 * self.span - i32::from(self.song.fm_tune[0]);
            }
            let oct = i32::from(tr.octave) - 1;
            return 0x800 + (tr.bnew >> oct) + tr.bold;
        }
        if tr.porta {
            let d = tr.pk * (tr.pto - tr.pfrom).abs() / tr.pstep;
            return if tr.pto >= tr.pfrom { tr.pfrom + d } else { tr.pfrom - d };
        }
        match self.chip(i) {
            Chip::Fm(_) => self.fm_note(i, tr.note),
            _ => self.period(i, tr.note),
        }
    }

    fn set_pitch(&mut self, i: usize) {
        let tr = self.t[i];
        match self.chip(i) {
            Chip::Psg(c) | Chip::Scc(c) => {
                let mut p = self.base_pitch(i);
                p += tr.lfo.value;
                let p = p as u16;
                if let Chip::Psg(_) = self.chip(i) {
                    self.regs.psg[usize::from(c) * 2] = p as u8;
                    self.regs.psg[usize::from(c) * 2 + 1] = (p >> 8) as u8;
                } else {
                    // The high byte is written when it changes (from 8, at the start).
                    let c = usize::from(c);
                    self.regs.scc_freq[c * 2] = p as u8;
                    if self.scc_hi[c] != (p >> 8) as u8 {
                        self.scc_hi[c] = (p >> 8) as u8;
                        self.regs.scc_freq[c * 2 + 1] = (p >> 8) as u8;
                    }
                }
            }
            Chip::Fm(c) => {
                let p = (self.base_pitch(i) + tr.lfo.value).max(0);
                let block = (p / self.span).min(7);
                let f = (p - block * self.span + i32::from(self.song.fm_tune[0])) as u16;
                let c = usize::from(c);
                self.regs.opll[0x10 + c] = f as u8;
                self.regs.opll[0x20 + c] = (self.regs.opll[0x20 + c] & 0x30) | (block as u8) << 1 | ((f >> 8) as u8 & 1);
            }
            Chip::Rhythm => {}
        }
    }

    fn keyon(&mut self, i: usize, tie: bool) {
        self.set_pitch(i);
        let tr = self.t[i];
        match self.chip(i) {
            Chip::Psg(ch) => {
                if tr.hw {
                    self.regs.psg[8 + usize::from(ch)] = 0x10;
                    self.regs.psg[13] = tr.shape;
                    self.regs.shape = true;
                }
            }
            Chip::Scc(_) => {}
            Chip::Fm(c) => {
                let c = usize::from(c);
                // (A track that chose no voice keys instrument 0.)
                let rom = tr.fm_inst.unwrap_or(0);
                if rom == 0 && !tie && tr.fm_inst.is_some() {
                    self.regs.opll[..8].copy_from_slice(&self.song.fm_voices[usize::from(tr.patch & 31)]);
                }
                self.regs.opll[0x30 + c] = (rom << 4) | (15 - tr.vol);
                let r = &mut self.regs.opll[0x20 + c];
                if !tie {
                    self.regs.rekey |= 1 << c;
                }
                *r = (*r & 0x0f) | 0x10 | if tr.sus { 0x20 } else { 0 };
            }
            Chip::Rhythm => {}
        }
    }

    fn keyoff(&mut self, i: usize) {
        self.t[i].keyed = false;
        self.t[i].rel = self.t[i].k;
        // An @r envelope releases once its attack and decay are done.
        if self.t[i].run.phase >= 2 {
            self.t[i].run.phase = 3;
        } else {
            self.t[i].run.off = true;
        }
        if let Chip::Fm(c) = self.chip(i) {
            self.regs.opll[0x20 + usize::from(c)] &= !0x10;
        }
    }

    // The @e envelope: one frame of it, its volume (0-15) after.
    fn run_e(&mut self, i: usize) -> i32 {
        let e = self.t[i].env.unwrap_or(0);
        let Some(Env::E { data, .. }) = self.song.envs[e].clone() else { return 15 };
        let mut r = self.t[i].run;
        if r.wait > 0 {
            r.wait -= 1;
            if r.ramp {
                r.k += 1;
                let d = r.k * (r.target - r.start).abs() / r.step;
                r.val = if r.target >= r.start { r.start + d } else { r.start - d };
            }
            if r.wait > 0 {
                self.t[i].run = r;
                return r.val;
            }
            if r.ramp {
                r.val = r.target;
                r.ramp = false;
            }
        }
        let mut guard = 0;
        while r.pos < data.len() && guard < 512 {
            guard += 1;
            let c = data[r.pos];
            r.pos += 1;
            let arg = |p: usize| data.get(p).copied().unwrap_or(0);
            match c {
                0x00..=0x0f => {
                    r.val = i32::from(c);
                    r.wait = 1;
                    break;
                }
                0xe0..=0xef => {
                    r.val = i32::from(c & 15);
                    r.wait = arg(r.pos).max(1);
                    r.pos += 1;
                    break;
                }
                0x20..=0x2f => {
                    let cc = arg(r.pos).max(1);
                    r.pos += 1;
                    r.start = r.val;
                    r.target = i32::from(c & 15);
                    r.step = i32::from(cc);
                    r.k = 0;
                    r.ramp = true;
                    r.wait = cc;
                    break;
                }
                0x40 => r.mark = r.pos,
                0x60 => r.pos = r.mark,
                0x10 => {
                    let n = arg(r.pos);
                    r.pos += 1;
                    match self.chip(i) {
                        Chip::Scc(ch) => self.regs.scc_wave[usize::from(ch.min(3))] = self.song.waves[usize::from(n & 31)],
                        Chip::Fm(ch) => {
                            // The voice only: the envelope stays.
                            let n = usize::from(n & 31);
                            let rom = self.song.rom[n];
                            self.t[i].patch = n as u8;
                            self.t[i].fm_inst = Some(rom);
                            let r = &mut self.regs.opll[0x30 + usize::from(ch)];
                            *r = (rom << 4) | (*r & 0x0f);
                            if rom == 0 {
                                self.regs.opll[..8].copy_from_slice(&self.song.fm_voices[n]);
                            }
                        }
                        _ => {}
                    }
                }
                0x11 => {
                    let (reg, d) = (usize::from(arg(r.pos)), arg(r.pos + 1));
                    r.pos += 2;
                    self.write_reg(i, reg, d);
                }
                0x12 => {
                    // Raises the pitch (a PSG/SCC period goes down). Heard at
                    // once, unless the LFO is on: it writes the pitch before the
                    // envelope runs, so then from the next frame (but a note's
                    // first frame).
                    let d = i32::from(arg(r.pos) as i8);
                    self.t[i].n_det += if matches!(self.chip(i), Chip::Fm(_)) { d } else { -d };
                    r.pos += 1;
                    if self.t[i].fresh_env || !self.t[i].lfo.on {
                        self.set_pitch(i);
                    } else {
                        self.t[i].pitch_dirty = true;
                    }
                }
                0x80..=0x9f => {
                    if let Chip::Psg(_) = self.chip(i) {
                        self.regs.psg[6] = c & 31;
                    }
                }
                0xa0..=0xaf => {
                    if let Chip::Psg(ch) = self.chip(i) {
                        self.mixer(ch, c & 3, false);
                    }
                }
                _ => {}
            }
        }
        self.t[i].run = r;
        r.val
    }

    // The @r envelope: one frame, its level (0-255) after.
    fn run_r(&mut self, i: usize) -> i32 {
        let e = self.t[i].env.unwrap_or(0);
        let Some(Env::R { ar, dr, sl, sr, rr, .. }) = self.song.envs[e].clone() else { return 255 };
        let r = &mut self.t[i].run;
        match r.phase {
            0 => {
                r.level += i32::from(ar);
                if r.level >= 255 {
                    r.level = 255;
                    r.phase = 1;
                }
            }
            1 => {
                r.level -= i32::from(dr);
                if r.level <= i32::from(sl) {
                    r.level = i32::from(sl);
                    // A key-off in the attack or decay releases from here.
                    r.phase = if r.off { 3 } else { 2 };
                }
            }
            2 => r.level = (r.level - i32::from(sr)).max(0),
            _ => r.level = (r.level - i32::from(rr)).max(0),
        }
        r.level
    }

    fn per_frame(&mut self, i: usize) {
        let ch = self.chip(i);
        if ch == Chip::Rhythm {
            return;
        }
        if std::mem::take(&mut self.t[i].pitch_dirty) {
            self.set_pitch(i);
        }
        let fresh_env = self.t[i].fresh;
        self.t[i].fresh_env = fresh_env;
        // Pitch: the LFO, bend and portamento, from the frame after the key-on.
        // Before any note, a bend still runs, from a period of 0.
        if !self.t[i].played && self.t[i].active && self.t[i].bend != 0 && matches!(ch, Chip::Psg(_) | Chip::Scc(_)) {
            self.bend_step(i);
            self.set_pitch(i);
        }
        let pre_lfo = !self.t[i].played && self.t[i].active && self.t[i].lfo.on && std::mem::replace(&mut self.t[i].lfo_go, true);
        // (On the track's last frame, the bend still runs; the LFO stops.)
        let live = self.t[i].active || std::mem::take(&mut self.t[i].ending);
        if pre_lfo || live && !self.t[i].fresh && self.t[i].played {
            let mut moved = false;
            let tr = &mut self.t[i];
            if tr.lfo.on && tr.active && std::mem::replace(&mut tr.lfo_go, true) {
                let l = &mut tr.lfo;
                let mut go = false;
                if l.dcnt > 0 {
                    l.dcnt -= 1;
                    if l.dcnt == 0 {
                        l.scnt = l.speed;
                    }
                } else {
                    l.scnt = l.scnt.saturating_sub(1);
                    go = l.scnt == 0;
                }
                if go {
                    // A leg's steps done: turn (first leg depth/2 steps, then depth+1).
                    l.scnt = l.speed;
                    if l.leg == 0 {
                        l.dir = -l.dir;
                        l.leg = u16::from(l.depth) + 1;
                    }
                    l.value += l.dir * l.coarse;
                    l.leg -= 1;
                    moved = true;
                }
            }
            if tr.bend != 0 {
                self.bend_step(i);
                moved = true;
            }
            let tr = &mut self.t[i];
            if tr.porta {
                tr.pk += 1;
                moved = true;
            }
            if moved {
                self.set_pitch(i);
            }
        }
        self.t[i].fresh = false;
        self.t[i].lfo_go = true;
        if let Some(b) = self.t[i].bend_set.take() {
            self.t[i].bend = b;
        }
        self.t[i].bend_oct = self.t[i].octave;
        let tr = &mut self.t[i];
        if let Chip::Fm(c) = ch {
            if tr.fm_off > 0 {
                tr.fm_off -= 1;
                if tr.fm_off == 0 {
                    let r = &mut self.regs.opll[0x20 + usize::from(c)];
                    if *r & 0x0f == 0 {
                        *r |= 0x08;
                    }
                }
            }
            let tr = &mut self.t[i];
            if std::mem::take(&mut tr.env_wait) {
                return;
            }
            if tr.run.on {
                let out = if tr.run.adsr {
                    (self.run_r(i) * (i32::from(self.t[i].vol) + 1)) >> 8
                } else {
                    (self.run_e(i) - (15 - i32::from(self.t[i].vol))).max(0)
                };
                if let Chip::Fm(c) = ch {
                    let r = &mut self.regs.opll[0x30 + usize::from(c)];
                    *r = (*r & 0xf0) | (15 - out as u8);
                }
            }
            return;
        }
        if tr.hw {
            return;
        }
        // An envelope just chosen runs from the next frame.
        let wait = std::mem::take(&mut tr.env_wait);
        if tr.rest_off {
            // The rest's own frame: an @e envelope still runs; the release
            // starts on the next.
            tr.rest_off = false;
            if tr.run.on && tr.keyed && !wait {
                let v = if tr.run.adsr {
                    ((self.run_r(i) * (i32::from(self.t[i].vol) + 1)) >> 8) as u8
                } else {
                    (self.run_e(i) - (15 - i32::from(self.t[i].vol))).max(0) as u8
                };
                self.t[i].outvol = v;
                match ch {
                    Chip::Psg(c) => self.regs.psg[8 + usize::from(c)] = v,
                    Chip::Scc(c) => self.regs.scc_vol[usize::from(c)] = v,
                    _ => {}
                }
            }
            let tr = &mut self.t[i];
            tr.keyed = false;
            tr.rel = tr.k;
            if tr.run.phase >= 2 {
                tr.run.phase = 3;
            } else {
                tr.run.off = true;
            }
            return;
        }
        let v = if !tr.played {
            0
        } else if wait {
            tr.outvol
        } else if tr.run.on && tr.run.adsr {
            let l = self.run_r(i);
            ((l * (i32::from(self.t[i].vol) + 1)) >> 8) as u8
        } else if tr.run.on && tr.keyed {
            let e = self.run_e(i);
            let v = (e - (15 - i32::from(self.t[i].vol))).max(0) as u8;
            self.t[i].outvol = v;
            v
        } else {
            let tr = &mut self.t[i];
            if !tr.keyed && tr.outvol > 0 {
                if tr.rel == 0 {
                    tr.outvol = 0;
                } else {
                    tr.rel -= 1;
                    if tr.rel == 0 {
                        tr.outvol -= 1;
                        tr.rel = tr.k;
                    }
                }
            }
            if std::mem::take(&mut tr.rel_reset) {
                tr.rel = tr.k;
            }
            tr.outvol
        };
        match ch {
            Chip::Psg(c) => self.regs.psg[8 + usize::from(c)] = v,
            Chip::Scc(c) => self.regs.scc_vol[usize::from(c)] = v,
            _ => {}
        }
    }

    /// The registers as the black-box logger prints them.
    pub fn log_line(&self) -> String {
        let r = &self.regs;
        let mut s = String::new();
        for b in &r.psg[..14] {
            s += &format!("{b:02x}");
        }
        s.push(' ');
        for b in r.scc_freq.iter().chain(r.scc_vol.iter()) {
            s += &format!("{b:02x}");
        }
        s += &format!("{:02x} ", r.scc_enable);
        for b in &r.opll[..0x39] {
            s += &format!("{b:02x}");
        }
        let mut sum: u32 = 0;
        for w in &r.scc_wave {
            for &b in w {
                sum = sum.wrapping_mul(31).wrapping_add(u32::from(b));
            }
        }
        s += &format!(" {sum:08x}");
        s
    }
}

/// The Tracker's channels: PSG, SCC, FM, then the drums.
pub const CHANNEL_NAMES: [&str; 22] = [
    "PSG 1", "PSG 2", "PSG 3", "SCC 1", "SCC 2", "SCC 3", "SCC 4", "SCC 5", "FM 1", "FM 2", "FM 3", "FM 4", "FM 5", "FM 6", "FM 7", "FM 8", "FM 9", "BD", "HH", "SD", "TOM", "CYM",
];

impl crate::replay::Tune for Song {
    fn format(&self) -> &'static str {
        "mgs"
    }
    fn file(&self) -> &[u8] {
        &self.file
    }
    fn driver(&self, _song: usize) -> Box<dyn crate::replay::Driver> {
        Box::new(Player::new(self))
    }
    fn length(&self, _song: usize) -> Option<f64> {
        Some(self.timeline().seconds)
    }

    /// What the song defines, each once (as it stands when the song
    /// starts): its OPLL voices (@v, the 8 register bytes; slots 1 to 32),
    /// its PSG envelopes (@e programs and @r ADSRs, with their mode and
    /// noise; slots 33 to 64) and its SCC waves (@s, 32 points; slots 65
    /// to 96).
    fn patches(&self) -> Vec<crate::replay::Patch> {
        let mut out = Vec::new();
        let mut seen = Vec::new();
        for &(kind, k) in self.defined.iter().rev() {
            if seen.contains(&(kind, k)) {
                continue;
            }
            seen.push((kind, k));
            let mut b = crate::replay::PatchBytes::default();
            b.u8(kind);
            let n = usize::from(k);
            let (slot, name) = match kind {
                0 => {
                    b.bytes(&self.fm_voices[n]);
                    (n + 1, format!("@v{k}"))
                }
                2 => {
                    match &self.envs[n] {
                        Some(Env::E { mode, noise, data }) => b.u8(0).u8(*mode).u8(*noise).bytes(data),
                        Some(Env::R { mode, noise, al, ar, dr, sl, sr, rr }) => b.u8(1).u8(*mode).u8(*noise).bytes(&[*al, *ar, *dr, *sl, *sr, *rr]),
                        None => continue,
                    };
                    (n + 33, format!("@e{k}"))
                }
                _ => {
                    b.bytes(&self.waves[n]);
                    (n + 65, format!("@s{k}"))
                }
            };
            out.push(b.patch(slot, &name));
        }
        out.sort_by_key(|p| p.slot);
        out
    }
}

/// An MSX frame (the driver's interrupt), in seconds.
pub const FRAME: f64 = 1.0 / 60.0;

// The most a timeline follows: 15 minutes.
const MAX_FRAMES: u32 = 60 * 60 * 15;

/// A song's notes, followed without playing it.
pub struct Timeline {
    /// Step (from 1; 192 a whole note), channel (CHANNEL_NAMES), MIDI note
    /// (None where a note is cut by a rest).
    pub events: Vec<(u64, usize, Option<u8>)>,
    /// Steps once through.
    pub steps: u64,
    /// The step it loops back to, when it loops.
    pub restart: Option<u64>,
    /// Seconds once through.
    pub seconds: f64,
}

// A channel at full volume reaches about this on each side (centred: 0.4 in all).
const LEVEL: f32 = 0.2;

/// A song played on the MSX's PSG, SCC and MSX-MUSIC.
pub struct Player {
    d: Driver,
    ay: crate::ay::Ay,
    scc: crate::scc::Scc,
    opll: crate::opll::Opll,
    // The registers as the chips have them.
    psg: [u8; 14],
    scc_freq: [u8; 10],
    scc_vol: [u8; 5],
    waves: [[u8; 32]; 4],
    opll_regs: [u8; 0x39],
    rate: u32,
    done: bool,
}

impl Player {
    pub fn new(song: &Song) -> Player {
        let mut p = Player {
            d: Driver::new(song),
            ay: crate::ay::Ay::new(false, 1_789_772.5, 48_000),
            scc: crate::scc::Scc::new(crate::scc::CLOCK, 48_000),
            opll: crate::opll::Opll::new(crate::opll::CLOCK, 48_000),
            psg: [0; 14],
            scc_freq: [0; 10],
            scc_vol: [0; 5],
            waves: [[0; 32]; 4],
            opll_regs: [0; 0x39],
            rate: 48_000,
            done: false,
        };
        p.scc.write(0xe1, 0x1f);
        p.d.frame();
        p.write_chips(true);
        p
    }

    // The driver's registers into the chips: what changed, and the key-ons
    // and drum hits again (a key already on goes off first).
    fn write_chips(&mut self, all: bool) {
        let r = self.d.regs.clone();
        let psg: [u8; 14] = r.psg[..14].try_into().unwrap();
        if all || psg != self.psg || r.shape {
            self.ay.write_regs(&psg, r.shape || all);
            self.psg = psg;
        }
        for c in 0..4 {
            if all || r.scc_wave[c] != self.waves[c] {
                for (k, &b) in r.scc_wave[c].iter().enumerate() {
                    self.scc.write((c * 32 + k) as u8, b);
                }
                self.waves[c] = r.scc_wave[c];
            }
        }
        for k in 0..10 {
            if all || r.scc_freq[k] != self.scc_freq[k] {
                self.scc.write(0xc0 + k as u8, r.scc_freq[k]);
                self.scc_freq[k] = r.scc_freq[k];
            }
        }
        for k in 0..5 {
            if all || r.scc_vol[k] != self.scc_vol[k] {
                self.scc.write(0xd0 + k as u8, r.scc_vol[k]);
                self.scc_vol[k] = r.scc_vol[k];
            }
        }
        for c in 0..9 {
            if r.rekey & (1 << c) != 0 && self.opll_regs[0x20 + c] & 0x10 != 0 {
                self.opll.write(0x20 + c, self.opll_regs[0x20 + c] & !0x10);
                self.opll_regs[0x20 + c] &= !0x10;
            }
        }
        if r.hits & self.opll_regs[0x0e] & 0x1f != 0 {
            let v = self.opll_regs[0x0e] & !r.hits;
            self.opll.write(0x0e, v);
            self.opll_regs[0x0e] = v;
        }
        for k in 0..0x39 {
            if all || r.opll[k] != self.opll_regs[k] {
                self.opll.write(k, r.opll[k]);
                self.opll_regs[k] = r.opll[k];
            }
        }
    }

    /// The driver, as it plays.
    pub fn driver(&self) -> &Driver {
        &self.d
    }
}

impl crate::replay::Driver for Player {
    fn channels(&self) -> usize {
        CHANNEL_NAMES.len()
    }

    fn tick(&mut self) {
        self.d.frame();
        self.write_chips(false);
        if self.d.take_round() {
            self.done = true;
        }
    }

    fn tick_seconds(&self) -> f64 {
        FRAME
    }

    fn render(&mut self, rate: u32, frames: usize, out: &mut crate::replay::Bufs) {
        if rate != self.rate {
            self.rate = rate;
            self.ay.set_rate(rate);
            let wanted = self.d.regs.clone();
            self.scc = crate::scc::Scc::new(crate::scc::CLOCK, rate);
            self.scc.write(0xe1, 0x1f);
            self.opll = crate::opll::Opll::new(crate::opll::CLOCK, rate);
            self.d.regs = wanted;
            self.write_chips(true);
        }
        for i in 0..frames {
            self.ay.process(true);
            for c in 0..3 {
                let v = self.ay.out[c] as f32 * LEVEL;
                out.left[c][i] = v;
                out.right[c][i] = v;
            }
            self.scc.calc();
            for (c, &x) in self.scc.channels().iter().enumerate() {
                let v = f32::from(x) * (LEVEL / 1904.0);
                out.left[3 + c][i] = v;
                out.right[3 + c][i] = v;
            }
            for (c, &x) in self.opll.calc_channels().iter().enumerate() {
                let v = f32::from(x) * (LEVEL / 2048.0);
                out.left[8 + c][i] = v;
                out.right[8 + c][i] = v;
            }
        }
    }

    fn position(&self) -> (usize, usize) {
        // A pattern a whole note, a row a sixteenth.
        let s = self.d.steps.saturating_sub(1);
        ((s / 192) as usize, ((s % 192) / 12) as usize)
    }

    fn speed(&self) -> (u32, u32) {
        let bpm = if self.d.song.legacy() { 125 } else { u32::from(self.d.song.tempo.max(1)) };
        (1, bpm)
    }

    fn take_done(&mut self) -> bool {
        std::mem::take(&mut self.done)
    }

    fn playing(&self) -> bool {
        self.d.playing()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tempo_tables_as_the_compiler_makes_them() {
        let t = tempo_table(120);
        assert_eq!(&t[..8], &[1, 0, 1, 1, 0, 1, 0, 1]);
        assert_eq!(tempo_table(75), [1; 192]);
    }

    // Two log lines the same, but for an FM channel's registers before the
    // reference first keys it on (the driver leaves them in states nobody hears).
    fn same_heard(ours: &str, want: &str, keyed: &mut [bool; 9], opll_mode: u8) -> bool {
        if ours == want {
            return true;
        }
        let (o, w): (Vec<&str>, Vec<&str>) = (ours.split(' ').collect(), want.split(' ').collect());
        if o.len() != 4 || w.len() != 4 || o[0] != w[0] || o[1] != w[1] || o[3] != w[3] {
            return false;
        }
        let (ob, wb) = (o[2].as_bytes(), w[2].as_bytes());
        let reg = |b: &[u8], r: usize| u8::from_str_radix(std::str::from_utf8(&b[r * 2..r * 2 + 2]).unwrap(), 16).unwrap();
        for c in 0..9 {
            if reg(wb, 0x20 + c) & 0x10 != 0 {
                keyed[c] = true;
            }
        }
        (0..0x39).all(|r| {
            let c = r & 0x0f;
            let masked = (0x10..0x39).contains(&r) && c < 9 && !keyed[c] && !(opll_mode == 1 && c >= 6);
            masked || reg(ob, r) == reg(wb, r)
        })
    }

    /// The chips' registers each frame as MGSDRV leaves them, run as a black
    /// box: MGS_DIR the songs, MGS_REF the logs (<name>.log).
    #[test]
    #[ignore]
    fn the_same_as_mgsdrv() {
        let (Ok(dir), Ok(rf)) = (std::env::var("MGS_DIR"), std::env::var("MGS_REF")) else { return };
        let only = std::env::var("MGS_ONLY").ok();
        let show = std::env::var("MGS_SHOW").is_ok();
        let mut paths: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("mgs")))
            .collect();
        paths.sort();
        let (mut same, mut total) = (0, 0);
        for path in paths {
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            if only.as_ref().is_some_and(|o| !name.contains(o.as_str())) {
                continue;
            }
            let stem = name.trim_end_matches(".mgs").trim_end_matches(".MGS");
            let Ok(log) = std::fs::read_to_string(format!("{rf}/{stem}.log")).or_else(|_| std::fs::read_to_string(format!("{rf}/{name}.log"))) else {
                continue;
            };
            total += 1;
            let Some(song) = Song::parse(&std::fs::read(&path).unwrap()) else {
                eprintln!("{name}: not read");
                continue;
            };
            // Ours against the reference, line by line. The reference's frames
            // slip where the driver's interrupts ran long (two logged as one, or
            // one twice), so a mismatch is forgiven where the next lines agree
            // with one side a frame on.
            let lines: Vec<&str> = log.lines().collect();
            // MGSDRV takes a frame or two to load a song before it plays
            // (longer for a packed one): the reference starts where ours
            // matches longest, at most 4 frames in.
            let mut best: Option<(usize, usize)> = None;
            for skip in 0..=4usize.min(lines.len()) {
                let mut d = Driver::new(&song);
                let mut keyed = [false; 9];
                let n = lines[skip..]
                    .iter()
                    .take_while(|w| {
                        d.frame();
                        same_heard(&d.log_line(), w, &mut keyed, song.opll_mode)
                    })
                    .count();
                if best.is_none_or(|b| n > b.1) {
                    best = Some((skip, n));
                }
            }
            let want: Vec<&str> = lines[best.map_or(0, |b| b.0)..].to_vec();
            let mut d = Driver::new(&song);
            let ours: Vec<String> = (0..want.len() + 40)
                .map(|_| {
                    d.frame();
                    d.log_line()
                })
                .collect();
            let mut keyed = [false; 9];
            let (mut o, mut r, mut slips, mut bad) = (0usize, 0usize, 0, None);
            let agree = |o: usize, r: usize, keyed: &[bool; 9]| (0..1).all(|k| r + k >= want.len() || same_heard(&ours[o + k], want[r + k], &mut keyed.clone(), song.opll_mode));
            while r < want.len() {
                if same_heard(&ours[o], want[r], &mut keyed, song.opll_mode) {
                    o += 1;
                    r += 1;
                    continue;
                }
                if slips < 64 && agree(o + 1, r, &keyed) {
                    o += 1;
                } else if slips < 64 && agree(o, r + 1, &keyed) {
                    r += 1;
                } else if slips < 64 && o > 0 && agree(o - 1, r, &keyed) {
                    // A late interrupt: the reference holds a frame.
                    r += 1;
                } else {
                    bad = Some(r);
                    break;
                }
                slips += 1;
            }
            match bad {
                None => {
                    same += 1;
                    if slips > 0 {
                        eprintln!("{name}: the same, {slips} slips");
                    }
                }
                Some(f) => {
                    eprintln!("{name}: frame {f} (ours {o}, {slips} slips)");
                    if show {
                        for g in f.saturating_sub(3)..=f {
                            let Some(og) = (o + g).checked_sub(f) else { continue };
                            let (ol, w) = (&ours[og], want.get(g).unwrap_or(&""));
                            let mark: String = ol.chars().zip(w.chars()).map(|(a, b)| if a == b { ' ' } else { '^' }).collect();
                            eprintln!("  {g:5} ours {ol}\n        want {w}\n             {mark}");
                        }
                    }
                }
            }
            if let Ok(out) = std::env::var("MGS_DUMP") {
                std::fs::write(&out, ours.join("\n") + "\n").unwrap();
            }
        }
        eprintln!("{same}/{total} the same");
        assert_eq!(same, total);
    }
}
