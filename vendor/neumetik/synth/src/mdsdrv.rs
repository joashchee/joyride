// Copyright 2026 ansiapps. Neumetik Synth, proprietary: see LICENSE.
//
// A port of MDSDRV (github.com/superctr/MDSDRV), the Mega Drive sound driver, by Ian Karlsson:
//
// Copyright (c) 2019-2022 Ian Karlsson
//
// This software is provided 'as-is', without any express or implied
// warranty. In no event will the authors be held liable for any damages
// arising from the use of this software.
//
// Permission is granted to anyone to use this software for any purpose,
// including commercial applications, and to alter it and redistribute
// it freely, subject to the following restrictions:
//
// 1. The origin of this software must not be misrepresented; you must
//    not claim that you wrote the original software. If you use this
//    software in a product, an acknowledgment in the product
//    documentation would be appreciated but is not required.
// 2. Altered source versions must be plainly marked as such, and must
//    not be misrepresented as being the original software.
// 3. This notice may not be removed or altered from any source
//    distribution.
//
// This is an altered version: the driver's 68000 code (`src/mdsdrv.68k`) rewritten in Rust, its writes to
// the YM2612 and the SN76489 going to our emulations of them (`ymfm.rs`, `sn76489.rs`).

//! MDSDRV: sound data (`.mds`) played the way the driver plays it, frame by frame.
//!
//! `Song::parse` reads an `.mds` (RIFF `MDS0`: the sequence, its data blocks, its PCM) and lays it out as
//! the ROM image the driver expects (`mdslink`'s job): the sound data header, the request table, the sequence
//! with the data blocks' offsets written into its song data table. `Driver` is `mds_update` and what it
//! calls: requests and priorities, the tempo counter, the sequence commands, macro tables, the FM and PSG
//! voices (the Z80's bus waits left out, the register writes in the driver's order), pitch envelopes,
//! portamento. The track's memory is kept as the driver has it (bytes at the driver's offsets), because
//! macro tables write to a track variable by its offset.
//!
//! The PCM channels are the Z80 mixer's behaviour (not its cycles): two channels at 17.8 kHz or three at 13.4 kHz, the
//! driver's volume table, the rate steps, mixed with the Z80's clamp into the YM2612's DAC.
//!
//! **Not done:** the command API's DMA protection and the PCM buffer. **Compared with the original** 68000 driver
//! (`m68k/mds_ref.rs`, assembled and run by our own 68000 core): every FM and PSG write and the work memory, on
//! songs and effects and on requests and commands. **Not compared:** the Z80's PCM mixer (a Z80 isn't run).

use crate::replay::{Bufs, Driver as ReplayDriver, Tune};
use crate::sn76489::Sn76489;
use crate::ymfm::Ym2612;
pub const CHANNEL_NAMES: [&str; 10] = ["FM 1", "FM 2", "FM 3", "FM 4", "FM 5", "FM 6", "PSG 1", "PSG 2", "PSG 3", "Noise"];

const RCOUNT: usize = 4;
const TCOUNT: usize = 16;

// Track memory (`mddef`'s `t_*`, as byte offsets).
const T_NOTE_FLAG: usize = 0;
const T_CHANNEL_FLAG: usize = 1;
const T_BASE_ADDR: usize = 2; // a long: the top byte is the operator mask
const T_POSITION: usize = 6;
const T_STACK_POS: usize = 8;
const T_COUNTER: usize = 9;
const T_REST_TIME: usize = 10;
const T_NOTE_TIME: usize = 11;
const T_CHANNEL_ID: usize = 12;
const T_REQUEST_ID: usize = 13;
const T_INS: usize = 14;
const T_INS_TRS: usize = 15;
const T_NOTE: usize = 16;
const T_DTN: usize = 17;
const T_PITCH: usize = 18;
const T_LAST_PITCH: usize = 20;
const T_TRS: usize = 22;
const T_PTA: usize = 23;
const T_VOL: usize = 24;
const T_MTAB_REPEAT: usize = 25;
const T_MTAB_ADDR: usize = 26;
const T_MTAB_DELAY: usize = 28;
const T_MTAB_POS: usize = 29;
const T_PEG_ADDR: usize = 30;
const T_PEG_MOD: usize = 32;
const T_PEG_DELAY: usize = 34;
const T_PEG_POS: usize = 35;
const T_STACK: usize = 36;
const T_TYPE: usize = 52;
pub const TSIZE: usize = 58;
// FM: pan/lfo, algorithm, four TLs. PSG: envelope address (word), position, delay, noise reset, noise mode.
const T_FM_PAN_LFO: usize = T_TYPE;
const T_FM_ALG: usize = T_TYPE + 1;
const T_FM_TL: usize = T_TYPE + 2;
const T_PSG_EG_ADDR: usize = T_TYPE;
const T_PSG_EG_POS: usize = T_TYPE + 2;
const T_PSG_EG_DELAY: usize = T_TYPE + 3;
const T_PSG_NRESET: usize = T_TYPE + 4;
const T_PSG_NMODE: usize = T_TYPE + 5;

// Note flags (the high byte of a track's flag word) and channel flags.
const NF_INS: u8 = 0;
const NF_VOL: u8 = 1;
const NF_FM3: u8 = 2;
const NF_PAN_LFO: u8 = 3;
const NF_SLUR: u8 = 4;
const NF_KEY_OFF: u8 = 5;
const NF_KEY_ON: u8 = 6;
const NF_ENABLED: u8 = 7;
const NF_SUSTAIN: u8 = NF_INS;
const NF_PCM_HEADER: u8 = NF_INS;
const NF_PCM_PITCH: u8 = NF_FM3;
const PE_PCM1: u8 = 7;
const PE_PCM2: u8 = 6;
const PE_PCM3: u8 = 5;
const T_PCM_PITCH: usize = T_TYPE + 1;
const T_PCM_HEADER: usize = T_TYPE + 2;
const T_PCM_LENGTH: usize = T_TYPE + 4;
const CF_DRUM_MODE: u8 = 0;
const CF_MTAB_CARRY: u8 = 1;
const CF_PCM_CONTROL: u8 = 2;
const CF_STOP: u8 = 3;
const CF_SUSPEND: u8 = 4;
const CF_BACKGROUND: u8 = 5;
const CF_KEY_ON: u8 = 7;
const NM_INIT: u8 = (1 << NF_INS) | (1 << NF_PAN_LFO) | (1 << NF_ENABLED);
const NM_RESTORE: u8 = (1 << NF_KEY_OFF) | (1 << NF_INS) | (1 << NF_VOL) | (1 << NF_PAN_LFO) | (1 << NF_FM3);

const NOTE_START: u8 = 0x82;
const CHN_CMD_BASE: u8 = 0xE0;

#[derive(Clone, Copy)]
struct Fl {
    nf: u8,
    cf: u8,
}

impl Fl {
    fn nf_set(&mut self, b: u8) {
        self.nf |= 1 << b;
    }
    /// `bclr`: clears, true when it was set.
    fn nf_clr(&mut self, b: u8) -> bool {
        let was = self.nf & (1 << b) != 0;
        self.nf &= !(1 << b);
        was
    }
    fn nf_test(&self, b: u8) -> bool {
        self.nf & (1 << b) != 0
    }
    fn cf_set(&mut self, b: u8) {
        self.cf |= 1 << b;
    }
    fn cf_clr(&mut self, b: u8) -> bool {
        let was = self.cf & (1 << b) != 0;
        self.cf &= !(1 << b);
        was
    }
    fn cf_test(&self, b: u8) -> bool {
        self.cf & (1 << b) != 0
    }
    fn enabled(&self) -> bool {
        self.nf & 0x80 != 0
    }
}

// The note and octave tables (256 bytes, the octave table following the note table).
fn note_table(i: usize) -> u8 {
    if i < 120 {
        ((i % 12) * 2) as u8
    } else if i < 120 + 132 {
        (((i - 120) / 12) * 8) as u8
    } else if i < 256 {
        88
    } else {
        0
    }
}

fn octave_table(note: usize) -> u8 {
    note_table(120 + note)
}

const FM_FREQ: [u16; 45] = [
    161, 171, 181, 191, 203, 215, 228, 241, 255, 271, 287, 304, 322, 341, 361, 383, 406, 430, 455, 482, 511, 541, 574, 608, 644, 682, 723, 766, 811, 859, 910, 965, 1022, 1083, 1147,
    1215, 1288, 1364, 1445, 1531, 1622, 1719, 1821, 1929, 2044,
];
const PSG_FREQ: [u16; 13] = [1710, 1614, 1524, 1438, 1357, 1281, 1209, 1141, 1077, 1017, 960, 906, 855];
const FM_OP_TABLE: [u8; 8] = [3, 3, 3, 3, 2, 1, 1, 0];
const FM_VOL_TABLE: [u8; 16] = [42, 40, 37, 34, 32, 29, 26, 24, 21, 18, 16, 13, 10, 8, 5, 2];

fn psg_vol_table(i: usize) -> u8 {
    // (count, value) runs, 128 entries.
    const RUNS: [(usize, u8); 18] = [
        (5, 0x8f),
        (3, 0x8e),
        (2, 0x8d),
        (3, 0x8c),
        (3, 0x8b),
        (2, 0x8a),
        (3, 0x89),
        (3, 0x88),
        (2, 0x87),
        (3, 0x86),
        (3, 0x85),
        (2, 0x84),
        (3, 0x83),
        (3, 0x82),
        (2, 0x81),
        (22, 0x81),
        (64, 0x80),
        (0, 0),
    ];
    let mut at = 0;
    for (n, v) in RUNS {
        at += n;
        if i < at {
            return v;
        }
    }
    0x80
}

/// `mds_convert_vol`: the converted volume of a song volume (0 loudest to 15).
fn convert_vol(v: u8) -> u8 {
    0x8fu8.wrapping_sub(psg_vol_table(usize::from(v)))
}

/// Where the driver's file reader found a problem.
#[derive(Debug)]
pub struct Error(pub String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A song laid out for the driver.
pub struct Song {
    /// The sound data: the header (8 bytes: magic, version, count), then everything offsets are counted from.
    rom: Vec<u8>,
    /// The `.mds` it was read from.
    file: Vec<u8>,
    /// The PCM sample data.
    pub pcm: std::sync::Arc<Vec<u8>>,
}

fn le32(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(at..at + 4)?.try_into().ok()?))
}

impl Song {
    pub fn parse(file: &[u8]) -> Result<Song, Error> {
        if file.get(..4) != Some(b"RIFF") || file.get(8..12) != Some(b"MDS0") {
            return Err(Error("not an MDS file".into()));
        }
        let mut seq: Option<&[u8]> = None;
        let mut blocks: Vec<(u32, &[u8])> = Vec::new();
        let mut pcmh: Vec<(u32, &[u8])> = Vec::new();
        let mut pcm: &[u8] = &[];
        let mut major = 0;
        let mut at = 12;
        while at + 8 <= file.len() {
            let id = &file[at..at + 4];
            let size = le32(file, at + 4).ok_or_else(|| Error("cut short".into()))? as usize;
            let body = file.get(at + 8..at + 8 + size).ok_or_else(|| Error("a chunk runs past the end".into()))?;
            match id {
                b"ver " => major = body.first().copied().unwrap_or(0),
                b"seq " => seq = Some(body),
                b"pcmd" => pcm = body,
                b"LIST" if body.get(..4) == Some(b"dblk") => {
                    let mut j = 4;
                    while j + 8 <= body.len() {
                        let sid = &body[j..j + 4];
                        let ssize = le32(body, j + 4).unwrap_or(0) as usize;
                        let sbody = body.get(j + 8..j + 8 + ssize).ok_or_else(|| Error("a data block runs past its list".into()))?;
                        match sid {
                            b"glob" if sbody.len() >= 4 => blocks.push((le32(sbody, 0).unwrap_or(0), &sbody[4..])),
                            b"pcmh" if sbody.len() >= 4 => pcmh.push((le32(sbody, 0).unwrap_or(0), &sbody[4..])),
                            _ => {}
                        }
                        j += 8 + ssize + (ssize & 1);
                    }
                }
                _ => {}
            }
            at += 8 + size + (size & 1);
        }
        let seq = seq.ok_or_else(|| Error("no sequence in it".into()))?;
        if major > 1 {
            return Err(Error(format!("sound data version {major} is newer than the driver's")));
        }
        if seq.len() < 4 {
            return Err(Error("the sequence is too short".into()));
        }
        // The ROM image: header, the request table (PCM table, sequence 1), the sequence, the data blocks.
        let mut rom: Vec<u8> = vec![0x10, 0x01, 0x1f, 0x00, 0x00, 0x06, 0x00, 0x01];
        // sdtop is 8; offsets from it.
        let seq_off = 8usize;
        rom.extend([0u8; 8]);
        let seq_abs = rom.len();
        rom.extend_from_slice(seq);
        if rom.len() % 2 == 1 {
            rom.push(0);
        }
        let tbase = seq_abs.wrapping_add(i16::from_be_bytes([seq[0], seq[1]]) as isize as usize);
        let place = |rom: &mut Vec<u8>, id: u32, data: &[u8], ext: bool| -> Result<(), Error> {
            let off = rom.len() - 8;
            if off >= 0x8000 {
                return Err(Error("the sound data is too big for the driver's 16-bit offsets".into()));
            }
            rom.extend_from_slice(data);
            if rom.len() % 2 == 1 {
                rom.push(0);
            }
            let entry = tbase + (id & 0x7fff_ffff) as usize * 2;
            if entry + 2 > rom.len() {
                return Err(Error("a data block names a place outside the song data table".into()));
            }
            let v = (off as u16) | if ext { 0x8000 } else { 0 };
            rom[entry..entry + 2].copy_from_slice(&v.to_be_bytes());
            Ok(())
        };
        for (id, data) in &blocks {
            place(&mut rom, *id & 0x7fff_ffff, data, *id & 0x8000_0000 != 0)?;
        }
        // PCM headers: eight bytes each (rate, address, length); the sample data is outside the image.
        for (id, h) in &pcmh {
            let position = le32(h, 0).unwrap_or(0);
            let start = le32(h, 4).unwrap_or(0);
            let size = le32(h, 8).unwrap_or(0);
            // The rate byte (1 to 8, in steps of about 2190 Hz) from the sample's own rate.
            let rate = (f64::from(le32(h, 20).unwrap_or(0)) / 2190.0).round().clamp(1.0, 8.0) as u8;
            // Rate, the address's high byte and low word, the length (a long).
            let mut h8 = vec![rate, ((position + start) >> 16) as u8];
            h8.extend(((position + start) as u16).to_be_bytes());
            h8.extend(size.to_be_bytes());
            place(&mut rom, *id, &h8, false)?;
        }
        // The request table's entries: 0 for the PCM table, then the sequence.
        rom[8..12].copy_from_slice(&0u32.to_be_bytes());
        rom[12..16].copy_from_slice(&((seq_off) as u32).to_be_bytes());
        // (offsets are from sdtop = 8: the sequence starts at rom[16], offset 8.)
        Ok(Song { rom, file: file.to_vec(), pcm: std::sync::Arc::new(pcm.to_vec()) })
    }

