// Copyright 2026 ansiapps. Neumetik Synth, proprietary: see LICENSE.
//
// A port of SiON's SiOPMOperator, SiOPMChannelBase and SiOPMChannelFM
// (org.si.sion.module.channels, github.com/keim/SiON):
//
// Copyright (c) 2008-2016 keim, MIT License (see table.rs for the text).

//! The operator (envelope generator and pulse generator, from MAME's OPM,
//! with the wave tables of SiOPM, key scale level, fixed pitch and SSG
//! envelopes) and the FM channel with one to four of them: every
//! algorithm, feedback, the LFO, the state variable filter, ring
//! modulation, and the analog-like two-oscillator modes.
//!
//! In SiON the pipes between operators are one-cell rings, so here they
//! are cells of the channel (`Z`, `P0`, `P1` and one feedback cell for
//! each operator); the pipes of the module are rings a block long, and a
//! channel reads and writes them at its buffer index.

use super::module::*;
use super::pcm::{PcmTable, SampleBank, SamplerData, SamplerTable};
use super::table::*;
use std::sync::Arc;

pub const EG_ATTACK: i32 = 0;
pub const EG_DECAY: i32 = 1;
pub const EG_SUSTAIN: i32 = 2;
pub const EG_RELEASE: i32 = 3;
pub const EG_OFF: i32 = 4;

/// PCM waves have 2^11 samples a phase unit.
const PCM_WAVE_FIXED_BITS: i32 = 11;

/// Cells: zero (never written), the two internal pipes, and a feedback
/// cell for each operator.
pub const Z: usize = 0;
pub const P0: usize = 1;
pub const P1: usize = 2;
const FEED: usize = 3;

pub const OUTPUT_STANDARD: i32 = 0;
pub const OUTPUT_OVERWRITE: i32 = 1;
pub const OUTPUT_ADD: i32 = 2;
pub const INPUT_ZERO: i32 = 0;
pub const INPUT_PIPE: i32 = 1;
pub const INPUT_FEEDBACK: i32 = 2;
pub const FILTER_LP: i32 = 0;
pub const FILTER_BP: i32 = 1;
pub const FILTER_HP: i32 = 2;

const NEXT_STATE: [[i32; 5]; 2] = [[EG_DECAY, EG_SUSTAIN, EG_OFF, EG_OFF, EG_OFF], [EG_DECAY, EG_SUSTAIN, EG_ATTACK, EG_OFF, EG_OFF]];

/// `SiOPMOperator`.
#[derive(Clone)]
pub struct Operator {
    table: Arc<Table>,
    pub ar: i32,
    pub dr: i32,
    pub sr: i32,
    pub rr: i32,
    pub sl: i32,
    pub tl: i32,
    ks: i32,
    ksl: i32,
    pub multiple: i32,
    dt1: i32,
    dt2: i32,
    pub ams: i32,
    kc: i32,
    ssg_type: i32,
    mute: i32,
    erst: bool,
    pg_type: i32,
    pt_type: usize,
    wave: Arc<WaveTable>,
    wave_fixed_bits: i32,
    wave_phase_step_shift: i32,
    pitch_table_filter: i32,
    pub phase: i32,
    pub phase_step: i32,
    keyon_phase: i32,
    pitch_fixed: bool,
    dt1_index: usize,
    pitch_index: i32,
    pitch_index_shift: i32,
    pitch_index_shift2: i32,
    pub fm_shift: i32,
    pub eg_state: i32,
    pub eg_timer: i32,
    pub eg_timer_step: i32,
    pub eg_counter: i32,
    eg_sustain_level: i32,
    eg_total_level: i32,
    eg_tl_offset: i32,
    eg_key_scale_rate: i32,
    eg_key_scale_level_rshift: i32,
    pub eg_level: i32,
    pub eg_out: i32,
    eg_ssgec_ar: usize,
    eg_ssgec_state: i32,
    eg_inc_sel: usize,
    eg_inc_att: bool,
    eg_state_shift_level: i32,
    eg_next_ssg: bool,
    eg_level_table: usize,
    /// Is it a final carrier.
    pub final_: bool,
    pub in_cell: usize,
    pub base_cell: usize,
    pub out_cell: usize,
    feed_cell: usize,
    // PCM
    pub pcm_channels: i32,
    pub pcm_start_point: i32,
    pub pcm_end_point: i32,
    pub pcm_loop_point: i32,
    /// The samples of a PCM wave, when it plays one.
    pub pcm_wave: Option<Arc<Vec<i32>>>,
}

impl Operator {
    pub fn new(table: Arc<Table>, index: usize) -> Operator {
        let wave = table.get_wave_table(PG_SINE);
        Operator {
            table,
            ar: 0,
            dr: 0,
            sr: 0,
            rr: 0,
            sl: 0,
            tl: 0,
            ks: 0,
            ksl: 0,
            multiple: 0,
            dt1: 0,
            dt2: 0,
            ams: 0,
            kc: 0,
            ssg_type: 0,
            mute: 0,
            erst: false,
            pg_type: 0,
            pt_type: 0,
            wave,
            wave_fixed_bits: 0,
            wave_phase_step_shift: 0,
            pitch_table_filter: 0,
            phase: 0,
            phase_step: 0,
            keyon_phase: 0,
            pitch_fixed: false,
            dt1_index: 0,
            pitch_index: 0,
            pitch_index_shift: 0,
            pitch_index_shift2: 0,
            fm_shift: 0,
            eg_state: EG_OFF,
            eg_timer: 0,
            eg_timer_step: 0,
            eg_counter: 0,
            eg_sustain_level: 0,
            eg_total_level: 0,
            eg_tl_offset: 0,
            eg_key_scale_rate: 0,
            eg_key_scale_level_rshift: 0,
            eg_level: 0,
            eg_out: 0,
            eg_ssgec_ar: 0,
            eg_ssgec_state: 0,
            eg_inc_sel: 17,
            eg_inc_att: false,
            eg_state_shift_level: 0,
            eg_next_ssg: false,
            eg_level_table: 0,
            final_: true,
            in_cell: Z,
            base_cell: Z,
            out_cell: P0,
            feed_cell: FEED + index,
            pcm_channels: 0,
            pcm_start_point: 0,
            pcm_end_point: 0,
            pcm_loop_point: -1,
            pcm_wave: None,
        }
    }

    pub fn feed_cell(&self) -> usize {
        self.feed_cell
    }

    // setters of the FM parameters

    pub fn set_ar(&mut self, i: i32) {
        self.ar = i & 63;
        self.eg_ssgec_ar = if self.ssg_type == 8 || self.ssg_type == 12 { usize::from(self.ar >= 56) } else { usize::from(self.ar >= 60) };
    }

    pub fn set_dr(&mut self, i: i32) {
        self.dr = i & 63;
    }

    pub fn set_sr(&mut self, i: i32) {
        self.sr = i & 63;
    }

    pub fn set_rr(&mut self, i: i32) {
        self.rr = i & 63;
    }

    pub fn set_sl(&mut self, i: i32) {
        self.sl = i & 15;
        self.eg_sustain_level = self.table.eg_sl_table[(i & 15) as usize];
    }

    pub fn set_tl(&mut self, i: i32) {
        self.tl = i.clamp(0, 127);
        self.update_total_level();
    }

    pub fn set_ks(&mut self, i: i32) {
        self.ks = 5 - (i & 3);
        self.eg_key_scale_rate = self.kc >> self.ks;
    }

    pub fn set_mul(&mut self, m: i32) {
        let m = m & 15;
        self.multiple = if m != 0 { m << 7 } else { 64 };
        self.update_pitch();
    }

    pub fn set_dt1(&mut self, d: i32) {
        self.dt1 = d & 7;
        self.dt1_index = self.dt1 as usize;
        self.update_pitch();
    }

    pub fn set_dt2(&mut self, d: i32) {
        self.dt2 = d & 3;
        self.pitch_index_shift = self.table.dt2_table[self.dt2 as usize];
        self.update_pitch();
    }

    pub fn set_ams(&mut self, s: i32) {
        self.ams = if s != 0 { 3 - s } else { 16 };
    }

    pub fn set_ksl(&mut self, i: i32) {
        self.ksl = i;
        self.eg_key_scale_level_rshift = if i == 0 { 8 } else { 5 - i };
        self.update_total_level();
    }

    pub fn set_ssgec(&mut self, i: i32) {
        if i > 7 {
            self.eg_next_ssg = true;
            self.ssg_type = i;
            if self.ssg_type > 17 {
                self.ssg_type = 9;
            }
        } else {
            self.eg_next_ssg = false;
            self.ssg_type = 0;
        }
    }

    pub fn set_mute(&mut self, b: bool) {
        self.mute = if b { ENV_BOTTOM } else { 0 };
        self.update_total_level();
    }

    pub fn set_erst(&mut self, b: bool) {
        self.erst = b;
    }

    pub fn get_ks(&self) -> i32 {
        5 - self.ks
    }

    pub fn get_ksl(&self) -> i32 {
        self.ksl
    }

    pub fn get_dt1(&self) -> i32 {
        self.dt1
    }

    pub fn get_ams(&self) -> i32 {
        if self.ams == 16 {
            0
        } else {
            3 - self.ams
        }
    }

    pub fn get_ssgec(&self) -> i32 {
        self.ssg_type
    }

    pub fn kc(&self) -> i32 {
        self.kc
    }

    pub fn set_kc(&mut self, i: i32) {
        if self.pitch_fixed {
            return;
        }
        self.update_kc(i & 127);
        self.pitch_index = ((self.kc - (self.kc >> 2)) << 6) | (self.pitch_index & 63);
        self.update_pitch();
    }

    pub fn set_kf(&mut self, f: i32) {
        self.pitch_index = (self.pitch_index & 0xffc0) | (f & 63);
        self.update_pitch();
    }

    pub fn set_fixed_pitch_index(&mut self, i: i32) {
        if i > 0 {
            self.pitch_index = i;
            self.update_kc(self.table.nn_to_kc[((i >> 6) & 127) as usize]);
            self.update_pitch();
            self.pitch_fixed = true;
        } else {
            self.pitch_fixed = false;
        }
    }

    pub fn pitch_index(&self) -> i32 {
        self.pitch_index
    }

    pub fn set_pitch_index(&mut self, i: i32) {
        if self.pitch_fixed {
            return;
        }
        self.pitch_index = i;
        self.update_kc(self.table.nn_to_kc[((i >> 6) & 127) as usize]);
        self.update_pitch();
    }

    pub fn set_detune(&mut self, d: i32) {
        self.dt2 = 0;
        self.pitch_index_shift = d;
        self.update_pitch();
    }

    pub fn detune(&self) -> i32 {
        self.pitch_index_shift
    }

    pub fn set_detune2(&mut self, d: i32) {
        self.pitch_index_shift2 = d;
        self.update_pitch();
    }

    pub fn set_fmul(&mut self, m: i32) {
        self.multiple = m;
        self.update_pitch();
    }

    pub fn set_key_on_phase(&mut self, p: i32) {
        self.keyon_phase = if p == 255 {
            -2
        } else if p == -1 {
            -1
        } else {
            (p & 255) << (PHASE_BITS - 8)
        };
    }

    pub fn key_on_phase(&self) -> i32 {
        if self.keyon_phase >= 0 {
            self.keyon_phase >> (PHASE_BITS - 8)
        } else if self.keyon_phase == -1 {
            -1
        } else {
            255
        }
    }

    pub fn set_pg_type(&mut self, n: i32) {
        self.pg_type = n & PG_FILTER;
        self.wave = self.table.get_wave_table(self.pg_type as usize);
        self.wave_fixed_bits = self.wave.fixed_bits;
    }

    pub fn pg_type(&self) -> i32 {
        self.pg_type
    }

    pub fn set_pt_type(&mut self, n: usize) {
        self.pt_type = n;
        self.wave_phase_step_shift = (PHASE_BITS - self.wave_fixed_bits) & self.table.phase_step_shift_filter[n];
        self.pitch_table_filter = self.table.pitch_table[n].len() as i32 - 1;
    }

    pub fn pt_type(&self) -> usize {
        self.pt_type
    }

    pub fn set_mod_level(&mut self, m: i32) {
        self.fm_shift = if m != 0 { m + 10 } else { 0 };
    }

    pub fn mod_level(&self) -> i32 {
        if self.fm_shift > 10 {
            self.fm_shift - 10
        } else {
            0
        }
    }

    /// `_tlOffset`: the level offset the velocity and expression make.
    pub fn tl_offset(&mut self, i: i32) {
        self.eg_tl_offset = i;
        self.update_total_level();
    }

