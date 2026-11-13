// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.
//
// Ported from 98fmplayer's PMD driver (github.com/takamichih/fmplayer),
// under its BSD 2-Clause License; the notice is in `opna.rs`.

//! PMD (Professional Music Driver, M. Kajihara), the PC-98's most used
//! sound driver, playing its compiled songs (.M, .M2, .MZ) on the OPNA:
//! six FM parts (FM 3's operators also as three more parts of their own),
//! three SSG parts with software envelopes, the rhythm part (driving the
//! OPNA's rhythm sounds, or the SSG drum effects), ADPCM and the eight
//! PPZ8 parts. A port of 98fmplayer's, which is a reimplementation of
//! PMD 4.8's own code.

use crate::fmwork::{u16s16, u8s8, Work};
use crate::pmd_ssgeff::SSGEFF;

const OLD_AL: u8 = 0;
const OLD_SR: u8 = 1;
const OLD_RR: u8 = 2;
const OLD_OFF: u8 = 3;
const OLD_NEW: u8 = 0xff;

const NEW_OFF: u8 = 0;
const NEW_AR: u8 = 1;
const NEW_DR: u8 = 2;
const NEW_SR: u8 = 3;
const NEW_RR: u8 = 4;

const P_NEW_AR: usize = 0;
const P_NEW_DR: usize = 1;
const P_NEW_SR: usize = 2;
const P_NEW_RR: usize = 3;

const P_OLD_AL: usize = 0;
const P_OLD_AD: usize = 1;
const P_OLD_SR: usize = 2;
const P_OLD_RR: usize = 3;

const WF_TRIANGLE1: u8 = 0;
const WF_SAWTOOTH: u8 = 1;
const WF_SQUARE: u8 = 2;
const WF_RANDOM: u8 = 3;
const WF_TRIANGLE2: u8 = 4;
const WF_TRIANGLE3: u8 = 5;
const WF_ONESHOT: u8 = 6;

pub const FM_1: usize = 0;
pub const FM_3: usize = 2;
pub const FM_4: usize = 3;
pub const FM_6: usize = 5;
pub const SSG_1: usize = 6;
pub const SSG_3: usize = 8;
pub const ADPCM: usize = 9;
pub const RHYTHM: usize = 10;
pub const FM_3B: usize = 11;
pub const FM_3D: usize = 13;
pub const PPZ_1: usize = 14;
pub const PPZ_8: usize = 21;
const PART_NUM: usize = 23;

#[derive(Clone, Copy, Default)]
struct Lfo {
    delay: u8,
    speed: u8,
    step: u8,
    times: u8,
}

#[derive(Clone, Copy, Default)]
struct LfoFlags {
    freq: bool,
    vol: bool,
    sync: bool,
    portamento: bool,
}

#[derive(Clone, Copy, Default)]
struct FlagExt {
    detune: bool,
    lfo: bool,
    env: bool,
}

#[derive(Clone, Copy, Default)]
struct Loop {
    looped: bool,
    ended: bool,
    env: bool,
}

#[derive(Clone, Copy, Default)]
struct Mask {
    ext: bool,
    effect: bool,
    disabled: bool,
    slot: bool,
    mml: bool,
    ff: bool,
}

#[derive(Clone, Copy, Default)]
struct KeyStatus {
    off: bool,
    off_mask: bool,
}

#[derive(Clone, Copy, Default)]
pub struct Part {
    ptr: u16,
    loop_ptr: u16,
    len_cnt: u8,
    gate: u8,
    actual_freq: u16,
    detune: i16,
    lfo_diff: i16,
    portamento_diff: i16,
    portamento_add: i16,
    portamento_rem: i16,
    vol: u8,
    transpose: u8,
    lfo: Lfo,
    lfo_set: Lfo,
    lfof: LfoFlags,
    lfof_b: LfoFlags,
    volume_save: u8,
    md_depth: u8,
    md_speed: u8,
    md_speed_set: u8,
    ssg_env_state_old: u8,
    ssg_env_state_new: u8,
    ssg_env_param_set: [u8; 4],
    ssg_env_param_sl: u8,
    ssg_env_param_al: u8,
    ssg_env_param: [u8; 4],
    ssg_env_vol: u8,
    flagext: FlagExt,
    flagext_b: FlagExt,
    pan: u8,
    ssg_mix: u8,
    tonenum: u8,
    loop_: Loop,
    fm_slotout: u8,
    fm_tone_tl: [u8; 4],
    fm_slotmask: u8,
    fm_tone_slotmask: u8,
    lfo_waveform: u8,
    mask: Mask,
    keystatus: KeyStatus,
    vol_lfo_slotmask: u8,
    gate_abs: u8,
    gate_rel: u8,
    hlfo_delay_set: u8,
    hlfo_delay: u8,
    lfo_diff_b: i16,
    lfo_b: Lfo,
    lfo_set_b: Lfo,
    md_depth_b: u8,
    md_speed_b: u8,
    md_speed_set_b: u8,
    lfo_waveform_b: u8,
    vol_lfo_slotmask_b: u8,
    md_cnt: u8,
    md_cnt_set: u8,
    md_cnt_b: u8,
    md_cnt_set_b: u8,
    pub actual_note: u8,
    slot_delay: u8,
    slot_delay_cnt: u8,
    slot_delay_mask: u8,
    fb_alg: u8,
    note_proc: u8,
    gate_min: u8,
    actual_freq_upper: u16,
    curr_note: u8,
    transpose_master: u8,
    gate_rand_range: u8,
    proc_masked: bool,
    len: u8,
    output_freq: u32,
}

/// A PMD song being played.
#[derive(Clone)]
pub struct Pmd {
    // The song from its second byte on; the first is `opm_byte`.
    data: Vec<u8>,
    datalen: u16,
    opm_byte: u8,
    ssgeff_data: &'static [crate::pmd_ssgeff::Eff],
    ssgeff_pos: usize,
    ssgeff_tonefreq: u16,
    ssgeff_tonefreq_add: i16,
    ssgeff_wait: u8,
    ssgeff_noisefreq: u8,
    ssgeff_noisefreq_add: i8,
    ssgeff_noisefreq_cnt_set: u8,
    ssgeff_noisefreq_cnt: u8,
    ssgeff_priority: u8,
    ssgeff_num: u8,
    ssgeff_internal: bool,
    rand: u16,
    proc_ch: u8,
    no_keyoff: bool,
    volume_saved: bool,
    opna_a1: bool,
    fm_slotkey: [u8; 6],
    loop_: Loop,
    ppsdrv_enabled: bool,
    adpcm_start_loop: u16,
    adpcm_stop_loop: u16,
    adpcm_release: u16,
    timera_cnt_b: u8,
    fm3ex_force: bool,
    fm3ex_needed: u8,
    fm3ex_fb_alg: u8,
    tonemask_fb_alg: bool,
    lfoprocf: LfoFlags,
    lfoprocf_b: LfoFlags,
    tone_ptr: u16,
    r_offset: u16,
    r_ptr: u16,
    opnarhythm_mask: u8,
    fm_voldown: u8,
    ssg_voldown: u8,
    adpcm_voldown: u8,
    opnarhythm_voldown: u8,
    tone_included: bool,
    opm_flag: bool,
    status1: u8,
    status2: u8,
    timerb: u8,
    fadeout_speed: u8,
    fadeout_vol: u8,
    timerb_bak: u8,
    meas_len: u8,
    tick_cnt: u8,
    timera_cnt: u8,
    ssgeff_disabled: bool,
    ssg_noise_freq: u8,
    ssg_noise_freq_wrote: u8,
    fmeff_playing: bool,
    adpcm_start: u16,
    adpcm_stop: u16,
    opnarhythm: u8,
    opnarhythm_ilpan: [u8; 6],
    opnarhythm_tl: u8,
    ssgrhythm: u16,
    playing: bool,
    paused: bool,
    ssgrhythm_opna: bool,
    adpcm_disabled: bool,
    fm3ex_det: [i16; 4],
    timerb_wrote: u8,
    faded_out: bool,
    pcm86_vol_spb: bool,
    meas_cnt: u16,
    opna_last22: u8,
    tempo: u8,
    tempo_bak: u8,
    fm_voldown_orig: u8,
    ssg_voldown_orig: u8,
    adpcm_voldown_orig: u8,
    opnarhythm_voldown_orig: u8,
    pcm86_vol_spb_orig: bool,
    opnarhythm_shot_inc: [u8; 6],
    opnarhythm_dump_inc: [u8; 6],
    fm3ex_mode: u8,
    ppz8_voldown: u8,
    ppz8_voldown_orig: u8,
    pub parts: [Part; PART_NUM],
    adpcm_addr: [[u16; 2]; 256],
    ssgeff_tone_mix: bool,
    ssgeff_noise_mix: bool,
    ssgeff_on: bool,
}

impl Pmd {
    /// The FM voices the song carries (when it carries them): each its
    /// number and 25 bytes, the first of a number only (as the driver
    /// finds them), up to the data's end or a number FF.
    pub fn fm_voices(&self) -> Vec<(u8, Vec<u8>)> {
        let mut out: Vec<(u8, Vec<u8>)> = Vec::new();
        if self.datalen < 0x1a || self.d(0) == 0x18 {
            return out;
        }
        let mut t = u32::from(self.r16(0x18));
        while t + 0x1a <= u32::from(self.datalen) {
            let n = self.d(t as u16);
            if n == 0xFF {
                break;
            }
            if !out.iter().any(|(k, _)| *k == n) {
                out.push((n, (1..0x1a).map(|i| self.d((t + i) as u16)).collect()));
            }
            t += 0x1a;
        }
        out
    }

    /// The song was made for the OPM (an X68000's PMD) rather than the OPNA.
    pub fn opm(&self) -> bool {
        self.opm_byte != 0
    }

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

    // 3447
    fn reg_write(&self, w: &mut Work, addr: u8, data: u8) {
        w.writereg(u32::from(addr) | if self.opna_a1 { 0x100 } else { 0 }, u32::from(data));
    }

    // 1058
    fn reset_state(&mut self) {
        self.fadeout_vol = 0;
        self.fadeout_speed = 0;
        self.faded_out = false;
        for p in self.parts.iter_mut() {
            p.actual_note = 0xff;
            p.curr_note = 0xff;
        }
        for p in &mut self.parts[PPZ_1..=PPZ_8] {
            p.loop_.ended = true;
        }
        for p in &mut self.parts[FM_3B..=FM_3D] {
            p.loop_.ended = true;
        }
        self.no_keyoff = false;
        self.status1 = 0;
        self.status2 = 0;
        self.meas_cnt = 0;
        self.tick_cnt = 0;
        self.timera_cnt = 0;
        self.timera_cnt_b = 0;
        self.fm_slotkey = [0; 6];
        self.fm3ex_fb_alg = 0;
        self.tonemask_fb_alg = false;
        self.adpcm_start = 0;
        self.adpcm_stop = 0;
        self.adpcm_release = 0x8000;
        self.ssgrhythm = 0;
        self.opnarhythm = 0;
        self.rand = 0;
        self.fm3ex_force = false;
        self.fm3ex_det = [0; 4];
        self.fm3ex_needed = 0;
        self.fm3ex_mode = 0x3f;
        self.opna_a1 = false;
        self.meas_len = 96;
        self.fm_voldown = self.fm_voldown_orig;
        self.ssg_voldown = self.ssg_voldown_orig;
        self.adpcm_voldown = self.adpcm_voldown_orig;
        self.ppz8_voldown = self.ppz8_voldown_orig;
        self.opnarhythm_voldown = self.opnarhythm_voldown_orig;
        self.pcm86_vol_spb = self.pcm86_vol_spb_orig;
    }

    // 0fa9
    fn data_init(&mut self) -> bool {
        if self.opm_byte > 1 {
            return false;
        }
        self.opm_flag = self.opm_byte != 0;
        if self.datalen < 0x18 {
            return false;
        }
        if self.d(0) != 0x18 {
            if self.datalen < 0x18 + 2 {
                return false;
            }
            self.tone_ptr = self.r16(0x18);
            self.tone_included = true;
        } else {
            self.tone_included = false;
        }
        for pi in 0..=RHYTHM {
            let ptr = self.r16(pi as u16 * 2);
            let p = &mut self.parts[pi];
            p.ptr = ptr;
            if u32::from(self.datalen) < u32::from(p.ptr) + 1 {
                return false;
            }
            p.len_cnt = 1;
            p.keystatus.off = true;
            p.keystatus.off_mask = true;
            p.md_cnt = 0xff;
            p.md_cnt_set = 0xff;
            p.md_cnt_b = 0xff;
            p.md_cnt_set_b = 0xff;
            p.actual_note = 0xff;
            p.curr_note = 0xff;
            if pi <= FM_6 {
                p.vol = 108;
                p.pan = 0xc0;
                p.fm_slotmask = 0xf0;
                p.fm_tone_slotmask = 0xff;
            } else if pi <= SSG_3 {
                p.vol = 8;
                p.ssg_mix = 0x07;
                p.ssg_env_state_old = OLD_OFF;
            } else if pi == ADPCM {
                p.vol = 128;
                p.pan = 0xc0;
            } else if pi == RHYTHM {
                p.vol = 15;
            }
        }
        self.r_offset = self.r16(0x16);
        self.r_ptr = 0;
        true
    }

    // 111c
    fn reset_opna(&mut self, w: &mut Work) {
        w.writereg(0x29, 0x83);
        self.ssg_noise_freq = 0;
        if self.ssgeff_priority == 0 {
            w.writereg(0x06, 0x00);
            self.ssg_noise_freq_wrote = 0;
        }
        for a in 0..2 {
            self.opna_a1 = a != 0;
            for c in 0..3 {
                self.reg_write(w, 0xb4 + c, 0xc0);
            }
        }
        self.opna_a1 = false;
        self.opna_last22 = 0;
        w.writereg(0x22, 0x00);
        self.opnarhythm_ilpan = [0xcf; 6];
        w.writereg(0x10, 0xff);
        self.opnarhythm_tl = if self.opnarhythm_voldown != 0 {
            ((0xc0 * (0x100 - u32::from(self.opnarhythm_voldown))) >> 10) as u8
        } else {
            0x30
        };
        w.writereg(0x11, u32::from(self.opnarhythm_tl));
        if !self.adpcm_disabled {
            w.writereg(0x10c, 0xff);
            w.writereg(0x10d, 0xff);
        }
        for c in 0..8 {
            w.ppz8.pan(c, 5);
        }
    }

    // 2329
    fn calc_tempo(&mut self) {
        let timerb = 0x100 - u32::from(self.timerb);
        let timerb = timerb as u8;
        let mut tempo = 0xffu32;
        if timerb != 0 {
            tempo = 0x112c / u32::from(timerb);
            if (0x112c % u32::from(timerb)) & 0x80 != 0 {
                tempo += 1;
            }
            tempo = tempo.min(0xff);
        }
        self.tempo = tempo as u8;
        self.tempo_bak = tempo as u8;
    }

    // 2348
    fn calc_tempo_rev(&mut self) {
        let tempo = i32::from(self.tempo);
        let mut timerb = 0i32;
        if tempo != 0 {
            timerb = 0x100 - 0x112c / tempo;
            if (0x112c % tempo) & 0x80 != 0 {
                timerb -= 1;
            }
            timerb = timerb.max(0);
        }
        self.timerb = timerb as u8;
        self.timerb_bak = timerb as u8;
    }

    // 3e43
    fn timerb_write(&mut self, w: &mut Work) {
        self.timerb_wrote = self.timerb;
        w.writereg(0x26, u32::from(self.timerb));
    }

    // 32eb
    fn reset_timer(&mut self, w: &mut Work) {
        self.timerb = 200;
        w.timerb = self.timerb;
        self.timerb_bak = self.timerb;
        self.calc_tempo();
        self.timerb_write(w);
        w.writereg(0x25, 0);
        w.writereg(0x24, 0);
        w.writereg(0x27, 0x3f);
        self.tick_cnt = 0;
        self.meas_cnt = 0;
        self.meas_len = 96;
    }

    // 26ec
    fn opnarhythm_tl_write(&self, w: &mut Work, vol: u8) {
        let mut outvol = self.fadeout_vol;
        if outvol != 0 {
            outvol = ((u32::from(vol) * (0x100 - u32::from(outvol))) >> 8) as u8;
        }
        w.writereg(0x11, u32::from(outvol));
    }

    fn cmdload(&mut self, p: usize) -> u8 {
        let ptr = self.parts[p].ptr;
        if ptr == 0 || u32::from(self.datalen) < u32::from(ptr) + 1 {
            self.parts[p].ptr = 0;
            return 0x80;
        }
        self.parts[p].ptr = ptr.wrapping_add(1);
        self.d(ptr)
    }

    // 2fa8
    fn rand(&mut self, range: u16) -> u16 {
        let r = (self.rand.wrapping_mul(0x103).wrapping_add(3)) & 0x7fff;
        self.rand = r;
        ((u32::from(r) * u32::from(range)) / 0x7fff) as u16
    }
}

// 2cf6
fn part_off_ssg(part: &mut Part) {
    if part.actual_note == 0xff {
        return;
    }
    if part.ssg_env_state_old == OLD_NEW {
        part.ssg_env_state_new = NEW_RR;
    } else {
        part.ssg_env_state_old = OLD_RR;
    }
}

// 3102
fn lfo_reset(part: &mut Part) {
    part.lfo_diff = 0;
    part.lfo = part.lfo_set;
    part.md_cnt = part.md_cnt_set;
    if part.lfo_waveform == WF_SQUARE || part.lfo_waveform == WF_RANDOM {
        part.lfo.speed = 1;
    } else {
        part.lfo.speed = part.lfo.speed.wrapping_add(1);
    }
}

// 2f5a
fn md_tick(part: &mut Part) {
    part.md_speed = part.md_speed.wrapping_sub(1);
    if part.md_speed != 0 {
        return;
    }
    part.md_speed = part.md_speed_set;
    if part.md_cnt == 0 {
        return;
    }
    if part.md_cnt & 0x80 == 0 {
        part.md_cnt -= 1;
    }
    let mut step = part.lfo.step;
    if step & 0x80 != 0 {
        step = step.wrapping_neg().wrapping_add(part.md_depth);
        if step & 0x80 != 0 {
            step = if part.md_depth & 0x80 != 0 { 0x81 } else { 0 };
        } else {
            step = step.wrapping_neg();
        }
    } else {
        step = step.wrapping_add(part.md_depth);
        if step & 0x80 != 0 {
            step = if part.md_depth & 0x80 != 0 { 0x7f } else { 0 };
        }
    }
    part.lfo.step = step;
}

// 2493
fn lfo_flip(p: &mut Part) {
    std::mem::swap(&mut p.lfo_diff_b, &mut p.lfo_diff);
    std::mem::swap(&mut p.lfof_b, &mut p.lfof);
    std::mem::swap(&mut p.flagext_b, &mut p.flagext);
    std::mem::swap(&mut p.lfo_b, &mut p.lfo);
    std::mem::swap(&mut p.lfo_set_b, &mut p.lfo_set);
    std::mem::swap(&mut p.md_depth_b, &mut p.md_depth);
    std::mem::swap(&mut p.md_speed_b, &mut p.md_speed);
    std::mem::swap(&mut p.md_speed_set_b, &mut p.md_speed_set);
    std::mem::swap(&mut p.lfo_waveform_b, &mut p.lfo_waveform);
    std::mem::swap(&mut p.md_cnt_b, &mut p.md_cnt);
    std::mem::swap(&mut p.md_cnt_set_b, &mut p.md_cnt_set);
}

