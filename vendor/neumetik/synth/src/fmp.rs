// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.
//
// Ported from 98fmplayer's FMP driver (github.com/takamichih/fmplayer),
// under its BSD 2-Clause License; the notice is in `opna.rs`.

//! FMP (Guu's sound driver for the PC-98), playing its compiled songs
//! (.OPI, .OVI, .OZI) on the OPNA: six FM parts and three more on FM 3's
//! operators, three SSG parts with software envelopes (or PPZ8 channels in
//! their place), the rhythm sounds and ADPCM, and PDZF, which plays parts
//! through PPZ8 alongside. A port of 98fmplayer's, which is a
//! reimplementation of FMP's own code.

use crate::fmwork::{u16s16, u8s8, Work};

pub const FM_1: usize = 0;
pub const FM_3: usize = 2;
pub const FM_4: usize = 3;
pub const FM_EX1: usize = 6;
pub const FM_EX3: usize = 8;
pub const PPZ8_1: usize = 9;
pub const SSG_1: usize = 18;
pub const ADPCM: usize = 21;
const PART_NUM: usize = 22;
/// The rhythm part, kept apart as 98fmplayer keeps it.
pub const RHYTHM: usize = PART_NUM;

const DATA_NUM: usize = 14;

const LFOWF_TRIANGLE: u8 = 0;
const LFOWF_SAWTOOTH: u8 = 1;
const LFOWF_SQUARE: u8 = 2;
const LFOWF_LINEAR: u8 = 3;
const LFOWF_STAIRCASE: u8 = 4;
const LFOWF_TRIANGLE2: u8 = 5;
const LFOWF_RANDOM: u8 = 6;

const PDZF_ENV_VOL_MIN: i8 = -15;
const PDZF_ENV_ATT: u8 = 0;
const PDZF_ENV_DEC: u8 = 1;
const PDZF_ENV_REL: u8 = 2;
const PDZF_ENV_OFF: u8 = 3;

const OPNA_DTMUL: u8 = 0x30;
const OPNA_TL: u8 = 0x40;
const OPNA_SLRR: u8 = 0x80;
const OPNA_FNUM1: u8 = 0xa0;
const OPNA_BLKFNUM2: u8 = 0xa4;
const OPNA_FBALG: u8 = 0xb0;
const OPNA_LRAMSPMS: u8 = 0xb4;

// The part's union (FM, SSG and ADPCM share its bytes, as in 98fmplayer).
// FM:
const U_TONE_TL: usize = 0;
const A_DELAY: usize = 4;
const A_SPEED: usize = 5;
const A_DELAY_CNT: usize = 6;
const A_SPEED_CNT: usize = 7;
const A_DEPTH: usize = 8;
const A_DEPTH_CNT: usize = 9;
const A_RATE: usize = 10;
const A_RATE_ORIG: usize = 11;
const W_DELAY: usize = 12;
const W_SPEED: usize = 13;
const W_DELAY_CNT: usize = 14;
const W_SPEED_CNT: usize = 15;
const W_DEPTH: usize = 16;
const W_DEPTH_CNT: usize = 17;
const W_RATE: usize = 18;
const W_RATE_ORIG: usize = 19;
const W_RATE_CURR: usize = 20;
const W_SYNC: usize = 21;
const H_DELAY: usize = 22;
const H_DELAY_CNT: usize = 23;
const H_FREQ: usize = 24;
const H_APMS: usize = 25;
const U_SLOT_MASK: usize = 26;
const U_SLOT_REL_VOL: usize = 27;
// SSG:
const S_CURR_VOL: usize = 0;
const S_ENV_F: usize = 1;
const S_OCTAVE: usize = 2;
const S_VOL: usize = 3;
const S_ENV: usize = 4;
const S_ENVBAK: usize = 10;
const E_STARTVOL: usize = 0;
const E_ATTACK: usize = 1;
const E_DECAY: usize = 2;
const E_SUSTAIN_LV: usize = 3;
const E_SUSTAIN_RATE: usize = 4;
const E_RELEASE: usize = 5;
// The SSG envelope's flags, as the compiler packs them.
const F_PPZ: u8 = 0x01;
const F_NOISE: u8 = 0x02;
const F_PORTAMENTO: u8 = 0x04;
const F_RELEASE: u8 = 0x08;
const F_SUSTAIN: u8 = 0x10;
const F_DECAY: u8 = 0x20;
const F_ATTACK: u8 = 0x40;
// ADPCM:
const D_START: usize = 0;
const D_END: usize = 2;
const D_DELTAT: usize = 4;

#[derive(Clone, Copy, Default)]
struct Lfo {
    delay: u8,
    speed: u8,
    delay_cnt: u8,
    speed_cnt: u8,
    depth: u8,
    depth_cnt: u8,
    rate: u16,
    rate2: u16,
    waveform: u8,
}

#[derive(Clone, Copy, Default)]
struct LfoF {
    mask: bool,
    lfo: bool,
    e: bool,
    w: bool,
    a: bool,
    r: bool,
    q: bool,
    p: bool,
}

#[derive(Clone, Copy, Default)]
struct Status {
    keyon: bool,
    slur: bool,
    tie_cont: bool,
    off: bool,
    lfo_sync: bool,
    pitchbend: bool,
    rest: bool,
    tie: bool,
}

#[derive(Clone, Copy, Default)]
struct Pit {
    rate: u8,
    delay: u8,
    speed: u8,
    speed_cnt: u8,
    target_freq: u16,
    target_note: u8,
}

#[derive(Clone, Copy, Default)]
struct Type {
    fm: bool,
    fm_3: bool,
    ssg: bool,
    rhythm: bool,
    adpcm: bool,
}

#[derive(Clone, Copy, Default)]
struct PdzfPit {
    on: bool,
    rate: u8,
    delay: u8,
    speed: u8,
    speed_cnt: u8,
    target_freq: u32,
    target_note: u8,
    pitchdiff: i32,
}

#[derive(Clone, Copy, Default)]
struct Pdzf {
    loopstart32: u32,
    loopend32: u32,
    mode: u8,
    ppz8_channel: u8,
    voice: u8,
    vol: u8,
    prev_note: u8,
    pan: i8,
    env_al: u8,
    env_dd: i8,
    env_sr: u8,
    env_rr: u8,
    env_status: u8,
    env_cnt: u8,
    env_vol: i8,
    keyon: bool,
    pit: PdzfPit,
    lfodiff: i32,
    lastfreq: u32,
}

#[derive(Clone, Copy, Default)]
pub struct Part {
    lfo_f: LfoF,
    default_len: u8,
    current_vol: u8,
    gate_cmp: u8,
    tonelen_cnt: u8,
    gate_cnt: u8,
    actual_vol: u8,
    keyon_delay: u8,
    keyon_delay_cnt: u8,
    pub prev_note: u8,
    status: Status,
    sync: u8,
    detune: u16,
    pit: Pit,
    lfo: [Lfo; 3],
    actual_freq: u16,
    prev_freq: u16,
    note_diff: u8,
    tone: u8,
    ext_keyon: u16,
    pan_ams_pms: u8,
    slot_vol_mask: u8,
    current_ptr: u16,
    loop_ptr: u16,
    type_: Type,
    part_bit: u16,
    opna_keyon_out: u8,
    eff_chan: u8,
    u: [u8; 32],
    tonelen: u8,
    pdzf: Pdzf,
    // Notes and rests read (for the screen; not FMP's).
    notes: u8,
}

impl Part {
    fn envf(&self, f: u8) -> bool {
        self.u[S_ENV_F] & f != 0
    }

    fn set_envf(&mut self, f: u8, on: bool) {
        if on {
            self.u[S_ENV_F] |= f;
        } else {
            self.u[S_ENV_F] &= !f;
        }
    }

    fn u16at(&self, o: usize) -> u16 {
        u16::from(self.u[o]) | u16::from(self.u[o + 1]) << 8
    }

    fn set_u16at(&mut self, o: usize, v: u16) {
        self.u[o] = v as u8;
        self.u[o + 1] = (v >> 8) as u8;
    }

    fn env(&self, i: usize) -> u8 {
        self.u[S_ENV + i]
    }
}

#[derive(Clone, Copy, Default)]
struct PdzfRhythm {
    voice: [u8; 2],
    pan: i8,
    note: u8,
    enabled: bool,
}

#[derive(Clone, Copy, Default)]
struct Rhythm {
    mask: u8,
    len_cnt: u8,
    default_len: u8,
    tl_volume: u8,
    volumes: [u8; 6],
    pans: [u8; 6],
    sync: u8,
    status: bool,
}

#[derive(Clone, Copy, Default)]
struct DataInfo {
    partptr: [u16; DATA_NUM],
    loopptr: [u16; DATA_NUM],
    bar: u8,
    fmtoneptr: u16,
    ssgtoneptr: u16,
    adpcmptr: u16,
    q: bool,
    ppz: bool,
    lfo_octave_fix: bool,
}

/// An FMP song being played.
#[derive(Clone)]
pub struct Fmp {
    data: Vec<u8>,
    datalen: u16,
    timerb: u8,
    sync_data: u8,
    sync_cnt: u16,
    looped: bool,
    stopped: bool,
    total_clocks: u16,
    clock_divider: u8,
    fm_vol: u8,
    loop_cnt: u8,
    timerb_bak: u8,
    ssg_mix: u8,
    timer_ch3: u8,
    ssg_noise_freq: u8,
    loop_dec: u8,
    loop_times: u8,
    data_version: u8,
    fm3_slot_keyon: u8,
    fm3_alg: u8,
    part_playing_bit: u16,
    part_loop_bit: u16,
    ch3_se_freqdiff: [i16; 4],
    adpcm_deltat: u16,
    adpcm_c1: u8,
    adpcm_startaddr: [u16; 0x80],
    adpcm_endaddr: [u16; 0x80],
    /// The PCM files the song asks for, without their extension: PPZ8's
    /// (.PVI) and the ADPCM's (.PVI).
    pub ppz_name: String,
    pub pvi_name: String,
    // The parts, then the rhythm part (`RHYTHM`).
    parts: [Part; PART_NUM + 1],
    rhythm: Rhythm,
    datainfo: DataInfo,
    rand71: u8,
    pdzf_mode: u8,
    pdzf_rhythm: [PdzfRhythm; 2],
    pdzf_rhythm_current_note: u8,
    // The rhythm sounds struck (for the screen): a count, and the last.
    rhythm_hits: u8,
    rhythm_last: u8,
    /// The song's three comment lines (Shift-JIS).
    pub comment: [Vec<u8>; 3],
}

// 30ec
fn fm_freq(note: u8) -> u16 {
    const T: [u16; 12] = [0x026a, 0x028f, 0x02b6, 0x02df, 0x030b, 0x0339, 0x036a, 0x039e, 0x03d5, 0x0410, 0x044e, 0x048f];
    T[usize::from(note % 0xc)].wrapping_add(u16::from(note / 0xc) << 11)
}

fn ssg_octave(note: u8) -> u8 {
    note / 0xc
}

fn ppz_freq(note: u8) -> u16 {
    const T: [u16; 12] = [0x8000, 0x87a6, 0x8fb3, 0x9838, 0xa146, 0xaade, 0xb4ff, 0xbfcc, 0xcb34, 0xd747, 0xe418, 0xf1a5];
    T[usize::from(note % 0xc)]
}

// 311e
fn part_ssg_freq(p: &Part, note: u8) -> u16 {
    const T: [u16; 12] = [0x0ee8, 0x0e12, 0x0d48, 0x0c89, 0x0bd5, 0x0b2b, 0x0a8a, 0x09f3, 0x0964, 0x08dd, 0x085e, 0x07e6];
    let f = if p.envf(F_PPZ) { ppz_freq(note) } else { T[usize::from(note % 0xc)] };
    p.detune.wrapping_add(f)
}

// 3172
fn adpcm_freq(note: u8) -> u16 {
    const T: [[u16; 12]; 4] = [
        [0xdb22, 0xdd53, 0xdfa6, 0xe21c, 0xe4b7, 0xe777, 0xea65, 0xed7f, 0xf0c8, 0xf443, 0xf7f4, 0xfbdc],
        [0x0000, 0x0463, 0x0909, 0x0df6, 0x132d, 0x1864, 0x1e8a, 0x24bd, 0x2b4e, 0x3244, 0x39a3, 0x4173],
        [0x49ba, 0x527e, 0x5bc8, 0x65a0, 0x700d, 0x7b19, 0x86cc, 0x9336, 0xa057, 0xae42, 0xbd01, 0xcca2],
        [0xc8b4, 0xc9cc, 0xcaf5, 0xcc30, 0xcd7e, 0xcedf, 0xd056, 0xd1e3, 0xd387, 0xd544, 0xd71c, 0xd911],
    ];
    let octave = (u16::from(note / 0xc).wrapping_sub(3)) & 3;
    T[usize::from(octave)][usize::from(note % 0xc)]
}

fn pdzf_vol_clamp(v: u8, ev: u8) -> u8 {
    (i16::from(v) + i16::from(u8s8(ev))).clamp(0, 0xf) as u8
}

fn ppz8_note_freq(note: u8) -> u32 {
    let f = u32::from(ppz_freq(note));
    let octave = note / 0xc;
    if octave > 4 {
        f.wrapping_shl(u32::from(octave - 4))
    } else {
        f >> (4 - octave)
    }
}

// The PPZ8 frequency for PDZF's extended mode, `diff` off the note.
fn pdzf_extended_freqdiff(p: &Part, diff: i32) -> u32 {
    let mut f = ppz8_note_freq(p.pdzf.prev_note);
    f = f.wrapping_add(diff.wrapping_shl(6) as u32);
    let coeff = i32::from(p.pdzf.prev_note / 0xc) - 5;
    if coeff > 0 {
        f = f.wrapping_add(diff.wrapping_shl(coeff as u32) as u32);
    } else {
        f = f.wrapping_add((diff >> (-coeff).min(31)) as u32);
    }
    f
}

fn pdzf_extended_freq(p: &Part) -> u32 {
    pdzf_extended_freqdiff(p, i32::from(u16s16(p.detune)).wrapping_add(p.pdzf.pit.pitchdiff).wrapping_add(p.pdzf.lfodiff))
}

