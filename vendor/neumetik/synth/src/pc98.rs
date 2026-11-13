// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! The PC-98's sound-driver songs: PMD's (.M, .M2, .MZ) and FMP's (.OPI,
//! .OVI, .OZI), each played by its driver (`pmd`, `fmp`) on the OPNA and
//! PPZ8 (`fmwork`). A song keeps the PCM files it asks for beside it (the
//! ADPCM's voices, PPZ8's banks) when they were found: they're packed with
//! it into the file this keeps, so a saved song keeps them.
//!
//! The SSG is played as the OPNA's analog output, as 98fmplayer's own
//! players play it; the rhythm sounds need the chip's ROM, which isn't
//! shipped (they're silent without it).

use std::sync::Arc;

use crate::fmp::{self, Fmp};
use crate::fmwork::{run_tick, Interrupt, Out, Work};
use crate::opna::RATE;
use crate::pmd::{self, Pmd};
use crate::replay::{Bufs, Driver, Patch, PatchBytes, Tune};

/// Which driver a song is for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Which {
    Pmd,
    Fmp,
}

/// A PCM file a song plays: the ADPCM's voices (PMD's .PPC, FMP's .PVI).
pub const PCM_ADPCM: u8 = 0;
/// PPZ8's first bank (.PZI or .PVI).
pub const PCM_PPZ1: u8 = 1;
/// PPZ8's second bank (PMD only).
pub const PCM_PPZ2: u8 = 2;

const MAGIC: &[u8; 8] = b"NMPC98\x00\x01";

/// The channels: six FM, three SSG, the rhythm sounds, ADPCM, then PPZ8's
/// eight when the song has a bank for it.
pub const CHANNEL_NAMES: [&str; 19] = [
    "FM 1", "FM 2", "FM 3", "FM 4", "FM 5", "FM 6", "SSG 1", "SSG 2", "SSG 3", "Rhythm", "ADPCM", "PPZ 1", "PPZ 2", "PPZ 3", "PPZ 4", "PPZ 5", "PPZ 6", "PPZ 7", "PPZ 8",
];
const BASE_CHANNELS: usize = 11;

/// The level a sample reaches the Tracker at: the chip's full scale 1.
const LEVEL: f32 = 1.0 / 32768.0;

/// Ticks a song's timing is followed for at most (as 98fmplayer's).
const MAX_TICKS: u32 = 0xfffff;

#[derive(Clone)]
pub struct Song {
    which: Which,
    file: Vec<u8>,
    song: Vec<u8>,
    pcm: Vec<(u8, Vec<u8>)>,
}

#[derive(Clone)]
enum Drv {
    Pmd(Box<Pmd>),
    Fmp(Box<Fmp>),
}

impl Interrupt for Drv {
    fn interrupt(&mut self, w: &mut Work) {
        match self {
            Drv::Pmd(d) => d.interrupt(w),
            Drv::Fmp(d) => d.interrupt(w),
        }
    }
}

/// A note in a song's timeline, as the screen shows it.
#[derive(Clone, Copy, Debug)]
pub struct Event {
    pub measure: usize,
    /// Its sixteenth of the measure.
    pub row: usize,
    pub channel: usize,
    /// Its MIDI note, None for a rest.
    pub note: Option<u8>,
}

/// A song's timeline, followed without playing it.
pub struct Timeline {
    pub events: Vec<Event>,
    /// Measures played once through.
    pub measures: usize,
    /// The measure it loops back to, when it loops.
    pub restart: Option<usize>,
    /// Seconds once through, and the driver's ticks.
    pub seconds: f64,
    pub ticks: u32,
}

pub fn probe_pmd(d: &[u8]) -> bool {
    Pmd::load(d).is_some()
}

pub fn probe_fmp(d: &[u8]) -> bool {
    Fmp::load(d).is_some()
}

impl Song {
    /// The song `song` for `which`, with the PCM files found for it.
    pub fn new(which: Which, song: &[u8], pcm: Vec<(u8, Vec<u8>)>) -> Option<Song> {
        let ok = match which {
            Which::Pmd => probe_pmd(song),
            Which::Fmp => probe_fmp(song),
        };
        if !ok {
            return None;
        }
        let file = if pcm.is_empty() {
            song.to_vec()
        } else {
            let mut f = MAGIC.to_vec();
            f.extend_from_slice(&(song.len() as u32).to_le_bytes());
            f.extend_from_slice(song);
            f.push(pcm.len() as u8);
            for (kind, data) in &pcm {
                f.push(*kind);
                f.extend_from_slice(&(data.len() as u32).to_le_bytes());
                f.extend_from_slice(data);
            }
            f
        };
        Some(Song {
            which,
            file,
            song: song.to_vec(),
            pcm,
        })
    }

