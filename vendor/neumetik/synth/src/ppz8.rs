// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.
//
// Ported from 98fmplayer's PPZ8 emulation (github.com/takamichih/fmplayer),
// under its BSD 2-Clause License; the notice is in `opna.rs`.

//! PPZ8, the PC-98's 8-channel PCM driver PMD and FMP songs can play
//! through: two banks of 128 voices (from .PVI, ADPCM, or .PZI, 8-bit),
//! each channel a voice at a frequency, volume (16 steps of 3 dB) and pan
//! (9 places), looped or not, resampled by a 7-point windowed sinc and
//! mixed into the OPNA's output. A port of 98fmplayer's.

const PAN_VOL: [[i32; 2]; 10] = [[0, 0], [4, 0], [4, 1], [4, 2], [4, 3], [4, 4], [3, 4], [2, 4], [1, 4], [0, 4]];
const NONE: u64 = u64::MAX;
const NONE32: u32 = u32::MAX;

fn sinctable() -> &'static [[i16; 7]; 256] {
    use std::sync::OnceLock;
    static T: OnceLock<[[i16; 7]; 256]> = OnceLock::new();
    T.get_or_init(|| {
        let mut t = [[0i16; 7]; 256];
        for (i, row) in t.iter_mut().enumerate() {
            for (j, v) in row.iter_mut().enumerate() {
                let x = j as f64 - 3.5 + (255 - i) as f64 / 256.0;
                let s = if x == 0.0 { 1.0 } else { (std::f64::consts::PI * x).sin() / (std::f64::consts::PI * x) };
                let w = 0.5 * (1.0 + (2.0 * std::f64::consts::PI * x / 7.0).cos());
                *v = (s * w * 32767.0).round() as i16;
            }
        }
        t
    })
}

#[derive(Clone, Copy, Default)]
pub struct Voice {
    start: u32,
    len: u32,
    loopstart: u32,
    loopend: u32,
    origfreq: u16,
}

#[derive(Clone)]
pub struct Bank {
    data: Vec<i16>,
    voice: [Voice; 128],
}

impl Default for Bank {
    fn default() -> Bank {
        Bank {
            data: Vec::new(),
            voice: [Voice::default(); 128],
        }
    }
}

#[derive(Clone, Copy)]
struct Channel {
    ptr: u64,
    loopstartptr: u64,
    loopendptr: u64,
    endptr: u64,
    freq: u32,
    loopstartoff: u32,
    loopendoff: u32,
    vol: u8,
    pan: u8,
    voice: u8,
    playing: bool,
    looped: bool,
}

#[derive(Clone)]
pub struct Ppz8 {
    pub banks: [Bank; 2],
    channel: [Channel; 8],
    srate: u16,
    totalvol: u8,
    mix_volume: u16,
    pub mask: u32,
}

fn adpcm_acc(acc: i16, d: u16, data: u8) -> i16 {
    let data = data & 0xf;
    let mut a = i32::from(((data & 7) << 1) | 1);
    if data & 8 != 0 {
        a = -a;
    }
    (i32::from(acc) + a * i32::from(d) / 8).clamp(-32768, 32767) as i16
}

fn adpcm_d(d: u16, data: u8) -> u16 {
    const T: [u8; 8] = [57, 57, 57, 57, 77, 102, 128, 153];
    (u32::from(d) * u32::from(T[usize::from(data & 7)]) / 64).clamp(127, 24576) as u16
}

impl Ppz8 {
    pub fn new(srate: u16, mix_volume: u16) -> Ppz8 {
        let ch = Channel {
            ptr: NONE,
            loopstartptr: NONE,
            loopendptr: NONE,
            endptr: 0,
            freq: 0,
            loopstartoff: NONE32,
            loopendoff: NONE32,
            vol: 8,
            pan: 5,
            voice: 0,
            playing: false,
            looped: false,
        };
        Ppz8 {
            banks: [Bank::default(), Bank::default()],
            channel: [ch; 8],
            srate,
            totalvol: 12,
            mix_volume,
            mask: 0,
        }
    }

    fn bank(&self, v: u8) -> &Bank {
        &self.banks[usize::from(v >> 7)]
    }

