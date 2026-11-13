// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! Songs that play by their own replayer: the synth and chip trackers,
//! whose sounds are made as the song plays rather than kept as samples.
//!
//! A format's module here reads its file into a song (a [`Tune`]), whose
//! [`Driver`] plays it a tick at a time and renders each of its channels.
//! [`Machine`] is what a host plays: it times the ticks, seeks, mutes,
//! fades and mixes, the same for every format. [`open`] reads a song back
//! from its file by the format's id (a UMF keeps the file).

use std::fmt;
use std::sync::Arc;

/// A song read from its file, ready to play.
pub trait Tune: Send + Sync {
    /// The format's id ("ahx", "hvl"...), which [`open`] knows it by.
    fn format(&self) -> &'static str;
    /// The file it was read from.
    fn file(&self) -> &[u8];
    /// How many songs the file has (subsongs); at least 1.
    fn songs(&self) -> usize {
        1
    }
    /// A replayer at the start of song `song`.
    fn driver(&self, song: usize) -> Box<dyn Driver>;
    /// How long song `song` plays once through, in seconds, when that's
    /// known without playing it (as [`Machine::length`] would find it).
    fn length(&self, _song: usize) -> Option<f64> {
        None
    }
    /// Its instruments, each whole (see [`Patch`]), for a repository to
    /// keep. None when the format's sounds aren't instruments of their own.
    fn patches(&self) -> Vec<Patch> {
        Vec::new()
    }
}

/// An instrument of a song, whole: everything its format needs to make
/// its sound (the instrument and the tables, waves or samples it plays
/// from), so the same instrument found in two songs is the same bytes.
/// Its name isn't part of it, nor anything the song sets around it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Patch {
    /// Its number in the song, from 1.
    pub slot: u16,
    pub name: String,
    /// The format's own bytes for it: a format's patches are only ever
    /// compared with that format's.
    pub data: Vec<u8>,
    /// The host's samples it plays (0-based), for a song whose samples
    /// the host keeps (Musicline's, ProTrekkr's): `data` names them by
    /// their place here, from 1, never by their number in the song.
    pub samples: Vec<u16>,
}

/// Bytes put together for a [`Patch`]'s data, little-endian.
#[derive(Default)]
pub struct PatchBytes(pub Vec<u8>);

impl PatchBytes {
    pub fn u8(&mut self, v: u8) -> &mut Self {
        self.0.push(v);
        self
    }

    pub fn u16(&mut self, v: u16) -> &mut Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }

    pub fn i32(&mut self, v: i32) -> &mut Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }

    pub fn u32(&mut self, v: u32) -> &mut Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }

    pub fn f32(&mut self, v: f32) -> &mut Self {
        self.0.extend_from_slice(&v.to_bits().to_le_bytes());
        self
    }

    /// A run of bytes, its length first.
    pub fn bytes(&mut self, b: &[u8]) -> &mut Self {
        self.u32(b.len() as u32);
        self.0.extend_from_slice(b);
        self
    }

    /// Signed points (a waveform or sample), their count first.
    pub fn i8s(&mut self, v: &[i8]) -> &mut Self {
        self.u32(v.len() as u32);
        self.0.extend(v.iter().map(|&x| x as u8));
        self
    }

    /// The points `len` long at `at` in a song's memory (as many as there are).
    pub fn mem(&mut self, mem: &[i8], at: usize, len: usize) -> &mut Self {
        let from = at.min(mem.len());
        self.i8s(&mem[from..at.saturating_add(len).min(mem.len())])
    }

    pub fn patch(self, slot: usize, name: &str) -> Patch {
        Patch { slot: slot as u16, name: name.trim().to_string(), data: self.0, samples: Vec::new() }
    }
}

impl fmt::Debug for dyn Tune {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Tune({}, {} bytes)", self.format(), self.file().len())
    }
}

/// Each channel's next frames, left and right, filled by [`Driver::render`].
pub struct Bufs {
    pub left: Vec<Vec<f32>>,
    pub right: Vec<Vec<f32>>,
}

impl Bufs {
    fn new(channels: usize, frames: usize) -> Bufs {
        Bufs { left: vec![vec![0.0; frames]; channels], right: vec![vec![0.0; frames]; channels] }
    }
}

/// A song's replayer: it moves a tick at a time and renders what its
/// channels play between ticks.
pub trait Driver: Send {
    /// The song's channels.
    fn channels(&self) -> usize;
    /// One tick: the song moves on.
    fn tick(&mut self);
    /// How long the tick just played lasts, in seconds.
    fn tick_seconds(&self) -> f64;
    /// The next `frames` frames at `rate`, each channel's into `out`
    /// (cleared first), at the Tracker's level: a channel at full volume
    /// panned to one side peaks at about 0.4.
    fn render(&mut self, rate: u32, frames: usize, out: &mut Bufs);
    /// Where the song is: its order (in the song's own order list) and row.
    fn position(&self) -> (usize, usize);
    /// Ticks a row now, and beats a minute, for the screen.
    fn speed(&self) -> (u32, u32);
    /// Whether the song came round to an order it played before (or
    /// stopped) since this was last asked.
    fn take_done(&mut self) -> bool;
    /// Whether the song still plays (false once it stops itself).
    fn playing(&self) -> bool {
        true
    }
}

