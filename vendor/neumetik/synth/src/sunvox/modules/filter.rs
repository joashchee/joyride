// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! Filter (psynths_filter.cpp): a state-variable filter (low, high,
//! band-pass, notch), up to four in a row, its cutoff, resonance and
//! volume moving to their controllers at the response's rate, a tick
//! (5 ms or the song's) at a time, an LFO on the cutoff.

use super::super::dsp::HSIN_TAB;
use super::super::net::{Cmd, Event, Module, Net};

const OUTPUTS: usize = 2;
const FILTERS: usize = 4;

const C_VOLUME: usize = 0;
const C_CUTOFF: usize = 1;
const C_RESONANCE: usize = 2;
const C_TYPE: usize = 3;
const C_RESPONSE: usize = 4;
const C_MODE: usize = 5;
const C_IMPULSE: usize = 6;
const C_MIX: usize = 7;
const C_LFO_FREQ: usize = 8;
const C_LFO_AMP: usize = 9;
const C_SET_LFO_PHASE: usize = 10;
const C_EXP: usize = 11;
const C_ROLLOFF: usize = 12;
const C_LFO_FREQ_UNITS: usize = 13;
const C_LFO_TYPE: usize = 14;

const MODE_HQ: i32 = 0;
const MODE_HQ_MONO: i32 = 1;
const MODE_LQ: i32 = 2;
const MODE_LQ_MONO: i32 = 3;

#[derive(Clone, Copy, Default)]
struct FChan {
    d1: f32,
    d2: f32,
    d3: f32,
}

#[derive(Default)]
pub(crate) struct Filter {
    fchan: [FChan; OUTPUTS * FILTERS],
    tick_counter: i32,
    lfo_phase: i32,
    lfo_rand: i32,
    lfo_rand_prev: i32,
    floating_volume: i32,
    floating_cutoff: i32,
    floating_resonance: i32,
    impulse: i32,
    empty_frames_counter: i32,
    empty_frames_counter_max: i32,
    tick_size: i32,
    ticks_per_line: u8,
}

impl Filter {
    fn set_floating_values(&mut self, net: &Net, n: usize) {
        let m = &net.mods[n];
        self.floating_volume = m.ctl(C_VOLUME) * 256;
        let cut = m.ctl(C_CUTOFF);
        self.floating_cutoff = if m.ctl(C_EXP) != 0 { ((cut * cut) / 14000) * 256 } else { cut * 256 };
        self.floating_resonance = m.ctl(C_RESONANCE) * 256;
    }

