// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.
//
// Ported from libopna, part of 98fmplayer (github.com/takamichih/fmplayer),
// under its BSD 2-Clause License:
//
// Copyright (c) 2016, Takamichi Horikawa
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// * Redistributions of source code must retain the above copyright notice, this
//   list of conditions and the following disclaimer.
//
// * Redistributions in binary form must reproduce the above copyright notice,
//   this list of conditions and the following disclaimer in the documentation
//   and/or other materials provided with the distribution.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
// FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
// DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
// SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
// CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
// OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
// OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

//! The Yamaha YM2608 (OPNA), as the PC-98's sound board carries it: six
//! 4-operator FM channels (channel 3's operators on their own frequencies
//! in its special mode), the SSG (three square-wave channels, noise and an
//! envelope; as the OPNA's analog output through a band-limited resampler,
//! or as the YMF288 sums it), the six rhythm sounds (from the chip's ROM,
//! which isn't shipped: the user's own, else silent), the ADPCM channel
//! (its 256 KB of RAM) and the two timers that pace a sound driver. All at
//! the chip's 55,467 Hz (7.9872 MHz over 144).
//!
//! A port of libopna (98fmplayer's), which plays the FM bit for bit with
//! the chip in most cases. Its mix is kept exact (`mix`, clamping as it
//! goes, as libopna does); each channel's part is also kept apart
//! (`Taps`), for a host to pan, mute and level them.

/// The chip's output rate.
pub const RATE: u32 = 55_467;

const ENV_MAX: i32 = 1023;
const ENV_ATTACK: u8 = 0;
const ENV_DECAY: u8 = 1;
const ENV_SUSTAIN: u8 = 2;
const ENV_RELEASE: u8 = 3;
const ENV_OFF: u8 = 4;

const CH3_NORMAL: u8 = 0;

const RATEINC: [[u8; 8]; 8] = [
    [1, 0, 1, 0, 1, 0, 1, 0],
    [1, 0, 1, 0, 1, 1, 1, 0],
    [1, 0, 1, 1, 1, 0, 1, 1],
    [1, 0, 1, 1, 1, 1, 1, 1],
    [1, 1, 1, 1, 1, 1, 1, 1],
    [1, 1, 1, 2, 1, 1, 1, 2],
    [1, 2, 1, 2, 1, 2, 1, 2],
    [1, 2, 2, 2, 1, 2, 2, 2],
];

const DETUNE: [[u8; 32]; 4] = [
    [0; 32],
    [0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 3, 3, 3, 4, 4, 4, 5, 5, 6, 6, 7, 8, 8, 8, 8],
    [1, 1, 1, 1, 2, 2, 2, 2, 2, 3, 3, 3, 4, 4, 4, 5, 5, 6, 6, 7, 8, 8, 9, 10, 11, 12, 13, 14, 16, 16, 16, 16],
    [2, 2, 2, 2, 2, 3, 3, 3, 4, 4, 4, 5, 5, 6, 6, 7, 8, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 20, 22, 22, 22, 22],
];

/// The FM's log-sine and exponent tables, as libopna computes them:
/// round(-256·log2(sin((2i+1)π/1024))), round(2048 / 2^((i+1)/256)).
struct Tables {
    logsin: [u16; 256],
    exp: [u16; 256],
}

fn tables() -> &'static Tables {
    use std::sync::OnceLock;
    static T: OnceLock<Tables> = OnceLock::new();
    T.get_or_init(|| {
        let mut logsin = [0u16; 256];
        let mut exp = [0u16; 256];
        for i in 0..256 {
            logsin[i] = (-256.0 * ((2 * i + 1) as f64 * std::f64::consts::PI / 1024.0).sin().log2()).round() as u16;
            exp[i] = (2048.0 / 2f64.powf((i as f64 + 1.0) / 256.0)).round() as u16;
        }
        Tables { logsin, exp }
    })
}

// ---- FM ----

#[derive(Clone, Copy, Default)]
struct Slot {
    phase: u32,
    env: i32,
    env_count: u16,
    env_state: u8,
    rate_shifter: u8,
    rate_selector: u8,
    rate_mul: u8,
    tl: u8,
    sl: u8,
    ar: u8,
    dr: u8,
    sr: u8,
    rr: u8,
    mul: u8,
    det: u8,
    ks: u8,
    keycode: u8,
    keyon_ext: bool,
    keyon: bool,
    prevout: i16,
}

impl Slot {
    fn reset() -> Slot {
        Slot {
            env: ENV_MAX,
            env_state: ENV_RELEASE,
            ..Default::default()
        }
    }

    fn out(&mut self, modulation: i16) -> i16 {
        let t = tables();
        let mut pind = (self.phase >> 10).wrapping_add((i32::from(modulation) >> 1) as u32);
        let minus = pind & (1 << 9) != 0;
        if pind & (1 << 8) != 0 {
            pind = !pind;
        }
        pind &= 255;
        let mut logout = i32::from(t.logsin[pind as usize]);
        logout += self.env << 2;
        logout += i32::from(self.tl) << 5;
        let selector = logout & 255;
        let shifter = (logout >> 8).min(13);
        let mut out = ((i32::from(t.exp[selector as usize]) << 2) >> shifter) as i16;
        if minus {
            out = out.wrapping_neg();
        }
        self.prevout = out;
        out
    }

    fn phase(&mut self, freq: u32) {
        let mut det = u32::from(DETUNE[usize::from(self.det & 3)][usize::from(self.keycode & 31)]);
        if self.det & 4 != 0 {
            det = det.wrapping_neg();
        }
        let freq = freq.wrapping_add(det) & ((1 << 17) - 1);
        let mut mul = u32::from(self.mul) << 1;
        if mul == 0 {
            mul = 1;
        }
        self.phase = self.phase.wrapping_add(freq.wrapping_mul(mul) >> 1);
    }

