// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! The SN76489 as Sega's consoles carry it (the Master System's and the
//! Genesis's PSG): three square waves and a noise channel, each at one of
//! 16 levels 2 dB apart. Written from SMS Power's description of the
//! chip: 10-bit tone counters run at a sixteenth of the clock, the noise a
//! 16-bit shift register tapped at bits 0 and 3, reset on a noise write.

/// The levels, 2 dB apart (15 is silent), full scale 1.
fn level(att: u8) -> f32 {
    if att >= 15 {
        0.0
    } else {
        10f32.powf(-0.1 * f32::from(att))
    }
}

pub struct Sn76489 {
    clock: f64,
    /// Tone periods (three) and the noise register.
    period: [u16; 3],
    noise: u8,
    att: [u8; 4],
    latch: u8,
    counter: [u16; 4],
    out: [bool; 4],
    lfsr: u16,
    /// Chip steps (clock / 16) per output frame, and the step's fraction.
    pos: f64,
}

impl Sn76489 {
    pub fn new(clock: f64) -> Sn76489 {
        Sn76489 {
            clock,
            period: [0; 3],
            noise: 0,
            att: [15; 4],
            latch: 0,
            counter: [0; 4],
            out: [false; 4],
            lfsr: 0x8000,
            pos: 0.0,
        }
    }

    /// A byte written to the chip's port.
    pub fn write(&mut self, v: u8) {
        if v & 0x80 != 0 {
            self.latch = (v >> 4) & 7;
            let ch = usize::from(self.latch >> 1);
            if self.latch & 1 != 0 {
                self.att[ch] = v & 15;
            } else if ch < 3 {
                self.period[ch] = (self.period[ch] & 0x3F0) | u16::from(v & 15);
            } else {
                self.noise = v & 7;
                self.lfsr = 0x8000;
            }
        } else {
            let ch = usize::from(self.latch >> 1);
            if self.latch & 1 != 0 {
                self.att[ch] = v & 15;
            } else if ch < 3 {
                self.period[ch] = (self.period[ch] & 15) | (u16::from(v & 0x3F) << 4);
            } else {
                self.noise = v & 7;
                self.lfsr = 0x8000;
            }
        }
    }

    /// Sets a tone channel's period (0-1023) as its two writes would.
    pub fn set_period(&mut self, ch: usize, p: u16) {
        let p = p.min(0x3FF);
        let latch = 0x80 | ((ch as u8) << 5);
        self.write(latch | (p & 15) as u8);
        self.write((p >> 4) as u8);
    }

    pub fn set_att(&mut self, ch: usize, att: u8) {
        self.write(0x90 | ((ch as u8) << 5) | att.min(15));
    }

    /// The noise register (bit 2 white, bits 0-1 the rate, 3 for tone 3's).
    pub fn set_noise(&mut self, v: u8) {
        self.write(0xE0 | (v & 7));
    }

    pub fn noise(&self) -> u8 {
        self.noise
    }

    /// The registers as text: each tone channel's period/volume, then the noise's.
    pub fn state(&self) -> String {
        let v = |c: usize| 15 - i32::from(self.att[c]);
        format!("{}/{} {}/{} {}/{} {}/{}", self.period[0], v(0), self.period[1], v(1), self.period[2], v(2), self.noise, v(3))
    }

    fn step(&mut self) {
        for ch in 0..3 {
            if self.counter[ch] > 0 {
                self.counter[ch] -= 1;
            }
            if self.counter[ch] == 0 {
                self.counter[ch] = self.period[ch].max(1);
                // A period of 0 or 1 holds the output high (samples played that way).
                self.out[ch] = if self.period[ch] <= 1 { true } else { !self.out[ch] };
            }
        }
        if self.counter[3] > 0 {
            self.counter[3] -= 1;
        }
        if self.counter[3] == 0 {
            self.counter[3] = match self.noise & 3 {
                0 => 0x10,
                1 => 0x20,
                2 => 0x40,
                _ => self.period[2].max(1),
            };
            self.out[3] = !self.out[3];
            // The shift register moves on the counter's rising edge.
            if self.out[3] {
                let fb = if self.noise & 4 != 0 { (self.lfsr ^ (self.lfsr >> 3)) & 1 } else { self.lfsr & 1 };
                self.lfsr = (self.lfsr >> 1) | (fb << 15);
            }
        }
    }

    /// The next `n` frames at `rate` of each channel (-1 to 1), into `out`.
    pub fn render(&mut self, rate: u32, out: &mut [&mut [f32]; 4]) {
        let n = out[0].len();
        let per = self.clock / 16.0 / f64::from(rate.max(1));
        for i in 0..n {
            let mut acc = [0f32; 4];
            let mut steps = 0u32;
            self.pos += per;
            while self.pos >= 1.0 {
                self.pos -= 1.0;
                self.step();
                for (ch, a) in acc.iter_mut().enumerate() {
                    let on = if ch < 3 { self.out[ch] } else { self.lfsr & 1 != 0 };
                    *a += if on { 1.0 } else { -1.0 };
                }
                steps += 1;
            }
            let k = 1.0 / steps.max(1) as f32;
            for ch in 0..4 {
                out[ch][i] = acc[ch] * k * level(self.att[ch]);
            }
        }
    }
}