    /// SET_PHASE: the LFO's phase from its controller, through the handler.
    fn set_phase(&mut self, n: usize, net: &mut Net, check_version: bool) {
        if !check_version || net.base_host_version >= 0x01090300 {
            let mut e = Event::new(Cmd::SetGlobalController);
            e.set_ctl_num(10);
            e.set_ctl_val((net.mods[n].ctl(C_SET_LFO_PHASE) << 7) as u16);
            self.handle(n, &mut e, net);
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
            net.set_number_of_inputs(2, n);
        }
        let srate = net.sampling_freq;
        let base = net.base_host_version;
        let m = &net.mods[n];
        let (frames, offset) = (m.frames, m.offset);
        let outputs_num = m.output_channels;
        let c = |i: usize| m.ctl(i);
        let (ctl_volume, ctl_cutoff, ctl_resonance, ctl_type, ctl_response, ctl_mix, ctl_lfo_freq, ctl_lfo_amp, ctl_exp, ctl_rolloff, ctl_units, ctl_lfo_type) =
            (c(C_VOLUME), c(C_CUTOFF), c(C_RESONANCE), c(C_TYPE), c(C_RESPONSE), c(C_MIX), c(C_LFO_FREQ), c(C_LFO_AMP), c(C_EXP), c(C_ROLLOFF), c(C_LFO_FREQ_UNITS), c(C_LFO_TYPE));
        let mut input_signal = false;
        for ch in 0..outputs_num.max(0) as usize {
            if m.in_empty[ch] < offset + frames {
                input_signal = true;
                break;
            }
        }
        if input_signal {
            self.empty_frames_counter = 0;
        } else if self.empty_frames_counter < self.empty_frames_counter_max {
            self.empty_frames_counter += frames;
            input_signal = true;
        }
        let tick_size = match ctl_units {
            0 => (srate / 200) * 256,
            1 | 2 => (srate * 256) / 1000,
            _ => (srate * 256) / 200,
        };
        if self.impulse != 0 {
            self.floating_cutoff = if ctl_exp != 0 { ((self.impulse * self.impulse) / 14000) * 256 } else { self.impulse * 256 };
            self.impulse = 0;
        }
        let m = &mut net.mods[n];
        let mut ptr = 0i32;
        loop {
            let mut buf_size = frames - ptr;
            let left = tick_size - self.tick_counter;
            if buf_size > left / 256 {
                buf_size = left / 256;
            }
            if left & 255 != 0 {
                buf_size += 1;
            }
            if buf_size > frames - ptr {
                buf_size = frames - ptr;
            }
            if buf_size < 0 {
                buf_size = 0;
            }
            if input_signal {
                let rolloff_steps = ctl_rolloff + 1;
                let vol = self.floating_volume / 256;
                let fvol: f32 = if base >= 0x01080000 { (vol * ctl_mix) as f32 / 256.0 / 256.0 } else { ((vol * ctl_mix) / 256) as f32 / 256.0 };
                for ch in 0..outputs_num.max(0) as usize {
                    let mut fvol2 = fvol;
                    let (chin, chout) = (&m.chin, &mut m.chout);
                    let inp = &chin[ch][offset as usize..];
                    let out = &mut chout[ch][offset as usize..];
                    for fnum in 0..rolloff_steps {
                        if base >= 0x02000000 {
                            fvol2 = if fnum < rolloff_steps - 1 { 256.0 / 256.0 } else { fvol };
                        }
                        let mut floating_resonance = self.floating_resonance / 256;
                        if fnum < rolloff_steps - 1 {
                            floating_resonance = 0;
                        }
                        let (cc, f, q);
                        if mode == MODE_HQ || mode == MODE_HQ_MONO {
                            cc = ((self.floating_cutoff / 256) * 32768) / (srate * 2);
                        } else {
                            cc = (((self.floating_cutoff / 256) / 2) * 32768) / srate;
                        }
                        let mut ff = (6433 * cc) / 32768;
                        if ff >= 1024 {
                            ff = 1023;
                        }
                        f = ff;
                        q = 1536 - floating_resonance;
                        let fch = &mut self.fchan[OUTPUTS * fnum as usize + ch];
                        if ctl_mix > 0 {
                            let (mut d1, mut d2, mut d3) = (fch.d1, fch.d2, fch.d3);
                            let ff = f as f32;
                            let qf = q as f32;
                            let range = ptr as usize..(ptr + buf_size) as usize;
                            let mut dont_render = false;
                            if f == 0 {
                                if ctl_type == 0 {
                                    for i in range.clone() {
                                        let outp = d2;
                                        d2 = (d2 * 255.0) / 256.0;
                                        out[i] = outp * fvol2;
                                    }
                                    dont_render = true;
                                }
                                if ctl_type == 2 {
                                    for i in range.clone() {
                                        let outp = d1;
                                        d1 = (d1 * 255.0) / 256.0;
                                        out[i] = outp * fvol2;
                                    }
                                    dont_render = true;
                                }
                            }
                            if !dont_render {
                                // fnum > 0 filters what the previous stage wrote.
                                let hq = mode == MODE_HQ || mode == MODE_HQ_MONO;
                                for i in range {
                                    let x = if fnum > 0 { out[i] } else { inp[i] };
                                    let mut low = d2 + (ff * d1) / 1024.0;
                                    let mut high = x - low - (qf * d1) / 1024.0;
                                    let mut band = (ff * high) / 1024.0 + d1;
                                    let outp;
                                    if hq {
                                        let (slow, shigh, sband, snotch) = (low, high, band, high + low);
                                        low += (ff * band) / 1024.0;
                                        high = x - low - (qf * band) / 1024.0;
                                        band += (ff * high) / 1024.0;
                                        outp = match ctl_type {
                                            0 => (d2 + slow) / 2.0,
                                            1 => (d3 + shigh) / 2.0,
                                            2 => (d1 + sband) / 2.0,
                                            _ => (d3 + snotch) / 2.0,
                                        };
                                        match ctl_type {
                                            1 => d3 = high,
                                            3 => d3 = high + low,
                                            _ => {}
                                        }
                                    } else {
                                        outp = match ctl_type {
                                            0 => low,
                                            1 => high,
                                            2 => band,
                                            _ => high + low,
                                        };
                                    }
                                    d1 = band;
                                    d2 = low;
                                    out[i] = outp * fvol2;
                                }
                            }
                            fch.d1 = d1;
                            fch.d2 = d2;
                            fch.d3 = d3;
                        }
                    }
                    let range = ptr as usize..(ptr + buf_size) as usize;
                    if ctl_mix == 0 {
                        if vol == 256 {
                            out[range.clone()].copy_from_slice(&inp[range]);
                        } else if vol == 0 {
                            out[range].fill(0.0);
                        } else {
                            for i in range {
                                out[i] = inp[i] * vol as f32 / 256.0;
                            }
                        }
                    } else if ctl_mix > 0 && ctl_mix < 256 {
                        let mut vol2 = (self.floating_volume / 256) as f32;
                        vol2 *= (256 - ctl_mix) as f32;
                        if base >= 0x01080000 {
                            vol2 /= 256.0;
                        } else {
                            vol2 = (vol2 as i32 / 256) as f32;
                        }
                        for i in range {
                            let mut v = inp[i];
                            v *= vol2;
                            v /= 256.0;
                            v += out[i];
                            out[i] = v;
                        }
                    }
                }
            }
            ptr += buf_size;
            self.tick_counter += buf_size * 256;
            if self.tick_counter >= tick_size {
                let mut cut = ctl_cutoff;
                if ctl_exp != 0 {
                    cut = (cut * cut) / 14000;
                }
                if ctl_lfo_amp != 0 {
                    let lfo_phase = match ctl_units {
                        0 => (self.lfo_phase / 16) & 511,
                        1 | 2 => (self.lfo_phase / 1000) & 511,
                        3 => (self.lfo_phase / self.tick_size) & 511,
                        _ => (self.lfo_phase / ((self.tick_size * self.ticks_per_line as i32) / 8)) & 511,
                    };
                    let mut lfo_value = match ctl_lfo_type {
                        0 => {
                            if lfo_phase < 256 {
                                HSIN_TAB[lfo_phase as usize] as i32
                            } else {
                                -(HSIN_TAB[(lfo_phase & 255) as usize] as i32)
                            }
                        }
                        1 => lfo_phase - 255,
                        2 => -lfo_phase + 255,
                        3 => {
                            if lfo_phase < 256 {
                                255
                            } else {
                                -255
                            }
                        }
                        4 => {
                            if self.lfo_rand_prev != lfo_phase / 256 {
                                self.lfo_rand_prev = lfo_phase / 256;
                                self.lfo_rand = (net.globals.pseudo_random() & 511) as i32;
                            }
                            self.lfo_rand - 255
                        }
                        _ => 0,
                    };
                    lfo_value *= 14000 / 2;
                    lfo_value /= 256;
                    lfo_value *= ctl_lfo_amp;
                    lfo_value /= 256;
                    let mut lfo_offset = 0;
                    let lfo_max = (ctl_lfo_amp * (14000 / 2)) / 256;
                    if cut - lfo_max < 0 {
                        lfo_offset = -(cut - lfo_max);
                    }
                    if cut + lfo_max > 14000 {
                        lfo_offset = -((cut + lfo_max) - 14000);
                    }
                    cut += lfo_value + lfo_offset;
                }
                if self.floating_cutoff / 256 > cut {
                    self.floating_cutoff -= ctl_response * 14000;
                    if self.floating_cutoff / 256 < cut {
                        self.floating_cutoff = cut * 256;
                    }
                } else if self.floating_cutoff / 256 < cut {
                    self.floating_cutoff += ctl_response * 14000;
                    if self.floating_cutoff / 256 > cut {
                        self.floating_cutoff = cut * 256;
                    }
                }
                if self.floating_resonance / 256 > ctl_resonance {
                    self.floating_resonance -= ctl_response * 1530;
                    if self.floating_resonance / 256 < ctl_resonance {
                        self.floating_resonance = ctl_resonance * 256;
                    }
                } else if self.floating_resonance / 256 < ctl_resonance {
                    self.floating_resonance += ctl_response * 1530;
                    if self.floating_resonance / 256 > ctl_resonance {
                        self.floating_resonance = ctl_resonance * 256;
                    }
                }
                if self.floating_volume / 256 > ctl_volume {
                    self.floating_volume -= ctl_response * 256;
                    if self.floating_volume / 256 < ctl_volume {
                        self.floating_volume = ctl_volume * 256;
                    }
                } else if self.floating_volume / 256 < ctl_volume {
                    if base == 0x01070000 {
                        self.floating_volume += ctl_volume * 256;
                    } else {
                        self.floating_volume += ctl_response * 256;
                    }
                    if self.floating_volume / 256 > ctl_volume {
                        self.floating_volume = ctl_volume * 256;
                    }
                }
                self.tick_counter %= tick_size;
                match ctl_units {
                    0 => {
                        self.lfo_phase += ctl_lfo_freq;
                        self.lfo_phase &= 8191;
                    }
                    1 => {
                        if ctl_lfo_freq > 0 {
                            self.lfo_phase += (1000 * 512) / ctl_lfo_freq;
                            self.lfo_phase %= 1000 * 512;
                        }
                    }
                    2 => {
                        self.lfo_phase += ctl_lfo_freq * 512;
                        self.lfo_phase %= 1000 * 512;
                    }
                    3 => {
                        if ctl_lfo_freq != 0 {
                            self.lfo_phase += (tick_size * 512) / ctl_lfo_freq;
                            self.lfo_phase %= self.tick_size * 512;
                        }
                    }
                    _ => {
                        if ctl_lfo_freq != 0 {
                            self.lfo_phase += (((tick_size / 8) * 512) / ctl_lfo_freq) * (ctl_units - 3);
                            self.lfo_phase %= ((self.tick_size * self.ticks_per_line as i32) / 8) * 512;
                        }
                    }
                }
            }
            if ptr >= frames {
                break;
            }
        }
        input_signal as isize
    }
}