/// At most this many ticks are played silently to seek (an hour at 50 a second).
const SEEK_TICKS: usize = 180_000;
/// Frames rendered at a time.
const BLOCK: usize = 256;

/// A song played: its replayer timed, sought, muted, faded and mixed.
pub struct Machine {
    tune: Arc<dyn Tune>,
    song: usize,
    driver: Box<dyn Driver>,
    /// Frames to go before the next tick.
    left: f64,
    /// A seek played the tick it stopped at: render plays it before ticking again.
    pending_first: bool,
    muted: Vec<bool>,
    gains: Vec<f32>,
    levels: Vec<f32>,
    bufs: Bufs,
    /// The fade's level, falling to 0 once fading.
    fade: f32,
    fading: bool,
}

impl Machine {
    pub fn new(tune: Arc<dyn Tune>, song: usize) -> Machine {
        let song = song.min(tune.songs().saturating_sub(1));
        let driver = tune.driver(song);
        let n = driver.channels();
        Machine { tune, song, driver, left: 0.0, pending_first: false, muted: vec![false; n], gains: vec![1.0; n], levels: vec![0.0; n], bufs: Bufs::new(n, BLOCK), fade: 1.0, fading: false }
    }

    pub fn tune(&self) -> &Arc<dyn Tune> {
        &self.tune
    }

    pub fn channels(&self) -> usize {
        self.muted.len()
    }

    /// Plays from an order and row: from the song's start, silently, to
    /// the first tick there (or as near as the song goes).
    pub fn seek(&mut self, order: usize, row: usize) {
        self.driver = self.tune.driver(self.song);
        self.left = 0.0;
        self.pending_first = false;
        self.fade = 1.0;
        self.fading = false;
        if (order, row) == (0, 0) {
            return;
        }
        for _ in 0..SEEK_TICKS {
            self.driver.tick();
            self.driver.take_done();
            if self.driver.position() == (order, row) || !self.driver.playing() {
                break;
            }
        }
        // The tick that reached it plays first.
        self.pending_first = true;
    }

    /// Where the song is: its order and row.
    pub fn position(&self) -> (usize, usize) {
        self.driver.position()
    }

    pub fn playing(&self) -> bool {
        self.driver.playing()
    }

    /// Whether the song came round (or stopped) since this was last asked.
    pub fn take_done(&mut self) -> bool {
        self.driver.take_done()
    }

    /// Ticks a row and beats a minute.
    pub fn speed(&self) -> (u32, u32) {
        self.driver.speed()
    }

    pub fn set_muted(&mut self, channel: usize, muted: bool) {
        if let Some(m) = self.muted.get_mut(channel) {
            *m = muted;
        }
    }

    pub fn set_gain(&mut self, channel: usize, gain: f32) {
        if let Some(g) = self.gains.get_mut(channel) {
            *g = gain;
        }
    }

    /// Each channel's level now (0 to 1), for meters.
    pub fn levels(&self, out: &mut [f32]) {
        for (o, l) in out.iter_mut().zip(&self.levels) {
            *o = l.min(1.0);
        }
    }

    /// Fades the song out over a few milliseconds (a stop).
    pub fn fade_out(&mut self) {
        self.fading = true;
    }

    pub fn faded(&self) -> bool {
        self.fading && self.fade <= 0.0
    }

    /// Adds the next frames, at `rate`, `gain` and stereo separation `sep`
    /// (0 mono to 1 as the song has it), into `left` and `right`.
    pub fn render(&mut self, rate: u32, gain: f32, sep: f32, left: &mut [f32], right: &mut [f32]) {
        let frames = left.len().min(right.len());
        let rate = rate.max(1);
        let fade_step = 1.0 / (0.01 * rate as f32);
        let (same, cross) = ((1.0 + sep) * 0.5, (1.0 - sep) * 0.5);
        let mut at = 0;
        while at < frames {
            if self.left <= 0.0 {
                if !self.pending_first {
                    self.driver.tick();
                }
                self.pending_first = false;
                self.left += self.driver.tick_seconds() * f64::from(rate);
            }
            let n = (self.left.ceil() as usize).clamp(1, (frames - at).min(BLOCK));
            self.driver.render(rate, n, &mut self.bufs);
            for c in 0..self.muted.len() {
                let (bl, br) = (&self.bufs.left[c][..n], &self.bufs.right[c][..n]);
                let mut peak = 0.0f32;
                for i in 0..n {
                    peak = peak.max(bl[i].abs()).max(br[i].abs());
                }
                // A meter's level: the peak, falling back slowly.
                let lv = &mut self.levels[c];
                *lv = (peak * 2.5).max(*lv * 0.97f32.powf(n as f32 / 64.0));
                if self.muted[c] {
                    continue;
                }
                let g = self.gains[c] * gain;
                let mut fade = self.fade;
                for i in 0..n {
                    let (l, r) = (bl[i] * g, br[i] * g);
                    let f = if self.fading { fade = (fade - fade_step).max(0.0); fade } else { 1.0 };
                    left[at + i] += (l * same + r * cross) * f;
                    right[at + i] += (r * same + l * cross) * f;
                }
            }
            if self.fading {
                self.fade = (self.fade - fade_step * n as f32).max(0.0);
            }
            self.left -= n as f64;
            at += n;
        }
    }