    fn set_rate(&mut self, status: u8) {
        let r: i32 = match status {
            ENV_ATTACK => i32::from(self.ar),
            ENV_DECAY => i32::from(self.dr),
            ENV_SUSTAIN => i32::from(self.sr),
            ENV_RELEASE => i32::from(self.rr) * 2 + 1,
            _ => return,
        };
        if r == 0 {
            self.rate_selector = 0;
            self.rate_mul = 0;
            self.rate_shifter = 0;
            return;
        }
        let mut rate = 2 * r + (i32::from(self.keycode) >> (3 - self.ks));
        if rate > 63 {
            rate = 63;
        }
        if status == ENV_ATTACK && rate >= 62 {
            rate += 4;
        }
        let shifter = 11 - (rate >> 2);
        if shifter < 0 {
            self.rate_selector = ((rate & 3) + 4) as u8;
            self.rate_mul = (1 << (-shifter - 1)) as u8;
            self.rate_shifter = 0;
        } else {
            self.rate_selector = (rate & 3) as u8;
            self.rate_mul = 1;
            self.rate_shifter = shifter as u8;
        }
    }

    fn env(&mut self) {
        let shifter = u32::from(self.rate_shifter);
        let mask = (1u32 << shifter) - 1;
        if u32::from(self.env_count) & mask == mask {
            let index = (u32::from(self.env_count) >> shifter) & 7;
            let inc = i32::from(RATEINC[usize::from(self.rate_selector & 7)][index as usize]) * i32::from(self.rate_mul);
            match self.env_state {
                ENV_ATTACK => {
                    let newenv = self.env + (((-self.env - 1) * inc) >> 4);
                    if newenv <= 0 {
                        self.env = 0;
                        self.env_state = ENV_DECAY;
                        self.set_rate(ENV_DECAY);
                    } else {
                        self.env = newenv;
                    }
                }
                ENV_DECAY => {
                    self.env += inc;
                    let sl = if self.sl == 0xf { 0x1f } else { i32::from(self.sl) };
                    if self.env >= sl << 5 {
                        self.env_state = ENV_SUSTAIN;
                        self.set_rate(ENV_SUSTAIN);
                    }
                }
                ENV_SUSTAIN => {
                    self.env += inc;
                    if self.env >= ENV_MAX {
                        self.env = ENV_MAX;
                    }
                }
                ENV_RELEASE => {
                    self.env += inc;
                    if self.env >= ENV_MAX {
                        self.env = ENV_MAX;
                        self.env_state = ENV_OFF;
                    }
                }
                _ => {}
            }
        }
        self.env_count = self.env_count.wrapping_add(1);
    }

    fn key(&mut self, on: bool) {
        if on {
            if !self.keyon {
                self.keyon = true;
                self.env_state = ENV_ATTACK;
                self.env_count = 0;
                self.phase = 0;
                self.prevout = 0;
                self.set_rate(ENV_ATTACK);
            }
        } else if self.env_state != ENV_OFF && self.keyon {
            self.keyon = false;
            self.env_state = ENV_RELEASE;
            self.set_rate(ENV_RELEASE);
        }
    }
}

#[derive(Clone, Copy)]
struct Channel {
    slot: [Slot; 4],
    fbmem: u16,
    alg_mem: u16,
    alg: u8,
    fb: u8,
    fnum: u16,
    blk: u8,
}

impl Channel {
    fn reset() -> Channel {
        Channel {
            slot: [Slot::reset(); 4],
            fbmem: 0,
            alg_mem: 0,
            alg: 0,
            fb: 0,
            fnum: 0,
            blk: 0,
        }
    }

    fn set_blkfnum(&mut self, blk: u16, fnum: u16) {
        self.blk = (blk & 7) as u8;
        self.fnum = fnum & 0x7ff;
        let kc = keycode(self.blk.into(), self.fnum);
        for s in &mut self.slot {
            s.keycode = kc;
            let st = s.env_state;
            s.set_rate(st);
        }
    }