// 31ce
fn ssg_env_tick_new(p: &mut Part) {
    match p.ssg_env_state_new {
        NEW_AR => {
            let ar = p.ssg_env_param[P_NEW_AR];
            if ar.wrapping_sub(1) & 0x80 != 0 {
                if p.ssg_env_param_set[P_NEW_AR] != 0 {
                    p.ssg_env_param[P_NEW_AR] = p.ssg_env_param[P_NEW_AR].wrapping_add(1);
                }
            } else {
                p.ssg_env_vol = p.ssg_env_vol.wrapping_add(ar);
                if p.ssg_env_vol < 0xf {
                    p.ssg_env_param[P_NEW_AR] = p.ssg_env_param_set[P_NEW_AR].wrapping_sub(0x10);
                } else {
                    p.ssg_env_vol = 0xf;
                    p.ssg_env_state_new = NEW_DR;
                    if p.ssg_env_param_sl == 0xf {
                        p.ssg_env_state_new = NEW_SR;
                    }
                }
            }
        }
        NEW_DR => {
            let dr = p.ssg_env_param[P_NEW_DR];
            if dr.wrapping_sub(1) & 0x80 != 0 {
                if p.ssg_env_param_set[P_NEW_DR] != 0 {
                    p.ssg_env_param[P_NEW_DR] = p.ssg_env_param[P_NEW_DR].wrapping_add(1);
                }
            } else {
                let carry = p.ssg_env_vol < dr;
                p.ssg_env_vol = p.ssg_env_vol.wrapping_sub(dr);
                let sl = p.ssg_env_param_sl;
                if !carry && p.ssg_env_vol >= sl {
                    let mut newdr = p.ssg_env_param_set[P_NEW_DR].wrapping_sub(0x10);
                    if newdr & 0x80 != 0 {
                        newdr = newdr.wrapping_add(newdr);
                    }
                    p.ssg_env_param[P_NEW_DR] = newdr;
                } else {
                    p.ssg_env_vol = sl;
                    p.ssg_env_state_new = NEW_SR;
                }
            }
        }
        NEW_SR => {
            let sr = p.ssg_env_param[P_NEW_SR];
            if sr.wrapping_sub(1) & 0x80 != 0 {
                if p.ssg_env_param_set[P_NEW_SR] != 0 {
                    p.ssg_env_param[P_NEW_SR] = p.ssg_env_param[P_NEW_SR].wrapping_add(1);
                }
            } else {
                let carry = p.ssg_env_vol < sr;
                p.ssg_env_vol = p.ssg_env_vol.wrapping_sub(sr);
                if carry {
                    p.ssg_env_vol = 0;
                }
                let mut newsr = p.ssg_env_param_set[P_NEW_SR].wrapping_sub(0x10);
                if newsr & 0x80 != 0 {
                    newsr = newsr.wrapping_add(newsr);
                }
                p.ssg_env_param[P_NEW_SR] = newsr;
            }
        }
        _ => {
            let rr = p.ssg_env_param[P_NEW_RR];
            if rr.wrapping_sub(1) & 0x80 != 0 {
                if p.ssg_env_param_set[P_NEW_RR] != 0 {
                    p.ssg_env_param[P_NEW_RR] = p.ssg_env_param[P_NEW_RR].wrapping_add(1);
                }
            } else {
                let carry = p.ssg_env_vol < rr;
                p.ssg_env_vol = p.ssg_env_vol.wrapping_sub(rr);
                if carry {
                    p.ssg_env_vol = 0;
                }
                let s = p.ssg_env_param_set[P_NEW_RR];
                p.ssg_env_param[P_NEW_RR] = s.wrapping_add(s).wrapping_sub(0x10);
            }
        }
    }
}

// 31b9
fn ssg_env_tick_new_check(p: &mut Part) -> bool {
    if p.ssg_env_state_new == NEW_OFF {
        return false;
    }
    let prev = p.ssg_env_vol;
    ssg_env_tick_new(p);
    p.ssg_env_vol != prev
}

// 3ee7
const ALG_OUTPUT_SLOT: [u8; 8] = [0x80, 0x80, 0x80, 0x80, 0xa0, 0xe0, 0xe0, 0xf0];
// 3eef
const ALG_TONE_SLOT: [u8; 8] = [0xee, 0xee, 0xee, 0xee, 0xcc, 0x88, 0x88, 0x00];

impl Pmd {
    // 2e7f
    fn lfo_tick_waveform(&mut self, p: usize) {
        let pt = &mut self.parts[p];
        if pt.lfo.speed != 1 {
            if pt.lfo.speed != 0xff {
                pt.lfo.speed = pt.lfo.speed.wrapping_sub(1);
            }
            return;
        }
        pt.lfo.speed = pt.lfo_set.speed;
        match pt.lfo_waveform {
            WF_TRIANGLE1 | WF_TRIANGLE2 | WF_TRIANGLE3 => {
                let mut deptha = i16::from(u8s8(pt.lfo.step));
                if pt.lfo_waveform == WF_TRIANGLE3 {
                    deptha = deptha.wrapping_mul(deptha.abs());
                }
                pt.lfo_diff = pt.lfo_diff.wrapping_add(deptha);
                if pt.lfo_diff == 0 {
                    md_tick(pt);
                }
                if pt.lfo.times != 0xff {
                    pt.lfo.times = pt.lfo.times.wrapping_sub(1);
                    if pt.lfo.times == 0 {
                        let mut times = pt.lfo_set.times;
                        if pt.lfo_waveform != WF_TRIANGLE2 {
                            times = times.wrapping_mul(2);
                        }
                        pt.lfo.times = times;
                        pt.lfo.step = pt.lfo.step.wrapping_neg();
                    }
                }
            }
            WF_SAWTOOTH => {
                pt.lfo_diff = pt.lfo_diff.wrapping_add(i16::from(u8s8(pt.lfo.step)));
                if pt.lfo.times != 0xff {
                    pt.lfo.times = pt.lfo.times.wrapping_sub(1);
                    if pt.lfo.times == 0 {
                        pt.lfo_diff = pt.lfo_diff.wrapping_neg();
                        md_tick(pt);
                        pt.lfo.times = pt.lfo_set.times.wrapping_mul(2);
                    }
                }
            }
            WF_SQUARE => {
                pt.lfo_diff = i16::from(u8s8(pt.lfo.step)) * i16::from(u8s8(pt.lfo.times));
                md_tick(pt);
                pt.lfo.step = pt.lfo.step.wrapping_neg();
            }
            WF_RANDOM => {
                let deptha = i32::from(u8s8(pt.lfo.step)).abs() as u32;
                let range = (deptha * u32::from(pt.lfo.times)) as u16;
                let r = self.rand(range.wrapping_mul(2)).wrapping_sub(range);
                let pt = &mut self.parts[p];
                pt.lfo_diff = r as i16;
                md_tick(pt);
            }
            WF_ONESHOT => {
                if pt.lfo.times != 0 {
                    if pt.lfo.times != 0xff {
                        pt.lfo.times -= 1;
                    }
                    pt.lfo_diff = pt.lfo_diff.wrapping_add(i16::from(u8s8(pt.lfo.step)));
                }
            }
            _ => {}
        }
    }

    // 2e48: whether lfo_diff changed
    fn lfo_tick(&mut self, p: usize) -> bool {
        if self.parts[p].lfo.delay != 0 {
            self.parts[p].lfo.delay -= 1;
            return false;
        }
        let prev = self.parts[p].lfo_diff;
        if self.parts[p].flagext.lfo {
            let cnt = self.timera_cnt.wrapping_sub(self.timera_cnt_b);
            for _ in 0..cnt {
                self.lfo_tick_waveform(p);
            }
        } else {
            self.lfo_tick_waveform(p);
        }
        self.parts[p].lfo_diff != prev
    }

    fn lfo_tick_b(&mut self, p: usize) -> bool {
        lfo_flip(&mut self.parts[p]);
        let r = self.lfo_tick(p);
        lfo_flip(&mut self.parts[p]);
        r
    }

    // 30a6
    fn lfo_tick_if_needed_hlfo(&mut self, w: &mut Work, p: usize) {
        self.parts[p].hlfo_delay = self.parts[p].hlfo_delay_set;
        if self.parts[p].hlfo_delay != 0 {
            self.reg_write(w, 0xb3u8.wrapping_add(self.proc_ch), self.parts[p].pan & 0xc0);
        }
        self.parts[p].slot_delay_cnt = self.parts[p].slot_delay;
        let f = self.parts[p].lfof;
        if f.freq || f.vol {
            if !f.sync {
                lfo_reset(&mut self.parts[p]);
            }
            self.lfo_tick(p);
        }
        let f = self.parts[p].lfof_b;
        if f.freq || f.vol {
            if !f.sync {
                let pt = &mut self.parts[p];
                lfo_flip(pt);
                lfo_reset(pt);
                lfo_flip(pt);
            }
            self.lfo_tick_b(p);
        }
    }

    // 3086
    fn lfo_tick_if_needed(&mut self, p: usize) {
        let f = self.parts[p].lfof;
        if f.freq || f.vol {
            self.lfo_tick(p);
        }
        let f = self.parts[p].lfof_b;
        if f.freq || f.vol {
            self.lfo_tick_b(p);
        }
    }

    // 3060
    fn lfo_init_fm(&mut self, w: &mut Work, p: usize, note: u8) -> u8 {
        let mut note = note;
        let mut n = note & 0xf;
        if n == 0xc {
            note = self.parts[p].curr_note;
            n = note & 0xf;
        }
        self.parts[p].curr_note = note;
        if n == 0xf {
            self.lfo_tick_if_needed(p);
            return note;
        }
        self.parts[p].portamento_diff = 0;
        if self.no_keyoff {
            self.lfo_tick_if_needed(p);
        } else {
            self.lfo_tick_if_needed_hlfo(w, p);
        }
        note
    }

    // 2fc4
    fn lfo_init_ssg(&mut self, w: &mut Work, p: usize, note: u8) -> u8 {
        let mut note = note;
        let mut n = note & 0xf;
        if n == 0xc {
            note = self.parts[p].curr_note;
            n = note & 0xf;
        }
        self.parts[p].curr_note = note;
        if n == 0xf {
            self.lfo_tick_if_needed(p);
            return note;
        }
        self.parts[p].portamento_diff = 0;
        if self.no_keyoff {
            self.lfo_tick_if_needed(p);
            return note;
        }
        let pt = &mut self.parts[p];
        if pt.ssg_env_state_old != OLD_NEW {
            pt.ssg_env_state_old = OLD_AL;
            pt.ssg_env_vol = 0;
            pt.ssg_env_param_set[P_OLD_AL] = pt.ssg_env_param[P_OLD_AL];
            if pt.ssg_env_param_set[P_OLD_AL] == 0 {
                pt.ssg_env_state_old = OLD_SR;
                pt.ssg_env_vol = pt.ssg_env_param_set[P_OLD_AD];
            }
            pt.ssg_env_param_set[P_OLD_SR] = pt.ssg_env_param[P_OLD_SR];
            pt.ssg_env_param_set[P_OLD_RR] = pt.ssg_env_param[P_OLD_RR];
        } else {
            pt.ssg_env_param[P_NEW_AR] = pt.ssg_env_param_set[P_NEW_AR].wrapping_sub(0x10);
            let mut dr = i32::from(u8s8(pt.ssg_env_param_set[P_NEW_DR].wrapping_sub(0x10)));
            if dr < 0 {
                dr += dr;
            }
            pt.ssg_env_param[P_NEW_DR] = dr as u8;
            let mut sr = i32::from(u8s8(pt.ssg_env_param_set[P_NEW_SR].wrapping_sub(0x10)));
            if sr < 0 {
                sr += sr;
            }
            pt.ssg_env_param[P_NEW_SR] = sr as u8;
            let rr = pt.ssg_env_param_set[P_NEW_RR];
            pt.ssg_env_param[P_NEW_RR] = rr.wrapping_add(rr).wrapping_sub(0x10);
            pt.ssg_env_vol = pt.ssg_env_param_al;
            pt.ssg_env_state_new = NEW_AR;
            ssg_env_tick_new_check(pt);
        }
        self.lfo_tick_if_needed_hlfo(w, p);
        note
    }

    // 0e5e
    fn ssgeff_stop(&mut self, w: &mut Work) {
        w.writereg(0x0a, 0);
        let out = w.readreg(0x07) | (1 << 2) | (1 << 5);
        w.writereg(0x07, out);
        self.ssgeff_priority = 0;
        self.ssgeff_num = 0xff;
        self.ssgeff_on = false;
    }

    // 0def
    fn ssgeff_advance(&mut self, w: &mut Work) {
        let Some(e) = self.ssgeff_data.get(self.ssgeff_pos).copied() else { return self.ssgeff_stop(w) };
        if e.wait == 0xff {
            self.ssgeff_stop(w);
            return;
        }
        self.ssgeff_wait = e.wait;
        self.ssgeff_tonefreq = e.tone_freq;
        w.writereg(0x04, u32::from(self.ssgeff_tonefreq & 0xff));
        w.writereg(0x05, u32::from(self.ssgeff_tonefreq >> 8));
        self.ssgeff_noisefreq = e.noise_freq;
        w.writereg(0x06, u32::from(self.ssgeff_noisefreq));
        self.ssg_noise_freq_wrote = self.ssgeff_noisefreq;
        let mut out = w.readreg(0x07) & !((1 << 2) | (1 << 5));
        out |= (u32::from(!e.tone_mix) << 2) | (u32::from(!e.noise_mix) << 5);
        self.ssgeff_tone_mix = e.tone_mix;
        self.ssgeff_noise_mix = e.noise_mix;
        w.writereg(0x07, out);
        w.writereg(0x0a, u32::from(e.env_level));
        w.writereg(0x0b, u32::from(e.env_freq & 0xff));
        w.writereg(0x0c, u32::from(e.env_freq >> 8));
        w.writereg(0x0d, u32::from(e.env_waveform));
        self.ssgeff_tonefreq_add = i16::from(e.tonefreq_add);
        self.ssgeff_noisefreq_add = e.noisefreq_add;
        self.ssgeff_noisefreq_cnt_set = e.noisefreq_wait;
        self.ssgeff_noisefreq_cnt = self.ssgeff_noisefreq_cnt_set;
        self.ssgeff_pos += 1;
    }

    // 0e8d
    fn ssgeff_tick_idle(&mut self, w: &mut Work) {
        self.ssgeff_tonefreq = self.ssgeff_tonefreq.wrapping_add(self.ssgeff_tonefreq_add as u16);
        w.writereg(0x04, u32::from(self.ssgeff_tonefreq & 0xff));
        w.writereg(0x05, u32::from(self.ssgeff_tonefreq >> 8));
        if self.ssgeff_noisefreq_add == 0 && self.ssgeff_noisefreq_cnt_set == 0 {
            return;
        }
        self.ssgeff_noisefreq_cnt = self.ssgeff_noisefreq_cnt.wrapping_sub(1);
        if self.ssgeff_noisefreq_cnt != 0 {
            return;
        }
        self.ssgeff_noisefreq_cnt = self.ssgeff_noisefreq_cnt_set;
        self.ssgeff_noisefreq = self.ssgeff_noisefreq.wrapping_add(self.ssgeff_noisefreq_add as u8);
        w.writereg(0x06, u32::from(self.ssgeff_noisefreq));
        self.ssg_noise_freq_wrote = self.ssgeff_noisefreq;
    }

    // 0dde
    fn ssgeff_tick(&mut self, w: &mut Work) {
        self.ssgeff_wait = self.ssgeff_wait.wrapping_sub(1);
        if self.ssgeff_wait != 0 {
            self.ssgeff_tick_idle(w);
        } else {
            self.ssgeff_advance(w);
        }
    }

    // 2dda: where the voice's 25 bytes start
    fn toneptr(&self, tonenum: u8) -> Option<u16> {
        if self.tone_included {
            let mut t = self.tone_ptr;
            loop {
                if u32::from(self.datalen) < u32::from(t) + 0x1a {
                    return None;
                }
                if self.d(t) == tonenum {
                    return Some(t + 1);
                }
                t = t.wrapping_add(0x1a);
            }
        }
        None
    }

    // 2cbc
    fn do_keyoff_fm(&mut self, w: &mut Work, p: usize) {
        let mut out = self.proc_ch.wrapping_sub(1);
        let mut ch = usize::from(self.proc_ch.wrapping_sub(1));
        if self.opna_a1 {
            ch += 3;
            out |= 4;
        }
        let Some(k) = self.fm_slotkey.get_mut(ch) else { return };
        *k &= !self.parts[p].fm_slotmask;
        out |= *k;
        w.writereg(0x28, u32::from(out));
    }

    // 2e15
    fn tone_slotmask_stop(&mut self, w: &mut Work, p: usize) -> bool {
        let mask = self.parts[p].fm_tone_slotmask;
        if mask == 0 {
            return false;
        }
        let mut addr = 0x3fu8.wrapping_add(self.proc_ch);
        for s in 0..4 {
            if mask & (1 << (7 - s)) != 0 {
                self.reg_write(w, addr, 0x7f);
                self.reg_write(w, addr.wrapping_add(0x40), 0x7f);
            }
            addr = addr.wrapping_add(4);
        }
        self.do_keyoff_fm(w, p);
        true
    }

    // 2d0d
    fn tone_fm(&mut self, w: &mut Work, p: usize) {
        let Some(t) = self.toneptr(self.parts[p].tonenum) else { return };
        let mut tone = [0u8; 25];
        for (i, x) in tone.iter_mut().enumerate() {
            *x = self.d(t + i as u16);
        }
        let tb = |i: u16| tone[usize::from(i)];
        if self.tone_slotmask_stop(w, p) {
            let mut fb_alg = tb(0x18);
            if self.tonemask_fb_alg {
                fb_alg = self.parts[p].fb_alg;
            }
            if !self.opna_a1 && self.proc_ch == 3 {
                if self.tonemask_fb_alg {
                    fb_alg = self.fm3ex_fb_alg;
                } else {
                    if self.parts[p].fm_slotmask & 0x10 == 0 {
                        fb_alg &= 7;
                        fb_alg |= self.fm3ex_fb_alg & 0x38;
                    }
                    self.fm3ex_fb_alg = fb_alg;
                }
            }
            let ch = self.proc_ch;
            self.reg_write(w, 0xafu8.wrapping_add(ch), fb_alg);
            let pt = &mut self.parts[p];
            pt.fb_alg = fb_alg;
            let outslot = ALG_OUTPUT_SLOT[usize::from(fb_alg & 7)];
            if pt.vol_lfo_slotmask & 0xf == 0 {
                pt.vol_lfo_slotmask = outslot;
            }
            if pt.vol_lfo_slotmask_b & 0xf == 0 {
                pt.vol_lfo_slotmask_b = outslot;
            }
            pt.fm_slotout = outslot;
            let tone_slotmask = pt.fm_tone_slotmask;
            let tl_slotmask = ALG_TONE_SLOT[usize::from(fb_alg & 7)] & tone_slotmask;
            for s in 0..4u8 {
                if tone_slotmask & (1 << (7 - s)) != 0 {
                    self.reg_write(w, (0x2f + s * 4).wrapping_add(ch), tb(u16::from(s)));
                }
            }
            for s in 0..4u8 {
                if tl_slotmask & (1 << (7 - s)) != 0 {
                    self.reg_write(w, (0x3f + s * 4).wrapping_add(ch), tb(0x04 + u16::from(s)));
                }
            }
            for s in 0..4u8 {
                if tone_slotmask & (1 << (3 - s)) != 0 {
                    self.reg_write(w, (0x4f + s * 4).wrapping_add(ch), tb(0x08 + u16::from(s)));
                    self.reg_write(w, (0x6f + s * 4).wrapping_add(ch), tb(0x10 + u16::from(s)));
                }
                if tone_slotmask & (1 << (7 - s)) != 0 {
                    self.reg_write(w, (0x5f + s * 4).wrapping_add(ch), tb(0x0c + u16::from(s)));
                    self.reg_write(w, (0x7f + s * 4).wrapping_add(ch), tb(0x14 + u16::from(s)));
                }
            }
        }
        self.parts[p].fm_tone_tl = [tone[4], tone[5], tone[6], tone[7]];
    }
}

