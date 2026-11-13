// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! DC Blocker (psynths_dc_blocker.cpp): a one-pole high-pass at 5 Hz,
//! also the filter Reverb puts before its combs.

use super::super::net::{Cmd, Event, Module, Net};

const CHANNELS: usize = 2;

/// dc_filter.
#[derive(Clone, Copy, Default)]
pub(crate) struct DcFilter {
    pole: f32,
    prev_x: [f32; CHANNELS],
    prev_y: [f32; CHANNELS],
    empty_samples_counter: [i32; CHANNELS],
    empty_frames_counter_max: i32,
}

impl DcFilter {
    /// dc_filter_init.
    pub fn new(sample_rate: i32) -> DcFilter {
        let pole = ((2.0 * std::f64::consts::PI) * 5.0f32 as f64 / (sample_rate as f32) as f64) as f32;
        let mut f = DcFilter { pole: 1.0 - pole, empty_frames_counter_max: sample_rate, ..DcFilter::default() };
        f.stop();
        f
    }

    /// dc_filter_stop.
    pub fn stop(&mut self) {
        for i in 0..CHANNELS {
            self.prev_x[i] = 0.0;
            self.prev_y[i] = 0.0;
            self.empty_samples_counter[i] = self.empty_frames_counter_max;
        }
    }

    pub fn finished(&self, ch: usize) -> bool {
        self.empty_samples_counter[ch] >= self.empty_frames_counter_max
    }

    /// dc_filter_run: `input` (None: silence) filtered into `out`; true when
    /// the filter has stopped and wrote nothing.
    pub fn run(&mut self, ch: usize, input: Option<&[f32]>, out: &mut [f32]) -> bool {
        let pole = self.pole;
        let mut px = self.prev_x[ch];
        let mut py = self.prev_y[ch];
        match input {
            Some(inp) => {
                self.empty_samples_counter[ch] = 0;
                for (o, &x) in out.iter_mut().zip(inp) {
                    py = x - px + py * pole;
                    px = x;
                    *o = py;
                }
            }
            None => {
                if self.empty_samples_counter[ch] >= self.empty_frames_counter_max {
                    return true;
                }
                self.empty_samples_counter[ch] += out.len() as i32;
                for o in out.iter_mut() {
                    py = 0.0 - px + py * pole;
                    px = 0.0;
                    *o = py;
                }
            }
        }
        self.prev_x[ch] = px;
        self.prev_y[ch] = py;
        false
    }
}

const C_MONO: usize = 0;

#[derive(Default)]
pub(crate) struct DcBlocker {
    f: DcFilter,
}

impl Module for DcBlocker {
    fn handle(&mut self, n: usize, e: &mut Event, net: &mut Net) -> isize {
        let mut retval = 0;
        match e.cmd {
            Cmd::Init => {
                net.register_ctl(n, 0, 1, 0, 1, C_MONO, -1, 0);
                self.f = DcFilter::new(net.sampling_freq);
                retval = 1;
            }
            Cmd::Clean => {
                self.f.stop();
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
                if !input_signal && self.f.finished(0) {
                    return 0;
                }
                for ch in 0..outputs_num {
                    let (chin, chout) = (&m.chin, &mut m.chout);
                    let out = &mut chout[ch][offset..offset + frames];
                    let inp = if input_signal { Some(&chin[ch][offset..offset + frames]) } else { None };
                    self.f.run(ch, inp, out);
                }
                retval = 1;
            }
            Cmd::Close => retval = 1,
            _ => {}
        }
        retval
    }
}
