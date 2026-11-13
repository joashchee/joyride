// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! SunVox's shared DSP (psynth_dsp.cpp and lib_dsp): the biquad filter,
//! the one-pole parameter smoother, the render buffer's volume, pan and
//! anticlick, the FFT, curves and window functions. Float and double are
//! kept where C keeps them, a promotion at a time.

use std::f64::consts::PI;

use super::net::Smp;

/// g_hsin_tab: half a sine, 0..255, as the engine has it (drawn by hand
/// near its peak, so not computed).
pub(crate) const HSIN_TAB: [u8; 256] = [
    0, 3, 6, 9, 12, 15, 18, 21, 25, 28, 31, 34, 37, 40, 43, 46, 49, 52, 56, 59, 62, 65, 68, 71, 74, 77, 80, 83, 86, 89, 92, 95, 97, 100, 103, 106, 109, 112, 115, 117, 120, 123, 126, 128, 131, 134, 136, 139, 142, 144,
    147, 149, 152, 154, 157, 159, 162, 164, 167, 169, 171, 174, 176, 178, 180, 183, 185, 187, 189, 191, 193, 195, 197, 199, 201, 203, 205, 207, 209, 211, 212, 214, 216, 217, 219, 221, 222, 224, 225, 227, 228, 229, 231,
    232, 233, 235, 236, 237, 238, 239, 240, 242, 243, 243, 244, 245, 246, 247, 248, 249, 249, 250, 251, 251, 252, 252, 253, 253, 254, 254, 254, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
    254, 254, 254, 253, 253, 252, 252, 251, 251, 250, 249, 249, 248, 247, 246, 245, 245, 244, 243, 242, 241, 240, 238, 237, 236, 235, 234, 232, 231, 230, 228, 227, 225, 224, 222, 221, 219, 218, 216, 214, 213, 211, 209,
    207, 205, 203, 201, 200, 198, 196, 194, 191, 189, 187, 185, 183, 181, 179, 176, 174, 172, 169, 167, 165, 162, 160, 157, 155, 152, 150, 147, 145, 142, 139, 137, 134, 131, 129, 126, 123, 120, 118, 115, 112, 109, 106,
    104, 101, 98, 95, 92, 89, 86, 83, 80, 77, 74, 71, 68, 65, 62, 59, 56, 53, 50, 47, 44, 41, 37, 34, 31, 28, 25, 22, 19, 16, 12, 9, 6, 3,
];

// ---------------------------------------------------------------------
// Biquad filter.

pub(crate) const BQ_LPF: u32 = 0;
pub(crate) const BQ_HPF: u32 = 1;
pub(crate) const BQ_BPF_CSG: u32 = 2;
pub(crate) const BQ_BPF_CPG: u32 = 3;
pub(crate) const BQ_NOTCH: u32 = 4;
pub(crate) const BQ_APF: u32 = 5;
pub(crate) const BQ_PEAKING: u32 = 6;
pub(crate) const BQ_LOWSHELF: u32 = 7;
pub(crate) const BQ_HIGHSHELF: u32 = 8;
pub(crate) const BQ_1POLE_LPF: u32 = 9;
pub(crate) const BQ_1POLE_HPF: u32 = 10;

pub(crate) const BQ_Q_IS_Q: u32 = 0;
pub(crate) const BQ_Q_IS_BANDWIDTH: u32 = 1;
pub(crate) const BQ_Q_IS_SHELF_SLOPE: u32 = 2;

pub(crate) const BQ_FLAG_IGNORE_STAGES: u32 = 1 << 0;
pub(crate) const BQ_FLAG_USE_STAGES_FOR_APF: u32 = 1 << 1;

const POLES: usize = 2;
const CHANNELS: usize = 2;
const STAGES: usize = 8;
const BUF_SIZE: usize = 256;

const FTYPE_BITS: u32 = 5;
const QTYPE_OFFSET: u32 = FTYPE_BITS;
const QTYPE_BITS: u32 = 2;
const STAGES_OFFSET: u32 = FTYPE_BITS + QTYPE_BITS;
const STAGES_BITS: u32 = 4;
pub(crate) const BFT_SMOOTH: u32 = 1 << (FTYPE_BITS + QTYPE_BITS + STAGES_BITS);

/// make_biquad_filter_type.
pub(crate) fn bq_type(ftype: u32, qtype: u32, stages: i32, smooth: bool) -> u32 {
    ftype | (qtype << QTYPE_OFFSET) | ((stages as u32) << STAGES_OFFSET) | ((smooth as u32) << (FTYPE_BITS + QTYPE_BITS + STAGES_BITS))
}

fn ftype_of(t: u32) -> u32 {
    t & ((1 << FTYPE_BITS) - 1)
}

fn qtype_of(t: u32) -> u32 {
    (t >> QTYPE_OFFSET) & ((1 << QTYPE_BITS) - 1)
}

fn stages_of(t: u32) -> usize {
    ((t >> STAGES_OFFSET) & ((1 << STAGES_BITS) - 1)) as usize
}

#[derive(Clone)]
struct BqState {
    a: [f64; POLES + 1],
    b: [f64; POLES + 1],
    x: [f64; POLES * CHANNELS * STAGES],
    y: [f64; POLES * CHANNELS * STAGES],
}

impl Default for BqState {
    fn default() -> BqState {
        BqState { a: [0.0; 3], b: [0.0; 3], x: [0.0; POLES * CHANNELS * STAGES], y: [0.0; POLES * CHANNELS * STAGES] }
    }
}

/// biquad_filter.
#[derive(Clone)]
pub(crate) struct Biquad {
    pub typ: u32,
    pub sfreq: i32,
    pub freq: f64,
    pub db_gain: f64,
    pub q: f64,
    pub flags: u32,
    state: BqState,
    interp_state: BqState,
    interp_len: i32,
    interp_ptr: [i32; CHANNELS],
    buf: [f64; BUF_SIZE],
}

