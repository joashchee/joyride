// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! FMX (psynths_fm2.cpp): a five-operator FM synth. Each operator a
//! wave (a base wave or the custom one) under an ADSR, modulated by the
//! operators before it (phase, frequency, multiply, add, subtract, min,
//! max, AND, XOR, by magnitude), with self-modulation, feedback and
//! noise, routed by its output mode into later operators or the output;
//! at a sample rate of its own through the stream resampler.

use super::super::dsp::{self, renderbuf2output, Resampler, RenderbufPars, Smoother, SmootherCoefs};
use super::super::net::{self, Cmd, Event, Module, Net};
use super::adsr::{AdsrEnv, CURVE_TYPES, SMOOTH_MODES, SUSTAIN_MODES};
use super::generator::to_int16;

const OUTPUTS: usize = 2;
const MAX_CHANNELS: usize = 32;
const PHASE_BITS: u32 = 8 + 16;
const WAVETABLE_BITS: u32 = 8;
const WAVETABLE_SIZE: usize = 1 << WAVETABLE_BITS;
const WAVETABLE_MASK: u32 = (1 << WAVETABLE_BITS) - 1;
const NUM_OPS: usize = 5;
const OP_BUF_SIZE: usize = 64;
const EMPTY_ID: u32 = !0;

const CHANGED_IOCHANNELS: u32 = 1 << 0;
const CHANGED_RESAMPLER: u32 = 1 << 1;
const CHANGED_PITCH_PARS: u32 = 1 << 2;
const CHANGED_ADSR_SMOOTH: u32 = 1 << 3;
const CHANGED_NOISE_FILTER: u32 = 1 << 4;
const CHANGED_OP1_OFFSET: u32 = 5;
const CHANGED_OP1: u32 = 1 << CHANGED_OP1_OFFSET;
const CHANGED_OP1_VOL_OFFSET: u32 = CHANGED_OP1_OFFSET + NUM_OPS as u32;
const CHANGED_OP1_VOL: u32 = 1 << CHANGED_OP1_VOL_OFFSET;

const NUM_RATES: usize = 7;
const RATES: [i32; NUM_RATES - 1] = [8000, 11025, 16000, 22050, 32000, 44100];

const MOD_PHASE: i32 = 0;
const MOD_FREQ: i32 = 1;
const MOD_MUL: i32 = 2;
const MOD_ADD: i32 = 3;
const MOD_SUB: i32 = 4;
const MOD_MIN: i32 = 5;
const MOD_MAX: i32 = 6;
const MOD_AND: i32 = 7;
const MOD_XOR: i32 = 8;
const MOD_PHASE2: i32 = 9;
const MOD_MINABS: i32 = 10;
const MOD_MAXABS: i32 = 11;
const MOD_TYPES: i32 = 12;

const P_VOLUME: usize = 0;
const P_A: usize = 1;
const P_D: usize = 2;
const P_S: usize = 3;
const P_R: usize = 4;
const P_ACURVE: usize = 5;
const P_DCURVE: usize = 6;
const P_RCURVE: usize = 7;
const P_SUSTAIN: usize = 8;
const P_SUSTAIN_PEDAL: usize = 9;
const P_ENV_SCALING: usize = 10;
const P_VOL_SCALING: usize = 11;
const P_VEL_SENS: usize = 12;
const P_WAVEFORM: usize = 13;
const P_NOISE: usize = 14;
const P_PHASE: usize = 15;
const P_FMUL: usize = 16;
const P_CONST_PITCH: usize = 17;
const P_SELFMOD: usize = 18;
const P_FEEDBACK: usize = 19;
const P_MOD: usize = 20;
const P_MODE: usize = 21;
const PARS: usize = 22;
const OP_CTLS: usize = PARS * NUM_OPS - 1;

/// (min, max, def, normal, show offset, flags, type) of each operator parameter.
const PAR_DESC: [(i32, i32, i32, i32, i32, u32, u8); PARS] = [
    (0, 32768, 32768, -1, 0, 0, 0),
    (0, 10000, 100, -1, 0, 2, 0),
    (0, 10000, 100, -1, 0, 2, 0),
    (0, 32768, 32768 / 2, -1, 0, 0, 0),
    (0, 10000, 100, -1, 0, 2, 0),
    (0, CURVE_TYPES - 1, 0, -1, 0, 0, 1),
    (0, CURVE_TYPES - 1, 0, -1, 0, 0, 1),
    (0, CURVE_TYPES - 1, 0, -1, 0, 0, 1),
    (0, SUSTAIN_MODES - 1, 0, -1, 0, 0, 1),
    (0, 1, 0, -1, 0, 0, 1),
    (0, 256, 128, 128, 128, 0, 0),
    (0, 256, 128, 128, 128, 0, 0),
    (0, 256, 128 + 64, 128 + 64, 128, 0, 0),
    (0, net::BASE_WAVES as i32, 5 + 1, -1, 0, 0, 1),
    (0, 32768, 0, -1, 0, 0, 0),
    (0, 32768, 0, -1, 0, 0, 0),
    (0, 32 * 1000, 1000, 1000, 0, 2, 0),
    (0, 16384, 8192, 8192, 8192, 0, 0),
    (0, 32768, 0, -1, 0, 0, 0),
    (0, 32768, 0, -1, 0, 2, 0),
    (0, MOD_TYPES - 1, 0, -1, 0, 0, 1),
    (0, 15, 0, -1, 0, 0, 1),
];

const OP_CTL: usize = 9;
const C_VOLUME: usize = 0;
const C_PAN: usize = 1;
const C_SRATE: usize = 2;
const C_CHANNELS: usize = 3;
const C_STEREO: usize = 4;
const C_SEND: usize = 5;
const C_CAPTURE: usize = 6;
const C_ADSR_SMOOTH: usize = 7;
const C_NOISE_FILTER: usize = 8;
/// The vars: 0..9 the module's controllers, then the operators' parameters.
const V_ADSR_GAIN: usize = 9;
const V_PARS: usize = 16;

#[derive(Clone, Copy, Default)]
struct Op {
    env: AdsrEnv,
    feedback_val: f32,
    ptr: u32,
    delta: u64,
    noise_v: [i16; 4],
    noise_ptr: u32,
}

