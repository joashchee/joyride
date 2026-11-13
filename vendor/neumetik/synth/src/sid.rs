// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.
//
// Ported from m6581.h of floooh's chips (github.com/floooh/chips), under its
// zlib/libpng license; altered (ported to Rust, each voice's output kept
// apart):
//
// Copyright (c) 2018 Andre Weissflog
// This software is provided 'as-is', without any express or implied warranty.
// In no event will the authors be held liable for any damages arising from the
// use of this software.
// Permission is granted to anyone to use this software for any purpose,
// including commercial applications, and to alter it and redistribute it
// freely, subject to the following restrictions:
//     1. The origin of this software must not be misrepresented; you must not
//     claim that you wrote the original software. If you use this software in a
//     product, an acknowledgment in the product documentation would be
//     appreciated but is not required.
//     2. Altered source versions must be plainly marked as such, and must not
//     be misrepresented as being the original software.
//     3. This notice may not be removed or altered from any source
//     distribution.

//! The Commodore 64's SID (the 6581): three voices of a 24-bit oscillator
//! (triangle, sawtooth, pulse, noise and their combinations, sync and ring
//! modulation) under an ADSR envelope, a state-variable filter (low, band,
//! high pass) and the master volume, clocked at the machine's rate. A port
//! of floooh's m6581, its waveforms centred on 0; each voice's part of the output is kept apart (the
//! filter's output shared among the voices through it by their input).

/// The PAL C64's clock.
pub const PAL_CLOCK: u32 = 985_248;

const RATE_COUNT_PERIOD: [u32; 16] = [
    0x7F00, 0x0006, 0x003C, 0x0330, 0x20C0, 0x6755, 0x3800, 0x500E, 0x1212, 0x0222, 0x1848, 0x59B8, 0x3840, 0x77E2, 0x7625, 0x0A93,
];

fn dr_divisor(level: u32) -> u32 {
    match level & 0xff {
        0..=5 => 30,
        6..=13 => 16,
        14..=25 => 8,
        26..=53 => 4,
        54..=92 => 2,
        _ => 1,
    }
}

const CTRL_GATE: u8 = 1;
const CTRL_SYNC: u8 = 2;
const CTRL_RING: u8 = 4;
const CTRL_TEST: u8 = 8;

#[derive(Clone, Copy, PartialEq)]
enum Env {
    Frozen,
    Attack,
    Decay,
    Release,
}

#[derive(Clone, Copy)]
struct Voice {
    muted: bool,
    freq: u32,
    pulse_width: u32,
    ctrl: u8,
    sync: bool,
    noise_shift: u32,
    wav_accum: u32,
    wav_output: u32,
    env_state: Env,
    env_attack_add: u32,
    env_decay_sub: u32,
    env_sustain_level: u32,
    env_release_sub: u32,
    env_cur_level: u32,
    env_counter: u32,
    env_exp_counter: u32,
    env_counter_compare: u32,
}

impl Voice {
    fn new() -> Voice {
        Voice {
            muted: false,
            freq: 0,
            pulse_width: 0,
            ctrl: 0,
            sync: false,
            noise_shift: 0x007F_FFFC,
            wav_accum: 0,
            wav_output: 0,
            env_state: Env::Frozen,
            env_attack_add: 0,
            env_decay_sub: 0,
            env_sustain_level: 0,
            env_release_sub: 0,
            env_cur_level: 0,
            env_counter: 0x7FFF,
            env_exp_counter: 0,
            env_counter_compare: 0,
        }
    }

    fn set_atkdec(&mut self, d: u8) {
        self.env_attack_add = u32::from(d >> 4);
        self.env_decay_sub = u32::from(d & 0x0f);
        match self.env_state {
            Env::Attack => self.env_counter_compare = self.env_attack_add,
            Env::Decay => self.env_counter_compare = self.env_decay_sub,
            _ => {}
        }
    }