impl Biquad {
    /// biquad_filter_new.
    pub fn new(flags: u32) -> Biquad {
        Biquad {
            typ: 1 << STAGES_OFFSET,
            sfreq: 0,
            freq: 0.0,
            db_gain: 0.0,
            q: 0.0,
            flags,
            state: BqState::default(),
            interp_state: BqState::default(),
            interp_len: 0,
            interp_ptr: [0; CHANNELS],
            buf: [0.0; BUF_SIZE],
        }
    }

    fn stages2(&self) -> usize {
        if self.flags & BQ_FLAG_IGNORE_STAGES != 0 {
            1
        } else {
            stages_of(self.typ)
        }
    }

    fn calc_coeffs(&mut self) {
        self.flags &= !BQ_FLAG_IGNORE_STAGES;
        let ftype = ftype_of(self.typ);
        let freq = self.freq;
        let omega = 2.0 * PI * freq / self.sfreq as f64;
        if ftype >= BQ_1POLE_LPF {
            let a1 = (-omega).exp();
            let b0 = 1.0 - a1;
            let s = &mut self.state;
            if ftype == BQ_1POLE_LPF {
                s.b = [b0, 0.0, 0.0];
                s.a[1] = -a1;
                s.a[2] = 0.0;
            } else if ftype == BQ_1POLE_HPF {
                s.b = [1.0 - b0, -a1, 0.0];
                s.a[1] = -a1;
                s.a[2] = 0.0;
            }
            return;
        }
        let mut alpha = 0.0;
        let mut q = self.q;
        let mut db_gain = self.db_gain;
        if q < 0.00001 {
            q = 0.00001;
        }
        let stages = stages_of(self.typ);
        if stages > 1 {
            let mut ignore_after = BQ_APF;
            if self.flags & BQ_FLAG_USE_STAGES_FOR_APF != 0 {
                ignore_after += 1;
            }
            if ftype >= ignore_after {
                self.flags |= BQ_FLAG_IGNORE_STAGES;
            } else if ftype != BQ_BPF_CPG && ftype != BQ_NOTCH {
                // pow( double, float ): the exponent a float division.
                let e = (1.0f32 / stages as f32) as f64;
                if q > 1.0 {
                    q = q.powf(e);
                }
                db_gain = db_gain.powf(e);
            }
        }
        let tsin = omega.sin();
        let tcos = omega.cos();
        match qtype_of(self.typ) {
            BQ_Q_IS_Q => alpha = tsin / (2.0 * q),
            BQ_Q_IS_BANDWIDTH => alpha = tsin * ((2.0f64).ln() / 2.0 * q * omega / tsin).sinh(),
            _ => {}
        }
        let (mut a0, mut a1, mut a2, mut b0, mut b1, mut b2) = (0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64);
        match ftype {
            BQ_LPF => {
                b0 = (1.0 - tcos) / 2.0;
                b1 = 1.0 - tcos;
                b2 = b0;
                a0 = 1.0 + alpha;
                a1 = -2.0 * tcos;
                a2 = 1.0 - alpha;
            }
            BQ_HPF => {
                b0 = (1.0 + tcos) / 2.0;
                b1 = -(1.0 + tcos);
                b2 = b0;
                a0 = 1.0 + alpha;
                a1 = -2.0 * tcos;
                a2 = 1.0 - alpha;
            }
            BQ_BPF_CSG => {
                b0 = tsin / 2.0;
                b1 = 0.0;
                b2 = -b0;
                a0 = 1.0 + alpha;
                a1 = -2.0 * tcos;
                a2 = 1.0 - alpha;
            }
            BQ_BPF_CPG => {
                b0 = alpha;
                b1 = 0.0;
                b2 = -alpha;
                a0 = 1.0 + alpha;
                a1 = -2.0 * tcos;
                a2 = 1.0 - alpha;
            }
            BQ_NOTCH => {
                b0 = 1.0;
                b1 = -2.0 * tcos;
                b2 = 1.0;
                a0 = 1.0 + alpha;
                a1 = -2.0 * tcos;
                a2 = 1.0 - alpha;
            }
            BQ_APF => {
                b0 = 1.0 - alpha;
                b1 = -2.0 * tcos;
                b2 = 1.0 + alpha;
                a0 = 1.0 + alpha;
                a1 = -2.0 * tcos;
                a2 = 1.0 - alpha;
            }
            BQ_PEAKING => {
                let a = 10f64.powf(db_gain / 40.0);
                b0 = 1.0 + alpha * a;
                b1 = -2.0 * tcos;
                b2 = 1.0 - alpha * a;
                a0 = 1.0 + alpha / a;
                a1 = -2.0 * tcos;
                a2 = 1.0 - alpha / a;
            }
            BQ_LOWSHELF => {
                let a = 10f64.powf(db_gain / 40.0);
                if qtype_of(self.typ) == BQ_Q_IS_SHELF_SLOPE {
                    alpha = tsin / 2.0 * ((a + 1.0 / a) * (1.0 / q - 1.0) + 2.0).sqrt();
                }
                b0 = a * ((a + 1.0) - (a - 1.0) * tcos + 2.0 * a.sqrt() * alpha);
                b1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * tcos);
                b2 = a * ((a + 1.0) - (a - 1.0) * tcos - 2.0 * a.sqrt() * alpha);
                a0 = (a + 1.0) + (a - 1.0) * tcos + 2.0 * a.sqrt() * alpha;
                a1 = -2.0 * ((a - 1.0) + (a + 1.0) * tcos);
                a2 = (a + 1.0) + (a - 1.0) * tcos - 2.0 * a.sqrt() * alpha;
            }
            BQ_HIGHSHELF => {
                let a = 10f64.powf(db_gain / 40.0);
                if qtype_of(self.typ) == BQ_Q_IS_SHELF_SLOPE {
                    alpha = tsin / 2.0 * ((a + 1.0 / a) * (1.0 / q - 1.0) + 2.0).sqrt();
                }
                b0 = a * ((a + 1.0) + (a - 1.0) * tcos + 2.0 * a.sqrt() * alpha);
                b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * tcos);
                b2 = a * ((a + 1.0) + (a - 1.0) * tcos - 2.0 * a.sqrt() * alpha);
                a0 = (a + 1.0) - (a - 1.0) * tcos + 2.0 * a.sqrt() * alpha;
                a1 = 2.0 * ((a - 1.0) - (a + 1.0) * tcos);
                a2 = (a + 1.0) - (a - 1.0) * tcos - 2.0 * a.sqrt() * alpha;
            }
            _ => {}
        }
        let a = 1.0 / a0;
        b0 *= a;
        b1 *= a;
        b2 *= a;
        a1 *= a;
        a2 *= a;
        let s = &mut self.state;
        s.b = [b0, b1, b2];
        s.a[1] = a1;
        s.a[2] = a2;
    }

    /// biquad_filter_stop.
    pub fn stop(&mut self) {
        let n = POLES * CHANNELS * self.stages2();
        self.state.x[..n].fill(0.0);
        self.state.y[..n].fill(0.0);
        self.interp_len = 0;
    }

    fn copy_state_to_interp(&mut self) {
        let n = POLES * CHANNELS * self.stages2();
        self.interp_state.a[0] = self.state.a[0];
        self.interp_state.a[1] = self.state.a[1];
        // memcpy( dest->a, src->a, sizeof( dest->a ) * 2 ): a[] and the
        // first three of b[] (the arrays are laid out a, b in the struct).
        self.interp_state.a[2] = self.state.a[2];
        self.interp_state.b = self.state.b;
        self.interp_state.x[..n].copy_from_slice(&self.state.x[..n]);
        self.interp_state.y[..n].copy_from_slice(&self.state.y[..n]);
    }

    fn stop_and_interp(&mut self, interp_len: i32) {
        if interp_len != 0 {
            self.copy_state_to_interp();
        }
        self.stop();
        self.interp_len = interp_len;
        self.interp_ptr = [0; CHANNELS];
    }

    fn interp(&mut self, interp_len: i32) {
        if interp_len != 0 {
            self.copy_state_to_interp();
            self.interp_len = interp_len;
            self.interp_ptr = [0; CHANNELS];
        }
    }

    /// biquad_filter_change: true if the coefficients changed.
    pub fn change(&mut self, interp_len: i32, typ: u32, sfreq: i32, freq: f64, db_gain: f64, q: f64) -> bool {
        let mut recalc = false;
        let mut stop = false;
        let mut interp = false;
        if self.typ != typ {
            self.typ = typ;
            stop = true;
            recalc = true;
        }
        if self.sfreq != sfreq {
            self.sfreq = sfreq;
            stop = true;
            recalc = true;
        }
        let mut freq = freq;
        // LIMIT_NUM( freq, 0, f->sfreq / 2 - 50 ): an int limit.
        let lim = (self.sfreq / 2 - 50) as f64;
        if freq < 0.0 {
            freq = 0.0;
        }
        if freq > lim {
            freq = lim;
        }
        if self.freq != freq {
            if ftype_of(typ) >= BQ_1POLE_LPF {
                if freq < self.freq && freq < 0.1 {
                    stop = true;
                    interp = true;
                }
            } else {
                let mut unstable = false;
                if freq < self.freq {
                    let d = self.freq / freq;
                    if d > 2.0 {
                        unstable = true;
                    }
                    if freq < 40.0 {
                        unstable = true;
                    }
                } else if freq > (self.sfreq / 2 - 400) as f64 {
                    unstable = true;
                }
                if unstable {
                    stop = true;
                    interp = true;
                }
            }
            self.freq = freq;
            recalc = true;
        }
        if self.db_gain != db_gain {
            self.db_gain = db_gain;
            recalc = true;
        }
        if self.q != q {
            self.q = q;
            recalc = true;
        }
        if recalc {
            if stop {
                if interp {
                    self.stop_and_interp(interp_len);
                } else {
                    self.stop();
                }
            } else if self.typ & BFT_SMOOTH != 0 {
                self.interp(interp_len);
            }
            self.calc_coeffs();
        }
        recalc
    }

    /// biquad_filter_run: `len` frames of channel `ch` from `input` to `out`
    /// (which may be the same buffer: pass `None` for `input` then).
    pub fn run(&mut self, ch: usize, input: Option<&[Smp]>, out: &mut [Smp], len: usize) {
        let ftype = ftype_of(self.typ);
        let stages = self.stages2();
        for scnt in 0..2 {
            let mut interp_ptr = 0i32;
            let mut size = len;
            if scnt == 1 {
                interp_ptr = self.interp_ptr[ch];
                if interp_ptr as u32 >= self.interp_len as u32 {
                    break;
                }
                if ((self.interp_len - interp_ptr) as usize) < size {
                    size = (self.interp_len - interp_ptr) as usize;
                }
            }
            let fs = if scnt == 1 { &mut self.interp_state } else { &mut self.state };
            let a1 = fs.a[1];
            let a2 = fs.a[2];
            let b0 = fs.b[0];
            let b1 = fs.b[1];
            let b2 = fs.b[2];
            let buf = &mut self.buf;
            let mut i = 0usize;
            while i < size {
                let mut s = size - i;
                if s > BUF_SIZE {
                    s = BUF_SIZE;
                }
                for (i2, b) in buf.iter_mut().enumerate().take(s) {
                    *b = match input {
                        Some(inp) => inp[i + i2] as f64,
                        None => out[i + i2] as f64,
                    };
                }
                for stage in 0..stages {
                    let ch2 = stage * CHANNELS + ch;
                    let mut x1 = fs.x[POLES * ch2];
                    let mut x2 = fs.x[POLES * ch2 + 1];
                    let mut y1 = fs.y[POLES * ch2];
                    let mut y2 = fs.y[POLES * ch2 + 1];
                    if ftype >= BQ_1POLE_LPF {
                        if ftype == BQ_1POLE_HPF {
                            for b in buf.iter_mut().take(s) {
                                let x0 = *b;
                                let y0 = (1.0 - b0) * x0 - a1 * y1;
                                y1 = y0;
                                *b -= y0;
                            }
                        } else {
                            for b in buf.iter_mut().take(s) {
                                let x0 = *b;
                                let y0 = b0 * x0 - a1 * y1;
                                y1 = y0;
                                *b = y0;
                            }
                        }
                    } else {
                        for b in buf.iter_mut().take(s) {
                            let x0 = *b;
                            let y0 = b0 * x0 + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
                            x2 = x1;
                            x1 = x0;
                            y2 = y1;
                            y1 = y0;
                            *b = y0;
                        }
                    }
                    fs.x[POLES * ch2] = x1;
                    fs.x[POLES * ch2 + 1] = x2;
                    fs.y[POLES * ch2] = y1;
                    fs.y[POLES * ch2 + 1] = y2;
                }
                if scnt == 0 {
                    for i2 in 0..s {
                        out[i] = buf[i2] as f32;
                        i += 1;
                    }
                } else {
                    for b in buf.iter().take(s) {
                        let v1 = out[i];
                        let v2 = *b as f32;
                        let v = (v1 * interp_ptr as f32 + v2 * (self.interp_len - interp_ptr) as f32) / self.interp_len as f32;
                        out[i] = v;
                        i += 1;
                        interp_ptr += 1;
                    }
                }
            }
            if scnt == 1 {
                self.interp_ptr[ch] = interp_ptr;
            }
        }
    }

    /// biquad_filter_freq_response.
    pub fn freq_response(&self, freq: f64) -> f64 {
        let fs = &self.state;
        let w = freq / self.sfreq as f64 * PI * 2.0;
        let (a1, a2, b0, b1, b2) = (fs.a[1], fs.a[2], fs.b[0], fs.b[1], fs.b[2]);
        let mut rv = ((b0 * b0 + b1 * b1 + b2 * b2 + 2.0 * (b0 * b1 + b1 * b2) * w.cos() + 2.0 * (b0 * b2) * (2.0 * w).cos())
            / (1.0 + a1 * a1 + a2 * a2 + 2.0 * (a1 + a1 * a2) * w.cos() + 2.0 * a2 * (2.0 * w).cos()))
        .sqrt();
        let t = rv;
        if self.flags & BQ_FLAG_IGNORE_STAGES == 0 {
            for _ in 1..stages_of(self.typ) {
                rv *= t;
            }
        }
        rv
    }
}

