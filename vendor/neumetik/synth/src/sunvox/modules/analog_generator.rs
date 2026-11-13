// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! Analog generator (psynths_generator2.cpp): a virtual analog voice of
//! two oscillators (17 waveforms, band-limited by averaging in HQ, the
//! second detuned and mixed by one of nine operations), noise, a
//! state-variable filter of up to two stages with its own envelope, and
//! an attack and release; all stepped in ticks of 64 frames at 44.1 kHz.

use super::super::dsp::{self, Smoother, SmootherCoefs, HSIN_TAB};
use super::super::net::{self, Cmd, Event, Module, Net};
use super::generator::DRAWN_WAVEFORM;

const SFREQ: i32 = 44100;
const MAX_CHANNELS: usize = 32;
const SUBCHANNELS: usize = 2;
const MAX_PITCH: i32 = 10 * 12 * 256;
const ENV_VOL_PREC: i32 = 10;
const ENV_VOL_MAX: i32 = 32768 << ENV_VOL_PREC;
const TICK_SIZE: i32 = 64;
const FILTER_TICK_SIZE: i32 = 64;
const FILTERS: usize = 2;

const F_ENV_OFF: i32 = 0;
const F_ENV_SUSTAIN_ON: i32 = 2;
const F_ENV_MODES: i32 = 3;

const T_TRI: i32 = 0;
const T_SAW: i32 = 1;
const T_SQ: i32 = 2;
const T_NOISE: i32 = 3;
const T_DRAWN: i32 = 4;
const T_SIN: i32 = 5;
const T_HSIN: i32 = 6;
const T_ASIN: i32 = 7;
const T_DRAWN_SPLINE: i32 = 8;
const T_NOISE_SPLINE: i32 = 9;
const T_WHITE: i32 = 10;
const T_PINK: i32 = 11;
const T_RED: i32 = 12;
const T_BLUE: i32 = 13;
const T_VIOLET: i32 = 14;
const T_GREY: i32 = 15;
const T_HARMONICS: i32 = 16;
const TYPES: i32 = 17;
const MAX_TYPE: i32 = TYPES - 1;

const TYPE_NOISE_COLORED: [bool; TYPES as usize] = [false, false, false, false, false, false, false, false, false, false, true, true, true, true, true, true, false];
const TYPE_SHIFT: [u32; TYPES as usize] = [11, 11, 10, 0, 10, 10, 10, 11, 10, 0, 0, 0, 0, 0, 0, 0, 0];

const MAX_DUTY_CYCLE: i32 = 1024;
const MAX_PITCH2: i32 = 2000;
const MAX_FILTER_TYPE: i32 = 8;
const MAX_FILTER_FREQ: i32 = 14000;
const MAX_FILTER_RES: i32 = 1530;

const MODE_HQ_MONO: i32 = 1;
const MODE_LQ: i32 = 2;
const MODE_LQ_MONO: i32 = 3;
const MODES: i32 = 4;

const C_VOLUME: usize = 0;
const C_TYPE: usize = 1;
const C_PAN: usize = 2;
const C_ATTACK: usize = 3;
const C_RELEASE: usize = 4;
const C_SUSTAIN: usize = 5;
const C_EXP: usize = 6;
const C_DUTY_CYCLE: usize = 7;
const C_PITCH2: usize = 8;
const C_F_TYPE: usize = 9;
const C_F_FREQ: usize = 10;
const C_F_RES: usize = 11;
const C_F_EXP: usize = 12;
const C_F_ATTACK: usize = 13;
const C_F_RELEASE: usize = 14;
const C_F_ENVELOPE_MODE: usize = 15;
const C_CHANNELS: usize = 16;
const C_MODE: usize = 17;
const C_NOISE: usize = 18;
const C_OSC2_VOL: usize = 19;
const C_OSC2_MODE: usize = 20;
const C_OSC2_PHASE: usize = 21;

#[derive(Clone, Copy, Default)]
struct Sub {
    ptr: u64,
    delta: u64,
    pitch: i32,
    target_pitch: i32,
    pitch_delta: i32,
    noise_seed: u32,
    noise_smp: [f32; 5],
}

#[derive(Clone, Copy, Default)]
struct FilterChan {
    d1: f32,
    d2: f32,
    d3: f32,
}

#[derive(Clone, Copy, Default)]
struct Chan {
    playing: bool,
    id: u32,
    vel: i32,
    env_vol: i32,
    sustain: bool,
    tick_counter: i32,
    f_env_vol: i32,
    f_sustain: bool,
    f_tick_counter: i32,
    f_cur_freq: i32,
    f_cur_res: i32,
    f_init_request: bool,
    f: [FilterChan; FILTERS],
    sc: [Sub; SUBCHANNELS],
    last_sample: f32,
    no_last_sample: bool,
    anticlick_counter: i32,
    anticlick_sample: f32,
    vol: [Smoother; SUBCHANNELS],
    osc2_vol: Smoother,
    first_frame: bool,
    local_volume: i32,
    local_type: i32,
    local_pan: i32,
    local_duty_cycle: i32,
    local_f_type: i32,
    local_f_freq: i32,
    local_f_res: i32,
}

/// gen2_options (chunk 1, 15 bytes).
#[derive(Clone, Copy, Default)]
struct Opt {
    volume_env_scaling: bool,
    filter_env_scaling: bool,
    volume_scaling: bool,
    filter_scaling: bool,
    filter_velocity_scaling: bool,
    freq_div: bool,
    no_smooth_freq: bool,
    filter_scaling_reverse: bool,
    retain_phase: bool,
    random_phase: bool,
    filter_pitch: bool,
    filter_res_velocity_scaling: bool,
    fast_zero_attack_release: bool,
    freq_accuracy: bool,
    always_play_osc2: bool,
}

/// The controllers a render reads.
#[derive(Clone, Copy)]
struct Ctls {
    volume: i32,
    typ: i32,
    pan: i32,
    attack: i32,
    release: i32,
    sustain: i32,
    exp: i32,
    duty_cycle: i32,
    pitch2: i32,
    f_type: i32,
    f_freq: i32,
    f_res: i32,
    f_exp: i32,
    f_attack: i32,
    f_release: i32,
    f_envelope_mode: i32,
    mode: i32,
    noise: i32,
    osc2_vol: i32,
    osc2_mode: i32,
    osc2_phase: i32,
}

impl Ctls {
    fn read(m: &net::ModCommon) -> Ctls {
        Ctls {
            volume: m.ctl(C_VOLUME),
            typ: m.ctl(C_TYPE),
            pan: m.ctl(C_PAN),
            attack: m.ctl(C_ATTACK),
            release: m.ctl(C_RELEASE),
            sustain: m.ctl(C_SUSTAIN),
            exp: m.ctl(C_EXP),
            duty_cycle: m.ctl(C_DUTY_CYCLE),
            pitch2: m.ctl(C_PITCH2),
            f_type: m.ctl(C_F_TYPE),
            f_freq: m.ctl(C_F_FREQ),
            f_res: m.ctl(C_F_RES),
            f_exp: m.ctl(C_F_EXP),
            f_attack: m.ctl(C_F_ATTACK),
            f_release: m.ctl(C_F_RELEASE),
            f_envelope_mode: m.ctl(C_F_ENVELOPE_MODE),
            mode: m.ctl(C_MODE),
            noise: m.ctl(C_NOISE),
            osc2_vol: m.ctl(C_OSC2_VOL),
            osc2_mode: m.ctl(C_OSC2_MODE),
            osc2_phase: m.ctl(C_OSC2_PHASE),
        }
    }
}

