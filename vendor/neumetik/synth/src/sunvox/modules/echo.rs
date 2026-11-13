// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! Echo (psynths_echo.cpp): a delay line a channel with feedback, its
//! length in 256ths of a second, milliseconds, Hz or the song's ticks or
//! lines, the right channel's writes offset, a one-pole low or high cut
//! in the loop.

use super::super::net::{Cmd, Event, Module, Net};

const OUTPUTS: usize = 2;
const MAX_FFREQ: i32 = 22000;
const DELAY_UNIT_MAX: i32 = 6;
const CHANGED_BUFSIZE: u32 = 1 << 0;
const CHANGED_FILTER: u32 = 1 << 1;

const C_DRY: usize = 0;
const C_WET: usize = 1;
const C_FEEDBACK: usize = 2;
const C_DELAY: usize = 3;
const C_ROFFSET: usize = 4;
const C_DELAY_UNITS: usize = 5;
const C_ROFFSET_VAL: usize = 6;
const C_FILTER: usize = 7;
const C_FFREQ: usize = 8;

#[derive(Default)]
pub(crate) struct Echo {
    max_buf_size: i32,
    buf_size: i32,
    delay_len: i32,
    buf: [Vec<f32>; OUTPUTS],
    buf_ptr: i32,
    buf_clean: bool,
    empty: bool,
    tick_size: i32,
    ticks_per_line: u8,
    f_a0: f32,
    f_b1: f32,
    f_v: [f32; OUTPUTS],
    changed: u32,
}

impl Echo {
    fn change_ctl_limits(&mut self, n: usize, net: &mut Net) {
        if net.base_host_version < 0x01090202 {
            return;
        }
        let max = match net.mods[n].ctl(C_DELAY_UNITS) {
            1 => 4000,
            2 => 8192,
            _ => 256,
        };
        if net.mods[n].ctls.get(3).is_some_and(|c| c.max != max) {
            net.change_ctl_limits(n, 3, -1, max, max);
        }
    }

    fn handle_changes(&mut self, n: usize, net: &Net) {
        let m = &net.mods[n];
        let srate = net.sampling_freq;
        if self.changed & CHANGED_BUFSIZE != 0 {
            let units = m.ctl(C_DELAY_UNITS);
            let delay = m.ctl(C_DELAY);
            let mut delay_len = match units {
                0 => (srate * delay) >> 8,
                1 => (srate * delay) / 1000,
                2 => {
                    if delay == 0 {
                        srate
                    } else {
                        srate / delay
                    }
                }
                3 => ((self.tick_size as u32 as u64 * delay as u32 as u64) / 256) as i32,
                4..=6 => {
                    let d = ((self.tick_size as u32 as u64 * self.ticks_per_line as u64 * delay as u32 as u64) / 256) as i32;
                    d / (units - 3)
                }
                _ => 0,
            };
            if delay_len == 0 {
                delay_len = 1;
            }
            if delay_len > self.max_buf_size {
                delay_len = self.max_buf_size;
            }
            let mut changed = false;
            while delay_len > self.buf_size {
                if self.buf_size == 0 {
                    self.buf_size = srate;
                } else {
                    self.buf_size += srate;
                }
                changed = true;
            }
            if changed {
                for b in self.buf.iter_mut() {
                    b.resize(self.buf_size as usize, 0.0);
                }
            }
            self.delay_len = delay_len;
            if self.buf_ptr >= delay_len {
                self.buf_ptr = 0;
            }
        }
        if self.changed & CHANGED_FILTER != 0 {
            let (mut a0, mut b1) = (0.0f32, 0.0f32);
            if matches!(m.ctl(C_FILTER), 1 | 2) {
                b1 = (-2.0 * std::f64::consts::PI * (m.ctl(C_FFREQ) as f32 / srate as f32) as f64).exp() as f32;
                a0 = (1.0 - b1 as f64) as f32;
            }
            self.f_b1 = b1;
            self.f_a0 = a0;
        }
        self.changed = 0;
    }