// ---------------------------------------------------------------------
// Parameter smoothing.

/// psmoother_coefs: a one-pole low-pass's.
#[derive(Clone, Copy, Default, Debug)]
pub(crate) struct SmootherCoefs {
    pub a0: f32,
    pub b1: f32,
}

impl SmootherCoefs {
    /// psmoother_init.
    pub fn new(low_pass_filter_freq: f32, sample_rate: f32) -> SmootherCoefs {
        let b1 = (-2.0 * PI * (low_pass_filter_freq / sample_rate) as f64).exp() as f32;
        let a0 = (1.0 - b1 as f64) as f32;
        SmootherCoefs { a0, b1 }
    }
}

/// psmoother: a value low-passed twice.
#[derive(Clone, Copy, Default, Debug)]
pub(crate) struct Smoother {
    pub v: f32,
    pub v2: f32,
}

impl Smoother {
    pub fn reset(&mut self, cur: f32, norm: f32) {
        self.v = cur / norm;
        self.v2 = cur / norm;
    }
    pub fn check(&self, norm_target: f32) -> bool {
        (self.v2 - norm_target).abs() > 1.0 / 32768.0
    }
    pub fn val(&mut self, c: &SmootherCoefs, norm_target: f32) -> f32 {
        self.v = norm_target * c.a0 + self.v * c.b1;
        self.v2 = self.v * c.a0 + self.v2 * c.b1;
        self.v2
    }
}

