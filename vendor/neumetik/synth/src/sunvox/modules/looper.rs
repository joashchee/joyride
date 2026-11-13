// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! Loop (psynths_loop.cpp): a tape loop, a stretch of the input (its
//! length in lines, ticks, milliseconds or Hz) recorded then repeated a
//! number of times, forward or ping-pong, with a short crossfade.

use super::super::net::{Cmd, Event, Module, Net};

const OUTPUTS: usize = 2;
const CHANGED_BUFSIZE: u32 = 1 << 0;
const CHANGED_IOCHANNELS: u32 = 1 << 1;
const LEN_UNIT_MAX: i32 = 6;

const C_VOLUME: usize = 0;
const C_LEN: usize = 1;
const C_STEREO: usize = 2;
const C_REPEAT: usize = 3;
const C_MODE: usize = 4;
const C_LEN_UNITS: usize = 5;
const C_MAX_BUF_SIZE: usize = 6;
const C_ON_NOTEON: usize = 7;

#[derive(Default)]
pub(crate) struct Looper {
    rep_counter: i32,
    anticlick: bool,
    anticlick_from: [f32; OUTPUTS],
    buf: [Vec<f32>; OUTPUTS],
    buf_size: i32,
    len: i32,
    buf_ptr: i32,
    buf_clean: bool,
    empty: bool,
    tick_size: i32,
    ticks_per_line: u8,
    changed: u32,
}

impl Looper {
    fn change_ctl_limits(n: usize, net: &mut Net) {
        if net.base_host_version < 0x02010000 {
            return;
        }
        let max = if net.mods[n].ctl(C_LEN_UNITS) >= 1 { 8192 } else { 256 };
        if net.mods[n].ctls.get(C_LEN).is_some_and(|c| c.max != max) {
            net.change_ctl_limits(n, C_LEN, -1, max, max);
        }
    }

    fn handle_changes(&mut self, n: usize, net: &mut Net) {
        if self.changed & CHANGED_IOCHANNELS != 0 {
            let want = if net.mods[n].ctl(C_STEREO) != 0 { OUTPUTS as i32 } else { 1 };
            if net.mods[n].output_channels != want {
                net.set_number_of_outputs(want, n);
                net.set_number_of_inputs(want, n);
            }
        }
        if self.changed & CHANGED_BUFSIZE != 0 {
            let m = &net.mods[n];
            let srate = net.sampling_freq;
            let (ctl_len, units) = (m.ctl(C_LEN), m.ctl(C_LEN_UNITS));
            let line = |div: u64| ((self.tick_size as u32 as u64 * self.ticks_per_line as u64 * ctl_len as u32 as u64) / div) as i32;
            let mut len = match units {
                0 => {
                    let mut len = line(256) / 128;
                    if net.base_host_version < 0x02010000 && len > srate / 3 {
                        len = srate / 3;
                    }
                    len
                }
                1..=3 => line(256) / units,
                4 => ((self.tick_size as u32 as u64 * ctl_len as u32 as u64) / 256) as i32,
                5 => (srate * ctl_len) / 1000,
                6 => {
                    if ctl_len == 0 {
                        srate
                    } else {
                        srate / ctl_len
                    }
                }
                _ => 0,
            };
            if len == 0 {
                len = 1;
            }
            let max_buf_size = m.ctl(C_MAX_BUF_SIZE) * srate;
            if len > max_buf_size {
                len = max_buf_size;
            }
            let mut buf_size_changed = false;
            while len > self.buf_size {
                if self.buf_size == 0 {
                    self.buf_size = len;
                } else {
                    self.buf_size += srate;
                }
                buf_size_changed = true;
            }
            if buf_size_changed {
                for b in self.buf.iter_mut() {
                    b.resize(self.buf_size as usize, 0.0);
                }
            }
            self.len = len;
            if self.buf_ptr >= len {
                self.buf_ptr %= len;
            }
        }
        self.changed = 0;
    }

