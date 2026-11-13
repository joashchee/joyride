// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! DrumSynth (psynths_drumsynth.cpp): bass drums (a sine oscillator
//! swept and faded through four points), hi-hats and snares (noise
//! through a state-variable filter, or plain, likewise enveloped), a sound
//! a note (C to D# bass, E to F# hi-hat, G to B snare) and a variation an
//! octave, at 44.1 kHz in integers.

use super::super::dsp::{renderbuf2output, RenderbufPars, SmootherCoefs};
use super::super::net::{self, Cmd, Event, Module, Net};

const SFREQ: i32 = 44100;
const MAX_CHANNELS: usize = 8;
const ENV_POINTS: usize = 4;
const HT_TYPE_BAND: i32 = 0;
const HT_TYPE_HIGH: i32 = 1;
const HT_TYPE_PURE: i32 = 8;

const C_VOLUME: usize = 0;
const C_PAN: usize = 1;
const C_CHANNELS: usize = 2;
const C_BD_VOL: usize = 3;
const C_BD_ENV: usize = 4;
const C_BD_TONE: usize = 5;
const C_BD_LENGTH: usize = 6;
const C_HT_VOL: usize = 7;
const C_HT_LENGTH: usize = 8;
const C_SD_VOL: usize = 9;
const C_SD_TONE: usize = 10;
const C_SD_LENGTH: usize = 11;
const C_BD_PAN: usize = 12;
const C_HT_PAN: usize = 13;
const C_SD_PAN: usize = 14;

/// An envelope of four points: its coefficient (pitch or cutoff), amplitude and time.
#[derive(Clone, Copy, Default)]
struct Env {
    coef: [i32; ENV_POINTS],
    amp: [i32; ENV_POINTS],
    time: [i32; ENV_POINTS],
    point: i32,
    counter: i32,
    pcoef: i32,
    pcoef_delta: i32,
    pamp: i32,
    pamp_delta: i32,
}

impl Env {
    /// Point `p`'s stretch to the next one.
    fn segment(&mut self, p: usize) {
        self.counter = self.time[p + 1] - self.time[p];
        self.pcoef_delta = (self.coef[p + 1] - self.coef[p]).wrapping_mul(1 << 15) / self.counter;
        self.pamp_delta = (self.amp[p + 1] - self.amp[p]).wrapping_mul(1 << 15) / self.counter;
    }

    fn start(&mut self) {
        self.point = 0;
        self.segment(0);
        self.pcoef = self.coef[0].wrapping_mul(1 << 15);
        self.pamp = self.amp[0].wrapping_mul(1 << 15);
    }

    /// The counter ran out: the next point, false past the last.
    fn next(&mut self) -> bool {
        self.point += 1;
        if self.point > ENV_POINTS as i32 - 2 {
            return false;
        }
        self.segment(self.point as usize);
        true
    }
}

#[derive(Clone, Copy, Default)]
struct Chan {
    playing: bool,
    bd: bool,
    ht: bool,
    ty: u8,
    id: u32,
    vel: i32,
    sin_state: [i32; 2],
    sin: Env,
    ht_q: i32,
    ht_type: i32,
    ht_random_type: i32,
    ht_state: [i32; 2],
    hte: Env,
    ht_random: u32,
    highpass: i32,
    highpass_prev: [f32; 4],
    rb: RenderbufPars,
    local_pan: i32,
}

/// drumsynth_reset_channel.
fn reset(c: &mut Chan) {
    c.playing = false;
    c.id = !0;
    c.sin_state = [32768, 0];
    c.sin.point = -1;
    c.bd = false;
    c.ht_type = 0;
    c.ht_random_type = 0;
    c.ht_q = 10000;
    c.ht_state = [0, 0];
    c.hte.point = -1;
    c.ht_random = c.ht_random.wrapping_mul(328753);
    c.ht = false;
    c.highpass = 0;
}

