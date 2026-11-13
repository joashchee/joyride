// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! Future Composer BSI ("FUCO"), Tony Bybell's 1990 editor (no kin of
//! Future Composer 1.3 and 1.4): each of the four channels its own track
//! of 256 bytes (blocks to play, each repeated so many times, transposes,
//! volumes, jumps), stepping at its own pace through 64 shared blocks of
//! 256 bytes (notes, each held its own count of steps, sounds, detunes,
//! glides up or down, the tempo); the sounds 64 sampled "digis", played
//! once or looped.
//!
//! Played as its own replayer, `PLAYER.S` (Future Composer Music Driver
//! v1.02, in the editor's archive on Aminet, mus/edit/FutureComposer),
//! plays it, on the Amiga's vertical blank. Its author's grant, from the
//! archive's readme (1996): "anyone out there is free to do with this
//! what they want and that includes the sourcecode--it's why I included
//! it all." (He asks to be sent a copy of what's made with it.)

use std::sync::Arc;

use crate::paula::{Paula, VBLANK};
use crate::replay::{Bufs, Driver, Tune};

/// The replayer's periods: seven octaves from ProTracker's C-1 an octave
/// down, with its own few roundings (165 for 160, 83 for 80).
const PERTAB: [u16; 84] = [
    1712, 1616, 1520, 1440, 1360, 1280, 1200, 1136, 1072, 1016, 960, 904, 856, 808, 760, 720, 680, 640, 600, 568, 536, 508, 480, 452, 428, 404, 380, 360, 340, 320, 300, 284, 268, 254, 240, 226, 214,
    202, 190, 180, 170, 165, 150, 142, 134, 127, 120, 113, 107, 101, 95, 90, 85, 83, 75, 71, 67, 64, 60, 57, 54, 51, 48, 45, 43, 41, 38, 36, 34, 32, 30, 28, 27, 25, 24, 23, 21, 20, 19, 18, 17, 16,
    15, 14,
];

pub fn period(i: usize) -> u16 {
    PERTAB.get(i).copied().unwrap_or(0)
}

const TRACKS: usize = 4;
const BLOCKS: usize = TRACKS + 0x400;
const DIGBLOK: usize = BLOCKS + 0x4004;
/// Where the digis' points start (the 64th entry's last 12 bytes are
/// read from there too, as the replayer reads them).
const DIGIDAT: usize = DIGBLOK + 0x3F4;

/// A digi: where its points are, how many, and its loop (none: played once).
#[derive(Clone, Copy, Debug, Default)]
pub struct Digi {
    pub name: [u8; 4],
    pub at: u32,
    pub len: u32,
    pub loop_start: u32,
    pub loop_len: u32,
}

#[derive(Clone, Debug)]
pub struct Song {
    pub file: Arc<Vec<u8>>,
    pub digis: Vec<Digi>,
    pub mem: Vec<i8>,
}

fn be32(d: &[u8], i: usize) -> u32 {
    d.get(i..i + 4).map_or(0, |b| u32::from_be_bytes(b.try_into().unwrap_or([0; 4])))
}

pub fn probe(d: &[u8]) -> bool {
    d.len() >= DIGIDAT && &d[..4] == b"FUCO"
}

impl Song {
    pub fn parse(d: &[u8]) -> Option<Song> {
        if !probe(d) {
            return None;
        }
        // Each digi follows the one before, as the replayer lays them out.
        let mut at = DIGIDAT as u32;
        let digis = (0..64)
            .map(|k| {
                let e = DIGBLOK + 16 * k;
                let mut name = [0u8; 4];
                name.copy_from_slice(&d[e..e + 4]);
                let g = Digi {
                    name,
                    at,
                    len: be32(d, e + 4),
                    loop_start: be32(d, e + 8),
                    loop_len: be32(d, e + 12),
                };
                at = at.wrapping_add(g.len);
                g
            })
            .collect();
        let mut mem: Vec<i8> = d.iter().map(|&b| b as i8).collect();
        mem.resize(mem.len() + 8, 0);
        Some(Song {
            file: Arc::new(d.to_vec()),
            digis,
            mem,
        })
    }

    fn track(&self, v: usize, i: u8) -> u8 {
        self.file.get(TRACKS + v * 256 + usize::from(i)).copied().unwrap_or(0)
    }

    fn block(&self, i: u16) -> u8 {
        self.file.get(BLOCKS + usize::from(i)).copied().unwrap_or(0)
    }

