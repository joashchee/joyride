// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! SpectraVoice (psynths_spectravoice.cpp): a wave made by an inverse FFT
//! of a drawn spectrum, 16 harmonics of a position, volume, width and
//! shape each, played looped (left and right half a wave apart) under an
//! attack and release.
//!
//! The library redraws the wave on a thread of its own when a harmonic
//! changes, the voices silent while it does; here it is redrawn at the
//! next block.

use super::super::dsp::{self, renderbuf2output, RenderbufPars, SmootherCoefs, HSIN_TAB};
use super::super::net::{self, Cmd, Event, Module, Net};

const MAX_CHANNELS: usize = 32;
const MAX_HARMONICS: usize = 16;

const C_VOLUME: usize = 0;
const C_PAN: usize = 1;
const C_ATTACK: usize = 2;
const C_RELEASE: usize = 3;
const C_CHANNELS: usize = 4;
const C_MODE: usize = 5;
const C_SUSTAIN: usize = 6;
const C_SAMPLE_SIZE: usize = 7;
const C_HARM: usize = 8;
const C_FREQ: usize = 9;
const C_FREQ_VOL: usize = 10;
const C_FREQ_BAND: usize = 11;
const C_FREQ_TYPE: usize = 12;

const MODE_HQ: i32 = 0;
const MODE_HQ_MONO: i32 = 1;
const MODE_LQ: i32 = 2;
const MODE_LQ_MONO: i32 = 3;
const MODE_CUBIC: i32 = 4;
const MODES: i32 = 5;

#[rustfmt::skip]
const FFT_RSIN: [i16; 128] = [
    0, 27572, 29794, 4624, -24798, -31421, -9155, 21527, 32418, 13503, -17825, -32766, -17581, 13767, 32459, 21307,
    -9433, -31502, -24607, 4911, 29914, 27414, -290, -27728, -29673, -4336, 24986, 31337, 8876, -21745, -32374, -13239,
    18068, 32764, 17336, -14030, -32497, -21086, 9711, 31580, 24415, -5197, -30031, -27254, 580, 27881, 29548, 4049,
    -25173, -31251, -8597, 21961, 32328, 12973, -18309, -32758, -17089, 14291, 32533, 20863, -9987, -31656, -24220, 5483,
    30146, 27092, -870, -28032, -29422, -3761, 25358, 31163, 8317, -22175, -32280, -12706, 18549, 32751, 16841, -14552,
    -32566, -20639, 10263, 31730, 24024, -5769, -30258, -26928, 1159, 28181, 29293, 3472, -25540, -31072, -8036, 22388,
    32229, 12438, -18788, -32741, -16592, 14811, 32597, 20413, -10538, -31801, -23826, 6054, 30369, 26762, -1449, -28328,
    -29162, -3184, 25721, 30979, 7754, -22599, -32175, -12169, 19024, 32728, 16341, -15069, -32625, -20185, 10812, 31870,
];

#[rustfmt::skip]
const FFT_RCOS: [i16; 128] = [
    32767, 17704, -13635, -32439, -21417, 9294, 31461, 24703, -4767, -29855, -27493, 145, 27650, 29734, 4480, -24892,
    -31379, -9016, 21636, 32396, 13371, -17947, -32765, -17459, 13899, 32478, 21197, -9572, -31541, -24511, 5054, 29973,
    27334, -435, -27805, -29611, -4192, 25080, 31294, 8737, -21853, -32352, -13106, 18189, 32761, 17213, -14161, -32515,
    -20975, 9849, 31619, 24318, -5340, -30089, -27173, 725, 27957, 29485, 3905, -25265, -31207, -8457, 22068, 32304,
    12839, -18429, -32755, -16965, 14422, 32550, 20751, -10125, -31693, -24122, 5626, 30203, 27010, -1014, -28107, -29358,
    -3617, 25449, 31118, 8176, -22282, -32255, -12572, 18669, 32746, 16716, -14682, -32582, -20526, 10401, 31766, 23925,
    -5912, -30314, -26845, 1304, 28255, 29228, 3328, -25631, -31026, -7895, 22494, 32202, 12304, -18906, -32734, -16466,
    14940, 32611, 20299, -10675, -31836, -23726, 6197, 30423, 26678, -1594, -28401, -29096, -3039, 25811, 30931, 7613,
];

