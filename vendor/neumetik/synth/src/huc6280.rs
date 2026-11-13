// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! The PC Engine's sound (the HuC6280's PSG), written from its public
//! descriptions: six channels each playing a 32-step wave of 5-bit
//! samples (or, in DDA mode, whatever value is written), the last two
//! also noise; a 5-bit volume per channel and 4-bit left and right levels
//! for each and for the whole, all in 1.5 dB steps; and the LFO, which
//! plays channel 2's wave into channel 1's period.

/// The chip's clock.
pub const CLOCK: f64 = 3_579_545.0;

#[derive(Clone, Copy, Default)]
struct Channel {
    period: u16,
    control: u8,
    balance: u8,
    wave: [u8; 32],
    /// Where writes to the wave go, and where playback is.
    write_at: usize,
    pos: usize,
    /// Clocks to the next step.
    timer: u32,
    dda: u8,
    noise: u8,
    lfsr: u32,
    noise_timer: u32,
    noise_out: bool,
}

pub struct HuC6280 {
    select: usize,
    main: u8,
    ch: [Channel; 6],
    lfo_freq: u8,
    lfo_ctrl: u8,
    owed: f64,
}

impl Default for HuC6280 {
    fn default() -> HuC6280 {
        let mut ch = [Channel::default(); 6];
        for c in ch.iter_mut() {
            c.lfsr = 1;
        }
        HuC6280 {
            select: 0,
            main: 0,
            ch,
            lfo_freq: 0,
            lfo_ctrl: 0,
            owed: 0.0,
        }
    }
}

/// An attenuation in 1.5 dB steps as a level (31 and more silent).
fn level(att: u32) -> f32 {
    if att >= 31 {
        0.0
    } else {
        10f32.powf(-1.5 * att as f32 / 20.0)
    }
}

impl HuC6280 {
    pub fn clock() -> f64 {
        CLOCK
    }

    /// Writes register `reg` (0 to 9).
    pub fn write(&mut self, reg: u8, v: u8) {
        let c = &mut self.ch[self.select];
        match reg {
            0 => {
                if usize::from(v & 7) < 6 {
                    self.select = usize::from(v & 7);
                }
            }
            1 => self.main = v,
            2 => c.period = (c.period & 0xF00) | u16::from(v),
            3 => c.period = (c.period & 0xFF) | (u16::from(v & 15) << 8),
            4 => {
                // Leaving DDA mode resets where wave writes go.
                if c.control & 0x40 != 0 && v & 0x40 == 0 {
                    c.write_at = 0;
                }
                c.control = v;
            }
            5 => c.balance = v,
            6 => {
                if c.control & 0x40 != 0 {
                    c.dda = v & 31;
                } else if c.control & 0x80 == 0 {
                    c.wave[c.write_at] = v & 31;
                    c.write_at = (c.write_at + 1) & 31;
                }
            }
            7 => c.noise = v,
            8 => self.lfo_freq = v,
            9 => self.lfo_ctrl = v,
            _ => {}
        }
    }

    /// A DDA channel's value (5 bits), as a write to register 6 would set it.
    pub fn set_dda(&mut self, ch: usize, v: u8) {
        if let Some(c) = self.ch.get_mut(ch) {
            c.dda = v & 31;
        }
    }

    /// The registers as text: each channel's period/control/balance/noise
    /// and a sum over its wave, then the main levels and the LFO's.
    pub fn state(&self) -> String {
        let mut t: Vec<String> = self
            .ch
            .iter()
            .map(|c| {
                let ws: u32 = c.wave.iter().map(|&x| u32::from(x) * u32::from(x) * 7 + u32::from(x)).sum::<u32>() % 997;
                format!("{}/{:02x}/{:02x}/{:02x}/{}", c.period, c.control, c.balance, c.noise, ws)
            })
            .collect();
        t.push(format!("{:02x}/{:02x}/{:02x}", self.main, self.lfo_freq, self.lfo_ctrl));
        t.join(" ")
    }

    /// Runs `n` clocks.
    fn run(&mut self, n: u32) {
        let lfo = self.lfo_ctrl & 3 != 0;
        for k in 0..6 {
            // With the LFO on, channel 2 only drives channel 1.
            let period = if k == 0 && lfo {
                let m = &self.ch[1];
                let d = i32::from(m.wave[m.pos]) - 16;
                let shift = [0, 0, 4, 8][usize::from(self.lfo_ctrl & 3)];
                (i32::from(self.ch[0].period) + (d << shift)).clamp(1, 4095) as u32
            } else if k == 1 && lfo {
                (u32::from(self.ch[1].period).max(1)) * u32::from(self.lfo_freq.max(1))
            } else {
                match u32::from(self.ch[k].period) {
                    0 => 4096,
                    p => p,
                }
            };
            let c = &mut self.ch[k];
            let mut left = n;
            while left > 0 {
                if c.timer == 0 {
                    c.timer = period;
                    c.pos = (c.pos + 1) & 31;
                }
                let s = left.min(c.timer);
                c.timer -= s;
                left -= s;
            }
            if k >= 4 && c.noise & 0x80 != 0 {
                let np = 64 * (31 - u32::from(c.noise & 31)).max(1);
                let mut left = n;
                while left > 0 {
                    if c.noise_timer == 0 {
                        c.noise_timer = np;
                        let bit = (c.lfsr ^ (c.lfsr >> 1) ^ (c.lfsr >> 11) ^ (c.lfsr >> 12) ^ (c.lfsr >> 17)) & 1;
                        c.lfsr = (c.lfsr >> 1) | (bit << 17);
                        c.noise_out = c.lfsr & 1 != 0;
                    }
                    let s = left.min(c.noise_timer);
                    c.noise_timer -= s;
                    left -= s;
                }
            }
        }
    }

    /// The next frames at `rate`, each channel's left and right.
    pub fn render(&mut self, rate: u32, left: &mut [&mut [f32]; 6], right: &mut [&mut [f32]; 6]) {
        let per = CLOCK / f64::from(rate.max(1));
        let n = left[0].len();
        let lfo = self.lfo_ctrl & 3 != 0;
        for i in 0..n {
            self.owed += per;
            let clocks = self.owed as u32;
            self.owed -= f64::from(clocks);
            let mut acc = [0f32; 6];
            let mut done = 0;
            while done < clocks {
                let k = (clocks - done).min(8);
                self.run(k);
                for (c, a) in acc.iter_mut().enumerate() {
                    let ch = &self.ch[c];
                    if ch.control & 0x80 == 0 || (c == 1 && lfo) {
                        continue;
                    }
                    let v = if ch.control & 0x40 != 0 {
                        ch.dda
                    } else if c >= 4 && ch.noise & 0x80 != 0 {
                        if ch.noise_out {
                            31
                        } else {
                            0
                        }
                    } else {
                        ch.wave[ch.pos]
                    };
                    *a += (f32::from(v) - 15.5) / 15.5 * k as f32;
                }
                done += k;
            }
            let d = clocks.max(1) as f32;
            for c in 0..6 {
                let ch = &self.ch[c];
                let vol = 31 - u32::from(ch.control & 31);
                let att = |main: u8, bal: u8| vol + 2 * (15 - u32::from(main & 15)) + 2 * (15 - u32::from(bal & 15));
                let v = acc[c] / d;
                left[c][i] = v * level(att(self.main >> 4, ch.balance >> 4));
                right[c][i] = v * level(att(self.main, ch.balance));
            }
        }
    }
}
