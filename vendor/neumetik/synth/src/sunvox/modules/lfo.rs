// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! LFO (psynths_lfo.cpp): the input's amplitude (or, with its channels
//! a half period apart, its panning) moved by a sine, square, saw,
//! random or triangle; or, as a generator, the wave itself.

use super::super::net::{self, Cmd, Event, Module, Net};

const MAX_DUTY_CYCLE: i32 = 256;
const FREQ_UNIT_MAX: i32 = 8;

const C_VOLUME: usize = 0;
const C_TYPE: usize = 1;
const C_AMP: usize = 2;
const C_FREQ: usize = 3;
const C_SHAPE: usize = 4;
const C_SET_PHASE: usize = 5;
const C_MONO: usize = 6;
const C_FREQ_UNITS: usize = 7;
const C_DUTY_CYCLE: usize = 8;
const C_GEN: usize = 9;
const C_FREQ_SCALE: usize = 10;
const C_SMOOTH: usize = 11;
const C_SIN_QUALITY: usize = 12;
const C_TRANSPOSE: usize = 13;
const C_FINETUNE: usize = 14;

#[derive(Default)]
pub(crate) struct Lfo {
    sin_tab: Vec<u16>,
    ptr: u64,
    delta: u64,
    recalc_delta: bool,
    rand: u32,
    rand_prev: u32,
    rand_prev_phase: u32,
    rand_seed: u32,
    tick_size: i32,
    ticks_per_line: u8,
}

impl Lfo {
    fn change_ctl_limits(&self, n: usize, net: &mut Net) {
        if net.base_host_version < 0x01090202 {
            return;
        }
        let (min, max) = match net.mods[n].ctl(C_FREQ_UNITS) {
            1 => (1, 4000),
            2 => (1, 8192 * 2),
            7 => (0, 256),
            8 => (0, 256 * 100),
            _ => (1, 256),
        };
        if net.mods[n].ctls.get(3).is_some_and(|c| c.max != max || c.min != min) {
            net.change_ctl_limits(n, 3, min, max, max);
        }
    }

    fn set_phase(&mut self, n: usize, net: &mut Net, check_version: bool) {
        if !check_version || net.base_host_version >= 0x01090300 {
            let mut e = Event::new(Cmd::SetGlobalController);
            e.set_ctl_num(5);
            e.set_ctl_val((net.mods[n].ctl(C_SET_PHASE) << 7) as u16);
            self.handle(n, &mut e, net);
        }
    }

    fn recalc(&mut self, n: usize, net: &Net) {
        let m = &net.mods[n];
        let srate = net.sampling_freq;
        let freq = m.ctl(C_FREQ);
        let units = m.ctl(C_FREQ_UNITS);
        let note_delta = |div: f32| -> u64 {
            let note = freq as f32;
            let mut f = 2.0f32.powf(note / div) * 522.6875 / 32.0;
            if f > (srate / 2) as f32 {
                1 << 41
            } else {
                f *= (1u64 << 42) as f32;
                (f as u64) / srate as u64
            }
        };
        self.delta = match units {
            0 => ((freq as u64) << 36) / srate as u64,
            1 => ((64u64 << 36) * 1000) / (freq.wrapping_mul(srate) as i64 as u64),
            2 => (((freq as u64) << 36) * 64) / srate as u64,
            3 => ((64u64 << 36) * 256) / ((freq as u32).wrapping_mul(self.tick_size as u32) as u64),
            4..=6 => ((64u64 << 36) * 256) / (freq as u64 * self.tick_size as i64 as u64 * self.ticks_per_line as u64 / (units - 3) as u64),
            7 => note_delta(12.0),
            8 => note_delta(100.0 * 12.0),
            _ => self.delta,
        };
        let scale = m.ctl(C_FREQ_SCALE);
        if scale != 100 {
            self.delta = self.delta.wrapping_mul(scale as i64 as u64) / 100;
        }
        let transpose = m.ctl(C_TRANSPOSE) - 128;
        let finetune = m.ctl(C_FINETUNE) - 256;
        if transpose != 0 || finetune != 0 {
            let nn = transpose as f64 + finetune as f64 / 256.0;
            let s = 2.0f64.powf(nn / 12.0);
            self.delta = (self.delta as f64 * s) as u64;
        }
        if self.delta < 1 {
            self.delta = 1;
        }
        let shape = m.ctl(C_SHAPE);
        if !(shape == 5 || shape == 7) && self.delta > (256u64 << 33) {
            self.delta = 256u64 << 33;
        }
    }