    /// A .PVI file (ADPCM voices) into bank `b`; false when it isn't one.
    pub fn load_pvi(&mut self, b: usize, pvi: &[u8]) -> bool {
        if b >= 2 || pvi.len() < 0x210 {
            return false;
        }
        let samples = (pvi.len() - 0x210) * 2;
        let mut data = vec![0i16; samples];
        let origfreq = ((0x49bau32 * 55467) >> 16) as u16;
        let mut voices = [Voice::default(); 128];
        let mut last = 0u32;
        let rd = |i: usize| u32::from(u16::from_le_bytes([pvi[i], pvi[i + 1]]));
        for (i, v) in voices.iter_mut().enumerate() {
            let start = rd(0x10 + i * 4) << 6;
            let end = (rd(0x10 + i * 4 + 2) + 1) << 6;
            if start != last || start >= end {
                break;
            }
            *v = Voice {
                start: start << 1,
                len: (end - start) << 1,
                loopstart: NONE32,
                loopend: NONE32,
                origfreq,
            };
            let (mut acc, mut d) = (0i16, 127u16);
            for a in start..end {
                let Some(&byte) = pvi.get(0x210 + (a as usize >> 1)) else { return false };
                let nib = if a & 1 != 0 { byte & 0xf } else { byte >> 4 };
                acc = adpcm_acc(acc, d, nib);
                d = adpcm_d(d, nib);
                if let Some(x) = data.get_mut(a as usize) {
                    *x = acc;
                }
            }
            last = end;
        }
        self.banks[b] = Bank { data, voice: voices };
        true
    }

    /// A .PZI file (8-bit voices) into bank `b`; false when it isn't one.
    pub fn load_pzi(&mut self, b: usize, pzi: &[u8]) -> bool {
        if b >= 2 || pzi.len() < 0x20 + 18 * 128 || !(pzi.starts_with(b"PZI0") || pzi.starts_with(b"PZI1")) {
            return false;
        }
        let r32 = |i: usize| u32::from_le_bytes([pzi[i], pzi[i + 1], pzi[i + 2], pzi[i + 3]]);
        let mut voices = [Voice::default(); 128];
        for (i, v) in voices.iter_mut().enumerate() {
            let o = 0x20 + 18 * i;
            *v = Voice {
                start: r32(o).wrapping_mul(2),
                len: r32(o + 4).wrapping_mul(2),
                loopstart: r32(o + 8),
                loopend: r32(o + 12),
                origfreq: u16::from_le_bytes([pzi[o + 16], pzi[o + 17]]),
            };
        }
        let data = pzi[0x20 + 18 * 128..].iter().map(|&b| ((i32::from(b) - 0x80) << 8) as i16).collect();
        self.banks[b] = Bank { data, voice: voices };
        true
    }

    fn lp(c: &Channel, ptr: u64) -> u64 {
        if c.loopstartptr != NONE {
            let le = if c.loopendptr == NONE { c.endptr } else { c.loopendptr };
            if ptr >= le {
                if le == c.loopstartptr {
                    return NONE;
                }
                let mut off = ((ptr - le) >> 16) as u32;
                let period = ((le.wrapping_sub(c.loopstartptr)) >> 16) as u32;
                if period == 0 {
                    return NONE;
                }
                off %= period;
                off = off.wrapping_add((c.loopstartptr >> 16) as u32);
                return (ptr & 0xffff) | (u64::from(off) << 16);
            }
        }
        let mut limit = c.endptr;
        if c.loopstartptr != NONE && c.loopendptr != NONE && c.loopendptr > c.endptr {
            limit = c.loopendptr;
        }
        if ptr >= limit {
            return NONE;
        }
        ptr
    }

    fn centered(&self, c: &Channel, out: &mut [i16]) {
        let bank = self.bank(c.voice);
        let n = out.len() as i32;
        let curr = (c.ptr >> 16) as u32;
        for (i, o) in out.iter_mut().enumerate() {
            let indoff = i as i32 - (n - 1) / 2;
            let mut ind = curr.wrapping_add(indoff as u32);
            if indoff < 0 {
                if c.loopstartptr != NONE && c.looped {
                    let le = ((if c.loopendptr == NONE { c.endptr } else { c.loopendptr }) >> 16) as u32;
                    let ls = (c.loopstartptr >> 16) as u32;
                    let period = le.wrapping_sub(ls);
                    if period == 0 {
                        ind = NONE32;
                    } else {
                        let mut lo = ind.wrapping_sub(ls).wrapping_sub(period).wrapping_neg();
                        lo %= period;
                        if lo == 0 {
                            lo = period;
                        }
                        ind = ls.wrapping_add(period).wrapping_sub(lo);
                    }
                } else if (indoff.unsigned_abs()) > curr {
                    ind = NONE32;
                }
            } else {
                // Unshifted, as 98fmplayer has it.
                let le = (if c.loopendptr == NONE { c.endptr } else { c.loopendptr }) as u32;
                if c.loopstartptr != NONE && ind >= le {
                    let ls = (c.loopstartptr >> 16) as u32;
                    let period = le.wrapping_sub(ls);
                    if period == 0 {
                        ind = NONE32;
                    } else {
                        ind = ls.wrapping_add(ind.wrapping_sub(ls) % period);
                    }
                }
            }
            *o = if u64::from(ind) >= (c.endptr >> 16) || ind as usize >= bank.data.len() {
                0
            } else {
                bank.data[ind as usize]
            };
        }
    }

