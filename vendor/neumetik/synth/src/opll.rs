// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.
//
// A port of emu2413 (YM2413 emulator), MIT:
//
// Copyright (c) 2001-2019 Mitsutaka Okazaki
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

//! Yamaha's YM2413 (OPLL, MSX-MUSIC): nine two-operator FM channels, or six
//! and five rhythm sounds, fifteen ROM voices and one of the user's. A port of
//! Mitsutaka Okazaki's emu2413 (YM2413 voices, mono), its rate converter too.

use std::sync::OnceLock;

/// The YM2413's voices: the user's (0), fifteen ROM voices, three rhythm.
#[rustfmt::skip]
const DEFAULT_INST: [u8; 19 * 8] = [
    0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,
    0x71,0x61,0x1e,0x17,0xd0,0x78,0x00,0x17,
    0x13,0x41,0x1a,0x0d,0xd8,0xf7,0x23,0x13,
    0x13,0x01,0x99,0x00,0xf2,0xc4,0x21,0x23,
    0x11,0x61,0x0e,0x07,0x8d,0x64,0x70,0x27,
    0x32,0x21,0x1e,0x06,0xe1,0x76,0x01,0x28,
    0x31,0x22,0x16,0x05,0xe0,0x71,0x00,0x18,
    0x21,0x61,0x1d,0x07,0x82,0x81,0x11,0x07,
    0x33,0x21,0x2d,0x13,0xb0,0x70,0x00,0x07,
    0x61,0x61,0x1b,0x06,0x64,0x65,0x10,0x17,
    0x41,0x61,0x0b,0x18,0x85,0xf0,0x81,0x07,
    0x33,0x01,0x83,0x11,0xea,0xef,0x10,0x04,
    0x17,0xc1,0x24,0x07,0xf8,0xf8,0x22,0x12,
    0x61,0x50,0x0c,0x05,0xd2,0xf5,0x40,0x42,
    0x01,0x01,0x55,0x03,0xe9,0x90,0x03,0x02,
    0x41,0x41,0x89,0x03,0xf1,0xe4,0xc0,0x13,
    0x01,0x01,0x18,0x0f,0xdf,0xf8,0x6a,0x6d,
    0x01,0x01,0x00,0x00,0xc8,0xd8,0xa7,0x68,
    0x05,0x01,0x00,0x00,0xf8,0xaa,0x59,0x55,
];

/// The VRC7's voices (Konami's YM2413 variant), from emu2413's VRC7
/// presets (Nuke.YKT's).
#[rustfmt::skip]
const VRC7_INST: [u8; 19 * 8] = [
    0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,
    0x03,0x21,0x05,0x06,0xe8,0x81,0x42,0x27,
    0x13,0x41,0x14,0x0d,0xd8,0xf6,0x23,0x12,
    0x11,0x11,0x08,0x08,0xfa,0xb2,0x20,0x12,
    0x31,0x61,0x0c,0x07,0xa8,0x64,0x61,0x27,
    0x32,0x21,0x1e,0x06,0xe1,0x76,0x01,0x28,
    0x02,0x01,0x06,0x00,0xa3,0xe2,0xf4,0xf4,
    0x21,0x61,0x1d,0x07,0x82,0x81,0x11,0x07,
    0x23,0x21,0x22,0x17,0xa2,0x72,0x01,0x17,
    0x35,0x11,0x25,0x00,0x40,0x73,0x72,0x01,
    0xb5,0x01,0x0f,0x0f,0xa8,0xa5,0x51,0x02,
    0x17,0xc1,0x24,0x07,0xf8,0xf8,0x22,0x12,
    0x71,0x23,0x11,0x06,0x65,0x74,0x18,0x16,
    0x01,0x02,0xd3,0x05,0xc9,0x95,0x03,0x02,
    0x61,0x63,0x0c,0x00,0x94,0xc0,0x33,0xf6,
    0x21,0x72,0x0d,0x00,0xc1,0xd5,0x56,0x06,
    0x01,0x01,0x18,0x0f,0xdf,0xf8,0x6a,0x6d,
    0x01,0x01,0x00,0x00,0xc8,0xd8,0xa7,0x68,
    0x05,0x01,0x00,0x00,0xf8,0xaa,0x59,0x55,
];

const DP_BITS: u32 = 19;
const DP_WIDTH: u32 = 1 << DP_BITS;
const PG_BITS: u32 = 10;
const PG_WIDTH: usize = 1 << PG_BITS;
const DP_BASE_BITS: u32 = DP_BITS - PG_BITS;
const EG_STEP: f64 = 0.375;
const EG_BITS: u32 = 7;
const EG_MUTE: u32 = (1 << EG_BITS) - 1;
const EG_MAX: u32 = EG_MUTE - 4;
const DAMPER_RATE: u32 = 12;

/// The MSX's clock.
pub const CLOCK: u32 = 3_579_545;

#[rustfmt::skip]
const EXP_TABLE: [u16; 256] = [
0,    3,    6,    8,    11,   14,   17,   20,   22,   25,   28,   31,   34,   37,   40,   42,
45,   48,   51,   54,   57,   60,   63,   66,   69,   72,   75,   78,   81,   84,   87,   90,
93,   96,   99,   102,  105,  108,  111,  114,  117,  120,  123,  126,  130,  133,  136,  139,
142,  145,  148,  152,  155,  158,  161,  164,  168,  171,  174,  177,  181,  184,  187,  190,
194,  197,  200,  204,  207,  210,  214,  217,  220,  224,  227,  231,  234,  237,  241,  244,
248,  251,  255,  258,  262,  265,  268,  272,  276,  279,  283,  286,  290,  293,  297,  300,
304,  308,  311,  315,  318,  322,  326,  329,  333,  337,  340,  344,  348,  352,  355,  359,
363,  367,  370,  374,  378,  382,  385,  389,  393,  397,  401,  405,  409,  412,  416,  420,
424,  428,  432,  436,  440,  444,  448,  452,  456,  460,  464,  468,  472,  476,  480,  484,
488,  492,  496,  501,  505,  509,  513,  517,  521,  526,  530,  534,  538,  542,  547,  551,
555,  560,  564,  568,  572,  577,  581,  585,  590,  594,  599,  603,  607,  612,  616,  621,
625,  630,  634,  639,  643,  648,  652,  657,  661,  666,  670,  675,  680,  684,  689,  693,
698,  703,  708,  712,  717,  722,  726,  731,  736,  741,  745,  750,  755,  760,  765,  770,
774,  779,  784,  789,  794,  799,  804,  809,  814,  819,  824,  829,  834,  839,  844,  849,
854,  859,  864,  869,  874,  880,  885,  890,  895,  900,  906,  911,  916,  921,  927,  932,
937,  942,  948,  953,  959,  964,  969,  975,  980,  986,  991,  996, 1002, 1007, 1013, 1018,
];