    fn render(&mut self, n: usize, net: &mut Net) -> isize {
        let want = if net.mods[n].ctl(C_MONO) != 0 { 1 } else { 2 };
        if net.mods[n].output_channels != want {
            net.set_number_of_outputs(want, n);
            net.set_number_of_inputs(want, n);
        }
        if self.recalc_delta {
            self.recalc_delta = false;
            self.recalc(n, net);
        }
        let base = net.base_host_version;
        let m = &mut net.mods[n];
        let (offset, frames) = (m.offset as usize, m.frames as usize);
        let outputs_num = m.output_channels.max(0) as usize;
        let c = |i| m.ctl(i);
        let (ctl_volume, ctl_type, ctl_amp, ctl_shape, ctl_duty, ctl_gen, ctl_smooth, ctl_sinq) = (c(C_VOLUME), c(C_TYPE), c(C_AMP), c(C_SHAPE), c(C_DUTY_CYCLE), c(C_GEN), c(C_SMOOTH), c(C_SIN_QUALITY));
        let mut no_input = (0..outputs_num).all(|ch| m.in_empty[ch] >= (offset + frames) as i32);
        if ctl_gen != 0 {
            no_input = false;
        }
        if ctl_volume == 0 {
            no_input = true;
        }
        if no_input {
            self.ptr = self.ptr.wrapping_add(self.delta.wrapping_mul(frames as u64));
            return 0;
        }
        let delta = self.delta;
        let mut ptr = 0u64;
        let (mut rand, mut rand_prev, mut rand_prev_phase, mut rand_seed) = (0u32, 0u32, 0u32, 0u32);
        let sin_coef = 0.5f32 * ctl_amp as f32 / 256.0;
        let sin_quality = if ctl_sinq == 0 { 3 } else { ctl_sinq };
        for ch in 0..outputs_num {
            let (chin, chout) = (&m.chin, &mut m.chout);
            let inp = &chin[ch][offset..offset + frames];
            let out = &mut chout[ch][offset..offset + frames];
            let mut off: u32 = 0;
            let mut apply_out_volume = false;
            let mut apply_lfo_amp = false;
            ptr = self.ptr;
            if ctl_shape == 5 || ctl_shape == 7 {
                rand = self.rand;
                rand_prev = self.rand_prev;
                rand_prev_phase = self.rand_prev_phase;
                rand_seed = self.rand_seed;
            }
            if ctl_type == 1 && ch == 1 {
                off = 128;
            }
            match ctl_shape {
                0 | 2 => {
                    if ctl_shape == 2 {
                        off *= 2;
                    }
                    let mut add = ((off as u64) << 33).wrapping_sub(1u64 << 40);
                    match sin_quality {
                        1 => {
                            add = add.wrapping_add(1u64 << 41);
                            ptr = ptr.wrapping_add(add);
                            for o in out.iter_mut() {
                                let mut amp = -(self.sin_tab[((ptr >> 33) & 511) as usize] as i32);
                                amp *= ctl_amp;
                                amp += 65536 * 256;
                                amp /= 256;
                                let v = 1.0f32 * amp as f32;
                                *o = v / 65536.0;
                                ptr = ptr.wrapping_add(delta);
                            }
                            ptr = ptr.wrapping_sub(add);
                        }
                        2 => {
                            add = add.wrapping_add(1u64 << 41);
                            ptr = ptr.wrapping_add(add);
                            for o in out.iter_mut() {
                                let a1 = -(self.sin_tab[((ptr >> 33) & 511) as usize] as i32);
                                let a2 = -(self.sin_tab[(((ptr >> 33) + 1) & 511) as usize] as i32);
                                let cc = ((ptr >> (33 - 15)) & 32767) as i32;
                                let mut amp = a1.wrapping_mul(32768 - cc).wrapping_add(a2.wrapping_mul(cc));
                                amp /= 32768;
                                amp *= ctl_amp;
                                amp += 65536 * 256;
                                amp /= 256;
                                let v = 1.0f32 * amp as f32;
                                *o = v / 65536.0;
                                ptr = ptr.wrapping_add(delta);
                            }
                            ptr = ptr.wrapping_sub(add);
                        }
                        3 => {
                            ptr = ptr.wrapping_add(add);
                            let two_pi = (std::f64::consts::PI * 2.0) as f32;
                            for o in out.iter_mut() {
                                let x = ((ptr >> 16) & ((1 << 26) - 1)) as f32 / (1 << 26) as f32;
                                let amp = (x * two_pi).sin() - 1.0;
                                *o = (amp * sin_coef + 1.0) * 1.0;
                                ptr = ptr.wrapping_add(delta);
                            }
                            ptr = ptr.wrapping_sub(add);
                        }
                        _ => {}
                    }
                    apply_out_volume = true;
                }
                1 => {
                    let mut v1 = ctl_volume;
                    let mut v2 = ((256 - ctl_amp) * ctl_volume) / 256;
                    let mut len = ctl_duty as u32;
                    if off != 0 && base >= 0x02010100 {
                        std::mem::swap(&mut v1, &mut v2);
                        off = 0;
                    }
                    if len < 1 {
                        len = 1;
                    }
                    if len > (MAX_DUTY_CYCLE - 2) as u32 {
                        len = (MAX_DUTY_CYCLE - 2) as u32;
                    }
                    for o in out.iter_mut() {
                        let p = (((ptr >> 34) as u32).wrapping_add(off)) & 255;
                        let amp: i32 = if ctl_smooth == 0 {
                            if p >= len {
                                v1 << 6
                            } else {
                                v2 << 6
                            }
                        } else if p >= len {
                            if p >= len + 1 {
                                v1 << 6
                            } else {
                                let cc = (((ptr >> 19) as u32).wrapping_add(off << 15)) & 32767;
                                ((v1 as u32).wrapping_mul(cc).wrapping_add((v2 as u32).wrapping_mul(32768 - cc)) >> 9) as i32
                            }
                        } else if p >= 1 {
                            v2 << 6
                        } else {
                            let cc = (((ptr >> 19) as u32).wrapping_add(off << 15)) & 32767;
                            ((v2 as u32).wrapping_mul(cc).wrapping_add((v1 as u32).wrapping_mul(32768 - cc)) >> 9) as i32
                        };
                        *o = (1.0f32 * amp as f32) / (256 << 6) as f32;
                        ptr = ptr.wrapping_add(delta);
                    }
                }
                3 | 4 => {
                    let mut shape = ctl_shape;
                    if off != 0 && base >= 0x02010100 {
                        shape = if shape == 3 { 4 } else { 3 };
                        off = 0;
                    }
                    let off = (off << 18) as u64;
                    let mask: i64 = (1 << 26) - 1;
                    for o in out.iter_mut() {
                        let p = ((ptr >> 16).wrapping_add(off) & mask as u64) as i32;
                        let mut amp: i32 = if shape == 3 { -((mask as i32) - p) } else { -p };
                        if ctl_smooth != 0 && p < (128 << 11) {
                            amp = if shape == 3 {
                                ((amp as i64 * p as i64) / (128 << 11)) as i32
                            } else {
                                ((amp as i64 * p as i64 + (-mask * ((128 << 11) - p) as i64)) / (128 << 11)) as i32
                            };
                        }
                        *o = amp as f32 * ctl_amp as f32 / 256.0 / (1 << 26) as f32 + 1.0;
                        ptr = ptr.wrapping_add(delta);
                    }
                    apply_out_volume = true;
                }
                5 => {
                    for o in out.iter_mut() {
                        let p1 = ((ptr >> 27) & 32767) as u32;
                        let p2 = ((ptr >> 27) >> 15) as u32;
                        if rand_prev_phase != p2 {
                            rand_prev_phase = p2;
                            rand_prev = rand;
                            rand = net::pseudo_random(&mut rand_seed);
                        }
                        let mut amp: i32 = if ctl_smooth != 0 && p1 < 128 { ((rand.wrapping_mul(p1)).wrapping_add(rand_prev.wrapping_mul(127u32.wrapping_sub(p1))) >> 7) as i32 } else { rand as i32 };
                        if off != 0 {
                            amp = 32767 - amp;
                        }
                        let mut v = 1.0f32;
                        v *= amp as f32;
                        v /= 32768.0;
                        *o = v;
                        ptr = ptr.wrapping_add(delta);
                    }
                    apply_out_volume = true;
                    apply_lfo_amp = true;
                }
                7 => {
                    for o in out.iter_mut() {
                        let p2 = (ptr >> 42) as u32;
                        if rand_prev_phase != p2 {
                            rand_prev_phase = p2;
                            rand_prev = rand;
                            rand = net::pseudo_random(&mut rand_seed);
                        }
                        let p1 = ((ptr >> 16) & ((1 << 26) - 1)) as u32;
                        let mut amp = ((rand as i64 * p1 as i64 + rand_prev as i64 * ((1i64 << 26) - p1 as i64)) >> 15) as i32;
                        if off != 0 {
                            amp = (32767 << 11) - amp;
                        }
                        *o = amp as f32 / (1 << 26) as f32;
                        ptr = ptr.wrapping_add(delta);
                    }
                    apply_out_volume = true;
                    apply_lfo_amp = true;
                }
                6 => {
                    let off = (off << 19) as u64;
                    for o in out.iter_mut() {
                        let mut v = ((ptr >> 15).wrapping_add(off) & ((1 << 27) - 1)) as i32;
                        if v > (1 << 26) {
                            v = (1 << 27) - v;
                        }
                        let v2 = -v as f32 / (1 << 26) as f32;
                        *o = v2 * ctl_amp as f32 / 256.0 + 1.0;
                        ptr = ptr.wrapping_add(delta);
                    }
                    apply_out_volume = true;
                }
                _ => {}
            }
            if apply_lfo_amp && ctl_amp != 256 {
                for o in out.iter_mut() {
                    let mut v = *o * ctl_amp as f32;
                    v += 1.0 * (256 - ctl_amp) as f32;
                    v /= 256.0;
                    *o = v;
                }
            }
            if ctl_gen == 0 {
                for i in 0..frames {
                    out[i] *= inp[i];
                }
            }
            if apply_out_volume && ctl_volume != 256 {
                for o in out.iter_mut() {
                    let v = *o * ctl_volume as f32;
                    *o = v / 256.0;
                }
            }
        }
        self.ptr = ptr;
        if ctl_shape == 5 || ctl_shape == 7 {
            self.rand = rand;
            self.rand_prev = rand_prev;
            self.rand_prev_phase = rand_prev_phase;
            self.rand_seed = rand_seed;
        }
        1
    }
}