#[derive(Clone, Copy, Default)]
struct Chan {
    playing: bool,
    id: u32,
    vel: i32,
    ptr_h: i32,
    ptr_l: i32,
    delta_h: u32,
    delta_l: u32,
    env_vol: u32,
    sustain: bool,
    rb: RenderbufPars,
    local_pan: i32,
}

#[derive(Default)]
pub(crate) struct SpectraVoice {
    channels: [Chan; MAX_CHANNELS],
    no_active_channels: bool,
    search_ptr: usize,
    sample: Vec<i16>,
    sample_size: i32,
    note_offset: i32,
    correct_fft: bool,
    recalc_pending: bool,
    coefs: SmootherCoefs,
    bufs: [Vec<f32>; 2],
}

/// PSYNTH_FP64_ADD.
fn fp_add(h: &mut i32, l: &mut i32, h2: u32, l2: u32) {
    *h = h.wrapping_add(h2 as i32);
    *l = l.wrapping_add(l2 as i32);
    if *l > (1 << 16) - 1 {
        *h = h.wrapping_add(1);
        *l -= 1 << 16;
    }
}

/// The harmonics' chunk (0: freq u16; 1: volume; 2: width; 3: type), byte or word `i`.
fn harm(net: &Net, n: usize, chunk: usize, i: usize) -> i32 {
    let Some(c) = net.mods[n].chunk(chunk) else { return 0 };
    if chunk == 0 {
        c.data.get(i * 2..i * 2 + 2).map_or(0, |b| u16::from_le_bytes([b[0], b[1]]) as i32)
    } else {
        c.data.get(i).copied().unwrap_or(0) as i32
    }
}

fn set_harm(net: &mut Net, n: usize, chunk: usize, i: usize, v: i32) {
    let Some(c) = net.mods[n].chunk_mut(chunk) else { return };
    if chunk == 0 {
        if let Some(b) = c.data.get_mut(i * 2..i * 2 + 2) {
            b.copy_from_slice(&(v as u16).to_le_bytes());
        }
    } else if let Some(b) = c.data.get_mut(i) {
        *b = v as u8;
    }
}

impl SpectraVoice {
    fn set_number_of_outputs(n: usize, net: &mut Net) {
        let mode = net.mods[n].ctl(C_MODE);
        let want = if mode == MODE_HQ_MONO || mode == MODE_LQ_MONO { 1 } else { 2 };
        if net.mods[n].output_channels != want {
            net.set_number_of_outputs(want, n);
        }
    }