// 2fa2
fn init_lfo(l: &mut Lfo) {
    l.delay_cnt = l.delay;
    l.speed_cnt = 1;
    l.depth_cnt = l.depth >> 1;
    let mut rate2 = u16s16(l.rate);
    if l.waveform != LFOWF_TRIANGLE {
        rate2 = rate2.wrapping_mul(i16::from(l.depth));
        if l.waveform == LFOWF_STAIRCASE {
            rate2 >>= 1;
            l.depth_cnt = 1;
        }
    }
    if l.waveform == LFOWF_RANDOM {
        rate2 = 0;
    }
    l.rate2 = rate2 as u16;
}

// 2f84
fn init_lfo_pqr(p: &mut Part) {
    if p.lfo_f.p {
        init_lfo(&mut p.lfo[0]);
    }
    if p.lfo_f.q {
        init_lfo(&mut p.lfo[1]);
    }
    if p.lfo_f.r {
        init_lfo(&mut p.lfo[2]);
    }
}

// 2458
fn envreset_ssg(p: &mut Part) {
    for i in 0..6 {
        p.u[S_ENV + i] = p.u[S_ENVBAK + i];
    }
}

// 20b7
fn part_sync(p: &mut Part) {
    if p.sync != 1 {
        p.sync = 0;
    }
}

// 2e80-2ed6
fn ssg_env_adsr(p: &mut Part) {
    if !p.envf(F_DECAY) {
        let v = u32::from(p.actual_vol) + u32::from(p.env(E_ATTACK));
        if v & 0x100 == 0 && u32::from(p.u[S_VOL]) > v {
            p.actual_vol = v as u8;
            return;
        }
        p.actual_vol = p.u[S_VOL];
        p.set_envf(F_DECAY, true);
    }
    if !p.envf(F_SUSTAIN) {
        let v = u32::from(p.actual_vol).wrapping_sub(u32::from(p.env(E_DECAY)));
        if v & 0x100 == 0 && u32::from(p.env(E_SUSTAIN_LV)) < v {
            p.actual_vol = v as u8;
            return;
        }
        p.actual_vol = p.env(E_SUSTAIN_LV);
        p.set_envf(F_SUSTAIN, true);
    }
    if p.envf(F_RELEASE) {
        return;
    }
    let v = u32::from(p.actual_vol).wrapping_sub(u32::from(p.env(E_SUSTAIN_RATE)));
    if v & 0x100 == 0 {
        p.actual_vol = v as u8;
        return;
    }
    p.actual_vol = 0;
    p.set_envf(F_RELEASE, true);
    p.set_envf(F_ATTACK, false);
}

// 2f61
fn ssg_env_reset(p: &mut Part) {
    p.set_envf(F_DECAY, false);
    p.set_envf(F_SUSTAIN, false);
    p.set_envf(F_RELEASE, false);
    let mut vol = p.env(E_STARTVOL);
    if vol >= p.u[S_VOL] {
        p.set_envf(F_DECAY, true);
        vol = p.u[S_VOL];
    }
    p.actual_vol = vol;
    if p.env(E_SUSTAIN_LV) >= p.u[S_VOL] {
        p.set_envf(F_SUSTAIN, true);
    }
}

// 1cde
fn pit_end_ssg(p: &mut Part) {
    p.u[S_OCTAVE] = ssg_octave(p.pit.target_note);
    p.actual_freq = part_ssg_freq(p, p.pit.target_note);
    p.pit.rate = 0;
    p.status.pitchbend = false;
    p.prev_note = p.pit.target_note;
}

// 1cc4
fn pit_end_fm(p: &mut Part) {
    let d = [p.lfo[0].delay_cnt, p.lfo[1].delay_cnt, p.lfo[2].delay_cnt];
    init_lfo_pqr(p);
    for i in 0..3 {
        p.lfo[i].delay_cnt = d[i];
    }
    p.actual_freq = p.pit.target_freq;
    p.pit.rate = 0;
    p.status.pitchbend = false;
    p.prev_note = p.pit.target_note;
}

fn shr16(v: u16, s: u8) -> u16 {
    if s >= 16 {
        0
    } else {
        v >> s
    }
}

// 1c4b
fn part_pit(p: &mut Part) {
    if p.type_.adpcm {
        return;
    }
    p.pit.delay = p.pit.delay.wrapping_sub(1);
    if p.pit.delay != 0 {
        return;
    }
    p.pit.delay = 1;
    p.pit.speed_cnt = p.pit.speed_cnt.wrapping_sub(1);
    if p.pit.speed_cnt != 0 {
        return;
    }
    p.pit.speed_cnt = p.pit.speed;
    let rate = i16::from(p.pit.rate as i8) as u16;
    if rate == 0 {
        return;
    }
    let freq = p.actual_freq.wrapping_add(rate);
    if rate & 0x8000 == 0 {
        if !p.type_.fm {
            if shr16(freq, p.u[S_OCTAVE]) <= p.pit.target_freq {
                p.actual_freq = freq;
            } else {
                pit_end_ssg(p);
            }
        } else if freq < p.pit.target_freq {
            p.actual_freq = freq;
        } else {
            pit_end_fm(p);
        }
    } else if !p.type_.fm {
        if shr16(freq, p.u[S_OCTAVE]) >= p.pit.target_freq {
            p.actual_freq = freq;
        } else {
            pit_end_ssg(p);
        }
    } else if freq > p.pit.target_freq {
        p.actual_freq = freq;
    } else {
        pit_end_fm(p);
    }
}

fn pit_pdzf(p: &mut Part) {
    if p.pdzf.mode != 2 {
        return;
    }
    p.pdzf.pit.delay = p.pdzf.pit.delay.wrapping_sub(1);
    if p.pdzf.pit.delay != 0 {
        return;
    }
    p.pdzf.pit.delay = 1;
    p.pdzf.pit.speed_cnt = p.pdzf.pit.speed_cnt.wrapping_sub(1);
    if p.pdzf.pit.speed_cnt != 0 {
        return;
    }
    p.pdzf.pit.speed_cnt = p.pdzf.pit.speed;
    let rate = i32::from(u8s8(p.pdzf.pit.rate));
    if rate == 0 {
        return;
    }
    p.pdzf.pit.pitchdiff = p.pdzf.pit.pitchdiff.wrapping_add(rate);
    let f = pdzf_extended_freqdiff(p, p.pdzf.pit.pitchdiff);
    if (rate > 0 && f > p.pdzf.pit.target_freq) || (rate < 0 && f < p.pdzf.pit.target_freq) {
        p.pdzf.pit.on = false;
        p.pdzf.prev_note = p.pdzf.pit.target_note;
        p.pdzf.pit.pitchdiff = 0;
    }
}

impl Fmp {
    /// The FM voices the song carries: 25 bytes each, numbered from 0,
    /// from where they start to where the next of its data does.
    pub fn fm_voices(&self) -> Vec<(u8, Vec<u8>)> {
        let di = &self.datainfo;
        let start = u32::from(di.fmtoneptr);
        let end = di.partptr.iter().chain(&di.loopptr).chain([&di.ssgtoneptr, &di.adpcmptr]).map(|&p| u32::from(p)).filter(|&p| p > start && p != 0xffff).min().unwrap_or(u32::from(self.datalen)).min(u32::from(self.datalen));
        let d = |i: u32| self.data.get(i as usize).copied().unwrap_or(0);
        (0..(end.saturating_sub(start) / 25).min(256)).map(|k| (k as u8, (0..25).map(|i| d(start + k * 25 + i)).collect())).collect()
    }
}

impl Fmp {
    fn d(&self, i: u16) -> u8 {
        self.data.get(usize::from(i)).copied().unwrap_or(0)
    }

    fn set_d(&mut self, i: u16, v: u8) {
        if let Some(x) = self.data.get_mut(usize::from(i)) {
            *x = v;
        }
    }

    fn r16(&self, i: u16) -> u16 {
        u16::from(self.d(i)) | u16::from(self.d(i.wrapping_add(1))) << 8
    }

    fn rand71(&mut self) -> u8 {
        let v = self.rand71;
        self.rand71 = (v >> 1) | ((((v >> 7) ^ (v >> 5) ^ (v >> 4) ^ (v >> 3) ^ 1) & 1) << 7);
        self.rand71
    }

    fn cmdload(&mut self, p: usize) -> u8 {
        let ptr = self.parts[p].current_ptr;
        if ptr >= self.datalen {
            self.parts[p].current_ptr = 0xffff;
            return 0x74;
        }
        self.parts[p].current_ptr = ptr.wrapping_add(1);
        self.d(ptr)
    }

    fn cmdload_rhythm(&mut self, p: usize) -> u8 {
        let ptr = self.parts[p].current_ptr;
        if ptr >= self.datalen {
            self.parts[p].current_ptr = 0xffff;
            return 0x93;
        }
        self.parts[p].current_ptr = ptr.wrapping_add(1);
        self.d(ptr)
    }

    fn cmdload16(&mut self, p: usize) -> u16 {
        let lo = u16::from(self.cmdload(p));
        lo | u16::from(self.cmdload(p)) << 8
    }
}

fn fm_reg_write(w: &mut Work, p: &Part, addr: u8, data: u8) {
    let mut a = u32::from(p.opna_keyon_out & 3);
    if p.opna_keyon_out & 4 != 0 {
        a += 0x100;
    }
    w.writereg(a + u32::from(addr), u32::from(data));
}

fn pdzf_vol_update(w: &mut Work, p: &mut Part) {
    p.pdzf.vol = if p.type_.fm { (0x7f_u8.wrapping_sub(p.current_vol)) & 0xf } else { p.current_vol.wrapping_sub(1) };
    if p.pdzf.mode != 0 {
        let ev = if p.lfo_f.q { p.pdzf.env_vol as u8 } else { 0 };
        w.ppz8.volume(p.pdzf.ppz8_channel, pdzf_vol_clamp(p.pdzf.vol, ev));
    }
}

// 2362
fn ppz8_vol(w: &mut Work, p: &Part) {
    let v = p.current_vol.wrapping_sub(1).min(0xf);
    w.ppz8.volume(p.opna_keyon_out, v);
}

// 234d
fn ssg_vol(w: &mut Work, p: &mut Part) {
    pdzf_vol_update(w, p);
    if !p.envf(F_PPZ) {
        let vol = u32::from(p.current_vol);
        p.u[S_VOL] = if vol == 0 { 0 } else { ((vol.wrapping_sub(1) << 8) / vol) as u8 };
    } else {
        ppz8_vol(w, p);
    }
}

// 2229
fn write_pan(w: &mut Work, p: &Part) {
    fm_reg_write(w, p, OPNA_LRAMSPMS, p.pan_ams_pms);
}

// 2b1c-2b79
fn part_wlfo(w: &mut Work, p: &mut Part) {
    if p.u[W_DELAY] == 0 {
        return;
    }
    p.u[W_DELAY_CNT] = p.u[W_DELAY_CNT].wrapping_sub(1);
    if p.u[W_DELAY_CNT] != 0 {
        return;
    }
    p.u[W_DELAY_CNT] = 1;
    p.u[W_SPEED_CNT] = p.u[W_SPEED_CNT].wrapping_sub(1);
    if p.u[W_SPEED_CNT] != 0 {
        return;
    }
    p.u[W_SPEED_CNT] = p.u[W_SPEED];
    p.u[W_RATE_CURR] = p.u[W_RATE_CURR].wrapping_add(p.u[W_RATE]);
    for i in 0..4 {
        if p.u[W_SYNC] & (1 << i) == 0 {
            continue;
        }
        let mut tl = p.u[U_TONE_TL + i].wrapping_add(p.u[W_RATE_CURR]);
        if tl & 0x80 != 0 {
            tl = if p.u[W_RATE_CURR] & 0x80 != 0 { 0 } else { 0x7f };
        }
        fm_reg_write(w, p, OPNA_TL + 4 * i as u8, tl);
    }
    p.u[W_DEPTH_CNT] = p.u[W_DEPTH_CNT].wrapping_sub(1);
    if p.u[W_DEPTH_CNT] == 0 {
        p.u[W_DEPTH_CNT] = p.u[W_DEPTH];
        p.u[W_RATE] = p.u[W_RATE].wrapping_neg();
    }
}

// 2e0c
fn freq_ppz8(w: &mut Work, p: &Part) {
    let mut f = u32::from(p.actual_freq);
    let octave = p.u[S_OCTAVE];
    if octave == 0 {
        return;
    }
    if octave > 4 {
        f = f.wrapping_shl(u32::from(octave - 4));
    } else if octave < 4 {
        f >>= 4 - octave;
    }
    f = f.wrapping_add((i32::from(u16s16(p.detune)) << 6) as u32);
    w.ppz8.freq(p.opna_keyon_out, f);
}

// 30b1
fn init_wlfo(w: &mut Work, p: &mut Part) {
    if p.u[W_SYNC] & 0x80 == 0 {
        return;
    }
    for s in 0..4 {
        if p.u[W_SYNC] & (1 << s) != 0 {
            fm_reg_write(w, p, OPNA_TL + 4 * s as u8, p.u[U_TONE_TL + s]);
        }
    }
    p.u[W_DELAY_CNT] = p.u[W_DELAY];
    p.u[W_DEPTH_CNT] = p.u[W_DEPTH];
    p.u[W_RATE_CURR] = 0;
    p.u[W_RATE] = p.u[W_RATE_ORIG];
}

// 309b
fn part_hlfo(w: &mut Work, p: &mut Part) {
    w.writereg(0x22, u32::from(p.u[H_FREQ]));
    p.pan_ams_pms = (p.pan_ams_pms & 0xc0) | p.u[H_APMS];
    write_pan(w, p);
}

