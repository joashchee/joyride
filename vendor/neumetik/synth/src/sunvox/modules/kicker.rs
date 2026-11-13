// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! Kicker (psynths_kicker.cpp): a kick drum, a triangle, square or sine
//! whose pitch falls with its envelope, the release accelerating.

use super::super::dsp::{renderbuf2output, RenderbufPars, SmootherCoefs};
use super::super::net::{self, Cmd, Event, Module, Net};

const MAX_CHANNELS: usize = 4;
const WAVETYPE_MAX: i32 = 2;
const MAX_ENV_VOL: u32 = 1024 * 64;

const C_VOLUME: usize = 0;
const C_TYPE: usize = 1;
const C_PAN: usize = 2;
const C_ATTACK: usize = 3;
const C_RELEASE: usize = 4;
const C_VOL_ADD: usize = 5;
const C_ENV_ACCEL: usize = 6;
const C_CHANNELS: usize = 7;
const C_ANTICLICK: usize = 8;

#[derive(Clone, Copy, Default)]
struct Chan {
    playing: bool,
    id: u32,
    vel: i32,
    ptr: u32,
    delta: u32,
    env_vol: u32,
    sustain: bool,
    accel: bool,
    rb: RenderbufPars,
    local_type: i32,
    local_pan: i32,
}

#[derive(Default)]
pub(crate) struct Kicker {
    channels: [Chan; MAX_CHANNELS],
    no_active_channels: bool,
    search_ptr: usize,
    coefs: SmootherCoefs,
    buf: Vec<f32>,
}

impl Kicker {
    fn render(&mut self, n: usize, net: &mut Net) -> isize {
        let m = &net.mods[n];
        let (offset, frames) = (m.offset as usize, m.frames as usize);
        let (ctl_volume, ctl_pan, ctl_attack, ctl_release, ctl_vol_add, ctl_env_accel, ctl_channels) =
            (m.ctl(C_VOLUME), m.ctl(C_PAN), m.ctl(C_ATTACK), m.ctl(C_RELEASE), m.ctl(C_VOL_ADD), m.ctl(C_ENV_ACCEL), m.ctl(C_CHANNELS));
        let srate = net.sampling_freq;
        let base = net.base_host_version;
        let mut retval: i32 = 0;
        let (mut attack_delta, mut release_delta) = (0i32, 0i32);
        let mut env_accel = 0u32;
        let mut vol_add = 0i32;
        let mut env_ready = false;
        self.no_active_channels = true;
        if self.buf.len() < frames {
            self.buf.resize(frames, 0.0);
        }
        for c in 0..ctl_channels.max(0) as usize {
            if !self.channels[c].playing {
                continue;
            }
            self.no_active_channels = false;
            if !env_ready {
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
                env_accel = (1024 - ctl_env_accel) as u32;
                vol_add = ctl_vol_add;
                if base <= 0x01070000 || base >= 0x01090400 {
                    vol_add = vol_add * MAX_ENV_VOL as i32 / 1024;
                }
                env_ready = true;
            }
            let chan = &mut self.channels[c];
            let mut ptr = chan.ptr;
            let mut env_vol = chan.env_vol;
            let mut sustain = chan.sustain;
            let mut accel = chan.accel;
            let mut playing = chan.playing;
            let delta = chan.delta;
            let vol = ctl_volume * chan.vel / 2;
            let rb = &mut self.buf;
            let mut i = 0usize;
            let ty = chan.local_type;
            let (mut max_val, mut min_val) = (32767.0f32 / 32768.0, -32767.0f32 / 32768.0);
            if ty == 1 && vol != 32768 {
                max_val = max_val * vol as f32 / 32768.0;
                min_val = min_val * vol as f32 / 32768.0;
            }
            if (0..=2).contains(&ty) {
                while i < frames {
                    let val: f32 = match ty {
                        0 => {
                            let t = (ptr & 0xFFFF) / 8 + ((ptr >> 16) & 7) * 8192;
                            if ((ptr >> 16) & 15) < 8 {
                                (t as i32 - 32767) as f32 / 32768.0
                            } else {
                                (32767i32.wrapping_sub(t as i32)) as f32 / 32768.0
                            }
                        }
                        1 => {
                            if (ptr >> 16) & 16 != 0 {
                                max_val
                            } else {
                                min_val
                            }
                        }
                        _ => {
                            let p2 = ptr.wrapping_add(128 << 12) & ((512 << 12) - 1);
                            let x = (p2 as f32 / (512 << 12) as f32) as f64;
                            ((std::f64::consts::PI * 2.0 * x) as f32).sin()
                        }
                    };
                    let mut new_env = (env_vol >> 14).wrapping_add(vol_add as u32);
                    if new_env > MAX_ENV_VOL {
                        new_env = MAX_ENV_VOL;
                    }
                    if new_env > (env_vol >> (14 - 3)) {
                        new_env = env_vol >> (14 - 3);
                    }
                    let val = val * new_env as f32 / MAX_ENV_VOL as f32;
                    rb[i] = val;
                    ptr = ptr.wrapping_add(delta.wrapping_mul(env_vol >> 20) / 1024);
                    if sustain {
                        env_vol = env_vol.wrapping_add(attack_delta as u32);
                        if env_vol >= 1 << 30 {
                            env_vol = 1 << 30;
                            sustain = false;
                        }
                    } else {
                        env_vol = env_vol.wrapping_sub(release_delta as u32);
                        if env_vol > env_accel << 20 {
                            env_vol = env_vol.wrapping_sub(release_delta as u32);
                        } else if accel {
                            accel = false;
                            env_vol = env_accel << 20;
                        }
                        if env_vol > 1 << 30 {
                            env_vol = 0;
                            playing = false;
                            i += 1;
                            break;
                        }
                    }
                    i += 1;
                }
            }
            let rendered = i;
            chan.ptr = ptr;
            chan.env_vol = env_vol;
            chan.sustain = sustain;
            chan.accel = accel;
            chan.playing = playing;
            if !playing {
                chan.id = !0;
            }
            let pan = (ctl_pan + (chan.local_pan - 128)) * 256;
            let m = &mut net.mods[n];
            retval = renderbuf2output(retval, &mut m.chout, 2, offset, frames, &rb[..frames], None, rendered, vol, pan, &mut chan.rb, &self.coefs, srate);
        }
        retval as isize
    }
}