    fn render(&mut self, n: usize, net: &mut Net) -> isize {
        let m = &net.mods[n];
        let (frames, offset) = (m.frames as usize, m.offset as usize);
        let mut input_signal = false;
        for ch in 0..OUTPUTS {
            if m.in_empty[ch] < (offset + frames) as i32 {
                self.empty = false;
                input_signal = true;
                break;
            }
        }
        if self.empty {
            return 0;
        }
        if self.changed != 0 {
            self.handle_changes(n, net);
        }
        self.buf_clean = false;
        let m = &mut net.mods[n];
        let (ctl_dry, ctl_wet, ctl_feedback) = (m.ctl(C_DRY), m.ctl(C_WET), m.ctl(C_FEEDBACK));
        let dry = ctl_dry as f32 / 256.0;
        let wet = ctl_wet as f32 / 256.0;
        let feedback = ctl_feedback as f32 / 256.0;
        let filter = m.ctl(C_FILTER);
        let mut roffset = 0i32;
        if m.ctl(C_ROFFSET) != 0 {
            roffset = (self.delay_len as u32 as u64 * m.ctl(C_ROFFSET_VAL) as u32 as u64 / 32768) as i32;
        }
        let dl = self.delay_len;
        let mut buf_ptr = self.buf_ptr;
        for ch in 0..OUTPUTS {
            let (chin, chout) = (&m.chin, &mut m.chout);
            let inp = &chin[ch][offset..offset + frames];
            let out = &mut chout[ch][offset..offset + frames];
            let cbuf = &mut self.buf[ch];
            buf_ptr = self.buf_ptr;
            if filter != 0 {
                let write_offset = if ch != 0 && roffset != 0 { roffset } else { 0 };
                let mut f_v = self.f_v[ch];
                for i in 0..frames {
                    let mut v = cbuf[buf_ptr as usize];
                    f_v = v * self.f_a0 + f_v * self.f_b1;
                    v = if filter == 1 { f_v } else { v - f_v };
                    out[i] = v * wet;
                    v *= feedback;
                    cbuf[buf_ptr as usize] = v;
                    let mut p2 = buf_ptr + write_offset;
                    if p2 >= dl {
                        p2 -= dl;
                    }
                    cbuf[p2 as usize] += inp[i];
                    buf_ptr += 1;
                    if buf_ptr >= dl {
                        buf_ptr = 0;
                    }
                }
                self.f_v[ch] = f_v;
            } else if input_signal {
                for i in 0..frames {
                    let mut v = cbuf[buf_ptr as usize];
                    out[i] = v * wet;
                    v *= feedback;
                    cbuf[buf_ptr as usize] = v;
                    if ch != 0 && roffset != 0 {
                        let mut p2 = buf_ptr + roffset;
                        if p2 >= dl {
                            p2 -= dl;
                        }
                        cbuf[p2 as usize] += inp[i];
                    } else {
                        cbuf[buf_ptr as usize] += inp[i];
                    }
                    buf_ptr += 1;
                    if buf_ptr >= dl {
                        buf_ptr = 0;
                    }
                }
            } else {
                for o in out.iter_mut() {
                    let mut v = cbuf[buf_ptr as usize];
                    *o = v * wet;
                    v *= feedback;
                    cbuf[buf_ptr as usize] = v;
                    buf_ptr += 1;
                    if buf_ptr >= dl {
                        buf_ptr = 0;
                    }
                }
            }
            if input_signal {
                if ctl_dry > 0 && ctl_dry != 256 {
                    for i in 0..frames {
                        out[i] += inp[i] * dry;
                    }
                }
                if ctl_dry == 256 {
                    for i in 0..frames {
                        out[i] += inp[i];
                    }
                }
            }
        }
        self.buf_ptr = buf_ptr;
        1
    }
}

impl Module for Echo {
    fn handle(&mut self, n: usize, e: &mut Event, net: &mut Net) -> isize {
        let mut retval: isize = 0;
        match e.cmd {
            Cmd::Init => {
                net.register_ctl(n, 0, 256, 256, 0, C_DRY, -1, 0);
                net.register_ctl(n, 0, 256, 40, 0, C_WET, -1, 0);
                net.register_ctl(n, 0, 256, 128, 0, C_FEEDBACK, -1, 1);
                net.register_ctl(n, 0, 256, 256, 0, C_DELAY, -1, 1);
                net.register_ctl(n, 0, 1, 1, 1, C_ROFFSET, -1, 1);
                net.register_ctl(n, 0, DELAY_UNIT_MAX, 0, 1, C_DELAY_UNITS, -1, 1);
                net.register_ctl(n, 0, 32768, 32768 / 2, 0, C_ROFFSET_VAL, -1, 1);
                net.register_ctl(n, 0, 2, 0, 1, C_FILTER, -1, 1);
                net.register_ctl(n, 0, MAX_FFREQ, 2000, 0, C_FFREQ, -1, 1);
                net.set_ctl_flags(n, 8, 2);
                self.max_buf_size = net.sampling_freq * 4;
                self.buf_ptr = 0;
                self.buf_clean = true;
                self.empty = true;
                self.changed = 0xFFFF_FFFF;
                retval = 1;
            }
            Cmd::SetupFinished => {
                self.change_ctl_limits(n, net);
                self.handle_changes(n, net);
                retval = 1;
            }
            Cmd::Clean => {
                if !self.buf_clean {
                    for i in 0..OUTPUTS {
                        self.buf[i].fill(0.0);
                        self.f_v[i] = 0.0;
                    }
                    self.buf_clean = true;
                }
                self.buf_ptr = 0;
                self.empty = true;
                retval = 1;
            }
            Cmd::RenderReplace => retval = self.render(n, net),
            Cmd::SetLocalController | Cmd::SetGlobalController => match e.ctl_num() {
                3 => self.changed |= CHANGED_BUFSIZE,
                5 => {
                    let v = (e.ctl_val() as i32).min(DELAY_UNIT_MAX);
                    if let Some(var) = net.mods[n].ctls.get(C_DELAY_UNITS).and_then(|c| c.var) {
                        net.mods[n].vars[var] = v;
                    }
                    self.change_ctl_limits(n, net);
                    self.changed |= CHANGED_BUFSIZE;
                    retval = 1;
                }
                7 | 8 => self.changed |= CHANGED_FILTER,
                _ => {}
            },
            Cmd::SpeedChanged => {
                self.tick_size = e.tick_size();
                self.ticks_per_line = e.tpl();
                if net.mods[n].ctl(C_DELAY_UNITS) >= 3 {
                    self.changed |= CHANGED_BUFSIZE;
                }
                retval = 1;
            }
            Cmd::Close => retval = 1,
            _ => {}
        }
        retval
    }
}