// The first quarter of -log2(sin) (×256); the rest is made from it.
#[rustfmt::skip]
const SIN_QUARTER: [u16; 256] = [
2137, 1731, 1543, 1419, 1326, 1252, 1190, 1137, 1091, 1050, 1013, 979,  949,  920,  894,  869,
846,  825,  804,  785,  767,  749,  732,  717,  701,  687,  672,  659,  646,  633,  621,  609,
598,  587,  576,  566,  556,  546,  536,  527,  518,  509,  501,  492,  484,  476,  468,  461,
453,  446,  439,  432,  425,  418,  411,  405,  399,  392,  386,  380,  375,  369,  363,  358,
352,  347,  341,  336,  331,  326,  321,  316,  311,  307,  302,  297,  293,  289,  284,  280,
276,  271,  267,  263,  259,  255,  251,  248,  244,  240,  236,  233,  229,  226,  222,  219,
215,  212,  209,  205,  202,  199,  196,  193,  190,  187,  184,  181,  178,  175,  172,  169,
167,  164,  161,  159,  156,  153,  151,  148,  146,  143,  141,  138,  136,  134,  131,  129,
127,  125,  122,  120,  118,  116,  114,  112,  110,  108,  106,  104,  102,  100,  98,   96,
94,   92,   91,   89,   87,   85,   83,   82,   80,   78,   77,   75,   74,   72,   70,   69,
67,   66,   64,   63,   62,   60,   59,   57,   56,   55,   53,   52,   51,   49,   48,   47,
46,   45,   43,   42,   41,   40,   39,   38,   37,   36,   35,   34,   33,   32,   31,   30,
29,   28,   27,   26,   25,   24,   23,   23,   22,   21,   20,   20,   19,   18,   17,   17,
16,   15,   15,   14,   13,   13,   12,   12,   11,   10,   10,   9,    9,    8,    8,    7,
7,    7,    6,    6,    5,    5,    5,    4,    4,    4,    3,    3,    3,    2,    2,    2,
2,    1,    1,    1,    1,    1,    1,    1,    0,    0,    0,    0,    0,    0,    0,    0,
];

const PM_TABLE: [[i8; 8]; 8] = [
    [0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 1, 0, 0, 0, -1, 0],
    [0, 1, 2, 1, 0, -1, -2, -1],
    [0, 1, 3, 1, 0, -1, -3, -1],
    [0, 2, 4, 2, 0, -2, -4, -2],
    [0, 2, 5, 2, 0, -2, -5, -2],
    [0, 3, 6, 3, 0, -3, -6, -3],
    [0, 3, 7, 3, 0, -3, -7, -3],
];

#[rustfmt::skip]
const AM_TABLE: [u8; 210] = [
    0,  0,  0,  0,  0,  0,  0,  0,  1,  1,  1,  1,  1,  1,  1,  1,
    2,  2,  2,  2,  2,  2,  2,  2,  3,  3,  3,  3,  3,  3,  3,  3,
    4,  4,  4,  4,  4,  4,  4,  4,  5,  5,  5,  5,  5,  5,  5,  5,
    6,  6,  6,  6,  6,  6,  6,  6,  7,  7,  7,  7,  7,  7,  7,  7,
    8,  8,  8,  8,  8,  8,  8,  8,  9,  9,  9,  9,  9,  9,  9,  9,
    10, 10, 10, 10, 10, 10, 10, 10, 11, 11, 11, 11, 11, 11, 11, 11,
    12, 12, 12, 12, 12, 12, 12, 12,
    13, 13, 13,
    12, 12, 12, 12, 12, 12, 12, 12,
    11, 11, 11, 11, 11, 11, 11, 11, 10, 10, 10, 10, 10, 10, 10, 10,
    9,  9,  9,  9,  9,  9,  9,  9,  8,  8,  8,  8,  8,  8,  8,  8,
    7,  7,  7,  7,  7,  7,  7,  7,  6,  6,  6,  6,  6,  6,  6,  6,
    5,  5,  5,  5,  5,  5,  5,  5,  4,  4,  4,  4,  4,  4,  4,  4,
    3,  3,  3,  3,  3,  3,  3,  3,  2,  2,  2,  2,  2,  2,  2,  2,
    1,  1,  1,  1,  1,  1,  1,  1,  0,  0,  0,  0,  0,  0,  0,
];

const EG_STEP_TABLES: [[u8; 8]; 4] = [[0, 1, 0, 1, 0, 1, 0, 1], [0, 1, 0, 1, 1, 1, 0, 1], [0, 1, 1, 1, 0, 1, 1, 1], [0, 1, 1, 1, 1, 1, 1, 1]];

const ML_TABLE: [u32; 16] = [1, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 20, 24, 24, 30, 30];

const KL_TABLE: [f64; 16] = [0.0, 18.0, 24.0, 27.75, 30.0, 32.25, 33.75, 35.25, 36.0, 37.5, 38.25, 39.0, 39.75, 40.5, 41.25, 42.0];

const ATTACK: u8 = 0;
const DECAY: u8 = 1;
const SUSTAIN: u8 = 2;
const RELEASE: u8 = 3;
const DAMP: u8 = 4;

const SLOT_BD1: usize = 12;
const SLOT_HH: usize = 14;
const SLOT_SD: usize = 15;
const SLOT_TOM: usize = 16;
const SLOT_CYM: usize = 17;