const FM_EFF: usize = 22;

// 271a
fn note_transpose(p: &Part, note: u8) -> u8 {
    if note == 0x0f {
        return note;
    }
    let transpose = i32::from(u8s8(p.transpose.wrapping_add(p.transpose_master)));
    if transpose == 0 {
        return note;
    }
    let mut octave = note >> 4;
    let mut n = i32::from(note & 0x0f) + transpose;
    if transpose < 0 {
        while n < 0 {
            octave = octave.wrapping_sub(1);
            n += 0xc;
        }
    } else {
        while n >= 0xc {
            octave = octave.wrapping_add(1);
            n -= 0xc;
        }
    }
    (octave << 4) | n as u8
}

fn rest_freq(p: &mut Part) {
    p.actual_note = 0xff;
    if !p.lfof.freq && !p.lfof_b.freq {
        p.actual_freq = 0;
    }
}

// 2771
fn note_freq_fm(p: &mut Part, note: u8) {
    if note & 0xf == 0xf {
        return rest_freq(p);
    }
    p.actual_note = note;
    const T: [u16; 16] = [0x026a, 0x028f, 0x02b6, 0x02df, 0x030b, 0x0339, 0x036a, 0x039e, 0x03d5, 0x0410, 0x044e, 0x048f, 0, 0, 0, 0];
    p.actual_freq = T[usize::from(note & 0xf)] | (u16::from(note & 0x70) << 7);
}

// 27a8
fn note_freq_ssg(p: &mut Part, note: u8) {
    if note & 0xf == 0xf {
        return rest_freq(p);
    }
    p.actual_note = note;
    const T: [u16; 16] = [0x0ee8, 0x0e12, 0x0d48, 0x0c89, 0x0bd5, 0x0b2b, 0x0a8a, 0x09f3, 0x0964, 0x08dd, 0x085e, 0x07e6, 0, 0, 0, 0];
    let octave = note >> 4;
    let mut f = T[usize::from(note & 0xf)];
    if octave != 0 {
        f >>= octave - 1;
        let inc = f & 1;
        f = (f >> 1) + inc;
    }
    p.actual_freq = f;
}

// 0671
fn note_freq_adpcm(p: &mut Part, note: u8) {
    if note & 0xf == 0xf {
        return rest_freq(p);
    }
    p.actual_note = note;
    const T: [u16; 16] = [0x6264, 0x6840, 0x6e74, 0x7506, 0x7bfc, 0x835e, 0x8b2e, 0x9376, 0x9c3c, 0xa588, 0xaf62, 0xb9d0, 0, 0, 0, 0];
    let mut octave = note >> 4;
    let mut f = T[usize::from(note & 0xf)];
    if octave <= 5 {
        f >>= 5 - octave;
    } else {
        octave = 5;
        if f & 0x8000 == 0 {
            f <<= 1;
            octave += 1;
        }
        p.actual_note = (p.actual_note & 0xf) | (octave << 4);
    }
    p.actual_freq = f;
}

// 0c97
fn note_freq_ppz8(p: &mut Part, note: u8) {
    if note & 0xf == 0xf {
        p.actual_note = 0xff;
        if !p.lfof.freq && !p.lfof_b.freq {
            p.actual_freq = 0;
            p.actual_freq_upper = 0;
        }
        return;
    }
    p.actual_note = note;
    const T: [u16; 16] = [0x8000, 0x87a6, 0x8fb3, 0x9838, 0xa146, 0xaade, 0xb4ff, 0xbfcc, 0xcb34, 0xd747, 0xe418, 0xf1a5, 0, 0, 0, 0];
    let octave = i32::from(note >> 4) - 4;
    let mut f = u32::from(T[usize::from(note & 0xf)]);
    if octave < 0 {
        f >>= -octave;
    } else {
        f <<= octave;
    }
    p.actual_freq = f as u16;
    p.actual_freq_upper = (f >> 16) as u16;
}

// 2b4c
fn fm_vol_out_sub(slotmask: u8, table: &mut [u8; 4], vol: u8) {
    for s in 0..4 {
        if slotmask & (1 << (7 - s)) == 0 {
            continue;
        }
        if vol & 0x80 == 0 {
            table[s] = table[s].saturating_sub(vol);
        } else {
            table[s] = table[s].saturating_add(vol.wrapping_neg());
        }
    }
}

// 2942
fn blkfnum_normalize(blk: u16, fnum: u16) -> u32 {
    let (mut blk, mut fnum) = (blk, fnum);
    loop {
        if fnum & 0x8000 != 0 || fnum < 0x26a {
            let iblk = i32::from(blk) - 0x0800;
            if iblk < 0 {
                blk = 0;
                if fnum & 0x8000 != 0 {
                    fnum = 0x0008;
                }
                if fnum < 0x0008 {
                    fnum = 0x0008;
                }
                break;
            }
            blk = iblk as u16;
            fnum = fnum.wrapping_add(0x26a);
            continue;
        }
        if fnum < 0x4d4 {
            break;
        }
        blk = blk.wrapping_add(0x0800);
        if blk != 0x4000 {
            fnum = fnum.wrapping_sub(0x26a);
            continue;
        }
        blk = 0x3800;
        if fnum > 0x07ff {
            fnum = 0x7ff;
        }
        break;
    }
    (u32::from(blk) << 16) | u32::from(fnum)
}

// none
fn masked(p: &Part) -> bool {
    p.mask.ext || p.mask.effect || p.mask.disabled || p.mask.slot || p.mask.mml || p.mask.ff
}

impl Pmd {
    // 0d28, 0d09
    fn ssg_effect(&mut self, w: &mut Work, effnum: u8) {
        if usize::from(effnum) >= SSGEFF.len() {
            return;
        }
        self.ssgeff_internal = true;
        if self.ssgeff_disabled {
            return;
        }
        self.ssgeff_num = effnum;
        let (prio, data) = SSGEFF[usize::from(effnum)];
        if self.ssgeff_priority > prio {
            return;
        }
        self.ssgeff_data = data;
        self.ssgeff_pos = 0;
        self.parts[SSG_3].mask.effect = true;
        self.ssgeff_advance(w);
        self.ssgeff_priority = prio;
        self.ssgeff_on = true;
    }

    fn cmd_null(&mut self, p: usize, n: usize) {
        for _ in 0..n {
            self.cmdload(p);
        }
    }

    // 14a0
    fn calc_gate(&mut self, p: usize) {
        // 98fmplayer's check here is inverted, reading past the song (where
        // nothing is 0xc1); so it never acts.
        let pt = self.parts[p];
        let mut gate = pt.gate_abs;
        if pt.gate_rel != 0 {
            gate = gate.wrapping_add(((u16::from(pt.len_cnt) * u16::from(pt.gate_rel)) >> 8) as u8);
        }
        if pt.gate_rand_range != 0 {
            let range = u16::from(pt.gate_rand_range & 0x7f) + 1;
            let r = self.rand(range) as u8;
            if pt.gate_rand_range & 0x80 != 0 {
                gate = gate.wrapping_sub(r);
            } else {
                gate = gate.wrapping_add(r);
            }
        }
        let pt = &mut self.parts[p];
        if pt.gate_min != 0 {
            let len = i32::from(pt.len_cnt) - i32::from(pt.gate_min);
            if len < 0 {
                pt.gate = 0;
                return;
            }
            if i32::from(gate) >= len {
                gate = len as u8;
            }
        }
        pt.gate = gate;
    }

    // 2a38
    fn fm_vol_out(&mut self, w: &mut Work, p: usize) {
        let pt = self.parts[p];
        let slotmask = pt.fm_slotmask;
        if slotmask == 0 {
            return;
        }
        let mut vol = if pt.volume_save != 0 { pt.volume_save - 1 } else { pt.vol };
        if p != FM_EFF {
            if self.fm_voldown != 0 {
                vol = ((u32::from(vol) * u32::from(self.fm_voldown.wrapping_neg())) >> 8) as u8;
            }
            if self.fadeout_vol != 0 {
                vol = ((u32::from(vol) * u32::from(self.fadeout_vol.wrapping_neg())) >> 8) as u8;
            }
        }
        let mut table = [0x80u8; 4];
        vol ^= 0xff;
        let oslotmask = slotmask & pt.fm_slotout;
        let mut update = oslotmask;
        for s in 0..4 {
            if oslotmask & (1 << (7 - s)) != 0 {
                table[s] = vol;
            }
        }
        if vol != 0xff {
            if pt.lfof.vol {
                let l = slotmask & pt.vol_lfo_slotmask;
                update |= l;
                fm_vol_out_sub(l, &mut table, pt.lfo_diff as u8);
            }
            if pt.lfof_b.vol {
                let l = slotmask & pt.vol_lfo_slotmask_b;
                update |= l;
                fm_vol_out_sub(l, &mut table, pt.lfo_diff_b as u8);
            }
        }
        const TLADDR: [u8; 4] = [0x4b, 0x43, 0x47, 0x3f];
        const TONEIND: [usize; 4] = [3, 1, 2, 0];
        for s in 0..4 {
            if update & (1 << (7 - s)) == 0 {
                continue;
            }
            let o = (u32::from(table[s]) + u32::from(pt.fm_tone_tl[TONEIND[s]])).min(0xff).saturating_sub(0x80);
            self.reg_write(w, TLADDR[s].wrapping_add(self.proc_ch), o as u8);
        }
    }

    // 2b70
    fn ssg_vol_out(&mut self, w: &mut Work, p: usize) {
        let pt = self.parts[p];
        if pt.ssg_env_state_old == OLD_OFF || (pt.ssg_env_state_old == OLD_NEW && pt.ssg_env_state_new == NEW_OFF) {
            return;
        }
        let mut vol = if pt.volume_save != 0 { pt.volume_save - 1 } else { pt.vol };
        if self.ssg_voldown != 0 {
            vol = ((u32::from(vol) * u32::from(self.ssg_voldown.wrapping_neg())) >> 8) as u8;
        }
        if self.fadeout_vol != 0 {
            vol = ((u32::from(vol) * u32::from(self.fadeout_vol.wrapping_neg())) >> 8) as u8;
        }
        if vol != 0 {
            if pt.ssg_env_state_old == OLD_NEW {
                let envvol = u32::from(pt.ssg_env_vol);
                if envvol == 0 {
                    vol = 0;
                } else {
                    vol = (((u32::from(vol) * (envvol + 1)) & 0xff) >> 3) as u8;
                    if vol & 1 != 0 {
                        vol = (vol >> 1) + 1;
                    } else {
                        vol >>= 1;
                    }
                }
            } else {
                vol = vol.wrapping_add(pt.ssg_env_vol);
                if vol & 0x80 != 0 {
                    vol = 0;
                }
                if vol > 0xf {
                    vol = 0xf;
                }
            }
            if vol != 0 && (pt.lfof.vol || pt.lfof_b.vol) {
                let mut l = 0i32;
                if pt.lfof.vol {
                    l += i32::from(pt.lfo_diff);
                }
                if pt.lfof_b.vol {
                    l += i32::from(pt.lfo_diff_b);
                }
                l += i32::from(vol);
                vol = l.clamp(0, 0xf) as u8;
            }
        }
        w.writereg(0x07 + u32::from(self.proc_ch), u32::from(vol));
    }

    // 049a
    fn adpcm_vol_out(&mut self, w: &mut Work, p: usize) {
        let pt = self.parts[p];
        let mut vol = if pt.volume_save != 0 { pt.volume_save } else { pt.vol };
        if self.adpcm_voldown != 0 {
            vol = ((u32::from(vol) * u32::from(self.adpcm_voldown.wrapping_neg())) >> 8) as u8;
        }
        if self.fadeout_vol != 0 {
            let f = u32::from(self.fadeout_vol.wrapping_neg());
            let f = (f * f) >> 8;
            vol = ((u32::from(vol) * f) >> 8) as u8;
        }
        if vol != 0 {
            if pt.ssg_env_state_old == OLD_NEW {
                let envvol = u32::from(pt.ssg_env_vol);
                if envvol == 0 {
                    vol = 0;
                } else {
                    let mut i = (u32::from(vol) * (envvol + 1)) >> 3;
                    if i & 1 != 0 {
                        i = (i >> 1) + 1;
                    } else {
                        i >>= 1;
                    }
                    vol = i as u8;
                }
            } else {
                vol = (i32::from(vol) + (i32::from(u8s8(pt.ssg_env_vol)) << 4)).clamp(0, 0xff) as u8;
            }
            // As 98fmplayer has it: the LFO is added into the volume, wrapping.
            if vol != 0 && (pt.lfof.vol || pt.lfof_b.vol) {
                if pt.lfof.vol {
                    vol = vol.wrapping_add(pt.lfo_diff as u8);
                }
                if pt.lfof_b.vol {
                    vol = vol.wrapping_add(pt.lfo_diff_b as u8);
                }
            }
        }
        w.writereg(0x10b, u32::from(vol));
    }

    // 0b33
    fn ppz8_vol_out(&mut self, w: &mut Work, p: usize) {
        let pt = self.parts[p];
        let mut vol = if pt.volume_save != 0 { pt.volume_save } else { pt.vol };
        if self.ppz8_voldown != 0 {
            vol = ((u32::from(vol) * u32::from(self.ppz8_voldown.wrapping_neg())) >> 8) as u8;
        }
        if self.fadeout_vol != 0 {
            vol = ((u32::from(vol) * u32::from(self.fadeout_vol.wrapping_neg())) >> 8) as u8;
        }
        if vol != 0 {
            if pt.ssg_env_state_old == OLD_NEW {
                let envvol = u32::from(pt.ssg_env_vol);
                if envvol == 0 {
                    vol = 0;
                } else {
                    let mut i = (u32::from(vol) * (envvol + 1)) >> 3;
                    if i & 1 != 0 {
                        i = (i >> 1) + 1;
                    } else {
                        i >>= 1;
                    }
                    vol = i as u8;
                }
            } else {
                vol = (i32::from(vol) + (i32::from(u8s8(pt.ssg_env_vol)) << 4)).clamp(0, 0xff) as u8;
            }
            if pt.lfof.vol || pt.lfof_b.vol {
                let mut l = 0i32;
                if pt.lfof.vol {
                    l += i32::from(pt.lfo_diff);
                }
                if pt.lfof_b.vol {
                    l += i32::from(pt.lfo_diff_b);
                }
                l += i32::from(vol);
                vol = l.clamp(0, 0xff) as u8;
            }
        }
        if vol != 0 {
            w.ppz8.volume(self.proc_ch, vol >> 4);
        } else {
            w.ppz8.stop(self.proc_ch);
        }
    }

    // 2985
    fn ssg_freq_out(&mut self, w: &mut Work, p: usize) {
        let pt = self.parts[p];
        let mut freq = pt.actual_freq;
        if freq == 0 {
            return;
        }
        freq = freq.wrapping_add(pt.portamento_diff as u16);
        if !pt.flagext.detune {
            freq = freq.wrapping_add(pt.detune.wrapping_neg() as u16);
            if pt.lfof.freq {
                freq = freq.wrapping_add(pt.lfo_diff.wrapping_neg() as u16);
            }
            if pt.lfof_b.freq {
                freq = freq.wrapping_add(pt.lfo_diff_b.wrapping_neg() as u16);
            }
        } else {
            if pt.detune != 0 {
                let mut det = (i32::from(freq) * i32::from(pt.detune)) >> 12;
                if det >= 0 {
                    det += 1;
                } else {
                    det -= 1;
                }
                freq = freq.wrapping_add(det.wrapping_neg() as u16);
            }
            if pt.lfof.freq || pt.lfof_b.freq {
                let mut l = 0i32;
                if pt.lfof.freq {
                    l += i32::from(pt.lfo_diff);
                }
                if pt.lfof_b.freq {
                    l += i32::from(pt.lfo_diff_b);
                }
                if l != 0 {
                    l = (i32::from(freq) * l) >> 12;
                    if l >= 0 {
                        l += 1;
                    } else {
                        l -= 1;
                    }
                    freq = freq.wrapping_add(l.wrapping_neg() as u16);
                }
            }
        }
        if freq >= 0x8000 {
            freq = 0;
        }
        if freq >= 0x1000 {
            freq = 0x0fff;
        }
        self.parts[p].output_freq = u32::from(freq);
        let reg = u32::from(self.proc_ch.wrapping_sub(1)) * 2;
        w.writereg(reg, u32::from(freq & 0xff));
        w.writereg(reg + 1, u32::from(freq >> 8));
    }

    // 0620
    fn adpcm_freq_out(&mut self, w: &mut Work, p: usize) {
        let pt = self.parts[p];
        let mut freq = pt.actual_freq;
        if freq == 0 {
            return;
        }
        freq = freq.wrapping_add(pt.portamento_diff as u16);
        let mut det = 0u32;
        if pt.lfof.freq || pt.lfof_b.freq {
            if pt.lfof.freq {
                det = det.wrapping_add(pt.lfo_diff as i32 as u32);
            }
            if pt.lfof_b.freq {
                det = det.wrapping_add(pt.lfo_diff_b as i32 as u32);
            }
            det <<= 2;
        }
        det = det.wrapping_add(pt.detune as i32 as u32);
        let f = (i32::from(freq) + i32::from(u16s16(det as u16))).clamp(0, 0xffff) as u16;
        self.parts[p].output_freq = u32::from(f);
        w.writereg(0x109, u32::from(f & 0xff));
        w.writereg(0x10a, u32::from(f >> 8));
    }

    // 0c27
    fn ppz8_freq_out(&mut self, w: &mut Work, p: usize) {
        let pt = self.parts[p];
        let mut freq = u32::from(pt.actual_freq) | (u32::from(pt.actual_freq_upper) << 16);
        if freq == 0 {
            return;
        }
        if pt.portamento_diff != 0 {
            freq = freq.wrapping_add((i32::from(pt.portamento_diff) << 4) as u32);
        }
        let mut det = 0i32;
        if pt.lfof.freq {
            det += i32::from(pt.lfo_diff);
        }
        if pt.lfof_b.freq {
            det += i32::from(pt.lfo_diff_b);
        }
        det += i32::from(pt.detune);
        let det = (det as u32).wrapping_mul(freq >> 8);
        let out = freq.wrapping_add(det).min(i32::MAX as u32);
        self.parts[p].output_freq = out;
        w.ppz8.freq(self.proc_ch, out);
    }

    // 2c9f
    fn ssg_readout(&self, w: &Work) -> u16 {
        let sh = u32::from(self.proc_ch.wrapping_sub(1)) & 31;
        ((0x0900u32 << sh) as u16) | w.readreg(0x07) as u16
    }

    // 2c67
    fn ssg_note_noise_out(&mut self, w: &mut Work, p: usize) {
        if self.parts[p].actual_note == 0xff {
            return;
        }
        let n = self.ssg_readout(w);
        let mut out = (n | (n >> 8)) as u8;
        let pmdout = ((n >> 8) as u8) & self.parts[p].ssg_mix;
        out &= !pmdout;
        w.writereg(0x07, u32::from(out));
        if self.ssg_noise_freq != self.ssg_noise_freq_wrote && self.ssgeff_num & 0x80 != 0 {
            w.writereg(0x06, u32::from(self.ssg_noise_freq));
            self.ssg_noise_freq_wrote = self.ssg_noise_freq;
        }
    }

    // 148b
    fn loop_check(&mut self, p: usize) {
        let l = self.parts[p].loop_;
        self.loop_.looped &= l.looped;
        self.loop_.ended &= l.ended;
        self.loop_.env &= l.env;
    }

