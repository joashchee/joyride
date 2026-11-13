// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.
//
// Ported from m6561.h of floooh's chips (github.com/floooh/chips), its
// audio part, under its zlib/libpng license; altered (ported to Rust, each
// voice's output kept apart and centred on 0 in place of the shared
// 512-sample running mean taken off the mix):
//
// Copyright (c) 2019 Andre Weissflog
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

//! The VIC-20's sound: the VIC chip's (6560 NTSC, 6561 PAL) three square
//! wave voices an octave apart (bass, alto, soprano), its noise voice and
//! its master volume, clocked at the machine's rate. Each voice's register
//! is a gate (bit 7) and a 7-bit value X: the voice's half wave lasts
//! 128 - X steps of a counter clocked at the CPU clock divided by 128, 64
//! and 32 for the three voices. The noise is a 23-bit shift register
//! stepped as floooh's chips step it. A port of floooh's m6561.

/// The PAL VIC-20's clock (4.433618 MHz / 4).
pub const PAL_HZ: f64 = 4_433_618.0 / 4.0;
/// The NTSC VIC-20's clock (14.318181 MHz / 14).
pub const NTSC_HZ: f64 = 14_318_181.0 / 14.0;

#[derive(Clone, Copy, Default)]
struct Voice {
    count: u16,
    period: u16,
    bit: bool,
    enabled: bool,
}

/// The chip's sound part.
pub struct Vic {
    clock: f64,
    voices: [Voice; 4],
    shift: u32,
    volume: u8,
    /// Cycles not yet run.
    owed: f64,
}

impl Vic {
    pub fn new(clock: f64) -> Vic {
        Vic { clock, voices: [Voice::default(); 4], shift: 0x7F_FFF8, volume: 0, owed: 0.0 }
    }

    /// Writes register `reg` (0x0A to 0x0E: the three voices, the noise, the volume).
    pub fn write(&mut self, reg: usize, v: u8) {
        // The noise's 0x40 is floooh's, "tweaked to sound right".
        let (i, scale) = match reg {
            0x0A => (0, 0x80),
            0x0B => (1, 0x40),
            0x0C => (2, 0x20),
            0x0D => (3, 0x40),
            0x0E => {
                self.volume = v & 0x0F;
                return;
            }
            _ => return,
        };
        let voice = &mut self.voices[i];
        voice.enabled = v & 0x80 != 0;
        voice.period = scale * (0x80 - u16::from(v & 0x7F));
    }

    /// The noise's level from its shift register (as the SID takes its noise's bits).
    fn noise_level(s: u32) -> f32 {
        const BITS: [u32; 8] = [22, 20, 16, 13, 11, 7, 4, 2];
        let a = BITS.iter().fold(0u32, |a, &b| a << 1 | (s >> b) & 1);
        a as f32 / 256.0
    }

    /// The next frames at `rate`, each voice's own (bass, alto, soprano,
    /// noise), from -1 to 1 at full volume.
    pub fn render(&mut self, rate: u32, out: &mut [&mut [f32]; 4]) {
        let frames = out.iter().map(|o| o.len()).min().unwrap_or(0);
        let step = self.clock / f64::from(rate.max(1));
        let vol = f32::from(self.volume) / 15.0;
        for i in 0..frames {
            self.owed += step;
            let n = self.owed.floor();
            self.owed -= n;
            let n = n as u32;
            let mut acc = [0.0f32; 4];
            for _ in 0..n {
                for (k, v) in self.voices[..3].iter_mut().enumerate() {
                    if v.count == 0 {
                        v.count = v.period;
                        v.bit = !v.bit;
                    } else {
                        v.count -= 1;
                    }
                    // Centred: a square of ±1 while the voice is on.
                    if v.enabled {
                        acc[k] += if v.bit { 1.0 } else { -1.0 };
                    }
                }
                let v = &mut self.voices[3];
                if v.count == 0 {
                    v.count = v.period;
                    v.bit = !v.bit;
                    if v.bit {
                        let s = self.shift;
                        self.shift = ((s << 1) | ((s >> 22) ^ (s >> 13)) & 1) & 0x7F_FFFF;
                    }
                } else {
                    v.count -= 1;
                }
                // Centred on the half of its level it averages.
                if v.enabled {
                    acc[3] += if v.bit { Vic::noise_level(self.shift) * 2.0 - 0.5 } else { -0.5 };
                }
            }
            for k in 0..4 {
                out[k][i] = if n == 0 { 0.0 } else { acc[k] / n as f32 * vol };
            }
        }
    }
}
