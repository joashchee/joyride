// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! FFT (psynths_fft.cpp): its input taken to the frequency domain in
//! blocks (overlapped, Hann-windowed at 4x), reshaped (noise reduction,
//! phase gain, all-pass, random frequencies and phases, frequency shift,
//! deformations, band limits, volume, feedback) and back; at a sample
//! rate of its own through the stream resampler.

use super::super::dsp::{self, fft_f32, window_function, Resampler, FFT_FLAG_INVERSE, RESAMP_FLAG_DONT_RESET_WHEN_EMPTY, RESAMP_FLAG_MODE1, WIN_HANN};
use super::super::net::{self, Cmd, Event, Module, Net};

const OUTPUTS: usize = 2;
const CHANGED_IOCHANNELS: u32 = 1 << 0;
const CHANGED_RESAMPLER: u32 = 1 << 1;
const CHANGED_BUF: u32 = 1 << 2;
const RATES: [i32; 10] = [8000, 11025, 16000, 22050, 32000, 44100, 48000, 88200, 96000, 192000];

const C_SRATE: usize = 0;
const C_STEREO: usize = 1;
const C_BUF_SIZE: usize = 2;
const C_OVERLAP: usize = 3;
const C_FEEDBACK: usize = 4;
const C_NOISERED: usize = 5;
const C_PHASE_GAIN: usize = 6;
const C_ALLPASS: usize = 7;
const C_RANDOM_FREQS: usize = 8;
const C_RANDOM_PHASE: usize = 9;
const C_RANDOM_PHASE_SHIFT: usize = 10;
const C_SHIFT: usize = 11;
const C_DEFORM1: usize = 12;
const C_DEFORM2: usize = 13;
const C_LOW: usize = 14;
const C_HIGH: usize = 15;
const C_VOLUME: usize = 16;

const PI: f32 = std::f32::consts::PI;
const PI64: f64 = std::f64::consts::PI;

#[derive(Default)]
pub(crate) struct Fft {
    srate: i32,
    resamp1: Option<Resampler>,
    resamp2: Option<Resampler>,
    use_resampler: bool,
    buf_size: i32,
    native_buf_size: f32,
    buf_ptr: i32,
    bufs: [Vec<f32>; OUTPUTS],
    bufs2: [Vec<f32>; OUTPUTS],
    feedback_bufs: [Vec<f32>; OUTPUTS],
    fft_i: Vec<f32>,
    fft_r: Vec<f32>,
    fft_win: Vec<f32>,
    fft_phase_rotate: [f32; OUTPUTS],
    bufs_clean: bool,
    feedback_bufs_clean: bool,
    empty: bool,
    empty_frames_counter: i32,
    noise_seed: u32,
    changed: u32,
}

impl Fft {
    fn reinit_resampler(&mut self, n: usize, net: &Net) {
        let new_srate = RATES[net.mods[n].ctl(C_SRATE).clamp(0, 9) as usize];
        if new_srate != self.srate {
            self.srate = new_srate;
        }
        self.use_resampler = false;
        if self.srate != net.sampling_freq {
            self.use_resampler = true;
            let flags = RESAMP_FLAG_DONT_RESET_WHEN_EMPTY;
            let (mb, sf) = (net.max_buf_size, net.sampling_freq);
            match self.resamp1.as_mut() {
                None => {
                    let r1 = Resampler::new(mb, sf, self.srate, sf, 0, flags);
                    let ratio2 = ((65536i64 * 65536) / r1.ratio_fp as i64) as u32;
                    let r2 = Resampler::new(mb, sf, sf, self.srate, ratio2, flags | RESAMP_FLAG_MODE1);
                    self.resamp1 = Some(r1);
                    self.resamp2 = Some(r2);
                }
                Some(r1) => {
                    if r1.in_smprate != self.srate {
                        r1.change(self.srate, sf, 0, flags);
                        let ratio2 = ((65536i64 * 65536) / r1.ratio_fp as i64) as u32;
                        self.resamp2.as_mut().unwrap().change(sf, self.srate, ratio2, flags | RESAMP_FLAG_MODE1);
                    }
                }
            }
            for ch in 0..OUTPUTS {
                self.resamp1.as_mut().unwrap().input_buf(ch);
                self.resamp2.as_mut().unwrap().input_buf(ch);
            }
        }
    }