#[derive(Clone, Copy, Default)]
struct Chan {
    active: bool,
    on: bool,
    id: u32,
    vel: i32,
    pitch: i32,
    noise_seed: u32,
    ops: [Op; NUM_OPS],
    rb: RenderbufPars,
    local_pan: i32,
}

#[derive(Default)]
pub(crate) struct Fmx {
    channels: Vec<Chan>,
    no_active_channels: bool,
    search_ptr: usize,
    srate: i32,
    resamp: Option<Resampler>,
    use_resampler: bool,
    custom_wave: Vec<f32>,
    in_buf: Vec<f32>,
    cap_buf: Vec<f32>,
    cap_buf_ptr: i32,
    op_buf_bits: u32,
    op_buf: Vec<f32>,
    tmp_buf: Vec<f32>,
    env_buf: Vec<f32>,
    env_buf_frames: usize,
    env_buf_state: i32,
    feedback: [Smoother; NUM_OPS],
    selfmod: [Smoother; NUM_OPS],
    coefs: SmootherCoefs,
    noise_delta: u32,
    changed: u32,
    native_rate: i32,
    render_buf: Vec<f32>,
}

fn par(m: &net::ModCommon, opn: usize, p: usize) -> i32 {
    m.vars.get(V_PARS + opn * PARS + p).copied().unwrap_or(0)
}

fn get_vel2(v: i32, sens: i32) -> i32 {
    let mut v = v * 128;
    match sens {
        0 => v = 32768,
        1 => {
            let mut v2 = 32768 - v;
            v2 = (v2 * v2) >> 15;
            v2 = (v2 * v2) >> 15;
            v = 32768 - v2;
        }
        3 => {
            v = (v * v) >> 15;
            v = (v * v) >> 15;
        }
        4 => {
            v = (v * v) >> 15;
            v = (v * v) >> 15;
            v = (v * v) >> 15;
        }
        _ => {}
    }
    v
}

fn get_vel(v: i32, sens: i32) -> i32 {
    let (mut v, mut sens) = (v, sens);
    if sens < 0 {
        sens = -sens;
        v = 256 - v;
    }
    let s1 = sens / 32;
    let s2 = sens & 31;
    let v1 = get_vel2(v, s1);
    let v2 = get_vel2(v, s1 + 1);
    (v1 * (32 - s2) + v2 * s2) / 32
}

fn get_vol(v: i32, s: i32) -> i32 {
    let mut v = v.clamp(0, 32768);
    let mut s = s;
    if s < 0 {
        v = 32768 - v;
        s = -s;
    }
    let s1 = s / 16;
    let s2 = s & 15;
    let mut v1 = 32768;
    let mut v2 = 32768 - ((v * v) >> 15);
    if s1 > 0 {
        v1 = v2;
        for _ in 1..s1 {
            v1 = (v1 * v1) >> 15;
        }
        v2 = (v1 * v1) >> 15;
    }
    (v1 * (16 - s2) + v2 * s2) / 16
}

#[inline]
fn wave_sample(wave: &[f32], ptr: u32) -> f32 {
    let h = ptr >> (PHASE_BITS - WAVETABLE_BITS);
    let l = ((ptr << (16 - (PHASE_BITS - WAVETABLE_BITS))) & 65535) as f32 / 65536.0;
    wave[(h & WAVETABLE_MASK) as usize] * (1.0 - l) + wave[((h + 1) & WAVETABLE_MASK) as usize] * l
}

#[inline]
fn wave_sample2(wave: &[f32], ptr: u32, selfmod: f32) -> f32 {
    let v = wave_sample(wave, ptr);
    let ptr = ptr.wrapping_add((v * selfmod * (1u32 << (PHASE_BITS + 3)) as f32) as i32 as u32);
    wave_sample(wave, ptr)
}

#[inline]
fn wave_sample3(wave: &[f32], ptr: u32, selfmod: f32, feedback_env: bool, env: f32) -> f32 {
    let mut v = wave_sample(wave, ptr);
    if feedback_env {
        v = v * env / 1.0;
    }
    let ptr = ptr.wrapping_add((v * selfmod * (1u32 << (PHASE_BITS + 3)) as f32) as i32 as u32);
    wave_sample(wave, ptr)
}

impl Fmx {
    fn change_env_volume(&self, m: &net::ModCommon, chan: &mut Chan, opn: usize) {
        let mut vol = par(m, opn, P_VOLUME);
        let vol_scaling = par(m, opn, P_VOL_SCALING) - 128;
        let sens = par(m, opn, P_VEL_SENS) - 128;
        if sens != 0 {
            vol = vol * get_vel(chan.vel, sens) / 32768;
        }
        if vol_scaling != 0 {
            vol = vol * get_vol(net::NOTE0_PITCH - chan.pitch, vol_scaling) / 32768;
        }
        chan.ops[opn].env.ctl_volume = vol * m.vars[V_ADSR_GAIN] / 1000;
    }

    fn change_env_len(&self, m: &net::ModCommon, chan: &mut Chan, opn: usize) {
        let (mut a, mut d, mut r) = (par(m, opn, P_A), par(m, opn, P_D), par(m, opn, P_R));
        let env_scaling = par(m, opn, P_ENV_SCALING) - 128;
        if env_scaling != 0 {
            let v = get_vol(net::NOTE0_PITCH - chan.pitch, env_scaling);
            a = a * v / 32768;
            d = d * v / 32768;
            r = r * v / 32768;
        }
        chan.ops[opn].env.change_adr(a, d, r);
    }

    fn change_env_pars(&self, m: &net::ModCommon, chan: &mut Chan, opn: usize) {
        self.change_env_volume(m, chan, opn);
        self.change_env_len(m, chan, opn);
        let e = &mut chan.ops[opn].env;
        e.ctl_s = par(m, opn, P_S);
        e.ctl_acurve = par(m, opn, P_ACURVE) as i8;
        e.ctl_dcurve = par(m, opn, P_DCURVE) as i8;
        e.ctl_rcurve = par(m, opn, P_RCURVE) as i8;
        e.ctl_sustain = par(m, opn, P_SUSTAIN) as i8;
        e.ctl_sustain_pedal = par(m, opn, P_SUSTAIN_PEDAL) as i8;
    }