impl Module for Lfo {
    fn handle(&mut self, n: usize, e: &mut Event, net: &mut Net) -> isize {
        let mut retval = 0;
        match e.cmd {
            Cmd::Init => {
                net.register_ctl(n, 0, 512, 256, 0, C_VOLUME, 256, 0);
                net.register_ctl(n, 0, 1, 0, 1, C_TYPE, -1, 0);
                net.register_ctl(n, 0, 256, 256, 0, C_AMP, -1, 1);
                net.register_ctl(n, 1, 2048, 256, 0, C_FREQ, -1, 1);
                net.register_ctl(n, 0, 7, 2, 1, C_SHAPE, -1, 1);
                net.register_ctl(n, 0, 256, 0, 0, C_SET_PHASE, -1, 1);
                net.register_ctl(n, 0, 1, 0, 1, C_MONO, -1, 0);
                net.register_ctl(n, 0, FREQ_UNIT_MAX, 0, 1, C_FREQ_UNITS, -1, 1);
                net.register_ctl(n, 0, MAX_DUTY_CYCLE, MAX_DUTY_CYCLE / 2, 0, C_DUTY_CYCLE, MAX_DUTY_CYCLE / 2, 1);
                net.register_ctl(n, 0, 1, 0, 1, C_GEN, -1, 0);
                net.register_ctl(n, 0, 200, 100, 0, C_FREQ_SCALE, 100, 1);
                net.register_ctl(n, 0, 1, 1, 1, C_SMOOTH, -1, 2);
                net.register_ctl(n, 0, 3, 0, 1, C_SIN_QUALITY, -1, 1);
                net.register_ctl(n, 0, 256, 128, 1, C_TRANSPOSE, 128, 1);
                net.set_ctl_show_offset(n, 13, -128);
                net.register_ctl(n, 0, 512, 256, 0, C_FINETUNE, 256, 1);
                net.set_ctl_show_offset(n, 14, -256);
                self.ptr = 0;
                self.rand = 0;
                self.rand_prev = 0;
                self.rand_prev_phase = 0;
                // stime_ns() + pseudo_random() + mod_num * 49157 + mod->id * 3079, the clock at 0.
                self.rand_seed = net.globals.pseudo_random().wrapping_add((n as u32).wrapping_mul(49157)).wrapping_add(net.mods[n].id.wrapping_mul(3079));
                self.sin_tab = net::sine_table(false, 9, 65535).into_iter().map(|v| v as u16).collect();
                self.recalc_delta = true;
                retval = 1;
            }
            Cmd::SetupFinished => {
                self.change_ctl_limits(n, net);
                self.set_phase(n, net, true);
                retval = 1;
            }
            Cmd::Clean => {
                self.ptr = 0;
                self.set_phase(n, net, true);
                retval = 1;
            }
            Cmd::RenderReplace => retval = self.render(n, net),
            Cmd::SetLocalController | Cmd::SetGlobalController => match e.ctl_num() {
                3 | 4 | 10 | 13 | 14 => self.recalc_delta = true,
                5 => self.ptr = ((e.ctl_val() >> 6) as u64) << 33,
                7 => {
                    let v = (e.ctl_val() as i32).min(FREQ_UNIT_MAX);
                    if let Some(var) = net.mods[n].ctls.get(C_FREQ_UNITS).and_then(|c| c.var) {
                        net.mods[n].vars[var] = v;
                    }
                    self.change_ctl_limits(n, net);
                    self.recalc_delta = true;
                    retval = 1;
                }
                _ => {}
            },
            Cmd::NoteOn => {
                self.set_phase(n, net, false);
                retval = 1;
            }
            Cmd::SpeedChanged => {
                self.tick_size = e.tick_size();
                self.ticks_per_line = e.tpl();
                self.recalc_delta = true;
                retval = 1;
            }
            Cmd::Close => retval = 1,
            _ => {}
        }
        retval
    }
}
