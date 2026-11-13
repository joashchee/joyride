// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! FamiTracker's modules (.ftm; also 0CC-FamiTracker's .0cc and
//! Dn-FamiTracker's): NES music for the 2A03 and the expansion chips
//! (VRC6, VRC7, FDS, MMC5, N163, Sunsoft 5B). Read from the module format
//! as Dn-FamiTracker's documentation describes it, every version from
//! FamiTracker 0.2.2 to Dn-FamiTracker 0.5.

use std::collections::HashMap;

/// A sequence: its values, loop and release points (-1 for none) and setting.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sequence {
    pub values: Vec<i8>,
    pub loop_point: i32,
    pub release: i32,
    pub setting: u32,
}

/// Which chip an instrument is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum InstrumentKind {
    #[default]
    Apu,
    Vrc6,
    Vrc7,
    Fds,
    N163,
    S5b,
}

/// A DPCM sample a note plays: the sample, its pitch (0-15, bit 7 loops)
/// and the delta counter it starts at (-1 for unchanged).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DpcmKey {
    pub sample: u8,
    pub pitch: u8,
    pub delta: i8,
}

#[derive(Clone, Debug, Default)]
pub struct Instrument {
    pub kind: InstrumentKind,
    pub name: String,
    /// Each sequence type's (enabled, index): volume, arpeggio, pitch, hi-pitch, duty.
    pub seq: [(bool, u8); 5],
    /// 2A03: the DPCM sample each of 96 notes plays (sample 0 none, else index + 1).
    pub dpcm: Vec<DpcmKey>,
    /// VRC7: the patch, and the custom patch's registers.
    pub vrc7_patch: u8,
    pub vrc7_regs: [u8; 8],
    /// FDS: its wave (64 steps of 0-63), modulation table (32 of 0-7),
    /// speed, depth and delay, and its own volume, arpeggio and pitch sequences.
    pub fds_wave: Vec<u8>,
    pub fds_mod: Vec<u8>,
    pub fds_mod_speed: i32,
    pub fds_mod_depth: i32,
    pub fds_mod_delay: i32,
    pub fds_seqs: Vec<Sequence>,
    /// N163: its wave size, position, and waves (each of 4-bit samples).
    pub n163_size: usize,
    pub n163_pos: usize,
    pub n163_auto: bool,
    pub n163_waves: Vec<Vec<u8>>,
}

/// A row of a channel: note (0 none, 1-12 C to B, 13 release, 14 halt,
/// 15 echo), octave, instrument (64 none), volume (16 none), and up to four
/// effects (as FamiTracker numbers them in memory, 0 none) and parameters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Row {
    pub note: u8,
    pub octave: u8,
    pub instrument: u8,
    pub volume: u8,
    pub fx: [(u8, u8); 4],
}

impl Default for Row {
    fn default() -> Row {
        Row { note: 0, octave: 0, instrument: NO_INSTRUMENT, volume: NO_VOLUME, fx: [(0, 0); 4] }
    }
}

pub const NO_INSTRUMENT: u8 = 64;
pub const NO_VOLUME: u8 = 16;
pub const HOLD_INSTRUMENT: u8 = 0xFF;
pub const NOTE_RELEASE: u8 = 13;
pub const NOTE_HALT: u8 = 14;
pub const NOTE_ECHO: u8 = 15;

