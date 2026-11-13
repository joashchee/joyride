// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! Vibrato (psynths_vibrato.cpp): the input read back from a buffer of
//! 1/25 second at a delay a sine swings, its rate in Hz-like units,
//! milliseconds, Hz or the song's ticks or lines.

use super::super::net::{Cmd, Event, Module, Net};

const OUTPUTS: usize = 2;
const FREQ_UNIT_MAX: i32 = 6;

const C_VOLUME: usize = 0;
const C_AMP: usize = 1;
const C_FREQ: usize = 2;
const C_MONO: usize = 3;
const C_SET_PHASE: usize = 4;
const C_FREQ_UNITS: usize = 5;
const C_EXP_AMP: usize = 6;

#[derive(Default)]
pub(crate) struct Vibrato {
    buf_size: i32,
    buf: [Vec<f32>; OUTPUTS],
    buf_ptr: i32,
    buf_clean: bool,
    ptr: u32,
    empty_frames_counter: i32,
    tick_size: i32,
    ticks_per_line: u8,
}

impl Vibrato {
    fn change_ctl_limits(&self, n: usize, net: &mut Net) {
        if net.base_host_version < 0x01090202 {
            return;
        }
        let max = match net.mods[n].ctl(C_FREQ_UNITS) {
            1 => 4000,
            2 => 8192 * 2,
            _ => 2048,
        };
        if net.mods[n].ctls.get(2).is_some_and(|c| c.max != max) {
            net.change_ctl_limits(n, 2, -1, max, max);
        }
    }

    fn set_phase(&mut self, n: usize, net: &mut Net, check_version: bool) {
        if !check_version || net.base_host_version >= 0x01090300 {
            let mut e = Event::new(Cmd::SetGlobalController);
            e.set_ctl_num(4);
            e.set_ctl_val((net.mods[n].ctl(C_SET_PHASE) << 7) as u16);
            self.handle(n, &mut e, net);
        }
    }

    fn render(&mut self, n: usize, net: &mut Net) -> isize {
        let want = if net.mods[n].ctl(C_MONO) != 0 { 1 } else { 2 };
        if net.mods[n].output_channels != want {
            net.set_number_of_outputs(want, n);
            net.set_number_of_inputs(want, n);
        }
        let srate = net.sampling_freq;
        let m = &mut net.mods[n];
        let (offset, frames) = (m.offset as usize, m.frames as usize);
        let freq = m.ctl(C_FREQ);
        let units = m.ctl(C_FREQ_UNITS);
        let delta: u32 = match units {
            0 => ((freq as u32) << 20) / srate as u32,
            1 => (((64u64 << 20) * 1000) / (freq.wrapping_mul(srate)) as u32 as u64) as u32,
            2 => ((((freq as u32 as u64) << 20) * 64) / srate as u64) as u32,
            3 => (((64u64 << 20) * 256) / ((freq as u32).wrapping_mul(self.tick_size as u32)) as u64) as u32,
            _ => (((64u64 << 20) * 256) / (freq as u32 as u64 * self.tick_size as u32 as u64 * self.ticks_per_line as u64 / (units - 3) as u64)) as u32,
        };
        let outputs_num = m.output_channels.max(0) as usize;
        let input_signal = (0..outputs_num).any(|ch| m.in_empty[ch] < (offset + frames) as i32);
        if input_signal {
            self.empty_frames_counter = 0;
        } else {
            if self.empty_frames_counter >= self.buf_size {
                self.ptr = self.ptr.wrapping_add(delta.wrapping_mul(frames as u32));
                return 0;
            }
            self.empty_frames_counter += frames as i32;
        }
        self.buf_clean = false;
        let buf_size = self.buf_size;
        let amp_c = m.ctl(C_AMP);
        let mut buf_size_amp = if m.ctl(C_EXP_AMP) != 0 {
            let mut b = (buf_size * amp_c * amp_c) / (256 * 256);
            if amp_c > 0 && b == 0 {
                b = 1;
            }
            b
        } else {
            (buf_size * amp_c) / 256
        };
        if buf_size_amp > 1 {
            buf_size_amp -= 1;
        }
        let ctl_volume = m.ctl(C_VOLUME);
        let volume = ctl_volume as f32 / 256.0;
        let mut buf_ptr = 0;
        let mut ptr = 0u32;
        for ch in 0..outputs_num {
            let (chin, chout) = (&m.chin, &mut m.chout);
            let inp = &chin[ch][offset..offset + frames];
            let out = &mut chout[ch][offset..offset + frames];
            let cbuf = &mut self.buf[ch];
            buf_ptr = self.buf_ptr;
            ptr = self.ptr;
            for i in 0..frames {
                cbuf[buf_ptr as usize] = inp[i];
                let p = (ptr & ((512 << 17) - 1)) as f32 / (512 << 17) as f32;
                let amp = (((p * 2.0) as f64 * std::f64::consts::PI) as f32).sin();
                let amp = ((amp as f64 + 1.0) / 2.0) as f32;
                let mut bp1 = buf_ptr as f32 - amp * buf_size_amp as f32;
                if bp1 < 0.0 {
                    bp1 += buf_size as f32;
                }
                if bp1 >= buf_size as f32 {
                    bp1 -= buf_size as f32;
                }
                let i1 = bp1 as i32;
                let c = bp1 - i1 as f32;
                let mut i2 = i1 + 1;
                if i2 >= buf_size {
                    i2 = 0;
                }
                let v = cbuf[i1 as usize] * (1.0 - c) + cbuf[i2 as usize] * c;
                out[i] = v * volume;
                buf_ptr += 1;
                if buf_ptr >= buf_size {
                    buf_ptr = 0;
                }
                ptr = ptr.wrapping_add(delta);
            }
        }
        self.buf_ptr = buf_ptr;
        self.ptr = ptr;
        if ctl_volume == 0 {
            2
        } else {
            1
        }
    }
}