    fn set_susrel(&mut self, d: u8) {
        self.env_sustain_level = u32::from(d >> 4) * 0x11;
        self.env_release_sub = u32::from(d & 0x0f);
        if self.env_state == Env::Release {
            self.env_counter_compare = self.env_release_sub;
        }
    }

    fn set_ctrl(&mut self, d: u8) {
        if d & CTRL_TEST != 0 && self.ctrl & CTRL_TEST == 0 {
            self.wav_accum = 0;
            let bit19 = (self.noise_shift >> 19) & 1;
            self.noise_shift = (self.noise_shift & 0x007F_FFFD) | ((bit19 ^ 1) << 1);
        } else if d & CTRL_TEST == 0 && self.ctrl & CTRL_TEST != 0 {
            let bit0 = ((self.noise_shift >> 22) ^ (self.noise_shift >> 17)) & 1;
            self.noise_shift = ((self.noise_shift << 1) & 0x007F_FFFF) | bit0;
        }
        if d & CTRL_GATE != 0 && self.ctrl & CTRL_GATE == 0 {
            self.env_state = Env::Attack;
            self.env_counter_compare = self.env_attack_add;
        } else if d & CTRL_GATE == 0 && self.ctrl & CTRL_GATE != 0 {
            self.env_state = Env::Release;
            self.env_counter_compare = self.env_release_sub;
        }
        if d & 0xf0 > 0x80 {
            self.noise_shift &= 0x007F_FFFF ^ (1 << 22) ^ (1 << 20) ^ (1 << 16) ^ (1 << 13) ^ (1 << 11) ^ (1 << 7) ^ (1 << 4) ^ (1 << 2);
        }
        self.ctrl = d;
    }

    fn wavnone(&mut self) -> u32 {
        if self.wav_accum != 0 {
            let sm = self.wav_accum >> 12;
            self.wav_accum >>= 1;
            sm
        } else {
            0
        }
    }

    fn triangle(&self, sync_accum: u32) -> u32 {
        let ring = self.ctrl & CTRL_RING != 0;
        let msb = (if ring { self.wav_accum ^ sync_accum } else { self.wav_accum }) & 0x0080_0000;
        ((if msb != 0 { !self.wav_accum } else { self.wav_accum }) >> 11) & 0x0fff
    }

    fn sawtooth(&self) -> u32 {
        self.wav_accum >> 12
    }

    fn pulse(&self) -> u32 {
        if self.ctrl & CTRL_TEST != 0 || (self.wav_accum >> 12) >= self.pulse_width {
            0x0fff
        } else {
            0
        }
    }

    fn noise(&self) -> u32 {
        let s = self.noise_shift;
        let b = |n: u32| (s >> n) & 1;
        (b(22) << 11) | (b(20) << 10) | (b(16) << 9) | (b(13) << 8) | (b(11) << 7) | (b(7) << 6) | (b(4) << 5) | (b(2) << 4)
    }
}

#[derive(Clone, Copy, Default)]
struct Filter {
    cutoff: u32,
    resonance: u8,
    voices: u8,
    mode: u8,
    volume: u8,
    nyquist_freq: i32,
    resonance_coeff_div_1024: i32,
    w0: i32,
    v_hp: i32,
    v_bp: i32,
    v_lp: i32,
}

fn cutoff_freq(i: u32) -> f32 {
    let x = i as f32 / 8.0;
    let cf = -0.0156 * x * x + 48.473 * x - 45.074;
    if cf <= 0.0 {
        0.0
    } else {
        cf
    }
}

impl Filter {
    fn set_cutoff(&mut self) {
        let k = 2.0 * std::f32::consts::PI * 1.048576;
        self.w0 = (cutoff_freq(self.cutoff) * k) as i32;
        let n = self.nyquist_freq as f32;
        let max_cutoff = if n > 16000.0 { 16000.0 } else { n };
        let max = (max_cutoff * k) as i32;
        if self.w0 > max {
            self.w0 = max;
        }
    }