// Effects, in memory (0CC/Dn-FamiTracker's order).
pub const EF_SPEED: u8 = 0x01;
pub const EF_JUMP: u8 = 0x02;
pub const EF_SKIP: u8 = 0x03;
pub const EF_HALT: u8 = 0x04;
pub const EF_VOLUME: u8 = 0x05;
pub const EF_PORTAMENTO: u8 = 0x06;
pub const EF_PORTAOFF: u8 = 0x07;
pub const EF_SWEEPUP: u8 = 0x08;
pub const EF_SWEEPDOWN: u8 = 0x09;
pub const EF_ARPEGGIO: u8 = 0x0A;
pub const EF_VIBRATO: u8 = 0x0B;
pub const EF_TREMOLO: u8 = 0x0C;
pub const EF_PITCH: u8 = 0x0D;
pub const EF_DELAY: u8 = 0x0E;
pub const EF_DAC: u8 = 0x0F;
pub const EF_PORTA_UP: u8 = 0x10;
pub const EF_PORTA_DOWN: u8 = 0x11;
pub const EF_DUTY_CYCLE: u8 = 0x12;
pub const EF_SAMPLE_OFFSET: u8 = 0x13;
pub const EF_SLIDE_UP: u8 = 0x14;
pub const EF_SLIDE_DOWN: u8 = 0x15;
pub const EF_VOLUME_SLIDE: u8 = 0x16;
pub const EF_NOTE_CUT: u8 = 0x17;
pub const EF_RETRIGGER: u8 = 0x18;
pub const EF_DELAYED_VOLUME: u8 = 0x19;
pub const EF_FDS_MOD_DEPTH: u8 = 0x1A;
pub const EF_FDS_MOD_SPEED_HI: u8 = 0x1B;
pub const EF_FDS_MOD_SPEED_LO: u8 = 0x1C;
pub const EF_DPCM_PITCH: u8 = 0x1D;
pub const EF_SUNSOFT_ENV_TYPE: u8 = 0x1E;
pub const EF_SUNSOFT_ENV_HI: u8 = 0x1F;
pub const EF_SUNSOFT_ENV_LO: u8 = 0x20;
pub const EF_SUNSOFT_NOISE: u8 = 0x21;
pub const EF_VRC7_PORT: u8 = 0x22;
pub const EF_VRC7_WRITE: u8 = 0x23;
pub const EF_NOTE_RELEASE: u8 = 0x24;
pub const EF_GROOVE: u8 = 0x25;
pub const EF_TRANSPOSE: u8 = 0x26;
pub const EF_N163_WAVE_BUFFER: u8 = 0x27;
pub const EF_FDS_VOLUME: u8 = 0x28;
pub const EF_FDS_MOD_BIAS: u8 = 0x29;
pub const EF_PHASE_RESET: u8 = 0x2A;
pub const EF_HARMONIC: u8 = 0x2B;
pub const EF_TARGET_VOLUME_SLIDE: u8 = 0x2C;
pub const EF_COUNT: u8 = 0x2D;

/// An effect number as the file has it (FamiTracker 0.5's order) to the order in memory.
fn effect_from_file(e: u8) -> u8 {
    match e {
        0x21 => EF_NOTE_RELEASE,
        0x22 => EF_GROOVE,
        0x23 => EF_TRANSPOSE,
        0x24 => EF_N163_WAVE_BUFFER,
        0x25 => EF_FDS_VOLUME,
        0x26 => EF_FDS_MOD_BIAS,
        0x27 => EF_SUNSOFT_NOISE,
        0x28 => EF_VRC7_PORT,
        0x29 => EF_VRC7_WRITE,
        e if e < EF_COUNT => e,
        _ => 0,
    }
}

/// A channel's chip and number on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chan {
    Pulse(u8),
    Triangle,
    Noise,
    Dpcm,
    Vrc6Pulse(u8),
    Vrc6Saw,
    Mmc5Pulse(u8),
    N163(u8),
    Fds,
    Vrc7(u8),
    S5b(u8),
}

impl Chan {
    /// From FamiTracker's channel id.
    pub fn from_id(id: u8) -> Option<Chan> {
        Some(match id {
            0 | 1 => Chan::Pulse(id),
            2 => Chan::Triangle,
            3 => Chan::Noise,
            4 => Chan::Dpcm,
            5 | 6 => Chan::Vrc6Pulse(id - 5),
            7 => Chan::Vrc6Saw,
            8 | 9 => Chan::Mmc5Pulse(id - 8),
            0x0B..=0x12 => Chan::N163(id - 0x0B),
            0x13 => Chan::Fds,
            0x14..=0x19 => Chan::Vrc7(id - 0x14),
            0x1A..=0x1C => Chan::S5b(id - 0x1A),
            _ => return None,
        })
    }

    pub fn name(&self) -> String {
        match self {
            Chan::Pulse(n) => format!("Pulse {}", n + 1),
            Chan::Triangle => "Triangle".into(),
            Chan::Noise => "Noise".into(),
            Chan::Dpcm => "DPCM".into(),
            Chan::Vrc6Pulse(n) => format!("VRC6 Pulse {}", n + 1),
            Chan::Vrc6Saw => "VRC6 Sawtooth".into(),
            Chan::Mmc5Pulse(n) => format!("MMC5 Pulse {}", n + 1),
            Chan::N163(n) => format!("N163 {}", n + 1),
            Chan::Fds => "FDS".into(),
            Chan::Vrc7(n) => format!("VRC7 FM {}", n + 1),
            Chan::S5b(n) => format!("5B {}", ["A", "B", "C"][usize::from(*n)]),
        }
    }
}

/// A song (FamiTracker's "track") of the module.
#[derive(Clone, Debug, Default)]
pub struct Track {
    pub name: String,
    pub speed: u32,
    pub tempo: u32,
    pub rows: usize,
    /// Each frame's pattern for each channel.
    pub frames: Vec<Vec<u8>>,
    /// Each channel's effect columns (1 to 4).
    pub columns: Vec<usize>,
    /// (channel, pattern) to its rows.
    pub patterns: HashMap<(usize, u8), Vec<Row>>,
    /// Grooves instead of a speed (0CC).
    pub groove: bool,
}

