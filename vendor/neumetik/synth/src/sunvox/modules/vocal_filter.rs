// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! Vocal filter (psynths_vocal_filter.cpp): a formant filter, up to five
//! band-passes a channel tuned to a vowel (blended between two of the
//! five chosen, for one of four voices), their frequencies optionally
//! jittered from a seed.

use super::super::net::{self, Cmd, Event, Module, Net};

const OUTPUTS: usize = 2;
const VOWELS_NUM: usize = 5;
const FORMANTS_NUM: usize = 5;

#[rustfmt::skip]
const VOWELS1: [u16; FORMANTS_NUM * 3 * VOWELS_NUM] = [
    800, 1150, 2900, 3900, 4950, 80, 90, 120, 130, 140, 256, 128, 6, 25, 0,
    350, 2000, 2800, 3600, 4950, 60, 100, 120, 150, 200, 256, 25, 45, 2, 0,
    270, 2140, 2950, 3900, 4950, 60, 90, 100, 120, 120, 256, 64, 12, 12, 1,
    450, 800, 2830, 3800, 4950, 70, 80, 100, 130, 135, 256, 72, 20, 20, 0,
    325, 700, 2700, 3800, 4950, 50, 60, 170, 180, 200, 256, 40, 4, 2, 0,
];
#[rustfmt::skip]
const VOWELS2: [u16; FORMANTS_NUM * 3 * VOWELS_NUM] = [
    800, 1150, 2800, 3500, 4950, 80, 90, 120, 130, 140, 256, 161, 25, 4, 0,
    400, 1600, 2700, 3300, 4950, 60, 80, 120, 150, 200, 256, 16, 8, 4, 0,
    350, 1700, 2700, 3700, 4950, 50, 100, 120, 150, 200, 256, 25, 8, 4, 0,
    450, 800, 2830, 3500, 4950, 70, 80, 100, 130, 135, 256, 90, 40, 10, 0,
    325, 700, 2530, 3500, 4950, 50, 60, 170, 180, 200, 256, 64, 8, 2, 0,
];
#[rustfmt::skip]
const VOWELS3: [u16; FORMANTS_NUM * 3 * VOWELS_NUM] = [
    650, 1080, 2650, 2900, 3250, 80, 90, 120, 130, 140, 256, 128, 114, 101, 20,
    400, 1700, 2600, 3200, 3580, 70, 80, 100, 120, 120, 256, 51, 64, 51, 25,
    290, 1870, 2800, 3250, 3540, 40, 90, 100, 120, 120, 256, 45, 32, 25, 8,
    400, 800, 2600, 2800, 3000, 40, 80, 100, 120, 120, 256, 80, 64, 64, 12,
    350, 600, 2700, 2900, 3300, 40, 60, 100, 120, 120, 256, 25, 36, 51, 12,
];
#[rustfmt::skip]
const VOWELS4: [u16; FORMANTS_NUM * 3 * VOWELS_NUM] = [
    600, 1040, 2250, 2450, 2750, 60, 70, 110, 120, 130, 256, 114, 90, 90, 25,
    400, 1620, 2400, 2800, 3100, 40, 80, 100, 120, 120, 256, 64, 90, 64, 32,
    250, 1750, 2600, 3050, 3340, 60, 90, 100, 120, 120, 256, 8, 40, 20, 10,
    400, 750, 2400, 2600, 2900, 40, 80, 100, 120, 120, 256, 72, 22, 25, 2,
    350, 600, 2400, 2675, 2950, 40, 80, 100, 120, 120, 256, 25, 6, 10, 4,
];

const C_VOLUME: usize = 0;
const C_BW: usize = 1;
const C_AMP_ADD: usize = 2;
const C_FORMANTS_NUM: usize = 3;
const C_VOWEL: usize = 4;
const C_CHARACTER: usize = 5;
const C_MONO: usize = 6;
const C_RANDOM: usize = 7;
const C_RANDOM_SEED: usize = 8;
const C_V1: usize = 9;

fn interp(v1: i32, v2: i32, p: i32) -> i32 {
    ((v1 * (256 - p)) + (v2 * p)) / 256
}

/// A band-pass (ff).
#[derive(Clone, Copy, Default)]
struct Ff {
    amp: i32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
    b0a0: f32,
    b2a0: f32,
    a1a0: f32,
    a2a0: f32,
}

impl Ff {
    fn stop(&mut self) {
        self.x1 = 0.0;
        self.x2 = 0.0;
        self.y1 = 0.0;
        self.y2 = 0.0;
    }

