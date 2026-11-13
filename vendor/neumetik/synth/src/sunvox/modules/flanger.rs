// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! Flanger (psynths_flanger.cpp): a delay of up to 1/64 second, read
//! between two samples, with feedback; its length glides to the delay
//! plus an LFO (a half sine or a sine) stepped a tick at a time.

use super::super::dsp::HSIN_TAB;
use super::super::net::{Cmd, Event, Module, Net};

const OUTPUTS: usize = 2;
const RESP_PREC: i32 = 19;

const C_DRY: usize = 0;
const C_WET: usize = 1;
const C_FEEDBACK: usize = 2;
const C_DELAY: usize = 3;
const C_RESPONSE: usize = 4;
const C_VIB_SPEED: usize = 5;
const C_VIB_AMP: usize = 6;
const C_VIB_TYPE: usize = 7;
const C_SET_VIB_PHASE: usize = 8;
const C_VIB_UNITS: usize = 9;

#[derive(Default)]
pub(crate) struct Flanger {
    buf_size: i32,
    buf: [Vec<f32>; OUTPUTS],
    buf_ptr: i32,
    buf_empty: bool,
    tick_counter: i32,
    floating_delay: i32,
    vibrato_ptr: u32,
    empty: bool,
    tick_size: i32,
    ticks_per_line: u8,
}

impl Flanger {
    fn set_phase(&mut self, n: usize, net: &mut Net, check_version: bool) {
        if !check_version || net.base_host_version >= 0x01090300 {
            let mut e = Event::new(Cmd::SetGlobalController);
            e.set_ctl_num(8);
            e.set_ctl_val((net.mods[n].ctl(C_SET_VIB_PHASE) << 7) as u16);
            self.handle(n, &mut e, net);
        }
    }