const UPDATE_WS: u32 = 1;
const UPDATE_TLL: u32 = 2;
const UPDATE_RKS: u32 = 4;
const UPDATE_EG: u32 = 8;
const UPDATE_ALL: u32 = 255;

// The sinc rate converter's length, phases and precision.
const LW: usize = 16;
const SINC_RESO_BITS: u32 = 8;
const SINC_RESO: usize = 1 << SINC_RESO_BITS;
const SINC_AMP_BITS: u32 = 12;

struct Tables {
    fullsin: [u16; PG_WIDTH],
    halfsin: [u16; PG_WIDTH],
    tll: [[u16; 4]; 8 * 16],
    rks: [[u32; 2]; 16],
    default_patch: [Patch; 38],
}

fn tables() -> &'static Tables {
    static T: OnceLock<Tables> = OnceLock::new();
    T.get_or_init(|| {
        let mut fullsin = [0u16; PG_WIDTH];
        fullsin[..256].copy_from_slice(&SIN_QUARTER);
        for x in 0..PG_WIDTH / 4 {
            fullsin[PG_WIDTH / 4 + x] = fullsin[PG_WIDTH / 4 - x - 1];
        }
        for x in 0..PG_WIDTH / 2 {
            fullsin[PG_WIDTH / 2 + x] = 0x8000 | fullsin[x];
        }
        let mut halfsin = [0u16; PG_WIDTH];
        halfsin[..PG_WIDTH / 2].copy_from_slice(&fullsin[..PG_WIDTH / 2]);
        for h in halfsin.iter_mut().skip(PG_WIDTH / 2) {
            *h = 0xfff;
        }
        let mut tll = [[0u16; 4]; 8 * 16];
        for fnum in 0..16 {
            for block in 0..8 {
                for kl in 1..4 {
                    let tmp = (KL_TABLE[fnum] - 6.0 * (7 - block) as f64) as i32;
                    tll[(block << 4) | fnum][kl] = if tmp <= 0 { 0 } else { ((tmp >> (3 - kl)) as f64 / EG_STEP) as u16 };
                }
            }
        }
        let mut rks = [[0u32; 2]; 16];
        for fnum8 in 0..2u32 {
            for block in 0..8u32 {
                rks[((block << 1) | fnum8) as usize][1] = (block << 1) + fnum8;
                rks[((block << 1) | fnum8) as usize][0] = block >> 1;
            }
        }
        let mut default_patch = [Patch::default(); 38];
        for j in 0..19 {
            let p = dump_to_patch(&DEFAULT_INST[j * 8..j * 8 + 8]);
            default_patch[j * 2] = p[0];
            default_patch[j * 2 + 1] = p[1];
        }
        Tables {
            fullsin,
            halfsin,
            tll,
            rks,
            default_patch,
        }
    })
}

#[derive(Clone, Copy, Default)]
struct Patch {
    tl: u32,
    fb: u32,
    eg: u32,
    ml: u32,
    ar: u32,
    dr: u32,
    sl: u32,
    rr: u32,
    kr: u32,
    kl: u32,
    am: u32,
    pm: u32,
    ws: u32,
}

fn dump_to_patch(d: &[u8]) -> [Patch; 2] {
    let d: Vec<u32> = d.iter().map(|&b| u32::from(b)).collect();
    let mut p = [Patch::default(); 2];
    p[0].am = (d[0] >> 7) & 1;
    p[1].am = (d[1] >> 7) & 1;
    p[0].pm = (d[0] >> 6) & 1;
    p[1].pm = (d[1] >> 6) & 1;
    p[0].eg = (d[0] >> 5) & 1;
    p[1].eg = (d[1] >> 5) & 1;
    p[0].kr = (d[0] >> 4) & 1;
    p[1].kr = (d[1] >> 4) & 1;
    p[0].ml = d[0] & 15;
    p[1].ml = d[1] & 15;
    p[0].kl = (d[2] >> 6) & 3;
    p[1].kl = (d[3] >> 6) & 3;
    p[0].tl = d[2] & 63;
    p[0].fb = d[3] & 7;
    p[0].ws = (d[3] >> 3) & 1;
    p[1].ws = (d[3] >> 4) & 1;
    p[0].ar = (d[4] >> 4) & 15;
    p[1].ar = (d[5] >> 4) & 15;
    p[0].dr = d[4] & 15;
    p[1].dr = d[5] & 15;
    p[0].sl = (d[6] >> 4) & 15;
    p[1].sl = (d[7] >> 4) & 15;
    p[0].rr = d[6] & 15;
    p[1].rr = d[7] & 15;
    p
}

// The null patch: index 38.
const NULL_PATCH: usize = 38;

#[derive(Clone, Copy)]
struct Slot {
    // 0 modulator, 1 carrier, +2 a single-slot rhythm sound.
    ty: u8,
    patch: usize,
    output: [i32; 2],
    half: bool,
    pg_phase: u32,
    pg_out: u32,
    pg_keep: bool,
    blk_fnum: u32,
    fnum: u32,
    blk: u32,
    eg_state: u8,
    volume: u32,
    key_flag: bool,
    sus_flag: bool,
    tll: u32,
    rks: u32,
    eg_rate_h: u32,
    eg_rate_l: u32,
    eg_shift: u32,
    eg_out: u32,
    update_requests: u32,
}

impl Slot {
    fn new(number: usize) -> Slot {
        Slot {
            ty: (number % 2) as u8,
            patch: NULL_PATCH,
            output: [0; 2],
            half: false,
            pg_phase: 0,
            pg_out: 0,
            pg_keep: false,
            blk_fnum: 0,
            fnum: 0,
            blk: 0,
            eg_state: RELEASE,
            volume: 0,
            key_flag: false,
            sus_flag: false,
            tll: 0,
            rks: 0,
            eg_rate_h: 0,
            eg_rate_l: 0,
            eg_shift: 0,
            eg_out: EG_MUTE,
            update_requests: 0,
        }
    }
}

#[derive(Clone)]
struct RateConv {
    sinc: Vec<i16>,
    buf: [i16; LW * 2],
    pos: usize,
}