    /// One sample: (as libopna's data[0], data[1]).
    fn out(&mut self) -> [i16; 2] {
        let s0 = self.slot[0].prevout;
        let s1 = self.slot[1].prevout;
        let s2 = self.slot[2].prevout;
        let s3 = self.slot[3].prevout;
        let mut fb = (i32::from(self.fbmem) + i32::from(self.slot[0].prevout)) as i16;
        self.fbmem = s0 as u16;
        if self.fb == 0 {
            fb = 0;
        }
        self.slot[0].out(((i32::from(fb)) >> (9 - i32::from(self.fb))) as i16);
        let prev_alg_mem = self.alg_mem as i16;
        let h = |x: i16| i32::from(x) >> 1;
        let r: [i16; 2];
        match self.alg {
            0 => {
                let p0 = self.slot[0].prevout;
                self.slot[1].out(p0);
                self.slot[2].out(s1);
                self.slot[3].out(s2);
                let v = (self.slot[3].prevout >> 1) as i16;
                r = [v, v];
            }
            1 => {
                self.slot[1].out(0);
                self.slot[2].out(prev_alg_mem);
                self.slot[3].out(s2);
                let m = (self.slot[0].prevout as u16).wrapping_add(self.slot[1].prevout as u16) & !1;
                self.alg_mem = m;
                let v = self.slot[3].prevout >> 1;
                r = [v, v];
            }
            2 => {
                self.slot[1].out(0);
                self.slot[2].out(s1);
                self.slot[3].out((i32::from(s0) + i32::from(s2)) as i16);
                let v = self.slot[3].prevout >> 1;
                r = [v, v];
            }
            3 => {
                let p0 = self.slot[0].prevout;
                self.slot[1].out(p0);
                self.slot[2].out(0);
                self.slot[3].out((i32::from(s2) + i32::from(prev_alg_mem)) as i16);
                self.alg_mem = s1 as u16;
                let v = self.slot[3].prevout >> 1;
                r = [v, v];
            }
            4 => {
                self.slot[1].out(s0);
                self.slot[2].out(0);
                let p2 = self.slot[2].prevout;
                self.slot[3].out(p2);
                let base = h(s3);
                r = [(base + h(self.slot[1].prevout)) as i16, (base + h(s1)) as i16];
            }
            5 => {
                self.slot[1].out(s0);
                self.slot[2].out(s0);
                self.slot[3].out(s0);
                self.alg_mem = (s2 as u16) & !1;
                let base = h(s3);
                r = [(base + h(self.slot[1].prevout) + h(s2)) as i16, (base + h(s1) + h(prev_alg_mem)) as i16];
            }
            6 => {
                self.slot[1].out(s0);
                self.slot[2].out(0);
                self.slot[3].out(0);
                self.alg_mem = (s2 as u16) & !1;
                let base = h(s3);
                r = [(base + h(self.slot[1].prevout) + h(s2)) as i16, (base + h(s1) + h(prev_alg_mem)) as i16];
            }
            _ => {
                self.slot[1].out(0);
                self.slot[2].out(0);
                self.slot[3].out(0);
                self.alg_mem = ((i32::from(self.slot[1].prevout) + i32::from(self.slot[2].prevout)) as u16) & !1;
                let base = h(self.slot[0].prevout) + h(self.slot[3].prevout);
                let d0 = (base + (i32::from(self.alg_mem) >> 1)) as i16;
                let d1 = (base + h(prev_alg_mem)) as i16;
                // Sign-extended from 15 bits, as libopna's shifts make it.
                r = [((d0 as i32) << 1) as i16 >> 1, ((d1 as i32) << 1) as i16 >> 1];
            }
        }
        r
    }

    fn phase(&mut self) {
        let freq = (u32::from(self.fnum) << self.blk) >> 1;
        for s in &mut self.slot {
            s.phase(freq);
        }
    }
}

fn keycode(blk: u16, fnum: u16) -> u8 {
    let f = |n: u32| fnum & (1 << (n - 1)) != 0;
    let mut kc = (blk << 2) as u8;
    kc |= u8::from(f(11)) << 1;
    kc |= u8::from((f(11) && (f(10) || f(9) || f(8))) || (!f(11) && f(10) && f(9) && f(8)));
    kc
}

#[derive(Clone)]
pub struct Fm {
    channel: [Channel; 6],
    blkfnum_h: u8,
    ch3_fnum: [u16; 3],
    ch3_blk: [u8; 3],
    ch3_mode: u8,
    env_div3: u8,
    lselect: [bool; 6],
    rselect: [bool; 6],
    pub mask: u32,
}

impl Fm {
    fn new() -> Fm {
        Fm {
            channel: [Channel::reset(); 6],
            blkfnum_h: 0,
            ch3_fnum: [0; 3],
            ch3_blk: [0; 3],
            ch3_mode: CH3_NORMAL,
            env_div3: 0,
            lselect: [true; 6],
            rselect: [true; 6],
            mask: 0,
        }
    }