fn random1(ty: i32, s: &mut u32) -> u32 {
    if ty == 0 {
        net::psynth_rand(s)
    } else {
        *s = s.wrapping_mul(1103515245).wrapping_add(12);
        *s & 32767
    }
}

fn s16(v: i32) -> f32 {
    v as f32 / 32768.0
}

#[derive(Default)]
pub(crate) struct DrumSynth {
    channels: [Chan; MAX_CHANNELS],
    no_active_channels: bool,
    search_ptr: usize,
    coefs: SmootherCoefs,
    buf: Vec<f32>,
}

impl DrumSynth {
    fn render(&mut self, n: usize, net: &mut Net) -> isize {
        let m = &mut net.mods[n];
        let (offset, frames) = (m.offset as usize, m.frames as usize);
        let c = |i| m.ctl(i);
        let (ctl_volume, ctl_pan, ctl_channels, bd_pan, ht_pan, sd_pan) = (c(C_VOLUME), c(C_PAN), c(C_CHANNELS), c(C_BD_PAN), c(C_HT_PAN), c(C_SD_PAN));
        let mut retval = 0i32;
        if self.buf.len() < frames {
            self.buf.resize(frames, 0.0);
        }
        self.no_active_channels = true;
        for ci in 0..ctl_channels.max(0) as usize {
            let chan = &mut self.channels[ci];
            if !chan.playing {
                continue;
            }
            self.no_active_channels = false;
            let rb = &mut self.buf[..frames];
            if chan.bd {
                if chan.sin.point < 0 {
                    chan.sin.start();
                }
                let mut i = 0;
                loop {
                    let full = chan.sin.pamp == (32768 << 15) && chan.sin.pamp_delta == 0;
                    while i < frames && chan.sin.counter > 0 {
                        let coef = chan.sin.pcoef >> 15;
                        chan.sin_state[0] = chan.sin_state[0].wrapping_sub(coef.wrapping_mul(chan.sin_state[1]) >> 15);
                        chan.sin_state[1] = chan.sin_state[1].wrapping_add(coef.wrapping_mul(chan.sin_state[0]) >> 15);
                        let mut iv = chan.sin_state[1];
                        if !full {
                            iv = iv.wrapping_mul(chan.sin.pamp >> 15);
                            iv >>= 15;
                        }
                        rb[i] = s16(iv);
                        chan.sin.pcoef = chan.sin.pcoef.wrapping_add(chan.sin.pcoef_delta);
                        if !full {
                            chan.sin.pamp = chan.sin.pamp.wrapping_add(chan.sin.pamp_delta);
                        }
                        chan.sin.counter -= 1;
                        i += 1;
                    }
                    if chan.sin.counter > 0 {
                        break;
                    }
                    if !chan.sin.next() {
                        chan.playing = false;
                        rb[i..].fill(0.0);
                        break;
                    }
                }
            }
            if chan.ht {
                let q = chan.ht_q;
                let ty = chan.ht_type;
                let rt = chan.ht_random_type;
                let mut d1 = chan.ht_state[0];
                let mut d2 = chan.ht_state[1];
                if chan.hte.point < 0 {
                    chan.hte.start();
                }
                let mut i = 0;
                loop {
                    let full = chan.hte.pamp == (32768 << 15) && chan.hte.pamp_delta == 0;
                    while i < frames && chan.hte.counter > 0 {
                        let mut iv = (random1(rt, &mut chan.ht_random).wrapping_mul(2)).wrapping_sub(32768) as i32;
                        if ty != HT_TYPE_PURE {
                            let coef = chan.hte.pcoef >> 15;
                            iv /= 2;
                            let low = d2.wrapping_add(coef.wrapping_mul(d1) >> 15);
                            let high = iv.wrapping_sub(low).wrapping_sub(q.wrapping_mul(d1) >> 15);
                            let band = (coef.wrapping_mul(high) >> 15).wrapping_add(d1);
                            d1 = band;
                            d2 = low;
                            iv = if ty == 0 { band } else { high };
                            iv = iv.wrapping_mul(2);
                        }
                        if !full {
                            iv = iv.wrapping_mul(chan.hte.pamp >> 16);
                            iv >>= 14;
                        }
                        rb[i] = s16(iv);
                        if ty != HT_TYPE_PURE {
                            chan.hte.pcoef = chan.hte.pcoef.wrapping_add(chan.hte.pcoef_delta);
                        }
                        if !full {
                            chan.hte.pamp = chan.hte.pamp.wrapping_add(chan.hte.pamp_delta);
                        }
                        chan.hte.counter -= 1;
                        i += 1;
                    }
                    if chan.hte.counter > 0 {
                        break;
                    }
                    if !chan.hte.next() {
                        chan.playing = false;
                        rb[i..].fill(0.0);
                        break;
                    }
                }
                chan.ht_state = [d1, d2];
            }
            for h in 0..chan.highpass.max(0) as usize {
                for v in rb.iter_mut() {
                    let x = *v;
                    *v = (x - chan.highpass_prev[h]) / 2.0;
                    chan.highpass_prev[h] = x;
                }
            }
            if !chan.playing {
                chan.id = !0;
            }
            let mut pan = ctl_pan + (chan.local_pan - 128);
            match chan.ty {
                0 => pan += bd_pan - 128,
                1 => pan += ht_pan - 128,
                2 => pan += sd_pan - 128,
                _ => {}
            }
            retval = renderbuf2output(retval, &mut m.chout, 2, offset, frames, rb, None, frames, ctl_volume * chan.vel / 2, pan * 256, &mut chan.rb, &self.coefs, SFREQ);
        }
        retval as isize
    }

