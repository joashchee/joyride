// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! Generator (psynths_generator.cpp): a polyphonic oscillator (triangle,
//! saw, square, noise, a drawn 32-point wave, sine and its halves) under
//! an attack and release, its pitch modulated by its input.

use super::super::net::{self, Cmd, Event, Module, Net};

const MAX_CHANNELS: usize = 16;

pub(crate) const DRAWN_WAVEFORM: [i8; 32] = [0, -100, -90, 0, 90, -119, -20, 45, 2, -20, 111, -23, 2, -98, 60, 32, 100, 50, 0, -50, 65, 98, 50, 32, -90, -120, 100, 90, 59, 21, 0, 54];

const C_VOLUME: usize = 0;
const C_TYPE: usize = 1;
const C_PAN: usize = 2;
const C_ATTACK: usize = 3;
const C_RELEASE: usize = 4;
const C_CHANNELS: usize = 5;
const C_MODE: usize = 6;
const C_SUSTAIN: usize = 7;
const C_MOD: usize = 8;
const C_DUTY: usize = 9;

const MODE_MONO: i32 = 1;
const TYPES: i32 = 9;

const T_TRI: i32 = 0;
const T_SAW: i32 = 1;
const T_SQ: i32 = 2;
const T_NOISE: i32 = 3;
const T_DRAWN: i32 = 4;
const T_SIN: i32 = 5;
const T_HSIN: i32 = 6;
const T_ASIN: i32 = 7;
const T_PSIN: i32 = 8;

#[derive(Clone, Copy, Default)]
struct Chan {
    playing: bool,
    id: u32,
    vel: i32,
    ptr: u32,
    delta: u32,
    env_vol: u32,
    sustain: bool,
    local_type: i32,
    local_pan: i32,
}

#[derive(Default)]
pub(crate) struct Generator {
    channels: [Chan; MAX_CHANNELS],
    no_active_channels: bool,
    drawn_wave: [i8; 32],
    drawn_wave_changed: bool,
    search_ptr: usize,
    wrong_saw_generation: bool,
}

fn s16(v: i32) -> f32 {
    v as f32 / 32768.0
}

impl Generator {
    /// One frame of a waveform at `ptr` (the GET_VAL_* macros).
    #[inline]
    fn val(&self, ty: i32, ptr: u32, add: i32, duty: u32, wrong_saw: bool, min_max: (f32, f32)) -> f32 {
        match ty {
            T_SAW => {
                if wrong_saw {
                    s16(32767 - ((ptr & 65535) / 32 + ((ptr >> 16) & 31) * 4096) as i32)
                } else {
                    s16(32767 - ((ptr >> 4) & 65535) as i32)
                }
            }
            T_SQ => {
                if ((ptr >> 11) & 1023) > duty {
                    min_max.1
                } else {
                    min_max.0
                }
            }
            T_NOISE => s16(net::noise_table()[((ptr >> 16).wrapping_add(add as u32) & 32767) as usize] as i32 * 256),
            T_DRAWN => s16(self.drawn_wave[((ptr >> 18) & 31) as usize] as i32 * 256),
            T_SIN => {
                let p2 = (ptr.wrapping_sub(4 << 16) & ((1 << 21) - 1)) as i32;
                ((p2 as f32 / (1 << 20) as f32) * std::f32::consts::PI).sin()
            }
            T_HSIN => {
                let p2 = (ptr.wrapping_sub(4 << 16) & ((1 << 21) - 1)) as i32;
                if p2 & (16 << 16) == 0 {
                    ((p2 as f32 / (1 << 20) as f32) * std::f32::consts::PI).sin()
                } else {
                    0.0
                }
            }
            T_ASIN => {
                let p2 = (ptr.wrapping_sub(4 << 16) & ((1 << 20) - 1)) as i32;
                ((p2 as f32 / (1 << 20) as f32) * std::f32::consts::PI).sin()
            }
            T_PSIN => {
                let p2 = (ptr.wrapping_sub(4 << 16) & ((1 << 20) - 1)) as i32;
                if (((p2 >> 10) & 1023) as u32) < duty {
                    ((p2 as f32 / (1 << 20) as f32) * std::f32::consts::PI).sin()
                } else {
                    0.0
                }
            }
            _ => {
                if ((ptr >> 16) & 15) < 8 {
                    s16(-32767 + ((ptr >> 3) & 65535) as i32)
                } else {
                    s16(32767 - ((ptr >> 3) & 65535) as i32)
                }
            }
        }
    }

