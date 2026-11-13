// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! Sampler (psynths_sampler.cpp): an XM-like instrument of up to 128
//! samples (8 or 16-bit or float, mono or stereo) mapped over the notes,
//! played without, with linear or with spline interpolation, forward,
//! ping-pong or reversed loops; volume, panning, pitch and four effect
//! envelopes stepped a tick (subdivided) at a time; an attack, a release,
//! the instrument's fadeout and vibrato; an optional effect module run
//! on each voice.

use super::super::net::{self, Chunk, Cmd, Event, ModCommon, Module, Net};
use super::super::subengine::SubEngine;

const OUTPUTS: usize = 2;
const MAX_CHANNELS: usize = 32;
const MAX_SAMPLES: usize = 128;
const CHUNK_INS: usize = 0;
const fn chunk_smp(s: usize) -> usize {
    1 + s * 2
}
const fn chunk_smp_data(s: usize) -> usize {
    1 + s * 2 + 1
}
const CHUNK_OPT: usize = chunk_smp(MAX_SAMPLES);
const fn chunk_env(e: usize) -> usize {
    CHUNK_OPT + 1 + e
}
const CHUNK_EFFECT_MOD: usize = chunk_env(8);
const FP_PREC: i32 = 16;
const INTERP_PREC: i32 = FP_PREC - 1;
const INS_SIGN: u32 = 0x5341_4D50;
const INS_VERSION: u32 = 6;
const INS_SIZE: usize = 400;
const SMP_SIZE: usize = 44;
const ENV_HEADER: usize = 20;

const ENV_VOL: usize = 0;
const ENV_PAN: usize = 1;
const ENV_PITCH: usize = 2;
const ENV_EFF1: usize = 3;
const ENV_EFF4: usize = 6;
const ENV_COUNT: usize = 7;
const FIRST_ENV_WITH_SUBDIV: usize = ENV_PITCH;
const ENVELOPES_SIGNED: u32 = (1 << ENV_PAN) | (1 << ENV_PITCH);
const ENV_FLAG_ENABLED: u16 = 1 << 0;
const ENV_FLAG_SUSTAIN: u16 = 1 << 1;
const ENV_FLAG_LOOP: u16 = 1 << 2;

const F_PLAYING: u32 = 1 << 0;
const F_SUSTAIN: u32 = 1 << 1;
const F_INLOOP: u32 = 1 << 2;
const F_REVERSE: u32 = 1 << 3;
const F_ENV_START: u32 = 1 << 4;
const F_READY_TO_PLAY: u32 = 1 << 5;

const SMP_LOOPRELEASE: u8 = 1 << 2;
const SMP_STEREO: u8 = 1 << 6;

const C_VOLUME: usize = 0;
const C_PAN: usize = 1;
const C_SMP_INT: usize = 2;
const C_ENV_INT: usize = 3;
const C_CHANNELS: usize = 4;
const C_REC_THRESHOLD: usize = 5;
const C_TICK_SCALE: usize = 6;
const C_RECORD: usize = 7;
const C_REVERSE: usize = 8;
const C_ATTACK: usize = 9;
const C_RELEASE: usize = 10;

fn rd_u16(d: &[u8], at: usize) -> u16 {
    d.get(at..at + 2).map_or(0, |b| u16::from_le_bytes([b[0], b[1]]))
}