    /// recalc_samples: the spectrum drawn and turned into the wave.
    fn recalc_samples(&mut self, n: usize, net: &Net) {
        let ctl_sample_size = net.mods[n].ctl(C_SAMPLE_SIZE);
        let prev_ss = self.sample_size;
        self.sample_size = 4096 << ctl_sample_size;
        let size = self.sample_size as usize;
        if self.sample_size != prev_ss {
            self.sample = vec![0; size];
        }
        let mut hr = vec![0.0f32; size];
        let mut hi = vec![0.0f32; size];
        let mut distribution = [0u8; 256];
        let mut rseed = 0u32;
        let half = self.sample_size / 2;
        for h in 0..MAX_HARMONICS {
            let mut clones = 1;
            let mut harmonic_vol = harm(net, n, 1, h);
            if harmonic_vol == 0 {
                continue;
            }
            let mut bandwidth = harm(net, n, 2, h);
            let mut bandwidth_add = 0;
            let mut vol_mul = 1.0f32;
            let freq = harm(net, n, 0, h);
            let ftype = harm(net, n, 3, h);
            let freq_ptr = ((freq as u64 * half as u64) / 22050) as i32;
            let freq_ptr = freq_ptr.clamp(0, half);
            if ftype == 18 {
                let mut rseed2 = (h as i32 + bandwidth) as u32;
                for i in 0..half {
                    let mut vol = 0;
                    if (net::pseudo_random(&mut rseed2) & 255) as i32 >= 245 + ctl_sample_size * 2 {
                        vol = (net::pseudo_random(&mut rseed2) & 255) as i32;
                        vol = (vol * vol) / 255;
                    }
                    let mut sin_ptr = (i - freq_ptr) >> (4 + ctl_sample_size);
                    sin_ptr += 128;
                    if !(0..=255).contains(&sin_ptr) {
                        vol = 0;
                    } else {
                        vol = vol * HSIN_TAB[sin_ptr as usize] as i32 / 255;
                    }
                    let amp = (harmonic_vol * vol) / 256;
                    let phase = net::pseudo_random(&mut rseed) as usize;
                    hi[i as usize] += ((FFT_RSIN[phase & 127] as i32 * amp) / 256) as f32 / 32767.0;
                    hr[i as usize] += ((FFT_RCOS[phase & 127] as i32 * amp) / 256) as f32 / 32767.0;
                }
                let fadein = half / 32;
                for i in 0..fadein {
                    let mut v = i as f32 / fadein as f32;
                    v *= v;
                    hi[i as usize] *= v;
                    hr[i as usize] *= v;
                }
                continue;
            }
            match ftype {
                1 => distribution.fill(255),
                0 | 2..=5 => distribution.copy_from_slice(&HSIN_TAB),
                6 => {
                    for a in 0..128 {
                        distribution[(a + 64) & 255] = HSIN_TAB[a * 2] / 2 + 127;
                        distribution[((a + 128) + 64) & 255] = (127 - (HSIN_TAB[a * 2] / 2) as i32) as u8;
                    }
                }
                7 => {
                    let mut rseed2 = bandwidth as u32;
                    distribution.fill(0);
                    for _ in 0..8 {
                        distribution[(net::pseudo_random(&mut rseed2) & 255) as usize] = 255;
                    }
                    distribution[126] = 128;
                    distribution[127] = 255;
                    distribution[128] = 128;
                }
                8..=17 => {
                    for a in 0..128 {
                        distribution[a] = (a * 2) as u8;
                        distribution[a + 128] = ((127 - a) * 2) as u8;
                    }
                    if ftype >= 9 {
                        for d in distribution.iter_mut() {
                            let mut v = *d as i32;
                            v = (v * v) >> 8;
                            v = (v * v) >> 8;
                            *d = v as u8;
                        }
                    }
                }
                _ => {}
            }
            match ftype {
                3 => bandwidth_add = 2,
                4 => bandwidth_add = 4,
                5 => bandwidth_add = 8,
                _ => {}
            }
            match ftype {
                2..=5 => clones = 8,
                10 => (clones, vol_mul) = (16, 0.8),
                11 => (clones, vol_mul) = (16, 0.6),
                12 => (clones, vol_mul) = (16, 0.5),
                13 => (clones, vol_mul) = (16, 0.25),
                14 => (clones, vol_mul, bandwidth_add) = (16, 0.8, 8),
                15 => (clones, vol_mul, bandwidth_add) = (16, 0.6, 9),
                16 => (clones, vol_mul, bandwidth_add) = (16, 0.5, 10),
                17 => (clones, vol_mul, bandwidth_add) = (16, 0.25, 11),
                _ => {}
            }
            for cc in 0..clones {
                let mut ptr = if net.base_host_version < 0x01090000 {
                    (freq * self.sample_size) / 22050
                } else {
                    ((freq as u64 * self.sample_size as u64) / 22050) as i32
                };
                if ptr >= self.sample_size {
                    ptr = self.sample_size - 1;
                }
                if cc > 0 {
                    ptr += ptr * cc;
                    bandwidth += bandwidth_add;
                    if vol_mul != 1.0 {
                        harmonic_vol = (harmonic_vol as f32 * vol_mul) as i32;
                    }
                }
                ptr /= 2;
                let mut bw = ((bandwidth + 1) * (self.sample_size / 16)) / 256;
                bw /= 2;
                if bw <= 0 {
                    bw = 1;
                }
                let p1 = (ptr - bw).max(0);
                let p2 = if ptr + bw >= half { half - 1 } else { ptr + bw };
                if harmonic_vol != 0 {
                    for i in p1..=p2 {
                        let mut d = i - ptr;
                        d *= (128 << 14) / bw;
                        d >>= 14;
                        if (-128..128).contains(&d) {
                            let vol = distribution[(128 + d) as usize] as i32;
                            let amp = (harmonic_vol * vol) / 256;
                            let phase = net::pseudo_random(&mut rseed) as usize;
                            hi[i as usize] += ((FFT_RSIN[phase & 127] as i32 * amp) / 256) as f32 / 32767.0;
                            hr[i as usize] += ((FFT_RCOS[phase & 127] as i32 * amp) / 256) as f32 / 32767.0;
                        }
                    }
                }
            }
        }
        if self.correct_fft {
            for i in 0..size / 2 - 1 {
                hr[size - 1 - i] = hr[i + 1];
                hi[size - 1 - i] = -hi[i + 1];
            }
        } else {
            for i in 0..size / 2 {
                hr[size - 1 - i] = -hr[i];
                hi[size - 1 - i] = -hi[i];
            }
        }
        hi[0] = 0.0;
        hr[0] = 0.0;
        // fft_with_normalization.
        dsp::fft_f32(0, &mut hi, &mut hr, size);
        let mut max = 0.0f32;
        for &v in hr.iter() {
            let v = v.abs();
            if v > max {
                max = v;
            }
        }
        if max == 0.0 {
            self.sample.fill(0);
            return;
        }
        let coef = 1.0f32 / max;
        for (s, &v) in self.sample.iter_mut().zip(hr.iter()) {
            let ires = ((v * coef) * 32768.0) as i32;
            *s = ires.clamp(-32767, 32767) as i16;
        }
    }

