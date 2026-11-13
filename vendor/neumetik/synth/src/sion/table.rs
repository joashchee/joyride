// Copyright 2026 ansiapps. Neumetik Synth, proprietary: see LICENSE.
//
// A port of SiON's SiOPMTable (org.si.sion.module.SiOPMTable and
// SiOPMWaveTable, github.com/keim/SiON):
//
// Copyright (c) 2008-2016 keim
//
// The MIT License (MIT)
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

//! SiOPM's tables: the envelope's rates and levels, the log table, the
//! pitch tables, the wave tables (sine, saw, triangle, square, pulse,
//! noise, ramp, MA3), the LFO waves and the filter's rates. Everything the
//! channels read, made once for a sample rate, the way `SiOPMTable` makes
//! it (its arithmetic: integers where it has them, `int()` of a big
//! number wrapping as ActionScript's does).

use std::sync::Arc;

pub const ENV_BITS: i32 = 10;
pub const ENV_TIMER_BITS: i32 = 24;
pub const SAMPLING_TABLE_BITS: i32 = 10;
pub const HALF_TONE_BITS: i32 = 6;
pub const NOTE_BITS: i32 = 7;
pub const NOISE_TABLE_BITS: i32 = 15;
pub const LOG_TABLE_RESOLUTION: i32 = 256;
pub const LOG_VOLUME_BITS: i32 = 13;
pub const LOG_TABLE_MAX_BITS: i32 = 16;
pub const FIXED_BITS: i32 = 16;
pub const PCM_BITS: i32 = 20;
pub const LFO_FIXED_BITS: i32 = 20;
pub const CLOCK_RATIO_BITS: i32 = 10;
pub const NOISE_WAVE_OUTPUT: f64 = 1.0;
pub const SQUARE_WAVE_OUTPUT: f64 = 1.0;
pub const OUTPUT_MAX: f64 = 0.5;

pub const ENV_LSHIFT: i32 = ENV_BITS - 7;
pub const ENV_TIMER_INITIAL: i32 = (2047 * 3) << CLOCK_RATIO_BITS;
pub const LFO_TIMER_INITIAL: i32 = 1 << LFO_FIXED_BITS;
pub const PHASE_BITS: i32 = SAMPLING_TABLE_BITS + FIXED_BITS;
pub const PHASE_MAX: i32 = 1 << PHASE_BITS;
pub const PHASE_FILTER: i32 = PHASE_MAX - 1;
pub const PHASE_SIGN_RSHIFT: i32 = PHASE_BITS - 1;
pub const SAMPLING_TABLE_SIZE: usize = 1 << SAMPLING_TABLE_BITS;
pub const NOISE_TABLE_SIZE: usize = 1 << NOISE_TABLE_BITS;
pub const PITCH_TABLE_SIZE: usize = 1 << (HALF_TONE_BITS + NOTE_BITS);
pub const NOTE_TABLE_SIZE: usize = 1 << NOTE_BITS;
pub const HALF_TONE_RESOLUTION: usize = 1 << HALF_TONE_BITS;
pub const LOG_TABLE_SIZE: usize = (LOG_TABLE_MAX_BITS * LOG_TABLE_RESOLUTION * 2) as usize;
pub const LFO_TABLE_SIZE: usize = 256;
pub const KEY_CODE_TABLE_SIZE: usize = 128;
pub const LOG_TABLE_BOTTOM: i32 = LOG_VOLUME_BITS * LOG_TABLE_RESOLUTION * 2;
pub const ENV_BOTTOM: i32 = (LOG_VOLUME_BITS * LOG_TABLE_RESOLUTION) >> 2;
pub const ENV_TOP: i32 = ENV_BOTTOM - (1 << ENV_BITS);
pub const ENV_BOTTOM_SSGEC: i32 = 1 << (ENV_BITS - 3);

pub const PT_OPM: usize = 0;
pub const PT_PCM: usize = 1;
pub const PT_PSG: usize = 2;
pub const PT_OPM_NOISE: usize = 3;
pub const PT_PSG_NOISE: usize = 4;
pub const PT_APU_NOISE: usize = 5;
pub const PT_GB_NOISE: usize = 6;
pub const PT_MAX: usize = 7;

pub const PG_SINE: usize = 0;
pub const PG_SAW_UP: usize = 1;
pub const PG_SAW_DOWN: usize = 2;
pub const PG_TRIANGLE_FC: usize = 3;
pub const PG_TRIANGLE: usize = 4;
pub const PG_SQUARE: usize = 5;
pub const PG_NOISE: usize = 6;
pub const PG_KNMBSMM: usize = 7;
pub const PG_SYNC_LOW: usize = 8;
pub const PG_SYNC_HIGH: usize = 9;
pub const PG_OFFSET: usize = 10;
pub const PG_SAW_VC6: usize = 11;
pub const PG_NOISE_WHITE: usize = 16;
pub const PG_NOISE_PULSE: usize = 17;
pub const PG_NOISE_SHORT: usize = 18;
pub const PG_NOISE_HIPAS: usize = 19;
pub const PG_NOISE_PINK: usize = 20;
pub const PG_NOISE_GB_SHORT: usize = 21;
pub const PG_PC_NZ_16BIT: usize = 24;
pub const PG_PC_NZ_SHORT: usize = 25;
pub const PG_PC_NZ_OPM: usize = 26;
pub const PG_MA3_WAVE: usize = 32;
pub const PG_PULSE: usize = 64;
pub const PG_PULSE_SPIKE: usize = 80;
pub const PG_RAMP: usize = 128;
pub const PG_CUSTOM: usize = 256;
pub const PG_PCM: usize = 384;
pub const DEFAULT_PG_MAX: usize = 256;
pub const PG_FILTER: i32 = 511;

pub const WAVE_TABLE_MAX: usize = 128;
pub const PCM_DATA_MAX: usize = 128;
pub const SAMPLER_TABLE_MAX: usize = 4;
pub const SAMPLER_DATA_MAX: usize = NOTE_TABLE_SIZE;

pub const VM_LINEAR: usize = 0;
pub const VM_DR96DB: usize = 1;
pub const VM_DR64DB: usize = 2;
pub const VM_DR48DB: usize = 3;
pub const VM_DR32DB: usize = 4;
pub const VM_MAX: usize = 5;