    /// A song back from the file `Tune::file` gave.
    pub fn parse(which: Which, file: &[u8]) -> Option<Song> {
        let Some(rest) = file.strip_prefix(MAGIC.as_slice()) else {
            return Song::new(which, file, Vec::new());
        };
        let mut at = 0usize;
        let mut take = |n: usize| -> Option<&[u8]> {
            let s = rest.get(at..at.checked_add(n)?)?;
            at += n;
            Some(s)
        };
        let u32le = |b: &[u8]| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize;
        let n = u32le(take(4)?);
        let song = take(n)?.to_vec();
        let count = take(1)?[0];
        let mut pcm = Vec::new();
        for _ in 0..count {
            let kind = take(1)?[0];
            let n = u32le(take(4)?);
            pcm.push((kind, take(n)?.to_vec()));
        }
        let mut s = Song::new(which, &song, pcm)?;
        s.file = file.to_vec();
        Some(s)
    }

    /// The song without its PCM.
    pub fn song(&self) -> &[u8] {
        &self.song
    }

    pub fn which(&self) -> Which {
        self.which
    }

    /// Whether it has a bank for PPZ8 (and so its eight channels).
    pub fn has_ppz(&self) -> bool {
        self.pcm.iter().any(|(k, _)| *k == PCM_PPZ1 || *k == PCM_PPZ2)
    }

    pub fn channels(&self) -> usize {
        if self.has_ppz() {
            CHANNEL_NAMES.len()
        } else {
            BASE_CHANNELS
        }
    }

    /// The PCM files the song asks for, as its kind and the name it gives
    /// (without an extension; each kind's are tried in turn).
    pub fn wanted(which: Which, song: &[u8]) -> Vec<(u8, String, &'static [&'static str])> {
        let mut out = Vec::new();
        match which {
            Which::Pmd => {
                let Some(p) = Pmd::load(song) else { return out };
                let [ppc, ppz1, ppz2] = p.pcm_files();
                for (k, n, ext) in [(PCM_ADPCM, ppc, &["PPC"][..]), (PCM_PPZ1, ppz1, &["PVI", "PZI"][..]), (PCM_PPZ2, ppz2, &["PVI", "PZI"][..])] {
                    if let Some(n) = n {
                        out.push((k, n, ext));
                    }
                }
            }
            Which::Fmp => {
                let Some(f) = Fmp::load(song) else { return out };
                if !f.pvi_name.is_empty() {
                    out.push((PCM_ADPCM, f.pvi_name.clone(), &["PVI"][..]));
                }
                if !f.ppz_name.is_empty() {
                    out.push((PCM_PPZ1, f.ppz_name.clone(), &["PVI"][..]));
                }
            }
        }
        out
    }

    fn start(&self, w: &mut Work) -> Drv {
        let mut d = match self.which {
            Which::Pmd => {
                let mut p = Box::new(Pmd::load(&self.song).expect("read when the song was made"));
                p.init(w);
                Drv::Pmd(p)
            }
            Which::Fmp => {
                let mut f = Box::new(Fmp::load(&self.song).expect("read when the song was made"));
                f.init(w);
                Drv::Fmp(f)
            }
        };
        for (kind, data) in &self.pcm {
            match (&mut d, *kind) {
                (Drv::Pmd(p), PCM_ADPCM) => {
                    p.load_ppc(w, data);
                }
                (Drv::Fmp(f), PCM_ADPCM) => {
                    f.load_pvi(w, data);
                }
                (_, PCM_PPZ1 | PCM_PPZ2) => {
                    let b = usize::from(*kind - PCM_PPZ1);
                    if !w.ppz8.load_pvi(b, data) {
                        w.ppz8.load_pzi(b, data);
                    }
                }
                _ => {}
            }
        }
        d
    }

    /// Its notes laid out by measure, followed without the chips; and how
    /// long it plays once through.
    pub fn timeline(&self) -> Timeline {
        let mut w = Work::dummy();
        let mut d = self.start(&mut w);
        let mut events = Vec::new();
        let mut samples = 0u64;
        let mut ticks = 0u32;
        let mut once: Option<(u32, u64, usize)> = None;
        let mut measure_at = Vec::new();
        let first = |d: &Drv| match d {
            Drv::Pmd(p) => {
                let mut s = [0u8; 32];
                for (i, x) in s.iter_mut().enumerate().take(23) {
                    *x = p.note_event(i).0;
                }
                (s, 0)
            }
            Drv::Fmp(f) => {
                let mut s = [0u8; 32];
                for (i, x) in s.iter_mut().enumerate().take(23) {
                    *x = f.note_event(i).0;
                }
                (s, f.rhythm_hit().0)
            }
        };
        let (mut seen, mut rhythm_seen) = first(&d);
        while ticks < MAX_TICKS {
            let (measure, row) = position(&d, ticks);
            if measure_at.len() <= measure {
                measure_at.resize(measure + 1, ticks);
            }
            samples += w.timerb_samples() as u64;
            w.timer.status = 2;
            d.interrupt(&mut w);
            ticks += 1;
            if once.is_none() {
                note_events(&d, measure, row, &mut seen, &mut rhythm_seen, &mut events);
            }
            if once.is_none() && (w.loop_cnt >= 1 || !w.playing) {
                // The tick it stops on isn't counted (as 98fmplayer counts).
                once = Some((if w.playing { ticks } else { ticks - 1 }, samples, measure + usize::from(row > 0)));
                if !w.playing || w.loop_cnt == 0xff {
                    break;
                }
            }
            if w.loop_cnt >= 2 && w.loop_cnt != 0xff {
                break;
            }
        }
        let (t1, s1, measures) = once.unwrap_or((ticks, samples, measure_at.len()));
        let mut restart = None;
        if ticks > t1 && w.loop_cnt != 0xff {
            // Its loop as long as the second time through: back that far from the end.
            let start = t1.saturating_sub(ticks - t1);
            restart = Some(measure_at.iter().rposition(|&t| t <= start).unwrap_or(0));
        }
        Timeline {
            events,
            measures: measures.max(1),
            restart,
            seconds: s1 as f64 / f64::from(RATE),
            ticks: t1,
        }
    }
}