impl RateConv {
    fn new(f_inp: f64, f_out: f64) -> RateConv {
        let pi = std::f64::consts::PI;
        let ratio = f_inp / f_out;
        let blackman = |x: f64| 0.42 - 0.5 * (2.0 * pi * x).cos() + 0.08 * (4.0 * pi * x).cos();
        let sinc = |x: f64| if x == 0.0 { 1.0 } else { (pi * x).sin() / (pi * x) };
        let windowed = |x: f64| blackman(0.5 + 0.5 * x / (LW / 2) as f64) * sinc(x);
        let mut table = vec![0i16; (SINC_RESO + 1) * LW];
        for (i, t) in table.iter_mut().enumerate() {
            let x = ((i % LW) as f64 - (LW / 2 - 1) as f64 - (i / LW) as f64 / SINC_RESO as f64).abs();
            *t = if f_out < f_inp {
                ((1 << SINC_AMP_BITS) as f64 * windowed(x / ratio) / ratio) as i16
            } else {
                ((1 << SINC_AMP_BITS) as f64 * windowed(x)) as i16
            };
        }
        RateConv {
            sinc: table,
            buf: [0; LW * 2],
            pos: 0,
        }
    }

    fn put(&mut self, data: i16) {
        self.buf[self.pos] = data;
        self.buf[self.pos + LW] = data;
        self.pos = if self.pos + 1 == LW { 0 } else { self.pos + 1 };
    }

    fn get(&self, p: u32) -> i16 {
        let buf = &self.buf[self.pos..self.pos + LW];
        let coef = &self.sinc[p as usize * LW..p as usize * LW + LW];
        let sum: i32 = buf.iter().zip(coef).map(|(&b, &c)| i32::from(b) * i32::from(c)).sum();
        (sum >> SINC_AMP_BITS) as i16
    }
}

#[derive(Clone)]
pub struct Opll {
    inp_step: u32,
    out_step: u32,
    out_time: u32,
    inp_recip: u32,
    inp_shift: u32,
    reg: [u8; 0x40],
    test_flag: u8,
    slot_key_status: u32,
    rhythm_mode: bool,
    eg_counter: u32,
    pm_phase: u32,
    am_phase: i32,
    lfo_am: u8,
    noise: u32,
    short_noise: bool,
    patch_number: [usize; 9],
    /// A VRC7: its own voices, and no rhythm mode.
    vrc7: bool,
    slot: [Slot; 18],
    patch: [Patch; 39],
    /// 0-8 the channels, 9 BD, 10 HH, 11 SD, 12 TOM, 13 CYM.
    ch_out: [i16; 14],
    mix_out: i16,
    conv: Option<RateConv>,
    // Each sound's converter, for calc_channels.
    chconv: Vec<RateConv>,
}

impl Opll {
    pub fn new(clock: u32, rate: u32) -> Opll {
        let t = tables();
        let mut o = Opll {
            inp_step: 0,
            out_step: 0,
            out_time: 0,
            inp_recip: 0,
            inp_shift: 0,
            reg: [0; 0x40],
            test_flag: 0,
            slot_key_status: 0,
            rhythm_mode: false,
            eg_counter: 0,
            pm_phase: 0,
            am_phase: 0,
            lfo_am: 0,
            noise: 1,
            short_noise: false,
            patch_number: [0; 9],
            vrc7: false,
            slot: std::array::from_fn(Slot::new),
            patch: [Patch::default(); 39],
            ch_out: [0; 14],
            mix_out: 0,
            conv: None,
            chconv: Vec::new(),
        };
        // The rate converter, unless the rate is the chip's own.
        let (f_out, f_inp) = (f64::from(rate), f64::from(clock) / 72.0);
        o.out_step = clock;
        o.inp_step = rate * 72;
        o.inp_shift = 32;
        while ((1u64 << (o.inp_shift + 1)) - 1) / u64::from(o.inp_step) < 0xffff_ffff {
            o.inp_shift += 1;
        }
        o.inp_recip = (((1u64 << o.inp_shift) - 1) / u64::from(o.inp_step) + 1) as u32;
        if f_inp.floor() != f_out && (f_inp + 0.5).floor() != f_out {
            o.conv = Some(RateConv::new(f_inp, f_out));
        }
        for i in 0..9 {
            o.set_patch(i, 0);
        }
        for r in 0..0x40 {
            o.write(r, 0);
        }
        o.patch[..38].copy_from_slice(&t.default_patch);
        o
    }

    /// The chip as Konami's VRC7: its fifteen voices, and no rhythm mode.
    pub fn set_vrc7(&mut self) {
        self.vrc7 = true;
        for j in 0..19 {
            let p = dump_to_patch(&VRC7_INST[j * 8..j * 8 + 8]);
            self.patch[j * 2] = p[0];
            self.patch[j * 2 + 1] = p[1];
        }
    }

    fn get_parameter_rate(&self, s: usize) -> u32 {
        let slot = &self.slot[s];
        if slot.ty & 1 == 0 && !slot.key_flag {
            return 0;
        }
        let p = &self.patch[slot.patch];
        match slot.eg_state {
            ATTACK => p.ar,
            DECAY => p.dr,
            SUSTAIN => {
                if p.eg != 0 {
                    0
                } else {
                    p.rr
                }
            }
            RELEASE => {
                if slot.sus_flag {
                    5
                } else if p.eg != 0 {
                    p.rr
                } else {
                    7
                }
            }
            DAMP => DAMPER_RATE,
            _ => 0,
        }
    }