    fn channel_on(&mut self, m: &net::ModCommon, c: usize, id: u32, vel: i32) {
        let mut chan = self.channels[c];
        if chan.active {
            let zero_attack = !(0..NUM_OPS).any(|opn| par(m, opn, P_A) != 0 && par(m, opn, P_MODE) != 0);
            if !zero_attack {
                chan.rb.anticlick = true;
            }
        } else {
            for op in chan.ops.iter_mut() {
                op.env.reset();
            }
        }
        chan.active = true;
        chan.on = true;
        chan.id = id;
        chan.vel = vel;
        chan.local_pan = 128;
        chan.rb.start = true;
        for opn in 0..NUM_OPS {
            chan.ops[opn].ptr = (par(m, opn, P_PHASE) as u32) << (PHASE_BITS - 15);
            chan.ops[opn].feedback_val = 0.0;
            self.change_env_pars(m, &mut chan, opn);
            chan.ops[opn].env.start();
        }
        if self.no_active_channels {
            for opn in 0..NUM_OPS {
                self.feedback[opn].reset(par(m, opn, P_FEEDBACK) as f32, 32768.0);
                self.selfmod[opn].reset(par(m, opn, P_SELFMOD) as f32, 32768.0);
            }
        }
        self.channels[c] = chan;
    }

    fn channel_off(chan: &mut Chan) {
        chan.on = false;
        for op in chan.ops.iter_mut() {
            op.env.stop();
        }
    }

    fn recalc_pitch(&self, m: &net::ModCommon, chan: &mut Chan) {
        let pitch = chan.pitch / 4 - 64 * 12 * 3;
        let delta = net::get_delta64_hq(self.srate, net::get_freq(pitch));
        for opn in 0..NUM_OPS {
            let mut delta2 = delta;
            let const_pitch = par(m, opn, P_CONST_PITCH) - PAR_DESC[P_CONST_PITCH].4;
            if const_pitch != 0 {
                delta2 = net::get_delta64_hq(self.srate, net::get_freq(net::NOTE0_PITCH / 4 - const_pitch - 64 * 12 * 3));
            }
            let fmul = par(m, opn, P_FMUL);
            chan.ops[opn].delta = delta2.wrapping_mul(fmul as i64 as u64) / 1000;
        }
    }

    fn reinit_resampler(&mut self, n: usize, net: &Net) {
        let m = &net.mods[n];
        let idx = m.ctl(C_SRATE).clamp(0, NUM_RATES as i32 - 1) as usize;
        let new_srate = if idx == NUM_RATES - 1 { self.native_rate } else { RATES[idx] };
        let mut srate_changed = false;
        if new_srate != self.srate {
            self.srate = new_srate;
            srate_changed = true;
        }
        self.use_resampler = false;
        if self.srate != net.sampling_freq {
            self.use_resampler = true;
            match self.resamp.as_mut() {
                None => {
                    let mut r = Resampler::new(net.max_buf_size, net.sampling_freq, self.srate, net.sampling_freq, 0, 0);
                    for ch in 0..OUTPUTS {
                        r.input_buf(ch);
                    }
                    self.resamp = Some(r);
                }
                Some(r) => {
                    if r.in_smprate != self.srate {
                        r.change(self.srate, net.sampling_freq, 0, 0);
                        for ch in 0..OUTPUTS {
                            r.input_buf(ch);
                        }
                    }
                }
            }
        }
        if srate_changed {
            self.coefs = SmootherCoefs::new(100.0, self.srate as f32);
            if !self.no_active_channels {
                for c in 0..(m.ctl(C_CHANNELS).max(0) as usize).min(MAX_CHANNELS) {
                    if !self.channels[c].active {
                        continue;
                    }
                    let mut chan = self.channels[c];
                    self.recalc_pitch(m, &mut chan);
                    self.channels[c] = chan;
                }
            }
            for chan in self.channels.iter_mut() {
                for op in chan.ops.iter_mut() {
                    op.env.change_srate(self.srate);
                }
            }
        }
    }

    fn handle_changes(&mut self, n: usize, net: &mut Net) {
        if self.changed & CHANGED_IOCHANNELS != 0 {
            let want = if net.mods[n].ctl(C_STEREO) != 0 { OUTPUTS as i32 } else { 1 };
            if net.mods[n].output_channels != want {
                net.set_number_of_outputs(want, n);
            }
        }
        if self.changed & CHANGED_RESAMPLER != 0 {
            self.reinit_resampler(n, net);
        }
        let m = &net.mods[n];
        let ctl_channels = (m.ctl(C_CHANNELS).max(0) as usize).min(MAX_CHANNELS);
        if self.changed & CHANGED_PITCH_PARS != 0 && !self.no_active_channels {
            for c in 0..ctl_channels {
                if self.channels[c].active {
                    let mut chan = self.channels[c];
                    self.recalc_pitch(m, &mut chan);
                    self.channels[c] = chan;
                }
            }
        }
        if self.changed & CHANGED_ADSR_SMOOTH != 0 {
            let s = m.ctl(C_ADSR_SMOOTH) as i8;
            for chan in self.channels.iter_mut() {
                for op in chan.ops.iter_mut() {
                    op.env.ctl_smooth = s;
                }
            }
        }
        if self.changed & CHANGED_NOISE_FILTER != 0 {
            self.noise_delta = 1 << 24;
            let nf = m.ctl(C_NOISE_FILTER);
            if nf < 32768 {
                let mut freq = nf as f32 / 32768.0;
                freq = freq * freq * 44100.0;
                if freq < self.srate as f32 {
                    self.noise_delta = (freq / self.srate as f32 * (1u32 << 24) as f32) as u32;
                    if self.noise_delta == 0 {
                        self.noise_delta = 1;
                    }
                }
            }
        }
        if !self.no_active_channels {
            let all = (1u32 << NUM_OPS) - 1;
            if self.changed & (all << CHANGED_OP1_OFFSET) != 0 {
                let op_bits = (self.changed >> CHANGED_OP1_OFFSET) & all;
                self.changed &= !(op_bits << CHANGED_OP1_VOL_OFFSET);
                for c in 0..ctl_channels {
                    if !self.channels[c].active {
                        continue;
                    }
                    let mut chan = self.channels[c];
                    for opn in 0..NUM_OPS {
                        if self.changed & (CHANGED_OP1 << opn) != 0 {
                            self.change_env_pars(m, &mut chan, opn);
                        }
                    }
                    self.channels[c] = chan;
                }
            }
            if self.changed & (all << CHANGED_OP1_VOL_OFFSET) != 0 {
                for c in 0..ctl_channels {
                    if !self.channels[c].active {
                        continue;
                    }
                    let mut chan = self.channels[c];
                    for opn in 0..NUM_OPS {
                        if self.changed & (CHANGED_OP1_VOL << opn) != 0 {
                            self.change_env_volume(m, &mut chan, opn);
                        }
                    }
                    self.channels[c] = chan;
                }
            }
        }
        self.changed = 0;
    }