fn pdzf_env(w: &mut Work, p: &mut Part) {
    let ch = p.pdzf.ppz8_channel;
    match p.pdzf.env_status {
        PDZF_ENV_ATT => {
            let c = p.pdzf.env_cnt;
            p.pdzf.env_cnt = c.wrapping_sub(1);
            if c == 0 {
                p.pdzf.env_vol = p.pdzf.env_dd;
                p.pdzf.env_status = PDZF_ENV_DEC;
                if p.pdzf.env_vol > PDZF_ENV_VOL_MIN {
                    p.pdzf.env_cnt = p.pdzf.env_sr;
                    w.ppz8.volume(ch, pdzf_vol_clamp(p.pdzf.vol, p.pdzf.env_vol as u8));
                } else {
                    p.pdzf.env_vol = PDZF_ENV_VOL_MIN;
                    p.pdzf.env_status = PDZF_ENV_OFF;
                    w.ppz8.stop(ch);
                }
            }
        }
        PDZF_ENV_DEC => {
            if p.pdzf.env_sr != 0 {
                p.pdzf.env_cnt = p.pdzf.env_cnt.wrapping_sub(1);
                if p.pdzf.env_cnt == 0 {
                    p.pdzf.env_vol = p.pdzf.env_vol.wrapping_sub(1);
                    if p.pdzf.env_vol > PDZF_ENV_VOL_MIN {
                        p.pdzf.env_cnt = p.pdzf.env_sr;
                        w.ppz8.volume(ch, pdzf_vol_clamp(p.pdzf.vol, p.pdzf.env_vol as u8));
                    }
                }
            }
            if p.pdzf.env_vol <= PDZF_ENV_VOL_MIN {
                p.pdzf.env_vol = PDZF_ENV_VOL_MIN;
                p.pdzf.env_status = PDZF_ENV_OFF;
                w.ppz8.stop(ch);
            }
        }
        PDZF_ENV_REL => {
            p.pdzf.env_cnt = p.pdzf.env_cnt.wrapping_sub(1);
            if p.pdzf.env_cnt == 0 {
                p.pdzf.env_vol = p.pdzf.env_vol.wrapping_sub(1);
                if p.pdzf.env_vol > PDZF_ENV_VOL_MIN {
                    p.pdzf.env_cnt = p.pdzf.env_rr;
                    w.ppz8.volume(ch, pdzf_vol_clamp(p.pdzf.vol, p.pdzf.env_vol as u8));
                } else {
                    p.pdzf.env_vol = PDZF_ENV_VOL_MIN;
                    p.pdzf.env_status = PDZF_ENV_OFF;
                    w.ppz8.stop(ch);
                }
            }
        }
        _ => {}
    }
}

// 2e7a
fn ssg_env(w: &mut Work, p: &mut Part) {
    if p.envf(F_ATTACK) {
        ssg_env_adsr(p);
    } else {
        if !p.envf(F_PORTAMENTO) {
            return;
        }
        if !p.envf(F_RELEASE) {
            let v = u32::from(p.actual_vol).wrapping_sub(u32::from(p.env(E_RELEASE)));
            if v & 0x100 == 0 {
                p.actual_vol = v as u8;
            } else {
                p.actual_vol = 0;
                p.set_envf(F_RELEASE, true);
                p.set_envf(F_PORTAMENTO, false);
            }
        }
    }
    let out = ((u32::from(p.actual_vol) * u32::from(p.current_vol)) >> 8) as u8;
    if out == p.u[S_CURR_VOL] {
        return;
    }
    w.writereg(0x8 + u32::from(p.opna_keyon_out), u32::from(out));
}

// A part reading this many commands in one tick without a note is stuck;
// 98fmplayer would hang, so the part stops.
const STUCK: u32 = 0x10000;

#[derive(Clone, Copy, PartialEq)]
enum Table {
    Fm,
    Ssg,
    Adpcm,
}

// 29e7
fn fm_vol(w: &mut Work, p: &mut Part, fm_vol: u8, version: u8) {
    pdzf_vol_update(w, p);
    let mut pvol = p.actual_vol.wrapping_add(fm_vol);
    if pvol & 0x80 != 0 {
        pvol = if fm_vol & 0x80 != 0 { 0 } else { 0x7f };
    }
    for s in 0..4 {
        if p.slot_vol_mask & (1 << s) == 0 {
            continue;
        }
        let rel = p.u[U_SLOT_REL_VOL + s];
        let mut svol = pvol.wrapping_sub(rel);
        if svol & 0x80 != 0 {
            svol = if rel & 0x80 != 0 { 0x7f } else { 0 };
        }
        if version >= 6 {
            svol = svol.wrapping_add(p.u[U_TONE_TL + s]);
            if svol & 0x80 != 0 {
                svol = 0x7f;
            }
        }
        if p.u[U_SLOT_MASK] & (1 << s) == 0 {
            fm_reg_write(w, p, OPNA_TL + 4 * s as u8, svol);
        }
    }
}

impl Fmp {
    fn fm_vol(&mut self, w: &mut Work, p: usize) {
        let (v, ver) = (self.fm_vol, self.data_version);
        fm_vol(w, &mut self.parts[p], v, ver);
    }

    // 3273
    fn set_timer_ch3(&self, w: &mut Work) {
        w.writereg(0x27, u32::from(self.timer_ch3 | 0x2a));
    }

    // 326a
    fn set_tempo(&self, w: &mut Work) {
        w.writereg(0x26, u32::from(self.timerb));
        self.set_timer_ch3(w);
    }

    // 2979
    fn keyoff_fm(&mut self, w: &mut Work, p: usize) {
        let pt = self.parts[p];
        let mut out = pt.opna_keyon_out;
        if pt.type_.fm_3 {
            self.fm3_slot_keyon &= pt.u[U_SLOT_MASK] | 0x0f;
            out |= self.fm3_slot_keyon;
        }
        w.writereg(0x28, u32::from(out));
    }

    fn pdzf_keyoff(&mut self, w: &mut Work, p: usize) {
        let pt = &mut self.parts[p];
        if pt.pdzf.mode != 0 {
            pt.pdzf.keyon = false;
            if pt.lfo_f.q && pt.pdzf.env_rr != 0 {
                pt.pdzf.env_status = PDZF_ENV_REL;
                pt.pdzf.env_cnt = pt.pdzf.env_rr;
            } else {
                pt.pdzf.env_status = PDZF_ENV_OFF;
                w.ppz8.stop(pt.pdzf.ppz8_channel);
            }
        }
    }

    fn ssg_keyoff(p: &mut Part) {
        p.u[S_CURR_VOL] = 0xff;
        if p.env(E_RELEASE) == 0 {
            p.actual_vol = 0;
        }
        p.set_envf(F_PORTAMENTO, true);
        p.set_envf(F_ATTACK, false);
    }

    // 2961
    fn keyoff(&mut self, w: &mut Work, p: usize) {
        self.parts[p].ext_keyon = 0;
        self.parts[p].status.tie_cont = false;
        if self.parts[p].type_.adpcm {
            return;
        }
        if !self.parts[p].type_.ssg {
            self.keyoff_fm(w, p);
        } else {
            Fmp::ssg_keyoff(&mut self.parts[p]);
        }
        self.pdzf_keyoff(w, p);
    }

    // 2936
    fn keyoff_q(&mut self, w: &mut Work, p: usize) {
        if self.parts[p].type_.adpcm {
            self.parts[p].ext_keyon = 0;
            self.parts[p].status.tie_cont = false;
            w.writereg(0x100, 0x00);
            w.writereg(0x100, 0x01);
            return;
        }
        if self.parts[p].type_.fm || !self.parts[p].envf(F_PPZ) {
            self.parts[p].ext_keyon = 0;
            self.parts[p].status.tie_cont = false;
            if !self.parts[p].type_.ssg {
                self.keyoff_fm(w, p);
            } else {
                Fmp::ssg_keyoff(&mut self.parts[p]);
            }
        } else {
            w.ppz8.volume(self.parts[p].opna_keyon_out, 0);
        }
        self.pdzf_keyoff(w, p);
    }

    fn ssg_ppz8_pdzf_mode_update(&mut self, w: &mut Work, p: usize) {
        let (mix, mode) = (self.ssg_mix, self.pdzf_mode);
        let pt = &mut self.parts[p];
        let prev = pt.pdzf.mode;
        let mask = 9u32 << pt.opna_keyon_out;
        if u32::from(mix) & mask == mask {
            pt.set_envf(F_PPZ, true);
            pt.pdzf.mode = 0;
        } else {
            pt.set_envf(F_PPZ, false);
            if mode == 2 {
                pt.pdzf.mode = if pt.u[S_ENVBAK + E_STARTVOL] != 0 || pt.u[S_ENVBAK + E_ATTACK] != 0 { 0 } else { 2 };
            }
        }
        if prev != 0 && pt.pdzf.mode == 0 {
            pt.pdzf.keyon = false;
            w.ppz8.stop(pt.pdzf.ppz8_channel);
        }
    }

    fn fm_pdzf_mode_update(&mut self, w: &mut Work, p: usize) {
        let mode = self.pdzf_mode;
        let pt = &mut self.parts[p];
        let prev = pt.pdzf.mode;
        pt.pdzf.mode = if pt.u[U_SLOT_MASK] == 0xff { mode } else { 0 };
        if prev != 0 && pt.pdzf.mode == 0 {
            pt.pdzf.keyon = false;
            w.ppz8.stop(pt.pdzf.ppz8_channel);
        }
    }

    // 1fc8
    fn c62_tempo(&mut self, w: &mut Work, p: usize) -> bool {
        let t = self.cmdload(p);
        self.timerb_bak = t;
        self.timerb = t;
        w.timerb = t;
        self.set_tempo(w);
        true
    }

    // 2067
    fn c63_vol_d_fm(&mut self, w: &mut Work, p: usize) -> bool {
        let v = self.cmdload(p);
        self.parts[p].current_vol = v;
        self.parts[p].actual_vol = v;
        self.fm_vol(w, p);
        true
    }

    // 23a2
    fn c63_mix_ssg(&mut self, w: &mut Work, p: usize) -> bool {
        let v = self.cmdload(p);
        if v & 0x80 != 0 {
            self.ssg_mix &= v;
            let v = self.cmdload(p);
            self.ssg_mix |= v;
            self.ssg_ppz8_pdzf_mode_update(w, p);
            w.writereg(0x07, u32::from(self.ssg_mix));
        } else {
            self.cmdload(p);
            self.cmdload16(p);
        }
        true
    }

    // 2574
    fn c63_vol_d_adpcm(&mut self, w: &mut Work, p: usize) -> bool {
        let v = self.cmdload(p);
        self.parts[p].current_vol = v;
        self.parts[p].actual_vol = v;
        w.writereg(0x10b, u32::from(v));
        true
    }

    // 1fd9
    fn c64_loop(&mut self, _w: &mut Work, p: usize) -> bool {
        let lp = self.cmdload(p).wrapping_sub(1);
        if lp != 0 {
            let at = self.parts[p].current_ptr.wrapping_sub(1);
            self.set_d(at, lp);
            let diff = self.cmdload16(p);
            self.parts[p].current_ptr = self.parts[p].current_ptr.wrapping_sub(diff);
        } else {
            let ptr = self.parts[p].current_ptr.wrapping_add(3);
            self.parts[p].current_ptr = ptr;
            if ptr < self.datalen {
                self.set_d(ptr.wrapping_sub(4), self.d(ptr.wrapping_sub(1)));
            }
        }
        true
    }

    // 1fee
    fn c65_loopend(&mut self, _w: &mut Work, p: usize) -> bool {
        let diff = self.cmdload16(p);
        let at = self.parts[p].current_ptr.wrapping_add(diff).wrapping_sub(4);
        if at >= self.datalen {
            self.parts[p].current_ptr = 0xffff;
        } else if self.d(at) == 1 {
            let ptr = self.parts[p].current_ptr.wrapping_add(diff);
            self.parts[p].current_ptr = ptr;
            self.set_d(ptr.wrapping_sub(4), self.d(ptr.wrapping_sub(1)));
        }
        true
    }

    // 1fff
    fn c66_tie(&mut self, _w: &mut Work, p: usize) -> bool {
        self.parts[p].status.tie = true;
        self.parts[p].status.slur = false;
        true
    }

    // 200f
    fn c67_q(&mut self, _w: &mut Work, p: usize) -> bool {
        let v = self.cmdload(p);
        if self.datainfo.q {
            self.parts[p].gate_cnt = v;
        } else {
            self.parts[p].gate_cmp = v;
        }
        true
    }

    // 201e
    fn c68_pitchbend(&mut self, _w: &mut Work, p: usize) -> bool {
        let mut note = self.cmdload(p).wrapping_add(self.parts[p].note_diff);
        if note > 0x60 {
            note = 0;
        }
        let delay = self.cmdload(p);
        let speed = self.cmdload(p);
        let rate = self.cmdload(p);
        let pt = &mut self.parts[p];
        pt.pit.target_note = note;
        pt.pit.target_freq = if !pt.type_.ssg { fm_freq(note) } else { shr16(part_ssg_freq(pt, note), ssg_octave(note)) };
        pt.pit.delay = delay;
        pt.pit.speed = speed;
        pt.pit.speed_cnt = speed;
        pt.pdzf.pit.target_note = note;
        pt.pdzf.pit.target_freq = ppz8_note_freq(note);
        pt.pdzf.pit.delay = delay;
        pt.pdzf.pit.speed = speed;
        pt.pdzf.pit.speed_cnt = speed;
        pt.pdzf.pit.rate = rate;
        pt.pit.rate = if pt.type_.fm { rate } else { rate.wrapping_neg() };
        pt.status.pitchbend = true;
        pt.pdzf.pit.on = true;
        pt.pdzf.pit.pitchdiff = 0;
        true
    }

    // 25e3, and 205d on the ADPCM part
    fn c_skip1(&mut self, _w: &mut Work, p: usize) -> bool {
        self.cmdload(p);
        true
    }

    // 205d
    fn c69_vol_fm(&mut self, w: &mut Work, p: usize) -> bool {
        const T: [u8; 16] = [64, 59, 56, 52, 48, 42, 40, 37, 34, 32, 29, 26, 24, 21, 18, 16];
        let i = self.cmdload(p);
        let v = if i < 16 { T[usize::from(i)] } else { 0x7f };
        self.parts[p].current_vol = v;
        self.parts[p].actual_vol = v;
        self.fm_vol(w, p);
        true
    }

    // 2348
    fn c69_vol_ssg(&mut self, w: &mut Work, p: usize) -> bool {
        let mut v = self.cmdload(p).wrapping_add(1);
        if v == 0 {
            v = 1;
        }
        self.parts[p].current_vol = v;
        ssg_vol(w, &mut self.parts[p]);
        true
    }

    fn fm_relvol(&mut self, w: &mut Work, p: usize, vol: u8) {
        let mut v = self.parts[p].current_vol.wrapping_sub(vol);
        if v & 0x80 != 0 {
            v = if vol & 0x80 != 0 { 0x7f } else { 0 };
        }
        self.parts[p].current_vol = v;
        self.parts[p].actual_vol = v;
        self.fm_vol(w, p);
    }