    fn commit_slot_update(&mut self, s: usize) {
        let t = tables();
        let req = self.slot[s].update_requests;
        let p = self.patch[self.slot[s].patch];
        if req & UPDATE_WS != 0 {
            self.slot[s].half = p.ws != 0;
        }
        if req & UPDATE_TLL != 0 {
            let slot = &mut self.slot[s];
            let tl = if slot.ty & 1 == 0 { p.tl } else { slot.volume };
            slot.tll = u32::from(t.tll[(slot.blk_fnum >> 5) as usize][p.kl as usize]) + (tl << 1);
        }
        if req & UPDATE_RKS != 0 {
            let slot = &mut self.slot[s];
            slot.rks = t.rks[(slot.blk_fnum >> 8) as usize][p.kr as usize];
        }
        if req & (UPDATE_RKS | UPDATE_EG) != 0 {
            let p_rate = self.get_parameter_rate(s);
            let slot = &mut self.slot[s];
            if p_rate == 0 {
                slot.eg_shift = 0;
                slot.eg_rate_h = 0;
                slot.eg_rate_l = 0;
            } else {
                slot.eg_rate_h = 15.min(p_rate + (slot.rks >> 2));
                slot.eg_rate_l = slot.rks & 3;
                slot.eg_shift = if slot.eg_state == ATTACK {
                    if 0 < slot.eg_rate_h && slot.eg_rate_h < 12 {
                        13 - slot.eg_rate_h
                    } else {
                        0
                    }
                } else if slot.eg_rate_h < 13 {
                    13 - slot.eg_rate_h
                } else {
                    0
                };
            }
        }
        self.slot[s].update_requests = 0;
    }

    fn slot_on(&mut self, i: usize) {
        let slot = &mut self.slot[i];
        slot.key_flag = true;
        slot.eg_state = DAMP;
        slot.update_requests |= UPDATE_EG;
    }

    fn slot_off(&mut self, i: usize) {
        let slot = &mut self.slot[i];
        slot.key_flag = false;
        if slot.ty & 1 != 0 {
            slot.eg_state = RELEASE;
            slot.update_requests |= UPDATE_EG;
        }
    }

    fn update_key_status(&mut self) {
        let r14 = self.reg[0x0e];
        let mut new = 0u32;
        for ch in 0..9 {
            if self.reg[0x20 + ch] & 0x10 != 0 {
                new |= 3 << (ch * 2);
            }
        }
        if r14 & 0x20 != 0 {
            if r14 & 0x10 != 0 {
                new |= 3 << SLOT_BD1;
            }
            if r14 & 0x01 != 0 {
                new |= 1 << SLOT_HH;
            }
            if r14 & 0x08 != 0 {
                new |= 1 << SLOT_SD;
            }
            if r14 & 0x04 != 0 {
                new |= 1 << SLOT_TOM;
            }
            if r14 & 0x02 != 0 {
                new |= 1 << SLOT_CYM;
            }
        }
        let updated = self.slot_key_status ^ new;
        for i in 0..18 {
            if updated >> i & 1 != 0 {
                if new >> i & 1 != 0 {
                    self.slot_on(i);
                } else {
                    self.slot_off(i);
                }
            }
        }
        self.slot_key_status = new;
    }

    fn set_patch(&mut self, ch: usize, num: usize) {
        self.patch_number[ch] = num;
        self.slot[ch * 2].patch = num * 2;
        self.slot[ch * 2 + 1].patch = num * 2 + 1;
        self.slot[ch * 2].update_requests |= UPDATE_ALL;
        self.slot[ch * 2 + 1].update_requests |= UPDATE_ALL;
    }

    fn set_sus_flag(&mut self, ch: usize, flag: bool) {
        self.slot[ch * 2 + 1].sus_flag = flag;
        self.slot[ch * 2 + 1].update_requests |= UPDATE_EG;
        if self.slot[ch * 2].ty & 1 != 0 {
            self.slot[ch * 2].sus_flag = flag;
            self.slot[ch * 2].update_requests |= UPDATE_EG;
        }
    }

    fn set_volume(&mut self, ch: usize, volume: u32) {
        self.slot[ch * 2 + 1].volume = volume;
        self.slot[ch * 2 + 1].update_requests |= UPDATE_TLL;
    }

    fn set_slot_volume(&mut self, s: usize, volume: u32) {
        self.slot[s].volume = volume;
        self.slot[s].update_requests |= UPDATE_TLL;
    }

    fn set_fnumber(&mut self, ch: usize, fnum: u32) {
        for s in [ch * 2 + 1, ch * 2] {
            let slot = &mut self.slot[s];
            slot.fnum = fnum;
            slot.blk_fnum = (slot.blk_fnum & 0xe00) | (fnum & 0x1ff);
            slot.update_requests |= UPDATE_EG | UPDATE_RKS | UPDATE_TLL;
        }
    }

    fn set_block(&mut self, ch: usize, blk: u32) {
        for s in [ch * 2 + 1, ch * 2] {
            let slot = &mut self.slot[s];
            slot.blk = blk;
            slot.blk_fnum = ((blk & 7) << 9) | (slot.blk_fnum & 0x1ff);
            slot.update_requests |= UPDATE_EG | UPDATE_RKS | UPDATE_TLL;
        }
    }

    fn update_rhythm_mode(&mut self) {
        let new = (self.reg[0x0e] >> 5) & 1 != 0;
        if self.rhythm_mode != new {
            if new {
                self.slot[SLOT_HH].ty = 3;
                self.slot[SLOT_HH].pg_keep = true;
                self.slot[SLOT_SD].ty = 3;
                self.slot[SLOT_TOM].ty = 3;
                self.slot[SLOT_CYM].ty = 3;
                self.slot[SLOT_CYM].pg_keep = true;
                self.set_patch(6, 16);
                self.set_patch(7, 17);
                self.set_patch(8, 18);
                self.set_slot_volume(SLOT_HH, u32::from((self.reg[0x37] >> 4) & 15) << 2);
                self.set_slot_volume(SLOT_TOM, u32::from((self.reg[0x38] >> 4) & 15) << 2);
            } else {
                self.slot[SLOT_HH].ty = 0;
                self.slot[SLOT_HH].pg_keep = false;
                self.slot[SLOT_SD].ty = 1;
                self.slot[SLOT_TOM].ty = 0;
                self.slot[SLOT_CYM].ty = 1;
                self.slot[SLOT_CYM].pg_keep = false;
                self.set_patch(6, usize::from(self.reg[0x36] >> 4));
                self.set_patch(7, usize::from(self.reg[0x37] >> 4));
                self.set_patch(8, usize::from(self.reg[0x38] >> 4));
            }
        }
        self.rhythm_mode = new;
    }