    fn set_resonance(&mut self) {
        self.resonance_coeff_div_1024 = (1024.0 / (0.707 + 1.9 * f32::from(self.resonance) / 15.0) + 0.5) as i32;
    }

    fn output(&mut self, vi: i32) -> i32 {
        let w0_dt = self.w0 / (1 << 6);
        let vi = vi / (1 << 7);
        self.v_lp -= (w0_dt * self.v_bp) / (1 << 14);
        self.v_bp -= (w0_dt * self.v_hp) / (1 << 14);
        self.v_hp = ((self.v_bp * self.resonance_coeff_div_1024) / (1 << 10)) - self.v_lp - vi;
        let mut vf = 0;
        if self.mode & 1 != 0 {
            vf += self.v_lp;
        }
        if self.mode & 2 != 0 {
            vf += self.v_bp;
        }
        if self.mode & 4 != 0 {
            vf += self.v_hp;
        }
        vf * (1 << 7)
    }
}

/// A SID chip.
#[derive(Clone)]
pub struct Sid {
    voice: [Voice; 3],
    filter: Filter,
    /// Its registers as last written (for a host to see).
    pub regs: [u8; 32],
}

impl Sid {
    /// A SID whose output is sampled at `sound_hz` (the filter's top cutoff
    /// follows it, as m6581's does).
    pub fn new(sound_hz: u32) -> Sid {
        let mut f = Filter {
            nyquist_freq: (sound_hz / 2) as i32,
            ..Default::default()
        };
        f.set_cutoff();
        f.set_resonance();
        Sid {
            voice: [Voice::new(); 3],
            filter: f,
            regs: [0; 32],
        }
    }

    pub fn write(&mut self, reg: u8, d: u8) {
        let reg = reg & 0x1f;
        self.regs[usize::from(reg)] = d;
        if reg < 21 {
            let v = &mut self.voice[usize::from(reg / 7)];
            match reg % 7 {
                0 => v.freq = (v.freq & 0xff00) | u32::from(d),
                1 => v.freq = (u32::from(d) << 8) | (v.freq & 0xff),
                2 => v.pulse_width = (v.pulse_width & 0x0f00) | u32::from(d),
                3 => v.pulse_width = (u32::from(d & 0x0f) << 8) | (v.pulse_width & 0xff),
                4 => v.set_ctrl(d),
                5 => v.set_atkdec(d),
                _ => v.set_susrel(d),
            }
            return;
        }
        let f = &mut self.filter;
        match reg {
            21 => {
                if (u32::from(d) ^ f.cutoff) & 7 != 0 {
                    f.cutoff = (f.cutoff & 0x7f8) | u32::from(d & 7);
                    f.set_cutoff();
                }
            }
            22 => {
                f.cutoff = (u32::from(d) << 3) | (f.cutoff & 7);
                f.set_cutoff();
            }
            23 => {
                f.voices = d & 7;
                f.resonance = d >> 4;
                f.set_resonance();
            }
            24 => {
                f.volume = d & 0x0f;
                f.mode = d >> 4;
                self.voice[2].muted = f.mode & 8 != 0;
            }
            _ => {}
        }
    }