    /// Each channel's notes as the song steps through them, until every
    /// channel has come round (or the song ends; at most `most` steps):
    /// (step, note as played, digi, steps held).
    pub fn timeline(&self, most: usize) -> [Vec<(usize, u8, u8, usize)>; 4] {
        let mut p = Player::new(Arc::new(self.clone()));
        let mut out: [Vec<(usize, u8, u8, usize)>; 4] = Default::default();
        for step in 0..most {
            p.notes = Some(Vec::new());
            p.step();
            for (c, note, digi, held) in p.notes.take().unwrap_or_default() {
                out[c].push((step, note, digi, held));
            }
            if p.done || p.ended {
                break;
            }
        }
        out
    }

    /// The steps every note's start and hold is a multiple of (at most 16),
    /// and how many rows the song runs before coming round.
    pub fn rows(&self) -> (usize, usize) {
        fn gcd(a: usize, b: usize) -> usize {
            if b == 0 {
                a
            } else {
                gcd(b, a % b)
            }
        }
        let lines = self.timeline(100_000);
        let grain = lines.iter().flatten().fold(0, |g, &(t, _, _, w)| gcd(gcd(g, t), w)).clamp(1, 16);
        let end = lines.iter().filter_map(|l| l.last().map(|&(t, _, _, w)| t + w)).max().unwrap_or(0);
        (grain, end.div_ceil(grain).max(1))
    }
}

impl Tune for Song {
    fn format(&self) -> &'static str {
        "bsi"
    }
    fn file(&self) -> &[u8] {
        &self.file
    }
    fn driver(&self, _song: usize) -> Box<dyn Driver> {
        let mut p = Player::new(Arc::new(self.clone()));
        p.rows = self.rows();
        Box::new(p)
    }
}

#[derive(Clone, Copy, Default)]
struct Voice {
    dur: u16,
    cdur: i16,
    glide: i16,
    transpose: u16,
    period: u16,
    blk: u16,
    pos: u16,
    detune: u16,
    sound: u16,
    bf: u16,
    rp: u16,
    vol: u16,
    /// Paula's registers as the replayer left them, and the channel's DMA.
    lc: u32,
    len: u16,
    dma: bool,
    /// What the replayer last wrote to its period and volume, and where
    /// its DMA last started (for the test).
    per: u16,
    volume: u16,
    started: (u32, u16),
}

pub struct Player {
    song: Arc<Song>,
    paula: Paula,
    voices: [Voice; 4],
    tempo: i16,
    count: i16,
    infchk: u16,
    /// The song ended (an end mark, or a loop that never plays a note).
    ended: bool,
    /// Steps taken, the channels come round since the song last did, and
    /// the notes started this step (when asked for).
    steps: usize,
    looped: u8,
    done: bool,
    notes: Option<Vec<(usize, u8, u8, usize)>>,
    rows: (usize, usize),
    /// The channels started this tick (for the test).
    started: u8,
}

impl Player {
    fn new(song: Arc<Song>) -> Player {
        let v = Voice {
            dur: 0x10,
            rp: 1,
            bf: 0xFF,
            vol: 64,
            ..Default::default()
        };
        Player {
            paula: Paula::new(4, song.mem.clone()),
            song,
            voices: [v; 4],
            tempo: 2,
            count: 0,
            infchk: 0,
            ended: false,
            steps: 0,
            looped: 0,
            done: false,
            notes: None,
            rows: (1, 1),
            started: 0,
        }
    }

    fn set_regs(&mut self, c: usize, lc: u32, len: u16) {
        let v = &mut self.voices[c];
        v.lc = lc;
        v.len = len;
        if v.dma {
            self.paula.set_loop(c, lc as usize, bytes(len));
        }
    }

    fn dma_off(&mut self, c: usize) {
        self.voices[c].dma = false;
        self.paula.stop(c);
    }

    fn dma_on(&mut self, c: usize) {
        let v = &mut self.voices[c];
        if !v.dma {
            v.dma = true;
            self.started |= 1 << c;
            v.started = (v.lc, v.len);
            self.paula.play(c, v.lc as usize, bytes(v.len));
            self.paula.set_loop(c, v.lc as usize, bytes(v.len));
        }
    }

    fn set_period(&mut self, c: usize, p: u16) {
        self.voices[c].per = p;
        self.paula.set_period(c, u32::from(p));
    }

    fn set_volume(&mut self, c: usize, v: u16) {
        self.voices[c].volume = v;
        self.paula.set_volume(c, u32::from(v));
    }

    fn come_round(&mut self, c: usize) {
        self.looped |= 1 << c;
        if self.looped == 0x0F {
            self.looped = 0;
            self.done = true;
        }
    }