    fn render(&mut self, n: usize, net: &mut Net) -> isize {
        let mut retval: isize = 0;
        let (ctl_volume, ctl_pan, ctl_attack, ctl_release, ctl_channels, ctl_mode, ctl_sustain, ctl_mod, ctl_duty) = {
            let m = &net.mods[n];
            (m.ctl(C_VOLUME), m.ctl(C_PAN), m.ctl(C_ATTACK), m.ctl(C_RELEASE), m.ctl(C_CHANNELS), m.ctl(C_MODE), m.ctl(C_SUSTAIN), m.ctl(C_MOD), m.ctl(C_DUTY))
        };
        let outs_wanted = if ctl_mode == MODE_MONO { 1 } else { 2 };
        if net.mods[n].output_channels != outs_wanted {
            net.set_number_of_outputs(outs_wanted, n);
        }
        let srate = net.sampling_freq;
        let base = net.base_host_version;
        let m = &mut net.mods[n];
        let (offset, frames) = (m.offset as usize, m.frames as usize);
        let mut no_env = false;
        let (mut attack_delta, mut release_delta) = (0i32, 0i32);
        let mut env_ready = false;
        let mut zero_channels = 0u32;
        self.no_active_channels = true;
        for c in 0..ctl_channels.max(0) as usize {
            let chan = self.channels[c];
            if !chan.playing {
                continue;
            }
            self.no_active_channels = false;
            if !env_ready {
                if ctl_attack == 0 && ctl_release == 0 {
                    no_env = true;
                }
                attack_delta = 1 << 30;
                release_delta = 1 << 30;
                let mut attack_len = (srate * ctl_attack) / 256;
                let mut release_len = (srate * ctl_release) / 256;
                if base < 0x01090600 {
                    attack_len &= !1023;
                    release_len &= !1023;
                }
                if attack_len != 0 {
                    attack_delta = (1 << 30) / attack_len;
                }
                if release_len != 0 {
                    release_delta = (1 << 30) / release_len;
                }
                env_ready = true;
            }
            let delta = chan.delta;
            let mut sustain = chan.sustain;
            let mut ptr = 0u32;
            let mut playing = false;
            let sustain_enabled = ctl_sustain != 0;
            let mut env_vol = 0u32;
            let outputs_num = m.output_channels;
            for ch in 0..outputs_num.max(0) as usize {
                sustain = chan.sustain;
                ptr = chan.ptr;
                env_vol = chan.env_vol;
                playing = chan.playing;
                if no_env {
                    if !sustain_enabled {
                        sustain = false;
                    }
                    if !sustain {
                        playing = false;
                    }
                }
                if !playing {
                    continue;
                }
                let mut vol = (ctl_volume * chan.vel) >> 8;
                let pan = (ctl_pan + (chan.local_pan - 128)).clamp(0, 256);
                if pan < 128 {
                    if ch == 1 {
                        vol *= pan;
                        vol >>= 7;
                    }
                } else if ch == 0 {
                    vol *= 128 - (pan - 128);
                    vol >>= 7;
                }
                if vol != 0 {
                    retval = 1;
                }
                let add = (ch as i32) * 32768 / 2;
                let ty = chan.local_type;
                let duty = ctl_duty as u32;
                if zero_channels & (1 << ch) == 0 {
                    let empty = (m.out_empty[ch] - offset as i32).max(0) as usize;
                    if empty < frames {
                        m.chout[ch][offset + empty..offset + frames].fill(0.0);
                    }
                    zero_channels |= 1 << ch;
                }
                let volf = vol as f32;
                let mut sq = (s16(32767), s16(-32767));
                let (inp, out) = {
                    let (ci, co) = (&m.chin, &mut m.chout);
                    (&ci[0], &mut co[ch])
                };
                if ctl_mod != 0 {
                    let mut max_val = s16(32767);
                    let mut min_val = s16(-32767);
                    if base < 0x02000005 {
                        max_val = max_val * volf / 256.0;
                        min_val = min_val * volf / 256.0;
                    }
                    let mod_freq = net::get_freq(100);
                    let mut mod_delta = net::get_delta64(srate, mod_freq) as u32;
                    mod_delta = ((mod_delta as u64 * (((ctl_mod << (16 - 8)) / 8) as i64 as u64)) >> 16) as u32;
                    for i in 0..frames {
                        let mut val = self.val(ty, ptr, add, duty, false, (min_val, max_val));
                        val = val * volf / 256.0;
                        val = val * (env_vol >> 15) as f32 / 32768.0;
                        out[offset + i] += val;
                        ptr = ptr.wrapping_add(delta);
                        if sustain {
                            env_vol = env_vol.wrapping_add(attack_delta as u32);
                            if env_vol >= 1 << 30 {
                                env_vol = 1 << 30;
                                if !sustain_enabled {
                                    sustain = false;
                                }
                            }
                        } else {
                            env_vol = env_vol.wrapping_sub(release_delta as u32);
                            if env_vol > 1 << 30 {
                                env_vol = 0;
                                playing = false;
                                break;
                            }
                        }
                        let mut in_v = inp[offset + i];
                        let mut neg = false;
                        if in_v < 0.0 {
                            neg = true;
                            in_v = -in_v;
                        }
                        if in_v > 1.0 {
                            in_v = 1.0;
                        }
                        let mul = to_int16(in_v) as u32;
                        let mod_delta2 = ((mod_delta as u64 * mul as u64) >> 16) as u32;
                        if neg {
                            ptr = ptr.wrapping_sub(mod_delta2);
                        } else {
                            ptr = ptr.wrapping_add(mod_delta2);
                        }
                    }
                } else {
                    let wrong_saw = self.wrong_saw_generation && ty == 1;
                    if ty == T_SQ {
                        sq = (s16(-32767) * volf / 256.0, s16(32767) * volf / 256.0);
                    }
                    if no_env && !wrong_saw {
                        for i in 0..frames {
                            let mut val = self.val(ty, ptr, add, duty, false, sq);
                            if ty != T_SQ {
                                val = val * volf / 256.0;
                            }
                            out[offset + i] += val;
                            ptr = ptr.wrapping_add(delta);
                        }
                    } else {
                        for i in 0..frames {
                            let mut val = self.val(ty, ptr, add, duty, wrong_saw, sq);
                            if ty != T_SQ {
                                val = val * volf / 256.0;
                            }
                            val = val * (env_vol >> 15) as f32 / 32768.0;
                            out[offset + i] += val;
                            ptr = ptr.wrapping_add(delta);
                            if sustain {
                                env_vol = env_vol.wrapping_add(attack_delta as u32);
                                if env_vol >= 1 << 30 {
                                    env_vol = 1 << 30;
                                    if !sustain_enabled {
                                        sustain = false;
                                    }
                                }
                            } else {
                                env_vol = env_vol.wrapping_sub(release_delta as u32);
                                if env_vol > 1 << 30 {
                                    env_vol = 0;
                                    playing = false;
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            let chan = &mut self.channels[c];
            chan.ptr = ptr;
            chan.delta = delta;
            chan.env_vol = env_vol;
            chan.sustain = sustain;
            chan.playing = playing;
            if !playing {
                chan.id = !0;
            }
        }
        if retval == 0 && zero_channels != 0 {
            retval = 2;
        }
        retval
    }
}

/// PS_STYPE_TO_INT16.
pub(crate) fn to_int16(v: f32) -> i16 {
    let t = (v * 32768.0) as i32;
    t.clamp(-32768, 32767) as i16
}

/// gen_set_offset: the phase for a sample offset.
fn set_offset(chan: &mut Chan, offset: u32) {
    let shift = match chan.local_type {
        T_TRI | T_SAW => 11,
        T_SQ => 10,
        T_NOISE => 0,
        T_DRAWN => 8,
        T_SIN | T_HSIN => 10,
        T_ASIN | T_PSIN => 11,
        _ => 0,
    };
    chan.ptr = (offset << 16) >> shift;
}

impl Module for Generator {
    fn handle(&mut self, n: usize, e: &mut Event, net: &mut Net) -> isize {
        let mut retval: isize = 0;
        match e.cmd {
            Cmd::Init => {
                net.register_ctl(n, 0, 256, 128, 0, C_VOLUME, -1, 0);
                net.register_ctl(n, 0, TYPES - 1, 0, 1, C_TYPE, -1, 0);
                net.register_ctl(n, 0, 256, 128, 0, C_PAN, 128, 0);
                net.set_ctl_show_offset(n, 2, -128);
                net.register_ctl(n, 0, 512, 0, 0, C_ATTACK, -1, 1);
                net.register_ctl(n, 0, 512, 0, 0, C_RELEASE, -1, 1);
                net.register_ctl(n, 1, MAX_CHANNELS as i32, 8, 1, C_CHANNELS, -1, 2);
                net.register_ctl(n, 0, 1, MODE_MONO, 1, C_MODE, -1, 2);
                net.register_ctl(n, 0, 1, 1, 1, C_SUSTAIN, -1, 1);
                net.register_ctl(n, 0, 256, 0, 0, C_MOD, -1, 2);
                net.register_ctl(n, 0, 1022, 511, 0, C_DUTY, 511, 2);
                for c in self.channels.iter_mut() {
                    *c = Chan { id: !0, ..Chan::default() };
                }
                self.no_active_channels = true;
                self.drawn_wave = DRAWN_WAVEFORM;
                self.search_ptr = 0;
                self.wrong_saw_generation = false;
                retval = 1;
            }
            Cmd::SetupFinished => {
                if let Some(c) = net.mods[n].chunk(0) {
                    for (i, w) in self.drawn_wave.iter_mut().enumerate() {
                        *w = c.data.get(i).copied().unwrap_or(0) as i8;
                    }
                    self.drawn_wave_changed = true;
                }
                retval = 1;
            }
            Cmd::RenderReplace => retval = self.render(n, net),
            Cmd::NoteOn => {
                let ctl_channels = net.mods[n].ctl(C_CHANNELS).max(1) as usize;
                if !self.no_active_channels {
                    for c in 0..MAX_CHANNELS {
                        if self.channels[c].id == e.id {
                            if self.channels[c].sustain {
                                self.channels[c].sustain = false;
                            }
                            self.channels[c].id = !0;
                            break;
                        }
                    }
                    let mut c = 0;
                    while c < ctl_channels {
                        if !self.channels[self.search_ptr].playing {
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
                let freq = net::get_freq(e.pitch() / 4);
                let delta = net::get_delta64(net.sampling_freq, freq) as u32;
                let attack = net.mods[n].ctl(C_ATTACK);
                let ty = net.mods[n].ctl(C_TYPE);
                let chan = &mut self.channels[self.search_ptr];
                chan.playing = true;
                chan.vel = e.velocity() as i32;
                chan.delta = delta;
                chan.ptr = 4 << 16;
                chan.id = e.id;
                chan.env_vol = if attack == 0 { 1 << 30 } else { 0 };
                chan.sustain = true;
                chan.local_type = ty;
                chan.local_pan = 128;
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
                        let freq = net::get_freq(e.pitch() / 4);
                        self.channels[c].delta = net::get_delta64(net.sampling_freq, freq) as u32;
                        self.channels[c].vel = e.velocity() as i32;
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
                for c in self.channels.iter_mut() {
                    if c.id == e.id {
                        set_offset(c, e.sample_offset());
                    }
                }
                retval = 1;
            }
            Cmd::NoteOff => {
                if self.no_active_channels {
                    return 0;
                }
                for c in self.channels.iter_mut() {
                    if c.id == e.id {
                        c.sustain = false;
                        break;
                    }
                }
                retval = 1;
            }
            Cmd::AllNotesOff => {
                for c in self.channels.iter_mut() {
                    c.sustain = false;
                    c.id = !0;
                }
                retval = 1;
            }
            Cmd::Clean => {
                for c in self.channels.iter_mut() {
                    c.playing = false;
                    c.id = !0;
                }
                self.no_active_channels = true;
                retval = 1;
            }
            Cmd::SetLocalController => {
                if self.no_active_channels {
                    return 0;
                }
                let ctl_channels = net.mods[n].ctl(C_CHANNELS).max(0) as usize;
                for c in 0..ctl_channels.min(MAX_CHANNELS) {
                    let chan = &mut self.channels[c];
                    if chan.id == e.id {
                        match e.ctl_num() {
                            1 => {
                                chan.local_type = (e.ctl_val() as i32).min(TYPES - 1);
                                retval = 1;
                            }
                            2 => {
                                chan.local_pan = (e.ctl_val() >> 7) as i32;
                                retval = 1;
                            }
                            _ => {}
                        }
                        break;
                    }
                }
            }
            Cmd::SetGlobalController => {
                if e.ctl_num() == 0x98 {
                    self.wrong_saw_generation = true;
                }
            }
            Cmd::Close => retval = 1,
            _ => {}
        }
        retval
    }
}
