// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! Pitch shifter (psynths_pitch_shifter.cpp): the input read back from a
//! ring buffer at another rate in two cross-faded grains, with feedback;
//! notes set the shift; optionally the dry sound when unshifted.

use super::super::net::{self, Cmd, Event, Module, Net};

const OUTPUTS: usize = 2;
const PITCH_BITS: i32 = 15;
const PITCH_SEMITONE: i32 = 10;
const PITCH_OCTAVES: i32 = 5;

const MODE_HQ_MONO: i32 = 1;
const MODE_LQ_MONO: i32 = 3;

const C_VOLUME: usize = 0;
const C_PITCH: usize = 1;
const C_PITCH_SCALE: usize = 2;
const C_FEEDBACK: usize = 3;
const C_GRAIN_SIZE: usize = 4;
const C_MODE: usize = 5;
const C_RET: usize = 6;

#[derive(Default)]
pub(crate) struct PitchShifter {
    max_grain_size: i32,
    min_grain_size: i32,
    buf_size: i32,
    buf: [Vec<f32>; OUTPUTS],
    buf_ptr: i32,
    buf_clean: bool,
    out_ptr: i64,
    out_delta: i64,
    mix: i32,
    mix_step: i32,
    empty_frames_counter: i32,
    recalc_req: bool,
}

impl PitchShifter {
    fn recalc_delta(&mut self, m: &net::ModCommon) {
        let pitch = m.ctl(C_PITCH) - PITCH_OCTAVES * 12 * PITCH_SEMITONE;
        let base_freq = net::get_freq(PITCH_OCTAVES * 12 * 64);
        let freq = net::get_freq(PITCH_OCTAVES * 12 * 64 - ((pitch * m.ctl(C_PITCH_SCALE) * 64) / 100) / PITCH_SEMITONE);
        self.out_delta = ((freq as i64) << PITCH_BITS) / base_freq as i64;
        self.out_delta -= 1 << PITCH_BITS;
    }

    fn set_mix(&mut self, m: &net::ModCommon) {
        self.mix = 32768 << 15;
        if m.ctl(C_RET) != 0 && self.out_delta == 0 && m.ctl(C_FEEDBACK) == 0 {
            self.mix = 0;
        }
    }

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
        let m = &mut net.mods[n];
        let (offset, frames) = (m.offset as usize, m.frames as usize);
        let outputs_num = m.output_channels.clamp(0, 2) as usize;
        let input_signal = (0..outputs_num).any(|ch| m.in_empty[ch] < (offset + frames) as i32);
        let (ctl_feedback, ctl_volume, ctl_ret, ctl_grain) = (m.ctl(C_FEEDBACK), m.ctl(C_VOLUME), m.ctl(C_RET), m.ctl(C_GRAIN_SIZE));
        if input_signal {
            self.empty_frames_counter = 0;
        } else if ctl_feedback == 0 {
            if self.empty_frames_counter >= self.buf_size {
                if self.empty_frames_counter < 10000000 {
                    self.out_ptr = 0;
                    self.buf_clean = true;
                    self.set_mix(m);
                    self.empty_frames_counter = 10000000;
                }
                return 0;
            }
            self.empty_frames_counter += frames as i32;
        }
        self.buf_clean = false;
        if self.recalc_req {
            self.recalc_delta(m);
            self.recalc_req = false;
        }
        let mut len = (ctl_grain * self.max_grain_size) / 256;
        if len < self.min_grain_size {
            len = self.min_grain_size;
        }
        let len2 = len / 2;
        let len3 = (len2 as i64) << PITCH_BITS;
        let mask = (self.buf_size - 1) as i32;
        let mut mix_target = 32768 << 15;
        let mut mix_step = self.mix_step;
        if ctl_ret != 0 {
            if self.out_delta == 0 && ctl_feedback == 0 {
                mix_target = 0;
            }
            if ctl_ret == 2 {
                mix_step *= 4;
            }
        }
        let mut ptr = self.buf_ptr;
        let mut out_ptr = self.out_ptr;
        let mut mix = self.mix;
        let hq = mode <= MODE_HQ_MONO;
        for ch in 0..outputs_num {
            let (ci, co) = (&m.chin, &mut m.chout);
            let inp = &ci[ch][offset..offset + frames];
            let out = &mut co[ch][offset..offset + frames];
            ptr = self.buf_ptr;
            out_ptr = self.out_ptr;
            mix = self.mix;
            let buf = &mut self.buf[ch];
            for i in 0..frames {
                buf[ptr as usize] = inp[i];
                if out_ptr > 0 {
                    out_ptr %= len3;
                } else {
                    while out_ptr < 0 {
                        out_ptr += len3;
                    }
                }
                let out_ptr2 = (out_ptr >> PITCH_BITS) as i32;
                let out_ptr3 = (ptr - len2 + out_ptr2) & mask;
                let out_ptr4 = (out_ptr3 - len2) & mask;
                let mut val = if hq {
                    let c1 = (out_ptr & ((1 << PITCH_BITS) - 1)) as i32;
                    let c2 = (1 << PITCH_BITS) - c1;
                    let val1 = (buf[out_ptr3 as usize] * c2 as f32 + buf[((out_ptr3 + 1) & mask) as usize] * c1 as f32) / (1 << PITCH_BITS) as f32;
                    let val2 = (buf[out_ptr4 as usize] * c2 as f32 + buf[((out_ptr4 + 1) & mask) as usize] * c1 as f32) / (1 << PITCH_BITS) as f32;
                    let c1 = (out_ptr2 << 15) / len2;
                    let c2 = 32768 - c1;
                    (val1 * c2 as f32 + val2 * c1 as f32) / 32768.0
                } else {
                    let c1 = (out_ptr2 << 15) / len2;
                    let c2 = 32768 - c1;
                    (buf[out_ptr3 as usize] * c2 as f32 + buf[out_ptr4 as usize] * c1 as f32) / 32768.0
                };
                out[i] = val;
                if ctl_feedback != 0 {
                    val = (val * ctl_feedback as f32) / 256.0;
                    buf[ptr as usize] += val;
                }
                ptr += 1;
                ptr &= mask;
                out_ptr += self.out_delta;
            }
            if ctl_volume != 256 {
                for o in out.iter_mut() {
                    *o = (*o * ctl_volume as f32) / 256.0;
                }
            }
            if mix == mix_target {
                if mix == 0 {
                    out.copy_from_slice(inp);
                } else if mix < 32768 << 15 {
                    for (o, &x) in out.iter_mut().zip(inp.iter()) {
                        *o = *o * (mix >> 15) as f32 / 32768.0 + x * (32768 - (mix >> 15)) as f32 / 32768.0;
                    }
                }
            } else {
                for (o, &x) in out.iter_mut().zip(inp.iter()) {
                    *o = *o * (mix >> 15) as f32 / 32768.0 + x * (32768 - (mix >> 15)) as f32 / 32768.0;
                    if mix < mix_target {
                        mix += mix_step;
                        if mix >= mix_target {
                            mix = mix_target;
                        }
                    } else {
                        mix -= mix_step;
                        if mix < mix_target {
                            mix = mix_target;
                        }
                    }
                }
            }
        }
        self.buf_ptr = ptr;
        self.out_ptr = out_ptr;
        self.mix = mix;
        if ctl_volume == 0 {
            2
        } else {
            1
        }
    }
}