    // operations

    /// `initialize`: the connections back, every parameter as the module's
    /// initial operator parameter has it.
    pub fn initialize(&mut self, init: &OperatorParam) {
        self.final_ = true;
        self.in_cell = Z;
        self.base_cell = Z;
        self.set_operator_param(init);
        self.eg_tl_offset = 0;
        self.pitch_index_shift2 = 0;
        self.pcm_channels = 0;
        self.pcm_start_point = 0;
        self.pcm_end_point = 0;
        self.pcm_loop_point = -1;
        self.pcm_wave = None;
        self.reset();
    }

    pub fn reset(&mut self) {
        self.eg_shift_state(EG_OFF);
        self.eg_out = (self.table.eg_level_tables[self.eg_level_table][self.eg_level as usize] + self.eg_total_level) << 3;
        self.eg_timer = ENV_TIMER_INITIAL;
        self.eg_counter = 0;
        self.eg_ssgec_state = 0;
        self.phase = 0;
    }

    pub fn set_operator_param(&mut self, p: &OperatorParam) {
        self.set_pg_type(p.pg_type);
        self.set_pt_type(p.pt_type);
        self.keyon_phase = if p.phase == 255 {
            -2
        } else if p.phase == -1 {
            -1
        } else {
            (p.phase & 255) << (PHASE_BITS - 8)
        };
        self.ar = p.ar & 63;
        self.dr = p.dr & 63;
        self.sr = p.sr & 63;
        self.rr = p.rr & 63;
        self.ks = 5 - (p.ksr & 3);
        self.ksl = p.ksl & 3;
        self.ams = if p.ams != 0 { 3 - p.ams } else { 16 };
        self.multiple = p.fmul;
        self.fm_shift = (p.mod_level & 7) + 10;
        self.dt1 = p.dt1 & 7;
        self.dt1_index = self.dt1 as usize;
        self.pitch_index_shift = p.detune;
        self.set_ssgec(p.ssgec);
        self.mute = if p.mute { ENV_BOTTOM } else { 0 };
        self.erst = p.erst;
        if p.fixed_pitch == 0 {
            self.pitch_fixed = false;
        } else {
            self.pitch_index = p.fixed_pitch;
            self.update_kc(self.table.nn_to_kc[((self.pitch_index >> 6) & 127) as usize]);
            self.pitch_fixed = true;
        }
        self.eg_key_scale_level_rshift = if self.ksl == 0 { 8 } else { 5 - self.ksl };
        self.eg_ssgec_ar = if self.ssg_type == 8 || self.ssg_type == 12 { usize::from(self.ar >= 56) } else { usize::from(self.ar >= 60) };
        self.set_sl(p.sl & 15);
        self.set_tl(p.tl);
        self.update_pitch();
    }

    pub fn operator_param(&self) -> OperatorParam {
        OperatorParam {
            pg_type: self.pg_type,
            pt_type: self.pt_type,
            ar: self.ar,
            dr: self.dr,
            sr: self.sr,
            rr: self.rr,
            sl: self.sl,
            tl: self.tl,
            ksr: self.get_ks(),
            ksl: self.ksl,
            fmul: self.multiple,
            dt1: self.dt1,
            detune: self.pitch_index_shift,
            ams: self.get_ams(),
            ssgec: self.ssg_type,
            phase: self.key_on_phase(),
            mod_level: self.mod_level(),
            erst: self.erst,
            fixed_pitch: 0,
            mute: self.mute != 0,
        }
    }

    pub fn set_wave_table(&mut self, w: &Arc<WaveTable>) {
        self.pg_type = -1;
        self.wave = w.clone();
        self.wave_fixed_bits = w.fixed_bits;
        self.set_pt_type(w.default_pt_type);
    }

    /// `setPCMData`: a PCM wave (samples as log table indices).
    pub fn set_pcm_data(&mut self, wavelet: Option<(Arc<Vec<i32>>, i32, i32, i32, i32)>) {
        match wavelet {
            Some((w, channels, start, end, loop_point)) => {
                self.pg_type = -2;
                self.wave = Arc::new(WaveTable { wavelet: vec![], fixed_bits: PCM_WAVE_FIXED_BITS, default_pt_type: PT_PCM });
                self.pcm_wave = Some(w);
                self.wave_fixed_bits = PCM_WAVE_FIXED_BITS;
                self.pcm_channels = channels;
                self.pcm_start_point = start;
                self.pcm_end_point = end;
                self.pcm_loop_point = loop_point;
                self.keyon_phase = start << PCM_WAVE_FIXED_BITS;
                self.set_pt_type(PT_PCM);
            }
            None => {
                self.pcm_end_point = 0;
                self.pcm_loop_point = -1;
            }
        }
    }

    pub fn note_on(&mut self, rng: &mut u32) {
        if self.keyon_phase >= 0 {
            self.phase = self.keyon_phase;
        } else if self.keyon_phase == -1 {
            *rng ^= *rng << 13;
            *rng ^= *rng >> 17;
            *rng ^= *rng << 5;
            self.phase = (f64::from(*rng) / 4294967296.0 * f64::from(PHASE_MAX)) as i32;
        }
        self.eg_ssgec_state = -1;
        self.eg_shift_state(EG_ATTACK);
        self.eg_out = (self.table.eg_level_tables[self.eg_level_table][self.eg_level as usize] + self.eg_total_level) << 3;
    }

    /// `_eg_out` from the level and the total level.
    pub fn update_eg_out(&mut self) {
        self.eg_out = (self.table.eg_level_tables[self.eg_level_table][self.eg_level as usize] + self.eg_total_level) << 3;
    }

    pub fn note_off(&mut self) {
        self.eg_shift_state(EG_RELEASE);
        self.eg_out = (self.table.eg_level_tables[self.eg_level_table][self.eg_level as usize] + self.eg_total_level) << 3;
    }

    /// `_setPipes`: the cells it writes, reads its modulation from, and
    /// whether it's a final carrier.
    pub fn set_pipes(&mut self, out: usize, modulation: Option<usize>, final_osc: bool) {
        self.final_ = final_osc;
        self.base_cell = if Some(out) == modulation { Z } else { out };
        self.out_cell = out;
        self.in_cell = modulation.unwrap_or(Z);
        self.fm_shift = 15;
    }

    pub fn set_base_cell(&mut self, c: usize) {
        self.base_cell = c;
    }

    /// The envelope generator's step (the inlined `eg_update`).
    #[inline]
    pub fn eg_update(&mut self, eg_timer_initial: i32) {
        self.eg_timer -= self.eg_timer_step;
        if self.eg_timer < 0 {
            let inc = if self.eg_inc_att { self.table.eg_inc_tables_att[self.eg_inc_sel][self.eg_counter as usize] } else { self.table.eg_inc_tables[self.eg_inc_sel][self.eg_counter as usize] };
            if self.eg_state == EG_ATTACK {
                if inc > 0 {
                    self.eg_level -= 1 + (self.eg_level >> inc);
                    if self.eg_level <= 0 {
                        self.eg_shift_state(NEXT_STATE[usize::from(self.eg_next_ssg)][self.eg_state as usize]);
                    }
                }
            } else {
                self.eg_level += inc;
                if self.eg_level >= self.eg_state_shift_level {
                    self.eg_shift_state(NEXT_STATE[usize::from(self.eg_next_ssg)][self.eg_state as usize]);
                }
            }
            self.eg_out = (self.table.eg_level_tables[self.eg_level_table][self.eg_level.clamp(0, 1023) as usize] + self.eg_total_level) << 3;
            self.eg_counter = (self.eg_counter + 1) & 7;
            self.eg_timer += eg_timer_initial;
        }
    }

    /// `_eg_shiftState`.
    pub fn eg_shift_state(&mut self, state: i32) {
        let table = self.table.clone();
        let mut st = state;
        let rate = |r: i32| r.clamp(0, 127) as usize;
        if st == EG_ATTACK {
            self.eg_ssgec_state += 1;
            if self.eg_ssgec_state == 3 {
                self.eg_ssgec_state = 1;
            }
            if self.ar + self.eg_key_scale_rate < 62 {
                if self.erst {
                    self.eg_level = ENV_BOTTOM;
                }
                self.eg_state = EG_ATTACK;
                let r = rate(if self.ar != 0 { self.ar + self.eg_key_scale_rate } else { 96 });
                self.eg_inc_att = true;
                self.eg_inc_sel = table.eg_table_selector[r] as usize;
                self.eg_timer_step = table.eg_timer_steps[r];
                self.eg_level_table = 0;
                return;
            }
            st = EG_DECAY;
        }
        if st == EG_DECAY {
            if self.eg_sustain_level != 0 {
                self.eg_state = EG_DECAY;
                if self.ssg_type != 0 {
                    self.eg_level = 0;
                    self.eg_state_shift_level = self.eg_sustain_level >> 2;
                    if self.eg_state_shift_level > ENV_BOTTOM_SSGEC {
                        self.eg_state_shift_level = ENV_BOTTOM_SSGEC;
                    }
                    self.eg_level_table = table.eg_ssg_table_index[(self.ssg_type - 8) as usize][self.eg_ssgec_ar][self.eg_ssgec_state.clamp(0, 2) as usize];
                } else {
                    self.eg_level = 0;
                    self.eg_state_shift_level = self.eg_sustain_level;
                    self.eg_level_table = 0;
                }
                let r = rate(if self.dr != 0 { self.dr + self.eg_key_scale_rate } else { 96 });
                self.eg_inc_att = false;
                self.eg_inc_sel = table.eg_table_selector[r] as usize;
                self.eg_timer_step = table.eg_timer_steps[r];
                return;
            }
            st = EG_SUSTAIN;
        }
        if st == EG_SUSTAIN {
            self.eg_state = EG_SUSTAIN;
            if self.ssg_type != 0 {
                self.eg_level = self.eg_sustain_level >> 2;
                self.eg_state_shift_level = ENV_BOTTOM_SSGEC;
                self.eg_level_table = table.eg_ssg_table_index[(self.ssg_type - 8) as usize][self.eg_ssgec_ar][self.eg_ssgec_state.clamp(0, 2) as usize];
            } else {
                self.eg_level = self.eg_sustain_level;
                self.eg_state_shift_level = ENV_BOTTOM;
                self.eg_level_table = 0;
            }
            let r = rate(if self.sr != 0 { self.sr + self.eg_key_scale_rate } else { 96 });
            self.eg_inc_att = false;
            self.eg_inc_sel = table.eg_table_selector[r] as usize;
            self.eg_timer_step = table.eg_timer_steps[r];
            return;
        }
        if st == EG_RELEASE {
            if self.eg_level < ENV_BOTTOM {
                self.eg_state = EG_RELEASE;
                self.eg_state_shift_level = ENV_BOTTOM;
                let r = rate(self.rr + self.eg_key_scale_rate);
                self.eg_inc_att = false;
                self.eg_inc_sel = table.eg_table_selector[r] as usize;
                self.eg_timer_step = table.eg_timer_steps[r];
                self.eg_level_table = usize::from(self.ssg_type != 0);
                return;
            }
        }
        // EG_OFF, and the catch-all.
        self.eg_state = EG_OFF;
        self.eg_level = ENV_BOTTOM;
        self.eg_state_shift_level = ENV_BOTTOM + 1;
        self.eg_inc_att = false;
        self.eg_inc_sel = 17;
        self.eg_timer_step = table.eg_timer_steps[96];
        self.eg_level_table = 0;
    }

    fn update_kc(&mut self, i: i32) {
        self.kc = i;
        self.eg_key_scale_rate = self.kc >> self.ks;
        self.update_total_level();
    }

    fn update_pitch(&mut self) {
        let n = (self.pitch_index.wrapping_add(self.pitch_index_shift).wrapping_add(self.pitch_index_shift2)) & self.pitch_table_filter;
        let ps = self.table.pitch_table[self.pt_type][n as usize] >> self.wave_phase_step_shift;
        self.update_phase_step(ps);
    }

    fn update_phase_step(&mut self, ps: i32) {
        self.phase_step = ps;
        self.phase_step = self.phase_step.wrapping_add(self.table.dt1_table[self.dt1_index][self.kc as usize]);
        self.phase_step = self.phase_step.wrapping_mul(self.multiple);
        self.phase_step >>= 7 - self.table.sample_rate_pitch_shift;
    }