impl Module for Vibrato {
    fn handle(&mut self, n: usize, e: &mut Event, net: &mut Net) -> isize {
        let mut retval = 0;
        match e.cmd {
            Cmd::Init => {
                net.register_ctl(n, 0, 256, 256, 0, C_VOLUME, -1, 0);
                net.register_ctl(n, 0, 256, 16, 0, C_AMP, -1, 0);
                net.register_ctl(n, 1, 2048, 256, 0, C_FREQ, -1, 0);
                net.register_ctl(n, 0, 1, 0, 1, C_MONO, -1, 0);
                net.register_ctl(n, 0, 256, 0, 0, C_SET_PHASE, -1, 0);
                net.register_ctl(n, 0, FREQ_UNIT_MAX, 0, 1, C_FREQ_UNITS, -1, 0);
                net.register_ctl(n, 0, 1, 0, 1, C_EXP_AMP, -1, 0);
                self.buf_size = net.sampling_freq / 25;
                for b in self.buf.iter_mut() {
                    *b = vec![0.0; self.buf_size as usize];
                }
                self.buf_ptr = 0;
                self.buf_clean = true;
                self.ptr = 0;
                self.empty_frames_counter = self.buf_size;
                retval = 1;
            }
            Cmd::SetupFinished => {
                self.change_ctl_limits(n, net);
                self.set_phase(n, net, true);
                retval = 1;
            }
            Cmd::Clean => {
                if !self.buf_clean {
                    for b in self.buf.iter_mut() {
                        b.fill(0.0);
                    }
                    self.buf_clean = true;
                }
                self.buf_ptr = 0;
                self.empty_frames_counter = self.buf_size;
                self.set_phase(n, net, true);
                retval = 1;
            }
            Cmd::RenderReplace => retval = self.render(n, net),
            Cmd::SetLocalController | Cmd::SetGlobalController => {
                if e.ctl_num() == 4 {
                    self.ptr = ((e.ctl_val() >> 6) as u32) << 17;
                }
                if e.ctl_num() == 5 {
                    let v = (e.ctl_val() as i32).min(FREQ_UNIT_MAX);
                    if let Some(var) = net.mods[n].ctls.get(C_FREQ_UNITS).and_then(|c| c.var) {
                        net.mods[n].vars[var] = v;
                    }
                    self.change_ctl_limits(n, net);
                    retval = 1;
                }
            }
            Cmd::NoteOn => {
                self.set_phase(n, net, false);
                retval = 1;
            }
            Cmd::SpeedChanged => {
                self.tick_size = e.tick_size();
                self.ticks_per_line = e.tpl();
                retval = 1;
            }
            Cmd::Close => retval = 1,
            _ => {}
        }
        retval
    }
}
