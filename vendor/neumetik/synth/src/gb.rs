// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! The Game Boy's sound (the DMG's APU), written from the Pan Docs'
//! description: two square channels (the first with a frequency sweep),
//! a wave channel playing 32 four-bit steps and a noise channel (a 15- or
//! 7-bit shift register), each with its length counter, the squares and
//! noise with a volume envelope, run by the 512 Hz frame sequencer, and
//! panned to either side by NR51.

/// The CPU clock the APU runs on.
pub const CLOCK: f64 = 4_194_304.0;

#[derive(Clone, Copy, Default)]
struct Envelope {
    volume: u8,
    up: bool,
    period: u8,
    timer: u8,
}

impl Envelope {
    fn load(&mut self, nrx2: u8) {
        self.volume = nrx2 >> 4;
        self.up = nrx2 & 8 != 0;
        self.period = nrx2 & 7;
        self.timer = self.period;
    }

    fn clock(&mut self) {
        if self.period == 0 {
            return;
        }
        if self.timer > 0 {
            self.timer -= 1;
        }
        if self.timer == 0 {
            self.timer = self.period;
            if self.up && self.volume < 15 {
                self.volume += 1;
            } else if !self.up && self.volume > 0 {
                self.volume -= 1;
            }
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Square {
    on: bool,
    duty: u8,
    length: u16,
    length_on: bool,
    freq: u16,
    timer: u32,
    step: u8,
    env: Envelope,
    nrx2: u8,
    // The sweep (channel 1 only).
    sweep_period: u8,
    sweep_down: bool,
    sweep_shift: u8,
    sweep_timer: u8,
    sweep_on: bool,
    shadow: u16,
}

impl Square {
    fn dac(&self) -> bool {
        self.nrx2 & 0xF8 != 0
    }

    fn sweep_calc(&mut self) -> u16 {
        let d = self.shadow >> self.sweep_shift;
        let f = if self.sweep_down { self.shadow.wrapping_sub(d) } else { self.shadow + d };
        if f > 2047 {
            self.on = false;
        }
        f
    }

    fn clock_sweep(&mut self) {
        if self.sweep_timer > 0 {
            self.sweep_timer -= 1;
        }
        if self.sweep_timer == 0 {
            self.sweep_timer = if self.sweep_period == 0 { 8 } else { self.sweep_period };
            if self.sweep_on && self.sweep_period != 0 {
                let f = self.sweep_calc();
                if f <= 2047 && self.sweep_shift != 0 {
                    self.shadow = f;
                    self.freq = f;
                    self.sweep_calc();
                }
            }
        }
    }

    fn output(&self) -> f32 {
        const DUTY: [u8; 4] = [0b0000_0001, 0b1000_0001, 0b1000_0111, 0b0111_1110];
        if !self.on || !self.dac() {
            return 0.0;
        }
        let high = DUTY[usize::from(self.duty)] >> self.step & 1 != 0;
        let v = f32::from(self.env.volume) / 15.0;
        if high {
            v
        } else {
            -v
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Wave {
    on: bool,
    dac: bool,
    length: u16,
    length_on: bool,
    freq: u16,
    timer: u32,
    pos: u8,
    shift: u8,
}

#[derive(Clone, Copy, Default)]
struct Noise {
    on: bool,
    length: u16,
    length_on: bool,
    env: Envelope,
    nrx2: u8,
    lfsr: u16,
    narrow: bool,
    period: u32,
    timer: u32,
}

/// The APU.
pub struct Gb {
    regs: [u8; 0x30],
    sq: [Square; 2],
    wave: Wave,
    wave_ram: [u8; 16],
    noise: Noise,
    /// The frame sequencer's step, and CPU clocks to its next.
    seq_step: u8,
    seq_timer: u32,
    /// CPU clocks owed to the next output frame.
    owed: f64,
}

impl Default for Gb {
    fn default() -> Gb {
        Gb {
            regs: [0; 0x30],
            sq: [Square::default(); 2],
            wave: Wave::default(),
            wave_ram: [0; 16],
            noise: Noise { lfsr: 0x7FFF, ..Noise::default() },
            seq_step: 0,
            seq_timer: 8192,
            owed: 0.0,
        }
    }
}

impl Gb {
    /// Writes register $FF10 + `reg` (0x00 to 0x2F).
    pub fn write(&mut self, reg: u8, v: u8) {
        let reg = reg & 0x3F;
        if (0x20..0x30).contains(&reg) {
            self.wave_ram[usize::from(reg - 0x20)] = v;
            return;
        }
        if usize::from(reg) >= self.regs.len() {
            return;
        }
        self.regs[usize::from(reg)] = v;
        match reg {
            0x00 => {
                let s = &mut self.sq[0];
                s.sweep_period = (v >> 4) & 7;
                s.sweep_down = v & 8 != 0;
                s.sweep_shift = v & 7;
            }
            0x01 | 0x06 => {
                let s = &mut self.sq[usize::from(reg == 0x06)];
                s.duty = v >> 6;
                s.length = 64 - u16::from(v & 63);
            }
            0x02 | 0x07 => {
                let s = &mut self.sq[usize::from(reg == 0x07)];
                s.nrx2 = v;
                // (A write while it sounds takes its volume at once: what
                // the chip's odd "zombie" handling of it comes to as players use it.)
                if s.on {
                    s.env.load(v);
                }
                if !s.dac() {
                    s.on = false;
                }
            }
            0x03 | 0x08 => {
                let s = &mut self.sq[usize::from(reg == 0x08)];
                s.freq = (s.freq & 0x700) | u16::from(v);
            }
            0x04 | 0x09 => {
                let first = reg == 0x04;
                let s = &mut self.sq[usize::from(!first)];
                s.freq = (s.freq & 0xFF) | (u16::from(v & 7) << 8);
                s.length_on = v & 0x40 != 0;
                if v & 0x80 != 0 {
                    s.on = s.dac();
                    if s.length == 0 {
                        s.length = 64;
                    }
                    s.timer = (2048 - u32::from(s.freq)) * 4;
                    s.env.load(s.nrx2);
                    if first {
                        s.shadow = s.freq;
                        s.sweep_timer = if s.sweep_period == 0 { 8 } else { s.sweep_period };
                        s.sweep_on = s.sweep_period != 0 || s.sweep_shift != 0;
                        if s.sweep_shift != 0 {
                            s.sweep_calc();
                        }
                    }
                }
            }
            0x0A => {
                self.wave.dac = v & 0x80 != 0;
                if !self.wave.dac {
                    self.wave.on = false;
                }
            }
            0x0B => self.wave.length = 256 - u16::from(v),
            0x0C => self.wave.shift = (v >> 5) & 3,
            0x0D => self.wave.freq = (self.wave.freq & 0x700) | u16::from(v),
            0x0E => {
                let w = &mut self.wave;
                w.freq = (w.freq & 0xFF) | (u16::from(v & 7) << 8);
                w.length_on = v & 0x40 != 0;
                if v & 0x80 != 0 {
                    w.on = w.dac;
                    if w.length == 0 {
                        w.length = 256;
                    }
                    w.timer = (2048 - u32::from(w.freq)) * 2;
                    w.pos = 0;
                }
            }
            0x10 => self.noise.length = 64 - u16::from(v & 63),
            0x11 => {
                self.noise.nrx2 = v;
                if self.noise.on {
                    self.noise.env.load(v);
                }
                if v & 0xF8 == 0 {
                    self.noise.on = false;
                }
            }
            0x12 => {
                let n = &mut self.noise;
                let r = u32::from(v & 7);
                let div = if r == 0 { 8 } else { 16 * r };
                n.period = div << (v >> 4);
                n.narrow = v & 8 != 0;
            }
            0x13 => {
                let n = &mut self.noise;
                n.length_on = v & 0x40 != 0;
                if v & 0x80 != 0 {
                    n.on = n.nrx2 & 0xF8 != 0;
                    if n.length == 0 {
                        n.length = 64;
                    }
                    n.env.load(n.nrx2);
                    n.lfsr = 0x7FFF;
                    n.timer = n.period;
                }
            }
            _ => {}
        }
    }

    pub fn reg(&self, reg: u8) -> u8 {
        let reg = usize::from(reg & 0x3F);
        if reg >= 0x20 {
            self.wave_ram[reg - 0x20]
        } else {
            self.regs[reg]
        }
    }

    fn sequencer(&mut self) {
        let s = self.seq_step;
        if s & 1 == 0 {
            for q in self.sq.iter_mut() {
                if q.length_on && q.length > 0 {
                    q.length -= 1;
                    if q.length == 0 {
                        q.on = false;
                    }
                }
            }
            let w = &mut self.wave;
            if w.length_on && w.length > 0 {
                w.length -= 1;
                if w.length == 0 {
                    w.on = false;
                }
            }
            let n = &mut self.noise;
            if n.length_on && n.length > 0 {
                n.length -= 1;
                if n.length == 0 {
                    n.on = false;
                }
            }
        }
        if s == 2 || s == 6 {
            self.sq[0].clock_sweep();
        }
        if s == 7 {
            self.sq[0].env.clock();
            self.sq[1].env.clock();
            self.noise.env.clock();
        }
        self.seq_step = (s + 1) & 7;
    }

    /// Runs `n` CPU clocks.
    fn run(&mut self, mut n: u32) {
        while n > 0 {
            let step = n.min(self.seq_timer).min(4);
            for q in self.sq.iter_mut() {
                let mut left = step;
                while left > 0 {
                    if q.timer == 0 {
                        q.timer = (2048 - u32::from(q.freq)).max(1) * 4;
                        q.step = (q.step + 1) & 7;
                    }
                    let k = left.min(q.timer);
                    q.timer -= k;
                    left -= k;
                }
            }
            let w = &mut self.wave;
            let mut left = step;
            while left > 0 {
                if w.timer == 0 {
                    w.timer = (2048 - u32::from(w.freq)).max(1) * 2;
                    w.pos = (w.pos + 1) & 31;
                }
                let k = left.min(w.timer);
                w.timer -= k;
                left -= k;
            }
            let ns = &mut self.noise;
            let mut left = step;
            while left > 0 && ns.period > 0 {
                if ns.timer == 0 {
                    ns.timer = ns.period;
                    let bit = (ns.lfsr ^ (ns.lfsr >> 1)) & 1;
                    ns.lfsr = (ns.lfsr >> 1) | (bit << 14);
                    if ns.narrow {
                        ns.lfsr = (ns.lfsr & !0x40) | (bit << 6);
                    }
                }
                let k = left.min(ns.timer);
                ns.timer -= k;
                left -= k;
            }
            self.seq_timer -= step;
            if self.seq_timer == 0 {
                self.seq_timer = 8192;
                self.sequencer();
            }
            n -= step;
        }
    }

    /// Each channel's output now (-1 to 1, before panning).
    fn sample(&self) -> [f32; 4] {
        let w = &self.wave;
        let wave = if w.on && w.dac && w.shift != 0 {
            let b = self.wave_ram[usize::from(w.pos >> 1)];
            let nib = if w.pos & 1 == 0 { b >> 4 } else { b & 15 };
            (f32::from(nib >> (w.shift - 1)) / 7.5) - 1.0
        } else {
            0.0
        };
        let n = &self.noise;
        let noise = if n.on && n.nrx2 & 0xF8 != 0 {
            let v = f32::from(n.env.volume) / 15.0;
            if n.lfsr & 1 == 0 {
                v
            } else {
                -v
            }
        } else {
            0.0
        };
        [self.sq[0].output(), self.sq[1].output(), wave, noise]
    }

    /// The next frames at `rate`: each channel's left and right (NR51's
    /// panning applied, NR50's master volume not).
    pub fn render(&mut self, rate: u32, left: &mut [&mut [f32]; 4], right: &mut [&mut [f32]; 4]) {
        let per = CLOCK / f64::from(rate.max(1));
        let n = left[0].len();
        let pan = self.regs[0x15];
        for i in 0..n {
            self.owed += per;
            let clocks = self.owed as u32;
            self.owed -= f64::from(clocks);
            // The channels' levels averaged over the frame, a step at a time.
            let mut acc = [0f32; 4];
            let mut done = 0;
            while done < clocks {
                let k = (clocks - done).min(16);
                self.run(k);
                let s = self.sample();
                for c in 0..4 {
                    acc[c] += s[c] * k as f32;
                }
                done += k;
            }
            let d = clocks.max(1) as f32;
            for c in 0..4 {
                let v = acc[c] / d;
                left[c][i] = if pan & (0x10 << c) != 0 { v } else { 0.0 };
                right[c][i] = if pan & (1 << c) != 0 { v } else { 0.0 };
            }
        }
    }
}