#[derive(Default)]
pub(crate) struct AnalogGenerator {
    channels: Vec<Chan>,
    no_active_channels: bool,
    drawn_wave: [i8; 32],
    opt: Opt,
    search_ptr: usize,
    noise_seed: u32,
    random_seed: u32,
    coefs: SmootherCoefs,
    add_buf: Vec<f32>,
    buf: Vec<f32>,
}

/// SET_PHASE: C's int shifted, then widened.
fn phase(phase: i32, typ: i32) -> u64 {
    (phase.wrapping_shl(16) >> TYPE_SHIFT[typ as usize]) as i64 as u64
}

fn s16(v: i32) -> f32 {
    v as f32 / 32768.0
}

#[inline]
fn put(buf: &mut [f32], i: usize, v: f32, add: bool) {
    if add {
        buf[i] += v;
    } else {
        buf[i] = v;
    }
}

/// The averaged ramp (HQ triangle and saw) over `p`..`p + size`.
fn ramp_avg(prev_ptr: i32, ptr_offset: i32, tri: bool) -> i32 {
    let mut acc = 0i32;
    let mut p = prev_ptr;
    let mut size = ptr_offset;
    while size > 0 {
        let mut cur_size = ((p as u32 & 0xFFFF_0000).wrapping_add(0x0001_0000)).wrapping_sub(p as u32) as i32;
        if cur_size > size {
            cur_size = size;
        }
        let mut v1 = 32767 - (p & 0xFFFF);
        let mut v2 = 32767 - (p.wrapping_add(cur_size).wrapping_sub(1) & 0xFFFF);
        if tri && p & 0x10000 != 0 {
            v1 = -v1;
            v2 = -v2;
        }
        acc = acc.wrapping_add(cur_size.wrapping_mul(v1 - (v1 - v2) / 2));
        size -= cur_size;
        p = p.wrapping_add(cur_size);
    }
    acc / ptr_offset
}

impl AnalogGenerator {
    fn sub_set_pitch(&self, chan: &mut Chan, sub: usize, pitch: i32) {
        chan.sc[sub].pitch = pitch;
        let mut pitch = pitch / 4;
        if self.opt.freq_div {
            pitch += 64 * 12;
        }
        let freq = net::get_freq(pitch);
        chan.sc[sub].delta = if self.opt.freq_accuracy { net::get_delta64_hq(SFREQ, freq) } else { net::get_delta64(SFREQ, freq) };
    }

    fn set_pitch(&self, chan: &mut Chan, pitch: i32) {
        self.sub_set_pitch(chan, 0, pitch);
        for i in 1..SUBCHANNELS {
            chan.sc[i].pitch = pitch;
            chan.sc[i].delta = chan.sc[0].delta;
        }
    }

    fn set_target_pitch(&self, chan: &mut Chan, pitch: i32) {
        let mut delta = if self.opt.no_smooth_freq { pitch - chan.sc[0].pitch } else { (pitch - chan.sc[0].pitch) / 12 };
        if delta == 0 {
            delta = if pitch > chan.sc[0].pitch { 1 } else { -1 };
        }
        chan.sc[0].target_pitch = pitch;
        chan.sc[0].pitch_delta = delta;
    }

    fn set_offset(&self, chan: &mut Chan, offset: i32, c: &Ctls) {
        if self.opt.retain_phase || self.opt.random_phase {
            return;
        }
        let typ = if chan.local_type >= 0 { chan.local_type } else { c.typ };
        let ptr = phase(offset, typ);
        for sc in chan.sc.iter_mut() {
            sc.ptr = ptr;
        }
        if c.osc2_phase != offset {
            chan.sc[1].ptr = phase((offset + c.osc2_phase) & 32767, typ);
        }
    }

    fn prepare_channel(&mut self, chan: &mut Chan, c: &Ctls) {
        if chan.playing {
            if chan.no_last_sample {
                chan.anticlick_counter = 0;
            } else if self.opt.fast_zero_attack_release && c.attack == 0 {
                chan.no_last_sample = true;
                chan.anticlick_counter = 0;
            } else {
                chan.anticlick_counter = 32768;
                chan.anticlick_sample = chan.last_sample;
            }
        } else {
            chan.no_last_sample = true;
            chan.anticlick_counter = 0;
            chan.first_frame = true;
        }
        for i in 0..SUBCHANNELS {
            let sc = &mut chan.sc[i];
            if !self.opt.retain_phase && !self.opt.random_phase {
                sc.ptr = 0;
                if i == 1 {
                    sc.ptr = phase(c.osc2_phase, c.typ);
                }
            } else if self.opt.random_phase {
                let a = net::pseudo_random(&mut self.random_seed);
                let b = net::pseudo_random(&mut self.random_seed);
                sc.ptr = a.wrapping_add(b << 16) as u64;
            }
            sc.target_pitch = sc.pitch;
            sc.pitch_delta = 0;
            if TYPE_NOISE_COLORED[c.typ as usize] {
                sc.noise_smp = [0.0; 5];
            }
        }
        chan.env_vol = 0;
        chan.sustain = true;
        chan.tick_counter = 0;
        chan.f_env_vol = 0;
        chan.f_sustain = true;
        chan.f_tick_counter = 0;
        chan.f = [FilterChan::default(); FILTERS];
        chan.f_init_request = true;
        chan.playing = true;
        chan.local_volume = 256;
        chan.local_type = -1;
        chan.local_pan = 128;
        chan.local_duty_cycle = -1;
        chan.local_f_type = -1;
        chan.local_f_freq = MAX_FILTER_FREQ;
        chan.local_f_res = MAX_FILTER_RES;
    }

    /// gen2_render_sin: the sine at `vol` (into or onto the buffer); the phase after.
    fn render_sin(mode: i32, vol: f32, buf: &mut [f32], frames: usize, mut ptr: u64, delta: u64, add: bool) -> u64 {
        let vol = if (delta >> 16) >= 16 { 0.0 } else { vol };
        if vol == 0.0 {
            if !add {
                buf[..frames].fill(0.0);
            }
            return ptr.wrapping_add(delta.wrapping_mul(frames as u64));
        }
        let lq = mode > MODE_HQ_MONO;
        for i in 0..frames {
            let mut v = if lq {
                let sin_ptr = ((ptr as u32) >> 12) & 255;
                let v = s16(HSIN_TAB[sin_ptr as usize] as i32 * 128);
                if ptr as u32 & (16 << 16) != 0 {
                    -v
                } else {
                    v
                }
            } else {
                let sin_ptr = (ptr as u32 & ((1 << (16 + 5)) - 1)) as f32 / (65536 * 16) as f32;
                (sin_ptr * std::f32::consts::PI).sin()
            };
            if vol != 1.0 {
                v = v * vol / 1.0;
            }
            put(buf, i, v, add);
            ptr = ptr.wrapping_add(delta);
        }
        ptr
    }

