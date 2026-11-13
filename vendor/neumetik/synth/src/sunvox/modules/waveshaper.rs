// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! WaveShaper (psynths_waveshaper.cpp): each sample mapped through a
//! drawn 256-point curve (symmetric or not, interpolated in HQ), mixed
//! with the dry sound, then DC-blocked.

use super::super::net::{Cmd, Event, Module, Net};
use super::dc_blocker::DcFilter;
use super::generator::to_int16;

const OUTPUTS: usize = 2;
const SHAPE_SIZE: usize = 256;

const MODE_HQ_MONO: i32 = 1;
const MODE_LQ: i32 = 2;
const MODE_LQ_MONO: i32 = 3;
const MODES: i32 = 4;

const C_IN_VOLUME: usize = 0;
const C_MIX: usize = 1;
const C_OUT_VOLUME: usize = 2;
const C_SYMMETRIC: usize = 3;
const C_MODE: usize = 4;
const C_DC_FILTER: usize = 5;

/// 0x1.FEp-62, keeping the input off zero.
const TINY: f32 = f32::from_bits(0x20FF_0000);

#[derive(Default)]
pub(crate) struct WaveShaper {
    f: DcFilter,
    empty_frames_counter: i32,
    empty_frames_counter_max: i32,
}

fn shape(m: &super::super::net::ModCommon, i: usize) -> i32 {
    m.chunk(0).and_then(|c| c.data.get(i * 2..i * 2 + 2)).map_or(0, |b| u16::from_le_bytes([b[0], b[1]]) as i32)
}

impl WaveShaper {
    fn render(&mut self, n: usize, net: &mut Net) -> isize {
        let mode = net.mods[n].ctl(C_MODE);
        if mode == MODE_HQ_MONO || mode == MODE_LQ_MONO {
            if net.mods[n].output_channels != 1 {
                net.set_number_of_outputs(1, n);
                net.set_number_of_inputs(1, n);
            }
        } else if net.mods[n].output_channels != OUTPUTS as i32 {
            net.set_number_of_outputs(OUTPUTS as i32, n);
            net.set_number_of_inputs(OUTPUTS as i32, n);
        }
        let base = net.base_host_version;
        let m = &mut net.mods[n];
        let (offset, frames) = (m.offset as usize, m.frames as usize);
        let outputs_num = m.output_channels.clamp(0, 2) as usize;
        let input_signal = (0..outputs_num).any(|ch| m.in_empty[ch] < (offset + frames) as i32);
        let (ctl_in_volume, ctl_mix, ctl_out_volume, ctl_symmetric, ctl_dc) = (m.ctl(C_IN_VOLUME), m.ctl(C_MIX), m.ctl(C_OUT_VOLUME), m.ctl(C_SYMMETRIC), m.ctl(C_DC_FILTER));
        let sh: Vec<i32> = (0..SHAPE_SIZE).map(|i| shape(m, i)).collect();
        if input_signal {
            self.empty_frames_counter = 0;
        } else {
            self.empty_frames_counter += frames as i32;
            if self.empty_frames_counter >= self.empty_frames_counter_max {
                let mut sleep = true;
                if base >= 0x02010200 && ctl_dc == 0 {
                    if ctl_symmetric != 0 {
                        if sh[0] != 0 {
                            sleep = false;
                        }
                    } else {
                        let sp = 32768 / (65536 / SHAPE_SIZE);
                        if sh[sp] != 32768 {
                            sleep = false;
                        }
                    }
                }
                if sleep {
                    return 0;
                }
                self.empty_frames_counter = 0;
            }
        }
        let lim = (32767 - 128) as f32 / 32768.0;
        for ch in 0..outputs_num {
            let (ci, co) = (&m.chin, &mut m.chout);
            let inp = &ci[ch][offset..offset + frames];
            let out = &mut co[ch][offset..offset + frames];
            let src: Vec<f32> = if ctl_in_volume != 256 {
                for (o, &x) in out.iter_mut().zip(inp.iter()) {
                    *o = x * ctl_in_volume as f32 / 256.0;
                }
                out.to_vec()
            } else {
                inp.to_vec()
            };
            if mode == MODE_LQ || mode == MODE_LQ_MONO {
                for (o, &v) in out.iter_mut().zip(src.iter()) {
                    let mut v16 = to_int16(v) as i32;
                    let res = if ctl_symmetric != 0 {
                        if v16 > 0 {
                            sh[(v16 / (32768 / SHAPE_SIZE as i32)) as usize] / 2
                        } else {
                            v16 = -v16;
                            if v16 == 32768 {
                                v16 = 32767;
                            }
                            -sh[(v16 / (32768 / SHAPE_SIZE as i32)) as usize] / 2
                        }
                    } else {
                        v16 += 32768;
                        sh[(v16 / (65536 / SHAPE_SIZE as i32)) as usize] - 32768
                    };
                    *o = res as f32 / 32768.0;
                }
            } else if ctl_symmetric != 0 {
                let tiny = sh[0] != 0;
                for (o, &x) in out.iter_mut().zip(src.iter()) {
                    let mut v = if tiny { x + TINY } else { x };
                    v = v.clamp_c(-lim, lim);
                    v *= 256.0;
                    if v > 0.0 {
                        let iv = v as i32;
                        let frac = v - iv as f32;
                        let res1 = sh[iv as usize] as f32;
                        let res2 = sh[iv as usize + 1] as f32;
                        *o = (res1 * (1.0 - frac) + res2 * frac) * (1.0f32 / 65536.0);
                    } else {
                        v = -v;
                        let iv = v as i32;
                        let frac = v - iv as f32;
                        let res1 = sh[iv as usize] as f32;
                        let res2 = sh[iv as usize + 1] as f32;
                        *o = -(res1 * (1.0 - frac) + res2 * frac) * (1.0f32 / 65536.0);
                    }
                }
            } else {
                let hi = (65535 - 256) as f32 / 32768.0;
                for (o, &x) in out.iter_mut().zip(src.iter()) {
                    let mut v = x + 1.0;
                    v = v.clamp_c(0.0, hi);
                    v *= 128.0;
                    let iv = v as i32;
                    let frac = v - iv as f32;
                    let res1 = sh[iv as usize] as f32;
                    let res2 = sh[iv as usize + 1] as f32;
                    *o = ((res1 * (1.0 - frac) + res2 * frac) - 32768.0) * (1.0f32 / 32768.0);
                }
            }
            if ctl_mix != 256 {
                for (o, &x) in out.iter_mut().zip(inp.iter()) {
                    let v2 = *o * ctl_mix as f32 / 256.0;
                    let v1 = x * (256 - ctl_mix) as f32 / 256.0;
                    *o = v1 + v2;
                }
            }
            if ctl_out_volume != 256 {
                for o in out.iter_mut() {
                    *o = *o * ctl_out_volume as f32 / 256.0;
                }
            }
            if ctl_dc != 0 {
                let x = out.to_vec();
                self.f.run(ch, Some(&x), out);
            }
        }
        1
    }
}

