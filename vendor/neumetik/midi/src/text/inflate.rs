// Copyright 2026 ansiapps. Neumetik MIDI, MIT licensed: see LICENSE.

//! **Inflate** (RFC 1951's DEFLATE, read), written from the RFC, and the
//! zip archive around it (PKWARE's APPNOTE): for compressed MusicXML.

use crate::Error;

const FAIL: Error = Error::BadText { line: 1, column: 1, why: "the compressed file is damaged" };

struct Bits<'a> {
    data: &'a [u8],
    at: usize,
    bit: u32,
    bits: u32,
}

impl<'a> Bits<'a> {
    fn need(&mut self, n: u32) -> Result<(), Error> {
        while self.bits < n {
            let b = *self.data.get(self.at).ok_or(FAIL)?;
            self.at += 1;
            self.bit |= u32::from(b) << self.bits;
            self.bits += 8;
        }
        Ok(())
    }
    fn take(&mut self, n: u32) -> Result<u32, Error> {
        if n == 0 {
            return Ok(0);
        }
        self.need(n)?;
        let v = self.bit & ((1u32 << n) - 1);
        self.bit >>= n;
        self.bits -= n;
        Ok(v)
    }
    fn align(&mut self) {
        let drop = self.bits % 8;
        self.bit >>= drop;
        self.bits -= drop;
    }
}

/// A canonical Huffman code: counts of each length, symbols by code.
struct Huffman {
    counts: [u16; 16],
    symbols: Vec<u16>,
}

impl Huffman {
    fn new(lengths: &[u8]) -> Result<Huffman, Error> {
        let mut counts = [0u16; 16];
        for &l in lengths {
            counts[usize::from(l)] += 1;
        }
        counts[0] = 0;
        let mut left: i32 = 1;
        for &c in &counts[1..] {
            left = left * 2 - i32::from(c);
            if left < 0 {
                return Err(FAIL);
            }
        }
        let mut offsets = [0u16; 16];
        for l in 1..15 {
            offsets[l + 1] = offsets[l] + counts[l];
        }
        let mut symbols = vec![0u16; lengths.len()];
        for (s, &l) in lengths.iter().enumerate() {
            if l != 0 {
                symbols[usize::from(offsets[usize::from(l)])] = s as u16;
                offsets[usize::from(l)] += 1;
            }
        }
        Ok(Huffman { counts, symbols })
    }

    fn decode(&self, b: &mut Bits) -> Result<u16, Error> {
        let (mut code, mut first, mut index) = (0i32, 0i32, 0i32);
        for len in 1..16 {
            code |= b.take(1)? as i32;
            let count = i32::from(self.counts[len]);
            if code - count < first {
                return self.symbols.get((index + code - first) as usize).copied().ok_or(FAIL);
            }
            index += count;
            first += count;
            first <<= 1;
            code <<= 1;
        }
        Err(FAIL)
    }
}

const LENGTH_BASE: [u16; 29] = [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258];
const LENGTH_EXTRA: [u8; 29] = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
const DIST_BASE: [u16; 30] = [1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577];
const DIST_EXTRA: [u8; 30] = [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];

/// Raw DEFLATE data, inflated; at most `most` bytes.
pub fn inflate(data: &[u8], most: usize) -> Result<Vec<u8>, Error> {
    let mut b = Bits { data, at: 0, bit: 0, bits: 0 };
    let mut out: Vec<u8> = Vec::new();
    loop {
        let last = b.take(1)?;
        match b.take(2)? {
            0 => {
                b.align();
                let len = b.take(16)? as usize;
                let nlen = b.take(16)? as usize;
                if len != !nlen & 0xffff {
                    return Err(FAIL);
                }
                for _ in 0..len {
                    out.push(b.take(8)? as u8);
                }
            }
            1 => {
                let mut lengths = [0u8; 288];
                lengths[..144].fill(8);
                lengths[144..256].fill(9);
                lengths[256..280].fill(7);
                lengths[280..].fill(8);
                let lit = Huffman::new(&lengths)?;
                let dist = Huffman::new(&[5u8; 30])?;
                block(&mut b, &mut out, &lit, &dist, most)?;
            }
            2 => {
                let hlit = b.take(5)? as usize + 257;
                let hdist = b.take(5)? as usize + 1;
                let hclen = b.take(4)? as usize + 4;
                const ORDER: [usize; 19] = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];
                let mut cl = [0u8; 19];
                for &o in ORDER.iter().take(hclen) {
                    cl[o] = b.take(3)? as u8;
                }
                let code = Huffman::new(&cl)?;
                let mut lengths = vec![0u8; hlit + hdist];
                let mut i = 0;
                while i < hlit + hdist {
                    let sym = code.decode(&mut b)?;
                    let (value, repeat) = match sym {
                        0..=15 => (sym as u8, 1),
                        16 => (*lengths.get(i.wrapping_sub(1)).ok_or(FAIL)?, 3 + b.take(2)? as usize),
                        17 => (0, 3 + b.take(3)? as usize),
                        18 => (0, 11 + b.take(7)? as usize),
                        _ => return Err(FAIL),
                    };
                    if i + repeat > lengths.len() {
                        return Err(FAIL);
                    }
                    lengths[i..i + repeat].fill(value);
                    i += repeat;
                }
                let lit = Huffman::new(&lengths[..hlit])?;
                let dist = Huffman::new(&lengths[hlit..])?;
                block(&mut b, &mut out, &lit, &dist, most)?;
            }
            _ => return Err(FAIL),
        }
        if out.len() > most {
            return Err(FAIL);
        }
        if last == 1 {
            return Ok(out);
        }
    }
}

