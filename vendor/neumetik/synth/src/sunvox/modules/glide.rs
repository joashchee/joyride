// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! Glide (psynths_glide.cpp): its notes passed on to the modules it links
//! to, each slid from the last note's pitch to its own by SET_FREQs at a
//! rate of its own (one voice, or one a note), transposed and scaled.

use super::super::net::{self, Cmd, Event, Module, Net, FLAG_EXISTS, RT_MUTE};

const MAX_CHANNELS: usize = 32;
const MAX_RESPONSE: i32 = 1000;
const PITCH_SEMITONE: i32 = 10;
const PITCH_OCTAVES: i32 = 5;

const C_RESPONSE: usize = 0;
const C_FREQ: usize = 1;
const C_RESET_ON_START: usize = 2;
const C_POLY: usize = 3;
const C_PITCH: usize = 4;
const C_PITCH_SCALE: usize = 5;
const C_RESET: usize = 6;
const C_OCTAVE: usize = 7;
const C_FREQ_MUL: usize = 8;
const C_FREQ_DIV: usize = 9;

#[derive(Clone, Copy, Default)]
struct Chan {
    state: f32,
    id: u32,
    vel: i32,
    pitch: i32,
    floating_pitch: f32,
    parent_channel: i32,
    counter: u32,
}

#[derive(Default)]
pub(crate) struct Glide {
    tick_counter: i32,
    channels: Vec<Chan>,
    ch_flags_on: u32,
    search_ptr: usize,
    prev_ch: i32,
    ch_counter: u32,
}

/// 1 << c as the CPU does it (the shift taken mod 32).
fn bit(c: usize) -> u32 {
    1u32.wrapping_shl(c as u32)
}

impl Glide {
    fn channel_reset(&mut self, c: usize) {
        if c > MAX_CHANNELS {
            return;
        }
        let ch = &mut self.channels[c];
        ch.state = 0.0;
        ch.id = !0;
        ch.parent_channel = -1;
        if c < MAX_CHANNELS {
            self.ch_flags_on &= !bit(c);
        }
    }

    fn channel_off(&mut self, c: usize) {
        if c > MAX_CHANNELS {
            return;
        }
        let ch = &mut self.channels[c];
        if ch.state >= 1.0 {
            ch.state = 0.999;
        }
        if c < MAX_CHANNELS {
            self.ch_flags_on &= !bit(c);
        }
    }

    fn reset(&mut self) {
        self.tick_counter = 0;
        for c in 0..=MAX_CHANNELS {
            self.channels[c].counter = 0;
            self.channel_reset(c);
        }
        self.prev_ch = -1;
    }

    fn calc_pitch(m: &net::ModCommon, pitch: i32) -> i32 {
        let pitch_offset = m.ctl(C_PITCH) - PITCH_OCTAVES * 12 * PITCH_SEMITONE;
        let scale = m.ctl(C_PITCH_SCALE).max(1);
        let mut pitch = pitch - pitch_offset * scale * 256 / 100 / PITCH_SEMITONE - (m.ctl(C_OCTAVE) - 10) * 12 * 256;
        let (mul, div) = (m.ctl(C_FREQ_MUL), m.ctl(C_FREQ_DIV));
        if mul > 1 || div > 1 {
            let mut freq = 2.0f32.powf((30720.0 - pitch as f32) / 3072.0) * 16.333984375;
            if mul > 1 {
                freq *= mul as f32;
            }
            if div > 1 {
                freq /= div as f32;
            }
            pitch = (30720.0f32 - (freq / 16.333984375).log2() * 3072.0) as i32;
        }
        pitch
    }

    fn floating_step(&mut self, m: &net::ModCommon, c: usize, response: f32) {
        let ch = &mut self.channels[c];
        if ch.state < 1.0 {
            ch.state = (1.0 - response) * ch.state;
            if (ch.state as f64) < 0.01 {
                ch.state = 0.0;
            }
        }
        ch.floating_pitch = (1.0 - response) * ch.floating_pitch + response * Self::calc_pitch(m, ch.pitch) as f32;
    }