    /// The ROM image the driver reads (the sequence data table and what it points to).
    pub fn rom(&self) -> &[u8] {
        &self.rom
    }

    pub fn file_bytes(&self) -> &[u8] {
        &self.file
    }
}

fn be16(b: &[u8], at: usize) -> u16 {
    u16::from_be_bytes([b[at], b[at + 1]])
}

/// The driver's memory and the chips it writes.
pub struct Driver {
    rom: Vec<u8>,
    sdtop: usize,
    request: [u16; RCOUNT],
    tempo: [u16; RCOUNT],
    counter: [u16; RCOUNT],
    seq_step: [u16; RCOUNT],
    volume: [u16; RCOUNT],
    tmask: [u16; RCOUNT],
    chmask: [u16; RCOUNT],
    bgm_volume: u8,
    se_volume: u8,
    priority: u8,
    fade_rate: u8,
    fade_target: u8,
    comm: u8,
    pcm_mode: u8,
    pub pcm: [PcmCh; 3],
    pcm_data: std::sync::Arc<Vec<u8>>,
    dac_on: bool,
    fm3_mask: u8,
    fm3_alg: u8,
    fm3_tl: [u8; 4],
    gtempo: u16,
    track: Vec<[u8; TSIZE]>,
    pub ym: Ym2612,
    pub sn: Sn76489,
    /// Ticks (frames) played, and sequence steps of the music slot.
    pub frames: u64,
    pub steps: u64,
    /// Notes started, for a timeline: (step from 1, channel, MIDI note or None for a rest).
    pub heard: Option<Vec<(u64, usize, Option<u8>)>>,
    looped: u16,
    done: bool,
    /// Every write to the chips, in order, when a test asks: (0 or 1 the YM2612's port, 2 the PSG; register; value).
    pub chip_log: Option<Vec<(u8, u8, u8)>>,
}

/// One of the Z80's PCM channels.
#[derive(Clone, Default)]
pub struct PcmCh {
    /// The volume register: 15 to 30 (the table 0 to 15), bit 7 set for a key off.
    vol: u8,
    pitch: u8,
    /// The sample's address in the PCM data, and its length in bytes.
    addr: u32,
    len: u32,
    playing: bool,
    /// Source bytes played so far.
    pos: f64,
}

/// The Z80 driver's volume multipliers.
const PCM_VOL: [i32; 16] = [256, 203, 161, 128, 102, 81, 64, 51, 40, 32, 26, 20, 16, 13, 10, 0];
/// The Z80's mixing rates: a sample every 201 (two channels) or 268 (three) cycles of 3.579545 MHz.
const PCM_CLOCK: f64 = 3_579_545.0;

/// The NTSC frame, in seconds.
/// The track mask's bit of a track: track 0 is bit 15, as in the driver's work area.
fn tbit(tn: usize) -> u16 {
    1 << (TCOUNT - 1 - tn)
}

/// `mds_fade_rate`: the fade's rate by speed (bit pattern rotated once a frame; a set top bit steps the volume).
const FADE_RATES: [u8; 8] = [0x01, 0x11, 0x49, 0x55, 0x57, 0x77, 0x7f, 0xff];

pub const FRAME: f64 = 1.0 / 59.922_74;

impl Driver {
    /// `mds_init`, then the request for song 1 as music (priority 3).
    pub fn new(song: &Song) -> Driver {
        let mut d = Driver {
            rom: song.rom.clone(),
            sdtop: 8,
            request: [0; RCOUNT],
            tempo: [0; RCOUNT],
            counter: [0; RCOUNT],
            seq_step: [0; RCOUNT],
            volume: [0; RCOUNT],
            tmask: [0; RCOUNT],
            chmask: [0; RCOUNT],
            bgm_volume: 0,
            se_volume: 0,
            priority: 0,
            fade_rate: 0,
            fade_target: 0,
            comm: 0,
            pcm_mode: 2,
            pcm: Default::default(),
            pcm_data: song.pcm.clone(),
            dac_on: false,
            fm3_mask: 0,
            fm3_alg: 0,
            fm3_tl: [0; 4],
            gtempo: 128,
            track: vec![[0; TSIZE]; TCOUNT],
            ym: Ym2612::default(),
            sn: Sn76489::new(3_579_545.0),
            frames: 0,
            steps: 0,
            heard: None,
            looped: 0,
            done: false,
            chip_log: None,
        };
        for t in d.track.iter_mut() {
            t[T_NOTE_FLAG] = 0;
            t[T_CHANNEL_FLAG] = 0;
            t[T_PEG_ADDR..T_PEG_ADDR + 2].copy_from_slice(&0u16.to_be_bytes());
            t[T_PSG_EG_ADDR..T_PSG_EG_ADDR + 2].copy_from_slice(&0xfff8u16.to_be_bytes());
            t[T_PSG_EG_POS] = 0xff;
            t[T_PSG_EG_DELAY] = 0x0f;
            t[T_REQUEST_ID] = (RCOUNT << 1) as u8;
        }
        // mds_halt: the PSG muted, the FM chip reset.
        for v in [0x9f, 0xbf, 0xdf, 0xff] {
            d.sn.write(v);
        }
        d.request_song(1, 3);
        d
    }

    /// The tracks' memory, one slice of `TSIZE` bytes each, laid out as the driver's work area has it.
    pub fn tracks(&self) -> &[[u8; TSIZE]] {
        &self.track
    }

    /// The work area before the tracks, as the driver's own layout has it (after `w_sdtop`), for comparing with the original's memory.
    pub fn work_image(&self) -> Vec<u8> {
        let mut v = vec![];
        for tab in [&self.request, &self.tempo, &self.counter, &self.seq_step, &self.volume, &self.tmask, &self.chmask] {
            for w in tab.iter() {
                v.extend(w.to_be_bytes());
            }
        }
        v.extend([self.bgm_volume, self.se_volume, self.priority, self.fade_rate, self.fade_target, self.comm]);
        v
    }

    /// `mds_request`.
    pub fn request_song(&mut self, id: u16, priority: usize) {
        self.request[priority & 3] = id | 0xC000;
    }

    /// `mds_command`: the commands of `doc/api.md`, by number, with `d1` and `d2` as the driver takes them. Returns `d0` (a command with no value returns 0;
    /// the version string, 0x03, isn't here). Slots (`d1` of the per-slot commands) wrap to 0 to 3 as the request's do. The FM writes go to the chip at once.
    pub fn command(&mut self, cmd: u8, d1: u32, d2: u32) -> u32 {
        let slot = (d1 & 3) as usize;
        match cmd {
            0x00 => 0x13,
            0x01 => u32::from(self.rd16(self.sdtop as isize - 2)),
            0x02 => u32::from(self.tmask[slot]),
            0x04 => u32::from(self.gtempo),
            0x05 => {
                self.gtempo = d1 as u16;
                0
            }
            0x06 => u32::from(self.bgm_volume) << 8 | u32::from(self.se_volume),
            0x07 => {
                self.bgm_volume = (d1 >> 8) as u8;
                self.se_volume = d1 as u8;
                0
            }
            0x08 | 0x09 => {
                self.fm(usize::from(cmd == 0x09), (d1 >> 8) as u8, d1 as u8);
                0
            }
            0x0a => {
                // `fade_bgm`: bit 7 of the target stops the song at the end of the fade; the rate is the upper byte of `d1`.
                self.pcm_mode &= !0x10;
                if d1 & 0x80 != 0 {
                    self.pcm_mode |= 0x10;
                }
                // (the driver indexes the table by the whole upper byte: past 7 it reads beyond it; the last entry stands in)
                self.fade_rate = FADE_RATES[((d1 >> 8) as usize & 0xff).min(7)];
                self.fade_target = (d1 & 0x7f) as u8;
                0
            }
            0x0b => {
                let tm = self.tmask[slot];
                for tn in 0..TCOUNT {
                    if tm & tbit(tn) == 0 {
                        continue;
                    }
                    if d2 & 0xffff != 0 {
                        if self.track[tn][T_NOTE_FLAG] & (1 << NF_ENABLED) != 0 {
                            self.track[tn][T_CHANNEL_FLAG] |= (1 << CF_SUSPEND) | (1 << CF_STOP);
                        }
                    } else if self.track[tn][T_CHANNEL_FLAG] & (1 << CF_SUSPEND) != 0 {
                        self.track[tn][T_CHANNEL_FLAG] &= !(1 << CF_SUSPEND);
                        self.track[tn][T_NOTE_FLAG] |= NM_INIT | NM_RESTORE;
                    }
                }
                0
            }
            0x0c => u32::from(self.vol_hi(slot * 2)),
            0x0d => {
                let v = d2 as u8;
                self.volume[slot] = u16::from(v) << 8 | u16::from(convert_vol(v));
                let tm = self.tmask[slot];
                for tn in 0..TCOUNT {
                    if tm & tbit(tn) != 0 {
                        self.track[tn][T_NOTE_FLAG] |= 1 << NF_VOL;
                    }
                }
                0
            }
            0x0e => u32::from(self.tempo[slot]),
            0x0f => {
                self.tempo[slot] = d2 as u16;
                0
            }
            0x10 => u32::from(self.comm),
            0x11 => {
                // The mixing mode only (2 or 3; 0 keeps it): the DMA buffering in `d2` isn't modelled.
                let m = d1 as u8;
                if m == 2 || m == 3 {
                    self.set_pcm_mode(m);
                }
                0
            }
            0x12 => u32::from(self.pcm_mode & 3),
            _ => 0,
        }
    }

    fn fm(&mut self, port: usize, reg: u8, v: u8) {
        if let Some(l) = &mut self.chip_log {
            l.push((port as u8, reg, v));
        }
        self.ym.write(port != 0, reg, v);
    }

    fn psg(&mut self, v: u8) {
        if let Some(l) = &mut self.chip_log {
            l.push((2, 0, v));
        }
        self.sn.write(v);
    }

    fn rd8(&self, at: isize) -> u8 {
        self.rom.get(at as usize).copied().unwrap_or(0)
    }

    fn rd16(&self, at: isize) -> u16 {
        u16::from(self.rd8(at)) << 8 | u16::from(self.rd8(at + 1))
    }

    fn tb(&self, tn: usize) -> isize {
        (u32::from_be_bytes(self.track[tn][T_BASE_ADDR..T_BASE_ADDR + 4].try_into().unwrap()) & 0x00ff_ffff) as isize
    }

    fn op_mask(&self, tn: usize) -> u8 {
        self.track[tn][T_BASE_ADDR]
    }

    fn tw(&self, tn: usize, o: usize) -> u16 {
        be16(&self.track[tn], o)
    }

    fn set_tw(&mut self, tn: usize, o: usize, v: u16) {
        self.track[tn][o..o + 2].copy_from_slice(&v.to_be_bytes());
    }

    fn vol_hi(&self, rn: usize) -> u8 {
        self.volume.get(rn / 2).map_or(0, |v| (v >> 8) as u8)
    }

    fn vol_lo(&self, rn: usize) -> u8 {
        self.volume.get(rn / 2).map_or(0, |v| *v as u8)
    }