    fn writereg(&mut self, reg: u32, val: u32) {
        let val = val & 0xff;
        if reg > 0x1ff {
            return;
        }
        match reg {
            0x27 => {
                let mode = (val >> 6) as u8;
                if mode != self.ch3_mode {
                    self.ch3_mode = mode;
                    for c in 0..2 {
                        let (blk, fnum) = if self.ch3_mode == CH3_NORMAL {
                            (self.channel[2].blk, self.channel[2].fnum)
                        } else {
                            (self.ch3_blk[c], self.ch3_fnum[c])
                        };
                        let s = &mut self.channel[2].slot[c];
                        s.keycode = keycode(blk.into(), fnum);
                        let st = s.env_state;
                        s.set_rate(st);
                    }
                }
                return;
            }
            0x28 => {
                let mut c = (val & 3) as usize;
                if c == 3 {
                    return;
                }
                if val & 4 != 0 {
                    c += 3;
                }
                for i in 0..4 {
                    let on = val & (1 << (4 + i)) != 0;
                    self.channel[c].slot[i].keyon_ext = on;
                    if !on {
                        self.channel[c].slot[i].key(false);
                    }
                }
                return;
            }
            _ => {}
        }
        let mut c = (reg & 3) as usize;
        if c == 3 {
            return;
        }
        if reg & (1 << 8) != 0 {
            c += 3;
        }
        let s = (((reg & (1 << 3)) >> 3) | ((reg & (1 << 2)) >> 1)) as usize;
        match reg & 0xf0 {
            0x30 => {
                let sl = &mut self.channel[c].slot[s];
                sl.det = ((val >> 4) & 7) as u8;
                sl.mul = (val & 0xf) as u8;
            }
            0x40 => self.channel[c].slot[s].tl = (val & 0x7f) as u8,
            0x50 => {
                let sl = &mut self.channel[c].slot[s];
                sl.ks = ((val >> 6) & 3) as u8;
                sl.ar = (val & 0x1f) as u8;
                if sl.env_state == ENV_ATTACK {
                    sl.set_rate(ENV_ATTACK);
                }
            }
            0x60 => {
                let sl = &mut self.channel[c].slot[s];
                sl.dr = (val & 0x1f) as u8;
                if sl.env_state == ENV_DECAY {
                    sl.set_rate(ENV_DECAY);
                }
            }
            0x70 => {
                let sl = &mut self.channel[c].slot[s];
                sl.sr = (val & 0x1f) as u8;
                if sl.env_state == ENV_SUSTAIN {
                    sl.set_rate(ENV_SUSTAIN);
                }
            }
            0x80 => {
                let sl = &mut self.channel[c].slot[s];
                sl.sl = ((val >> 4) & 0xf) as u8;
                sl.rr = (val & 0xf) as u8;
                if sl.env_state == ENV_RELEASE {
                    sl.set_rate(ENV_RELEASE);
                }
            }
            0xa0 => {
                let blk = u16::from((self.blkfnum_h >> 3) & 7);
                let fnum = (u16::from(self.blkfnum_h & 7) << 8) | (val & 0xff) as u16;
                match reg & 0xc {
                    0x0 => {
                        if c != 2 || self.ch3_mode == CH3_NORMAL {
                            self.channel[c].set_blkfnum(blk, fnum);
                        } else {
                            let ch = &mut self.channel[c];
                            ch.blk = blk as u8;
                            ch.fnum = fnum;
                            ch.slot[3].keycode = keycode(blk, fnum);
                            let st = ch.slot[3].env_state;
                            ch.slot[3].set_rate(st);
                        }
                    }
                    0x8 => {
                        let c = (c + 2) % 3;
                        self.ch3_blk[c] = blk as u8;
                        self.ch3_fnum[c] = fnum;
                        if self.ch3_mode != CH3_NORMAL {
                            let s = &mut self.channel[2].slot[c];
                            s.keycode = keycode(blk, fnum);
                            let st = s.env_state;
                            s.set_rate(st);
                        }
                    }
                    _ => self.blkfnum_h = (val & 0x3f) as u8,
                }
            }
            0xb0 => match reg & 0xc {
                0x0 => {
                    self.channel[c].alg = (val & 7) as u8;
                    self.channel[c].fb = ((val >> 3) & 7) as u8;
                }
                0x4 => {
                    self.lselect[c] = val & 0x80 != 0;
                    self.rselect[c] = val & 0x40 != 0;
                }
                _ => {}
            },
            _ => {}
        }
    }

    /// One sample of each channel: [left, right] as it reaches the mix
    /// (none where a channel is masked or panned away).
    fn sample(&mut self, out: &mut [[i32; 2]; 6]) {
        if self.env_div3 == 0 {
            for ch in &mut self.channel {
                for s in &mut ch.slot {
                    if s.keyon_ext {
                        s.key(true);
                        s.env();
                    }
                }
            }
        }
        for c in 0..6 {
            let o = self.channel[c].out();
            if c == 2 && self.ch3_mode != CH3_NORMAL {
                let ch = &mut self.channel[2];
                for k in 0..3 {
                    let f = (u32::from(self.ch3_fnum[k]) << self.ch3_blk[k]) >> 1;
                    ch.slot[k].phase(f);
                }
                let f = (u32::from(ch.fnum) << ch.blk) >> 1;
                ch.slot[3].phase(f);
            } else {
                self.channel[c].phase();
            }
            out[c] = [0, 0];
            if self.mask & (1 << c) != 0 {
                continue;
            }
            if self.lselect[c] {
                out[c][0] = i32::from(o[1]);
            }
            if self.rselect[c] {
                out[c][1] = i32::from(o[0]);
            }
        }
        if self.env_div3 == 0 {
            for ch in &mut self.channel {
                for s in &mut ch.slot {
                    if s.keyon_ext {
                        s.keyon_ext = false;
                    } else {
                        s.env();
                    }
                }
            }
            self.env_div3 = 3;
        }
        self.env_div3 -= 1;
    }
}

// ---- SSG ----

const VOLTABLE: [u16; 32] = [
    0, 0, 0, 0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 44, 52, 64, 76, 92, 108, 128, 152, 180, 216, 256, 304, 360, 428, 512, 608, 720, 856, 1020,
];

const SINC: [i16; 256] = [
    1, -1, -3, -6, -6, -2, 7, 16, 20, 13, -5, -29, -44, -40, -11, 36, 79, 91, 56, -22, -112, -166, -144, -37, 119, 251, 280, 166, -64, -315, -455, -385, -96, 306, 632, 694, 405, -154, -744, -1062,
    -889, -220, 692, 1421, 1552, 902, -343, -1655, -2369, -1994, -498, 1585, 3306, 3690, 2206, -868, -4391, -6670, -6067, -1681, 6178, 15928, 25133, 31275, 32600, 28724, 20765, 10957, 1886, -4359,
    -6809, -5798, -2673, 815, 3185, 3714, 2557, 523, -1365, -2333, -2125, -1032, 328, 1322, 1580, 1108, 230, -607, -1043, -954, -464, 147, 593, 705, 491, 101, -264, -450, -406, -195, 61, 241, 282,
    193, 39, -100, -166, -146, -68, 21, 80, 90, 60, 12, -29, -45, -38, -17, 5, 18, 19, 11, 2, -5, -6, -5, -2, 0, 0, -2, -5, -6, -5, 2, 11, 19, 18, 5, -17, -38, -45, -29, 12, 60, 90, 80, 21, -68,
    -146, -166, -100, 39, 193, 282, 241, 61, -195, -406, -450, -264, 101, 491, 705, 593, 147, -464, -954, -1043, -607, 230, 1108, 1580, 1322, 328, -1032, -2125, -2333, -1365, 523, 2557, 3714, 3185,
    815, -2673, -5798, -6809, -4359, 1886, 10957, 20765, 28724, 32600, 31275, 25133, 15928, 6178, -1681, -6067, -6670, -4391, -868, 2206, 3690, 3306, 1585, -498, -1994, -2369, -1655, -343, 902, 1552,
    1421, 692, -220, -889, -1062, -744, -154, 405, 694, 632, 306, -96, -385, -455, -315, -64, 166, 280, 251, 119, -37, -144, -166, -112, -22, 56, 91, 79, 36, -11, -40, -44, -29, -5, 13, 20, 16, 7,
    -2, -6, -6, -3, -1, 1,
];