    /// gen2_render_waveform.
    #[allow(clippy::too_many_arguments)]
    fn render_waveform(&self, chan: &mut Chan, sub: usize, no_wave: bool, add: bool, buf: &mut [f32], frames: usize, c: &Ctls) {
        let typ = if chan.local_type >= 0 { chan.local_type } else { c.typ };
        let duty_cycle = if chan.local_duty_cycle >= 0 { chan.local_duty_cycle } else { c.duty_cycle };
        let sc = &mut chan.sc[sub];
        if no_wave {
            if !add {
                buf[..frames].fill(0.0);
            }
            sc.ptr = sc.ptr.wrapping_add(sc.delta.wrapping_mul(frames as u64));
            return;
        }
        let mut wave_ptr = sc.ptr;
        let delta = sc.delta;
        let lq = c.mode > MODE_HQ_MONO;
        let noise = net::noise_table();
        let dw = &self.drawn_wave;
        match typ {
            T_TRI | T_SAW => {
                let shift = if typ == T_TRI { 3 } else { 4 };
                let tri = typ == T_TRI;
                if lq {
                    for i in 0..frames {
                        let ptr = (wave_ptr >> shift) as i32;
                        let mut acc = 32767 - (ptr & 0xFFFF);
                        if tri && ptr & 0x10000 != 0 {
                            acc = -acc;
                        }
                        put(buf, i, s16(acc), add);
                        wave_ptr = wave_ptr.wrapping_add(delta);
                    }
                } else {
                    let mut prev_ptr = (wave_ptr >> shift) as i32;
                    for i in 0..frames {
                        wave_ptr = wave_ptr.wrapping_add(delta);
                        let ptr = (wave_ptr >> shift) as i32;
                        let ptr_offset = ptr.wrapping_sub(prev_ptr);
                        let acc = if ptr_offset > 1 {
                            ramp_avg(prev_ptr, ptr_offset, tri)
                        } else {
                            let mut acc = 32767 - (ptr & 0xFFFF);
                            if tri && ptr & 0x10000 != 0 {
                                acc = -acc;
                            }
                            acc
                        };
                        put(buf, i, s16(acc), add);
                        prev_ptr = ptr;
                    }
                }
            }
            T_SQ => {
                let dc = -((duty_cycle - MAX_DUTY_CYCLE / 2) as f32 * 1.0) / (MAX_DUTY_CYCLE / 2) as f32;
                if lq {
                    for i in 0..frames {
                        let ptr = (wave_ptr >> 5) as i32;
                        let mut v = if (ptr & 0xFFFF) / 64 < duty_cycle { 1.0f32 } else { -1.0 };
                        v += dc;
                        put(buf, i, v, add);
                        wave_ptr = wave_ptr.wrapping_add(delta);
                    }
                } else {
                    let mut prev_ptr = (wave_ptr >> 5) as i32;
                    for i in 0..frames {
                        wave_ptr = wave_ptr.wrapping_add(delta);
                        let ptr = (wave_ptr >> 5) as i32;
                        let ptr_offset = ptr.wrapping_sub(prev_ptr);
                        let mut acc = 0i32;
                        if ptr_offset > 1 {
                            let mut p = prev_ptr;
                            let mut size = ptr_offset;
                            while size > 0 {
                                let (mut cur_size, sign) = if (p & 0xFFFF) / 64 < duty_cycle {
                                    (((p as u32 & 0xFFFF_0000).wrapping_add((duty_cycle * 64) as u32)).wrapping_sub(p as u32) as i32, 1)
                                } else {
                                    (((p as u32 & 0xFFFF_0000).wrapping_add(0x0001_0000)).wrapping_sub(p as u32) as i32, -1)
                                };
                                if cur_size > size {
                                    cur_size = size;
                                }
                                acc = acc.wrapping_add(cur_size.wrapping_mul(32767).wrapping_mul(sign));
                                size -= cur_size;
                                p = p.wrapping_add(cur_size);
                            }
                            acc /= ptr_offset;
                        } else {
                            acc = if (ptr & 0xFFFF) / 64 < duty_cycle { 32767 } else { -32767 };
                        }
                        let mut v = s16(acc);
                        v += dc;
                        put(buf, i, v, add);
                        prev_ptr = ptr;
                    }
                }
            }
            T_NOISE => {
                for i in 0..frames {
                    put(buf, i, s16(noise[((wave_ptr as u32 >> 16) & 32767) as usize] as i32 * 256), add);
                    wave_ptr = wave_ptr.wrapping_add(delta);
                }
            }
            T_NOISE_SPLINE | T_DRAWN_SPLINE => {
                let get = |p: u32| -> i32 {
                    if typ == T_NOISE_SPLINE {
                        noise[(p & 32767) as usize] as i32 * 256
                    } else {
                        dw[(p & 31) as usize] as i32 * 256
                    }
                };
                for i in 0..frames {
                    let ptr = wave_ptr as u32 >> 16;
                    let y = dsp::catmull_rom_int16(get(ptr.wrapping_sub(1)), get(ptr), get(ptr.wrapping_add(1)), get(ptr.wrapping_add(2)), ((wave_ptr as u32 & 65535) >> 1) as i32);
                    put(buf, i, s16(y), add);
                    wave_ptr = wave_ptr.wrapping_add(delta);
                }
            }
            T_DRAWN => {
                if lq {
                    for i in 0..frames {
                        put(buf, i, s16(dw[((wave_ptr as u32 >> 16) & 31) as usize] as i32 * 256), add);
                        wave_ptr = wave_ptr.wrapping_add(delta);
                    }
                } else {
                    let mut prev_ptr = (wave_ptr >> 5) as i32;
                    for i in 0..frames {
                        wave_ptr = wave_ptr.wrapping_add(delta);
                        let ptr = (wave_ptr >> 5) as i32;
                        let ptr_offset = ptr.wrapping_sub(prev_ptr);
                        let v = if ptr_offset > 1 {
                            let mut acc = 0i32;
                            let mut p = prev_ptr;
                            let mut size = ptr_offset;
                            while size > 0 {
                                let mut cur_size = ((p as u32 & 0xFFFF_F800).wrapping_add(0x0001_0000 / 32)).wrapping_sub(p as u32) as i32;
                                if cur_size > size {
                                    cur_size = size;
                                }
                                acc = acc.wrapping_add(cur_size.wrapping_mul(dw[((p & 0xFFFF) >> 11) as usize] as i32 * 256));
                                size -= cur_size;
                                p = p.wrapping_add(cur_size);
                            }
                            acc /= ptr_offset;
                            s16(acc)
                        } else {
                            s16(dw[((ptr & 0xFFFF) >> 11) as usize] as i32 * 256)
                        };
                        put(buf, i, v, add);
                        prev_ptr = ptr;
                    }
                }
            }
            T_SIN => wave_ptr = Self::render_sin(c.mode, 1.0, buf, frames, wave_ptr, delta, add),
            T_HARMONICS => {
                let mut add = add;
                for i in 0..32u64 {
                    let ptr = wave_ptr.wrapping_mul(i + 1);
                    let delta2 = delta.wrapping_mul(i + 1);
                    let mut v = dw[i as usize] as i32;
                    if v == 127 {
                        v = 128;
                    }
                    let mut vol = v as f32 * 1.0 / 128.0;
                    vol = vol * vol / 1.0;
                    vol = vol * vol / 1.0;
                    if v < 0 {
                        vol = -vol;
                    }
                    Self::render_sin(c.mode, vol, buf, frames, ptr, delta2, add);
                    add = true;
                }
                wave_ptr = wave_ptr.wrapping_add(delta.wrapping_mul(frames as u64));
            }
            T_HSIN => {
                for i in 0..frames {
                    let mut v = if wave_ptr as u32 & (16 << 16) != 0 {
                        0.0
                    } else if lq {
                        s16(HSIN_TAB[((wave_ptr as u32 >> 12) & 255) as usize] as i32 * 128)
                    } else {
                        let sin_ptr = (wave_ptr as u32 & ((1 << (16 + 5)) - 1)) as f32 / (65536 * 16) as f32;
                        (sin_ptr * std::f32::consts::PI).sin()
                    };
                    v -= (84.0f32 * 1.0) / 256.0;
                    put(buf, i, v, add);
                    wave_ptr = wave_ptr.wrapping_add(delta);
                }
            }
            T_ASIN => {
                for i in 0..frames {
                    let mut v = if lq {
                        s16(HSIN_TAB[((wave_ptr as u32 >> 12) & 255) as usize] as i32 * 128)
                    } else {
                        let sin_ptr = (wave_ptr as u32 & ((1 << (16 + 4)) - 1)) as f32 / (65536 * 16) as f32;
                        (sin_ptr * std::f32::consts::PI).sin()
                    };
                    v -= (162.0f32 * 1.0) / 256.0;
                    put(buf, i, v, add);
                    wave_ptr = wave_ptr.wrapping_add(delta);
                }
            }
            T_WHITE => {
                for i in 0..frames {
                    let rnd = net::psynth_rand2(&mut sc.noise_seed);
                    put(buf, i, s16(rnd), add);
                }
            }
            T_PINK | T_BLUE => {
                let b0 = 0.99765f64 as f32;
                let b1 = 0.96300f64 as f32;
                let b2 = 0.57000f64 as f32;
                let a0 = 0.0990460f64 as f32;
                let a1 = 0.2965164f64 as f32;
                let a2 = 1.0526913f64 as f32;
                let a3 = 0.1848f64 as f32;
                let n = &mut sc.noise_smp;
                for i in 0..frames {
                    let rnd = net::psynth_rand2(&mut sc.noise_seed);
                    let mut v = s16(rnd);
                    n[0] = b0 * n[0] + v * a0;
                    n[1] = b1 * n[1] + v * a1;
                    n[2] = b2 * n[2] + v * a2;
                    n[3] = n[0] + n[1] + n[2] + v * a3;
                    if typ == T_BLUE {
                        v = (n[3] - n[4]) / 2.0;
                        n[4] = n[3];
                    } else {
                        v = n[3] / 4.0;
                    }
                    put(buf, i, v, add);
                }
            }
            T_RED | T_GREY => {
                let a0 = 4.548848e-03f64 as f32;
                let b1 = -9.954512e-01f64 as f32;
                for i in 0..frames {
                    let rnd = net::psynth_rand2(&mut sc.noise_seed).wrapping_mul(8);
                    let v = s16(rnd);
                    sc.noise_smp[0] = a0 * v - b1 * sc.noise_smp[0];
                    put(buf, i, sc.noise_smp[0], add);
                }
                if typ == T_GREY {
                    for b in buf[..frames].iter_mut() {
                        let rnd = net::psynth_rand2(&mut sc.noise_seed);
                        let v = s16(rnd);
                        let v2 = (v - sc.noise_smp[1]) / 2.0;
                        sc.noise_smp[1] = v;
                        *b += v2;
                    }
                }
            }
            T_VIOLET => {
                for i in 0..frames {
                    let rnd = net::psynth_rand2(&mut sc.noise_seed);
                    let v = s16(rnd);
                    let v2 = (v - sc.noise_smp[0]) / 2.0;
                    sc.noise_smp[0] = v;
                    put(buf, i, v2, add);
                }
            }
            _ => {}
        }
        sc.ptr = wave_ptr;
    }

