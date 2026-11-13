// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! FLAC (the Free Lossless Audio Codec) decoded, written from its
//! specification (RFC 9639): the STREAMINFO block, then frames of
//! subframes (constant, verbatim, fixed and LPC predictors over Rice-coded
//! residuals, escaped partitions, wasted bits), each frame's channels
//! decorrelated (left/side, side/right, mid/side). Checksums aren't
//! checked: a damaged frame ends the stream where it is.

/// A FLAC stream decoded: its channels' samples, at their own bit depth.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Audio {
    pub rate: u32,
    pub bits: u32,
    /// Each channel's samples (as many as the stream decoded).
    pub channels: Vec<Vec<i32>>,
}

impl Audio {
    /// A channel's samples scaled to -1..1.
    pub fn to_f32(&self, channel: usize) -> Vec<f32> {
        let scale = 1.0 / (1u64 << self.bits.clamp(1, 32).saturating_sub(1)) as f32;
        self.channels.get(channel).map_or_else(Vec::new, |c| c.iter().map(|&s| s as f32 * scale).collect())
    }
}

/// Whether the bytes start a FLAC stream.
pub fn probe(d: &[u8]) -> bool {
    d.starts_with(b"fLaC")
}

struct Bits<'a> {
    d: &'a [u8],
    /// The next bit, counted from the stream's first.
    pos: usize,
}