    /// One frame: `mds_update`.
    pub fn update(&mut self) {
        self.frames += 1;
        for r in (0..RCOUNT).rev() {
            if self.request[r] & 0x8000 != 0 {
                self.handle_request(r);
            }
            let tm = self.tmask[r];
            if tm == 0 {
                continue;
            }
            let mut counter = self.counter[r].wrapping_add(self.tempo[r]).wrapping_add(1);
            let mut step = 0u16;
            while counter >= self.gtempo {
                counter -= self.gtempo;
                step += 1;
            }
            self.seq_step[r] = step;
            self.counter[r] = counter;
            if r == 3 {
                self.steps += u64::from(step);
            }
        }
        if self.priority != 0 && self.priority & 0x80 != 0 {
            self.update_priority();
            self.update_fade();
        } else if self.priority != 0 || self.fade_rate != 0 {
            self.update_fade();
        }
        for tn in 0..TCOUNT {
            let mut f = Fl { nf: self.track[tn][T_NOTE_FLAG], cf: self.track[tn][T_CHANNEL_FLAG] };
            if !f.enabled() {
                continue;
            }
            let rn = usize::from(self.track[tn][T_REQUEST_ID]);
            let steps = self.seq_step.get(rn / 2).copied().unwrap_or(0);
            for _ in 0..steps {
                self.update_seq(tn, rn / 2, &mut f);
            }
            let mut chn = self.track[tn][T_CHANNEL_ID];
            if f.cf_test(CF_BACKGROUND) {
                // Only a stop reaches a track that plays in the background.
                if f.cf_clr(CF_STOP) {
                    f.nf_clr(NF_ENABLED);
                }
            } else {
                if f.cf_test(CF_STOP) {
                    f.nf |= (1 << NF_FM3) | (1 << NF_KEY_OFF);
                    f.nf &= !(1 << NF_ENABLED);
                    f.cf &= !(1 << CF_STOP);
                }
                chn = chn.min(15);
                self.voice_update(tn, rn, chn, &mut f);
            }
            self.track[tn][T_NOTE_FLAG] = f.nf;
            self.track[tn][T_CHANNEL_FLAG] = f.cf;
        }
        // A round: every track that plays has come round (or has finished).
        let active: u16 = (0..TCOUNT).filter(|&t| self.track[t][T_NOTE_FLAG] & 0x80 != 0).fold(0, |m, t| m | 1 << t);
        if self.tmask[3] != 0 && active != 0 && self.looped & active == active {
            self.looped = 0;
            self.done = true;
        } else if self.tmask[3] == 0 && self.frames > 1 {
            self.done = true;
        }
    }

    /// Whether the music came round (or finished) since this was last asked.
    pub fn take_round(&mut self) -> bool {
        std::mem::take(&mut self.done)
    }

    /// Whether anything of the music still plays.
    pub fn playing(&self) -> bool {
        // (a request takes a frame to stop what was there, and one to start)
        self.tmask[3] != 0 || self.frames < 3
    }

    /// The music's tempo, beats a minute (24 steps a beat).
    pub fn bpm(&self) -> u32 {
        // Steps a second: 60 frames, each (tempo + 1) / gtempo of a step; 24 steps a beat.
        let sps = 59.92 * f64::from(self.tempo[3] + 1) / f64::from(self.gtempo.max(1));
        (sps * 60.0 / 24.0).round() as u32
    }

    // ---- requests ----

    fn handle_request(&mut self, r: usize) {
        self.priority = 0xff;
        let mut req = self.request[r];
        req &= !(1 << 14);
        // A stop request.
        if self.request[r] & (1 << 14) != 0 {
            self.request[r] = req;
            self.stop_song(r);
            return;
        }
        req &= !(1 << 15);
        self.request[r] = req;
        if req == 0 {
            return;
        }
        let maxid = self.rd16(self.sdtop as isize - 2);
        // BGM fade-in: `$2000 + (speed << 10)` added to the sound number (bits 10-13 are the rate's index into `FADE_RATES`, 8 to 15).
        let mut fade_in = false;
        if req > maxid {
            let rate = req & 0x3c00;
            if rate == 0 {
                return;
            }
            req ^= rate;
            if req == 0 || req > maxid {
                return;
            }
            // An index below 8 reads the driver's code before the rate table: not modelled, the request is turned away.
            let Some(&fr) = usize::from(rate >> 10).checked_sub(8).and_then(|i| FADE_RATES.get(i)) else {
                return;
            };
            self.fade_rate = fr;
            // Only the music (slot 3) starts silent and fades; another slot keeps its rate and starts normally, as the driver's overflow flag decides.
            fade_in = r == 3;
        }
        // The song's header.
        self.tempo[r] = self.gtempo - 1;
        self.counter[r] = 0;
        self.seq_step[r] = 0;
        self.tmask[r] = 0;
        let ent = self.sdtop as isize + 4 * req as isize;
        let off = u32::from(self.rd16(ent)) << 16 | u32::from(self.rd16(ent + 2));
        let mut p = self.sdtop as isize + off as isize;
        let start = p;
        let tbase = start + i16::from_be_bytes([self.rd8(p), self.rd8(p + 1)]) as isize;
        p += 2;
        // Initial volume: the music's or the effects'.
        let base = if r == 3 { self.bgm_volume } else { self.se_volume };
        let mut v = base.wrapping_add(self.rd8(p));
        p += 1;
        if v & 0x80 != 0 {
            v = 0x7f;
        }
        if fade_in {
            // The fade's target is the song's volume; it starts at $50 (the quietest steps) and rises, never stopping the song afterwards.
            self.fade_target = v;
            self.pcm_mode &= !0x10;
            self.volume[r] = 0x500f;
        } else {
            self.volume[r] = u16::from(v) << 8 | u16::from(convert_vol(v));
        }
        let tcount = self.rd8(p);
        p += 1;
        if tcount == 0 {
            return;
        }
        let mut left = i32::from(tcount);
        let mut tn = 0;
        while left > 0 && tn < TCOUNT {
            let busy = self.track[tn][T_NOTE_FLAG] & (1 << NF_ENABLED) != 0 || self.track[tn][T_CHANNEL_FLAG] & (1 << CF_SUSPEND) != 0;
            if busy {
                tn += 1;
                continue;
            }
            let ch = self.rd8(p);
            let cflag = self.rd8(p + 1);
            let pos = self.rd16(p + 2);
            p += 4;
            let t = &mut self.track[tn];
            t[T_BASE_ADDR..T_BASE_ADDR + 4].copy_from_slice(&(tbase as u32).to_be_bytes());
            t[T_CHANNEL_ID] = ch;
            t[T_REQUEST_ID] = (r * 2) as u8;
            t[T_NOTE_FLAG] = NM_INIT;
            t[T_CHANNEL_FLAG] = cflag;
            self.tmask[r] |= tbit(tn);
            self.chmask[r] |= 1u16.checked_shl(u32::from(ch)).unwrap_or(0);
            let t = &mut self.track[tn];
            t[T_LAST_PITCH] = 0xff; // `st`: the high byte only; the low byte keeps what it had
            t[T_INS] = 0x00;
            t[T_INS_TRS] = 0x30;
            t[T_NOTE] = 0;
            t[T_DTN] = 0;
            t[T_TRS] = 0;
            t[T_PTA] = 0;
            t[T_VOL] = 0x8f;
            t[T_MTAB_REPEAT] = 0;
            t[T_MTAB_ADDR] = 0;
            t[T_MTAB_ADDR + 1] = 0;
            t[T_PEG_ADDR] = 0;
            t[T_PEG_ADDR + 1] = 0;
            t[T_PEG_MOD] = 0;
            t[T_PEG_MOD + 1] = 0;
            t[T_POSITION..T_POSITION + 2].copy_from_slice(&pos.to_be_bytes());
            t[T_STACK_POS] = 0;
            t[T_COUNTER] = 0;
            t[T_REST_TIME] = 0x0b;
            t[T_NOTE_TIME] = 0x0b;
            if ch >= 6 {
                t[T_PSG_NRESET] = 0;
                t[T_PSG_NMODE] = 0;
                t[T_PSG_EG_ADDR..T_PSG_EG_ADDR + 2].copy_from_slice(&0xfff8u16.to_be_bytes());
                t[T_PSG_EG_POS] = 0xff;
                t[T_PSG_EG_DELAY] = 0x0f;
            } else {
                t[T_FM_PAN_LFO] = 0xc0;
                t[T_FM_ALG] = 0;
                t[T_FM_TL..T_FM_TL + 4].copy_from_slice(&[0x7f; 4]);
            }
            left -= 1;
            tn += 1;
        }
    }

    fn stop_song(&mut self, r: usize) {
        let tm = self.tmask[r];
        for tn in 0..TCOUNT {
            if tm & tbit(tn) != 0 {
                self.track[tn][T_CHANNEL_FLAG] |= 1 << CF_STOP;
                self.track[tn][T_CHANNEL_FLAG] &= !(1 << CF_SUSPEND);
                self.track[tn][T_REQUEST_ID] = (RCOUNT << 1) as u8;
            }
        }
        self.tmask[r] = 0;
        self.chmask[r] = 0;
        if r == 3 {
            self.fade_target = 0;
            self.fade_rate = 0;
            self.pcm_mode &= !0x10;
        }
    }

    fn update_priority(&mut self) {
        self.priority = 0;
        for tn in 0..TCOUNT {
            let mut f = Fl { nf: self.track[tn][T_NOTE_FLAG], cf: self.track[tn][T_CHANNEL_FLAG] };
            // Only a track that is enabled or suspended.
            if !(f.cf_test(CF_SUSPEND) || f.enabled()) {
                continue;
            }
            let bit = 1u16.checked_shl(u32::from(self.track[tn][T_CHANNEL_ID])).unwrap_or(0);
            let mut req = self.track[tn][T_REQUEST_ID];
            // A request of a higher priority (a lower slot) using the channel takes it.
            let mut has_priority = None;
            for k in 0..RCOUNT - 1 {
                if req == 0 {
                    has_priority = Some(true);
                    break;
                }
                if self.chmask[k] & bit != 0 {
                    has_priority = Some(false);
                    break;
                }
                req = req.wrapping_sub(2);
            }
            let has_priority = has_priority.unwrap_or_else(|| req == 0 || self.chmask[RCOUNT - 1] & bit == 0);
            if has_priority {
                if f.cf_clr(CF_BACKGROUND) {
                    // Restored from the background: a pending key on is dropped for a drum, or when
                    // less than a sixteenth is left of the note.
                    if f.cf_test(CF_DRUM_MODE) || self.track[tn][T_COUNTER] < 5 {
                        f.nf &= !(1 << NF_KEY_ON);
                    }
                    self.track[tn][T_NOTE_FLAG] = f.nf;
                    self.track[tn][T_CHANNEL_FLAG] = f.cf;
                }
            } else {
                self.track[tn][T_LAST_PITCH] = 0xff;
                f.nf |= NM_RESTORE;
                f.cf |= 1 << CF_BACKGROUND;
                self.track[tn][T_NOTE_FLAG] = f.nf;
                self.track[tn][T_CHANNEL_FLAG] = f.cf;
            }
        }
    }

    fn update_fade(&mut self) {
        self.fade_rate = self.fade_rate.rotate_right(1);
        if self.fade_rate & 0x80 == 0 {
            return;
        }
        let vol = self.vol_hi(6);
        let target = self.fade_target;
        let delta = if target < vol { 0xffu8 } else { 1u8 };
        let mut v = vol.wrapping_add(delta);
        if (v.wrapping_sub(target) as i8 ^ delta as i8) >= 0 {
            self.fade_rate = 0;
            v = target;
            if self.pcm_mode & 0x10 != 0 {
                self.request[3] = 0xC000;
            }
        }
        self.volume[3] = u16::from(v) << 8 | u16::from(convert_vol(v));
        let tm = self.tmask[3];
        for tn in 0..TCOUNT {
            if tm & tbit(tn) != 0 {
                self.track[tn][T_NOTE_FLAG] |= 1 << NF_VOL;
            }
        }
    }

    // ---- the sequence ----

    fn note_event(&mut self, tn: usize, note: Option<u8>) {
        let ch = usize::from(self.track[tn][T_CHANNEL_ID]);
        if ch >= CHANNEL_NAMES.len() {
            return;
        }
        let at = self.steps.max(1);
        let trs = self.track[tn][T_TRS] as i8;
        // FM note 0 is C0 (16.35 Hz, MIDI 12), PSG note 0 is C2 (65.4 Hz, MIDI 36).
        let base = if ch < 6 { 12 } else { 36 };
        if let Some(h) = &mut self.heard {
            h.push((at, ch, note.map(|n| (i32::from(n) + base + i32::from(trs)).clamp(0, 127) as u8)));
        }
    }