fn block(b: &mut Bits, out: &mut Vec<u8>, lit: &Huffman, dist: &Huffman, most: usize) -> Result<(), Error> {
    loop {
        let sym = lit.decode(b)?;
        match sym {
            0..=255 => out.push(sym as u8),
            256 => return Ok(()),
            257..=285 => {
                let i = usize::from(sym - 257);
                let len = usize::from(LENGTH_BASE[i]) + b.take(u32::from(LENGTH_EXTRA[i]))? as usize;
                let d = usize::from(dist.decode(b)?);
                if d >= 30 {
                    return Err(FAIL);
                }
                let back = usize::from(DIST_BASE[d]) + b.take(u32::from(DIST_EXTRA[d]))? as usize;
                if back > out.len() {
                    return Err(FAIL);
                }
                let from = out.len() - back;
                for k in 0..len {
                    out.push(out[from + k]);
                }
            }
            _ => return Err(FAIL),
        }
        if out.len() > most {
            return Err(FAIL);
        }
    }
}

/// A zip archive's files: (name, contents), stored or deflated ones.
pub fn unzip(data: &[u8], most: usize) -> Result<Vec<(String, Vec<u8>)>, Error> {
    let u16le = |i: usize| -> Result<usize, Error> { Ok(usize::from(u16::from_le_bytes(data.get(i..i + 2).ok_or(FAIL)?.try_into().map_err(|_| FAIL)?))) };
    let u32le = |i: usize| -> Result<usize, Error> { Ok(u32::from_le_bytes(data.get(i..i + 4).ok_or(FAIL)?.try_into().map_err(|_| FAIL)?) as usize) };
    // The central directory, from its end record.
    let eocd = (0..data.len().saturating_sub(21)).rev().take(70_000).find(|&i| data[i..].starts_with(b"PK\x05\x06")).ok_or(FAIL)?;
    let count = u16le(eocd + 10)?;
    let mut at = u32le(eocd + 16)?;
    let mut files = vec![];
    for _ in 0..count {
        if !data.get(at..).is_some_and(|d| d.starts_with(b"PK\x01\x02")) {
            return Err(FAIL);
        }
        let method = u16le(at + 10)?;
        let size = u32le(at + 20)?;
        let (nlen, xlen, clen) = (u16le(at + 28)?, u16le(at + 30)?, u16le(at + 32)?);
        let local = u32le(at + 42)?;
        let name = String::from_utf8_lossy(data.get(at + 46..at + 46 + nlen).ok_or(FAIL)?).to_string();
        at += 46 + nlen + xlen + clen;
        if !data.get(local..).is_some_and(|d| d.starts_with(b"PK\x03\x04")) {
            return Err(FAIL);
        }
        let start = local + 30 + u16le(local + 26)? + u16le(local + 28)?;
        let raw = data.get(start..start + size).ok_or(FAIL)?;
        let contents = match method {
            0 => raw.to_vec(),
            8 => inflate(raw, most)?,
            _ => continue,
        };
        files.push((name, contents));
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_and_fixed_blocks() {
        // A stored block: "hi".
        assert_eq!(inflate(&[0x01, 0x02, 0x00, 0xfd, 0xff, b'h', b'i'], 100).unwrap(), b"hi");
        // zlib's fixed-Huffman deflate of "abcabcabcabc".
        let fixed = [0x4b, 0x4c, 0x4a, 0x4e, 0x84, 0x21, 0x00];
        assert_eq!(inflate(&fixed, 100).unwrap(), b"abcabcabcabc");
        assert!(inflate(&[0xff], 100).is_err());
    }

    #[test]
    fn dynamic_block() {
        // zlib's raw deflate, level 9, of 60 MusicXML notes: a dynamic block.
        let text: Vec<u8> = (0..60u32)
            .flat_map(|i| format!("<note><pitch><step>{}</step><octave>{}</octave></pitch><duration>{}</duration></note>\n", &"CDEFGAB"[(i % 7) as usize..(i % 7) as usize + 1], 3 + i % 3, 1 + i % 5).into_bytes())
            .collect();
        let packed = [0xad, 0xd4, 0xcd, 0x8a, 0xc2, 0x30, 0x14, 0xc5, 0xf1, 0xfd, 0xbc, 0x4c, 0x18, 0xdb, 0xec, 0x2e, 0x81, 0x7c, 0xdc, 0xe4, 0x39, 0xc4, 0x29, 0x8c, 0x1b, 0x2b, 0x63, 0xf4, 0xf9, 0x05, 0x11, 0x17, 0x0e, 0x48, 0xd2, 0xfc, 0x57, 0xcd, 0xa2, 0x1c, 0x38, 0x9c, 0xcb, 0x4f, 0x4e, 0x6b, 0x5d, 0x9c, 0x9c, 0x8f, 0xf5, 0xf0, 0xeb, 0xe4, 0x52, 0x97, 0xb3, 0x8b, 0x62, 0x1e, 0x5f, 0x59, 0x0f, 0x75, 0x7f, 0x5b, 0xdc, 0x24, 0xe6, 0xf9, 0x12, 0xf3, 0xfc, 0xef, 0xe7, 0xfa, 0xb7, 0xaf, 0xc7, 0xf5, 0xe4, 0xbe, 0xc5, 0xbc, 0xde, 0x62, 0x1e, 0x59, 0x5f, 0xf2, 0x3f, 0x32, 0xbd, 0x45, 0xce, 0x1f, 0x22, 0x77, 0x6d, 0x91, 0xfa, 0x16, 0x69, 0x3f, 0x44, 0x4e, 0x6d, 0x91, 0xb9, 0xa3, 0xf8, 0xdc, 0x16, 0x59, 0x3a, 0x8a, 0xdb, 0xb6, 0x48, 0xdf, 0x51, 0xbc, 0x71, 0x9e, 0xd0, 0x51, 0xbc, 0x71, 0x9e, 0xd8, 0x51, 0x7c, 0xda, 0x76, 0x44, 0x76, 0x7c, 0x1e, 0xed, 0x28, 0x6e, 0xb7, 0x1d, 0xd1, 0x3c, 0x3e, 0x4f, 0xe9, 0x28, 0xbe, 0xdb, 0x76, 0x44, 0xd3, 0xf8, 0x3c, 0xa1, 0xa3, 0xf8, 0xbc, 0xed, 0x88, 0xec, 0xf8, 0x3c, 0x89, 0xc7, 0x4d, 0x79, 0xdc, 0x32, 0x8f, 0x5b, 0xe1, 0x71, 0xf3, 0x3c, 0x6e, 0x81, 0xc7, 0x2d, 0xf2, 0xb8, 0x25, 0x1e, 0x37, 0xe5, 0x71, 0xcb, 0x3c, 0x6e, 0x85, 0xc7, 0xcd, 0xf3, 0xb8, 0x05, 0x1e, 0xb7, 0xc8, 0xe3, 0x96, 0x78, 0xdc, 0x94, 0xc7, 0x2d, 0xf3, 0xb8, 0x15, 0x1e, 0x37, 0xcf, 0xe3, 0x16, 0x78, 0xdc, 0x22, 0x8f, 0x5b, 0xe2, 0x71, 0x53, 0x1e, 0xb7, 0xcc, 0xe3, 0x56, 0x78, 0xdc, 0x3c, 0x8f, 0x5b, 0xe0, 0x71, 0x8b, 0x3c, 0x6e, 0x89, 0xc7, 0x4d, 0x79, 0xdc, 0x32, 0x8f, 0x5b, 0xe1, 0x71, 0xf3, 0x3c, 0x6e, 0x81, 0xc7, 0x2d, 0xf2, 0xb8, 0x25, 0x1e, 0x37, 0xe5, 0x71, 0xcb, 0x3c, 0x6e, 0x85, 0xc7, 0xcd, 0xf3, 0xb8, 0x05, 0x1e, 0xb7, 0xc8, 0xe3, 0x96, 0x78, 0xdc, 0x94, 0xc7, 0x2d, 0x0f, 0xe2, 0x76, 0x07];
        assert_eq!((packed[0] >> 1) & 3, 2);
        assert_eq!(inflate(&packed, 1 << 20).unwrap(), text);
        assert!(inflate(&packed, 1000).is_err(), "past the most it may make");
    }
}
