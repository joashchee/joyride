// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! Compressor (psynths_compressor.cpp): peak or RMS gain reduction above
//! a threshold by a slope, its envelope attacking and releasing a
//! millisecond tick at a time over a 1 ms lookahead (or a sample at a
//! time without latency); one of its inputs may be a sidechain, which it
//! listens to but doesn't play.

use super::super::net::{self, two, Cmd, Event, Module, Net};

const OUTPUTS: usize = 2;
const BUF_SIZE: u32 = 512;

const C_VOLUME: usize = 0;
const C_THRESHOLD: usize = 1;
const C_SLOPE: usize = 2;
const C_ATTACK: usize = 3;
const C_RELEASE: usize = 4;
const C_PEAKMODE: usize = 5;
const C_SIDECHAIN: usize = 6;

const MODE_PEAK: i32 = 0;
const MODE_PEAK_ZERO_LATENCY: i32 = 2;

#[derive(Default)]
pub(crate) struct Compressor {
    alg_version: i32,
    tick_counter: i32,
    tick_size: i32,
    peak: f32,
    rms: f32,
    peak_cnt: i32,
    env: f32,
    gain: f32,
    gain_delta: f32,
    buf: [Vec<f32>; OUTPUTS],
    buf_ptr: u32,
    empty: bool,
    ctls_changed: bool,
    threshold: f32,
    zero_threshold: bool,
    slope: f32,
    attack_coef: f32,
    release_coef: f32,
}

impl Compressor {
    fn clean(&mut self) {
        self.tick_counter = 0;
        for b in self.buf.iter_mut() {
            b.fill(0.0);
        }
        self.buf_ptr = 0;
        self.peak = 0.0;
        self.rms = 0.0;
        self.peak_cnt = 0;
        self.env = if self.alg_version == 0 { 0.0 } else { 1.0 };
        self.gain = 0.0;
        self.gain_delta = 0.0;
        self.empty = true;
    }

    /// The env update and gain from a peak (algorithm 1 and zero latency).
    fn gain_from_peak(&mut self, peak: f32) -> f32 {
        let mut gain = 1.0f32;
        if peak > self.threshold {
            gain = if self.zero_threshold { 0.0 } else { self.threshold / (self.threshold + (peak - self.threshold) * self.slope) };
        }
        let theta = if gain * 1.0 > self.env { self.release_coef } else { self.attack_coef };
        self.env = ((1.0 - theta as f64) * gain as f64 + (theta * self.env) as f64) as f32;
        self.env
    }