    /// One step of every channel, the last first.
    fn step(&mut self) {
        self.steps += 1;
        for c in (0..4).rev() {
            if self.ended {
                return;
            }
            self.drive(c);
        }
    }

    fn song_end(&mut self) {
        self.ended = true;
        self.done = true;
    }

    fn drive(&mut self, c: usize) {
        self.infchk = 0;
        let song = self.song.clone();
        let v = &mut self.voices[c];
        if v.glide != 0 {
            let d = if v.glide < 0 { v.detune } else { v.detune.wrapping_neg() };
            let p = u32::from(v.period.wrapping_add(d)).clamp(127, 0x800) as u16;
            v.period = p;
            self.set_period(c, p);
        }
        if self.voices[c].bf & 0xFF == 0xFF {
            self.voices[c].bf = 0;
            self.voices[c].rp = self.voices[c].rp.wrapping_sub(1);
            if self.voices[c].rp == 0 && !self.next_block(c, &song) {
                return;
            }
        }
        self.play_block(c, &song);
    }

    /// The track read on to its next block (NXV); false when the song ended.
    fn next_block(&mut self, c: usize, song: &Song) -> bool {
        self.voices[c].rp = 1;
        loop {
            self.infchk += 1;
            if self.infchk & 0x100 != 0 {
                self.song_end();
                return false;
            }
            let v = &mut self.voices[c];
            let d0 = v.pos as u8;
            let d1 = song.track(c, d0);
            let inc = |v: &mut Voice| v.pos = (v.pos & 0xFF00) | u16::from((v.pos as u8).wrapping_add(1));
            match d1 {
                0x00..=0x3F => {
                    inc(v);
                    v.blk = u16::from(d1);
                    return true;
                }
                0x40..=0x7F => {
                    v.rp = u16::from(d1) - 0x3F;
                    inc(v);
                }
                0x80..=0xBF if d1 & 0x3F <= 35 => {
                    v.transpose = u16::from(d1 & 0x3F);
                    inc(v);
                }
                // The LED filter, on or off: not played.
                0x80..=0xFB => inc(v),
                0xFC => {
                    let at = d0.wrapping_add(1);
                    v.pos = u16::from(at);
                    let vol = u16::from(song.track(c, at));
                    v.vol = vol;
                    inc(v);
                    self.set_volume(c, vol);
                }
                0xFD => {
                    let to = song.track(c, d0.wrapping_add(1));
                    if to <= d0 {
                        self.come_round(c);
                    }
                    let v = &mut self.voices[c];
                    v.pos = (v.pos & 0xFF00) | u16::from(to);
                    v.rp = 1;
                }
                0xFE => {
                    self.song_end();
                    return false;
                }
                0xFF => {
                    self.come_round(c);
                    let v = &mut self.voices[c];
                    v.pos = 0;
                    v.rp = 1;
                }
            }
        }
    }

    /// The block played on: its next note, once the last is held its time.
    fn play_block(&mut self, c: usize, song: &Song) {
        let v = &mut self.voices[c];
        v.cdur = v.cdur.wrapping_sub(1);
        if v.cdur > 0 {
            return;
        }
        loop {
            self.infchk += 1;
            if self.infchk & 0x100 != 0 {
                self.song_end();
                return;
            }
            let v = &mut self.voices[c];
            let d0 = v.bf.wrapping_add(v.blk << 8);
            let d1 = song.block(d0);
            let nxbf = |v: &mut Voice| v.bf = (v.bf & 0xFF00) | u16::from((v.bf as u8).wrapping_add(1));
            match d1 {
                0xFF => {
                    v.bf = 0;
                    v.rp = v.rp.wrapping_sub(1);
                    if v.rp == 0 && !self.next_block(c, song) {
                        return;
                    }
                    let v = &mut self.voices[c];
                    v.cdur = v.cdur.wrapping_sub(1);
                    if v.cdur > 0 {
                        return;
                    }
                }
                0x00..=0x2F => {
                    self.note(c, d1, song);
                    return;
                }
                0x30..=0x6F => {
                    match d1 - 0x30 {
                        0x3D => v.glide = -1,
                        0x3E => v.glide = 0,
                        0x3F => v.glide = 1,
                        x => v.detune = u16::from(x) * 2,
                    }
                    nxbf(v);
                }
                0x70..=0x7F => {
                    self.tempo = i16::from(d1 & 0x0F);
                    nxbf(v);
                }
                0x80..=0xBF => {
                    let d = u16::from(d1 & 0x3F);
                    v.dur = if d == 0 { 0x40 } else { d };
                    nxbf(v);
                }
                _ => {
                    v.sound = u16::from(d1 & 0x3F);
                    nxbf(v);
                }
            }
        }
    }

