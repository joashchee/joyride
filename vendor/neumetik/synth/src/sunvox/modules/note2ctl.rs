// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! Velocity2Ctl and Pitch2Ctl (psynths_velocity2ctl.cpp,
//! psynths_pitch2ctl.cpp): the last note held, its velocity or its pitch
//! (as a frequency or a note range), sent as a controller value to the
//! modules they link to; the two follow notes the same way.

use super::super::net::{self, Cmd, Event, Module, Net, FLAG_EXISTS, RT_MUTE};

const MAX_CHANNELS: usize = 16;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Which {
    Velocity,
    Pitch,
}

// Velocity2Ctl's controllers.
const V_NOTE_OFF: usize = 0;
const V_MIN: usize = 1;
const V_MAX: usize = 2;
const V_OFFSET: usize = 3;
const V_NUM: usize = 4;

// Pitch2Ctl's controllers.
const P_MODE: usize = 0;
const P_NOTE_OFF: usize = 1;
const P_NOTE: usize = 2;
const P_NOTES: usize = 3;
const P_MIN: usize = 4;
const P_MAX: usize = 5;
const P_NUM: usize = 6;

#[derive(Clone, Copy, Default)]
struct Chan {
    id: u32,
    val: i32,
    counter: u32,
}

pub(crate) struct Note2Ctl {
    which: Which,
    channels: [Chan; MAX_CHANNELS],
    ch_flags: u32,
    search_ptr: usize,
    counter: u32,
    fg: usize,
}

impl Note2Ctl {
    pub fn new(which: Which) -> Note2Ctl {
        Note2Ctl { which, channels: [Chan::default(); MAX_CHANNELS], ch_flags: 0, search_ptr: 0, counter: 0, fg: 0 }
    }

    fn channel_reset(&mut self, c: usize) {
        if c >= MAX_CHANNELS {
            return;
        }
        self.channels[c].id = 0xFFFF_FFFF;
        self.ch_flags &= !(1 << c);
    }

    fn reset(&mut self) {
        for c in 0..MAX_CHANNELS {
            self.channel_reset(c);
        }
        self.counter = 0;
        self.fg = 0;
        self.ch_flags = 0;
    }

    fn send(&mut self, n: usize, net: &mut Net, src: &Event) {
        let m = &net.mods[n];
        if m.realtime_flags & RT_MUTE != 0 {
            return;
        }
        let vel = self.which == Which::Velocity;
        let ctl_num = m.ctl(if vel { V_NUM } else { P_NUM }) - 1;
        if ctl_num < 0 {
            return;
        }
        let (min, max) = if vel { (m.ctl(V_MIN), m.ctl(V_MAX)) } else { (m.ctl(P_MIN), m.ctl(P_MAX)) };
        let mut note_off = 0;
        if self.ch_flags == 0 {
            note_off = m.ctl(if vel { V_NOTE_OFF } else { P_NOTE_OFF });
            if note_off == 0 {
                return;
            }
        }
        let chan = self.channels[self.fg];
        let mut e = Event::new(Cmd::SetGlobalController);
        e.offset = src.offset;
        e.id = src.id;
        e.set_ctl_num(ctl_num as u16);
        let mode = m.ctl(P_MODE);
        let (mut freq, mut pitch) = (0, 0);
        if !vel && note_off == 0 {
            if mode == 0 {
                freq = net::get_freq(chan.val / 4 + 64 * 12 * 5);
            } else {
                pitch = net::NOTE0_PITCH - chan.val;
                pitch -= m.ctl(P_NOTE) * 256;
            }
        }
        let (offset_ctl, notes) = (m.ctl(V_OFFSET), m.ctl(P_NOTES).max(1));
        let links = m.output_links.clone();
        for l in links {
            if (l as u32) >= net.mods_num() as u32 || net.mods[l as usize].flags & FLAG_EXISTS == 0 {
                continue;
            }
            let mut val = 0;
            if note_off == 0 {
                if vel {
                    val = min + (chan.val * (max - min)) / 256;
                } else if mode == 0 {
                    val = net.get_scaled_ctl_value(l as usize, ctl_num as usize, freq, false);
                    if min != 0 || max != 32768 {
                        val = min + (val * (max - min)) / 32768;
                    }
                } else {
                    val = min + (pitch * (max - min)) / (notes * 256);
                }
            } else {
                match note_off {
                    1 => val = min,
                    2 => val = max,
                    _ => {}
                }
            }
            if vel {
                val += offset_ctl - 16384;
            }
            if max >= min {
                val = val.max(min).min(max);
            } else {
                if val < max {
                    val = max;
                }
                if val > min {
                    val = min;
                }
            }
            e.set_ctl_val(val as u16);
            net.add_event(l as usize, &e);
        }
    }
}