const SINCLEN: usize = 128;

#[derive(Clone)]
pub struct Ssg {
    regs: [u8; 16],
    tone_counter: [u16; 3],
    out: [bool; 3],
    noise_counter: u8,
    lfsr: u32,
    env_counter: u16,
    env_level: u8,
    env_att: bool,
    env_alt: bool,
    env_hld: bool,
    env_holding: bool,
    pub mask: u32,
    previn: [i32; 3],
    prevout: [i32; 3],
    /// The SSG's level in the analog mix (0x10000 as the PC-9801-86 has it).
    pub mix: u32,
    /// Summed as the YMF288 sums it, rather than the OPNA's analog output.
    pub ymf288: bool,
    buf: Vec<i16>,
    index: u32,
}

impl Ssg {
    fn new() -> Ssg {
        Ssg {
            regs: [0; 16],
            tone_counter: [0; 3],
            out: [false; 3],
            noise_counter: 0,
            lfsr: 0,
            env_counter: 0,
            env_level: 0,
            env_att: false,
            env_alt: false,
            env_hld: false,
            env_holding: false,
            mask: 0,
            previn: [0; 3],
            prevout: [0; 3],
            mix: 0x10000,
            ymf288: true,
            buf: vec![0; SINCLEN * 4 * 2],
            index: 0,
        }
    }

    /// The YMF288's sum or the OPNA's analog output (the resampler's
    /// memory cleared when it changes, as libopna does).
    pub fn set_ymf288(&mut self, on: bool) {
        if self.ymf288 != on {
            self.ymf288 = on;
            self.buf.iter_mut().for_each(|x| *x = 0);
        }
    }

    fn writereg(&mut self, reg: u32, val: u32) {
        if reg > 0xf {
            return;
        }
        self.regs[reg as usize] = val as u8;
        if reg == 0xd {
            let r = self.regs[0xd];
            self.env_att = r & 4 != 0;
            if r & 8 != 0 {
                self.env_alt = r & 2 != 0;
                self.env_hld = r & 1 != 0;
            } else {
                self.env_alt = self.env_att;
                self.env_hld = true;
            }
            self.env_holding = false;
            self.env_level = 0;
            self.env_counter = 0;
        }
    }

    fn tone_period(&self, ch: usize) -> u16 {
        u16::from(self.regs[ch * 2]) | (u16::from(self.regs[ch * 2 + 1] & 0xf) << 8)
    }

    fn level(&self, ch: usize) -> usize {
        if self.regs[8 + ch] & 0x10 != 0 {
            usize::from(if self.env_att { self.env_level } else { 31 - self.env_level })
        } else {
            usize::from((self.regs[8 + ch] & 0xf) << 1) + 1
        }
    }

    /// One sample at the SSG's own rate (the clock over 32), each channel.
    fn raw(&mut self) -> [i16; 3] {
        self.noise_counter = self.noise_counter.wrapping_add(1);
        if (self.noise_counter >> 1) >= self.regs[6] & 0x1f {
            self.noise_counter = 0;
            self.lfsr |= u32::from((self.lfsr & 1) ^ ((self.lfsr >> 3) & 1) == 0) << 17;
            self.lfsr >>= 1;
        }
        if !self.env_holding {
            self.env_counter = self.env_counter.wrapping_add(1);
            if i32::from(self.env_counter) >= ((i32::from(self.regs[0xc]) << 8) | i32::from(self.regs[0xb])) {
                self.env_counter = 0;
                self.env_level += 1;
                if self.env_level == 0x20 {
                    self.env_level = 0;
                    if self.env_alt {
                        self.env_att = !self.env_att;
                    }
                    if self.env_hld {
                        self.env_level = 0x1f;
                        self.env_holding = true;
                    }
                }
            }
        }
        let mut out = [0i16; 3];
        for ch in 0..3 {
            self.tone_counter[ch] = self.tone_counter[ch].wrapping_add(1);
            if self.tone_counter[ch] >= self.tone_period(ch) {
                self.tone_counter[ch] = 0;
                self.out[ch] = !self.out[ch];
            }
            let reg = self.regs[7] >> ch;
            if !self.ymf288 {
                let mut prev = 0i32;
                if (self.out[ch] || reg & 1 != 0) && (self.lfsr & 1 != 0 || reg & 8 != 0) {
                    prev = i32::from(VOLTABLE[self.level(ch)]) * 5;
                }
                prev *= 0x3fff;
                self.prevout[ch] = prev - self.previn[ch] + ((0x3fffi64 * i64::from(self.prevout[ch])) >> 14) as i32;
                self.previn[ch] = prev;
                out[ch] = (self.prevout[ch] >> 14) as i16;
            } else {
                let level = self.level(ch);
                if !(reg & 1 != 0 && reg & 8 != 0) {
                    let tone = if self.tone_period(ch) < 8 { true } else { self.out[ch] };
                    let on = (tone || reg & 1 != 0) && (self.lfsr & 1 != 0 || reg & 8 != 0);
                    out[ch] = if on { VOLTABLE[level] as i16 } else { -(VOLTABLE[level] as i16) };
                } else {
                    out[ch] = (VOLTABLE[level] * 2) as i16;
                }
            }
        }
        out
    }