    fn update_ampm(&mut self) {
        if self.test_flag & 2 != 0 {
            self.pm_phase = 0;
            self.am_phase = 0;
        } else {
            self.pm_phase = self.pm_phase.wrapping_add(if self.test_flag & 8 != 0 { 1024 } else { 1 });
            self.am_phase = self.am_phase.wrapping_add(if self.test_flag & 8 != 0 { 64 } else { 1 });
        }
        self.lfo_am = AM_TABLE[((self.am_phase >> 6) as usize) % AM_TABLE.len()];
    }

    fn update_noise(&mut self, cycle: u32) {
        for _ in 0..cycle {
            if self.noise & 1 != 0 {
                self.noise ^= 0x800200;
            }
            self.noise >>= 1;
        }
    }

    fn update_short_noise(&mut self) {
        let hh = self.slot[SLOT_HH].pg_out;
        let cym = self.slot[SLOT_CYM].pg_out;
        let bit = |x: u32, b: u32| (x >> b) & 1;
        let h2 = bit(hh, PG_BITS - 8);
        let h7 = bit(hh, PG_BITS - 3);
        let h3 = bit(hh, PG_BITS - 7);
        let c3 = bit(cym, PG_BITS - 7);
        let c5 = bit(cym, PG_BITS - 5);
        self.short_noise = ((h2 ^ h7) | (h3 ^ c5) | (c3 ^ c5)) != 0;
    }

    fn calc_phase(&mut self, s: usize, reset: bool) {
        let pm_phase = self.pm_phase;
        let p = self.patch[self.slot[s].patch];
        let slot = &mut self.slot[s];
        let pm = if p.pm != 0 {
            i32::from(PM_TABLE[((slot.fnum >> 6) & 7) as usize][((pm_phase >> 10) & 7) as usize])
        } else {
            0
        };
        if reset {
            slot.pg_phase = 0;
        }
        let inc = ((((slot.fnum & 0x1ff) as i32 * 2 + pm) * ML_TABLE[p.ml as usize] as i32) << slot.blk) >> 2;
        slot.pg_phase = slot.pg_phase.wrapping_add(inc as u32) & (DP_WIDTH - 1);
        slot.pg_out = slot.pg_phase >> DP_BASE_BITS;
    }

    fn lookup_attack_step(slot: &Slot, counter: u32) -> u32 {
        let l = slot.eg_rate_l as usize;
        match slot.eg_rate_h {
            12 => 4 - u32::from(EG_STEP_TABLES[l][((counter & 0xc) >> 1) as usize]),
            13 => 3 - u32::from(EG_STEP_TABLES[l][((counter & 0xc) >> 1) as usize]),
            14 => 2 - u32::from(EG_STEP_TABLES[l][((counter & 0xc) >> 1) as usize]),
            0 | 15 => 0,
            _ => {
                if EG_STEP_TABLES[l][((counter >> slot.eg_shift) & 7) as usize] != 0 {
                    4
                } else {
                    0
                }
            }
        }
    }

    fn lookup_decay_step(slot: &Slot, counter: u32) -> u32 {
        let l = slot.eg_rate_l as usize;
        match slot.eg_rate_h {
            0 => 0,
            13 => u32::from(EG_STEP_TABLES[l][(((counter & 0xc) >> 1) | (counter & 1)) as usize]),
            14 => u32::from(EG_STEP_TABLES[l][((counter & 0xc) >> 1) as usize]) + 1,
            15 => 2,
            _ => u32::from(EG_STEP_TABLES[l][((counter >> slot.eg_shift) & 7) as usize]),
        }
    }

    fn start_envelope(&mut self, s: usize) {
        let ar = self.patch[self.slot[s].patch].ar;
        let slot = &mut self.slot[s];
        if 15.min(ar + (slot.rks >> 2)) == 15 {
            slot.eg_state = DECAY;
            slot.eg_out = 0;
        } else {
            slot.eg_state = ATTACK;
        }
        slot.update_requests |= UPDATE_EG;
    }

    fn calc_envelope(&mut self, s: usize, buddy: Option<usize>, eg_counter: u32, test: bool) {
        {
            let slot = &self.slot[s];
            if !test && (slot.eg_state == SUSTAIN || slot.eg_state == RELEASE) && (slot.eg_out >= EG_MUTE || slot.eg_rate_h == 0) {
                return;
            }
        }
        let mask = (1u32 << self.slot[s].eg_shift) - 1;
        {
            let slot = &mut self.slot[s];
            if slot.eg_state == ATTACK {
                if 0 < slot.eg_out && 0 < slot.eg_rate_h && (eg_counter & mask & !3) == 0 {
                    let st = Self::lookup_attack_step(slot, eg_counter);
                    if st > 0 {
                        slot.eg_out = (slot.eg_out as i32 - (slot.eg_out >> st) as i32 - 1).max(0) as u32;
                    }
                }
            } else if slot.eg_rate_h > 0 && (eg_counter & mask) == 0 {
                slot.eg_out = EG_MUTE.min(slot.eg_out + Self::lookup_decay_step(slot, eg_counter));
            }
        }
        match self.slot[s].eg_state {
            DAMP => {
                if self.slot[s].eg_out >= EG_MAX && (eg_counter & mask) == 0 {
                    self.start_envelope(s);
                    if self.slot[s].ty & 1 != 0 {
                        if !self.slot[s].pg_keep {
                            self.slot[s].pg_phase = 0;
                        }
                        if let Some(b) = buddy {
                            if !self.slot[b].pg_keep {
                                self.slot[b].pg_phase = 0;
                            }
                        }
                    }
                }
            }
            ATTACK => {
                if self.slot[s].eg_out == 0 {
                    self.slot[s].eg_state = DECAY;
                    self.slot[s].update_requests |= UPDATE_EG;
                }
            }
            DECAY => {
                let sl = self.patch[self.slot[s].patch].sl;
                if (self.slot[s].eg_out >> 3) == sl {
                    self.slot[s].eg_state = SUSTAIN;
                    self.slot[s].update_requests |= UPDATE_EG;
                }
            }
            _ => {}
        }
        if test {
            self.slot[s].eg_out = 0;
        }
    }