impl Module for Note2Ctl {
    fn handle(&mut self, n: usize, e: &mut Event, net: &mut Net) -> isize {
        let mut retval: isize = 0;
        match e.cmd {
            Cmd::Init => {
                match self.which {
                    Which::Velocity => {
                        net.register_ctl(n, 0, 2, 0, 1, V_NOTE_OFF, -1, 0);
                        net.register_ctl(n, 0, 32768, 0, 0, V_MIN, -1, 1);
                        net.register_ctl(n, 0, 32768, 32768, 0, V_MAX, -1, 1);
                        net.register_ctl(n, 0, 32768, 16384, 0, V_OFFSET, 16384, 1);
                        net.set_ctl_show_offset(n, 3, -16384);
                        net.register_ctl(n, 0, 255, 0, 1, V_NUM, -1, 1);
                    }
                    Which::Pitch => {
                        net.register_ctl(n, 0, 1, 0, 1, P_MODE, -1, 0);
                        net.register_ctl(n, 0, 2, 0, 1, P_NOTE_OFF, -1, 0);
                        net.register_ctl(n, 0, 256, 0, 1, P_NOTE, -1, 1);
                        net.register_ctl(n, 0, 256, 120, 1, P_NOTES, 120, 1);
                        net.register_ctl(n, 0, 32768, 0, 0, P_MIN, -1, 2);
                        net.register_ctl(n, 0, 32768, 32768, 0, P_MAX, -1, 2);
                        net.register_ctl(n, 0, 255, 0, 1, P_NUM, -1, 2);
                    }
                }
                self.channels = [Chan::default(); MAX_CHANNELS];
                self.reset();
                self.search_ptr = 0;
                retval = 1;
            }
            Cmd::NoteOn => {
                if self.ch_flags != 0 {
                    if let Some(c) = (0..MAX_CHANNELS).find(|&c| self.channels[c].id == e.id) {
                        self.channel_reset(c);
                    }
                    let mut c = 0;
                    while c < MAX_CHANNELS {
                        if self.channels[self.search_ptr].id == 0xFFFF_FFFF {
                            break;
                        }
                        self.search_ptr = (self.search_ptr + 1) % MAX_CHANNELS;
                        c += 1;
                    }
                    if c == MAX_CHANNELS {
                        self.search_ptr = (self.search_ptr + 1) % MAX_CHANNELS;
                    }
                } else {
                    self.search_ptr = 0;
                }
                let c = self.search_ptr;
                self.channels[c] = Chan { id: e.id, counter: self.counter, val: if self.which == Which::Velocity { e.velocity() as i32 } else { e.pitch() } };
                self.counter = self.counter.wrapping_add(1);
                self.ch_flags |= 1 << c;
                self.fg = c;
                self.send(n, net, e);
                retval = 1;
            }
            Cmd::SetVelocity | Cmd::SetFreq => {
                let mine = if self.which == Which::Velocity { Cmd::SetVelocity } else { Cmd::SetFreq };
                if e.cmd != mine {
                    return 0;
                }
                if self.ch_flags == 0 {
                    return 0;
                }
                if let Some(c) = (0..MAX_CHANNELS).find(|&c| self.channels[c].id == e.id) {
                    self.channels[c].val = if self.which == Which::Velocity { e.velocity() as i32 } else { e.pitch() };
                    if self.fg == c {
                        self.send(n, net, e);
                    }
                }
                retval = 1;
            }
            Cmd::NoteOff => {
                if self.ch_flags == 0 {
                    return 0;
                }
                if let Some(c) = (0..MAX_CHANNELS).find(|&c| self.channels[c].id == e.id) {
                    self.channel_reset(c);
                    if self.ch_flags != 0 {
                        let mut cnt = 0u32;
                        let mut last_ch = 0usize;
                        for c2 in 0..MAX_CHANNELS {
                            if self.ch_flags & (1 << c2) != 0 && self.channels[c2].counter >= cnt {
                                cnt = self.channels[c2].counter;
                                last_ch = c2;
                            }
                        }
                        if self.fg != last_ch {
                            self.fg = last_ch;
                            self.send(n, net, e);
                        }
                    } else {
                        self.send(n, net, e);
                    }
                }
                retval = 1;
            }
            Cmd::Clean | Cmd::AllNotesOff => {
                self.reset();
                if e.cmd == Cmd::AllNotesOff {
                    self.send(n, net, e);
                }
                retval = 1;
            }
            Cmd::Close => retval = 1,
            _ => {}
        }
        retval
    }
}