impl Track {
    /// A channel's row in a frame (blank past what's written).
    pub fn row(&self, frame: usize, channel: usize, row: usize) -> Row {
        let Some(&p) = self.frames.get(frame).and_then(|f| f.get(channel)) else { return Row::default() };
        self.patterns.get(&(channel, p)).and_then(|r| r.get(row)).copied().unwrap_or_default()
    }
}

/// A FamiTracker module.
#[derive(Clone, Debug, Default)]
pub struct Module {
    pub version: u32,
    pub dn: bool,
    pub title: String,
    pub artist: String,
    pub copyright: String,
    pub comment: String,
    /// NSF expansion bits: 1 VRC6, 2 VRC7, 4 FDS, 8 MMC5, 16 N163, 32 5B.
    pub expansion: u8,
    pub pal: bool,
    /// The engine's rate in Hz (0 for the machine's).
    pub engine_speed: u32,
    pub vibrato_new: bool,
    pub n163_channels: usize,
    pub speed_split: u32,
    pub linear_pitch: bool,
    pub detune_semitone: i8,
    pub detune_cent: i8,
    pub channels: Vec<Chan>,
    pub tracks: Vec<Track>,
    pub instruments: HashMap<u8, Instrument>,
    /// Sequences of each chip ("2A03", "VRC6", "N163", "5B"): (index, type) to sequence.
    pub sequences: HashMap<(InstrumentKind, u8, u8), Sequence>,
    pub samples: HashMap<u8, Vec<u8>>,
    pub sample_names: HashMap<u8, String>,
    pub grooves: HashMap<u8, Vec<u8>>,
    /// Detune tables (chip 0 NTSC .. 5 N163): note to offset.
    pub detunes: HashMap<(u8, u8), i32>,
}

struct R<'a> {
    d: &'a [u8],
    at: usize,
    end: usize,
}

impl R<'_> {
    fn ok(&self, n: usize) -> bool {
        self.at + n <= self.end
    }
    fn u8(&mut self) -> Option<u8> {
        if !self.ok(1) {
            return None;
        }
        self.at += 1;
        Some(self.d[self.at - 1])
    }
    fn i8(&mut self) -> Option<i8> {
        self.u8().map(|b| b as i8)
    }
    fn u32(&mut self) -> Option<u32> {
        if !self.ok(4) {
            return None;
        }
        let v = u32::from_le_bytes(self.d[self.at..self.at + 4].try_into().ok()?);
        self.at += 4;
        Some(v)
    }
    fn i32(&mut self) -> Option<i32> {
        self.u32().map(|v| v as i32)
    }
    fn bytes(&mut self, n: usize) -> Option<&[u8]> {
        if !self.ok(n) {
            return None;
        }
        self.at += n;
        Some(&self.d[self.at - n..self.at])
    }
    fn cstr(&mut self) -> Option<String> {
        let start = self.at;
        while self.at < self.end && self.d[self.at] != 0 {
            self.at += 1;
        }
        let s = text(&self.d[start..self.at]);
        if self.at < self.end {
            self.at += 1;
        }
        Some(s)
    }
}

fn text(b: &[u8]) -> String {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    b[..end].iter().map(|&c| if (32..127).contains(&c) { c as char } else if c >= 0xA0 { c as char } else { ' ' }).collect::<String>().trim().to_string()
}

pub fn probe(d: &[u8]) -> bool {
    d.starts_with(b"FamiTracker Module") || d.starts_with(b"Dn-FamiTracker Module")
}

/// The channels a module has, from its expansion chips (as FamiTracker lays them out).
fn layout(expansion: u8, n163: usize) -> Vec<Chan> {
    let mut c = vec![Chan::Pulse(0), Chan::Pulse(1), Chan::Triangle, Chan::Noise, Chan::Dpcm];
    if expansion & 1 != 0 {
        c.extend([Chan::Vrc6Pulse(0), Chan::Vrc6Pulse(1), Chan::Vrc6Saw]);
    }
    if expansion & 8 != 0 {
        c.extend([Chan::Mmc5Pulse(0), Chan::Mmc5Pulse(1)]);
    }
    if expansion & 16 != 0 {
        c.extend((0..n163.clamp(1, 8) as u8).map(Chan::N163));
    }
    if expansion & 4 != 0 {
        c.push(Chan::Fds);
    }
    if expansion & 2 != 0 {
        c.extend((0..6).map(Chan::Vrc7));
    }
    if expansion & 32 != 0 {
        c.extend((0..3).map(Chan::S5b));
    }
    c
}