    // 2c19
    fn fm_keyon(&mut self, w: &mut Work, p: usize) {
        let pt = self.parts[p];
        if pt.actual_note == 0xff {
            return;
        }
        let i = usize::from(self.proc_ch.wrapping_sub(1)) + 3 * usize::from(self.opna_a1);
        let Some(k) = self.fm_slotkey.get_mut(i) else { return };
        *k |= pt.fm_slotmask;
        if pt.slot_delay_cnt != 0 {
            *k &= pt.slot_delay_mask;
        }
        let v = *k | self.proc_ch.wrapping_sub(1) | (u8::from(self.opna_a1) << 2);
        w.writereg(0x28, u32::from(v));
    }

    // 0547
    fn adpcm_keyon(&mut self, w: &mut Work, p: usize) {
        let pt = self.parts[p];
        if pt.actual_note == 0xff {
            return;
        }
        w.writereg(0x101, 0x02);
        w.writereg(0x100, 0x21);
        w.writereg(0x102, u32::from(self.adpcm_start & 0xff));
        w.writereg(0x103, u32::from(self.adpcm_start >> 8));
        w.writereg(0x104, u32::from(self.adpcm_stop & 0xff));
        w.writereg(0x105, u32::from(self.adpcm_stop >> 8));
        if self.adpcm_start_loop == 0 && self.adpcm_stop_loop == 0 {
            w.writereg(0x100, 0xa0);
            w.writereg(0x101, u32::from(pt.pan | 0x02));
        } else {
            w.writereg(0x100, 0xb0);
            w.writereg(0x101, u32::from(pt.pan | 0x02));
            w.writereg(0x102, u32::from(self.adpcm_start_loop & 0xff));
            w.writereg(0x103, u32::from(self.adpcm_start_loop >> 8));
            w.writereg(0x104, u32::from(self.adpcm_stop_loop & 0xff));
            w.writereg(0x105, u32::from(self.adpcm_stop_loop >> 8));
        }
    }

    // 0bf6
    fn ppz8_keyon(&mut self, w: &mut Work, p: usize) {
        if self.parts[p].actual_note == 0xff {
            return;
        }
        w.ppz8.play(self.proc_ch, self.parts[p].tonenum);
    }

    // 27d8
    fn fm_freq_out(&mut self, w: &mut Work, p: usize) {
        let pt = self.parts[p];
        let mut fnum = pt.actual_freq;
        if fnum == 0 || pt.fm_slotmask == 0 {
            return;
        }
        let blk = fnum & 0x3800;
        fnum &= 0x7ff;
        fnum = fnum.wrapping_add(pt.portamento_diff as u16).wrapping_add(pt.detune as u16);
        if self.proc_ch != 3 || self.opna_a1 || self.fm3ex_mode == 0x3f {
            if pt.lfof.freq {
                fnum = fnum.wrapping_add(pt.lfo_diff as u16);
            }
            if pt.lfof_b.freq {
                fnum = fnum.wrapping_add(pt.lfo_diff_b as u16);
            }
            let b = blkfnum_normalize(blk, fnum);
            let b = (b | (b >> 16)) & 0xffff;
            self.parts[p].output_freq = b;
            self.reg_write(w, self.proc_ch.wrapping_add(0xa3), (b >> 8) as u8);
            self.reg_write(w, self.proc_ch.wrapping_add(0x9f), b as u8);
        } else {
            let mut l = pt.vol_lfo_slotmask;
            if l & 0x0f == 0 {
                l = 0xf0;
            }
            let mut lb = pt.vol_lfo_slotmask_b;
            if lb & 0x0f == 0 {
                lb = 0xf0;
            }
            const ADDR: [[u8; 4]; 2] = [[0xad, 0xae, 0xac, 0xa6], [0xa9, 0xaa, 0xa8, 0xa2]];
            for s in (0..4).rev() {
                let bit = 1u8 << (4 + s);
                if pt.fm_slotmask & bit != 0 {
                    let mut f = fnum.wrapping_add(self.fm3ex_det[s] as u16);
                    if l & bit != 0 && pt.lfof.freq {
                        f = f.wrapping_add(pt.lfo_diff as u16);
                    }
                    if lb & bit != 0 && pt.lfof_b.freq {
                        f = f.wrapping_add(pt.lfo_diff_b as u16);
                    }
                    let b = blkfnum_normalize(blk, f);
                    let b = (b | (b >> 16)) & 0xffff;
                    self.parts[p].output_freq = b;
                    self.reg_write(w, ADDR[0][s], (b >> 8) as u8);
                    self.reg_write(w, ADDR[1][s], b as u8);
                }
            }
        }
    }

    // After a note: the bookkeeping each kind of part shares.
    fn note_done(&mut self, p: usize) {
        let pt = &mut self.parts[p];
        pt.note_proc = pt.note_proc.wrapping_add(1);
        self.no_keyoff = false;
        self.volume_saved = false;
        pt.keystatus.off = false;
        pt.keystatus.off_mask = false;
        let ptr = pt.ptr;
        if u32::from(self.datalen) > u32::from(ptr) + 1 && self.d(ptr) == 0xfb {
            self.parts[p].keystatus.off_mask = true;
        }
        self.loop_check(p);
    }

    fn volume_save_check(&mut self, p: usize) {
        let pt = &mut self.parts[p];
        if pt.volume_save != 0 && pt.actual_note != 0xff {
            if !self.volume_saved {
                pt.volume_save = 0;
            }
            self.volume_saved = false;
        }
    }

    // 13b5
    fn fm_out(&mut self, w: &mut Work, p: usize) {
        self.volume_save_check(p);
        self.fm_vol_out(w, p);
        self.fm_freq_out(w, p);
        self.fm_keyon(w, p);
        self.note_done(p);
    }

    // 01b1
    fn adpcm_out(&mut self, w: &mut Work, p: usize) {
        self.volume_save_check(p);
        self.adpcm_vol_out(w, p);
        self.adpcm_freq_out(w, p);
        if self.parts[p].keystatus.off {
            self.adpcm_keyon(w, p);
        }
        self.note_done(p);
    }

    // 0803
    fn ppz8_out(&mut self, w: &mut Work, p: usize) {
        self.volume_save_check(p);
        self.ppz8_vol_out(w, p);
        self.ppz8_freq_out(w, p);
        if self.parts[p].keystatus.off {
            self.ppz8_keyon(w, p);
        }
        self.note_done(p);
    }

    // 1f7e
    fn fm_freq_out_mask(&mut self, w: &mut Work, p: usize) {
        if !masked(&self.parts[p]) {
            self.fm_freq_out(w, p);
        }
    }

    // 1ee3
    fn fm3ex_mode_update(&mut self, w: &mut Work, p: usize) {
        let partbit = match p {
            FM_3 => 1,
            FM_3B => 2,
            12 => 4,
            _ => 8,
        };
        let pt = self.parts[p];
        let fm3ex = if pt.fm_slotmask & 0xf0 == 0 {
            false
        } else if pt.fm_slotmask != 0xf0 {
            true
        } else if pt.vol_lfo_slotmask & 0x0f == 0 {
            false
        } else if pt.lfof.freq {
            true
        } else if pt.vol_lfo_slotmask_b & 0x0f == 0 {
            false
        } else {
            pt.lfof_b.freq
        };
        if fm3ex {
            self.fm3ex_needed |= partbit;
        } else {
            self.fm3ex_needed &= !partbit;
        }
        let mode = if self.fm3ex_needed != 0 || self.fm3ex_force { 0x7f } else { 0x3f };
        if mode == self.fm3ex_mode {
            return;
        }
        self.fm3ex_mode = mode;
        w.writereg(0x27, u32::from(mode & 0xcf));
        if mode == 0x3f || p == FM_3 {
            return;
        }
        self.fm_freq_out_mask(w, FM_3);
        if p == FM_3B {
            return;
        }
        self.fm_freq_out_mask(w, FM_3B);
        if p == 12 {
            return;
        }
        self.fm_freq_out_mask(w, 12);
    }

    // 1d7b
    fn fm3ex_mode_update_check(&mut self, w: &mut Work, p: usize) -> bool {
        if self.proc_ch == 3 && !self.opna_a1 {
            self.fm3ex_mode_update(w, p);
            return true;
        }
        false
    }
}

// 25cc
fn hlfo_delay_check(p: &Part, pan: u8) -> u8 {
    if p.hlfo_delay != 0 {
        pan & 0xc0
    } else {
        pan
    }
}

// 265d
fn opnarhythm_inc(inc: &mut [u8; 6], val: u8) {
    for (i, x) in inc.iter_mut().enumerate() {
        if val & (1 << i) != 0 {
            *x = x.wrapping_add(1);
        }
    }
}

fn clamp_u8(v: i32, lo: i32, hi: i32) -> u8 {
    v.clamp(lo, hi) as u8
}

impl Pmd {
    fn load16(&mut self, p: usize) -> u16 {
        let lo = u16::from(self.cmdload(p));
        lo | u16::from(self.cmdload(p)) << 8
    }

    // 2268
    fn c_ff_tonenum(&mut self, w: &mut Work, p: usize) {
        let tonenum = self.cmdload(p);
        self.parts[p].tonenum = tonenum;
        if !masked(&self.parts[p]) {
            self.tone_fm(w, p);
            return;
        }
        let Some(t) = self.toneptr(tonenum) else { return };
        let fb = self.d(t + 0x18);
        let tl = [self.d(t + 4), self.d(t + 5), self.d(t + 6), self.d(t + 7)];
        let pt = &mut self.parts[p];
        pt.fb_alg = fb;
        pt.fm_tone_tl = tl;
        if self.proc_ch == 3 && pt.fm_tone_slotmask != 0 && !self.opna_a1 {
            let mut fb_alg = pt.fb_alg;
            if pt.fm_slotmask & 0x10 == 0 {
                fb_alg &= 0x07;
                fb_alg |= self.fm3ex_fb_alg & 0x38;
            }
            self.fm3ex_fb_alg = fb_alg;
            pt.fb_alg = fb_alg;
        }
    }

    // 046c
    fn c_ff_tonenum_adpcm(&mut self, _w: &mut Work, p: usize) {
        let t = self.cmdload(p);
        self.parts[p].tonenum = t;
        self.adpcm_start = self.adpcm_addr[usize::from(t)][0];
        self.adpcm_stop = self.adpcm_addr[usize::from(t)][1];
        self.adpcm_start_loop = 0;
        self.adpcm_stop_loop = 0;
        self.adpcm_release = 0x8000;
    }

    // 0afd
    fn c_ff_tonenum_ppz8(&mut self, w: &mut Work, p: usize) {
        let t = self.cmdload(p);
        self.parts[p].tonenum = t;
        w.ppz8.loop_voice(self.proc_ch, t);
    }

    // 22b3
    fn c_fe_gate_abs(&mut self, _w: &mut Work, p: usize) {
        self.parts[p].gate_abs = self.cmdload(p);
        self.parts[p].gate_rand_range = 0;
    }

    // 22cb
    fn c_fd_vol(&mut self, _w: &mut Work, p: usize) {
        self.parts[p].vol = self.cmdload(p);
    }

    // 22d0
    fn c_fc_tempo(&mut self, w: &mut Work, p: usize) {
        let val = self.cmdload(p);
        if val < 0xfb {
            self.timerb = val;
            self.timerb_bak = val;
            self.calc_tempo();
        } else if val == 0xff {
            let tempo = self.cmdload(p).max(0x12);
            self.tempo = tempo;
            self.tempo_bak = tempo;
            self.calc_tempo_rev();
        } else if val == 0xfe {
            let d = i32::from(u8s8(self.cmdload(p)));
            let t = clamp_u8(i32::from(self.timerb_bak) + d, 0, 0xfa);
            self.timerb = t;
            self.timerb_bak = t;
            self.calc_tempo();
        } else {
            let d = i32::from(u8s8(self.cmdload(p)));
            let t = clamp_u8(i32::from(self.tempo_bak) + d, 0x12, 0xff);
            self.tempo = t;
            self.tempo_bak = t;
            self.calc_tempo_rev();
        }
        w.timerb = self.timerb;
    }

    // 236b
    fn c_fb_tie(&mut self, _w: &mut Work, _p: usize) {
        self.no_keyoff = true;
    }

    // 2371
    fn c_fa_det(&mut self, _w: &mut Work, p: usize) {
        self.parts[p].detune = u16s16(self.load16(p));
    }

    // 2376
    fn c_d5_detrel(&mut self, _w: &mut Work, p: usize) {
        let v = u16s16(self.load16(p));
        self.parts[p].detune = self.parts[p].detune.wrapping_add(v);
    }

    // 237b
    fn c_f9_repeat_reset(&mut self, _w: &mut Work, p: usize) {
        let ptr = self.load16(p).wrapping_add(1);
        if u32::from(self.datalen) < u32::from(ptr) + 1 {
            self.parts[p].ptr = 0;
        } else {
            self.set_d(ptr, 0);
        }
    }

    // 2391
    fn c_f8_repeat(&mut self, _w: &mut Work, p: usize) {
        let repeat = self.cmdload(p);
        if repeat != 0 {
            let ptr = self.parts[p].ptr;
            if u32::from(self.datalen) < u32::from(ptr) + 1 {
                self.parts[p].ptr = 0;
                return;
            }
            self.set_d(ptr, self.d(ptr).wrapping_add(1));
            let cnt = self.cmdload(p);
            if repeat == cnt {
                self.parts[p].ptr = self.parts[p].ptr.wrapping_add(2);
                return;
            }
        } else {
            let pt = &mut self.parts[p];
            pt.ptr = pt.ptr.wrapping_add(1);
            pt.loop_.looped = true;
            pt.loop_.ended = false;
        }
        let ptr = self.load16(p).wrapping_add(2);
        self.parts[p].ptr = ptr;
    }

    // 23bd
    fn c_f7_repeat_exit(&mut self, _w: &mut Work, p: usize) {
        let ptr = self.load16(p);
        if u32::from(self.datalen) < u32::from(ptr) + 2 {
            self.parts[p].ptr = 0;
            return;
        }
        let repeat = i32::from(self.d(ptr));
        let cnt = i32::from(self.d(ptr + 1));
        if cnt == repeat - 1 {
            self.parts[p].ptr = ptr.wrapping_add(4);
        }
    }

    // 23de
    fn c_f6_set_loop(&mut self, _w: &mut Work, p: usize) {
        self.parts[p].loop_ptr = self.parts[p].ptr;
    }

    // 23e2
    fn c_f5_transpose(&mut self, _w: &mut Work, p: usize) {
        self.parts[p].transpose = self.cmdload(p);
    }

    // 23f4
    fn c_f4_volinc_fm(&mut self, _w: &mut Work, p: usize) {
        let pt = &mut self.parts[p];
        pt.vol = pt.vol.wrapping_add(4).min(0x7f);
    }

    // 2409
    fn c_f4_volinc_ssg(&mut self, _w: &mut Work, p: usize) {
        let pt = &mut self.parts[p];
        if pt.vol < 0xf {
            pt.vol += 1;
        }
    }

    // 0421
    fn c_f4_volinc_adpcm(&mut self, _w: &mut Work, p: usize) {
        let pt = &mut self.parts[p];
        pt.vol = pt.vol.saturating_add(0x10);
    }

    // 241c
    fn c_f3_voldec_fm(&mut self, _w: &mut Work, p: usize) {
        let pt = &mut self.parts[p];
        pt.vol = pt.vol.saturating_sub(4);
    }

    // 2435
    fn c_f3_voldec_ssg(&mut self, _w: &mut Work, p: usize) {
        let pt = &mut self.parts[p];
        pt.vol = pt.vol.saturating_sub(1);
    }

    // 0434
    fn c_f3_voldec_adpcm(&mut self, _w: &mut Work, p: usize) {
        let pt = &mut self.parts[p];
        pt.vol = pt.vol.saturating_sub(0x10);
    }

    // 24ea
    fn c_f2_lfo(&mut self, _w: &mut Work, p: usize) {
        let l = Lfo {
            delay: self.cmdload(p),
            speed: self.cmdload(p),
            step: self.cmdload(p),
            times: self.cmdload(p),
        };
        let pt = &mut self.parts[p];
        pt.lfo_set = l;
        pt.lfo = l;
        lfo_reset(pt);
    }

    // 2513
    fn c_f1_lfo_switch(&mut self, _w: &mut Work, p: usize) {
        let mut v = self.cmdload(p);
        if v & 0xf8 != 0 {
            v = 1;
        }
        let pt = &mut self.parts[p];
        pt.lfof.freq = v & 1 != 0;
        pt.lfof.vol = v & 2 != 0;
        pt.lfof.sync = v & 4 != 0;
        lfo_reset(pt);
    }

    // 2526
    fn c_f1_lfo_switch_fm(&mut self, w: &mut Work, p: usize) {
        self.c_f1_lfo_switch(w, p);
        self.fm3ex_mode_update_check(w, p);
    }

    // 1d31: PPSDRV, which isn't played
    fn c_f1_ppsdrv(&mut self, _w: &mut Work, p: usize) {
        self.cmdload(p);
    }

    // 252c
    fn c_f0_env_old(&mut self, _w: &mut Work, p: usize) {
        let v = [self.cmdload(p), self.cmdload(p), self.cmdload(p), self.cmdload(p)];
        let pt = &mut self.parts[p];
        pt.ssg_env_param_set[P_OLD_AL] = v[0];
        pt.ssg_env_param[P_OLD_AL] = v[0];
        pt.ssg_env_param_set[P_OLD_AD] = v[1];
        pt.ssg_env_param_set[P_OLD_SR] = v[2];
        pt.ssg_env_param[P_OLD_SR] = v[2];
        pt.ssg_env_param_set[P_OLD_RR] = v[3];
        pt.ssg_env_param[P_OLD_RR] = v[3];
        if pt.ssg_env_state_old == OLD_NEW {
            pt.ssg_env_state_old = OLD_RR;
            pt.ssg_env_vol = (-15i8) as u8;
        }
    }

    // 2554
    fn c_ef_poke(&mut self, w: &mut Work, p: usize) {
        let reg = self.cmdload(p);
        let data = self.cmdload(p);
        self.reg_write(w, reg, data);
    }

    // 255c
    fn c_ee_noise_freq(&mut self, _w: &mut Work, p: usize) {
        self.ssg_noise_freq = self.cmdload(p);
    }

    // 2576
    fn c_ed_ssgmix(&mut self, _w: &mut Work, p: usize) {
        self.parts[p].ssg_mix = self.cmdload(p);
    }

    fn set_fm3_pans(&mut self, pan: u8) {
        if self.proc_ch == 3 && !self.opna_a1 {
            for i in [FM_3, FM_3B, 12, FM_3D] {
                self.parts[i].pan = pan;
            }
        }
    }

    // 257c
    fn set_pan(&mut self, w: &mut Work, p: usize, pan: u8) {
        let v = ((pan & 3) << 6) | (self.parts[p].pan & 0x3f);
        self.parts[p].pan = v;
        self.set_fm3_pans(v);
        if masked(&self.parts[p]) {
            return;
        }
        let v = hlfo_delay_check(&self.parts[p], v);
        self.reg_write(w, 0xb3u8.wrapping_add(self.proc_ch), v);
    }

    // 257b
    fn c_ec_pan(&mut self, w: &mut Work, p: usize) {
        let v = self.cmdload(p);
        self.set_pan(w, p, v);
    }

    // 044d
    fn c_ec_pan_adpcm(&mut self, _w: &mut Work, p: usize) {
        self.parts[p].pan = self.cmdload(p) << 6;
    }

    // 0aca
    fn c_ec_pan_ppz8(&mut self, w: &mut Work, p: usize) {
        let pan = self.cmdload(p);
        const T: [u8; 4] = [0, 9, 1, 5];
        self.parts[p].pan = if pan < 4 { T[usize::from(pan)] } else { 0 };
        w.ppz8.pan(self.proc_ch, self.parts[p].pan);
    }