    fn calc(&mut self, p: usize) -> i32 {
        let c = self.channel[p];
        let voice = self.bank(c.voice).voice[usize::from(c.voice & 0x7f)];
        let mut out = 0i32;
        if c.vol != 0 {
            let mut s = [0i16; 7];
            self.centered(&c, &mut s);
            let t = &sinctable()[((c.ptr >> 8) & 0xff) as usize];
            for i in 0..7 {
                out = out.wrapping_add(i32::from(s[i]) * i32::from(t[i]));
            }
            out >>= 15;
            out >>= 7 - ((c.vol & 0xf) >> 1);
            if c.vol & 1 == 0 {
                out = (out.wrapping_mul(0xb505)) >> 16;
            }
        }
        let diff = ((u64::from(c.freq) * u64::from(voice.origfreq)) << 1) / u64::from(self.srate.max(1));
        let newptr = c.ptr.wrapping_add(diff);
        let ch = &mut self.channel[p];
        ch.ptr = Ppz8::lp(&c, newptr);
        if newptr != ch.ptr {
            ch.looped = true;
        }
        if ch.ptr == NONE {
            ch.playing = false;
        }
        out
    }

    /// One sample of each channel: [left, right] as it reaches the mix.
    pub fn sample(&mut self, out: &mut [[i32; 2]; 8]) {
        for p in 0..8 {
            out[p] = [0, 0];
            if !self.channel[p].playing {
                continue;
            }
            let o = self.calc(p);
            if self.mask & (1 << p) != 0 {
                continue;
            }
            let o = o.wrapping_mul(i32::from(self.mix_volume)) >> 15;
            let pan = PAN_VOL[usize::from(self.channel[p].pan.min(9))];
            out[p] = [o.wrapping_mul(pan[0]) >> 2, o.wrapping_mul(pan[1]) >> 2];
        }
    }

    /// Its next samples added into `buf`, clamped as 98fmplayer clamps them.
    pub fn mix(&mut self, buf: &mut [i16], samples: usize) {
        let mut o = [[0i32; 2]; 8];
        for i in 0..samples {
            self.sample(&mut o);
            let (mut lo, mut ro) = (i32::from(buf[i * 2]), i32::from(buf[i * 2 + 1]));
            for c in &o {
                lo += c[0];
                ro += c[1];
            }
            buf[i * 2] = lo.clamp(-32768, 32767) as i16;
            buf[i * 2 + 1] = ro.clamp(-32768, 32767) as i16;
        }
    }

    pub fn play(&mut self, ch: u8, v: u8) {
        let Some(c) = self.channel.get(usize::from(ch)).copied() else { return };
        let voice = self.bank(v).voice[usize::from(v & 0x7f)];
        let ptr = (u64::from(voice.start) >> 1) << 16;
        let c = Channel {
            voice: v,
            ptr,
            endptr: (u64::from(voice.start.wrapping_add(voice.len)) >> 1) << 16,
            loopstartptr: if c.loopstartoff == NONE32 { NONE } else { ptr.wrapping_add(u64::from(c.loopstartoff) << 16) },
            loopendptr: if c.loopendoff == NONE32 { NONE } else { ptr.wrapping_add(u64::from(c.loopendoff) << 16) },
            playing: true,
            looped: false,
            ..c
        };
        self.channel[usize::from(ch)] = c;
    }

    pub fn stop(&mut self, ch: u8) {
        if let Some(c) = self.channel.get_mut(usize::from(ch)) {
            c.playing = false;
            c.ptr = NONE;
        }
    }

    pub fn volume(&mut self, ch: u8, vol: u8) {
        if vol < 16 {
            if let Some(c) = self.channel.get_mut(usize::from(ch)) {
                c.vol = vol;
            }
        }
    }

    pub fn freq(&mut self, ch: u8, freq: u32) {
        if let Some(c) = self.channel.get_mut(usize::from(ch)) {
            c.freq = freq;
        }
    }

    pub fn loop_offset(&mut self, ch: u8, start: u32, end: u32) {
        if let Some(c) = self.channel.get_mut(usize::from(ch)) {
            c.loopstartoff = start;
            c.loopendoff = end;
        }
    }

    pub fn pan(&mut self, ch: u8, pan: u8) {
        if pan < 10 {
            if let Some(c) = self.channel.get_mut(usize::from(ch)) {
                c.pan = pan;
            }
        }
    }

    pub fn total_volume(&mut self, vol: u8) {
        if vol < 16 {
            self.totalvol = vol;
        }
    }

    pub fn loop_voice(&mut self, ch: u8, v: u8) {
        let voice = self.bank(v).voice[usize::from(v & 0x7f)];
        if let Some(c) = self.channel.get_mut(usize::from(ch)) {
            c.loopstartoff = voice.loopstart;
            c.loopendoff = voice.loopend;
        }
    }

    pub fn voice_length(&self, v: u8) -> u32 {
        self.bank(v).voice[usize::from(v & 0x7f)].len
    }
}