    /// A note: the digi started at its period (PNOT).
    fn note(&mut self, c: usize, d1: u8, song: &Song) {
        let v = self.voices[c];
        let g = song.digis[usize::from(v.sound & 0x3F)];
        let mut played = None;
        if g.len != 0 {
            self.dma_off(c);
            self.set_regs(c, g.at, (g.len >> 1) as u16);
            let p = period(usize::from(u16::from(d1) + v.transpose)).wrapping_add(v.detune);
            self.set_period(c, p);
            self.voices[c].period = p;
            self.dma_on(c);
            if g.loop_len != 0 {
                self.set_regs(c, g.at.wrapping_add(g.loop_start), (g.loop_len >> 1) as u16);
            } else {
                let lc = self.voices[c].lc;
                self.set_regs(c, lc, 1);
            }
            self.set_volume(c, v.vol);
            played = Some(d1 as usize + v.transpose as usize);
        }
        let v = &mut self.voices[c];
        v.cdur = v.dur as i16;
        v.bf = v.bf.wrapping_add(1);
        if let (Some(n), Some(notes)) = (played, self.notes.as_mut()) {
            notes.push((c, n.min(255) as u8, (v.sound & 0x3F) as u8, usize::from(v.dur).max(1)));
        }
    }
}

/// Paula's length register (in words; 0 is 65,536) as bytes.
fn bytes(len: u16) -> usize {
    if len == 0 {
        0x2_0000
    } else {
        usize::from(len) * 2
    }
}

impl Driver for Player {
    fn channels(&self) -> usize {
        4
    }

    fn tick(&mut self) {
        self.started = 0;
        if self.ended {
            for c in 0..4 {
                self.dma_off(c);
            }
            return;
        }
        self.count = self.count.wrapping_sub(1);
        if self.count >= 0 {
            return;
        }
        self.count = self.tempo;
        self.step();
    }

    fn tick_seconds(&self) -> f64 {
        1.0 / VBLANK
    }

    fn render(&mut self, rate: u32, frames: usize, out: &mut Bufs) {
        self.paula.render(rate, frames, out);
    }

    fn position(&self) -> (usize, usize) {
        let (grain, rows) = self.rows;
        let r = (self.steps.saturating_sub(1) / grain) % rows;
        (r / 64, r % 64)
    }

    fn speed(&self) -> (u32, u32) {
        ((self.tempo as u32 + 1) * self.rows.0 as u32, 125)
    }

    fn take_done(&mut self) -> bool {
        std::mem::take(&mut self.done)
    }

    fn playing(&self) -> bool {
        !self.ended
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn periods() {
        assert_eq!(period(0), 1712);
        assert_eq!(period(12), 856);
        assert_eq!(period(83), 14);
    }

    /// Paula's registers each tick, as the original replayer under a 68000
    /// logs them (deli2log): BSI_DIR the songs, BSI_REF the logs.
    #[test]
    #[ignore]
    fn the_same_as_the_original() {
        let (Ok(dir), Ok(rf)) = (std::env::var("BSI_DIR"), std::env::var("BSI_REF")) else { return };
        let mut bad = 0;
        for e in std::fs::read_dir(&dir).unwrap().flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            let Ok(log) = std::fs::read_to_string(format!("{rf}/{name}.log")) else { continue };
            let song = Song::parse(&std::fs::read(e.path()).unwrap()).unwrap();
            let mut p = Player::new(Arc::new(song));
            let mut first = None;
            for (t, line) in log.lines().skip(1).enumerate() {
                p.tick();
                let mut ours = format!("{t}");
                for c in 0..4 {
                    let v = &p.voices[c];
                    ours += &if p.started & (1 << c) != 0 {
                        format!(" | T {:x} {:x}", v.started.0, v.started.1)
                    } else {
                        " | -".into()
                    };
                    ours += &format!(" {:x} {:x} {:x} {:x}{}", v.lc, v.len, v.per, v.volume, if v.dma { "" } else { " off" });
                }
                let theirs = line.replace(" END", "").replace("?0 ", "0 ");
                if ours != theirs && first.is_none() {
                    first = Some(format!("{name} tick {t}\n  ours   {ours}\n  theirs {theirs}"));
                }
            }
            match first {
                Some(f) => {
                    eprintln!("{f}");
                    bad += 1;
                }
                None => eprintln!("{name}: same"),
            }
        }
        assert_eq!(bad, 0);
    }
}