    fn update_seq(&mut self, tn: usize, rn: usize, f: &mut Fl) {
        let c = self.track[tn][T_COUNTER];
        self.track[tn][T_COUNTER] = c.wrapping_sub(1);
        if c != 0 {
            let m = self.tw(tn, T_MTAB_ADDR);
            if m != 0 {
                self.update_mtab(tn, f, m);
            }
            return;
        }
        let tbase = self.tb(tn);
        let mut trackpos = i32::from(self.tw(tn, T_POSITION));
        let mut sp = i32::from(self.track[tn][T_STACK_POS]);
        let mut cmdlen = 1i32;
        let rd = |s: &Driver, pos: i32| -> u8 { s.rd8(tbase + (pos as i16) as isize) };
        // Saves the position and ends the command read.
        macro_rules! finish {
            ($s:ident) => {{
                $s.set_tw(tn, T_POSITION, trackpos as u16);
                $s.track[tn][T_STACK_POS] = sp as u8;
                let m = $s.tw(tn, T_MTAB_ADDR);
                if m != 0 {
                    $s.update_mtab(tn, f, m);
                }
                return;
            }};
        }
        loop {
            trackpos = i32::from((trackpos + cmdlen) as u16);
            let cmd = rd(self, trackpos - 1);
            if cmd < 0x80 {
                // A rest.
                self.track[tn][T_REST_TIME] = cmd;
                self.track[tn][T_COUNTER] = cmd;
                self.note_event(tn, None);
                f.nf_set(NF_KEY_OFF);
                f.nf_clr(NF_KEY_ON);
                finish!(self);
            }
            let mut cmd8 = cmd;
            if cmd >= CHN_CMD_BASE {
                // A command.
                let n = usize::from(cmd & 0x1f);
                const LEN: [u8; 32] = [1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 3, 3, 3, 2, 2, 2, 0, 0, 3, 3, 0, 2, 2, 1, 2, 2, 3, 1, 0];
                cmdlen = i32::from(LEN[n]);
                let arg = |s: &Driver| s.rd8(tbase + (trackpos as i16) as isize);
                match cmd {
                    0xe0 => f.nf_set(NF_SLUR),
                    0xe1 => {
                        // ins
                        let a = arg(self);
                        self.track[tn][T_INS] = a;
                        f.nf_set(NF_KEY_OFF);
                        f.cf_clr(CF_PCM_CONTROL);
                        let ptr = self.rd16(tbase + isize::from(a) * 2);
                        if self.track[tn][T_CHANNEL_ID] < 6 {
                            let ins = self.sdtop + usize::from(ptr) + 24;
                            let mut at = ins;
                            if self.track[tn][T_CHANNEL_ID] == 2 {
                                for k in 0..4 {
                                    self.fm3_tl[k] = self.rd8(at as isize + k as isize);
                                }
                                self.fm3_alg = self.rd8(at as isize + 4);
                            }
                            for k in 0..4 {
                                self.track[tn][T_FM_TL + k] = self.rd8(at as isize + k as isize);
                            }
                            at += 4;
                            self.track[tn][T_FM_ALG] = self.rd8(at as isize);
                            self.track[tn][T_INS_TRS] = self.rd8(at as isize + 1);
                            f.nf_set(NF_INS);
                            self.track[tn][T_LAST_PITCH] = 0xff; // `st`: the high byte only
                        } else {
                            f.nf_set(NF_KEY_OFF);
                            f.nf_clr(NF_SLUR);
                            self.set_tw(tn, T_PSG_EG_ADDR, ptr);
                            self.track[tn][T_PSG_EG_POS] = 0xff;
                            self.track[tn][T_PSG_EG_DELAY] = 0x0f;
                        }
                    }
                    0xe2 => {
                        self.track[tn][T_VOL] = arg(self);
                        f.nf_set(NF_VOL);
                    }
                    0xe3 => {
                        let a = arg(self);
                        self.track[tn][T_VOL] = self.track[tn][T_VOL].wrapping_add(a);
                        f.nf_set(NF_VOL);
                    }
                    0xe4 => self.track[tn][T_TRS] = arg(self),
                    0xe5 => {
                        let a = arg(self);
                        self.track[tn][T_TRS] = self.track[tn][T_TRS].wrapping_add(a);
                    }
                    0xe6 => self.track[tn][T_DTN] = arg(self),
                    0xe7 => self.track[tn][T_PTA] = arg(self),
                    0xe8 => {
                        // peg
                        self.set_tw(tn, T_PEG_MOD, 0);
                        self.set_tw(tn, T_PEG_DELAY, 0);
                        let a = arg(self);
                        if a == 0 {
                            self.set_tw(tn, T_PEG_ADDR, 0);
                        } else {
                            let v = self.rd16(tbase + isize::from(a) * 2 - 2);
                            self.set_tw(tn, T_PEG_ADDR, v);
                        }
                    }
                    0xe9 => {
                        if self.track[tn][T_CHANNEL_ID] < 6 {
                            let v = (self.track[tn][T_FM_PAN_LFO] & 0x3f) | arg(self);
                            self.track[tn][T_FM_PAN_LFO] = v;
                            f.nf_set(NF_PAN_LFO);
                        }
                    }
                    0xea => {
                        if self.track[tn][T_CHANNEL_ID] < 6 {
                            let v = (self.track[tn][T_FM_PAN_LFO] & 0xc0) | arg(self);
                            self.track[tn][T_FM_PAN_LFO] = v;
                            f.nf_set(NF_PAN_LFO);
                        } else {
                            self.track[tn][T_LAST_PITCH] = 0xff; // `st`: the high byte only
                            self.track[tn][T_PSG_NMODE] = arg(self);
                            f.nf_set(NF_PAN_LFO);
                        }
                    }
                    0xeb => {
                        self.set_tw(tn, T_MTAB_DELAY, 0);
                        let a = arg(self);
                        if a == 0 {
                            self.set_tw(tn, T_MTAB_ADDR, 0);
                            f.cf_clr(CF_MTAB_CARRY);
                        } else {
                            let v = self.rd16(tbase + (isize::from(a) - 1) * 2);
                            self.set_tw(tn, T_MTAB_ADDR, v);
                            f.cf_set(CF_MTAB_CARRY);
                        }
                    }
                    0xec => {
                        self.track[tn][T_LAST_PITCH] = 0xff; // `st`: the high byte only
                        let a = arg(self);
                        if a & 0x80 != 0 {
                            f.nf |= (1 << NF_FM3) | (1 << NF_VOL);
                            self.track[tn][T_BASE_ADDR] = (a << 3) ^ 0x80;
                        } else if a & 8 != 0 {
                            f.cf |= 1 << (a & 7);
                        } else {
                            f.cf &= !(1 << (a & 7));
                        }
                    }
                    0xed => {
                        if !f.cf_test(CF_BACKGROUND) {
                            let ch = usize::from(self.track[tn][T_CHANNEL_ID]).min(5);
                            let slot = [0u8, 1, 2, 0, 1, 2][ch];
                            let reg = slot.wrapping_add(arg(self));
                            let v = self.rd8(tbase + (trackpos as i16) as isize + 1);
                            self.fm(ch / 3, reg, v);
                        }
                    }
                    0xee | 0xef => {
                        let a = arg(self) as i8;
                        let v = self.rd8(tbase + (trackpos as i16) as isize + 1);
                        if a >= 0 {
                            f.nf_set(NF_VOL);
                        }
                        let at = T_FM_TL as isize + isize::from(a);
                        if at >= 0 && (at as usize) < TSIZE {
                            let at = at as usize;
                            if cmd == 0xee {
                                self.track[tn][at] = v;
                            } else {
                                let s = self.track[tn][at].wrapping_add(v);
                                self.track[tn][at] = if s & 0x80 != 0 { 0x7f } else { s };
                            }
                        }
                    }
                    0xf0 => {
                        // pcm: the sample's header read (rate, length), and the channel under its control.
                        let a = arg(self);
                        let ptr = self.rd16(tbase + isize::from(a) * 2);
                        self.set_tw(tn, T_PCM_HEADER, ptr);
                        let h = (self.sdtop + usize::from(ptr)) as isize;
                        self.track[tn][T_PCM_PITCH] = self.rd8(h);
                        let len = self.rd16(h + 6);
                        self.set_tw(tn, T_PCM_LENGTH, len);
                        f.cf_set(CF_PCM_CONTROL);
                        f.nf_set(NF_PCM_HEADER);
                        f.nf_set(NF_PCM_PITCH);
                    }
                    0xf1 => {
                        self.track[tn][T_PCM_PITCH] = arg(self);
                        f.nf_set(NF_PCM_PITCH);
                    }
                    0xf2 => {
                        let m = arg(self);
                        self.set_pcm_mode(m);
                    }
                    0xf5 => {
                        let hi = i32::from(self.rd8(tbase + (trackpos as i16) as isize));
                        let lo = i32::from(self.rd8(tbase + (trackpos as i16) as isize + 1));
                        let w = ((hi << 8) | lo) as u16;
                        if w & 0x8000 != 0 {
                            self.looped |= 1 << tn;
                        }
                        cmdlen += i32::from(w as i16);
                    }
                    0xf6 => {
                        let r = arg(self);
                        let v = self.rd8(tbase + (trackpos as i16) as isize + 1);
                        // Both ports' registers: a register in 0x100 and up is the second port's (the driver writes port 0).
                        self.fm(0, r, v);
                    }
                    0xf7 => {
                        // dmfinish
                        self.track[tn][T_NOTE] = arg(self);
                        let n = self.track[tn][T_NOTE];
                        self.note_event(tn, Some(n));
                        sp -= 2;
                        trackpos = i32::from(be16(&self.track[tn], T_STACK + sp as usize));
                        // Continue as after a note: read its length.
                        let next = rd(self, trackpos);
                        self.note_length(tn, next, &mut trackpos, sp);
                        let m = self.tw(tn, T_MTAB_ADDR);
                        if m != 0 {
                            self.update_mtab(tn, f, m);
                        }
                        return;
                    }
                    0xf8 => self.comm = arg(self),
                    0xf9 => self.tempo[rn] = u16::from(arg(self)),
                    0xfa => {
                        self.track[tn][T_STACK + sp as usize] = 0xff;
                        self.set_tw(tn, T_STACK + sp as usize + 2, trackpos as u16);
                        sp += 4;
                    }
                    0xfb => {
                        sp -= 4;
                        let mut t = self.track[tn][T_STACK + sp as usize];
                        if t == 0xff {
                            t = arg(self);
                        }
                        t = t.wrapping_sub(1);
                        if t != 0 {
                            self.track[tn][T_STACK + sp as usize] = t;
                            trackpos = i32::from(be16(&self.track[tn], T_STACK + sp as usize + 2));
                            cmdlen = 1;
                            sp += 4;
                            if t == 0xff {
                                self.looped |= 1 << tn;
                            }
                        }
                    }
                    0xfc => {
                        if self.track[tn][T_STACK + sp as usize - 4] == 1 {
                            sp -= 4;
                            cmdlen += i32::from(arg(self));
                        }
                    }
                    0xfd | 0xf3 | 0xf4 if cmd == 0xfd => {
                        if self.track[tn][T_STACK + sp as usize - 4] == 1 {
                            sp -= 4;
                            let hi = i32::from(self.rd8(tbase + (trackpos as i16) as isize));
                            let lo = i32::from(self.rd8(tbase + (trackpos as i16) as isize + 1));
                            cmdlen += (hi << 8) | lo;
                        }
                    }
                    0xfe => {
                        trackpos += 2;
                        self.set_tw(tn, T_STACK + sp as usize, trackpos as u16);
                        sp += 2;
                        let a = rd(self, trackpos - 2);
                        trackpos = i32::from(self.rd16(tbase + isize::from(a) * 2));
                    }
                    0xff => {
                        if sp != 0 {
                            sp -= 2;
                            trackpos = i32::from(be16(&self.track[tn], T_STACK + sp as usize));
                        } else {
                            // finish
                            self.priority = 0xff;
                            f.cf_set(CF_STOP);
                            self.track[tn][T_REQUEST_ID] = (RCOUNT << 1) as u8;
                            self.tmask[rn] &= !tbit(tn);
                            let ch = self.track[tn][T_CHANNEL_ID];
                            self.chmask[rn] &= !(1u16.checked_shl(u32::from(ch)).unwrap_or(0));
                            self.looped |= 1 << tn;
                            finish!(self);
                        }
                    }
                    _ => {}
                }
                continue;
            }
            cmd8 = cmd8.wrapping_sub(NOTE_START);
            if cmd < NOTE_START {
                if cmd == 0x81 {
                    // A tie.
                } else {
                    // 0x80: rest, the last rest's length.
                    self.track[tn][T_COUNTER] = self.track[tn][T_REST_TIME];
                    self.note_event(tn, None);
                    f.nf_set(NF_KEY_OFF);
                    f.nf_clr(NF_KEY_ON);
                    finish!(self);
                }
            } else {
                // A note.
                self.track[tn][T_NOTE] = cmd8;
                f.nf_set(NF_KEY_ON);
                if !f.nf_test(NF_SLUR) {
                    if f.cf_test(CF_MTAB_CARRY) {
                        self.set_tw(tn, T_MTAB_DELAY, 0);
                    }
                    f.nf_set(NF_KEY_OFF);
                    if f.cf_test(CF_DRUM_MODE) {
                        // A drum: its subroutine runs first.
                        self.set_tw(tn, T_STACK + sp as usize, trackpos as u16);
                        sp += 2;
                        let sub = self.rd16(tbase + isize::from(cmd8) * 2);
                        trackpos = i32::from(sub);
                        cmdlen = 1;
                        continue;
                    }
                }
                let n = self.track[tn][T_NOTE];
                self.note_event(tn, Some(n));
            }
            // The note's length (or the last one).
            let next = rd(self, trackpos);
            self.note_length(tn, next, &mut trackpos, sp);
            let m = self.tw(tn, T_MTAB_ADDR);
            if m != 0 {
                self.update_mtab(tn, f, m);
            }
            return;
        }
    }