    fn reinit_bufs(&mut self, n: usize, net: &Net) {
        let buf_size = 64 << net.mods[n].ctl(C_BUF_SIZE).clamp(0, 7);
        let size = buf_size as usize;
        if size > self.fft_r.len() {
            for ch in 0..OUTPUTS {
                self.bufs[ch].resize(size, 0.0);
                self.bufs2[ch].resize(size, 0.0);
                self.feedback_bufs[ch].resize(size, 0.0);
            }
            self.fft_i.resize(size, 0.0);
            self.fft_r.resize(size, 0.0);
            self.fft_win.resize(size, 0.0);
        }
        if self.buf_size != buf_size {
            self.buf_size = buf_size;
            self.buf_ptr = 0;
            window_function(&mut self.fft_win[..size], WIN_HANN);
        }
    }

    fn handle_changes(&mut self, n: usize, net: &mut Net) {
        if self.changed & CHANGED_IOCHANNELS != 0 {
            let want = if net.mods[n].ctl(C_STEREO) != 0 { OUTPUTS as i32 } else { 1 };
            if net.mods[n].output_channels != want {
                net.set_number_of_inputs(want, n);
                net.set_number_of_outputs(want, n);
            }
        }
        if self.changed & CHANGED_RESAMPLER != 0 {
            self.reinit_resampler(n, net);
        }
        if self.changed & CHANGED_BUF != 0 {
            self.reinit_bufs(n, net);
        }
        self.native_buf_size = self.buf_size as f32;
        if self.use_resampler {
            self.native_buf_size = (self.buf_size * net.sampling_freq) as f32 / self.srate as f32;
        }
        self.changed = 0;
    }