    fn voice_tick(&mut self, i: usize) {
        let sync_accum = self.voice[(i + 2) % 3].wav_accum;
        let v = &mut self.voice[i];
        if v.ctrl & CTRL_TEST == 0 {
            let prev = v.wav_accum;
            v.wav_accum = (v.wav_accum + v.freq) & 0x00FF_FFFF;
            if v.wav_accum & 0x0008_0000 != 0 && prev & 0x0008_0000 == 0 {
                let s = v.noise_shift;
                v.noise_shift = ((s << 1) | (((s >> 22) ^ (s >> 17)) & 1)) & 0x007F_FFFF;
            }
            v.sync = v.wav_accum & 0x0080_0000 != 0 && prev & 0x0080_0000 == 0;
        }
        let comb = |sm: u32| (sm >> 1) & (sm << 1);
        let sm = match (v.ctrl >> 4) & 0x0f {
            0 => v.wavnone(),
            1 => v.triangle(sync_accum),
            2 => v.sawtooth(),
            3 => comb(v.triangle(sync_accum) & v.sawtooth()),
            4 => v.pulse(),
            5 => comb(v.triangle(sync_accum) & v.pulse()),
            6 => comb(v.sawtooth() & v.pulse()),
            // As m6581 has it: a logical and (1 or 0).
            7 => {
                let sm = v.triangle(sync_accum) & v.sawtooth() & v.pulse();
                u32::from((sm >> 1) != 0 && (sm << 1) != 0)
            }
            8 => v.noise(),
            _ => 0,
        };
        v.wav_output = sm;
        let lfsr = v.env_counter;
        if lfsr != RATE_COUNT_PERIOD[(v.env_counter_compare & 0x0f) as usize] {
            let fb = ((lfsr >> 14) ^ (lfsr >> 13)) & 1;
            v.env_counter = ((lfsr << 1) | fb) & 0x7FFF;
        } else {
            v.env_counter = 0x7FFF;
            let step = v.env_state == Env::Attack || {
                v.env_exp_counter += 1;
                v.env_exp_counter == dr_divisor(v.env_cur_level)
            };
            if step {
                v.env_exp_counter = 0;
                match v.env_state {
                    Env::Attack => {
                        v.env_cur_level += 1;
                        if v.env_cur_level & 0xff == 0xff {
                            v.env_state = Env::Decay;
                            v.env_counter_compare = v.env_decay_sub;
                        }
                    }
                    Env::Decay => {
                        if v.env_cur_level != v.env_sustain_level {
                            v.env_cur_level = v.env_cur_level.wrapping_sub(1) & 0xff;
                            if v.env_cur_level == 0 {
                                v.env_state = Env::Frozen;
                            }
                        }
                    }
                    Env::Release => {
                        v.env_cur_level = v.env_cur_level.wrapping_sub(1) & 0xff;
                        if v.env_cur_level == 0 {
                            v.env_state = Env::Frozen;
                        }
                    }
                    Env::Frozen => v.env_cur_level = 0,
                }
            }
        }
    }

    /// One clock: each voice's part of the output (the chip's full scale
    /// about ±1 summed).
    pub fn clock(&mut self) -> [f32; 3] {
        for i in 0..3 {
            self.voice_tick(i);
        }
        for i in 0..3 {
            let (s, c) = (self.voice[i].sync, (i + 2) % 3);
            if s && self.voice[c].ctrl & CTRL_SYNC != 0 {
                self.voice[c].wav_accum = 0;
            }
        }
        let mut parts = [0i32; 3];
        let mut filtered = 0i32;
        for (i, v) in self.voice.iter().enumerate() {
            // Centred on 0 (m6581 leaves its waveforms unsigned).
            let out = (v.wav_output as i32 - 0x800) * v.env_cur_level as i32;
            if self.filter.voices & (1 << i) != 0 {
                filtered += out;
                parts[i] = out;
            } else {
                parts[i] = if v.muted { 0 } else { out };
            }
        }
        let fout = self.filter.output(filtered);
        let vol = i32::from(self.filter.volume);
        let mut res = [0f32; 3];
        let through: i32 = (0..3).filter(|i| self.filter.voices & (1 << i) != 0).map(|i| parts[i].abs()).sum();
        let n_through = (0..3).filter(|i| self.filter.voices & (1 << i) != 0).count() as f32;
        for i in 0..3 {
            let s = if self.filter.voices & (1 << i) != 0 {
                let share = if through > 0 { parts[i].abs() as f32 / through as f32 } else { 1.0 / n_through };
                fout as f32 * share
            } else {
                parts[i] as f32
            };
            res[i] = s * vol as f32 / 4096.0 / 16384.0;
        }
        res
    }
}