    fn update_slots(&mut self) {
        self.eg_counter = self.eg_counter.wrapping_add(1);
        let (counter, test, reset) = (self.eg_counter, self.test_flag & 1 != 0, self.test_flag & 4 != 0);
        for i in 0..18 {
            let buddy = match self.slot[i].ty {
                0 => Some(i + 1),
                1 => Some(i - 1),
                _ => None,
            };
            if self.slot[i].update_requests != 0 {
                self.commit_slot_update(i);
            }
            self.calc_envelope(i, buddy, counter, test);
            self.calc_phase(i, reset);
        }
    }

    fn wave(&self, s: usize, i: usize) -> u16 {
        let t = tables();
        if self.slot[s].half {
            t.halfsin[i]
        } else {
            t.fullsin[i]
        }
    }

    fn to_linear(&self, h: u16, s: usize, am: u32) -> i16 {
        let slot = &self.slot[s];
        if slot.eg_out > EG_MAX {
            return 0;
        }
        let att = (EG_MUTE.min(slot.eg_out + slot.tll + am) << 4) as u16;
        let i = h.wrapping_add(att);
        // (The shift is at most 16: h's level and 127 << 4.)
        let t = i32::from(EXP_TABLE[usize::from((i & 0xff) ^ 0xff)] + 1024);
        let res = (t >> ((i & 0x7f00) >> 8)) as i16;
        (if i & 0x8000 != 0 { !res } else { res }) << 1
    }

    fn calc_slot_car(&mut self, ch: usize, fm: i16) -> i16 {
        let s = ch * 2 + 1;
        let am = if self.patch[self.slot[s].patch].am != 0 { u32::from(self.lfo_am) } else { 0 };
        let idx = (self.slot[s].pg_out as i32 + 2 * (i32::from(fm) >> 1)) as usize & (PG_WIDTH - 1);
        let v = self.to_linear(self.wave(s, idx), s, am);
        self.slot[s].output[1] = self.slot[s].output[0];
        self.slot[s].output[0] = i32::from(v);
        v
    }

    fn calc_slot_mod(&mut self, ch: usize) -> i16 {
        let s = ch * 2;
        let p = self.patch[self.slot[s].patch];
        let fm = if p.fb > 0 { ((self.slot[s].output[1] + self.slot[s].output[0]) >> (9 - p.fb)) as i16 } else { 0 };
        let am = if p.am != 0 { u32::from(self.lfo_am) } else { 0 };
        let idx = (self.slot[s].pg_out as i32 + i32::from(fm)) as usize & (PG_WIDTH - 1);
        let v = self.to_linear(self.wave(s, idx), s, am);
        self.slot[s].output[1] = self.slot[s].output[0];
        self.slot[s].output[0] = i32::from(v);
        v
    }

    fn calc_slot_tom(&self) -> i16 {
        let s = 16;
        self.to_linear(self.wave(s, self.slot[s].pg_out as usize), s, 0)
    }

    fn calc_slot_snare(&self) -> i16 {
        let s = 15;
        let phase = if (self.slot[s].pg_out >> (PG_BITS - 2)) & 1 != 0 {
            if self.noise & 1 != 0 {
                0x300
            } else {
                0x200
            }
        } else if self.noise & 1 != 0 {
            0x0
        } else {
            0x100
        };
        self.to_linear(self.wave(s, phase), s, 0)
    }

    fn calc_slot_cym(&self) -> i16 {
        let s = 17;
        let phase = if self.short_noise { 0x300 } else { 0x100 };
        self.to_linear(self.wave(s, phase), s, 0)
    }

    fn calc_slot_hat(&self) -> i16 {
        let s = 14;
        let phase = if self.short_noise {
            if self.noise & 1 != 0 {
                0x2d0
            } else {
                0x234
            }
        } else if self.noise & 1 != 0 {
            0x34
        } else {
            0xd0
        };
        self.to_linear(self.wave(s, phase), s, 0)
    }

    fn update_output(&mut self) {
        self.update_ampm();
        self.update_short_noise();
        self.update_slots();
        let mo = |x: i16| (-(i32::from(x)) >> 1) as i16;
        for i in 0..6 {
            let m = self.calc_slot_mod(i);
            self.ch_out[i] = mo(self.calc_slot_car(i, m));
        }
        if !self.rhythm_mode {
            let m = self.calc_slot_mod(6);
            self.ch_out[6] = mo(self.calc_slot_car(6, m));
        } else {
            let m = self.calc_slot_mod(6);
            self.ch_out[9] = self.calc_slot_car(6, m);
        }
        self.update_noise(14);
        if !self.rhythm_mode {
            let m = self.calc_slot_mod(7);
            self.ch_out[7] = mo(self.calc_slot_car(7, m));
        } else {
            self.ch_out[10] = self.calc_slot_hat();
            self.ch_out[11] = self.calc_slot_snare();
        }
        self.update_noise(2);
        if !self.rhythm_mode {
            let m = self.calc_slot_mod(8);
            self.ch_out[8] = mo(self.calc_slot_car(8, m));
        } else {
            self.ch_out[12] = self.calc_slot_tom();
            self.ch_out[13] = self.calc_slot_cym();
        }
        self.update_noise(2);
    }

    fn conv_phase(&self) -> u32 {
        let d = if self.out_time != 0 { self.inp_step - self.out_time } else { 0 };
        let dn = ((u64::from(d) * u64::from(self.inp_recip)) >> (self.inp_shift - 32)) as u32;
        ((u64::from(dn) + (1u64 << (31 - SINC_RESO_BITS))) >> (32 - SINC_RESO_BITS)) as u32
    }

    /// One output sample.
    pub fn calc(&mut self) -> i16 {
        while self.out_step > self.out_time {
            self.out_time = self.out_time.wrapping_add(self.inp_step);
            self.update_output();
            let out = self.ch_out.iter().fold(0i16, |a, &c| a.wrapping_add(c));
            match self.conv.as_mut() {
                Some(c) => c.put(out),
                None => self.mix_out = out,
            }
        }
        self.out_time = self.out_time.wrapping_sub(self.out_step);
        if let Some(c) = self.conv.as_ref() {
            self.mix_out = c.get(self.conv_phase());
        }
        self.mix_out
    }