    /// One sample at the chip's rate, each channel (none where masked).
    fn sample(&mut self, out: &mut [i32; 3]) {
        let bi = |index: u32, n: u32| ((((index) >> 1) + n) as usize) & (SINCLEN - 1);
        let n = ((self.index + 9) >> 1) - (self.index >> 1);
        for j in 0..n {
            let r = self.raw();
            let b = bi(self.index, j);
            self.buf[b * 4..b * 4 + 3].copy_from_slice(&r);
        }
        self.index += 9;
        self.index &= (1 << 8) - 1;
        let (lo, hi) = self.buf.split_at_mut(SINCLEN * 4);
        hi.copy_from_slice(lo);
        let mut ob = [0i32; 3];
        if !self.ymf288 {
            for (c, o) in ob.iter_mut().enumerate() {
                let mut acc = 0i32;
                for j in 0..SINCLEN {
                    let si = if self.index & 1 == 0 { j + SINCLEN } else { j };
                    acc = acc.wrapping_add(i32::from(self.buf[(((self.index as usize) >> 1) + j) * 4 + c]) * i32::from(SINC[si]));
                }
                let mut x = acc >> 16;
                x = x.wrapping_mul(13000) >> 16;
                x = ((x as u32).wrapping_mul(self.mix) as i32) >> 16;
                *o = x;
            }
        } else {
            for (c, o) in ob.iter_mut().enumerate() {
                let ind = if self.index & 1 != 0 { bi(self.index, 5) } else { bi(self.index, 0) };
                let mut x = i32::from(self.buf[ind * 4 + c]);
                for s in 0..4 {
                    x += i32::from(self.buf[bi(self.index, s + 1) * 4 + c]) * 2;
                }
                *o = x / 9;
            }
        }
        for c in 0..3 {
            out[c] = if self.mask & (1 << c) == 0 { ob[c] } else { 0 };
        }
    }
}

// ---- Rhythm ----

const ROM_STARTS: [(usize, usize, usize); 6] = [
    (0x0000, 0x01c0, 3),
    (0x01c0, 0x0440, 3),
    (0x0440, 0x1b80, 3),
    (0x1b80, 0x1d00, 3),
    (0x1d00, 0x1f80, 6),
    (0x1f80, 0x2000, 6),
];

const STEPS: [u16; 49] = [
    16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66, 73, 80, 88, 97, 107, 118, 130, 143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408, 449, 494, 544, 598, 658, 724, 796, 876,
    963, 1060, 1166, 1282, 1411, 1552,
];
const STEP_INC: [i8; 8] = [-1, -1, -1, -1, 2, 5, 7, 9];

#[derive(Clone, Default)]
struct Drum {
    data: Vec<i16>,
    playing: bool,
    index: usize,
    level: u32,
    left: bool,
    right: bool,
}

#[derive(Clone, Default)]
pub struct Rhythm {
    drums: [Drum; 6],
    total_level: u32,
    pub mask: u32,
}

impl Rhythm {
    /// The YM2608's rhythm ROM (8 KB), decoded into its six sounds.
    pub fn set_rom(&mut self, rom: &[u8]) {
        if rom.len() < 0x2000 {
            return;
        }
        for (p, &(start, end, div)) in ROM_STARTS.iter().enumerate() {
            let d = &mut self.drums[p];
            d.playing = false;
            d.index = 0;
            d.data.clear();
            let mut addr = start << 1;
            let mut step = 0i32;
            let mut acc: u32 = 0;
            loop {
                if (addr >> 1) == end - 1 {
                    break;
                }
                let mut data = u32::from(rom[addr >> 1]);
                if addr & 1 == 0 {
                    data >>= 4;
                }
                data &= 0xf;
                let mut diff = (((((data & 7) << 1) | 1) * u32::from(STEPS[step as usize])) >> 3) as i32;
                if data & 8 != 0 {
                    diff = -diff;
                }
                acc = acc.wrapping_add(diff as u32);
                step = (step + i32::from(STEP_INC[(data & 7) as usize])).clamp(0, 48);
                addr += 1;
                let mut out = (acc & 0xfff) as i32;
                if out >= 1 << 11 {
                    out -= 1 << 12;
                }
                let out16 = (out << 4) as i16;
                for _ in 0..div {
                    d.data.push(out16);
                }
            }
        }
    }

    fn writereg(&mut self, reg: u32, val: u32) {
        match reg {
            0x10 => {
                for d in 0..6 {
                    if val & (1 << d) != 0 {
                        self.drums[d].playing = val & 0x80 == 0;
                        self.drums[d].index = 0;
                    }
                }
            }
            0x11 => self.total_level = val & 0x3f,
            0x18..=0x1d => {
                let d = &mut self.drums[(reg - 0x18) as usize];
                d.left = val & 0x80 != 0;
                d.right = val & 0x40 != 0;
                d.level = val & 0x1f;
            }
            _ => {}
        }
    }

    /// One sample of each sound: [left, right].
    fn sample(&mut self, out: &mut [[i32; 2]; 6]) {
        for (k, d) in self.drums.iter_mut().enumerate() {
            out[k] = [0, 0];
            if d.playing && !d.data.is_empty() {
                let mut co = i32::from(d.data[d.index]) >> 4;
                let level = (d.level ^ 0x1f) + (self.total_level ^ 0x3f);
                co *= 15 - (level & 7) as i32;
                co >>= 1 + (level >> 3);
                if self.mask & (1 << k) == 0 {
                    if d.left {
                        out[k][0] = co;
                    }
                    if d.right {
                        out[k][1] = co;
                    }
                }
                d.index += 1;
                if d.index == d.data.len() {
                    d.index = 0;
                    d.playing = false;
                }
            }
        }
    }
}

