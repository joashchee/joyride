// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! Sound2Ctl (psynths_sound2ctl.cpp): its input's level (sampled, or
//! averaged, at a rate of its own), scaled and smoothed, sent as a
//! controller value to the first module it links to.

use super::super::net::{Cmd, Event, Module, Net, FLAG_EXISTS, RT_MUTE};

const UNKNOWN_CTL_VAL: i32 = 0x10000;

const C_FREQ: usize = 0;
const C_STEREO: usize = 1;
const C_ABS: usize = 2;
const C_GAIN: usize = 3;
const C_SMOOTH: usize = 4;
const C_ALG: usize = 5;
const C_MIN: usize = 6;
const C_MAX: usize = 7;
const C_NUM: usize = 8;

#[derive(Default)]
pub(crate) struct Sound2Ctl {
    tick_counter: i32,
    accum: f32,
    accum_cnt: i32,
    accum_empty: bool,
    prev_ctl_val: i32,
    empty_frames_counter: i32,
    empty_frames_counter_max: i32,
    optimize: bool,
}

impl Sound2Ctl {
    fn render(&mut self, n: usize, net: &mut Net) -> isize {
        let m = &net.mods[n];
        let ctl_num = m.ctl(C_NUM);
        if ctl_num == 0 || m.realtime_flags & RT_MUTE != 0 {
            return 0;
        }
        let (offset, frames) = (m.offset as usize, m.frames as usize);
        let mut input_signal = m.in_empty[0] < (offset + frames) as i32;
        let stereo = m.ctl(C_STEREO) != 0;
        if !stereo {
            if m.input_channels != 1 {
                net.set_number_of_inputs(1, n);
            }
        } else {
            if m.input_channels != 2 {
                net.set_number_of_inputs(2, n);
            }
            let m = &mut net.mods[n];
            if m.in_empty[1] < (offset + frames) as i32 {
                input_signal = true;
            }
            if input_signal {
                let (a, b) = m.chin.split_at_mut(1);
                for (x, &y) in a[0][offset..offset + frames].iter_mut().zip(b[0][offset..offset + frames].iter()) {
                    *x = (*x + y) / 2.0;
                }
            }
        }
        let srate = net.sampling_freq;
        let m = &mut net.mods[n];
        let ctl_abs = m.ctl(C_ABS);
        if input_signal {
            self.empty_frames_counter = 0;
            if ctl_abs != 0 {
                for x in m.chin[0][offset..offset + frames].iter_mut() {
                    *x = x.abs();
                }
            }
        } else if self.empty_frames_counter < self.empty_frames_counter_max {
            self.empty_frames_counter += frames as i32;
        } else if self.optimize {
            return 0;
        }
        let ctl_freq = m.ctl(C_FREQ);
        let mut tick_size = 1;
        if ctl_freq > 0 {
            tick_size = (srate * 256) / ctl_freq;
        }
        if tick_size <= 0 {
            tick_size = 1;
        }
        let (ctl_min, ctl_max, ctl_alg, ctl_gain) = (m.ctl(C_MIN), m.ctl(C_MAX), m.ctl(C_ALG), m.ctl(C_GAIN));
        let range = ctl_max - ctl_min;
        let ctl_smooth = m.ctl(C_SMOOTH).min(255);
        let ctl_smooth2 = 256 - ctl_smooth;
        let mut accum = self.accum;
        let mut accum_cnt = self.accum_cnt;
        let mut accum_empty = self.accum_empty;
        let inp = m.chin[0][offset..offset + frames].to_vec();
        let links = m.output_links.clone();
        let mut ptr = 0usize;
        loop {
            let mut buf_size = (frames - ptr) as i32;
            if buf_size > (tick_size - self.tick_counter) / 256 {
                buf_size = (tick_size - self.tick_counter) / 256;
            }
            if (tick_size - self.tick_counter) & 255 != 0 {
                buf_size += 1;
            }
            if buf_size > (frames - ptr) as i32 {
                buf_size = (frames - ptr) as i32;
            }
            if buf_size < 0 {
                buf_size = 0;
            }
            let buf_size = buf_size as usize;
            if ctl_alg == 0 {
                if accum_empty {
                    accum = inp.get(ptr).copied().unwrap_or(0.0);
                    accum_empty = false;
                }
            } else {
                for &v in &inp[ptr..ptr + buf_size] {
                    accum += v;
                }
                accum_cnt += buf_size as i32;
                accum_empty = false;
            }
            ptr += buf_size;
            self.tick_counter += buf_size as i32 * 256;
            if self.tick_counter >= tick_size {
                if ctl_alg == 1 {
                    accum /= accum_cnt as f32;
                }
                if ctl_gain != 256 {
                    accum = (accum * ctl_gain as f32) / 256.0;
                }
                let mut ctl_val = (accum * 32768.0 / 1.0) as i32;
                if ctl_val > 32768 {
                    ctl_val = 32768;
                }
                if ctl_abs == 0 {
                    if ctl_val < -32768 {
                        ctl_val = -32768;
                    }
                    ctl_val = (ctl_val + 32768) / 2;
                }
                let r = ctl_val.wrapping_mul(range);
                ctl_val = ctl_min + r / 32768;
                if self.prev_ctl_val == UNKNOWN_CTL_VAL {
                    if ctl_val == 0 {
                        self.prev_ctl_val = ctl_val;
                    }
                } else {
                    ctl_val = (self.prev_ctl_val * ctl_smooth + ctl_val * ctl_smooth2) / 256;
                }
                if !self.optimize || self.prev_ctl_val != ctl_val {
                    self.empty_frames_counter = 0;
                    self.prev_ctl_val = ctl_val;
                    for &l in &links {
                        let Some(l) = usize::try_from(l).ok().filter(|&l| l < net.mods_num() && net.mods[l].flags & FLAG_EXISTS != 0) else { continue };
                        let mut e = Event::new(Cmd::SetGlobalController);
                        e.offset = if ptr > 0 { (offset + ptr - 1) as i32 } else { offset as i32 };
                        e.set_ctl_num((ctl_num - 1) as u16);
                        e.set_ctl_val(ctl_val as u16);
                        net.add_event(l, &e);
                        break;
                    }
                }
                accum = 0.0;
                accum_cnt = 0;
                accum_empty = true;
                self.tick_counter %= tick_size;
            }
            if ptr >= frames {
                break;
            }
        }
        self.accum = accum;
        self.accum_cnt = accum_cnt;
        self.accum_empty = accum_empty;
        0
    }
}