    /// One output sample of each sound (0-8 the channels, 9 BD, 10 HH, 11
    /// SD, 12 TOM, 13 CYM), each through its own rate converter: their sum is
    /// what [`calc`](Self::calc) gives, but for rounding.
    pub fn calc_channels(&mut self) -> [i16; 14] {
        if self.chconv.is_empty() {
            if let Some(c) = &self.conv {
                self.chconv = vec![c.clone(); 14];
            }
        }
        while self.out_step > self.out_time {
            self.out_time = self.out_time.wrapping_add(self.inp_step);
            self.update_output();
            for (c, conv) in self.chconv.iter_mut().enumerate() {
                conv.put(self.ch_out[c]);
            }
        }
        self.out_time = self.out_time.wrapping_sub(self.out_step);
        if self.chconv.is_empty() {
            return self.ch_out;
        }
        let p = self.conv_phase();
        std::array::from_fn(|c| self.chconv[c].get(p))
    }

    pub fn write(&mut self, reg: usize, data: u8) {
        if reg >= 0x40 {
            return;
        }
        let reg = if (0x19..=0x1f).contains(&reg) || (0x29..=0x2f).contains(&reg) || (0x39..=0x3f).contains(&reg) {
            reg - 9
        } else {
            reg
        };
        self.reg[reg] = data;
        let d = u32::from(data);
        let user = |o: &mut Opll, slot_of: fn(usize) -> usize, flags: u32| {
            for i in 0..9 {
                if o.patch_number[i] == 0 {
                    o.slot[slot_of(i)].update_requests |= flags;
                }
            }
        };
        let m = |i: usize| i * 2;
        let c = |i: usize| i * 2 + 1;
        match reg {
            0x00 | 0x01 => {
                let p = &mut self.patch[reg];
                p.am = (d >> 7) & 1;
                p.pm = (d >> 6) & 1;
                p.eg = (d >> 5) & 1;
                p.kr = (d >> 4) & 1;
                p.ml = d & 15;
                user(self, if reg == 0 { m } else { c }, UPDATE_RKS | UPDATE_EG);
            }
            0x02 => {
                self.patch[0].kl = (d >> 6) & 3;
                self.patch[0].tl = d & 63;
                user(self, m, UPDATE_TLL);
            }
            0x03 => {
                self.patch[1].kl = (d >> 6) & 3;
                self.patch[1].ws = (d >> 4) & 1;
                self.patch[0].ws = (d >> 3) & 1;
                self.patch[0].fb = d & 7;
                user(self, m, UPDATE_WS);
                user(self, c, UPDATE_WS | UPDATE_TLL);
            }
            0x04 | 0x05 => {
                let p = &mut self.patch[reg - 4];
                p.ar = (d >> 4) & 15;
                p.dr = d & 15;
                user(self, if reg == 4 { m } else { c }, UPDATE_EG);
            }
            0x06 | 0x07 => {
                let p = &mut self.patch[reg - 6];
                p.sl = (d >> 4) & 15;
                p.rr = d & 15;
                user(self, if reg == 6 { m } else { c }, UPDATE_EG);
            }
            0x0e if self.vrc7 => {}
            0x0e => {
                self.update_rhythm_mode();
                self.update_key_status();
            }
            0x0f => self.test_flag = data,
            0x10..=0x18 => {
                let ch = reg - 0x10;
                self.set_fnumber(ch, d + ((u32::from(self.reg[0x20 + ch]) & 1) << 8));
            }
            0x20..=0x28 => {
                let ch = reg - 0x20;
                self.set_fnumber(ch, ((d & 1) << 8) + u32::from(self.reg[0x10 + ch]));
                self.set_block(ch, (d >> 1) & 7);
                self.set_sus_flag(ch, (d >> 5) & 1 != 0);
                self.update_key_status();
            }
            0x30..=0x38 => {
                if self.reg[0x0e] & 32 != 0 && reg >= 0x36 {
                    match reg {
                        0x37 => self.set_slot_volume(14, ((d >> 4) & 15) << 2),
                        0x38 => self.set_slot_volume(16, ((d >> 4) & 15) << 2),
                        _ => {}
                    }
                } else {
                    self.set_patch(reg - 0x30, ((d >> 4) & 15) as usize);
                }
                self.set_volume(reg - 0x30, (d & 15) << 2);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rom_voice_sounds() {
        let mut o = Opll::new(CLOCK, 44100);
        o.write(0x30, 0x10);
        o.write(0x10, 0xac);
        o.write(0x20, 0x16);
        let s: Vec<i16> = (0..4000).map(|_| o.calc()).collect();
        assert!(s.iter().any(|&x| x.abs() > 500));
    }

    /// The same samples as emu2413 itself: OPLL_SCRIPT "frame reg value" lines
    /// (735 samples a frame), OPLL_EXPECT its output, a sample a line.
    #[test]
    #[ignore]
    fn the_same_as_emu2413() {
        let (Ok(script), Ok(expect)) = (std::env::var("OPLL_SCRIPT"), std::env::var("OPLL_EXPECT")) else {
            return;
        };
        let writes: Vec<(usize, usize, u8)> = std::fs::read_to_string(script)
            .unwrap()
            .lines()
            .map(|l| {
                let v: Vec<usize> = l.split(' ').map(|x| x.parse().unwrap()).collect();
                (v[0], v[1], v[2] as u8)
            })
            .collect();
        let want: Vec<i16> = std::fs::read_to_string(expect).unwrap().lines().map(|l| l.parse().unwrap()).collect();
        let mut o = Opll::new(CLOCK, 44100);
        let mut w = writes.iter().peekable();
        for (n, &x) in want.iter().enumerate() {
            if n % 735 == 0 {
                while let Some(&&(f, r, v)) = w.peek() {
                    if f != n / 735 {
                        break;
                    }
                    o.write(r, v);
                    w.next();
                }
            }
            assert_eq!(o.calc(), x, "sample {n}");
        }
    }
}
