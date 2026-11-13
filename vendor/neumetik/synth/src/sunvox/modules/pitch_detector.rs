// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! Pitch Detector (psynths_pitch_detector.cpp): its input's pitch found
//! (zero crossings; YIN through FFT autocorrelation; cepstrum; the
//! spectrum's peak) after a low-pass, and sent on as notes to the
//! modules it links to.

use super::super::dsp::{fft_f32, Resampler, FFT_FLAG_INVERSE, RESAMP_FLAG_MODE2};
use super::super::net::{self, Cmd, Event, Module, Net, RT_MUTE};

const RATES: [i32; 3] = [12000, 24000, 44100];
const HIST_SIZE: usize = 16;
const FILTERS: usize = 4;
const FILTER_STATE_VARS: usize = 2;
const NOTE0_FREQ: f32 = 16.333984375;
/// SUNVOX_VIRTUAL_PATTERN_PITCH_DETECTOR.
const VIRTUAL_PATTERN_PITCH_DETECTOR: u32 = 0xFFFF - 3;

const C_ALG: usize = 0;
const C_THRESHOLD: usize = 1;
const C_VEL_GAIN: usize = 2;
const C_MICROTONES: usize = 3;
const C_DETECTOR_FINETUNE: usize = 4;
const C_LP_FREQ: usize = 5;
const C_LP_ROLLOFF: usize = 6;
const C_SRATE: usize = 7;
const C_BUF_SIZE: usize = 8;
const C_BUF_OVERLAP: usize = 9;
const C_ABS_THRESHOLD: usize = 10;
const C_REC: usize = 11;

#[derive(Clone, Copy, Default)]
struct Estimate {
    t: f32,
    v: f32,
    frame: u32,
}

#[derive(Clone, Copy, PartialEq)]
enum Send {
    On,
    Off,
    Change,
    Vel,
}

#[derive(Default)]
pub(crate) struct PitchDetector {
    prev_smp: f32,
    period_add: f32,
    period: i32,
    period_amp: f32,
    period_cnt: i32,
    max_period: i32,
    srate: i32,
    buf_size: i32,
    buf_size_scaled: i32,
    buf_hop: i32,
    buf: Vec<f32>,
    buf_ptr: i32,
    energy_terms: Vec<f32>,
    fft_i1: Vec<f32>,
    fft_i2: Vec<f32>,
    fft_r1: Vec<f32>,
    fft_r2: Vec<f32>,
    fft_win: Vec<f32>,
    fft_win_type: i32,
    hist: [Estimate; HIST_SIZE],
    hist_ptr: i32,
    lp_state: [f32; FILTER_STATE_VARS * FILTERS],
    out: Option<Event>,
    empty: bool,
    freq_detected: bool,
    playing: bool,
    pitch: i32,
    freq: f32,
    vel: i32,
    frame_cnt: u32,
    resamp: Option<Resampler>,
    temp: Vec<f32>,
}

impl PitchDetector {
    fn send(&mut self, n: usize, net: &mut Net, offset: i32, cmd: Send) {
        let mut e = self.out.unwrap_or_else(|| Event::new(Cmd::NoteOn));
        e.offset = offset;
        e.id = (VIRTUAL_PATTERN_PITCH_DETECTOR << 16) | n as u32;
        e.set_velocity(self.vel);
        match cmd {
            Send::On => {
                self.playing = true;
                e.cmd = Cmd::NoteOn;
            }
            Send::Off => {
                self.playing = false;
                e.cmd = Cmd::NoteOff;
            }
            Send::Change => e.cmd = Cmd::SetFreq,
            Send::Vel => e.cmd = Cmd::SetVelocity,
        }
        if cmd != Send::Vel {
            net.multisend_pitch(n, &mut e, net::NOTE0_PITCH - self.pitch);
        } else {
            net.multisend(n, &e);
        }
        self.out = Some(e);
    }