/// psmoother_target.
pub(crate) fn smoother_target(target: f32, norm: f32) -> f32 {
    target / norm
}

// ---------------------------------------------------------------------
// The render buffer into a module's outputs.

/// psynth_renderbuf_pars.
#[derive(Clone, Copy, Default, Debug)]
pub(crate) struct RenderbufPars {
    pub vol: [Smoother; 2],
    pub last_sample: [Smp; 2],
    pub anticlick_sample: [Smp; 2],
    pub anticlick_counter: i32,
    pub start: bool,
    pub anticlick: bool,
}

/// psynth_renderbuf2output: rendered frames (one buffer, or one a channel)
/// into the outputs at `vol` (32768 = 1) and `pan`, smoothed and
/// anticlicked; the render buffers are read as they were before writing.
#[allow(clippy::too_many_arguments)]
pub(crate) fn renderbuf2output(
    retval: i32,
    outputs: &mut [Vec<Smp>; 2],
    outputs_num: usize,
    out_offset: usize,
    out_frames: usize,
    render_buf1: &[Smp],
    render_buf2: Option<&[Smp]>,
    rendered_frames: usize,
    volume: i32,
    pan: i32,
    p: &mut RenderbufPars,
    coefs: &SmootherCoefs,
    srate: i32,
) -> i32 {
    let norm = 32768.0f32;
    let bufs = [render_buf1, render_buf2.unwrap_or(render_buf1)];
    let mut vol_smoothing = false;
    let empty_buf = retval == 0 || retval == 2;
    const TMP: usize = 64;
    let mut tmp = [0.0f32; TMP];
    let mut anticlick_counter = 0;
    let mut pan = pan;
    for ch in 0..outputs_num {
        let rb = bufs[ch.min(1)];
        anticlick_counter = p.anticlick_counter;
        let mut vol = volume;
        if pan != 32768 {
            if pan < 32768 {
                if ch == 1 {
                    if pan < 0 {
                        pan = 0;
                    }
                    vol = ((volume as i64 * pan as i64) / 32768) as i32;
                }
            } else if ch == 0 {
                if pan > 32768 * 2 {
                    pan = 32768 * 2;
                }
                vol = ((volume as i64 * (32768 - (pan - 32768)) as i64) / 32768) as i32;
            }
        }
        if p.start {
            p.vol[ch].reset(vol as f32, norm);
            anticlick_counter = 0;
        }
        if p.anticlick {
            anticlick_counter = 32768;
            p.anticlick_sample[ch] = p.last_sample[ch];
        }
        let target = smoother_target(vol as f32, norm);
        let out = &mut outputs[ch][out_offset..];
        let mut ptr = 0usize;
        while ptr < rendered_frames {
            let size = (rendered_frames - ptr).min(TMP);
            let sm = p.vol[ch].check(target);
            if !sm && anticlick_counter == 0 {
                break;
            }
            if sm {
                vol_smoothing = true;
                for i in 0..size {
                    let v = p.vol[ch].val(coefs, target);
                    tmp[i] = rb[i + ptr] * v;
                }
            } else if vol as f32 == norm {
                tmp[..size].copy_from_slice(&rb[ptr..ptr + size]);
            } else if vol == 0 {
                tmp[..size].fill(0.0);
            } else {
                let vol2 = vol as f32 / norm;
                for i in 0..size {
                    tmp[i] = rb[i + ptr] * vol2;
                }
            }
            if anticlick_counter != 0 {
                let mut step = 1024;
                if srate != 44100 {
                    step = step * 44100 / srate;
                }
                for t in tmp.iter_mut().take(size) {
                    let v = p.anticlick_sample[ch] * anticlick_counter as f32 / 32768.0;
                    let v2 = *t * (32768 - anticlick_counter) as f32 / 32768.0;
                    *t = v2 + v;
                    anticlick_counter -= step;
                    if anticlick_counter < 0 {
                        anticlick_counter = 0;
                        break;
                    }
                }
            }
            if empty_buf {
                out[ptr..ptr + size].copy_from_slice(&tmp[..size]);
            } else {
                for i in 0..size {
                    out[i + ptr] += tmp[i];
                }
            }
            p.last_sample[ch] = tmp[size - 1];
            ptr += size;
        }
        if ptr < rendered_frames {
            if vol as f32 == norm {
                p.last_sample[ch] = rb[rendered_frames - 1];
                if empty_buf {
                    out[ptr..rendered_frames].copy_from_slice(&rb[ptr..rendered_frames]);
                } else {
                    for i in ptr..rendered_frames {
                        out[i] += rb[i];
                    }
                }
            } else if vol == 0 {
                p.last_sample[ch] = 0.0;
                if empty_buf {
                    out[ptr..rendered_frames].fill(0.0);
                }
            } else {
                let vol2 = vol as f32 / norm;
                p.last_sample[ch] = rb[rendered_frames - 1] * vol2;
                if empty_buf {
                    for i in ptr..rendered_frames {
                        out[i] = rb[i] * vol2;
                    }
                } else {
                    for i in ptr..rendered_frames {
                        out[i] += rb[i] * vol2;
                    }
                }
            }
        }
        if empty_buf {
            out[rendered_frames..out_frames].fill(0.0);
        }
    }
    let mut retval = 1;
    if empty_buf && volume == 0 && !vol_smoothing {
        retval = 2;
    }
    p.start = false;
    p.anticlick = false;
    p.anticlick_counter = anticlick_counter;
    retval
}