    // 2080
    fn c6a_voldec_fm(&mut self, w: &mut Work, p: usize) -> bool {
        self.fm_relvol(w, p, 3u8.wrapping_neg());
        true
    }

    // 2086
    fn c6b_volinc_fm(&mut self, w: &mut Work, p: usize) -> bool {
        self.fm_relvol(w, p, 3);
        true
    }

    // 238a
    fn c6a_voldec_ssg(&mut self, w: &mut Work, p: usize) -> bool {
        let pt = &mut self.parts[p];
        if pt.current_vol > 1 {
            pt.current_vol -= 1;
        }
        if pt.current_vol == 0 {
            pt.current_vol = 1;
        }
        ssg_vol(w, pt);
        true
    }

    // 2396
    fn c6b_volinc_ssg(&mut self, w: &mut Work, p: usize) -> bool {
        let pt = &mut self.parts[p];
        if pt.current_vol < 0x10 {
            pt.current_vol += 1;
        }
        ssg_vol(w, pt);
        true
    }

    // 25f5
    fn adpcm_relvol(&mut self, w: &mut Work, p: usize, vol: u8) {
        let rel = i16::from(vol as i8) as u16;
        let mut v = u16::from(self.parts[p].current_vol).wrapping_add(rel);
        if v & 0x100 != 0 {
            v = if vol & 0x80 != 0 { 0 } else { 0xff };
        }
        self.parts[p].current_vol = v as u8;
        self.parts[p].actual_vol = v as u8;
        w.writereg(0x10b, u32::from(v as u8));
    }

    // 259e
    fn c6a_voldec_adpcm(&mut self, w: &mut Work, p: usize) -> bool {
        self.adpcm_relvol(w, p, 3u8.wrapping_neg());
        true
    }

    fn c6b_volinc_adpcm(&mut self, w: &mut Work, p: usize) -> bool {
        self.adpcm_relvol(w, p, 3);
        true
    }

    // 208c
    fn c6c_kondelay(&mut self, _w: &mut Work, p: usize) -> bool {
        self.parts[p].keyon_delay = self.cmdload(p);
        true
    }

    // 2090
    fn c6d_detune(&mut self, _w: &mut Work, p: usize) -> bool {
        self.parts[p].detune = i16::from(self.cmdload(p) as i8) as u16;
        true
    }

    // 2095
    fn c6e_poke(&mut self, w: &mut Work, p: usize) -> bool {
        let mut port = u32::from(self.cmdload(p));
        let data = self.cmdload(p);
        if self.parts[p].opna_keyon_out & 4 != 0 {
            port |= 0x100;
        }
        w.writereg(port, u32::from(data));
        true
    }

    // 209e
    fn c6f_sync(&mut self, _w: &mut Work, p: usize) -> bool {
        let v = self.cmdload(p);
        if v & 0x80 == 0 {
            for i in 0..3 {
                if v & (1 << i) != 0 {
                    part_sync(&mut self.parts[FM_1 + i]);
                }
            }
            for i in 0..3 {
                if v & (1 << (i + 3)) != 0 {
                    part_sync(&mut self.parts[SSG_1 + i]);
                }
            }
        } else {
            for i in 0..3 {
                if v & (1 << i) != 0 {
                    part_sync(&mut self.parts[FM_4 + i]);
                }
            }
            if v & 8 != 0 {
                part_sync(&mut self.parts[ADPCM]);
            }
            if v & 0x10 != 0 && self.rhythm.sync != 1 {
                self.rhythm.sync = 0;
            }
        }
        true
    }

    // 20eb
    fn c70_wait(&mut self, _w: &mut Work, p: usize) -> bool {
        self.parts[p].sync = 2;
        false
    }

    // 20f6
    fn c71_tone_fm(&mut self, w: &mut Work, p: usize) -> bool {
        let tone = self.cmdload(p);
        self.parts[p].tone = tone;
        for i in 0..4 {
            if self.parts[p].u[U_SLOT_MASK] & (1 << i) == 0 {
                fm_reg_write(w, &self.parts[p], OPNA_SLRR + 4 * i, 0x0f);
            }
        }
        for i in 0..4 {
            self.parts[p].u[U_SLOT_REL_VOL + i] = 0;
        }
        let t = self.datainfo.fmtoneptr.wrapping_add(u16::from(tone).wrapping_mul(25));
        for i in 0..4 {
            self.parts[p].u[U_TONE_TL + i] = self.d(t.wrapping_add(4 + i as u16));
        }
        for r in 0..6u16 {
            for s in 0..4u16 {
                if self.parts[p].u[U_SLOT_MASK] & (1 << s) != 0 {
                    continue;
                }
                let v = self.d(t.wrapping_add(r * 4 + s));
                fm_reg_write(w, &self.parts[p], OPNA_DTMUL + (r as u8) * 0x10 + 4 * s as u8, v);
            }
        }
        let mut fbalg = self.d(t.wrapping_add(0x18));
        let pt = self.parts[p];
        if pt.type_.fm_3 && pt.u[U_SLOT_MASK] & 1 != 0 {
            fbalg = (fbalg & 0x38) | (self.fm3_alg & 0x07);
        }
        fm_reg_write(w, &pt, OPNA_FBALG, fbalg);
        if pt.type_.fm_3 {
            self.fm3_alg = fbalg & 0x07;
        }
        const ALG_VOL: [u8; 8] = [0x8, 0x8, 0x8, 0x8, 0xc, 0xe, 0xe, 0xf];
        self.parts[p].slot_vol_mask = ALG_VOL[usize::from(fbalg & 7)];
        self.fm_vol(w, p);
        true
    }

    // 2458
    fn c71_envreset_ssg(&mut self, _w: &mut Work, p: usize) -> bool {
        self.cmdload(p);
        envreset_ssg(&mut self.parts[p]);
        true
    }

    // 25a8
    fn c71_tone_adpcm(&mut self, _w: &mut Work, p: usize) -> bool {
        let tone = self.cmdload(p);
        self.parts[p].tone = tone;
        if tone & 0x80 == 0 {
            let a = self.datainfo.adpcmptr.wrapping_add(6 * u16::from(tone));
            if u32::from(a) + 6 <= u32::from(self.datalen) {
                let (s, e, d) = (self.r16(a), self.r16(a + 2), self.r16(a + 4));
                let pt = &mut self.parts[p];
                pt.set_u16at(D_START, s);
                pt.set_u16at(D_END, e);
                pt.set_u16at(D_DELTAT, d);
            }
        } else {
            let t = usize::from(tone & 0x7f);
            let (s, e, d) = (self.adpcm_startaddr[t], self.adpcm_endaddr[t], self.adpcm_deltat);
            let pt = &mut self.parts[p];
            pt.set_u16at(D_START, s);
            pt.set_u16at(D_END, e);
            pt.set_u16at(D_DELTAT, d);
        }
        true
    }

    // 20f2
    fn c72_deflen(&mut self, _w: &mut Work, p: usize) -> bool {
        self.parts[p].default_len = self.cmdload(p);
        true
    }

    // 1fac
    fn c73_relvol_fm(&mut self, w: &mut Work, p: usize) -> bool {
        let v = self.cmdload(p);
        self.fm_relvol(w, p, v);
        true
    }

    // 2424
    fn c73_noise_ssg(&mut self, w: &mut Work, p: usize) -> bool {
        let v = self.cmdload(p);
        self.parts[p].set_envf(F_NOISE, true);
        if v & 0x80 != 0 {
            self.parts[p].set_envf(F_NOISE, false);
        } else {
            self.ssg_noise_freq = v;
            w.writereg(0x06, u32::from(v));
        }
        true
    }

    // 25f5
    fn c73_relvol_adpcm(&mut self, w: &mut Work, p: usize) -> bool {
        let v = self.cmdload(p);
        self.adpcm_relvol(w, p, v);
        true
    }

    // 2466
    fn c74_loop(&mut self, w: &mut Work, p: usize) -> bool {
        let bit = self.parts[p].part_bit;
        self.part_loop_bit &= !bit;
        if self.part_loop_bit == 0 {
            self.loop_dec = self.loop_dec.wrapping_sub(1);
            if self.loop_dec == 0 {
                self.looped = true;
            }
            self.loop_cnt = self.loop_cnt.wrapping_add(1);
            w.loop_cnt = self.loop_cnt;
            w.timerb_cnt_loop = 0;
            self.part_loop_bit = self.part_playing_bit;
        }
        let ptr = self.parts[p].loop_ptr;
        if ptr != 0xffff {
            self.parts[p].current_ptr = ptr;
            return true;
        }
        self.part_loop_bit &= !bit;
        self.part_playing_bit &= !bit;
        if self.part_playing_bit == 0 {
            self.stopped = true;
            self.looped = true;
            w.loop_cnt = 0xff;
        }
        if !self.parts[p].type_.rhythm {
            let pt = &mut self.parts[p];
            pt.prev_note = 0x61;
            pt.pdzf.prev_note = 0x61;
            pt.status.off = true;
            if pt.type_.ssg {
                pt.actual_vol = 0;
            }
            self.keyoff(w, p);
        } else {
            self.rhythm.status = true;
        }
        false
    }

    // 22e0
    fn c75_lfo(&mut self, w: &mut Work, p: usize) -> bool {
        let v = self.cmdload(p);
        if v & 0x80 != 0 {
            let set = v & 0x40 != 0;
            let pt = &mut self.parts[p];
            if v & 1 != 0 {
                pt.lfo_f.e = set;
            }
            if v & 2 != 0 {
                pt.lfo_f.w = set;
            }
            if v & 4 != 0 {
                pt.lfo_f.a = set;
            }
            if v & 8 != 0 {
                pt.lfo_f.r = set;
            }
            if v & 0x10 != 0 {
                pt.lfo_f.q = set;
            }
            if v & 0x20 != 0 {
                pt.lfo_f.p = set;
            }
            if !set {
                pt.pdzf.lfodiff = 0;
                pt.actual_freq = if !pt.type_.fm { part_ssg_freq(pt, pt.prev_note) } else { fm_freq(pt.prev_note) };
                pt.prev_freq = 0;
            } else if pt.lfo_f.e {
                pt.lfo_f.a = false;
            }
        } else {
            if v & 0x40 != 0 {
                self.timer_ch3 |= 0x40;
                if v & 0x20 != 0 {
                    for i in 0..4 {
                        self.ch3_se_freqdiff[i] = i16::from(u8s8(self.cmdload(p)));
                    }
                }
            } else {
                self.timer_ch3 &= 0x3f;
            }
            self.set_timer_ch3(w);
        }
        true
    }

    // 21a6
    fn cmd_lfo(&mut self, p: usize, n: usize) {
        let delay = self.cmdload(p);
        let speed = self.cmdload(p);
        let rate = i16::from(self.cmdload(p) as i8) as u16;
        let depth = self.cmdload(p);
        let waveform = self.cmdload(p);
        let pt = &mut self.parts[p];
        let l = &mut pt.lfo[n];
        l.delay = delay;
        l.speed = speed;
        l.rate = rate;
        l.rate2 = rate;
        l.depth = depth;
        l.waveform = waveform;
        if waveform == 6 {
            l.rate = (l.rate & 0xff).wrapping_mul(u16::from(depth));
        }
        pt.lfo_f.lfo = true;
        init_lfo_pqr(pt);
    }

    // 21a1
    fn c76_lfo_p(&mut self, _w: &mut Work, p: usize) -> bool {
        self.parts[p].lfo_f.p = true;
        self.cmd_lfo(p, 0);
        true
    }

    // 21d0
    fn c77_lfo_q(&mut self, _w: &mut Work, p: usize) -> bool {
        self.parts[p].lfo_f.q = true;
        self.cmd_lfo(p, 1);
        let pt = &mut self.parts[p];
        let l = pt.lfo[1];
        pt.pdzf.env_rr = l.delay.wrapping_sub(2);
        pt.pdzf.env_sr = l.speed;
        pt.pdzf.env_dd = u16s16(l.rate) as i8;
        pt.pdzf.env_al = l.depth;
        true
    }

    // 21da
    fn c78_lfo_r(&mut self, _w: &mut Work, p: usize) -> bool {
        self.parts[p].lfo_f.r = true;
        self.cmd_lfo(p, 2);
        let pt = &mut self.parts[p];
        let l = pt.lfo[2];
        if pt.pdzf.mode != 2 || l.depth == 0 {
            pt.pdzf.voice = l.speed;
            if pt.pdzf.mode == 2 && l.delay != 2 {
                let upan = l.delay.wrapping_sub(2);
                if upan < 10 {
                    pt.pdzf.pan = upan as i8 - 5;
                }
            } else {
                pt.pdzf.pan = (l.rate as u8 as i8).clamp(-4, 4);
            }
        }
        true
    }

    fn pdzf_loop_freq(&mut self, w: &mut Work, p: usize, d: [u8; 4]) {
        let pt = &mut self.parts[p];
        if pt.pdzf.mode == 0 {
            return;
        }
        if pt.pdzf.mode != 2 || pt.lfo[2].depth == 0 {
            let mut start = (u32::from(d[0]) << 8) | u32::from(d[1]);
            let mut end = (u32::from(d[2]) << 8) | u32::from(d[3]);
            if start == 0xffff {
                start = u32::MAX;
            }
            if end == 0xffff {
                end = u32::MAX;
            }
            if start != u32::MAX && end != u32::MAX && start >= end {
                start = u32::MAX;
                end = u32::MAX;
            }
            w.ppz8.loop_offset(pt.pdzf.ppz8_channel, start, end);
        }
        if pt.pdzf.mode == 2 && (pt.lfo[2].depth == 1 || pt.lfo[2].depth == 2) {
            let addr = u32::from_be_bytes(d);
            if pt.lfo[2].depth == 1 {
                pt.pdzf.loopstart32 = addr;
            } else {
                pt.pdzf.loopend32 = addr;
            }
            w.ppz8.loop_offset(pt.pdzf.ppz8_channel, pt.pdzf.loopstart32, pt.pdzf.loopend32);
        }
    }

    fn c79_lfo_a_fm(&mut self, w: &mut Work, p: usize) -> bool {
        self.parts[p].lfo_f.a = true;
        self.parts[p].lfo_f.e = false;
        let v = [self.cmdload(p), self.cmdload(p), self.cmdload(p), self.cmdload(p)];
        let u = &mut self.parts[p].u;
        u[A_DELAY] = v[0];
        u[A_DELAY_CNT] = v[0];
        u[A_SPEED] = v[1];
        u[A_SPEED_CNT] = v[1];
        u[A_RATE] = v[2];
        u[A_RATE_ORIG] = v[2];
        u[A_DEPTH] = v[3];
        u[A_DEPTH_CNT] = v[3];
        let d = [u[A_DELAY], u[A_SPEED], u[A_RATE], u[A_DEPTH]];
        self.pdzf_loop_freq(w, p, d);
        true
    }