    fn render(&mut self, n: usize, net: &mut Net) -> isize {
        let srate = net.sampling_freq;
        let m = &net.mods[n];
        let (offset, frames) = (m.offset as usize, m.frames as usize);
        for ch in 0..OUTPUTS {
            if m.in_empty[ch] < (offset + frames) as i32 {
                self.empty = false;
                break;
            }
        }
        if self.empty {
            return 0;
        }
        let peakmode = m.ctl(C_PEAKMODE);
        let ctl_volume = m.ctl(C_VOLUME);
        let sidechain = m.ctl(C_SIDECHAIN);
        if self.ctls_changed {
            let mut sr = 1000.0f32;
            self.ctls_changed = false;
            self.threshold = m.ctl(C_THRESHOLD) as f32 * 1.0 / 256.0;
            self.zero_threshold = false;
            if self.alg_version == 1 || peakmode >= MODE_PEAK_ZERO_LATENCY {
                if peakmode >= MODE_PEAK_ZERO_LATENCY {
                    sr = srate as f32;
                }
                if self.threshold == 0.0 {
                    self.threshold += 0.5 / 256.0 * 1.0;
                    self.zero_threshold = true;
                }
            }
            self.slope = m.ctl(C_SLOPE) as f32 * 1.0 / 100.0;
            self.attack_coef = 0.0;
            self.release_coef = 0.0;
            let (a, r) = (m.ctl(C_ATTACK), m.ctl(C_RELEASE));
            if a > 0 {
                self.attack_coef = (-1.0f32 / (sr * (a as f32 / 1000.0))).exp() * 1.0 * 1.0;
            }
            if r > 0 {
                self.release_coef = (-1.0f32 / (sr * (r as f32 / 1000.0))).exp() * 1.0 * 1.0;
            }
        }
        // The inputs summed (the sidechain left out).
        let links = net.mods[n].input_links.clone();
        let mut filled = false;
        for (i, &l) in links.iter().enumerate() {
            if i as i32 + 1 == sidechain {
                continue;
            }
            let Some(src) = net.get(l) else { continue };
            let mcc = src.output_channels;
            if mcc == 0 || src.chout[0].is_empty() || l as usize == n {
                continue;
            }
            let (me, src) = two(&mut net.mods, n, l as usize);
            let mut in_ch = 0usize;
            let mut in_empty = 0i32;
            for ch in 0..OUTPUTS {
                if !src.chout[ch].is_empty() && (ch as i32) < mcc {
                    in_ch = ch;
                    in_empty = src.out_empty[ch] - offset as i32;
                }
                let main_in = &mut me.chin[ch][offset..offset + frames];
                let inp = &src.chout[in_ch][offset..offset + frames];
                if !filled {
                    if in_empty >= frames as i32 {
                        main_in.fill(0.0);
                    } else {
                        main_in.copy_from_slice(inp);
                    }
                } else if in_empty < frames as i32 {
                    for p in 0..frames {
                        main_in[p] += inp[p];
                    }
                }
            }
            filled = true;
        }
        let mut sc: Option<(usize, i32)> = None;
        let links_num = links.len() as i32;
        if sidechain > 0 && sidechain <= links_num {
            let l = links[(sidechain - 1) as usize];
            if let Some(s) = net.get(l) {
                if !s.chout[0].is_empty() && s.output_channels != 0 {
                    sc = Some((l as usize, s.output_channels));
                }
            }
        }
        // The sidechain's outputs, copied (it may be any module).
        let sc_bufs: Option<[Vec<f32>; 2]> = sc.map(|(l, num)| {
            let s = &net.mods[l];
            let a = s.chout[0][offset..offset + frames].to_vec();
            let b = if num > 1 && !s.chout[1].is_empty() { s.chout[1][offset..offset + frames].to_vec() } else { a.clone() };
            [a, b]
        });
        let m = &mut net.mods[n];
        if peakmode >= MODE_PEAK_ZERO_LATENCY {
            if !filled {
                m.chin[0][offset..offset + frames].fill(0.0);
                m.chin[1][offset..offset + frames].fill(0.0);
            }
            for i in 0..frames {
                let (s0, s1) = match &sc_bufs {
                    Some(b) => (b[0][i], b[1][i]),
                    None => (m.chin[0][offset + i], m.chin[1][offset + i]),
                };
                let mut peak = 0.0f32;
                let v = s0.abs();
                if v > peak {
                    peak = v;
                }
                let v = s1.abs();
                if v > peak {
                    peak = v;
                }
                let gain = self.gain_from_peak(peak);
                m.chout[0][offset + i] = m.chin[0][offset + i] * gain;
                m.chout[1][offset + i] = m.chin[1][offset + i] * gain;
            }
            if ctl_volume != 256 {
                let vol = ctl_volume as f32 / 256.0;
                for ch in 0..OUTPUTS {
                    for o in m.chout[ch][offset..offset + frames].iter_mut() {
                        *o *= vol;
                    }
                }
            }
            return if ctl_volume == 0 { 2 } else { 1 };
        }
        let lag = (srate / 1000) as u32;
        let mut ptr = 0usize;
        while ptr < frames {
            let mut size = (frames - ptr) as i32;
            let left = self.tick_size - self.tick_counter;
            if size > left / 256 {
                size = left / 256;
            }
            if left & 255 != 0 {
                size += 1;
            }
            if size > (frames - ptr) as i32 {
                size = (frames - ptr) as i32;
            }
            if size < 0 {
                size = 0;
            }
            let size = size as usize;
            let start_buf_ptr = self.buf_ptr;
            let mut buf_ptr = self.buf_ptr;
            if !filled {
                for b in self.buf.iter_mut() {
                    buf_ptr = self.buf_ptr;
                    for _ in 0..size {
                        b[buf_ptr as usize] = 0.0;
                        buf_ptr = (buf_ptr + 1) & (BUF_SIZE - 1);
                    }
                }
            }
            if let Some(sb) = &sc_bufs {
                for s in sb.iter() {
                    if peakmode == MODE_PEAK {
                        for &v in &s[ptr..ptr + size] {
                            let v = if v < 0.0 { -v } else { v };
                            if v > self.peak {
                                self.peak = v;
                            }
                        }
                    } else {
                        for &v in &s[ptr..ptr + size] {
                            self.rms += v * v;
                        }
                    }
                }
                if filled {
                    for ch in 0..OUTPUTS {
                        let inp = &m.chin[ch][offset..];
                        buf_ptr = self.buf_ptr;
                        for &v in &inp[ptr..ptr + size] {
                            self.buf[ch][buf_ptr as usize] = v;
                            buf_ptr = (buf_ptr + 1) & (BUF_SIZE - 1);
                        }
                    }
                }
            } else if filled {
                for ch in 0..OUTPUTS {
                    let inp = &m.chin[ch][offset..];
                    buf_ptr = self.buf_ptr;
                    for &x in &inp[ptr..ptr + size] {
                        if peakmode == MODE_PEAK {
                            let v = if x < 0.0 { -x } else { x };
                            if v > self.peak {
                                self.peak = v;
                            }
                        } else {
                            self.rms += x * x;
                        }
                        self.buf[ch][buf_ptr as usize] = x;
                        buf_ptr = (buf_ptr + 1) & (BUF_SIZE - 1);
                    }
                }
            }
            self.buf_ptr = buf_ptr;
            self.peak_cnt += (size * OUTPUTS) as i32;
            let mut cur_gain = self.gain;
            let gain_delta = self.gain_delta;
            for ch in 0..OUTPUTS {
                let out = &mut m.chout[ch][offset..];
                let mut bp = start_buf_ptr.wrapping_sub(lag) & (BUF_SIZE - 1);
                cur_gain = self.gain;
                for o in out[ptr..ptr + size].iter_mut() {
                    let v = self.buf[ch][bp as usize];
                    bp = (bp + 1) & (BUF_SIZE - 1);
                    *o = v * cur_gain;
                    cur_gain += gain_delta;
                }
            }
            self.gain = cur_gain;
            ptr += size;
            self.tick_counter += 256 * size as i32;
            if self.tick_counter >= self.tick_size {
                self.tick_counter -= self.tick_size;
                let peak = if peakmode == MODE_PEAK { self.peak } else { ((self.rms / self.peak_cnt as f32) as f64).sqrt() as f32 };
                self.peak = 0.0;
                self.rms = 0.0;
                self.peak_cnt = 0;
                let mut gain;
                if self.alg_version == 0 {
                    gain = 1.0f32;
                    let theta = if peak * 1.0 > self.env { self.attack_coef } else { self.release_coef };
                    self.env = ((1.0 - theta as f64) * peak as f64 + (theta * self.env) as f64) as f32;
                    if self.env > self.threshold {
                        gain -= (self.env - self.threshold) * self.slope;
                    }
                } else {
                    gain = self.gain_from_peak(peak);
                }
                if ctl_volume != 256 {
                    gain = gain * ctl_volume as f32 / 256.0;
                }
                if gain < 0.0 {
                    gain = 0.0;
                }
                let tick_size2 = self.tick_size / 256;
                self.gain_delta = (gain - self.gain) / tick_size2 as f32;
                if !net::is_not_min(self.gain_delta) {
                    self.gain_delta = 0.0;
                    self.gain = gain;
                }
            }
        }
        1
    }
}