impl Module {
    pub fn parse(d: &[u8]) -> Option<Module> {
        let (dn, id_len) = if d.starts_with(b"Dn-FamiTracker Module") {
            (true, 21)
        } else if d.starts_with(b"FamiTracker Module") {
            (false, 18)
        } else {
            return None;
        };
        let mut m = Module { dn, speed_split: 0x20, n163_channels: 4, vibrato_new: true, ..Default::default() };
        let mut r = R { d, at: id_len, end: d.len() };
        m.version = r.u32()?;
        let mut channels_available = 5usize;
        let mut header: Option<(Vec<u8>, Vec<Vec<usize>>)> = None;
        // Blocks read after HEADER/FRAMES need the track count first: keep the raw ones.
        let mut blocks: Vec<(String, u32, usize, usize)> = Vec::new();
        loop {
            if r.ok(3) && &d[r.at..r.at + 3] == b"END" {
                break;
            }
            if !r.ok(24) {
                break;
            }
            let id = text(&d[r.at..r.at + 16]);
            r.at += 16;
            let ver = r.u32()? & 0xFFFF;
            let size = r.u32()? as usize;
            let start = r.at;
            let end = (start + size).min(d.len());
            blocks.push((id, ver, start, end));
            r.at = end;
        }
        // PARAMS first: it says how many channels there are.
        let mut track_count = 1usize;
        for (id, ver, start, end) in &blocks {
            let mut b = R { d, at: *start, end: *end };
            match id.as_str() {
                "PARAMS" => {
                    if *ver == 1 {
                        let speed = b.u32()?;
                        let _ = speed;
                    } else {
                        m.expansion = b.u8()?;
                    }
                    channels_available = b.u32()? as usize;
                    m.pal = b.u32()? == 1;
                    if *ver >= 7 {
                        let _rate_type = b.u32()?;
                        let _rate = b.u32()?;
                    } else {
                        m.engine_speed = b.u32()?;
                    }
                    if *ver >= 3 {
                        m.vibrato_new = b.u32()? == 1;
                    } else {
                        m.vibrato_new = false;
                    }
                    if *ver >= 7 {
                        let _sweep_reset = b.u32()?;
                    }
                    if (4..=6).contains(ver) {
                        b.u32()?;
                        b.u32()?;
                    }
                    // Written only when the N163 is on.
                    if *ver >= 5 && m.expansion & 16 != 0 {
                        m.n163_channels = (b.u32()? as usize).clamp(1, 8);
                    }
                    if *ver >= 6 {
                        m.speed_split = b.u32()?;
                    } else {
                        m.speed_split = 0x15;
                    }
                    if (8..=9).contains(ver) {
                        m.detune_semitone = b.i8().unwrap_or(0);
                        m.detune_cent = b.i8().unwrap_or(0);
                    }
                }
                "INFO" => {
                    if *ver >= 2 && b.end - b.at != 96 {
                        m.title = b.cstr()?;
                        m.artist = b.cstr()?;
                        m.copyright = b.cstr()?;
                    } else {
                        m.title = text(b.bytes(32)?);
                        m.artist = text(b.bytes(32)?);
                        m.copyright = text(b.bytes(32)?);
                    }
                }
                "HEADER" => {
                    if *ver == 1 {
                        track_count = 1;
                    } else {
                        track_count = usize::from(b.u8()?) + 1;
                    }
                    let mut names = Vec::new();
                    if *ver >= 3 {
                        for _ in 0..track_count {
                            names.push(b.cstr()?);
                        }
                    }
                    let mut types = Vec::new();
                    let mut columns = vec![vec![1usize; channels_available]; track_count];
                    for c in 0..channels_available {
                        types.push(b.u8()?);
                        let n = if *ver == 1 { 1 } else { track_count };
                        for col in columns.iter_mut().take(n) {
                            col[c] = usize::from(b.u8()?).min(3) + 1;
                        }
                    }
                    header = Some((types, columns));
                    m.tracks = names.into_iter().map(|name| Track { name, ..Default::default() }).collect();
                }
                "COMMENTS" => {
                    b.u32()?;
                    m.comment = b.cstr().unwrap_or_default();
                }
                "PARAMS_EXTRA" => {
                    m.linear_pitch = b.u32()? != 0;
                    if *ver >= 2 {
                        m.detune_semitone = b.i8().unwrap_or(0);
                        m.detune_cent = b.i8().unwrap_or(0);
                    }
                }
                "TUNING" => {
                    m.detune_semitone = b.i8().unwrap_or(0);
                    m.detune_cent = b.i8().unwrap_or(0);
                }
                _ => {}
            }
        }
        if m.tracks.len() < track_count {
            m.tracks.resize(track_count, Track::default());
        }
        // The channels: the header's types where they're known, else the chips' layout.
        let laid = layout(m.expansion, m.n163_channels);
        m.channels = match &header {
            Some((types, _)) => types.iter().enumerate().map(|(k, &t)| Chan::from_id(t).or_else(|| laid.get(k).copied()).unwrap_or(Chan::Pulse(0))).collect(),
            None => laid.into_iter().take(channels_available).collect(),
        };
        if let Some((_, columns)) = &header {
            for (t, cols) in m.tracks.iter_mut().zip(columns) {
                t.columns = cols.clone();
            }
        }
        for t in m.tracks.iter_mut() {
            t.columns.resize(m.channels.len(), 1);
        }
        let nch = m.channels.len();
        for (id, ver, start, end) in &blocks {
            let mut b = R { d, at: *start, end: *end };
            if m.read_block(id, *ver, &mut b, nch).is_none() {
                if std::env::var("FT_DEBUG").is_ok() {
                    eprintln!("block {id} v{ver} failed at {} of {}..{}", b.at, start, end);
                }
                return None;
            }
        }
        if m.version <= 0x0200 && m.tracks.len() == 1 && m.tracks[0].rows == 0 {
            m.tracks[0].rows = 64;
        }
        for t in m.tracks.iter_mut() {
            if t.frames.is_empty() {
                t.frames.push(vec![0; nch]);
            }
            for f in t.frames.iter_mut() {
                f.resize(nch, 0);
            }
            if t.rows == 0 {
                t.rows = 64;
            }
            if t.speed == 0 {
                t.speed = 6;
            }
        }
        if m.tracks.is_empty() {
            return None;
        }
        Some(m)
    }

