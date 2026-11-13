// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! Ogg Vorbis decoded in fixed point: a port of Tremor 1.0.3 (the
//! integer decoder, as SunVox's library carries it in lib_vorbis/tremor;
//! Xiph.Org Foundation, BSD-3), operation by operation, so its 16-bit
//! output is Tremor's to the sample. Its Ogg framing kept each queue as
//! a chain of shared buffers; here each is one run of bytes, read the
//! same way. Only what playing a file in memory needs: open, read, seek
//! to a sample, the length.
//!
//! > Copyright (c) 2002, Xiph.org Foundation
//! >
//! > Redistribution and use in source and binary forms, with or without
//! > modification, are permitted provided that the following conditions
//! > are met:
//! >
//! > - Redistributions of source code must retain the above copyright
//! >   notice, this list of conditions and the following disclaimer.
//! > - Redistributions in binary form must reproduce the above copyright
//! >   notice, this list of conditions and the following disclaimer in the
//! >   documentation and/or other materials provided with the distribution.
//! > - Neither the name of the Xiph.org Foundation nor the names of its
//! >   contributors may be used to endorse or promote products derived from
//! >   this software without specific prior written permission.
//! >
//! > THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
//! > "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
//! > LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
//! > A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE FOUNDATION
//! > OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
//! > SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
//! > LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
//! > DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
//! > THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
//! > (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
//! > OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

use std::sync::Arc;

use crate::tremor_tables::*;

pub const OV_FALSE: i64 = -1;
pub const OV_EOF: i64 = -2;
pub const OV_HOLE: i64 = -3;
pub const OV_EREAD: i64 = -128;
pub const OV_EFAULT: i64 = -129;
pub const OV_EINVAL: i64 = -131;
pub const OV_ENOTVORBIS: i64 = -132;
pub const OV_EBADHEADER: i64 = -133;
pub const OV_EVERSION: i64 = -134;
pub const OV_ENOTAUDIO: i64 = -135;
pub const OV_EBADPACKET: i64 = -136;
pub const OV_EBADLINK: i64 = -137;
pub const OV_ENOSEEK: i64 = -138;

const CHUNKSIZE: i64 = 1024;

// ---------------------------------------------------------------------
// Fixed-point math (misc.h, full accuracy).

#[inline]
fn mult32(x: i32, y: i32) -> i32 {
    ((x as i64 * y as i64) >> 32) as i32
}

#[inline]
fn mult31(x: i32, y: i32) -> i32 {
    mult32(x, y).wrapping_shl(1)
}

#[inline]
fn mult31_shift15(x: i32, y: i32) -> i32 {
    let w = x as i64 * y as i64;
    ((w as u32 >> 15) as i32) | ((w >> 32) as i32).wrapping_shl(17)
}

/// XPROD31: (x, y) = (a·t + b·v, b·t − a·v).
#[inline]
fn xprod31(a: i32, b: i32, t: i32, v: i32) -> (i32, i32) {
    (mult31(a, t).wrapping_add(mult31(b, v)), mult31(b, t).wrapping_sub(mult31(a, v)))
}

/// XNPROD31: (x, y) = (a·t − b·v, b·t + a·v).
#[inline]
fn xnprod31(a: i32, b: i32, t: i32, v: i32) -> (i32, i32) {
    (mult31(a, t).wrapping_sub(mult31(b, v)), mult31(b, t).wrapping_add(mult31(a, v)))
}

#[inline]
fn clip_to_15(x: i32) -> i16 {
    x.clamp(-32768, 32767) as i16
}

/// _ilog: the bits a value needs.
fn ilog(v: u32) -> i32 {
    32 - v.leading_zeros() as i32
}

/// The other ilog (dsp.c, mapping0.c): bits for values below v.
fn ilog1(v: u32) -> i32 {
    ilog(v.saturating_sub(1))
}

// ---------------------------------------------------------------------
// The bit reader (bitwise.c) over one packet.

const MASK: [u64; 33] = [
    0x00000000, 0x00000001, 0x00000003, 0x00000007, 0x0000000f, 0x0000001f, 0x0000003f, 0x0000007f, 0x000000ff, 0x000001ff, 0x000003ff, 0x000007ff, 0x00000fff, 0x00001fff, 0x00003fff, 0x00007fff, 0x0000ffff,
    0x0001ffff, 0x0003ffff, 0x0007ffff, 0x000fffff, 0x001fffff, 0x003fffff, 0x007fffff, 0x00ffffff, 0x01ffffff, 0x03ffffff, 0x07ffffff, 0x0fffffff, 0x1fffffff, 0x3fffffff, 0x7fffffff, 0xffffffff,
];

#[derive(Default)]
struct Pack {
    d: Vec<u8>,
    pos: usize,
    headbit: i32,
    headend: i64,
}

impl Pack {
    fn new(d: Vec<u8>) -> Pack {
        let headend = d.len() as i64;
        Pack { d, pos: 0, headbit: 0, headend }
    }

    fn look(&self, bits: i32) -> i64 {
        if !(0..=32).contains(&bits) {
            return -1;
        }
        let m = MASK[bits as usize];
        let total = bits + self.headbit;
        if total as i64 >= self.headend * 8 {
            if self.headend < 0 {
                return -1;
            }
            if total == 0 {
                return 0;
            }
            if ((total + 7) / 8) as i64 > self.headend {
                return -1;
            }
        }
        let p = &self.d[self.pos..];
        let hb = self.headbit;
        let mut ret = (p[0] as u64) >> hb;
        if total > 8 {
            ret |= (p[1] as u64) << (8 - hb);
            if total > 16 {
                ret |= (p[2] as u64) << (16 - hb);
                if total > 24 {
                    ret |= (p[3] as u64) << (24 - hb);
                    if total > 32 && hb != 0 {
                        ret |= (p[4] as u64) << (32 - hb);
                    }
                }
            }
        }
        (ret & m) as i64
    }

    fn adv(&mut self, bits: i32) {
        let total = bits + self.headbit;
        self.headbit = total & 7;
        self.headend -= (total >> 3) as i64;
        self.pos = self.pos.saturating_add((total >> 3).max(0) as usize).min(self.d.len());
        if self.headend < 1 && self.headend * 8 < self.headbit as i64 {
            self.headend = -1;
        }
    }

    fn read(&mut self, bits: i32) -> i64 {
        let r = self.look(bits);
        self.adv(bits);
        r
    }

    fn eop(&self) -> bool {
        self.headend < 0
    }
}

// ---------------------------------------------------------------------
// Codebooks (codebook.c), their decode tree kept as _make_words makes it.

#[derive(Default, Clone)]
struct Codebook {
    dim: i64,
    entries: i64,
    used_entries: i64,
    dec_maxlength: i32,
    table: Vec<u32>,
    dec_type: i32,
    q_min: i32,
    q_minp: i32,
    q_del: i32,
    q_delp: i32,
    q_seq: i32,
    q_bits: i32,
    q_pack: i32,
    q_val: Vec<u16>,
}

fn float32_unpack(val: i64, point: &mut i32) -> i32 {
    let mut mant = val & 0x1fffff;
    let sign = val & 0x8000_0000;
    *point = (((val & 0x7fe0_0000) >> 21) - 788) as i32;
    if mant != 0 {
        while mant & 0x4000_0000 == 0 {
            mant <<= 1;
            *point -= 1;
        }
        if sign != 0 {
            mant = -mant;
        }
    } else {
        *point = -9999;
    }
    mant as i32
}

fn determine_node_bytes(used: i64, leafwidth: i32) -> i32 {
    if used < 2 {
        return 4;
    }
    let lw = if leafwidth == 3 { 4 } else { leafwidth };
    if ilog((3 * used - 6) as u32) + 1 <= lw * 4 {
        return if lw / 2 != 0 { lw / 2 } else { 1 };
    }
    lw
}

impl Codebook {
    /// decpack: a leaf's value as the decode table keeps it.
    fn decpack(&self, entry: i64, used_entry: i64, quantvals: i64, q1: &[u16], opb: &mut Pack, maptype: i64) -> u32 {
        let mut ret = 0u32;
        match self.dec_type {
            0 => entry as u32,
            1 => {
                let mut entry = entry;
                for j in 0..self.dim {
                    let v = if maptype == 1 {
                        let off = entry % quantvals;
                        entry /= quantvals;
                        q1.get(off as usize).copied().unwrap_or(0) as u32
                    } else {
                        opb.read(self.q_bits) as u32
                    };
                    ret |= v.wrapping_shl((self.q_bits as i64 * j) as u32);
                }
                ret
            }
            2 => {
                let mut entry = entry;
                for j in 0..self.dim {
                    let off = (entry % quantvals) as u32;
                    entry /= quantvals;
                    ret |= off.wrapping_shl((self.q_pack as i64 * j) as u32);
                }
                ret
            }
            3 => used_entry as u32,
            _ => 0,
        }
    }

    /// _make_words: the tree, a pair of children a node (leaves flagged).
    fn make_words(&mut self, l: &[i8], quantvals: i64, q1: &[u16], opb: &mut Pack, maptype: i64) -> bool {
        let n = l.len();
        let mut r = vec![0u32; (self.used_entries.max(0) as usize) * 2 + 2];
        if n < 2 {
            r[0] = 0x8000_0000;
        } else {
            let mut marker = [0u32; 33];
            let mut count = 0i64;
            let mut top = 0u32;
            for i in 0..n {
                let length = l[i] as i32;
                if length <= 0 {
                    continue;
                }
                let mut entry = marker[length as usize];
                let mut chase = 0u32;
                if count != 0 && entry == 0 {
                    return false;
                }
                let mut j = 0;
                while j < length - 1 {
                    let bit = ((entry >> (length - j - 1)) & 1) as usize;
                    if chase >= top {
                        top += 1;
                        set(&mut r, chase as usize * 2, top);
                        set(&mut r, chase as usize * 2 + 1, 0);
                    } else if get(&r, chase as usize * 2 + bit) == 0 {
                        set(&mut r, chase as usize * 2 + bit, top);
                    }
                    chase = get(&r, chase as usize * 2 + bit);
                    j += 1;
                }
                {
                    let bit = ((entry >> (length - j - 1)) & 1) as usize;
                    if chase >= top {
                        top += 1;
                        set(&mut r, chase as usize * 2 + 1, 0);
                    }
                    let v = self.decpack(i as i64, count, quantvals, q1, opb, maptype) | 0x8000_0000;
                    count += 1;
                    set(&mut r, chase as usize * 2 + bit, v);
                }
                let mut j = length as usize;
                while j > 0 {
                    if marker[j] & 1 != 0 {
                        marker[j] = marker[j - 1] << 1;
                        break;
                    }
                    marker[j] = marker[j].wrapping_add(1);
                    j -= 1;
                }
                for j in length as usize + 1..33 {
                    if (marker[j] >> 1) == entry {
                        entry = marker[j];
                        marker[j] = marker[j - 1] << 1;
                    } else {
                        break;
                    }
                }
            }
        }
        self.table = r;
        true
    }

    fn maptype1_quantvals(&self) -> i64 {
        let bits = ilog(self.entries as u32) as i64;
        if self.dim <= 0 {
            return 0;
        }
        let mut vals = self.entries >> ((bits - 1) * (self.dim - 1) / self.dim).clamp(0, 63);
        for _ in 0..100_000 {
            let mut acc = 1i64;
            let mut acc1 = 1i64;
            for _ in 0..self.dim {
                acc = acc.saturating_mul(vals);
                acc1 = acc1.saturating_mul(vals + 1);
            }
            if acc <= self.entries && acc1 > self.entries {
                return vals;
            }
            if acc > self.entries {
                vals -= 1;
            } else {
                vals += 1;
            }
        }
        vals
    }

    /// vorbis_book_unpack.
    fn unpack(opb: &mut Pack) -> Option<Codebook> {
        let mut s = Codebook::default();
        if opb.read(24) != 0x564342 {
            return None;
        }
        s.dim = opb.read(16);
        s.entries = opb.read(24);
        if s.entries == -1 {
            return None;
        }
        let entries = s.entries as usize;
        let mut lengthlist = vec![0i8; entries];
        match opb.read(1) {
            0 => {
                if opb.read(1) != 0 {
                    for l in lengthlist.iter_mut() {
                        if opb.read(1) != 0 {
                            let num = opb.read(5);
                            if num == -1 {
                                return None;
                            }
                            *l = (num + 1) as i8;
                            s.used_entries += 1;
                            if num as i32 + 1 > s.dec_maxlength {
                                s.dec_maxlength = num as i32 + 1;
                            }
                        }
                    }
                } else {
                    s.used_entries = s.entries;
                    for l in lengthlist.iter_mut() {
                        let num = opb.read(5);
                        if num == -1 {
                            return None;
                        }
                        *l = (num + 1) as i8;
                        if num as i32 + 1 > s.dec_maxlength {
                            s.dec_maxlength = num as i32 + 1;
                        }
                    }
                }
            }
            1 => {
                let mut length = opb.read(5) + 1;
                s.used_entries = s.entries;
                let mut i = 0usize;
                while i < entries {
                    let num = opb.read(ilog((entries - i) as u32));
                    if num == -1 {
                        return None;
                    }
                    let mut j = 0;
                    while j < num && i < entries {
                        lengthlist[i] = length as i8;
                        i += 1;
                        j += 1;
                    }
                    s.dec_maxlength = length as i32;
                    length += 1;
                }
            }
            _ => return None,
        }
        let maptype = opb.read(4);
        if maptype > 0 {
            s.q_min = float32_unpack(opb.read(32), &mut s.q_minp);
            s.q_del = float32_unpack(opb.read(32), &mut s.q_delp);
            s.q_bits = opb.read(4) as i32 + 1;
            s.q_seq = opb.read(1) as i32;
            s.q_del = s.q_del.wrapping_shr(s.q_bits as u32);
            s.q_delp += s.q_bits;
        }
        match maptype {
            0 => {
                s.dec_type = 0;
                if !s.make_words(&lengthlist, 0, &[], opb, maptype) {
                    return None;
                }
            }
            1 => {
                let quantvals = s.maptype1_quantvals();
                let total1 = (s.q_bits as i64 * s.dim + 8) / 8;
                let total2 = (ilog((quantvals - 1) as u32) as i64 * s.dim + 8) / 8 + (s.q_bits as i64 + 7) / 8;
                if total1 <= 4 && total1 <= total2 {
                    let q1: Vec<u16> = (0..quantvals).map(|_| opb.read(s.q_bits) as u16).collect();
                    if opb.eop() {
                        return None;
                    }
                    s.dec_type = 1;
                    if !s.make_words(&lengthlist, quantvals, &q1, opb, maptype) {
                        return None;
                    }
                } else {
                    s.q_val = (0..quantvals).map(|_| opb.read(s.q_bits) as u16 & if s.q_bits <= 8 { 0xff } else { 0xffff }).collect();
                    if opb.eop() {
                        return None;
                    }
                    s.q_pack = ilog((quantvals - 1) as u32);
                    s.dec_type = 2;
                    if !s.make_words(&lengthlist, quantvals, &[], opb, maptype) {
                        return None;
                    }
                }
            }
            2 => {
                let quantvals = s.entries * s.dim;
                if (s.q_bits as i64 * s.dim + 8) / 8 <= 4 {
                    s.dec_type = 1;
                    if !s.make_words(&lengthlist, quantvals, &[], opb, maptype) {
                        return None;
                    }
                } else {
                    s.dec_type = 3;
                    if !s.make_words(&lengthlist, quantvals, &[], opb, maptype) {
                        return None;
                    }
                    s.q_pack = (s.q_bits + 7) / 8 * s.dim as i32;
                    let mask = if s.q_bits <= 8 { 0xff } else { 0xffff };
                    s.q_val = (0..s.used_entries * s.dim).map(|_| opb.read(s.q_bits) as u16 & mask).collect();
                }
            }
            _ => return None,
        }
        if opb.eop() {
            return None;
        }
        let _ = determine_node_bytes;
        Some(s)
    }