    fn note_length(&mut self, tn: usize, next: u8, trackpos: &mut i32, sp: i32) {
        if next & 0x80 != 0 {
            self.track[tn][T_COUNTER] = self.track[tn][T_NOTE_TIME];
        } else {
            self.track[tn][T_COUNTER] = next;
            self.track[tn][T_NOTE_TIME] = next;
            *trackpos += 1;
        }
        self.set_tw(tn, T_POSITION, *trackpos as u16);
        self.track[tn][T_STACK_POS] = sp as u8;
    }

    // ---- macro tables ----

    fn update_mtab(&mut self, tn: usize, f: &mut Fl, addr: u16) {
        let d = self.track[tn][T_MTAB_DELAY];
        self.track[tn][T_MTAB_DELAY] = d.wrapping_sub(1);
        if d != 0 {
            return;
        }
        let tbase = self.tb(tn) + isize::from(addr as i16);
        let mut pos = i32::from(self.track[tn][T_MTAB_POS]) * 2;
        loop {
            pos += 2;
            let param = self.rd8(tbase + pos as isize - 1);
            let cmd = self.rd8(tbase + pos as isize - 2);
            if cmd < 0x80 {
                let mut v = param;
                let mut o = usize::from(cmd);
                if cmd >= 0x40 {
                    o -= 0x40;
                    let cur = self.track[tn].get(o).copied().unwrap_or(0);
                    if o >= T_FM_TL {
                        let s = cur.wrapping_add(v);
                        v = if s & 0x80 != 0 { 0x7f } else { s };
                    } else {
                        v = cur.wrapping_add(v);
                    }
                }
                if o < TSIZE {
                    self.track[tn][o] = v;
                }
                if o >= T_VOL {
                    f.nf_set(NF_VOL);
                }
                continue;
            }
            if cmd >= 0xc0 {
                let ch = usize::from(self.track[tn][T_CHANNEL_ID]).min(5);
                let slot = [0u8, 1, 2, 0, 1, 2][ch];
                self.fm(ch / 3, (cmd << 2).wrapping_add(slot), param);
                continue;
            }
            let jump = |pos: &mut i32, back: i32| {
                *pos += back * 2;
            };
            match cmd {
                0x80 => {
                    if param != 0 {
                        jump(&mut pos, i32::from(param as i8));
                    } else {
                        // End: wait for ever at this node.
                        let at = (pos >> 1) as u8;
                        self.track[tn][T_MTAB_DELAY] = 0xff;
                        self.track[tn][T_MTAB_POS] = at.wrapping_sub(1);
                        return;
                    }
                }
                0x81 | 0x82 => {
                    if cmd == 0x82 {
                        f.nf |= (1 << NF_KEY_ON) | (1 << NF_KEY_OFF);
                    }
                    self.track[tn][T_MTAB_DELAY] = param;
                    self.track[tn][T_MTAB_POS] = (pos >> 1) as u8;
                    return;
                }
                0x83 => {
                    f.cf_clr(CF_MTAB_CARRY);
                }
                0x84 => self.track[tn][T_MTAB_REPEAT] = param,
                0x85 => {
                    if self.track[tn][T_MTAB_REPEAT] == 0 {
                        jump(&mut pos, i32::from(param));
                    }
                }
                0x86 => {
                    let r = self.track[tn][T_MTAB_REPEAT];
                    self.track[tn][T_MTAB_REPEAT] = r.wrapping_sub(1);
                    if r != 0 {
                        jump(&mut pos, i32::from(param as i8));
                    }
                }
                0x87 => {
                    if self.track[tn][T_CHANNEL_ID] < 6 {
                        self.track[tn][T_FM_PAN_LFO] = (self.track[tn][T_FM_PAN_LFO] & 0x3f) | param;
                        f.nf_set(NF_PAN_LFO);
                    }
                }
                0x88 => {
                    if self.track[tn][T_CHANNEL_ID] < 6 {
                        self.track[tn][T_FM_PAN_LFO] = (self.track[tn][T_FM_PAN_LFO] & 0xc0) | param;
                        f.nf_set(NF_PAN_LFO);
                    }
                }
                0x89 => {
                    if self.track[tn][T_CHANNEL_ID] >= 6 {
                        self.track[tn][T_LAST_PITCH] = 0xff; // `st`: the high byte only
                        self.track[tn][T_PSG_NMODE] = param;
                        f.nf_set(NF_PAN_LFO);
                    }
                }
                0x8a | 0x8b => {
                    let d = self.track[tn][T_DTN] as i8;
                    let (r, o) = if cmd == 0x8a { d.overflowing_add(param as i8) } else { d.overflowing_sub(param as i8) };
                    self.track[tn][T_DTN] = r as u8;
                    if o {
                        self.track[tn][T_NOTE] = if cmd == 0x8a { self.track[tn][T_NOTE].wrapping_add(1) } else { self.track[tn][T_NOTE].wrapping_sub(1) };
                    }
                }
                _ => {}
            }
        }
    }

    // ---- pitch ----

    /// `mds_pitch_update`: the pitch (note in the high byte).
    fn pitch_update(&mut self, tn: usize, f: &Fl) -> u16 {
        let t = &self.track[tn];
        let mut d1 = u16::from(t[T_NOTE]) << 8;
        d1 = d1.wrapping_add(u16::from(t[T_TRS]) << 8 | u16::from(t[T_PTA])) & 0xff00;
        let mut d0 = (t[T_DTN] as i8 as i16 as u16).wrapping_add(d1);
        let d2 = d0.wrapping_sub(self.tw(tn, T_PITCH));
        if d2 != 0 {
            let pta = i16::from(self.track[tn][T_PTA]);
            if pta != 0 {
                let mut s = i16::from((d2 >> 8) as u8 as i8);
                if s < 0 {
                    s -= 2;
                }
                s += 1;
                let prod = (i32::from(pta) * i32::from(s)) as i16;
                let new = (prod >> 1).wrapping_add(self.tw(tn, T_PITCH) as i16) as u16;
                let over = ((d0.wrapping_sub(new)) ^ d2) & 0x8000 != 0;
                if !over {
                    d0 = new;
                }
            }
            self.set_tw(tn, T_PITCH, d0);
        }
        let peg = self.tw(tn, T_PEG_ADDR);
        if peg == 0 {
            return d0;
        }
        let key_on = f.nf_test(NF_KEY_ON);
        if peg & 0x8000 == 0 {
            if key_on {
                self.set_tw(tn, T_PEG_DELAY, 0);
            }
            let pos = u16::from(self.track[tn][T_PEG_POS]) * 4;
            let at = (self.sdtop + usize::from(peg) + usize::from(pos) + 2) as isize;
            let mut modv = self.tw(tn, T_PEG_MOD);
            let delay = self.track[tn][T_PEG_DELAY];
            if delay == 0 {
                modv = self.rd16(at - 2);
            }
            let delta = i16::from(self.rd8(at) as i8);
            let m = modv.wrapping_add(delta as u16);
            self.set_tw(tn, T_PEG_MOD, m);
            d0 = d0.wrapping_add(m);
            if delay == self.rd8(at + 1) {
                // `subi.w #$7f00` then `blt`: a signed compare of the word, so only $7f00 to $7fff jump.
                let w = self.rd16(at + 2) as i16;
                if w >= 0x7f00 {
                    self.set_tw(tn, T_PEG_DELAY, (w - 0x7f00) as u16);
                } else {
                    self.track[tn][T_PEG_DELAY] = 0;
                    self.track[tn][T_PEG_POS] = self.track[tn][T_PEG_POS].wrapping_add(1);
                }
            } else {
                let d = delay.wrapping_add(2);
                if d != 0 {
                    self.track[tn][T_PEG_DELAY] = d.wrapping_sub(1);
                }
            }
            return d0;
        }
        // Extended: six-byte nodes.
        let peg = peg & 0x7fff;
        if key_on {
            self.set_tw(tn, T_PEG_DELAY, 0);
        }
        let pos = usize::from(self.track[tn][T_PEG_POS]) * 6;
        let at = (self.sdtop + usize::from(peg) + pos + 2) as isize;
        let mut modv = self.tw(tn, T_PEG_MOD);
        let delay = self.track[tn][T_PEG_DELAY];
        if delay == 0 {
            modv = self.rd16(at - 2);
        }
        let m = modv.wrapping_add(self.rd16(at));
        self.set_tw(tn, T_PEG_MOD, m);
        d0 = d0.wrapping_add(m);
        if delay == self.rd8(at + 2) {
            self.track[tn][T_PEG_POS] = self.rd8(at + 3);
            self.track[tn][T_PEG_DELAY] = 0;
        } else {
            // (the driver doesn't count up here: the delay is bumped elsewhere)
            let d = delay.wrapping_add(1);
            self.track[tn][T_PEG_DELAY] = d;
        }
        d0
    }

    /// `mds_get_fm_pitch`: the A4/A0 register values, the A4 byte low.
    fn fm_pitch(&self, tn: usize, pitch: u16) -> u16 {
        let note = usize::from(pitch >> 8);
        let frac = u32::from(pitch & 0xff) << 8;
        let idx = usize::from(note_table(note)).wrapping_add(usize::from(self.track[tn][T_INS_TRS])) & 0xff;
        let tab = |i: usize| FM_FREQ.get(i / 2).copied().unwrap_or(0);
        let mut v = if frac == 0 {
            tab(idx)
        } else {
            let (cur, next) = (tab(idx), tab(idx + 2));
            let diff = u32::from(next.wrapping_sub(cur));
            (((diff * frac) >> 16) as u16).wrapping_add(cur)
        };
        v = v.rotate_left(8);
        let oct = octave_table(note);
        (v & 0xff00) | u16::from((v as u8).wrapping_add(oct))
    }

    /// `mds_get_psg_pitch`: the 10-bit period (shifted by the octave).
    fn psg_pitch(&self, pitch: u16) -> u16 {
        let note = usize::from(pitch >> 8);
        let frac = u32::from(pitch & 0xff) << 8;
        let oct = octave_table(note) >> 3;
        let i = usize::from(note_table(note)) / 2;
        let cur = PSG_FREQ.get(i).copied().unwrap_or(0);
        let next = PSG_FREQ.get(i + 1).copied().unwrap_or(0);
        let diff = u32::from(cur.wrapping_sub(next));
        let interp = ((diff * frac) >> 16) as u16;
        cur.wrapping_sub(interp) >> (oct & 15)
    }

    // ---- voices ----

    fn voice_update(&mut self, tn: usize, rn: usize, chn: u8, f: &mut Fl) {
        match chn {
            0..=4 => self.fm_update(tn, rn, chn, f),
            5 => {
                if f.cf_test(CF_PCM_CONTROL) {
                    self.pcm1_update(tn, rn, f);
                } else {
                    if self.pcm_mode & 0x80 != 0 {
                        // Switched from PCM to FM: the PCM channel is stopped.
                        self.pcm_mode &= !0x80;
                        self.pcm[0].vol |= 0x80;
                    }
                    self.fm_update(tn, rn, chn, f);
                }
            }
            6 | 7 => self.psg_update(tn, rn, chn, f),
            8 => {
                let d4 = self.op_mask(tn);
                if d4 & 0x80 == 0 {
                    self.psg_update(tn, rn, chn, f);
                } else {
                    self.psg(0xdf);
                    let m = d4 & 0x7f;
                    if m != 0 {
                        self.fm3_update(tn, rn, f, m);
                    } else {
                        self.track[tn][T_BASE_ADDR] = m;
                        self.psg_update(tn, rn, chn, f);
                    }
                }
            }
            9 => self.psgn_update(tn, rn, f),
            10 | 11 => {
                let (n, pe) = if chn == 10 { (1, PE_PCM2) } else { (2, PE_PCM3) };
                if f.cf_test(CF_PCM_CONTROL) {
                    if !f.enabled() {
                        self.pcm_mode &= !(1 << pe);
                        self.pcm_key_off(n, f);
                    } else {
                        self.pcm_mode |= 1 << pe;
                        self.pcm_update(tn, rn, n, f);
                    }
                } else if self.pcm_mode & (1 << pe) != 0 {
                    self.pcm_mode &= !(1 << pe);
                    self.pcm[n].vol |= 0x80;
                }
            }
            12..=15 => {
                let d4 = self.op_mask(tn);
                if d4 & 0x80 != 0 {
                    let m = d4 & 0x7f;
                    if m != 0 {
                        self.fm3_update(tn, rn, f, m);
                    } else {
                        self.track[tn][T_BASE_ADDR] = m;
                    }
                }
            }
            _ => {}
        }
    }

