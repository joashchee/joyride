// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! Amplifier (psynths_amplifier.cpp): out = width(inverse(abs(pan(in *
//! volume) + dc))), with a fine volume, a gain and a bipolar DC offset.

use super::super::net::{Cmd, Event, Module, Net};

const C_VOLUME: usize = 0;
const C_PAN: usize = 1;
const C_DC: usize = 2;
const C_INVERSE: usize = 3;
const C_WIDTH: usize = 4;
const C_ABS: usize = 5;
const C_FINE_VOLUME: usize = 6;
const C_GAIN: usize = 7;
const C_BIPOLAR_DC: usize = 8;

#[derive(Default)]
pub(crate) struct Amplifier;

impl Module for Amplifier {
    fn handle(&mut self, n: usize, e: &mut Event, net: &mut Net) -> isize {
        match e.cmd {
            Cmd::Init => {
                net.register_ctl(n, 0, 1024, 256, 0, C_VOLUME, 256, 0);
                net.register_ctl(n, 0, 256, 128, 0, C_PAN, 128, 0);
                net.set_ctl_show_offset(n, 1, -128);
                net.register_ctl(n, 0, 256, 128, 0, C_DC, 128, 1);
                net.set_ctl_show_offset(n, 2, -128);
                net.register_ctl(n, 0, 1, 0, 1, C_INVERSE, 1, 1);
                net.register_ctl(n, 0, 256, 128, 0, C_WIDTH, 128, 1);
                net.register_ctl(n, 0, 1, 0, 1, C_ABS, 1, 1);
                net.register_ctl(n, 0, 32768, 32768, 0, C_FINE_VOLUME, -1, 0);
                net.register_ctl(n, 0, 5000, 1, 0, C_GAIN, 1, 0);
                net.register_ctl(n, 0, 32768, 32768 / 2, 0, C_BIPOLAR_DC, 32768 / 2, 1);
                net.set_ctl_show_offset(n, 8, -32768 / 2);
                1
            }
            Cmd::Clean => 1,
            Cmd::RenderReplace => {
                let m = &mut net.mods[n];
                let (offset, frames) = (m.offset as usize, m.frames as usize);
                let c = |i| m.ctl(i);
                let (volume, pan, dc_c, inverse, width, abs, fine, gain, bdc_c) =
                    (c(C_VOLUME), c(C_PAN), c(C_DC), c(C_INVERSE), c(C_WIDTH), c(C_ABS), c(C_FINE_VOLUME), c(C_GAIN), c(C_BIPOLAR_DC));
                let mut no_input = (0..2).all(|ch| m.in_empty[ch] >= (offset + frames) as i32);
                if volume == 0 || fine == 0 || gain == 0 {
                    no_input = true;
                }
                if dc_c != 128 || bdc_c > 32768 / 2 {
                    no_input = false;
                }
                if no_input {
                    return 0;
                }
                let ctl_dc = dc_c - 128;
                let ctl_bdc = bdc_c - 32768 / 2;
                let dc = (ctl_dc as f32 * 1.0) / 128.0;
                let bdc = (ctl_bdc as f32 * 1.0) / (32768 / 2) as f32;
                for ch in 0..2 {
                    let (chin, chout) = (&m.chin, &mut m.chout);
                    let inp = &chin[ch][offset..offset + frames];
                    let out = &mut chout[ch][offset..offset + frames];
                    let mut vol = volume;
                    if pan < 128 {
                        if ch == 1 {
                            vol *= pan;
                            vol /= 128;
                        }
                    } else if ch == 0 {
                        vol *= 128 - (pan - 128);
                        vol /= 128;
                    }
                    if vol == 0 || fine == 0 || gain == 0 {
                        out.fill(0.0);
                    } else {
                        let fvol = (vol as f32 / 256.0) * (fine as f32 / 32768.0) * gain as f32;
                        if fvol != 1.0 {
                            for i in 0..frames {
                                out[i] = inp[i] * fvol;
                            }
                        } else {
                            out.copy_from_slice(inp);
                        }
                    }
                    if ctl_dc != 0 {
                        for o in out.iter_mut() {
                            *o += dc;
                        }
                    }
                    if ctl_bdc != 0 {
                        for o in out.iter_mut() {
                            let mut v = *o;
                            if v >= 0.0 {
                                v += bdc;
                                if v < 0.0 {
                                    v = 0.0;
                                }
                            } else {
                                v -= bdc;
                                if v > 0.0 {
                                    v = 0.0;
                                }
                            }
                            *o = v;
                        }
                    }
                    if abs != 0 {
                        for o in out.iter_mut() {
                            if *o < 0.0 {
                                *o = -*o;
                            }
                        }
                    }
                    if inverse != 0 {
                        for o in out.iter_mut() {
                            *o = -*o;
                        }
                    }
                }
                if width != 128 {
                    let w2 = 128 - width;
                    let [ref mut o1, ref mut o2] = m.chout;
                    for i in offset..offset + frames {
                        let (v1, v2) = (o1[i], o2[i]);
                        let v3 = (((v1 + v2) / 2.0) * w2 as f32) / 128.0;
                        o1[i] = v3 + (v1 * width as f32) / 128.0;
                        o2[i] = v3 + (v2 * width as f32) / 128.0;
                    }
                }
                1
            }
            Cmd::Close => 1,
            _ => 0,
        }
    }
}