    fn reinit_buffers(&mut self, n: usize, net: &Net) {
        let m = &net.mods[n];
        self.srate = RATES[m.ctl(C_SRATE).clamp(0, 2) as usize];
        match self.resamp.as_mut() {
            None => {
                let mut r = Resampler::new(net.max_buf_size, net.sampling_freq, net.sampling_freq, self.srate, 0, RESAMP_FLAG_MODE2);
                r.input_buf(0);
                self.resamp = Some(r);
            }
            Some(r) => {
                if r.out_smprate() != self.srate {
                    r.change(net.sampling_freq, self.srate, 0, RESAMP_FLAG_MODE2);
                    r.input_buf(0);
                }
            }
        }
        let fft_win_type = if m.ctl(C_ALG) == 3 { 2 } else { 1 };
        let mut buf_size = 256 << m.ctl(C_BUF_SIZE).clamp(0, 4);
        if self.srate <= 24000 {
            buf_size /= 2;
        }
        if self.srate <= 12000 {
            buf_size /= 2;
        }
        let mut buf_hop = buf_size * (100 - m.ctl(C_BUF_OVERLAP)) / 100;
        if buf_hop < 8 {
            buf_hop = 8;
        }
        self.buf_size_scaled = ((buf_size as u64 * self.resamp.as_ref().unwrap().ratio_fp as u64) / 65536) as i32;
        self.buf_hop = buf_hop;
        let size = buf_size as usize;
        if size > self.buf.len() {
            self.buf.resize(size, 0.0);
            self.energy_terms.resize(size / 2, 0.0);
            self.fft_i1.resize(size, 0.0);
            self.fft_r1.resize(size, 0.0);
            self.fft_i2.resize(size, 0.0);
            self.fft_r2.resize(size, 0.0);
            self.fft_win.resize(size, 0.0);
        }
        if self.buf_size != buf_size || self.fft_win_type != fft_win_type {
            self.buf_ptr = 0;
            self.buf_size = buf_size;
            self.fft_win_type = fft_win_type;
            if fft_win_type == 1 {
                for i in 0..size {
                    self.fft_win[i] = (std::f64::consts::PI * i as f64 / (buf_size - 1) as f64).sin() as f32;
                }
            } else {
                let sigma = 1.0f32 / (8.0 / 2.0);
                let n1 = (buf_size - 1) as f32 / 2.0;
                for i in 0..size {
                    let n2 = (i as f32 - n1) / (sigma * n1);
                    self.fft_win[i] = (-0.5f32 * n2 * n2).exp();
                }
            }
        }
    }

    fn get_t(&self, v: &[f32], t: i32) -> f32 {
        let x0 = (t - 1).max(0);
        let x1 = t;
        let x2 = if t + 1 >= self.buf_size / 2 { x1 } else { t + 1 };
        let (v0, v1, v2) = (v[x0 as usize], v[x1 as usize], v[x2 as usize]);
        let correction = (v2 - v0) / (2.0 * (2.0 * v1 - v2 - v0));
        t as f32 + correction
    }

    fn get_pitch(&self, m: &net::ModCommon, freq: f32) -> i32 {
        // LOG2 of a float: C++'s float overload, log2f.
        let mut pitch = ((freq / NOTE0_FREQ).log2() * 256.0 * 12.0 - (m.ctl(C_DETECTOR_FINETUNE) - 256) as f32) as i32;
        if m.ctl(C_MICROTONES) == 0 {
            pitch = ((pitch + 128) / 256) * 256;
        }
        pitch + m.relative_note * 256 + m.finetune
    }

    fn reset_state(&mut self) {
        self.buf_ptr = 0;
        for h in self.hist.iter_mut() {
            h.t = 0.0;
        }
        self.prev_smp = 0.0;
        self.period = 0;
        self.period_amp = 0.0;
        self.lp_state = [0.0; FILTER_STATE_VARS * FILTERS];
    }