    /// One block of the buffer through the transform and back (channel `ch`).
    fn process(&mut self, m: &net::ModCommon, ch: usize, buf_ptr: i32) {
        let buf_size = self.buf_size as usize;
        let mask = buf_size - 1;
        let half = buf_size / 2;
        let ctl = |c: usize| m.ctl(c);
        let overlap = ctl(C_OVERLAP);
        let hop = buf_size >> if overlap == 1 { 1 } else if overlap == 2 { 2 } else { 0 };
        let (fr, fi) = (&mut self.fft_r, &mut self.fft_i);
        for t in 0..buf_size {
            fr[t] = self.bufs[ch][(buf_ptr as usize).wrapping_sub(buf_size).wrapping_add(t) & mask];
        }
        if overlap == 2 {
            for t in 0..buf_size {
                fr[t] *= self.fft_win[t];
            }
        }
        if ctl(C_FEEDBACK) != 0 {
            self.feedback_bufs_clean = false;
            let fb = ctl(C_FEEDBACK) as f32 / 32768.0;
            for t in 0..buf_size {
                fr[t] += self.feedback_bufs[ch][t] * fb;
            }
        }
        fi[..buf_size].fill(0.0);
        fft_f32(0, &mut fi[..buf_size], &mut fr[..buf_size], buf_size);
        let mut mirror = false;
        let rf = ctl(C_RANDOM_FREQS);
        if rf != 0 {
            let width = rf as f32 / 32768.0 * (buf_size / 16) as f32;
            if width >= 1.0 {
                let c = 1.0f32 / 16384.0 * width;
                for t in 1..half {
                    let rnd = ((net::pseudo_random(&mut self.noise_seed) as i32 - 16384) as f32 * c) as i32;
                    let t2 = (t as i32 + rnd).clamp(1, half as i32 - 1) as usize;
                    let re = fr[buf_size - t2];
                    let im = -fi[buf_size - t2];
                    fr[t] = re;
                    fi[t] = im;
                }
                mirror = true;
            }
        }
        let nr = ctl(C_NOISERED);
        if nr != 0 {
            let mut min = nr as f32 / 32768.0;
            min *= half as f32;
            min *= min;
            for t in 0..=half {
                let (re, im) = (fr[t], fi[t]);
                if re * re + im * im < min {
                    fr[t] = 0.0;
                    fi[t] = 0.0;
                }
            }
            mirror = true;
        }
        let pg = ctl(C_PHASE_GAIN);
        if pg != 32768 / 2 {
            if pg == 0 {
                for t in 1..half {
                    let (re, im) = (fr[t], fi[t]);
                    fr[t] = (re * re + im * im).sqrt();
                    fi[t] = 0.0;
                }
            } else {
                let g = pg as f32 / 16384.0;
                for t in 1..half {
                    let (re, im) = (fr[t], fi[t]);
                    let phase = im.atan2(re) * g;
                    let md = (re * re + im * im).sqrt();
                    let (sn, cs) = dsp::sincosf(phase);
                    fr[t] = md * cs;
                    fi[t] = md * sn;
                }
            }
            mirror = true;
        }
        let ap = ctl(C_ALLPASS);
        if ap != 0 {
            let w = ap as f32 / 32768.0;
            let c = (w as f64 * PI64 * 2.0).cos() as f32;
            let s = (w as f64 * PI64 * 2.0).sin() as f32;
            for t in 1..half {
                let (rr, ii) = (fr[t], fi[t]);
                fr[t] = rr * c - ii * s;
                fi[t] = rr * s + ii * c;
            }
            mirror = true;
        }
        let rp = ctl(C_RANDOM_PHASE);
        if rp != 0 {
            let c = 1.0f32 / 16384.0 * PI * rp as f32 / 32768.0;
            for t in 1..half {
                let rnd = (net::pseudo_random(&mut self.noise_seed) as i32 - 16384) as f32 * c;
                let (re, im) = (fr[t], fi[t]);
                let phase = im.atan2(re) + rnd;
                let md = (re * re + im * im).sqrt();
                let (sn, cs) = dsp::sincosf(phase);
                fr[t] = md * cs;
                fi[t] = md * sn;
            }
            mirror = true;
        }
        let rps = ctl(C_RANDOM_PHASE_SHIFT);
        if rps != 0 {
            let mut w = net::pseudo_random(&mut self.noise_seed) as f32 / 32768.0;
            w = w * rps as f32 / 32768.0;
            let c = (w as f64 * PI64 * 2.0).cos() as f32;
            let s = (w as f64 * PI64 * 2.0).sin() as f32;
            for t in 1..half {
                let (rr, ii) = (fr[t], fi[t]);
                fr[t] = rr * c - ii * s;
                fi[t] = rr * s + ii * c;
            }
            mirror = true;
        }
        let cs = ctl(C_SHIFT);
        if cs != 4096 {
            let shift = (cs - 4096) * half as i32 / 4096;
            if shift != 0 {
                let mut pr = self.fft_phase_rotate[ch];
                let (pr_sin, pr_cos) = dsp::sincosf(pr);
                if shift < 0 {
                    let s2 = (-shift) as usize;
                    let mut t = 1usize;
                    fr[half] /= 2.0;
                    while t + s2 <= half {
                        let (re, im) = (fr[t + s2], fi[t + s2]);
                        fr[t] = re * pr_cos - im * pr_sin;
                        fi[t] = re * pr_sin + im * pr_cos;
                        t += 1;
                    }
                    while t <= half {
                        fr[t] = 0.0;
                        fi[t] = 0.0;
                        t += 1;
                    }
                } else {
                    let s = shift as usize;
                    let mut t = half;
                    fr[0] /= 2.0;
                    while t > s {
                        let (re, im) = (fr[t - s], fi[t - s]);
                        fr[t] = re * pr_cos - im * pr_sin;
                        fi[t] = re * pr_sin + im * pr_cos;
                        t -= 1;
                    }
                    while t > 0 {
                        fr[t] = 0.0;
                        fi[t] = 0.0;
                        t -= 1;
                    }
                }
                pr += shift as f32 / (1 << overlap) as f32 * 2.0 * PI;
                self.fft_phase_rotate[ch] = (pr as f64 % (2.0 * PI64)) as f32;
                mirror = true;
            }
        }
        let d1 = ctl(C_DEFORM1);
        if d1 != 0 {
            for t in 1..=half {
                let (re, im) = (fr[t], fi[t]);
                let phase = im.atan2(re) * t as f32 * d1 as f32 / 32768.0;
                let md = (re * re + im * im).sqrt();
                let (sn, cs) = dsp::sincosf(phase);
                fr[t] = md * cs;
                fi[t] = md * sn;
            }
            mirror = true;
        }
        let d2 = ctl(C_DEFORM2);
        if d2 != 0 {
            let p = (d2 as f32 / 32768.0) as f64 * 0.9;
            let p = p as f32;
            let mut max = 1.0f32 - p;
            let mul2 = max.sqrt();
            max *= max;
            max *= max;
            let mul1 = 1.0f32 / max;
            let max = half as f32;
            for t in 1..=half {
                let (mut re, mut im) = (fr[t], fi[t]);
                re *= mul1;
                im *= mul1;
                let rrii = re * re + im * im;
                if rrii != 0.0 {
                    let d = max / rrii.sqrt();
                    if d < 1.0 {
                        re *= d;
                        im *= d;
                    }
                }
                fr[t] = re * mul2;
                fi[t] = im * mul2;
            }
            mirror = true;
        }
        let (lo_c, hi_c) = (ctl(C_LOW), ctl(C_HIGH));
        if lo_c != 0 || hi_c != 32768 {
            let low = (lo_c * half as i32 / 32768) as usize;
            let high = (hi_c * half as i32 / 32768) as usize;
            if high < low {
                for t in high..=low {
                    fr[t] = 0.0;
                    fi[t] = 0.0;
                }
            } else {
                if low != 0 {
                    for t in 0..=low {
                        fr[t] = 0.0;
                        fi[t] = 0.0;
                    }
                }
                for t in high..=half {
                    fr[t] = 0.0;
                    fi[t] = 0.0;
                }
            }
            mirror = true;
        }
        let vol = ctl(C_VOLUME);
        if vol != 32768 {
            let v = vol as f32 / 32768.0;
            for t in 0..=half {
                fr[t] *= v;
            }
            for t in 0..=half {
                fi[t] *= v;
            }
            mirror = true;
        }
        if mirror {
            for t in 0..half - 1 {
                fr[buf_size - 1 - t] = fr[t + 1];
            }
            for t in 0..half - 1 {
                fi[buf_size - 1 - t] = -fi[t + 1];
            }
        }
        fft_f32(FFT_FLAG_INVERSE, &mut fi[..buf_size], &mut fr[..buf_size], buf_size);
        if ctl(C_FEEDBACK) != 0 {
            self.feedback_bufs[ch][..buf_size].copy_from_slice(&fr[..buf_size]);
        }
        if overlap != 0 {
            for t in 0..buf_size {
                fr[t] *= self.fft_win[t];
            }
            if overlap == 2 {
                for t in 0..buf_size {
                    fr[t] *= 1.0f32 / 1.5;
                }
            }
            let b2 = &mut self.bufs2[ch];
            b2.copy_within(hop..buf_size, 0);
            for t in 0..buf_size - hop {
                b2[t] += fr[t];
            }
            b2[buf_size - hop..buf_size].copy_from_slice(&fr[buf_size - hop..buf_size]);
        } else {
            self.bufs2[ch][..buf_size].copy_from_slice(&fr[..buf_size]);
        }
    }