    // 244f
    fn c79_env_ssg(&mut self, w: &mut Work, p: usize) -> bool {
        for i in 0..6 {
            let v = self.cmdload(p);
            self.parts[p].u[S_ENVBAK + i] = v;
        }
        envreset_ssg(&mut self.parts[p]);
        self.ssg_ppz8_pdzf_mode_update(w, p);
        let u = self.parts[p].u;
        self.pdzf_loop_freq(w, p, [u[S_ENVBAK + E_DECAY], u[S_ENVBAK + E_SUSTAIN_LV], u[S_ENVBAK + E_SUSTAIN_RATE], u[S_ENVBAK + E_RELEASE]]);
        true
    }

    // 2009
    fn c7a_tie(&mut self, _w: &mut Work, p: usize) -> bool {
        self.parts[p].status.tie = true;
        self.parts[p].status.slur = true;
        true
    }

    // 1f9e
    fn c7b_transpose(&mut self, _w: &mut Work, p: usize) -> bool {
        let v = self.cmdload(p);
        let pt = &mut self.parts[p];
        pt.note_diff = if v != 0 { pt.note_diff.wrapping_add(v) } else { 0 };
        true
    }

    // 2206
    fn c7c_lfo_pan_fm(&mut self, w: &mut Work, p: usize) -> bool {
        let v1 = self.cmdload(p);
        if v1 & 1 != 0 {
            let v2 = self.cmdload(p);
            let mut v3 = self.cmdload(p);
            if v3 & 0x80 != 0 {
                if v3 & 0x20 != 0 {
                    self.parts[p].u[H_DELAY] = v2;
                } else {
                    v3 = (v3 & 0x7f) >> 3;
                    let pt = &mut self.parts[p];
                    pt.u[H_FREQ] = (pt.u[H_FREQ] & 0xf7) | v3;
                    w.writereg(0x22, u32::from(pt.u[H_FREQ]));
                    if v3 == 0 {
                        pt.pan_ams_pms &= 0xc0;
                        write_pan(w, pt);
                    }
                }
            } else {
                let apms = self.cmdload(p);
                let pt = &mut self.parts[p];
                pt.u[H_DELAY] = v2;
                pt.u[H_FREQ] = v3 | 0x08;
                pt.u[H_APMS] = apms;
            }
        } else if v1 & 2 != 0 {
            let v = [self.cmdload(p), self.cmdload(p), self.cmdload(p), self.cmdload(p), self.cmdload(p)];
            let pt = &mut self.parts[p];
            pt.lfo_f.w = true;
            pt.u[W_DELAY] = v[0];
            pt.u[W_DELAY_CNT] = v[0];
            pt.u[W_SPEED] = v[1];
            pt.u[W_SPEED_CNT] = v[1];
            pt.u[W_RATE] = v[2];
            pt.u[W_RATE_ORIG] = v[2];
            pt.u[W_RATE_CURR] = 0;
            pt.u[W_DEPTH] = v[3];
            pt.u[W_DEPTH_CNT] = v[3];
            pt.u[W_SYNC] = v[4];
            let i = p as isize - FM_EX1 as isize;
            if i == 1 || i == 2 {
                let pr = &mut self.pdzf_rhythm[(i - 1) as usize];
                pr.voice = [v[0], v[1]];
                pr.pan = (v[2] as i8).clamp(-4, 4);
                pr.note = v[3];
                pr.enabled = true;
            }
        } else if v1 & 4 != 0 {
            let v = self.cmdload(p);
            self.timer_ch3 |= 0x40;
            if v == 0xff {
                self.keyoff(w, p);
                self.timer_ch3 &= 0x3f;
            }
            self.parts[p].u[U_SLOT_MASK] = v;
            self.fm_pdzf_mode_update(w, p);
        } else {
            self.parts[p].pan_ams_pms = (self.parts[p].pan_ams_pms & 0x3f) | v1;
            if self.parts[p].type_.fm_3 {
                for i in [FM_3, FM_EX1, FM_EX1 + 1, FM_EX3] {
                    self.parts[i].pan_ams_pms = (self.parts[i].pan_ams_pms & 0x3f) | v1;
                }
            }
            write_pan(w, &self.parts[p]);
        }
        true
    }

    // 1f78
    fn c7c_tone_ssg(&mut self, w: &mut Work, p: usize) -> bool {
        let tone = self.cmdload(p);
        self.parts[p].tone = tone;
        let t = self.datainfo.ssgtoneptr.wrapping_add(u16::from(tone).wrapping_mul(6));
        if self.datainfo.ssgtoneptr != 0xffff && u32::from(self.datalen) >= u32::from(t) + 6 {
            for i in 0..6 {
                let v = self.d(t + i as u16);
                self.parts[p].u[S_ENVBAK + i] = v;
            }
        }
        self.ssg_ppz8_pdzf_mode_update(w, p);
        let u = self.parts[p].u;
        self.pdzf_loop_freq(w, p, [u[S_ENVBAK + E_DECAY], u[S_ENVBAK + E_SUSTAIN_LV], u[S_ENVBAK + E_SUSTAIN_RATE], u[S_ENVBAK + E_RELEASE]]);
        envreset_ssg(&mut self.parts[p]);
        true
    }

    // 25e4
    fn adpcm_pan(&mut self, w: &mut Work, p: usize, v: u8) {
        let v = v & 0xc0;
        self.parts[p].pan_ams_pms = v;
        w.writereg(0x101, u32::from(v | self.adpcm_c1));
    }

    fn c7c_pan_adpcm(&mut self, w: &mut Work, p: usize) -> bool {
        let v = self.cmdload(p);
        self.adpcm_pan(w, p, v);
        true
    }

    // 1f70
    fn c7d_sync(&mut self, _w: &mut Work, p: usize) -> bool {
        self.sync_data = self.cmdload(p);
        self.sync_cnt = self.sync_cnt.wrapping_add(1);
        true
    }

    // 1f46
    fn c7e_loop_det(&mut self, _w: &mut Work, p: usize) -> bool {
        let v = self.cmdload(p);
        if v != 0 {
            self.loop_dec = v;
            self.loop_times = v;
        } else {
            let d = i16::from(u8s8(self.cmdload(p)));
            self.parts[p].detune = self.parts[p].detune.wrapping_add(d as u16);
        }
        true
    }

    // 1f2c
    fn c7f_lfo_delay(&mut self, _w: &mut Work, p: usize) -> bool {
        let v = self.cmdload(p);
        let delay = self.cmdload(p);
        if v < 3 {
            self.parts[p].lfo[usize::from(v)].delay = delay;
        } else if v == 3 {
            self.parts[p].u[A_DELAY] = delay;
        }
        true
    }

    // 1eaf
    fn update_slot_rel_vol(&mut self, w: &mut Work, p: usize) {
        let fmv = self.fm_vol;
        let pt = &self.parts[p];
        let mut pvol = pt.actual_vol.wrapping_add(fmv);
        if pvol & 0x80 != 0 {
            pvol = if fmv & 0x80 != 0 { 0 } else { 0x7f };
        }
        for s in 0..4 {
            let rel = pt.u[U_SLOT_REL_VOL + s];
            let svol;
            if pt.slot_vol_mask & (1 << s) == 0 {
                let mut v = pt.u[U_TONE_TL + s].wrapping_sub(rel);
                if v & 0x80 != 0 {
                    v = if rel & 0x80 != 0 { 0x7f } else { 0 };
                }
                svol = v;
            } else {
                let mut v = pvol.wrapping_sub(rel);
                if v & 0x80 != 0 {
                    v = if rel & 0x80 != 0 { 0x7f } else { 0 };
                }
                v = v.wrapping_add(pt.u[U_TONE_TL + s]);
                if v & 0x80 != 0 {
                    v = 0x7f;
                }
                svol = v;
            }
            if pt.u[U_SLOT_MASK] & (1 << s) == 0 {
                fm_reg_write(w, pt, OPNA_TL + 4 * s as u8, svol);
            }
        }
    }

    // 1e68
    fn ce2_slotrelvol_set_fm(&mut self, w: &mut Work, p: usize) -> bool {
        let v = self.cmdload(p);
        let mask = self.cmdload(p);
        for i in 0..4 {
            if mask & (1 << i) != 0 {
                self.parts[p].u[U_SLOT_REL_VOL + i] = v;
            }
        }
        self.update_slot_rel_vol(w, p);
        true
    }

    // 1e7d
    fn ce3_slotrelvol_add_fm(&mut self, w: &mut Work, p: usize) -> bool {
        let v = self.cmdload(p);
        let mask = self.cmdload(p);
        for i in 0..4 {
            if mask & (1 << i) == 0 {
                continue;
            }
            let mut s = i32::from(self.parts[p].u[U_SLOT_REL_VOL + i]) + i32::from(v);
            if s > 0x7f {
                s = if v & 0x80 != 0 { 0 } else { 0x7f };
            }
            self.parts[p].u[U_SLOT_REL_VOL + i] = s as u8;
        }
        self.update_slot_rel_vol(w, p);
        true
    }

    // 1d81
    fn cmd_exec(&mut self, w: &mut Work, p: usize, cmd: u8) -> bool {
        let t = if self.parts[p].type_.adpcm {
            Table::Adpcm
        } else if self.parts[p].type_.ssg {
            Table::Ssg
        } else {
            Table::Fm
        };
        use Table::*;
        match (cmd, t) {
            (0x62, _) => self.c62_tempo(w, p),
            (0x63, Fm) => self.c63_vol_d_fm(w, p),
            (0x63, Ssg) => self.c63_mix_ssg(w, p),
            (0x63, Adpcm) => self.c63_vol_d_adpcm(w, p),
            (0x64, _) => self.c64_loop(w, p),
            (0x65, _) => self.c65_loopend(w, p),
            (0x66, _) => self.c66_tie(w, p),
            (0x67, _) => self.c67_q(w, p),
            (0x68, Adpcm) => self.c_skip1(w, p),
            (0x68, _) => self.c68_pitchbend(w, p),
            (0x69, Fm) => self.c69_vol_fm(w, p),
            (0x69, Ssg) => self.c69_vol_ssg(w, p),
            (0x69, Adpcm) => self.c_skip1(w, p),
            (0x6a, Fm) => self.c6a_voldec_fm(w, p),
            (0x6a, Ssg) => self.c6a_voldec_ssg(w, p),
            (0x6a, Adpcm) => self.c6a_voldec_adpcm(w, p),
            (0x6b, Fm) => self.c6b_volinc_fm(w, p),
            (0x6b, Ssg) => self.c6b_volinc_ssg(w, p),
            (0x6b, Adpcm) => self.c6b_volinc_adpcm(w, p),
            (0x6c, _) => self.c6c_kondelay(w, p),
            (0x6d, _) => self.c6d_detune(w, p),
            (0x6e, _) => self.c6e_poke(w, p),
            (0x6f, _) => self.c6f_sync(w, p),
            (0x70, _) => self.c70_wait(w, p),
            (0x71, Fm) => self.c71_tone_fm(w, p),
            (0x71, Ssg) => self.c71_envreset_ssg(w, p),
            (0x71, Adpcm) => self.c71_tone_adpcm(w, p),
            (0x72, _) => self.c72_deflen(w, p),
            (0x73, Fm) => self.c73_relvol_fm(w, p),
            (0x73, Ssg) => self.c73_noise_ssg(w, p),
            (0x73, Adpcm) => self.c73_relvol_adpcm(w, p),
            (0x74, _) => self.c74_loop(w, p),
            (0x75, _) => self.c75_lfo(w, p),
            (0x76, _) => self.c76_lfo_p(w, p),
            (0x77, _) => self.c77_lfo_q(w, p),
            (0x78, _) => self.c78_lfo_r(w, p),
            (0x79, Ssg) => self.c79_env_ssg(w, p),
            (0x79, _) => self.c79_lfo_a_fm(w, p),
            (0x7a, _) => self.c7a_tie(w, p),
            (0x7b, _) => self.c7b_transpose(w, p),
            (0x7c, Fm) => self.c7c_lfo_pan_fm(w, p),
            (0x7c, Ssg) => self.c7c_tone_ssg(w, p),
            (0x7c, Adpcm) => self.c7c_pan_adpcm(w, p),
            (0x7d, _) => self.c7d_sync(w, p),
            (0x7e, _) => self.c7e_loop_det(w, p),
            _ => self.c7f_lfo_delay(w, p),
        }
    }

    fn cmd_exec2(&mut self, w: &mut Work, p: usize, cmd: u8) -> bool {
        if !self.parts[p].type_.fm || cmd > 0xe3 {
            self.parts[p].current_ptr = 0xffff;
            return false;
        }
        if cmd == 0xe2 {
            self.ce2_slotrelvol_set_fm(w, p)
        } else {
            self.ce3_slotrelvol_add_fm(w, p)
        }
    }

