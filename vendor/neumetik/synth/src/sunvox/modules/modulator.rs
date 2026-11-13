// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! Modulator (psynths_modulator.cpp): its first input (the carrier)
//! combined with the others: multiplied (amplitude), added, subtracted,
//! min, max, AND, XOR, by magnitude; or delayed by them (phase, signed or
//! absolute) through a ring buffer read without, with linear or with
//! spline interpolation.

use super::super::dsp::catmull_rom_f32;
use super::super::net::{Cmd, Event, Module, Net, FLAG_EXISTS, FLAG_OUTPUT_IS_EMPTY};
use super::generator::to_int16;

const OUTPUTS: usize = 2;
const CHANGED_MAX_DELAY: u32 = 1 << 0;

const C_VOLUME: usize = 0;
const C_TYPE: usize = 1;
const C_MONO: usize = 2;
const C_MAX_DELAY: usize = 3;
const C_INTERP: usize = 4;

#[derive(Default)]
pub(crate) struct Modulator {
    max_delay: i32,
    buf_size: i32,
    buf: [Vec<f32>; OUTPUTS],
    buf_clean: bool,
    buf_ptr: u32,
    empty_frames_counter: i32,
    changed: u32,
}

impl Modulator {
    fn handle_changes(&mut self, n: usize, net: &Net) {
        if self.changed & CHANGED_MAX_DELAY != 0 {
            let sf = net.sampling_freq;
            self.max_delay = match net.mods[n].ctl(C_MAX_DELAY) {
                1 => (sf as f64 * 0.08) as i32,
                2 => (sf as f64 * 0.2) as i32,
                3 => (sf as f64 * 0.5) as i32,
                4 => sf,
                5 => sf * 2,
                6 => sf * 4,
                7 => sf * 8,
                8 => sf * 16,
                9 => sf * 32,
                _ => sf / 25,
            };
            let prev = self.buf_size;
            let new_size = ((self.max_delay + 4) as u32).next_power_of_two() as i32;
            self.max_delay -= 1;
            if new_size > prev {
                self.buf_size = new_size;
                for b in self.buf.iter_mut() {
                    *b = vec![0.0; new_size as usize];
                }
                self.buf_clean = true;
                self.buf_ptr = 0;
                self.empty_frames_counter = self.buf_size;
            }
        }
        self.changed = 0;
    }