// ---------------------------------------------------------------------
// lib_dsp.

/// catmull_rom_spline_interp_int16: x 0..32767 between y1 and y2.
pub(crate) fn catmull_rom_int16(y0: i32, y1: i32, y2: i32, y3: i32, x: i32) -> i32 {
    let x = x >> 2;
    let a = (3 * (y1 - y2) - y0 + y3) / 2;
    let b = 2 * y2 + y0 - (5 * y1 + y3) / 2;
    let c2 = (y2 - y0) / 2;
    ((((((((a * x) >> 13) + b) * x) >> 13) + c2) * x) >> 13) + y1
}

/// catmull_rom_spline_interp_float32: x 0..1 between y1 and y2.
pub(crate) fn catmull_rom_f32(y0: f32, y1: f32, y2: f32, y3: f32, x: f32) -> f32 {
    let a = (3.0 * (y1 - y2) - y0 + y3) / 2.0;
    let b = 2.0 * y2 + y0 - (5.0 * y1 + y3) / 2.0;
    let c2 = (y2 - y0) / 2.0;
    ((a * x + b) * x + c2) * x + y1
}

pub(crate) const CURVE_LINEAR: i32 = 0;
pub(crate) const CURVE_QUADRATIC1: i32 = 1;
pub(crate) const CURVE_QUADRATIC2: i32 = 2;
pub(crate) const CURVE_SPLINE: i32 = 3;
pub(crate) const CURVE_RECTANGULAR: i32 = 4;