    /// The FM channel's registers: the slot (0 to 2) and port (0 or 1) of FM channel `chn` (0, 1, 2, 3, 4, 5).
    fn fm_ch(chn: u8) -> (u8, usize, u8) {
        // (the key-on id, the port, the slot)
        match chn {
            0 => (0, 0, 0),
            1 => (1, 0, 1),
            2 => (2, 0, 2),
            3 => (4, 1, 0),
            4 => (5, 1, 1),
            _ => (6, 1, 2),
        }
    }

    fn fm_cutoff(&mut self, port: usize, slot: u8) {
        for op in 0..4u8 {
            self.fm(port, 0x80 + slot + op * 4, 0xff);
        }
    }

    fn fm3_update_flag(&mut self, tn: usize, f: &Fl, d1: u8) {
        if !f.cf_test(CF_SUSPEND) {
            self.track[tn][T_BASE_ADDR] &= d1;
        }
        self.fm3_mask &= d1;
        self.fm(0, 0x27, (d1 & 0x40) | 0x15);
    }

    fn fm_update(&mut self, tn: usize, rn: usize, chn: u8, f: &mut Fl) {
        let (kid, port, slot) = Driver::fm_ch(chn);
        if f.nf_clr(NF_KEY_OFF) {
            f.cf_clr(CF_KEY_ON);
            let d0 = self.op_mask(tn);
            if d0 & 0x80 != 0 {
                // FM3 special mode.
                let m = d0 & self.fm3_mask;
                self.fm3_mask = m;
                self.fm(0, 0x28, (m << 1).wrapping_add(2));
                if f.nf_clr(NF_FM3) {
                    if !f.enabled() {
                        self.fm3_update_flag(tn, f, 0);
                        self.fm_cutoff(0, 2);
                        return;
                    }
                    self.fm3_update_flag(tn, f, 0xff);
                }
            } else {
                // Normal.
                self.fm(0, 0x28, kid);
                if !f.enabled() {
                    self.fm_cutoff(port, slot);
                    return;
                }
            }
        }
        if f.nf_clr(NF_INS) {
            let ptr = self.rd16(self.tb(tn) + isize::from(self.track[tn][T_INS]) * 2);
            let ins = self.sdtop + usize::from(ptr);
            // The channel is muted, then the instrument loaded.
            for op in 0..4u8 {
                self.fm(port, 0x40 + slot + op * 4, 0x7f);
            }
            for (k, base) in [0x30u8, 0x50, 0x60, 0x70, 0x80, 0x90].iter().enumerate() {
                for op in 0..4u8 {
                    let v = self.rd8(ins as isize + (k * 4) as isize + isize::from(op));
                    self.fm(port, base + slot + op * 4, v);
                }
            }
            let v = self.rd8(ins as isize + 28);
            self.fm(port, 0xb0 + slot, v);
            f.nf_clr(NF_VOL);
            self.fm_update_vol(tn, rn, port, slot);
        } else if f.nf_clr(NF_VOL) {
            self.fm_update_vol(tn, rn, port, slot);
        }
        let pitch = self.pitch_update(tn, f);
        if pitch != self.tw(tn, T_LAST_PITCH) {
            if self.op_mask(tn) & 0x80 != 0 {
                self.set_tw(tn, T_LAST_PITCH, pitch);
                self.fm3_pitch(pitch, self.op_mask(tn), tn);
            } else {
                let v = self.fm_pitch(tn, pitch);
                self.fm(port, 0xa4 + slot, v as u8);
                self.fm(port, 0xa0 + slot, (v >> 8) as u8);
                self.set_tw(tn, T_LAST_PITCH, pitch);
            }
        }
        if f.nf_clr(NF_PAN_LFO) {
            let v = self.track[tn][T_FM_PAN_LFO];
            self.fm(port, 0xb4 + slot, v);
        }
        if f.nf_clr(NF_KEY_ON) && !f.nf_clr(NF_SLUR) {
            f.cf_set(CF_KEY_ON);
            let mut d0 = !self.op_mask(tn);
            if d0 & 0x80 == 0 {
                d0 |= self.fm3_mask;
                self.fm3_mask = d0;
            }
            self.fm(0, 0x28, (d0 << 1 & 0xf0) | kid);
        }
    }

    fn fm_update_vol(&mut self, tn: usize, rn: usize, port: usize, slot: u8) {
        if self.op_mask(tn) & 0x80 != 0 {
            return self.fm3_update_vol(tn, rn);
        }
        let mut d3 = self.track[tn][T_VOL];
        if d3 & 0x80 != 0 {
            let c = if d3 >= 0x90 { 0xff } else { 0 };
            d3 = FM_VOL_TABLE[usize::from((d3 | c) & 0x0f)];
        }
        d3 = d3.wrapping_add(self.vol_hi(rn));
        let alg = usize::from(self.track[tn][T_FM_ALG] & 7);
        for op in (0..4u8).rev() {
            let mut d0 = self.track[tn][T_FM_TL + usize::from(op)];
            if op >= FM_OP_TABLE[alg] {
                let (s, c) = d0.overflowing_add(d3);
                d0 = if c || s & 0x80 != 0 { 0xff } else { s };
            }
            self.fm(port, 0x40 + slot + op * 4, d0);
        }
    }

    fn fm3_update_vol(&mut self, tn: usize, rn: usize) {
        let mut d3 = self.track[tn][T_VOL];
        if d3 & 0x80 != 0 {
            let c = if d3 >= 0x90 { 0xff } else { 0 };
            d3 = FM_VOL_TABLE[usize::from((d3 | c) & 0x0f)];
        }
        d3 = d3.wrapping_add(self.vol_hi(rn));
        let d4 = self.op_mask(tn);
        let alg = usize::from(self.fm3_alg & 7);
        for (op, bit) in [(3u8, 6), (2, 4), (1, 5), (0, 3)] {
            if d4 & (1 << bit) != 0 {
                continue;
            }
            let mut d0 = self.fm3_tl[usize::from(op)];
            if op >= FM_OP_TABLE[alg] {
                let (s, c) = d0.overflowing_add(d3);
                d0 = if c || s & 0x80 != 0 { 0x7f } else { s };
            }
            // Operator `op` of FM3 is register 0x42 + op*4; the mask bits go 6, 4, 5, 3 down the table (as the original has them).
            self.fm(0, 0x42 + op * 4, d0);
        }
    }

    fn fm3_pitch(&mut self, pitch: u16, d4: u8, tn: usize) {
        let v = self.fm_pitch(tn, pitch);
        for (bit, reg) in [(3, 0xadu8), (4, 0xae), (5, 0xac), (6, 0xa6)] {
            if d4 & (1 << bit) == 0 {
                self.fm(0, reg, v as u8);
                self.fm(0, reg - 4, (v >> 8) as u8);
            }
        }
    }

    /// `mds_fm3_update`: a track that controls operators of FM3 (on PSG 3 or a dummy channel).
    fn fm3_update(&mut self, tn: usize, rn: usize, f: &mut Fl, d4: u8) {
        if f.nf_clr(NF_KEY_OFF) {
            f.cf_clr(CF_KEY_ON);
            let m = d4 & self.fm3_mask;
            self.fm3_mask = m;
            self.fm(0, 0x28, (m << 1).wrapping_add(2));
            if f.nf_clr(NF_VOL) {
                self.fm3_update_vol(tn, rn);
            }
        } else if f.nf_clr(NF_VOL) {
            self.fm3_update_vol(tn, rn);
        }
        let pitch = self.pitch_update(tn, f);
        if pitch != self.tw(tn, T_LAST_PITCH) {
            self.set_tw(tn, T_LAST_PITCH, pitch);
            self.fm3_pitch(pitch, d4, tn);
        }
        if f.nf_clr(NF_KEY_ON) && !f.nf_clr(NF_SLUR) {
            f.cf_set(CF_KEY_ON);
            let m = !d4 | self.fm3_mask;
            self.fm3_mask = m;
            self.fm(0, 0x28, (m << 1 & 0xf2) | 0);
        }
    }

    // ---- PCM ----

    /// `mds_set_pcm_mode`: 2 or 3 channels mixed. The Z80 restarts, so every channel stops.
    fn set_pcm_mode(&mut self, m: u8) {
        self.pcm_mode = (self.pcm_mode & 0x10) | m;
        if m == 2 || m == 3 {
            for c in self.pcm.iter_mut() {
                c.vol |= 0x80;
                c.playing = false;
            }
        }
    }

    /// `mds_z80_get_vol`: the track's volume as the Z80's register (15 loudest to 30).
    fn pcm_get_vol(&self, tn: usize, rn: usize) -> u8 {
        let mut d1 = self.track[tn][T_VOL];
        if d1 & 0x80 == 0 {
            d1 = psg_vol_table(usize::from(d1));
        }
        d1 = d1.wrapping_sub(self.vol_lo(rn));
        if d1 & 0x80 == 0 {
            d1 = 0;
        }
        ((!d1) & 15) + 15
    }

    fn pcm1_update(&mut self, tn: usize, rn: usize, f: &mut Fl) {
        if !f.enabled() {
            self.pcm_mode &= !(1 << PE_PCM1);
            return self.pcm_key_off(0, f);
        }
        let was = self.pcm_mode & (1 << PE_PCM1) != 0;
        self.pcm_mode |= 1 << PE_PCM1;
        if !was {
            // Switched from FM: the FM6 key is released.
            self.fm(1, 0x28, 0x02);
        }
        if f.nf_clr(NF_PAN_LFO) {
            let v = self.track[tn][T_FM_PAN_LFO];
            self.fm(1, 0xb6, v);
        }
        self.pcm_update(tn, rn, 0, f);
    }

    fn pcm_update(&mut self, tn: usize, rn: usize, n: usize, f: &mut Fl) {
        if f.nf_clr(NF_KEY_ON) {
            // A key on needs the key off that goes with it.
            if !f.nf_clr(NF_KEY_OFF) {
                return;
            }
            f.cf_set(CF_KEY_ON);
            f.nf &= !((1 << NF_VOL) | (1 << NF_SLUR));
            if f.nf_clr(NF_PCM_HEADER) {
                let h = (self.sdtop + usize::from(self.tw(tn, T_PCM_HEADER))) as isize;
                self.pcm[n].addr = u32::from(self.rd8(h + 1)) << 16 | u32::from(self.rd16(h + 2));
            }
            if f.nf_clr(NF_PCM_PITCH) {
                self.pcm[n].pitch = self.track[tn][T_PCM_PITCH];
                self.pcm[n].len = u32::from(self.tw(tn, T_PCM_LENGTH));
            }
            let v = self.pcm_get_vol(tn, rn);
            let c = &mut self.pcm[n];
            c.vol = v;
            c.playing = true;
            c.pos = 0.0;
        } else if f.nf_clr(NF_KEY_OFF) {
            self.pcm_key_off(n, f);
        } else if f.nf_clr(NF_VOL) {
            let v = self.pcm_get_vol(tn, rn);
            self.pcm[n].vol = v;
        }
    }

    fn pcm_key_off(&mut self, n: usize, f: &mut Fl) {
        f.cf_clr(CF_KEY_ON);
        self.pcm[n].vol |= 0x80;
    }

    /// One sample of the Z80's mix (the DAC's byte), and whether anything plays.
    fn pcm_mix(&mut self) -> Option<u8> {
        let mode3 = self.pcm_mode & 15 == 3;
        let mut sum = 0i32;
        let mut any = false;
        for n in 0..if mode3 { 3 } else { 2 } {
            let c = &mut self.pcm[n];
            if !c.playing || c.vol & 0x80 != 0 {
                c.playing = false;
                continue;
            }
            let at = c.pos as u32;
            if at >= c.len {
                c.playing = false;
                continue;
            }
            any = true;
            let b = self.pcm_data.get((c.addr + at) as usize).copied().unwrap_or(0x80);
            let m = PCM_VOL[usize::from(c.vol.clamp(15, 30) - 15)];
            let v = (i32::from(b) - 128) * m >> 8;
            sum = (sum + v).clamp(-128, 127);
            // The source bytes a sample: rate / 8 for two channels, rate / 6 (at most 1) for three.
            let r = f64::from(c.pitch.clamp(1, 8));
            c.pos += if mode3 { r.min(6.0) / 6.0 } else { r / 8.0 };
        }
        any.then(|| (sum as i8 as u8) ^ 0x80)
    }

    /// The Z80's output rate now.
    fn pcm_rate(&self) -> f64 {
        PCM_CLOCK / if self.pcm_mode & 15 == 3 { 268.0 } else { 201.0 }
    }

    // ---- PSG ----

    fn psg_id(chn: u8) -> u8 {
        match chn {
            6 => 0x80,
            7 => 0xa0,
            8 => 0xc0,
            _ => 0xe0,
        }
    }