    fn read_block(&mut self, id: &str, ver: u32, b: &mut R, nch: usize) -> Option<()> {
        let m = self;
        let ver = &ver;
        {
            match id {
                "INSTRUMENTS" => m.read_instruments(b, *ver)?,
                "SEQUENCES" => m.read_sequences(b, *ver, InstrumentKind::Apu)?,
                // Its first version is laid out as the 2A03's third.
                "SEQUENCES_VRC6" => m.read_sequences(b, if *ver == 1 { 3 } else { *ver }, InstrumentKind::Vrc6)?,
                "SEQUENCES_N163" | "SEQUENCES_N106" => m.read_sequences_n163(b, InstrumentKind::N163)?,
                "SEQUENCES_S5B" => m.read_sequences_n163(b, InstrumentKind::S5b)?,
                "FRAMES" => {
                    if *ver == 1 {
                        let frames = b.u32()? as usize;
                        let chans = b.u32()? as usize;
                        let t = &mut m.tracks[0];
                        for _ in 0..frames {
                            let mut f = Vec::new();
                            for _ in 0..chans {
                                f.push(b.u8()?);
                            }
                            t.frames.push(f);
                        }
                    } else {
                        for ti in 0..m.tracks.len() {
                            let frames = b.u32()? as usize;
                            let speed = b.u32()?;
                            let (mut sp, mut tempo) = (speed, if m.pal { 125 } else { 150 });
                            if *ver >= 3 {
                                tempo = b.u32()?;
                            } else if speed >= 20 {
                                tempo = speed;
                                sp = 6;
                            }
                            let rows = b.u32()? as usize;
                            let t = &mut m.tracks[ti];
                            t.speed = sp;
                            t.tempo = tempo;
                            t.rows = rows.clamp(1, 256);
                            for _ in 0..frames {
                                let mut f = Vec::new();
                                for _ in 0..nch {
                                    f.push(b.u8()?);
                                }
                                t.frames.push(f);
                            }
                        }
                    }
                }
                "PATTERNS" => m.read_patterns(b, *ver)?,
                "DPCM SAMPLES" => {
                    let count = b.u8()?;
                    for _ in 0..count {
                        let i = b.u8()?;
                        let n = b.u32()? as usize;
                        let name = text(b.bytes(n)?);
                        let size = (b.u32()? as usize).min(0x7FFF);
                        let data = b.bytes(size)?.to_vec();
                        m.samples.insert(i, data);
                        m.sample_names.insert(i, name);
                    }
                }
                "GROOVES" => {
                    let count = b.u8()?;
                    for _ in 0..count {
                        let i = b.u8()?;
                        let size = b.u8()?;
                        let v = b.bytes(usize::from(size))?.to_vec();
                        m.grooves.insert(i, v);
                    }
                    let tracks = b.u8()?;
                    for t in 0..usize::from(tracks) {
                        let u = b.u8()?;
                        if let Some(tr) = m.tracks.get_mut(t) {
                            tr.groove = u != 0;
                        }
                    }
                }
                "DETUNETABLES" => {
                    let count = b.u8()?;
                    for _ in 0..count {
                        let chip = b.u8()?;
                        let items = b.u8()?;
                        for _ in 0..items {
                            let note = b.u8()?;
                            let off = b.i32()?;
                            m.detunes.insert((chip, note), off);
                        }
                    }
                }
                "TRACK" => {}
                _ => {}
            }
        }
        Some(())
    }