/// dsp_curve: 0..32768 through a curve.
pub(crate) fn dsp_curve(x: i32, typ: i32) -> i32 {
    match typ {
        CURVE_QUADRATIC1 => {
            let y2 = 32768 - x;
            32768 - ((y2 * y2) >> 15)
        }
        CURVE_QUADRATIC2 => (x * x) >> 15,
        CURVE_SPLINE => {
            let a = (x as f32 / 32768.0f32) as f64 * PI - PI / 2.0;
            (((a.sin() + 1.0) / 2.0) * 32768.0) as i32
        }
        CURVE_RECTANGULAR => {
            if x < 16384 {
                0
            } else {
                32768
            }
        }
        _ => x,
    }
}

pub(crate) const WIN_RECTANGULAR: i32 = 0;
pub(crate) const WIN_TRIANGULAR: i32 = 1;
pub(crate) const WIN_SINE: i32 = 2;
pub(crate) const WIN_HANN: i32 = 3;
pub(crate) const WIN_BLACKMAN: i32 = 4;
pub(crate) const WIN_NUTTALL: i32 = 5;

/// dsp_window_function: the window into `dest`; its amplitude correction.
pub(crate) fn window_function(dest: &mut [f32], typ: i32) -> f32 {
    let size = dest.len() as i32;
    let size2 = size - 1;
    match typ {
        WIN_TRIANGULAR => {
            for i in 0..size {
                dest[i as usize] = if i < size / 2 { i as f32 / size as f32 * 2.0 } else { ((1.0 - (i as f32 / size as f32) as f64) * 2.0) as f32 };
            }
        }
        WIN_SINE => {
            for i in 0..size {
                dest[i as usize] = ((PI * i as f64 / size2 as f64) as f32).sin();
            }
        }
        WIN_HANN => {
            for i in 0..size {
                let v = ((PI * i as f64 / size2 as f64) as f32).sin();
                dest[i as usize] = v * v;
            }
        }
        WIN_BLACKMAN => {
            for i in 0..size {
                let c1 = ((2.0 * PI * i as f64 / size2 as f64) as f32).cos() as f64;
                let c2 = ((4.0 * PI * i as f64 / size2 as f64) as f32).cos() as f64;
                dest[i as usize] = (0.42 - 0.5 * c1 + 0.08 * c2) as f32;
            }
        }
        WIN_NUTTALL => {
            for i in 0..size {
                let v = ((2.0 * PI * i as f64) / size2 as f64) as f32;
                let r = 0.355768 - 0.487396 * v.cos() as f64 + 0.144232 * (2.0 * v).cos() as f64 - 0.012604 * (3.0 * v).cos() as f64;
                dest[i as usize] = r as f32;
            }
        }
        _ => {}
    }
    let mut sum = 0.0f32;
    for v in dest.iter() {
        sum += *v;
    }
    size as f32 / sum
}

pub(crate) const FFT_FLAG_INVERSE: u32 = 1;

/// fft (float): in place, `fi` imaginary and `fr` real parts.
/// sinf and cosf of one value as the library gets them: clang turns a
/// float sin and cos of the same value into one __sincosf_stret on Apple
/// platforms, which rounds differently from sinf and cosf about one time
/// in forty. (Rust's optimizer merges them only sometimes.)
#[inline]
pub(crate) fn sincosf(x: f32) -> (f32, f32) {
    #[cfg(target_vendor = "apple")]
    {
        #[repr(C)]
        struct Float2 {
            s: f32,
            c: f32,
        }
        extern "C" {
            fn __sincosf_stret(x: f32) -> Float2;
        }
        // SAFETY: a pure function of its argument in Apple's libm.
        let r = unsafe { __sincosf_stret(x) };
        (r.s, r.c)
    }
    #[cfg(not(target_vendor = "apple"))]
    {
        (x.sin(), x.cos())
    }
}

pub(crate) fn fft_f32(flags: u32, fi: &mut [f32], fr: &mut [f32], size: usize) {
    let size2 = size / 2;
    let isign: f32 = if flags & FFT_FLAG_INVERSE != 0 { 1.0 } else { -1.0 };
    let mut j = size2;
    for i in 1..size.saturating_sub(1) {
        if i < j {
            fr.swap(i, j);
            fi.swap(i, j);
        }
        let mut k = size2;
        while k <= j {
            j -= k;
            k >>= 1;
        }
        j += k;
    }
    let mut mmax = 1usize;
    while mmax < size {
        let istep = mmax << 1;
        let theta = (isign as f64 * PI / mmax as f64) as f32;
        let wtemp = (0.5 * theta as f64).sin() as f32;
        let wpr = (-2.0 * wtemp as f64 * wtemp as f64) as f32;
        // sin( theta ) of a float: C++'s float overload, sinf.
        let wpi = theta.sin();
        let mut wr = 1.0f32;
        let mut wi = 0.0f32;
        for m in 0..mmax {
            let mut i = m;
            while i < size {
                let j = i + mmax;
                let tr = wr * fr[j] - wi * fi[j];
                let ti = wr * fi[j] + wi * fr[j];
                fr[j] = fr[i] - tr;
                fi[j] = fi[i] - ti;
                fr[i] += tr;
                fi[i] += ti;
                i += istep;
            }
            let wt = wr;
            wr += wt * wpr - wi * wpi;
            wi += wt * wpi + wi * wpr;
        }
        mmax = istep;
    }
    if flags & FFT_FLAG_INVERSE != 0 {
        let scale = 1.0f32 / size as f32;
        for i in 0..size {
            fr[i] *= scale;
            fi[i] = -fi[i] * scale;
        }
    }
}