    fn render(&mut self, n: usize, net: &mut Net) -> isize {
        let srate = net.sampling_freq;
        let m = &mut net.mods[n];
        let (frames, offset) = (m.frames, m.offset as usize);
        let mut input_signal = false;
        for ch in 0..OUTPUTS {
            if m.in_empty[ch] < offset as i32 + frames {
                self.empty = false;
                input_signal = true;
                break;
            }
        }
        if self.empty {
            return 0;
        }
        self.buf_empty = false;
        let units = m.ctl(C_VIB_UNITS);
        let tick_size = match units {
            0 => (srate / 200) * 256,
            1 | 2 => (srate * 256) / 1000,
            _ => (srate * 256) / 200,
        };
        let step = ((m.ctl(C_RESPONSE) << 2) as u32).wrapping_mul((256u32 << RESP_PREC) / srate as u32);
        let dry = m.ctl(C_DRY) as f32 / 256.0;
        let wet = m.ctl(C_WET) as f32 / 256.0;
        let feedback = m.ctl(C_FEEDBACK) as f32 / 256.0;
        let (vib_amp, vib_type, vib_speed, ctl_delay) = (m.ctl(C_VIB_AMP), m.ctl(C_VIB_TYPE), m.ctl(C_VIB_SPEED), m.ctl(C_DELAY));
        let bs = self.buf_size;
        let mut ptr = 0i32;
        loop {
            let mut nf = frames - ptr;
            let left = tick_size - self.tick_counter;
            if nf > left / 256 {
                nf = left / 256;
            }
            if left & 255 != 0 {
                nf += 1;
            }
            if nf > frames - ptr {
                nf = frames - ptr;
            }
            if nf < 0 {
                nf = 0;
            }
            let vib_ptr = match units {
                0 => (self.vibrato_ptr / 8) & 511,
                1 | 2 => (self.vibrato_ptr / 1000) & 511,
                3 => (self.vibrato_ptr / self.tick_size as u32) & 511,
                _ => (self.vibrato_ptr / ((self.tick_size * self.ticks_per_line as i32) / 8) as u32) & 511,
            };
            let mut vib_value = match vib_type {
                0 => HSIN_TAB[(vib_ptr / 2) as usize] as i32 * 2 - 256,
                1 => {
                    if vib_ptr < 256 {
                        HSIN_TAB[vib_ptr as usize] as i32
                    } else {
                        -(HSIN_TAB[(vib_ptr & 255) as usize] as i32)
                    }
                }
                _ => 0,
            };
            vib_value *= vib_amp;
            vib_value /= 256;
            let mut cur_delay = ctl_delay;
            if cur_delay - vib_amp < 8 {
                cur_delay = 8 + vib_amp;
            }
            if cur_delay + vib_amp > 1000 {
                cur_delay = 1000 - vib_amp;
            }
            cur_delay += vib_value;
            let mut buf_ptr = 0;
            let mut floating_delay = 0;
            for ch in 0..OUTPUTS {
                let (chin, chout) = (&m.chin, &mut m.chout);
                let inp = &chin[ch][offset..];
                let out = &mut chout[ch][offset..];
                let cbuf = &mut self.buf[ch];
                buf_ptr = self.buf_ptr;
                floating_delay = self.floating_delay;
                for i in ptr as usize..(ptr + nf) as usize {
                    let sub_fp = (floating_delay >> (RESP_PREC - 5)) * bs;
                    let sub = sub_fp / 32768;
                    let mut p2 = buf_ptr - sub;
                    if p2 < 0 {
                        p2 += bs;
                    }
                    let mut p3 = buf_ptr - (sub + 1);
                    if p3 < 0 {
                        p3 += bs;
                    }
                    let c = sub_fp & 32767;
                    let buf_val = (cbuf[p2 as usize] * (32768 - c) as f32 + cbuf[p3 as usize] * c as f32) / 32768.0;
                    out[i] = buf_val * wet;
                    if input_signal {
                        out[i] += inp[i] * dry;
                        cbuf[buf_ptr as usize] = inp[i] + buf_val * feedback;
                    } else {
                        cbuf[buf_ptr as usize] = buf_val * feedback;
                    }
                    buf_ptr += 1;
                    if buf_ptr >= bs {
                        buf_ptr = 0;
                    }
                    if (floating_delay >> RESP_PREC) > cur_delay {
                        floating_delay = floating_delay.wrapping_sub(step as i32);
                        if (floating_delay >> RESP_PREC) < cur_delay {
                            floating_delay = cur_delay << RESP_PREC;
                        }
                    } else if (floating_delay >> RESP_PREC) < cur_delay {
                        floating_delay = floating_delay.wrapping_add(step as i32);
                        if (floating_delay >> RESP_PREC) > cur_delay {
                            floating_delay = cur_delay << RESP_PREC;
                        }
                    }
                }
            }
            self.buf_ptr = buf_ptr;
            self.floating_delay = floating_delay;
            ptr += nf;
            self.tick_counter += nf * 256;
            if self.tick_counter >= tick_size {
                match units {
                    0 => {
                        self.vibrato_ptr = self.vibrato_ptr.wrapping_add(vib_speed as u32);
                        self.vibrato_ptr &= 4095;
                    }
                    1 => {
                        if vib_speed > 0 {
                            self.vibrato_ptr = self.vibrato_ptr.wrapping_add(((1000 * 512) / vib_speed) as u32);
                            self.vibrato_ptr %= 1000 * 512;
                        }
                    }
                    2 => {
                        self.vibrato_ptr = self.vibrato_ptr.wrapping_add((vib_speed * 512) as u32);
                        self.vibrato_ptr %= 1000 * 512;
                    }
                    3 => {
                        if vib_speed != 0 {
                            self.vibrato_ptr = self.vibrato_ptr.wrapping_add(((tick_size * 512) / vib_speed) as u32);
                            self.vibrato_ptr %= (self.tick_size * 512) as u32;
                        }
                    }
                    _ => {
                        if vib_speed != 0 {
                            self.vibrato_ptr = self.vibrato_ptr.wrapping_add(((((tick_size / 8) * 512) / vib_speed) * (units - 3)) as u32);
                            self.vibrato_ptr %= (((self.tick_size * self.ticks_per_line as i32) / 8) * 512) as u32;
                        }
                    }
                }
                self.tick_counter %= tick_size;
            }
            if ptr >= frames {
                break;
            }
        }
        1
    }
}