impl Module for Kicker {
    fn handle(&mut self, n: usize, e: &mut Event, net: &mut Net) -> isize {
        let mut retval: isize = 0;
        match e.cmd {
            Cmd::Init => {
                net.register_ctl(n, 0, 256, 256, 0, C_VOLUME, -1, 0);
                net.register_ctl(n, 0, WAVETYPE_MAX, 2, 1, C_TYPE, -1, 0);
                net.register_ctl(n, 0, 256, 128, 0, C_PAN, 128, 0);
                net.set_ctl_show_offset(n, 2, -128);
                net.register_ctl(n, 0, 512, 0, 0, C_ATTACK, -1, 1);
                net.register_ctl(n, 0, 512, 32, 0, C_RELEASE, -1, 1);
                net.register_ctl(n, 0, 1024, 0, 0, C_VOL_ADD, -1, 1);
                net.register_ctl(n, 0, 1024, 256, 0, C_ENV_ACCEL, 256, 1);
                net.register_ctl(n, 1, MAX_CHANNELS as i32, 1, 1, C_CHANNELS, -1, 2);
                net.register_ctl(n, 0, 1, 0, 1, C_ANTICLICK, -1, 2);
                for c in self.channels.iter_mut() {
                    *c = Chan { id: !0, ..Chan::default() };
                }
                self.no_active_channels = true;
                self.search_ptr = 0;
                self.coefs = SmootherCoefs::new(100.0, net.sampling_freq as f32);
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
            Cmd::RenderReplace => retval = self.render(n, net),
            Cmd::NoteOn => {
                let ctl_channels = net.mods[n].ctl(C_CHANNELS).max(1) as usize;
                if !self.no_active_channels {
                    for c in self.channels.iter_mut() {
                        if c.id == e.id {
                            if c.sustain {
                                c.sustain = false;
                            }
                            c.id = !0;
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
                    }
                } else {
                    self.search_ptr = 0;
                }
                let freq = net::get_freq(e.pitch() / 4);
                let (dh, dl) = net::get_delta(net.sampling_freq, freq);
                let m = &net.mods[n];
                let (anticlick, ty) = (m.ctl(C_ANTICLICK), m.ctl(C_TYPE));
                let chan = &mut self.channels[self.search_ptr];
                chan.playing = true;
                chan.vel = e.velocity() as i32;
                chan.delta = dl | (dh << 16);
                chan.ptr = if anticlick != 0 {
                    if ty == 2 {
                        128 << 12
                    } else {
                        4 << 16
                    }
                } else {
                    0
                };
                chan.id = e.id;
                chan.env_vol = 0;
                chan.sustain = true;
                chan.accel = true;
                chan.rb.start = true;
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
                        let (dh, dl) = net::get_delta(net.sampling_freq, freq);
                        self.channels[c].delta = dl | (dh << 16);
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
                                chan.local_type = (e.ctl_val() as i32).min(WAVETYPE_MAX);
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
            Cmd::Close => retval = 1,
            _ => {}
        }
        retval
    }
}