    fn render(&mut self, n: usize, net: &mut Net) -> isize {
        if self.recalc_pending {
            self.recalc_pending = false;
            self.recalc_samples(n, net);
        }
        if self.no_active_channels {
            return 0;
        }
        let m = &net.mods[n];
        let (offset, frames) = (m.offset as usize, m.frames as usize);
        let (ctl_volume, ctl_pan, ctl_attack, ctl_release, ctl_channels, ctl_mode, ctl_sustain) =
            (m.ctl(C_VOLUME), m.ctl(C_PAN), m.ctl(C_ATTACK), m.ctl(C_RELEASE), m.ctl(C_CHANNELS), m.ctl(C_MODE), m.ctl(C_SUSTAIN));
        let outputs_num = m.output_channels.max(0) as usize;
        let srate = net.sampling_freq;
        let mut no_env = false;
        let mut attack_delta = 1i32 << 30;
        let mut release_delta = 1i32 << 30;
        if ctl_attack == 0 && ctl_release == 0 {
            no_env = true;
        } else {
            let mut attack_len = (srate * ctl_attack) / 256;
            let mut release_len = (srate * ctl_release) / 256;
            if net.base_host_version < 0x01090600 {
                attack_len &= !1023;
                release_len &= !1023;
            }
            if attack_len != 0 {
                attack_delta = (1 << 30) / attack_len;
            }
            if release_len != 0 {
                release_delta = (1 << 30) / release_len;
            }
        }
        for b in self.bufs.iter_mut() {
            if b.len() < frames {
                b.resize(frames, 0.0);
            }
        }
        let mut retval = 0i32;
        self.no_active_channels = true;
        let size = self.sample_size;
        let mask = size - 1;
        let smp = &self.sample;
        for c in 0..ctl_channels.max(0) as usize {
            let chan = &mut self.channels[c];
            if !chan.playing {
                continue;
            }
            self.no_active_channels = false;
            let (delta_h, delta_l) = (chan.delta_h, chan.delta_l);
            let mut sustain = chan.sustain;
            let (mut ptr_h, mut ptr_l) = (0i32, 0i32);
            let mut playing = false;
            let sustain_enabled = ctl_sustain != 0;
            let mut env_vol = 0u32;
            let mut rendered_frames = 0usize;
            for ch in 0..outputs_num.min(2) {
                let out = &mut self.bufs[ch];
                sustain = chan.sustain;
                ptr_h = chan.ptr_h;
                ptr_l = chan.ptr_l;
                env_vol = chan.env_vol;
                playing = chan.playing;
                if smp.is_empty() {
                    break;
                }
                let poff = if ch == 1 { size / 2 } else { 0 };
                if no_env {
                    if !sustain_enabled {
                        sustain = false;
                    }
                    if !sustain {
                        playing = false;
                        break;
                    }
                    rendered_frames = frames;
                }
                let at = |p: i32| smp[(p & mask) as usize] as i32;
                if ctl_mode == MODE_LQ_MONO || ctl_mode == MODE_LQ {
                    for o in out[..frames].iter_mut() {
                        *o = at(ptr_h + poff) as f32 / 32768.0;
                        fp_add(&mut ptr_h, &mut ptr_l, delta_h, delta_l);
                    }
                }
                if ctl_mode == MODE_HQ_MONO || ctl_mode == MODE_HQ {
                    for o in out[..frames].iter_mut() {
                        let mut s = at(ptr_h + poff) * (32767 - (ptr_l >> 1));
                        s += at(ptr_h + poff + 1) * (ptr_l >> 1);
                        s >>= 15;
                        *o = s as f32 / 32768.0;
                        fp_add(&mut ptr_h, &mut ptr_l, delta_h, delta_l);
                    }
                }
                if ctl_mode == MODE_CUBIC {
                    for o in out[..frames].iter_mut() {
                        let s = dsp::catmull_rom_int16(at(ptr_h + poff - 1), at(ptr_h + poff), at(ptr_h + poff + 1), at(ptr_h + poff + 2), ptr_l >> 1);
                        *o = s as f32 / 32768.0;
                        fp_add(&mut ptr_h, &mut ptr_l, delta_h, delta_l);
                    }
                }
                if !no_env {
                    let mut i = 0;
                    if sustain {
                        if sustain_enabled && env_vol == 1 << 30 {
                            i = frames;
                        } else {
                            while i < frames {
                                out[i] = (out[i] * (env_vol >> 15) as f32) / 32768.0;
                                env_vol = env_vol.wrapping_add(attack_delta as u32);
                                if env_vol >= 1 << 30 {
                                    env_vol = 1 << 30;
                                    if !sustain_enabled {
                                        sustain = false;
                                        i += 1;
                                        break;
                                    }
                                }
                                i += 1;
                            }
                        }
                    }
                    if !sustain {
                        while i < frames {
                            out[i] = (out[i] * (env_vol >> 15) as f32) / 32768.0;
                            env_vol = env_vol.wrapping_sub(release_delta as u32);
                            if env_vol > 1 << 30 {
                                env_vol = 0;
                                playing = false;
                                i += 1;
                                break;
                            }
                            i += 1;
                        }
                    }
                    rendered_frames = i;
                }
            }
            chan.ptr_h = ptr_h;
            chan.ptr_l = ptr_l;
            chan.env_vol = env_vol;
            chan.sustain = sustain;
            chan.playing = playing;
            if !playing {
                chan.id = !0;
            }
            let local_vol = (ctl_volume * chan.vel) / 2;
            let local_pan = (ctl_pan + (chan.local_pan - 128)) * 256;
            let m = &mut net.mods[n];
            let (b0, b1) = self.bufs.split_at(1);
            let rb2 = if outputs_num == 1 { None } else { Some(&b1[0][..frames]) };
            retval = renderbuf2output(retval, &mut m.chout, outputs_num, offset, frames, &b0[0][..frames], rb2, rendered_frames, local_vol, local_pan, &mut chan.rb, &self.coefs, srate);
        }
        retval as isize
    }
}