    /// decode_packed_entry_number: an entry, or 0xFFFFFFFF.
    fn decode_entry(&self, b: &mut Pack) -> u32 {
        let mut chase = 0u32;
        let mut read = self.dec_maxlength;
        let mut lok = b.look(read);
        while lok < 0 && read > 1 {
            read -= 1;
            lok = b.look(read);
        }
        if lok < 0 {
            b.adv(1);
            return u32::MAX;
        }
        let mut i = 0;
        while i < read {
            chase = get(&self.table, chase as usize * 2 + ((lok >> i) & 1) as usize);
            if chase & 0x8000_0000 != 0 {
                break;
            }
            i += 1;
        }
        chase &= 0x7fff_ffff;
        if i < read {
            b.adv(i + 1);
            return chase;
        }
        b.adv(read + 1);
        u32::MAX
    }

    /// vorbis_book_decode: an entry of a book that maps none, -1 at the end.
    fn decode(&self, b: &mut Pack) -> i32 {
        if self.dec_type != 0 {
            return -1;
        }
        self.decode_entry(b) as i32
    }

    /// decode_map: the entry's values at `point`.
    fn decode_map(&self, b: &mut Pack, v: &mut [i32], point: i32) -> bool {
        let entry = self.decode_entry(b);
        if b.eop() {
            return false;
        }
        let dim = self.dim as usize;
        match self.dec_type {
            1 => {
                let mask = (1u32.wrapping_shl(self.q_bits as u32)).wrapping_sub(1);
                let mut e = entry;
                for x in v[..dim].iter_mut() {
                    *x = (e & mask) as i32;
                    e = e.wrapping_shr(self.q_bits as u32);
                }
            }
            2 => {
                let mask = (1u32.wrapping_shl(self.q_pack as u32)).wrapping_sub(1);
                let mut e = entry;
                for x in v[..dim].iter_mut() {
                    *x = self.q_val.get((e & mask) as usize).copied().unwrap_or(0) as i32;
                    e = e.wrapping_shr(self.q_pack as u32);
                }
            }
            3 => {
                for (i, x) in v[..dim].iter_mut().enumerate() {
                    *x = self.q_val.get((entry as usize).wrapping_mul(dim).wrapping_add(i)).copied().unwrap_or(0) as i32;
                }
            }
            _ => return false,
        }
        let shift_m = point - self.q_delp;
        let add = point - self.q_minp;
        let add = if add > 0 { self.q_min.wrapping_shr(add as u32) } else { self.q_min.wrapping_shl(add.wrapping_neg() as u32) };
        for x in v[..dim].iter_mut() {
            let p = x.wrapping_mul(self.q_del);
            *x = add.wrapping_add(if shift_m > 0 { p.wrapping_shr(shift_m as u32) } else { p.wrapping_shl(shift_m.wrapping_neg() as u32) });
        }
        if self.q_seq != 0 {
            for i in 1..dim {
                v[i] = v[i].wrapping_add(v[i - 1]);
            }
        }
        true
    }

    fn decodevs_add(&self, a: &mut [i32], at: usize, b: &mut Pack, n: i32, point: i32) -> bool {
        if self.used_entries > 0 {
            let dim = self.dim as usize;
            let step = n as usize / dim.max(1);
            let mut v = vec![0i32; dim];
            for j in 0..step {
                if !self.decode_map(b, &mut v, point) {
                    return false;
                }
                for (i, &x) in v.iter().enumerate() {
                    if let Some(y) = a.get_mut(at + j + i * step) {
                        *y = y.wrapping_add(x);
                    }
                }
            }
        }
        true
    }

    fn decodev_add(&self, a: &mut [i32], at: usize, b: &mut Pack, n: i32, point: i32) -> bool {
        if self.used_entries > 0 {
            let mut v = vec![0i32; self.dim as usize];
            let mut i = 0usize;
            while i < n as usize {
                if !self.decode_map(b, &mut v, point) {
                    return false;
                }
                for &x in &v {
                    if let Some(y) = a.get_mut(at + i) {
                        *y = y.wrapping_add(x);
                    }
                    i += 1;
                }
            }
        }
        true
    }

    fn decodev_set(&self, a: &mut [i32], at: usize, b: &mut Pack, n: i32, point: i32) -> bool {
        let dim = self.dim.max(1) as usize;
        let mut i = 0usize;
        if self.used_entries > 0 {
            let mut v = vec![0i32; dim];
            while i < n as usize {
                if !self.decode_map(b, &mut v, point) {
                    return false;
                }
                for &x in &v {
                    if let Some(y) = a.get_mut(at + i) {
                        *y = x;
                    }
                    i += 1;
                }
            }
        } else {
            while i < n as usize {
                for _ in 0..dim {
                    if let Some(y) = a.get_mut(at + i) {
                        *y = 0;
                    }
                    i += 1;
                }
            }
        }
        true
    }

    fn decodevv_add(&self, a: &mut [Vec<i32>], chans: &[usize], offset: i64, b: &mut Pack, n: i32, point: i32) -> bool {
        if self.used_entries > 0 {
            let ch = chans.len();
            let mut v = vec![0i32; self.dim as usize];
            let mut chptr = 0usize;
            let mut i = offset;
            while i < offset + n as i64 {
                if !self.decode_map(b, &mut v, point) {
                    return false;
                }
                for &x in &v {
                    if let Some(y) = a[chans[chptr]].get_mut(i as usize) {
                        *y = y.wrapping_add(x);
                    }
                    chptr += 1;
                    if chptr == ch {
                        chptr = 0;
                        i += 1;
                    }
                }
            }
        }
        true
    }
}

fn get(r: &[u32], i: usize) -> u32 {
    r.get(i).copied().unwrap_or(0)
}

fn set(r: &mut Vec<u32>, i: usize, v: u32) {
    if i >= r.len() {
        r.resize(i + 1, 0);
    }
    r[i] = v;
}

// ---------------------------------------------------------------------
// The setup (info.c, floor0.c, floor1.c, res012.c, mapping0.c).

#[derive(Clone, Default)]
struct Floor0 {
    order: i32,
    rate: i64,
    barkmap: i64,
    ampbits: i32,
    ampdb: i32,
    numbooks: i32,
    books: [u8; 16],
}

#[derive(Clone, Copy, Default)]
struct Floor1Class {
    dim: i32,
    subs: i32,
    book: u8,
    subbook: [u8; 8],
}

#[derive(Clone, Default)]
struct Floor1 {
    class: Vec<Floor1Class>,
    partitionclass: Vec<i8>,
    postlist: Vec<u16>,
    forward_index: Vec<i8>,
    hineighbor: Vec<i8>,
    loneighbor: Vec<i8>,
    partitions: i32,
    posts: i32,
    mult: i32,
}

#[derive(Clone)]
enum Floor {
    F0(Floor0),
    F1(Floor1),
}

#[derive(Clone, Default)]
struct Residue {
    typ: i32,
    stagemasks: Vec<u8>,
    stagebooks: Vec<u8>,
    begin: i64,
    end: i64,
    grouping: i32,
    partitions: i32,
    groupbook: u8,
    stages: i32,
}

#[derive(Clone, Default)]
struct Mapping {
    submaps: i32,
    chmuxlist: Option<Vec<u8>>,
    submaplist: Vec<(i8, i8)>,
    coupling: Vec<(u8, u8)>,
}

#[derive(Clone, Default)]
struct Setup {
    blocksizes: [i64; 2],
    modes: Vec<(u8, u8)>,
    maps: Vec<Mapping>,
    floors: Vec<Floor>,
    residues: Vec<Residue>,
    books: Vec<Codebook>,
}

/// vorbis_info.
#[derive(Clone, Default)]
pub struct Info {
    pub version: i64,
    pub channels: i32,
    pub rate: i64,
    pub bitrate_upper: i64,
    pub bitrate_nominal: i64,
    pub bitrate_lower: i64,
    setup: Option<Box<Setup>>,
}

impl Info {
    fn init() -> Info {
        Info { setup: Some(Box::default()), ..Info::default() }
    }

    fn ci(&self) -> &Setup {
        self.setup.as_deref().expect("codec setup")
    }

    fn blocksize(&self, zo: usize) -> i64 {
        self.setup.as_ref().map_or(-1, |c| c.blocksizes[zo])
    }
}

fn floor0_unpack(books: usize, opb: &mut Pack) -> Option<Floor> {
    let mut f = Floor0 {
        order: opb.read(8) as i32,
        rate: opb.read(16),
        barkmap: opb.read(16),
        ampbits: opb.read(6) as i32,
        ampdb: opb.read(8) as i32,
        numbooks: opb.read(4) as i32 + 1,
        books: [0; 16],
    };
    if f.order < 1 || f.rate < 1 || f.barkmap < 1 {
        return None;
    }
    for j in 0..f.numbooks.max(0) as usize {
        // A char in Tremor: a book past 127 reads before the table there.
        let b = opb.read(8) as i8;
        if b < 0 || b as usize >= books {
            return None;
        }
        f.books[j] = b as u8;
    }
    if opb.eop() {
        return None;
    }
    Some(Floor::F0(f))
}

fn floor1_unpack(books: usize, opb: &mut Pack) -> Option<Floor> {
    let mut f = Floor1 { partitions: opb.read(5) as i32, ..Floor1::default() };
    let mut maxclass = -1i32;
    for _ in 0..f.partitions.max(0) {
        let c = opb.read(4) as i8;
        f.partitionclass.push(c);
        maxclass = maxclass.max(c as i32);
    }
    for _ in 0..maxclass + 1 {
        let mut c = Floor1Class { dim: (opb.read(3) + 1) as i8 as i32, subs: opb.read(2) as i8 as i32, ..Floor1Class::default() };
        if opb.eop() {
            return None;
        }
        c.book = if c.subs != 0 { opb.read(8) as u8 } else { 0 };
        if c.book as usize >= books {
            return None;
        }
        for k in 0..(1usize << c.subs.clamp(0, 3)) {
            c.subbook[k] = (opb.read(8) - 1) as u8;
            if c.subbook[k] as usize >= books && c.subbook[k] != 0xff {
                return None;
            }
        }
        f.class.push(c);
    }
    f.mult = opb.read(2) as i32 + 1;
    let rangebits = opb.read(4) as i32;
    let mut count = 0usize;
    for j in 0..f.partitions as usize {
        count += f.class.get(f.partitionclass[j] as usize)?.dim.max(0) as usize;
    }
    f.postlist = vec![0; count + 2];
    let mut count = 0usize;
    let mut k = 0usize;
    for j in 0..f.partitions as usize {
        count += f.class[f.partitionclass[j] as usize].dim.max(0) as usize;
        while k < count {
            let t = opb.read(rangebits) as u16;
            f.postlist[k + 2] = t;
            if t as i64 >= 1i64 << rangebits.clamp(0, 62) {
                return None;
            }
            k += 1;
        }
    }
    if opb.eop() {
        return None;
    }
    f.postlist[0] = 0;
    f.postlist[1] = 1u32.wrapping_shl(rangebits as u32) as u16;
    f.posts = count as i32 + 2;
    f.forward_index = (0..f.posts).map(|j| j as i8).collect();
    mergesort(&mut f.forward_index, &f.postlist);
    for j in 0..(f.posts - 2).max(0) as usize {
        let (mut lo, mut hi) = (0usize, 1usize);
        let (mut lx, mut hx) = (0i32, f.postlist[1] as i32);
        let currentx = f.postlist[j + 2] as i32;
        for k in 0..j + 2 {
            let x = f.postlist[k] as i32;
            if x > lx && x < currentx {
                lo = k;
                lx = x;
            }
            if x < hx && x > currentx {
                hi = k;
                hx = x;
            }
        }
        f.loneighbor.push(lo as i8);
        f.hineighbor.push(hi as i8);
    }
    Some(Floor::F1(f))
}

/// vorbis_mergesort: the posts' order by x.
fn mergesort(index: &mut [i8], vals: &[u16]) {
    let n = index.len();
    let mut a = index.to_vec();
    let mut b = vec![0i8; n];
    let val = |i: i8| vals.get(i as u8 as usize).copied().unwrap_or(0);
    let mut i = 1usize;
    while i < n {
        let mut j = 0usize;
        while j + i < n {
            let mut k1 = j;
            let mid = j + i;
            let mut k2 = mid;
            let end = (j + i * 2).min(n);
            while k1 < mid && k2 < end {
                if val(a[k1]) < val(a[k2]) {
                    b[j] = a[k1];
                    k1 += 1;
                } else {
                    b[j] = a[k2];
                    k2 += 1;
                }
                j += 1;
            }
            while k1 < mid {
                b[j] = a[k1];
                j += 1;
                k1 += 1;
            }
            while k2 < end {
                b[j] = a[k2];
                j += 1;
                k2 += 1;
            }
        }
        while j < n {
            b[j] = a[j];
            j += 1;
        }
        std::mem::swap(&mut a, &mut b);
        i <<= 1;
    }
    index.copy_from_slice(&a);
}

fn res_unpack(books: usize, opb: &mut Pack) -> Option<Residue> {
    let mut r = Residue { typ: opb.read(16) as i32, ..Residue::default() };
    if !(0..=2).contains(&r.typ) {
        return None;
    }
    r.begin = opb.read(24);
    r.end = opb.read(24);
    r.grouping = opb.read(24) as i32 + 1;
    r.partitions = (opb.read(6) + 1) as i8 as i32;
    r.groupbook = opb.read(8) as u8;
    if r.groupbook as usize >= books {
        return None;
    }
    for _ in 0..r.partitions.max(0) {
        let mut cascade = opb.read(3);
        if opb.read(1) != 0 {
            cascade |= opb.read(5) << 3;
        }
        r.stagemasks.push(cascade as u8);
    }
    for j in 0..r.partitions.max(0) as usize {
        for k in 0..8 {
            if (r.stagemasks[j] >> k) & 1 != 0 {
                let book = opb.read(8) as u8;
                if book as usize >= books {
                    return None;
                }
                r.stagebooks.push(book);
                if k + 1 > r.stages {
                    r.stages = k + 1;
                }
            } else {
                r.stagebooks.push(0xff);
            }
        }
    }
    if opb.eop() {
        return None;
    }
    Some(r)
}