    fn update_total_level(&mut self) {
        self.eg_total_level = ((self.tl + (self.kc >> self.eg_key_scale_level_rshift)) << ENV_LSHIFT) + self.eg_tl_offset + self.mute;
        if self.eg_total_level > ENV_BOTTOM {
            self.eg_total_level = ENV_BOTTOM;
        }
        self.eg_total_level -= ENV_TOP;
        self.eg_out = (self.table.eg_level_tables[self.eg_level_table][self.eg_level.clamp(0, 1023) as usize] + self.eg_total_level) << 3;
    }

    /// The wave's sample at a phase.
    #[inline]
    fn sample(&self, phase: i32) -> i32 {
        let t = ((phase & PHASE_FILTER) >> self.wave_fixed_bits) as usize;
        self.wave.wavelet.get(t).copied().unwrap_or(LOG_TABLE_BOTTOM)
    }
}

/// What a channel reads as its input.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Source {
    Zero,
    Feed(usize),
    Pipe(usize),
}

/// The module a channel is: FM and the rest, the PCM module (`%7`) or the sampler (`%10`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Fm,
    Pcm,
    Sampler,
}

/// `SiOPMChannelPCM`'s own state.
struct PcmState {
    table: Option<Arc<PcmTable>>,
    pitch_shift: i32,
    volume: f64,
    pan: i32,
    /// The second pipe of a stereo wave.
    pipe2: usize,
}

/// `SiOPMChannelSampler`'s own state.
struct SamplerState {
    table: Option<Arc<SamplerTable>>,
    data: Option<Arc<SamplerData>>,
    wave_number: i32,
    expression: f64,
    index: i32,
    start_phase: i32,
    sample_pan: i32,
}

/// Which loop a channel runs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Proc {
    Op1,
    Op2,
    Op3,
    Op4,
    Ana,
    Ring,
    Sync,
    Afm,
    Pcm,
}

/// The Karplus-Strong state of a channel (`SiOPMChannelKS`).
pub struct KsState {
    delay_buffer: Vec<i32>,
    delay_buffer_index: f64,
    pitch_index: i32,
    decay_lpf: f64,
    decay: f64,
    mute_decay_lpf: f64,
    mute_decay: f64,
    output: f64,
    cur_decay_lpf: f64,
    cur_decay: f64,
    expression: f64,
    pub seed_type: i32,
    pub seed_index: i32,
}

/// Samples of the longest delay: 5394 is MIDI note 0.
const KS_BUFFER_SIZE: usize = 5400;
pub const KS_SEED_DEFAULT: i32 = 0;
pub const KS_SEED_FM: i32 = 1;
pub const KS_SEED_PCM: i32 = 2;

impl KsState {
    fn new() -> KsState {
        KsState {
            delay_buffer: vec![0; KS_BUFFER_SIZE],
            delay_buffer_index: 0.0,
            pitch_index: 0,
            decay_lpf: 0.875,
            decay: 0.98,
            mute_decay_lpf: 0.5,
            mute_decay: 0.75,
            output: 0.0,
            cur_decay_lpf: 0.5,
            cur_decay: 0.75,
            expression: 1.0,
            seed_type: 0,
            seed_index: 0,
        }
    }
}

/// `SiOPMChannelBase` and `SiOPMChannelFM`, and with `ks` set, `SiOPMChannelKS`.
pub struct FmChannel {
    table: Arc<Table>,
    pub operators: [Operator; 4],
    pub active_operator: usize,
    operator_count: usize,
    algorism: i32,
    proc_type: Proc,
    cells: [i32; 7],
    am_depth: i32,
    am_out: i32,
    pm_depth: i32,
    pm_out: i32,
    eg_timer_initial: i32,
    lfo_timer_initial: i32,
    // base
    pub is_note_on: bool,
    pub is_idling: bool,
    buffer_index: usize,
    input_level: i32,
    ringmod_level: f64,
    input_mode: i32,
    output_mode: i32,
    in_src: Source,
    ring_pipe: Option<usize>,
    base_add: bool,
    out_pipe: usize,
    pub volumes: [f64; STREAM_SEND_SIZE],
    pub has_effect_send: bool,
    pub pan: i32,
    pub mute: bool,
    velocity_table: usize,
    expression_table: usize,
    filter_on: bool,
    filter_type: i32,
    cutoff: i32,
    cutoff_offset: i32,
    resonance: f64,
    filter_variables: [f64; 3],
    filter_variables2: [f64; 3],
    prev_step_remain: i32,
    filter_eg_step: i32,
    filter_eg_next: i32,
    filter_eg_cutoff_inc: i32,
    filter_eg_state: i32,
    filter_eg_time: [i32; 6],
    filter_eg_cutoff: [i32; 6],
    freq_ratio: i32,
    lfo_on: bool,
    lfo_timer: i32,
    lfo_timer_step: i32,
    lfo_timer_step_: i32,
    lfo_phase: i32,
    lfo_wave: Vec<i32>,
    lfo_wave_shape: i32,
    rng: u32,
    init_operator_param: OperatorParam,
    ks: Option<Box<KsState>>,
    kind: Kind,
    /// The channel number the register writes are for (the second argument of `%`).
    register_channel: i32,
    reg_pmd: i32,
    reg_amd: i32,
    pcm: PcmState,
    sampler: SamplerState,
}

// Filter EG states (the base class's own).
const FEG_ATTACK: i32 = 0;
const FEG_DECAY1: i32 = 1;
const FEG_DECAY2: i32 = 2;
const FEG_SUSTAIN: i32 = 3;
const FEG_RELEASE: i32 = 4;
const FEG_OFF: i32 = 5;

/// `eg_out` below which a final carrier is audible.
pub const IDLING_THRESHOLD: i32 = 5120;

impl FmChannel {
    pub fn new(module: &Module) -> FmChannel {
        let table = module.table.clone();
        let t = table.clone();
        let mut c = FmChannel {
            operators: [Operator::new(t.clone(), 0), Operator::new(t.clone(), 1), Operator::new(t.clone(), 2), Operator::new(t.clone(), 3)],
            table,
            active_operator: 0,
            operator_count: 1,
            algorism: 0,
            proc_type: Proc::Op1,
            cells: [0; 7],
            am_depth: 0,
            am_out: 0,
            pm_depth: 0,
            pm_out: 0,
            eg_timer_initial: ENV_TIMER_INITIAL,
            lfo_timer_initial: LFO_TIMER_INITIAL,
            is_note_on: false,
            is_idling: true,
            buffer_index: 0,
            input_level: 0,
            ringmod_level: 0.0,
            input_mode: INPUT_ZERO,
            output_mode: OUTPUT_STANDARD,
            in_src: Source::Zero,
            ring_pipe: None,
            base_add: false,
            out_pipe: 4,
            volumes: [0.0; STREAM_SEND_SIZE],
            has_effect_send: false,
            pan: 64,
            mute: false,
            velocity_table: VM_LINEAR,
            expression_table: VM_LINEAR,
            filter_on: false,
            filter_type: 0,
            cutoff: 0,
            cutoff_offset: 0,
            resonance: 0.0,
            filter_variables: [0.0; 3],
            filter_variables2: [0.0; 3],
            prev_step_remain: 0,
            filter_eg_step: 0,
            filter_eg_next: 0,
            filter_eg_cutoff_inc: 0,
            filter_eg_state: 0,
            filter_eg_time: [0; 6],
            filter_eg_cutoff: [0; 6],
            freq_ratio: 100,
            lfo_on: false,
            lfo_timer: 0,
            lfo_timer_step: 0,
            lfo_timer_step_: 0,
            lfo_phase: 0,
            lfo_wave: vec![0; 256],
            lfo_wave_shape: 0,
            rng: 0x2545f491,
            init_operator_param: module.init_operator_param.clone(),
            ks: None,
            kind: Kind::Fm,
            register_channel: 0,
            reg_pmd: 0,
            reg_amd: 0,
            pcm: PcmState { table: None, pitch_shift: 0, volume: 1.0, pan: 0, pipe2: 3 },
            sampler: SamplerState { table: None, data: None, wave_number: -1, expression: 1.0, index: 0, start_phase: 0, sample_pan: 0 },
        };
        c.initialize(None, 0, module);
        c
    }

    pub fn operator_count(&self) -> usize {
        self.operator_count
    }

    pub fn algorism(&self) -> i32 {
        self.algorism
    }

    pub fn buffer_index(&self) -> usize {
        self.buffer_index
    }

    pub fn lfo_timer_step(&self) -> i32 {
        self.lfo_timer_step_
    }

    fn active(&mut self) -> &mut Operator {
        &mut self.operators[self.active_operator]
    }

    // chip, LFO

    pub fn set_frequency_ratio(&mut self, ratio: i32) {
        self.freq_ratio = ratio;
        let r = if ratio != 0 { 100.0 / f64::from(ratio) } else { 1.0 };
        self.eg_timer_initial = to_i32(f64::from(ENV_TIMER_INITIAL) * r);
        self.lfo_timer_initial = to_i32(f64::from(LFO_TIMER_INITIAL) * r);
    }

    /// `initializeLFO`: a waveform (-1 for a table of your own) and stop it.
    pub fn initialize_lfo(&mut self, waveform: i32, custom: Option<&[i32]>) {
        if self.kind == Kind::Pcm {
            // `SiOPMChannelPCM.initializeLFO` forgets the table.
            self.pcm.table = None;
        }
        if waveform == -1 && custom.is_some_and(|c| c.len() == 256) {
            self.lfo_wave_shape = -1;
            self.lfo_wave = custom.unwrap().to_vec();
        } else {
            self.lfo_wave_shape = if (0..=LFO_WAVE_MAX as i32).contains(&waveform) { waveform } else { LFO_WAVE_TRIANGLE as i32 };
            self.lfo_wave = self.table.lfo_wave_tables.get(self.lfo_wave_shape as usize).cloned().unwrap_or_else(|| vec![0; 256]);
        }
        self.lfo_timer = 1;
        self.lfo_timer_step_ = 0;
        self.lfo_timer_step = 0;
        self.lfo_phase = 0;
        self.lfo_switch(false);
        self.am_depth = 0;
        self.pm_depth = 0;
        self.am_out = 0;
        self.pm_out = 0;
        for o in self.operators.iter_mut().take(self.operator_count) {
            o.set_detune2(0);
        }
    }

    /// `setLFOCycleTime` in milliseconds.
    pub fn set_lfo_cycle_time(&mut self, ms: f64) {
        self.lfo_timer = 0;
        let v = to_i32(f64::from(LFO_TIMER_INITIAL) / (ms * 0.17294117647058824)) << self.table.sample_rate_pitch_shift;
        self.lfo_timer_step_ = v;
        self.lfo_timer_step = v;
    }

    pub fn set_lfo_timer(&mut self, t: i32) {
        self.lfo_timer = t;
    }

    pub fn set_lfo_freq_step(&mut self, step: i32) {
        self.lfo_timer = i32::from(step > 0);
        self.lfo_timer_step_ = step;
        self.lfo_timer_step = step;
    }

    pub fn set_amplitude_modulation(&mut self, depth: i32) {
        self.am_depth = depth << 2;
        self.am_out = ((self.lfo_wave[self.lfo_phase as usize] * self.am_depth) >> 7) << 3;
        self.lfo_switch(self.pm_depth != 0 || self.am_depth > 0);
    }

    pub fn set_pitch_modulation(&mut self, depth: i32) {
        self.pm_depth = depth;
        self.pm_out = (((self.lfo_wave[self.lfo_phase as usize] << 1) - 255) * self.pm_depth) >> 8;
        self.lfo_switch(self.pm_depth != 0 || self.am_depth > 0);
        if self.pm_depth == 0 {
            for o in self.operators.iter_mut().take(self.operator_count) {
                o.set_detune2(0);
            }
        }
    }

    fn lfo_switch(&mut self, sw: bool) {
        if self.ks.is_some() {
            self.lfo_on = false;
            return;
        }
        self.lfo_on = sw;
        self.lfo_timer_step = if sw { self.lfo_timer_step_ } else { 0 };
    }

    // volume and connections

    pub fn set_all_stream_send_levels(&mut self, param: &[i32]) {
        for i in 0..STREAM_SEND_SIZE {
            let v = param.get(i).copied().unwrap_or(i32::MIN);
            self.volumes[i] = if v != i32::MIN { f64::from(v) * 0.0078125 } else { 0.0 };
        }
        self.has_effect_send = self.volumes[1..].iter().any(|&v| v > 0.0);
    }

    pub fn set_stream_send(&mut self, stream: usize, volume: f64) {
        self.volumes[stream] = volume;
        if stream == 0 {
            return;
        }
        self.has_effect_send = self.volumes[1..].iter().any(|&v| v > 0.0);
    }