    fn multisend(n: usize, net: &mut Net, e: &Event) {
        let links = net.mods[n].output_links.clone();
        for l in links {
            if (l as u32) < net.mods_num() as u32 && net.mods[l as usize].flags & FLAG_EXISTS != 0 {
                net.add_event(l as usize, e);
            }
        }
    }

    fn multisend_pitch(n: usize, net: &mut Net, e: &mut Event, pitch: i32) {
        let links = net.mods[n].output_links.clone();
        for l in links {
            if (l as u32) < net.mods_num() as u32 && net.mods[l as usize].flags & FLAG_EXISTS != 0 {
                let s = &net.mods[l as usize];
                e.set_pitch(pitch - s.finetune - s.relative_note * 256);
                net.add_event(l as usize, e);
            }
        }
    }

    fn render(&mut self, n: usize, net: &mut Net) -> isize {
        let m = &net.mods[n];
        let (offset, frames) = (m.offset, m.frames);
        if m.realtime_flags & RT_MUTE != 0 {
            if self.ch_flags_on != 0 {
                for c in 0..=MAX_CHANNELS {
                    if self.ch_flags_on & bit(c) != 0 {
                        let mut e = Event::new(Cmd::NoteOff);
                        e.offset = offset;
                        e.set_velocity(256);
                        e.set_pitch(0);
                        e.id = self.channels[c].id;
                        net.multisend(n, &e);
                        self.channel_off(c);
                    }
                }
            }
            return 0;
        }
        let r = m.ctl(C_RESPONSE).max(1);
        let response = r as f32 / MAX_RESPONSE as f32;
        let (first_ch, last_ch) = if m.ctl(C_POLY) == 0 { (MAX_CHANNELS, MAX_CHANNELS) } else { (0, MAX_CHANNELS - 1) };
        let mut tick_size = 1;
        let f = m.ctl(C_FREQ);
        if f > 0 {
            tick_size = (net.sampling_freq * 256) / f;
        }
        if tick_size <= 0 {
            tick_size = 1;
        }
        let mut ptr = 0;
        loop {
            let mut buf_size = frames - ptr;
            if buf_size > (tick_size - self.tick_counter) / 256 {
                buf_size = (tick_size - self.tick_counter) / 256;
            }
            if (tick_size - self.tick_counter) & 255 != 0 {
                buf_size += 1;
            }
            if buf_size > frames - ptr {
                buf_size = frames - ptr;
            }
            if buf_size < 0 {
                buf_size = 0;
            }
            ptr += buf_size;
            self.tick_counter += buf_size * 256;
            if self.tick_counter >= tick_size {
                self.tick_counter %= tick_size;
                for c in first_ch..=last_ch {
                    if self.channels[c].state != 0.0 {
                        let prev = self.channels[c].floating_pitch as i32;
                        self.floating_step(&net.mods[n], c, response);
                        let ch = self.channels[c];
                        if ch.floating_pitch as i32 != prev {
                            let mut e = Event::new(Cmd::SetFreq);
                            e.id = ch.id;
                            e.offset = if ptr > 0 { offset + ptr - 1 } else { offset };
                            e.set_velocity(ch.vel);
                            Self::multisend_pitch(n, net, &mut e, ch.floating_pitch as i32);
                        }
                    }
                }
            }
            if ptr >= frames {
                break;
            }
        }
        0
    }
}