impl Module for PitchShifter {
    fn handle(&mut self, n: usize, e: &mut Event, net: &mut Net) -> isize {
        let mut retval: isize = 0;
        match e.cmd {
            Cmd::Init => {
                let mid = PITCH_OCTAVES * 12 * PITCH_SEMITONE;
                net.register_ctl(n, 0, 512, 256, 0, C_VOLUME, 256, 0);
                net.register_ctl(n, 0, mid * 2, mid, 0, C_PITCH, mid, 0);
                net.set_ctl_show_offset(n, 1, -mid);
                net.register_ctl(n, 0, 200, 100, 0, C_PITCH_SCALE, 100, 0);
                net.register_ctl(n, 0, 256, 0, 0, C_FEEDBACK, 0, 0);
                net.register_ctl(n, 0, 256, 64, 0, C_GRAIN_SIZE, -1, 0);
                net.register_ctl(n, 0, MODE_LQ_MONO, 0, 1, C_MODE, -1, 1);
                net.register_ctl(n, 0, 2, 0, 1, C_RET, 0, 1);
                self.max_grain_size = net.sampling_freq / 4;
                self.min_grain_size = net.sampling_freq / 5512;
                self.buf_size = ((self.max_grain_size * 2) as u32).next_power_of_two() as i32;
                for b in self.buf.iter_mut() {
                    *b = vec![0.0; self.buf_size as usize];
                }
                self.buf_ptr = 0;
                self.buf_clean = true;
                self.out_ptr = 0;
                self.out_delta = 0;
                self.mix_step = (32768 << 15) / net.sampling_freq;
                self.empty_frames_counter = self.buf_size;
                retval = 1;
            }
            Cmd::SetupFinished => {
                self.recalc_delta(&net.mods[n]);
                self.set_mix(&net.mods[n]);
                retval = 1;
            }
            Cmd::Clean => {
                self.recalc_delta(&net.mods[n]);
                self.set_mix(&net.mods[n]);
                self.buf_ptr = 0;
                self.out_ptr = 0;
                if !self.buf_clean {
                    for b in self.buf.iter_mut() {
                        b.fill(0.0);
                    }
                    self.buf_clean = true;
                }
                self.empty_frames_counter = self.buf_size + 1;
                retval = 1;
            }
            Cmd::RenderReplace => retval = self.render(n, net),
            Cmd::NoteOn | Cmd::SetFreq => {
                let m = &mut net.mods[n];
                let scale = m.ctl(C_PITCH_SCALE).max(1);
                let mut pitch = (net::NOTE0_PITCH - 5 * 12 * 256 - e.pitch()) * PITCH_SEMITONE * 100 / scale / 256;
                pitch += PITCH_OCTAVES * 12 * PITCH_SEMITONE;
                if m.ctl(C_PITCH) != pitch {
                    if let Some(var) = m.ctls.get(C_PITCH).and_then(|c| c.var) {
                        m.vars[var] = pitch;
                    }
                    self.recalc_req = true;
                }
                retval = 1;
            }
            Cmd::SetLocalController | Cmd::SetGlobalController => {
                if matches!(e.ctl_num(), 1 | 2) {
                    self.recalc_req = true;
                }
            }
            Cmd::Close => retval = 1,
            _ => {}
        }
        retval
    }
}