    pub fn set_master_volume(&mut self, v: i32) {
        self.volumes[0] = f64::from(v.clamp(0, 128)) * 0.0078125;
    }

    pub fn set_pan(&mut self, p: i32) {
        self.pan = if p < -64 {
            0
        } else if p > 64 {
            128
        } else {
            p + 64
        };
    }

    pub fn set_filter_type(&mut self, mode: i32) {
        self.filter_type = if !(0..=2).contains(&mode) { 0 } else { mode };
    }

    pub fn set_volume_tables(&mut self, vtable: usize, xtable: usize) {
        self.velocity_table = vtable;
        self.expression_table = xtable;
    }

    /// `setInput`: from a pipe at a level (5 is a standard FM input).
    pub fn set_input(&mut self, level: i32, pipe_index: i32) {
        let p = (pipe_index & 3) as usize;
        if level > 0 {
            self.in_src = Source::Pipe(p);
            self.input_mode = INPUT_PIPE;
            self.input_level = level + 10;
        } else {
            self.in_src = Source::Zero;
            self.input_mode = INPUT_ZERO;
            self.input_level = 0;
        }
    }

    pub fn set_ring_modulation(&mut self, level: i32, pipe_index: i32) {
        let p = (pipe_index & 3) as usize;
        self.ringmod_level = f64::from(level * 4) / f64::from(1 << LOG_VOLUME_BITS);
        self.ring_pipe = if level > 0 { Some(p) } else { None };
    }

    pub fn set_output(&mut self, output_mode: i32, pipe_index: i32) {
        let mut p = (pipe_index & 3) as usize;
        let add;
        if output_mode == OUTPUT_STANDARD {
            p = 4;
            add = false;
        } else {
            add = output_mode == OUTPUT_ADD;
        }
        self.output_mode = output_mode;
        self.out_pipe = p;
        self.base_add = add;
    }

    /// `setSVFilter`.
    #[allow(clippy::too_many_arguments)]
    pub fn set_sv_filter(&mut self, cutoff: i32, resonance: i32, ar: i32, dr1: i32, dr2: i32, rr: i32, dc1: i32, dc2: i32, sc: i32, rc: i32) {
        let c = |v: i32| v.clamp(0, 128);
        self.filter_eg_cutoff[FEG_ATTACK as usize] = c(cutoff);
        self.filter_eg_cutoff[FEG_DECAY1 as usize] = c(dc1);
        self.filter_eg_cutoff[FEG_DECAY2 as usize] = c(dc2);
        self.filter_eg_cutoff[FEG_SUSTAIN as usize] = c(sc);
        self.filter_eg_cutoff[FEG_RELEASE as usize] = 0;
        self.filter_eg_cutoff[FEG_OFF as usize] = c(rc);
        let rate = |v: i32| self.table.filter_eg_rate[(v & 63) as usize];
        self.filter_eg_time = [rate(ar), rate(dr1), rate(dr2), i32::MAX, rate(rr), i32::MAX];
        let res = resonance.clamp(0, 9);
        self.resonance = f64::from(1 << (9 - res)) * 0.001953125;
        self.filter_on = cutoff < 128 || resonance > 0 || ar > 0 || rr > 0;
    }

    pub fn set_sv_filter_default(&mut self) {
        self.set_sv_filter(128, 0, 0, 0, 0, 0, 128, 128, 128, 128);
    }

    pub fn offset_filter(&mut self, i: i32) {
        self.cutoff_offset = i - 128;
    }

    fn reset_sv_filter_state(&mut self) {
        self.cutoff = self.filter_eg_cutoff[FEG_ATTACK as usize];
    }

    fn shift_sv_filter_state(&mut self, state: i32) {
        let mut st = state;
        let mut shift = |me: &mut FmChannel, st: i32| -> bool {
            let t = me.filter_eg_time[st as usize];
            if t == 0 {
                return false;
            }
            me.filter_eg_state = st;
            me.filter_eg_step = t;
            me.filter_eg_next = me.filter_eg_cutoff[(st + 1) as usize];
            me.filter_eg_cutoff_inc = if me.cutoff < me.filter_eg_next { 1 } else { -1 };
            me.cutoff != me.filter_eg_next
        };
        let mut done = false;
        if st == FEG_ATTACK {
            if shift(self, st) {
                done = true;
            } else {
                st += 1;
            }
        }
        if !done && st == FEG_DECAY1 {
            if shift(self, st) {
                done = true;
            } else {
                st += 1;
            }
        }
        if !done && st == FEG_DECAY2 {
            if shift(self, st) {
                done = true;
            } else {
                st += 1;
            }
        }
        if !done && st == FEG_SUSTAIN {
            self.filter_eg_state = FEG_SUSTAIN;
            self.filter_eg_step = i32::MAX;
            self.filter_eg_next = self.cutoff + 1;
            self.filter_eg_cutoff_inc = 0;
            done = true;
        }
        if !done && st == FEG_RELEASE {
            if shift(self, st) {
                done = true;
            } else {
                st += 1;
            }
        }
        if !done && st >= FEG_OFF {
            self.filter_eg_state = FEG_OFF;
            self.filter_eg_step = i32::MAX;
            self.filter_eg_next = self.cutoff + 1;
            self.filter_eg_cutoff_inc = 0;
        }
        self.prev_step_remain = self.filter_eg_step;
    }

    // parameters

    pub fn set_channel_param(&mut self, p: &ChannelParam, with_volume: bool, with_modulation: bool) {
        if p.ope_count == 0 {
            return;
        }
        if with_volume {
            self.volumes = p.volumes;
            self.has_effect_send = self.volumes[1..].iter().any(|&v| v > 0.0);
            self.pan = p.pan;
        }
        if self.kind == Kind::Sampler {
            return;
        }
        self.set_frequency_ratio(p.fratio);
        self.set_algorism(p.ope_count, p.alg);
        self.set_feedback(p.fb, p.fbc);
        if with_modulation {
            self.initialize_lfo(p.lfo_wave_shape, None);
            self.lfo_timer = i32::from(p.lfo_freq_step > 0);
            self.lfo_timer_step_ = p.lfo_freq_step;
            self.lfo_timer_step = p.lfo_freq_step;
            self.set_amplitude_modulation(p.amd);
            self.set_pitch_modulation(p.pmd);
        }
        self.set_filter_type(p.filter_type);
        self.set_sv_filter(p.cutoff, p.resonance, p.far, p.fdr1, p.fdr2, p.frr, p.fdc1, p.fdc2, p.fsc, p.frc);
        for i in 0..self.operator_count {
            self.operators[i].set_operator_param(&p.operator_param[i]);
        }
    }

    pub fn channel_param(&self) -> ChannelParam {
        let mut p = ChannelParam { volumes: self.volumes, pan: self.pan, fratio: self.freq_ratio, ope_count: self.operator_count as i32, alg: self.algorism, ..Default::default() };
        for i in 0..self.operator_count {
            if self.in_src == Source::Feed(self.operators[i].feed_cell()) {
                p.fb = self.input_level - 6;
                p.fbc = i as i32;
                break;
            }
        }
        p.lfo_wave_shape = self.lfo_wave_shape;
        p.lfo_freq_step = self.lfo_timer_step_;
        p.amd = self.am_depth;
        p.pmd = self.pm_depth;
        for i in 0..self.operator_count {
            p.operator_param[i] = self.operators[i].operator_param();
        }
        p
    }

    /// `setSiOPMParameters`: 14 parameters of the active operator (`i32::MIN` leaves one).
    pub fn set_parameters14(&mut self, p: &[i32; 14]) {
        let none = i32::MIN;
        let ope = self.active();
        if p[0] != none {
            ope.set_ar(p[0]);
        }
        if p[1] != none {
            ope.set_dr(p[1]);
        }
        if p[2] != none {
            ope.set_sr(p[2]);
        }
        if p[3] != none {
            ope.set_rr(p[3]);
        }
        if p[4] != none {
            ope.set_sl(p[4]);
        }
        if p[5] != none {
            ope.set_tl(p[5]);
        }
        if p[6] != none {
            ope.set_ks(p[6]);
        }
        if p[7] != none {
            ope.set_ksl(p[7]);
        }
        if p[8] != none {
            ope.set_mul(p[8]);
        }
        if p[9] != none {
            ope.set_dt1(p[9]);
        }
        if p[10] != none {
            ope.set_detune(p[10]);
        }
        if p[11] != none {
            ope.set_ams(p[11]);
        }
        if p[12] != none {
            ope.set_key_on_phase(p[12]);
        }
        if p[13] != none {
            ope.set_fixed_pitch_index(p[13] << 6);
        }
    }

    /// `setParameters` (`@`): the tone number, then the 14.
    pub fn set_parameters(&mut self, param: &[i32]) {
        if self.kind == Kind::Sampler {
            return;
        }
        if self.ks.is_some() {
            let g = |i: usize| param.get(i).copied().unwrap_or(i32::MIN);
            let seed = if g(0) == i32::MIN { 0 } else { g(0) };
            let index = if g(1) == i32::MIN { 0 } else { g(1) };
            if let Some(ks) = self.ks.as_mut() {
                ks.seed_type = seed;
                ks.seed_index = index;
            }
            // An FM seed is the track's to set (it has the voices); PCM has no data here.
            if seed != KS_SEED_FM && seed != KS_SEED_PCM {
                if let Some(ks) = self.ks.as_mut() {
                    ks.seed_type = KS_SEED_DEFAULT;
                }
                let n = i32::MIN;
                let _ = n;
                self.set_parameters14(&[g(1), g(2), 0, 63, 15, g(3), 0, 0, 1, 0, 0, 0, 0, g(4)]);
                let pg = if g(5) == i32::MIN { PG_NOISE_PINK as i32 } else { g(5) };
                let ope = self.active_operator;
                self.operators[ope].set_pg_type(pg);
                let pt = self.table.get_wave_table((pg & PG_FILTER) as usize).default_pt_type;
                self.operators[ope].set_pt_type(pt);
            }
            return;
        }
        let mut p = [i32::MIN; 14];
        for (i, slot) in p.iter_mut().enumerate() {
            *slot = param.get(i + 1).copied().unwrap_or(i32::MIN);
        }
        self.set_parameters14(&p);
    }

    pub fn set_wave_table(&mut self, w: &Arc<WaveTable>) {
        for o in self.operators.iter_mut().take(self.operator_count) {
            o.set_wave_table(w);
        }
    }

    pub fn set_algorism(&mut self, cnt: i32, alg: i32) {
        if self.kind != Kind::Fm {
            // The PCM module and the sampler have one operator, and no algorithm.
            return;
        }
        match cnt {
            2 => self.algorism2(alg),
            3 => self.algorism3(alg),
            4 => self.algorism4(alg),
            5 => self.analog(alg),
            _ => self.algorism1(alg),
        }
    }

    pub fn set_feedback(&mut self, fb: i32, fbc: i32) {
        if fb > 0 {
            let fbc = if fbc < 0 || fbc as usize >= self.operator_count { 0 } else { fbc as usize };
            let cell = self.operators[fbc].feed_cell();
            self.in_src = Source::Feed(cell);
            self.cells[cell] = 0;
            self.input_level = fb + 6;
            self.input_mode = INPUT_FEEDBACK;
        } else {
            self.in_src = Source::Zero;
            self.input_level = 0;
            self.input_mode = INPUT_ZERO;
        }
    }

    /// `setChannelNumber`: the channel of the chip that register writes are for.
    pub fn set_channel_number(&mut self, n: i32) {
        self.register_channel = n;
    }