    // 25ed
    fn c_eb_opnarhythm(&mut self, w: &mut Work, p: usize) {
        let val = self.cmdload(p) & self.opnarhythm_mask;
        if val == 0 {
            return;
        }
        if self.fadeout_vol != 0 {
            self.opnarhythm_tl_write(w, self.opnarhythm_tl);
        }
        if val & 0x80 == 0 {
            for i in 0..6 {
                if val & (1 << i) != 0 {
                    w.writereg(0x18 + i, u32::from(self.opnarhythm_ilpan[i as usize]));
                }
            }
        }
        w.writereg(0x10, u32::from(val));
        if val & 0x80 != 0 {
            opnarhythm_inc(&mut self.opnarhythm_dump_inc, val);
            self.opnarhythm &= val ^ 0xff;
        } else {
            opnarhythm_inc(&mut self.opnarhythm_shot_inc, val);
            self.opnarhythm |= val;
        }
    }

    // 266c
    fn c_ea_opnarhythm_il(&mut self, w: &mut Work, p: usize) {
        let val = self.cmdload(p);
        let i = (val >> 5).wrapping_sub(1);
        if i >= 6 {
            return;
        }
        let v = (self.opnarhythm_ilpan[usize::from(i)] & !0x1f) | (val & 0x1f);
        self.opnarhythm_ilpan[usize::from(i)] = v;
        w.writereg(0x18 + u32::from(i), u32::from(v));
    }

    // 26c8
    fn c_e9_opnarhythm_pan(&mut self, w: &mut Work, p: usize) {
        let val = self.cmdload(p);
        let i = (val >> 5).wrapping_sub(1);
        if i >= 6 {
            return;
        }
        let v = (self.opnarhythm_ilpan[usize::from(i)] & !0xc0) | ((val & 3) << 6);
        self.opnarhythm_ilpan[usize::from(i)] = v;
        w.writereg(0x18 + u32::from(i), u32::from(v));
    }

    // 26d8
    fn c_e8_opnarhythm_tl(&mut self, w: &mut Work, p: usize) {
        let mut tl = self.cmdload(p);
        if self.opnarhythm_voldown != 0 {
            tl = ((u32::from(tl) * (0x100 - u32::from(self.opnarhythm_voldown))) >> 8) as u8;
        }
        self.opnarhythm_tl = tl;
        if self.fadeout_vol != 0 {
            tl = ((u32::from(tl) * (0x100 - u32::from(self.fadeout_vol))) >> 8) as u8;
        }
        w.writereg(0x11, u32::from(tl));
    }

    // 23e7
    fn c_e7_transpose_rel(&mut self, _w: &mut Work, p: usize) {
        let v = self.cmdload(p);
        self.parts[p].transpose = self.parts[p].transpose.wrapping_add(v);
    }

    // 26fe
    fn c_e6_opnarhythm_tl_rel(&mut self, w: &mut Work, p: usize) {
        let d = i32::from(u8s8(self.cmdload(p)));
        let mut tl = clamp_u8(i32::from(self.opnarhythm_tl) + d, 0, 0x3f);
        self.opnarhythm_tl = tl;
        if self.fadeout_vol != 0 {
            tl = ((u32::from(tl) * (0x100 - u32::from(self.fadeout_vol))) >> 8) as u8;
        }
        w.writereg(0x11, u32::from(tl));
    }

    // 2695
    fn c_e5_opnarhythm_il_rel(&mut self, w: &mut Work, p: usize) {
        let i = self.cmdload(p).wrapping_sub(1);
        if i >= 6 {
            self.parts[p].ptr = self.parts[p].ptr.wrapping_add(1);
            return;
        }
        let i = usize::from(i);
        let d = i32::from(u8s8(self.cmdload(p)));
        let il = clamp_u8(i32::from(self.opnarhythm_ilpan[i] & 0x1f) + d, 0, 0x1f);
        self.opnarhythm_ilpan[i] = (self.opnarhythm_ilpan[i] & !0x1f) | il;
        w.writereg(0x18 + i as u32, u32::from(self.opnarhythm_ilpan[i]));
    }

    // 225e
    fn c_e4_hlfo_delay(&mut self, _w: &mut Work, p: usize) {
        self.parts[p].hlfo_delay_set = self.cmdload(p);
    }

    // 2403
    fn c_e3_vol_add_fm(&mut self, _w: &mut Work, p: usize) {
        let v = self.cmdload(p);
        let pt = &mut self.parts[p];
        pt.vol = pt.vol.wrapping_add(v).min(0x7f);
    }

    // 2416
    fn c_e3_vol_add_ssg(&mut self, _w: &mut Work, p: usize) {
        let v = self.cmdload(p);
        let pt = &mut self.parts[p];
        pt.vol = pt.vol.wrapping_add(v).min(0xf);
    }

    // 042e
    fn c_e3_vol_add_adpcm(&mut self, _w: &mut Work, p: usize) {
        let v = self.cmdload(p);
        let pt = &mut self.parts[p];
        pt.vol = pt.vol.saturating_add(v);
    }

    // 2427
    fn c_e2_vol_sub(&mut self, _w: &mut Work, p: usize) {
        let v = self.cmdload(p);
        let pt = &mut self.parts[p];
        pt.vol = pt.vol.saturating_sub(v);
    }

    // 2209
    fn c_e1_ams_pms(&mut self, w: &mut Work, p: usize) {
        let v = self.cmdload(p) | (self.parts[p].pan & 0xc0);
        self.parts[p].pan = v;
        self.set_fm3_pans(v);
        if masked(&self.parts[p]) {
            return;
        }
        let v = hlfo_delay_check(&self.parts[p], v);
        self.reg_write(w, 0xb3u8.wrapping_add(self.proc_ch), v);
    }

    // 2252
    fn c_e0_hlfo(&mut self, w: &mut Work, p: usize) {
        let v = self.cmdload(p);
        self.opna_last22 = v;
        w.writereg(0x22, u32::from(v));
    }

    // 2263
    fn c_df_meas_len(&mut self, _w: &mut Work, p: usize) {
        self.meas_len = self.cmdload(p);
    }

    fn echo(&mut self, p: usize, vol: u8) {
        self.parts[p].volume_save = vol.wrapping_add(1);
        self.volume_saved = true;
    }

    // 21cc
    fn c_de_echo_init_add_fm(&mut self, _w: &mut Work, p: usize) {
        let v = self.parts[p].vol.wrapping_add(self.cmdload(p)).min(0x7f);
        self.echo(p, v);
    }

    // 21e1
    fn c_de_echo_init_add_ssg(&mut self, _w: &mut Work, p: usize) {
        let v = self.parts[p].vol.wrapping_add(self.cmdload(p)).min(0xf);
        self.echo(p, v);
    }

    // 21ed
    fn c_de_echo_init_add_adpcm(&mut self, _w: &mut Work, p: usize) {
        let v = (u32::from(self.parts[p].vol) + u32::from(self.cmdload(p))).min(0xfe) as u8;
        self.echo(p, v);
    }

    // 21fb
    fn c_dd_echo_init_sub(&mut self, _w: &mut Work, p: usize) {
        let v = self.parts[p].vol.saturating_sub(self.cmdload(p));
        self.echo(p, v);
    }

    // 21be
    fn c_dc_status1(&mut self, _w: &mut Work, p: usize) {
        self.status1 = self.cmdload(p);
    }

    // 21c3
    fn c_db_status1_add(&mut self, _w: &mut Work, p: usize) {
        self.status1 = self.status1.wrapping_add(self.cmdload(p));
    }

    fn portamento_set(&mut self, p: usize, diff: i32, clocks: u8) {
        let (mut a, mut r) = (0i16, 0i16);
        if clocks != 0 {
            a = (diff / i32::from(clocks)) as i16;
            r = (diff % i32::from(clocks)) as i16;
        }
        let pt = &mut self.parts[p];
        pt.portamento_add = a;
        pt.portamento_rem = r;
        pt.lfof.portamento = true;
    }

    // 2104
    fn c_da_portamento_fm(&mut self, w: &mut Work, p: usize) {
        let note = self.cmdload(p);
        if masked(&self.parts[p]) {
            return;
        }
        let note = self.lfo_init_fm(w, p, note);
        let note = note_transpose(&self.parts[p], note);
        note_freq_fm(&mut self.parts[p], note);
        let (f1, n1) = (self.parts[p].actual_freq, self.parts[p].actual_note);
        let note = self.cmdload(p);
        let note = note_transpose(&self.parts[p], note);
        note_freq_fm(&mut self.parts[p], note);
        let f2 = self.parts[p].actual_freq;
        self.parts[p].actual_freq = f1;
        self.parts[p].actual_note = n1;
        let blkdiff = (i32::from((f2 >> 11) as u8) - i32::from((f1 >> 11) as u8)) * 0x26a;
        let diff = i32::from(f2 & 0x7ff) - i32::from(f1 & 0x7ff) + blkdiff;
        let clocks = self.cmdload(p);
        self.parts[p].len = clocks;
        self.parts[p].len_cnt = clocks;
        self.calc_gate(p);
        self.portamento_set(p, diff, clocks);
        self.fm_out(w, p);
    }

    // 2176
    fn c_da_portamento_ssg(&mut self, w: &mut Work, p: usize) {
        let note = self.cmdload(p);
        if masked(&self.parts[p]) {
            return;
        }
        let note = self.lfo_init_ssg(w, p, note);
        let note = note_transpose(&self.parts[p], note);
        note_freq_ssg(&mut self.parts[p], note);
        let (f1, n1) = (self.parts[p].actual_freq, self.parts[p].actual_note);
        let note = self.cmdload(p);
        let note = note_transpose(&self.parts[p], note);
        note_freq_ssg(&mut self.parts[p], note);
        let f2 = self.parts[p].actual_freq;
        self.parts[p].actual_freq = f1;
        self.parts[p].actual_note = n1;
        let clocks = self.cmdload(p);
        self.parts[p].len = clocks;
        self.parts[p].len_cnt = clocks;
        let diff = i32::from(f2) - i32::from(f1);
        self.calc_gate(p);
        self.portamento_set(p, diff, clocks);
        self.ssg_out(w, p);
    }

    // 03d6
    fn c_da_portamento_adpcm(&mut self, w: &mut Work, p: usize) {
        let note = self.cmdload(p);
        if masked(&self.parts[p]) {
            return;
        }
        // As 98fmplayer has it: the note from the LFO's set-up isn't used.
        self.lfo_init_ssg(w, p, note);
        let note = note_transpose(&self.parts[p], note);
        note_freq_adpcm(&mut self.parts[p], note);
        let (f1, n1) = (self.parts[p].actual_freq, self.parts[p].actual_note);
        let note = self.cmdload(p);
        let note = note_transpose(&self.parts[p], note);
        note_freq_adpcm(&mut self.parts[p], note);
        let f2 = self.parts[p].actual_freq;
        self.parts[p].actual_freq = f1;
        self.parts[p].actual_note = n1;
        let diff = i32::from(f2) - i32::from(f1);
        let clocks = self.cmdload(p);
        self.parts[p].len = clocks;
        self.parts[p].len_cnt = clocks;
        self.calc_gate(p);
        self.portamento_set(p, diff, clocks);
        self.adpcm_out(w, p);
    }

    // 0a62
    fn c_da_portamento_ppz8(&mut self, w: &mut Work, p: usize) {
        let note = self.cmdload(p);
        if masked(&self.parts[p]) {
            return;
        }
        self.lfo_init_ssg(w, p, note);
        let note = note_transpose(&self.parts[p], note);
        note_freq_ppz8(&mut self.parts[p], note);
        let f = |pt: &Part| (u32::from(pt.actual_freq_upper) << 16) | u32::from(pt.actual_freq);
        let f1 = f(&self.parts[p]);
        let n1 = self.parts[p].actual_note;
        let note = self.cmdload(p);
        let note = note_transpose(&self.parts[p], note);
        note_freq_ppz8(&mut self.parts[p], note);
        let f2 = f(&self.parts[p]);
        self.parts[p].actual_freq = f1 as u16;
        self.parts[p].actual_freq_upper = (f1 >> 16) as u16;
        self.parts[p].actual_note = n1;
        let diff = i32::from(u16s16((f2.wrapping_sub(f1) >> 4) as u16));
        let clocks = self.cmdload(p);
        self.parts[p].len = clocks;
        self.parts[p].len_cnt = clocks;
        self.calc_gate(p);
        self.portamento_set(p, diff, clocks);
        self.ppz8_out(w, p);
    }

    // 20bf
    fn c_d6_md(&mut self, _w: &mut Work, p: usize) {
        let s = self.cmdload(p);
        let d = self.cmdload(p);
        let pt = &mut self.parts[p];
        pt.md_speed = s;
        pt.md_speed_set = s;
        pt.md_depth = d;
    }

    // 2054
    fn c_d4_ssgeff(&mut self, w: &mut Work, p: usize) {
        let c = self.cmdload(p);
        if masked(&self.parts[p]) {
            return;
        }
        if c != 0 {
            self.ssg_effect(w, c);
        } else {
            self.ssgeff_stop(w);
        }
    }

    // 206f: FM effects, which 98fmplayer doesn't play
    fn c_d3_fmeff(&mut self, _w: &mut Work, p: usize) {
        self.cmdload(p);
    }

    // 20b6: fade-out, which 98fmplayer doesn't do
    fn c_d2_fadeout(&mut self, _w: &mut Work, p: usize) {
        self.cmdload(p);
    }

    // 2561
    fn c_d0_ssgnoisefreqadd(&mut self, _w: &mut Work, p: usize) {
        let d = i32::from(u8s8(self.cmdload(p)));
        self.ssg_noise_freq = clamp_u8(i32::from(self.ssg_noise_freq) + d, 0, 0x1f);
    }

    // 2044
    fn fm_keyon_if_not_masked(&mut self, w: &mut Work, p: usize) {
        if !masked(&self.parts[p]) && !self.parts[p].keystatus.off_mask {
            self.fm_keyon(w, p);
        }
    }

    // 1f88
    fn c_cf_slotmask(&mut self, w: &mut Work, p: usize) {
        let mut data = self.cmdload(p);
        if data & 0x0f != 0 {
            self.parts[p].fm_slotout = data << 4;
        } else {
            let mut alg = 0;
            if self.proc_ch == 3 && !self.opna_a1 {
                alg = self.fm3ex_fb_alg;
            } else if let Some(t) = self.toneptr(self.parts[p].tonenum) {
                alg = self.d(t + 0x18);
            }
            self.parts[p].fm_slotout = ALG_OUTPUT_SLOT[usize::from(alg & 7)];
        }
        data &= 0xf0;
        if data == self.parts[p].fm_slotmask {
            return;
        }
        self.parts[p].fm_slotmask = data;
        self.parts[p].mask.slot = data == 0;
        if self.fm3ex_mode_update_check(w, p) && p != FM_3 {
            self.fm_keyon_if_not_masked(w, FM_3);
            if p != FM_3B {
                self.fm_keyon_if_not_masked(w, FM_3B);
                if p != 12 {
                    self.fm_keyon_if_not_masked(w, 12);
                }
            }
        }
        let pt = &mut self.parts[p];
        let mut tm = 0;
        if pt.fm_slotmask & 0x80 != 0 {
            tm |= 0x11;
        }
        if pt.fm_slotmask & 0x40 != 0 {
            tm |= 0x44;
        }
        if pt.fm_slotmask & 0x20 != 0 {
            tm |= 0x22;
        }
        if pt.fm_slotmask & 0x10 != 0 {
            tm |= 0x88;
        }
        pt.fm_tone_slotmask = tm;
        pt.proc_masked = masked(pt);
    }

    // 039a
    fn c_ce_adpcm_loop(&mut self, _w: &mut Work, p: usize) {
        let mut d = self.load16(p);
        d = d.wrapping_add(if d & 0x8000 == 0 { self.adpcm_start } else { self.adpcm_stop });
        self.adpcm_start_loop = d;
        // As 98fmplayer has it: bit 7, not 15.
        let mut d = self.load16(p);
        d = d.wrapping_add(if d != 0 && d & 0x80 == 0 { self.adpcm_start } else { self.adpcm_stop });
        self.adpcm_stop_loop = d;
        let mut d = self.load16(p);
        if d != 0x8000 {
            d = d.wrapping_add(if d & 0x80 == 0 { self.adpcm_start } else { self.adpcm_stop });
        }
        self.adpcm_release = d;
    }

    // 0a04
    fn c_ce_ppz8_loop(&mut self, w: &mut Work, p: usize) {
        let mut start = u32::from(self.load16(p));
        let mut end = u32::from(self.load16(p));
        let len = w.ppz8.voice_length(self.parts[p].tonenum);
        if start & 0x8000 != 0 {
            start = len.wrapping_add(i32::from(u16s16(start as u16)) as u32);
        }
        if end & 0x8000 != 0 {
            end = len.wrapping_add(i32::from(u16s16(end as u16)) as u32);
        }
        w.ppz8.loop_offset(self.proc_ch, start, end);
        self.cmdload(p);
        self.cmdload(p);
    }

    // 1e1b
    fn c_cd_env_new(&mut self, _w: &mut Work, p: usize) {
        let v = [self.cmdload(p), self.cmdload(p), self.cmdload(p), self.cmdload(p), self.cmdload(p)];
        let pt = &mut self.parts[p];
        pt.ssg_env_param_set[P_NEW_AR] = v[0] & 0x1f;
        pt.ssg_env_param_set[P_NEW_DR] = v[1] & 0x1f;
        pt.ssg_env_param_set[P_NEW_SR] = v[2] & 0x1f;
        pt.ssg_env_param_set[P_NEW_RR] = v[3] & 0xf;
        pt.ssg_env_param_sl = (v[3] >> 4) ^ 0xf;
        pt.ssg_env_param_al = v[4] & 0xf;
        if pt.ssg_env_state_old != OLD_NEW {
            pt.ssg_env_state_old = OLD_NEW;
            pt.ssg_env_state_new = NEW_RR;
            pt.ssg_env_vol = 0;
        }
    }

    // 1def
    fn c_cc_det_ext(&mut self, _w: &mut Work, p: usize) {
        self.parts[p].flagext.detune = self.cmdload(p) & 1 != 0;
    }

    // 1e16
    fn c_cb_lfo_waveform(&mut self, _w: &mut Work, p: usize) {
        self.parts[p].lfo_waveform = self.cmdload(p);
    }

    // 1dfa
    fn c_ca_lfo_ext(&mut self, _w: &mut Work, p: usize) {
        self.parts[p].flagext.lfo = self.cmdload(p) & 1 != 0;
    }

    // 1e07
    fn c_c9_env_ext(&mut self, _w: &mut Work, p: usize) {
        self.parts[p].flagext.env = self.cmdload(p) & 1 != 0;
    }

    fn fm3ex_force_update(&mut self, w: &mut Work, p: usize) {
        self.fm3ex_force = self.fm3ex_det.iter().any(|&d| d != 0);
        self.fm3ex_mode_update(w, p);
    }

    // 1e99
    fn c_c8_fm3ex_det(&mut self, w: &mut Work, p: usize) {
        if self.proc_ch != 3 || self.opna_a1 {
            return self.cmd_null(p, 3);
        }
        let mask = self.cmdload(p);
        let det = u16s16(self.load16(p));
        for s in 0..4 {
            if mask & (1 << s) != 0 {
                self.fm3ex_det[s] = det;
            }
        }
        self.fm3ex_force_update(w, p);
    }