/// fft (double).
pub(crate) fn fft_f64(flags: u32, fi: &mut [f64], fr: &mut [f64], size: usize) {
    let size2 = size / 2;
    let isign: f64 = if flags & FFT_FLAG_INVERSE != 0 { 1.0 } else { -1.0 };
    let mut j = size2;
    for i in 1..size.saturating_sub(1) {
        if i < j {
            fr.swap(i, j);
            fi.swap(i, j);
        }
        let mut k = size2;
        while k <= j {
            j -= k;
            k >>= 1;
        }
        j += k;
    }
    let mut mmax = 1usize;
    while mmax < size {
        let istep = mmax << 1;
        let theta = isign * PI / mmax as f64;
        let wtemp = (0.5 * theta).sin();
        let wpr = -2.0 * wtemp * wtemp;
        let wpi = theta.sin();
        let mut wr = 1.0f64;
        let mut wi = 0.0f64;
        for m in 0..mmax {
            let mut i = m;
            while i < size {
                let j = i + mmax;
                let tr = wr * fr[j] - wi * fi[j];
                let ti = wr * fi[j] + wi * fr[j];
                fr[j] = fr[i] - tr;
                fi[j] = fi[i] - ti;
                fr[i] += tr;
                fi[i] += ti;
                i += istep;
            }
            let wt = wr;
            wr += wt * wpr - wi * wpi;
            wi += wt * wpi + wi * wpr;
        }
        mmax = istep;
    }
    if flags & FFT_FLAG_INVERSE != 0 {
        let scale = 1.0 / size as f64;
        for i in 0..size {
            fr[i] *= scale;
            fi[i] = -fi[i] * scale;
        }
    }
}

// ---------------------------------------------------------------------
// The stream resampler (psynth_resampler, the full-quality build's
// spline: two frames after the current one, one before).

const RESAMP_INTERP_AFTER: usize = 2;
const RESAMP_INTERP_BEFORE: usize = 1;
pub(crate) const RESAMP_BUF_TAIL: usize = RESAMP_INTERP_AFTER + RESAMP_INTERP_BEFORE + 1;
pub(crate) const RESAMP_FLAG_MODE1: u32 = 1;
pub(crate) const RESAMP_FLAG_MODE2: u32 = 2;
const RESAMP_FLAG_MODE: u32 = 3;
pub(crate) const RESAMP_FLAG_DONT_RESET_WHEN_EMPTY: u32 = 1 << 4;
const RESAMP_CHANNELS: usize = 2;

/// psynth_resampler, with its own input buffers (the library's are the
/// net's, shared, each use writing all it reads).
#[derive(Default)]
pub(crate) struct Resampler {
    flags: u32,
    pub in_smprate: i32,
    out_smprate: i32,
    pub ratio_fp: u32,
    input_buf_size: i32,
    input_frames: u32,
    input_frames_fp: u32,
    input_ptr_fp: u32,
    input_buf_tail: [f32; RESAMP_BUF_TAIL * RESAMP_CHANNELS],
    input_empty_frames: u32,
    input_empty_frames_max: u32,
    input_frames_avail: i32,
    pub input_delay: i32,
    input_delay_bufs: [Vec<f32>; RESAMP_CHANNELS],
    input_delay_bufs_empty: bool,
    input_offset: i32,
    output_frames: u32,
    state: i8,
    /// The input buffers.
    pub bufs: [Vec<f32>; RESAMP_CHANNELS],
    max_buf_size: i32,
    sampling_freq: i32,
}

impl Resampler {
    pub fn new(max_buf_size: i32, sampling_freq: i32, in_smprate: i32, out_smprate: i32, ratio_fp: u32, flags: u32) -> Resampler {
        let mut r = Resampler { max_buf_size, sampling_freq, ..Resampler::default() };
        r.change(in_smprate, out_smprate, ratio_fp, flags);
        r
    }

    fn mode(&self) -> u32 {
        self.flags & RESAMP_FLAG_MODE
    }

    pub fn change(&mut self, in_smprate: i32, out_smprate: i32, ratio_fp: u32, flags: u32) {
        self.flags = flags;
        self.in_smprate = in_smprate;
        self.out_smprate = out_smprate;
        self.ratio_fp = if ratio_fp != 0 { ratio_fp } else { (in_smprate as i64 * 65536 / out_smprate as i64) as u32 };
        self.input_buf_size = 0;
        self.input_empty_frames_max = RESAMP_BUF_TAIL as u32;
        if self.mode() == RESAMP_FLAG_MODE1 {
            self.input_delay = (((RESAMP_INTERP_AFTER + 1) as i64 * self.ratio_fp as i64) / 65536) as i32 + RESAMP_INTERP_AFTER as i32 + 1;
            self.input_empty_frames_max += self.input_delay as u32;
            // The library compares the new size in bytes with the old in
            // samples, so it remakes the buffers (zeroed) unless the delay
            // shrinks to a quarter or less; otherwise they keep what they had.
            let prev_size = self.input_delay_bufs[0].len();
            let new_size = self.input_delay as usize * 4;
            if new_size > prev_size {
                for b in self.input_delay_bufs.iter_mut() {
                    b.clear();
                    b.resize(self.input_delay as usize, 0.0);
                }
            }
            self.input_delay_bufs_empty = true;
        }
        self.reset();
    }

    /// psynth_resampler_input_buf: an input buffer, made big enough.
    pub fn input_buf(&mut self, ch: usize) -> &mut Vec<f32> {
        if self.input_buf_size == 0 || self.input_buf_size as usize > self.bufs[ch].len() {
            let mut size = ((self.max_buf_size as i64 * self.ratio_fp as i64 * self.out_smprate as i64) / self.sampling_freq as i64 / 65536) as i32 + 4;
            if self.mode() == RESAMP_FLAG_MODE1 {
                size += self.input_delay;
            }
            size += RESAMP_BUF_TAIL as i32 * 2;
            self.input_buf_size = size;
            if self.bufs[ch].len() < size as usize + 32 {
                self.bufs[ch].resize(size as usize + 32, 0.0);
            }
        }
        &mut self.bufs[ch]
    }

    pub fn out_smprate(&self) -> i32 {
        self.out_smprate
    }

    pub fn state(&self) -> i8 {
        self.state
    }

    pub fn output_frames(&self) -> u32 {
        self.output_frames
    }