    /// `setRegister`: a write to the YM2151's register map (`_setByOPMRegister`).
    /// SiON's MML has no command for it; a program reaches it through the
    /// track's `onUpdateRegister`.
    pub fn set_register(&mut self, addr: i32, data: i32) {
        if self.kind != Kind::Fm {
            return;
        }
        let channel = self.register_channel;
        if addr < 0x20 {
            match addr {
                15 => {
                    // NOISE:7 FREQ:4-0 for channel 7
                    if channel == 7 && self.operator_count == 4 && (data & 128) != 0 {
                        self.operators[3].set_pg_type(PG_NOISE_PULSE as i32);
                        self.operators[3].set_pt_type(PT_OPM_NOISE);
                        self.operators[3].set_pitch_index(((data & 31) << 6) + 2048);
                    }
                }
                24 => {
                    // LFO FREQ for all channels
                    let v = self.table.lfo_timer_steps.get((data & 255) as usize).copied().unwrap_or(0);
                    self.lfo_timer = i32::from(v > 0);
                    self.lfo_timer_step_ = v;
                    self.lfo_timer_step = v;
                }
                25 => {
                    // A(0)/P(1):7 DEPTH:6-0
                    if data & 128 != 0 {
                        self.reg_amd = data & 127;
                    } else {
                        self.reg_pmd = data & 127;
                    }
                }
                27 => self.initialize_lfo(data & 3, None),
                _ => {}
            }
            return;
        }
        if channel != (addr & 7) {
            return;
        }
        if addr < 0x40 {
            match (addr - 0x20) >> 3 {
                0 => {
                    // L:7 R:6 FB:5-3 ALG:2-0
                    let v = data >> 6;
                    self.set_algorism(4, data & 7);
                    self.set_feedback((data >> 3) & 7, 0);
                    self.volumes[0] = if v != 0 { 0.5 } else { 0.0 };
                    self.pan = match v {
                        1 => 128,
                        2 => 0,
                        _ => 64,
                    };
                }
                1 => {
                    for o in self.operators.iter_mut() {
                        o.set_kc(data & 127);
                    }
                }
                2 => {
                    for o in self.operators.iter_mut() {
                        o.set_kf(data & 127);
                    }
                }
                3 => {
                    // PMS:6-4 AMS:1-0
                    let pms = (data >> 4) & 7;
                    let ams = data & 3;
                    if data & 128 != 0 {
                        let d = if pms < 6 { self.reg_pmd >> (6 - pms) } else { self.reg_pmd << (pms - 5) };
                        self.set_pitch_modulation(d);
                    } else {
                        let d = if ams > 0 { self.reg_amd << (ams - 1) } else { 0 };
                        self.set_amplitude_modulation(d);
                    }
                }
                _ => {}
            }
        } else {
            let o = &mut self.operators[[0usize, 2, 1, 3][((addr >> 3) & 3) as usize]];
            match (addr - 0x40) >> 5 {
                0 => {
                    o.set_dt1((data >> 4) & 7);
                    o.set_mul(data & 15);
                }
                1 => o.set_tl(data & 127),
                2 => {
                    o.set_ks((data >> 6) & 3);
                    o.set_ar((data & 31) << 1);
                }
                3 => {
                    o.set_ams(((data >> 7) & 1) << 1);
                    o.set_dr((data & 31) << 1);
                }
                4 => {
                    o.set_detune([0, 384, 500, 608][((data >> 6) & 3) as usize]);
                    o.set_sr((data & 31) << 1);
                }
                5 => {
                    o.set_sl((data >> 4) & 15);
                    o.set_rr((data & 15) << 2);
                }
                _ => {}
            }
        }
    }

    /// Makes this channel the PCM module, the sampler or an FM one again.
    pub fn set_kind(&mut self, k: Kind) {
        if k != self.kind {
            self.kind = k;
        }
    }

    pub fn kind(&self) -> Kind {
        self.kind
    }

    /// `setWaveData` of the PCM module: a table of waves, tuned by the one at key 60.
    fn set_pcm_table(&mut self, table: Arc<PcmTable>) {
        let first = table.table.get(60).cloned().flatten();
        self.pcm.table = Some(table);
        if let Some(d) = &first {
            self.pcm.pitch_shift = d.sampling_pitch - 4416;
        }
        self.set_pcm_data_of(first);
    }

    fn set_pcm_data_of(&mut self, d: Option<Arc<super::pcm::PcmData>>) {
        let w = d.map(|d| (d.wavelet.clone(), d.channel_count, d.start_point, d.end_point, d.loop_point));
        self.operators[0].set_pcm_data(w);
    }

    /// `setType` with the song's waves (`%7@n` takes the PCM table n, `%10@n` a bank).
    pub fn set_type_with(&mut self, pg_type: i32, pt_type: usize, samples: &SampleBank) {
        match self.kind {
            Kind::Pcm => {
                if let Some(t) = samples.pcm_table(pg_type) {
                    self.set_pcm_table(t);
                } else {
                    self.pcm.pitch_shift = 0;
                    self.operators[0].set_pcm_data(None);
                }
            }
            Kind::Sampler => {
                // The bank number is kept by SiON and not used: the first table plays.
                self.sampler.table = samples.sampler_table(0);
            }
            Kind::Fm => self.set_type(pg_type, pt_type),
        }
    }

    pub fn set_type(&mut self, pg_type: i32, pt_type: usize) {
        if let Some(ks) = self.ks.as_mut() {
            ks.seed_type = pg_type;
            ks.seed_index = 0;
            let _ = pt_type;
            return;
        }
        let ope = self.active_operator;
        self.operators[ope].set_pg_type(pg_type);
        self.operators[ope].set_pt_type(pt_type);
    }

    pub fn set_all_attack_rate(&mut self, ar: i32) {
        if self.ks.is_some() {
            let o = &mut self.operators[0];
            o.set_ar(ar);
            o.set_dr(if ar > 48 { 48 } else { ar });
            o.set_tl(if ar > 48 { 0 } else { 48 - ar });
            return;
        }
        for o in self.operators.iter_mut().take(self.operator_count) {
            if o.final_ {
                o.set_ar(ar);
            }
        }
    }

    pub fn set_all_release_rate(&mut self, rr: i32) {
        if let Some(ks) = self.ks.as_mut() {
            ks.decay_lpf = 1.0 - f64::from(rr) * 0.015625;
            return;
        }
        for o in self.operators.iter_mut().take(self.operator_count) {
            if o.final_ {
                o.set_rr(rr);
            }
        }
    }

    pub fn pitch(&self) -> i32 {
        match self.kind {
            Kind::Sampler => return self.sampler.wave_number << 6,
            Kind::Pcm => return self.operators[0].pitch_index() + self.pcm.pitch_shift,
            Kind::Fm => {}
        }
        if let Some(ks) = self.ks.as_ref() {
            return ks.pitch_index;
        }
        self.operators[self.operator_count - 1].pitch_index()
    }

    pub fn set_pitch(&mut self, p: i32) {
        match self.kind {
            Kind::Sampler => {
                self.sampler.wave_number = p >> 6;
                return;
            }
            Kind::Pcm => {
                if let Some(t) = self.pcm.table.clone() {
                    let note = (p >> 6) as usize;
                    let d = t.table.get(note).cloned().flatten();
                    if let Some(d) = &d {
                        self.pcm.pitch_shift = d.sampling_pitch - 4416;
                        self.pcm.volume = t.volume[note];
                        self.pcm.pan = t.pan[note];
                    }
                    self.set_pcm_data_of(d);
                }
                let shift = self.pcm.pitch_shift;
                self.operators[0].set_pitch_index(p - shift);
                return;
            }
            Kind::Fm => {}
        }
        if let Some(ks) = self.ks.as_mut() {
            ks.pitch_index = p;
            return;
        }
        for o in self.operators.iter_mut().take(self.operator_count) {
            o.set_pitch_index(p);
        }
    }

    pub fn set_active_operator_index(&mut self, i: i32) {
        self.active_operator = i.clamp(0, self.operator_count as i32 - 1) as usize;
    }

    pub fn set_erst(&mut self, b: bool) {
        for o in self.operators.iter_mut().take(self.operator_count) {
            o.set_erst(b);
        }
    }

    pub fn set_active_rr(&mut self, i: i32) {
        if let Some(ks) = self.ks.as_mut() {
            ks.decay_lpf = 1.0 - f64::from(i) * 0.015625;
            return;
        }
        self.active().set_rr(i);
    }

    pub fn set_active_tl(&mut self, i: i32) {
        self.active().set_tl(i);
    }

    pub fn set_active_fmul(&mut self, i: i32) {
        self.active().set_fmul(i);
    }

    pub fn set_active_phase(&mut self, i: i32) {
        if self.kind == Kind::Sampler {
            self.sampler.start_phase = i;
            return;
        }
        self.active().set_key_on_phase(i);
    }

    pub fn set_active_detune(&mut self, i: i32) {
        self.active().set_detune(i);
    }

    pub fn set_active_fixed_pitch(&mut self, i: i32) {
        if self.ks.is_some() {
            // SiON's loop shadows its own argument: each operator gets its own number.
            for k in 0..self.operator_count {
                self.operators[k].set_fixed_pitch_index(k as i32);
            }
            return;
        }
        self.active().set_fixed_pitch_index(i);
    }

    pub fn set_active_ssgec(&mut self, i: i32) {
        self.active().set_ssgec(i);
    }

    /// `offsetVolume`: the expression and velocity as a level offset of the final carriers.
    pub fn offset_volume(&mut self, expression: i32, velocity: i32) {
        if self.kind == Kind::Sampler {
            self.sampler.expression = f64::from(expression) * f64::from(velocity) * 0.00006103515625;
            return;
        }
        let (expression, velocity) = if let Some(ks) = self.ks.as_mut() {
            ks.expression = f64::from(expression) * 0.0078125;
            (128, velocity)
        } else {
            (expression, velocity)
        };
        let x = (expression << 1).clamp(0, 512) as usize;
        let v = velocity.clamp(0, 512) as usize;
        let tl = self.table.eg_tl_tables[self.expression_table][x] + self.table.eg_tl_tables[self.velocity_table][v];
        for o in self.operators.iter_mut().take(self.operator_count) {
            if o.final_ {
                o.tl_offset(tl);
            } else {
                o.tl_offset(0);
            }
        }
    }

    // operation

    pub fn initialize(&mut self, prev: Option<&FmChannel>, buffer_index: usize, module: &Module) {
        self.init_operator_param = module.init_operator_param.clone();
        if let Some(ks) = self.ks.as_mut() {
            ks.delay_buffer_index = 0.0;
            ks.pitch_index = 0;
            ks.decay_lpf = 0.875;
            ks.decay = 0.98;
            ks.mute_decay_lpf = 0.5;
            ks.mute_decay = 0.75;
            ks.output = 0.0;
            ks.cur_decay_lpf = ks.mute_decay_lpf;
            ks.cur_decay = ks.mute_decay;
            ks.expression = 1.0;
        }
        self.update_operator_count(1);
        let init = self.init_operator_param.clone();
        self.operators[0].initialize(&init);
        self.is_note_on = false;
        // the base
        if let Some(p) = prev {
            self.volumes = p.volumes;
            self.pan = p.pan;
            self.has_effect_send = p.has_effect_send;
            self.mute = p.mute;
            self.velocity_table = p.velocity_table;
            self.expression_table = p.expression_table;
        } else {
            self.volumes = [0.0; STREAM_SEND_SIZE];
            self.volumes[0] = 0.5;
            self.pan = 64;
            self.has_effect_send = false;
            self.mute = false;
            self.velocity_table = VM_LINEAR;
            self.expression_table = VM_LINEAR;
        }
        self.is_note_on = false;
        self.is_idling = true;
        self.buffer_index = buffer_index;
        self.initialize_lfo(LFO_WAVE_TRIANGLE as i32, None);
        self.set_lfo_cycle_time(333.0);
        self.set_frequency_ratio(100);
        self.set_input(0, 0);
        self.set_ring_modulation(0, 0);
        self.set_output(OUTPUT_STANDARD, 0);
        self.filter_variables = [0.0; 3];
        self.cutoff_offset = 0;
        self.filter_type = FILTER_LP;
        self.set_sv_filter_default();
        self.shift_sv_filter_state(FEG_OFF);
        self.reg_pmd = 0;
        self.reg_amd = 0;
        match self.kind {
            Kind::Pcm => {
                self.proc_type = Proc::Pcm;
                self.pcm.pitch_shift = 0;
                self.pcm.volume = 1.0;
                self.pcm.pan = 0;
                self.pcm.table = None;
                self.filter_variables2 = [0.0; 3];
            }
            Kind::Sampler => {
                self.sampler = SamplerState { table: None, data: None, wave_number: -1, expression: 1.0, index: 0, start_phase: 0, sample_pan: 0 };
            }
            Kind::Fm => {}
        }
        if self.ks.is_some() {
            if let Some(ks) = self.ks.as_mut() {
                ks.seed_type = 0;
                ks.seed_index = 0;
            }
            self.set_parameters14(&[48, 48, 0, 63, 15, 0, 0, 0, 1, 0, 0, 0, -1, 0]);
            let ope = self.active_operator;
            self.operators[ope].set_pg_type(PG_NOISE_PINK as i32);
            self.operators[ope].set_pt_type(PT_PCM);
        }
    }