    /// `mds_psg_update_env`. The envelope's state: the address, the position of the next command, and the
    /// delay byte (the high nibble frames left, the low the volume; 0 in the high nibble waits for a key off).
    fn psg_update_env(&mut self, tn: usize, rn: usize, chn: u8, f: &mut Fl) {
        enum St {
            Command,
            Tie,
            KeyOff,
        }
        let chnid = Driver::psg_id(chn);
        if !f.enabled() {
            return self.psg_silence(tn, chnid, f);
        }
        let st = if f.nf_clr(NF_KEY_ON) {
            if f.nf_clr(NF_SLUR) {
                if f.nf_test(NF_KEY_OFF) {
                    St::KeyOff
                } else {
                    St::Tie
                }
            } else if f.nf_clr(NF_KEY_OFF) {
                // A key on: the envelope starts.
                f.cf_set(CF_KEY_ON);
                self.track[tn][T_PSG_EG_POS] = 0;
                self.track[tn][T_PSG_EG_DELAY] = 0x1f;
                f.nf_set(NF_SUSTAIN);
                self.track[tn][T_LAST_PITCH] = 0xff; // `st`: the high byte only
                self.set_tw(tn, T_PEG_DELAY, 0);
                St::Command
            } else {
                St::Tie
            }
        } else if f.nf_test(NF_KEY_OFF) {
            St::KeyOff
        } else {
            St::Tie
        };
        match st {
            St::KeyOff => {
                f.nf &= !((1 << NF_SUSTAIN) | (1 << NF_KEY_OFF));
                f.cf &= !(1 << CF_KEY_ON);
            }
            St::Tie => {
                if f.nf_test(NF_SUSTAIN) {
                    let delay = self.track[tn][T_PSG_EG_DELAY];
                    let (c, borrow) = delay.overflowing_sub(0x10);
                    if borrow {
                        // Waiting for a key off: the volume only.
                        if f.nf_clr(NF_VOL) {
                            self.psg_write_vol(tn, rn, chnid, c & 15);
                        }
                        return;
                    }
                    if c >= 0x10 {
                        // A frame of the delay passes.
                        self.track[tn][T_PSG_EG_DELAY] = c;
                        if f.nf_clr(NF_VOL) {
                            self.psg_write_vol(tn, rn, chnid, c & 15);
                        }
                        return;
                    }
                }
            }
            St::Command => {}
        }
        // Read the next command.
        let mut pos = self.track[tn][T_PSG_EG_POS].wrapping_add(1);
        if pos == 0 {
            return self.psg_silence(tn, chnid, f);
        }
        let eg = (self.sdtop + usize::from(self.tw(tn, T_PSG_EG_ADDR))) as isize;
        let mut cmd = self.rd8(eg + isize::from(pos) - 1);
        let mut read_next = false;
        let mut jumped = false;
        if !f.nf_test(NF_SUSTAIN) {
            // Released: a sustain command is passed over, a jump isn't taken.
            if cmd == 0x01 {
                f.nf_set(NF_SUSTAIN);
                read_next = true;
            }
        } else if cmd == 0x01 {
            // Sustain: hold the level until the key off.
            let v = self.track[tn][T_PSG_EG_DELAY] & 0x0f;
            self.track[tn][T_PSG_EG_DELAY] = v;
            if f.nf_clr(NF_VOL) {
                self.psg_write_vol(tn, rn, chnid, v);
            }
            return;
        } else if cmd == 0x02 {
            pos = self.rd8(eg + isize::from(pos));
            read_next = true;
            jumped = true;
        }
        let _ = jumped;
        if read_next {
            cmd = self.rd8(eg + isize::from(pos));
            pos = pos.wrapping_add(1);
        }
        if cmd < 0x10 {
            return self.psg_silence(tn, chnid, f);
        }
        self.track[tn][T_PSG_EG_POS] = pos;
        self.track[tn][T_PSG_EG_DELAY] = cmd;
        f.nf_clr(NF_VOL);
        self.psg_write_vol(tn, rn, chnid, cmd & 0x0f);
    }

    fn psg_silence(&mut self, tn: usize, chnid: u8, f: &mut Fl) {
        self.track[tn][T_PSG_EG_POS] = 0xff;
        self.track[tn][T_PSG_EG_DELAY] = 0x0f;
        f.cf_clr(CF_KEY_ON);
        f.nf_clr(NF_KEY_OFF);
        self.psg(0x1f | chnid);
    }

    fn psg_write_vol(&mut self, tn: usize, rn: usize, chnid: u8, evol: u8) {
        let mut tvol = self.track[tn][T_VOL];
        if tvol & 0x80 == 0 {
            tvol = psg_vol_table(usize::from(tvol));
        }
        tvol = !tvol & 15;
        tvol = tvol.wrapping_add(evol & 15).wrapping_add(self.vol_lo(rn));
        if tvol >= 15 {
            tvol = 15;
        }
        self.psg(0x10 | tvol | chnid);
    }

    fn psg_update(&mut self, tn: usize, rn: usize, chn: u8, f: &mut Fl) {
        self.psg_update_env(tn, rn, chn, f);
        let pitch = self.pitch_update(tn, f);
        if pitch == self.tw(tn, T_LAST_PITCH) {
            return;
        }
        self.psg_set_pitch(tn, chn, pitch);
    }

    fn psg_set_pitch(&mut self, tn: usize, chn: u8, pitch: u16) {
        self.set_tw(tn, T_LAST_PITCH, pitch);
        let p = self.psg_pitch(pitch);
        let chnid = Driver::psg_id(chn);
        self.psg((p as u8 & 0x0f) | chnid);
        self.psg(((p >> 4) as u8) & 0x7f);
    }

    fn psgn_update(&mut self, tn: usize, rn: usize, f: &mut Fl) {
        self.psg_update_env(tn, rn, 9, f);
        let pitch = self.pitch_update(tn, f);
        let nreset = self.tw(tn, T_PSG_NRESET);
        let mode = nreset as u8;
        let reset = nreset & 0x8000 != 0 || f.nf_clr(NF_PAN_LFO);
        if !reset {
            if pitch == self.tw(tn, T_LAST_PITCH) {
                return;
            }
            if mode != 0 {
                return self.psg_set_pitch(tn, 8, pitch);
            }
        } else if mode != 0 {
            self.psg(mode);
            if pitch != self.tw(tn, T_LAST_PITCH) {
                self.psg_set_pitch(tn, 8, pitch);
            }
            return;
        }
        // Preset noise: the octave picks the rate.
        self.set_tw(tn, T_LAST_PITCH, pitch);
        self.psg(0xe0 | (((pitch >> 8) as u8) & 7));
    }
}

// ---- playing ----

/// A song for the Tracker: its notes without playing it.
pub struct Timeline {
    /// Step (from 1; 96 a bar), channel, MIDI note (None where a rest cuts it).
    pub events: Vec<(u64, usize, Option<u8>)>,
    pub steps: u64,
    pub restart: Option<u64>,
    pub seconds: f64,
}

impl Song {
    /// The notes, followed without sound (at most a quarter hour).
    pub fn timeline(&self) -> Timeline {
        let mut d = Driver::new(self);
        d.heard = Some(Vec::new());
        let mut rounds: Vec<(u32, u64)> = Vec::new();
        let mut frames = 0u32;
        while frames < 54_000 && rounds.len() < 2 {
            d.update();
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
        events.retain(|e| e.0 <= once_steps.max(1));
        Timeline { events, steps: once_steps, restart, seconds: f64::from(once_frames) * FRAME }
    }
}

impl Tune for Song {
    fn format(&self) -> &'static str {
        "mds"
    }

    fn file(&self) -> &[u8] {
        &self.file
    }

    fn driver(&self, _song: usize) -> Box<dyn ReplayDriver> {
        Box::new(Player::new(self))
    }

    fn length(&self, _song: usize) -> Option<f64> {
        Some(self.timeline().seconds)
    }
}

const YM_CLOCK: f64 = 7_670_454.0;

/// The song playing: a frame a tick, the chips between.
pub struct Player {
    d: Driver,
    pos: f64,
    dac_pos: f64,
    done: bool,
}

impl Player {
    pub fn new(song: &Song) -> Player {
        let mut d = Driver::new(song);
        // Every operator released, the LFO on at its slowest, as a console's driver would leave it.
        for hi in [false, true] {
            for r in 0x80..0x90u8 {
                d.ym.write(hi, r, 0xff);
            }
        }
        for c in [0u8, 1, 2, 4, 5, 6] {
            d.ym.write(false, 0x28, c);
        }
        d.ym.write(false, 0x22, 0x08);
        Player { d, pos: 0.0, dac_pos: 0.0, done: false }
    }
}

impl ReplayDriver for Player {
    fn channels(&self) -> usize {
        CHANNEL_NAMES.len()
    }

    fn tick(&mut self) {
        self.d.update();
        if self.d.take_round() {
            self.done = true;
        }
    }

    fn tick_seconds(&self) -> f64 {
        FRAME
    }

    fn render(&mut self, rate: u32, frames: usize, out: &mut Bufs) {
        {
            let (l, r) = (&mut out.left, &mut out.right);
            let [.., _] = l.as_mut_slice() else { return };
            let _ = r;
        }
        // The PSG into its four channels' buffers (6 to 9).
        let mut psg = vec![vec![0f32; frames]; 4];
        {
            let [a, b, c, e] = &mut psg[..] else { return };
            self.d.sn.render(rate, &mut [&mut a[..], &mut b[..], &mut c[..], &mut e[..]]);
        }
        for ch in 0..4 {
            for i in 0..frames {
                let s = psg[ch][i] * 0.12;
                out.left[6 + ch][i] = s;
                out.right[6 + ch][i] = s;
            }
        }
        // The YM2612, its samples averaged into the output's; the Z80's mix writes the DAC between them.
        let per = Ym2612::rate(YM_CLOCK) / f64::from(rate.max(1));
        let level = 128.0 * 64.0 / (6.0 * 65.0) / 32768.0 * 2.4;
        let mut o = [[0i32; 2]; 6];
        for i in 0..frames {
            self.pos += per;
            let mut n = 0;
            let mut acc = [[0f32; 2]; 6];
            while self.pos >= 1.0 {
                self.pos -= 1.0;
                self.dac_pos += self.d.pcm_rate() / Ym2612::rate(YM_CLOCK);
                while self.dac_pos >= 1.0 {
                    self.dac_pos -= 1.0;
                    match self.d.pcm_mix() {
                        Some(v) => {
                            if !self.d.dac_on {
                                self.d.dac_on = true;
                                self.d.ym.write(false, 0x2b, 0x80);
                            }
                            self.d.ym.write(false, 0x2a, v);
                        }
                        None if self.d.dac_on => {
                            self.d.dac_on = false;
                            self.d.ym.write(false, 0x2a, 0x80);
                            self.d.ym.write(false, 0x2b, 0x00);
                        }
                        None => {}
                    }
                }
                self.d.ym.generate(&mut o);
                for c in 0..6 {
                    acc[c][0] += o[c][0] as f32;
                    acc[c][1] += o[c][1] as f32;
                }
                n += 1;
            }
            let k = level / n.max(1) as f32;
            for c in 0..6 {
                out.left[c][i] = acc[c][0] * k;
                out.right[c][i] = acc[c][1] * k;
            }
        }
    }

    fn position(&self) -> (usize, usize) {
        // A pattern a bar (96 steps), a row a sixteenth (6 steps).
        let s = self.d.steps;
        ((s / 96) as usize, ((s % 96) / 6) as usize)
    }