impl Module for Compressor {
    fn handle(&mut self, n: usize, e: &mut Event, net: &mut Net) -> isize {
        let mut retval = 0;
        match e.cmd {
            Cmd::Init => {
                net.register_ctl(n, 0, 512, 256, 0, C_VOLUME, 256, 0);
                net.register_ctl(n, 0, 512, 256, 0, C_THRESHOLD, 256, 1);
                net.register_ctl(n, 0, 200, 100, 0, C_SLOPE, 100, 1);
                net.register_ctl(n, 0, 500, 1, 0, C_ATTACK, -1, 1);
                net.register_ctl(n, 1, 1000, 300, 0, C_RELEASE, -1, 1);
                net.register_ctl(n, 0, 2, 0, 1, C_PEAKMODE, -1, 2);
                net.register_ctl(n, 0, 32, 0, 1, C_SIDECHAIN, -1, 2);
                self.alg_version = if net.base_host_version >= 0x01070303 { 1 } else { 0 };
                self.tick_size = (net.sampling_freq * 256) / 1000;
                for b in self.buf.iter_mut() {
                    *b = vec![0.0; BUF_SIZE as usize];
                }
                self.clean();
                self.ctls_changed = true;
                retval = 1;
            }
            Cmd::Clean => {
                self.clean();
                retval = 1;
            }
            Cmd::RenderReplace => retval = self.render(n, net),
            Cmd::SetLocalController | Cmd::SetGlobalController => self.ctls_changed = true,
            Cmd::Close => retval = 1,
            _ => {}
        }
        retval
    }
}