    /// `initialize(this, bufferIndex)`: back to the start, keeping the volumes.
    pub fn reinitialize(&mut self, buffer_index: usize, module: &Module) {
        let (volumes, pan, send, mute, vt, xt) = (self.volumes, self.pan, self.has_effect_send, self.mute, self.velocity_table, self.expression_table);
        self.initialize(None, buffer_index, module);
        self.volumes = volumes;
        self.pan = pan;
        self.has_effect_send = send;
        self.mute = mute;
        self.velocity_table = vt;
        self.expression_table = xt;
    }

    pub fn rate(&self) -> i32 {
        self.table.rate
    }

    pub fn is_filter_active(&self) -> bool {
        self.filter_on
    }

    pub fn activate_filter(&mut self, b: bool) {
        self.filter_on = b;
    }

    pub fn reset(&mut self) {
        if self.kind == Kind::Sampler {
            self.is_note_on = false;
            self.is_idling = true;
            self.sampler = SamplerState { table: self.sampler.table.clone(), data: None, wave_number: -1, expression: 1.0, index: 0, start_phase: 0, sample_pan: 0 };
            return;
        }
        if let Some(ks) = self.ks.as_mut() {
            ks.delay_buffer.fill(0);
        }
        for o in self.operators.iter_mut().take(self.operator_count) {
            o.reset();
        }
        self.is_note_on = false;
        self.is_idling = true;
    }

    pub fn note_on(&mut self) {
        if self.kind == Kind::Sampler {
            let s = &mut self.sampler;
            if s.wave_number >= 0 {
                if let Some(t) = &s.table {
                    s.data = t.get_sample(s.wave_number & 127);
                }
                if let Some(d) = s.data.clone() {
                    if s.start_phase != 255 {
                        s.index = d.initial_sample_index(f64::from(s.start_phase) * 0.00390625);
                        s.sample_pan = (self.pan + d.pan).clamp(0, 128);
                    }
                }
                self.is_idling = s.data.is_none();
                self.is_note_on = !self.is_idling;
            }
            return;
        }
        if let Some(ks) = self.ks.as_mut() {
            ks.output = 0.0;
            for v in ks.delay_buffer.iter_mut() {
                *v = (f64::from(*v) * 0.3) as i32;
            }
            ks.cur_decay_lpf = ks.decay_lpf;
            ks.cur_decay = ks.decay;
        }
        for i in 0..self.operator_count {
            let mut rng = self.rng;
            self.operators[i].note_on(&mut rng);
            self.rng = rng;
        }
        self.is_note_on = true;
        self.is_idling = false;
        self.lfo_phase = 0;
        if self.filter_on {
            self.reset_sv_filter_state();
            self.shift_sv_filter_state(FEG_ATTACK);
        }
    }

    pub fn note_off(&mut self) {
        if self.kind == Kind::Sampler {
            if self.sampler.data.as_ref().is_some_and(|d| !d.ignore_note_off) {
                self.is_note_on = false;
                self.is_idling = true;
                if self.sampler.table.is_some() {
                    self.sampler.data = None;
                }
            }
            return;
        }
        if let Some(ks) = self.ks.as_mut() {
            ks.cur_decay_lpf = ks.mute_decay_lpf;
            ks.cur_decay = ks.mute_decay;
            return;
        }
        for o in self.operators.iter_mut().take(self.operator_count) {
            o.note_off();
        }
        self.is_note_on = false;
        if self.filter_on {
            self.shift_sv_filter_state(FEG_RELEASE);
        }
    }

    /// `resetChannelBufferStatus`: a new block begins.
    pub fn reset_channel_buffer_status(&mut self) {
        if self.kind == Kind::Sampler {
            self.buffer_index = 0;
            return;
        }
        if self.ks.is_some() {
            self.buffer_index = 0;
            self.is_idling = false;
            return;
        }
        self.buffer_index = 0;
        self.is_idling = true;
        for o in self.operators.iter().take(self.operator_count) {
            if o.final_ && (o.eg_out < IDLING_THRESHOLD || o.eg_state == EG_ATTACK) {
                self.is_idling = false;
                break;
            }
        }
    }

    fn update_operator_count(&mut self, cnt: usize) {
        if self.operator_count < cnt {
            let init = self.init_operator_param.clone();
            for i in self.operator_count..cnt {
                self.operators[i].initialize(&init);
            }
        }
        self.operator_count = cnt;
        self.proc_type = match cnt {
            1 => Proc::Op1,
            2 => Proc::Op2,
            3 => Proc::Op3,
            _ => Proc::Op4,
        };
        self.active_operator = cnt - 1;
        if self.input_mode == INPUT_FEEDBACK {
            self.set_feedback(0, 0);
        }
    }

    fn algorism1(&mut self, alg: i32) {
        self.update_operator_count(1);
        self.algorism = alg;
        self.operators[0].set_pipes(P0, None, true);
    }

    fn algorism2(&mut self, alg: i32) {
        self.update_operator_count(2);
        self.algorism = alg;
        let o = &mut self.operators;
        match alg {
            0 => {
                o[0].set_pipes(P0, None, false);
                o[1].set_pipes(P0, Some(P0), true);
            }
            2 => {
                o[0].set_pipes(P0, None, true);
                o[1].set_pipes(P0, Some(P0), true);
                o[1].set_base_cell(P0);
            }
            _ => {
                o[0].set_pipes(P0, None, true);
                o[1].set_pipes(P0, None, true);
            }
        }
    }

    fn algorism3(&mut self, alg: i32) {
        self.update_operator_count(3);
        self.algorism = alg;
        let o = &mut self.operators;
        match alg {
            0 => {
                o[0].set_pipes(P0, None, false);
                o[1].set_pipes(P0, Some(P0), false);
                o[2].set_pipes(P0, Some(P0), true);
            }
            1 => {
                o[0].set_pipes(P0, None, false);
                o[1].set_pipes(P0, None, false);
                o[2].set_pipes(P0, Some(P0), true);
            }
            2 => {
                o[0].set_pipes(P0, None, true);
                o[1].set_pipes(P1, None, false);
                o[2].set_pipes(P0, Some(P1), true);
            }
            3 => {
                o[0].set_pipes(P0, None, false);
                o[1].set_pipes(P0, Some(P0), true);
                o[2].set_pipes(P0, None, true);
            }
            4 => {
                o[0].set_pipes(P1, None, false);
                o[1].set_pipes(P0, Some(P1), true);
                o[2].set_pipes(P0, Some(P1), true);
            }
            6 => {
                o[0].set_pipes(P0, None, false);
                o[1].set_pipes(P0, Some(P0), true);
                o[1].set_base_cell(P0);
                o[2].set_pipes(P0, None, true);
            }
            _ => {
                o[0].set_pipes(P0, None, true);
                o[1].set_pipes(P0, None, true);
                o[2].set_pipes(P0, None, true);
            }
        }
    }

    fn algorism4(&mut self, alg: i32) {
        self.update_operator_count(4);
        self.algorism = alg;
        let o = &mut self.operators;
        // (out, modulation, final) for each operator
        let set = |o: &mut [Operator; 4], cfg: [(usize, Option<usize>, bool); 4]| {
            for (i, (out, m, f)) in cfg.into_iter().enumerate() {
                o[i].set_pipes(out, m, f);
            }
        };
        match alg {
            0 => set(o, [(P0, None, false), (P0, Some(P0), false), (P0, Some(P0), false), (P0, Some(P0), true)]),
            1 => set(o, [(P0, None, false), (P0, None, false), (P0, Some(P0), false), (P0, Some(P0), true)]),
            2 => set(o, [(P0, None, false), (P1, None, false), (P0, Some(P1), false), (P0, Some(P0), true)]),
            3 => set(o, [(P0, None, false), (P0, Some(P0), false), (P0, None, false), (P0, Some(P0), true)]),
            4 => set(o, [(P0, None, false), (P0, Some(P0), true), (P1, None, false), (P0, Some(P1), true)]),
            5 => set(o, [(P1, None, false), (P0, Some(P1), true), (P0, Some(P1), true), (P0, Some(P1), true)]),
            6 => set(o, [(P0, None, false), (P0, Some(P0), true), (P0, None, true), (P0, None, true)]),
            7 => set(o, [(P0, None, true), (P0, None, true), (P0, None, true), (P0, None, true)]),
            8 => set(o, [(P0, None, true), (P1, None, false), (P1, Some(P1), false), (P0, Some(P1), true)]),
            9 => set(o, [(P0, None, true), (P1, None, false), (P0, Some(P1), true), (P0, None, true)]),
            10 => set(o, [(P0, None, false), (P0, None, false), (P0, None, false), (P0, Some(P0), true)]),
            11 => set(o, [(P0, None, true), (P1, None, false), (P1, None, false), (P0, Some(P1), true)]),
            12 => {
                set(o, [(P0, None, false), (P0, Some(P0), true), (P1, None, false), (P0, Some(P1), true)]);
                o[1].set_base_cell(P0);
            }
            _ => set(o, [(P0, None, true), (P0, None, true), (P0, None, true), (P0, None, true)]),
        }
    }

    fn analog(&mut self, alg: i32) {
        self.update_operator_count(2);
        self.operators[0].set_pipes(P0, None, true);
        self.operators[1].set_pipes(P0, None, true);
        self.algorism = if (0..=3).contains(&alg) { alg } else { 0 };
        self.proc_type = match self.algorism {
            0 => Proc::Ana,
            1 => Proc::Ring,
            2 => Proc::Sync,
            _ => Proc::Afm,
        };
    }

    // processing

    /// `buffer`: `len` samples into the module's streams.
    pub fn buffer(&mut self, len: usize, module: &mut Module) {
        if self.kind == Kind::Sampler {
            self.buffer_sampler(len, module);
            return;
        }
        if self.kind == Kind::Pcm && !self.is_idling {
            let start = self.buffer_index;
            self.proc_pcm(len, module);
            self.write_pcm(module, start, len);
            self.buffer_index += len;
            return;
        }
        if self.is_idling {
            self.nop(len);
            return;
        }
        let start = self.buffer_index;
        self.process(len, module);
        if let Some(r) = self.ring_pipe {
            self.apply_ring_modulation(module, start, len, r);
        }
        let gain = self.ks.as_ref().map_or(1.0, |k| k.expression);
        if self.ks.is_some() {
            self.apply_karplus_strong(module, start, len);
        }
        if self.filter_on {
            self.apply_sv_filter(module, start, len);
        }
        if self.output_mode == OUTPUT_STANDARD && !self.mute {
            let pipe = self.out_pipe;
            if self.has_effect_send {
                for i in 0..STREAM_SEND_SIZE {
                    if self.volumes[i] > 0.0 {
                        module.write_stream(i, pipe, start, len, self.volumes[i] * gain, self.pan);
                    }
                }
            } else {
                module.write_stream(0, pipe, start, len, self.volumes[0] * gain, self.pan);
            }
        }
        self.buffer_index += len;
    }

    /// `nop`: no process, the buffer moves on.
    pub fn nop(&mut self, len: usize) {
        self.buffer_index += len;
    }

    /// `SiOPMChannelPCM._proc`: the operator reads its wave.
    fn proc_pcm(&mut self, len: usize, module: &mut Module) {
        let mask = module.buffer_length - 1;
        let table = self.table.clone();
        let stereo = self.operators[0].pcm_channels != 1;
        let pipe2 = self.pcm.pipe2;
        let out = self.out_pipe;
        for k in 0..len {
            let pos = (self.buffer_index + k) & mask;
            if self.operators[0].pcm_end_point <= 0 {
                // no operation
                module.pipes[out][pos] = 0;
                if stereo {
                    module.pipes[pipe2][pos] = 0;
                }
                continue;
            }
            if self.lfo_on {
                self.lfo_update();
            }
            let (ei, am_out) = (self.eg_timer_initial, self.am_out);
            let ope = &mut self.operators[0];
            ope.eg_update(ei);
            ope.phase = ope.phase.wrapping_add(ope.phase_step);
            let mut t = ((ope.phase as u32) >> PCM_WAVE_FIXED_BITS) as i32;
            if t >= ope.pcm_end_point {
                if ope.pcm_loop_point == -1 {
                    ope.eg_shift_state(EG_OFF);
                    ope.update_eg_out();
                    for j in k..len {
                        let p = (self.buffer_index + j) & mask;
                        module.pipes[out][p] = 0;
                        if stereo {
                            module.pipes[pipe2][p] = 0;
                        }
                    }
                    break;
                }
                t -= ope.pcm_end_point - ope.pcm_loop_point;
                ope.phase = ope.phase.wrapping_sub((ope.pcm_end_point - ope.pcm_loop_point) << PCM_WAVE_FIXED_BITS);
            }
            let ams = ope.ams;
            let eg = ope.eg_out;
            let lfo = self.lfo_on;
            let wave = ope.pcm_wave.clone();
            let w = |i: i32| -> i32 { wave.as_ref().and_then(|w| w.get(i as usize).copied()).unwrap_or(LOG_TABLE_BOTTOM) };
            let log = |l: i32| -> i32 { table.log_table.get(l.max(0) as usize).copied().unwrap_or(0) };
            let am = if lfo { am_out >> ams } else { 0 };
            if stereo {
                let t2 = t << 1;
                module.pipes[out][pos] = log(w(t2) + eg + am);
                module.pipes[pipe2][pos] = log(w(t2 + 1) + eg + am);
            } else {
                module.pipes[out][pos] = log(w(t) + eg + am);
            }
        }
    }