impl Module for Sound2Ctl {
    fn handle(&mut self, n: usize, e: &mut Event, net: &mut Net) -> isize {
        let mut retval: isize = 0;
        match e.cmd {
            Cmd::Init => {
                net.register_ctl(n, 1, 32768, 50, 0, C_FREQ, 50, 0);
                net.set_ctl_flags(n, 0, 2);
                net.register_ctl(n, 0, 1, 0, 1, C_STEREO, -1, 0);
                net.register_ctl(n, 0, 1, 1, 1, C_ABS, -1, 0);
                net.register_ctl(n, 0, 1024, 256, 0, C_GAIN, 256, 0);
                net.register_ctl(n, 0, 256, 128, 0, C_SMOOTH, -1, 0);
                net.register_ctl(n, 0, 1, 1, 1, C_ALG, -1, 0);
                net.register_ctl(n, 0, 32768, 0, 0, C_MIN, -1, 1);
                net.register_ctl(n, 0, 32768, 32768, 0, C_MAX, -1, 1);
                net.register_ctl(n, 0, 255, 0, 1, C_NUM, -1, 1);
                self.tick_counter = 0;
                self.accum = 0.0;
                self.accum_cnt = 0;
                self.accum_empty = true;
                self.prev_ctl_val = UNKNOWN_CTL_VAL;
                self.empty_frames_counter_max = net.sampling_freq * 2;
                retval = 1;
            }
            Cmd::SetupFinished => {
                let created = net.chunk_autocreate(n, 0, 2, true);
                if created && net.base_host_version >= 0x01090600 {
                    net.mods[n].chunk_mut(0).unwrap().data[1] = 1;
                }
                self.optimize = net.mods[n].chunk(0).unwrap().data[1] != 0;
                retval = 1;
            }
            Cmd::Clean => {
                self.tick_counter = 0;
                self.accum = 0.0;
                self.accum_cnt = 0;
                self.accum_empty = true;
                self.prev_ctl_val = UNKNOWN_CTL_VAL;
                retval = 1;
            }
            Cmd::RenderReplace => retval = self.render(n, net),
            Cmd::SetLocalController | Cmd::SetGlobalController => {
                self.empty_frames_counter = 0;
                let v = e.ctl_val() as i32;
                match e.ctl_num() as i32 + 1 {
                    3 => {
                        let m = &mut net.mods[n];
                        if let Some(var) = m.ctls.get(C_ABS).and_then(|c| c.var) {
                            m.vars[var] = v;
                        }
                        self.accum = 0.0;
                        self.accum_cnt = 0;
                        self.accum_empty = true;
                        retval = 1;
                    }
                    127 => retval = 1,
                    126 => {
                        self.optimize = v != 0;
                        retval = 1;
                    }
                    _ => {}
                }
            }
            Cmd::Close => retval = 1,
            _ => {}
        }
        retval
    }
}