    // 1e5f
    fn c_c7_fm3ex_det_add(&mut self, w: &mut Work, p: usize) {
        if self.proc_ch != 3 || self.opna_a1 {
            return self.cmd_null(p, 3);
        }
        let mask = self.cmdload(p);
        let det = self.load16(p);
        for s in 0..4 {
            if mask & (1 << s) != 0 {
                self.fm3ex_det[s] = (self.fm3ex_det[s] as u16).wrapping_add(det) as i16;
            }
        }
        self.fm3ex_force_update(w, p);
    }

    // 1d90, 1dc0
    fn c_c6_fm3ex_init(&mut self, _w: &mut Work, p: usize) {
        for i in 0..3 {
            let ptr = self.load16(p);
            if ptr == 0 {
                continue;
            }
            let pan = self.parts[FM_3].pan;
            let pt = &mut self.parts[FM_3B + i];
            pt.ptr = ptr;
            pt.len = 1;
            pt.len_cnt = 1;
            pt.keystatus.off = true;
            pt.keystatus.off_mask = true;
            pt.md_cnt = 0xff;
            pt.md_cnt_set = 0xff;
            pt.md_cnt_b = 0xff;
            pt.md_cnt_set_b = 0xff;
            pt.actual_note = 0xff;
            pt.curr_note = 0xff;
            pt.vol = 0x6c;
            pt.pan = pan;
            pt.mask.slot = true;
            pt.loop_.ended = false;
        }
    }

    // 1d45
    fn c_c5_lfo_slotmask(&mut self, w: &mut Work, p: usize) {
        let m = self.cmdload(p) & 0x0f;
        let pt = &mut self.parts[p];
        pt.vol_lfo_slotmask = if m != 0 { (m << 4) | 0x0f } else { pt.fm_slotout };
        self.fm3ex_mode_update_check(w, p);
    }

    // 22c6
    fn c_c4_gate_rel(&mut self, _w: &mut Work, p: usize) {
        self.parts[p].gate_rel = self.cmdload(p);
    }

    // 25d9
    fn c_c3_pan_ex(&mut self, w: &mut Work, p: usize) {
        let d = self.cmdload(p);
        self.cmdload(p);
        let pan = if d == 0 {
            3
        } else if d & 0x80 != 0 {
            1
        } else {
            2
        };
        self.set_pan(w, p, pan);
    }

    // 0458
    fn c_c3_pan_ex_adpcm(&mut self, _w: &mut Work, p: usize) {
        let d = self.cmdload(p);
        self.cmdload(p);
        self.parts[p].pan = if d == 0 {
            0xc0
        } else if d & 0x80 != 0 {
            0x40
        } else {
            0x80
        };
    }

    // 0ae5
    fn c_c3_pan_ex_ppz8(&mut self, w: &mut Work, p: usize) {
        let mut d = self.cmdload(p);
        self.cmdload(p);
        if d & 0x80 == 0 {
            d = d.min(4);
        } else {
            d = d.max(0xfc);
        }
        let pan = d.wrapping_add(5);
        self.parts[p].pan = pan;
        w.ppz8.pan(self.proc_ch, pan);
    }

    // 2509
    fn c_c2_lfo_delay(&mut self, _w: &mut Work, p: usize) {
        let d = self.cmdload(p);
        let pt = &mut self.parts[p];
        pt.lfo.delay = d;
        pt.lfo_set.delay = d;
        lfo_reset(pt);
    }

    fn voldown(&mut self, p: usize, which: u8) {
        let v = self.cmdload(p);
        let (cur, orig) = match which {
            0 => (&mut self.fm_voldown, self.fm_voldown_orig),
            1 => (&mut self.ssg_voldown, self.ssg_voldown_orig),
            2 => (&mut self.adpcm_voldown, self.adpcm_voldown_orig),
            3 => (&mut self.opnarhythm_voldown, self.opnarhythm_voldown_orig),
            _ => (&mut self.ppz8_voldown, self.ppz8_voldown_orig),
        };
        *cur = v;
        let _ = orig;
    }

    fn voldown_rel(&mut self, p: usize, which: u8) {
        let v = self.cmdload(p);
        let (cur, orig) = match which {
            0 => (&mut self.fm_voldown, self.fm_voldown_orig),
            1 => (&mut self.ssg_voldown, self.ssg_voldown_orig),
            2 => (&mut self.adpcm_voldown, self.adpcm_voldown_orig),
            3 => (&mut self.opnarhythm_voldown, self.opnarhythm_voldown_orig),
            _ => (&mut self.ppz8_voldown, self.ppz8_voldown_orig),
        };
        *cur = if v == 0 { orig } else { clamp_u8(i32::from(*cur) + i32::from(u8s8(v)), 0, 0xff) };
    }

    // 1a62
    fn c0_extended(&mut self, p: usize, data: u8) {
        match data {
            0xff => self.voldown(p, 0),
            0xfe => self.voldown_rel(p, 0),
            0xfd => self.voldown(p, 1),
            0xfc => self.voldown_rel(p, 1),
            0xfb => self.voldown(p, 2),
            0xfa => self.voldown_rel(p, 2),
            0xf9 => self.voldown(p, 3),
            0xf8 => self.voldown_rel(p, 3),
            0xf7 => self.pcm86_vol_spb = self.cmdload(p) & 1 != 0,
            _ => self.parts[p].ptr = 0,
        }
    }

    // 1cc0
    fn fm_unmask(&mut self, w: &mut Work, p: usize) {
        if self.parts[p].fm_tone_slotmask == 0 {
            return;
        }
        let tl = self.parts[p].fm_tone_tl;
        self.tonemask_fb_alg = true;
        self.tone_fm(w, p);
        self.tonemask_fb_alg = false;
        self.parts[p].fm_tone_tl = tl;
        let pt = self.parts[p];
        let slotmask = (pt.fm_slotout ^ 0xff) & pt.fm_slotmask;
        const REGOFF: [u8; 4] = [3, 1, 2, 0];
        for s in 0..4 {
            if slotmask & (1 << (7 - s)) != 0 {
                let r = REGOFF[s];
                self.reg_write(w, (0x3f + r * 4).wrapping_add(self.proc_ch), tl[usize::from(r)]);
            }
        }
        let pan = hlfo_delay_check(&pt, pt.pan);
        self.reg_write(w, 0xb3u8.wrapping_add(self.proc_ch), pan);
    }

    // 1c50
    fn c_c0_mml_mask_fm(&mut self, w: &mut Work, p: usize) {
        let data = self.cmdload(p);
        if data >= 2 {
            return self.c0_extended(p, data);
        }
        if data == 1 {
            self.parts[p].mask.mml = false;
            let was = masked(&self.parts[p]);
            self.parts[p].mask.mml = true;
            if !was {
                self.tone_slotmask_stop(w, p);
            }
            self.parts[p].proc_masked = true;
        } else {
            self.parts[p].mask.mml = false;
            if masked(&self.parts[p]) {
                self.parts[p].proc_masked = true;
            } else {
                self.fm_unmask(w, p);
                self.parts[p].proc_masked = false;
            }
        }
    }

    // 1c7a
    fn c_c0_mml_mask_ssg(&mut self, w: &mut Work, p: usize) {
        let data = self.cmdload(p);
        if data >= 2 {
            return self.c0_extended(p, data);
        }
        if data == 1 {
            self.parts[p].mask.mml = false;
            let was = masked(&self.parts[p]);
            self.parts[p].mask.mml = true;
            if !was {
                let o = self.ssg_readout(w);
                w.writereg(0x07, u32::from((o | (o >> 8)) & 0xff));
            }
            self.parts[p].proc_masked = true;
        } else {
            self.parts[p].mask.mml = false;
            self.parts[p].proc_masked = masked(&self.parts[p]);
        }
    }

    // 036a
    fn c_c0_mml_mask_adpcm(&mut self, w: &mut Work, p: usize) {
        let data = self.cmdload(p);
        if data >= 2 {
            return self.c0_extended(p, data);
        }
        if data == 1 {
            self.parts[p].mask.mml = false;
            let was = masked(&self.parts[p]);
            self.parts[p].mask.mml = true;
            if !was {
                w.writereg(0x101, 0x02);
                w.writereg(0x100, 0x01);
            }
            self.parts[p].proc_masked = true;
        } else {
            self.parts[p].mask.mml = false;
            self.parts[p].proc_masked = masked(&self.parts[p]);
        }
    }

    // 09d8
    fn c_c0_mml_mask_ppz8(&mut self, w: &mut Work, p: usize) {
        let data = self.cmdload(p);
        if data >= 2 {
            return self.c0_extended(p, data);
        }
        if data == 1 {
            self.parts[p].mask.mml = false;
            let was = masked(&self.parts[p]);
            self.parts[p].mask.mml = true;
            if !was {
                w.ppz8.stop(self.proc_ch);
            }
            self.parts[p].proc_masked = true;
        } else {
            self.parts[p].mask.mml = false;
            self.parts[p].proc_masked = masked(&self.parts[p]);
        }
    }

    // 1caa
    fn c_c0_mml_mask_rhythm(&mut self, _w: &mut Work, p: usize) {
        let data = self.cmdload(p);
        if data >= 2 {
            return self.c0_extended(p, data);
        }
        self.parts[p].mask.mml = data == 1;
    }

    // 244e
    fn c_bf_lfo2(&mut self, w: &mut Work, p: usize) {
        lfo_flip(&mut self.parts[p]);
        self.c_f2_lfo(w, p);
        lfo_flip(&mut self.parts[p]);
    }

    // 2473
    fn c_be_lfo2_switch(&mut self, _w: &mut Work, p: usize) {
        let v = self.cmdload(p);
        let pt = &mut self.parts[p];
        pt.lfof_b.freq = v & 1 != 0;
        pt.lfof_b.vol = v & 2 != 0;
        pt.lfof_b.sync = v & 4 != 0;
        lfo_flip(pt);
        lfo_reset(pt);
        lfo_flip(pt);
    }

    // 248d
    fn c_be_lfo2_switch_fm(&mut self, w: &mut Work, p: usize) {
        self.c_be_lfo2_switch(w, p);
        self.fm3ex_mode_update_check(w, p);
    }

    // 245f
    fn c_bd_lfo2_md(&mut self, w: &mut Work, p: usize) {
        lfo_flip(&mut self.parts[p]);
        self.c_d6_md(w, p);
        lfo_flip(&mut self.parts[p]);
    }

    // 2464
    fn c_bc_lfo2_waveform(&mut self, w: &mut Work, p: usize) {
        lfo_flip(&mut self.parts[p]);
        self.c_cb_lfo_waveform(w, p);
        lfo_flip(&mut self.parts[p]);
    }

    // 2469
    fn c_bb_lfo2_ext(&mut self, w: &mut Work, p: usize) {
        lfo_flip(&mut self.parts[p]);
        self.c_ca_lfo_ext(w, p);
        lfo_flip(&mut self.parts[p]);
    }

    // 1d61
    fn c_ba_lfo2_slotmask(&mut self, w: &mut Work, p: usize) {
        let m = self.cmdload(p) & 0x0f;
        let pt = &mut self.parts[p];
        pt.vol_lfo_slotmask_b = if m != 0 { (m << 4) | 0x0f } else { pt.fm_slotout };
        self.fm3ex_mode_update_check(w, p);
    }

    // 246e
    fn c_b9_lfo2_delay(&mut self, w: &mut Work, p: usize) {
        lfo_flip(&mut self.parts[p]);
        self.c_c2_lfo_delay(w, p);
        lfo_flip(&mut self.parts[p]);
    }

    // 1b87
    fn c_b8_slottl(&mut self, w: &mut Work, p: usize) {
        let data = self.cmdload(p);
        let slotmask = (data & 0xf) & (self.parts[p].fm_slotmask >> 4);
        let val = self.cmdload(p);
        let is_masked = masked(&self.parts[p]);
        let relative = data & 0x80 != 0;
        const SLOT: [usize; 4] = [0, 2, 1, 3];
        for s in 0..4 {
            if slotmask & (1 << s) == 0 {
                continue;
            }
            let i = SLOT[s];
            let pt = &mut self.parts[p];
            if !relative {
                pt.fm_tone_tl[i] = val & 0x7f;
            } else {
                let mut v = pt.fm_tone_tl[i].wrapping_add(val);
                if v & 0x80 != 0 {
                    v = if val & 0x80 != 0 { 0 } else { 0x7f };
                }
                pt.fm_tone_tl[i] = v;
            }
            if !is_masked {
                let v = self.parts[p].fm_tone_tl[i];
                self.reg_write(w, (0x3f + 4 * i as u8).wrapping_add(self.proc_ch), v);
            }
        }
    }

    // 20cb
    fn c_b7_lfo_md_cnt(&mut self, _w: &mut Work, p: usize) {
        let d = self.cmdload(p);
        let b = d & 0x80 != 0;
        let mut d = d & 0x7f;
        if d == 0 {
            d = 0xff;
        }
        let pt = &mut self.parts[p];
        if !b {
            pt.md_cnt = d;
            pt.md_cnt_set = d;
        } else {
            pt.md_cnt_b = d;
            pt.md_cnt_set_b = d;
        }
    }

    // 1b19
    fn write_fb(&mut self, w: &mut Work, p: usize, data: u8) {
        let newfb = (data & 7) << 3;
        let fbalg;
        let fm3 = self.proc_ch == 3 && !self.opna_a1;
        if fm3 {
            self.fm3ex_fb_alg = (self.fm3ex_fb_alg & 7) | newfb;
            fbalg = self.fm3ex_fb_alg;
        } else {
            let pt = &mut self.parts[p];
            pt.fb_alg = (pt.fb_alg & 7) | newfb;
            fbalg = pt.fb_alg;
        }
        if fm3 && self.parts[p].fm_slotmask & 0x10 == 0 {
            return;
        }
        self.reg_write(w, 0xafu8.wrapping_add(self.proc_ch), fbalg);
        self.parts[p].fb_alg = fbalg;
    }

    // 1b0e
    fn c_b6_fb(&mut self, w: &mut Work, p: usize) {
        let mut data = self.cmdload(p);
        if data & 0x80 == 0 {
            self.write_fb(w, p, data);
        } else {
            if data & 0x40 == 0 {
                data &= 7;
            }
            let fbalg = if self.proc_ch == 3 && !self.opna_a1 { self.fm3ex_fb_alg } else { self.parts[p].fb_alg };
            let fb = ((fbalg >> 3) & 7).wrapping_add(data);
            let v = if fb & 0x80 != 0 { 0 } else { fb.min(7) };
            self.write_fb(w, p, v);
        }
    }

    // 1af6
    fn c_b5_slot_delay(&mut self, _w: &mut Work, p: usize) {
        let d = self.cmdload(p);
        let delay = self.cmdload(p);
        let pt = &mut self.parts[p];
        pt.slot_delay_mask = ((!d) & 0xf) << 4;
        pt.slot_delay = delay;
        pt.slot_delay_cnt = delay;
    }

    // 099f
    fn c_b4_ppz8_init(&mut self, _w: &mut Work, p: usize) {
        for i in 0..8 {
            let ptr = self.load16(p);
            if ptr == 0 {
                continue;
            }
            let pt = &mut self.parts[PPZ_1 + i];
            pt.ptr = ptr;
            pt.len = 1;
            pt.len_cnt = 1;
            pt.keystatus.off = true;
            pt.keystatus.off_mask = true;
            pt.md_cnt = 0xff;
            pt.md_cnt_set = 0xff;
            pt.md_cnt_b = 0xff;
            pt.md_cnt_set_b = 0xff;
            pt.actual_note = 0xff;
            pt.vol = 0x80;
            pt.pan = 5;
            pt.loop_.ended = false;
        }
    }

    // 22bc
    fn c_b3_gate_min(&mut self, _w: &mut Work, p: usize) {
        self.parts[p].gate_min = self.cmdload(p);
    }

    // 23ef
    fn c_b2_transpose_master(&mut self, _w: &mut Work, p: usize) {
        self.parts[p].transpose_master = self.cmdload(p);
    }

    // 22c1
    fn c_b1_gate_rand_range(&mut self, _w: &mut Work, p: usize) {
        self.parts[p].gate_rand_range = self.cmdload(p);
    }
}

// A part reading this many commands in one tick without a note is stuck
// (a loop of nothing but commands); 98fmplayer would hang, so it stops.
const STUCK: u32 = 0x10000;

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Fm,
    Ssg,
    Rhythm,
    Adpcm,
    Ppz8,
}

// 3161
fn ssg_env_tick_old(p: &mut Part) {
    let in_range = |v: u8| (-15..15).contains(&i32::from(u8s8(v)));
    match p.ssg_env_state_old {
        OLD_AL => {
            p.ssg_env_param_set[P_OLD_AL] = p.ssg_env_param_set[P_OLD_AL].wrapping_sub(1);
            if p.ssg_env_param_set[P_OLD_AL] != 0 {
                return;
            }
            p.ssg_env_state_old = OLD_SR;
            p.ssg_env_vol = p.ssg_env_param_set[P_OLD_AD];
        }
        OLD_SR => {
            if p.ssg_env_param_set[P_OLD_SR] == 0 {
                return;
            }
            p.ssg_env_param_set[P_OLD_SR] -= 1;
            if p.ssg_env_param_set[P_OLD_SR] != 0 {
                return;
            }
            p.ssg_env_param_set[P_OLD_SR] = p.ssg_env_param[P_OLD_SR];
            p.ssg_env_vol = p.ssg_env_vol.wrapping_sub(1);
            if !in_range(p.ssg_env_vol) {
                p.ssg_env_vol = (-15i8) as u8;
            }
        }
        OLD_RR => {
            if p.ssg_env_param_set[P_OLD_RR] == 0 {
                p.ssg_env_vol = (-15i8) as u8;
                return;
            }
            p.ssg_env_param_set[P_OLD_RR] -= 1;
            if p.ssg_env_param_set[P_OLD_RR] != 0 {
                return;
            }
            p.ssg_env_param_set[P_OLD_RR] = p.ssg_env_param[P_OLD_RR];
            p.ssg_env_vol = p.ssg_env_vol.wrapping_sub(1);
            if !in_range(p.ssg_env_vol) {
                p.ssg_env_vol = (-15i8) as u8;
            }
        }
        _ => {}
    }
}

// 314e: whether the volume needs writing
fn ssg_env_tick_check(p: &mut Part) -> bool {
    if p.ssg_env_state_old == OLD_NEW {
        return ssg_env_tick_new_check(p);
    }
    let old = p.ssg_env_vol;
    ssg_env_tick_old(p);
    old != p.ssg_env_vol
}

// 20e8
fn portamento_tick(p: &mut Part) {
    p.portamento_diff = p.portamento_diff.wrapping_add(p.portamento_add);
    if p.portamento_rem > 0 {
        p.portamento_rem -= 1;
        p.portamento_diff = p.portamento_diff.wrapping_add(1);
    } else if p.portamento_rem < 0 {
        p.portamento_rem += 1;
        p.portamento_diff = p.portamento_diff.wrapping_sub(1);
    }
}

// 42d6
const SSGRHYTHM_OPNA: [(u8, u8, u8); 11] = [
    (0x18, 0xdf, 0x01),
    (0x19, 0xdf, 0x02),
    (0x1c, 0x5f, 0x10),
    (0x1c, 0xdf, 0x10),
    (0x1c, 0x9f, 0x10),
    (0x1d, 0xd3, 0x20),
    (0x19, 0xdf, 0x02),
    (0x1b, 0x9c, 0x88),
    (0x1a, 0x9d, 0x04),
    (0x1a, 0xdf, 0x04),
    (0x1a, 0x5e, 0x04),
];

impl Pmd {
    fn cmd(&mut self, w: &mut Work, p: usize, kind: Kind, cmd: u8) {
        if cmd < 0xb1 {
            self.parts[p].ptr = 0;
            return;
        }
        let t = match kind {
            Kind::Fm => &T_FM,
            Kind::Ssg => &T_SSG,
            Kind::Rhythm => &T_RHYTHM,
            Kind::Adpcm => &T_ADPCM,
            Kind::Ppz8 => &T_PPZ8,
        };
        self.exec(w, p, t[usize::from(cmd ^ 0xff)]);
    }