    fn render(&mut self, n: usize, net: &mut Net) -> isize {
        if self.changed != 0 {
            self.handle_changes(n, net);
        }
        let srate = net.sampling_freq;
        let m = &mut net.mods[n];
        let (offset, frames) = (m.offset as usize, m.frames as usize);
        let outputs_num = m.output_channels.clamp(0, 2) as usize;
        if (0..outputs_num).any(|ch| m.in_empty[ch] < (offset + frames) as i32) {
            self.empty = false;
        }
        if self.empty {
            return 0;
        }
        self.buf_clean = false;
        let len = self.len;
        let (ctl_volume, ctl_mode, ctl_repeat) = (m.ctl(C_VOLUME), m.ctl(C_MODE), m.ctl(C_REPEAT));
        let volume = ctl_volume as f32 / 256.0;
        let mut anticlick_len = srate / 1000;
        if anticlick_len > len / 2 {
            anticlick_len = len / 2;
        }
        let (mut rep_counter, mut anticlick, mut buf_ptr) = (0, false, 0);
        for ch in 0..outputs_num {
            let (ci, co) = (&m.chin, &mut m.chout);
            let inp = &ci[ch][offset..offset + frames];
            let out = &mut co[ch][offset..offset + frames];
            let cbuf = &mut self.buf[ch];
            buf_ptr = self.buf_ptr;
            rep_counter = self.rep_counter;
            anticlick = self.anticlick;
            let mut i = 0usize;
            while i < frames {
                let mut size2 = len - buf_ptr;
                if size2 > (frames - i) as i32 {
                    size2 = (frames - i) as i32;
                }
                let i_end = i + size2.max(0) as usize;
                if rep_counter == 0 {
                    while i < i_end {
                        if anticlick && buf_ptr < anticlick_len {
                            let c = (buf_ptr << 8) / anticlick_len;
                            out[i] = (inp[i] * c as f32) / 256.0 + (self.anticlick_from[ch] * (256 - c) as f32) / 256.0;
                        } else {
                            out[i] = inp[i];
                        }
                        cbuf[buf_ptr as usize] = inp[i];
                        buf_ptr += 1;
                        i += 1;
                    }
                } else if ctl_mode == 0 {
                    let interp_from = cbuf[(len - 1) as usize];
                    while i < i_end {
                        if buf_ptr < anticlick_len {
                            let c = (buf_ptr << 8) / anticlick_len;
                            out[i] = (cbuf[buf_ptr as usize] * c as f32) / 256.0 + (interp_from * (256 - c) as f32) / 256.0;
                        } else {
                            out[i] = cbuf[buf_ptr as usize];
                        }
                        buf_ptr += 1;
                        i += 1;
                    }
                } else if ctl_mode == 1 {
                    let rev = rep_counter & 1 != 0;
                    while i < i_end {
                        out[i] = if rev { cbuf[(len - 1 - buf_ptr) as usize] } else { cbuf[buf_ptr as usize] };
                        buf_ptr += 1;
                        i += 1;
                    }
                } else {
                    i = i_end;
                }
                if buf_ptr >= len {
                    rep_counter += 1;
                    anticlick = false;
                    if rep_counter > ctl_repeat && ctl_repeat != 128 {
                        if rep_counter > 1 {
                            self.anticlick_from[ch] = if ctl_mode == 1 && rep_counter & 1 == 0 { cbuf[0] } else { cbuf[(len - 1) as usize] };
                            anticlick = true;
                        }
                        rep_counter = 0;
                    }
                    buf_ptr = 0;
                }
            }
            if ctl_volume != 256 {
                for o in out.iter_mut() {
                    *o *= volume;
                }
            }
        }
        self.buf_ptr = buf_ptr;
        self.rep_counter = rep_counter;
        self.anticlick = anticlick;
        if ctl_volume == 0 {
            2
        } else {
            1
        }
    }
}

impl Module for Looper {
    fn handle(&mut self, n: usize, e: &mut Event, net: &mut Net) -> isize {
        let mut retval: isize = 0;
        match e.cmd {
            Cmd::Init => {
                net.register_ctl(n, 0, 256, 256, 0, C_VOLUME, -1, 0);
                net.register_ctl(n, 0, 256, 256, 0, C_LEN, -1, 0);
                net.register_ctl(n, 0, 1, 1, 1, C_STEREO, -1, 0);
                net.register_ctl(n, 0, 128, 0, 1, C_REPEAT, -1, 0);
                net.register_ctl(n, 0, 1, 0, 1, C_MODE, -1, 0);
                net.register_ctl(n, 0, LEN_UNIT_MAX, 0, 1, C_LEN_UNITS, -1, 0);
                net.register_ctl(n, 1, 60 * 4, 4, 1, C_MAX_BUF_SIZE, -1, 0);
                net.register_ctl(n, 0, 1, 0, 1, C_ON_NOTEON, -1, 0);
                self.buf_clean = true;
                self.empty = true;
                self.changed = 0xFFFF_FFFF;
                retval = 1;
            }
            Cmd::SetupFinished => {
                self.handle_changes(n, net);
                Self::change_ctl_limits(n, net);
                retval = 1;
            }
            Cmd::Clean => {
                if !self.buf_clean {
                    for b in self.buf.iter_mut() {
                        b.fill(0.0);
                    }
                    self.buf_clean = true;
                }
                self.buf_ptr = 0;
                self.rep_counter = 0;
                self.anticlick = false;
                self.anticlick_from = [0.0; OUTPUTS];
                self.empty = true;
                retval = 1;
            }
            Cmd::RenderReplace => retval = self.render(n, net),
            Cmd::SetLocalController | Cmd::SetGlobalController => match e.ctl_num() {
                2 => self.changed |= CHANGED_IOCHANNELS,
                3 => {
                    self.rep_counter = 0;
                    self.buf_ptr = 0;
                }
                1 | 6 => self.changed |= CHANGED_BUFSIZE,
                5 => {
                    let v = (e.ctl_val() as i32).min(LEN_UNIT_MAX);
                    let m = &mut net.mods[n];
                    if let Some(var) = m.ctls.get(C_LEN_UNITS).and_then(|c| c.var) {
                        m.vars[var] = v;
                    }
                    Self::change_ctl_limits(n, net);
                    self.changed |= CHANGED_BUFSIZE;
                    retval = 1;
                }
                _ => {}
            },
            Cmd::NoteOn => {
                if net.mods[n].ctl(C_ON_NOTEON) == 1 {
                    self.buf_ptr = 0;
                } else {
                    self.rep_counter = 0;
                    self.buf_ptr = 0;
                }
            }
            Cmd::SpeedChanged => {
                self.tick_size = e.tick_size();
                self.ticks_per_line = e.tpl();
                if net.mods[n].ctl(C_LEN_UNITS) <= 4 {
                    self.changed |= CHANGED_BUFSIZE;
                }
                retval = 1;
            }
            Cmd::ResetMsb => self.changed |= CHANGED_IOCHANNELS,
            Cmd::Close => retval = 1,
            _ => {}
        }
        retval
    }
}