impl Module for Glide {
    fn handle(&mut self, n: usize, ev: &mut Event, net: &mut Net) -> isize {
        let mut retval: isize = 0;
        let mut e = *ev;
        e.id = net::evt_id_inc(e.id, n as u32);
        match ev.cmd {
            Cmd::Init => {
                let mid = PITCH_OCTAVES * 12 * PITCH_SEMITONE;
                net.register_ctl(n, 0, MAX_RESPONSE, (MAX_RESPONSE as f64 * 0.5) as i32, 0, C_RESPONSE, -1, 0);
                net.register_ctl(n, 1, 32768, 150, 0, C_FREQ, 150, 0);
                net.set_ctl_flags(n, 1, 2);
                net.register_ctl(n, 0, 1, 0, 1, C_RESET_ON_START, -1, 0);
                net.register_ctl(n, 0, 1, 1, 1, C_POLY, -1, 0);
                net.register_ctl(n, 0, mid * 2, mid, 0, C_PITCH, mid, 1);
                net.set_ctl_show_offset(n, 4, -mid);
                net.register_ctl(n, 0, 200, 100, 0, C_PITCH_SCALE, 100, 1);
                net.register_ctl(n, 0, 1, 0, 1, C_RESET, -1, 2);
                net.register_ctl(n, 0, 20, 10, 1, C_OCTAVE, 10, 1);
                net.set_ctl_show_offset(n, 7, -10);
                net.register_ctl(n, 1, 256, 1, 1, C_FREQ_MUL, 1, 1);
                net.set_ctl_flags(n, 8, 2);
                net.register_ctl(n, 1, 256, 1, 1, C_FREQ_DIV, 1, 1);
                net.set_ctl_flags(n, 9, 2);
                self.tick_counter = 0;
                self.ch_flags_on = 0;
                self.channels = vec![Chan::default(); MAX_CHANNELS + 1];
                for c in 0..=MAX_CHANNELS {
                    self.channel_reset(c);
                }
                self.search_ptr = 0;
                self.prev_ch = -1;
                self.ch_counter = 0;
                retval = 1;
            }
            Cmd::SetupFinished => retval = 1,
            Cmd::Clean => {
                self.reset();
                retval = 1;
            }
            Cmd::RenderReplace => retval = self.render(n, net),
            Cmd::NoteOn => {
                let mut channel_replace = false;
                if let Some(c) = (0..MAX_CHANNELS).find(|&c| self.channels[c].id == e.id) {
                    channel_replace = true;
                    self.channel_reset(c);
                }
                let mut k = 0;
                while k < MAX_CHANNELS {
                    if self.channels[self.search_ptr].state == 0.0 {
                        break;
                    }
                    self.search_ptr += 1;
                    if self.search_ptr >= MAX_CHANNELS {
                        self.search_ptr = 0;
                    }
                    k += 1;
                }
                if k == MAX_CHANNELS {
                    self.search_ptr += 1;
                    if self.search_ptr >= MAX_CHANNELS {
                        self.search_ptr = 0;
                    }
                    for _ in 0..MAX_CHANNELS {
                        if self.channels[self.search_ptr].state < 1.0 {
                            break;
                        }
                        self.search_ptr += 1;
                        if self.search_ptr >= MAX_CHANNELS {
                            self.search_ptr = 0;
                        }
                    }
                }
                let c = self.search_ptr;
                let m = &net.mods[n];
                let (poly, reset_on_start, response) = (m.ctl(C_POLY), m.ctl(C_RESET_ON_START), m.ctl(C_RESPONSE));
                {
                    let ch = &mut self.channels[c];
                    ch.state = 1.0;
                    ch.id = e.id;
                    ch.counter = self.ch_counter;
                    ch.vel = e.velocity() as i32;
                    ch.pitch = e.pitch();
                }
                self.ch_counter = self.ch_counter.wrapping_add(1);
                self.channels[c].floating_pitch = if self.prev_ch < 0 || (reset_on_start != 0 && self.ch_flags_on == 0) || response == MAX_RESPONSE {
                    Self::calc_pitch(m, self.channels[c].pitch) as f32
                } else if poly != 0 {
                    self.channels[self.prev_ch as usize].floating_pitch
                } else {
                    self.channels[MAX_CHANNELS].floating_pitch
                };
                self.prev_ch = c as i32;
                if poly == 0 {
                    if self.ch_flags_on == 0 {
                        let mchan = self.channels[MAX_CHANNELS];
                        if channel_replace && mchan.id != self.channels[c].id {
                            let mut e2 = e;
                            e2.cmd = Cmd::NoteOff;
                            e2.id = mchan.id;
                            Self::multisend_pitch(n, net, &mut e2, mchan.floating_pitch as i32);
                        }
                        self.channels[MAX_CHANNELS] = self.channels[c];
                        self.channels[MAX_CHANNELS].parent_channel = c as i32;
                    } else {
                        let (vel, pitch) = (self.channels[c].vel, self.channels[c].pitch);
                        let mc = &mut self.channels[MAX_CHANNELS];
                        mc.vel = vel;
                        mc.pitch = pitch;
                        mc.parent_channel = c as i32;
                    }
                }
                let first = self.ch_flags_on == 0;
                self.ch_flags_on |= bit(c);
                if !(!first && poly == 0) && net.mods[n].realtime_flags & RT_MUTE == 0 {
                    let fp = self.channels[c].floating_pitch as i32;
                    Self::multisend_pitch(n, net, &mut e, fp);
                }
                retval = 1;
            }
            Cmd::SetFreq => {
                let poly = net.mods[n].ctl(C_POLY);
                if let Some(c) = (0..MAX_CHANNELS).find(|&c| self.channels[c].id == e.id) {
                    let ch = &mut self.channels[c];
                    ch.pitch = e.pitch();
                    ch.vel = e.velocity() as i32;
                    if ch.state < 1.0 {
                        ch.state = 0.999;
                    }
                    let (p, v) = (ch.pitch, ch.vel);
                    if poly == 0 {
                        let mc = &mut self.channels[MAX_CHANNELS];
                        if mc.parent_channel == c as i32 {
                            mc.pitch = p;
                            mc.vel = v;
                            if mc.state < 1.0 {
                                mc.state = 0.999;
                            }
                        }
                    }
                }
                retval = 1;
            }
            Cmd::SetVelocity => {
                let poly = net.mods[n].ctl(C_POLY);
                if let Some(c) = (0..MAX_CHANNELS).find(|&c| self.channels[c].id == e.id) {
                    self.channels[c].vel = e.velocity() as i32;
                    if poly == 0 && self.channels[MAX_CHANNELS].parent_channel == c as i32 {
                        self.channels[MAX_CHANNELS].vel = self.channels[c].vel;
                        e.id = self.channels[MAX_CHANNELS].id;
                    }
                }
                Self::multisend(n, net, &e);
                retval = 1;
            }
            Cmd::NoteOff => {
                if self.ch_flags_on == 0 {
                    return 0;
                }
                let poly = net.mods[n].ctl(C_POLY);
                if let Some(c) = (0..MAX_CHANNELS).find(|&c| self.ch_flags_on & bit(c) != 0 && self.channels[c].id == e.id) {
                    self.channel_off(c);
                    if poly == 0 && self.channels[MAX_CHANNELS].parent_channel == c as i32 {
                        if self.ch_flags_on == 0 {
                            self.channel_off(MAX_CHANNELS);
                            e.id = self.channels[MAX_CHANNELS].id;
                            Self::multisend(n, net, &e);
                        } else {
                            let mut cnt = 0u32;
                            let mut last_ch = -1i32;
                            for c2 in 0..MAX_CHANNELS {
                                if self.ch_flags_on & bit(c2) != 0 && self.channels[c2].counter >= cnt {
                                    cnt = self.channels[c2].counter;
                                    last_ch = c2 as i32;
                                }
                            }
                            if last_ch >= 0 {
                                let ch2 = self.channels[last_ch as usize];
                                let mc = &mut self.channels[MAX_CHANNELS];
                                mc.vel = ch2.vel;
                                mc.pitch = ch2.pitch;
                                mc.parent_channel = last_ch;
                            }
                        }
                    }
                }
                if poly != 0 {
                    Self::multisend(n, net, &e);
                }
                retval = 1;
            }
            Cmd::AllNotesOff => {
                if self.ch_flags_on != 0 {
                    for c in 0..=MAX_CHANNELS {
                        self.channel_off(c);
                    }
                }
                Self::multisend(n, net, &e);
                retval = 1;
            }
            Cmd::SetLocalController | Cmd::SetGlobalController => {
                let mut release_all = false;
                match ev.ctl_num() as usize {
                    C_POLY => release_all = true,
                    C_RESET => {
                        release_all = true;
                        let m = &mut net.mods[n];
                        if let Some(var) = m.ctls.get(C_RESET).and_then(|c| c.var) {
                            m.vars[var] = 0;
                        }
                        retval = 1;
                    }
                    _ => {}
                }
                if release_all {
                    let e = Event::new(Cmd::AllNotesOff);
                    Self::multisend(n, net, &e);
                    self.reset();
                }
            }
            Cmd::Close => retval = 1,
            _ => {}
        }
        retval
    }
}