    // 312e: whether the volume needs writing
    fn ssg_env_proc(&mut self, p: usize) -> bool {
        if self.parts[p].flagext.env {
            let n = self.timera_cnt.wrapping_sub(self.timera_cnt_b);
            let mut u = false;
            for _ in 0..n {
                u |= ssg_env_tick_check(&mut self.parts[p]);
            }
            u
        } else {
            ssg_env_tick_check(&mut self.parts[p])
        }
    }

    // The LFOs' tick for the SSG, ADPCM and PPZ8: whether the frequency
    // needs writing. (As 98fmplayer has it, LFO 2's frequency flag is
    // taken from LFO 1's.)
    fn lfo_tick_ssglike(&mut self, p: usize) -> bool {
        self.lfoprocf = LfoFlags::default();
        self.lfoprocf_b = LfoFlags::default();
        let pt = self.parts[p];
        self.lfoprocf.portamento = pt.lfof.portamento;
        let any = |f: LfoFlags| f.freq || f.vol || f.sync || f.portamento;
        if !(any(pt.lfof) || any(pt.lfof_b)) {
            return false;
        }
        if (pt.lfof.freq || pt.lfof.vol) && self.lfo_tick(p) {
            self.lfoprocf.freq = self.parts[p].lfof.freq;
            self.lfoprocf.vol = self.parts[p].lfof.vol;
        }
        if pt.lfof_b.freq || pt.lfof_b.vol {
            lfo_flip(&mut self.parts[p]);
            if self.lfo_tick(p) {
                self.lfoprocf_b.freq = self.parts[p].lfof_b.freq;
                self.lfoprocf_b.vol = self.parts[p].lfof_b.vol;
            }
            lfo_flip(&mut self.parts[p]);
        }
        if self.lfoprocf.freq || self.lfoprocf.portamento || self.lfoprocf_b.freq {
            if self.lfoprocf.portamento {
                portamento_tick(&mut self.parts[p]);
            }
            return true;
        }
        false
    }

    fn vol_update_due(&mut self, p: usize) -> bool {
        self.ssg_env_proc(p) || self.lfoprocf.vol || self.lfoprocf_b.vol || self.fadeout_speed != 0
    }

    // 1617
    fn proc_ssg_lfoenv(&mut self, w: &mut Work, p: usize) {
        if self.lfo_tick_ssglike(p) {
            self.ssg_freq_out(w, p);
        }
        if self.vol_update_due(p) {
            self.ssg_vol_out(w, p);
        }
        self.loop_check(p);
    }

    // 01fa
    fn proc_adpcm_lfoenv(&mut self, w: &mut Work, p: usize) {
        if self.lfo_tick_ssglike(p) {
            self.adpcm_freq_out(w, p);
        }
        if self.vol_update_due(p) {
            self.adpcm_vol_out(w, p);
        }
        self.loop_check(p);
    }

    // 084c
    fn proc_ppz8_lfoenv(&mut self, w: &mut Work, p: usize) {
        if self.lfo_tick_ssglike(p) {
            self.ppz8_freq_out(w, p);
        }
        if self.vol_update_due(p) {
            self.ppz8_vol_out(w, p);
        }
        self.loop_check(p);
    }

    // 13f8
    fn proc_fm_lfo(&mut self, w: &mut Work, p: usize) {
        if self.parts[p].hlfo_delay != 0 {
            self.parts[p].hlfo_delay -= 1;
            if self.parts[p].hlfo_delay == 0 {
                self.reg_write(w, 0xb3u8.wrapping_add(self.proc_ch), self.parts[p].pan);
            }
        }
        if self.parts[p].slot_delay_cnt != 0 {
            self.parts[p].slot_delay_cnt -= 1;
            if self.parts[p].slot_delay_cnt == 0 && !self.parts[p].keystatus.off {
                self.fm_keyon(w, p);
            }
        }
        self.lfoprocf = LfoFlags::default();
        self.lfoprocf_b = LfoFlags::default();
        let pt = self.parts[p];
        self.lfoprocf.portamento = pt.lfof.portamento;
        let any = |f: LfoFlags| f.freq || f.vol || f.sync || f.portamento;
        if any(pt.lfof) || any(pt.lfof_b) {
            if (pt.lfof.freq || pt.lfof.vol) && self.lfo_tick(p) {
                self.lfoprocf.freq = self.parts[p].lfof.freq;
                self.lfoprocf.vol = self.parts[p].lfof.vol;
            }
            if pt.lfof_b.freq || pt.lfof_b.vol {
                lfo_flip(&mut self.parts[p]);
                if self.lfo_tick(p) {
                    self.lfoprocf_b.freq = self.parts[p].lfof.freq;
                    self.lfoprocf_b.vol = self.parts[p].lfof.vol;
                }
                lfo_flip(&mut self.parts[p]);
            }
            if self.lfoprocf.freq || self.lfoprocf.portamento || self.lfoprocf_b.freq {
                if self.lfoprocf.portamento {
                    portamento_tick(&mut self.parts[p]);
                }
                self.fm_freq_out(w, p);
            }
        }
        if self.lfoprocf.vol || self.lfoprocf_b.vol || self.fadeout_speed != 0 {
            self.fm_vol_out(w, p);
        }
        self.loop_check(p);
    }

    fn lfoenv(&mut self, w: &mut Work, p: usize, kind: Kind) {
        match kind {
            Kind::Fm => self.proc_fm_lfo(w, p),
            Kind::Ssg => self.proc_ssg_lfoenv(w, p),
            Kind::Adpcm => self.proc_adpcm_lfoenv(w, p),
            _ => self.proc_ppz8_lfoenv(w, p),
        }
    }

    // 1561
    fn loop_check_masked(&mut self, p: usize) {
        self.no_keyoff = false;
        self.volume_saved = false;
        self.loop_check(p);
    }

    // 1541
    fn proc_note_masked(&mut self, p: usize) {
        let len = self.cmdload(p);
        let pt = &mut self.parts[p];
        pt.actual_freq = 0;
        pt.actual_note = 0xff;
        pt.curr_note = 0xff;
        pt.len = len;
        pt.len_cnt = len;
        pt.note_proc = pt.note_proc.wrapping_add(1);
        if !self.volume_saved {
            pt.volume_save = 0;
        }
        self.loop_check_masked(p);
    }

    // 16d9
    fn ssg_next_masked(&mut self, w: &mut Work, p: usize, note: u8) -> bool {
        let pt = self.parts[p];
        if pt.mask.ext || !pt.mask.effect || self.ssgeff_priority >= 2 || note & 0xf == 0xf {
            return true;
        }
        if self.ssgeff_priority == 1 {
            self.ssgeff_stop(w);
        }
        self.parts[p].mask.effect = false;
        masked(&self.parts[p])
    }

    // 2cb5
    fn off_fm(&mut self, w: &mut Work, p: usize) {
        let k = self.parts[p].keystatus;
        if k.off && k.off_mask {
            return;
        }
        self.do_keyoff_fm(w, p);
    }

    // 05cf
    fn off_adpcm(&mut self, w: &mut Work, p: usize) {
        let pt = self.parts[p];
        if pt.ssg_env_state_old == OLD_NEW {
            if pt.ssg_env_state_new == NEW_RR {
                return;
            }
        } else if pt.ssg_env_state_old == OLD_RR {
            return;
        }
        if self.adpcm_release != 0x8000 {
            w.writereg(0x100, 0x21);
            w.writereg(0x102, u32::from(self.adpcm_release & 0xff));
            w.writereg(0x103, u32::from(self.adpcm_release >> 8));
            w.writereg(0x104, u32::from(self.adpcm_stop & 0xff));
            w.writereg(0x105, u32::from(self.adpcm_stop >> 8));
            w.writereg(0x100, 0xa0);
        }
        part_off_ssg(&mut self.parts[p]);
    }

    // 0c0e
    fn off_ppz8(&mut self, p: usize) {
        let pt = &mut self.parts[p];
        if pt.ssg_env_state_old != OLD_NEW {
            if pt.ssg_env_state_old != OLD_RR {
                part_off_ssg(pt);
            }
        } else if pt.ssg_env_state_new != NEW_RR {
            part_off_ssg(pt);
        }
    }

    // 15d4
    fn ssg_out(&mut self, w: &mut Work, p: usize) {
        self.volume_save_check(p);
        self.ssg_vol_out(w, p);
        self.ssg_freq_out(w, p);
        self.ssg_note_noise_out(w, p);
        self.note_done(p);
    }

    // The gate (key-off before the note's end) for a part this tick.
    fn gate_tick(&mut self, w: &mut Work, p: usize, kind: Kind) {
        let pt = &mut self.parts[p];
        pt.len_cnt = pt.len_cnt.wrapping_sub(1);
        if pt.proc_masked {
            pt.keystatus.off = true;
            pt.keystatus.off_mask = true;
        } else if !pt.keystatus.off && !pt.keystatus.off_mask && pt.len_cnt <= pt.gate {
            match kind {
                Kind::Fm => self.off_fm(w, p),
                Kind::Ssg => part_off_ssg(&mut self.parts[p]),
                Kind::Adpcm => self.off_adpcm(w, p),
                _ => self.off_ppz8(p),
            }
            let pt = &mut self.parts[p];
            pt.keystatus.off = true;
            pt.keystatus.off_mask = true;
        }
    }

    // 1350 / 14fc (FM), 0149 / 026c (ADPCM), 079b / 08be (PPZ8), 156f / 1689 (SSG)
    fn proc_part(&mut self, w: &mut Work, p: usize, kind: Kind) {
        if self.parts[p].ptr == 0 && matches!(kind, Kind::Fm | Kind::Adpcm) {
            return;
        }
        self.parts[p].proc_masked = masked(&self.parts[p]);
        self.gate_tick(w, p, kind);
        if self.parts[p].len_cnt != 0 {
            if self.parts[p].proc_masked {
                self.loop_check(p);
            } else {
                self.lfoenv(w, p, kind);
            }
            return;
        }
        if kind == Kind::Fm && self.parts[p].proc_masked && self.parts[p].mask.effect && !self.fmeff_playing {
            self.parts[p].mask.effect = false;
            self.parts[p].proc_masked = masked(&self.parts[p]);
        }
        if kind != Kind::Fm || !self.parts[p].proc_masked {
            self.parts[p].lfof.portamento = false;
        }
        for _ in 0..STUCK {
            let cmd = self.cmdload(p);
            if cmd & 0x80 != 0 {
                if cmd != 0x80 {
                    if kind == Kind::Ssg && self.parts[p].proc_masked && cmd == 0xda {
                        let m = self.ssg_next_masked(w, p, cmd);
                        self.parts[p].proc_masked = m;
                    }
                    self.cmd(w, p, kind, cmd);
                    if cmd == 0xda && !masked(&self.parts[p]) {
                        return;
                    }
                } else {
                    let pt = &mut self.parts[p];
                    pt.ptr = 0;
                    pt.loop_.looped = true;
                    pt.loop_.ended = true;
                    pt.actual_note = 0xff;
                    if pt.loop_ptr == 0 {
                        if pt.proc_masked {
                            self.loop_check_masked(p);
                        } else {
                            self.lfoenv(w, p, kind);
                        }
                        return;
                    }
                    pt.ptr = pt.loop_ptr;
                    pt.loop_.ended = false;
                }
            } else {
                let is_masked = self.parts[p].proc_masked && (kind != Kind::Ssg || self.ssg_next_masked(w, p, cmd));
                if is_masked {
                    if kind == Kind::Ppz8 {
                        self.parts[p].actual_freq_upper = 0;
                    }
                    self.proc_note_masked(p);
                    return;
                }
                let note = if kind == Kind::Fm { self.lfo_init_fm(w, p, cmd) } else { self.lfo_init_ssg(w, p, cmd) };
                let note = note_transpose(&self.parts[p], note);
                match kind {
                    Kind::Fm => note_freq_fm(&mut self.parts[p], note),
                    Kind::Ssg => note_freq_ssg(&mut self.parts[p], note),
                    Kind::Adpcm => note_freq_adpcm(&mut self.parts[p], note),
                    _ => note_freq_ppz8(&mut self.parts[p], note),
                }
                let len = self.cmdload(p);
                self.parts[p].len = len;
                self.parts[p].len_cnt = len;
                self.calc_gate(p);
                match kind {
                    Kind::Fm => self.fm_out(w, p),
                    Kind::Ssg => self.ssg_out(w, p),
                    Kind::Adpcm => self.adpcm_out(w, p),
                    _ => self.ppz8_out(w, p),
                }
                return;
            }
        }
        self.parts[p].ptr = 0;
        self.parts[p].loop_ptr = 0;
    }

    // 1813
    fn ssgrhythm_opna_out(&mut self, w: &mut Work, r: (u8, u8, u8)) {
        w.writereg(u32::from(r.0), u32::from(r.1));
        let mut kon = r.2 & self.opnarhythm_mask;
        if kon == 0 {
            return;
        }
        if kon & 0x80 != 0 {
            w.writereg(0x10, 0x84);
            kon = 0x08 & self.opnarhythm_mask;
            if kon == 0 {
                return;
            }
        }
        w.writereg(0x10, u32::from(kon));
    }

    // 170d
    fn proc_opnarhythm(&mut self, w: &mut Work, p: usize) {
        if self.parts[p].ptr == 0 {
            return;
        }
        self.parts[p].len_cnt = self.parts[p].len_cnt.wrapping_sub(1);
        if self.parts[p].len_cnt != 0 {
            self.loop_check(p);
            return;
        }
        let mut n = 0;
        loop {
            while self.r_ptr != 0 && u32::from(self.datalen) >= u32::from(self.r_ptr) + 1 && self.d(self.r_ptr) != 0xff {
                n += 1;
                if n > STUCK {
                    self.parts[p].ptr = 0;
                    self.parts[p].loop_ptr = 0;
                    self.r_ptr = 0;
                    return;
                }
                let rcmd = self.d(self.r_ptr);
                self.r_ptr = self.r_ptr.wrapping_add(1);
                if rcmd & 0x80 != 0 {
                    if rcmd & 0x40 != 0 {
                        std::mem::swap(&mut self.parts[p].ptr, &mut self.r_ptr);
                        self.cmd(w, p, Kind::Rhythm, rcmd);
                        std::mem::swap(&mut self.parts[p].ptr, &mut self.r_ptr);
                        continue;
                    }
                    if masked(&self.parts[p]) {
                        self.ssgrhythm = 0;
                        self.r_ptr = self.r_ptr.wrapping_add(1);
                        let len = self.d(self.r_ptr);
                        self.r_ptr = self.r_ptr.wrapping_add(1);
                        self.parts[p].len = len;
                        self.parts[p].len_cnt = len;
                        self.loop_check_masked(p);
                        self.parts[p].note_proc = self.parts[p].note_proc.wrapping_add(1);
                        return;
                    }
                    self.ssgrhythm = ((u16::from(rcmd) << 8) | u16::from(self.d(self.r_ptr))) & 0x3fff;
                    self.r_ptr = self.r_ptr.wrapping_add(1);
                    if self.ssgrhythm != 0 {
                        if self.ssgrhythm_opna {
                            for (i, &r) in SSGRHYTHM_OPNA.iter().enumerate() {
                                if self.ssgrhythm & (1 << i) != 0 {
                                    self.ssgrhythm_opna_out(w, r);
                                }
                            }
                        }
                        if self.fadeout_vol != 0 && self.ssgrhythm_opna {
                            self.opnarhythm_tl_write(w, self.opnarhythm_tl);
                        }
                        let eff = self.ssgrhythm.trailing_zeros() as u8;
                        self.ssg_effect(w, eff);
                    }
                } else {
                    self.ssgrhythm = 0;
                }
                let len = self.d(self.r_ptr);
                self.r_ptr = self.r_ptr.wrapping_add(1);
                self.parts[p].len = len;
                self.parts[p].len_cnt = len;
                self.loop_check_masked(p);
                self.parts[p].note_proc = self.parts[p].note_proc.wrapping_add(1);
                return;
            }
            loop {
                n += 1;
                if n > STUCK {
                    self.parts[p].ptr = 0;
                    self.parts[p].loop_ptr = 0;
                    return;
                }
                let cmd = self.cmdload(p);
                if cmd & 0x80 != 0 {
                    if cmd != 0x80 {
                        self.cmd(w, p, Kind::Rhythm, cmd);
                    } else {
                        let pt = &mut self.parts[p];
                        pt.ptr = 0;
                        pt.loop_.looped = true;
                        pt.loop_.ended = true;
                        if pt.loop_ptr == 0 {
                            self.loop_check_masked(p);
                            return;
                        }
                        pt.ptr = pt.loop_ptr;
                        pt.loop_.ended = false;
                    }
                } else {
                    let ptr = self.r_offset.wrapping_add(u16::from(cmd) * 2);
                    if u32::from(self.datalen) < u32::from(ptr) + 2 {
                        self.parts[p].ptr = 0;
                        return;
                    }
                    self.r_ptr = self.r16(ptr);
                    break;
                }
            }
        }
    }

    // 11fd
    fn proc_parts(&mut self, w: &mut Work) {
        self.loop_ = Loop {
            looped: true,
            ended: true,
            env: false,
        };
        if !self.opm_flag {
            for c in 0..3 {
                self.proc_ch = c as u8 + 1;
                self.proc_part(w, SSG_1 + c, Kind::Ssg);
            }
        }
        self.opna_a1 = true;
        for c in 0..3 {
            self.proc_ch = c as u8 + 1;
            self.proc_part(w, FM_4 + c, Kind::Fm);
        }
        self.opna_a1 = false;
        for c in 0..3 {
            self.proc_ch = c as u8 + 1;
            self.proc_part(w, FM_1 + c, Kind::Fm);
        }
        for c in 0..3 {
            self.proc_part(w, FM_3B + c, Kind::Fm);
        }
        if !self.opm_flag {
            self.proc_opnarhythm(w, RHYTHM);
            self.proc_part(w, ADPCM, Kind::Adpcm);
            for c in 0..8 {
                self.proc_ch = c as u8;
                self.proc_part(w, PPZ_1 + c, Kind::Ppz8);
            }
        }
        let l = self.loop_;
        if !l.looped && !l.ended && !l.env {
            return;
        }
        for pt in &mut self.parts[FM_1..FM_1 + 12] {
            if !pt.loop_.looped || !pt.loop_.ended || pt.loop_.env {
                pt.loop_ = Loop::default();
            }
        }
        if !l.looped || !l.ended || l.env {
            w.timerb_cnt_loop = 0;
            self.status2 = self.status2.wrapping_add(1);
            if self.status2 == 0xff {
                self.status2 = 1;
            }
        } else {
            self.status2 = 0xff;
            w.playing = false;
        }
        w.loop_cnt = self.status2;
    }

    // 3d68
    fn timerb(&mut self, w: &mut Work) {
        if self.playing {
            self.proc_parts(w);
            self.timerb_write(w);
            self.tick_cnt = self.tick_cnt.wrapping_add(1);
            if self.tick_cnt == self.meas_len {
                self.tick_cnt = 0;
                self.meas_cnt = self.meas_cnt.wrapping_add(1);
            }
            self.timera_cnt_b = self.timera_cnt;
        }
    }

    // 329e
    fn fadeout_tick(&mut self, w: &mut Work) {
        if self.paused || self.fadeout_speed == 0 {
            return;
        }
        let v = i32::from(self.fadeout_vol) + i32::from(u8s8(self.fadeout_speed));
        if v < 0 {
            self.fadeout_vol = 0;
            self.fadeout_speed = 0;
            self.opnarhythm_tl_write(w, self.opnarhythm_tl);
        } else if v > 0xff {
            self.fadeout_speed = 0;
        }
    }

