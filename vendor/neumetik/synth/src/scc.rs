// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.
//
// A port of emu2212 (S.C.C. emulator), MIT:
//
// Copyright (c) 2014 Mitsutaka Okazaki
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

//! Konami's SCC (K051649): five channels of 32-step wavetables. A port of
//! Mitsutaka Okazaki's emu2212, in its exact (chip-rate) mode, the registers
//! as `SCC_writeReg` has them.

const GETA_BITS: u32 = 22;

/// The MSX's clock.
pub const CLOCK: u32 = 3_579_545;

#[derive(Clone)]
pub struct Scc {
    base_incr: u32,
    realstep: u32,
    scctime: u32,
    sccstep: u32,
    /// SCC+ (each channel its own wave) or SCC (channels 4 and 5 share one).
    plus: bool,
    incr: [u32; 5],
    wave: [[i8; 32]; 5],
    count: [u32; 5],
    freq: [u32; 5],
    volume: [u32; 5],
    offset: [u32; 5],
    ch_enable: u32,
    ch_enable_next: u32,
    cycle_4bit: bool,
    cycle_8bit: bool,
    refresh: bool,
    rotate: [u32; 5],
    ch_out: [i16; 5],
}

impl Scc {
    pub fn new(clock: u32, rate: u32) -> Scc {
        let rate = if rate == 0 { 44100 } else { rate };
        Scc {
            base_incr: 2 << GETA_BITS,
            realstep: ((1u64 << 31) / u64::from(rate)) as u32,
            scctime: 0,
            sccstep: ((1u64 << 31) / u64::from(clock / 2)) as u32,
            plus: false,
            incr: [0; 5],
            wave: [[0; 32]; 5],
            count: [0; 5],
            freq: [0; 5],
            volume: [0; 5],
            offset: [0; 5],
            ch_enable: 0xff,
            ch_enable_next: 0xff,
            cycle_4bit: false,
            cycle_8bit: false,
            refresh: false,
            rotate: [0; 5],
            ch_out: [0; 5],
        }
    }

    fn update_output(&mut self) {
        for i in 0..5 {
            self.count[i] = self.count[i].wrapping_add(self.incr[i]);
            if self.count[i] & (1 << (GETA_BITS + 5)) != 0 {
                self.count[i] &= (1 << (GETA_BITS + 5)) - 1;
                self.offset[i] = (self.offset[i] + 31) & self.rotate[i];
                self.ch_enable &= !(1 << i);
                self.ch_enable |= self.ch_enable_next & (1 << i);
            }
            if self.ch_enable & (1 << i) != 0 {
                let phase = ((self.count[i] >> GETA_BITS) + self.offset[i]) & 0x1f;
                let v = (self.volume[i] as i32 * i32::from(self.wave[i][phase as usize])) & 0xfff0;
                self.ch_out[i] = self.ch_out[i].wrapping_add(v as i16);
            }
            self.ch_out[i] >>= 1;
        }
    }

    /// One output sample.
    pub fn calc(&mut self) -> i16 {
        while self.realstep > self.scctime {
            self.scctime = self.scctime.wrapping_add(self.sccstep);
            self.update_output();
        }
        self.scctime = self.scctime.wrapping_sub(self.realstep);
        self.ch_out.iter().fold(0i16, |a, &c| a.wrapping_add(c))
    }

    /// Each channel's output, as of the last sample.
    pub fn channels(&self) -> [i16; 5] {
        self.ch_out
    }

    /// `SCC_writeReg`'s map: waves 0x00-0x9f, frequencies 0xc0-0xc9, volumes
    /// 0xd0-0xd4, 0xe0 mode (bit 0 SCC+), 0xe1 channel enable, 0xe2 flags.
    pub fn write(&mut self, adr: u8, val: u8) {
        let adr = usize::from(adr);
        let val = u32::from(val);
        if adr < 0xa0 {
            let ch = (adr & 0xf0) >> 5;
            if self.rotate[ch] == 0 {
                self.wave[ch][adr & 0x1f] = val as u8 as i8;
                if !self.plus && ch == 3 {
                    self.wave[4][adr & 0x1f] = val as u8 as i8;
                }
            }
        } else if (0xc0..=0xc9).contains(&adr) {
            let ch = (adr & 0x0f) >> 1;
            if adr & 1 != 0 {
                self.freq[ch] = ((val & 0xf) << 8) | (self.freq[ch] & 0xff);
            } else {
                self.freq[ch] = (self.freq[ch] & 0xf00) | (val & 0xff);
            }
            if self.refresh {
                self.count[ch] = 0;
            }
            let mut freq = self.freq[ch];
            if self.cycle_8bit {
                freq &= 0xff;
            }
            if self.cycle_4bit {
                freq >>= 8;
            }
            self.incr[ch] = if freq <= 8 { 0 } else { self.base_incr / (freq + 1) };
        } else if (0xd0..=0xd4).contains(&adr) {
            self.volume[adr & 0x0f] = val & 0xf;
        } else if adr == 0xe0 {
            self.plus = val & 1 != 0;
        } else if adr == 0xe1 {
            self.ch_enable_next = val & 0x1f;
        } else if adr == 0xe2 {
            self.cycle_4bit = val & 1 != 0;
            self.cycle_8bit = val & 2 != 0;
            self.refresh = val & 32 != 0;
            self.rotate = if val & 64 != 0 { [0x1f; 5] } else { [0; 5] };
            if val & 128 != 0 {
                self.rotate[3] = 0x1f;
                self.rotate[4] = 0x1f;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_square_wave_sounds() {
        let mut scc = Scc::new(CLOCK, 44100);
        for k in 0..32 {
            scc.write(k, if k < 16 { 0x7f } else { 0x80 });
        }
        scc.write(0xc0, 0xfe);
        scc.write(0xc1, 0x00);
        scc.write(0xd0, 15);
        let s: Vec<i16> = (0..2000).map(|_| scc.calc()).collect();
        assert!(s.iter().any(|&x| x > 1000) && s.iter().any(|&x| x < -1000));
    }

    /// The same samples as emu2212 (exact mode): SCC_SCRIPT "frame reg value"
    /// lines (735 samples a frame), SCC_EXPECT its output, a sample a line.
    #[test]
    #[ignore]
    fn the_same_as_emu2212() {
        let (Ok(script), Ok(expect)) = (std::env::var("SCC_SCRIPT"), std::env::var("SCC_EXPECT")) else {
            return;
        };
        let writes: Vec<(usize, u8, u8)> = std::fs::read_to_string(script)
            .unwrap()
            .lines()
            .map(|l| {
                let v: Vec<usize> = l.split(' ').map(|x| x.parse().unwrap()).collect();
                (v[0], v[1] as u8, v[2] as u8)
            })
            .collect();
        let want: Vec<i16> = std::fs::read_to_string(expect).unwrap().lines().map(|l| l.parse().unwrap()).collect();
        let mut scc = Scc::new(CLOCK, 44100);
        let mut w = writes.iter().peekable();
        for (n, &x) in want.iter().enumerate() {
            if n % 735 == 0 {
                while let Some(&&(f, r, v)) = w.peek() {
                    if f != n / 735 {
                        break;
                    }
                    scc.write(r, v);
                    w.next();
                }
            }
            assert_eq!(scc.calc(), x, "sample {n}");
        }
    }
}
