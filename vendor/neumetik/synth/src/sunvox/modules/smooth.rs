// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! Smooth (psynths_smooth.cpp): its input followed at a limited rate of
//! rise and fall, linearly or exponentially.

use super::super::net::{self, Cmd, Event, Module, Net};

const OUTPUTS: usize = 2;

const C_RISE: usize = 0;
const C_FALL: usize = 1;
const C_RISE_ONLY: usize = 2;
const C_SCALE: usize = 3;
const C_MODE: usize = 4;
const C_MONO: usize = 5;

#[derive(Default)]
pub(crate) struct Smooth {
    v: [f32; OUTPUTS],
    c1: f32,
    c2: f32,
    changed: bool,
    empty: bool,
}

impl Smooth {
    fn handle_changes(&mut self, n: usize, net: &mut Net) {
        if self.changed {
            let want = if net.mods[n].ctl(C_MONO) != 0 { 1 } else { OUTPUTS as i32 };
            if net.mods[n].output_channels != want {
                net.set_number_of_outputs(want, n);
                net.set_number_of_inputs(want, n);
                self.v = [0.0; OUTPUTS];
            }
        }
        self.changed = false;
    }

    fn recalc(&mut self, m: &net::ModCommon, srate: i32) {
        let ctl_rise = m.ctl(C_RISE);
        let ctl_fall = if m.ctl(C_RISE_ONLY) != 0 { ctl_rise } else { m.ctl(C_FALL) };
        let mut rise = ctl_rise as f32;
        let mut fall = ctl_fall as f32;
        let scale = m.ctl(C_SCALE);
        if scale != 100 {
            rise = rise * scale as f32 / 100.0;
            fall = fall * scale as f32 / 100.0;
        }
        match m.ctl(C_MODE) {
            0 => {
                self.c1 = rise / srate as f32 * 1.0 * 1.0;
                self.c2 = fall / srate as f32 * 1.0 * 1.0;
            }
            1 => {
                // exp of a float: C++'s float overload, expf.
                let c1 = (-2.0f32 * std::f32::consts::PI * (rise / srate as f32)).exp();
                let c2 = (-2.0f32 * std::f32::consts::PI * (fall / srate as f32)).exp();
                self.c1 = c1 * 1.0;
                self.c2 = c2 * 1.0;
            }
            _ => {}
        }
    }

    fn render(&mut self, n: usize, net: &mut Net) -> isize {
        if self.changed {
            self.handle_changes(n, net);
        }
        let m = &mut net.mods[n];
        let (offset, frames) = (m.offset as usize, m.frames as usize);
        let outputs_num = m.output_channels.clamp(0, 2) as usize;
        let input_signal = (0..outputs_num).any(|ch| m.in_empty[ch] < (offset + frames) as i32);
        if !input_signal && self.empty {
            return 0;
        }
        self.empty = false;
        let (c1, c2) = (self.c1, self.c2);
        let mode = m.ctl(C_MODE);
        for ch in 0..outputs_num {
            let (ci, co) = (&m.chin, &mut m.chout);
            let out = &mut co[ch][offset..offset + frames];
            let mut v = self.v[ch];
            if mode == 0 {
                if input_signal {
                    for (o, &x) in out.iter_mut().zip(&ci[ch][offset..offset + frames]) {
                        let in_val = x * 1.0;
                        if v < in_val {
                            v += c1;
                            if v >= in_val {
                                v = in_val;
                            }
                        } else {
                            v -= c2;
                            if v < in_val {
                                v = in_val;
                            }
                        }
                        *o = v / 1.0;
                    }
                } else {
                    for o in out.iter_mut() {
                        if v < 0.0 {
                            v += c1;
                            if v >= 0.0 {
                                v = 0.0;
                            }
                        } else {
                            v -= c2;
                            if v < 0.0 {
                                v = 0.0;
                            }
                        }
                        *o = v / 1.0;
                    }
                    if !net::is_not_min(v) {
                        self.empty = true;
                    }
                }
            } else if mode == 1 {
                let (c1_, c2_) = (1.0 - c1, 1.0 - c2);
                if input_signal {
                    for (o, &x) in out.iter_mut().zip(&ci[ch][offset..offset + frames]) {
                        let in_val = x * 1.0;
                        v = if v < in_val { v * c1 + in_val * c1_ } else { v * c2 + in_val * c2_ };
                        *o = v / 1.0;
                    }
                } else {
                    for o in out.iter_mut() {
                        v = if v < 0.0 { v * c1 } else { v * c2 };
                        *o = v / 1.0;
                    }
                    if !net::is_not_min(v) {
                        self.empty = true;
                    }
                }
            }
            self.v[ch] = v;
        }
        1
    }
}

impl Module for Smooth {
    fn handle(&mut self, n: usize, e: &mut Event, net: &mut Net) -> isize {
        let mut retval: isize = 0;
        match e.cmd {
            Cmd::Init => {
                net.register_ctl(n, 0, 32768, 5000, 0, C_RISE, -1, 0);
                net.register_ctl(n, 0, 32768, 5000, 0, C_FALL, -1, 0);
                net.set_ctl_flags(n, 0, 2);
                net.set_ctl_flags(n, 1, 2);
                net.register_ctl(n, 0, 1, 0, 1, C_RISE_ONLY, -1, 0);
                net.register_ctl(n, 0, 400, 100, 0, C_SCALE, 100, 0);
                net.register_ctl(n, 0, 1, 0, 1, C_MODE, -1, 0);
                net.register_ctl(n, 0, 1, 0, 1, C_MONO, -1, 1);
                self.changed = true;
                self.empty = true;
                retval = 1;
            }
            Cmd::SetupFinished => {
                self.handle_changes(n, net);
                self.recalc(&net.mods[n], net.sampling_freq);
                retval = 1;
            }
            Cmd::Clean => {
                self.v = [0.0; OUTPUTS];
                self.empty = true;
                retval = 1;
            }
            Cmd::RenderReplace => retval = self.render(n, net),
            Cmd::SetLocalController | Cmd::SetGlobalController => match e.ctl_num() {
                0..=4 => {
                    net::set_ctl(&mut net.mods[n], e);
                    retval = 1;
                    let srate = net.sampling_freq;
                    self.recalc(&net.mods[n], srate);
                }
                5 => self.changed = true,
                _ => {}
            },
            Cmd::Close => retval = 1,
            _ => {}
        }
        retval
    }
}