    /// The frames in `io` (input and output both) through the buffers.
    fn run_channel(&mut self, m: &net::ModCommon, ch: usize, io: &mut [f32], filled: bool) -> i32 {
        let buf_size = self.buf_size;
        let overlap = m.ctl(C_OVERLAP);
        let hop = buf_size >> if overlap == 1 { 1 } else if overlap == 2 { 2 } else { 0 };
        let (mask, mask2) = (buf_size - 1, hop - 1);
        let mut buf_ptr = self.buf_ptr;
        for v in io.iter_mut() {
            self.bufs[ch][buf_ptr as usize] = if filled { *v } else { 0.0 };
            *v = self.bufs2[ch][(buf_ptr & mask2) as usize];
            buf_ptr += 1;
            if buf_ptr & mask2 == 0 {
                self.process(m, ch, buf_ptr);
                if buf_ptr >= buf_size {
                    buf_ptr = 0;
                }
            }
            let _ = mask;
        }
        buf_ptr
    }

    fn render(&mut self, n: usize, net: &mut Net) -> isize {
        if self.changed != 0 {
            self.handle_changes(n, net);
        }
        let m = &net.mods[n];
        let (offset, frames) = (m.offset as usize, m.frames as usize);
        let outputs_num = m.output_channels.clamp(0, 2) as usize;
        let input_signal = (0..outputs_num).any(|ch| m.in_empty[ch] < (offset + frames) as i32);
        if input_signal {
            self.empty = false;
            self.empty_frames_counter = 0;
        } else if self.empty {
            return 0;
        }
        let mut retval;
        let m = &mut net.mods[n];
        if self.use_resampler {
            let (r1, r2) = (self.resamp1.as_mut().unwrap(), self.resamp2.as_mut().unwrap());
            let resamp_frames = r1.begin(0, frames as i32) as usize;
            r2.begin(frames as i32, resamp_frames as i32);
            let off2 = r2.input_buf_offset();
            for ch in 0..outputs_num {
                let b = r2.input_buf(ch);
                b[off2..off2 + frames].copy_from_slice(&m.chin[ch][offset..offset + frames]);
            }
            let off1 = r1.input_buf_offset();
            let mut ins = std::mem::take(&mut r1.bufs);
            retval = r2.end(if input_signal { 1 } else { 2 }, &mut ins, outputs_num, off1);
            self.bufs_clean = false;
            let mut buf_ptr = self.buf_ptr;
            for ch in 0..outputs_num {
                let mut io = ins[ch][off1..off1 + resamp_frames].to_vec();
                buf_ptr = self.run_channel(m, ch, &mut io, retval != 0);
                ins[ch][off1..off1 + resamp_frames].copy_from_slice(&io);
            }
            self.buf_ptr = buf_ptr;
            let r1 = self.resamp1.as_mut().unwrap();
            r1.bufs = ins;
            retval = r1.end(1, &mut m.chout, outputs_num, offset);
        } else {
            retval = if input_signal { 1 } else { 2 };
            self.bufs_clean = false;
            let mut buf_ptr = self.buf_ptr;
            for ch in 0..outputs_num {
                let mut io = m.chin[ch][offset..offset + frames].to_vec();
                buf_ptr = self.run_channel(m, ch, &mut io, retval != 0);
                m.chout[ch][offset..offset + frames].copy_from_slice(&io);
            }
            self.buf_ptr = buf_ptr;
            retval = 1;
        }
        if !input_signal {
            let mut finished = false;
            if m.ctl(C_FEEDBACK) != 0 {
                finished = true;
                if retval != 0 && (0..outputs_num).any(|ch| m.chout[ch][offset..offset + frames].iter().any(|&v| net::is_not_min(v))) {
                    finished = false;
                }
            } else if (self.empty_frames_counter as f32) < self.native_buf_size * 2.0 {
                self.empty_frames_counter += frames as i32;
            } else {
                finished = true;
            }
            if finished {
                self.empty = true;
                self.empty_frames_counter = 0;
                if self.use_resampler {
                    self.resamp1.as_mut().unwrap().reset();
                    self.resamp2.as_mut().unwrap().reset();
                }
            }
        }
        retval as isize
    }
}