    fn render(&mut self, n: usize, net: &mut Net) -> isize {
        if net.mods[n].realtime_flags & RT_MUTE != 0 {
            return 0;
        }
        let srate = net.sampling_freq;
        let base = net.base_host_version;
        let m = &net.mods[n];
        let (offset, frames) = (m.offset as usize, m.frames as usize);
        self.frame_cnt = self.frame_cnt.wrapping_add(frames as u32);
        let input_signal = m.in_empty[0] < (offset + frames) as i32;
        if !input_signal {
            if !self.empty {
                if self.playing {
                    let off = m.offset;
                    self.send(n, net, off, Send::Off);
                }
                self.reset_state();
                if let Some(r) = self.resamp.as_mut() {
                    if r.state() != 0 {
                        r.reset();
                    }
                }
                self.freq_detected = false;
                self.empty = true;
            }
            return 0;
        }
        self.empty = false;
        let m = &net.mods[n];
        let ctl = |c: usize| m.ctl(c);
        let (alg, lp_freq, lp_rolloff) = (ctl(C_ALG), ctl(C_LP_FREQ), ctl(C_LP_ROLLOFF));
        let mod_offset = m.offset;
        let input: Vec<f32> = m.chin[0][offset..offset + frames].to_vec();
        if self.temp.len() < net.max_buf_size as usize {
            self.temp.resize(net.max_buf_size as usize, 0.0);
        }
        let resampling = alg != 0 && self.resamp.as_ref().is_some_and(|r| r.out_smprate() != srate);
        // `work` is where the filtered input goes (the resampler's input, or temp).
        let mut work: Vec<f32> = vec![0.0; frames];
        let mut cur: Vec<f32> = input.clone();
        if lp_freq != 0 {
            let q = 1536.0f32 / 1024.0;
            let f = (2.0 * (std::f64::consts::PI * lp_freq as f64 / srate as f64).sin()) as f32;
            for fnum in 0..(lp_rolloff + 1).clamp(0, FILTERS as i32) as usize {
                let st = &mut self.lp_state[fnum * FILTER_STATE_VARS..fnum * FILTER_STATE_VARS + 2];
                for i in 0..frames {
                    let v = if fnum > 0 { work[i] } else { input[i] };
                    let low = st[1] + f * st[0];
                    let high = v - low - q * st[0];
                    let band = f * high + st[0];
                    st[0] = band;
                    st[1] = low;
                    work[i] = low;
                }
            }
            cur = work.clone();
        } else if resampling {
            work.copy_from_slice(&input);
        }
        let mut in_frames = frames;
        if resampling {
            let r = self.resamp.as_mut().unwrap();
            let off = r.input_buf_offset();
            r.input_buf(0)[off..off + frames].copy_from_slice(&work);
            r.begin(frames as i32, 0);
            let mut outs = [std::mem::take(&mut self.temp), Vec::new()];
            r.end(if input_signal { 1 } else { 2 }, &mut outs, 1, 0);
            in_frames = r.output_frames() as usize;
            self.temp = std::mem::take(&mut outs[0]);
            cur = self.temp[..in_frames.min(self.temp.len())].to_vec();
            in_frames = cur.len();
        } else if lp_freq == 0 {
            cur = input;
        }
        match alg {
            0 => {
                let amp_threshold = (ctl(C_THRESHOLD) as f32 * 1.0) / 10000.0;
                let vel_gain = ctl(C_VEL_GAIN);
                for (i, &v) in cur.iter().enumerate().take(in_frames) {
                    self.period += 1;
                    if self.period > self.max_period {
                        self.period = 0;
                        self.period_cnt = 0;
                        if self.playing {
                            self.send(n, net, mod_offset + i as i32, Send::Off);
                        }
                    }
                    let amp = v.abs();
                    if amp > self.period_amp {
                        self.period_amp = amp;
                    }
                    if self.prev_smp < 0.0 && v >= 0.0 {
                        if self.period_amp < amp_threshold {
                            self.period_cnt = 0;
                            if self.playing {
                                self.send(n, net, mod_offset + i as i32, Send::Off);
                            }
                        } else {
                            let x = self.prev_smp / (self.prev_smp - v);
                            self.freq = srate as f32 / (self.period as f32 + self.period_add - 1.0 + x);
                            self.freq_detected = true;
                            self.period_add = 1.0 - x;
                            if self.period_cnt >= 1 {
                                let prev_pitch = self.pitch;
                                let prev_vel = self.vel;
                                self.pitch = self.get_pitch(&net.mods[n], self.freq);
                                self.vel = (self.period_amp * 256.0 / 1.0 * (vel_gain + 1) as f32) as i32;
                                if self.vel > 256 {
                                    self.vel = 256;
                                }
                                let mut cmd = Some(Send::On);
                                if self.playing {
                                    cmd = Some(Send::Change);
                                    if (self.pitch >> 2) == (prev_pitch >> 2) {
                                        cmd = if prev_vel == self.vel { None } else { Some(Send::Vel) };
                                    }
                                }
                                if let Some(c) = cmd {
                                    self.send(n, net, mod_offset + i as i32, c);
                                }
                            }
                            self.period_cnt += 1;
                        }
                        self.period = 0;
                        self.period_amp = 0.0;
                    }
                    self.prev_smp = v;
                }
            }
            1..=3 => {
                let mut max_freq = if alg == 1 { 2000 } else { 1000 };
                if base >= 0x02010200 && lp_freq == 0 && self.srate >= 44100 {
                    max_freq *= 2;
                }
                let min_t = (self.srate / max_freq).max(2);
                let (thr, abs_thr, vel_gain) = (ctl(C_THRESHOLD), ctl(C_ABS_THRESHOLD), ctl(C_VEL_GAIN));
                let bs = self.buf_size as usize;
                let half = bs / 2;
                for (i, &x) in cur.iter().enumerate().take(in_frames) {
                    self.buf[self.buf_ptr as usize] = x;
                    self.buf_ptr += 1;
                    if self.buf_ptr < self.buf_size {
                        continue;
                    }
                    let mut amp = 0.0f32;
                    if alg == 1 {
                        self.energy_terms[0] = 0.0;
                        for t in 0..half {
                            self.energy_terms[0] += self.buf[t] * self.buf[t];
                        }
                        let mut amp2 = 0.0f32;
                        let mut t = half;
                        while t < bs {
                            amp2 += self.buf[t] * self.buf[t];
                            t += 2;
                        }
                        amp = ((self.energy_terms[0] / half as f32).sqrt() + (amp2 / (bs / 4) as f32).sqrt()) / 2.0;
                    }
                    if alg >= 2 {
                        let mut t = 0;
                        while t < bs {
                            amp += self.buf[t] * self.buf[t];
                            t += 2;
                        }
                        amp = (amp / half as f32).sqrt();
                    }
                    let amp_threshold = thr as f32 / 10000.0;
                    if amp > 1.0 / (32768.0 * 2.0) || self.playing {
                        let mut t_found = min_t;
                        if alg == 1 {
                            for t in 1..half {
                                self.energy_terms[t] = self.energy_terms[t - 1] - self.buf[t - 1] * self.buf[t - 1] + self.buf[t + half] * self.buf[t + half];
                            }
                            self.fft_i1[..bs].fill(0.0);
                            self.fft_r1[..bs].copy_from_slice(&self.buf[..bs]);
                            fft_f32(0, &mut self.fft_i1[..bs], &mut self.fft_r1[..bs], bs);
                            self.fft_i2[..bs].fill(0.0);
                            for t in 0..=half {
                                self.fft_r2[t] = 0.0;
                            }
                            self.fft_r2[0] = self.buf[0];
                            for t in 1..half {
                                self.fft_r2[bs - t] = self.buf[t];
                            }
                            fft_f32(0, &mut self.fft_i2[..bs], &mut self.fft_r2[..bs], bs);
                            for t in 0..bs {
                                let r1 = self.fft_r1[t];
                                self.fft_r1[t] = self.fft_r1[t] * self.fft_r2[t] - self.fft_i1[t] * self.fft_i2[t];
                                self.fft_i1[t] = self.fft_i1[t] * self.fft_r2[t] + r1 * self.fft_i2[t];
                            }
                            fft_f32(FFT_FLAG_INVERSE, &mut self.fft_i1[..bs], &mut self.fft_r1[..bs], bs);
                            for t in 0..half {
                                self.fft_r1[t] = self.energy_terms[0] + self.energy_terms[t] - 2.0 * self.fft_r1[t];
                            }
                            let mut sum = 0.0f32;
                            self.fft_r1[0] = 1.0;
                            for t in 1..half {
                                sum += self.fft_r1[t];
                                self.fft_r1[t] *= t as f32 / sum;
                            }
                            let threshold = abs_thr as f32 / 100.0;
                            while (t_found as usize) < half {
                                if self.fft_r1[t_found as usize] < threshold {
                                    while (t_found as usize) + 1 < half && self.fft_r1[t_found as usize + 1] < self.fft_r1[t_found as usize] {
                                        t_found += 1;
                                    }
                                    break;
                                }
                                t_found += 1;
                            }
                        }
                        if alg == 2 {
                            for t in 0..bs {
                                self.fft_r1[t] = self.buf[t] * self.fft_win[t];
                            }
                            self.fft_i1[..bs].fill(0.0);
                            fft_f32(0, &mut self.fft_i1[..bs], &mut self.fft_r1[..bs], bs);
                            for t in 0..=half {
                                let (rv, iv) = (self.fft_r1[t], self.fft_i1[t]);
                                self.fft_r1[t] = (1.0 + (rv * rv + iv * iv).sqrt()).ln();
                            }
                            for t in 0..half - 1 {
                                self.fft_r1[bs - 1 - t] = self.fft_r1[t + 1];
                            }
                            self.fft_i1[..bs].fill(0.0);
                            fft_f32(1, &mut self.fft_i1[..bs], &mut self.fft_r1[..bs], bs);
                            let mut max = -10000000.0f32;
                            for t in min_t as usize..half {
                                let v = self.fft_r1[t];
                                if v > max {
                                    t_found = t as i32;
                                    max = v;
                                }
                            }
                        }
                        if alg == 3 {
                            for t in 0..bs {
                                self.fft_r1[t] = self.buf[t] * self.fft_win[t];
                            }
                            self.fft_i1[..bs].fill(0.0);
                            fft_f32(0, &mut self.fft_i1[..bs], &mut self.fft_r1[..bs], bs);
                            for b in 0..=half {
                                let (rv, iv) = (self.fft_r1[b], self.fft_i1[b]);
                                self.fft_r1[b] = (rv * rv + iv * iv).sqrt();
                            }
                            self.fft_r1[0] /= 2.0;
                            self.fft_r1[half] /= 2.0;
                            let mut max = -10000000.0f32;
                            let mut max_b = 0;
                            for b in 1..half - 1 {
                                let v = self.fft_r1[b];
                                if v > max {
                                    max = v;
                                    max_b = b;
                                }
                            }
                            t_found = max_b as i32;
                        }
                        let ratio = self.resamp.as_ref().unwrap().ratio_fp as u64;
                        let mut evt_offset = ((i as u64 * ratio) / 65536) as i32;
                        if evt_offset >= frames as i32 {
                            evt_offset = frames as i32 - 1;
                        }
                        if alg == 3 {
                            if t_found != 0 {
                                let b = t_found as usize;
                                let (v0, v1, v2) = (self.fft_r1[b - 1], self.fft_r1[b], self.fft_r1[b + 1]);
                                let correction = (v2 / v0).ln() / (2.0 * ((v1 * v1) / (v2 * v0)).ln());
                                let bf = b as f32 + correction;
                                self.freq = bf / half as f32 * (self.srate / 2) as f32;
                                self.freq_detected = true;
                            }
                        } else if t_found > min_t && (t_found as usize) < half {
                            let t2 = if alg == 1 {
                                let frame = self.frame_cnt.wrapping_sub(frames as u32).wrapping_add(evt_offset as u32);
                                let mut best = (self.hist_ptr as usize) & (HIST_SIZE - 1);
                                self.hist[best] = Estimate { t: self.get_t(&self.fft_r1, t_found), v: self.fft_r1[t_found as usize], frame };
                                for h in 1..HIST_SIZE {
                                    let k = (self.hist_ptr as usize).wrapping_sub(h) & (HIST_SIZE - 1);
                                    let e2 = self.hist[k];
                                    if e2.t == 0.0 || frame.wrapping_sub(e2.frame) > (self.buf_size_scaled as f32 * 0.51) as u32 {
                                        break;
                                    }
                                    if e2.v < self.hist[best].v {
                                        best = k;
                                    }
                                }
                                self.hist_ptr += 1;
                                self.hist[best].t
                            } else {
                                self.get_t(&self.fft_r1, t_found)
                            };
                            if t2 != 0.0 {
                                self.freq = self.srate as f32 / t2;
                                self.freq_detected = true;
                            }
                        }
                        if amp < amp_threshold {
                            if self.playing {
                                self.send(n, net, mod_offset + evt_offset, Send::Off);
                            }
                        } else if self.freq_detected {
                            let prev_pitch = self.pitch;
                            let prev_vel = self.vel;
                            self.pitch = self.get_pitch(&net.mods[n], self.freq);
                            self.vel = (amp * 256.0 * (vel_gain + 1) as f32) as i32;
                            if self.vel > 256 {
                                self.vel = 256;
                            }
                            let mut cmd = Some(Send::On);
                            if self.playing {
                                cmd = Some(Send::Change);
                                if (self.pitch >> 2) == (prev_pitch >> 2) {
                                    cmd = if prev_vel == self.vel { None } else { Some(Send::Vel) };
                                }
                            }
                            if let Some(c) = cmd {
                                self.send(n, net, mod_offset + evt_offset, c);
                            }
                        }
                    }
                    self.buf_ptr = self.buf_size - self.buf_hop;
                    let hop = self.buf_hop as usize;
                    self.buf.copy_within(hop..bs, 0);
                }
            }
            _ => {}
        }
        0
    }
}