    /// fm2_op_gen: operator `opn`'s wave into its buffer (onto what the
    /// operators before it left there).
    fn op_gen(&mut self, m: &net::ModCommon, chan: &mut Chan, opn: usize, frames: usize) {
        let base = net::base_wavetable();
        let waveform = par(m, opn, P_WAVEFORM) - 1;
        let mut mod_type = par(m, opn, P_MOD);
        let mut feedback_env = false;
        if mod_type == MOD_PHASE2 {
            mod_type = MOD_PHASE;
            feedback_env = true;
            if self.env_buf_state == 0 {
                self.env_buf[..frames].fill(0.0);
            }
        }
        let target_feedback = dsp::smoother_target(par(m, opn, P_FEEDBACK) as f32, 32768.0);
        let target_selfmod = dsp::smoother_target(par(m, opn, P_SELFMOD) as f32, 32768.0);
        let feedback_change = self.feedback[opn].check(target_feedback);
        let selfmod_change = self.selfmod[opn].check(target_selfmod);
        let wave: &[f32] = if waveform >= 0 { &base[waveform as usize * net::BASE_WAVE_SIZE..(waveform as usize + 1) * net::BASE_WAVE_SIZE] } else { &self.custom_wave };
        let op = &mut chan.ops[opn];
        let mut ptr = op.ptr;
        let delta = op.delta;
        let delta32 = delta as u32;
        let buf_filled = self.op_buf_bits & (1 << opn) != 0;
        let mut simple = false;
        if !buf_filled && !selfmod_change {
            if mod_type >= MOD_MUL {
                simple = true;
            }
            if target_feedback == 0.0 && !feedback_change {
                simple = true;
            }
        }
        let (buf, rest) = self.op_buf.split_at_mut(0);
        let _ = buf;
        let buf = &mut rest[opn * OP_BUF_SIZE..opn * OP_BUF_SIZE + frames];
        let env = &self.env_buf[..frames];
        let buf2 = &mut self.tmp_buf[..frames];
        if simple {
            if !(mod_type == MOD_MUL || mod_type == MOD_AND || mod_type == MOD_MINABS) {
                if target_selfmod != 0.0 {
                    for (i, b) in buf.iter_mut().enumerate() {
                        *b = wave_sample3(wave, ptr, target_selfmod, feedback_env, env[i]);
                        ptr = ptr.wrapping_add(delta32);
                    }
                } else {
                    for b in buf.iter_mut() {
                        *b = wave_sample(wave, ptr);
                        ptr = ptr.wrapping_add(delta32);
                    }
                }
                match mod_type {
                    MOD_SUB => buf.iter_mut().for_each(|b| *b = -*b),
                    MOD_MIN => buf.iter_mut().for_each(|b| {
                        if *b > 0.0 {
                            *b = 0.0
                        }
                    }),
                    MOD_MAX => buf.iter_mut().for_each(|b| {
                        if *b < 0.0 {
                            *b = 0.0
                        }
                    }),
                    _ => {}
                }
                self.op_buf_bits |= 1 << opn;
            }
        } else {
            if !buf_filled {
                buf.fill(0.0);
                self.op_buf_bits |= 1 << opn;
            }
            let and_xor = |a: f32, b: f32, xor: bool| -> f32 {
                let (i1, i2) = (to_int16(a) as i32, to_int16(b) as i32);
                (if xor { i1 ^ i2 } else { i1 & i2 }) as f32 / 32768.0
            };
            if feedback_change || selfmod_change {
                for i in 0..frames {
                    let feedback = self.feedback[opn].val(&self.coefs, target_feedback);
                    let selfmod = self.selfmod[opn].val(&self.coefs, target_selfmod);
                    let mut v = buf[i];
                    if mod_type < MOD_MUL {
                        v += op.feedback_val * feedback;
                    }
                    let mut ptr2 = ptr;
                    if mod_type == MOD_PHASE {
                        ptr2 = ptr.wrapping_add((v * (1u32 << PHASE_BITS) as f32) as i32 as u32);
                    }
                    let v2 = wave_sample3(wave, ptr2, selfmod, feedback_env, env[i]);
                    op.feedback_val = v2;
                    if feedback_env {
                        op.feedback_val = op.feedback_val * env[i] / 1.0;
                    }
                    match mod_type {
                        MOD_PHASE => {
                            buf[i] = v2;
                            ptr = ptr.wrapping_add(delta32);
                        }
                        MOD_FREQ => {
                            buf[i] = v2;
                            ptr = ptr.wrapping_add((delta as f32 * (v + 1.0) / 1.0) as i32 as u32);
                        }
                        _ => {
                            match mod_type {
                                MOD_MUL => buf[i] = buf[i] * v2 / 1.0,
                                MOD_ADD => buf[i] += v2,
                                MOD_SUB => buf[i] -= v2,
                                MOD_MIN => {
                                    if v2 < buf[i] {
                                        buf[i] = v2
                                    }
                                }
                                MOD_MAX => {
                                    if v2 > buf[i] {
                                        buf[i] = v2
                                    }
                                }
                                MOD_AND => buf[i] = and_xor(buf[i], v2, false),
                                MOD_XOR => buf[i] = and_xor(buf[i], v2, true),
                                MOD_MINABS => {
                                    if v2.abs() < buf[i].abs() {
                                        buf[i] = v2
                                    }
                                }
                                MOD_MAXABS => {
                                    if v2.abs() > buf[i].abs() {
                                        buf[i] = v2
                                    }
                                }
                                _ => {}
                            }
                            ptr = ptr.wrapping_add(delta32);
                        }
                    }
                }
            } else {
                let sample = |p: u32| if target_selfmod != 0.0 { wave_sample2(wave, p, target_selfmod) } else { wave_sample(wave, p) };
                match mod_type {
                    MOD_PHASE => {
                        for i in 0..frames {
                            let mut v = buf[i];
                            v += op.feedback_val * target_feedback;
                            let ptr2 = ptr.wrapping_add((v * (1u32 << PHASE_BITS) as f32) as i32 as u32);
                            v = if target_selfmod != 0.0 { wave_sample3(wave, ptr2, target_selfmod, feedback_env, env[i]) } else { wave_sample(wave, ptr2) };
                            buf[i] = v;
                            if feedback_env {
                                v = v * env[i] / 1.0;
                            }
                            op.feedback_val = v;
                            ptr = ptr.wrapping_add(delta32);
                        }
                    }
                    MOD_FREQ => {
                        for b in buf.iter_mut() {
                            let mut v = *b;
                            v += op.feedback_val * target_feedback;
                            let v2 = sample(ptr);
                            op.feedback_val = v2;
                            *b = v2;
                            ptr = ptr.wrapping_add((delta as f32 * (v + 1.0) / 1.0) as i32 as u32);
                        }
                    }
                    MOD_MUL => {
                        for b in buf.iter_mut() {
                            let v = sample(ptr);
                            *b = *b * v / 1.0;
                            ptr = ptr.wrapping_add(delta32);
                        }
                    }
                    MOD_ADD => {
                        for b in buf.iter_mut() {
                            *b += sample(ptr);
                            ptr = ptr.wrapping_add(delta32);
                        }
                    }
                    _ => {
                        for b in buf2.iter_mut() {
                            *b = sample(ptr);
                            ptr = ptr.wrapping_add(delta32);
                        }
                        for (a, &b) in buf.iter_mut().zip(buf2.iter()) {
                            match mod_type {
                                MOD_SUB => *a -= b,
                                MOD_MIN => {
                                    if b < *a {
                                        *a = b
                                    }
                                }
                                MOD_MAX => {
                                    if b > *a {
                                        *a = b
                                    }
                                }
                                MOD_AND => *a = and_xor(*a, b, false),
                                MOD_XOR => *a = and_xor(*a, b, true),
                                MOD_MINABS => {
                                    if b.abs() < a.abs() {
                                        *a = b
                                    }
                                }
                                MOD_MAXABS => {
                                    if b.abs() > a.abs() {
                                        *a = b
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
        op.ptr = ptr;
        let noise = par(m, opn, P_NOISE);
        if noise != 0 {
            if self.op_buf_bits & (1 << opn) == 0 {
                buf.fill(0.0);
            }
            let nv = noise as f32 / 32768.0;
            if self.noise_delta != 1 << 24 {
                for b in buf.iter_mut() {
                    let p = (op.noise_ptr >> 24) as i32;
                    let y = |k: i32| op.noise_v[(k & 3) as usize] as i32;
                    let rnd = dsp::catmull_rom_int16(y(p), y(p + 1), y(p + 2), y(p + 3), ((op.noise_ptr >> (24 - 15)) & 32767) as i32);
                    *b += rnd as f32 / 32768.0 * nv;
                    op.noise_ptr = op.noise_ptr.wrapping_add(self.noise_delta);
                    let new_p = (op.noise_ptr >> 24) as i32;
                    if new_p != p {
                        op.noise_v[(p & 3) as usize] = net::psynth_rand2(&mut chan.noise_seed) as i16;
                    }
                }
            } else {
                for b in buf.iter_mut() {
                    let rnd = net::psynth_rand2(&mut chan.noise_seed);
                    *b += rnd as f32 / 32768.0 * nv;
                }
            }
            self.op_buf_bits |= 1 << opn;
        }
    }

    /// fm2_op_copy: the operator's buffer to where its mode sends it.
    fn op_copy(&mut self, m: &net::ModCommon, opn: usize, frames: usize, main_out: &mut [f32]) {
        let mode = par(m, opn, P_MODE);
        for b in 0..NUM_OPS - opn {
            let out_opn = NUM_OPS - b;
            if mode & (1 << b) == 0 || self.op_buf_bits & (1 << opn) == 0 {
                continue;
            }
            let src: Vec<f32> = self.op_buf[opn * OP_BUF_SIZE..opn * OP_BUF_SIZE + frames].to_vec();
            let dest: &mut [f32] = if out_opn < NUM_OPS { &mut self.op_buf[out_opn * OP_BUF_SIZE..out_opn * OP_BUF_SIZE + frames] } else { &mut main_out[..frames] };
            if self.op_buf_bits & (1 << out_opn) != 0 {
                for (d, s) in dest.iter_mut().zip(src.iter()) {
                    *d += s;
                }
            } else {
                dest.copy_from_slice(&src);
                self.op_buf_bits |= 1 << out_opn;
            }
        }
    }

    fn render(&mut self, n: usize, net: &mut Net) -> isize {
        if self.changed != 0 {
            self.handle_changes(n, net);
        }
        let max_buf = net.max_buf_size;
        let mut retval: i32 = 0;
        let m = &net.mods[n];
        let (offset, frames) = (m.offset as usize, m.frames as usize);
        let outputs_num = m.output_channels.clamp(0, 2) as usize;
        let capture = m.ctl(C_CAPTURE);
        if capture != 0 {
            if capture == 2 && self.cap_buf_ptr >= max_buf {
                self.cap_buf_ptr = 0;
            }
            if self.cap_buf_ptr < max_buf {
                if self.cap_buf.len() < max_buf as usize {
                    self.cap_buf.resize(max_buf as usize, 0.0);
                }
                for i in 0..frames {
                    self.cap_buf[self.cap_buf_ptr as usize] = m.chin[0].get(offset + i).copied().unwrap_or(0.0);
                    self.cap_buf_ptr += 1;
                    if self.cap_buf_ptr >= max_buf {
                        let mut src_p = 0u32;
                        let src_step = (max_buf as u32 * 32768 / WAVETABLE_SIZE as u32) / 2;
                        let at = |p: u32| -> f32 {
                            let p2 = (p / 32768) as usize;
                            let v0 = self.cap_buf[p2];
                            let v1 = if (p2 as i32) + 1 < max_buf { self.cap_buf[p2 + 1] } else { v0 };
                            let pp = (p & 32767) as f32;
                            (v0 * (32768.0 - pp) + v1 * pp) / 32768.0
                        };
                        let mut w = vec![0.0f32; WAVETABLE_SIZE];
                        for (a, x) in w.iter_mut().enumerate() {
                            *x = at(src_p) * a as f32 / WAVETABLE_SIZE as f32;
                            src_p = src_p.wrapping_add(src_step);
                        }
                        for (a, x) in w.iter_mut().enumerate() {
                            *x += at(src_p) * (WAVETABLE_SIZE - a) as f32 / WAVETABLE_SIZE as f32;
                            src_p = src_p.wrapping_add(src_step);
                        }
                        self.custom_wave = w;
                        if capture == 1 {
                            break;
                        }
                        self.cap_buf_ptr = 0;
                    }
                }
            }
        }
        let mut resamp_frames = frames;
        if self.use_resampler {
            let r = self.resamp.as_mut().unwrap();
            for ch in 0..outputs_num {
                r.input_buf(ch);
            }
            resamp_frames = r.begin(0, frames as i32) as usize;
        }
        let ctl_channels = (m.ctl(C_CHANNELS).max(0) as usize).min(MAX_CHANNELS);
        let mut outs = if self.use_resampler { std::mem::take(&mut self.resamp.as_mut().unwrap().bufs) } else { std::mem::take(&mut net.mods[n].chout) };
        let out_off = if self.use_resampler { self.resamp.as_ref().unwrap().input_buf_offset() } else { offset };
        let m = &net.mods[n];
        if resamp_frames != 0 && !self.no_active_channels {
            let send = m.ctl(C_SEND);
            if send != 0 {
                if self.in_buf.len() < resamp_frames + OP_BUF_SIZE + 8 {
                    self.in_buf.resize(resamp_frames + OP_BUF_SIZE + 8, 0.0);
                }
                if m.in_empty[0] >= (offset + frames) as i32 {
                    self.in_buf[..resamp_frames].fill(0.0);
                } else {
                    let inp = &m.chin[0];
                    let mut in_ptr = 0u32;
                    let in_step = (frames as u32 * 32768) / resamp_frames as u32;
                    for i in 0..resamp_frames {
                        let hh = (in_ptr / 32768) as usize;
                        let ll = (in_ptr & 32767) as f32 / 32768.0;
                        let x = |k: usize| inp.get(offset + k).copied().unwrap_or(0.0);
                        self.in_buf[i] = x(hh) * (1.0 - ll) + x(hh + 1) * ll;
                        in_ptr = in_ptr.wrapping_add(in_step);
                    }
                }
            }
            let tmp_feedback = self.feedback;
            let tmp_selfmod = self.selfmod;
            self.no_active_channels = true;
            if self.render_buf.len() < resamp_frames {
                self.render_buf.resize(resamp_frames, 0.0);
            }
            for c in 0..ctl_channels {
                if !self.channels[c].active {
                    continue;
                }
                self.no_active_channels = false;
                let mut chan = self.channels[c];
                let mut rbuf = std::mem::take(&mut self.render_buf);
                self.feedback = tmp_feedback;
                self.selfmod = tmp_selfmod;
                let mut i = 0usize;
                loop {
                    let size = (resamp_frames - i).min(OP_BUF_SIZE);
                    self.op_buf_bits = 0;
                    if send != 0 {
                        self.op_buf_bits = 1 << (send - 1);
                        let at = (send as usize - 1) * OP_BUF_SIZE;
                        for a in 0..OP_BUF_SIZE {
                            self.op_buf[at + a] = self.in_buf.get(i + a).copied().unwrap_or(0.0);
                        }
                    }
                    let mut main_frames = 0usize;
                    for opn in 0..NUM_OPS {
                        let mode = par(m, opn, P_MODE);
                        if mode == 0 {
                            continue;
                        }
                        // fm2_op_gen_env.
                        self.env_buf_frames = 0;
                        let (st, f) = chan.ops[opn].env.run(&mut self.env_buf[..size]);
                        self.env_buf_state = st;
                        self.env_buf_frames = f;
                        self.op_gen(m, &mut chan, opn, size);
                        // fm2_op_apply_env.
                        if self.env_buf_state != 0 {
                            if self.op_buf_bits & (1 << opn) != 0 {
                                let b = &mut self.op_buf[opn * OP_BUF_SIZE..opn * OP_BUF_SIZE + size];
                                for (x, &e) in b.iter_mut().zip(self.env_buf[..size].iter()) {
                                    *x = *x * e / 1.0;
                                }
                            }
                        } else {
                            self.op_buf_bits &= !(1 << opn);
                        }
                        self.op_copy(m, opn, size, &mut rbuf[i..]);
                        if mode & 1 != 0 && self.env_buf_frames > main_frames {
                            main_frames = self.env_buf_frames;
                        }
                    }
                    if self.op_buf_bits & (1 << NUM_OPS) == 0 {
                        rbuf[i..i + size].fill(0.0);
                    }
                    if main_frames < size {
                        chan.active = false;
                        chan.id = EMPTY_ID;
                        if main_frames == 0 {
                            for opn in 0..NUM_OPS {
                                let tf = dsp::smoother_target(par(m, opn, P_FEEDBACK) as f32, 32768.0);
                                let ts = dsp::smoother_target(par(m, opn, P_SELFMOD) as f32, 32768.0);
                                if self.feedback[opn].check(tf) {
                                    for _ in i + size..resamp_frames {
                                        self.feedback[opn].val(&self.coefs, tf);
                                    }
                                }
                                if self.selfmod[opn].check(ts) {
                                    for _ in i + size..resamp_frames {
                                        self.selfmod[opn].val(&self.coefs, ts);
                                    }
                                }
                            }
                            break;
                        }
                    }
                    i += size;
                    if i >= resamp_frames {
                        break;
                    }
                }
                let pan = (m.ctl(C_PAN) + (chan.local_pan - 128)) * 256;
                retval = renderbuf2output(retval, &mut outs, outputs_num, out_off, resamp_frames, &rbuf[..resamp_frames], None, i, m.ctl(C_VOLUME), pan, &mut chan.rb, &self.coefs, self.srate);
                self.render_buf = rbuf;
                self.channels[c] = chan;
            }
        }
        if self.use_resampler {
            let r = self.resamp.as_mut().unwrap();
            r.bufs = outs;
            let mm = &mut net.mods[n];
            retval = r.end(retval, &mut mm.chout, outputs_num, offset);
        } else {
            net.mods[n].chout = outs;
        }
        retval as isize
    }
}

impl Module for Fmx {
    fn handle(&mut self, n: usize, e: &mut Event, net: &mut Net) -> isize {
        let mut retval: isize = 0;
        match e.cmd {
            Cmd::Init => {
                net.register_ctl(n, 0, 32768, 32768 / 2, 0, C_VOLUME, -1, 0);
                net.register_ctl(n, 0, 256, 128, 0, C_PAN, 128, 0);
                net.set_ctl_show_offset(n, 1, -128);
                net.register_ctl(n, 0, 6, 5, 1, C_SRATE, -1, 1);
                net.register_ctl(n, 1, MAX_CHANNELS as i32, 4, 1, C_CHANNELS, -1, 1);
                net.register_ctl(n, 0, 1, 0, 1, C_STEREO, -1, 1);
                net.register_ctl(n, 0, NUM_OPS as i32, 0, 1, C_SEND, -1, 2);
                net.register_ctl(n, 0, 2, 0, 1, C_CAPTURE, -1, 2);
                net.register_ctl(n, 0, SMOOTH_MODES - 1, 2, 1, C_ADSR_SMOOTH, -1, 3);
                net.register_ctl(n, 0, 32768, 32768, 0, C_NOISE_FILTER, -1, 3);
                for (p, d) in PAR_DESC.iter().enumerate() {
                    let group = if p >= P_WAVEFORM { 5 } else { 4 };
                    for opn in 0..NUM_OPS {
                        let var = V_PARS + opn * PARS + p;
                        let mut def = d.2;
                        let mut max = d.1;
                        if p == P_MODE {
                            max = ((1 << NUM_OPS) - 1) >> opn;
                            def = 0;
                            if opn == NUM_OPS - 2 {
                                def = 2;
                            }
                            if opn == NUM_OPS - 1 {
                                def = 1;
                            }
                            if opn == NUM_OPS - 1 {
                                let m = &mut net.mods[n];
                                if m.vars.len() <= var {
                                    m.vars.resize(var + 1, 0);
                                }
                                m.vars[var] = def;
                                break;
                            }
                        }
                        if p == P_SUSTAIN && opn == NUM_OPS - 1 {
                            def = 1;
                        }
                        let ctl = net.register_ctl(n, d.0, max, def, d.6, var, d.3, group);
                        if d.5 != 0 {
                            net.set_ctl_flags(n, ctl as usize, d.5);
                        }
                        if d.4 != 0 {
                            net.set_ctl_show_offset(n, ctl as usize, -d.4);
                        }
                    }
                }
                net.register_ctl(n, 0, 1000 * 8, 1000, 0, V_ADSR_GAIN, 1000, 3);
                self.channels = vec![Chan::default(); MAX_CHANNELS];
                self.no_active_channels = true;
                self.changed = 0xFFFF_FFFF ^ CHANGED_ADSR_SMOOTH;
                self.custom_wave = vec![0.0; WAVETABLE_SIZE];
                self.op_buf = vec![0.0; OP_BUF_SIZE * NUM_OPS];
                self.tmp_buf = vec![0.0; OP_BUF_SIZE];
                self.env_buf = vec![0.0; OP_BUF_SIZE];
                self.cap_buf_ptr = net.max_buf_size;
                self.native_rate = net.sampling_freq;
                net.temp_buf(0);
                retval = 1;
            }
            Cmd::SetupFinished => {
                self.handle_changes(n, net);
                let id = net.mods[n].id;
                for (c, chan) in self.channels.iter_mut().enumerate() {
                    chan.active = false;
                    chan.id = EMPTY_ID;
                    // stime_ns() + pseudo_random() + mod_num * 3 + c * 7 + mod->id * 3079, the clock at 0.
                    chan.noise_seed = net.globals.pseudo_random().wrapping_add((n as u32).wrapping_mul(3)).wrapping_add(c as u32 * 7).wrapping_add(id.wrapping_mul(3079));
                    for op in chan.ops.iter_mut() {
                        op.env = AdsrEnv::init_filled(op.env, self.srate);
                        for r in 0..4 {
                            op.noise_v[r] = net::psynth_rand2(&mut chan.noise_seed) as i16;
                        }
                    }
                }
                let s = net.mods[n].ctl(C_ADSR_SMOOTH) as i8;
                for chan in self.channels.iter_mut() {
                    for op in chan.ops.iter_mut() {
                        op.env.ctl_smooth = s;
                    }
                }
                if let Some(c) = net.mods[n].chunk(0) {
                    for (i, w) in self.custom_wave.iter_mut().enumerate() {
                        *w = c.data.get(i * 4..i * 4 + 4).map_or(0.0, |b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]));
                    }
                }
                retval = 1;
            }
            Cmd::RenderReplace => retval = self.render(n, net),
            Cmd::NoteOn => {
                let ctl_channels = (net.mods[n].ctl(C_CHANNELS).max(1) as usize).min(MAX_CHANNELS);
                if !self.no_active_channels {
                    if let Some(c) = (0..MAX_CHANNELS).find(|&c| self.channels[c].id == e.id) {
                        Self::channel_off(&mut self.channels[c]);
                        self.channels[c].id = EMPTY_ID;
                    }
                    let mut k = 0;
                    while k < ctl_channels {
                        if !self.channels[self.search_ptr].active {
                            break;
                        }
                        self.search_ptr += 1;
                        if self.search_ptr >= ctl_channels {
                            self.search_ptr = 0;
                        }
                        k += 1;
                    }
                    if k == ctl_channels {
                        self.search_ptr += 1;
                        if self.search_ptr >= ctl_channels {
                            self.search_ptr = 0;
                        }
                        if net.base_host_version >= 0x02010301 {
                            for _ in 0..ctl_channels {
                                if !self.channels[self.search_ptr].on {
                                    break;
                                }
                                self.search_ptr += 1;
                                if self.search_ptr >= ctl_channels {
                                    self.search_ptr = 0;
                                }
                            }
                        }
                    }
                } else {
                    self.search_ptr = 0;
                }
                let c = self.search_ptr;
                let m = &net.mods[n];
                let mut chan = self.channels[c];
                chan.pitch = e.pitch();
                self.recalc_pitch(m, &mut chan);
                self.channels[c] = chan;
                self.channel_on(m, c, e.id, e.velocity() as i32);
                self.no_active_channels = false;
                retval = 1;
            }
            Cmd::SetFreq | Cmd::SetVelocity => {
                if self.no_active_channels {
                    return 0;
                }
                let m = &net.mods[n];
                let ctl_channels = (m.ctl(C_CHANNELS).max(0) as usize).min(MAX_CHANNELS);
                if let Some(c) = (0..ctl_channels).find(|&c| self.channels[c].id == e.id) {
                    let mut chan = self.channels[c];
                    chan.vel = e.velocity() as i32;
                    if e.cmd == Cmd::SetFreq {
                        chan.pitch = e.pitch();
                        self.recalc_pitch(m, &mut chan);
                    }
                    for opn in 0..NUM_OPS {
                        self.change_env_volume(m, &mut chan, opn);
                    }
                    if e.cmd == Cmd::SetFreq {
                        for opn in 0..NUM_OPS {
                            self.change_env_len(m, &mut chan, opn);
                        }
                    }
                    self.channels[c] = chan;
                }
                retval = 1;
            }
            Cmd::SetSampleOffset | Cmd::SetSampleOffset2 => {
                if self.no_active_channels {
                    return 0;
                }
                let ctl_channels = (net.mods[n].ctl(C_CHANNELS).max(0) as usize).min(MAX_CHANNELS);
                for c in 0..ctl_channels {
                    if self.channels[c].id == e.id {
                        for op in self.channels[c].ops.iter_mut() {
                            op.ptr = e.sample_offset().wrapping_shl(PHASE_BITS - 15);
                        }
                    }
                }
                retval = 1;
            }
            Cmd::NoteOff => {
                if self.no_active_channels {
                    return 0;
                }
                if let Some(c) = (0..MAX_CHANNELS).find(|&c| self.channels[c].id == e.id) {
                    Self::channel_off(&mut self.channels[c]);
                }
                retval = 1;
            }
            Cmd::AllNotesOff => {
                if self.no_active_channels {
                    return 0;
                }
                for chan in self.channels.iter_mut() {
                    Self::channel_off(chan);
                    chan.id = EMPTY_ID;
                }
                retval = 1;
            }
            Cmd::Clean => {
                for chan in self.channels.iter_mut() {
                    chan.active = false;
                    chan.id = EMPTY_ID;
                }
                self.no_active_channels = true;
                if let Some(r) = self.resamp.as_mut() {
                    r.reset();
                }
                retval = 1;
            }
            Cmd::SetLocalController | Cmd::SetGlobalController => {
                if e.cmd == Cmd::SetLocalController && !self.no_active_channels {
                    let ctl_channels = (net.mods[n].ctl(C_CHANNELS).max(0) as usize).min(MAX_CHANNELS);
                    if let Some(c) = (0..ctl_channels).find(|&c| self.channels[c].id == e.id) {
                        if e.ctl_num() == 1 {
                            self.channels[c].local_pan = (e.ctl_val() >> 7) as i32;
                            return 1;
                        }
                    }
                }
                let num = e.ctl_num() as usize;
                if (OP_CTL..OP_CTL + OP_CTLS).contains(&num) {
                    let c = num - OP_CTL;
                    let opn = c % NUM_OPS;
                    let p = c / NUM_OPS;
                    if p <= P_SUSTAIN_PEDAL {
                        self.changed |= CHANGED_OP1 << opn;
                    } else {
                        match p {
                            P_ENV_SCALING => self.changed |= CHANGED_OP1 << opn,
                            P_VOL_SCALING | P_VEL_SENS => self.changed |= CHANGED_OP1_VOL << opn,
                            P_PHASE => {
                                if !self.no_active_channels {
                                    let m = &net.mods[n];
                                    let ctl_channels = (m.ctl(C_CHANNELS).max(0) as usize).min(MAX_CHANNELS);
                                    let diff = e.ctl_val() as i32 - par(m, opn, P_PHASE);
                                    for chan in self.channels[..ctl_channels].iter_mut() {
                                        if chan.active {
                                            chan.ops[opn].ptr = chan.ops[opn].ptr.wrapping_add(diff.wrapping_shl(PHASE_BITS - 15) as u32);
                                        }
                                    }
                                }
                            }
                            P_FMUL | P_CONST_PITCH => self.changed |= CHANGED_PITCH_PARS,
                            _ => {}
                        }
                    }
                } else {
                    match num {
                        2 => self.changed |= CHANGED_RESAMPLER | CHANGED_NOISE_FILTER,
                        4 => self.changed |= CHANGED_IOCHANNELS,
                        6 => {
                            if e.ctl_val() == 0 {
                                self.cap_buf_ptr = net.max_buf_size;
                            } else if self.cap_buf_ptr == net.max_buf_size {
                                self.cap_buf_ptr = 0;
                            }
                        }
                        7 => self.changed |= CHANGED_ADSR_SMOOTH,
                        8 => self.changed |= CHANGED_NOISE_FILTER,
                        x if x == OP_CTL + OP_CTLS => self.changed |= ((1 << NUM_OPS) - 1) << CHANGED_OP1_VOL_OFFSET,
                        _ => {}
                    }
                }
            }
            Cmd::Close => retval = 1,
            _ => {}
        }
        retval
    }
}