fn rd_u32(d: &[u8], at: usize) -> u32 {
    d.get(at..at + 4).map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn wr_u16(d: &mut [u8], at: usize, v: u16) {
    if let Some(b) = d.get_mut(at..at + 2) {
        b.copy_from_slice(&v.to_le_bytes());
    }
}

fn wr_u32(d: &mut [u8], at: usize, v: u32) {
    if let Some(b) = d.get_mut(at..at + 4) {
        b.copy_from_slice(&v.to_le_bytes());
    }
}

/// The instrument chunk's fields the player reads.
#[derive(Clone, Default)]
struct Ins {
    samples_num: u16,
    vibrato_type: u8,
    vibrato_sweep: u8,
    vibrato_depth: u8,
    vibrato_rate: u8,
    volume_fadeout: u16,
    finetune: i8,
    relative_note: i8,
    smp_num: Vec<u8>,
}

impl Ins {
    fn read(d: &[u8]) -> Ins {
        let mut smp_num = vec![0u8; 128];
        if let Some(b) = d.get(260..388) {
            smp_num.copy_from_slice(b);
        }
        Ins {
            samples_num: rd_u16(d, 28),
            vibrato_type: d.get(238).copied().unwrap_or(0),
            vibrato_sweep: d.get(239).copied().unwrap_or(0),
            vibrato_depth: d.get(240).copied().unwrap_or(0),
            vibrato_rate: d.get(241).copied().unwrap_or(0),
            volume_fadeout: rd_u16(d, 242),
            finetune: d.get(245).copied().unwrap_or(0) as i8,
            relative_note: d.get(247).copied().unwrap_or(0) as i8,
            smp_num,
        }
    }
}

/// A sample chunk's header.
#[derive(Clone, Copy, Default)]
struct Smp {
    length: u32,
    reppnt: u32,
    replen: u32,
    volume: u8,
    finetune: i8,
    typ: u8,
    panning: u8,
    relative_note: i8,
    start_pos: u32,
}

impl Smp {
    fn read(d: &[u8]) -> Smp {
        Smp {
            length: rd_u32(d, 0),
            reppnt: rd_u32(d, 4),
            replen: rd_u32(d, 8),
            volume: d.get(12).copied().unwrap_or(0),
            finetune: d.get(13).copied().unwrap_or(0) as i8,
            typ: d.get(14).copied().unwrap_or(0),
            panning: d.get(15).copied().unwrap_or(0),
            relative_note: d.get(16).copied().unwrap_or(0) as i8,
            start_pos: rd_u32(d, 40),
        }
    }
}

/// A sample's data as its type is.
#[derive(Clone, Default)]
enum SmpData {
    #[default]
    None,
    I8(Vec<i8>),
    I16(Vec<i16>),
    F32(Vec<f32>),
}

/// An envelope chunk: its header and points (time, value; u16 each).
#[derive(Clone, Default)]
struct Env {
    flags: u16,
    eff_ctl: u8,
    gain: u8,
    velocity_influence: u8,
    point_count: u16,
    sustain: u16,
    loop_start: u16,
    loop_end: u16,
    points: Vec<u16>,
}

impl Env {
    fn read(d: &[u8]) -> Env {
        let pts = d.get(ENV_HEADER..).unwrap_or(&[]).chunks_exact(2).map(|b| u16::from_le_bytes([b[0], b[1]])).collect();
        Env {
            flags: rd_u16(d, 0),
            eff_ctl: d.get(2).copied().unwrap_or(0),
            gain: d.get(3).copied().unwrap_or(0),
            velocity_influence: d.get(4).copied().unwrap_or(0),
            point_count: rd_u16(d, 8),
            sustain: rd_u16(d, 10),
            loop_start: rd_u16(d, 12),
            loop_end: rd_u16(d, 14),
            points: pts,
        }
    }
    /// points[ i ] as C reads it (past the chunk: 0).
    fn pt(&self, i: usize) -> u32 {
        self.points.get(i).copied().unwrap_or(0) as u32
    }
}

/// refresh_instrument_envelope: the points drawn into a table, a value a tick.
fn refresh_envelope(src: &Env, dest: Option<Vec<u16>>) -> Option<Vec<u16>> {
    let pnum = src.point_count as usize;
    if pnum == 0 {
        return dest;
    }
    let mut dest = dest.unwrap_or_default();
    let len_add = 64usize;
    let mut new_len = 0usize;
    for p in 0..pnum {
        let l = src.pt(p * 2) as usize + len_add;
        if l > new_len {
            new_len = l;
        }
    }
    if new_len > dest.len() {
        dest.resize(new_len, 0);
    }
    let max_t = new_len - len_add + 1;
    if pnum >= 2 {
        let mut prev_t = 0i32;
        for p in 0..pnum - 1 {
            let (t1, v1, t2, v2) = (src.pt(p * 2) as i32, src.pt(p * 2 + 1) as i32, src.pt(p * 2 + 2) as i32, src.pt(p * 2 + 3) as i32);
            if t2 < prev_t {
                break;
            }
            prev_t = t2;
            let mut val = v1 << 8;
            let delta = if t2 - t1 == 0 { 0 } else { ((v2 - v1) << 8) / (t2 - t1) };
            for i in t1..=t2 {
                if let Some(d) = dest.get_mut(i as usize) {
                    *d = (val >> 8) as u16;
                }
                val += delta;
            }
            if let Some(d) = dest.get_mut(t2 as usize) {
                *d = v2 as u16;
            }
        }
    } else {
        let v = src.pt(1) as u16;
        for i in 0..=max_t {
            if let Some(d) = dest.get_mut(i) {
                *d = v;
            }
        }
    }
    Some(dest)
}

/// sampler_options (chunk CHUNK_OPT, 8 bytes).
#[derive(Clone, Copy, Default)]
struct Opt {
    rec_on_play: bool,
    ignore_vel_for_volume: bool,
    freq_accuracy: bool,
    fit_to_pattern: u8,
}

#[derive(Clone, Copy, Default)]
struct Chan {
    flags: u32,
    id: u32,
    vel: i32,
    ptr_h: i32,
    ptr_l: i32,
    delta_h: u32,
    delta_l: u32,
    cur_pitch: i32,
    cur_pitch_delta: i32,
    pitch_add1: i32,
    pitch_add2: i32,
    final_pitch: i32,
    last_s: [f32; OUTPUTS],
    anticlick_in_s: [f32; OUTPUTS],
    anticlick_in_counter: u16,
    anticlick_out_counter: u16,
    tick_counter: i32,
    tick_counter2: i32,
    subtick_counter: i32,
    env_pos: [u32; ENV_COUNT],
    env_prev: [u16; ENV_COUNT],
    env_next: [u16; ENV_COUNT],
    prev_eff_ctl_val: i32,
    vib_pos: i16,
    vib_frame: i16,
    fadeout: i32,
    vol_step: i32,
    l_delta: i32,
    r_delta: i32,
    l_cur: i32,
    r_cur: i32,
    l_old: i32,
    r_old: i32,
    smp_num: i16,
    smp_note_num: i16,
    local_pan: i32,
    local_reverse: i32,
}

#[derive(Default)]
pub(crate) struct Sampler {
    channels: [Chan; MAX_CHANNELS],
    no_active_channels: bool,
    search_ptr: usize,
    last_ch: usize,
    sample_base_pitch: Vec<i32>,
    sample_base_freq: Vec<i32>,
    env: [Env; ENV_COUNT],
    env_buf: [Option<Vec<u16>>; ENV_COUNT],
    eff_env: bool,
    ticks_per_line: u8,
    tick_size: i32,
    tick_size2: i32,
    min_tick_size2: i32,
    tick_subdiv: u16,
    anticlick_len: u16,
    opt: Opt,
    ins: Ins,
    smps: Vec<Smp>,
    data: Vec<SmpData>,
    /// The effect run on each voice (CHUNK_EFFECT_MOD), voice c as its module 1 + c.
    ps: Option<SubEngine>,
    bufs: [Vec<f32>; OUTPUTS],
}

/// PSYNTH_FP64_SUB.
fn fp_sub(h: &mut i32, l: &mut i32, h2: i32, l2: i32) {
    *h = h.wrapping_sub(h2);
    *l = l.wrapping_sub(l2);
    if *l < 0 {
        *h -= 1;
        *l += 1 << FP_PREC;
    }
}

/// PSYNTH_FP64_ADD.
fn fp_add(h: &mut i32, l: &mut i32, h2: i32, l2: i32) {
    *h = h.wrapping_add(h2);
    *l = l.wrapping_add(l2);
    if *l > (1 << FP_PREC) - 1 {
        *h += 1;
        *l -= 1 << FP_PREC;
    }
}

fn spline(y0: f32, y1: f32, y2: f32, y3: f32, mu: f32) -> f32 {
    let a = (3.0 * (y1 - y2) - y0 + y3) / 2.0;
    let b = 2.0 * y2 + y0 - (5.0 * y1 + y3) / 2.0;
    let c2 = (y2 - y0) / 2.0;
    (((a * mu) + b) * mu + c2) * mu + y1
}

impl Sampler {
    fn read_envs(&mut self, m: &ModCommon) {
        for i in 0..ENV_COUNT {
            self.env[i] = m.chunk(chunk_env(i)).map(|c| Env::read(&c.data)).unwrap_or_default();
        }
    }

    fn refresh_envelopes(&mut self) {
        for i in 0..ENV_COUNT {
            let e = self.env[i].clone();
            self.env_buf[i] = refresh_envelope(&e, self.env_buf[i].take());
        }
    }

    fn handle_envelope_flags(&mut self) {
        self.tick_subdiv = 0;
        self.eff_env = false;
        if (FIRST_ENV_WITH_SUBDIV..ENV_COUNT).any(|i| self.env[i].flags & ENV_FLAG_ENABLED != 0) {
            self.tick_subdiv = 3;
        }
        if (ENV_EFF1..=ENV_EFF4).any(|i| self.env[i].flags & ENV_FLAG_ENABLED != 0) {
            self.eff_env = true;
        }
    }

    /// make_default_envelopes, into the module's chunks.
    fn make_default_envelopes(net: &mut Net, n: usize) {
        let write = |net: &mut Net, e: usize, flags: u16, sustain: u16, pts: &[(u32, u32)]| {
            let size = ENV_HEADER + 4 * pts.len();
            let mut d = vec![0u8; size];
            wr_u16(&mut d, 0, flags);
            d[3] = 100;
            wr_u16(&mut d, 8, pts.len() as u16);
            wr_u16(&mut d, 10, sustain);
            for (i, &(t, v)) in pts.iter().enumerate() {
                wr_u32(&mut d, ENV_HEADER + 4 * i, t | (v << 16));
            }
            // The chunk keeps its size if bigger (autocreate), zeroed by smem_zero.
            let old = net.mods[n].chunk(chunk_env(e)).map_or(0, |c| c.data.len());
            d.resize(size.max(old), 0);
            net.new_chunk(n, chunk_env(e), Chunk { data: d, flags: 0, freq: 0 });
        };
        write(net, ENV_VOL, ENV_FLAG_ENABLED | ENV_FLAG_SUSTAIN, 0, &[(0, 32768), (8, 0), (32, 0)]);
        write(net, ENV_PAN, 0, 0, &[(0, 16384), (64, 8192), (128, 24576), (180, 16384)]);
        write(net, ENV_PITCH, 0, 0, &[(0, 16384), (64, 16384)]);
        for e in ENV_EFF1..=ENV_EFF4 {
            write(net, e, 0, 0, &[(0, 32768), (64, 32768)]);
        }
    }

    /// convert_old_envelopes_to_new: XI-style volume and panning envelopes.
    fn convert_old_envelopes(net: &mut Net, n: usize) {
        let Some(ins) = net.mods[n].chunk(CHUNK_INS).map(|c| c.data.clone()) else { return };
        for (e, pts_at, num_at, sus_at, ls_at, le_at, type_at) in [(ENV_VOL, 132, 228, 230, 231, 232, 236), (ENV_PAN, 180, 229, 233, 234, 235, 237)] {
            if net.mods[n].chunk(chunk_env(e)).is_none() {
                continue;
            }
            let ty = ins[type_at];
            let mut flags = 0u16;
            if ty & 1 != 0 {
                flags |= ENV_FLAG_ENABLED;
            }
            if ty & 2 != 0 {
                flags |= ENV_FLAG_SUSTAIN;
            }
            if ty & 4 != 0 {
                flags |= ENV_FLAG_LOOP;
            }
            let count = ins[num_at] as usize;
            let old = net.mods[n].chunk(chunk_env(e)).map_or(0, |c| c.data.len());
            let mut d = vec![0u8; (ENV_HEADER + 4 * count).max(old)];
            wr_u16(&mut d, 0, flags);
            d[3] = 100;
            wr_u16(&mut d, 8, count as u16);
            wr_u16(&mut d, 10, ins[sus_at] as u16);
            wr_u16(&mut d, 12, ins[ls_at] as u16);
            wr_u16(&mut d, 14, ins[le_at] as u16);
            for i in 0..count {
                let x = rd_u16(&ins, pts_at + i * 4) as u32;
                let y = rd_u16(&ins, pts_at + i * 4 + 2) as u32;
                wr_u32(&mut d, ENV_HEADER + 4 * i, x | (y << (16 + 9)));
            }
            net.new_chunk(n, chunk_env(e), Chunk { data: d, flags: 0, freq: 0 });
        }
    }

    fn recalc_base_pitch(&mut self, s: usize, m: &ModCommon) {
        if m.chunk(chunk_smp(s)).is_none() {
            return;
        }
        let mut freq = m.chunk(chunk_smp_data(s)).map_or(0, |c| c.freq);
        if freq == 0 {
            freq = 44100;
        }
        // PS_SFREQ_TO_PITCH: 30720 - log2( freq / 522.6875 ) * 3072, as an int.
        // LOG2 of a float: C++'s float overload, log2f.
        let mut base_pitch = (30720.0f32 - (freq as f32 / 522.6875).log2() * 3072.0) as i32;
        base_pitch = net::NOTE0_PITCH / 4 - ((net::NOTE0_PITCH - base_pitch) / 4);
        base_pitch *= 4;
        self.sample_base_pitch[s] = base_pitch;
        self.sample_base_freq[s] = freq;
    }

    fn calc_final_pitch(&self, c: &mut Chan, srate: i32) {
        let new_pitch = c.cur_pitch + c.pitch_add1 + c.pitch_add2;
        if c.final_pitch != new_pitch {
            c.final_pitch = new_pitch;
            if self.opt.freq_accuracy {
                let base_pitch = self.sample_base_pitch[c.smp_num as usize];
                let p = base_pitch - new_pitch;
                if p % (12 * 256) == 0 {
                    let base_freq = self.sample_base_freq[c.smp_num as usize];
                    let new_freq = if p >= 0 { base_freq << (p / (12 * 256)) } else { base_freq >> (-p / (12 * 256)) };
                    let (h, l) = net::get_delta_hq(srate, new_freq);
                    c.delta_h = h;
                    c.delta_l = l;
                    return;
                }
                let (h, l) = net::get_delta_hq(srate, net::get_freq(new_pitch / 4));
                c.delta_h = h;
                c.delta_l = l;
                return;
            }
            let (h, l) = net::get_delta(srate, net::get_freq(new_pitch / 4));
            c.delta_h = h;
            c.delta_l = l;
        }
    }

    fn reset_channel(&self, ch: &mut Chan, ctl_attack: i32) {
        ch.env_pos = [0; ENV_COUNT];
        for i in ENV_PAN..ENV_COUNT {
            let v = if ENVELOPES_SIGNED & (1 << i) != 0 { 16384 } else { 0 };
            ch.env_prev[i] = v;
            ch.env_next[i] = v;
        }
        ch.flags |= F_SUSTAIN;
        ch.fadeout = 65536;
        if self.env[ENV_VOL].flags & ENV_FLAG_ENABLED != 0 && ctl_attack != 0 {
            ch.fadeout = 0;
        }
        ch.vib_pos = 0;
        ch.vib_frame = 0;
        ch.flags |= F_ENV_START;
        ch.vol_step = 0;
        ch.ptr_h = 0;
        ch.ptr_l = 0;
        ch.flags &= !(F_INLOOP | F_REVERSE);
        ch.tick_counter = 0xFFFFFFF;
        ch.tick_counter2 = 0;
    }

    fn release_channel(&self, ch: &mut Chan) {
        if ch.flags & F_SUSTAIN != 0 {
            ch.flags &= !F_SUSTAIN;
            if self.env[ENV_VOL].flags & ENV_FLAG_ENABLED == 0 {
                ch.anticlick_out_counter = 32768;
            }
        }
    }

    fn init_reverse_flags(ch: &mut Chan, smp: &Smp, ctl_reverse: i32) {
        let local_reverse = ch.local_reverse;
        ch.ptr_h = smp.start_pos as i32;
        if (ctl_reverse > 0) ^ (local_reverse > 0) {
            if ch.ptr_h == 0 {
                ch.ptr_h = smp.length as i32 - 1;
            }
            if smp.typ & 3 != 0 && smp.replen != 0 {
                ch.ptr_h = (smp.reppnt + smp.replen) as i32 - 1;
            }
            ch.flags |= F_REVERSE;
        }
    }

    /// sampler_render: up to `frames` frames of the voice into the buffers at `at`; how many it made.
    #[allow(clippy::too_many_arguments)]
    fn render_voice(chan: &mut Chan, smp: &Smp, data: &SmpData, smp_int: i32, bufs: &mut [Vec<f32>; OUTPUTS], at: usize, frames: usize) -> usize {
        let reppnt = smp.reppnt as i32;
        let mut replen = smp.replen as i32;
        let repend = reppnt.wrapping_add(replen);
        let loop_type = smp.typ & 3;
        let bits = (smp.typ >> 4) & 3;
        let smp_len = smp.length as i32;
        let stereo = smp.typ & SMP_STEREO != 0;
        if loop_type == 0 {
            replen = 0;
        } else if chan.flags & F_SUSTAIN == 0 && smp.typ & SMP_LOOPRELEASE != 0 {
            chan.flags &= !F_REVERSE;
            replen = 0;
        }
        let get = |i: i32| -> f32 {
            match data {
                SmpData::I8(v) => v.get(i as usize).copied().unwrap_or(0) as f32,
                SmpData::I16(v) => v.get(i as usize).copied().unwrap_or(0) as f32,
                SmpData::F32(v) => v.get(i as usize).copied().unwrap_or(0.0),
                SmpData::None => 0.0,
            }
        };
        let geti = |i: i32| -> i32 {
            match data {
                SmpData::I8(v) => v.get(i as usize).copied().unwrap_or(0) as i32,
                SmpData::I16(v) => v.get(i as usize).copied().unwrap_or(0) as i32,
                _ => 0,
            }
        };
        let scale = match bits {
            0 => 128.0f32,
            1 => 32768.0,
            _ => 1.0,
        };
        let sh = if stereo { 1 } else { 0 };
        let mut i = 0usize;
        while i < frames {
            if replen != 0 {
                if chan.ptr_h >= repend {
                    if loop_type == 1 {
                        loop {
                            chan.ptr_h -= replen;
                            if chan.ptr_h < repend {
                                break;
                            }
                        }
                    } else {
                        let rep_part = (chan.ptr_h - reppnt) / replen;
                        if rep_part & 1 != 0 {
                            chan.flags |= F_REVERSE;
                            let (th, tl) = (chan.ptr_h, chan.ptr_l);
                            chan.ptr_h = reppnt + replen * (rep_part + 1);
                            chan.ptr_l = 0;
                            fp_sub(&mut chan.ptr_h, &mut chan.ptr_l, th, tl);
                            chan.ptr_h += reppnt;
                            if chan.ptr_h == repend && chan.ptr_h > 0 {
                                chan.ptr_h = repend - 1;
                                chan.ptr_l = (1 << FP_PREC) - 1;
                            }
                        } else {
                            chan.flags &= !F_REVERSE;
                            chan.ptr_h -= replen * rep_part;
                        }
                    }
                }
                if chan.flags & F_REVERSE != 0 && chan.ptr_h < reppnt {
                    let (th2, tl2) = (chan.ptr_h, chan.ptr_l);
                    chan.ptr_h = reppnt;
                    chan.ptr_l = 0;
                    fp_sub(&mut chan.ptr_h, &mut chan.ptr_l, th2, tl2);
                    let rep_part = chan.ptr_h / replen;
                    chan.ptr_h += reppnt;
                    if rep_part & 1 != 0 {
                        chan.flags |= F_REVERSE;
                        let (th, tl) = (chan.ptr_h, chan.ptr_l);
                        chan.ptr_h = reppnt + replen * (rep_part + 1);
                        chan.ptr_l = 0;
                        fp_sub(&mut chan.ptr_h, &mut chan.ptr_l, th, tl);
                        chan.ptr_h += reppnt;
                    } else {
                        chan.flags &= !F_REVERSE;
                        chan.ptr_h -= replen * rep_part;
                    }
                }
            }
            let so = chan.ptr_h;
            if so as u32 >= smp_len as u32 {
                chan.flags &= !F_PLAYING;
                chan.id = !0;
                break;
            }
            let (mut s0, mut s1) = (0.0f32, 0.0f32);
            if smp_int != 0 {
                let mut so2 = so + 1;
                let (mut so0, mut so3) = (0, 0);
                if smp_int == 2 {
                    so3 = so + 2;
                    so0 = so - 1;
                    if replen != 0 {
                        if loop_type == 1 {
                            if chan.flags & F_INLOOP != 0 && so0 < reppnt {
                                so0 = repend - 1;
                            }
                            if so2 >= repend {
                                so2 -= replen;
                            }
                            if so3 >= repend {
                                so3 -= replen;
                                chan.flags |= F_INLOOP;
                                if so3 >= smp_len {
                                    so3 = smp_len - 1;
                                }
                            }
                        } else {
                            if chan.flags & F_INLOOP != 0 && so0 < reppnt {
                                so0 = reppnt;
                            }
                            if so2 >= repend {
                                so2 = (repend - 1) - (so2 - repend);
                            }
                            if so3 >= repend {
                                so3 = (repend - 1) - (so3 - repend);
                                chan.flags |= F_INLOOP;
                                if so3 >= smp_len {
                                    so3 = smp_len - 1;
                                }
                            }
                            if so3 < 0 {
                                so3 = 0;
                            }
                        }
                    } else {
                        if so2 >= smp_len {
                            so2 = smp_len - 1;
                        }
                        if so3 >= smp_len {
                            so3 = smp_len - 1;
                        }
                    }
                    if so0 < 0 {
                        so0 = 0;
                    }
                } else if replen != 0 {
                    if loop_type == 1 {
                        if so2 >= repend {
                            so2 = reppnt;
                        }
                    } else if so2 >= repend {
                        so2 = so;
                    }
                } else if so2 >= smp_len {
                    so2 = smp_len - 1;
                }
                if smp_int == 2 {
                    let mu = chan.ptr_l as f32 / (1 << FP_PREC) as f32;
                    for k in 0..1 + sh {
                        let y = |o: i32| get((o << sh) + k);
                        let mut v = spline(y(so0), y(so), y(so2), y(so3), mu);
                        if bits != 2 {
                            v /= scale;
                        }
                        if k == 0 {
                            s0 = v;
                        } else {
                            s1 = v;
                        }
                    }
                } else {
                    let intr = (chan.ptr_l >> (FP_PREC - INTERP_PREC)) as u32;
                    let intr2 = ((1u32 << INTERP_PREC) - 1).wrapping_sub(intr);
                    for k in 0..1 + sh {
                        let x1 = (so << sh) + k;
                        let x2 = (so2 << sh) + k;
                        let v = if bits == 2 {
                            let mut a = get(x1);
                            a *= intr2 as f32 / 32768.0;
                            let mut b = get(x2);
                            b *= intr as f32 / 32768.0;
                            (a + b) * 1.0
                        } else {
                            let up = if bits == 0 { 8 } else { 0 };
                            let a = (geti(x1) << up).wrapping_mul(intr2 as i32);
                            let b = (geti(x2) << up).wrapping_mul(intr as i32);
                            (a.wrapping_add(b) >> INTERP_PREC) as f32 / 32768.0
                        };
                        if k == 0 {
                            s0 = v;
                        } else {
                            s1 = v;
                        }
                    }
                }
            } else {
                for k in 0..1 + sh {
                    let x = (so << sh) + k;
                    let v = match bits {
                        0 => (geti(x) << 8) as f32 / 32768.0,
                        1 => geti(x) as f32 / 32768.0,
                        _ => get(x) * 1.0,
                    };
                    if k == 0 {
                        s0 = v;
                    } else {
                        s1 = v;
                    }
                }
            }
            bufs[0][at + i] = s0;
            if stereo {
                bufs[1][at + i] = s1;
            }
            if chan.flags & F_REVERSE != 0 {
                fp_sub(&mut chan.ptr_h, &mut chan.ptr_l, chan.delta_h as i32, chan.delta_l as i32);
            } else {
                fp_add(&mut chan.ptr_h, &mut chan.ptr_l, chan.delta_h as i32, chan.delta_l as i32);
            }
            i += 1;
        }
        if i > 0 && !stereo {
            let (b0, b1) = bufs.split_at_mut(1);
            b1[0][at..at + i].copy_from_slice(&b0[0][at..at + i]);
        }
        i
    }

    fn apply_volume_envelope(chan: &mut Chan, env_int: i32, bufs: &mut [Vec<f32>; OUTPUTS], at: usize, frames: usize) {
        if env_int != 0 && chan.vol_step != 0 && (chan.l_delta != 0 || chan.r_delta != 0) {
            let frames2 = (chan.vol_step as usize).min(frames);
            for (b, buf) in bufs.iter_mut().enumerate() {
                let (mut vol, delta) = if b == 0 { (chan.l_cur, chan.l_delta) } else { (chan.r_cur, chan.r_delta) };
                for v in buf[at..at + frames2].iter_mut() {
                    vol = vol.wrapping_add(delta);
                    *v = *v * (vol >> 12) as f32 * (1.0f64 / 32768.0) as f32;
                }
                for v in buf[at + frames2..at + frames].iter_mut() {
                    *v = *v * (vol >> 12) as f32 * (1.0f64 / 32768.0) as f32;
                }
                if b == 0 {
                    chan.l_cur = vol;
                } else {
                    chan.r_cur = vol;
                }
            }
            chan.vol_step -= frames2 as i32;
        } else {
            for (b, buf) in bufs.iter_mut().enumerate() {
                let vol = (if b == 0 { chan.l_cur } else { chan.r_cur } >> 12) as f32;
                if vol == 32768.0 {
                    continue;
                }
                if vol == 0.0 {
                    buf[at..at + frames].fill(0.0);
                } else {
                    let vol = vol / 32768.0;
                    for v in buf[at..at + frames].iter_mut() {
                        *v *= vol;
                    }
                }
            }
        }
    }

    fn anticlick_in(chan: &mut Chan, alen: i32, bufs: &mut [Vec<f32>; OUTPUTS], at: usize, frames: usize) {
        let vdelta = (32768 / alen).max(1);
        let mut cnt = 0;
        for (b, buf) in bufs.iter_mut().enumerate() {
            cnt = chan.anticlick_in_counter as i32;
            for v in buf[at..at + frames].iter_mut() {
                let mut s1 = chan.anticlick_in_s[b];
                let mut s2 = *v;
                s1 *= cnt as f32;
                s2 *= (32768 - cnt) as f32;
                s1 /= 32768.0;
                s2 /= 32768.0;
                *v = s1 + s2;
                cnt -= vdelta;
                if cnt < 0 {
                    cnt = 0;
                    break;
                }
            }
        }
        chan.anticlick_in_counter = cnt as u16;
    }

    fn anticlick_out(chan: &mut Chan, alen: i32, bufs: &mut [Vec<f32>; OUTPUTS], at: usize, frames: usize) -> bool {
        let vdelta = (32768 / alen).max(1);
        let mut cnt = 0;
        for buf in bufs.iter_mut() {
            cnt = chan.anticlick_out_counter as i32;
            for v in buf[at..at + frames].iter_mut() {
                let mut s = *v;
                s *= cnt as f32;
                s /= 32768.0;
                *v = s;
                cnt -= vdelta;
                if cnt < 0 {
                    cnt = 0;
                }
            }
        }
        chan.anticlick_out_counter = cnt as u16;
        if cnt == 0 {
            chan.flags &= !F_PLAYING;
            chan.id = !0;
            return true;
        }
        false
    }

    /// sampler_tick: the envelopes, fadeout, panning, volume and vibrato a tick on.
    fn tick(&mut self, chan: &mut Chan, ch: usize, m: &ModCommon, srate: i32) {
        let mut first_tick = false;
        if chan.flags & F_ENV_START != 0 {
            first_tick = true;
            chan.flags &= !F_ENV_START;
        }
        let (ctl_attack, ctl_release, ctl_env_int, ctl_pan, ctl_volume) = (m.ctl(C_ATTACK), m.ctl(C_RELEASE), m.ctl(C_ENV_INT), m.ctl(C_PAN), m.ctl(C_VOLUME));
        let mut r_vol: i32;
        let env = &self.env[ENV_VOL];
        let mut vol_env_on = false;
        if env.flags & ENV_FLAG_ENABLED != 0 && self.env_buf[ENV_VOL].is_some() {
            vol_env_on = true;
            let pos = &mut chan.env_pos[ENV_VOL];
            if env.flags & ENV_FLAG_LOOP != 0 && *pos >= env.pt((env.loop_end as usize) << 1) {
                *pos = env.pt((env.loop_start as usize) << 1);
            }
            let mut finished = false;
            let last = env.pt(((env.point_count as usize).wrapping_sub(1)) << 1);
            if *pos > last {
                *pos = last;
                finished = true;
            }
            r_vol = self.env_buf[ENV_VOL].as_ref().unwrap().get(*pos as usize).copied().unwrap_or(0) as i32;
            if env.flags & ENV_FLAG_LOOP == 0 && finished && r_vol == 0 {
                chan.flags &= !F_PLAYING;
                chan.id = !0;
            }
            if chan.flags & F_SUSTAIN != 0 {
                if chan.fadeout < 65536 {
                    if ctl_attack > 0 {
                        let mut add = (((65536 - chan.fadeout) as u32).wrapping_mul((32768 - ctl_attack) as u32)) / 32768;
                        if add == 0 {
                            add = 1;
                        }
                        chan.fadeout += add as i32;
                    } else {
                        chan.fadeout = 65536;
                    }
                }
            } else if chan.fadeout > 0 {
                if ctl_release < 32768 {
                    chan.fadeout = ((chan.fadeout as u32).wrapping_mul(ctl_release as u32) / 32768) as i32;
                }
                chan.fadeout -= (self.ins.volume_fadeout as i32) << 1;
                if chan.fadeout < 0 {
                    chan.fadeout = 0;
                }
            } else {
                chan.fadeout = 0;
                chan.flags &= !F_PLAYING;
                chan.id = !0;
            }
            if chan.fadeout != 65536 {
                r_vol = r_vol.wrapping_mul(chan.fadeout);
                r_vol >>= 16;
            }
            if env.gain != 100 {
                r_vol = (r_vol * env.gain as i32) / 100;
            }
            if env.velocity_influence != 0 && chan.vel != 256 {
                let vv = 100 - env.velocity_influence as i32;
                r_vol = (((r_vol * chan.vel) >> 8) * env.velocity_influence as i32 + r_vol * vv) / 100;
            }
            let pos = &mut chan.env_pos[ENV_VOL];
            if *pos >= env.pt((env.sustain as usize) << 1) {
                if env.flags & ENV_FLAG_SUSTAIN != 0 {
                    if chan.flags & F_SUSTAIN == 0 {
                        *pos += 1;
                    }
                } else {
                    *pos += 1;
                }
            } else {
                *pos += 1;
            }
        } else {
            r_vol = 32768;
            if chan.flags & F_SUSTAIN == 0 && chan.anticlick_out_counter == 0 && chan.flags & F_PLAYING != 0 {
                chan.anticlick_out_counter = 32768;
            }
        }
        let mut l_vol = r_vol;
        let mut env_val = [0i32; ENV_COUNT];
        for (i, v) in env_val.iter_mut().enumerate().skip(ENV_PAN) {
            *v = if ENVELOPES_SIGNED & (1 << i) != 0 { 16384 } else { 0 };
        }
        for en in ENV_PAN..ENV_COUNT {
            let env = &self.env[en];
            if env.flags & ENV_FLAG_ENABLED == 0 || self.env_buf[en].is_none() {
                continue;
            }
            let pos = &mut chan.env_pos[en];
            if env.flags & ENV_FLAG_LOOP != 0 && *pos >= env.pt((env.loop_end as usize) << 1) {
                *pos = env.pt((env.loop_start as usize) << 1);
            }
            let last = env.pt(((env.point_count as usize).wrapping_sub(1)) << 1);
            if *pos > last {
                *pos = last;
            }
            let mut v = self.env_buf[en].as_ref().unwrap().get(*pos as usize).copied().unwrap_or(0) as i32;
            let signed = ENVELOPES_SIGNED & (1 << en) != 0;
            if signed {
                v -= 16384;
            }
            if env.gain != 100 {
                v = (v * env.gain as i32) / 100;
            }
            if env.velocity_influence != 0 && chan.vel != 256 {
                let vv = 100 - env.velocity_influence as i32;
                v = (((v * chan.vel) >> 8) * env.velocity_influence as i32 + v * vv) / 100;
            }
            if signed {
                v += 16384;
            }
            chan.env_prev[en] = if first_tick { v as u16 } else { chan.env_next[en] };
            chan.env_next[en] = v as u16;
            env_val[en] = if en >= FIRST_ENV_WITH_SUBDIV && ctl_env_int != 0 { chan.env_prev[en] as i32 } else { v };
            let pos = &mut chan.env_pos[en];
            if *pos >= env.pt((env.sustain as usize) << 1) {
                if env.flags & ENV_FLAG_SUSTAIN != 0 {
                    if chan.flags & F_SUSTAIN == 0 {
                        *pos += 1;
                    }
                } else {
                    *pos += 1;
                }
            } else {
                *pos += 1;
            }
        }
        if self.eff_env {
            if let Some(ps) = self.ps.as_mut() {
                if first_tick {
                    ps.module_command(1 + ch, Cmd::Clean);
                }
                for en in ENV_EFF1..=ENV_EFF4 {
                    let env = &self.env[en];
                    if env.flags & ENV_FLAG_ENABLED == 0 {
                        continue;
                    }
                    if first_tick || chan.prev_eff_ctl_val != env_val[en] {
                        Self::eff_ctl(ps, chan, ch, env.eff_ctl as usize, env_val[en]);
                    }
                }
                if first_tick {
                    ps.module_command(1 + ch, Cmd::ApplyControllers);
                }
            }
        }
        let smp = self.smps.get(chan.smp_num as usize).copied().unwrap_or_default();
        let mut pan = env_val[ENV_PAN];
        pan += (ctl_pan - 128) * 128;
        pan += (chan.local_pan - 128) * 128;
        pan += (smp.panning as i32 - 128) * 128;
        pan = pan.clamp(0, 32768);
        let lpan = (32768 - pan).min(16384);
        let rpan = pan.min(16384);
        l_vol = (l_vol * lpan) >> 14;
        r_vol = (r_vol * rpan) >> 14;
        l_vol = (l_vol * smp.volume as i32) >> 6;
        r_vol = (r_vol * smp.volume as i32) >> 6;
        l_vol = (l_vol * ctl_volume) >> 9;
        r_vol = (r_vol * ctl_volume) >> 9;
        if !self.opt.ignore_vel_for_volume {
            l_vol = (l_vol * chan.vel) >> 8;
            r_vol = (r_vol * chan.vel) >> 8;
        }
        if ctl_env_int != 0 {
            if first_tick {
                chan.l_old = l_vol;
                chan.r_old = r_vol;
                if vol_env_on && ctl_attack > 0 {
                    chan.l_old = 0;
                    chan.r_old = 0;
                }
            }
            chan.vol_step = self.tick_size2 >> 8;
            chan.l_delta = ((l_vol - chan.l_old) << 12) / chan.vol_step;
            chan.r_delta = ((r_vol - chan.r_old) << 12) / chan.vol_step;
            chan.l_cur = chan.l_old << 12;
            chan.r_cur = chan.r_old << 12;
            chan.l_old = l_vol;
            chan.r_old = r_vol;
        } else {
            chan.l_cur = l_vol << 12;
            chan.r_cur = r_vol << 12;
        }
        let ins = &self.ins;
        if ins.vibrato_depth != 0 {
            let pos = (chan.vib_pos & 255) as usize;
            let mut v = match ins.vibrato_type & 3 {
                0 => {
                    let v = (super::super::dsp::HSIN_TAB[pos] as i32 * ins.vibrato_depth as i32) >> 8;
                    if chan.vib_pos > 0 {
                        -v
                    } else {
                        v
                    }
                }
                1 => ((-(chan.vib_pos as i32)) >> 1) * ins.vibrato_depth as i32 >> 4,
                2 => {
                    let v = (ins.vibrato_depth as i32 * 127) >> 4;
                    if chan.vib_pos < 0 {
                        -v
                    } else {
                        v
                    }
                }
                _ => 0,
            };
            if ins.vibrato_sweep != 0 {
                v = (v * chan.vib_frame as i32) / ins.vibrato_sweep as i32;
                chan.vib_frame += 2;
                if chan.vib_frame > ins.vibrato_sweep as i16 {
                    chan.vib_frame = ins.vibrato_sweep as i16;
                }
            }
            chan.vib_pos = chan.vib_pos.wrapping_add((ins.vibrato_rate as i16) << 1);
            if chan.vib_pos > 255 {
                chan.vib_pos -= 512;
            }
            chan.pitch_add1 = v * 4;
        } else {
            chan.pitch_add1 = 0;
        }
        chan.pitch_add2 = -(env_val[ENV_PITCH] - 16384);
        self.calc_final_pitch(chan, srate);
    }

    fn subtick(&mut self, chan: &mut Chan, ch: usize, srate: i32, subtick_num: i32) {
        for en in FIRST_ENV_WITH_SUBDIV..ENV_COUNT {
            let env = &self.env[en];
            if env.flags & ENV_FLAG_ENABLED == 0 {
                continue;
            }
            let prev = chan.env_prev[en] as i32;
            let next = chan.env_next[en] as i32;
            let mut v = prev;
            if v != next {
                v += ((next - prev) * subtick_num) >> self.tick_subdiv;
            }
            if en == ENV_PITCH {
                chan.pitch_add2 = -(v - 16384);
                self.calc_final_pitch(chan, srate);
            }
            if (ENV_EFF1..=ENV_EFF4).contains(&en) && chan.prev_eff_ctl_val != v {
                let ec = env.eff_ctl as usize;
                if let Some(ps) = self.ps.as_mut() {
                    Self::eff_ctl(ps, chan, ch, ec, v);
                }
            }
        }
    }

    /// sampler_eff_ctl.
    fn eff_ctl(ps: &mut SubEngine, chan: &mut Chan, ch: usize, ctl_num: usize, val: i32) {
        if let Some((typ, min, max)) = ps.module_ctl(1, ctl_num) {
            let v = if typ == 0 { val } else { ((val * (max - min)) >> 15) + min };
            ps.module_ctl_event(1 + ch, ctl_num, v);
        }
        chan.prev_eff_ctl_val = val;
    }

    fn handle_fit_to_pattern(&self, chan: &mut Chan, bufs: &[Vec<f32>; OUTPUTS], at: usize, rendered: usize) {
        if chan.tick_counter2 == self.opt.fit_to_pattern as i32 * self.ticks_per_line as i32 {
            chan.tick_counter2 = 0;
            let smp = self.smps.get(chan.smp_num as usize).copied().unwrap_or_default();
            let p = (chan.ptr_h as i64 * 32 / smp.length.max(1) as i64) as i32;
            if p == 0 || p == 31 {
                chan.ptr_h = 0;
                chan.ptr_l = 0;
                if rendered != 0 {
                    for (b, buf) in bufs.iter().enumerate() {
                        let s = buf[at + rendered - 1];
                        chan.anticlick_in_s[b] = s;
                        if s != 0.0 && chan.anticlick_in_counter == 0 {
                            chan.anticlick_in_counter = 32768;
                        }
                    }
                }
            }
        }
        chan.tick_counter2 += 1;
    }

    fn setup(&mut self, n: usize, net: &mut Net) {
        let size = net.mods[n].chunk(CHUNK_INS).map_or(0, |c| c.data.len());
        if size < INS_SIZE {
            if let Some(c) = net.mods[n].chunk_mut(CHUNK_INS) {
                c.data.resize(INS_SIZE, 0);
            }
        }
        for s in 0..MAX_SAMPLES {
            if let Some(c) = net.mods[n].chunk_mut(chunk_smp(s)) {
                if c.data.len() < SMP_SIZE {
                    c.data.resize(SMP_SIZE, 0);
                }
            }
        }
        let mut prev_version = 0u32;
        let mut prev_max_version = 0u32;
        if let Some(c) = net.mods[n].chunk_mut(CHUNK_INS) {
            let d = &mut c.data;
            if rd_u32(d, 252) != INS_SIGN {
                let old: Vec<u8> = d[36..132].to_vec();
                d[260..356].copy_from_slice(&old);
                let last = old[95];
                for b in d[356..388].iter_mut() {
                    *b = last;
                }
                wr_u32(d, 252, INS_SIGN);
                wr_u32(d, 256, INS_VERSION);
                wr_u32(d, 388, INS_VERSION);
            } else {
                prev_version = rd_u32(d, 256);
                prev_max_version = rd_u32(d, 388);
                wr_u32(d, 256, INS_VERSION);
                if prev_version < 6 {
                    wr_u32(d, 388, INS_VERSION);
                    prev_max_version = prev_version;
                } else if INS_VERSION > rd_u32(d, 388) {
                    wr_u32(d, 388, INS_VERSION);
                }
            }
        }
        if prev_version < 5 {
            Self::convert_old_envelopes(net, n);
        }
        if prev_max_version < 6 {
            if let Some(c) = net.mods[n].chunk_mut(CHUNK_INS) {
                wr_u32(&mut c.data, 392, 0);
                wr_u32(&mut c.data, 396, 0);
            }
            for s in 0..MAX_SAMPLES {
                if let Some(c) = net.mods[n].chunk_mut(chunk_smp(s)) {
                    wr_u32(&mut c.data, 40, 0);
                }
            }
        }
        self.smps = vec![Smp::default(); MAX_SAMPLES];
        self.data = vec![SmpData::None; MAX_SAMPLES];
        for s in 0..MAX_SAMPLES {
            let Some(sd) = net.mods[n].chunk(chunk_smp(s)).map(|c| Smp::read(&c.data)) else { continue };
            self.smps[s] = sd;
            if let Some(c) = net.mods[n].chunk_mut(chunk_smp_data(s)) {
                let mut flags = c.flags & !(net::CHUNK_SMP_TYPE_MASK | (3 << 3));
                let bits = match (sd.typ >> 4) & 3 {
                    1 => 16,
                    2 => 32,
                    _ => 8,
                };
                let channels = if sd.typ & SMP_STEREO != 0 { 2 } else { 1 };
                flags |= match bits {
                    32 => net::CHUNK_SMP_FLOAT32,
                    16 => net::CHUNK_SMP_INT16,
                    _ => net::CHUNK_SMP_INT8,
                };
                flags |= (channels - 1) << 3;
                c.flags = flags;
                let d = &c.data;
                self.data[s] = match bits {
                    8 => SmpData::I8(d.iter().map(|&b| b as i8).collect()),
                    16 => SmpData::I16(d.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]])).collect()),
                    _ => SmpData::F32(d.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect()),
                };
                let m = &net.mods[n];
                self.recalc_base_pitch(s, m);
            }
        }
        self.ins = net.mods[n].chunk(CHUNK_INS).map(|c| Ins::read(&c.data)).unwrap_or_default();
        self.read_envs(&net.mods[n]);
        self.refresh_envelopes();
        self.handle_envelope_flags();
        net.chunk_autocreate(n, CHUNK_OPT, 8, true);
        let o = &net.mods[n].chunk(CHUNK_OPT).unwrap().data;
        self.opt = Opt { rec_on_play: o[0] != 0, ignore_vel_for_volume: o[5] != 0, freq_accuracy: o[6] != 0, fit_to_pattern: o[7] };
        if let Some(file) = net.mods[n].chunk(CHUNK_EFFECT_MOD).map(|c| c.data.clone()) {
            if self.ps.is_none() {
                self.ps = Some(SubEngine::new(net, n, true));
            }
            let loaded = self.ps.as_mut().is_some_and(|ps| ps.set_module(&file, MAX_CHANNELS));
            if loaded {
                net.remove_chunk(n, CHUNK_EFFECT_MOD);
            }
        }
    }

    fn render(&mut self, n: usize, net: &mut Net) -> isize {
        if self.no_active_channels {
            return 0;
        }
        let srate = net.sampling_freq;
        let mut retval = 0;
        let m = &net.mods[n];
        let (offset, frames) = (m.offset as usize, m.frames as usize);
        let smp_int = m.ctl(C_SMP_INT);
        let env_int = m.ctl(C_ENV_INT);
        let ctl_channels = m.ctl(C_CHANNELS);
        self.tick_size2 = ((self.tick_size as u32 as u64 * m.ctl(C_TICK_SCALE) as u32 as u64) / 128) as i32;
        if self.tick_size2 < self.min_tick_size2 {
            self.tick_size2 = self.min_tick_size2;
        }
        let mut subtick_size = 0;
        if self.tick_subdiv != 0 && env_int != 0 {
            subtick_size = (self.tick_size2 >> self.tick_subdiv).max(1);
        }
        for b in self.bufs.iter_mut() {
            if b.len() < frames {
                b.resize(frames, 0.0);
            }
        }
        self.no_active_channels = true;
        let alen = self.anticlick_len as i32;
        for c in 0..ctl_channels.max(0) as usize {
            let mut chan = self.channels[c];
            if chan.flags & F_PLAYING == 0 {
                continue;
            }
            let si = chan.smp_num as usize;
            if net.mods[n].chunk(chunk_smp(si)).is_none() || net.mods[n].chunk(chunk_smp_data(si)).is_none() {
                continue;
            }
            let smp = self.smps[si];
            self.no_active_channels = false;
            chan.flags &= !F_READY_TO_PLAY;
            if chan.tick_counter == 0xFFFFFFF {
                chan.tick_counter = self.tick_size2;
                chan.subtick_counter = subtick_size;
            }
            // The first voice renders into the outputs; later ones into the
            // temporary buffers, added after.
            let into_out = retval == 0;
            let mut bufs = std::mem::take(&mut self.bufs);
            if into_out {
                for (b, o) in bufs.iter_mut().zip(net.mods[n].chout.iter()) {
                    b[..frames].copy_from_slice(&o[offset..offset + frames]);
                }
            }
            let mut ptr = 0usize;
            let mut sample_finished = false;
            while !sample_finished {
                let mut bs = (frames - ptr) as i32;
                let left = self.tick_size2 - chan.tick_counter;
                if bs > left / 256 {
                    bs = left / 256;
                }
                if left & 255 != 0 {
                    bs += 1;
                }
                if subtick_size != 0 {
                    let left = subtick_size - chan.subtick_counter;
                    if bs > left / 256 {
                        bs = left / 256;
                    }
                    if left & 255 != 0 {
                        bs += 1;
                    }
                }
                if bs > (frames - ptr) as i32 {
                    bs = (frames - ptr) as i32;
                }
                if bs < 0 {
                    bs = 0;
                }
                if chan.anticlick_out_counter != 0 && alen * 2 < bs {
                    bs = alen * 2;
                }
                let mut bs = bs as usize;
                let data = std::mem::take(&mut self.data[si]);
                let rendered = Self::render_voice(&mut chan, &smp, &data, smp_int, &mut bufs, ptr, bs);
                self.data[si] = data;
                if rendered < bs {
                    bs = rendered;
                    sample_finished = true;
                }
                if rendered != 0 {
                    if self.eff_env {
                        if let Some(ps) = self.ps.as_mut() {
                            ps.apply_module(1 + c, &mut bufs, ptr, rendered);
                        }
                    }
                    Self::apply_volume_envelope(&mut chan, env_int, &mut bufs, ptr, rendered);
                    if chan.anticlick_in_counter != 0 {
                        Self::anticlick_in(&mut chan, alen, &mut bufs, ptr, rendered);
                    }
                    if chan.anticlick_out_counter != 0 && Self::anticlick_out(&mut chan, alen, &mut bufs, ptr, rendered) {
                        sample_finished = true;
                    }
                }
                ptr += bs;
                chan.tick_counter += bs as i32 * 256;
                if chan.tick_counter >= self.tick_size2 {
                    if self.opt.fit_to_pattern != 0 && !sample_finished {
                        self.handle_fit_to_pattern(&mut chan, &bufs, ptr - bs, rendered);
                    }
                    let m = &net.mods[n];
                    self.tick(&mut chan, c, m, srate);
                    chan.tick_counter %= self.tick_size2;
                    if subtick_size != 0 {
                        chan.subtick_counter = chan.tick_counter % subtick_size;
                    }
                } else if subtick_size != 0 {
                    chan.subtick_counter += bs as i32 * 256;
                    if chan.subtick_counter >= subtick_size {
                        let subtick_num = chan.tick_counter / subtick_size;
                        if subtick_num < (1 << self.tick_subdiv) {
                            self.subtick(&mut chan, c, srate, subtick_num);
                        }
                        chan.subtick_counter %= subtick_size;
                    }
                }
                if ptr >= frames {
                    break;
                }
            }
            let m = &mut net.mods[n];
            if ptr > 0 {
                if into_out {
                    if ptr < frames {
                        for b in bufs.iter_mut() {
                            b[ptr..frames].fill(0.0);
                        }
                    }
                    for (b, o) in bufs.iter().zip(m.chout.iter_mut()) {
                        o[offset..offset + frames].copy_from_slice(&b[..frames]);
                    }
                    retval = 1;
                } else {
                    for (b, o) in bufs.iter().zip(m.chout.iter_mut()) {
                        for i in 0..ptr {
                            o[offset + i] += b[i];
                        }
                    }
                }
            } else if into_out {
                for (b, o) in bufs.iter().zip(m.chout.iter_mut()) {
                    o[offset..offset + frames].copy_from_slice(&b[..frames]);
                }
            }
            if ptr >= frames {
                for p in 0..OUTPUTS {
                    chan.last_s[p] = bufs[p][frames - 1];
                }
            } else {
                chan.last_s = [0.0; OUTPUTS];
            }
            self.bufs = bufs;
            self.channels[c] = chan;
        }
        retval
    }
}