// Where the song is before its tick `ticks`: the measure, and its sixteenth.
fn position(d: &Drv, ticks: u32) -> (usize, usize) {
    match d {
        Drv::Pmd(p) => {
            let (m, t, len) = p.measure();
            let len = usize::from(len.max(1));
            (usize::from(m), (usize::from(t) * 16 / len).min(15))
        }
        Drv::Fmp(f) => {
            let bar = u32::from(f.bar().0.max(1));
            ((ticks / bar) as usize, ((ticks % bar) * 16 / bar) as usize)
        }
    }
}

fn note_events(d: &Drv, measure: usize, row: usize, seen: &mut [u8; 32], rhythm_seen: &mut u8, out: &mut Vec<Event>) {
    let mut push = |channel: usize, note: Option<u8>| out.push(Event { measure, row, channel, note });
    match d {
        Drv::Pmd(p) => {
            let map = (0..6)
                .map(|c| (pmd::FM_1 + c, c, 12))
                .chain((0..3).map(|c| (pmd::SSG_1 + c, 6 + c, 24)))
                .chain([(pmd::ADPCM, 10, 12)])
                .chain((0..8).map(|c| (pmd::PPZ_1 + c, 11 + c, 12)));
            for (part, channel, base) in map {
                let (n, note) = p.note_event(part);
                if n != seen[part] {
                    seen[part] = n;
                    push(channel, (note != 0xff).then(|| ((note >> 4) * 12 + (note & 0xf) + base) as u8));
                }
            }
            let (n, _) = p.note_event(pmd::RHYTHM);
            if n != seen[pmd::RHYTHM] {
                seen[pmd::RHYTHM] = n;
                if p.rhythm_hit() != 0 {
                    push(9, Some(60));
                }
            }
        }
        Drv::Fmp(f) => {
            let map = (0..6)
                .map(|c| (fmp::FM_1 + c, c, 12u8))
                .chain((0..3).map(|c| (fmp::SSG_1 + c, 6 + c, 24)))
                .chain([(fmp::ADPCM, 10, 12)]);
            for (part, channel, base) in map {
                let (n, note, tied) = f.note_event(part);
                if n != seen[part] {
                    seen[part] = n;
                    if note == 0xff {
                        push(channel, None);
                    } else if !tied {
                        push(channel, Some(note.wrapping_add(base)));
                    }
                }
            }
            let (n, last) = f.rhythm_hit();
            if n != *rhythm_seen {
                *rhythm_seen = n;
                if last != 0 {
                    push(9, Some(60));
                }
            }
        }
    }
}

impl Tune for Song {
    fn format(&self) -> &'static str {
        match self.which {
            Which::Pmd => "pmd",
            Which::Fmp => "fmp",
        }
    }
    fn file(&self) -> &[u8] {
        &self.file
    }
    fn driver(&self, _song: usize) -> Box<dyn Driver> {
        Box::new(Player::new(Arc::new(self.clone())))
    }
    fn length(&self, _song: usize) -> Option<f64> {
        Some(self.timeline().seconds)
    }

    /// The song's FM voices (25 bytes each, as the driver writes them to
    /// the chip), each with whether it's for the OPM (PMD only). Its SSG
    /// and PCM sounds come from its PCM files and the driver.
    fn patches(&self) -> Vec<Patch> {
        let voices = match self.which {
            Which::Pmd => Pmd::load(&self.song).map(|p| (p.opm(), p.fm_voices())),
            Which::Fmp => Fmp::load(&self.song).map(|f| (false, f.fm_voices())),
        };
        let Some((opm, voices)) = voices else { return Vec::new() };
        voices
            .into_iter()
            .filter(|(_, v)| v.iter().any(|&x| x != 0))
            .map(|(n, v)| {
                let mut b = PatchBytes::default();
                b.u8(u8::from(opm)).bytes(&v);
                b.patch(usize::from(n) + 1, &format!("@{n}"))
            })
            .collect()
    }
}