    // 3daf
    fn timera(&mut self, w: &mut Work) {
        self.timera_cnt = self.timera_cnt.wrapping_add(1);
        if self.timera_cnt & 7 == 0 {
            self.fadeout_tick(w);
        }
        if self.ssgeff_priority != 0 && (!self.ppsdrv_enabled || self.ssgeff_num & 0x80 == 0) {
            self.ssgeff_tick(w);
        }
    }

    // 3c76, 3d1b: the OPNA's interrupt
    pub fn interrupt(&mut self, w: &mut Work) {
        let mut guard = 0;
        while w.status(false) & 3 != 0 && guard < 16 {
            guard += 1;
            let status = w.status(false);
            w.writereg(0x27, u32::from(self.fm3ex_mode));
            if status & 1 != 0 {
                self.timera(w);
            }
            if status & 2 != 0 {
                self.timerb(w);
                if w.playing {
                    w.timerb_cnt = w.timerb_cnt.wrapping_add(1);
                    w.timerb_cnt_loop = w.timerb_cnt_loop.wrapping_add(1);
                }
            }
        }
    }

    /// The song, or None when it isn't one PMD can play.
    pub fn load(song: &[u8]) -> Option<Pmd> {
        if song.is_empty() {
            return None;
        }
        let body = &song[1..song.len().min(0x10000)];
        let mut pmd = Pmd {
            data: body.to_vec(),
            datalen: body.len() as u16,
            opm_byte: song[0],
            ssgeff_data: &[],
            ssgeff_pos: 0,
            ssgeff_tonefreq: 0,
            ssgeff_tonefreq_add: 0,
            ssgeff_wait: 0,
            ssgeff_noisefreq: 0,
            ssgeff_noisefreq_add: 0,
            ssgeff_noisefreq_cnt_set: 0,
            ssgeff_noisefreq_cnt: 0,
            ssgeff_priority: 0,
            ssgeff_num: 0,
            ssgeff_internal: false,
            rand: 0,
            proc_ch: 0,
            no_keyoff: false,
            volume_saved: false,
            opna_a1: false,
            fm_slotkey: [0; 6],
            loop_: Loop::default(),
            ppsdrv_enabled: false,
            adpcm_start_loop: 0,
            adpcm_stop_loop: 0,
            adpcm_release: 0,
            timera_cnt_b: 0,
            fm3ex_force: false,
            fm3ex_needed: 0,
            fm3ex_fb_alg: 0,
            tonemask_fb_alg: false,
            lfoprocf: LfoFlags::default(),
            lfoprocf_b: LfoFlags::default(),
            tone_ptr: 0,
            r_offset: 0,
            r_ptr: 0,
            opnarhythm_mask: 0,
            fm_voldown: 0,
            ssg_voldown: 0,
            adpcm_voldown: 0,
            opnarhythm_voldown: 0,
            tone_included: false,
            opm_flag: false,
            status1: 0,
            status2: 0,
            timerb: 0,
            fadeout_speed: 0,
            fadeout_vol: 0,
            timerb_bak: 0,
            meas_len: 0,
            tick_cnt: 0,
            timera_cnt: 0,
            ssgeff_disabled: false,
            ssg_noise_freq: 0,
            ssg_noise_freq_wrote: 0,
            fmeff_playing: false,
            adpcm_start: 0,
            adpcm_stop: 0,
            opnarhythm: 0,
            opnarhythm_ilpan: [0; 6],
            opnarhythm_tl: 0,
            ssgrhythm: 0,
            playing: false,
            paused: false,
            ssgrhythm_opna: false,
            adpcm_disabled: false,
            fm3ex_det: [0; 4],
            timerb_wrote: 0,
            faded_out: false,
            pcm86_vol_spb: false,
            meas_cnt: 0,
            opna_last22: 0,
            tempo: 0,
            tempo_bak: 0,
            fm_voldown_orig: 0,
            ssg_voldown_orig: 0,
            adpcm_voldown_orig: 0,
            opnarhythm_voldown_orig: 0,
            pcm86_vol_spb_orig: false,
            opnarhythm_shot_inc: [0; 6],
            opnarhythm_dump_inc: [0; 6],
            fm3ex_mode: 0,
            ppz8_voldown: 0,
            ppz8_voldown_orig: 0,
            parts: [Part::default(); PART_NUM],
            adpcm_addr: [[0; 2]; 256],
            ssgeff_tone_mix: false,
            ssgeff_noise_mix: false,
            ssgeff_on: false,
        };
        pmd.reset_state();
        pmd.ssgeff_num = 0xff;
        if !pmd.data_init() {
            return None;
        }
        pmd.fadeout_vol = 0;
        if pmd.adpcm_disabled {
            pmd.parts[ADPCM].mask.disabled = true;
        }
        pmd.opnarhythm_mask = 0xff;
        pmd.ssgrhythm_opna = true;
        Some(pmd)
    }

    /// The driver's start on the chips (98fmplayer's pmd_init).
    pub fn init(&mut self, w: &mut Work) {
        self.reset_opna(w);
        self.reset_timer(w);
        self.playing = true;
        w.playing = true;
    }

    fn check_str(&self, ptr: u16) -> Option<&[u8]> {
        let s = self.data.get(usize::from(ptr)..usize::from(self.datalen))?;
        let n = s.iter().position(|&b| b == 0)?;
        Some(&s[..n])
    }

    // 383e: the song's memo strings; -2 is the PPZ file, -1 the PPS, 0 the
    // PPC, 1 the title, 2 the composer, 3 the arranger, then the memo.
    pub fn memo(&self, index: i32) -> Option<&[u8]> {
        let mut index = index;
        if index < -2 || self.datalen < 2 || self.r16(0) == 0x18 || self.datalen < 0x18 + 2 {
            return None;
        }
        let toneptr = self.r16(0x18);
        if toneptr < 4 || self.datalen < toneptr {
            return None;
        }
        let flaglow = self.d(toneptr - 2);
        let flaghigh = self.d(toneptr - 1);
        if flaglow != 0x40 && (flaghigh != 0xfe || flaglow < 0x41) {
            return None;
        }
        if flaglow >= 0x42 {
            index += 1;
        }
        if flaglow >= 0x48 {
            index += 1;
        }
        if index < 0 {
            return None;
        }
        let mut memoptr = self.r16(toneptr - 4);
        while u32::from(self.datalen) >= u32::from(memoptr) + 2 && self.r16(memoptr) != 0 {
            if index == 0 {
                return self.check_str(self.r16(memoptr));
            }
            memoptr = memoptr.wrapping_add(2);
            index -= 1;
        }
        None
    }

    /// The PCM files the song asks for: (PPC, PPZ 1, PPZ 2), as named in it
    /// (without their extension).
    pub fn pcm_files(&self) -> [Option<String>; 3] {
        let name = |s: &[u8]| {
            let n = s.iter().position(|&b| b == b'.' || b == b',').unwrap_or(s.len());
            let n = &s[..n.min(12)];
            if n.is_empty() {
                None
            } else {
                Some(n.iter().map(|&b| b as char).collect::<String>())
            }
        };
        let ppc = self.memo(0).and_then(name);
        let ppz = self.memo(-2);
        let ppz1 = ppz.and_then(name);
        let ppz2 = ppz.and_then(|s| s.iter().position(|&b| b == b',').map(|i| &s[i + 1..])).and_then(name);
        [ppc, ppz1, ppz2]
    }

    /// A .PPC file (ADPCM voices) into the OPNA's RAM; false when it isn't one.
    pub fn load_ppc(&mut self, w: &mut Work, ppc: &[u8]) -> bool {
        const HEADER: usize = 30 + 2 + 4 * 256;
        if ppc.len() < HEADER || &ppc[..30] != b"ADPCM DATA for  PMD ver.4.4-  " {
            return false;
        }
        for i in 0..256 {
            let r = |o: usize| u16::from(ppc[o]) | u16::from(ppc[o + 1]) << 8;
            self.adpcm_addr[i] = [r(32 + 4 * i), r(32 + 4 * i + 2)];
        }
        for (r, v) in [
            (0x100, 0x01),
            (0x110, 0x13),
            (0x110, 0x80),
            (0x100, 0x60),
            (0x101, 0x02),
            (0x102, 0x00),
            (0x103, 0x00),
            (0x104, 0xff),
            (0x105, 0xff),
            (0x10c, 0xff),
            (0x10d, 0xff),
        ] {
            w.writereg(r, v);
        }
        for _ in 0..0x4c0 {
            w.writereg(0x108, 0);
        }
        for &b in &ppc[HEADER..] {
            w.writereg(0x108, u32::from(b));
        }
        w.writereg(0x110, 0x0c);
        w.writereg(0x100, 0x01);
        true
    }

    /// Whether a part is playing its song (for a host showing which are).
    pub fn part_playing(&self, p: usize) -> bool {
        self.parts.get(p).is_some_and(|pt| !pt.loop_.ended)
    }

    /// A part's count of notes and rests read (wrapping), and its note now:
    /// octave in the high nibble, 0xff for none.
    pub fn note_event(&self, p: usize) -> (u8, u8) {
        let pt = &self.parts[p];
        (pt.note_proc, pt.actual_note)
    }

    /// The SSG drums the rhythm part struck last (a bit each), 0 for a rest.
    pub fn rhythm_hit(&self) -> u16 {
        self.ssgrhythm
    }

    /// The measure, its tick and ticks a measure.
    pub fn measure(&self) -> (u16, u8, u8) {
        (self.meas_cnt, self.tick_cnt, self.meas_len)
    }

    /// Its tempo (as the T command sets it).
    pub fn tempo(&self) -> u8 {
        self.tempo
    }
}
// Each part kind's command table, 0xff down to 0xb1 (from 98fmplayer's).
const T_FM: [u8; 79] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 17, 17, 17, 37, 38, 39, 40, 41, 17, 17, 42, 43, 44,
    17, 45, 46, 17, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70,
];
const T_SSG: [u8; 79] = [
    17, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 71, 72, 13, 73, 74, 16, 75, 76, 17, 19, 20, 21, 22, 23, 24, 25, 17, 77, 28, 17, 17, 31, 78, 33, 34, 35, 79, 17, 17, 17, 37, 38, 39, 40, 41, 17, 80, 17, 43, 81,
    82, 45, 46, 83, 84, 84, 43, 17, 51, 85, 53, 54, 86, 56, 87, 58, 59, 60, 17, 62, 85, 64, 17, 85, 67, 68, 69, 70,
];
const T_RHYTHM: [u8; 79] = [
    17, 17, 2, 3, 4, 5, 6, 7, 8, 9, 17, 71, 72, 15, 88, 15, 16, 17, 17, 17, 19, 20, 21, 22, 17, 24, 25, 17, 77, 28, 17, 17, 31, 78, 33, 34, 35, 17, 17, 17, 17, 85, 38, 39, 40, 41, 17, 17, 17, 43, 44,
    17, 17, 17, 17, 84, 84, 43, 17, 17, 85, 17, 54, 89, 15, 17, 85, 17, 17, 17, 17, 85, 17, 17, 85, 67, 17, 17, 17,
];
const T_ADPCM: [u8; 79] = [
    90, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 91, 92, 13, 73, 74, 16, 17, 17, 93, 19, 20, 21, 22, 23, 24, 25, 17, 94, 28, 17, 17, 31, 95, 33, 34, 35, 96, 17, 17, 17, 37, 38, 39, 40, 41, 17, 17, 17, 97, 81,
    17, 45, 46, 83, 84, 84, 43, 17, 51, 98, 53, 54, 99, 56, 87, 58, 59, 60, 61, 62, 85, 64, 17, 85, 100, 68, 69, 70,
];
const T_PPZ8: [u8; 79] = [
    101, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 91, 92, 13, 73, 74, 16, 17, 17, 102, 19, 20, 21, 22, 23, 24, 25, 17, 94, 28, 17, 17, 31, 95, 33, 34, 35, 103, 17, 17, 17, 37, 38, 39, 40, 41, 17, 17, 17, 104,
    81, 17, 45, 46, 83, 84, 84, 43, 17, 51, 105, 53, 54, 106, 56, 87, 58, 59, 60, 61, 62, 85, 64, 17, 85, 67, 68, 69, 70,
];

impl Pmd {
    fn exec(&mut self, w: &mut Work, p: usize, f: u8) {
        match f {
            0 => self.c_ff_tonenum(w, p),
            1 => self.c_fe_gate_abs(w, p),
            2 => self.c_fd_vol(w, p),
            3 => self.c_fc_tempo(w, p),
            4 => self.c_fb_tie(w, p),
            5 => self.c_fa_det(w, p),
            6 => self.c_f9_repeat_reset(w, p),
            7 => self.c_f8_repeat(w, p),
            8 => self.c_f7_repeat_exit(w, p),
            9 => self.c_f6_set_loop(w, p),
            10 => self.c_f5_transpose(w, p),
            11 => self.c_f4_volinc_fm(w, p),
            12 => self.c_f3_voldec_fm(w, p),
            13 => self.c_f2_lfo(w, p),
            14 => self.c_f1_lfo_switch_fm(w, p),
            15 => self.cmd_null(p, 4),
            16 => self.c_ef_poke(w, p),
            17 => self.cmd_null(p, 1),
            18 => self.c_ec_pan(w, p),
            19 => self.c_eb_opnarhythm(w, p),
            20 => self.c_ea_opnarhythm_il(w, p),
            21 => self.c_e9_opnarhythm_pan(w, p),
            22 => self.c_e8_opnarhythm_tl(w, p),
            23 => self.c_e7_transpose_rel(w, p),
            24 => self.c_e6_opnarhythm_tl_rel(w, p),
            25 => self.c_e5_opnarhythm_il_rel(w, p),
            26 => self.c_e4_hlfo_delay(w, p),
            27 => self.c_e3_vol_add_fm(w, p),
            28 => self.c_e2_vol_sub(w, p),
            29 => self.c_e1_ams_pms(w, p),
            30 => self.c_e0_hlfo(w, p),
            31 => self.c_df_meas_len(w, p),
            32 => self.c_de_echo_init_add_fm(w, p),
            33 => self.c_dd_echo_init_sub(w, p),
            34 => self.c_dc_status1(w, p),
            35 => self.c_db_status1_add(w, p),
            36 => self.c_da_portamento_fm(w, p),
            37 => self.c_d6_md(w, p),
            38 => self.c_d5_detrel(w, p),
            39 => self.c_d4_ssgeff(w, p),
            40 => self.c_d3_fmeff(w, p),
            41 => self.c_d2_fadeout(w, p),
            42 => self.c_cf_slotmask(w, p),
            43 => self.cmd_null(p, 6),
            44 => self.cmd_null(p, 5),
            45 => self.c_cb_lfo_waveform(w, p),
            46 => self.c_ca_lfo_ext(w, p),
            47 => self.c_c8_fm3ex_det(w, p),
            48 => self.c_c7_fm3ex_det_add(w, p),
            49 => self.c_c6_fm3ex_init(w, p),
            50 => self.c_c5_lfo_slotmask(w, p),
            51 => self.c_c4_gate_rel(w, p),
            52 => self.c_c3_pan_ex(w, p),
            53 => self.c_c2_lfo_delay(w, p),
            54 => self.cmd_null(p, 0),
            55 => self.c_c0_mml_mask_fm(w, p),
            56 => self.c_bf_lfo2(w, p),
            57 => self.c_be_lfo2_switch_fm(w, p),
            58 => self.c_bd_lfo2_md(w, p),
            59 => self.c_bc_lfo2_waveform(w, p),
            60 => self.c_bb_lfo2_ext(w, p),
            61 => self.c_ba_lfo2_slotmask(w, p),
            62 => self.c_b9_lfo2_delay(w, p),
            63 => self.c_b8_slottl(w, p),
            64 => self.c_b7_lfo_md_cnt(w, p),
            65 => self.c_b6_fb(w, p),
            66 => self.c_b5_slot_delay(w, p),
            67 => self.cmd_null(p, 16),
            68 => self.c_b3_gate_min(w, p),
            69 => self.c_b2_transpose_master(w, p),
            70 => self.c_b1_gate_rand_range(w, p),
            71 => self.c_f4_volinc_ssg(w, p),
            72 => self.c_f3_voldec_ssg(w, p),
            73 => self.c_f1_lfo_switch(w, p),
            74 => self.c_f0_env_old(w, p),
            75 => self.c_ee_noise_freq(w, p),
            76 => self.c_ed_ssgmix(w, p),
            77 => self.c_e3_vol_add_ssg(w, p),
            78 => self.c_de_echo_init_add_ssg(w, p),
            79 => self.c_da_portamento_ssg(w, p),
            80 => self.c_d0_ssgnoisefreqadd(w, p),
            81 => self.c_cd_env_new(w, p),
            82 => self.c_cc_det_ext(w, p),
            83 => self.c_c9_env_ext(w, p),
            84 => self.cmd_null(p, 3),
            85 => self.cmd_null(p, 2),
            86 => self.c_c0_mml_mask_ssg(w, p),
            87 => self.c_be_lfo2_switch(w, p),
            88 => self.c_f1_ppsdrv(w, p),
            89 => self.c_c0_mml_mask_rhythm(w, p),
            90 => self.c_ff_tonenum_adpcm(w, p),
            91 => self.c_f4_volinc_adpcm(w, p),
            92 => self.c_f3_voldec_adpcm(w, p),
            93 => self.c_ec_pan_adpcm(w, p),
            94 => self.c_e3_vol_add_adpcm(w, p),
            95 => self.c_de_echo_init_add_adpcm(w, p),
            96 => self.c_da_portamento_adpcm(w, p),
            97 => self.c_ce_adpcm_loop(w, p),
            98 => self.c_c3_pan_ex_adpcm(w, p),
            99 => self.c_c0_mml_mask_adpcm(w, p),
            100 => self.c_b4_ppz8_init(w, p),
            101 => self.c_ff_tonenum_ppz8(w, p),
            102 => self.c_ec_pan_ppz8(w, p),
            103 => self.c_da_portamento_ppz8(w, p),
            104 => self.c_ce_ppz8_loop(w, p),
            105 => self.c_c3_pan_ex_ppz8(w, p),
            106 => self.c_c0_mml_mask_ppz8(w, p),
            _ => {}
        }
    }
}

impl crate::fmwork::Interrupt for Pmd {
    fn interrupt(&mut self, w: &mut Work) {
        Pmd::interrupt(self, w);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 98fmplayer's own render (fmwav: 55,467 Hz 16-bit stereo, the SSG as
    /// the YMF288 sums it, or with FM_ANALOG as the OPNA's analog output): PMD_DIR the songs, PMD_REF the renders
    /// (<name>.raw), the first PMD_SECS seconds (10).
    #[test]
    #[ignore]
    fn the_same_as_98fmplayer() {
        let (Ok(dir), Ok(rf)) = (std::env::var("PMD_DIR"), std::env::var("PMD_REF")) else { return };
        let secs: usize = std::env::var("PMD_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(10);
        let only = std::env::var("PMD_ONLY").ok();
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
            let Some(mut pmd) = Pmd::load(&std::fs::read(&path).unwrap()) else {
                eprintln!("{name}: not read");
                continue;
            };
            let mut w = Work::new();
            pmd.init(&mut w);
            if std::env::var("FM_ANALOG").is_ok() {
                w.opna.ssg.set_ymf288(false);
            }
            let n = secs * crate::opna::RATE as usize;
            let mut buf = vec![0i16; n * 2];
            let mut at = 0;
            while at < n {
                let k = (n - at).min(4096);
                crate::fmwork::mix(&mut w, &mut pmd, &mut buf[at * 2..], k);
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
