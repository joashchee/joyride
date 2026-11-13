// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! EQ (psynths_eq.cpp): a three-band split (four-pole low-passes at 1 and
//! 5 kHz, the input three samples late), each band its own gain.

use super::super::net::{Cmd, Event, Module, Net};

const OUTPUTS: usize = 2;

const C_LGAIN: usize = 0;
const C_MGAIN: usize = 1;
const C_HGAIN: usize = 2;
const C_MONO: usize = 3;

#[derive(Clone, Copy, Default)]
struct State {
    f1: [f32; 4],
    f2: [f32; 4],
    sdm: [f32; 3],
}

#[derive(Default)]
pub(crate) struct Eq {
    lf: f32,
    hf: f32,
    st: [State; OUTPUTS],
    empty_frames_counter: i32,
    empty_frames_counter_max: i32,
}

impl Module for Eq {
    fn handle(&mut self, n: usize, e: &mut Event, net: &mut Net) -> isize {
        let mut retval = 0;
        match e.cmd {
            Cmd::Init => {
                net.register_ctl(n, 0, 512, 256, 0, C_LGAIN, 256, 0);
                net.register_ctl(n, 0, 512, 256, 0, C_MGAIN, 256, 0);
                net.register_ctl(n, 0, 512, 256, 0, C_HGAIN, 256, 0);
                net.register_ctl(n, 0, 1, 0, 1, C_MONO, 1, 1);
                self.st = Default::default();
                let sr = net.sampling_freq as f64;
                self.lf = (2.0 * (std::f64::consts::PI * (1000.0 / sr)).sin()) as f32;
                self.hf = (2.0 * (std::f64::consts::PI * (5000.0 / sr)).sin()) as f32;
                self.empty_frames_counter_max = net.sampling_freq * 4;
                self.empty_frames_counter = self.empty_frames_counter_max;
                retval = 1;
            }
            Cmd::Clean => {
                self.st = Default::default();
                self.empty_frames_counter = self.empty_frames_counter_max;
                retval = 1;
            }
            Cmd::RenderReplace => {
                let want = if net.mods[n].ctl(C_MONO) != 0 { 1 } else { 2 };
                if net.mods[n].output_channels != want {
                    net.set_number_of_outputs(want, n);
                    net.set_number_of_inputs(want, n);
                }
                let m = &mut net.mods[n];
                let (frames, offset) = (m.frames as usize, m.offset as usize);
                let outputs_num = m.output_channels.max(0) as usize;
                let input_signal = (0..outputs_num).any(|ch| m.in_empty[ch] < (offset + frames) as i32);
                if input_signal {
                    self.empty_frames_counter = 0;
                } else {
                    if self.empty_frames_counter >= self.empty_frames_counter_max {
                        return 0;
                    }
                    self.empty_frames_counter += frames as i32;
                }
                let (lf, hf) = (self.lf, self.hf);
                let lgain = m.ctl(C_LGAIN) as f32 / 256.0;
                let mgain = m.ctl(C_MGAIN) as f32 / 256.0;
                let hgain = m.ctl(C_HGAIN) as f32 / 256.0;
                for ch in 0..outputs_num {
                    let (chin, chout) = (&m.chin, &mut m.chout);
                    let inp = &chin[ch][offset..offset + frames];
                    let out = &mut chout[ch][offset..offset + frames];
                    let s = &mut self.st[ch];
                    let (mut f1, mut f2, mut sdm) = (s.f1, s.f2, s.sdm);
                    for i in 0..frames {
                        let input = inp[i];
                        f1[0] += lf * (input - f1[0]);
                        f1[1] += lf * (f1[0] - f1[1]);
                        f1[2] += lf * (f1[1] - f1[2]);
                        f1[3] += lf * (f1[2] - f1[3]);
                        let mut l = f1[3];
                        f2[0] += hf * (input - f2[0]);
                        f2[1] += hf * (f2[0] - f2[1]);
                        f2[2] += hf * (f2[1] - f2[2]);
                        f2[3] += hf * (f2[2] - f2[3]);
                        let mut h = sdm[2] - f2[3];
                        let mut md = sdm[2] - (h + l);
                        l *= lgain;
                        md *= mgain;
                        h *= hgain;
                        sdm[2] = sdm[1];
                        sdm[1] = sdm[0];
                        sdm[0] = input;
                        out[i] = l + md + h;
                    }
                    *s = State { f1, f2, sdm };
                }
                retval = 1;
            }
            Cmd::Close => retval = 1,
            _ => {}
        }
        retval
    }
}