impl<'a> Bits<'a> {
    fn new(d: &'a [u8], byte: usize) -> Bits<'a> {
        Bits { d, pos: byte * 8 }
    }

    fn left(&self) -> usize {
        (self.d.len() * 8).saturating_sub(self.pos)
    }

    /// `n` bits (at most 32), most significant first.
    fn u(&mut self, n: u32) -> Option<u32> {
        if n == 0 {
            return Some(0);
        }
        if self.left() < n as usize {
            return None;
        }
        let mut v: u64 = 0;
        let mut need = n;
        while need > 0 {
            let byte = self.d[self.pos >> 3];
            let used = (self.pos & 7) as u32;
            let take = (8 - used).min(need);
            let bits = (u32::from(byte) >> (8 - used - take)) & ((1 << take) - 1);
            v = v << take | u64::from(bits);
            need -= take;
            self.pos += take as usize;
        }
        Some(v as u32)
    }

    /// `n` bits as a two's complement number.
    fn s(&mut self, n: u32) -> Option<i32> {
        let v = self.u(n)?;
        Some(if n == 0 {
            0
        } else if n == 32 {
            v as i32
        } else {
            ((v << (32 - n)) as i32) >> (32 - n)
        })
    }

    /// A run of zero bits ended by a one: its length.
    fn unary(&mut self) -> Option<u32> {
        let mut n = 0;
        loop {
            if self.pos >= self.d.len() * 8 {
                return None;
            }
            let bit = (self.d[self.pos >> 3] >> (7 - (self.pos & 7))) & 1;
            self.pos += 1;
            if bit == 1 {
                return Some(n);
            }
            n += 1;
        }
    }

    fn align(&mut self) {
        self.pos = (self.pos + 7) & !7;
    }

    fn byte(&self) -> usize {
        self.pos >> 3
    }
}

struct Info {
    rate: u32,
    channels: u32,
    bits: u32,
    total: u64,
}

/// Decodes a whole FLAC stream; None when it isn't one. A stream damaged
/// part way keeps what it decoded before.
pub fn decode(d: &[u8]) -> Option<Audio> {
    if !probe(d) {
        return None;
    }
    let mut at = 4;
    let mut info = None;
    loop {
        let h = d.get(at..at + 4)?;
        let last = h[0] & 0x80 != 0;
        let kind = h[0] & 0x7f;
        let len = (usize::from(h[1]) << 16) | (usize::from(h[2]) << 8) | usize::from(h[3]);
        let body = d.get(at + 4..at + 4 + len)?;
        if kind == 0 && len >= 34 {
            let mut b = Bits::new(body, 10);
            let rate = b.u(20)?;
            let channels = b.u(3)? + 1;
            let bits = b.u(5)? + 1;
            let total = (u64::from(b.u(4)?) << 32) | u64::from(b.u(32)?);
            info = Some(Info { rate, channels, bits, total });
        }
        at += 4 + len;
        if last {
            break;
        }
    }
    let info = info?;
    let mut out = Audio {
        rate: info.rate,
        bits: info.bits,
        channels: vec![Vec::new(); info.channels as usize],
    };
    if info.total > 0 {
        for c in &mut out.channels {
            c.reserve(info.total.min(1 << 26) as usize);
        }
    }
    while at + 2 <= d.len() {
        // A frame starts with its sync code; skip anything else.
        if !(d[at] == 0xff && d[at + 1] & 0xfe == 0xf8) {
            at += 1;
            continue;
        }
        match frame(d, at, &info, &mut out) {
            Some(next) => at = next,
            None => break,
        }
    }
    Some(out)
}

/// Decodes the frame at `at` into `out`, returning where the next starts.
fn frame(d: &[u8], at: usize, info: &Info, out: &mut Audio) -> Option<usize> {
    let mut b = Bits::new(d, at);
    b.u(15)?;
    let _variable = b.u(1)?;
    let size_code = b.u(4)?;
    let rate_code = b.u(4)?;
    let chan_code = b.u(4)?;
    let bits_code = b.u(3)?;
    b.u(1)?;
    // The frame or sample number, UTF-8 coded.
    let first = b.u(8)?;
    let extra = match first {
        0x00..=0x7f => 0,
        0xc0..=0xdf => 1,
        0xe0..=0xef => 2,
        0xf0..=0xf7 => 3,
        0xf8..=0xfb => 4,
        0xfc..=0xfd => 5,
        0xfe => 6,
        _ => return None,
    };
    for _ in 0..extra {
        if b.u(8)? & 0xc0 != 0x80 {
            return None;
        }
    }
    let block = match size_code {
        0 => return None,
        1 => 192,
        2..=5 => 576 << (size_code - 2),
        6 => b.u(8)? + 1,
        7 => b.u(16)? + 1,
        _ => 256 << (size_code - 8),
    } as usize;
    match rate_code {
        12 => {
            b.u(8)?;
        }
        13 | 14 => {
            b.u(16)?;
        }
        15 => return None,
        _ => {}
    }
    let bits = match bits_code {
        0 => info.bits,
        1 => 8,
        2 => 12,
        4 => 16,
        5 => 20,
        6 => 24,
        7 => 32,
        _ => return None,
    };
    // The header's CRC-8.
    b.u(8)?;
    let (nch, decor) = match chan_code {
        0..=7 => (chan_code as usize + 1, 0),
        8..=10 => (2, chan_code),
        _ => return None,
    };
    let mut chans: Vec<Vec<i64>> = Vec::with_capacity(nch);
    for c in 0..nch {
        // The side channel has a bit more.
        let extra_bit = match (decor, c) {
            (8, 1) | (9, 0) | (10, 1) => 1,
            _ => 0,
        };
        chans.push(subframe(&mut b, block, bits + extra_bit)?);
    }
    b.align();
    // The frame's CRC-16.
    b.u(16)?;
    match decor {
        8 => {
            for i in 0..block {
                chans[1][i] = chans[0][i] - chans[1][i];
            }
        }
        9 => {
            for i in 0..block {
                chans[0][i] += chans[1][i];
            }
        }
        10 => {
            for i in 0..block {
                let (m, s) = (chans[0][i], chans[1][i]);
                let m = (m * 2) | (s & 1);
                chans[0][i] = (m + s) >> 1;
                chans[1][i] = (m - s) >> 1;
            }
        }
        _ => {}
    }
    // A frame at another depth than the stream's is scaled to it.
    let shift = info.bits as i32 - bits as i32;
    for (c, ch) in chans.into_iter().enumerate() {
        if let Some(o) = out.channels.get_mut(c) {
            o.extend(ch.into_iter().map(|s| if shift >= 0 { (s << shift) as i32 } else { (s >> -shift) as i32 }));
        }
    }
    Some(b.byte())
}

/// The fixed predictors' coefficients, by order.
const FIXED: [&[i64]; 5] = [&[], &[1], &[2, -1], &[3, -3, 1], &[4, -6, 4, -1]];

fn subframe(b: &mut Bits, block: usize, bits: u32) -> Option<Vec<i64>> {
    if b.u(1)? != 0 {
        return None;
    }
    let kind = b.u(6)?;
    let wasted = if b.u(1)? == 1 { b.unary()? + 1 } else { 0 };
    let bits = bits.checked_sub(wasted).filter(|&n| n > 0)?;
    let mut s: Vec<i64> = Vec::with_capacity(block);
    match kind {
        0 => {
            let v = i64::from(b.s(bits)?);
            s.resize(block, v);
        }
        1 => {
            for _ in 0..block {
                s.push(i64::from(b.s(bits)?));
            }
        }
        8..=12 => {
            let order = (kind - 8) as usize;
            if order > block {
                return None;
            }
            for _ in 0..order {
                s.push(i64::from(b.s(bits)?));
            }
            residual(b, block, order, &mut s)?;
            let c = FIXED[order];
            for i in order..block {
                let p: i64 = c.iter().enumerate().map(|(k, &a)| a * s[i - 1 - k]).sum();
                s[i] += p;
            }
        }
        32..=63 => {
            let order = (kind - 31) as usize;
            if order > block {
                return None;
            }
            for _ in 0..order {
                s.push(i64::from(b.s(bits)?));
            }
            let precision = b.u(4)? + 1;
            if precision == 16 {
                return None;
            }
            let shift = b.s(5)?;
            let coefs: Vec<i64> = (0..order).map(|_| b.s(precision).map(i64::from)).collect::<Option<_>>()?;
            residual(b, block, order, &mut s)?;
            for i in order..block {
                let p: i64 = coefs.iter().enumerate().map(|(k, &a)| a * s[i - 1 - k]).sum();
                s[i] += if shift >= 0 { p >> shift } else { p << -shift };
            }
        }
        _ => return None,
    }
    if wasted > 0 {
        for v in &mut s {
            *v <<= wasted;
        }
    }
    Some(s)
}

/// The Rice-coded residual after `order` warm-up samples, appended.
fn residual(b: &mut Bits, block: usize, order: usize, s: &mut Vec<i64>) -> Option<()> {
    let method = b.u(2)?;
    let param_bits = match method {
        0 => 4,
        1 => 5,
        _ => return None,
    };
    let escape = (1 << param_bits) - 1;
    let parts = 1usize << b.u(4)?;
    if block % parts != 0 || block / parts < order {
        return None;
    }
    for p in 0..parts {
        let n = block / parts - if p == 0 { order } else { 0 };
        let k = b.u(param_bits)?;
        if k == escape {
            let width = b.u(5)?;
            for _ in 0..n {
                s.push(i64::from(b.s(width)?));
            }
        } else {
            for _ in 0..n {
                let q = u64::from(b.unary()?);
                let v = (q << k) | u64::from(b.u(k)?);
                s.push(((v >> 1) as i64) ^ -((v & 1) as i64));
            }
        }
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 300 stereo frames (a chirp, and half of it with noise) as `flac -8
    /// -b 128` made them: verbatim, fixed and LPC subframes.
    const T_FLAC: &[u8] = &[
        0x66, 0x4c, 0x61, 0x43, 0x00, 0x00, 0x00, 0x22, 0x00, 0x80, 0x00, 0x80, 0x00, 0x00, 0x84, 0x00, 0x01, 0x31, 0x0a, 0xc4, 0x42, 0xf0, 0x00, 0x00, 0x01, 0x2c, 0x45, 0x68, 0x13, 0x23, 0xd9, 0x17,
        0xf6, 0xb9, 0x43, 0x6b, 0x32, 0x12, 0x3c, 0x8f, 0x9c, 0x2c, 0x84, 0x00, 0x00, 0x28, 0x20, 0x00, 0x00, 0x00, 0x72, 0x65, 0x66, 0x65, 0x72, 0x65, 0x6e, 0x63, 0x65, 0x20, 0x6c, 0x69, 0x62, 0x46,
        0x4c, 0x41, 0x43, 0x20, 0x31, 0x2e, 0x35, 0x2e, 0x30, 0x20, 0x32, 0x30, 0x32, 0x35, 0x30, 0x32, 0x31, 0x31, 0x00, 0x00, 0x00, 0x00, 0xff, 0xf8, 0x69, 0x18, 0x00, 0x7f, 0xc5, 0x18, 0x00, 0x00,
        0x00, 0x08, 0x00, 0x21, 0x00, 0x4b, 0x00, 0xa5, 0x67, 0x6b, 0x53, 0xa7, 0xd5, 0x8a, 0x4f, 0x11, 0xc8, 0xb1, 0x8a, 0x62, 0x91, 0xc8, 0x53, 0x3e, 0xca, 0x47, 0xcc, 0x56, 0xa8, 0xa9, 0x37, 0x4d,
        0x52, 0xb2, 0x93, 0x5c, 0x9b, 0x4d, 0x92, 0x6e, 0x4b, 0xd2, 0xd8, 0xb8, 0xb9, 0xe9, 0x61, 0xf0, 0xf9, 0xdd, 0x58, 0x54, 0x76, 0xd1, 0x08, 0x42, 0x18, 0x41, 0x06, 0x10, 0x31, 0x06, 0x30, 0x44,
        0x84, 0x94, 0x5e, 0x14, 0xe0, 0x51, 0x45, 0x00, 0x0f, 0x80, 0x21, 0x40, 0x8f, 0x5c, 0x8d, 0xe2, 0x3f, 0xa4, 0x92, 0x43, 0xa2, 0xd7, 0x30, 0x7a, 0x28, 0x85, 0x86, 0x46, 0xba, 0x2c, 0x2c, 0x01,
        0x4b, 0x3b, 0x8e, 0x4d, 0x0f, 0xb5, 0x26, 0x91, 0x64, 0xa1, 0x64, 0x74, 0xae, 0x3c, 0x26, 0x2b, 0xce, 0x4b, 0x5b, 0x49, 0x42, 0x44, 0x2d, 0x27, 0xb3, 0x5b, 0xad, 0x8e, 0xa5, 0x07, 0xbc, 0x7f,
        0x0c, 0x59, 0x19, 0x15, 0xc3, 0xbf, 0xa4, 0x78, 0x89, 0x1f, 0x07, 0x3d, 0x1f, 0xdd, 0xc9, 0xb6, 0x28, 0x09, 0x7c, 0x11, 0xda, 0xb9, 0xda, 0x21, 0x2f, 0x2e, 0x5b, 0x54, 0x8c, 0x67, 0x44, 0x7c,
        0x6b, 0x8b, 0x8b, 0x90, 0xdf, 0x89, 0x52, 0x82, 0x55, 0xc2, 0x85, 0xdd, 0xa3, 0xde, 0xc5, 0x31, 0xe9, 0xe7, 0x30, 0x8b, 0x41, 0x1d, 0x0a, 0x71, 0x2d, 0x50, 0x89, 0x03, 0x7e, 0xad, 0x5c, 0x1f,
        0x66, 0x0f, 0x29, 0x45, 0x6f, 0xca, 0x34, 0x4c, 0xd9, 0xab, 0xd3, 0xd2, 0x67, 0xb1, 0xba, 0x09, 0x2c, 0xca, 0x32, 0x62, 0xca, 0x09, 0xfe, 0x0e, 0xca, 0xb9, 0x85, 0x43, 0xf0, 0x0c, 0x0f, 0x7a,
        0x23, 0xd0, 0xfd, 0x35, 0x14, 0x57, 0xe8, 0x9d, 0x7e, 0xad, 0xc0, 0x00, 0x13, 0xff, 0xf8, 0x69, 0x18, 0x01, 0x7f, 0xd0, 0x18, 0xd6, 0x47, 0xda, 0xc6, 0xe0, 0x85, 0xe7, 0x5b, 0x09, 0x19, 0xfa,
        0x73, 0xfc, 0xa9, 0xe4, 0x18, 0x68, 0xc3, 0x82, 0x04, 0x0e, 0x12, 0x60, 0xa5, 0x39, 0x05, 0x7a, 0x6c, 0xa8, 0x88, 0xba, 0xb9, 0x72, 0xcd, 0x7a, 0x7d, 0xe9, 0x83, 0x64, 0x8c, 0x08, 0x0b, 0x89,
        0x8d, 0x0b, 0x09, 0x99, 0x2e, 0x9a, 0x4f, 0xd8, 0x93, 0x9b, 0x88, 0xc5, 0x22, 0x73, 0xb3, 0x16, 0xbb, 0xb4, 0x3c, 0x49, 0x0b, 0x4e, 0x84, 0x02, 0x63, 0xe3, 0x92, 0xf6, 0xca, 0x96, 0x52, 0x5a,
        0x49, 0xa5, 0xd3, 0x39, 0x7c, 0x9a, 0xcf, 0xf6, 0x3e, 0xc6, 0x82, 0x61, 0xf0, 0x46, 0x12, 0x0f, 0x85, 0x06, 0x27, 0x9e, 0xa7, 0xe5, 0x53, 0xb5, 0xdd, 0x2f, 0x36, 0xea, 0x9f, 0xc6, 0x36, 0xc9,
        0x20, 0xda, 0x22, 0x09, 0x62, 0x81, 0xd0, 0xaa, 0xc6, 0x8f, 0x42, 0xb7, 0x4a, 0x53, 0x0c, 0xcf, 0x25, 0xdb, 0x3b, 0xb1, 0x09, 0x87, 0xd2, 0x17, 0x5b, 0xbf, 0x68, 0x93, 0x15, 0xee, 0x10, 0x22,
        0x6b, 0x39, 0x84, 0xc3, 0xd6, 0xd5, 0xe9, 0x5a, 0xe4, 0xd7, 0x84, 0xca, 0x76, 0x4e, 0x37, 0xa5, 0x59, 0x80, 0x5d, 0xbf, 0x69, 0x86, 0xe9, 0x6f, 0xa6, 0x6e, 0x6d, 0xd9, 0x54, 0xbd, 0x0b, 0x53,
        0xee, 0xf5, 0x04, 0x75, 0x4b, 0xc1, 0x5b, 0x95, 0xf7, 0xd1, 0xf7, 0x17, 0x25, 0xd5, 0x64, 0xfe, 0x8a, 0xd6, 0xee, 0x64, 0xc8, 0x9b, 0xe3, 0xd2, 0xbb, 0x75, 0x7d, 0x1c, 0xf3, 0x36, 0x48, 0xb4,
        0x6f, 0x4b, 0x61, 0xfb, 0xe3, 0x18, 0x65, 0x5f, 0x62, 0x24, 0xcb, 0x1d, 0x99, 0x48, 0x7d, 0x7e, 0x52, 0x24, 0x5f, 0xa2, 0xa1, 0xf8, 0x0b, 0x4d, 0xe3, 0x5e, 0xb7, 0x6d, 0x67, 0x22, 0x35, 0xf7,
        0x8f, 0x1c, 0x8d, 0x52, 0xe9, 0x35, 0x40, 0xc9, 0x61, 0xdf, 0xeb, 0x0f, 0x9c, 0x1d, 0x65, 0xb5, 0x1c, 0xd9, 0x2d, 0x6e, 0x46, 0xd1, 0x7a, 0x04, 0x53, 0xca, 0x1f, 0xea, 0x1a, 0x79, 0x86, 0x53,
        0x8b, 0x6c, 0x1c, 0x23, 0xe8, 0xa0, 0x23, 0xfc, 0x23, 0xf2, 0xaf, 0xdc, 0x16, 0xec, 0x88, 0xba, 0x27, 0x40, 0x8a, 0x41, 0x06, 0x32, 0x79, 0xcf, 0xed, 0x63, 0xc1, 0x79, 0x74, 0xc9, 0xff, 0xf8,
        0x69, 0x18, 0x02, 0x2b, 0x44, 0x18, 0x2c, 0x76, 0x24, 0x69, 0x17, 0xa8, 0x07, 0xcc, 0x02, 0x3c, 0x15, 0x69, 0x8f, 0x1d, 0x47, 0x65, 0x5b, 0x56, 0x65, 0x41, 0xdb, 0xc6, 0xf3, 0x89, 0xd3, 0x54,
        0xf7, 0x3c, 0x4c, 0x52, 0x15, 0x42, 0xbf, 0xa8, 0x2e, 0x89, 0x21, 0x24, 0x2e, 0x3a, 0x53, 0x4d, 0xcb, 0x2d, 0xc4, 0x50, 0xb5, 0x15, 0x47, 0x91, 0x24, 0x8d, 0x76, 0xdd, 0x92, 0xda, 0x6c, 0xcb,
        0xba, 0x10, 0xb3, 0xa8, 0x92, 0xf3, 0x15, 0xc6, 0x30, 0x22, 0xc2, 0xb8, 0x14, 0x11, 0x9f, 0xdc, 0xff, 0xcd, 0xbc, 0xf4, 0xf8, 0x8b, 0x54, 0xb2, 0xa0, 0x7a, 0x84, 0x53, 0x7d, 0xaa, 0x4f, 0x8b,
        0xbe, 0x80, 0x9b, 0xa0, 0xa3, 0xfe, 0x99, 0x3b, 0x34, 0x05, 0xe9, 0x29, 0x0d, 0x03, 0x01, 0x70, 0x46, 0x93, 0x62, 0x9f, 0x6f, 0xca, 0x0b, 0x38, 0xc6, 0x98, 0x41, 0x0f, 0x20, 0xda, 0x48, 0x98,
        0x4c, 0xe5,
    ];

    #[test]
    fn decodes_what_flac_encoded() {
        let a = decode(T_FLAC).expect("a FLAC stream");
        assert_eq!((a.rate, a.bits, a.channels.len()), (44100, 16, 2));
        let mut seed: u64 = 12345;
        for i in 0..300 {
            seed = (seed * 1_103_515_245 + 12345) & 0x7fff_ffff;
            let l = (12000.0 * ((i * i) as f64 * 0.0007).sin()) as i32;
            let r = l.div_euclid(2) + (seed % 301) as i32 - 150;
            assert_eq!((a.channels[0][i], a.channels[1][i]), (l, r), "frame {i}");
        }
    }

    /// Every FLAC file in FLAC_DIR decodes to what `flac -d` made of it
    /// (FLAC_REF, the same names as .wav).
    #[test]
    #[ignore]
    fn against() {
        let dir = std::env::var("FLAC_DIR").expect("FLAC_DIR");
        let refd = std::env::var("FLAC_REF").expect("FLAC_REF");
        let (mut same, mut total) = (0, 0);
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            let Ok(d) = std::fs::read(&p) else { continue };
            let Some(a) = decode(&d) else { continue };
            let name = p.file_stem().unwrap().to_string_lossy().to_string();
            let Ok(w) = std::fs::read(std::path::Path::new(&refd).join(format!("{name}.wav"))) else {
                continue;
            };
            total += 1;
            // The reference's samples: 16- or 24-bit interleaved after "data".
            let Some(at) = w.windows(4).position(|x| x == b"data") else { continue };
            let size = u32::from_le_bytes([w[at + 4], w[at + 5], w[at + 6], w[at + 7]]) as usize;
            let at = at + 8;
            let end = (at + size).min(w.len());
            let bps = (a.bits as usize).div_ceil(8);
            let nch = a.channels.len();
            let mut ok = true;
            for (i, c) in w[at..end].chunks_exact(bps).enumerate() {
                let v = match bps {
                    1 => i32::from(c[0]) - 128,
                    2 => i32::from(i16::from_le_bytes([c[0], c[1]])),
                    3 => i32::from(c[0]) | i32::from(c[1]) << 8 | i32::from(c[2] as i8) << 16,
                    _ => i32::from_le_bytes([c[0], c[1], c[2], c[3]]),
                };
                if a.channels[i % nch].get(i / nch) != Some(&v) {
                    println!("{name}: differs at sample {} channel {}", i / nch, i % nch);
                    ok = false;
                    break;
                }
            }
            if ok && a.channels[0].len() * nch * bps != end - at {
                println!("{name}: {} samples, the reference {}", a.channels[0].len(), (end - at) / nch / bps);
                ok = false;
            }
            if ok {
                same += 1;
            }
        }
        println!("{same} of {total} the same");
    }
}