impl Module for SpectraVoice {
    fn handle(&mut self, n: usize, e: &mut Event, net: &mut Net) -> isize {
        let mut retval: isize = 0;
        match e.cmd {
            Cmd::Init => {
                net.register_ctl(n, 0, 256, 128, 0, C_VOLUME, -1, 0);
                net.register_ctl(n, 0, 256, 128, 0, C_PAN, 128, 0);
                net.set_ctl_show_offset(n, 1, -128);
                net.register_ctl(n, 0, 512, 1, 0, C_ATTACK, -1, 1);
                net.register_ctl(n, 0, 512, 512, 0, C_RELEASE, -1, 1);
                net.register_ctl(n, 1, MAX_CHANNELS as i32, 8, 1, C_CHANNELS, -1, 1);
                net.register_ctl(n, 0, MODES - 1, MODE_CUBIC, 1, C_MODE, -1, 1);
                net.register_ctl(n, 0, 1, 1, 1, C_SUSTAIN, -1, 1);
                net.register_ctl(n, 0, 5, 1, 1, C_SAMPLE_SIZE, -5, 2);
                net.register_ctl(n, 0, MAX_HARMONICS as i32 - 1, 0, 1, C_HARM, -1, 3);
                net.register_ctl(n, 0, 22050, 1098, 1, C_FREQ, 1098, 3);
                net.register_ctl(n, 0, 255, 255, 1, C_FREQ_VOL, -1, 3);
                net.register_ctl(n, 0, 255, 3, 1, C_FREQ_BAND, -1, 3);
                net.register_ctl(n, 0, 18, 0, 1, C_FREQ_TYPE, -1, 3);
                for c in self.channels.iter_mut() {
                    *c = Chan { id: !0, ..Chan::default() };
                }
                self.no_active_channels = true;
                self.sample = Vec::new();
                self.sample_size = 0;
                net.new_chunk_sized(n, 0, MAX_HARMONICS * 2, 0, 0);
                net.new_chunk_sized(n, 1, MAX_HARMONICS, 0, 0);
                net.new_chunk_sized(n, 2, MAX_HARMONICS, 0, 0);
                net.new_chunk_sized(n, 3, MAX_HARMONICS, 0, 0);
                let m = &net.mods[n];
                let (f, v, b, t) = (m.ctl(C_FREQ), m.ctl(C_FREQ_VOL), m.ctl(C_FREQ_BAND), m.ctl(C_FREQ_TYPE));
                set_harm(net, n, 0, 0, f);
                set_harm(net, n, 1, 0, v);
                set_harm(net, n, 2, 0, b);
                set_harm(net, n, 3, 0, t);
                self.note_offset = 0;
                self.search_ptr = 0;
                self.coefs = SmootherCoefs::new(100.0, net.sampling_freq as f32);
                for p in 0..2 {
                    net.temp_buf(p);
                }
                retval = 1;
            }
            Cmd::SetupFinished => {
                self.recalc_pending = false;
                self.recalc_samples(n, net);
                Self::set_number_of_outputs(n, net);
                retval = 1;
            }
            Cmd::RenderReplace => retval = self.render(n, net),
            Cmd::NoteOn => {
                let ctl_channels = net.mods[n].ctl(C_CHANNELS).max(1) as usize;
                if !self.no_active_channels {
                    for c in self.channels.iter_mut() {
                        if c.id == e.id {
                            c.sustain = false;
                            c.id = !0;
                            break;
                        }
                    }
                    let mut c = 0;
                    while c < ctl_channels {
                        if !self.channels[self.search_ptr].playing {
                            break;
                        }
                        self.search_ptr += 1;
                        if self.search_ptr >= ctl_channels {
                            self.search_ptr = 0;
                        }
                        c += 1;
                    }
                    if c == ctl_channels {
                        self.search_ptr += 1;
                        if self.search_ptr >= ctl_channels {
                            self.search_ptr = 0;
                        }
                        if net.base_host_version >= 0x02010301 {
                            for _ in 0..ctl_channels {
                                if !self.channels[self.search_ptr].sustain {
                                    break;
                                }
                                self.search_ptr += 1;
                                if self.search_ptr >= ctl_channels {
                                    self.search_ptr = 0;
                                }
                            }
                        }
                    }
                } else {
                    self.search_ptr = 0;
                }
                let freq = net::get_freq(e.pitch() / 4);
                let (dh, dl) = net::get_delta(net.sampling_freq, freq);
                let attack = net.mods[n].ctl(C_ATTACK);
                let chan = &mut self.channels[self.search_ptr];
                chan.rb.start = true;
                if chan.playing && attack != 0 {
                    chan.rb.anticlick = true;
                }
                chan.playing = true;
                chan.vel = e.velocity() as i32;
                chan.delta_h = dh;
                chan.delta_l = dl;
                chan.ptr_h = self.note_offset & (self.sample_size - 1);
                self.note_offset += 333;
                chan.ptr_l = 0;
                chan.id = e.id;
                chan.env_vol = 0;
                chan.sustain = true;
                chan.local_pan = 128;
                self.no_active_channels = false;
                retval = 1;
            }
            Cmd::SetFreq => {
                if self.no_active_channels {
                    return 0;
                }
                let ctl_channels = net.mods[n].ctl(C_CHANNELS).max(0) as usize;
                for c in 0..ctl_channels.min(MAX_CHANNELS) {
                    if self.channels[c].id == e.id {
                        let freq = net::get_freq(e.pitch() / 4);
                        let (dh, dl) = net::get_delta(net.sampling_freq, freq);
                        self.channels[c].delta_h = dh;
                        self.channels[c].delta_l = dl;
                        self.channels[c].vel = e.velocity() as i32;
                        break;
                    }
                }
                retval = 1;
            }
            Cmd::SetVelocity => {
                if self.no_active_channels {
                    return 0;
                }
                let ctl_channels = net.mods[n].ctl(C_CHANNELS).max(0) as usize;
                for c in 0..ctl_channels.min(MAX_CHANNELS) {
                    if self.channels[c].id == e.id {
                        self.channels[c].vel = e.velocity() as i32;
                        break;
                    }
                }
                retval = 1;
            }
            Cmd::NoteOff => {
                if self.no_active_channels {
                    return 0;
                }
                for c in self.channels.iter_mut() {
                    if c.id == e.id {
                        c.sustain = false;
                        break;
                    }
                }
                retval = 1;
            }
            Cmd::AllNotesOff => {
                for c in self.channels.iter_mut() {
                    c.sustain = false;
                    c.id = !0;
                }
                retval = 1;
            }
            Cmd::Clean => {
                for c in self.channels.iter_mut() {
                    c.playing = false;
                    c.id = !0;
                }
                self.no_active_channels = true;
                retval = 1;
            }
            Cmd::SetLocalController | Cmd::SetGlobalController => {
                if e.cmd == Cmd::SetLocalController && !self.no_active_channels {
                    let ctl_channels = net.mods[n].ctl(C_CHANNELS).max(0) as usize;
                    for c in 0..ctl_channels.min(MAX_CHANNELS) {
                        if self.channels[c].id == e.id {
                            if e.ctl_num() == 1 {
                                self.channels[c].local_pan = (e.ctl_val() >> 7) as i32;
                                retval = 1;
                            }
                            break;
                        }
                    }
                    if retval != 0 {
                        return retval;
                    }
                }
                let mut recalc = false;
                let v = e.ctl_val() as i32;
                match e.ctl_num() {
                    5 => {
                        net::set_ctl(&mut net.mods[n], e);
                        Self::set_number_of_outputs(n, net);
                        retval = 1;
                    }
                    7 => recalc = true,
                    8 => {
                        let h = v.clamp(0, MAX_HARMONICS as i32 - 1);
                        let vals = [harm(net, n, 0, h as usize), harm(net, n, 1, h as usize), harm(net, n, 2, h as usize), harm(net, n, 3, h as usize)];
                        let m = &mut net.mods[n];
                        let vars = [C_HARM, C_FREQ, C_FREQ_VOL, C_FREQ_BAND, C_FREQ_TYPE].map(|c| m.ctls.get(c).and_then(|c| c.var));
                        let all = [h, vals[0], vals[1], vals[2], vals[3]];
                        for (var, val) in vars.iter().zip(all) {
                            if let Some(var) = var {
                                m.vars[*var] = val;
                            }
                        }
                        retval = 1;
                    }
                    9..=12 => {
                        let h = net.mods[n].ctl(C_HARM) as usize;
                        set_harm(net, n, e.ctl_num() as usize - 9, h, v);
                        recalc = true;
                    }
                    126 => {
                        self.correct_fft = v != 0;
                        recalc = true;
                    }
                    _ => {}
                }
                if recalc {
                    self.recalc_pending = true;
                }
            }
            Cmd::Close => retval = 1,
            _ => {}
        }
        retval
    }
}