    fn apply_noise(&mut self, buf: &mut [f32], c: &Ctls) {
        if c.noise == 0 {
            return;
        }
        for b in buf.iter_mut() {
            self.noise_seed = self.noise_seed.wrapping_mul(196314165).wrapping_add(907633515);
            let r = (self.noise_seed as i32) >> 16;
            *b += (((r * c.noise) / 256) as f32 * 1.0) / 32768.0;
        }
    }

    /// gen2_apply_envelope: the filter and its envelope, then the volume envelope.
    fn apply_envelope(&self, chan: &mut Chan, buf: &mut [f32], c: &Ctls, base: u32) {
        let frames = buf.len() as i32;
        let (mut ctl_f_attack, mut ctl_f_release) = (c.f_attack, c.f_release);
        if self.opt.filter_env_scaling {
            let mut s = MAX_PITCH - chan.sc[0].pitch;
            if s > 10000 {
                s -= 10000;
                s >>= 8;
                s = s.clamp(0, 256);
                ctl_f_attack -= (s * (512 - ctl_f_attack)) / 512;
                ctl_f_release -= (s * (512 - ctl_f_release)) / 512;
                ctl_f_attack = ctl_f_attack.max(0);
                ctl_f_release = ctl_f_release.max(0);
            }
        }
        let curve = |v: i32| -> i32 {
            let mut v = 256 - v;
            if v > 192 {
                v = 192 + (v - 192) * 8;
            }
            v * v + 32
        };
        let attack = curve(ctl_f_attack);
        let release = curve(ctl_f_release);
        let wtype = if chan.local_type >= 0 { chan.local_type } else { c.typ };
        let mut typ = if chan.local_f_type >= 0 { chan.local_f_type } else { c.f_type };
        let f_enabled = typ > 0;
        typ -= 1;
        let rolloff_steps = typ / 4 + 1;
        if typ > 3 {
            typ -= 4;
        }
        let mut ctl_f_freq = (chan.local_f_freq * c.f_freq) / MAX_FILTER_FREQ;
        let mut ctl_f_res = (chan.local_f_res * c.f_res) / MAX_FILTER_RES;
        if self.opt.filter_pitch {
            let mut oct_offset = 5;
            if matches!(wtype, T_TRI | T_SAW | T_ASIN) {
                oct_offset -= 1;
            }
            if self.opt.freq_div {
                oct_offset += 1;
            }
            ctl_f_freq = net::get_freq(chan.sc[0].pitch / 4 + 64 * 12 * oct_offset);
            if ctl_f_freq >= MAX_FILTER_FREQ {
                ctl_f_freq = MAX_FILTER_FREQ;
            }
        } else {
            if self.opt.filter_scaling {
                let s = (((chan.sc[0].pitch * 256) / MAX_PITCH) + 16).clamp(0, 256);
                ctl_f_freq = (ctl_f_freq * s) / 256;
            }
            if self.opt.filter_scaling_reverse {
                let s = ((((MAX_PITCH - chan.sc[0].pitch) * 256) / MAX_PITCH) + 16).clamp(0, 256);
                ctl_f_freq = (ctl_f_freq * s) / 256;
            }
        }
        if self.opt.filter_velocity_scaling {
            ctl_f_freq = (ctl_f_freq * chan.vel) / 256;
        }
        if self.opt.filter_res_velocity_scaling {
            ctl_f_res = (ctl_f_res * chan.vel) / 256;
        }
        if chan.f_init_request {
            chan.f_cur_freq = ctl_f_freq * 256;
            chan.f_cur_res = ctl_f_res * 256;
            chan.f_init_request = false;
        }
        if f_enabled {
            let mut sustain = chan.f_sustain;
            let mut env_vol = chan.f_env_vol;
            let fixed_filter = base >= 0x02010300;
            if self.opt.fast_zero_attack_release {
                if c.f_attack == 0 && sustain {
                    env_vol = ENV_VOL_MAX;
                    if c.f_envelope_mode != F_ENV_SUSTAIN_ON {
                        sustain = false;
                    }
                }
                if c.f_release == 0 && !sustain {
                    env_vol = 0;
                }
            }
            let mut i = 0i32;
            let mut f = 0.0f32;
            while i < frames {
                let mut cur_size = FILTER_TICK_SIZE - chan.f_tick_counter;
                if i + cur_size > frames {
                    cur_size = frames - i;
                }
                if chan.f_tick_counter == 0 {
                    if chan.f_cur_freq / 256 > ctl_f_freq {
                        chan.f_cur_freq -= 4 * MAX_FILTER_FREQ;
                        if chan.f_cur_freq / 256 < ctl_f_freq {
                            chan.f_cur_freq = ctl_f_freq * 256;
                        }
                    }
                    if chan.f_cur_freq / 256 < ctl_f_freq {
                        chan.f_cur_freq += 4 * MAX_FILTER_FREQ;
                        if chan.f_cur_freq / 256 > ctl_f_freq {
                            chan.f_cur_freq = ctl_f_freq * 256;
                        }
                    }
                    if chan.f_cur_res / 256 > ctl_f_res {
                        chan.f_cur_res -= 4 * MAX_FILTER_RES;
                        if chan.f_cur_res / 256 < ctl_f_res {
                            chan.f_cur_res = ctl_f_res * 256;
                        }
                    }
                    if chan.f_cur_res / 256 < ctl_f_res {
                        chan.f_cur_res += 4 * MAX_FILTER_RES;
                        if chan.f_cur_res / 256 > ctl_f_res {
                            chan.f_cur_res = ctl_f_res * 256;
                        }
                    }
                    if sustain && env_vol < ENV_VOL_MAX {
                        env_vol += attack * FILTER_TICK_SIZE;
                        if env_vol >= ENV_VOL_MAX {
                            env_vol = ENV_VOL_MAX;
                            if c.f_envelope_mode != F_ENV_SUSTAIN_ON {
                                sustain = false;
                            }
                        }
                    } else if !((sustain && env_vol == ENV_VOL_MAX) || (!sustain && env_vol == 0)) {
                        env_vol -= release * FILTER_TICK_SIZE;
                        if env_vol < 0 {
                            env_vol = 0;
                            sustain = false;
                        }
                    }
                }
                let i1 = i;
                let i2 = i + cur_size;
                let fs = SFREQ;
                let mut f_cur_freq = chan.f_cur_freq / 256;
                if c.f_envelope_mode != F_ENV_OFF {
                    f_cur_freq = (f_cur_freq * (env_vol >> ENV_VOL_PREC)) / 32768;
                }
                if c.f_exp != 0 && !self.opt.filter_pitch {
                    f_cur_freq = (f_cur_freq * f_cur_freq) / MAX_FILTER_FREQ;
                }
                if fixed_filter {
                    let cf = if c.mode >= MODE_LQ {
                        if self.opt.filter_pitch {
                            f_cur_freq *= 2;
                        }
                        ((f_cur_freq * 32768 / 2) / fs) as f32
                    } else {
                        ((f_cur_freq * 32768) / (fs * 2)) as f32
                    };
                    f = (6433.0 * cf) / 32768.0;
                    if f >= 1024.0 {
                        f = 1023.0;
                    }
                }
                for fnum in 0..rolloff_steps.max(0) as usize {
                    let mut res = chan.f_cur_res / 256;
                    i = i1;
                    if (fnum as i32) < rolloff_steps - 1 {
                        res = 0;
                    }
                    let q = (1536 - res) as f32;
                    if !fixed_filter {
                        if c.mode > MODE_HQ_MONO {
                            f_cur_freq /= 2;
                        }
                        let cf = ((f_cur_freq * 32768) / (fs * 2)) as f32;
                        f = (6433.0 * cf) / 32768.0;
                        if f >= 1024.0 {
                            f = 1023.0;
                        }
                    }
                    let fc = &mut chan.f[fnum];
                    let (mut d1, mut d2, mut d3) = (fc.d1, fc.d2, fc.d3);
                    let mut dont_render = false;
                    let (i1u, i2u) = (i as usize, i2 as usize);
                    if f == 0.0 {
                        if typ == 0 {
                            for b in buf[i1u..i2u].iter_mut() {
                                let outp = d2;
                                d2 = (d2 * 255.0) / 256.0;
                                *b = outp;
                            }
                            dont_render = true;
                        }
                        if typ == 2 {
                            for b in buf[i1u..i2u].iter_mut() {
                                let outp = d1;
                                d1 = (d1 * 255.0) / 256.0;
                                *b = outp;
                            }
                            dont_render = true;
                        }
                    }
                    if !dont_render && (0..=3).contains(&typ) {
                        let lq = c.mode > MODE_HQ_MONO;
                        for b in buf[i1u..i2u].iter_mut() {
                            let inp = *b;
                            let mut low = d2 + (f * d1) / 1024.0;
                            let mut high = inp - low - (q * d1) / 1024.0;
                            let mut band = (f * high) / 1024.0 + d1;
                            let outp;
                            if lq {
                                outp = match typ {
                                    0 => low / 2.0,
                                    1 => high,
                                    2 => band,
                                    _ => high + low,
                                };
                            } else {
                                let (slow, shigh, sband, snotch) = (low, high, band, high + low);
                                low = low + (f * band) / 1024.0;
                                high = inp - low - (q * band) / 1024.0;
                                band = (f * high) / 1024.0 + band;
                                outp = match typ {
                                    0 => (d2 + slow) / 2.0,
                                    1 => (d3 + shigh) / 2.0,
                                    2 => (d1 + sband) / 2.0,
                                    _ => (d3 + snotch) / 2.0,
                                };
                                if typ == 1 {
                                    d3 = high;
                                } else if typ == 3 {
                                    d3 = high + low;
                                }
                            }
                            d1 = band;
                            d2 = low;
                            *b = outp;
                        }
                    }
                    fc.d1 = d1;
                    fc.d2 = d2;
                    fc.d3 = d3;
                }
                i = i2;
                chan.f_tick_counter += cur_size;
                chan.f_tick_counter &= FILTER_TICK_SIZE - 1;
            }
            chan.f_sustain = sustain;
            chan.f_env_vol = env_vol;
        }
        let (mut ctl_attack, mut ctl_release) = (c.attack, c.release);
        if self.opt.volume_env_scaling {
            let mut s = MAX_PITCH - chan.sc[0].pitch;
            if s > 10000 {
                s -= 10000;
                s >>= 8;
                s = s.clamp(0, 256);
                ctl_attack -= (s * (512 - ctl_attack)) / 512;
                ctl_release -= (s * (512 - ctl_release)) / 512;
                ctl_attack = ctl_attack.max(0);
                ctl_release = ctl_release.max(0);
            }
        }
        let attack = curve(ctl_attack);
        let release = curve(ctl_release);
        let mut sustain = chan.sustain;
        let mut env_vol = chan.env_vol;
        let mut playing = chan.playing;
        if self.opt.fast_zero_attack_release {
            if c.attack == 0 && sustain {
                env_vol = ENV_VOL_MAX;
                if c.sustain == 0 {
                    sustain = false;
                }
            }
            if c.release == 0 && !sustain {
                env_vol = 0;
            }
        }
        let frames = frames as usize;
        let mut i = 0usize;
        let env_of = |env_vol: i32| -> f32 {
            let mut env = env_vol >> ENV_VOL_PREC;
            if c.exp != 0 {
                env = (env * env) >> 15;
            }
            env as f32
        };
        'outer: while i < frames {
            if sustain && env_vol == ENV_VOL_MAX {
                break;
            }
            if sustain && env_vol < ENV_VOL_MAX {
                while i < frames {
                    let mut v = buf[i];
                    v *= env_of(env_vol);
                    v /= 32768.0;
                    buf[i] = v;
                    env_vol += attack;
                    i += 1;
                    if env_vol >= ENV_VOL_MAX {
                        env_vol = ENV_VOL_MAX;
                        if c.sustain == 0 {
                            sustain = false;
                        }
                        continue 'outer;
                    }
                }
            } else {
                while i < frames {
                    let mut v = buf[i];
                    v *= env_of(env_vol);
                    v /= 32768.0;
                    buf[i] = v;
                    env_vol -= release;
                    i += 1;
                    if env_vol < 0 {
                        env_vol = 0;
                        playing = false;
                        buf[i..frames].fill(0.0);
                        i = frames;
                        continue 'outer;
                    }
                }
            }
        }
        chan.sustain = sustain;
        chan.env_vol = env_vol;
        chan.playing = playing;
        if !playing {
            chan.id = !0;
        }
    }

    fn render(&mut self, n: usize, net: &mut Net) -> isize {
        let c = Ctls::read(&net.mods[n]);
        let want = if c.mode == MODE_HQ_MONO || c.mode == MODE_LQ_MONO { 1 } else { 2 };
        if net.mods[n].output_channels != want {
            net.set_number_of_outputs(want, n);
        }
        let base = net.base_host_version;
        let m = &net.mods[n];
        let outputs_num = m.output_channels.max(0) as usize;
        let ctl_channels = m.ctl(C_CHANNELS).max(0) as usize;
        let (offset, frames) = (m.offset as usize, m.frames as usize);
        let mut retval = 0isize;
        let mut empty_output = true;
        self.no_active_channels = true;
        let mut buf = std::mem::take(&mut self.buf);
        if buf.len() < frames {
            buf.resize(frames, 0.0);
        }
        for ci in 0..ctl_channels.min(MAX_CHANNELS) {
            let mut chan = self.channels[ci];
            if !chan.playing {
                continue;
            }
            self.no_active_channels = false;
            let mut first_frame = chan.first_frame;
            let mut first_frame2 = first_frame;
            if first_frame {
                chan.first_frame = false;
            }
            let mut chan_vol = (c.volume * chan.local_volume) / 256;
            if self.opt.volume_scaling {
                let s = (((chan.sc[0].pitch * 256) / MAX_PITCH) + 16).clamp(0, 256);
                chan_vol = (chan_vol * s) / 256;
            }
            let rb = &mut buf[..frames];
            let mut no_wave = false;
            if chan.sc[0].pitch < 10000 {
                if chan.sc[0].pitch <= 1700 {
                    chan_vol = 0;
                    no_wave = true;
                } else {
                    chan_vol *= chan.sc[0].pitch - 1700;
                    chan_vol /= 10000 - 1700;
                }
            }
            let mut i = 0usize;
            while i < frames {
                let mut cur_size = (TICK_SIZE - chan.tick_counter) as usize;
                if i + cur_size > frames {
                    cur_size = frames - i;
                }
                if chan.tick_counter == 0 {
                    let sc0 = &mut chan.sc[0];
                    if sc0.pitch != sc0.target_pitch {
                        sc0.pitch += sc0.pitch_delta;
                        if sc0.pitch_delta > 0 {
                            if sc0.pitch > sc0.target_pitch {
                                sc0.pitch = sc0.target_pitch;
                            }
                        } else if sc0.pitch < sc0.target_pitch {
                            sc0.pitch = sc0.target_pitch;
                        }
                        let p = sc0.pitch;
                        self.set_pitch(&mut chan, p);
                    }
                }
                let seg = &mut rb[i..i + cur_size];
                self.render_waveform(&mut chan, 0, no_wave, false, seg, cur_size, &c);
                if c.pitch2 == MAX_PITCH2 / 2 && !self.opt.always_play_osc2 {
                    self.render_waveform(&mut chan, 1, true, true, seg, cur_size, &c);
                } else {
                    let p = chan.sc[0].pitch - (c.pitch2 - 1000) * 4;
                    self.sub_set_pitch(&mut chan, 1, p);
                    if first_frame2 {
                        first_frame2 = false;
                        chan.osc2_vol.reset(c.osc2_vol as f32, 32768.0);
                    }
                    let target = dsp::smoother_target(c.osc2_vol as f32, 32768.0);
                    let smoothing = chan.osc2_vol.check(target);
                    if !smoothing && c.osc2_vol == 32768 && c.osc2_mode == 0 {
                        self.render_waveform(&mut chan, 1, no_wave, true, seg, cur_size, &c);
                    } else {
                        let mut add_buf = std::mem::take(&mut self.add_buf);
                        add_buf.resize(TICK_SIZE as usize, 0.0);
                        self.render_waveform(&mut chan, 1, no_wave, false, &mut add_buf, cur_size, &c);
                        let ab = &mut add_buf[..cur_size];
                        if smoothing {
                            for v in ab.iter_mut() {
                                *v *= chan.osc2_vol.val(&self.coefs, target);
                            }
                        } else if c.osc2_vol != 32768 {
                            for v in ab.iter_mut() {
                                *v *= target;
                            }
                        }
                        match c.osc2_mode {
                            0 => seg.iter_mut().zip(ab.iter()).for_each(|(a, b)| *a += *b),
                            1 => seg.iter_mut().zip(ab.iter()).for_each(|(a, b)| *a -= *b),
                            2 => seg.iter_mut().zip(ab.iter()).for_each(|(a, b)| *a *= *b),
                            3 => seg.iter_mut().zip(ab.iter()).for_each(|(a, b)| {
                                if *b < *a {
                                    *a = *b
                                }
                            }),
                            4 => seg.iter_mut().zip(ab.iter()).for_each(|(a, b)| {
                                if *b > *a {
                                    *a = *b
                                }
                            }),
                            5 | 6 => seg.iter_mut().zip(ab.iter()).for_each(|(a, b)| {
                                let v1 = super::generator::to_int16(*a) as i32;
                                let v2 = super::generator::to_int16(*b) as i32;
                                let v = if c.osc2_mode == 5 { v1 & v2 } else { v1 ^ v2 };
                                *a = s16(v);
                            }),
                            7 => seg.iter_mut().zip(ab.iter()).for_each(|(a, b)| {
                                if b.abs() < a.abs() {
                                    *a = *b
                                }
                            }),
                            8 => seg.iter_mut().zip(ab.iter()).for_each(|(a, b)| {
                                if b.abs() > a.abs() {
                                    *a = *b
                                }
                            }),
                            _ => {}
                        }
                        self.add_buf = add_buf;
                    }
                }
                chan.tick_counter += cur_size as i32;
                chan.tick_counter &= TICK_SIZE - 1;
                i += cur_size;
            }
            self.apply_noise(rb, &c);
            self.apply_envelope(&mut chan, rb, &c, base);
            if frames > 0 {
                if chan.anticlick_counter != 0 {
                    for v in rb.iter_mut() {
                        let mut a = chan.anticlick_sample;
                        a *= chan.anticlick_counter as f32;
                        a /= 32768.0;
                        *v += a;
                        chan.anticlick_counter -= 32768 / 32;
                        if chan.anticlick_counter < 0 {
                            chan.anticlick_counter = 0;
                            break;
                        }
                    }
                }
                chan.last_sample = rb[frames - 1];
                chan.no_last_sample = false;
            }
            let mut target_volume = [0i32; 2];
            target_volume[0] = (chan_vol * chan.vel) / 2;
            target_volume[1] = target_volume[0];
            let ctl_pan = (c.pan + (chan.local_pan - 128)).clamp(0, 256);
            if ctl_pan < 128 {
                target_volume[1] = target_volume[1] * ctl_pan / 128;
            } else {
                target_volume[0] = (target_volume[0] * (128 - (ctl_pan - 128))) / 128;
            }
            if first_frame {
                first_frame = false;
                let _ = first_frame;
                for ch in 0..2 {
                    chan.vol[ch].reset(target_volume[ch] as f32, 32768.0);
                }
            }
            let m = &mut net.mods[n];
            for ch in 0..outputs_num.min(2) {
                let out = &mut m.chout[ch][offset..offset + frames];
                let target = dsp::smoother_target(target_volume[ch] as f32, 32768.0);
                let svol = &mut chan.vol[ch];
                if svol.check(target) {
                    for (o, &r) in out.iter_mut().zip(rb.iter()) {
                        let v = r * svol.val(&self.coefs, target);
                        if retval == 0 {
                            *o = v;
                        } else {
                            *o += v;
                        }
                    }
                    empty_output = false;
                } else {
                    let vol = target_volume[ch];
                    let vol2 = vol as f32 / 32768.0;
                    if retval == 0 {
                        if vol != 32768 {
                            if vol == 0 {
                                out.fill(0.0);
                            } else {
                                for (o, &r) in out.iter_mut().zip(rb.iter()) {
                                    *o = r * vol2;
                                }
                                empty_output = false;
                            }
                        } else {
                            out.copy_from_slice(rb);
                            empty_output = false;
                        }
                    } else if vol != 32768 {
                        if vol != 0 {
                            for (o, &r) in out.iter_mut().zip(rb.iter()) {
                                *o += r * vol2;
                            }
                            empty_output = false;
                        }
                    } else {
                        for (o, &r) in out.iter_mut().zip(rb.iter()) {
                            *o += r;
                        }
                        empty_output = false;
                    }
                }
            }
            retval = 1;
            self.channels[ci] = chan;
        }
        self.buf = buf;
        if retval != 0 && empty_output {
            retval = 2;
        }
        retval
    }

    fn find(&self, id: u32, upto: usize) -> Option<usize> {
        (0..upto.min(MAX_CHANNELS)).find(|&c| self.channels[c].id == id)
    }
}