trait ClampC {
    fn clamp_c(self, min: f32, max: f32) -> f32;
}

impl ClampC for f32 {
    /// LIMIT_NUM: `if( v < min ) v = min; if( v > max ) v = max;`.
    fn clamp_c(self, min: f32, max: f32) -> f32 {
        let mut v = self;
        if v < min {
            v = min;
        }
        if v > max {
            v = max;
        }
        v
    }
}

impl Module for WaveShaper {
    fn handle(&mut self, n: usize, e: &mut Event, net: &mut Net) -> isize {
        let mut retval: isize = 0;
        match e.cmd {
            Cmd::Init => {
                net.register_ctl(n, 0, 512, 256, 0, C_IN_VOLUME, 256, 0);
                net.register_ctl(n, 0, 256, 256, 0, C_MIX, -1, 0);
                net.register_ctl(n, 0, 512, 256, 0, C_OUT_VOLUME, 256, 0);
                net.register_ctl(n, 0, 1, 1, 1, C_SYMMETRIC, -1, 0);
                net.register_ctl(n, 0, MODES - 1, 0, 1, C_MODE, -1, 0);
                net.register_ctl(n, 0, 1, 1, 1, C_DC_FILTER, -1, 0);
                self.f = DcFilter::new(net.sampling_freq);
                self.empty_frames_counter_max = net.sampling_freq;
                self.empty_frames_counter = self.empty_frames_counter_max;
                retval = 1;
            }
            Cmd::SetupFinished => {
                if net.mods[n].chunk(0).is_none() {
                    let mut d = vec![0u8; SHAPE_SIZE * 2];
                    for i in 0..SHAPE_SIZE {
                        d[i * 2..i * 2 + 2].copy_from_slice(&((i * (65536 / SHAPE_SIZE)) as u16).to_le_bytes());
                    }
                    net.new_chunk(n, 0, super::super::net::Chunk { data: d, flags: 0, freq: 0 });
                }
                retval = 1;
            }
            Cmd::Clean => {
                self.f.stop();
                self.empty_frames_counter = self.empty_frames_counter_max;
                retval = 1;
            }
            Cmd::RenderReplace => retval = self.render(n, net),
            Cmd::Close => retval = 1,
            _ => {}
        }
        retval
    }
}