    /// `_mwrite` and `_swrite`: the filter, then the streams.
    fn write_pcm(&mut self, module: &mut Module, start: usize, len: usize) {
        let stereo = self.operators[0].pcm_channels != 1;
        let vol = self.pcm.volume * module.pcm_volume;
        let pan = (self.pan + self.pcm.pan).clamp(0, 128);
        if self.mute {
            return;
        }
        if self.filter_on {
            self.apply_sv_filter(module, start, len);
            if stereo {
                // The right side runs its own variables (the envelope of the filter steps twice, as in SiON).
                let (out, vars) = (self.out_pipe, self.filter_variables);
                self.out_pipe = self.pcm.pipe2;
                self.filter_variables = self.filter_variables2;
                self.apply_sv_filter(module, start, len);
                self.filter_variables2 = self.filter_variables;
                self.filter_variables = vars;
                self.out_pipe = out;
            }
        }
        let pipe = self.out_pipe;
        let slots: Vec<(usize, f64)> = if self.has_effect_send {
            (0..STREAM_SEND_SIZE).filter(|&i| self.volumes[i] > 0.0).map(|i| (i, self.volumes[i])).collect()
        } else {
            vec![(0, self.volumes[0])]
        };
        for (slot, v) in slots {
            if stereo {
                module.write_stereo(slot, pipe, self.pcm.pipe2, start, len, v * vol, pan);
            } else {
                module.write_stream(slot, pipe, start, len, v * vol, pan);
            }
        }
    }

    /// `SiOPMChannelSampler.buffer`: a sample's numbers into the streams.
    fn buffer_sampler(&mut self, len: usize, module: &mut Module) {
        let data = self.sampler.data.clone();
        if let (false, Some(d), false) = (self.is_idling, data, self.mute) {
            let ch = d.channel_count;
            let mut residue = len as i32;
            while residue > 0 {
                let idx = self.sampler.index;
                let processed = if idx + residue < d.end_point { residue } else { d.end_point - idx };
                let processed = processed.max(0);
                let gain = self.sampler.expression * module.sampler_volume;
                if self.has_effect_send {
                    for i in 0..STREAM_SEND_SIZE {
                        if self.volumes[i] > 0.0 {
                            module.write_vector(i, &d.wave, idx.max(0) as usize, self.buffer_index, processed as usize, self.volumes[i] * gain, self.sampler.sample_pan, ch);
                        }
                    }
                } else {
                    module.write_vector(0, &d.wave, idx.max(0) as usize, self.buffer_index, processed as usize, self.volumes[0] * gain, self.sampler.sample_pan, ch);
                }
                self.sampler.index += processed;
                residue -= processed;
                if residue > 0 {
                    if d.loop_point >= 0 {
                        self.sampler.index = if d.loop_point > d.start_point { d.loop_point } else { d.start_point };
                    } else {
                        self.is_idling = true;
                        if self.sampler.table.is_some() {
                            self.sampler.data = None;
                        }
                        break;
                    }
                }
            }
        }
        self.buffer_index += len;
    }

    fn apply_ring_modulation(&mut self, module: &mut Module, start: usize, len: usize, ring: usize) {
        let mask = module.buffer_length - 1;
        for k in 0..len {
            let i = (start + k) & mask;
            let r = f64::from(module.pipes[ring][i]);
            let o = f64::from(module.pipes[self.out_pipe][i]);
            module.pipes[self.out_pipe][i] = to_i32(o * (r * self.ringmod_level));
        }
    }

    fn apply_sv_filter(&mut self, module: &mut Module, start: usize, len: usize) {
        let mask = module.buffer_length - 1;
        let pipe = self.out_pipe;
        let mut len = len as i32;
        let mut pos = start;
        let table = self.table.clone();
        let cut_of = |me: &FmChannel| -> f64 { table.filter_cutoff_table[(me.cutoff + me.cutoff_offset).clamp(0, 128) as usize] };
        let mut cut = cut_of(self);
        let fb = self.resonance;
        let mut step = self.prev_step_remain;
        let mut v = self.filter_variables;
        while step > 0 && len >= step {
            for _ in 0..step {
                let p = &mut module.pipes[pipe][pos & mask];
                v[2] = f64::from(*p) - v[0] - v[1] * fb;
                v[1] += v[2] * cut;
                v[0] += v[1] * cut;
                *p = to_i32(v[self.filter_type as usize]);
                pos += 1;
            }
            len -= step;
            self.cutoff += self.filter_eg_cutoff_inc;
            cut = cut_of(self);
            if self.cutoff == self.filter_eg_next {
                self.shift_sv_filter_state(self.filter_eg_state + 1);
            }
            step = self.filter_eg_step;
        }
        for _ in 0..len {
            let p = &mut module.pipes[pipe][pos & mask];
            v[2] = f64::from(*p) - v[0] - v[1] * fb;
            v[1] += v[2] * cut;
            v[0] += v[1] * cut;
            *p = to_i32(v[self.filter_type as usize]);
            pos += 1;
        }
        self.filter_variables = v;
        self.prev_step_remain = self.filter_eg_step.wrapping_sub(len);
    }

    /// Makes this channel a Karplus-Strong one (or an FM one again).
    pub fn set_ks_mode(&mut self, on: bool) {
        if on != self.ks.is_some() {
            self.ks = if on { Some(Box::new(KsState::new())) } else { None };
        }
    }

    pub fn is_ks(&self) -> bool {
        self.ks.is_some()
    }

    pub fn ks_seed(&self) -> (i32, i32) {
        self.ks.as_ref().map_or((0, 0), |k| (k.seed_type, k.seed_index))
    }

    /// `_applyKarplusStrong`: a delay line the length of the note's cycle, fed back with loss.
    fn apply_karplus_strong(&mut self, module: &mut Module, start: usize, len: usize) {
        let Some(mut ks) = self.ks.take() else { return };
        let mask = module.buffer_length - 1;
        let tmax = PITCH_TABLE_SIZE as i32 - 1;
        let index_of = |me: &FmChannel, ks: &KsState| -> f64 {
            let t = (ks.pitch_index + me.operators[0].detune() + me.pm_out).clamp(0, tmax);
            me.table.pitch_wave_length[t as usize]
        };
        let mut index_max = index_of(self, &ks);
        for k in 0..len {
            self.lfo_timer -= self.lfo_timer_step;
            if self.lfo_timer < 0 {
                self.lfo_phase = (self.lfo_phase + 1) & 255;
                let t = self.lfo_wave[self.lfo_phase as usize];
                self.pm_out = (((t << 1) - 255) * self.pm_depth) >> 8;
                index_max = index_of(self, &ks);
                self.lfo_timer += self.lfo_timer_initial;
            }
            ks.delay_buffer_index += 1.0;
            if ks.delay_buffer_index >= index_max {
                ks.delay_buffer_index %= index_max;
            }
            ks.output *= ks.cur_decay;
            let t = (ks.delay_buffer_index as usize).min(KS_BUFFER_SIZE - 1);
            let p = &mut module.pipes[self.out_pipe][(start + k) & mask];
            ks.output += (f64::from(ks.delay_buffer[t]) - ks.output) * ks.cur_decay_lpf + f64::from(*p);
            ks.delay_buffer[t] = to_i32(ks.output);
            *p = to_i32(ks.output);
        }
        self.ks = Some(ks);
    }

    fn lfo_update(&mut self) {
        self.lfo_timer -= self.lfo_timer_step;
        if self.lfo_timer < 0 {
            self.lfo_phase = (self.lfo_phase + 1) & 255;
            let t = self.lfo_wave[self.lfo_phase as usize];
            self.am_out = ((t * self.am_depth) >> 7) << 3;
            self.pm_out = (((t << 1) - 255) * self.pm_depth) >> 8;
            let pm = self.pm_out;
            for o in self.operators.iter_mut().take(self.operator_count) {
                o.set_detune2(pm);
            }
            self.lfo_timer += self.lfo_timer_initial;
        }
    }

    fn input_value(&self, module: &Module, pos: usize) -> i32 {
        match self.in_src {
            Source::Zero => 0,
            Source::Feed(c) => self.cells[c],
            Source::Pipe(p) => module.pipes[p][pos & (module.buffer_length - 1)],
        }
    }

    fn process(&mut self, len: usize, module: &mut Module) {
        match self.proc_type {
            Proc::Op1 => self.proc1op(len, module),
            Proc::Op2 | Proc::Op3 | Proc::Op4 | Proc::Afm => self.proc_nop(len, module),
            Proc::Ana => self.proc2ana(len, module, 0),
            Proc::Ring => self.proc2ana(len, module, 1),
            Proc::Sync => self.proc2ana(len, module, 2),
            Proc::Pcm => self.proc_pcm(len, module),
        }
    }

    /// The pipe a sample goes to, and the base to add.
    #[inline]
    fn base_value(&self, module: &Module, pos: usize) -> i32 {
        if self.base_add {
            module.pipes[self.out_pipe][pos & (module.buffer_length - 1)]
        } else {
            0
        }
    }

    fn proc1op(&mut self, len: usize, module: &mut Module) {
        let mask = module.buffer_length - 1;
        let table = self.table.clone();
        for k in 0..len {
            let pos = (self.buffer_index + k) & mask;
            if self.lfo_on {
                self.lfo_update();
            }
            let inv = self.input_value(module, pos);
            let base = self.base_value(module, pos);
            let (ei, am_out, il) = (self.eg_timer_initial, self.am_out, self.input_level);
            let lfo = self.lfo_on;
            let ope = &mut self.operators[0];
            ope.eg_update(ei);
            ope.phase = ope.phase.wrapping_add(ope.phase_step);
            let mut l = ope.sample(ope.phase.wrapping_add(inv << il));
            l += ope.eg_out;
            if lfo {
                l += am_out >> ope.ams;
            }
            let t = table.log_table.get(l.max(0) as usize).copied().unwrap_or(0);
            self.cells[ope.feed_cell] = t;
            module.pipes[self.out_pipe][pos] = t.wrapping_add(base);
        }
    }

    /// Two to four operators in a chain of cells.
    fn proc_nop(&mut self, len: usize, module: &mut Module) {
        let mask = module.buffer_length - 1;
        let table = self.table.clone();
        let n = self.operator_count;
        for k in 0..len {
            let pos = (self.buffer_index + k) & mask;
            self.cells[P0] = 0;
            self.cells[P1] = 0;
            self.lfo_update();
            let ei = self.eg_timer_initial;
            for j in 0..n {
                let inv = if j == 0 { self.input_value(module, pos) << self.input_level } else { self.cells[self.operators[j].in_cell] << self.operators[j].fm_shift };
                let base = self.cells[self.operators[j].base_cell];
                let am_out = self.am_out;
                let ope = &mut self.operators[j];
                ope.eg_update(ei);
                ope.phase = ope.phase.wrapping_add(ope.phase_step);
                let mut l = ope.sample(ope.phase.wrapping_add(inv));
                l += ope.eg_out + (am_out >> ope.ams);
                let t = table.log_table.get(l.max(0) as usize).copied().unwrap_or(0);
                let (fc, oc) = (ope.feed_cell, ope.out_cell);
                self.cells[fc] = t;
                self.cells[oc] = t.wrapping_add(base);
            }
            let base = self.base_value(module, pos);
            module.pipes[self.out_pipe][pos] = self.cells[P0].wrapping_add(base);
        }
    }

