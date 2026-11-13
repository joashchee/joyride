// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! Vorbis player (psynths_vorbis_player.cpp): an Ogg Vorbis file kept in
//! the module, decoded by Tremor (`crate::tremor`) for each of up to
//! four voices, played at the note's pitch (or its own rate), with or
//! without interpolation, once or looped.

use std::sync::Arc;

use super::super::net::{self, Cmd, Event, Module, Net};
use crate::tremor;

const MAX_CHANNELS: usize = 4;
const PCMBUF_SAMPLES: usize = 256;
const PCMBUF_BYTES: i64 = (PCMBUF_SAMPLES * 2) as i64;

const C_VOLUME: usize = 0;
const C_STATIC_FREQ: usize = 1;
const C_FINE: usize = 2;
const C_REL: usize = 3;
const C_INTERP: usize = 4;
const C_CHANNELS: usize = 5;
const C_LOOP: usize = 6;
const C_IGNORE_NOTEOFF: usize = 7;

struct Chan {
    playing: bool,
    id: u32,
    vel: i32,
    ptr_h: i32,
    ptr_l: i32,
    delta_h: u32,
    delta_l: u32,
    vf: Option<tremor::File>,
    loaded: i32,
    pcmbuf: [i16; PCMBUF_SAMPLES],
}

impl Default for Chan {
    fn default() -> Chan {
        Chan { playing: false, id: !0, vel: 0, ptr_h: 0, ptr_l: 0, delta_h: 0, delta_l: 0, vf: None, loaded: 0, pcmbuf: [0; PCMBUF_SAMPLES] }
    }
}

#[derive(Default)]
pub(crate) struct VorbisPlayer {
    channels: Vec<Chan>,
    no_active_channels: bool,
    search_ptr: usize,
    pcmbuf: Vec<i16>,
    src: Option<Arc<Vec<u8>>>,
    src_pcm_total: u64,
    pause: i32,
}

/// PSYNTH_FP64_ADD.
fn fp_add(h: &mut i32, l: &mut i32, dh: u32, dl: u32) {
    *h = h.wrapping_add(dh as i32);
    *l = l.wrapping_add(dl as i32);
    if *l > (1 << 16) - 1 {
        *h = h.wrapping_add(1);
        *l -= 1 << 16;
    }
}