    // 2c5b
    fn lfo_calc(&mut self, p: usize, n: usize) {
        let mut waveform = self.parts[p].lfo[n].waveform;
        if waveform > 6 {
            waveform = 0;
        }
        let step = |l: &mut Lfo| -> bool {
            if l.delay == 0 {
                return false;
            }
            l.delay_cnt = l.delay_cnt.wrapping_sub(1);
            if l.delay_cnt != 0 {
                return false;
            }
            l.delay_cnt = 1;
            l.speed_cnt = l.speed_cnt.wrapping_sub(1);
            if l.speed_cnt != 0 {
                return false;
            }
            l.speed_cnt = l.speed;
            true
        };
        if waveform == LFOWF_RANDOM {
            let pt = &mut self.parts[p];
            if !step(&mut pt.lfo[n]) {
                return;
            }
            let r = self.rand71();
            let pt = &mut self.parts[p];
            let l = &mut pt.lfo[n];
            let m = if l.rate == 0 { 0 } else { ((u32::from(r) * u32::from(l.depth_cnt)) % u32::from(l.rate)) as u16 };
            let a = (l.rate >> 1).wrapping_sub(m);
            l.depth_cnt = r;
            let rate2 = l.rate2;
            l.rate2 = a;
            pt.actual_freq = pt.actual_freq.wrapping_sub(rate2).wrapping_add(a);
            if n == 0 {
                pt.pdzf.lfodiff = pt.pdzf.lfodiff.wrapping_sub(i32::from(u16s16(rate2))).wrapping_add(i32::from(u16s16(a)));
            }
            return;
        }
        let pt = &mut self.parts[p];
        let mut l = pt.lfo[n];
        let add = |pt: &mut Part, v: u16| {
            pt.actual_freq = pt.actual_freq.wrapping_add(v);
            if n == 0 {
                pt.pdzf.lfodiff = pt.pdzf.lfodiff.wrapping_add(i32::from(u16s16(v)));
            }
        };
        match waveform {
            LFOWF_TRIANGLE | LFOWF_TRIANGLE2 => {
                if step(&mut l) {
                    add(pt, l.rate2);
                    l.depth_cnt = l.depth_cnt.wrapping_sub(1);
                    if l.depth_cnt == 0 {
                        l.depth_cnt = l.depth;
                        l.rate2 = l.rate2.wrapping_neg();
                    }
                }
            }
            LFOWF_SAWTOOTH => {
                if step(&mut l) {
                    add(pt, l.rate);
                    l.depth_cnt = l.depth_cnt.wrapping_sub(1);
                    if l.depth_cnt == 0 {
                        l.depth_cnt = l.depth;
                        add(pt, l.rate2.wrapping_neg());
                    }
                }
            }
            LFOWF_SQUARE => {
                if step(&mut l) {
                    let v = (u16s16(l.rate2) >> u32::from(l.depth_cnt != 0)) as u16;
                    add(pt, v);
                    l.depth_cnt = 0;
                    l.rate2 = l.rate2.wrapping_neg();
                }
            }
            LFOWF_LINEAR => {
                if l.delay != 0 && l.depth_cnt != 0 && step(&mut l) {
                    add(pt, l.rate);
                    l.depth_cnt -= 1;
                }
            }
            _ => {
                if step(&mut l) {
                    add(pt, l.rate2);
                    l.depth_cnt = l.depth_cnt.wrapping_sub(1);
                    if l.depth_cnt == 0 {
                        l.depth_cnt = 2;
                        l.rate2 = l.rate2.wrapping_neg();
                    }
                }
            }
        }
        pt.lfo[n] = l;
    }

    // 27c9
    fn keyon_fm(&mut self, w: &mut Work, p: usize) {
        let pt = self.parts[p];
        if pt.lfo_f.mask {
            return;
        }
        let mut out = pt.opna_keyon_out;
        if pt.type_.fm_3 {
            self.fm3_slot_keyon |= !pt.u[U_SLOT_MASK] & 0xf0;
            out |= self.fm3_slot_keyon;
        } else {
            out |= 0xf0;
        }
        w.writereg(0x28, u32::from(out));
    }

    fn keyon_pdzf(&mut self, w: &mut Work, p: usize) {
        let pt = &mut self.parts[p];
        let voice = pt.pdzf.voice;
        let pan = (pt.pdzf.pan as u8).wrapping_add(5);
        pt.pdzf.pit.pitchdiff = 0;
        pt.pdzf.lfodiff = 0;
        let f = if pt.pdzf.mode == 2 {
            pdzf_extended_freq(pt)
        } else {
            ppz8_note_freq(pt.prev_note).wrapping_add((i32::from(u16s16(pt.detune)) << 3) as u32)
        };
        let ch = pt.pdzf.ppz8_channel;
        if !pt.pdzf.keyon {
            w.ppz8.play(ch, voice);
            pt.pdzf.env_status = PDZF_ENV_ATT;
            pt.pdzf.env_vol = 0;
            pt.pdzf.env_cnt = pt.pdzf.env_al;
            pt.pdzf.keyon = true;
        }
        w.ppz8.pan(ch, pan);
        w.ppz8.freq(ch, f);
        w.ppz8.volume(ch, pdzf_vol_clamp(pt.pdzf.vol, pt.pdzf.env_vol as u8));
        pt.pdzf.lastfreq = f;
    }

    // 27a3
    fn keyon(&mut self, w: &mut Work, p: usize) {
        self.parts[p].ext_keyon = 0xffff;
        self.parts[p].status.keyon = false;
        if self.parts[p].pdzf.mode != 0 {
            self.keyon_pdzf(w, p);
        }
        if !self.parts[p].type_.ssg {
            let pt = &mut self.parts[p];
            if pt.lfo_f.lfo {
                pt.actual_freq = fm_freq(pt.prev_note);
                pt.prev_freq = 0;
                pt.lfo_f.lfo = false;
            }
            self.keyon_fm(w, p);
        } else {
            let pt = &mut self.parts[p];
            if pt.lfo_f.lfo {
                let note = pt.prev_note;
                pt.u[S_OCTAVE] = ssg_octave(note);
                pt.actual_freq = part_ssg_freq(pt, note);
                pt.prev_freq = 0;
                pt.lfo_f.lfo = false;
            }
            if pt.lfo_f.mask {
                pt.set_envf(F_ATTACK, false);
                pt.set_envf(F_PORTAMENTO, true);
                return;
            }
            if pt.envf(F_PPZ) {
                w.ppz8.play(pt.opna_keyon_out, pt.tone);
                ppz8_vol(w, pt);
                freq_ppz8(w, pt);
                return;
            }
            pt.set_envf(F_ATTACK, true);
        }
    }

    // 2fdc
    fn init_lfo_awe(&mut self, w: &mut Work, p: usize) {
        let (fmv, ver) = (self.fm_vol, self.data_version);
        let pt = &mut self.parts[p];
        if pt.lfo_f.w {
            init_wlfo(w, pt);
        }
        if pt.lfo_f.a || pt.lfo_f.e || pt.current_vol != pt.actual_vol {
            pt.actual_vol = pt.current_vol;
            let mut pvol = pt.actual_vol.wrapping_add(fmv);
            if pvol & 0x80 != 0 {
                pvol = if fmv & 0x80 != 0 { 0 } else { 0x7f };
            }
            for s in 0..4 {
                let rel = pt.u[U_SLOT_REL_VOL + s];
                let v;
                if pt.slot_vol_mask & (1 << s) == 0 {
                    let mut x = pt.u[U_TONE_TL + s].wrapping_sub(rel);
                    if x & 0x80 != 0 {
                        x = if rel & 0x80 != 0 { 0x7f } else { 0 };
                    }
                    v = x;
                } else {
                    let mut x = pvol;
                    if ver >= 6 {
                        x = x.wrapping_sub(rel);
                        if x & 0x80 != 0 {
                            x = if rel & 0x80 != 0 { 0x7f } else { 0 };
                        }
                        x = x.wrapping_add(pt.u[U_TONE_TL + s]);
                        if x & 0x80 != 0 {
                            x = 0x7f;
                        }
                    }
                    v = x;
                }
                if pt.u[U_SLOT_MASK] & (1 << s) == 0 {
                    fm_reg_write(w, pt, OPNA_TL + 4 * s as u8, v);
                }
            }
        }
        pt.u[A_DELAY_CNT] = pt.u[A_DELAY];
        pt.u[A_DEPTH_CNT] = pt.u[A_DEPTH];
        pt.u[A_SPEED_CNT] = pt.u[A_SPEED];
        pt.u[A_RATE] = pt.u[A_RATE_ORIG];
        if pt.u[H_FREQ] & 0x80 != 0 {
            w.writereg(0x22, 0);
            pt.u[H_DELAY_CNT] = pt.u[H_DELAY];
            if pt.u[H_DELAY_CNT] == 0 {
                part_hlfo(w, pt);
            }
        }
    }

    // 2ad9
    fn part_lfo(&mut self, w: &mut Work, p: usize) {
        for n in 0..3 {
            let f = self.parts[p].lfo_f;
            if [f.p, f.q, f.r][n] {
                self.lfo_calc(p, n);
            }
        }
        if !self.parts[p].type_.fm {
            return;
        }
        let pt = &mut self.parts[p];
        if pt.u[H_FREQ] & 0x04 != 0 && pt.u[H_DELAY_CNT] != 0 {
            pt.u[H_DELAY_CNT] -= 1;
            if pt.u[H_DELAY_CNT] == 0 {
                part_hlfo(w, pt);
            }
        }
        if pt.lfo_f.w {
            part_wlfo(w, pt);
        }
        if !(pt.lfo_f.a || pt.lfo_f.e) || pt.u[A_DELAY] == 0 {
            return;
        }
        pt.u[A_DELAY_CNT] = pt.u[A_DELAY_CNT].wrapping_sub(1);
        if pt.u[A_DELAY_CNT] != 0 {
            return;
        }
        pt.u[A_DELAY_CNT] = 1;
        pt.u[A_SPEED_CNT] = pt.u[A_SPEED_CNT].wrapping_sub(1);
        if pt.u[A_SPEED_CNT] != 0 {
            return;
        }
        pt.u[A_SPEED_CNT] = pt.u[A_SPEED];
        let rate = pt.u[A_RATE];
        let mut vol = pt.actual_vol.wrapping_add(rate);
        if vol & 0x80 != 0 {
            vol = if rate & 0x80 != 0 { 0 } else { 0x7f };
        }
        pt.actual_vol = vol;
        if !pt.lfo_f.e {
            self.fm_vol(w, p);
            let pt = &mut self.parts[p];
            pt.u[A_DEPTH_CNT] = pt.u[A_DEPTH_CNT].wrapping_sub(1);
            if pt.u[A_DEPTH_CNT] != 0 {
                return;
            }
            pt.u[A_DEPTH_CNT] = pt.u[A_DEPTH];
            pt.u[A_RATE] = pt.u[A_RATE].wrapping_neg();
        } else {
            self.keyoff_fm(w, p);
            self.fm_vol(w, p);
            self.keyon_fm(w, p);
            let pt = &mut self.parts[p];
            pt.u[A_SPEED_CNT] = pt.u[A_DEPTH_CNT];
            pt.u[A_DEPTH_CNT] = if pt.u[A_SPEED] == pt.u[A_SPEED_CNT] { pt.u[A_DEPTH] } else { pt.u[A_SPEED] };
        }
    }

    // 2a51
    fn part_fm(&mut self, w: &mut Work, p: usize) {
        if self.parts[p].status.lfo_sync {
            init_lfo_pqr(&mut self.parts[p]);
            self.init_lfo_awe(w, p);
            let pt = &mut self.parts[p];
            pt.actual_freq = fm_freq(pt.prev_note);
            pt.status.lfo_sync = false;
            pt.pdzf.lfodiff = 0;
        }
        let fix = self.datainfo.lfo_octave_fix;
        let pt = &mut self.parts[p];
        if pt.status.pitchbend || fix {
            let fnum = pt.actual_freq & 0x7ff;
            if fnum > 0x4d3 {
                pt.actual_freq = pt.actual_freq.wrapping_add(0x800).wrapping_sub(0x269);
            } else if fnum < 0x26a {
                pt.actual_freq = pt.actual_freq.wrapping_sub(0x800).wrapping_add(0x269);
                if pt.actual_freq & 0x8000 != 0 {
                    pt.actual_freq = 0x200;
                }
            }
        }
        let f = pt.actual_freq.wrapping_add(pt.detune);
        if f != pt.prev_freq {
            pt.prev_freq = f;
            if pt.type_.fm_3 && self.timer_ch3 & 0x40 != 0 {
                const ADDR: [u32; 4] = [0xad, 0xac, 0xae, 0xa6];
                const IDX: [usize; 4] = [0, 2, 1, 3];
                for i in 0..4 {
                    if pt.u[U_SLOT_MASK] & (1 << i) != 0 {
                        continue;
                    }
                    let f = pt.prev_freq.wrapping_add(self.ch3_se_freqdiff[IDX[i]] as u16);
                    w.writereg(ADDR[i], u32::from(f >> 8));
                    w.writereg(ADDR[i] - 4, u32::from(f & 0xff));
                }
            } else {
                fm_reg_write(w, pt, OPNA_BLKFNUM2, (pt.prev_freq >> 8) as u8);
                fm_reg_write(w, pt, OPNA_FNUM1, pt.prev_freq as u8);
            }
        }
        if self.parts[p].status.keyon {
            self.keyon(w, p);
        }
        self.part_lfo(w, p);
    }

    // 2db0
    fn part_ssg(&mut self, w: &mut Work, p: usize) {
        let pt = &mut self.parts[p];
        if pt.status.lfo_sync {
            init_lfo_pqr(pt);
            pt.u[S_OCTAVE] = ssg_octave(pt.prev_note);
            pt.actual_freq = part_ssg_freq(pt, pt.prev_note);
            pt.status.lfo_sync = false;
            pt.pdzf.lfodiff = 0;
        }
        if self.parts[p].status.keyon {
            self.keyon(w, p);
        }
        if self.parts[p].envf(F_PPZ) {
            return;
        }
        ssg_env(w, &mut self.parts[p]);
        let pt = self.parts[p];
        let f = shr16(pt.actual_freq, pt.u[S_OCTAVE]);
        if f == pt.prev_freq {
            if pt.status.keyon {
                self.keyon(w, p);
            }
        } else {
            self.parts[p].prev_freq = f;
            if pt.eff_chan == 0 {
                let reg = u32::from(pt.opna_keyon_out) * 2;
                w.writereg(reg, u32::from(f & 0xff));
                w.writereg(reg | 1, u32::from(f >> 8));
            }
        }
        self.part_lfo(w, p);
    }

    // 28b3
    fn adpcm_addr_set(&mut self, w: &mut Work, p: usize) {
        let pt = &self.parts[p];
        if pt.lfo_f.mask {
            return;
        }
        w.writereg(0x100, 0x00);
        w.writereg(0x100, 0x01);
        w.writereg(0x101, u32::from(pt.pan_ams_pms | self.adpcm_c1));
        let (s, e) = (pt.u16at(D_START), pt.u16at(D_END));
        w.writereg(0x102, u32::from(s & 0xff));
        w.writereg(0x103, u32::from(s >> 8));
        w.writereg(0x104, u32::from(e & 0xff));
        w.writereg(0x105, u32::from(e >> 8));
    }