    /// ff_set, FF_TYPE_BANDPASS2.
    fn set(&mut self, fs: i32, f0: i32, bw: i32, amp: i32) {
        self.amp = amp;
        let w0 = (2.0 * std::f64::consts::PI * (f0 as f32 / fs as f32) as f64) as f32;
        // cos( w0 ) and sin( w0 ) of a float: C++'s float overloads, as one __sincosf_stret.
        let (sn, cs) = super::super::dsp::sincosf(w0);
        let bw = bw.min(fs / 2).max(1);
        let q = f0 as f32 / bw as f32;
        let alpha = sn / (2.0 * q);
        let b0 = alpha;
        let b2 = -alpha;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cs;
        let a2 = 1.0 - alpha;
        self.b0a0 = b0 / a0;
        self.b2a0 = b2 / a0;
        self.a1a0 = a1 / a0;
        self.a2a0 = a2 / a0;
    }

    /// ff_do: whether it made anything.
    fn run(&mut self, out: &mut [f32], inp: &[f32], add: bool) -> bool {
        if self.amp == 0 {
            return false;
        }
        let (b0a0, b2a0, a1a0, a2a0) = (self.b0a0, self.b2a0, self.a1a0, self.a2a0);
        let (mut x1, mut x2, mut y1, mut y2) = (self.x1, self.x2, self.y1, self.y2);
        let amp = self.amp as f32 / 256.0;
        for (o, &i) in out.iter_mut().zip(inp.iter()) {
            let val = b0a0 * i + b2a0 * x2 - a1a0 * y1 - a2a0 * y2;
            x2 = x1;
            x1 = i;
            y2 = y1;
            y1 = val;
            if add {
                *o += val * amp;
            } else {
                *o = val * amp;
            }
        }
        self.x1 = x1;
        self.x2 = x2;
        self.y1 = y1;
        self.y2 = y2;
        true
    }
}

#[derive(Default)]
pub(crate) struct VocalFilter {
    filters: [Ff; FORMANTS_NUM * OUTPUTS],
    empty_frames_counter: i32,
    empty_frames_counter_max: i32,
    changed: bool,
}