    fn render(&mut self, n: usize, net: &mut Net) -> isize {
        if self.changed != 0 {
            self.handle_changes(n, net);
        }
        let want = if net.mods[n].ctl(C_MONO) != 0 { 1 } else { OUTPUTS as i32 };
        if net.mods[n].output_channels != want {
            net.set_number_of_outputs(want, n);
            net.set_number_of_inputs(want, n);
        }
        let base = net.base_host_version;
        let m = &net.mods[n];
        let (offset, frames) = (m.offset as usize, m.frames as usize);
        let outputs_num = m.output_channels.clamp(0, 2) as usize;
        let mut no_input_signal = !(0..outputs_num).any(|ch| m.in_empty[ch] < (offset + frames) as i32);
        let (ctl_volume, ctl_type, ctl_interp) = (m.ctl(C_VOLUME), m.ctl(C_TYPE), m.ctl(C_INTERP));
        let phase_modulation = ctl_type == 1 || ctl_type == 2;
        if !phase_modulation {
            if ctl_volume == 0 {
                no_input_signal = true;
            }
            if no_input_signal {
                return 0;
            }
        } else if no_input_signal {
            if self.empty_frames_counter >= self.buf_size {
                self.buf_ptr = self.buf_ptr.wrapping_add(frames as u32);
                return 0;
            }
            self.empty_frames_counter += frames as i32;
        } else {
            self.empty_frames_counter = 0;
        }
        self.buf_clean = false;
        // The inputs' outputs, a copy a link.
        let mut ins: Vec<Vec<Vec<f32>>> = Vec::new();
        for &l in &m.input_links {
            let Some(im) = usize::try_from(l).ok().filter(|&l| l < net.mods_num()).map(|l| &net.mods[l]).filter(|im| im.flags & FLAG_EXISTS != 0) else { continue };
            let mcc = im.output_channels.max(0) as usize;
            if mcc == 0 || im.chout[0].is_empty() {
                continue;
            }
            if !phase_modulation && im.flags & FLAG_OUTPUT_IS_EMPTY != 0 {
                continue;
            }
            let mut chans = Vec::new();
            let mut last: Option<usize> = None;
            for ch in 0..outputs_num {
                if ch < mcc && !im.chout[ch].is_empty() {
                    last = Some(ch);
                }
                chans.push(last.map_or_else(|| vec![0.0; frames], |c| im.chout[c][offset..offset + frames].to_vec()));
            }
            ins.push(chans);
        }
        let mut retval = 0i32;
        let m = &mut net.mods[n];
        let mut pm_carrier: Vec<Vec<f32>> = Vec::new();
        for chans in ins {
            if !phase_modulation {
                for ch in 0..outputs_num {
                    let inp = &chans[ch];
                    let out = &mut m.chout[ch][offset..offset + frames];
                    if retval == 0 {
                        out.copy_from_slice(inp);
                        continue;
                    }
                    for (o, &x) in out.iter_mut().zip(inp.iter()) {
                        match ctl_type {
                            0 => *o *= x,
                            3 => *o += x,
                            4 => *o -= x,
                            5 => {
                                if x < *o {
                                    *o = x
                                }
                            }
                            6 => {
                                if x > *o {
                                    *o = x
                                }
                            }
                            7 => *o = (to_int16(*o) as i32 & to_int16(x) as i32) as f32 / 32768.0,
                            8 => *o = (to_int16(*o) as i32 ^ to_int16(x) as i32) as f32 / 32768.0,
                            9 => {
                                if x.abs() < o.abs() {
                                    *o = x
                                }
                            }
                            10 => {
                                if x.abs() > o.abs() {
                                    *o = x
                                }
                            }
                            _ => {}
                        }
                    }
                }
                retval = 1;
            } else {
                if retval == 0 {
                    pm_carrier = chans;
                } else {
                    for ch in 0..outputs_num {
                        let out = &mut m.chout[ch][offset..offset + frames];
                        if retval == 1 {
                            out.copy_from_slice(&chans[ch]);
                        } else {
                            for (o, &x) in out.iter_mut().zip(chans[ch].iter()) {
                                *o += x;
                            }
                        }
                    }
                }
                retval += 1;
            }
        }
        if retval != 0 {
            if phase_modulation {
                if retval == 1 {
                    for ch in 0..outputs_num {
                        m.chout[ch][offset..offset + frames].copy_from_slice(&pm_carrier[ch]);
                    }
                } else {
                    let mut buf_ptr = self.buf_ptr;
                    let mask = (self.buf_size - 1) as u32;
                    let (interp_prec, interp_max) = if base < 0x02010100 { (15u32, 32768u32) } else { (24, 16777216) };
                    let max_delay = self.max_delay as i64;
                    for ch in 0..outputs_num {
                        let inp = &pm_carrier[ch];
                        let out = &mut m.chout[ch][offset..offset + frames];
                        let cbuf = &mut self.buf[ch];
                        buf_ptr = self.buf_ptr;
                        let signed = ctl_type == 1;
                        for p in 0..frames {
                            cbuf[(buf_ptr & mask) as usize] = inp[p];
                            let mut vf = out[p];
                            if signed {
                                vf = (vf + 1.0) * 0.5;
                                if vf < 0.0 {
                                    vf = 0.0;
                                }
                            } else if vf < 0.0 {
                                vf = -vf;
                            }
                            if vf > 1.0 {
                                vf = 1.0;
                            }
                            let mut v = (vf * interp_max as f32) as i64;
                            v *= max_delay;
                            let interp = (v & (interp_max as i64 - 1)) as f32 / interp_max as f32;
                            v >>= interp_prec;
                            let v = v as i32;
                            let at = |d: i32| cbuf[(buf_ptr.wrapping_sub(d as u32) & mask) as usize];
                            out[p] = match ctl_interp {
                                0 => at(v),
                                1 => {
                                    let (v1, v2) = (at(v), at(v + 1));
                                    v1 * (1.0 - interp) + v2 * interp
                                }
                                2 => catmull_rom_f32(at(v - 1), at(v), at(v + 1), at(v + 2), interp),
                                _ => out[p],
                            };
                            buf_ptr = buf_ptr.wrapping_add(1);
                        }
                    }
                    self.buf_ptr = buf_ptr;
                }
                retval = 1;
            }
            if ctl_volume != 256 {
                let v = ctl_volume as f32 / 256.0;
                for ch in 0..outputs_num {
                    for o in m.chout[ch][offset..offset + frames].iter_mut() {
                        *o *= v;
                    }
                }
            }
        }
        if retval != 0 {
            retval = 1;
        }
        if ctl_volume == 0 {
            retval = 2;
        }
        retval as isize
    }
}

impl Module for Modulator {
    fn handle(&mut self, n: usize, e: &mut Event, net: &mut Net) -> isize {
        let mut retval: isize = 0;
        match e.cmd {
            Cmd::Init => {
                net.register_ctl(n, 0, 512, 256, 0, C_VOLUME, 256, 0);
                net.register_ctl(n, 0, 10, 0, 1, C_TYPE, -1, 0);
                net.register_ctl(n, 0, 1, 0, 1, C_MONO, -1, 0);
                net.register_ctl(n, 0, 9, 0, 1, C_MAX_DELAY, -1, 0);
                net.register_ctl(n, 0, 2, 1, 1, C_INTERP, -1, 0);
                self.changed = 0xFFFF_FFFF;
                retval = 1;
            }
            Cmd::SetupFinished => {
                self.handle_changes(n, net);
                retval = 1;
            }
            Cmd::Clean => {
                self.empty_frames_counter = self.buf_size;
                if net.mods[n].ctl(C_TYPE) >= 1 && !self.buf_clean {
                    for b in self.buf.iter_mut() {
                        b.fill(0.0);
                    }
                    self.buf_clean = true;
                }
                retval = 1;
            }
            Cmd::RenderReplace => retval = self.render(n, net),
            Cmd::SetLocalController | Cmd::SetGlobalController => {
                if e.ctl_num() == 3 {
                    self.changed |= CHANGED_MAX_DELAY;
                }
            }
            Cmd::Close => retval = 1,
            _ => {}
        }
        retval
    }
}