    /// How long a song plays once through (to where it comes round or
    /// stops), in seconds, at most `most`.
    pub fn length(tune: &Arc<dyn Tune>, song: usize, most: f64) -> f64 {
        if let Some(t) = tune.length(song) {
            return t.min(most);
        }
        let mut d = tune.driver(song);
        let mut t = 0.0;
        while t < most {
            d.tick();
            t += d.tick_seconds();
            if d.take_done() || !d.playing() {
                break;
            }
        }
        t.min(most)
    }
}

/// Reads a song back from its file by its format's id; None when the id
/// isn't known or the file isn't the format's.
pub fn open(format: &str, file: &[u8]) -> Option<Arc<dyn Tune>> {
    Some(match format {
        "ahx" | "hvl" => Arc::new(crate::ahx::Song::parse(file)?),
        "fc" => Arc::new(crate::fc::Song::parse(file)?),
        "sid1" => Arc::new(crate::sidmon1::Song::parse(file)?),
        "sid2" => Arc::new(crate::sidmon2::Song::parse(file)?),
        "bp" => Arc::new(crate::soundmon::Song::parse(file)?),
        "dm1" => Arc::new(crate::deltamusic1::Song::parse(file)?),
        "dmu" => Arc::new(crate::mugician::Song::parse(file)?),
        "sa" => Arc::new(crate::sonic::Song::parse(file)?),
        "is" => Arc::new(crate::instereo::Song::parse(file)?),
        "ma" => Arc::new(crate::musicassembler::Song::parse(file)?),
        "syn" => Arc::new(crate::synthesis::Song::parse(file)?),
        "vss" => Arc::new(crate::voodoo::Song::parse(file)?),
        "dm2" => Arc::new(crate::deltamusic2::Song::parse(file)?),
        "st" => Arc::new(crate::sawteeth::Song::parse(file)?),
        "mon" => Arc::new(crate::monotone::Song::parse(file)?),
        "kt" => Arc::new(crate::klystrack::Song::parse(file)?),
        "v2m" => Arc::new(crate::v2::Song::parse(file)?),
        "sunvox" => Arc::new(crate::sunvox::Song::parse(file)?),
        "xrns" | "rns" => Arc::new(crate::renoise::Song::parse(file)?),
        "stone" => Arc::new(crate::stonetracker::Song::parse(file)?),
        "prt" => Arc::new(crate::pretracker::Song::parse(file)?),
        "org" => Arc::new(crate::organya::Song::parse(file)?),
        "bsi" => Arc::new(crate::fcbsi::Song::parse(file)?),
        "ptcop" => Arc::new(crate::pxtone::Song::parse(file)?),
        "pt3" => Arc::new(crate::pt3::Song::parse(file)?),
        "gt1" => Arc::new(crate::goattracker1::Song::parse(file)?),
        "gt2" => Arc::new(crate::goattracker::Song::parse(file)?),
        "mgs" => Arc::new(crate::mgs::Song::parse(file)?),
        "mmlgb" => Arc::new(crate::mmlgb::Song::parse(file)?),
        "mds" => Arc::new(crate::mdsdrv::Song::parse(file).ok()?),
        "nsf" => Arc::new(crate::nsf::Song::parse(file).ok()?),
        "arkos" => Arc::new(crate::arkos::Song::parse(file)?),
        "ftm" => Arc::new(crate::famitracker_play::Song::parse(file)?),
        "vt" => Arc::new(crate::victracker::Song::parse(file)?),
        "dmf" => Arc::new(crate::deflemask_play::Song::parse(file)?),
        "pmd" => Arc::new(crate::pc98::Song::parse(crate::pc98::Which::Pmd, file)?),
        "fmp" => Arc::new(crate::pc98::Song::parse(crate::pc98::Which::Fmp, file)?),
        _ => return None,
    })
}