impl VorbisPlayer {
    fn render(&mut self, n: usize, net: &mut Net) -> isize {
        if self.pause != 0 {
            return 0;
        }
        let srate = net.sampling_freq;
        let m = &mut net.mods[n];
        let (ctl_volume, ctl_static, ctl_interp, ctl_channels, ctl_loop) = (m.ctl(C_VOLUME), m.ctl(C_STATIC_FREQ), m.ctl(C_INTERP), m.ctl(C_CHANNELS), m.ctl(C_LOOP));
        let outputs_num = m.output_channels.clamp(0, 2) as usize;
        let mut retval = 0;
        self.no_active_channels = true;
        let main = &mut self.pcmbuf;
        for c in 0..(ctl_channels.max(0) as usize).min(MAX_CHANNELS) {
            let chan = &mut self.channels[c];
            if !chan.playing {
                continue;
            }
            self.no_active_channels = false;
            let mut offset = m.offset as usize;
            let mut frames = m.frames as i32;
            let Some(vf) = chan.vf.as_mut() else { continue };
            let (rate, vch) = (vf.vi.rate, vf.vi.channels);
            let vol = ((ctl_volume as f32 as f64 / 256.0) * (chan.vel as f32 as f64 / 256.0)) as f32;
            let (delta_h, delta_l) = if ctl_static != 0 {
                let delta = rate as f32 / srate as f32;
                let delta_i = (delta * 65536.0) as u32;
                (delta_i >> 16, delta_i & 0xffff)
            } else {
                (chan.delta_h, chan.delta_l)
            };
            while frames > 0 && chan.playing {
                let mut read_rv = 1i64;
                while chan.ptr_h + 1 >= chan.loaded {
                    let pcm_offset = ((chan.loaded as i64 * vch as i64 * 2) & (PCMBUF_BYTES - 1)) as usize;
                    read_rv = vf.read(main, PCMBUF_BYTES - 8);
                    if read_rv <= 0 {
                        if read_rv == 0 && ctl_loop != 0 {
                            vf.time_seek(0);
                        } else {
                            chan.playing = false;
                            chan.id = !0;
                            break;
                        }
                    } else {
                        for (k, i) in (pcm_offset / 2..(pcm_offset + read_rv as usize) / 2).enumerate() {
                            chan.pcmbuf[i & (PCMBUF_SAMPLES - 1)] = main[k];
                        }
                        chan.loaded += (read_rv / (vch as i64 * 2)) as i32;
                    }
                }
                let (mut ptr_h, mut ptr_l) = (0, 0);
                let mut frames_filled = 0i32;
                let loaded = chan.loaded;
                let buf = &chan.pcmbuf;
                let at = |i: i32| buf[i as usize & (PCMBUF_SAMPLES - 1)] as i32;
                for ch in 0..outputs_num {
                    let out = &mut m.chout[ch][offset..];
                    ptr_h = chan.ptr_h;
                    ptr_l = chan.ptr_l;
                    let put = |o: &mut f32, s: f32| {
                        if retval == 0 {
                            *o = s;
                        } else {
                            *o += s;
                        }
                    };
                    if read_rv > 0 {
                        let idx = |p: i32| -> i32 {
                            if vch == 2 {
                                at(((p * 2) & (PCMBUF_SAMPLES as i32 - 1)) + ch as i32)
                            } else {
                                at(p)
                            }
                        };
                        if vch != 1 && vch != 2 {
                            continue;
                        }
                        if ctl_interp != 0 && !(delta_h == 1 && delta_l == 0) {
                            frames_filled = 0;
                            while frames_filled < frames {
                                if ptr_h + 1 >= loaded {
                                    break;
                                }
                                let v1 = (idx(ptr_h) * ((1 << 16) - 1 - ptr_l)) >> 16;
                                let v2 = (idx(ptr_h + 1) * ptr_l) >> 16;
                                let s = (v1 + v2) as f32 / 32768.0;
                                put(&mut out[frames_filled as usize], s * vol);
                                fp_add(&mut ptr_h, &mut ptr_l, delta_h, delta_l);
                                frames_filled += 1;
                            }
                        } else {
                            frames_filled = 0;
                            while frames_filled < frames {
                                if ptr_h >= loaded {
                                    break;
                                }
                                let s = idx(ptr_h) as f32 / 32768.0;
                                put(&mut out[frames_filled as usize], if vol == 1.0 { s } else { s * vol });
                                fp_add(&mut ptr_h, &mut ptr_l, delta_h, delta_l);
                                frames_filled += 1;
                            }
                        }
                    } else if retval == 0 {
                        for o in out[..frames as usize].iter_mut() {
                            *o = 0.0;
                        }
                        frames_filled = frames;
                    }
                }
                if outputs_num > 0 {
                    chan.ptr_h = ptr_h;
                    chan.ptr_l = ptr_l;
                }
                offset += frames_filled.max(0) as usize;
                frames -= frames_filled;
            }
            retval = 1;
        }
        retval as isize
    }
}