impl Module for Filter {
    fn handle(&mut self, n: usize, e: &mut Event, net: &mut Net) -> isize {
        let mut retval: isize = 0;
        match e.cmd {
            Cmd::Init => {
                net.register_ctl(n, 0, 256, 256, 0, C_VOLUME, -1, 0);
                net.register_ctl(n, 0, 14000, 14000, 0, C_CUTOFF, -1, 0);
                net.set_ctl_flags(n, 1, 2);
                net.register_ctl(n, 0, 1530, 0, 0, C_RESONANCE, -1, 0);
                net.register_ctl(n, 0, 3, 0, 1, C_TYPE, -1, 0);
                net.register_ctl(n, 0, 256, 8, 0, C_RESPONSE, -1, 1);
                net.register_ctl(n, 0, MODE_LQ_MONO, MODE_HQ, 1, C_MODE, -1, 1);
                net.register_ctl(n, 0, 14000, 0, 0, C_IMPULSE, -1, 1);
                net.register_ctl(n, 0, 256, 256, 0, C_MIX, -1, 1);
                net.register_ctl(n, 0, 1024, 8, 0, C_LFO_FREQ, -1, 2);
                net.register_ctl(n, 0, 256, 0, 0, C_LFO_AMP, -1, 2);
                net.register_ctl(n, 0, 256, 0, 0, C_SET_LFO_PHASE, -1, 2);
                net.register_ctl(n, 0, 1, 0, 1, C_EXP, -1, 0);
                net.register_ctl(n, 0, 3, 0, 1, C_ROLLOFF, -1, 0);
                net.register_ctl(n, 0, 6, 0, 1, C_LFO_FREQ_UNITS, -1, 2);
                net.register_ctl(n, 0, 4, 0, 1, C_LFO_TYPE, -1, 2);
                self.set_floating_values(net, n);
                self.fchan = Default::default();
                self.tick_counter = 0;
                self.lfo_phase = 0;
                self.lfo_rand = 0;
                self.lfo_rand_prev = 0;
                self.impulse = 0;
                self.empty_frames_counter_max = net.sampling_freq * 2;
                self.empty_frames_counter = self.empty_frames_counter_max;
                retval = 1;
            }
            Cmd::SetupFinished => {
                self.set_phase(n, net, true);
                self.set_floating_values(net, n);
                retval = 1;
            }
            Cmd::ApplyControllers => {
                self.set_floating_values(net, n);
                retval = 1;
            }
            Cmd::Clean => {
                self.fchan = Default::default();
                self.tick_counter = 0;
                self.empty_frames_counter = self.empty_frames_counter_max;
                self.set_phase(n, net, true);
                retval = 1;
            }
            Cmd::RenderReplace => retval = self.render(n, net),
            Cmd::SetLocalController | Cmd::SetGlobalController => match e.ctl_num() {
                10 => {
                    let v = e.ctl_val() as i32;
                    let units = net.mods[n].ctl(C_LFO_FREQ_UNITS);
                    self.lfo_phase = match units {
                        0 => (v / 4) & 8191,
                        1 | 2 => ((v / 32) * (1000 * 512)) / 1024,
                        3 => (((v / 32) * self.tick_size) / 1024) * 512,
                        _ => (((v / 32) * ((self.tick_size * self.ticks_per_line as i32) / 8)) / 1024) * 512,
                    };
                }
                6 => {
                    let max = net.mods[n].ctls.get(6).map_or(0, |c| c.max);
                    self.impulse = (e.ctl_val() as i32 * max) / 32768;
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
                retval = 1;
            }
            Cmd::Close => retval = 1,
            _ => {}
        }
        retval
    }
}
