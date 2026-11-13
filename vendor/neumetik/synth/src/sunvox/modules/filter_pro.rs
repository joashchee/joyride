// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! Filter Pro (psynths_filter2.cpp): the 64-bit biquad (eleven types, up
//! to eight stages), its frequency, Q, gain, volume and mix eased toward
//! their controllers every tick of 5 or 1 ms, the frequency swept by an
//! LFO in one of seven units.

use super::super::dsp::{bq_type, Biquad, BQ_FLAG_USE_STAGES_FOR_APF, BQ_Q_IS_Q, HSIN_TAB};
use super::super::net::{self, Cmd, Event, Module, Net};

const OUTPUTS: usize = 2;
const MAX_FREQ: i32 = 22000;
const MAX_Q: i32 = 32768;
const MAX_GAIN: i32 = 32768;
const MAX_VOLUME: i32 = 32768;
const MAX_ROLLOFF: i32 = 8 - 1;
const MAX_RESPONSE: i32 = 1000;
const BIQUAD_TYPES: i32 = 11;

const C_VOLUME: usize = 0;
const C_TYPE: usize = 1;
const C_FREQ: usize = 2;
const C_FREQ_FINE: usize = 3;
const C_FREQ_SCALE: usize = 4;
const C_EXP_FREQ: usize = 5;
const C_Q: usize = 6;
const C_GAIN: usize = 7;
const C_ROLLOFF: usize = 8;
const C_RESPONSE: usize = 9;
const C_MODE: usize = 10;
const C_MIX: usize = 11;
const C_LFO_FREQ: usize = 12;
const C_LFO_AMP: usize = 13;
const C_LFO_TYPE: usize = 14;
const C_SET_LFO_PHASE: usize = 15;
const C_LFO_FREQ_UNITS: usize = 16;

#[derive(Default)]
pub(crate) struct FilterPro {
    f: Option<Biquad>,
    tick_counter: i32,
    first_reinit_without_interp: bool,
    lfo_phase: i32,
    lfo_rand: i32,
    lfo_rand_prev: i32,
    floating_freq: f32,
    floating_q: f32,
    floating_gain: f32,
    floating_vol: f32,
    floating_mix: f32,
    empty: bool,
    empty_frames_counter: i32,
    empty_frames_counter_max: i32,
    host_tick_size: i32,
    ticks_per_line: u8,
}

fn get_freq(m: &net::ModCommon) -> f32 {
    let mut f = m.ctl(C_FREQ) as f32 + (m.ctl(C_FREQ_FINE) as f32 / 1000.0 - 1.0);
    if m.ctl(C_EXP_FREQ) != 0 {
        f = f * f / MAX_FREQ as f32;
    }
    f * m.ctl(C_FREQ_SCALE) as f32 / 100.0
}