fn mapping_unpack(channels: i32, floors: usize, residues: usize, opb: &mut Pack) -> Option<Mapping> {
    let mut m = Mapping { submaps: if opb.read(1) != 0 { opb.read(4) as i32 + 1 } else { 1 }, ..Mapping::default() };
    if opb.read(1) != 0 {
        let steps = opb.read(8) + 1;
        for _ in 0..steps {
            let mag = opb.read(ilog1(channels as u32)) as u8;
            let ang = opb.read(ilog1(channels as u32)) as u8;
            if mag == ang || mag as i32 >= channels || ang as i32 >= channels {
                return None;
            }
            m.coupling.push((mag, ang));
        }
    }
    if opb.read(2) > 0 {
        return None;
    }
    if m.submaps > 1 {
        let mut l = Vec::new();
        for _ in 0..channels {
            let c = opb.read(4) as u8;
            if c as i32 >= m.submaps {
                return None;
            }
            l.push(c);
        }
        m.chmuxlist = Some(l);
    }
    for _ in 0..m.submaps {
        let _ = opb.read(8);
        let floor = opb.read(8) as i8;
        if floor < 0 || floor as usize >= floors {
            return None;
        }
        let residue = opb.read(8) as i8;
        if residue < 0 || residue as usize >= residues {
            return None;
        }
        m.submaplist.push((floor, residue));
    }
    Some(m)
}

fn unpack_info(vi: &mut Info, opb: &mut Pack) -> i64 {
    vi.version = opb.read(32);
    if vi.version != 0 {
        return OV_EVERSION;
    }
    vi.channels = opb.read(8) as i32;
    vi.rate = opb.read(32);
    vi.bitrate_upper = opb.read(32);
    vi.bitrate_nominal = opb.read(32);
    vi.bitrate_lower = opb.read(32);
    let b0 = 1i64 << opb.read(4).clamp(0, 62);
    let b1 = 1i64 << opb.read(4).clamp(0, 62);
    let ok = !(vi.rate < 64000 && b1 > 4096) && vi.rate >= 1 && vi.channels >= 1 && b0 >= 64 && b1 >= b0 && b1 <= 8192 && opb.read(1) == 1;
    if !ok {
        *vi = Info::default();
        return OV_EBADHEADER;
    }
    if let Some(ci) = vi.setup.as_mut() {
        ci.blocksizes = [b0, b1];
    }
    0
}

fn unpack_books(vi: &mut Info, opb: &mut Pack) -> i64 {
    let channels = vi.channels;
    let r = (|| -> Option<Setup> {
        let mut ci = Setup { blocksizes: vi.ci().blocksizes, ..Setup::default() };
        let books = opb.read(8) + 1;
        for _ in 0..books {
            ci.books.push(Codebook::unpack(opb)?);
        }
        let mut i = opb.read(6);
        while i >= 0 {
            if opb.read(16) != 0 {
                return None;
            }
            i -= 1;
        }
        let nb = ci.books.len();
        let floors = opb.read(6) + 1;
        for _ in 0..floors {
            let ty = opb.read(16) as i8;
            ci.floors.push(match ty {
                0 => floor0_unpack(nb, opb)?,
                1 => floor1_unpack(nb, opb)?,
                _ => return None,
            });
        }
        let residues = opb.read(6) + 1;
        for _ in 0..residues {
            ci.residues.push(res_unpack(nb, opb)?);
        }
        let maps = opb.read(6) + 1;
        for _ in 0..maps {
            if opb.read(16) != 0 {
                return None;
            }
            ci.maps.push(mapping_unpack(channels, ci.floors.len(), ci.residues.len(), opb)?);
        }
        let modes = opb.read(6) + 1;
        for _ in 0..modes {
            let blockflag = opb.read(1) as u8;
            if opb.read(16) != 0 || opb.read(16) != 0 {
                return None;
            }
            let mapping = opb.read(8) as u8;
            if mapping as usize >= ci.maps.len() {
                return None;
            }
            ci.modes.push((blockflag, mapping));
        }
        if opb.read(1) != 1 {
            return None;
        }
        Some(ci)
    })();
    match r {
        Some(ci) => {
            vi.setup = Some(Box::new(ci));
            0
        }
        None => {
            *vi = Info::default();
            OV_EBADHEADER
        }
    }
}

/// vorbis_dsp_headerin: one of the three headers.
fn headerin(vi: &mut Info, vendor: &mut bool, op: &Packet) -> i64 {
    let mut opb = Pack::new(op.data.clone());
    let packtype = opb.read(8);
    let mut buffer = [0u8; 6];
    for b in buffer.iter_mut() {
        *b = opb.read(8) as u8;
    }
    if &buffer != b"vorbis" {
        return OV_ENOTVORBIS;
    }
    match packtype {
        0x01 => {
            if op.b_o_s == 0 || vi.rate != 0 {
                return OV_EBADHEADER;
            }
            unpack_info(vi, &mut opb)
        }
        0x03 => {
            if vi.rate == 0 {
                return OV_EBADHEADER;
            }
            // _vorbis_unpack_comment, its strings passed over.
            let vendorlen = opb.read(32) as i32;
            if vendorlen < 0 {
                return OV_EBADHEADER;
            }
            *vendor = true;
            skip_bytes(&mut opb, vendorlen as i64);
            let comments = opb.read(32) as i32;
            if comments < 0 {
                *vendor = false;
                return OV_EBADHEADER;
            }
            for _ in 0..comments {
                let len = opb.read(32) as i32;
                if len < 0 {
                    *vendor = false;
                    return OV_EBADHEADER;
                }
                skip_bytes(&mut opb, len as i64);
            }
            if opb.read(1) != 1 {
                *vendor = false;
                return OV_EBADHEADER;
            }
            0
        }
        0x05 => {
            if vi.rate == 0 || !*vendor {
                return OV_EBADHEADER;
            }
            unpack_books(vi, &mut opb)
        }
        _ => OV_EBADHEADER,
    }
}

/// `n` bytes read and dropped (what _v_readstring's reads do to the position).
fn skip_bytes(opb: &mut Pack, n: i64) {
    let mut n = n;
    while n > 0 && !opb.eop() {
        opb.adv(8);
        n -= 1;
    }
    if n > 0 {
        opb.adv(8);
    }
}

// ---------------------------------------------------------------------
// The floors' decoding.

fn floor0_inverse1(f: &Floor0, ci: &Setup, opb: &mut Pack) -> Option<Vec<i32>> {
    let ampraw = opb.read(f.ampbits) as i32;
    if ampraw > 0 {
        let maxval = (1i64 << f.ampbits) - 1;
        let amp = ((((ampraw * f.ampdb) << 4) as i64) / maxval) as i32;
        let booknum = opb.read(ilog(f.numbooks as u32)) as i32;
        if booknum != -1 && booknum < f.numbooks {
            let b = ci.books.get(f.books[booknum as usize] as usize)?;
            let mut lsp = vec![0i32; f.order as usize + 1];
            let mut j = 0;
            while j < f.order {
                if !b.decodev_set(&mut lsp, j as usize, opb, b.dim as i32, -24) {
                    return None;
                }
                j += b.dim as i32;
            }
            let mut last = 0i32;
            let mut j = 0usize;
            while (j as i32) < f.order {
                for _ in 0..b.dim {
                    if let Some(x) = lsp.get_mut(j) {
                        *x = x.wrapping_add(last);
                    }
                    j += 1;
                }
                last = lsp.get(j - 1).copied().unwrap_or(0);
            }
            lsp[f.order as usize] = amp;
            return Some(lsp);
        }
    }
    None
}

const BARKLOOK: [u32; 54] = [
    0, 51, 102, 154, 206, 258, 311, 365, 420, 477, 535, 594, 656, 719, 785, 854, 926, 1002, 1082, 1166, 1256, 1352, 1454, 1564, 1683, 1812, 1953, 2107, 2276, 2463, 2670, 2900, 3155, 3440, 3756, 4106, 4493, 4919, 5387, 5901, 6466, 7094,
    7798, 8599, 9528, 10623, 11935, 13524, 15453, 17775, 20517, 23667, 27183, 31004,
];

fn bark(i: usize) -> u32 {
    BARKLOOK.get(i).copied().unwrap_or(u32::MAX >> 1)
}

fn to_bark(n: i32) -> i32 {
    let mut i = 0usize;
    while i < 54 {
        if n as i64 >= bark(i) as i64 && (n as i64) < bark(i + 1) as i64 {
            break;
        }
        i += 1;
    }
    if i == 54 {
        54 << 14
    } else {
        let d = (bark(i + 1) - bark(i)) as u64;
        ((i as i32) << 14) + (((n as i64 - bark(i) as i64) as u64).wrapping_mul((1u64 << 31) / d) >> 17) as i32
    }
}

const MLOOP_1: [u8; 64] = [
    0, 10, 11, 11, 12, 12, 12, 12, 13, 13, 13, 13, 13, 13, 13, 13, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15,
    15, 15, 15, 15, 15, 15,
];
const MLOOP_2: [u8; 64] = [
    0, 4, 5, 5, 6, 6, 6, 6, 7, 7, 7, 7, 7, 7, 7, 7, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9,
];
const MLOOP_3: [u8; 8] = [0, 1, 2, 2, 3, 3, 3, 3];

fn coslook_i(a: i64) -> i32 {
    let i = (a >> 9) as usize;
    let d = (a & 511) as i32;
    let c = |k: usize| COS.get(k).copied().unwrap_or(0);
    c(i).wrapping_sub(d.wrapping_mul(c(i) - c(i + 1)) >> 9)
}

fn coslook2_i(a: i64) -> i32 {
    let i = (a >> 9) as usize;
    let d = (a & 511) as i32;
    let c = |k: usize| COS.get(k).copied().unwrap_or(0);
    (c(i).wrapping_shl(9).wrapping_sub(d.wrapping_mul(c(i) - c(i + 1)))) >> 9
}

fn invsqlook_i(a: i64, e: i64) -> i32 {
    const ADJUST_SQRT2: [i64; 2] = [8192, 5792];
    let i = ((a & 0x7fff) >> 9) as usize;
    let d = a & 1023;
    let mut val = INVSQ[i] - ((INVSQ_DEL[i.min(63)] * d) >> 10);
    val *= ADJUST_SQRT2[(e & 1) as usize];
    let e = (e >> 1) + 21;
    (val >> e.clamp(0, 63)) as i32
}

fn from_db_look_i(a: i64) -> i32 {
    if a > 0 {
        return 0x7fffffff;
    }
    if a < (-140i64 << 12) {
        return 0;
    }
    FROM_DB[(((a + (140 << 12)) * 467) >> 20) as usize]
}

fn mloop(x: u32) -> u32 {
    let s = MLOOP_1[(x >> 25) as usize] as u32;
    if s != 0 {
        return s;
    }
    let s = MLOOP_2[((x >> 19) & 63) as usize] as u32;
    if s != 0 {
        return s;
    }
    MLOOP_3[((x >> 16) & 7) as usize] as u32
}

#[allow(clippy::too_many_arguments)]
fn lsp_to_curve(curve: &mut [i32], n: i32, ln: i32, lsp: &[i32], m: i32, amp: i32, ampoffset: i32, nyq: i32) {
    let ampoffseti = ampoffset.wrapping_mul(4096);
    let ampi = amp;
    let mut ilsp = vec![0i32; m as usize];
    let imap = ((1u64 << 31) / ln as u64) as u32;
    let tbnyq1 = (to_bark(nyq) as u32) << 1;
    let mut f = 0i32;
    let fdx = n;
    let fbase = nyq / fdx;
    let mut ferr = 0i32;
    let fdy = nyq - fbase * fdx;
    let mut map = 0i32;
    let nextf_of = |nextbark: u32| -> i32 { (bark((nextbark >> 14) as usize) as i32).wrapping_add(((nextbark & 0x3fff).wrapping_mul(bark((nextbark >> 14) as usize + 1).wrapping_sub(bark((nextbark >> 14) as usize))) >> 14) as i32) };
    let nextbark = mult31((imap >> 1) as i32, tbnyq1 as i32) as u32;
    let mut nextf = nextf_of(nextbark);
    for i in 0..m as usize {
        let val = mult32(lsp[i], 0x517cc2);
        if val < 0 || (val >> 9) >= 128 {
            for c in curve[..n as usize].iter_mut() {
                *c = 0;
            }
            return;
        }
        ilsp[i] = coslook_i(val as i64);
    }
    let mut i = 0i32;
    while i < n {
        let mut pi: u32 = 46341;
        let mut qi: u32 = 46341;
        let mut qexp: i32 = 0;
        let wi = coslook2_i(((map as u32).wrapping_mul(imap) >> 15) as i64);
        let labs = |x: i32| (x as i64).abs() as u32;
        qi = qi.wrapping_mul(labs(ilsp[0].wrapping_sub(wi)));
        pi = pi.wrapping_mul(labs(ilsp.get(1).copied().unwrap_or(0).wrapping_sub(wi)));
        let mut j = 3usize;
        while (j as i32) < m {
            let shift = mloop(pi | qi);
            qi = (qi >> shift).wrapping_mul(labs(ilsp[j - 1].wrapping_sub(wi)));
            pi = (pi >> shift).wrapping_mul(labs(ilsp[j].wrapping_sub(wi)));
            qexp += shift as i32;
            j += 2;
        }
        let shift = mloop(pi | qi);
        if m & 1 != 0 {
            qi = (qi >> shift).wrapping_mul(labs(ilsp.get(j - 1).copied().unwrap_or(0).wrapping_sub(wi)));
            pi = (pi >> shift) << 14;
            qexp += shift as i32;
            let shift = mloop(pi | qi);
            pi >>= shift;
            qi >>= shift;
            qexp += shift as i32 - 14 * ((m + 1) >> 1);
            pi = pi.wrapping_mul(pi) >> 16;
            qi = qi.wrapping_mul(qi) >> 16;
            qexp = qexp * 2 + m;
            pi = pi.wrapping_mul(((1i32 << 14) - (wi.wrapping_mul(wi) >> 14)) as u32);
            qi = qi.wrapping_add(pi >> 14);
        } else {
            pi >>= shift;
            qi >>= shift;
            qexp += shift as i32 - 7 * m;
            pi = pi.wrapping_mul(pi) >> 16;
            qi = qi.wrapping_mul(qi) >> 16;
            qexp = qexp * 2 + m;
            pi = pi.wrapping_mul(((1i32 << 14) - wi) as u32);
            qi = qi.wrapping_mul(((1i32 << 14) + wi) as u32);
            qi = qi.wrapping_add(pi) >> 14;
        }
        if qi & 0xffff_0000 != 0 {
            qi >>= 1;
            qexp += 1;
        } else {
            while qi != 0 && qi & 0x8000 == 0 {
                qi <<= 1;
                qexp -= 1;
            }
        }
        let amp = from_db_look_i(ampi.wrapping_mul(invsqlook_i(qi as i64, qexp as i64)).wrapping_sub(ampoffseti) as i64);
        curve[i as usize] = mult31_shift15(curve[i as usize], amp);
        loop {
            i += 1;
            if i >= n {
                break;
            }
            ferr += fdy;
            if ferr >= fdx {
                ferr -= fdx;
                f += 1;
            }
            f += fbase;
            if f >= nextf {
                break;
            }
            curve[i as usize] = mult31_shift15(curve[i as usize], amp);
        }
        loop {
            map += 1;
            if map + 1 < ln {
                let nextbark = mult31(((map + 1) as u32).wrapping_mul(imap >> 1) as i32, tbnyq1 as i32) as u32;
                nextf = nextf_of(nextbark);
                if f <= nextf {
                    break;
                }
            } else {
                nextf = 9999999;
                break;
            }
        }
        if map >= ln {
            map = ln - 1;
            nextf = 9999999;
        }
    }
}