impl Module for VorbisPlayer {
    fn handle(&mut self, n: usize, e: &mut Event, net: &mut Net) -> isize {
        let mut retval: isize = 0;
        match e.cmd {
            Cmd::Init => {
                net.register_ctl(n, 0, 512, 256, 0, C_VOLUME, 256, 0);
                net.register_ctl(n, 0, 1, 1, 1, C_STATIC_FREQ, -1, 0);
                net.register_ctl(n, -128, 128, 0, 0, C_FINE, 0, 0);
                net.register_ctl(n, 0, 256, 128, 1, C_REL, 128, 0);
                net.set_ctl_show_offset(n, 3, -128);
                net.register_ctl(n, 0, 1, 1, 1, C_INTERP, -1, 1);
                net.register_ctl(n, 1, MAX_CHANNELS as i32, 1, 1, C_CHANNELS, -1, 1);
                net.register_ctl(n, 0, 1, 0, 1, C_LOOP, -1, 1);
                net.register_ctl(n, 0, 1, 0, 1, C_IGNORE_NOTEOFF, -1, 1);
                self.channels = (0..=MAX_CHANNELS).map(|_| Chan::default()).collect();
                self.no_active_channels = true;
                self.search_ptr = 0;
                self.pcmbuf = vec![0; PCMBUF_SAMPLES];
                self.src = None;
                self.src_pcm_total = 0;
                self.pause = 0;
                retval = 1;
            }
            Cmd::SetupFinished => {
                self.src = net.mods[n].chunk(0).map(|c| Arc::new(c.data.clone()));
                // vplayer_get_total_pcm_time: a file opened to measure it.
                self.src_pcm_total = self.src.as_ref().and_then(|s| tremor::File::open(s.clone()).ok()).map_or(0, |f| f.pcm_total(-1) as u64);
                retval = 1;
            }
            Cmd::RenderReplace => retval = self.render(n, net),
            Cmd::NoteOn => {
                let Some(src) = self.src.clone() else { return 0 };
                let m = &net.mods[n];
                let (ctl_channels, ignore) = ((m.ctl(C_CHANNELS).max(1) as usize).min(MAX_CHANNELS), m.ctl(C_IGNORE_NOTEOFF) != 0);
                if !self.no_active_channels {
                    for c in 0..MAX_CHANNELS {
                        if self.channels[c].id == e.id && !ignore {
                            self.channels[c].playing = false;
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
                    }
                } else {
                    self.search_ptr = 0;
                }
                let (fine, rel) = (m.ctl(C_FINE), m.ctl(C_REL));
                let srate = net.sampling_freq;
                let chan = &mut self.channels[self.search_ptr];
                let ok = match chan.vf.as_mut() {
                    None => match tremor::File::open(src) {
                        Ok(f) => {
                            chan.vf = Some(f);
                            true
                        }
                        Err(_) => false,
                    },
                    Some(vf) => {
                        vf.rewind_source();
                        vf.time_seek(0);
                        true
                    }
                };
                if ok {
                    chan.playing = true;
                    chan.vel = e.velocity() as i32;
                    chan.id = e.id;
                    let pitch = e.pitch() / 4 - fine / 2 - (rel - 128) * 64;
                    let (dh, dl) = net::get_delta(srate, net::get_freq(pitch));
                    chan.delta_h = dh;
                    chan.delta_l = dl;
                    chan.ptr_h = 0;
                    chan.ptr_l = 0;
                    chan.loaded = 0;
                    self.no_active_channels = false;
                }
                retval = 1;
            }
            Cmd::SetFreq => {
                if self.no_active_channels {
                    return 0;
                }
                let m = &net.mods[n];
                let (ctl_channels, fine, rel) = ((m.ctl(C_CHANNELS).max(0) as usize).min(MAX_CHANNELS), m.ctl(C_FINE), m.ctl(C_REL));
                if let Some(c) = (0..ctl_channels).find(|&c| self.channels[c].id == e.id) {
                    let pitch = e.pitch() / 4 - fine / 2 - (rel - 128) * 64;
                    let (dh, dl) = net::get_delta(net.sampling_freq, net::get_freq(pitch));
                    let chan = &mut self.channels[c];
                    chan.delta_h = dh;
                    chan.delta_l = dl;
                    chan.vel = e.velocity() as i32;
                }
                retval = 1;
            }
            Cmd::SetVelocity => {
                if self.no_active_channels {
                    return 0;
                }
                let ctl_channels = (net.mods[n].ctl(C_CHANNELS).max(0) as usize).min(MAX_CHANNELS);
                if let Some(c) = (0..ctl_channels).find(|&c| self.channels[c].id == e.id) {
                    self.channels[c].vel = e.velocity() as i32;
                }
                retval = 1;
            }
            Cmd::SetSampleOffset | Cmd::SetSampleOffset2 => {
                if self.no_active_channels {
                    return 0;
                }
                let ctl_channels = (net.mods[n].ctl(C_CHANNELS).max(0) as usize).min(MAX_CHANNELS);
                if let Some(c) = (0..ctl_channels).find(|&c| self.channels[c].id == e.id) {
                    let total = self.src_pcm_total;
                    let chan = &mut self.channels[c];
                    if let Some(vf) = chan.vf.as_mut() {
                        let mut new_offset = if e.cmd == Cmd::SetSampleOffset { e.sample_offset() as u64 } else { total.wrapping_mul(e.sample_offset() as u64) / 0x8000 };
                        if new_offset >= total {
                            new_offset = total.wrapping_sub(1);
                        }
                        vf.pcm_seek(new_offset as i64);
                        retval = 1;
                    }
                }
            }
            Cmd::NoteOff => {
                if self.no_active_channels || net.mods[n].ctl(C_IGNORE_NOTEOFF) != 0 {
                    return 0;
                }
                if let Some(c) = (0..MAX_CHANNELS).find(|&c| self.channels[c].id == e.id) {
                    self.channels[c].playing = false;
                }
                retval = 1;
            }
            Cmd::SetGlobalController => {
                if e.ctl_num() == 0x98 {
                    self.pause = e.ctl_val() as i32;
                    retval = 1;
                }
            }
            Cmd::Clean | Cmd::AllNotesOff => {
                for c in self.channels.iter_mut() {
                    c.playing = false;
                    c.id = !0;
                }
                self.no_active_channels = true;
                retval = 1;
            }
            Cmd::Close => retval = 1,
            _ => {}
        }
        retval
    }
}