impl FilterPro {
    fn get_freq_with_lfo(&mut self, m: &net::ModCommon, net_rand: &net::Globals) -> f32 {
        let mut freq = get_freq(m);
        let ctl_lfo_amp = m.ctl(C_LFO_AMP);
        if ctl_lfo_amp != 0 {
            let div = |d: i32| if d != 0 { self.lfo_phase / d } else { 0 };
            let mut lfo_phase = match m.ctl(C_LFO_FREQ_UNITS) {
                0 => self.lfo_phase / 16,
                1 | 2 => self.lfo_phase / 1000,
                3 => div(self.host_tick_size),
                4..=6 => div((self.host_tick_size * self.ticks_per_line as i32) / 8),
                _ => 0,
            };
            lfo_phase &= 511;
            let v = match m.ctl(C_LFO_TYPE) {
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
                        self.lfo_rand = (net_rand.pseudo_random() & 511) as i32;
                    }
                    self.lfo_rand - 255
                }
                _ => 0,
            };
            let lfo_amp = ctl_lfo_amp as f32 / MAX_VOLUME as f32;
            let lfo_value = ((v as f32 * (MAX_FREQ / 2) as f32) / 256.0) * lfo_amp;
            let mut lfo_offset = 0.0f32;
            let lfo_max = lfo_amp * (MAX_FREQ / 2) as f32;
            if freq - lfo_max < 0.0 {
                lfo_offset = -(freq - lfo_max);
            }
            if freq + lfo_max > MAX_FREQ as f32 {
                lfo_offset = -((freq + lfo_max) - MAX_FREQ as f32);
            }
            freq += lfo_value + lfo_offset;
        }
        freq
    }

    fn set_floating_values(&mut self, m: &net::ModCommon, g: &net::Globals) {
        self.floating_freq = self.get_freq_with_lfo(m, g);
        self.floating_q = m.ctl(C_Q) as f32;
        self.floating_gain = m.ctl(C_GAIN) as f32;
        self.floating_vol = m.ctl(C_VOLUME) as f32;
        self.floating_mix = m.ctl(C_MIX) as f32;
    }

    fn floating_step(&mut self, m: &net::ModCommon, g: &net::Globals) {
        let r = m.ctl(C_RESPONSE).max(1);
        let response = r as f32 / MAX_RESPONSE as f32;
        let f = self.get_freq_with_lfo(m, g);
        self.floating_freq = (1.0 - response) * self.floating_freq + response * f;
        self.floating_q = (1.0 - response) * self.floating_q + response * m.ctl(C_Q) as f32;
        self.floating_gain = (1.0 - response) * self.floating_gain + response * m.ctl(C_GAIN) as f32;
        self.floating_vol = (1.0 - response) * self.floating_vol + response * m.ctl(C_VOLUME) as f32;
        self.floating_mix = (1.0 - response) * self.floating_mix + response * m.ctl(C_MIX) as f32;
    }

    /// The LFO's phase set (controller 15's handling).
    fn set_phase_from(&mut self, m: &net::ModCommon, val: i32) {
        let hts = self.host_tick_size;
        self.lfo_phase = match m.ctl(C_LFO_FREQ_UNITS) {
            0 => (val / 4) & 8191,
            1 | 2 => ((val / 32) * (1000 * 512)) / 1024,
            3 => (((val / 32) * hts) / 1024) * 512,
            4..=6 => (((val / 32) * ((hts * self.ticks_per_line as i32) / 8)) / 1024) * 512,
            _ => self.lfo_phase,
        };
    }

    /// SET_PHASE: the "set LFO phase" controller applied (from 1.9.3 when `check_version`).
    fn set_phase(&mut self, m: &net::ModCommon, base: u32, check_version: bool) {
        if !check_version || base >= 0x01090300 {
            let v = (m.ctl(C_SET_LFO_PHASE) << 7) as u16 as i32;
            self.set_phase_from(m, v);
        }
    }

    fn render(&mut self, n: usize, net: &mut Net) -> isize {
        let mode = net.mods[n].ctl(C_MODE);
        let mut channels_changed = false;
        if mode & 1 == 1 {
            if net.mods[n].output_channels != 1 {
                net.set_number_of_outputs(1, n);
                net.set_number_of_inputs(1, n);
                channels_changed = true;
            }
        } else if net.mods[n].output_channels != OUTPUTS as i32 {
            net.set_number_of_outputs(OUTPUTS as i32, n);
            net.set_number_of_inputs(OUTPUTS as i32, n);
            channels_changed = true;
        }
        let Some(mut f) = self.f.take() else { return 0 };
        if channels_changed {
            f.stop();
        }
        let srate = net.sampling_freq;
        let globals = net.globals.clone();
        let m = &mut net.mods[n];
        let (offset, frames) = (m.offset as usize, m.frames as usize);
        let outputs_num = m.output_channels.clamp(0, 2) as usize;
        let input_signal = (0..outputs_num).any(|ch| m.in_empty[ch] < (offset + frames) as i32);
        let mut check_filter_output = false;
        let mut finished = true;
        if input_signal {
            self.empty = false;
            self.empty_frames_counter = 0;
        } else if !self.empty {
            if self.empty_frames_counter < self.empty_frames_counter_max {
                self.empty_frames_counter += frames as i32;
            } else {
                check_filter_output = true;
                self.empty_frames_counter = 0;
            }
        }
        let units = m.ctl(C_LFO_FREQ_UNITS);
        let tick_size = if units == 1 || units == 2 { (srate * 256) / 1000 } else { (srate * 256) / 200 };
        let ctl_response = m.ctl(C_RESPONSE);
        if ctl_response == MAX_RESPONSE {
            self.floating_step(m, &globals);
        }
        let (ctl_type, ctl_rolloff, ctl_lfo_freq) = (m.ctl(C_TYPE), m.ctl(C_ROLLOFF), m.ctl(C_LFO_FREQ));
        let mut retval = 0;
        let mut ptr = 0usize;
        loop {
            let mut size = (frames - ptr) as i32;
            if size > (tick_size - self.tick_counter) / 256 {
                size = (tick_size - self.tick_counter) / 256;
            }
            if (tick_size - self.tick_counter) & 255 != 0 {
                size += 1;
            }
            if size > (frames - ptr) as i32 {
                size = (frames - ptr) as i32;
            }
            if size < 0 {
                size = 0;
            }
            let size = size as usize;
            let mut q = (self.floating_q / (MAX_Q / 2) as f32) as f64;
            q *= q;
            q *= q;
            let mut gain = (self.floating_gain - (MAX_GAIN / 2) as f32) as f64;
            gain = gain / (MAX_GAIN / 2) as f64 * 24.0;
            let mut interp_len = tick_size / 256;
            if ctl_response == MAX_RESPONSE {
                interp_len = 0;
            }
            if self.first_reinit_without_interp {
                self.first_reinit_without_interp = false;
                interp_len = 0;
            }
            f.change(interp_len, bq_type(ctl_type as u32, BQ_Q_IS_Q, ctl_rolloff + 1, mode & 2 != 0), srate, self.floating_freq as f64, gain, q);
            if !self.empty {
                let vol = self.floating_vol as i32;
                let mix = self.floating_mix as i32;
                for ch in 0..outputs_num {
                    let (ci, co) = (&m.chin, &mut m.chout);
                    let inp = &ci[ch][offset + ptr..offset + ptr + size];
                    let out = &mut co[ch][offset + ptr..offset + ptr + size];
                    f.run(ch, Some(inp), out, size);
                    if check_filter_output && finished && out.iter().any(|&v| net::is_not_min(v)) {
                        finished = false;
                    }
                    if vol == 0 {
                        out.fill(0.0);
                    } else {
                        if mix == 0 {
                            out.copy_from_slice(inp);
                        } else if mix != MAX_VOLUME {
                            let vol2 = mix as f32;
                            let vol3 = (MAX_VOLUME - mix) as f32;
                            for (o, &x) in out.iter_mut().zip(inp.iter()) {
                                *o = (x * vol3 + *o * vol2) / MAX_VOLUME as f32;
                            }
                        }
                        if vol != MAX_VOLUME {
                            for o in out.iter_mut() {
                                *o = (*o * vol as f32) / MAX_VOLUME as f32;
                            }
                        }
                    }
                }
                if vol != 0 {
                    retval = 1;
                }
            }
            ptr += size;
            self.tick_counter += size as i32 * 256;
            if self.tick_counter >= tick_size {
                self.floating_step(m, &globals);
                self.tick_counter %= tick_size;
                let hts = self.host_tick_size;
                let tpl = self.ticks_per_line as i32;
                match units {
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
                        if ctl_lfo_freq != 0 && hts != 0 {
                            self.lfo_phase += (tick_size * 512) / ctl_lfo_freq;
                            self.lfo_phase %= hts * 512;
                        }
                    }
                    4..=6 => {
                        let period = ((hts * tpl) / 8) * 512;
                        if ctl_lfo_freq != 0 && period != 0 {
                            self.lfo_phase += (((tick_size / 8) * 512) / ctl_lfo_freq) * (units - 3);
                            self.lfo_phase %= period;
                        }
                    }
                    _ => {}
                }
            }
            if ptr >= frames {
                break;
            }
        }
        if check_filter_output && finished {
            self.empty = true;
            self.empty_frames_counter = 0;
        }
        self.f = Some(f);
        retval
    }
}