fn render_point(x0: i32, x1: i32, y0: i32, y1: i32, x: i32) -> i32 {
    let y0 = y0 & 0x7fff;
    let y1 = y1 & 0x7fff;
    let dy = y1 - y0;
    let adx = x1 - x0;
    let ady = dy.abs();
    let err = ady * (x - x0);
    let off = if adx != 0 { err / adx } else { 0 };
    if dy < 0 {
        y0 - off
    } else {
        y0 + off
    }
}

fn render_line(n: i32, x0: i32, x1: i32, y0: i32, y1: i32, d: &mut [i32]) {
    let dy = y1 - y0;
    let adx = x1 - x0;
    if adx == 0 {
        return;
    }
    let mut ady = dy.abs();
    let base = dy / adx;
    let sy = if dy < 0 { base - 1 } else { base + 1 };
    let mut x = x0;
    let mut y = y0;
    let mut err = 0;
    let n = n.min(x1);
    ady -= (base * adx).abs();
    let db = |y: i32| FROM_DB.get(y as usize).copied().unwrap_or(0);
    if x < n {
        d[x as usize] = mult31_shift15(d[x as usize], db(y));
    }
    loop {
        x += 1;
        if x >= n {
            break;
        }
        err += ady;
        if err >= adx {
            err -= adx;
            y += sy;
        } else {
            y += base;
        }
        d[x as usize] = mult31_shift15(d[x as usize], db(y));
    }
}

const QUANT_LOOK: [i32; 4] = [256, 128, 86, 64];

fn floor1_inverse1(f: &Floor1, ci: &Setup, opb: &mut Pack) -> Option<Vec<i32>> {
    let quant_q = QUANT_LOOK[(f.mult - 1).clamp(0, 3) as usize];
    let mut fit = vec![0i32; f.posts.max(2) as usize];
    if opb.read(1) != 1 {
        return None;
    }
    fit[0] = opb.read(ilog((quant_q - 1) as u32)) as i32;
    fit[1] = opb.read(ilog((quant_q - 1) as u32)) as i32;
    let mut j = 2usize;
    for i in 0..f.partitions as usize {
        let class = f.class[f.partitionclass[i] as usize];
        let cdim = class.dim;
        let csubbits = class.subs;
        let csub = 1i32 << csubbits.clamp(0, 30);
        let mut cval = 0i32;
        if csubbits != 0 {
            cval = ci.books[class.book as usize].decode(opb);
            if cval == -1 {
                return None;
            }
        }
        for k in 0..cdim.max(0) as usize {
            let book = class.subbook[(cval & (csub - 1)) as usize & 7];
            cval >>= csubbits;
            if book != 0xff {
                let v = ci.books.get(book as usize).map_or(-1, |b| b.decode(opb));
                if let Some(x) = fit.get_mut(j + k) {
                    *x = v;
                }
                if v == -1 {
                    return None;
                }
            } else if let Some(x) = fit.get_mut(j + k) {
                *x = 0;
            }
        }
        j += cdim.max(0) as usize;
    }
    for i in 2..f.posts as usize {
        let lo = f.loneighbor[i - 2] as u8 as usize;
        let hi = f.hineighbor[i - 2] as u8 as usize;
        let predicted = render_point(f.postlist[lo] as i32, f.postlist[hi] as i32, fit[lo], fit[hi], f.postlist[i] as i32);
        let hiroom = quant_q - predicted;
        let loroom = predicted;
        let room = hiroom.min(loroom) << 1;
        let mut val = fit[i];
        if val != 0 {
            if val >= room {
                if hiroom > loroom {
                    val -= loroom;
                } else {
                    val = -1 - (val - hiroom);
                }
            } else if val & 1 != 0 {
                val = -((val + 1) >> 1);
            } else {
                val >>= 1;
            }
            fit[i] = val + predicted;
            fit[lo] &= 0x7fff;
            fit[hi] &= 0x7fff;
        } else {
            fit[i] = predicted | 0x8000;
        }
    }
    Some(fit)
}

fn floor1_inverse2(f: &Floor1, n: i32, fit: Option<&[i32]>, out: &mut [i32]) {
    let Some(fit) = fit else {
        for o in out[..n as usize].iter_mut() {
            *o = 0;
        }
        return;
    };
    let mut hx = 0i32;
    let mut lx = 0i32;
    let mut ly = fit[0].wrapping_mul(f.mult);
    for j in 1..f.posts as usize {
        let current = f.forward_index[j] as u8 as usize;
        let mut hy = fit[current] & 0x7fff;
        if hy == fit[current] {
            hy *= f.mult;
            hx = f.postlist[current] as i32;
            render_line(n, lx, hx, ly, hy, out);
            lx = hx;
            ly = hy;
        }
    }
    for o in out[hx.max(0) as usize..n as usize].iter_mut() {
        *o = o.wrapping_mul(ly);
    }
}

// ---------------------------------------------------------------------
// The residue (res012.c).