pub const LFO_WAVE_SAW: usize = 0;
pub const LFO_WAVE_SQUARE: usize = 1;
pub const LFO_WAVE_TRIANGLE: usize = 2;
pub const LFO_WAVE_NOISE: usize = 3;
pub const LFO_WAVE_MAX: usize = 8;

/// ActionScript's `int(Number)`: modulo 2^32, NaN and infinity zero.
pub fn to_i32(x: f64) -> i32 {
    if !x.is_finite() {
        return 0;
    }
    let t = x.trunc();
    if t.abs() < 2147483648.0 {
        return t as i32;
    }
    (t.rem_euclid(4294967296.0) as u64 as u32) as i32
}

/// A wave table: log-table indices of one cycle (`SiOPMWaveTable`).
#[derive(Debug, Clone)]
pub struct WaveTable {
    pub wavelet: Vec<i32>,
    pub fixed_bits: i32,
    pub default_pt_type: usize,
}

impl WaveTable {
    pub fn new(wavelet: Vec<i32>, default_pt_type: usize) -> WaveTable {
        let mut bits = 0;
        let mut len = wavelet.len() >> 1;
        while len != 0 {
            len >>= 1;
            bits += 1;
        }
        WaveTable { wavelet, fixed_bits: PHASE_BITS - bits, default_pt_type }
    }
}

/// `calcLogTableIndex`: the log table index of a number in [-1, 1].
pub fn calc_log_table_index(n: f64) -> i32 {
    // 369.3299304675746 = 256/log(2); 0.0001220703125 = 1/(2^13)
    if n < 0.0 {
        if n < -0.0001220703125 {
            ((to_i32((-n).ln() * -369.3299304675746 + 0.5) + 1) << 1) + 1
        } else {
            LOG_TABLE_BOTTOM
        }
    } else if n > 0.0001220703125 {
        (to_i32(n.ln() * -369.3299304675746 + 0.5) + 1) << 1
    } else {
        LOG_TABLE_BOTTOM
    }
}

/// A small random generator for the noise LFO (`Math.random()` there).
struct Random(u32);

impl Random {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        f64::from(self.0) / 4294967296.0
    }
}

/// The tables.
pub struct Table {
    pub eg_inc_tables: [[i32; 8]; 18],
    pub eg_inc_tables_att: [[i32; 8]; 18],
    pub eg_table_selector: [i32; 128],
    pub eg_level_tables: Vec<Vec<i32>>,
    pub eg_ssg_table_index: [[[usize; 3]; 2]; 10],
    pub eg_timer_steps: [i32; 128],
    pub eg_sl_table: [i32; 16],
    pub eg_tl_tables: Vec<Vec<i32>>,
    pub eg_lv2tl_table: Vec<i32>,
    pub lfo_timer_steps: Vec<i32>,
    pub lfo_wave_tables: Vec<Vec<i32>>,
    pub lfo_chorus_tables: Vec<i32>,
    pub filter_cutoff_table: Vec<f64>,
    pub filter_feedback_table: Vec<f64>,
    pub filter_eg_rate: Vec<i32>,
    pub pitch_table: Vec<Vec<i32>>,
    pub phase_step_shift_filter: Vec<i32>,
    pub log_table: Vec<i32>,
    pub nn_to_kc: Vec<i32>,
    pub pitch_wave_length: Vec<f64>,
    pub no_wave_table: Arc<WaveTable>,
    pub no_wave_table_opm: Arc<WaveTable>,
    pub wave_tables: Vec<Arc<WaveTable>>,
    pub custom_wave_tables: Vec<Option<Arc<WaveTable>>>,
    pub dt1_table: Vec<Vec<i32>>,
    pub dt2_table: [i32; 4],
    pub final_oscilator_flags: [[i32; 16]; 4],
    pub i2n: f64,
    pub pan_table: Vec<f64>,
    pub rate: i32,
    pub clock: i32,
    pub psg_clock: f64,
    pub clock_ratio: i32,
    pub sample_rate_pitch_shift: i32,
}

const INC: [[i32; 8]; 18] = [
    [0, 1, 0, 1, 0, 1, 0, 1],
    [0, 1, 0, 1, 1, 1, 0, 1],
    [0, 1, 1, 1, 0, 1, 1, 1],
    [0, 1, 1, 1, 1, 1, 1, 1],
    [1, 1, 1, 1, 1, 1, 1, 1],
    [1, 1, 1, 2, 1, 1, 1, 2],
    [1, 2, 1, 2, 1, 2, 1, 2],
    [1, 2, 2, 2, 1, 2, 2, 2],
    [2, 2, 2, 2, 2, 2, 2, 2],
    [2, 2, 2, 4, 2, 2, 2, 4],
    [2, 4, 2, 4, 2, 4, 2, 4],
    [2, 4, 4, 4, 2, 4, 4, 4],
    [4, 4, 4, 4, 4, 4, 4, 4],
    [4, 4, 4, 8, 4, 4, 4, 8],
    [4, 8, 4, 8, 4, 8, 4, 8],
    [4, 8, 8, 8, 4, 8, 8, 8],
    [8, 8, 8, 8, 8, 8, 8, 8],
    [0, 0, 0, 0, 0, 0, 0, 0],
];

const INC_ATT: [[i32; 8]; 18] = [
    [0, 4, 0, 4, 0, 4, 0, 4],
    [0, 4, 0, 4, 4, 4, 0, 4],
    [0, 4, 4, 4, 0, 4, 4, 4],
    [0, 4, 4, 4, 4, 4, 4, 4],
    [4, 4, 4, 4, 4, 4, 4, 4],
    [4, 4, 4, 3, 4, 4, 4, 3],
    [4, 3, 4, 3, 4, 3, 4, 3],
    [4, 3, 3, 3, 4, 3, 3, 3],
    [3, 3, 3, 3, 3, 3, 3, 3],
    [3, 3, 3, 2, 3, 3, 3, 2],
    [3, 2, 3, 2, 3, 2, 3, 2],
    [3, 2, 2, 2, 3, 2, 2, 2],
    [2, 2, 2, 2, 2, 2, 2, 2],
    [2, 2, 2, 1, 2, 2, 2, 1],
    [2, 8, 2, 1, 2, 1, 2, 1],
    [2, 1, 1, 1, 2, 1, 1, 1],
    [1, 1, 1, 1, 1, 1, 1, 1],
    [0, 0, 0, 0, 0, 0, 0, 0],
];

