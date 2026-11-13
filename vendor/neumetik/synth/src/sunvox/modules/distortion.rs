// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! Distortion (psynths_distortion.cpp): bit depth and sample rate cut,
//! then one of eleven shapers (clipping, folding, wrapping, overflow,
//! cubic, sine, power curves, soft clips) by its power, a volume, and
//! noise multiplied in.

use super::super::net::{self, Cmd, Event, Module, Net};

const C_VOLUME: usize = 0;
const C_TYPE: usize = 1;
const C_POWER: usize = 2;
const C_BITRATE: usize = 3;
const C_FREQ: usize = 4;
const C_NOISE: usize = 5;

#[derive(Default)]
pub(crate) struct Distortion {
    noise_seed: u32,
    cnt: u32,
    smp: [f32; 2],
    smp_clean: bool,
}

impl Distortion {
    fn render(&mut self, n: usize, net: &mut Net) -> isize {
        let base = net.base_host_version;
        let srate = net.sampling_freq;
        let m = &mut net.mods[n];
        let (frames, offset) = (m.frames as usize, m.offset as usize);
        if (0..2).all(|ch| m.in_empty[ch] >= (offset + frames) as i32) {
            return 0;
        }
        let volume = m.ctl(C_VOLUME) as f32 / 128.0;
        let ty = m.ctl(C_TYPE);
        let ctl_power = m.ctl(C_POWER);
        let power = ctl_power.min(255);
        let bitrate = m.ctl(C_BITRATE);
        let mut freq = m.ctl(C_FREQ);
        let ctl_noise = m.ctl(C_NOISE);
        if freq <= 0 {
            freq = 1;
        }
        let mut delta = 0u32;
        if freq < 44100 {
            delta = (((freq as i64) << 32) / srate as i64) as u32;
            if delta < 1 {
                delta = 1;
            }
        }
        let limit = (((256 - power) << 7) as f32) / 32768.0;
        let max_val = if base >= 0x02010002 { 32768.0f32 / 32768.0 } else { 32767.0f32 / 32768.0 };
        let coef = (max_val * 256.0) / limit;
        let mut cnt = 0u32;
        let mut vars_changed = false;
        let p = ctl_power as f32 / 256.0;
        let p2 = 1.0f32 - p;
        for ch in 0..2 {
            let (chin, chout) = (&m.chin, &mut m.chout);
            let out = &mut chout[ch][offset..offset + frames];
            out.copy_from_slice(&chin[ch][offset..offset + frames]);
            if bitrate < 16 && volume != 0.0 {
                for o in out.iter_mut() {
                    let mut r = super::generator::to_int16(*o) as i32;
                    r >>= 16 - bitrate;
                    r <<= 16 - bitrate;
                    *o = r as f32 / 32768.0;
                }
            }
            if freq < 44100 {
                vars_changed = true;
                if self.smp_clean {
                    self.smp_clean = false;
                    self.smp = [out[0]; 2];
                }
                cnt = self.cnt;
                let smp = &mut self.smp[ch];
                for o in out.iter_mut() {
                    let prev = cnt;
                    cnt = cnt.wrapping_add(delta);
                    if cnt < prev {
                        *smp = *o;
                    }
                    *o = *smp;
                }
            }
            if power != 0 && volume != 0.0 {
                match ty {
                    0 | 1 => {
                        for o in out.iter_mut() {
                            let mut v = *o;
                            if ty == 1 {
                                if v > limit {
                                    v = limit - (v - limit);
                                }
                                if -v > limit {
                                    v = -limit + (-v - limit);
                                }
                            }
                            if v > limit {
                                v = limit;
                            }
                            if -v > limit {
                                v = -limit;
                            }
                            v *= coef;
                            v /= 256.0;
                            *o = v;
                        }
                    }
                    2 => {
                        for o in out.iter_mut() {
                            let v = *o * coef / 256.0;
                            let mut v2 = v.abs();
                            if v2 > 1.0 {
                                let vv = v2 as i32;
                                if vv & 1 == 0 {
                                    v2 -= vv as f32;
                                } else {
                                    v2 = 1.0 - (v2 - vv as f32);
                                }
                            }
                            if v < 0.0 {
                                v2 = -v2;
                            }
                            *o = v2;
                        }
                    }
                    3 => {
                        for o in out.iter_mut() {
                            let v = *o * coef / 256.0;
                            let mut v2 = (v + 1.0).abs();
                            if v2 > 1.0 {
                                let vv = ((v2 as i32) as f32 / 2.0) as i32;
                                if vv & 1 == 0 {
                                    v2 -= vv as f32 * 2.0;
                                } else {
                                    v2 = 2.0 - (v2 - vv as f32 * 2.0);
                                }
                            }
                            *o = v2 - 1.0;
                        }
                    }
                    4 => {
                        for o in out.iter_mut() {
                            let v = *o * coef / 256.0;
                            let mut v2 = (v + 1.0).abs();
                            let vv = ((v2 as i32) as f32 / 2.0) as i32;
                            v2 -= vv as f32 * 2.0;
                            *o = v2 - 1.0;
                        }
                    }
                    5 => {
                        for o in out.iter_mut() {
                            let v = *o * coef / 256.0;
                            let mut vv = (v * 32768.0 + 32768.0) as i32;
                            vv &= 65535;
                            vv -= 32768;
                            *o = vv as f32 / 32768.0;
                        }
                    }
                    6 => {
                        for o in out.iter_mut() {
                            let v0 = *o;
                            let mut v = v0;
                            if v < -2.0 {
                                v = -2.0;
                            } else if v > 2.0 {
                                v = 2.0;
                            }
                            v = 1.5 * v - 0.5 * v * v * v;
                            *o = v * p + v0 * p2;
                        }
                    }
                    7 => {
                        for o in out.iter_mut() {
                            let v0 = *o;
                            let v = v0.sin();
                            *o = v * p + v0 * p2;
                        }
                    }
                    8 => {
                        for o in out.iter_mut() {
                            let v0 = *o;
                            let mut v = v0;
                            if v >= 0.0 {
                                if v > 1.0 {
                                    v = 1.0;
                                } else {
                                    v = 1.0 - v;
                                    v *= v;
                                    v *= v;
                                    v = 1.0 - v;
                                }
                            } else if v < -1.0 {
                                v = -1.0;
                            } else {
                                v += 1.0;
                                v *= v;
                                v *= v;
                                v -= 1.0;
                            }
                            *o = v * p + v0 * p2;
                        }
                    }
                    9 => {
                        for o in out.iter_mut() {
                            let v0 = *o;
                            let v = v0 * 2.0;
                            // sqrt of a float: C++'s float overload, sqrtf.
                            let v = v / (1.0f32 + v * v).sqrt();
                            *o = v * p + v0 * p2;
                        }
                    }
                    10 => {
                        for o in out.iter_mut() {
                            let v0 = *o;
                            let v = v0 * 4.0;
                            let v = (v as f64 / (1.0 + (v as f64).abs())) as f32;
                            *o = v * p + v0 * p2;
                        }
                    }
                    _ => {}
                }
            }
            // C compares the normalized volume with 128: always true.
            if volume != 128.0 {
                if volume == 0.0 {
                    out.fill(0.0);
                } else {
                    for o in out.iter_mut() {
                        *o *= volume;
                    }
                }
            }
            if ctl_noise > 0 && volume != 0.0 {
                let noise = ctl_noise as f32 / 256.0;
                for o in out.iter_mut() {
                    let r = net::psynth_rand2(&mut self.noise_seed);
                    let mut v = (r as f32 / 32768.0) * noise;
                    v *= *o;
                    *o += v;
                }
            }
        }
        if vars_changed {
            self.cnt = cnt;
        }
        if volume == 0.0 {
            2
        } else {
            1
        }
    }
}