impl VocalFilter {
    fn render(&mut self, n: usize, net: &mut Net) -> isize {
        let mono = net.mods[n].ctl(C_MONO) != 0;
        if mono {
            if net.mods[n].output_channels != 1 {
                net.set_number_of_outputs(1, n);
                net.set_number_of_inputs(1, n);
            }
        } else if net.mods[n].output_channels != OUTPUTS as i32 {
            net.set_number_of_outputs(OUTPUTS as i32, n);
            net.set_number_of_inputs(OUTPUTS as i32, n);
        }
        let srate = net.sampling_freq;
        let m = &mut net.mods[n];
        let (offset, frames) = (m.offset as usize, m.frames as usize);
        let outputs_num = m.output_channels.clamp(0, 2) as usize;
        let input_signal = (0..outputs_num).any(|ch| m.in_empty[ch] < (offset + frames) as i32);
        if input_signal {
            self.empty_frames_counter = 0;
        } else {
            if self.empty_frames_counter >= self.empty_frames_counter_max {
                return 0;
            }
            self.empty_frames_counter += frames as i32;
        }
        let formants = m.ctl(C_FORMANTS_NUM).clamp(0, FORMANTS_NUM as i32) as usize;
        if self.changed {
            let vn = m.ctl(C_VOWEL).min(255);
            let pos = ((256 * (VOWELS_NUM as i32 - 1)) * vn) / 256;
            let pos2 = (pos / 256) as usize;
            let pos3 = pos2 + 1;
            let vowels = match m.ctl(C_CHARACTER) {
                1 => &VOWELS2,
                2 => &VOWELS3,
                3 => &VOWELS4,
                _ => &VOWELS1,
            };
            let vmap: [usize; VOWELS_NUM] = std::array::from_fn(|i| m.ctl(C_V1 + i) as i8 as usize);
            let (pos2, pos3) = (vmap[pos2], vmap[pos3]);
            let (ctl_random, ctl_seed, ctl_character, ctl_amp_add, ctl_volume, ctl_bw) =
                (m.ctl(C_RANDOM), m.ctl(C_RANDOM_SEED), m.ctl(C_CHARACTER), m.ctl(C_AMP_ADD), m.ctl(C_VOLUME), m.ctl(C_BW));
            let v = |i: usize| vowels.get(i).copied().unwrap_or(0) as i32;
            let mut f = [0i32; FORMANTS_NUM];
            let mut b = [0i32; FORMANTS_NUM];
            let mut amp = [0i32; FORMANTS_NUM];
            for a in 0..formants {
                let mut f1 = v(pos2 * FORMANTS_NUM * 3 + a);
                let mut f2 = v(pos3 * FORMANTS_NUM * 3 + a);
                if ctl_random != 0 {
                    let base = ((ctl_seed + ctl_character) * 32768) as u32;
                    let mut seed = base.wrapping_add((pos2 * FORMANTS_NUM * 3 + a) as u32);
                    let mut rnd1 = net::pseudo_random(&mut seed) as i32;
                    let mut seed = base.wrapping_add((pos3 * FORMANTS_NUM * 3 + a) as u32);
                    let mut rnd2 = net::pseudo_random(&mut seed) as i32;
                    rnd1 = (rnd1 - 16384) * ctl_random / 1024;
                    rnd2 = (rnd2 - 16384) * ctl_random / 1024;
                    f1 += rnd1 / 4;
                    f2 += rnd2 / 4;
                    if f1 < 100 {
                        f1 = 100 * 2 - f1;
                    }
                    if f2 < 100 {
                        f2 = 100 * 2 - f2;
                    }
                    if f1 > 8000 {
                        f1 = 8000 * 2 - f1;
                    }
                    if f2 > 8000 {
                        f2 = 8000 * 2 - f2;
                    }
                }
                f[a] = interp(f1, f2, pos & 255);
                b[a] = interp(v(pos2 * FORMANTS_NUM * 3 + FORMANTS_NUM + a), v(pos3 * FORMANTS_NUM * 3 + FORMANTS_NUM + a), pos & 255);
                amp[a] = interp(v(pos2 * FORMANTS_NUM * 3 + FORMANTS_NUM * 2 + a), v(pos3 * FORMANTS_NUM * 3 + FORMANTS_NUM * 2 + a), pos & 255);
                amp[a] += ctl_amp_add;
                if amp[a] > 256 {
                    amp[a] = 256;
                }
                amp[a] = (amp[a] * ctl_volume) / 256;
            }
            for ch in 0..outputs_num {
                for a in 0..formants {
                    self.filters[OUTPUTS * a + ch].set(srate, f[a], (b[a] * ctl_bw) / 128, amp[a]);
                }
            }
        }
        let mut retval = 0;
        for ch in 0..outputs_num {
            let (ci, co) = (&m.chin, &mut m.chout);
            let inp = &ci[ch][offset..offset + frames];
            let out = &mut co[ch][offset..offset + frames];
            let mut add = false;
            for a in 0..formants {
                if self.filters[OUTPUTS * a + ch].run(out, inp, add) {
                    retval = 1;
                    add = true;
                }
            }
        }
        self.changed = false;
        retval
    }
}

impl Module for VocalFilter {
    fn handle(&mut self, n: usize, e: &mut Event, net: &mut Net) -> isize {
        let mut retval: isize = 0;
        match e.cmd {
            Cmd::Init => {
                net.register_ctl(n, 0, 512, 256, 0, C_VOLUME, 256, 0);
                net.register_ctl(n, 0, 256, 128, 0, C_BW, 128, 0);
                net.register_ctl(n, 0, 256, 128, 0, C_AMP_ADD, 128, 0);
                net.register_ctl(n, 1, FORMANTS_NUM as i32, FORMANTS_NUM as i32, 1, C_FORMANTS_NUM, -1, 0);
                net.register_ctl(n, 0, 256, 0, 0, C_VOWEL, -1, 1);
                net.register_ctl(n, 0, 3, 0, 1, C_CHARACTER, -1, 1);
                net.register_ctl(n, 0, 1, 0, 1, C_MONO, -1, 2);
                net.register_ctl(n, 0, 1024, 0, 0, C_RANDOM, -1, 3);
                net.register_ctl(n, 0, 32768, 0, 0, C_RANDOM_SEED, -1, 3);
                for i in 0..5 {
                    net.register_ctl(n, 0, 4, i, 1, C_V1 + i as usize, -1, 1);
                }
                self.filters = [Ff::default(); FORMANTS_NUM * OUTPUTS];
                self.changed = true;
                self.empty_frames_counter_max = net.sampling_freq * 4;
                self.empty_frames_counter = self.empty_frames_counter_max;
                retval = 1;
            }
            Cmd::Clean => {
                for f in self.filters.iter_mut() {
                    f.stop();
                }
                self.empty_frames_counter = self.empty_frames_counter_max;
                retval = 1;
            }
            Cmd::RenderReplace => retval = self.render(n, net),
            Cmd::SetLocalController | Cmd::SetGlobalController => self.changed = true,
            Cmd::Close => retval = 1,
            _ => {}
        }
        retval
    }
}