    pub fn input_buf_offset(&self) -> usize {
        RESAMP_BUF_TAIL + self.input_delay as usize
    }

    pub fn reset(&mut self) {
        self.state = 0;
        self.input_ptr_fp = (RESAMP_BUF_TAIL as u32) << 16;
        if self.mode() == RESAMP_FLAG_MODE2 {
            self.input_ptr_fp = ((RESAMP_INTERP_BEFORE + 1) as u32) << 16;
        }
        self.input_buf_tail = [0.0; RESAMP_BUF_TAIL * RESAMP_CHANNELS];
        self.input_empty_frames = 0;
        if self.mode() == RESAMP_FLAG_MODE1 {
            self.input_offset = 0;
            if !self.input_delay_bufs_empty {
                for b in self.input_delay_bufs.iter_mut() {
                    b.fill(0.0);
                }
                self.input_delay_bufs_empty = true;
            }
        }
    }

    /// psynth_resampler_begin: the input frames to put at input_buf_offset().
    pub fn begin(&mut self, input_frames: i32, output_frames: i32) -> i32 {
        self.input_frames_avail = input_frames;
        if output_frames != 0 {
            self.output_frames = output_frames as u32;
            self.input_frames_fp = (output_frames as u32).wrapping_mul(self.ratio_fp);
            let last = (self.input_ptr_fp.wrapping_add(self.input_frames_fp).wrapping_sub(self.ratio_fp) >> 16) + RESAMP_INTERP_AFTER as u32;
            self.input_frames = (last + 1).wrapping_sub(RESAMP_BUF_TAIL as u32) & 0xFFFF;
        } else {
            self.input_frames = input_frames as u32;
            self.output_frames = 0;
            let l = ((RESAMP_BUF_TAIL as u32 + input_frames as u32).wrapping_sub(RESAMP_INTERP_AFTER as u32)).wrapping_mul(65536).wrapping_sub(1);
            if self.input_ptr_fp <= l {
                self.output_frames = (l - self.input_ptr_fp) / self.ratio_fp + 1;
            }
            self.input_frames_fp = self.output_frames.wrapping_mul(self.ratio_fp);
        }
        self.input_frames as i32
    }

    /// psynth_resampler_end: the input buffers resampled into `outputs`
    /// from `output_offset`; whether anything was written.
    pub fn end(&mut self, input_filled: i32, outputs: &mut [Vec<f32>], output_count: usize, output_offset: usize) -> i32 {
        let mut retval = 0;
        if input_filled == 1 || input_filled == 3 {
            self.state = 1;
        }
        let skip = self.state == -1;
        if self.state != 0 {
            let mut input_offset = 0usize;
            if self.mode() == RESAMP_FLAG_MODE1 {
                if !skip {
                    self.input_delay_bufs_empty = false;
                    let d = self.input_delay as usize;
                    for ch in 0..output_count {
                        let buf = &mut self.bufs[ch];
                        let delay = &mut self.input_delay_bufs[ch];
                        buf[RESAMP_BUF_TAIL..RESAMP_BUF_TAIL + d].copy_from_slice(&delay[..d]);
                        let from = RESAMP_BUF_TAIL + self.input_frames_avail.max(0) as usize;
                        delay[..d].copy_from_slice(&buf[from..from + d]);
                    }
                }
                if self.input_offset < 0 {
                    self.input_offset = 0;
                }
                if self.input_offset > self.input_delay - 1 {
                    self.input_offset = self.input_delay - 1;
                }
                input_offset = self.input_offset as usize;
                self.input_offset += self.input_frames as i32 - self.input_frames_avail;
            }
            if !skip {
                for ch in 0..output_count {
                    let buf = &mut self.bufs[ch][input_offset..];
                    let tail = &mut self.input_buf_tail[RESAMP_BUF_TAIL * ch..RESAMP_BUF_TAIL * (ch + 1)];
                    if input_filled == 0 {
                        for b in buf[RESAMP_BUF_TAIL..self.input_frames as usize + RESAMP_BUF_TAIL].iter_mut() {
                            *b = 0.0;
                        }
                    }
                    buf[..RESAMP_BUF_TAIL].copy_from_slice(tail);
                    let out = &mut outputs[ch][output_offset..];
                    let mut ptr = self.input_ptr_fp;
                    for o in out[..self.output_frames as usize].iter_mut() {
                        let c = ptr & 0xFFFF;
                        let p = (ptr >> 16) as usize;
                        let (v0, v1, v2, v3) = (buf[p - 1], buf[p], buf[p + 1], buf[p + 2]);
                        let mu = c as f32 / 0x10000 as f32;
                        let a = (3.0 * (v1 - v2) - v0 + v3) / 2.0;
                        let b = 2.0 * v2 + v0 - (5.0 * v1 + v3) / 2.0;
                        let c2 = (v2 - v0) / 2.0;
                        *o = (((a * mu) + b) * mu + c2) * mu + v1;
                        ptr = ptr.wrapping_add(self.ratio_fp);
                    }
                    let last = self.input_frames as usize;
                    if last != 0 {
                        tail.copy_from_slice(&buf[last..last + RESAMP_BUF_TAIL]);
                    }
                }
                retval = 1;
            }
            self.input_ptr_fp = self.input_ptr_fp.wrapping_add(self.input_frames_fp).wrapping_sub(self.input_frames << 16);
            self.input_empty_frames = 0;
            if input_filled == 0 || input_filled == 2 {
                self.input_empty_frames += self.input_frames;
                if self.input_empty_frames >= self.input_empty_frames_max {
                    self.input_delay_bufs_empty = true;
                    if self.flags & RESAMP_FLAG_DONT_RESET_WHEN_EMPTY != 0 {
                        self.state = -1;
                    } else {
                        self.reset();
                    }
                }
            }
        }
        retval
    }
}