    fn read_seq_instrument(b: &mut R, ins: &mut Instrument) -> Option<()> {
        let count = b.i32()?;
        for k in 0..count.clamp(0, 5) as usize {
            let en = b.u8()? != 0;
            let i = b.u8()?;
            ins.seq[k] = (en, i);
        }
        for _ in 5..count.max(5) {
            b.u8()?;
            b.u8()?;
        }
        Some(())
    }

    fn read_instruments(&mut self, b: &mut R, ver: u32) -> Option<()> {
        let count = b.i32()?;
        for _ in 0..count.max(0) {
            let index = b.i32()? as u8;
            let kind = match b.u8()? {
                1 => InstrumentKind::Apu,
                2 => InstrumentKind::Vrc6,
                3 => InstrumentKind::Vrc7,
                4 => InstrumentKind::Fds,
                5 => InstrumentKind::N163,
                6 => InstrumentKind::S5b,
                _ => return None,
            };
            let mut ins = Instrument { kind, dpcm: vec![DpcmKey::default(); 96], ..Default::default() };
            match kind {
                InstrumentKind::Apu => {
                    Module::read_seq_instrument(b, &mut ins)?;
                    let octaves = if ver == 1 { 6 } else { 8 };
                    if ver >= 7 {
                        let n = b.i32()?;
                        for _ in 0..n.max(0) {
                            let note = usize::from(b.u8()?);
                            let s = b.u8()?;
                            let p = b.u8()?;
                            let delta = if ver >= 6 { b.i8()? } else { -1 };
                            if note < 96 {
                                ins.dpcm[note] = DpcmKey { sample: s, pitch: p, delta };
                            }
                        }
                    } else {
                        for note in 0..octaves * 12 {
                            let s = b.u8()?;
                            let p = b.u8()?;
                            let delta = if ver >= 6 { b.i8()? } else { -1 };
                            ins.dpcm[note] = DpcmKey { sample: s, pitch: p, delta };
                        }
                    }
                }
                InstrumentKind::Vrc6 | InstrumentKind::S5b => Module::read_seq_instrument(b, &mut ins)?,
                InstrumentKind::N163 => {
                    Module::read_seq_instrument(b, &mut ins)?;
                    let size = b.i32()? as usize;
                    let pos = b.i32()? as usize;
                    if ver >= 8 {
                        ins.n163_auto = b.i32()? != 0;
                    }
                    let count = b.i32()? as usize;
                    ins.n163_size = size.clamp(4, 240);
                    ins.n163_pos = pos.min(255);
                    for _ in 0..count.min(64) {
                        ins.n163_waves.push(b.bytes(size.min(240))?.to_vec());
                    }
                }
                InstrumentKind::Vrc7 => {
                    ins.vrc7_patch = b.i32()? as u8;
                    ins.vrc7_regs.copy_from_slice(b.bytes(8)?);
                }
                InstrumentKind::Fds => {
                    ins.fds_wave = b.bytes(64)?.to_vec();
                    ins.fds_mod = b.bytes(32)?.to_vec();
                    ins.fds_mod_speed = b.i32()?;
                    ins.fds_mod_depth = b.i32()?;
                    ins.fds_mod_delay = b.i32()?;
                    // Its volume, arpeggio and pitch sequences (FDS volume 0-31 past version 3).
                    let kinds = if ver > 2 { 3 } else { 2 };
                    for _ in 0..kinds {
                        let n = usize::from(b.u8()?);
                        let loop_point = b.i32()?;
                        let (release, setting) = if ver >= 4 { (b.i32()?, b.u32()?) } else { (-1, 0) };
                        let values = b.bytes(n)?.iter().map(|&v| v as i8).collect();
                        ins.fds_seqs.push(Sequence { values, loop_point, release, setting });
                    }
                    if ver <= 3 {
                        if let Some(s) = ins.fds_seqs.first_mut() {
                            for v in s.values.iter_mut() {
                                *v = v.saturating_mul(2);
                            }
                        }
                    }
                }
            }
            let n = b.i32()? as usize;
            ins.name = text(b.bytes(n.min(256))?);
            self.instruments.insert(index, ins);
        }
        Some(())
    }