impl Module for Sampler {
    fn handle(&mut self, n: usize, e: &mut Event, net: &mut Net) -> isize {
        let mut retval = 0;
        match e.cmd {
            Cmd::Init => {
                net.register_ctl(n, 0, 512, 256, 0, C_VOLUME, -1, 0);
                net.register_ctl(n, 0, 256, 128, 0, C_PAN, 128, 0);
                net.set_ctl_show_offset(n, 1, -128);
                net.register_ctl(n, 0, 2, 2, 1, C_SMP_INT, -1, 1);
                net.register_ctl(n, 0, 1, 1, 1, C_ENV_INT, -1, 1);
                net.register_ctl(n, 1, MAX_CHANNELS as i32, 8, 1, C_CHANNELS, -1, 2);
                net.register_ctl(n, 0, 10000, 4, 0, C_REC_THRESHOLD, -1, 3);
                net.set_ctl_flags(n, 5, 2);
                net.register_ctl(n, 0, 2048, 128, 0, C_TICK_SCALE, 128, 4);
                net.set_ctl_flags(n, 6, 2);
                net.register_ctl(n, 0, 2, 0, 1, C_RECORD, -1, 3);
                net.register_ctl(n, 0, 1, 0, 1, C_REVERSE, -1, 5);
                net.register_ctl(n, 0, 32768, 0, 0, C_ATTACK, -1, 4);
                net.register_ctl(n, 0, 32768, 32768, 0, C_RELEASE, -1, 4);
                net.set_ctl_flags(n, 9, 3);
                net.set_ctl_flags(n, 10, 3);
                for c in self.channels.iter_mut() {
                    *c = Chan { id: !0, ..Chan::default() };
                }
                self.no_active_channels = true;
                self.search_ptr = 0;
                self.last_ch = 0;
                self.tick_size = 1;
                self.tick_size2 = 1;
                self.min_tick_size2 = (64i64 * 256 * net.sampling_freq as i64 / 44100) as i32;
                self.tick_subdiv = 0;
                self.anticlick_len = (32 * net.sampling_freq / 44100) as u16;
                self.sample_base_pitch = vec![0; MAX_SAMPLES];
                self.sample_base_freq = vec![0; MAX_SAMPLES];
                for i in 0..ENV_COUNT {
                    self.env_buf[i] = None;
                    net.new_chunk_sized(n, chunk_env(i), ENV_HEADER + 4, 0, 0);
                }
                // new_instrument: the instrument chunk and the default envelopes.
                net.new_chunk_sized(n, CHUNK_INS, INS_SIZE, 0, 0);
                if let Some(c) = net.mods[n].chunk_mut(CHUNK_INS) {
                    c.data[244] = 64;
                    wr_u32(&mut c.data, 252, INS_SIGN);
                    wr_u32(&mut c.data, 256, INS_VERSION);
                    wr_u32(&mut c.data, 388, INS_VERSION);
                }
                Self::make_default_envelopes(net, n);
                self.read_envs(&net.mods[n]);
                self.handle_envelope_flags();
                self.refresh_envelopes();
                self.ins = net.mods[n].chunk(CHUNK_INS).map(|c| Ins::read(&c.data)).unwrap_or_default();
                retval = 1;
            }
            Cmd::SetupFinished => {
                self.setup(n, net);
                retval = 1;
            }
            Cmd::RenderReplace => retval = self.render(n, net),
            Cmd::NoteOn => {
                let ctl_channels = net.mods[n].ctl(C_CHANNELS).max(1) as usize;
                if !self.no_active_channels {
                    for c in 0..MAX_CHANNELS {
                        if self.channels[c].id == e.id {
                            if self.channels[c].flags & F_SUSTAIN != 0 {
                                let mut ch = self.channels[c];
                                self.release_channel(&mut ch);
                                self.channels[c] = ch;
                            }
                            self.channels[c].id = !0;
                            break;
                        }
                    }
                    let mut c = 0;
                    while c < ctl_channels {
                        if self.channels[self.search_ptr].flags & F_PLAYING == 0 {
                            break;
                        }
                        self.search_ptr += 1;
                        if self.search_ptr >= ctl_channels {
                            self.search_ptr = 0;
                        }
                        c += 1;
                    }
                    if c == ctl_channels {
                        self.search_ptr += 1;
                        if self.search_ptr >= ctl_channels {
                            self.search_ptr = 0;
                        }
                        if net.base_host_version >= 0x02010301 {
                            for _ in 0..ctl_channels {
                                if self.channels[self.search_ptr].flags & F_SUSTAIN == 0 {
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
                self.last_ch = c;
                let base = net.base_host_version;
                let finetune = if base == 0x01070000 { (net::NOTE0_PITCH - e.root_note() as i32 * 256) - e.pitch() } else { 0 };
                let mut note_num = if base == 0x01070000 { e.root_note() as i32 } else { (net::NOTE0_PITCH - (e.pitch() - 128)) / 256 };
                if note_num < 0 {
                    note_num = 0;
                }
                let smp_note_num = note_num.min(127);
                let mut ch = self.channels[c];
                ch.smp_note_num = smp_note_num as i16;
                let smp_num = self.ins.smp_num[smp_note_num as usize] as usize;
                if !(self.ins.samples_num != 0 && smp_num < self.ins.samples_num as usize) {
                    self.channels[c] = ch;
                    return 0;
                }
                ch.smp_num = smp_num as i16;
                if net.mods[n].chunk(chunk_smp(smp_num)).is_none() {
                    self.channels[c] = ch;
                    return 0;
                }
                let smp = self.smps[smp_num];
                let ins = &self.ins;
                let pitch = if base == 0x01070000 {
                    note_num += smp.relative_note as i32;
                    note_num += ins.relative_note as i32;
                    if note_num < 0 {
                        note_num = 0;
                    }
                    7680 - (note_num * 64) - ((smp.finetune as i32) >> 1) - ((ins.finetune as i32) >> 1) - (finetune >> 2)
                } else {
                    (e.pitch() >> 2) - (smp.relative_note as i32 * 64) - (ins.relative_note as i32 * 64) - ((smp.finetune as i32) >> 1) - ((ins.finetune as i32) >> 1)
                };
                ch.cur_pitch = pitch * 4;
                ch.cur_pitch_delta = ch.cur_pitch - e.pitch();
                ch.final_pitch = 0xFFFFFFF;
                ch.anticlick_in_counter = 0;
                ch.anticlick_out_counter = 0;
                if ch.flags & F_PLAYING != 0 {
                    for p in 0..OUTPUTS {
                        let s = ch.last_s[p];
                        ch.anticlick_in_s[p] = s;
                        if s != 0.0 && ch.anticlick_in_counter == 0 {
                            ch.anticlick_in_counter = 32768;
                        }
                    }
                }
                ch.flags |= F_PLAYING | F_READY_TO_PLAY;
                ch.vel = e.velocity() as i32;
                ch.id = e.id;
                ch.local_pan = 128;
                ch.local_reverse = 0;
                let attack = net.mods[n].ctl(C_ATTACK);
                self.reset_channel(&mut ch, attack);
                ch.ptr_h = smp.start_pos as i32;
                Self::init_reverse_flags(&mut ch, &smp, net.mods[n].ctl(C_REVERSE));
                self.channels[c] = ch;
                self.no_active_channels = false;
                retval = 1;
            }
            Cmd::SetFreq => {
                if self.no_active_channels {
                    return 0;
                }
                let ctl_channels = net.mods[n].ctl(C_CHANNELS).max(0) as usize;
                for c in 0..ctl_channels.min(MAX_CHANNELS) {
                    if self.channels[c].id == e.id {
                        let mut ch = self.channels[c];
                        ch.vel = e.velocity() as i32;
                        ch.cur_pitch = e.pitch() + ch.cur_pitch_delta;
                        self.calc_final_pitch(&mut ch, net.sampling_freq);
                        self.channels[c] = ch;
                        break;
                    }
                }
                retval = 1;
            }
            Cmd::SetVelocity => {
                if self.no_active_channels {
                    return 0;
                }
                let ctl_channels = net.mods[n].ctl(C_CHANNELS).max(0) as usize;
                for c in 0..ctl_channels.min(MAX_CHANNELS) {
                    if self.channels[c].id == e.id {
                        self.channels[c].vel = e.velocity() as i32;
                        break;
                    }
                }
                retval = 1;
            }
            Cmd::SetSampleOffset | Cmd::SetSampleOffset2 => {
                if self.no_active_channels {
                    return 0;
                }
                let ctl_channels = net.mods[n].ctl(C_CHANNELS).max(0) as usize;
                for c in 0..ctl_channels.min(MAX_CHANNELS) {
                    if self.channels[c].id == e.id {
                        let sn = self.channels[c].smp_num as usize;
                        if !(self.ins.samples_num != 0 && sn < self.ins.samples_num as usize) || net.mods[n].chunk(chunk_smp(sn)).is_none() {
                            break;
                        }
                        let smp = self.smps[sn];
                        let mut new_offset = if e.cmd == Cmd::SetSampleOffset {
                            smp.start_pos.wrapping_add(e.sample_offset())
                        } else {
                            let len = smp.length.wrapping_sub(smp.start_pos);
                            smp.start_pos.wrapping_add(((len as u64 * e.sample_offset() as u64) / 0x8000) as u32)
                        };
                        if new_offset >= smp.length {
                            new_offset = smp.length.wrapping_sub(1);
                        }
                        self.channels[c].ptr_h = new_offset as i32;
                        self.channels[c].ptr_l = 0;
                        retval = 1;
                        break;
                    }
                }
            }
            Cmd::NoteOff => {
                if self.no_active_channels {
                    return 0;
                }
                for c in 0..MAX_CHANNELS {
                    if self.channels[c].id == e.id {
                        let mut ch = self.channels[c];
                        self.release_channel(&mut ch);
                        self.channels[c] = ch;
                        break;
                    }
                }
                retval = 1;
            }
            Cmd::AllNotesOff => {
                for c in 0..MAX_CHANNELS {
                    let mut ch = self.channels[c];
                    self.release_channel(&mut ch);
                    ch.id = !0;
                    self.channels[c] = ch;
                }
                retval = 1;
            }
            Cmd::Clean => {
                for c in self.channels.iter_mut() {
                    c.flags &= !F_PLAYING;
                    c.id = !0;
                }
                self.no_active_channels = true;
                retval = 1;
            }
            Cmd::SetLocalController | Cmd::SetGlobalController => {
                if e.cmd == Cmd::SetLocalController {
                    let ctl_channels = net.mods[n].ctl(C_CHANNELS).max(0) as usize;
                    for c in 0..ctl_channels.min(MAX_CHANNELS) {
                        if self.channels[c].id == e.id {
                            match e.ctl_num() + 1 {
                                2 => {
                                    self.channels[c].local_pan = (e.ctl_val() >> 7) as i32;
                                    retval = 1;
                                }
                                9 => {
                                    let ch = &mut self.channels[c];
                                    let v = (e.ctl_val() >= 1) as i32;
                                    if ch.local_reverse != v {
                                        ch.local_reverse = v;
                                        ch.flags ^= F_REVERSE;
                                        if ch.flags & F_READY_TO_PLAY != 0 {
                                            let smp = self.smps[ch.smp_num as usize];
                                            let r = net.mods[n].ctl(C_REVERSE);
                                            Self::init_reverse_flags(ch, &smp, r);
                                        }
                                    }
                                    retval = 1;
                                }
                                _ => {}
                            }
                            break;
                        }
                    }
                    if retval != 0 {
                        return retval;
                    }
                }
                let v = e.ctl_val() as i32;
                let mut changed_opt = true;
                match e.ctl_num() + 1 {
                    8 => changed_opt = false,
                    9 => {
                        changed_opt = false;
                        let prev = net.mods[n].ctl(C_REVERSE);
                        if prev != v && !self.no_active_channels {
                            for c in 0..MAX_CHANNELS {
                                let ch = &mut self.channels[c];
                                if ch.flags & F_PLAYING == 0 {
                                    continue;
                                }
                                ch.flags ^= F_REVERSE;
                                if ch.flags & F_READY_TO_PLAY != 0 {
                                    let smp = self.smps[ch.smp_num as usize];
                                    Self::init_reverse_flags(ch, &smp, v);
                                }
                            }
                        }
                    }
                    127 => {
                        self.opt.rec_on_play = v != 0;
                        retval = 1;
                    }
                    122 => {
                        self.opt.ignore_vel_for_volume = v != 0;
                        retval = 1;
                    }
                    121 => {
                        self.opt.freq_accuracy = v != 0;
                        retval = 1;
                    }
                    120 => {
                        self.opt.fit_to_pattern = v as u8;
                        retval = 1;
                    }
                    126 | 125 | 124 | 123 => retval = 1,
                    _ => changed_opt = false,
                }
                if changed_opt {
                    if let Some(c) = net.mods[n].chunk_mut(CHUNK_OPT) {
                        let k = 127 - (e.ctl_num() as usize + 1);
                        if k < 8 {
                            c.data[k] = if k == 7 { v as u8 } else { (v != 0) as u8 };
                        }
                    }
                }
            }
            Cmd::SpeedChanged => {
                self.tick_size = e.tick_size();
                self.ticks_per_line = e.tpl();
                retval = 1;
            }
            Cmd::Play | Cmd::Stop => retval = 1,
            Cmd::Close => retval = 1,
            _ => {}
        }
        retval
    }
}