    // 2853
    fn part_adpcm(&mut self, w: &mut Work, p: usize) {
        if !self.parts[p].status.keyon {
            return;
        }
        self.parts[p].ext_keyon = 0xffff;
        self.parts[p].status.keyon = false;
        if self.parts[p].status.tie_cont {
            return;
        }
        self.adpcm_addr_set(w, p);
        let pt = &mut self.parts[p];
        pt.actual_freq = adpcm_freq(pt.prev_note);
        // As FMP has it, the detune's bytes swapped.
        let f = pt.actual_freq.wrapping_add(pt.detune.swap_bytes()).wrapping_add(pt.u16at(D_DELTAT));
        w.writereg(0x109, u32::from(f & 0xff));
        w.writereg(0x10a, u32::from(f >> 8));
        w.writereg(0x100, 0xa0);
    }

    // 2750
    fn keyon_pre(&mut self, p: usize) {
        let q = self.datainfo.q;
        let pt = &mut self.parts[p];
        if q {
            let mut tonelen = pt.tonelen_cnt;
            let shift = if tonelen < 0x10 { 3 } else { 0 };
            tonelen <<= shift;
            tonelen >>= 3;
            tonelen = tonelen.wrapping_mul(pt.gate_cnt);
            tonelen >>= shift;
            pt.gate_cmp = pt.tonelen_cnt.wrapping_sub(tonelen);
        }
        pt.status.keyon = true;
        if pt.type_.ssg && !pt.status.tie_cont && !pt.envf(F_PPZ) {
            pt.set_envf(F_PORTAMENTO, false);
            if !pt.lfo_f.mask {
                ssg_env_reset(pt);
            }
        }
    }

    // 1c0d
    fn part_cmd(&mut self, w: &mut Work, p: usize) {
        if self.parts[p].sync > 1 {
            return;
        }
        if self.parts[p].sync == 1 {
            if self.parts[p].keyon_delay_cnt != 0 {
                self.parts[p].keyon_delay_cnt -= 1;
                if self.parts[p].keyon_delay_cnt == 0 {
                    self.keyon_pre(p);
                }
            }
            let pt = self.parts[p];
            if !pt.status.tie && !pt.status.rest && pt.tonelen_cnt != 0 && pt.tonelen_cnt == pt.gate_cmp {
                self.keyoff_q(w, p);
            }
            if self.parts[p].status.pitchbend {
                part_pit(&mut self.parts[p]);
            }
            if self.parts[p].pdzf.pit.on {
                pit_pdzf(&mut self.parts[p]);
            }
            self.parts[p].tonelen_cnt = self.parts[p].tonelen_cnt.wrapping_sub(1);
            if self.parts[p].tonelen_cnt != 0 {
                return;
            }
            if !self.parts[p].status.tie {
                self.keyoff(w, p);
                self.parts[p].status.pitchbend = false;
                self.parts[p].pdzf.pit.on = false;
            } else {
                let st = &mut self.parts[p].status;
                st.slur = false;
                st.tie = false;
                st.tie_cont = true;
            }
        }
        self.parts[p].sync = 1;
        for _ in 0..STUCK {
            let mut cmd = self.cmdload(p);
            let len;
            if cmd & 0x80 == 0 {
                if cmd >= 0x62 {
                    if !self.cmd_exec(w, p, cmd) {
                        return;
                    }
                    continue;
                }
                len = self.cmdload(p);
            } else {
                cmd &= 0x7f;
                if cmd >= 0x62 {
                    if !self.cmd_exec2(w, p, cmd | 0x80) {
                        return;
                    }
                    continue;
                }
                len = self.parts[p].default_len;
            }
            self.parts[p].tonelen_cnt = len;
            self.parts[p].tonelen = len;
            self.parts[p].notes = self.parts[p].notes.wrapping_add(1);
            if cmd == 0x61 {
                let pt = &mut self.parts[p];
                pt.status.tie = false;
                pt.status.pitchbend = false;
                pt.pdzf.pit.on = false;
                pt.status.tie_cont = false;
                pt.status.rest = true;
                self.keyoff(w, p);
            } else {
                let cmd = cmd.wrapping_add(self.parts[p].note_diff);
                self.parts[p].status.rest = false;
                self.parts[p].keyon_delay_cnt = self.parts[p].keyon_delay;
                if self.parts[p].keyon_delay == 0 {
                    self.keyon_pre(p);
                }
                if !self.parts[p].status.tie_cont || cmd != self.parts[p].prev_note {
                    self.parts[p].status.lfo_sync = true;
                    self.parts[p].status.tie_cont = false;
                    if self.parts[p].status.slur {
                        self.keyoff(w, p);
                    }
                }
                self.parts[p].prev_note = cmd;
                self.parts[p].pdzf.prev_note = cmd;
            }
            return;
        }
        self.parts[p].status.off = true;
    }

    // 269b
    fn pan_vol_rhythm(&self, w: &mut Work, i: usize) {
        w.writereg(0x18 + i as u32, u32::from(self.rhythm.pans[i] | self.rhythm.volumes[i]));
    }

    // 2619
    fn part_cmd_rhythm(&mut self, w: &mut Work) {
        let p = RHYTHM;
        if self.rhythm.sync > 1 {
            return;
        }
        if self.rhythm.sync == 1 {
            self.rhythm.len_cnt = self.rhythm.len_cnt.wrapping_sub(1);
            if self.rhythm.len_cnt != 0 {
                return;
            }
        }
        for _ in 0..STUCK {
            self.rhythm.sync = 1;
            let cmd = self.cmdload_rhythm(p);
            if cmd & 0x80 != 0 {
                let i = usize::from(cmd & 7);
                if cmd & 0x40 != 0 {
                    self.rhythm.tl_volume = cmd & 0x3f;
                    w.writereg(0x11, u32::from(self.rhythm.tl_volume));
                } else if cmd & 0x20 != 0 {
                    // Only six sounds: 98fmplayer would write past them.
                    if i < 6 {
                        self.rhythm.pans[i] = (cmd & 0x18) << 3;
                        self.pan_vol_rhythm(w, i);
                    }
                } else if cmd == 0x90 {
                    self.rhythm.default_len = self.cmdload(p);
                } else if cmd == 0x91 {
                    self.c64_loop(w, p);
                } else if cmd == 0x92 {
                    self.c65_loopend(w, p);
                } else if cmd == 0x93 {
                    if !self.c74_loop(w, p) {
                        return;
                    }
                } else if cmd == 0x94 {
                    if self.rhythm.tl_volume < 0x3f {
                        self.rhythm.tl_volume += 1;
                        w.writereg(0x11, u32::from(self.rhythm.tl_volume));
                    }
                } else if cmd == 0x95 {
                    if self.rhythm.tl_volume != 0 {
                        self.rhythm.tl_volume -= 1;
                        w.writereg(0x11, u32::from(self.rhythm.tl_volume));
                    }
                } else {
                    match cmd & 0xf8 {
                        0x88 => {
                            if i < 6 {
                                let mut v = self.rhythm.volumes[i].wrapping_add(1);
                                if v & 0xe0 != 0 {
                                    v = 0x1f;
                                }
                                self.rhythm.volumes[i] = v;
                                self.pan_vol_rhythm(w, i);
                            }
                        }
                        0x98 => {
                            if i < 6 {
                                let mut v = self.rhythm.volumes[i].wrapping_sub(1);
                                if v & 0xe0 != 0 {
                                    v = 0;
                                }
                                self.rhythm.volumes[i] = v;
                                self.pan_vol_rhythm(w, i);
                            }
                        }
                        _ => {
                            let v = self.cmdload(p);
                            if i < 6 {
                                self.rhythm.volumes[i] = v;
                                self.pan_vol_rhythm(w, i);
                            }
                        }
                    }
                }
            } else {
                w.writereg(0x10, u32::from(cmd & self.rhythm.mask));
                self.rhythm_hits = self.rhythm_hits.wrapping_add(1);
                self.rhythm_last = cmd & 0x3f;
                if self.pdzf_mode == 2 {
                    let keyon = (cmd & 0x3f) & self.rhythm.mask;
                    let hit = (0..6).find(|&k| keyon & (1 << k) != 0 && self.rhythm.pans[k] == 0 && self.pdzf_rhythm[k & 1].enabled);
                    if let Some(k) = hit {
                        let r = self.pdzf_rhythm[k & 1];
                        let volume = self.rhythm.volumes[k] & 0x0f;
                        let voice = r.voice[usize::from((self.rhythm.volumes[k] & 0x10) >> 4)];
                        let pan = (r.pan as u8).wrapping_add(5);
                        let f = ppz8_note_freq(r.note);
                        self.pdzf_rhythm_current_note = r.note;
                        w.ppz8.play(6, voice);
                        w.ppz8.volume(6, volume);
                        w.ppz8.pan(6, pan);
                        w.ppz8.freq(6, f);
                    }
                }
                self.rhythm.len_cnt = if cmd & 0x40 != 0 { self.rhythm.default_len } else { self.cmdload(p) };
                return;
            }
        }
        self.rhythm.status = true;
    }

    fn pdzf_freq_update(&mut self, w: &mut Work, p: usize) {
        let pt = &mut self.parts[p];
        let f = pdzf_extended_freq(pt);
        if f != pt.pdzf.lastfreq {
            pt.pdzf.lastfreq = f;
            w.ppz8.freq(pt.pdzf.ppz8_channel, f);
        }
    }

    // 17f8-1903
    fn timerb(&mut self, w: &mut Work) {
        self.set_tempo(w);
        if self.stopped {
            w.playing = false;
        }
        self.clock_divider = self.clock_divider.wrapping_sub(1);
        if self.clock_divider == 0 {
            self.total_clocks = self.total_clocks.wrapping_add(1);
            self.clock_divider = 10;
        }
        for p in FM_1..FM_1 + 6 {
            if self.parts[p].status.off {
                continue;
            }
            self.part_cmd(w, p);
            self.part_fm(w, p);
        }
        if !self.parts[ADPCM].status.off {
            self.part_cmd(w, ADPCM);
            self.part_adpcm(w, ADPCM);
        }
        for p in SSG_1..SSG_1 + 3 {
            if self.parts[p].status.off {
                continue;
            }
            self.part_cmd(w, p);
            self.part_ssg(w, p);
            if self.parts[p].pdzf.mode == 2 {
                self.pdzf_freq_update(w, p);
            }
            if self.parts[p].pdzf.mode != 0 && self.parts[p].lfo_f.q {
                pdzf_env(w, &mut self.parts[p]);
            }
        }
        for p in FM_EX1..=FM_EX3 {
            if self.parts[p].status.off {
                continue;
            }
            self.part_cmd(w, p);
            if self.parts[p].status.off {
                continue;
            }
            self.part_fm(w, p);
            if self.parts[p].pdzf.mode == 2 {
                self.pdzf_freq_update(w, p);
            }
            if self.parts[p].pdzf.mode != 0 && self.parts[p].lfo_f.q {
                pdzf_env(w, &mut self.parts[p]);
            }
        }
        if !self.rhythm.status {
            self.part_cmd_rhythm(w);
        }
    }

    /// The OPNA's interrupt (98fmplayer's fmp_opna_interrupt).
    pub fn interrupt(&mut self, w: &mut Work) {
        if w.status(false) & 2 != 0 {
            self.timerb(w);
            if w.playing {
                w.timerb_cnt = w.timerb_cnt.wrapping_add(1);
                w.timerb_cnt_loop = w.timerb_cnt_loop.wrapping_add(1);
            }
        }
    }

    fn init_parts(&mut self, w: &mut Work) {
        let deflen = self.datainfo.bar >> 2;
        for i in 0..3 {
            let f = &mut self.parts[FM_1 + i];
            f.default_len = deflen;
            f.gate_cmp = 1;
            f.gate_cnt = 8;
            f.current_vol = 0x1a;
            f.actual_vol = 0x1a;
            let s = &mut self.parts[SSG_1 + i];
            s.default_len = deflen;
            s.gate_cmp = 1;
            s.gate_cnt = 8;
            for base in [S_ENV, S_ENVBAK] {
                s.u[base + E_STARTVOL] = 0xff;
                s.u[base + E_ATTACK] = 0xff;
                s.u[base + E_DECAY] = 0xff;
                s.u[base + E_SUSTAIN_LV] = 0xff;
                s.u[base + E_RELEASE] = 0x0a;
            }
            s.current_vol = 0x0e;
        }
        for i in 0..4 {
            let pt = &mut self.parts[FM_4 + i];
            pt.default_len = deflen;
            pt.gate_cmp = 1;
            pt.gate_cnt = 8;
            pt.current_vol = 0x1a;
            pt.actual_vol = 0x1a;
        }
        for p in [ADPCM, FM_EX1, FM_EX1 + 1, FM_EX3] {
            let pt = &mut self.parts[p];
            pt.default_len = deflen;
            pt.gate_cmp = 1;
            pt.gate_cnt = 8;
            pt.current_vol = 0x1a;
            pt.actual_vol = 0x1a;
            if p != ADPCM {
                pt.u[U_SLOT_MASK] = 0xff;
                pt.pan_ams_pms = 0xc0;
            }
        }
        self.rhythm.status = false;
        self.rhythm.len_cnt = 1;
        self.rhythm.default_len = deflen;
        self.rhythm.mask = 0x3f;
        self.rhythm.tl_volume = 0x3c;
        self.rhythm.volumes = [0x1c; 6];
        self.rhythm.pans = [0xc0; 6];
        w.writereg(0x11, u32::from(self.rhythm.tl_volume));
        for i in 0..6 {
            self.pan_vol_rhythm(w, i);
        }
        let pt = &mut self.parts[ADPCM];
        pt.current_vol = 0x80;
        pt.actual_vol = 0x80;
        pt.gate_cmp = 0;
        w.writereg(0x10b, 0x80);
        self.adpcm_pan(w, ADPCM, 0xc0);
        self.timerb = 0xca;
        w.timerb = self.timerb;
        self.timerb_bak = 0xca;
        let di = self.datainfo;
        for i in 0..6 {
            self.parts[FM_1 + i].current_ptr = di.partptr[i];
            self.parts[FM_1 + i].loop_ptr = di.loopptr[i];
        }
        for i in 0..3 {
            self.parts[SSG_1 + i].current_ptr = di.partptr[6 + i];
            self.parts[SSG_1 + i].loop_ptr = di.loopptr[6 + i];
        }
        self.parts[RHYTHM].current_ptr = di.partptr[9];
        self.parts[RHYTHM].loop_ptr = di.loopptr[9];
        self.parts[ADPCM].current_ptr = di.partptr[10];
        self.parts[ADPCM].loop_ptr = di.loopptr[10];
        for i in 0..3 {
            self.parts[FM_EX1 + i].current_ptr = di.partptr[11 + i];
            self.parts[FM_EX1 + i].loop_ptr = di.loopptr[11 + i];
        }
        self.part_playing_bit = 0x07ff;
        self.part_loop_bit = 0x07ff;
        self.looped = false;
        self.stopped = false;
        self.total_clocks = 0;
        self.clock_divider = 10;
        for p in [FM_EX1, FM_EX1 + 1, FM_EX3] {
            self.fm_pdzf_mode_update(w, p);
        }
        for p in [SSG_1, SSG_1 + 1, SSG_1 + 2] {
            self.ssg_ppz8_pdzf_mode_update(w, p);
        }
    }