impl Module for PitchDetector {
    fn handle(&mut self, n: usize, e: &mut Event, net: &mut Net) -> isize {
        let mut retval: isize = 0;
        match e.cmd {
            Cmd::Init => {
                net.register_ctl(n, 0, 3, 1, 1, C_ALG, 1, 0);
                let c = net.register_ctl(n, 0, 10000, 80, 0, C_THRESHOLD, -1, 0);
                net.set_ctl_flags(n, c as usize, 2);
                let c = net.register_ctl(n, 0, 256, 0, 0, C_VEL_GAIN, 0, 0);
                net.set_ctl_flags(n, c as usize, 2);
                net.register_ctl(n, 0, 1, 1, 1, C_MICROTONES, -1, 0);
                let c = net.register_ctl(n, 0, 512, 256, 0, C_DETECTOR_FINETUNE, 256, 0);
                net.set_ctl_show_offset(n, c as usize, -256);
                let c = net.register_ctl(n, 0, 4000, 1000, 0, C_LP_FREQ, 1000, 1);
                net.set_ctl_flags(n, c as usize, 2);
                net.register_ctl(n, 0, 3, 0, 1, C_LP_ROLLOFF, -1, 1);
                net.register_ctl(n, 0, 2, 0, 1, C_SRATE, 0, 2);
                net.register_ctl(n, 0, 4, 2, 1, C_BUF_SIZE, 2, 2);
                net.register_ctl(n, 0, 100, 50, 0, C_BUF_OVERLAP, 50, 2);
                net.register_ctl(n, 0, 100, 10, 0, C_ABS_THRESHOLD, 10, 2);
                net.register_ctl(n, 0, 1, 0, 1, C_REC, -1, 3);
                self.out = None;
                self.max_period = net.sampling_freq / 25;
                self.empty = true;
                net.temp_buf(0);
                retval = 1;
            }
            Cmd::SetupFinished => {
                self.reinit_buffers(n, net);
                retval = 1;
            }
            Cmd::RenderReplace => retval = self.render(n, net),
            Cmd::Clean => {
                self.reset_state();
                self.freq_detected = false;
                self.playing = false;
                self.frame_cnt = 0;
                if let Some(r) = self.resamp.as_mut() {
                    r.reset();
                }
                self.empty = true;
                retval = 1;
            }
            Cmd::AllNotesOff | Cmd::Stop | Cmd::Muted => {
                if self.playing {
                    self.send(n, net, e.offset, Send::Off);
                }
                retval = 1;
            }
            Cmd::SetLocalController | Cmd::SetGlobalController => {
                net::set_ctl(&mut net.mods[n], e);
                retval = 1;
                if matches!(e.ctl_num(), 0 | 7 | 8 | 9) {
                    self.reinit_buffers(n, net);
                }
            }
            Cmd::Close => retval = 1,
            _ => {}
        }
        retval
    }
}