pub struct Player {
    w: Work,
    d: Drv,
    bufs: Vec<Vec<[f32; 2]>>,
    prev: Vec<[f32; 2]>,
    len: usize,
    pos: f64,
    ticks: u32,
    loops: u8,
    done: bool,
    shown: (usize, usize),
}

impl Player {
    pub fn new(song: Arc<Song>) -> Player {
        let mut w = Work::new();
        w.opna.ssg.set_ymf288(false);
        let d = song.start(&mut w);
        let n = song.channels();
        Player {
            w,
            d,
            bufs: vec![Vec::new(); n],
            prev: vec![[0.0; 2]; n],
            len: 0,
            pos: 0.0,
            ticks: 0,
            loops: 0,
            done: false,
            shown: (0, 0),
        }
    }
}

fn channel_out(o: &Out, c: usize) -> [i32; 2] {
    match c {
        0..=5 => o.opna.fm[c],
        6..=8 => [o.opna.ssg[c - 6]; 2],
        9 => o.opna.rhythm.iter().fold([0, 0], |a, r| [a[0] + r[0], a[1] + r[1]]),
        10 => o.opna.adpcm,
        _ => o.ppz8[c - 11],
    }
}

impl Driver for Player {
    fn channels(&self) -> usize {
        self.bufs.len()
    }

    fn tick(&mut self) {
        for c in 0..self.bufs.len() {
            if self.len > 0 {
                self.prev[c] = self.bufs[c][self.len - 1];
            }
            self.bufs[c].clear();
        }
        self.pos = (self.pos - self.len as f64).max(0.0);
        self.shown = position(&self.d, self.ticks);
        let bufs = &mut self.bufs;
        self.len = run_tick(&mut self.w, &mut self.d, |o| {
            for (c, b) in bufs.iter_mut().enumerate() {
                let s = channel_out(o, c);
                b.push([s[0] as f32 * LEVEL, s[1] as f32 * LEVEL]);
            }
        });
        self.ticks += 1;
        if self.w.loop_cnt != self.loops {
            self.loops = self.w.loop_cnt;
            self.done = true;
        }
    }

    fn tick_seconds(&self) -> f64 {
        self.len as f64 / f64::from(RATE)
    }

    fn render(&mut self, rate: u32, frames: usize, out: &mut Bufs) {
        let step = f64::from(RATE) / f64::from(rate.max(1));
        for c in 0..self.bufs.len() {
            let (b, prev) = (&self.bufs[c], self.prev[c]);
            let last = b.last().copied().unwrap_or(prev);
            let at = |k: isize| if k < 0 { prev } else { b.get(k as usize).copied().unwrap_or(last) };
            let (l, r) = (&mut out.left[c][..frames], &mut out.right[c][..frames]);
            for i in 0..frames {
                let x = self.pos + i as f64 * step - 1.0;
                let k = x.floor();
                let f = (x - k) as f32;
                let (s0, s1) = (at(k as isize), at(k as isize + 1));
                l[i] = s0[0] + (s1[0] - s0[0]) * f;
                r[i] = s0[1] + (s1[1] - s0[1]) * f;
            }
        }
        self.pos += frames as f64 * step;
    }

    fn position(&self) -> (usize, usize) {
        self.shown
    }

    fn speed(&self) -> (u32, u32) {
        match &self.d {
            Drv::Pmd(p) => {
                let (_, _, len) = p.measure();
                ((u32::from(len) / 16).max(1), u32::from(p.tempo()).max(1))
            }
            Drv::Fmp(f) => {
                let (bar, timerb) = f.bar();
                let tps = f64::from(RATE) / (f64::from(256 - u32::from(timerb)) * 16.0);
                let beat = (f64::from(bar) / 4.0).max(1.0);
                ((u32::from(bar) / 16).max(1), (60.0 * tps / beat).round().clamp(1.0, 999.0) as u32)
            }
        }
    }

    fn take_done(&mut self) -> bool {
        std::mem::take(&mut self.done)
    }

    fn playing(&self) -> bool {
        self.w.playing
    }
}