// ---- ADPCM ----

const ADPCM_TABLE: [u8; 8] = [57, 57, 57, 57, 77, 102, 128, 153];
pub const ADPCM_RAM: usize = 1 << 18;

#[derive(Clone)]
pub struct Adpcm {
    control1: u8,
    control2: u8,
    vol: u8,
    delta: u16,
    start: u16,
    end: u16,
    limit: u16,
    ramptr: u32,
    step: u16,
    pub ram: Vec<u8>,
    acc: i16,
    prev_acc: i16,
    adpcmd: u16,
    out: i16,
    pub masked: bool,
}

impl Adpcm {
    fn new() -> Adpcm {
        Adpcm {
            control1: 0,
            control2: 0,
            vol: 0,
            delta: 0,
            start: 0,
            end: 0,
            limit: 0,
            ramptr: 0,
            step: 0,
            ram: vec![0; ADPCM_RAM],
            acc: 0,
            prev_acc: 0,
            adpcmd: 127,
            out: 0,
            masked: false,
        }
    }

    fn conv(&self, a: u16) -> u32 {
        if self.control2 & 2 != 0 {
            u32::from(a) << 6
        } else {
            u32::from(a) << 3
        }
    }

    fn conv_e(&self, a: u16) -> u32 {
        let a = u32::from(a) + 1;
        (if self.control2 & 2 != 0 { a << 6 } else { a << 3 }).wrapping_sub(1)
    }

    fn calc(&mut self) {
        let step = u32::from(self.step) + u32::from(self.delta);
        self.step = step as u16;
        if step >> 16 != 0 {
            if self.ramptr == self.conv(self.limit) {
                self.ramptr = 0;
            }
            if self.ramptr == self.conv_e(self.end) {
                if self.control1 & 0x10 != 0 {
                    self.ramptr = self.conv(self.start);
                    self.acc = 0;
                    self.adpcmd = 127;
                    self.prev_acc = 0;
                } else {
                    self.control1 = 0;
                    self.out = 0;
                    self.prev_acc = 0;
                }
            }
            let mut data = self.ram[((self.ramptr >> 1) as usize) & (ADPCM_RAM - 1)];
            if self.ramptr & 1 != 0 {
                data &= 0x0f;
            } else {
                data >>= 4;
            }
            self.ramptr = (self.ramptr + 1) & ((1 << 25) - 1);
            self.prev_acc = self.acc;
            let mut d = i32::from(((data & 7) << 1) | 1);
            if data & 8 != 0 {
                d = -d;
            }
            let acc = (i32::from(self.acc) + d * i32::from(self.adpcmd) / 8).clamp(-32768, 32767);
            self.acc = acc as i16;
            let adpcmd = (u32::from(self.adpcmd) * u32::from(ADPCM_TABLE[usize::from(data & 7)]) / 64).clamp(127, 24576);
            self.adpcmd = adpcmd as u16;
        }
        let mut out = i32::from(self.prev_acc).wrapping_mul(0x10000 - i32::from(self.step));
        out = out.wrapping_add(i32::from(self.acc).wrapping_mul(i32::from(self.step)));
        out >>= 16;
        out *= i32::from(self.vol);
        out >>= 8;
        self.out = out.clamp(-32768, 32767) as i16;
    }

    fn writereg(&mut self, reg: u32, val: u32) {
        if !(0x100..0x111).contains(&reg) {
            return;
        }
        let val = val as u8;
        match reg & 0xff {
            0x00 => {
                self.control1 = val & 0xf9;
                if self.control1 & 0x80 != 0 {
                    self.step = 0;
                    self.acc = 0;
                    self.prev_acc = 0;
                    self.out = 0;
                    self.adpcmd = 127;
                }
                if self.control1 & 0x20 != 0 {
                    self.ramptr = self.conv(self.start);
                }
                if self.control1 & 0x01 != 0 {
                    self.control1 = 0;
                }
            }
            0x01 => self.control2 = val & 0xcf,
            0x02 => self.start = (self.start & 0xff00) | u16::from(val),
            0x03 => self.start = (self.start & 0x00ff) | (u16::from(val) << 8),
            0x04 => self.end = (self.end & 0xff00) | u16::from(val),
            0x05 => self.end = (self.end & 0x00ff) | (u16::from(val) << 8),
            0x08 => {
                if self.control1 & (0x80 | 0x40 | 0x20) == (0x40 | 0x20) && self.ramptr != self.conv_e(self.end) {
                    self.ram[((self.ramptr >> 1) as usize) & (ADPCM_RAM - 1)] = val;
                    self.ramptr = self.ramptr.wrapping_add(2);
                }
            }
            0x09 => self.delta = (self.delta & 0xff00) | u16::from(val),
            0x0a => self.delta = (self.delta & 0x00ff) | (u16::from(val) << 8),
            0x0b => self.vol = val,
            0x0c => self.limit = (self.limit & 0xff00) | u16::from(val),
            0x0d => self.limit = (self.limit & 0x00ff) | (u16::from(val) << 8),
            _ => {}
        }
    }

    /// One sample: [left, right].
    fn sample(&mut self) -> [i32; 2] {
        if self.control1 & 0x80 == 0 {
            return [0, 0];
        }
        self.calc();
        if self.masked {
            return [0, 0];
        }
        let o = i32::from(self.out >> 1);
        [if self.control2 & 0x80 != 0 { o } else { 0 }, if self.control2 & 0x40 != 0 { o } else { 0 }]
    }
}