impl Module for AnalogGenerator {
    fn handle(&mut self, n: usize, e: &mut Event, net: &mut Net) -> isize {
        let mut retval: isize = 0;
        match e.cmd {
            Cmd::Init => {
                net.register_ctl(n, 0, 256, 80, 0, C_VOLUME, -1, 0);
                net.register_ctl(n, 0, MAX_TYPE, 0, 1, C_TYPE, -1, 0);
                net.register_ctl(n, 0, 256, 128, 0, C_PAN, 128, 0);
                net.set_ctl_show_offset(n, 2, -128);
                net.register_ctl(n, 0, 256, 0, 0, C_ATTACK, -1, 1);
                net.register_ctl(n, 0, 256, 0, 0, C_RELEASE, -1, 1);
                net.register_ctl(n, 0, 1, 1, 1, C_SUSTAIN, -1, 1);
                net.register_ctl(n, 0, 1, 1, 1, C_EXP, -1, 1);
                net.register_ctl(n, 0, MAX_DUTY_CYCLE, MAX_DUTY_CYCLE / 2, 0, C_DUTY_CYCLE, MAX_DUTY_CYCLE / 2, 0);
                net.register_ctl(n, 0, MAX_PITCH2, MAX_PITCH2 / 2, 0, C_PITCH2, MAX_PITCH2 / 2, 0);
                net.set_ctl_show_offset(n, 8, -1000);
                net.register_ctl(n, 0, MAX_FILTER_TYPE, 0, 1, C_F_TYPE, 0, 2);
                net.register_ctl(n, 0, MAX_FILTER_FREQ, MAX_FILTER_FREQ, 0, C_F_FREQ, -1, 2);
                net.set_ctl_flags(n, 10, 2);
                net.register_ctl(n, 0, MAX_FILTER_RES, 0, 0, C_F_RES, -1, 2);
                net.register_ctl(n, 0, 1, 1, 1, C_F_EXP, -1, 2);
                net.register_ctl(n, 0, 256, 0, 0, C_F_ATTACK, -1, 2);
                net.register_ctl(n, 0, 256, 0, 0, C_F_RELEASE, -1, 2);
                net.register_ctl(n, 0, F_ENV_MODES - 1, F_ENV_OFF, 1, C_F_ENVELOPE_MODE, -1, 2);
                net.register_ctl(n, 1, MAX_CHANNELS as i32, 16, 1, C_CHANNELS, -1, 3);
                net.register_ctl(n, 0, MODES - 1, MODE_HQ_MONO, 1, C_MODE, -1, 3);
                net.register_ctl(n, 0, 256, 0, 0, C_NOISE, -1, 3);
                net.register_ctl(n, 0, 32768, 32768, 0, C_OSC2_VOL, -1, 0);
                net.register_ctl(n, 0, 8, 0, 1, C_OSC2_MODE, -1, 0);
                net.register_ctl(n, 0, 32768, 0, 0, C_OSC2_PHASE, -1, 0);
                let id = net.mods[n].id;
                self.channels = vec![Chan::default(); MAX_CHANNELS];
                for ch in self.channels.iter_mut() {
                    ch.playing = false;
                    ch.id = !0;
                    ch.no_last_sample = true;
                    for (i, sc) in ch.sc.iter_mut().enumerate() {
                        // stime_ns() + pseudo_random() + mod_num * 3 + i * 7 + mod->id * 3079, the clock at 0.
                        sc.noise_seed = net.globals.pseudo_random().wrapping_add((n as u32).wrapping_mul(3)).wrapping_add(i as u32 * 7).wrapping_add(id.wrapping_mul(3079));
                    }
                }
                self.no_active_channels = true;
                self.drawn_wave = DRAWN_WAVEFORM;
                self.noise_seed = net.globals.pseudo_random().wrapping_add(11).wrapping_add(id.wrapping_mul(3079));
                self.random_seed = net.globals.pseudo_random().wrapping_add(22).wrapping_add(id.wrapping_mul(3079));
                self.coefs = SmootherCoefs::new(100.0, net.sampling_freq as f32);
                net.temp_buf(0);
                retval = 1;
            }
            Cmd::SetupFinished => {
                if let Some(c) = net.mods[n].chunk(0) {
                    for (i, w) in self.drawn_wave.iter_mut().enumerate() {
                        *w = c.data.get(i).copied().unwrap_or(0) as i8;
                    }
                }
                net.chunk_autocreate(n, 1, 15, true);
                let o = &net.mods[n].chunk(1).unwrap().data;
                let b = |i: usize| o[i] != 0;
                self.opt = Opt {
                    volume_env_scaling: b(0),
                    filter_env_scaling: b(1),
                    volume_scaling: b(2),
                    filter_scaling: b(3),
                    filter_velocity_scaling: b(4),
                    freq_div: b(5),
                    no_smooth_freq: b(6),
                    filter_scaling_reverse: b(7),
                    retain_phase: b(8),
                    random_phase: b(9),
                    filter_pitch: b(10),
                    filter_res_velocity_scaling: b(11),
                    fast_zero_attack_release: b(12),
                    freq_accuracy: b(13),
                    always_play_osc2: b(14),
                };
                retval = 1;
            }
            Cmd::RenderReplace => retval = self.render(n, net),
            Cmd::NoteOn => {
                let c = Ctls::read(&net.mods[n]);
                let ctl_channels = net.mods[n].ctl(C_CHANNELS).max(1) as usize;
                let base = net.base_host_version;
                if !self.no_active_channels {
                    for ch in self.channels.iter_mut() {
                        if ch.id == e.id {
                            if c.sustain != 0 {
                                ch.sustain = false;
                            }
                            if base >= 0x01070303 && c.f_envelope_mode == F_ENV_SUSTAIN_ON {
                                ch.f_sustain = false;
                            }
                            ch.id = !0;
                            break;
                        }
                    }
                    let mut k = 0;
                    while k < ctl_channels {
                        if !self.channels[self.search_ptr].playing {
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
                        if base >= 0x02010301 {
                            for _ in 0..ctl_channels {
                                if !self.channels[self.search_ptr].sustain {
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
                let mut chan = self.channels[self.search_ptr];
                self.set_pitch(&mut chan, e.pitch());
                self.prepare_channel(&mut chan, &c);
                chan.vel = e.velocity() as i32;
                chan.id = e.id;
                self.channels[self.search_ptr] = chan;
                self.no_active_channels = false;
                retval = 1;
            }
            Cmd::SetFreq => {
                if self.no_active_channels {
                    return 0;
                }
                if let Some(ci) = self.find(e.id, net.mods[n].ctl(C_CHANNELS).max(0) as usize) {
                    let mut chan = self.channels[ci];
                    self.set_target_pitch(&mut chan, e.pitch());
                    chan.vel = e.velocity() as i32;
                    self.channels[ci] = chan;
                }
                retval = 1;
            }
            Cmd::SetVelocity => {
                if self.no_active_channels {
                    return 0;
                }
                if let Some(ci) = self.find(e.id, net.mods[n].ctl(C_CHANNELS).max(0) as usize) {
                    self.channels[ci].vel = e.velocity() as i32;
                }
                retval = 1;
            }
            Cmd::SetSampleOffset | Cmd::SetSampleOffset2 => {
                if self.no_active_channels {
                    return 0;
                }
                let c = Ctls::read(&net.mods[n]);
                for ci in 0..MAX_CHANNELS {
                    if self.channels[ci].id == e.id {
                        let mut chan = self.channels[ci];
                        self.set_offset(&mut chan, e.sample_offset() as i32, &c);
                        self.channels[ci] = chan;
                    }
                }
                retval = 1;
            }
            Cmd::NoteOff => {
                if self.no_active_channels {
                    return 0;
                }
                let (sus, fenv) = (net.mods[n].ctl(C_SUSTAIN), net.mods[n].ctl(C_F_ENVELOPE_MODE));
                if let Some(ci) = self.find(e.id, MAX_CHANNELS) {
                    if sus != 0 {
                        self.channels[ci].sustain = false;
                    }
                    if fenv == F_ENV_SUSTAIN_ON {
                        self.channels[ci].f_sustain = false;
                    }
                }
                retval = 1;
            }
            Cmd::AllNotesOff => {
                for ch in self.channels.iter_mut() {
                    ch.sustain = false;
                    ch.f_sustain = false;
                    ch.id = !0;
                }
                retval = 1;
            }
            Cmd::Clean => {
                for ch in self.channels.iter_mut() {
                    ch.playing = false;
                    ch.id = !0;
                    ch.no_last_sample = true;
                }
                self.no_active_channels = true;
                retval = 1;
            }
            Cmd::SetLocalController | Cmd::SetGlobalController => {
                if e.cmd == Cmd::SetLocalController && !self.no_active_channels {
                    if let Some(ci) = self.find(e.id, net.mods[n].ctl(C_CHANNELS).max(0) as usize) {
                        let v = e.ctl_val() as i32;
                        let ch = &mut self.channels[ci];
                        retval = 1;
                        match e.ctl_num() {
                            0 => ch.local_volume = (v * 256) / 32768,
                            1 => ch.local_type = v.min(MAX_TYPE),
                            2 => ch.local_pan = v >> 7,
                            7 => ch.local_duty_cycle = (v * MAX_DUTY_CYCLE) / 32768,
                            9 => ch.local_f_type = v.min(MAX_FILTER_TYPE),
                            10 => ch.local_f_freq = (v * MAX_FILTER_FREQ) / 32768,
                            11 => ch.local_f_res = (v * MAX_FILTER_RES) / 32768,
                            _ => retval = 0,
                        }
                    }
                    if retval != 0 {
                        return retval;
                    }
                }
                let v = e.ctl_val() != 0;
                let o = &mut self.opt;
                retval = 1;
                match e.ctl_num() as i32 + 1 {
                    127 => o.volume_env_scaling = v,
                    126 => o.filter_env_scaling = v,
                    125 => o.volume_scaling = v,
                    124 => o.filter_scaling = v,
                    123 => o.filter_velocity_scaling = v,
                    122 => o.freq_div = v,
                    121 => o.no_smooth_freq = !v,
                    120 => o.filter_scaling_reverse = v,
                    119 => {
                        o.retain_phase = v;
                        if v {
                            o.random_phase = false;
                        }
                    }
                    118 => {
                        o.random_phase = v;
                        if v {
                            o.retain_phase = false;
                        }
                    }
                    117 => {
                        o.filter_pitch = v;
                        if v {
                            o.filter_scaling = false;
                            o.filter_scaling_reverse = false;
                        }
                    }
                    116 => o.filter_res_velocity_scaling = v,
                    115 => o.fast_zero_attack_release = v,
                    114 => o.freq_accuracy = v,
                    113 => o.always_play_osc2 = v,
                    _ => retval = 0,
                }
            }
            Cmd::Close => retval = 1,
            _ => {}
        }
        retval
    }
}