impl Module for Fft {
    fn handle(&mut self, n: usize, e: &mut Event, net: &mut Net) -> isize {
        let mut retval: isize = 0;
        match e.cmd {
            Cmd::Init => {
                net.register_ctl(n, 0, 9, 5, 1, C_SRATE, -1, 0);
                net.register_ctl(n, 0, 1, 0, 1, C_STEREO, -1, 0);
                net.register_ctl(n, 0, 7, 4, 1, C_BUF_SIZE, -1, 0);
                net.register_ctl(n, 0, 2, 1, 1, C_OVERLAP, -1, 0);
                net.register_ctl(n, 0, 32768, 0, 0, C_FEEDBACK, -1, 0);
                let c = net.register_ctl(n, 0, 32768, 0, 0, C_NOISERED, -1, 1);
                net.set_ctl_flags(n, c as usize, 2);
                net.register_ctl(n, 0, 32768, 32768 / 2, 1, C_PHASE_GAIN, 32768 / 2, 1);
                net.register_ctl(n, 0, 32768, 0, 0, C_ALLPASS, -1, 1);
                let c = net.register_ctl(n, 0, 32768, 0, 0, C_RANDOM_FREQS, -1, 2);
                net.set_ctl_flags(n, c as usize, 2);
                let c = net.register_ctl(n, 0, 32768, 0, 0, C_RANDOM_PHASE, -1, 2);
                net.set_ctl_flags(n, c as usize, 2);
                let c = net.register_ctl(n, 0, 32768, 0, 0, C_RANDOM_PHASE_SHIFT, -1, 2);
                net.set_ctl_flags(n, c as usize, 2);
                let c = net.register_ctl(n, 0, 8192, 4096, 0, C_SHIFT, 4096, 3);
                net.set_ctl_show_offset(n, c as usize, -4096);
                net.register_ctl(n, 0, 32768, 0, 0, C_DEFORM1, -1, 3);
                net.register_ctl(n, 0, 32768, 0, 0, C_DEFORM2, -1, 3);
                let c = net.register_ctl(n, 0, 32768, 0, 0, C_LOW, -1, 3);
                net.set_ctl_flags(n, c as usize, 2);
                let c = net.register_ctl(n, 0, 32768, 32768, 0, C_HIGH, -1, 3);
                net.set_ctl_flags(n, c as usize, 2);
                net.register_ctl(n, 0, 32768, 32768, 0, C_VOLUME, -1, 4);
                self.changed = 0xFFFF_FFFF;
                self.bufs_clean = true;
                self.feedback_bufs_clean = true;
                self.empty = true;
                retval = 1;
            }
            Cmd::SetupFinished => {
                self.handle_changes(n, net);
                // stime_ns() + pseudo_random() + mod_num * 371 + mod->id * 3079, the clock at 0.
                self.noise_seed = net.globals.pseudo_random().wrapping_add((n as u32).wrapping_mul(371)).wrapping_add(net.mods[n].id.wrapping_mul(3079));
                retval = 1;
            }
            Cmd::Clean => {
                if let Some(r) = self.resamp1.as_mut() {
                    r.reset();
                }
                if let Some(r) = self.resamp2.as_mut() {
                    r.reset();
                }
                for ch in 0..OUTPUTS {
                    self.fft_phase_rotate[ch] = 0.0;
                    if !self.bufs_clean {
                        self.bufs[ch].fill(0.0);
                        self.bufs2[ch].fill(0.0);
                    }
                    if !self.feedback_bufs_clean {
                        self.feedback_bufs[ch].fill(0.0);
                    }
                }
                self.bufs_clean = true;
                self.feedback_bufs_clean = true;
                self.empty = true;
                self.empty_frames_counter = 0;
                self.buf_ptr = 0;
                retval = 1;
            }
            Cmd::RenderReplace => retval = self.render(n, net),
            Cmd::SetLocalController | Cmd::SetGlobalController => match e.ctl_num() {
                0 => self.changed |= CHANGED_RESAMPLER,
                1 => self.changed |= CHANGED_IOCHANNELS,
                2 => self.changed |= CHANGED_BUF,
                _ => {}
            },
            Cmd::ResetMsb => self.changed |= CHANGED_IOCHANNELS,
            Cmd::Close => retval = 1,
            _ => {}
        }
        let _ = dsp::RESAMP_BUF_TAIL;
        retval
    }
}