// ---- The chip ----

/// Each part's sample, as it reaches the mix.
#[derive(Clone, Copy, Default)]
pub struct Taps {
    pub fm: [[i32; 2]; 6],
    pub ssg: [i32; 3],
    pub rhythm: [[i32; 2]; 6],
    pub adpcm: [i32; 2],
}

#[derive(Clone)]
pub struct Opna {
    pub fm: Fm,
    pub ssg: Ssg,
    pub rhythm: Rhythm,
    pub adpcm: Adpcm,
}

impl Default for Opna {
    fn default() -> Opna {
        Opna::new()
    }
}

impl Opna {
    pub fn new() -> Opna {
        Opna {
            fm: Fm::new(),
            ssg: Ssg::new(),
            rhythm: Rhythm::default(),
            adpcm: Adpcm::new(),
        }
    }

    pub fn writereg(&mut self, reg: u32, val: u32) {
        let val = val & 0xff;
        self.fm.writereg(reg, val);
        self.ssg.writereg(reg, val);
        self.rhythm.writereg(reg, val);
        self.adpcm.writereg(reg, val);
    }

    pub fn readreg(&self, reg: u32) -> u32 {
        if reg > 0xf {
            0xff
        } else {
            u32::from(self.ssg.regs[reg as usize])
        }
    }

    /// One sample of every part.
    pub fn sample(&mut self, t: &mut Taps) {
        self.fm.sample(&mut t.fm);
        self.ssg.sample(&mut t.ssg);
        self.rhythm.sample(&mut t.rhythm);
        t.adpcm = self.adpcm.sample();
    }

    /// The chip's next samples added into `buf` (interleaved stereo), each
    /// part's clamped in turn as libopna clamps them.
    pub fn mix(&mut self, buf: &mut [i16], samples: usize) {
        let mut t = Taps::default();
        for i in 0..samples {
            self.sample(&mut t);
            let clamp = |x: i32| x.clamp(-32768, 32767);
            let (mut lo, mut ro) = (i32::from(buf[i * 2]), i32::from(buf[i * 2 + 1]));
            for c in &t.fm {
                lo += c[0];
                ro += c[1];
            }
            (lo, ro) = (clamp(lo), clamp(ro));
            let s: i32 = t.ssg.iter().sum();
            (lo, ro) = (clamp(lo + s), clamp(ro + s));
            for d in &t.rhythm {
                lo += d[0];
                ro += d[1];
            }
            (lo, ro) = (clamp(lo), clamp(ro));
            (lo, ro) = (clamp(lo + t.adpcm[0]), clamp(ro + t.adpcm[1]));
            buf[i * 2] = lo as i16;
            buf[i * 2 + 1] = ro as i16;
        }
    }
}

/// The OPNA's two timers, pacing a sound driver: B counts the chip's
/// samples in steps of 16 (an interrupt each 256 − its value), A in ones.
#[derive(Clone, Default)]
pub struct Timer {
    pub status: u8,
    timera: u16,
    timerb: u8,
    timera_load: bool,
    timera_enable: bool,
    timerb_load: bool,
    timerb_enable: bool,
    timerb_cnt: u16,
}

impl Timer {
    /// A register write, as the timer sees it (the chip sees it too).
    pub fn writereg(&mut self, reg: u32, val: u32) {
        let val = val & 0xff;
        match reg {
            0x24 => self.timera = (self.timera & !0xff) | val as u16,
            0x25 => self.timera = (self.timera & 0xff) | (((val & 3) as u16) << 8),
            0x26 => {
                self.timerb = val as u8;
                self.timerb_cnt = u16::from(self.timerb) << 4;
            }
            0x27 => {
                self.timera_load = val & 1 != 0;
                self.timera_enable = val & 4 != 0;
                self.timerb_load = val & 2 != 0;
                self.timerb_enable = val & 8 != 0;
                if val & 0x10 != 0 {
                    self.status &= !1;
                }
                if val & 0x20 != 0 {
                    self.status &= !2;
                }
            }
            _ => {}
        }
    }

    /// Timer B's value (its period is 256 − this, in 16 samples).
    pub fn timerb(&self) -> u8 {
        self.timerb
    }

    /// How many samples until the next interrupt could come, at most `max`.
    pub fn until(&self, max: usize) -> usize {
        let mut n = max;
        if self.timerb_enable && self.timerb_load {
            n = n.min((1usize << 12) - usize::from(self.timerb_cnt));
        }
        if self.timera_enable && self.timera_load {
            n = n.min((1usize << 10) - usize::from(self.timera));
        }
        n
    }

    /// `n` samples gone by on timer A: whether its interrupt is due (the
    /// driver is run before B is counted, as libopna runs it).
    pub fn advance_a(&mut self, n: usize) -> bool {
        if self.timera_load {
            self.timera = ((usize::from(self.timera) + n) & ((1 << 10) - 1)) as u16;
            if self.timera == 0 && self.timera_enable && self.status & 1 == 0 {
                self.status |= 1;
                return true;
            }
        }
        false
    }

    /// `n` samples gone by on timer B: whether its interrupt is due.
    pub fn advance_b(&mut self, n: usize) -> bool {
        if self.timerb_load {
            self.timerb_cnt = ((usize::from(self.timerb_cnt) + n) & ((1 << 12) - 1)) as u16;
            if self.timerb_cnt == 0 && self.timerb_enable && self.status & 2 == 0 {
                self.status |= 2;
                return true;
            }
        }
        false
    }
}