    fn note_on(&mut self, n: usize, e: &Event, net: &Net) {
        let m = &net.mods[n];
        let ctl_channels = m.ctl(C_CHANNELS).max(1) as usize;
        if !self.no_active_channels {
            for c in self.channels.iter_mut() {
                if c.id == e.id {
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
        let chan = &mut self.channels[self.search_ptr];
        reset(chan);
        chan.playing = true;
        chan.vel = e.velocity() as i32;
        chan.id = e.id;
        chan.local_pan = 128;
        chan.rb.start = true;
        let nt = if net.base_host_version == 0x01070000 { e.root_note() as i32 } else { ((net::NOTE0_PITCH - e.pitch()) / 256).clamp(0, 10 * 12 - 1) };
        let oct = nt / 12;
        let nn = nt - oct * 12;
        let c = |i| m.ctl(i);
        let bd_env = c(C_BD_ENV);
        let bd_tone = c(C_BD_TONE) * c(C_BD_TONE) / 256;
        let bd_len = c(C_BD_LENGTH);
        let ht_len = c(C_HT_LENGTH);
        let sd_len = c(C_SD_LENGTH);
        let sd_tone = c(C_SD_TONE) * c(C_SD_TONE) / 256;
        match nn {
            0..=3 => {
                chan.ty = 0;
                chan.vel = (chan.vel * c(C_BD_VOL)) / 256;
                chan.bd = true;
                let s = &mut chan.sin;
                let mut set = |p: usize, coef: i32, amp: i32, time: i32| {
                    s.coef[p] = coef;
                    s.amp[p] = amp;
                    s.time[p] = time;
                };
                let t2 = |t1: i32| t1 + 1 + (6000 * bd_len) / 128;
                match oct {
                    0..=3 => {
                        let c0 = [10000, 0, 5000, 32768][oct as usize];
                        set(0, c0, 32768, 0);
                        set(1, 400 + (4000 * bd_tone) / 256, (32768 * bd_env) / 256, 8);
                        let t = t2(8);
                        set(2, 0, 0, t);
                        set(3, 0, 0, t + 1);
                    }
                    4 => {
                        set(0, 32768, 32768, 0);
                        set(1, 200 + (4000 * bd_tone) / 256, (32768 * (128 + bd_env / 2)) / 256, 8);
                        let t = t2(8);
                        set(2, 100 + (2000 * bd_tone) / 256, (32768 * bd_env) / 256, t);
                        set(3, 0, 0, t + 1 + (6000 * bd_len) / 128);
                    }
                    5 => {
                        set(0, 32768, 32768, 0);
                        set(1, 200 + (4000 * bd_tone) / 256, (32768 * (128 + bd_env / 2)) / 256, 8);
                        let t = t2(8);
                        set(2, 90 + (2000 * bd_tone) / 256, (32768 * bd_env) / 256, t);
                        set(3, 400 + (15000 * bd_tone) / 256, 0, t + 1 + (3000 * bd_len) / 128);
                    }
                    6 => {
                        set(0, 32768, 0, 0);
                        set(1, 200 + (4000 * bd_tone) / 256, (32768 * (128 + bd_env / 2)) / 256, 8);
                        let t = t2(8);
                        set(2, 80 + (2000 * bd_tone) / 256, (32768 * bd_env) / 256, t);
                        set(3, 0, 0, t + 1 + (6000 * bd_len) / 128);
                    }
                    7 => {
                        set(0, 32768, 32768, 0);
                        set(1, 400 + (8000 * bd_tone) / 256, (32768 * (128 + bd_env / 2)) / 256, 8);
                        let t = t2(8);
                        set(2, 200 + (2000 * bd_tone) / 256, (32768 * bd_env) / 256, t);
                        set(3, 0, 0, t + 1 + (6000 * bd_len) / 128);
                    }
                    8 => {
                        set(0, 32768, 32768, 0);
                        set(1, 400 + (8000 * bd_tone) / 256, (20768 * (128 + bd_env / 2)) / 256, 8);
                        let t = t2(8);
                        set(2, 200 + (2000 * bd_tone) / 256, (10768 * bd_env) / 256, t);
                        set(3, 0, 0, t + 1 + (6000 * bd_len) / 128);
                    }
                    9 => {
                        let c0 = 20 + (32768 * bd_tone) / 256;
                        set(0, c0, 32768, 0);
                        let div = 2 + nn;
                        let t1 = 8 + (10000 * bd_len) / 128;
                        set(1, c0 / div, (((20 + 32768) / div) * bd_env) / 256, t1);
                        let t = t1 + 1 + (10000 * bd_len) / 128;
                        set(2, 0, 0, t);
                        set(3, 0, 0, t + 1);
                    }
                    _ => {}
                }
                let add = match nn {
                    1 => 50,
                    2 => 200,
                    3 => 400,
                    _ => 0,
                };
                for t in s.time[1..].iter_mut() {
                    *t += add;
                }
            }
            4..=6 => {
                chan.ty = 1;
                chan.vel = (chan.vel * c(C_HT_VOL)) / 256;
                chan.ht = true;
                chan.ht_q = 20000;
                let s = &mut chan.hte;
                s.coef = [22000, 22000, 22000, 0];
                let l = |k: i32| (k * ht_len) / 128;
                let (amps, times): ([i32; 4], [i32; 4]) = match oct {
                    0 => {
                        let t1 = 1 + l(5000);
                        ([0, 32878, 0, 0], [0, t1, t1 + 1, t1 + 2])
                    }
                    1 => {
                        let t1 = 1 + l(5000);
                        ([32768, 32878, 1000, 0], [0, t1, t1 + 1, t1 + 2 + 5000])
                    }
                    2 => {
                        let t1 = 1 + l(1000);
                        ([32768, 0, 32878, 0], [0, t1, t1 + 1, t1 + 2 + l(1000)])
                    }
                    3 => {
                        let t1 = 1 + l(3000);
                        let t2 = t1 + 1 + l(50000);
                        ([32768, 32768 / 4, 0, 0], [0, t1, t2, t2 + 1])
                    }
                    4 => {
                        let t1 = 1 + l(5000);
                        let t2 = t1 + 1 + l(30000);
                        ([32768, 32768 / 2, 0, 0], [0, t1, t2, t2 + 1])
                    }
                    5 => {
                        let t1 = 1 + l(2000);
                        let t2 = t1 + 1 + l(2000);
                        ([0, 32768, 0, 0], [0, t1, t2, t2 + 1])
                    }
                    6 => {
                        let t1 = 1 + l(30000);
                        let t2 = t1 + 1 + l(60000);
                        ([0, 32878 / 3, 32768, 0], [0, t1, t2, t2 + 1 + l(2000)])
                    }
                    _ => {
                        let t1 = 1 + l(2000);
                        let t2 = t1 + 1 + l(2000);
                        let a2 = match oct {
                            8 => 32768 / 3,
                            9 => 32768 / 6,
                            _ => 32768,
                        };
                        ([32768, 0, a2, 0], [0, t1, t2, t2 + 1 + l(2000)])
                    }
                };
                s.amp = amps;
                s.time = times;
                match nn {
                    4 => {
                        chan.ht_type = HT_TYPE_PURE;
                        chan.highpass = 1;
                    }
                    5 => {
                        chan.ht_type = HT_TYPE_PURE;
                        chan.highpass = 2;
                    }
                    _ => {
                        chan.ht_type = HT_TYPE_PURE;
                        chan.highpass = 2;
                        let t = &mut chan.hte.time;
                        t[1] = t[1] / 4 + 1;
                        t[2] = t[2] / 4 + 4;
                        t[3] = t[3] / 4 + 8;
                    }
                }
            }
            7..=11 => {
                chan.ty = 2;
                chan.vel = (chan.vel * c(C_SD_VOL)) / 256;
                chan.ht = true;
                chan.ht_type = if (nn - 7) & 1 != 0 { HT_TYPE_HIGH } else { HT_TYPE_BAND };
                if nn == 11 {
                    chan.ht_random_type = 1;
                    chan.ht_random = 1;
                    chan.ht_type = HT_TYPE_BAND;
                }
                if nn == 10 {
                    chan.ht_random_type = 1;
                    chan.ht_random = 0;
                    chan.ht_type = HT_TYPE_BAND;
                }
                chan.ht_q = if nn == 9 { 40000 } else { 20000 };
                let l = |k: i32| (k * sd_len) / 128;
                let c0 = 100 + (22000 * sd_tone) / 256;
                let s = &mut chan.hte;
                let (coefs, amps, times): ([i32; 4], [i32; 4], [i32; 4]) = match oct {
                    0 => {
                        let t1 = 1 + l(8000);
                        ([c0, c0, c0, c0], [32768, 32768, 5000, 0], [0, t1, t1 + 1, t1 + 2 + l(30000)])
                    }
                    1 => {
                        let t1 = 1 + l(10000);
                        ([c0, 0, 0, 0], [32768, 0, 0, 0], [0, t1, t1 + 1, t1 + 2])
                    }
                    2 | 3 => {
                        let t1 = 1 + l(5000);
                        let t2 = if oct == 3 { t1 + 1 + l(5000) } else { t1 + 1 + l(12000) };
                        let t3 = if oct == 3 { t2 + 1 + l(5000) } else { t2 + 1 + l(15000) };
                        ([c0, 100, 100 + (11000 * sd_tone) / 256, 0], [32768, 0, 20000, 0], [0, t1, t2, t3])
                    }
                    4 | 5 => {
                        let c1 = if oct == 4 { c0 / 2 } else { c0 };
                        let c2 = if oct == 4 { c1 / 2 } else { c0 };
                        let c3 = if oct == 4 { 1 } else { c0 };
                        let t1 = 1 + l(10000);
                        let t2 = t1 + 1 + l(10000);
                        ([c0, c1, c2, c3], [32768, 32768 / 2, 32768 / 4, 0], [0, t1, t2, t2 + 1 + l(15000)])
                    }
                    6 => {
                        let t1 = 1 + l(5000);
                        let t2 = t1 + 1 + l(10000);
                        ([c0, c0, c0, c0], [32768, 5000, 2000, 0], [0, t1, t2, t2 + 1 + l(20000)])
                    }
                    7 => {
                        let t1 = 1 + l(5000);
                        let t2 = t1 + 1 + l(5000);
                        ([1, c0, 0, 0], [0, 32768, 0, 0], [0, t1, t2, t2 + 1])
                    }
                    _ => {
                        let a = if oct == 8 { 0 } else { 32768 };
                        let t1 = 1 + l(2000);
                        ([c0, 0, c0, 0], [32768, a, 32768, a], [0, t1, t1 + 1, t1 + 2 + l(2000)])
                    }
                };
                s.coef = coefs;
                s.amp = amps;
                s.time = times;
            }
            _ => {}
        }
        self.no_active_channels = false;
    }
}

impl Module for DrumSynth {
    fn handle(&mut self, n: usize, e: &mut Event, net: &mut Net) -> isize {
        let mut retval = 0;
        match e.cmd {
            Cmd::Init => {
                net.register_ctl(n, 0, 512, 256, 0, C_VOLUME, 256, 0);
                let k = net.register_ctl(n, 0, 256, 128, 0, C_PAN, 128, 0);
                net.set_ctl_show_offset(n, k as usize, -128);
                net.register_ctl(n, 1, MAX_CHANNELS as i32, 4, 1, C_CHANNELS, -1, 0);
                net.register_ctl(n, 0, 512, 200, 0, C_BD_VOL, 256, 1);
                net.register_ctl(n, 0, 256, 256, 0, C_BD_ENV, 256, 1);
                net.register_ctl(n, 0, 256, 64, 0, C_BD_TONE, 256, 1);
                net.register_ctl(n, 0, 256, 64, 0, C_BD_LENGTH, 256, 1);
                net.register_ctl(n, 0, 512, 256, 0, C_HT_VOL, 256, 2);
                net.register_ctl(n, 0, 256, 64, 0, C_HT_LENGTH, 256, 2);
                net.register_ctl(n, 0, 512, 256, 0, C_SD_VOL, 256, 3);
                net.register_ctl(n, 0, 256, 128, 0, C_SD_TONE, 256, 3);
                net.register_ctl(n, 0, 256, 64, 0, C_SD_LENGTH, 256, 3);
                for (var, group) in [(C_BD_PAN, 1), (C_HT_PAN, 2), (C_SD_PAN, 3)] {
                    let k = net.register_ctl(n, 0, 256, 128, 0, var, 128, group);
                    net.set_ctl_show_offset(n, k as usize, -128);
                }
                for c in self.channels.iter_mut() {
                    reset(c);
                    c.highpass_prev = [0.0; 4];
                    c.ht_random = 0;
                }
                self.no_active_channels = true;
                self.search_ptr = 0;
                self.coefs = SmootherCoefs::new(100.0, SFREQ as f32);
                retval = 1;
            }
            Cmd::Clean => {
                for c in self.channels.iter_mut() {
                    reset(c);
                }
                self.no_active_channels = true;
                retval = 1;
            }
            Cmd::RenderReplace => retval = self.render(n, net),
            Cmd::NoteOn => {
                self.note_on(n, e, net);
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
            Cmd::SetLocalController => {
                if self.no_active_channels {
                    return 0;
                }
                let ctl_channels = net.mods[n].ctl(C_CHANNELS).max(0) as usize;
                for c in 0..ctl_channels.min(MAX_CHANNELS) {
                    let chan = &mut self.channels[c];
                    if chan.id == e.id {
                        if e.ctl_num() == 1 {
                            chan.local_pan = (e.ctl_val() >> 7) as i32;
                            retval = 1;
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