    fn struct_init(&mut self, w: &mut Work) {
        self.ssg_mix = 0x38;
        w.writereg(0x07, u32::from(self.ssg_mix));
        w.writereg(0x29, 0x83);
        w.writereg(0x10, 0xbf);
        w.writereg(0x100, 0x21);
        w.writereg(0x101, 0x02);
        self.adpcm_c1 = 0x02;
        // The OPNA's FM is 6 dB below the OPN's: FMP raises it by 8.
        self.fm_vol = 8u8.wrapping_neg();
        for i in 0..3 {
            let pt = &mut self.parts[SSG_1 + i];
            pt.type_.ssg = true;
            pt.opna_keyon_out = i as u8;
            pt.part_bit = 0x40 << i;
        }
        const OUT: [u8; 6] = [0, 1, 2, 4, 5, 6];
        for i in 0..6 {
            let pt = &mut self.parts[FM_1 + i];
            pt.type_.fm = true;
            pt.opna_keyon_out = OUT[i];
            pt.part_bit = 1 << i;
        }
        self.parts[FM_3].type_.fm_3 = true;
        self.parts[ADPCM].type_.adpcm = true;
        self.parts[ADPCM].part_bit = 0x400;
        for i in 0..3 {
            let pt = &mut self.parts[FM_EX1 + i];
            pt.type_.fm = true;
            pt.type_.fm_3 = true;
            pt.opna_keyon_out = 2;
            pt.part_bit = 0x800 << i;
        }
        self.parts[RHYTHM].type_.rhythm = true;
        self.parts[RHYTHM].part_bit = 0x200;
        w.writereg(0x27, 0x30);
        for (i, p) in [SSG_1, SSG_1 + 1, SSG_1 + 2, FM_EX1, FM_EX1 + 1, FM_EX3].into_iter().enumerate() {
            let pt = &mut self.parts[p];
            pt.pdzf.ppz8_channel = i as u8;
            pt.pdzf.loopstart32 = u32::MAX;
            pt.pdzf.loopend32 = u32::MAX;
        }
        for i in 0..8 {
            self.parts[PPZ8_1 + i].status.off = true;
        }
    }

    fn title(&mut self, offset: u16) {
        let mut l = 0;
        let mut line = Vec::new();
        let mut si: u32 = 0;
        loop {
            let at = u32::from(offset) + si;
            si += 1;
            if at >= u32::from(self.datalen) || line.len() >= 256 {
                return;
            }
            let c = self.d(at as u16);
            if l >= 3 {
                if c != 0 {
                    self.pdzf_mode = 1;
                }
                return;
            }
            match c {
                b'\r' => {}
                b'\n' => {
                    self.comment[l] = std::mem::take(&mut line);
                    l += 1;
                }
                0 => {
                    self.comment[l] = line;
                    return;
                }
                _ => line.push(c),
            }
        }
    }

    /// The song, or None when it isn't one FMP can play.
    pub fn load(song: &[u8]) -> Option<Fmp> {
        let data = song[..song.len().min(0xffff)].to_vec();
        let datalen = data.len() as u16;
        let d = |i: u16| data.get(usize::from(i)).copied().unwrap_or(0);
        let r16 = |i: u16| u16::from(d(i)) | u16::from(d(i.wrapping_add(1))) << 8;
        let offset = r16(0);
        if u32::from(offset) + 4 > u32::from(datalen) {
            return None;
        }
        let mut di = DataInfo::default();
        let data_version;
        let dataflags;
        let pviname_valid;
        let id = [d(offset), d(offset + 1), d(offset + 2)];
        let read = |di: &mut DataInfo, parts: &[(usize, u16)], loops: &[(usize, u16)]| {
            di.partptr = [0xffff; DATA_NUM];
            di.loopptr = [0xffff; DATA_NUM];
            for &(i, at) in parts {
                di.partptr[i] = r16(at);
            }
            for &(i, at) in loops {
                di.loopptr[i] = r16(at);
            }
        };
        let seq = |first: usize, n: usize, at: u16| (0..n).map(move |k| (first + k, at + 2 * k as u16));
        if &id == b"FMC" {
            let ver = d(offset + 3);
            data_version = ver & 0x0f;
            if ver <= 0x29 {
                if datalen < 0x1a {
                    return None;
                }
                let parts: Vec<_> = seq(0, 3, 0x02).chain(seq(6, 3, 0x08)).collect();
                let loops: Vec<_> = seq(0, 3, 0x0e).chain(seq(6, 3, 0x14)).collect();
                read(&mut di, &parts, &loops);
                di.bar = d(0x1a);
                dataflags = d(0x1b);
                di.adpcmptr = 0xffff;
                di.fmtoneptr = 0x1c;
                pviname_valid = false;
            } else if ver <= 0x49 {
                if datalen < 0x2e {
                    return None;
                }
                let parts: Vec<_> = seq(0, 11, 0x02).collect();
                let loops: Vec<_> = seq(0, 11, 0x18).collect();
                read(&mut di, &parts, &loops);
                di.bar = d(0x2e);
                dataflags = d(0x2f);
                di.adpcmptr = r16(0x30);
                di.fmtoneptr = 0x32;
                pviname_valid = true;
            } else if ver <= 0x69 {
                if datalen < 0x5e {
                    return None;
                }
                let parts: Vec<_> = seq(0, 14, 0x02).collect();
                let loops: Vec<_> = seq(0, 14, 0x30).collect();
                read(&mut di, &parts, &loops);
                di.bar = d(0x5e);
                dataflags = d(0x5f);
                di.adpcmptr = r16(0x60);
                di.fmtoneptr = 0x66;
                pviname_valid = true;
            } else {
                return None;
            }
        } else if &id == b"ELF" {
            if datalen < 0x2a {
                return None;
            }
            data_version = 0x07;
            let parts: Vec<_> = seq(0, 10, 0x02).collect();
            let loops: Vec<_> = seq(0, 10, 0x16).collect();
            read(&mut di, &parts, &loops);
            di.bar = d(0x2a);
            dataflags = d(0x2b);
            di.adpcmptr = 0xffff;
            di.fmtoneptr = 0x2e;
            pviname_valid = false;
        } else {
            return None;
        }
        di.q = dataflags & 1 != 0;
        di.ppz = dataflags & 2 != 0;
        di.lfo_octave_fix = dataflags & 4 != 0;
        let ptr = r16(0).wrapping_sub(2);
        di.ssgtoneptr = if u32::from(ptr) + 2 > u32::from(datalen) { 0xffff } else { r16(ptr) };
        let name = |at: u16| {
            let b: Vec<u8> = (0..8).map(|i| d(at + i)).take_while(|&c| c != 0).collect();
            String::from_utf8_lossy(&b).trim_end().to_string()
        };
        let pcmptr = r16(0).wrapping_sub(0x12);
        let (mut pvi_name, mut ppz_name) = (String::new(), String::new());
        if pcmptr <= datalen && u32::from(pcmptr) + 16 < u32::from(datalen) && pviname_valid {
            pvi_name = name(pcmptr + 8);
            if di.ppz {
                ppz_name = name(pcmptr);
            }
        }
        Some(Fmp {
            data,
            datalen,
            timerb: 0,
            sync_data: 0,
            sync_cnt: 0,
            looped: false,
            stopped: false,
            total_clocks: 0,
            clock_divider: 0,
            fm_vol: 0,
            loop_cnt: 0,
            timerb_bak: 0,
            ssg_mix: 0,
            timer_ch3: 0,
            ssg_noise_freq: 0,
            loop_dec: 0,
            loop_times: 0,
            data_version,
            fm3_slot_keyon: 0,
            fm3_alg: 0,
            part_playing_bit: 0,
            part_loop_bit: 0,
            ch3_se_freqdiff: [0; 4],
            adpcm_deltat: 0,
            adpcm_c1: 0,
            adpcm_startaddr: [0; 0x80],
            adpcm_endaddr: [0; 0x80],
            ppz_name,
            pvi_name,
            parts: [Part::default(); PART_NUM + 1],
            rhythm: Rhythm::default(),
            datainfo: di,
            rand71: 0,
            pdzf_mode: 0,
            pdzf_rhythm: [PdzfRhythm::default(); 2],
            pdzf_rhythm_current_note: 0,
            rhythm_hits: 0,
            rhythm_last: 0,
            comment: [Vec::new(), Vec::new(), Vec::new()],
        })
    }

    /// The driver's start on the chips (98fmplayer's fmp_init).
    pub fn init(&mut self, w: &mut Work) {
        self.title(self.r16(0).wrapping_add(4));
        if self.comment.iter().any(|l| l.windows(10).any(|x| x == b"using PDZF")) {
            self.pdzf_mode = 2;
        }
        self.struct_init(w);
        self.init_parts(w);
        self.set_tempo(w);
        w.playing = true;
    }

    /// A .PVI file (the ADPCM's voices) into the OPNA's RAM; false when it
    /// isn't one.
    pub fn load_pvi(&mut self, w: &mut Work, pvi: &[u8]) -> bool {
        if pvi.len() < 0x210 || pvi.len() > 0x210 + (1 << 18) {
            return false;
        }
        let r = |o: usize| u16::from(pvi[o]) | u16::from(pvi[o + 1]) << 8;
        self.adpcm_deltat = r(0x8);
        for i in 0..0x80 {
            self.adpcm_startaddr[i] = r(0x10 + i * 4);
            self.adpcm_endaddr[i] = r(0x10 + i * 4 + 2);
        }
        for (a, v) in [
            (0x100, 0x01),
            (0x110, 0x13),
            (0x110, 0x80),
            (0x100, 0x60),
            (0x101, u32::from(self.adpcm_c1)),
            (0x102, 0),
            (0x103, 0),
            (0x104, 0xff),
            (0x105, 0xff),
            (0x10c, 0xff),
            (0x10d, 0xff),
        ] {
            w.writereg(a, v);
        }
        for &b in &pvi[0x210..] {
            w.writereg(0x108, u32::from(b));
        }
        w.writereg(0x110, 0x0c);
        w.writereg(0x100, 0x01);
        true
    }

    /// A part's count of notes and rests read (wrapping), its note (0xff a
    /// rest) and whether it's tied on from the one before.
    pub fn note_event(&self, p: usize) -> (u8, u8, bool) {
        let pt = &self.parts[p];
        (pt.notes, if pt.status.rest || pt.status.off { 0xff } else { pt.prev_note }, pt.status.tie_cont)
    }

    /// The rhythm sounds struck: a count (wrapping) and the last (a bit each).
    pub fn rhythm_hit(&self) -> (u8, u8) {
        (self.rhythm_hits, self.rhythm_last)
    }

    /// Ticks a bar, and timer B's value.
    pub fn bar(&self) -> (u8, u8) {
        (self.datainfo.bar, self.timerb)
    }

    /// Whether a part is playing its song (for a host showing which are).
    pub fn part_playing(&self, p: usize) -> bool {
        self.parts.get(p).is_some_and(|pt| !pt.status.off)
    }
}

impl crate::fmwork::Interrupt for Fmp {
    fn interrupt(&mut self, w: &mut Work) {
        Fmp::interrupt(self, w);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 98fmplayer's own render (fmwav: 55,467 Hz 16-bit stereo, the SSG as
    /// the YMF288 sums it, or with FM_ANALOG as the OPNA's analog output): FMP_DIR the songs, FMP_REF the renders
    /// (<name>.raw), the first FMP_SECS seconds (10).
    #[test]
    #[ignore]
    fn the_same_as_98fmplayer() {
        let (Ok(dir), Ok(rf)) = (std::env::var("FMP_DIR"), std::env::var("FMP_REF")) else { return };
        let secs: usize = std::env::var("FMP_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(10);
        let only = std::env::var("FMP_ONLY").ok();
        let mut paths: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).collect();
        paths.sort();
        let (mut same, mut total) = (0, 0);
        for path in paths {
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            if only.as_ref().is_some_and(|o| !name.contains(o.as_str())) {
                continue;
            }
            let Ok(want) = std::fs::read(format!("{rf}/{name}.raw")) else { continue };
            total += 1;
            let Some(mut fmp) = Fmp::load(&std::fs::read(&path).unwrap()) else {
                eprintln!("{name}: not read");
                continue;
            };
            let mut w = Work::new();
            fmp.init(&mut w);
            // Its PCM files, when they're beside it (as 98fmplayer loads them).
            let beside = |n: &str| (!n.is_empty()).then(|| std::fs::read(path.with_file_name(format!("{n}.PVI"))).ok()).flatten();
            if let Some(b) = beside(&fmp.pvi_name.clone()) {
                fmp.load_pvi(&mut w, &b);
            }
            if let Some(b) = beside(&fmp.ppz_name.clone()) {
                w.ppz8.load_pvi(0, &b);
            }
            if std::env::var("FM_ANALOG").is_ok() {
                w.opna.ssg.set_ymf288(false);
            }
            let n = secs * crate::opna::RATE as usize;
            let mut buf = vec![0i16; n * 2];
            let mut at = 0;
            while at < n {
                let k = (n - at).min(4096);
                crate::fmwork::mix(&mut w, &mut fmp, &mut buf[at * 2..], k);
                at += k;
            }
            let want: Vec<i16> = want.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]])).collect();
            let m = want.len().min(buf.len());
            match (0..m).find(|&i| want[i] != buf[i]) {
                None => same += 1,
                Some(i) => eprintln!("{name}: differs at frame {} ({} vs {})", i / 2, buf[i], want[i]),
            }
        }
        eprintln!("{same}/{total} the same");
        assert_eq!(same, total);
    }
}