impl Module for FilterPro {
    fn handle(&mut self, n: usize, e: &mut Event, net: &mut Net) -> isize {
        let mut retval: isize = 0;
        match e.cmd {
            Cmd::Init => {
                net.register_ctl(n, 0, MAX_VOLUME, MAX_VOLUME, 0, C_VOLUME, -1, 0);
                net.register_ctl(n, 0, BIQUAD_TYPES - 1, 0, 1, C_TYPE, -1, 0);
                net.register_ctl(n, 0, MAX_FREQ, MAX_FREQ, 0, C_FREQ, -1, 0);
                net.set_ctl_flags(n, 2, 2);
                net.register_ctl(n, 0, 2000, 1000, 0, C_FREQ_FINE, 1000, 0);
                net.set_ctl_show_offset(n, 3, -1000);
                net.register_ctl(n, 0, 200, 100, 0, C_FREQ_SCALE, 100, 0);
                net.register_ctl(n, 0, 1, 0, 1, C_EXP_FREQ, -1, 0);
                net.register_ctl(n, 0, MAX_Q, MAX_Q / 2, 0, C_Q, MAX_Q / 2, 0);
                net.register_ctl(n, 0, MAX_GAIN, MAX_GAIN / 2, 0, C_GAIN, MAX_GAIN / 2, 0);
                net.set_ctl_show_offset(n, 7, -MAX_GAIN / 2);
                net.register_ctl(n, 0, MAX_ROLLOFF, 0, 1, C_ROLLOFF, -1, 0);
                net.register_ctl(n, 0, MAX_RESPONSE, (MAX_RESPONSE as f64 * 0.25) as i32, 0, C_RESPONSE, -1, 1);
                net.register_ctl(n, 0, 3, 0, 1, C_MODE, -1, 1);
                net.register_ctl(n, 0, MAX_VOLUME, MAX_VOLUME, 0, C_MIX, -1, 1);
                net.register_ctl(n, 0, 1024, 8, 0, C_LFO_FREQ, -1, 2);
                net.register_ctl(n, 0, MAX_VOLUME, 0, 0, C_LFO_AMP, -1, 2);
                net.register_ctl(n, 0, 4, 0, 1, C_LFO_TYPE, -1, 2);
                net.register_ctl(n, 0, 256, 0, 0, C_SET_LFO_PHASE, -1, 2);
                net.register_ctl(n, 0, 6, 0, 1, C_LFO_FREQ_UNITS, -1, 2);
                let fflags = if net.base_host_version >= 0x02010200 { BQ_FLAG_USE_STAGES_FOR_APF } else { 0 };
                self.f = Some(Biquad::new(fflags));
                let g = net.globals.clone();
                self.set_floating_values(&net.mods[n], &g);
                self.tick_counter = 0;
                self.lfo_phase = 0;
                self.lfo_rand = 0;
                self.lfo_rand_prev = 0;
                self.empty = true;
                self.empty_frames_counter_max = net.sampling_freq * 2;
                retval = 1;
            }
            Cmd::SetupFinished => {
                let base = net.base_host_version;
                self.set_phase(&net.mods[n], base, true);
                let g = net.globals.clone();
                self.set_floating_values(&net.mods[n], &g);
                retval = 1;
            }
            Cmd::Clean => {
                if let Some(f) = self.f.as_mut() {
                    f.stop();
                }
                self.tick_counter = 0;
                self.empty = true;
                self.empty_frames_counter = 0;
                let base = net.base_host_version;
                self.set_phase(&net.mods[n], base, true);
                retval = 1;
            }
            Cmd::RenderReplace => retval = self.render(n, net),
            Cmd::SetLocalController | Cmd::SetGlobalController => {
                if e.ctl_num() == 15 {
                    self.set_phase_from(&net.mods[n], e.ctl_val() as i32);
                }
            }
            Cmd::ApplyControllers => {
                let g = net.globals.clone();
                self.set_floating_values(&net.mods[n], &g);
                self.first_reinit_without_interp = true;
                retval = 1;
            }
            Cmd::NoteOn => {
                let base = net.base_host_version;
                self.set_phase(&net.mods[n], base, false);
                retval = 1;
            }
            Cmd::SpeedChanged => {
                self.host_tick_size = e.tick_size();
                self.ticks_per_line = e.tpl();
                retval = 1;
            }
            Cmd::Close => retval = 1,
            _ => {}
        }
        retval
    }
}