    fn speed(&self) -> (u32, u32) {
        (1, self.d.bpm().max(1))
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

    // A hand-made .mds: one FM track playing C1 notes with the instrument `ins`.
    fn mds(seq: &[u8], blocks: &[(u32, Vec<u8>)]) -> Vec<u8> {
        let mut out = b"RIFF\0\0\0\0MDS0".to_vec();
        let chunk = |out: &mut Vec<u8>, id: &[u8; 4], body: &[u8]| {
            out.extend(id);
            out.extend((body.len() as u32).to_le_bytes());
            out.extend(body);
            if body.len() % 2 == 1 {
                out.push(0);
            }
        };
        chunk(&mut out, b"ver ", &[0, 4]);
        chunk(&mut out, b"seq ", seq);
        let mut list = b"dblk".to_vec();
        for (id, data) in blocks {
            list.extend(b"glob");
            list.extend(((data.len() + 4) as u32).to_le_bytes());
            list.extend(id.to_le_bytes());
            list.extend(data);
            if (data.len() + 4) % 2 == 1 {
                list.push(0);
            }
        }
        chunk(&mut out, b"LIST", &list);
        let n = out.len() as u32 - 8;
        out[4..8].copy_from_slice(&n.to_le_bytes());
        out
    }

    // Sequence: header (tbase 8: after the 4-byte header and one 4-byte track entry), a song data table of
    // `table` words, the track's commands.
    fn seq(channel: u8, table: usize, cmds: &[u8]) -> Vec<u8> {
        let tbase = 8usize;
        let pos = (table * 2) as u16 + 0;
        let mut s = vec![];
        s.extend((tbase as i16).to_be_bytes());
        s.push(0); // volume
        s.push(1); // one track
        s.push(channel);
        s.push(0);
        s.extend(pos.to_be_bytes());
        s.extend(std::iter::repeat(0).take(table * 2));
        s.extend(cmds);
        s
    }

    // A plain FM voice: a loud carrier on every operator.
    fn voice() -> Vec<u8> {
        let mut v = vec![];
        v.extend([0x01u8; 4]); // DT/MUL
        v.extend([0x1f; 4]); // KS/AR
        v.extend([0; 4]); // AM/DR
        v.extend([0; 4]); // SR
        v.extend([0x0f; 4]); // SL/RR
        v.extend([0; 4]); // SSG
        v.extend([0x00; 4]); // TL
        v.push(0x07); // FB/ALG: algorithm 7, all carriers
        v.push(0x30);
        v
    }

    // A song of one looping FM note, as a driver ready to be asked things.
    fn looping() -> Driver {
        let s = seq(0, 1, &[0xe1, 0x00, 0xe2, 0x00, 0x82 + 24, 0x17, 0x17, 0xf5, 0xff, 0xf4]);
        Driver::new(&Song::parse(&mds(&s, &[(0, voice())])).unwrap())
    }

    #[test]
    fn a_fade_in_request_starts_quiet_and_rises_to_the_songs_volume() {
        let mut d = looping();
        d.request_song(1 | 0x3c00, 3); // $2000 + (7 << 10): the fastest fade, a step a frame
        d.update();
        d.update();
        assert!(d.playing());
        let start = d.command(0x0c, 3, 0);
        assert!((0x40..=0x50).contains(&start), "starts near $50, not {start:#x}");
        for _ in 0..30 {
            d.update();
        }
        let later = d.command(0x0c, 3, 0);
        assert!(later < start, "louder after a while: {start:#x} then {later:#x}");
        for _ in 0..200 {
            d.update();
        }
        assert_eq!(d.command(0x0c, 3, 0), 0, "ends at the song's own volume");
        assert!(d.playing(), "a fade-in never stops the song");
        // A sound number out of range, a fade-in of song 0 and of a song that isn't there are turned away (the request still stops what played).
        for bad in [0x0002u16, 0x3c00, 0x3c05] {
            let mut e = looping();
            e.request_song(bad, 3);
            for _ in 0..5 {
                e.update();
            }
            assert!(!e.playing(), "{bad:#x} played");
        }
    }

    #[test]
    fn commands_ask_and_set_what_the_api_says() {
        let mut d = looping();
        for _ in 0..4 {
            d.update();
        }
        assert_eq!(d.command(0x00, 0, 0), 0x13);
        assert_eq!(d.command(0x01, 0, 0), 1);
        assert_ne!(d.command(0x02, 3, 0), 0, "the music's track is active");
        assert_eq!(d.command(0x02, 0, 0), 0);
        assert_eq!(d.command(0x04, 0, 0), 128);
        d.command(0x05, 100, 0);
        assert_eq!(d.command(0x04, 0, 0), 100);
        d.command(0x07, 0x0a05, 0);
        assert_eq!(d.command(0x06, 0, 0), 0x0a05);
        d.command(0x0d, 3, 0x20);
        assert_eq!(d.command(0x0c, 3, 0), 0x20);
        d.command(0x0f, 3, 77);
        assert_eq!(d.command(0x0e, 3, 0), 77);
        assert_eq!(d.command(0x12, 0, 0), 2);
        d.command(0x11, 3, 0);
        assert_eq!(d.command(0x12, 0, 0), 3);
        d.command(0x11, 0, 0);
        assert_eq!(d.command(0x12, 0, 0), 3, "0 keeps the mode");
        assert_eq!(d.command(0x7f, 0, 0), 0, "an unknown command does nothing");
    }

    #[test]
    fn pause_holds_the_music_and_resume_goes_on() {
        let mut d = looping();
        for _ in 0..4 {
            d.update();
        }
        let before = d.steps;
        d.command(0x0b, 3, 1);
        for _ in 0..60 {
            d.update();
        }
        let held = d.steps;
        assert!(held >= before);
        for _ in 0..60 {
            d.update();
        }
        // The steps counter is the slot's clock, which keeps counting; what stops is the track, so its note flag stays enabled and it is suspended.
        assert!(d.track.iter().any(|t| t[T_CHANNEL_FLAG] & (1 << CF_SUSPEND) != 0));
        d.command(0x0b, 3, 0);
        assert!(d.track.iter().all(|t| t[T_CHANNEL_FLAG] & (1 << CF_SUSPEND) == 0));
        for _ in 0..60 {
            d.update();
        }
        assert!(d.playing());
    }

    #[test]
    fn a_fade_out_with_bit_7_stops_the_song() {
        let mut d = looping();
        for _ in 0..4 {
            d.update();
        }
        d.command(0x0a, 0x0700 | 0x80 | 0x10, 0); // fastest, to volume 16, then stop
        for _ in 0..100 {
            d.update();
        }
        assert!(!d.playing(), "stopped after the fade");
        let mut e = looping();
        for _ in 0..4 {
            e.update();
        }
        e.command(0x0a, 0x0700 | 0x10, 0); // the same without the stop bit
        for _ in 0..100 {
            e.update();
        }
        assert!(e.playing());
        assert_eq!(e.command(0x0c, 3, 0), 0x10);
    }

    #[test]
    fn a_hand_made_song_plays_notes() {
        // ins 0, volume 0 (loudest), note C-3 (0x82 + 24) for a quarter, rest, loop.
        let s = seq(0, 1, &[0xe1, 0x00, 0xe2, 0x00, 0x82 + 24, 0x17, 0x17, 0xf5, 0xff, 0xf4]);
        let f = mds(&s, &[(0, voice())]);
        let song = Song::parse(&f).unwrap();
        let t = song.timeline();
        let notes: Vec<_> = t.events.iter().filter(|e| e.2.is_some()).collect();
        assert!(!notes.is_empty(), "{:?}", t.events);
        assert_eq!(notes[0].1, 0);
        assert_eq!(notes[0].2, Some(36));
    }

    #[test]
    fn the_tables_are_the_drivers() {
        assert_eq!(psg_vol_table(0), 0x8f);
        assert_eq!(psg_vol_table(127), 0x80);
        assert_eq!(psg_vol_table(41), 0x81);
        assert_eq!(convert_vol(0), 0);
        assert_eq!(convert_vol(127), 15);
        assert_eq!(note_table(0), 0);
        assert_eq!(note_table(13), 2);
        assert_eq!(octave_table(24), 16);
    }

    fn crossings(f: &[u8], ch: usize, seconds: f64) -> (f64, f32) {
        let song = Song::parse(f).unwrap();
        let mut p = Player::new(&song);
        let rate = 48_000u32;
        let mut samples: Vec<f32> = Vec::new();
        let mut t = 0.0;
        while t < seconds {
            p.tick();
            let frames = (FRAME * f64::from(rate)).round() as usize;
            let mut b = Bufs { left: vec![vec![0.0; frames]; 10], right: vec![vec![0.0; frames]; 10] };
            p.render(rate, frames, &mut b);
            samples.extend((0..frames).map(|i| b.left[ch][i] + b.right[ch][i]));
            t += FRAME;
        }
        // Skip the start (the first frames request and start the song), count rises through the middle.
        let tail = &samples[samples.len() / 3..];
        let mean = tail.iter().sum::<f32>() / tail.len() as f32;
        let tail: Vec<f32> = tail.iter().map(|v| v - mean).collect();
        let peak = tail.iter().cloned().fold(0.0f32, |a, b| a.max(b.abs()));
        let mut n = 0;
        let mut above = false;
        for &s in &tail {
            if s > 0.0 && !above {
                n += 1;
                above = true;
            } else if s < -peak * 0.1 {
                above = false;
            }
        }
        (f64::from(n) / (tail.len() as f64 / f64::from(rate)), peak)
    }

    #[test]
    fn an_fm_note_sounds_at_its_pitch() {
        // Note 24 is block 2, F-number 644: 65.4 Hz (note 0 is 16.35 Hz).
        let s = seq(0, 1, &[0xe1, 0x00, 0xe2, 0x00, 0x82 + 24, 0x7f, 0xf5, 0xff, 0xff]);
        let f = mds(&s, &[(0, voice())]);
        let (hz, peak) = crossings(&f, 0, 1.0);
        assert!(peak > 0.01, "peak {peak}");
        assert!((hz - 65.4).abs() < 2.0, "{hz}");
        // An octave up.
        let s = seq(0, 1, &[0xe1, 0x00, 0xe2, 0x00, 0x82 + 36, 0x7f, 0xf5, 0xff, 0xff]);
        let (hz, _) = crossings(&mds(&s, &[(0, voice())]), 0, 1.0);
        assert!((hz - 130.8).abs() < 3.0, "{hz}");
    }

    #[test]
    fn a_psg_note_sounds_at_its_pitch() {
        // PSG 1 (channel 6), an envelope of full volume held (volume 0, 1 frame, then sustain).
        let s = seq(6, 1, &[0xe1, 0x00, 0xe2, 0x8f, 0x82 + 12, 0x7f, 0xf5, 0xff, 0xff]);
        let f = mds(&s, &[(0, vec![0x10, 0x01])]);
        let (hz, peak) = crossings(&f, 6, 1.0);
        assert!(peak > 0.01, "peak {peak}");
        // Note 12 on the PSG: the period 1710 / 2 at the NTSC clock, 130.8 Hz (note 0 is 65.4 Hz).
        assert!((hz - 130.8).abs() < 4.0, "{hz}");
    }

    // A song on FM 6 playing a PCM sample (a square wave of 64 bytes a period) at `rate`, in `mode`.
    fn pcm_song(rate: u8, mode: u8) -> Vec<u8> {
        let s = seq(5, 1, &[0xf2, mode, 0xf0, 0x00, 0xf1, rate, 0xe2, 0x8f, 0x82 + 24, 0x7f, 0xf5, 0xff, 0xff]);
        let mut out = b"RIFF\0\0\0\0MDS0".to_vec();
        let mut chunk = |id: &[u8; 4], body: &[u8]| {
            out.extend(id);
            out.extend((body.len() as u32).to_le_bytes());
            out.extend(body);
            if body.len() % 2 == 1 {
                out.push(0);
            }
        };
        chunk(b"ver ", &[0, 4]);
        chunk(b"seq ", &s);
        let mut list = b"dblk".to_vec();
        let mut h = vec![];
        for v in [0u32, 0, 0, 30000, 0, 0, 17500, 0, 0] {
            h.extend(v.to_le_bytes());
        }
        list.extend(b"pcmh");
        list.extend((h.len() as u32).to_le_bytes());
        list.extend(&h);
        chunk(b"LIST", &list);
        let pcm: Vec<u8> = (0..30000).map(|i| if i % 64 < 32 { 0xe0 } else { 0x20 }).collect();
        chunk(b"pcmd", &pcm);
        let n = out.len() as u32 - 8;
        out[4..8].copy_from_slice(&n.to_le_bytes());
        out
    }

    #[test]
    fn a_pcm_sample_plays_at_its_rate() {
        // Two channels at 17.8 kHz: rate 8 reads a byte a sample, so 64 bytes are 278 Hz; rate 4 is half that.
        let (hz, peak) = crossings(&pcm_song(8, 2), 5, 0.6);
        assert!(peak > 0.01, "peak {peak}");
        assert!((hz - 278.0).abs() < 12.0, "{hz}");
        let (hz, _) = crossings(&pcm_song(4, 2), 5, 0.6);
        assert!((hz - 139.0).abs() < 8.0, "{hz}");
        // Three channels at 13.4 kHz: rate 6 reads a byte a sample, 209 Hz.
        let (hz, _) = crossings(&pcm_song(6, 3), 5, 0.6);
        assert!((hz - 209.0).abs() < 10.0, "{hz}");
    }

    // The real songs rendered: every channel's energy, when MDS_DIR is set.
    #[test]
    fn real_songs_sound() {
        let Ok(dir) = std::env::var("MDS_DIR") else { return };
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if !p.extension().is_some_and(|x| x == "mds") {
                continue;
            }
            let song = Song::parse(&std::fs::read(&p).unwrap()).unwrap();
            let mut pl = Player::new(&song);
            let rate = 44_100u32;
            let mut energy = [0f64; 10];
            let mut bad = false;
            for _ in 0..60 * 20 {
                pl.tick();
                let frames = (FRAME * f64::from(rate)).round() as usize;
                let mut b = Bufs { left: vec![vec![0.0; frames]; 10], right: vec![vec![0.0; frames]; 10] };
                pl.render(rate, frames, &mut b);
                for c in 0..10 {
                    for i in 0..frames {
                        let v = b.left[c][i];
                        bad |= !v.is_finite() || v.abs() > 4.0;
                        energy[c] += f64::from(v * v);
                    }
                }
            }
            eprintln!("{}: {:?}", p.file_name().unwrap().to_string_lossy(), energy.iter().map(|e| (e.sqrt() * 10.0).round() / 10.0).collect::<Vec<_>>());
            assert!(!bad);
            assert!(energy[..6].iter().sum::<f64>() > 0.0);
        }
    }

    // The files of the repository's data/bgm, when MDS_DIR is set.
    #[test]
    fn real_songs_play() {
        let Ok(dir) = std::env::var("MDS_DIR") else { return };
        let mut n = 0;
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "mds") {
                let data = std::fs::read(&p).unwrap();
                let song = Song::parse(&data).unwrap();
                let t = song.timeline();
                let notes = t.events.iter().filter(|e| e.2.is_some()).count();
                eprintln!("{}: {} notes, {} steps, {:.1}s restart {:?}", p.display(), notes, t.steps, t.seconds, t.restart);
                assert!(notes > 20, "{}", p.display());
                n += 1;
            }
        }
        assert!(n > 0);
    }
}