impl Module for Flanger {
    fn handle(&mut self, n: usize, e: &mut Event, net: &mut Net) -> isize {
        let mut retval = 0;
        match e.cmd {
            Cmd::Init => {
                net.register_ctl(n, 0, 256, 256, 0, C_DRY, -1, 0);
                net.register_ctl(n, 0, 256, 128, 0, C_WET, -1, 0);
                net.register_ctl(n, 0, 256, 128, 0, C_FEEDBACK, -1, 1);
                net.register_ctl(n, 8, 1000, 200, 0, C_DELAY, -1, 1);
                net.register_ctl(n, 0, 256, 2, 0, C_RESPONSE, -1, 1);
                net.register_ctl(n, 0, 512, 8, 0, C_VIB_SPEED, -1, 2);
                net.register_ctl(n, 0, 256, 32, 0, C_VIB_AMP, -1, 2);
                net.register_ctl(n, 0, 1, 0, 1, C_VIB_TYPE, -1, 2);
                net.register_ctl(n, 0, 256, 0, 0, C_SET_VIB_PHASE, -1, 2);
                net.register_ctl(n, 0, 6, 0, 1, C_VIB_UNITS, -1, 2);
                self.buf_size = net.sampling_freq / 64;
                for b in self.buf.iter_mut() {
                    *b = vec![0.0; self.buf_size as usize];
                }
                self.buf_ptr = 0;
                self.buf_empty = true;
                self.floating_delay = net.mods[n].ctl(C_DELAY) << RESP_PREC;
                self.tick_counter = 0;
                self.vibrato_ptr = 0;
                self.empty = true;
                retval = 1;
            }
            Cmd::SetupFinished => {
                self.set_phase(n, net, true);
                self.floating_delay = net.mods[n].ctl(C_DELAY) << RESP_PREC;
                retval = 1;
            }
            Cmd::ApplyControllers => {
                self.floating_delay = net.mods[n].ctl(C_DELAY) << RESP_PREC;
                retval = 1;
            }
            Cmd::Clean => {
                if !self.buf_empty {
                    for b in self.buf.iter_mut() {
                        b.fill(0.0);
                    }
                    self.buf_empty = true;
                }
                self.buf_ptr = 0;
                self.tick_counter = 0;
                self.vibrato_ptr = 0;
                self.empty = true;
                self.set_phase(n, net, true);
                retval = 1;
            }
            Cmd::RenderReplace => retval = self.render(n, net),
            Cmd::SetLocalController | Cmd::SetGlobalController => {
                if e.ctl_num() == 8 {
                    let v = e.ctl_val() as u32;
                    self.vibrato_ptr = match net.mods[n].ctl(C_VIB_UNITS) {
                        0 => (v / 8) & 4095,
                        1 | 2 => ((v / 32) * (1000 * 512)) / 1024,
                        3 => (((v / 32) as i32 * self.tick_size) / 1024 * 512) as u32,
                        _ => ((((v / 32) as i32 * ((self.tick_size * self.ticks_per_line as i32) / 8)) / 1024) * 512) as u32,
                    };
                }
            }
            Cmd::NoteOn => {
                self.set_phase(n, net, false);
                retval = 1;
            }
            Cmd::SpeedChanged => {
                self.tick_size = e.tick_size();
                self.ticks_per_line = e.tpl();
                retval = 1;
            }
            Cmd::Close => retval = 1,
            _ => {}
        }
        retval
    }
}