impl Table {
    /// The tables SiON builds first (`SiOPMTable.instance`): a 3.58 MHz FM
    /// clock, a 1789772.5 Hz PSG clock, at 44100 or 22050 Hz.
    pub fn new(rate: i32) -> Table {
        Table::with_clocks(3_580_000, 1_789_772.5, rate)
    }

    pub fn with_clocks(clock: i32, psg_clock: f64, rate: i32) -> Table {
        let sample_rate_pitch_shift = match rate {
            44100 => 0,
            22050 => 1,
            _ => -1,
        };
        let clock_ratio = to_i32(f64::from(to_i32(f64::from(clock) / 64.0) << CLOCK_RATIO_BITS) / f64::from(rate));
        let dummy = Arc::new(WaveTable::new(vec![0], PT_PCM));
        let mut t = Table {
            eg_inc_tables: INC,
            eg_inc_tables_att: INC_ATT,
            eg_table_selector: [0; 128],
            eg_level_tables: vec![],
            eg_ssg_table_index: [[[0; 3]; 2]; 10],
            eg_timer_steps: [0; 128],
            eg_sl_table: [0; 16],
            eg_tl_tables: vec![],
            eg_lv2tl_table: vec![],
            lfo_timer_steps: vec![],
            lfo_wave_tables: vec![],
            lfo_chorus_tables: vec![],
            filter_cutoff_table: vec![],
            filter_feedback_table: vec![],
            filter_eg_rate: vec![],
            pitch_table: vec![],
            phase_step_shift_filter: vec![],
            log_table: vec![],
            nn_to_kc: vec![],
            pitch_wave_length: vec![],
            no_wave_table: dummy.clone(),
            no_wave_table_opm: dummy.clone(),
            wave_tables: vec![],
            custom_wave_tables: vec![None; WAVE_TABLE_MAX],
            dt1_table: vec![],
            dt2_table: [0, 384, 500, 608],
            final_oscilator_flags: [
                [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                [2, 3, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                [4, 4, 5, 6, 6, 7, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                [8, 8, 8, 8, 10, 14, 14, 15, 9, 13, 8, 9, 9, 0, 0, 0],
            ],
            i2n: OUTPUT_MAX / f64::from(1 << LOG_VOLUME_BITS),
            pan_table: vec![],
            rate,
            clock,
            psg_clock,
            clock_ratio,
            sample_rate_pitch_shift,
        };
        t.create_eg_tables();
        t.create_pg_tables();
        t.create_wave_samples();
        t.create_lfo_tables();
        t.create_filter_tables();
        t
    }

    fn create_eg_tables(&mut self) {
        let cr = f64::from(self.clock_ratio);
        for i in 0..128usize {
            let (step, sel) = match i {
                0..=43 => (to_i32(f64::from(1 << (i >> 2)) * cr), (i & 3) as i32),
                44..=47 => (to_i32(2047.0 * cr), (i & 3) as i32),
                48..=59 => (to_i32(2047.0 * cr), i as i32 - 44),
                60..=95 => (to_i32(2047.0 * cr), 16),
                _ => (0, 17),
            };
            self.eg_timer_steps[i] = step;
            self.eg_table_selector[i] = sel;
        }
        let imax = 1usize << ENV_BITS;
        let imax2 = imax >> 2;
        self.eg_level_tables = vec![vec![0; imax]; 7];
        for i in 0..imax2 {
            let n = i as i32;
            self.eg_level_tables[0][i] = n;
            self.eg_level_tables[1][i] = n << 2;
            self.eg_level_tables[2][i] = 512 - (n << 2);
            self.eg_level_tables[3][i] = 512 + (n << 2);
            self.eg_level_tables[4][i] = 1024 - (n << 2);
            self.eg_level_tables[5][i] = 0;
            self.eg_level_tables[6][i] = 1024;
        }
        for i in imax2..imax {
            self.eg_level_tables[0][i] = i as i32;
            self.eg_level_tables[1][i] = 1024;
            self.eg_level_tables[2][i] = 0;
            self.eg_level_tables[3][i] = 1024;
            self.eg_level_tables[4][i] = 512;
            self.eg_level_tables[5][i] = 0;
            self.eg_level_tables[6][i] = 1024;
        }
        self.eg_ssg_table_index = [
            [[3, 3, 3], [1, 3, 3]],
            [[1, 6, 6], [1, 6, 6]],
            [[2, 1, 2], [1, 2, 1]],
            [[2, 5, 5], [1, 5, 5]],
            [[4, 4, 4], [2, 4, 4]],
            [[2, 5, 5], [2, 5, 5]],
            [[1, 2, 1], [2, 1, 2]],
            [[1, 6, 6], [2, 6, 6]],
            [[1, 1, 1], [1, 1, 1]],
            [[2, 2, 2], [2, 2, 2]],
        ];
        for i in 0..15 {
            self.eg_sl_table[i] = (i as i32) << 5;
        }
        self.eg_sl_table[15] = 31 << 5;

        // The log table comes first: the volume tables use it.
        self.create_log_table();

        let mut tl: Vec<Vec<i32>> = vec![vec![0; 513]; VM_MAX];
        for t in [VM_LINEAR, VM_DR96DB, VM_DR48DB, VM_DR32DB] {
            tl[t][0] = ENV_BOTTOM;
        }
        for i in 1..257usize {
            let f = i as f64;
            tl[VM_LINEAR][i] = calc_log_table_index(f * 0.00390625) >> (LOG_VOLUME_BITS - ENV_BITS);
            tl[VM_DR96DB][i] = (256 - i as i32) * 4;
            tl[VM_DR64DB][i] = to_i32((256.0 - f) * 2.6666666666666667);
            tl[VM_DR48DB][i] = (256 - i as i32) * 2;
            tl[VM_DR32DB][i] = to_i32((256.0 - f) * 1.333333333333333);
        }
        for i in 1..193usize {
            for t in 0..VM_MAX {
                tl[t][i + 256] = -(i as i32);
            }
        }
        for i in 1..65usize {
            for t in 0..VM_MAX {
                tl[t][i + 448] = ENV_TOP;
            }
        }
        self.eg_tl_tables = tl;
        self.eg_lv2tl_table = (0..129).map(|i| calc_log_table_index(f64::from(i) * 0.0078125) >> (LOG_VOLUME_BITS - ENV_BITS + ENV_LSHIFT)).collect();
        self.pan_table = (0..129).map(|i| (f64::from(i) * 0.01227184630308513).sin()).collect();
    }

    fn create_log_table(&mut self) {
        let mut log = vec![0i32; LOG_TABLE_SIZE * 3];
        let mut i = ((-ENV_TOP) << 3) as usize;
        let imax = i + (LOG_TABLE_RESOLUTION * 2) as usize;
        let jmax = LOG_TABLE_SIZE;
        let dp = 1.0 / f64::from(LOG_TABLE_RESOLUTION);
        let mut p = dp;
        while i < imax {
            let mut v = 2f64.powf(f64::from(LOG_VOLUME_BITS) - p);
            let mut j = i;
            while j < jmax {
                let iv = to_i32(v);
                log[j] = iv;
                log[j + 1] = -iv;
                v *= 0.5;
                j += (LOG_TABLE_RESOLUTION * 2) as usize;
            }
            i += 2;
            p += dp;
        }
        let imax = ((-ENV_TOP) << 3) as usize;
        let iv = log[imax];
        let mut i = 0;
        while i < imax {
            log[i] = iv;
            log[i + 1] = -iv;
            i += 2;
        }
        self.log_table = log;
    }

    fn create_pg_tables(&mut self) {
        // MIDI note number -> key code
        let mut nn = vec![0i32; NOTE_TABLE_SIZE];
        let (mut i, mut j) = (0i32, 0i32);
        while (j as usize) < NOTE_TABLE_SIZE {
            nn[j as usize] = if i < 16 {
                i
            } else if i >= KEY_CODE_TABLE_SIZE as i32 {
                KEY_CODE_TABLE_SIZE as i32 - 1
            } else {
                i - 16
            };
            i += 1;
            j = i - (i >> 2);
        }
        self.nn_to_kc = nn;

        let imax = HALF_TONE_RESOLUTION * 12;
        let jmax = PITCH_TABLE_SIZE;
        let dp = 1.0 / imax as f64;
        let rate = f64::from(self.rate);

        // wave length table
        let mut pwl = vec![0f64; PITCH_TABLE_SIZE];
        let n = rate / 8.175798915643707;
        let mut p = 0.0;
        for i in 0..imax {
            let mut v = 2f64.powf(-p) * n;
            let mut j = i;
            while j < jmax {
                pwl[j] = v;
                v *= 0.5;
                j += imax;
            }
            p += dp;
        }
        self.pitch_wave_length = pwl;

        let mut pitch: Vec<Vec<i32>> = vec![vec![]; PT_MAX];
        let mut shift = vec![0i32; PT_MAX];

        // OPM
        let mut table = vec![0i32; PITCH_TABLE_SIZE];
        let n = 8.175798915643707 * f64::from(PHASE_MAX) / rate;
        let mut p = 0.0;
        for i in 0..imax {
            let mut v = 2f64.powf(p) * n;
            let mut j = i;
            while j < jmax {
                table[j] = to_i32(v);
                v *= 2.0;
                j += imax;
            }
            p += dp;
        }
        pitch[PT_OPM] = table;
        shift[PT_OPM] = 0;

        // PCM
        let mut table = vec![0i32; PITCH_TABLE_SIZE];
        let n = 0.01858136117191752 * f64::from(PHASE_MAX);
        let mut p = 0.0;
        for i in 0..imax {
            let mut v = 2f64.powf(p) * n;
            let mut j = i;
            while j < jmax {
                table[j] = to_i32(v);
                v *= 2.0;
                j += imax;
            }
            p += dp;
        }
        pitch[PT_PCM] = table;
        shift[PT_PCM] = -1;

        // PSG (table size 16)
        let mut table = vec![0i32; PITCH_TABLE_SIZE];
        let n = self.psg_clock * f64::from(PHASE_MAX >> 4) / rate;
        let mut p = 0.0;
        for i in 0..imax {
            let mut v = self.psg_clock / (2f64.powf(p) * 130.8127826502993);
            let mut j = i;
            while j < jmax {
                let mut iv = to_i32(v + 0.5);
                if iv > 4096 {
                    iv = 4096;
                }
                table[j] = to_i32(n / f64::from(iv));
                v *= 0.5;
                j += imax;
            }
            p += dp;
        }
        pitch[PT_PSG] = table;
        shift[PT_PSG] = 0;

        // OPM noise period table
        let imax2 = 32 << HALF_TONE_BITS;
        let mut table = vec![0i32; imax2];
        let n = f64::from(PHASE_MAX) * f64::from(self.clock_ratio);
        let mut iv = 0;
        for i in 0..31usize {
            iv = to_i32(n / (f64::from(32 - i as i32) * 0.5)) >> CLOCK_RATIO_BITS;
            for j in 0..HALF_TONE_RESOLUTION {
                table[(i << HALF_TONE_BITS) + j] = iv;
            }
        }
        for slot in table.iter_mut().skip(31 << HALF_TONE_BITS) {
            *slot = iv;
        }
        pitch[PT_OPM_NOISE] = table;
        shift[PT_OPM_NOISE] = -1;

        // PSG noise period table
        let mut table = vec![0i32; imax2];
        let n = f64::from(PHASE_MAX) * f64::from(self.clock) / (rate * 16.0);
        for i in 0..32usize {
            let iv = to_i32(n / i as f64);
            for j in 0..HALF_TONE_RESOLUTION {
                table[(i << HALF_TONE_BITS) + j] = iv;
            }
        }
        pitch[PT_PSG_NOISE] = table;
        shift[PT_PSG_NOISE] = -1;

        // APU noise period table
        let fc_nf = [4, 8, 16, 32, 64, 96, 128, 160, 202, 254, 380, 508, 762, 1016, 2034, 4068];
        let mut table = vec![0i32; 16 << HALF_TONE_BITS];
        let n = f64::from(PHASE_MAX) * self.psg_clock / rate;
        for i in 0..16usize {
            let iv = to_i32(n / f64::from(fc_nf[i]));
            for j in 0..HALF_TONE_RESOLUTION {
                table[(i << HALF_TONE_BITS) + j] = iv;
            }
        }
        pitch[PT_APU_NOISE] = table;
        shift[PT_APU_NOISE] = -1;

        // Game Boy noise period
        let gb_nf: [i32; 64] = [
            2, 4, 8, 12, 16, 20, 24, 28, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 384, 448, 512, 640, 768, 896, 1024, 1280, 1536, 1792, 2048, 2560, 3072, 3584, 4096, 5120, 6144, 7168, 8192, 10240, 12288,
            14336, 16384, 20480, 24576, 28672, 32768, 40960, 49152, 57344, 65536, 81920, 98304, 114688, 131072, 163840, 196608, 229376, 262144, 327680, 393216, 458752,
        ];
        let mut table = vec![0i32; 64 << HALF_TONE_BITS];
        let n = f64::from(PHASE_MAX) * 1048576.0 / rate;
        for i in 0..64usize {
            let iv = to_i32(n / f64::from(gb_nf[i]));
            for j in 0..HALF_TONE_RESOLUTION {
                table[(i << HALF_TONE_BITS) + j] = iv;
            }
        }
        pitch[PT_GB_NOISE] = table;
        shift[PT_GB_NOISE] = -1;
        self.pitch_table = pitch;
        self.phase_step_shift_filter = shift;

        // dt1 table (from X68Sound.dll)
        let fmgen_dt1: [[i32; 32]; 4] = [
            [0; 32],
            [0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 3, 3, 3, 4, 4, 4, 5, 5, 6, 6, 7, 8, 8, 8, 8],
            [1, 1, 1, 1, 2, 2, 2, 2, 2, 3, 3, 3, 4, 4, 4, 5, 5, 6, 6, 7, 8, 8, 9, 10, 11, 12, 13, 14, 16, 16, 16, 16],
            [2, 2, 2, 2, 2, 3, 3, 3, 4, 4, 4, 5, 5, 6, 6, 7, 8, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 20, 22, 22, 22, 22],
        ];
        let mut dt1 = vec![vec![0i32; KEY_CODE_TABLE_SIZE]; 8];
        for i in 0..4 {
            for j in 0..KEY_CODE_TABLE_SIZE {
                let iv = (fmgen_dt1[i][j >> 2] * 64 * self.clock_ratio) >> CLOCK_RATIO_BITS;
                dt1[i][j] = iv;
                dt1[i + 4][j] = -iv;
            }
        }
        self.dt1_table = dt1;
    }

    fn wave(&self, i: usize) -> Arc<WaveTable> {
        self.wave_tables[i].clone()
    }

    fn create_wave_samples(&mut self) {
        let size = SAMPLING_TABLE_SIZE;
        let lg = calc_log_table_index;
        self.no_wave_table = Arc::new(WaveTable::new(vec![lg(1.0)], PT_PCM));
        self.no_wave_table_opm = Arc::new(WaveTable::new(vec![lg(1.0)], PT_OPM));
        self.wave_tables = vec![self.no_wave_table.clone(); DEFAULT_PG_MAX];
        let alloc = |w: Vec<i32>| Arc::new(WaveTable::new(w, 0));
        let alloc_pt = |w: Vec<i32>, pt: usize| Arc::new(WaveTable::new(w, pt));

        // sine
        let mut t1 = vec![0i32; size];
        let dp = 6.283185307179586 / size as f64;
        let imax = size >> 1;
        let mut p = dp * 0.5;
        for i in 0..imax {
            let iv = lg(p.sin());
            t1[i] = iv;
            t1[i + imax] = iv + 1;
            p += dp;
        }
        self.wave_tables[PG_SINE] = alloc(t1);

        // saws
        let mut t1 = vec![0i32; size];
        let mut t2 = vec![0i32; size];
        let dp = 1.0 / imax as f64;
        let mut p = dp * 0.5;
        for i in 0..imax {
            let iv = lg(p);
            t1[i] = iv;
            t1[size - i - 1] = iv + 1;
            t2[imax - i - 1] = iv;
            t2[imax + i] = iv + 1;
            p += dp;
        }
        self.wave_tables[PG_SAW_UP] = alloc(t1);
        self.wave_tables[PG_SAW_DOWN] = alloc(t2);
        let mut t1 = vec![0i32; 32];
        let mut p = -0.96875;
        for slot in t1.iter_mut() {
            *slot = lg(p);
            p += 0.0625;
        }
        self.wave_tables[PG_SAW_VC6] = alloc(t1);

        // triangles
        let mut t1 = vec![0i32; size];
        let imax = size >> 2;
        let imax2 = size >> 1;
        let imax4 = size;
        let dp = 1.0 / imax as f64;
        let mut p = dp * 0.5;
        for i in 0..imax {
            let iv = lg(p);
            t1[i] = iv;
            t1[imax2 - i - 1] = iv;
            t1[imax2 + i] = iv + 1;
            t1[imax4 - i - 1] = iv + 1;
            p += dp;
        }
        self.wave_tables[PG_TRIANGLE] = alloc(t1);
        let mut t1 = vec![0i32; 32];
        let mut p = 0.125;
        for i in 1..8usize {
            let iv = lg(p);
            t1[i] = iv;
            t1[15 - i] = iv;
            t1[15 + i] = iv + 1;
            t1[32 - i] = iv + 1;
            p += 0.125;
        }
        t1[0] = LOG_TABLE_BOTTOM;
        t1[15] = LOG_TABLE_BOTTOM;
        t1[23] = 3;
        t1[24] = 3;
        self.wave_tables[PG_TRIANGLE_FC] = alloc(t1);

        // square and pulses
        let iv = lg(SQUARE_WAVE_OUTPUT);
        self.wave_tables[PG_SQUARE] = alloc(vec![iv, iv + 1]);
        let sq = self.wave(PG_SQUARE).wavelet.clone();
        for j in 0..16usize {
            let t1: Vec<i32> = (0..16).map(|i| if i < j { sq[0] } else { sq[1] }).collect();
            self.wave_tables[PG_PULSE + j] = alloc(t1);
        }
        let iv = lg(0.0);
        for j in 0..16usize {
            let mut t1 = vec![0i32; 32];
            let imax = j << 1;
            for i in 0..imax {
                t1[i] = if i < j { sq[0] } else { sq[1] };
            }
            for slot in t1.iter_mut().skip(imax) {
                *slot = iv;
            }
            self.wave_tables[PG_PULSE_SPIKE + j] = alloc(t1);
        }

        // Konami Bubble System
        let wav: [i32; 32] = [-80, -112, -16, 96, 64, 16, 64, 96, 32, -16, 64, 112, 80, 0, 32, 48, -16, -96, 0, 80, 16, -64, -48, -16, -96, -128, -80, 0, -48, -112, -80, -32];
        self.wave_tables[PG_KNMBSMM] = alloc(wav.iter().map(|&w| lg(f64::from(w) / 128.0)).collect());

        // pseudo sync
        let mut t1 = vec![0i32; size];
        let mut t2 = vec![0i32; size];
        let dp = 1.0 / size as f64;
        let mut p = dp * 0.5;
        for i in 0..size {
            let iv = lg(p);
            t1[i] = iv + 1;
            t2[i] = iv;
            p += dp;
        }
        self.wave_tables[PG_SYNC_LOW] = alloc(t1);
        self.wave_tables[PG_SYNC_HIGH] = alloc(t2);

        // noise
        let mut t1 = vec![0i32; NOISE_TABLE_SIZE];
        let mut t2 = vec![0i32; NOISE_TABLE_SIZE];
        let iv = lg(NOISE_WAVE_OUTPUT);
        let n = NOISE_WAVE_OUTPUT / 32768.0;
        let mut j: i32 = 1;
        for i in 0..NOISE_TABLE_SIZE {
            j = (((j << 13) ^ (j << 14)) & 0x4000) | (j >> 1);
            t1[i] = lg(f64::from(j & 0x7fff) * n * 2.0 - 1.0);
            t2[i] = if j & 1 != 0 { iv } else { iv + 1 };
        }
        self.wave_tables[PG_NOISE_WHITE] = alloc_pt(t1, PT_PCM);
        self.wave_tables[PG_NOISE_PULSE] = alloc_pt(t2.clone(), PT_PCM);
        self.wave_tables[PG_PC_NZ_OPM] = alloc_pt(t2, PT_OPM_NOISE);
        self.wave_tables[PG_NOISE] = self.wave(PG_NOISE_WHITE);

        // FC short noise
        let mut t1 = vec![0i32; size];
        let iv = lg(NOISE_WAVE_OUTPUT);
        let mut j: i32 = 1;
        for slot in t1.iter_mut() {
            j = (((j << 8) ^ (j << 14)) & 0x4000) | (j >> 1);
            *slot = if j & 1 != 0 { iv } else { iv + 1 };
        }
        self.wave_tables[PG_NOISE_SHORT] = alloc_pt(t1, PT_PCM);

        // GB short noise
        let mut t1 = vec![0i32; 128];
        let iv = lg(NOISE_WAVE_OUTPUT);
        let mut j: i32 = 0xffff;
        let mut o: i32 = 0;
        for slot in t1.iter_mut() {
            j = j.wrapping_add(j.wrapping_add(((j >> 6) ^ (j >> 5)) & 1));
            o ^= j & 1;
            *slot = if o & 1 != 0 { iv } else { iv + 1 };
        }
        self.wave_tables[PG_NOISE_GB_SHORT] = alloc_pt(t1, PT_PCM);

        // periodic noise
        let mut t1 = vec![LOG_TABLE_BOTTOM; 16];
        t1[0] = lg(SQUARE_WAVE_OUTPUT);
        self.wave_tables[PG_PC_NZ_16BIT] = alloc(t1.clone());

        // high-passed white noise
        let white = self.wave(PG_NOISE_WHITE).wavelet.clone();
        let off = ((-ENV_TOP) << 3) as usize;
        let log_table = self.log_table.clone();
        let log = |i: i32| -> f64 { f64::from(log_table[i as usize]) };
        let n = 16.0 / f64::from(1 << LOG_VOLUME_BITS);
        let p = 0.0625;
        let mut v = (log(white[0] + off as i32) - log(white[NOISE_TABLE_SIZE - 1] + off as i32)) * p;
        let mut t1 = vec![0i32; NOISE_TABLE_SIZE];
        t1[0] = lg(v * n);
        for i in 1..NOISE_TABLE_SIZE {
            v = (v + log(white[i] + off as i32) - log(white[i - 1] + off as i32)) * p;
            t1[i] = lg(v * n);
        }
        self.wave_tables[PG_NOISE_HIPAS] = alloc_pt(t1, PT_PCM);

        // pink noise
        let (mut b0, mut b1, mut b2) = (0f64, 0f64, 0f64);
        let n = 0.125 / f64::from(1 << LOG_VOLUME_BITS);
        let mut t1 = vec![0i32; NOISE_TABLE_SIZE];
        for i in 0..NOISE_TABLE_SIZE {
            let v = log(white[i] + off as i32);
            b0 = 0.99765 * b0 + v * 0.0990460;
            b1 = 0.96300 * b1 + v * 0.2965164;
            b2 = 0.57000 * b2 + v * 1.0526913;
            t1[i] = lg((b0 + b1 + b2 + v * 0.1848) * n);
        }
        self.wave_tables[PG_NOISE_PINK] = alloc_pt(t1, PT_PCM);

        // pitch controllable noise
        let short = self.wave(PG_NOISE_SHORT).wavelet.clone();
        let mut t2 = vec![0i32; size];
        for j in 0..size {
            let mut i = j * 11;
            let imax = if i + 11 < size { i + 11 } else { size };
            while i < imax {
                t2[i] = short[j];
                i += 1;
            }
        }
        self.wave_tables[PG_PC_NZ_SHORT] = alloc(t2);

        // ramp waves
        let imax = size;
        let imax2 = size >> 1;
        let imax4 = (size >> 2) as i32;
        let mut prev = 0i32;
        for j in 1..60i32 {
            let mut iv = imax4 >> (j >> 3);
            iv -= (iv * (j & 7)) >> 4;
            if prev == iv {
                self.wave_tables[(PG_RAMP as i32 + 64 - j) as usize] = self.wave((PG_RAMP as i32 + 65 - j) as usize);
                self.wave_tables[(PG_RAMP as i32 + 64 + j) as usize] = self.wave((PG_RAMP as i32 + 63 + j) as usize);
                continue;
            }
            prev = iv;
            let mut t1 = vec![0i32; size];
            let mut t2 = vec![0i32; size];
            let imax3 = imax2 as i32 - iv;
            let mut dp = 1.0 / f64::from(imax3);
            let mut p = dp * 0.5;
            let mut i = 0usize;
            while (i as i32) < imax3 {
                let iv = lg(p);
                t1[i] = iv;
                t1[imax - i - 1] = iv + 1;
                t2[imax2 + i] = iv + 1;
                t2[imax2 - i - 1] = iv;
                i += 1;
                p += dp;
            }
            dp = 1.0 / f64::from(imax2 as i32 - imax3);
            while i < imax2 {
                let iv = lg(p);
                t1[i] = iv;
                t1[imax - i - 1] = iv + 1;
                t2[imax2 + i] = iv + 1;
                t2[imax2 - i - 1] = iv;
                i += 1;
                p -= dp;
            }
            self.wave_tables[(PG_RAMP as i32 + 64 - j) as usize] = alloc(t1);
            self.wave_tables[(PG_RAMP as i32 + 64 + j) as usize] = alloc(t2);
        }
        for j in 0..5 {
            self.wave_tables[PG_RAMP + j] = self.wave(PG_SAW_UP);
        }
        for j in 124..128 {
            self.wave_tables[PG_RAMP + j] = self.wave(PG_SAW_DOWN);
        }
        self.wave_tables[PG_RAMP + 64] = self.wave(PG_TRIANGLE);

        // MA3 waves
        self.wave_tables[PG_MA3_WAVE] = self.wave(PG_SINE);
        self.exp_ma3_waves(0);
        let t2 = self.wave(PG_SINE).wavelet.clone();
        let mut t1 = vec![0i32; size];
        let mut j = 0i32;
        for i in 0..size {
            t1[i] = t2[(i as i32 + j) as usize];
            j += 1 - ((((i >> (SAMPLING_TABLE_BITS - 3)) as i32) + 1) & 2);
        }
        self.wave_tables[PG_MA3_WAVE + 8] = alloc(t1);
        self.exp_ma3_waves(8);
        self.wave_tables[PG_MA3_WAVE + 16] = self.wave(PG_TRIANGLE);
        self.exp_ma3_waves(16);
        self.wave_tables[PG_MA3_WAVE + 24] = self.wave(PG_SAW_UP);
        self.exp_ma3_waves(24);
        self.wave_tables[PG_MA3_WAVE + 6] = self.wave(PG_SQUARE);
        let iv = lg(1.0);
        self.wave_tables[PG_MA3_WAVE + 14] = alloc(vec![iv, LOG_TABLE_BOTTOM]);
        self.wave_tables[PG_MA3_WAVE + 22] = alloc(vec![iv, LOG_TABLE_BOTTOM, iv, LOG_TABLE_BOTTOM]);
        self.wave_tables[PG_MA3_WAVE + 30] = alloc(vec![iv, LOG_TABLE_BOTTOM, LOG_TABLE_BOTTOM, LOG_TABLE_BOTTOM]);
        let mut t1 = vec![0i32; size];
        let dp = 6.283185307179586 / size as f64;
        let imax = size >> 2;
        let imax2 = size >> 1;
        let imax4 = size;
        let mut p = dp * 0.5;
        for i in 0..imax {
            let iv = lg(1.0 - p.sin());
            t1[i] = iv;
            t1[i + imax] = LOG_TABLE_BOTTOM;
            t1[i + imax2] = LOG_TABLE_BOTTOM;
            t1[imax4 - i - 1] = iv + 1;
            p += dp;
        }
        self.wave_tables[PG_MA3_WAVE + 7] = alloc(t1);
        self.wave_tables[PG_MA3_WAVE + 15] = self.no_wave_table.clone();
        self.wave_tables[PG_MA3_WAVE + 23] = self.no_wave_table.clone();
        self.wave_tables[PG_MA3_WAVE + 31] = self.no_wave_table.clone();
    }

    fn exp_ma3_waves(&mut self, index: usize) {
        let size = SAMPLING_TABLE_SIZE;
        let t2 = self.wave(PG_MA3_WAVE + index).wavelet.clone();
        let alloc = |w: Vec<i32>| Arc::new(WaveTable::new(w, 0));
        let half = size >> 1;
        let quarter = size >> 2;
        let mut t = vec![0i32; size];
        for i in 0..half {
            t[i] = t2[i];
            t[i + half] = LOG_TABLE_BOTTOM;
        }
        self.wave_tables[PG_MA3_WAVE + index + 1] = alloc(t);
        let mut t = vec![0i32; size];
        for i in 0..half {
            t[i] = t2[i];
            t[i + half] = t2[i];
        }
        self.wave_tables[PG_MA3_WAVE + index + 2] = alloc(t);
        let mut t = vec![0i32; size];
        for i in 0..quarter {
            t[i] = t2[i];
            t[i + quarter] = LOG_TABLE_BOTTOM;
            t[i + quarter * 2] = t2[i];
            t[i + quarter * 3] = LOG_TABLE_BOTTOM;
        }
        self.wave_tables[PG_MA3_WAVE + index + 3] = alloc(t);
        let mut t = vec![0i32; size];
        for i in 0..half {
            t[i] = t2[i << 1];
            t[i + half] = LOG_TABLE_BOTTOM;
        }
        self.wave_tables[PG_MA3_WAVE + index + 4] = alloc(t);
        let mut t = vec![0i32; size];
        for i in 0..quarter {
            t[i] = t2[i << 1];
            t[i + quarter] = t[i];
            t[i + quarter * 2] = LOG_TABLE_BOTTOM;
            t[i + quarter * 3] = LOG_TABLE_BOTTOM;
        }
        self.wave_tables[PG_MA3_WAVE + index + 5] = alloc(t);
    }

    fn create_lfo_tables(&mut self) {
        let cr = i64::from(self.clock_ratio);
        self.lfo_timer_steps = (0..256i64)
            .map(|i| {
                let t = 16 + (i & 15);
                let s = 15 - (i >> 4);
                // ((t << 16) * clock_ratio / (8 << s)) >> 10, as AS3 does it in Numbers
                let v = ((t << (LFO_FIXED_BITS - 4)) * cr) as f64 / (8i64 << s) as f64;
                to_i32(v) >> CLOCK_RATIO_BITS
            })
            .collect();
        let mut waves = vec![vec![0i32; 256]; LFO_WAVE_MAX];
        for i in 0..256usize {
            waves[LFO_WAVE_SAW][i] = 255 - i as i32;
            waves[LFO_WAVE_SAW + 4][i] = i as i32;
            waves[LFO_WAVE_SQUARE][i] = if i < 128 { 255 } else { 0 };
            waves[LFO_WAVE_SQUARE + 4][i] = 255 - waves[LFO_WAVE_SQUARE][i];
        }
        for i in 0..64usize {
            let t = (i as i32) << 1;
            waves[LFO_WAVE_TRIANGLE][i] = t + 128;
            waves[LFO_WAVE_TRIANGLE][127 - i] = t + 128;
            waves[LFO_WAVE_TRIANGLE][128 + i] = 126 - t;
            waves[LFO_WAVE_TRIANGLE][255 - i] = 126 - t;
        }
        for i in 0..256usize {
            waves[LFO_WAVE_TRIANGLE + 4][i] = 255 - waves[LFO_WAVE_TRIANGLE][i];
        }
        let mut rng = Random(0x9e3779b9);
        for i in 0..256usize {
            waves[LFO_WAVE_NOISE][i] = to_i32(rng.next() * 255.0);
            waves[LFO_WAVE_NOISE + 4][i] = 255 - waves[LFO_WAVE_NOISE][i];
        }
        self.lfo_wave_tables = waves;
        self.lfo_chorus_tables = (0..256).map(|i| (i - 128) * (i - 128)).collect();
    }

    fn create_filter_tables(&mut self) {
        let mut cutoff = vec![0f64; 129];
        let mut feedback = vec![0f64; 129];
        for i in 0..128usize {
            cutoff[i] = (i * i) as f64 * 0.00006103515625;
            feedback[i] = 1.0 + 1.0 / (1.0 - cutoff[i]);
        }
        cutoff[128] = 1.0;
        self.filter_cutoff_table = cutoff;
        self.filter_feedback_table = feedback;
        let mut rate = vec![0i32; 64];
        for i in 1..60usize {
            let shift = f64::from(1i32 << (14 - (i >> 2)));
            let liner = (i & 3) as f64 * 0.125 + 0.5;
            rate[i] = to_i32(2.36514 * shift * liner + 0.5);
        }
        for slot in rate.iter_mut().skip(60) {
            *slot = 1;
        }
        self.filter_eg_rate = rate;
    }

    /// `getWaveTable`: a pulse generator type's wave table.
    pub fn get_wave_table(&self, index: usize) -> Arc<WaveTable> {
        if index < PG_CUSTOM {
            return self.wave(index);
        }
        if index < PG_PCM {
            return self.custom_wave_tables[index - PG_CUSTOM].clone().unwrap_or_else(|| self.no_wave_table_opm.clone());
        }
        self.no_wave_table.clone()
    }

    /// `registerWaveTable`: a custom wave table (`#WAV`).
    pub fn register_wave_table(&mut self, index: usize, table: Vec<i32>) -> Arc<WaveTable> {
        let w = Arc::new(WaveTable::new(table, 0));
        let index = index & (WAVE_TABLE_MAX - 1);
        self.custom_wave_tables[index] = Some(w.clone());
        if index < 3 {
            self.wave_tables[15 + index * 8 + PG_MA3_WAVE] = w.clone();
        }
        w
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_constants_come_out_as_siopms_do() {
        assert_eq!(ENV_BOTTOM, 832);
        assert_eq!(ENV_TOP, -192);
        assert_eq!(LOG_TABLE_BOTTOM, 6656);
        assert_eq!(PHASE_BITS, 26);
        assert_eq!(to_i32(5.44e9), 1_145_032_704);
        assert_eq!(to_i32(-1.5), -1);
        assert_eq!(to_i32(f64::INFINITY), 0);
    }

    #[test]
    fn the_tables_are_built_and_the_waves_are_where_they_belong() {
        let t = Table::new(44100);
        assert_eq!(t.clock_ratio, 1298);
        assert_eq!(t.log_table.len(), LOG_TABLE_SIZE * 3);
        // A sine is positive then negative, a sample of a log table index each.
        let sine = &t.get_wave_table(PG_SINE).wavelet;
        assert_eq!(sine.len(), 1024);
        assert_eq!(sine[0] & 1, 0);
        assert_eq!(sine[512] & 1, 1);
        assert_eq!(t.get_wave_table(PG_SINE).fixed_bits, 16);
        // Every pulse generator below the custom range has a table.
        for i in 0..PG_CUSTOM {
            assert!(!t.get_wave_table(i).wavelet.is_empty(), "pg {i}");
        }
        // The log table gives about full scale at the top of the envelope.
        let top = t.log_table[((-ENV_TOP) << 3) as usize + 2];
        assert!((8000..=8192).contains(&top), "{top}");
        // 440 Hz: note 69 at 64 steps a half tone.
        let step = t.pitch_table[PT_OPM][69 * 64];
        let hz = f64::from(step) * 44100.0 / f64::from(PHASE_MAX);
        assert!((hz - 440.0).abs() < 0.5, "{hz}");
    }
}