impl Module for Distortion {
    fn handle(&mut self, n: usize, e: &mut Event, net: &mut Net) -> isize {
        let mut retval = 0;
        match e.cmd {
            Cmd::Init => {
                net.register_ctl(n, 0, 256, 128, 0, C_VOLUME, 128, 0);
                net.register_ctl(n, 0, 10, 0, 1, C_TYPE, -1, 1);
                net.register_ctl(n, 0, 256, 0, 0, C_POWER, -1, 1);
                net.register_ctl(n, 1, 16, 16, 1, C_BITRATE, -1, 2);
                net.register_ctl(n, 0, 44100, 44100, 0, C_FREQ, -1, 2);
                net.set_ctl_flags(n, 4, 2);
                net.register_ctl(n, 0, 256, 0, 0, C_NOISE, -1, 3);
                self.cnt = 0;
                // stime_ns() + mod_num * 49157 + mod->id * 3079, the clock at 0.
                self.noise_seed = (n as u32).wrapping_mul(49157).wrapping_add(net.mods[n].id.wrapping_mul(3079));
                self.smp = [0.0; 2];
                self.smp_clean = true;
                retval = 1;
            }
            Cmd::Clean => {
                self.smp = [0.0; 2];
                self.smp_clean = true;
                self.cnt = 0;
                retval = 1;
            }
            Cmd::RenderReplace => retval = self.render(n, net),
            Cmd::Close => retval = 1,
            _ => {}
        }
        retval
    }
}