    fn read_sequences(&mut self, b: &mut R, ver: u32, kind: InstrumentKind) -> Option<()> {
        let count = b.u32()? as usize;
        let mut order = Vec::new();
        for _ in 0..count.min(4096) {
            let index = b.u32()? as u8;
            let ty = if ver >= 2 { b.u32()? as u8 } else { 0 };
            if ver <= 2 {
                // Run-length pairs of value and length - 1.
                let runs = b.u8()?;
                let mut values = Vec::new();
                let mut loop_point = -1;
                for _ in 0..runs {
                    let v = b.i8()?;
                    let l = b.u8()?;
                    if ty == 0 || v as u8 != 0xFF {
                        for _ in 0..=l {
                            values.push(v);
                        }
                    } else {
                        loop_point = values.len() as i32 - i32::from(l);
                    }
                }
                self.sequences.insert((kind, index, ty), Sequence { values, loop_point, release: -1, setting: 0 });
                continue;
            }
            let n = usize::from(b.u8()?);
            let loop_point = b.i32()?;
            let (mut release, mut setting) = (-1, 0);
            if ver == 4 || ver >= 7 {
                release = b.i32()?;
                setting = b.u32()?;
            }
            let values = b.bytes(n)?.iter().map(|&v| v as i8).collect();
            self.sequences.insert((kind, index, ty), Sequence { values, loop_point, release, setting });
            order.push((index, ty));
        }
        if ver == 5 || ver == 6 {
            // Release points and settings in a list of their own, by index and type.
            for i in 0..128u8 {
                for ty in 0..5u8 {
                    if let Some(s) = self.sequences.get_mut(&(kind, i, ty)) {
                        if !order.contains(&(i, ty)) {
                            continue;
                        }
                        let Some(rel) = b.i32() else { return Some(()) };
                        let Some(set) = b.u32() else { return Some(()) };
                        s.release = rel;
                        s.setting = set;
                    }
                }
            }
        }
        Some(())
    }

    fn read_sequences_n163(&mut self, b: &mut R, kind: InstrumentKind) -> Option<()> {
        let count = b.u32()? as usize;
        for _ in 0..count.min(4096) {
            let index = b.u32()? as u8;
            let ty = b.u32()? as u8;
            let n = usize::from(b.u8()?);
            let loop_point = b.i32()?;
            let release = b.i32()?;
            let setting = b.u32()?;
            let values = b.bytes(n)?.iter().map(|&v| v as i8).collect();
            self.sequences.insert((kind, index, ty), Sequence { values, loop_point, release, setting });
        }
        Some(())
    }

    fn read_patterns(&mut self, b: &mut R, ver: u32) -> Option<()> {
        let nch = self.channels.len();
        if ver == 1 {
            let rows = b.u32()? as usize;
            if let Some(t) = self.tracks.get_mut(0) {
                t.rows = rows.clamp(1, 256);
            }
        }
        while b.at < b.end {
            let track = if ver > 1 { b.u32()? as usize } else { 0 };
            let channel = b.u32()? as usize;
            let pattern = b.u32()? as u8;
            let items = b.u32()? as usize;
            let cols = self.tracks.get(track).and_then(|t| t.columns.get(channel)).copied().unwrap_or(1);
            let chan = self.channels.get(channel).copied();
            let mut rows: Vec<Row> = vec![Row::default(); 256];
            for _ in 0..items.min(256) {
                let row = if self.version == 0x0200 || ver >= 6 { usize::from(b.u8()?) } else { b.u32()? as usize };
                let mut r = Row { note: b.u8()?, octave: b.u8()?, instrument: b.u8()?, volume: b.u8()?, fx: [(0, 0); 4] };
                let stored = if self.version == 0x0200 { 1 } else { cols };
                for k in 0..stored {
                    let e = b.u8()?;
                    let p = if ver >= 6 && e == 0 { 0 } else { b.u8()? };
                    if k < 4 {
                        r.fx[k] = (effect_from_file(e), p);
                    }
                }
                for k in cols..4 {
                    r.fx[k] = (0, 0);
                }
                self.fix_row(&mut r, ver, chan);
                if row < 256 {
                    rows[row] = r;
                }
            }
            if channel < nch {
                if let Some(t) = self.tracks.get_mut(track) {
                    t.patterns.insert((channel, pattern), rows);
                }
            }
        }
        Some(())
    }