fn res_inverse(ci: &Setup, w: usize, info: &Residue, work: &mut [Vec<i32>], chans: &[usize], nonzero: &[bool], opb: &mut Pack) {
    let Some(phrasebook) = ci.books.get(info.groupbook as usize) else { return };
    let mut spp = info.grouping;
    let ppw = phrasebook.dim.max(1) as usize;
    let pcmend = ci.blocksizes[w];
    let parts = info.partitions;
    let stagemask = |p: i8| info.stagemasks.get(p as i32 as usize).copied().unwrap_or(0) as i32;
    let stagebook = |p: i8, s: i32| -> Option<&Codebook> { ci.books.get(*info.stagebooks.get((((p as i32) << 3) + s) as usize)? as usize) };
    // The partition numbers of a word, as Tremor's chars hold them.
    let split = |pw: &mut [i8], i: usize, temp: i32| {
        let mut temp = temp;
        for k in 0..ppw {
            let div = pw[i + k] as i32 as u32;
            let q = (temp as u32).checked_div(div).unwrap_or(0) as i8;
            pw[i + k] = q;
            temp = (temp as u32).wrapping_sub((q as i32 as u32).wrapping_mul(div)) as i32;
        }
    };
    let fill = |pw: &mut [i8], i: usize| {
        pw[i + ppw - 1] = 1;
        for k in (0..ppw.saturating_sub(1)).rev() {
            pw[i + k] = (pw[i + k + 1] as i32).wrapping_mul(parts) as i8;
        }
    };
    if info.typ < 2 {
        let max = pcmend >> 1;
        let end = info.end.min(max);
        let n = end - info.begin;
        if n <= 0 {
            return;
        }
        let partvals = (n / spp as i64) as usize;
        let partwords = partvals.div_ceil(ppw);
        let used: Vec<usize> = chans.iter().zip(nonzero).filter(|(_, &z)| z).map(|(&c, _)| c).collect();
        if used.is_empty() {
            return;
        }
        let ch = used.len();
        let mut partword = vec![vec![0i8; partwords * ppw + ppw]; ch];
        for s in 0..info.stages {
            let mut i = 0usize;
            while i < partvals {
                if s == 0 {
                    fill(&mut partword[0], i);
                    for j in 1..ch {
                        for k in (0..ppw).rev() {
                            partword[j][i + k] = partword[j - 1][i + k];
                        }
                    }
                    for j in 0..ch {
                        let temp = phrasebook.decode(opb);
                        if temp == -1 {
                            return;
                        }
                        split(&mut partword[j], i, temp);
                    }
                }
                let mut k = 0;
                while k < ppw && i < partvals {
                    for j in 0..ch {
                        let offset = (info.begin + i as i64 * spp as i64) as usize;
                        let p = partword[j][i];
                        if stagemask(p) & (1 << s) != 0 {
                            let Some(book) = stagebook(p, s) else { return };
                            let ok = if info.typ != 0 { book.decodev_add(&mut work[used[j]], offset, opb, spp, -8) } else { book.decodevs_add(&mut work[used[j]], offset, opb, spp, -8) };
                            if !ok {
                                return;
                            }
                        }
                    }
                    k += 1;
                    i += 1;
                }
            }
        }
    } else {
        let ch = chans.len() as i64;
        let max = (pcmend * ch) >> 1;
        let end = info.end.min(max);
        let n = end - info.begin;
        if n <= 0 {
            return;
        }
        let partvals = (n / spp as i64) as usize;
        let partwords = partvals.div_ceil(ppw);
        let mut partword = vec![0i8; partwords * ppw + ppw];
        let beginoff = info.begin / ch;
        if !nonzero.iter().any(|&z| z) {
            return;
        }
        spp /= ch as i32;
        for s in 0..info.stages {
            let mut i = 0usize;
            while i < partvals {
                if s == 0 {
                    fill(&mut partword, i);
                    let temp = phrasebook.decode(opb);
                    if temp == -1 {
                        return;
                    }
                    split(&mut partword, i, temp);
                }
                let mut k = 0;
                while k < ppw && i < partvals {
                    let p = partword[i];
                    if stagemask(p) & (1 << s) != 0 {
                        let Some(book) = stagebook(p, s) else { return };
                        if !book.decodevv_add(work, chans, i as i64 * spp as i64 + beginoff, opb, spp, -8) {
                            return;
                        }
                    }
                    k += 1;
                    i += 1;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------
// The inverse MDCT (mdct.c), in place on the first n/2 values.

const CPI3_8: i32 = 0x30fbc54d;
const CPI2_8: i32 = 0x5a82799a;
const CPI1_8: i32 = 0x7641af3d;

fn presymmetry(x: &mut [i32], n2: isize, step: isize) {
    let n4 = n2 >> 1;
    let t = &SINCOS0;
    let mut ax = n2 - 3;
    let mut ti = 0isize;
    loop {
        let (r0, r2) = (x[ax as usize], x[ax as usize + 2]);
        let (a, b) = xprod31(r0, r2, t[ti as usize], t[ti as usize + 1]);
        x[ax as usize] = a;
        x[ax as usize + 2] = b;
        ti += step;
        ax -= 4;
        if ax < n4 {
            break;
        }
    }
    loop {
        let (r0, r2) = (x[ax as usize], x[ax as usize + 2]);
        let (a, b) = xprod31(r0, r2, t[ti as usize + 1], t[ti as usize]);
        x[ax as usize] = a;
        x[ax as usize + 2] = b;
        ti -= step;
        ax -= 4;
        if ax < 0 {
            break;
        }
    }
    let mut ax = n2 - 4;
    let mut bx = 0isize;
    let mut ti = 0isize;
    loop {
        let (ri0, ri2) = (x[ax as usize], x[ax as usize + 2]);
        let (ro0, ro2) = (x[bx as usize], x[bx as usize + 2]);
        let (a, b) = xnprod31(ro2, ro0, t[ti as usize + 1], t[ti as usize]);
        x[ax as usize] = a;
        x[ax as usize + 2] = b;
        ti += step;
        let (a, b) = xnprod31(ri2, ri0, t[ti as usize], t[ti as usize + 1]);
        x[bx as usize] = a;
        x[bx as usize + 2] = b;
        ax -= 4;
        bx += 4;
        if ax < n4 {
            break;
        }
    }
}

fn butterfly_8(x: &mut [i32]) {
    let r0 = x[0].wrapping_add(x[1]);
    let r1 = x[0].wrapping_sub(x[1]);
    let r2 = x[2].wrapping_add(x[3]);
    let r3 = x[2].wrapping_sub(x[3]);
    let r4 = x[4].wrapping_add(x[5]);
    let r5 = x[4].wrapping_sub(x[5]);
    let r6 = x[6].wrapping_add(x[7]);
    let r7 = x[6].wrapping_sub(x[7]);
    x[0] = r5.wrapping_add(r3);
    x[1] = r7.wrapping_sub(r1);
    x[2] = r5.wrapping_sub(r3);
    x[3] = r7.wrapping_add(r1);
    x[4] = r4.wrapping_sub(r0);
    x[5] = r6.wrapping_sub(r2);
    x[6] = r4.wrapping_add(r0);
    x[7] = r6.wrapping_add(r2);
}

fn butterfly_16(x: &mut [i32]) {
    let r0 = x[8].wrapping_sub(x[9]);
    x[8] = x[8].wrapping_add(x[9]);
    let r1 = x[10].wrapping_sub(x[11]);
    x[10] = x[10].wrapping_add(x[11]);
    let r2 = x[1].wrapping_sub(x[0]);
    x[9] = x[1].wrapping_add(x[0]);
    let r3 = x[3].wrapping_sub(x[2]);
    x[11] = x[3].wrapping_add(x[2]);
    x[0] = mult31(r0.wrapping_sub(r1), CPI2_8);
    x[1] = mult31(r2.wrapping_add(r3), CPI2_8);
    x[2] = mult31(r0.wrapping_add(r1), CPI2_8);
    x[3] = mult31(r3.wrapping_sub(r2), CPI2_8);
    let r2 = x[12].wrapping_sub(x[13]);
    x[12] = x[12].wrapping_add(x[13]);
    let r3 = x[14].wrapping_sub(x[15]);
    x[14] = x[14].wrapping_add(x[15]);
    let r0 = x[4].wrapping_sub(x[5]);
    x[13] = x[5].wrapping_add(x[4]);
    let r1 = x[7].wrapping_sub(x[6]);
    x[15] = x[7].wrapping_add(x[6]);
    x[4] = r2;
    x[5] = r1;
    x[6] = r3;
    x[7] = r0;
    butterfly_8(&mut x[..8]);
    butterfly_8(&mut x[8..16]);
}

fn butterfly_32(x: &mut [i32]) {
    let r0 = x[16].wrapping_sub(x[17]);
    x[16] = x[16].wrapping_add(x[17]);
    let r1 = x[18].wrapping_sub(x[19]);
    x[18] = x[18].wrapping_add(x[19]);
    let r2 = x[1].wrapping_sub(x[0]);
    x[17] = x[1].wrapping_add(x[0]);
    let r3 = x[3].wrapping_sub(x[2]);
    x[19] = x[3].wrapping_add(x[2]);
    (x[0], x[2]) = xnprod31(r0, r1, CPI3_8, CPI1_8);
    (x[1], x[3]) = xprod31(r2, r3, CPI1_8, CPI3_8);
    let r0 = x[20].wrapping_sub(x[21]);
    x[20] = x[20].wrapping_add(x[21]);
    let r1 = x[22].wrapping_sub(x[23]);
    x[22] = x[22].wrapping_add(x[23]);
    let r2 = x[5].wrapping_sub(x[4]);
    x[21] = x[5].wrapping_add(x[4]);
    let r3 = x[7].wrapping_sub(x[6]);
    x[23] = x[7].wrapping_add(x[6]);
    x[4] = mult31(r0.wrapping_sub(r1), CPI2_8);
    x[5] = mult31(r3.wrapping_add(r2), CPI2_8);
    x[6] = mult31(r0.wrapping_add(r1), CPI2_8);
    x[7] = mult31(r3.wrapping_sub(r2), CPI2_8);
    let r0 = x[24].wrapping_sub(x[25]);
    x[24] = x[24].wrapping_add(x[25]);
    let r1 = x[26].wrapping_sub(x[27]);
    x[26] = x[26].wrapping_add(x[27]);
    let r2 = x[9].wrapping_sub(x[8]);
    x[25] = x[9].wrapping_add(x[8]);
    let r3 = x[11].wrapping_sub(x[10]);
    x[27] = x[11].wrapping_add(x[10]);
    (x[8], x[10]) = xnprod31(r0, r1, CPI1_8, CPI3_8);
    (x[9], x[11]) = xprod31(r2, r3, CPI3_8, CPI1_8);
    let r0 = x[28].wrapping_sub(x[29]);
    x[28] = x[28].wrapping_add(x[29]);
    let r1 = x[30].wrapping_sub(x[31]);
    x[30] = x[30].wrapping_add(x[31]);
    let r2 = x[12].wrapping_sub(x[13]);
    x[29] = x[13].wrapping_add(x[12]);
    let r3 = x[15].wrapping_sub(x[14]);
    x[31] = x[15].wrapping_add(x[14]);
    x[12] = r0;
    x[13] = r3;
    x[14] = r1;
    x[15] = r2;
    butterfly_16(&mut x[..16]);
    butterfly_16(&mut x[16..32]);
}

fn butterfly_generic(x: &mut [i32], points: usize, step: usize) {
    let t = &SINCOS0;
    let mut ti = 0usize;
    let mut x1 = points as isize - 4;
    let mut x2 = (points >> 1) as isize - 4;
    loop {
        let (a, b) = (x1 as usize, x2 as usize);
        let r0 = x[a].wrapping_sub(x[a + 1]);
        x[a] = x[a].wrapping_add(x[a + 1]);
        let r1 = x[a + 3].wrapping_sub(x[a + 2]);
        x[a + 2] = x[a + 2].wrapping_add(x[a + 3]);
        let r2 = x[b + 1].wrapping_sub(x[b]);
        x[a + 1] = x[b + 1].wrapping_add(x[b]);
        let r3 = x[b + 3].wrapping_sub(x[b + 2]);
        x[a + 3] = x[b + 3].wrapping_add(x[b + 2]);
        (x[b], x[b + 2]) = xprod31(r1, r0, t[ti], t[ti + 1]);
        (x[b + 1], x[b + 3]) = xprod31(r2, r3, t[ti], t[ti + 1]);
        ti += step;
        x1 -= 4;
        x2 -= 4;
        if ti >= 1024 {
            break;
        }
    }
    loop {
        let (a, b) = (x1 as usize, x2 as usize);
        let r0 = x[a].wrapping_sub(x[a + 1]);
        x[a] = x[a].wrapping_add(x[a + 1]);
        let r1 = x[a + 2].wrapping_sub(x[a + 3]);
        x[a + 2] = x[a + 2].wrapping_add(x[a + 3]);
        let r2 = x[b].wrapping_sub(x[b + 1]);
        x[a + 1] = x[b + 1].wrapping_add(x[b]);
        let r3 = x[b + 3].wrapping_sub(x[b + 2]);
        x[a + 3] = x[b + 3].wrapping_add(x[b + 2]);
        (x[b], x[b + 2]) = xnprod31(r0, r1, t[ti], t[ti + 1]);
        (x[b + 1], x[b + 3]) = xnprod31(r3, r2, t[ti], t[ti + 1]);
        ti -= step;
        x1 -= 4;
        x2 -= 4;
        if ti == 0 {
            break;
        }
    }
}

fn butterflies(x: &mut [i32], points: usize, shift: i32) {
    let mut stages = 8 - shift;
    let mut i = 0;
    loop {
        stages -= 1;
        if stages <= 0 {
            break;
        }
        for j in 0..(1usize << i) {
            let p = points >> i;
            butterfly_generic(&mut x[p * j..p * j + p], p, 4 << (i + shift));
        }
        i += 1;
    }
    for j in (0..points).step_by(32) {
        butterfly_32(&mut x[j..j + 32]);
    }
}

const BITREV: [i32; 16] = [0, 8, 4, 12, 2, 10, 6, 14, 1, 9, 5, 13, 3, 11, 7, 15];

fn bitrev12(x: i32) -> i32 {
    BITREV[(x >> 8) as usize & 15] | (BITREV[((x & 0x0f0) >> 4) as usize] << 4) | (BITREV[(x & 0x00f) as usize] << 8)
}

fn bitreverse(x: &mut [i32], n: usize, shift: i32) {
    let mut bit = 0;
    let mut w = (n >> 1) as isize;
    loop {
        let b = bitrev12(bit);
        bit += 1;
        let xx = (b >> shift) as isize;
        w -= 2;
        if w > xx {
            x.swap(xx as usize, w as usize);
            x.swap(xx as usize + 1, w as usize + 1);
        }
        if w <= 0 {
            break;
        }
    }
}

fn step7(x: &mut [i32], n: usize, step: usize) {
    let mut w0 = 0usize;
    let mut w1 = n >> 1;
    let (t, base): (&[i32], usize) = if step >= 4 { (&SINCOS0, step >> 1) } else { (&SINCOS1, 0) };
    let mut ti = base;
    let ttop = base + 1024;
    loop {
        w1 -= 2;
        let r0 = x[w0].wrapping_add(x[w1]);
        let r1 = x[w1 + 1].wrapping_sub(x[w0 + 1]);
        let r2 = mult32(r0, t[ti + 1]).wrapping_add(mult32(r1, t[ti]));
        let r3 = mult32(r1, t[ti + 1]).wrapping_sub(mult32(r0, t[ti]));
        ti += step;
        let r0 = x[w0 + 1].wrapping_add(x[w1 + 1]) >> 1;
        let r1 = x[w0].wrapping_sub(x[w1]) >> 1;
        x[w0] = r0.wrapping_add(r2);
        x[w0 + 1] = r1.wrapping_add(r3);
        x[w1] = r0.wrapping_sub(r2);
        x[w1 + 1] = r3.wrapping_sub(r1);
        w0 += 2;
        if ti >= ttop {
            break;
        }
    }
    loop {
        w1 -= 2;
        let r0 = x[w0].wrapping_add(x[w1]);
        let r1 = x[w1 + 1].wrapping_sub(x[w0 + 1]);
        ti -= step;
        let r2 = mult32(r0, t[ti]).wrapping_add(mult32(r1, t[ti + 1]));
        let r3 = mult32(r1, t[ti]).wrapping_sub(mult32(r0, t[ti + 1]));
        let r0 = x[w0 + 1].wrapping_add(x[w1 + 1]) >> 1;
        let r1 = x[w0].wrapping_sub(x[w1]) >> 1;
        x[w0] = r0.wrapping_add(r2);
        x[w0 + 1] = r1.wrapping_add(r3);
        x[w1] = r0.wrapping_sub(r2);
        x[w1 + 1] = r3.wrapping_sub(r1);
        w0 += 2;
        if w0 >= w1 {
            break;
        }
    }
}

fn step8(x: &mut [i32], n: usize, step: usize) {
    let ix = n >> 1;
    let step = step >> 2;
    let mut p = 0usize;
    match step {
        0 => {
            let (t, v) = (&SINCOS0, &SINCOS1);
            let (mut ti, mut vi) = (2usize, 0usize);
            let mut t0 = t[0];
            let mut t1 = t[1];
            loop {
                let mut v0 = v[vi];
                let mut v1 = v[vi + 1];
                vi += 2;
                let q0 = v0.wrapping_sub(t0) >> 2;
                t0 = t0.wrapping_add(q0);
                let q1 = v1.wrapping_sub(t1) >> 2;
                t1 = t1.wrapping_add(q1);
                (x[p], x[p + 1]) = xprod31(x[p], x[p + 1].wrapping_neg(), t0, t1);
                t0 = v0.wrapping_sub(q0);
                t1 = v1.wrapping_sub(q1);
                (x[p + 2], x[p + 3]) = xprod31(x[p + 2], x[p + 3].wrapping_neg(), t0, t1);
                t0 = t[ti];
                t1 = t[ti + 1];
                ti += 2;
                let q0 = t0.wrapping_sub(v0) >> 2;
                v0 = v0.wrapping_add(q0);
                let q1 = t1.wrapping_sub(v1) >> 2;
                v1 = v1.wrapping_add(q1);
                (x[p + 4], x[p + 5]) = xprod31(x[p + 4], x[p + 5].wrapping_neg(), v0, v1);
                v0 = t0.wrapping_sub(q0);
                v1 = t1.wrapping_sub(q1);
                // Tremor writes this pair one place early (x+5, x+6).
                (x[p + 5], x[p + 6]) = xprod31(x[p + 6], x[p + 7].wrapping_neg(), v0, v1);
                p += 8;
                if p >= ix {
                    break;
                }
            }
        }
        1 => {
            let (t, v) = (&SINCOS0, &SINCOS1);
            let (mut ti, mut vi) = (2usize, 0usize);
            let mut t0 = t[0] >> 1;
            let mut t1 = t[1] >> 1;
            loop {
                let r0 = x[p];
                let r1 = x[p + 1].wrapping_neg();
                let mut v0 = v[vi] >> 1;
                t0 = t0.wrapping_add(v0);
                let mut v1 = v[vi + 1] >> 1;
                t1 = t1.wrapping_add(v1);
                vi += 2;
                (x[p], x[p + 1]) = xprod31(r0, r1, t0, t1);
                let r0 = x[p + 2];
                let r1 = x[p + 3].wrapping_neg();
                t0 = t[ti] >> 1;
                v0 = v0.wrapping_add(t0);
                t1 = t[ti + 1] >> 1;
                v1 = v1.wrapping_add(t1);
                ti += 2;
                (x[p + 2], x[p + 3]) = xprod31(r0, r1, v0, v1);
                p += 4;
                if p >= ix {
                    break;
                }
            }
        }
        _ => {
            let (t, base): (&[i32], usize) = if step >= 4 { (&SINCOS0, step >> 1) } else { (&SINCOS1, 0) };
            let mut ti = base;
            loop {
                let r0 = x[p];
                let r1 = x[p + 1].wrapping_neg();
                (x[p], x[p + 1]) = xprod31(r0, r1, t[ti], t[ti + 1]);
                ti += step;
                p += 2;
                if p >= ix {
                    break;
                }
            }
        }
    }
}

fn mdct_backward(n: usize, x: &mut [i32]) {
    let mut shift = 4;
    while n & (1 << shift) == 0 {
        shift += 1;
    }
    let shift = 13 - shift;
    let step = 2usize << shift;
    presymmetry(x, (n >> 1) as isize, step as isize);
    butterflies(x, n >> 1, shift);
    bitreverse(x, n, shift);
    step7(x, n, step);
    step8(x, n, step);
}

fn mdct_shift_right(n: usize, x: &[i32], right: &mut [i32]) {
    for i in 0..n >> 2 {
        right[i] = x[1 + (i << 1)];
    }
}

fn window(left: i64) -> &'static [i32] {
    match left {
        32 => &VWIN64,
        64 => &VWIN128,
        128 => &VWIN256,
        256 => &VWIN512,
        512 => &VWIN1024,
        1024 => &VWIN2048,
        2048 => &VWIN4096,
        4096 => &VWIN8192,
        _ => &[],
    }
}

/// _mdct_unroll_lap: frames start..end of the overlap of the last block's
/// right half and this one's left, into every `step`th of `out`.
#[allow(clippy::too_many_arguments)]
fn unroll_lap(n0: i64, n1: i64, lw: bool, w: bool, inp: &[i32], right: &[i32], w0: &[i32], w1: &[i32], out: &mut [i16], mut o: usize, step: usize, mut start: i64, mut end: i64) {
    let mut l = if w && lw { n1 >> 1 } else { n0 >> 1 } as isize;
    let mut r = if lw { n1 >> 2 } else { n0 >> 2 } as isize;
    let win = if w && lw { w1 } else { w0 };
    let mut wr = if w && lw { n1 >> 1 } else { n0 >> 1 } as isize;
    let mut wl = 0isize;
    let pre_lap = if lw && !w { (n1 >> 2) - (n0 >> 2) } else { 0 };
    let half_lap = if lw && w { n1 >> 2 } else { n0 >> 2 };
    let post_lap = if !lw && w { (n1 >> 2) - (n0 >> 2) } else { 0 };
    let ws = |i: isize| win.get(i as usize).copied().unwrap_or(0);
    let rs = |i: isize| right.get(i as usize).copied().unwrap_or(0);
    let is = |i: isize| inp.get(i as usize).copied().unwrap_or(0);
    let mut put = |o: &mut usize, v: i16| {
        if let Some(x) = out.get_mut(*o) {
            *x = v;
        }
        *o += step;
    };
    if pre_lap != 0 {
        let n = end.min(pre_lap);
        let off = start.min(pre_lap);
        let post = r - n as isize;
        r -= off as isize;
        start -= off;
        end -= n;
        while r > post {
            r -= 1;
            put(&mut o, clip_to_15(rs(r) >> 9));
        }
    }
    let n = end.min(half_lap);
    let off = start.min(half_lap);
    let post = r - n as isize;
    r -= off as isize;
    l -= off as isize * 2;
    start -= off;
    wr -= off as isize;
    wl += off as isize;
    end -= n;
    while r > post {
        l -= 2;
        r -= 1;
        wr -= 1;
        let v = mult31(rs(r), ws(wr)).wrapping_add(mult31(is(l), ws(wl)));
        wl += 1;
        put(&mut o, clip_to_15(v >> 9));
    }
    let n = end.min(half_lap);
    let off = start.min(half_lap);
    let post = r + n as isize;
    r += off as isize;
    l += off as isize * 2;
    start -= off;
    end -= n;
    wr -= off as isize;
    wl += off as isize;
    while r < post {
        wr -= 1;
        let v = mult31(rs(r), ws(wr)).wrapping_sub(mult31(is(l), ws(wl)));
        r += 1;
        wl += 1;
        put(&mut o, clip_to_15(v >> 9));
        l += 2;
    }
    if post_lap != 0 {
        let n = end.min(post_lap);
        let off = start.min(post_lap);
        let post = l + n as isize * 2;
        l += off as isize * 2;
        while l < post {
            put(&mut o, clip_to_15(is(l).wrapping_neg() >> 9));
            l += 2;
        }
    }
    let _ = start;
}

// ---------------------------------------------------------------------
// The decoder state (dsp.c, mapping0.c).

struct Dsp {
    work: Vec<Vec<i32>>,
    mdctright: Vec<Vec<i32>>,
    out_begin: i64,
    out_end: i64,
    lw: usize,
    w: usize,
    granulepos: i64,
    sequence: i64,
    sample_count: i64,
}

impl Dsp {
    fn create(vi: &Info) -> Dsp {
        let ci = vi.ci();
        let ch = vi.channels.max(0) as usize;
        let mut d = Dsp {
            work: vec![vec![0; (ci.blocksizes[1] >> 1) as usize]; ch],
            mdctright: vec![vec![0; (ci.blocksizes[1] >> 2) as usize]; ch],
            out_begin: -1,
            out_end: -1,
            lw: 0,
            w: 0,
            granulepos: -1,
            sequence: -1,
            sample_count: -1,
        };
        d.restart();
        d
    }

    fn restart(&mut self) {
        self.out_end = -1;
        self.out_begin = -1;
        self.granulepos = -1;
        self.sequence = -1;
        self.sample_count = -1;
    }

    /// vorbis_dsp_pcmout: the samples ready (written interleaved into `pcm`).
    fn pcmout(&self, vi: &Info, pcm: Option<&mut [i16]>, samples: i64) -> i64 {
        let ci = vi.ci();
        if self.out_begin > -1 && self.out_begin < self.out_end {
            let mut n = self.out_end - self.out_begin;
            if let Some(pcm) = pcm {
                n = n.min(samples);
                let ch = vi.channels as usize;
                for i in 0..ch {
                    unroll_lap(
                        ci.blocksizes[0],
                        ci.blocksizes[1],
                        self.lw != 0,
                        self.w != 0,
                        &self.work[i],
                        &self.mdctright[i],
                        window(ci.blocksizes[0] >> 1),
                        window(ci.blocksizes[1] >> 1),
                        pcm,
                        i,
                        ch,
                        self.out_begin,
                        self.out_begin + n,
                    );
                }
            }
            return n;
        }
        0
    }

    fn read(&mut self, s: i64) -> i64 {
        if s != 0 && self.out_begin + s > self.out_end {
            return OV_EINVAL;
        }
        self.out_begin += s;
        0
    }

    /// vorbis_dsp_synthesis: a packet decoded (`decodep`) or only taken account of.
    fn synthesis(&mut self, vi: &Info, op: &Packet, decodep: bool) -> i64 {
        let ci = vi.ci();
        let mut opb = Pack::new(op.data.clone());
        if opb.read(1) != 0 {
            return OV_ENOTAUDIO;
        }
        let mode = opb.read(ilog1(ci.modes.len() as u32));
        if mode == -1 || mode as usize >= ci.modes.len() {
            return OV_EBADPACKET;
        }
        self.lw = self.w;
        self.w = ci.modes[mode as usize].0 as usize & 1;
        let lw = ci.blocksizes[self.lw] as usize;
        for i in 0..vi.channels as usize {
            mdct_shift_right(lw, &self.work[i], &mut self.mdctright[i]);
        }
        if self.w != 0 {
            opb.read(1);
            if opb.read(1) == -1 {
                return OV_EBADPACKET;
            }
        }
        if decodep {
            let map = &ci.maps[ci.modes[mode as usize].1 as usize];
            self.mapping_inverse(vi, map, &mut opb);
            if self.out_begin == -1 {
                self.out_begin = 0;
                self.out_end = 0;
            } else {
                self.out_begin = 0;
                self.out_end = ci.blocksizes[self.lw] / 4 + ci.blocksizes[self.w] / 4;
            }
        }
        if self.sequence == -1 || self.sequence + 1 != op.packetno - 3 {
            self.granulepos = -1;
            self.sample_count = -1;
        }
        self.sequence = op.packetno - 3;
        if self.sample_count == -1 {
            self.sample_count = 0;
        } else {
            self.sample_count += ci.blocksizes[self.lw] / 4 + ci.blocksizes[self.w] / 4;
        }
        if self.granulepos == -1 {
            if op.granulepos != -1 {
                self.granulepos = op.granulepos;
                if self.sample_count > self.granulepos {
                    if op.e_o_s != 0 {
                        self.out_end -= self.sample_count - self.granulepos;
                    } else {
                        self.out_begin += self.sample_count - self.granulepos;
                        if self.out_begin > self.out_end {
                            self.out_begin = self.out_end;
                        }
                    }
                }
            }
        } else {
            self.granulepos += ci.blocksizes[self.lw] / 4 + ci.blocksizes[self.w] / 4;
            if op.granulepos != -1 && self.granulepos != op.granulepos {
                if self.granulepos > op.granulepos {
                    let extra = self.granulepos - op.granulepos;
                    if extra != 0 && op.e_o_s != 0 {
                        self.out_end -= extra;
                    }
                }
                self.granulepos = op.granulepos;
            }
        }
        0
    }

    fn mapping_inverse(&mut self, vi: &Info, info: &Mapping, opb: &mut Pack) {
        let ci = vi.ci();
        let ch = vi.channels as usize;
        let n = ci.blocksizes[self.w] as usize;
        let submap_of = |i: usize| -> usize { info.chmuxlist.as_ref().map_or(0, |l| l[i] as usize) };
        let mut memo: Vec<Option<Vec<i32>>> = Vec::with_capacity(ch);
        let mut nonzero = vec![false; ch];
        for i in 0..ch {
            let floorno = info.submaplist[submap_of(i)].0 as usize;
            let m = match &ci.floors[floorno] {
                Floor::F1(f) => floor1_inverse1(f, ci, opb),
                Floor::F0(f) => floor0_inverse1(f, ci, opb),
            };
            nonzero[i] = m.is_some();
            memo.push(m);
            for x in self.work[i][..n / 2].iter_mut() {
                *x = 0;
            }
        }
        for &(mag, ang) in &info.coupling {
            if nonzero[mag as usize] || nonzero[ang as usize] {
                nonzero[mag as usize] = true;
                nonzero[ang as usize] = true;
            }
        }
        for (i, &(_, residue)) in info.submaplist.iter().enumerate() {
            let mut chans = Vec::new();
            let mut zero = Vec::new();
            for j in 0..ch {
                if info.chmuxlist.as_ref().is_none_or(|l| l[j] as usize == i) {
                    chans.push(j);
                    zero.push(nonzero[j]);
                }
            }
            res_inverse(ci, self.w, &ci.residues[residue as usize], &mut self.work, &chans, &zero, opb);
        }
        for &(mag, ang) in info.coupling.iter().rev() {
            let (m, a) = (mag as usize, ang as usize);
            for j in 0..n / 2 {
                let mg = self.work[m][j];
                let an = self.work[a][j];
                if mg > 0 {
                    if an > 0 {
                        self.work[m][j] = mg;
                        self.work[a][j] = mg.wrapping_sub(an);
                    } else {
                        self.work[a][j] = mg;
                        self.work[m][j] = mg.wrapping_add(an);
                    }
                } else if an > 0 {
                    self.work[m][j] = mg;
                    self.work[a][j] = mg.wrapping_add(an);
                } else {
                    self.work[a][j] = mg;
                    self.work[m][j] = mg.wrapping_sub(an);
                }
            }
        }
        for i in 0..ch {
            let floorno = info.submaplist[submap_of(i)].0 as usize;
            match &ci.floors[floorno] {
                Floor::F1(f) => floor1_inverse2(f, (n / 2) as i32, memo[i].as_deref(), &mut self.work[i]),
                Floor::F0(f) => match &memo[i] {
                    Some(lsp) => lsp_to_curve(&mut self.work[i], (n / 2) as i32, f.barkmap as i32, lsp, f.order, lsp[f.order as usize], f.ampdb, (f.rate >> 1) as i32),
                    None => {
                        for x in self.work[i][..n / 2].iter_mut() {
                            *x = 0;
                        }
                    }
                },
            }
        }
        for i in 0..ch {
            mdct_backward(n, &mut self.work[i]);
        }
    }
}

/// vorbis_packet_blocksize.
fn packet_blocksize(vi: &Info, op: &Packet) -> i64 {
    let ci = vi.ci();
    let mut opb = Pack::new(op.data.clone());
    if opb.read(1) != 0 {
        return OV_ENOTAUDIO;
    }
    let mut modebits = 0;
    let mut v = ci.modes.len();
    while v > 1 {
        modebits += 1;
        v >>= 1;
    }
    let mode = opb.read(modebits);
    if mode == -1 || mode as usize >= ci.modes.len() {
        return OV_EBADPACKET;
    }
    ci.blocksizes[ci.modes[mode as usize].0 as usize & 1]
}

// ---------------------------------------------------------------------
// Ogg framing (framing.c), each queue one run of bytes.

static CRC_LOOKUP: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();

fn crc_table() -> &'static [u32; 256] {
    CRC_LOOKUP.get_or_init(|| {
        let mut t = [0u32; 256];
        for (i, x) in t.iter_mut().enumerate() {
            let mut r = (i as u32) << 24;
            for _ in 0..8 {
                r = if r & 0x8000_0000 != 0 { (r << 1) ^ 0x04c11db7 } else { r << 1 };
            }
            *x = r;
        }
        t
    })
}

#[derive(Default, Clone)]
struct Page {
    header: Vec<u8>,
    body: Vec<u8>,
}

impl Page {
    fn byte(&self, i: usize) -> u8 {
        self.header.get(i).copied().unwrap_or(0)
    }
    fn version(&self) -> i32 {
        if self.header.is_empty() {
            -1
        } else {
            self.byte(4) as i32
        }
    }
    fn continued(&self) -> bool {
        self.byte(5) & 1 != 0
    }
    fn bos(&self) -> i32 {
        (self.byte(5) & 2) as i32
    }
    fn eos(&self) -> i32 {
        (self.byte(5) & 4) as i32
    }
    fn granulepos(&self) -> i64 {
        if self.header.len() < 14 {
            return -1;
        }
        i64::from_le_bytes(self.header[6..14].try_into().unwrap())
    }
    fn serialno(&self) -> u32 {
        if self.header.len() < 18 {
            return 0xffff_ffff;
        }
        u32::from_le_bytes(self.header[14..18].try_into().unwrap())
    }
    fn pageno(&self) -> u32 {
        if self.header.len() < 22 {
            return 0xffff_ffff;
        }
        u32::from_le_bytes(self.header[18..22].try_into().unwrap())
    }
}

#[derive(Default, Clone)]
pub(crate) struct Packet {
    data: Vec<u8>,
    b_o_s: i32,
    e_o_s: i32,
    granulepos: i64,
    packetno: i64,
}

#[derive(Default)]
struct Sync {
    fifo: Vec<u8>,
    unsynced: bool,
    headerbytes: usize,
    bodybytes: usize,
}

impl Sync {
    fn reset(&mut self) {
        self.fifo.clear();
        self.unsynced = false;
        self.headerbytes = 0;
        self.bodybytes = 0;
    }

    /// ogg_sync_pageseek: a page's length (taken into `og`), 0 for more
    /// data, minus the bytes passed over to the next capture pattern.
    fn pageseek(&mut self, og: &mut Page) -> i64 {
        *og = Page::default();
        let bytes = self.fifo.len();
        if self.headerbytes == 0 {
            if bytes < 27 {
                return 0;
            }
            if &self.fifo[..4] != b"OggS" {
                return self.fail();
            }
            self.headerbytes = self.fifo[26] as usize + 27;
        }
        if bytes < self.headerbytes {
            return 0;
        }
        if self.bodybytes == 0 {
            self.bodybytes = self.fifo[27..self.headerbytes].iter().map(|&b| b as usize).sum();
        }
        if self.bodybytes + self.headerbytes > bytes {
            return 0;
        }
        let total = self.headerbytes + self.bodybytes;
        let chksum = u32::from_le_bytes(self.fifo[22..26].try_into().unwrap());
        let t = crc_table();
        let mut crc = 0u32;
        for (i, &b) in self.fifo[..total].iter().enumerate() {
            let b = if (22..26).contains(&i) { 0 } else { b };
            crc = (crc << 8) ^ t[(((crc >> 24) & 0xff) as u8 ^ b) as usize];
        }
        if crc != chksum {
            return self.fail();
        }
        og.header = self.fifo[..self.headerbytes].to_vec();
        og.body = self.fifo[self.headerbytes..total].to_vec();
        self.fifo.drain(..total);
        self.unsynced = false;
        self.headerbytes = 0;
        self.bodybytes = 0;
        total as i64
    }

    fn fail(&mut self) -> i64 {
        self.headerbytes = 0;
        self.bodybytes = 0;
        self.fifo.drain(..1.min(self.fifo.len()));
        let mut ret = -1i64;
        match self.fifo.iter().position(|&b| b == b'O') {
            Some(k) => {
                self.fifo.drain(..k);
                ret -= k as i64;
            }
            None => {
                ret -= self.fifo.len() as i64;
                self.fifo.clear();
            }
        }
        ret
    }
}

const FINFLAG: u32 = 0x8000_0000;
const FINMASK: u32 = 0x7fff_ffff;

#[derive(Default)]
struct Stream {
    header: Vec<u8>,
    body: Vec<u8>,
    e_o_s: i32,
    b_o_s: i32,
    serialno: i32,
    pageno: i64,
    packetno: i64,
    granulepos: i64,
    lacing_fill: i32,
    body_fill: u32,
    holeflag: i32,
    spanflag: i32,
    clearflag: bool,
    laceptr: i32,
    body_fill_next: u32,
}

impl Stream {
    fn create(serialno: i32) -> Stream {
        Stream { serialno, pageno: -1, ..Stream::default() }
    }

    fn reset_serialno(&mut self, serialno: i32) {
        *self = Stream::create(serialno);
        self.pageno = -1;
    }

    fn hbyte(&self, i: usize) -> u8 {
        self.header.get(i).copied().unwrap_or(0)
    }

    fn next_lace(&mut self) {
        self.body_fill_next = 0;
        while self.laceptr < self.lacing_fill {
            let val = self.hbyte(27 + self.laceptr as usize) as u32;
            self.laceptr += 1;
            self.body_fill_next += val;
            if val < 255 {
                self.body_fill_next |= FINFLAG;
                self.clearflag = true;
                break;
            }
        }
    }

    fn pretruncate_body(&mut self, n: u32) {
        let n = (n as usize).min(self.body.len());
        self.body.drain(..n);
    }

    fn span_queued_page(&mut self) {
        while self.body_fill & FINFLAG == 0 {
            if self.header.is_empty() {
                break;
            }
            if self.lacing_fill >= 0 {
                let n = ((self.lacing_fill + 27) as usize).min(self.header.len());
                self.header.drain(..n);
            }
            self.lacing_fill = 0;
            self.laceptr = 0;
            self.clearflag = false;
            if self.header.is_empty() {
                break;
            }
            let og = Page { header: self.header[..27.min(self.header.len())].to_vec(), body: Vec::new() };
            let pageno = og.pageno() as i64;
            self.lacing_fill = self.hbyte(26) as i32;
            if pageno != self.pageno {
                self.holeflag = if self.pageno == -1 { 1 } else { 2 };
                let f = self.body_fill;
                self.pretruncate_body(f);
                self.body_fill = 0;
            }
            if og.continued() {
                if self.body_fill == 0 {
                    self.next_lace();
                    let f = self.body_fill_next & FINMASK;
                    self.pretruncate_body(f);
                    if self.spanflag == 0 && self.holeflag == 0 {
                        self.spanflag = 2;
                    }
                }
            } else if self.body_fill > 0 {
                let f = self.body_fill;
                self.pretruncate_body(f);
                self.body_fill = 0;
                if self.spanflag == 0 && self.holeflag == 0 {
                    self.spanflag = 2;
                }
            }
            if self.laceptr < self.lacing_fill {
                self.granulepos = og.granulepos();
                self.next_lace();
                self.body_fill = self.body_fill.wrapping_add(self.body_fill_next);
                self.next_lace();
            }
            self.pageno = pageno + 1;
            self.e_o_s = og.eos();
            self.b_o_s = og.bos();
        }
    }

    /// ogg_stream_pagein: the page queued (and taken).
    fn pagein(&mut self, og: &mut Page) -> i32 {
        let og = std::mem::take(og);
        if og.serialno() as i32 != self.serialno {
            return -13;
        }
        if og.version() > 0 {
            return -12;
        }
        self.body.extend_from_slice(&og.body);
        if self.header.is_empty() {
            self.lacing_fill = -27;
        }
        self.header.extend_from_slice(&og.header);
        0
    }

    fn packetout_inner(&mut self, op: Option<&mut Packet>, adv: bool) -> i32 {
        self.span_queued_page();
        if self.holeflag != 0 {
            let temp = self.holeflag;
            self.holeflag = if self.clearflag { 0 } else { 1 };
            if temp == 2 {
                self.packetno += 1;
                return -10;
            }
        }
        if self.spanflag != 0 {
            let temp = self.spanflag;
            self.spanflag = if self.clearflag { 0 } else { 1 };
            if temp == 2 {
                self.packetno += 1;
                return -11;
            }
        }
        if self.body_fill & FINFLAG == 0 {
            return 0;
        }
        let has_op = op.is_some();
        if !has_op && !adv {
            return 1;
        }
        let len = ((self.body_fill & FINMASK) as usize).min(self.body.len());
        if let Some(op) = op {
            op.b_o_s = self.b_o_s;
            op.e_o_s = if self.e_o_s != 0 && self.body_fill_next == 0 { self.e_o_s } else { 0 };
            op.granulepos = if self.body_fill & FINFLAG != 0 && self.body_fill_next & FINFLAG == 0 { self.granulepos } else { -1 };
            op.packetno = self.packetno;
            op.data = self.body[..len].to_vec();
        }
        if adv {
            self.body.drain(..len);
            self.body_fill = self.body_fill_next;
            self.next_lace();
            self.packetno += 1;
            self.b_o_s = 0;
        }
        1
    }

    fn packetout(&mut self, op: Option<&mut Packet>) -> i32 {
        if let Some(op) = op {
            *op = Packet::default();
            return self.packetout_inner(Some(op), true);
        }
        self.packetout_inner(None, true)
    }

    fn packetpeek(&mut self, op: &mut Packet) -> i32 {
        *op = Packet::default();
        self.packetout_inner(Some(op), false)
    }
}

// ---------------------------------------------------------------------
// vorbisfile.c over a file in memory (the Vorbis player's callbacks).

const NOTOPEN: i32 = 0;
const PARTOPEN: i32 = 1;
const OPENED: i32 = 2;
const STREAMSET: i32 = 3;
const LINKSET: i32 = 4;
const INITSET: i32 = 5;

/// An Ogg Vorbis file being played (OggVorbis_File).
pub struct File {
    src: Arc<Vec<u8>>,
    src_offset: i64,
    has_source: bool,
    seekable: bool,
    offset: i64,
    end: i64,
    oy: Sync,
    links: i32,
    offsets: Vec<i64>,
    dataoffsets: Vec<i64>,
    serialnos: Vec<u32>,
    pcmlengths: Vec<i64>,
    pub vi: Info,
    vendor: bool,
    pcm_offset: i64,
    ready_state: i32,
    current_serialno: u32,
    current_link: i32,
    os: Stream,
    vd: Option<Dsp>,
}

impl File {
    fn src_read(&mut self, n: i64) -> Vec<u8> {
        let size = self.src.len() as i64;
        if self.src_offset >= size {
            return Vec::new();
        }
        let n = n.min(size - self.src_offset).max(0);
        let at = self.src_offset as usize;
        self.src_offset += n;
        self.src[at..at + n as usize].to_vec()
    }

    fn src_seek(&mut self, offset: i64, whence: i32) -> i32 {
        match whence {
            0 => self.src_offset = offset,
            1 => self.src_offset += offset,
            _ => self.src_offset = self.src.len() as i64 + offset,
        }
        if self.src_offset >= self.src.len() as i64 {
            -1
        } else {
            0
        }
    }

    fn get_data(&mut self) -> i64 {
        if !self.has_source {
            return 0;
        }
        let d = self.src_read(CHUNKSIZE);
        self.oy.fifo.extend_from_slice(&d);
        d.len() as i64
    }

    fn seek_helper(&mut self, offset: i64) {
        if self.has_source {
            self.src_seek(offset, 0);
            self.offset = offset;
            self.oy.reset();
        }
    }

    fn get_next_page(&mut self, og: &mut Page, boundary: i64) -> i64 {
        let boundary = if boundary > 0 { boundary + self.offset } else { boundary };
        loop {
            if boundary > 0 && self.offset >= boundary {
                return OV_FALSE;
            }
            let more = self.oy.pageseek(og);
            if more < 0 {
                self.offset -= more;
            } else if more == 0 {
                if boundary == 0 {
                    return OV_FALSE;
                }
                let ret = self.get_data();
                if ret == 0 {
                    return OV_EOF;
                }
                if ret < 0 {
                    return OV_EREAD;
                }
            } else {
                let ret = self.offset;
                self.offset += more;
                return ret;
            }
        }
    }

    fn get_prev_page(&mut self, og: &mut Page) -> i64 {
        let mut begin = self.offset;
        let end = begin;
        let mut offset = -1i64;
        while offset == -1 {
            begin -= CHUNKSIZE;
            if begin < 0 {
                begin = 0;
            }
            self.seek_helper(begin);
            while self.offset < end {
                let ret = self.get_next_page(og, end - self.offset);
                if ret == OV_EREAD {
                    return OV_EREAD;
                }
                if ret < 0 {
                    break;
                }
                offset = ret;
            }
            if begin == 0 && offset == -1 {
                // Nothing before: C would search the start forever.
                return OV_EFAULT;
            }
        }
        self.seek_helper(offset);
        let ret = self.get_next_page(og, CHUNKSIZE);
        if ret < 0 {
            return OV_EFAULT;
        }
        offset
    }

    fn bisect_forward_serialno(&mut self, begin: i64, mut searched: i64, end: i64, currentno: u32, m: i32) -> i64 {
        let mut endsearched = end;
        let mut next = end;
        let mut og = Page::default();
        while searched < endsearched {
            let bisect = if endsearched - searched < CHUNKSIZE { searched } else { (searched + endsearched) / 2 };
            self.seek_helper(bisect);
            let ret = self.get_next_page(&mut og, -1);
            if ret == OV_EREAD {
                return OV_EREAD;
            }
            if ret < 0 || og.serialno() != currentno {
                endsearched = bisect;
                if ret >= 0 {
                    next = ret;
                }
            } else {
                searched = ret + og.header.len() as i64 + og.body.len() as i64;
            }
        }
        self.seek_helper(next);
        let ret = self.get_next_page(&mut og, -1);
        if ret == OV_EREAD {
            return OV_EREAD;
        }
        if searched >= end || ret < 0 {
            self.links = m + 1;
            self.offsets = vec![0; self.links as usize + 1];
            self.serialnos = vec![0; self.links as usize];
            self.offsets[m as usize + 1] = searched;
        } else {
            let off = self.offset;
            let ret = self.bisect_forward_serialno(next, off, end, og.serialno(), m + 1);
            if ret == OV_EREAD {
                return OV_EREAD;
            }
        }
        self.offsets[m as usize] = begin;
        self.serialnos[m as usize] = currentno;
        0
    }

    fn decode_clear(&mut self) {
        if self.ready_state == INITSET {
            self.vd = None;
            self.ready_state = STREAMSET;
        }
        if self.ready_state >= STREAMSET {
            self.vi = Info::default();
            self.vendor = false;
            self.ready_state = OPENED;
        }
    }

    /// _fetch_headers into vi; `set_serial` writes the stream's serial to
    /// current_serialno; `og` the page to start from (taken).
    fn fetch_headers(&mut self, set_serial: bool, og_in: Option<&mut Page>) -> i64 {
        let mut local = Page::default();
        if self.ready_state > OPENED {
            self.decode_clear();
        }
        let og: &mut Page = match og_in {
            Some(p) => p,
            None => {
                let r = self.get_next_page(&mut local, CHUNKSIZE);
                if r == OV_EREAD {
                    return OV_EREAD;
                }
                if r < 0 {
                    return OV_ENOTVORBIS;
                }
                &mut local
            }
        };
        self.os.reset_serialno(og.serialno() as i32);
        if set_serial {
            self.current_serialno = self.os.serialno as u32;
        }
        self.vi = Info::init();
        self.vendor = false;
        let mut i = 0;
        let mut op = Packet::default();
        let fail = |s: &mut File, ret: i64| {
            s.vi = Info::default();
            s.vendor = false;
            s.ready_state = OPENED;
            ret
        };
        while i < 3 {
            self.os.pagein(og);
            while i < 3 {
                let result = self.os.packetout(Some(&mut op));
                if result == 0 {
                    break;
                }
                if result == -1 || result < 0 {
                    return fail(self, OV_EBADHEADER);
                }
                let mut vendor = self.vendor;
                let ret = headerin(&mut self.vi, &mut vendor, &op);
                self.vendor = vendor;
                if ret != 0 {
                    return fail(self, ret);
                }
                i += 1;
            }
            if i < 3 && self.get_next_page(og, CHUNKSIZE) < 0 {
                return fail(self, OV_EBADHEADER);
            }
        }
        self.ready_state = LINKSET;
        0
    }

    fn set_link_number(&mut self, link: i32) -> i64 {
        if link != self.current_link {
            self.decode_clear();
        }
        if self.ready_state < STREAMSET {
            let off = self.offsets[link as usize];
            self.seek_helper(off);
            self.os.reset_serialno(self.serialnos[link as usize] as i32);
            self.current_serialno = self.serialnos[link as usize];
            self.current_link = link;
            return self.fetch_headers(true, None);
        }
        0
    }

    fn set_link_number_preserve_pos(&mut self, link: i32) -> i64 {
        let pos = self.offset;
        let ret = self.set_link_number(link);
        if ret != 0 {
            return ret;
        }
        self.seek_helper(pos);
        if pos < self.offsets[link as usize] || pos >= self.offsets[link as usize + 1] {
            self.ready_state = STREAMSET;
        }
        0
    }

    fn prefetch_all_offsets(&mut self, dataoffset: i64) {
        let mut og = Page::default();
        let links = self.links as usize;
        self.dataoffsets = vec![0; links];
        self.pcmlengths = vec![0; links * 2];
        for i in 0..links {
            if i == 0 {
                self.dataoffsets[i] = dataoffset;
                self.seek_helper(dataoffset);
            } else {
                let off = self.offsets[i];
                self.seek_helper(off);
                if self.fetch_headers(false, None) < 0 {
                    self.dataoffsets[i] = -1;
                } else {
                    self.dataoffsets[i] = self.offset;
                }
            }
            if self.dataoffsets[i] != -1 {
                let mut accumulated = 0i64;
                let mut lastblock = -1i64;
                self.os.reset_serialno(self.serialnos[i] as i32);
                loop {
                    let mut op = Packet::default();
                    let ret = self.get_next_page(&mut og, -1);
                    if ret < 0 {
                        break;
                    }
                    if og.serialno() != self.serialnos[i] {
                        break;
                    }
                    let pos = og.granulepos();
                    self.os.pagein(&mut og);
                    loop {
                        let result = self.os.packetout(Some(&mut op));
                        if result == 0 {
                            break;
                        }
                        if result > 0 && self.vi.setup.is_some() {
                            let thisblock = packet_blocksize(&self.vi, &op);
                            if lastblock != -1 {
                                accumulated += (lastblock + thisblock) >> 2;
                            }
                            lastblock = thisblock;
                        }
                    }
                    if pos != -1 {
                        accumulated = pos - accumulated;
                        break;
                    }
                }
                if accumulated < 0 {
                    accumulated = 0;
                }
                self.pcmlengths[i * 2] = accumulated;
            }
            let end = self.offsets[i + 1];
            self.seek_helper(end);
            loop {
                let ret = self.get_prev_page(&mut og);
                if ret < 0 {
                    self.vi = Info::default();
                    self.vendor = false;
                    break;
                }
                if og.granulepos() != -1 {
                    self.pcmlengths[i * 2 + 1] = og.granulepos() - self.pcmlengths[i * 2];
                    break;
                }
                self.offset = ret;
            }
        }
    }

    fn make_decode_ready(&mut self) -> i64 {
        if self.ready_state == OPENED || self.ready_state == STREAMSET {
            let mut i = 0;
            while i < self.links {
                if self.offsets[i as usize + 1] >= self.offset {
                    break;
                }
                i += 1;
            }
            if i == self.links {
                return -1;
            }
            let r = self.set_link_number_preserve_pos(i);
            if r != 0 {
                return r;
            }
        }
        if self.ready_state == OPENED || self.ready_state == STREAMSET || self.ready_state == LINKSET {
            if self.vi.setup.is_none() {
                return -1;
            }
            self.vd = Some(Dsp::create(&self.vi));
            self.ready_state = INITSET;
        }
        if self.ready_state == INITSET {
            0
        } else {
            -1
        }
    }

    fn open_seekable2(&mut self) -> i64 {
        let serialno = self.current_serialno;
        let dataoffset = self.offset;
        let mut og = Page::default();
        self.src_seek(0, 2);
        self.offset = self.src_offset;
        self.end = self.src_offset;
        let end = self.get_prev_page(&mut og);
        if end < 0 {
            return end;
        }
        let tempserialno = og.serialno();
        if tempserialno != serialno {
            if self.bisect_forward_serialno(0, 0, end + 1, serialno, 0) < 0 {
                return OV_EREAD;
            }
        } else if self.bisect_forward_serialno(0, end, end + 1, serialno, 0) != 0 {
            return OV_EREAD;
        }
        self.prefetch_all_offsets(dataoffset);
        self.raw_seek(0)
    }

    fn fetch_and_process_packet(&mut self, readp: bool, spanp: bool) -> i64 {
        let mut og = Page::default();
        loop {
            if self.ready_state == INITSET {
                loop {
                    let mut op = Packet::default();
                    let result = self.os.packetout(Some(&mut op));
                    if result < 0 {
                        return OV_HOLE;
                    }
                    if result > 0 {
                        let mut granulepos = op.granulepos;
                        let vd = self.vd.as_mut().unwrap();
                        if vd.synthesis(&self.vi, &op, true) == 0 {
                            if granulepos != -1 && op.e_o_s == 0 {
                                let link = if self.seekable { self.current_link } else { 0 };
                                if self.seekable && link > 0 {
                                    granulepos -= self.pcmlengths[link as usize * 2];
                                }
                                if granulepos < 0 {
                                    granulepos = 0;
                                }
                                let samples = self.vd.as_ref().unwrap().pcmout(&self.vi, None, 0);
                                granulepos -= samples;
                                for i in 0..link as usize {
                                    granulepos += self.pcmlengths[i * 2 + 1];
                                }
                                self.pcm_offset = granulepos;
                            }
                            return 1;
                        }
                    } else {
                        break;
                    }
                }
            }
            if self.ready_state >= OPENED {
                if !readp {
                    return 0;
                }
                if self.get_next_page(&mut og, -1) < 0 {
                    return OV_EOF;
                }
                if self.ready_state == INITSET && self.current_serialno != og.serialno() {
                    if !spanp {
                        return OV_EOF;
                    }
                    self.decode_clear();
                }
            }
            if self.ready_state != INITSET {
                if self.ready_state < STREAMSET {
                    if self.seekable {
                        self.current_serialno = og.serialno();
                        let Some(link) = self.serialnos.iter().position(|&s| s == self.current_serialno) else { return OV_EBADLINK };
                        self.current_link = link as i32;
                        let ret = self.fetch_headers(true, Some(&mut og));
                        if ret != 0 {
                            return ret;
                        }
                    } else {
                        let ret = self.fetch_headers(true, Some(&mut og));
                        if ret != 0 {
                            return ret;
                        }
                        self.current_link += 1;
                    }
                }
                if self.make_decode_ready() != 0 {
                    return OV_EBADLINK;
                }
            }
            self.os.pagein(&mut og);
        }
    }

    /// ov_open_callbacks on the file's bytes.
    pub fn open(src: Arc<Vec<u8>>) -> Result<File, i64> {
        let mut vf = File {
            src,
            src_offset: 0,
            has_source: true,
            seekable: false,
            offset: 0,
            end: 0,
            oy: Sync::default(),
            links: 1,
            offsets: Vec::new(),
            dataoffsets: Vec::new(),
            serialnos: Vec::new(),
            pcmlengths: Vec::new(),
            vi: Info::default(),
            vendor: false,
            pcm_offset: 0,
            ready_state: NOTOPEN,
            current_serialno: 0,
            current_link: 0,
            os: Stream::create(-1),
            vd: None,
        };
        let offsettest = vf.src_seek(0, 1);
        if offsettest != -1 {
            vf.seekable = true;
        }
        let ret = vf.fetch_headers(true, None);
        if ret < 0 {
            return Err(ret);
        }
        if vf.ready_state < PARTOPEN {
            vf.ready_state = PARTOPEN;
        }
        if vf.ready_state < OPENED {
            vf.ready_state = OPENED;
        }
        if vf.seekable {
            let ret = vf.open_seekable2();
            if ret != 0 {
                return Err(ret);
            }
        }
        Ok(vf)
    }

    pub fn pcm_total(&self, i: i32) -> i64 {
        if self.ready_state < OPENED || !self.seekable || i >= self.links {
            return OV_EINVAL;
        }
        if i < 0 {
            return (0..self.links).map(|k| self.pcm_total(k)).sum();
        }
        self.pcmlengths[i as usize * 2 + 1]
    }

    fn time_total(&self, i: i32) -> i64 {
        if self.ready_state < OPENED || !self.seekable || i >= self.links {
            return OV_EINVAL;
        }
        if i < 0 {
            return (0..self.links).map(|k| self.time_total(k)).sum();
        }
        if self.vi.rate == 0 {
            return 0;
        }
        self.pcmlengths[i as usize * 2 + 1] * 1000 / self.vi.rate
    }

    fn raw_seek(&mut self, pos: i64) -> i64 {
        let mut og = Page::default();
        let mut op = Packet::default();
        if self.ready_state < OPENED {
            return OV_EINVAL;
        }
        if !self.seekable {
            return OV_ENOSEEK;
        }
        if pos < 0 || pos > self.end {
            return OV_EINVAL;
        }
        self.pcm_offset = -1;
        let cs = self.current_serialno as i32;
        self.os.reset_serialno(cs);
        if let Some(vd) = self.vd.as_mut() {
            vd.restart();
        }
        self.seek_helper(pos);
        let mut lastblock = 0i64;
        let mut accblock = 0i64;
        let mut eosflag = 0;
        let mut work_os = Stream::create(cs);
        loop {
            if self.ready_state >= STREAMSET {
                let result = work_os.packetout(Some(&mut op));
                if result > 0 {
                    if self.vi.setup.is_some() {
                        let mut thisblock = packet_blocksize(&self.vi, &op);
                        if thisblock < 0 {
                            self.os.packetout(None);
                            thisblock = 0;
                        } else if eosflag != 0 {
                            self.os.packetout(None);
                        } else if lastblock != 0 {
                            accblock += (lastblock + thisblock) >> 2;
                        }
                        if op.granulepos != -1 {
                            let link = self.current_link as usize;
                            let mut granulepos = op.granulepos - self.pcmlengths[link * 2];
                            if granulepos < 0 {
                                granulepos = 0;
                            }
                            for i in 0..link {
                                granulepos += self.pcmlengths[i * 2 + 1];
                            }
                            self.pcm_offset = granulepos - accblock;
                            break;
                        }
                        lastblock = thisblock;
                        continue;
                    } else {
                        self.os.packetout(None);
                    }
                }
            }
            if lastblock == 0 {
                if self.get_next_page(&mut og, -1) < 0 {
                    self.pcm_offset = self.pcm_total(-1);
                    break;
                }
            } else {
                self.pcm_offset = -1;
                break;
            }
            if self.ready_state >= STREAMSET && self.current_serialno != og.serialno() {
                self.decode_clear();
                work_os = Stream::create(cs);
            }
            if self.ready_state < STREAMSET {
                self.current_serialno = og.serialno();
                let Some(link) = self.serialnos.iter().position(|&s| s == self.current_serialno) else {
                    self.pcm_offset = -1;
                    self.decode_clear();
                    return OV_EBADLINK;
                };
                if self.set_link_number_preserve_pos(link as i32) != 0 {
                    self.pcm_offset = -1;
                    self.decode_clear();
                    return OV_EBADLINK;
                }
                let cs = self.current_serialno as i32;
                self.os.reset_serialno(cs);
                work_os.reset_serialno(cs);
            }
            let mut dup = og.clone();
            eosflag = og.eos();
            self.os.pagein(&mut og);
            work_os.pagein(&mut dup);
        }
        0
    }

    fn pcm_seek_page(&mut self, pos: i64) -> i64 {
        let mut total = self.pcm_total(-1);
        let mut og = Page::default();
        let mut op = Packet::default();
        if self.ready_state < OPENED {
            return OV_EINVAL;
        }
        if !self.seekable {
            return OV_ENOSEEK;
        }
        if pos < 0 || pos > total {
            return OV_EINVAL;
        }
        let mut link = self.links - 1;
        while link >= 0 {
            total -= self.pcmlengths[link as usize * 2 + 1];
            if pos >= total {
                break;
            }
            link -= 1;
        }
        if link < 0 {
            return OV_EINVAL;
        }
        let fail = |s: &mut File, r: i64| {
            s.pcm_offset = -1;
            s.decode_clear();
            r
        };
        if link != self.current_link {
            let ret = self.set_link_number(link);
            if ret != 0 {
                return fail(self, ret);
            }
        } else if let Some(vd) = self.vd.as_mut() {
            vd.restart();
        }
        self.os.reset_serialno(self.serialnos[link as usize] as i32);
        let lu = link as usize;
        let mut end = self.offsets[lu + 1];
        let mut begin = self.offsets[lu];
        let mut begintime = self.pcmlengths[lu * 2];
        let mut endtime = self.pcmlengths[lu * 2 + 1] + begintime;
        let target = pos - total + begintime;
        let mut best = begin;
        let mut result: i64;
        while begin < end {
            let mut bisect = if end - begin < CHUNKSIZE {
                begin
            } else {
                let b = begin + (target - begintime) * (end - begin) / (endtime - begintime).max(1) - CHUNKSIZE;
                if b <= begin {
                    begin + 1
                } else {
                    b
                }
            };
            self.seek_helper(bisect);
            while begin < end {
                let lim = end - self.offset;
                result = self.get_next_page(&mut og, lim);
                if result == OV_EREAD {
                    return fail(self, result);
                }
                if result < 0 {
                    if bisect <= begin + 1 {
                        end = begin;
                    } else {
                        if bisect == 0 {
                            return fail(self, result);
                        }
                        bisect -= CHUNKSIZE;
                        if bisect <= begin {
                            bisect = begin + 1;
                        }
                        self.seek_helper(bisect);
                    }
                } else {
                    let granulepos = og.granulepos();
                    if granulepos == -1 {
                        continue;
                    }
                    if granulepos < target {
                        best = result;
                        begin = self.offset;
                        begintime = granulepos;
                        if target - begintime > 44100 {
                            break;
                        }
                        bisect = begin;
                    } else if bisect <= begin + 1 {
                        end = begin;
                    } else if end == self.offset {
                        end = result;
                        bisect -= CHUNKSIZE;
                        if bisect <= begin {
                            bisect = begin + 1;
                        }
                        self.seek_helper(bisect);
                    } else {
                        end = result;
                        endtime = granulepos;
                        break;
                    }
                }
            }
        }
        self.seek_helper(best);
        self.pcm_offset = -1;
        if self.get_next_page(&mut og, -1) < 0 {
            return OV_EOF;
        }
        self.os.pagein(&mut og);
        loop {
            result = self.os.packetpeek(&mut op) as i64;
            if result == 0 {
                self.seek_helper(best);
                loop {
                    result = self.get_prev_page(&mut og);
                    if result < 0 {
                        return fail(self, result);
                    }
                    if og.granulepos() > -1 || !og.continued() {
                        return self.raw_seek(result);
                    }
                    self.offset = result;
                }
            }
            if result < 0 {
                return fail(self, OV_EBADPACKET);
            }
            if op.granulepos != -1 {
                self.pcm_offset = op.granulepos - self.pcmlengths[self.current_link as usize * 2];
                if self.pcm_offset < 0 {
                    self.pcm_offset = 0;
                }
                self.pcm_offset += total;
                break;
            }
            self.os.packetout(None);
        }
        if self.pcm_offset > pos || pos > self.pcm_total(-1) {
            return fail(self, OV_EFAULT);
        }
        0
    }

    /// ov_pcm_seek: to sample `pos`.
    pub fn pcm_seek(&mut self, pos: i64) -> i64 {
        let mut op = Packet::default();
        let mut og = Page::default();
        let mut lastblock = 0i64;
        let ret = self.pcm_seek_page(pos);
        if ret < 0 {
            return ret;
        }
        if self.make_decode_ready() != 0 {
            return OV_EBADLINK;
        }
        loop {
            let ret = self.os.packetpeek(&mut op) as i64;
            if ret > 0 {
                let thisblock = packet_blocksize(&self.vi, &op);
                if thisblock < 0 {
                    self.os.packetout(None);
                    continue;
                }
                if lastblock != 0 {
                    self.pcm_offset += (lastblock + thisblock) >> 2;
                }
                if self.pcm_offset + ((thisblock + self.vi.blocksize(1)) >> 2) >= pos {
                    break;
                }
                self.os.packetout(None);
                let vd = self.vd.as_mut().unwrap();
                vd.synthesis(&self.vi, &op, false);
                if op.granulepos > -1 {
                    self.pcm_offset = op.granulepos - self.pcmlengths[self.current_link as usize * 2];
                    if self.pcm_offset < 0 {
                        self.pcm_offset = 0;
                    }
                    for i in 0..self.current_link as usize {
                        self.pcm_offset += self.pcmlengths[i * 2 + 1];
                    }
                }
                lastblock = thisblock;
            } else {
                if ret < 0 && ret != OV_HOLE {
                    break;
                }
                if self.get_next_page(&mut og, -1) < 0 {
                    break;
                }
                if self.current_serialno != og.serialno() {
                    self.decode_clear();
                }
                if self.ready_state < STREAMSET {
                    self.current_serialno = og.serialno();
                    let Some(link) = self.serialnos.iter().position(|&s| s == self.current_serialno) else { return OV_EBADLINK };
                    self.current_link = link as i32;
                    let ret = self.fetch_headers(true, Some(&mut og));
                    if ret != 0 {
                        return ret;
                    }
                    if self.make_decode_ready() != 0 {
                        return OV_EBADLINK;
                    }
                    lastblock = 0;
                }
                self.os.pagein(&mut og);
            }
        }
        while self.pcm_offset < pos {
            let target = pos - self.pcm_offset;
            let mut samples = self.vd.as_ref().map_or(0, |vd| vd.pcmout(&self.vi, None, 0));
            if samples > target {
                samples = target;
            }
            if let Some(vd) = self.vd.as_mut() {
                vd.read(samples);
            }
            self.pcm_offset += samples;
            if samples < target && self.fetch_and_process_packet(true, true) <= 0 {
                self.pcm_offset = self.pcm_total(-1);
            }
        }
        0
    }

    /// ov_time_seek: to `milliseconds` in.
    pub fn time_seek(&mut self, milliseconds: i64) -> i64 {
        let mut pcm_total = self.pcm_total(-1);
        let mut time_total = self.time_total(-1);
        if self.ready_state < OPENED {
            return OV_EINVAL;
        }
        if !self.seekable {
            return OV_ENOSEEK;
        }
        if milliseconds < 0 || milliseconds > time_total {
            return OV_EINVAL;
        }
        let mut link = self.links - 1;
        while link >= 0 {
            pcm_total -= self.pcmlengths[link as usize * 2 + 1];
            time_total -= self.time_total(link);
            if milliseconds >= time_total {
                break;
            }
            link -= 1;
        }
        let ret = self.set_link_number(link);
        if ret != 0 {
            return ret;
        }
        self.pcm_seek(pcm_total + (milliseconds - time_total) * self.vi.rate / 1000)
    }

    /// The source's read position set back to the start (the Vorbis
    /// player does it before reopening a voice).
    pub fn rewind_source(&mut self) {
        self.src_offset = 0;
    }

    #[allow(dead_code)]
    pub fn pcm_tell(&self) -> i64 {
        if self.ready_state < OPENED {
            return OV_EINVAL;
        }
        self.pcm_offset
    }

    /// ov_read: up to `bytes_req` bytes of 16-bit samples, interleaved, into `buf`; the bytes, 0 at the end.
    pub fn read(&mut self, buf: &mut [i16], bytes_req: i64) -> i64 {
        if self.ready_state < OPENED {
            return OV_EINVAL;
        }
        loop {
            if self.ready_state == INITSET {
                let channels = self.vi.channels as i64;
                let vd = self.vd.as_ref().unwrap();
                let samples = vd.pcmout(&self.vi, Some(buf), (bytes_req >> 1) / channels);
                if samples != 0 {
                    if samples > 0 {
                        self.vd.as_mut().unwrap().read(samples);
                        self.pcm_offset += samples;
                        return samples * 2 * channels;
                    }
                    return samples;
                }
            }
            let ret = self.fetch_and_process_packet(true, true);
            if ret == OV_EOF {
                return 0;
            }
            if ret <= 0 {
                return ret;
            }
        }
    }
}