    /// The analog-like modes: 0 two oscillators added, 1 ring modulated, 2 synced.
    fn proc2ana(&mut self, len: usize, module: &mut Module, mode: u8) {
        let mask = module.buffer_length - 1;
        let table = self.table.clone();
        for k in 0..len {
            let pos = (self.buffer_index + k) & mask;
            self.lfo_update();
            let ei = self.eg_timer_initial;
            // The envelope of operator 0 drives both.
            {
                let o0 = &mut self.operators[0];
                o0.eg_timer -= o0.eg_timer_step;
                if o0.eg_timer < 0 {
                    let inc = if o0.eg_inc_att { o0.table.eg_inc_tables_att[o0.eg_inc_sel][o0.eg_counter as usize] } else { o0.table.eg_inc_tables[o0.eg_inc_sel][o0.eg_counter as usize] };
                    if o0.eg_state == EG_ATTACK {
                        if inc > 0 {
                            o0.eg_level -= 1 + (o0.eg_level >> inc);
                            if o0.eg_level <= 0 {
                                o0.eg_shift_state(NEXT_STATE[usize::from(o0.eg_next_ssg)][o0.eg_state as usize]);
                            }
                        }
                    } else {
                        o0.eg_level += inc;
                        if o0.eg_level >= o0.eg_state_shift_level {
                            o0.eg_shift_state(NEXT_STATE[usize::from(o0.eg_next_ssg)][o0.eg_state as usize]);
                        }
                    }
                    let lv = table.eg_level_tables[o0.eg_level_table][o0.eg_level.clamp(0, 1023) as usize];
                    o0.eg_out = (lv + o0.eg_total_level) << 3;
                    let tl1 = self.operators[1].eg_total_level;
                    self.operators[1].eg_out = (lv + tl1) << 3;
                    let o0 = &mut self.operators[0];
                    o0.eg_counter = (o0.eg_counter + 1) & 7;
                    o0.eg_timer += ei;
                }
            }
            let inv = self.input_value(module, pos);
            let base = self.base_value(module, pos);
            let (am_out, il) = (self.am_out, self.input_level);
            let ams0 = self.operators[0].ams;
            let (out0, out1);
            match mode {
                0 => {
                    let o0 = &mut self.operators[0];
                    o0.phase = o0.phase.wrapping_add(o0.phase_step);
                    let mut l = o0.sample(o0.phase.wrapping_add(inv << il));
                    l += o0.eg_out + (am_out >> o0.ams);
                    out0 = table.log_table.get(l.max(0) as usize).copied().unwrap_or(0);
                    let o1 = &mut self.operators[1];
                    o1.phase = o1.phase.wrapping_add(o1.phase_step);
                    let mut l = o1.sample(o1.phase);
                    l += o1.eg_out + (am_out >> ams0);
                    out1 = table.log_table.get(l.max(0) as usize).copied().unwrap_or(0);
                    self.cells[self.operators[0].feed_cell] = out0;
                    module.pipes[self.out_pipe][pos] = out0.wrapping_add(out1).wrapping_add(base);
                }
                1 => {
                    let o0 = &mut self.operators[0];
                    o0.phase = o0.phase.wrapping_add(o0.phase_step);
                    let mut l = o0.sample(o0.phase.wrapping_add(inv << il));
                    let o1 = &mut self.operators[1];
                    o1.phase = o1.phase.wrapping_add(o1.phase_step);
                    l += o1.sample(o1.phase);
                    l += o1.eg_out + (am_out >> ams0);
                    out0 = table.log_table.get(l.max(0) as usize).copied().unwrap_or(0);
                    self.cells[self.operators[0].feed_cell] = out0;
                    module.pipes[self.out_pipe][pos] = out0.wrapping_add(base);
                }
                _ => {
                    let o0 = &mut self.operators[0];
                    o0.phase = o0.phase.wrapping_add(o0.phase_step).wrapping_add(inv << il);
                    let reset = o0.phase & PHASE_MAX != 0;
                    o0.phase &= PHASE_FILTER;
                    let kp = self.operators[1].keyon_phase;
                    let o1 = &mut self.operators[1];
                    if reset {
                        o1.phase = kp;
                    }
                    o1.phase = o1.phase.wrapping_add(o1.phase_step);
                    let mut l = o1.sample(o1.phase);
                    l += o1.eg_out + (am_out >> ams0);
                    out0 = table.log_table.get(l.max(0) as usize).copied().unwrap_or(0);
                    self.cells[self.operators[0].feed_cell] = out0;
                    module.pipes[self.out_pipe][pos] = out0.wrapping_add(base);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BLOCK: usize = 2048;

    fn setup() -> (Module, FmChannel) {
        let m = Module::new(Arc::new(Table::new(44100)), BLOCK);
        let c = FmChannel::new(&m);
        (m, c)
    }

    /// Renders `blocks` blocks of a channel and returns what it wrote to pipe 4.
    fn render(m: &mut Module, c: &mut FmChannel, blocks: usize) -> Vec<i32> {
        let mut out = vec![];
        for _ in 0..blocks {
            m.begin_process();
            c.reset_channel_buffer_status();
            let idle = c.is_idling;
            c.buffer(BLOCK, m);
            // An idle channel writes nothing: the pipe is stale.
            out.extend(if idle { vec![0; BLOCK] } else { m.pipes[4].clone() });
        }
        out
    }

    fn crossings(v: &[i32]) -> usize {
        v.windows(2).filter(|w| w[0] <= 0 && w[1] > 0).count()
    }

    #[test]
    fn a_sine_plays_in_tune_and_a_released_note_dies() {
        let (mut m, mut c) = setup();
        c.set_pitch(69 << 6);
        c.note_on();
        let out = render(&mut m, &mut c, 10);
        let hz = crossings(&out[1000..]) as f64 * 44100.0 / (out.len() - 1000) as f64;
        assert!((hz - 440.0).abs() < 3.0, "{hz}");
        assert!(out.iter().map(|x| x.abs()).max().unwrap() > 6000);
        c.note_off();
        let _ = render(&mut m, &mut c, 4);
        let tail = render(&mut m, &mut c, 1);
        assert!(tail.iter().all(|&x| x.abs() < 40), "{:?}", tail.iter().map(|x| x.abs()).max());
        // Silent and idle: the channel stops costing anything.
        m.begin_process();
        c.reset_channel_buffer_status();
        assert!(c.is_idling);
    }

    #[test]
    fn an_octave_up_doubles_the_pitch_and_the_multiple_too() {
        let (mut m, mut c) = setup();
        c.set_pitch(81 << 6);
        c.note_on();
        let a = crossings(&render(&mut m, &mut c, 6));
        let (mut m2, mut c2) = setup();
        c2.set_pitch(69 << 6);
        c2.operators[0].set_mul(2);
        c2.note_on();
        let b = crossings(&render(&mut m2, &mut c2, 6));
        assert!((a as i32 - b as i32).abs() <= 1, "{a} {b}");
        let (mut m3, mut c3) = setup();
        c3.set_pitch(69 << 6);
        c3.note_on();
        let c0 = crossings(&render(&mut m3, &mut c3, 6));
        assert!((a as f64 / c0 as f64 - 2.0).abs() < 0.05);
    }

    #[test]
    fn four_equal_operators_added_are_four_times_one() {
        let (mut m, mut c) = setup();
        c.set_pitch(60 << 6);
        c.note_on();
        let one = render(&mut m, &mut c, 2);
        let (mut m4, mut c4) = setup();
        c4.set_algorism(4, 7);
        c4.set_pitch(60 << 6);
        c4.note_on();
        let four = render(&mut m4, &mut c4, 2);
        for i in 0..one.len() {
            assert_eq!(four[i], one[i] * 4, "sample {i}");
        }
        // And two added, by their algorithm, are twice.
        let (mut m2, mut c2) = setup();
        c2.set_algorism(2, 1);
        c2.set_pitch(60 << 6);
        c2.note_on();
        let two = render(&mut m2, &mut c2, 2);
        for i in 0..one.len() {
            assert_eq!(two[i], one[i] * 2, "sample {i}");
        }
    }

    #[test]
    fn a_silent_modulator_leaves_the_carrier_and_a_loud_one_changes_it() {
        let (mut m, mut c) = setup();
        c.set_pitch(60 << 6);
        c.note_on();
        let plain = render(&mut m, &mut c, 2);
        // Algorithm 0 of two: o1(o0). The modulator (o0) at total level 127 is silent.
        let make = |tl0: i32| {
            let (mut m, mut c) = setup();
            c.set_algorism(2, 0);
            c.operators[0].set_tl(tl0);
            c.set_pitch(60 << 6);
            c.note_on();
            render(&mut m, &mut c, 2)
        };
        let quiet = make(127);
        let max_diff = plain.iter().zip(&quiet).map(|(a, b)| (a - b).abs()).max().unwrap();
        assert!(max_diff < 20, "{max_diff}");
        let loud = make(0);
        let diff: i64 = plain.iter().zip(&loud).map(|(a, b)| i64::from((a - b).abs())).sum();
        assert!(diff > 1_000_000, "{diff}");
    }

    #[test]
    fn feedback_makes_the_sine_brighter() {
        let spread = |fb: i32| {
            let (mut m, mut c) = setup();
            c.set_feedback(fb, 0);
            c.set_pitch(48 << 6);
            c.note_on();
            let out = render(&mut m, &mut c, 4);
            // The number of direction changes: a plain sine has two a cycle.
            out.windows(3).filter(|w| (w[1] - w[0]).signum() != (w[2] - w[1]).signum() && w[1] != w[0]).count()
        };
        assert!(spread(7) > spread(0) * 2, "{} {}", spread(7), spread(0));
    }

    #[test]
    fn the_envelope_attacks_decays_to_the_sustain_level_and_the_sustain_rate_falls() {
        let (mut m, mut c) = setup();
        {
            let o = &mut c.operators[0];
            o.set_ar(31);
            o.set_dr(20);
            o.set_sl(8);
            o.set_sr(0);
        }
        c.set_pitch(60 << 6);
        c.note_on();
        let peak = |v: &[i32]| v.iter().map(|x| x.abs()).max().unwrap();
        let out = render(&mut m, &mut c, 40);
        let (early, late, latest) = (peak(&out[BLOCK..BLOCK * 2]), peak(&out[BLOCK * 30..BLOCK * 31]), peak(&out[BLOCK * 39..]));
        assert!(early > late, "{early} {late}");
        // Sustain rate 0 holds the level.
        assert!((i64::from(late) - i64::from(latest)).abs() < i64::from(late) / 20, "{late} {latest}");
        assert_eq!(c.operators[0].eg_state, EG_SUSTAIN);
    }

    #[test]
    fn the_lfo_vibrato_moves_the_pitch() {
        let (mut m, mut c) = setup();
        c.set_pitch(60 << 6);
        c.set_lfo_cycle_time(100.0);
        c.set_pitch_modulation(40);
        c.note_on();
        let out = render(&mut m, &mut c, 8);
        let per_block: Vec<usize> = out.chunks(BLOCK).map(crossings).collect();
        let (lo, hi) = (per_block.iter().min().unwrap(), per_block.iter().max().unwrap());
        assert!(hi > lo, "{per_block:?}");
    }

    #[test]
    fn the_filter_takes_the_highs_off() {
        let make = |cut: i32| {
            let (mut m, mut c) = setup();
            c.operators[0].set_pg_type(PG_SAW_UP as i32);
            c.set_pitch(48 << 6);
            if cut < 128 {
                c.set_sv_filter(cut, 0, 0, 0, 0, 0, 128, 128, 128, 128);
            }
            c.note_on();
            let out = render(&mut m, &mut c, 4);
            out.windows(3).map(|w| i64::from((w[2] - 2 * w[1] + w[0]).abs())).sum::<i64>()
        };
        assert!(make(20) < make(128) / 3, "{} {}", make(20), make(128));
    }

    #[test]
    fn the_opm_register_map_sets_a_voice() {
        let (mut m, mut c) = setup();
        c.set_channel_number(2);
        // Channel 2: algorithm 7 (all carriers), centre, no feedback; key code o4a; the first operator loud.
        c.set_register(0x22, 0xc0 | 7);
        c.set_register(0x2a, 0x4a);
        for op in 0..4 {
            let a = 0x40 + op * 8 + 2;
            c.set_register(a, 0x01); // DT1 0, MUL 1
            c.set_register(a + 0x20, if op == 0 { 0 } else { 127 }); // TL
            c.set_register(a + 0x40, 31); // KS 0, AR 31
            c.set_register(a + 0x60, 0); // DR 0
            c.set_register(a + 0x80, 0); // SR 0
            c.set_register(a + 0xa0, 0x0f); // SL 0, RR 15
        }
        // Another channel's writes are ignored.
        c.set_register(0x21, 0xc0);
        assert_eq!(c.algorism(), 7);
        assert_eq!(c.operator_count(), 4);
        c.set_pitch(69 << 6);
        c.note_on();
        let out = render(&mut m, &mut c, 6);
        assert!(out.iter().any(|&x| x.abs() > 100), "silent");
    }
}