    /// A row from an older module as newer versions have it.
    fn fix_row(&self, r: &mut Row, ver: u32, chan: Option<Chan>) {
        if self.version == 0x0200 {
            for (e, p) in r.fx.iter_mut() {
                if *e == EF_SPEED && *p < 20 {
                    *p = p.saturating_add(1);
                }
            }
            r.volume = if r.volume == 0 { NO_VOLUME } else { (r.volume - 1).min(0x0F) };
            if r.note == 0 {
                r.instrument = NO_INSTRUMENT;
            }
        }
        if ver <= 2 {
            for (e, p) in r.fx.iter_mut() {
                if *e == EF_PORTAOFF {
                    *e = EF_PORTAMENTO;
                    *p = 0;
                } else if *e == EF_PORTAMENTO && *p < 0xFF {
                    *p += 1;
                }
            }
        }
        if ver == 3 {
            for (e, p) in r.fx.iter_mut() {
                if matches!(chan, Some(Chan::Vrc7(_))) && (*e == EF_PORTA_UP || *e == EF_PORTA_DOWN) {
                    *e = if *e == EF_PORTA_UP { EF_PORTA_DOWN } else { EF_PORTA_UP };
                } else if chan == Some(Chan::Fds) && *e == EF_PITCH && *p != 0x80 {
                    *p = 0u8.wrapping_sub(*p);
                }
            }
        }
        if ver <= 5 && chan == Some(Chan::Fds) && (1..=12).contains(&r.note) {
            r.octave = (r.octave + 2).min(7);
        }
        if matches!(chan, Some(Chan::N163(_))) {
            for (e, _) in r.fx.iter_mut() {
                if *e == EF_SAMPLE_OFFSET {
                    *e = EF_N163_WAVE_BUFFER;
                }
            }
        }
        if r.instrument != HOLD_INSTRUMENT && r.instrument > NO_INSTRUMENT {
            r.instrument = NO_INSTRUMENT;
        }
    }

    /// The sequence an instrument uses for a type, if it's enabled.
    pub fn sequence<'a>(&'a self, ins: &'a Instrument, ty: usize) -> Option<&'a Sequence> {
        if ins.kind == InstrumentKind::Fds {
            // The FDS keeps its own volume, arpeggio and pitch sequences.
            return ins.fds_seqs.get(ty).filter(|s| !s.values.is_empty());
        }
        let (en, i) = *ins.seq.get(ty)?;
        if !en {
            return None;
        }
        let kind = match ins.kind {
            InstrumentKind::Vrc6 => InstrumentKind::Vrc6,
            InstrumentKind::N163 => InstrumentKind::N163,
            InstrumentKind::S5b => InstrumentKind::S5b,
            _ => InstrumentKind::Apu,
        };
        self.sequences.get(&(kind, i, ty as u8))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every module in FT_DIR reads.
    #[test]
    #[ignore]
    fn reads_all() {
        let Ok(dir) = std::env::var("FT_DIR") else { return };
        let mut n = 0;
        let mut bad = 0;
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let d = std::fs::read(e.path()).unwrap();
            n += 1;
            match Module::parse(&d) {
                Some(m) => {
                    if std::env::var("FT_SHOW").is_ok() {
                        println!("{:?}: v{:x} exp {:x} ch {} tracks {} ins {} seqs {} samples {}", e.file_name(), m.version, m.expansion, m.channels.len(), m.tracks.len(), m.instruments.len(), m.sequences.len(), m.samples.len());
                    }
                    if m.tracks.iter().any(|t| t.patterns.is_empty()) {
                        println!("{:?}: no patterns", e.file_name());
                    }
                }
                None => {
                    bad += 1;
                    println!("{:?}: not read", e.file_name());
                }
            }
        }
        println!("{} of {n} read", n - bad);
        assert_eq!(bad, 0);
    }
}

#[cfg(test)]
mod show {
    use super::*;

    /// Prints a channel's rows (FT_FILE, FT_CH, FT_FRAMES) and its instruments' sequences.
    #[test]
    #[ignore]
    fn rows() {
        let Ok(f) = std::env::var("FT_FILE") else { return };
        let c: usize = std::env::var("FT_CH").ok().and_then(|s| s.parse().ok()).unwrap_or(3);
        let frames: usize = std::env::var("FT_FRAMES").ok().and_then(|s| s.parse().ok()).unwrap_or(1);
        let m = Module::parse(&std::fs::read(f).unwrap()).unwrap();
        let t = &m.tracks[0];
        println!("speed {} tempo {} rows {} split {:x} vib_new {}", t.speed, t.tempo, t.rows, m.speed_split, m.vibrato_new);
        let mut used = std::collections::BTreeSet::new();
        for fr in 0..frames.min(t.frames.len()) {
            for r in 0..t.rows {
                let row = t.row(fr, c, r);
                if row != Row::default() {
                    println!("{fr:2}:{r:02} n{} o{} i{} v{} fx {:?}", row.note, row.octave, row.instrument, row.volume, row.fx.iter().filter(|f| f.0 != 0).collect::<Vec<_>>());
                    if row.instrument < 64 {
                        used.insert(row.instrument);
                    }
                }
            }
        }
        for i in used {
            if let Some(ins) = m.instruments.get(&i) {
                println!("ins {i} {:?} {:?}", ins.kind, ins.seq);
                for ty in 0..5 {
                    if let Some(s) = m.sequence(ins, ty) {
                        println!("   seq {ty}: {:?}", s);
                    }
                }
            }
        }
    }
}

