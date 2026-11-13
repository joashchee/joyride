//! Pixel decoding of the simple block-compressed textures, so the preview can
//! draw them: BC1 to BC5 (DXT1 to DXT5, ATI1, ATI2) and ETC1, first mip level
//! of the first face. Written from the public format descriptions; no code
//! from any other decoder. BC6H, BC7, ETC2, EAC, PVRTC and ASTC are not
//! decoded (`decode` answers `None` and the preview shows the header facts).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fmt {
    Bc1,
    Bc2,
    Bc3,
    Bc4,
    Bc5,
    Etc1,
}

impl Fmt {
    fn block_bytes(self) -> usize {
        match self {
            Fmt::Bc1 | Fmt::Bc4 | Fmt::Etc1 => 8,
            _ => 16,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decoded {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// The largest picture decoded (a guard against a header claiming gigapixels).
const MAX_EDGE: u32 = 16384;

fn le32(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(o..o + 4)?.try_into().ok()?))
}
fn be16(b: &[u8], o: usize) -> Option<u32> {
    Some(u16::from_be_bytes(b.get(o..o + 2)?.try_into().ok()?) as u32)
}

/// Decode the top level of a DDS, KTX 1, PKM or PVR 3 file, or `None` when the
/// container, format or data is not one this decodes.
pub fn decode(b: &[u8]) -> Option<Decoded> {
    let (fmt, w, h, data) = locate(b)?;
    if w == 0 || h == 0 || w > MAX_EDGE || h > MAX_EDGE {
        return None;
    }
    let (bw, bh) = (w.div_ceil(4) as usize, h.div_ceil(4) as usize);
    let need = bw.checked_mul(bh)?.checked_mul(fmt.block_bytes())?;
    let data = data.get(..need)?;
    let mut rgba = vec![0u8; w as usize * h as usize * 4];
    for by in 0..bh {
        for bx in 0..bw {
            let o = (by * bw + bx) * fmt.block_bytes();
            let px = block(fmt, &data[o..o + fmt.block_bytes()]);
            for y in 0..4 {
                for x in 0..4 {
                    let (px_x, px_y) = (bx * 4 + x, by * 4 + y);
                    if px_x < w as usize && px_y < h as usize {
                        let d = (px_y * w as usize + px_x) * 4;
                        rgba[d..d + 4].copy_from_slice(&px[y * 4 + x]);
                    }
                }
            }
        }
    }
    Some(Decoded { width: w, height: h, rgba })
}

fn locate(b: &[u8]) -> Option<(Fmt, u32, u32, &[u8])> {
    if b.starts_with(b"DDS ") {
        let (h, w) = (le32(b, 12)?, le32(b, 16)?);
        if le32(b, 24)? > 1 {
            return None; // volume texture
        }
        let (fmt, off) = match b.get(84..88)? {
            b"DXT1" => (Fmt::Bc1, 128),
            b"DXT2" | b"DXT3" => (Fmt::Bc2, 128),
            b"DXT4" | b"DXT5" => (Fmt::Bc3, 128),
            b"ATI1" | b"BC4U" => (Fmt::Bc4, 128),
            b"ATI2" | b"BC5U" => (Fmt::Bc5, 128),
            b"DX10" => {
                let f = match le32(b, 128)? {
                    70..=72 => Fmt::Bc1,
                    73..=75 => Fmt::Bc2,
                    76..=78 => Fmt::Bc3,
                    79 | 80 => Fmt::Bc4,
                    82 | 83 => Fmt::Bc5,
                    _ => return None,
                };
                (f, 148)
            }
            _ => return None,
        };
        Some((fmt, w, h, b.get(off..)?))
    } else if b.starts_with(b"\xabKTX 11\xbb\r\n\x1a\n") {
        if le32(b, 12)? != 0x04030201 || le32(b, 16)? != 0 {
            return None; // big-endian, or uncompressed data
        }
        let fmt = match le32(b, 28)? {
            0x83F0 | 0x83F1 => Fmt::Bc1,
            0x83F2 => Fmt::Bc2,
            0x83F3 => Fmt::Bc3,
            0x8DBB => Fmt::Bc4,
            0x8DBD => Fmt::Bc5,
            0x8D64 => Fmt::Etc1,
            _ => return None,
        };
        let (w, h) = (le32(b, 36)?, le32(b, 40)?.max(1));
        if le32(b, 44)? > 1 {
            return None;
        }
        let at = 64usize.checked_add(le32(b, 60)? as usize)?;
        let size = le32(b, at)? as usize;
        let data = b.get(at + 4..)?;
        Some((fmt, w, h, data.get(..size.min(data.len()))?))
    } else if b.starts_with(b"PKM 10") {
        if be16(b, 6)? != 0 {
            return None; // ETC2 and EAC
        }
        Some((Fmt::Etc1, be16(b, 8)?, be16(b, 10)?, b.get(16..)?))
    } else if b.starts_with(b"PVR\x03") {
        if le32(b, 12)? != 0 {
            return None;
        }
        let fmt = match le32(b, 8)? {
            6 => Fmt::Etc1,
            7 => Fmt::Bc1,
            9 => Fmt::Bc2,
            11 => Fmt::Bc3,
            12 => Fmt::Bc4,
            13 => Fmt::Bc5,
            _ => return None,
        };
        if le32(b, 32)? > 1 || le32(b, 40)? > 1 || le32(b, 36)? > 1 {
            return None;
        }
        let at = 52usize.checked_add(le32(b, 48)? as usize)?;
        Some((fmt, le32(b, 28)?, le32(b, 24)?, b.get(at..)?))
    } else {
        None
    }
}

fn block(f: Fmt, d: &[u8]) -> [[u8; 4]; 16] {
    let mut out = [[0, 0, 0, 255]; 16];
    match f {
        Fmt::Bc1 => bc1(d, &mut out, true),
        Fmt::Bc2 => {
            bc1(&d[8..], &mut out, false);
            for (i, p) in out.iter_mut().enumerate() {
                let n = (d[i / 2] >> (4 * (i % 2))) & 15;
                p[3] = n * 17;
            }
        }
        Fmt::Bc3 => {
            bc1(&d[8..], &mut out, false);
            let a = bc4_values(&d[..8]);
            for i in 0..16 {
                out[i][3] = a[i];
            }
        }
        Fmt::Bc4 => {
            let v = bc4_values(d);
            for i in 0..16 {
                out[i] = [v[i], v[i], v[i], 255];
            }
        }
        Fmt::Bc5 => {
            let (r, g) = (bc4_values(&d[..8]), bc4_values(&d[8..]));
            for i in 0..16 {
                out[i] = [r[i], g[i], 0, 255];
            }
        }
        Fmt::Etc1 => etc1(d, &mut out),
    }
    out
}

fn rgb565(c: u16) -> [u8; 3] {
    let (r, g, b) = ((c >> 11) as u32 & 31, (c >> 5) as u32 & 63, c as u32 & 31);
    [((r * 255 + 15) / 31) as u8, ((g * 255 + 31) / 63) as u8, ((b * 255 + 15) / 31) as u8]
}

/// `alpha_mode`: the 3-colour form's fourth colour is transparent (DXT1 with
/// alpha); BC2 and BC3 colour blocks are always the 4-colour form.
fn bc1(d: &[u8], out: &mut [[u8; 4]; 16], alpha_mode: bool) {
    let (c0, c1) = (u16::from_le_bytes([d[0], d[1]]), u16::from_le_bytes([d[2], d[3]]));
    let (a, b) = (rgb565(c0), rgb565(c1));
    let mix = |p: u32, q: u32| -> [u8; 3] { [0, 1, 2].map(|i| ((a[i] as u32 * p + b[i] as u32 * q) / (p + q)) as u8) };
    let four = c0 > c1 || !alpha_mode;
    let pal: [[u8; 4]; 4] = if four {
        let (m1, m2) = (mix(2, 1), mix(1, 2));
        [[a[0], a[1], a[2], 255], [b[0], b[1], b[2], 255], [m1[0], m1[1], m1[2], 255], [m2[0], m2[1], m2[2], 255]]
    } else {
        let m = mix(1, 1);
        [[a[0], a[1], a[2], 255], [b[0], b[1], b[2], 255], [m[0], m[1], m[2], 255], [0, 0, 0, 0]]
    };
    let bits = u32::from_le_bytes([d[4], d[5], d[6], d[7]]);
    for (i, p) in out.iter_mut().enumerate() {
        let c = pal[(bits >> (2 * i)) as usize & 3];
        p[0] = c[0];
        p[1] = c[1];
        p[2] = c[2];
        if alpha_mode {
            p[3] = c[3];
        }
    }
}

/// A 64-bit single-channel block (BC3's alpha, BC4, one half of BC5).
fn bc4_values(d: &[u8]) -> [u8; 16] {
    let (a0, a1) = (d[0] as u32, d[1] as u32);
    let pal: [u32; 8] = if a0 > a1 {
        [a0, a1, (6 * a0 + a1) / 7, (5 * a0 + 2 * a1) / 7, (4 * a0 + 3 * a1) / 7, (3 * a0 + 4 * a1) / 7, (2 * a0 + 5 * a1) / 7, (a0 + 6 * a1) / 7]
    } else {
        [a0, a1, (4 * a0 + a1) / 5, (3 * a0 + 2 * a1) / 5, (2 * a0 + 3 * a1) / 5, (a0 + 4 * a1) / 5, 0, 255]
    };
    let mut bits = 0u64;
    for (i, &x) in d[2..8].iter().enumerate() {
        bits |= (x as u64) << (8 * i);
    }
    let mut v = [0u8; 16];
    for (i, o) in v.iter_mut().enumerate() {
        *o = pal[((bits >> (3 * i)) & 7) as usize] as u8;
    }
    v
}

const ETC_TABLE: [[i32; 2]; 8] = [[2, 8], [5, 17], [9, 29], [13, 42], [18, 60], [24, 80], [33, 106], [47, 183]];

fn etc1(d: &[u8], out: &mut [[u8; 4]; 16]) {
    let (flip, diff) = (d[3] & 1 != 0, d[3] & 2 != 0);
    let (t1, t2) = ((d[3] >> 5) as usize, (d[3] >> 2 & 7) as usize);
    let (c1, c2): ([i32; 3], [i32; 3]) = if diff {
        let base = [d[0] >> 3, d[1] >> 3, d[2] >> 3];
        let delta = |x: u8| -> i32 {
            let v = (x & 7) as i32;
            if v >= 4 {
                v - 8
            } else {
                v
            }
        };
        let second = [0, 1, 2].map(|i| (base[i] as i32 + delta(d[i])).clamp(0, 31));
        let e = |v: i32| (v << 3) | (v >> 2);
        ([0, 1, 2].map(|i| e(base[i] as i32)), second.map(e))
    } else {
        let e = |v: u8| ((v & 15) as i32) * 17;
        ([e(d[0] >> 4), e(d[1] >> 4), e(d[2] >> 4)], [e(d[0]), e(d[1]), e(d[2])])
    };
    let bits = u32::from_be_bytes([d[4], d[5], d[6], d[7]]);
    for y in 0..4usize {
        for x in 0..4usize {
            let second = if flip { y >= 2 } else { x >= 2 };
            let (base, table) = if second { (c2, t2) } else { (c1, t1) };
            let k = x * 4 + y;
            let sel = ((bits >> (16 + k)) & 1) << 1 | ((bits >> k) & 1);
            let small = ETC_TABLE[table][0];
            let large = ETC_TABLE[table][1];
            let m = match sel {
                0 => small,
                1 => large,
                2 => -small,
                _ => -large,
            };
            let p = &mut out[y * 4 + x];
            for i in 0..3 {
                p[i] = (base[i] + m).clamp(0, 255) as u8;
            }
            p[3] = 255;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dds(four: &[u8; 4], w: u32, h: u32, data: &[u8]) -> Vec<u8> {
        let mut d = vec![0u8; 128];
        d[..4].copy_from_slice(b"DDS ");
        d[12..16].copy_from_slice(&h.to_le_bytes());
        d[16..20].copy_from_slice(&w.to_le_bytes());
        d[84..88].copy_from_slice(four);
        d.extend_from_slice(data);
        d
    }

    #[test]
    fn dxt1_two_colour_block_and_cropped_edge() {
        // red (0xF800) and blue (0x001F), indices: row 0 = 0,1,2,3, the rest 0.
        let mut blk = vec![0x00, 0xF8, 0x1F, 0x00];
        blk.extend([0b11_10_01_00, 0, 0, 0]);
        let r = decode(&dds(b"DXT1", 4, 4, &blk)).unwrap();
        assert_eq!((r.width, r.height), (4, 4));
        assert_eq!(&r.rgba[0..4], &[255, 0, 0, 255]);
        assert_eq!(&r.rgba[4..8], &[0, 0, 255, 255]);
        assert_eq!(&r.rgba[8..12], &[170, 0, 85, 255]); // 2/3 red + 1/3 blue
        assert_eq!(&r.rgba[12..16], &[85, 0, 170, 255]);
        // 3 x 2 picture: only the corner of the block is kept
        let c = decode(&dds(b"DXT1", 3, 2, &blk)).unwrap();
        assert_eq!(c.rgba.len(), 3 * 2 * 4);
        assert_eq!(&c.rgba[8..12], &[170, 0, 85, 255]);
    }

    #[test]
    fn dxt1_three_colour_mode_has_a_clear_pixel() {
        let mut blk = vec![0x1F, 0x00, 0x00, 0xF8]; // c0 < c1
        blk.extend([0b11, 0, 0, 0]);
        let r = decode(&dds(b"DXT1", 4, 4, &blk)).unwrap();
        assert_eq!(r.rgba[3], 0);
    }

    #[test]
    fn dxt3_and_dxt5_alpha() {
        let colour = [0x00, 0xF8, 0x00, 0xF8, 0, 0, 0, 0];
        let mut b3 = vec![0xF0u8, 0, 0, 0, 0, 0, 0, 0]; // pixel 0 alpha 0, pixel 1 alpha 15
        b3[0] = 0xF0;
        b3.extend(colour);
        let r = decode(&dds(b"DXT3", 4, 4, &b3)).unwrap();
        assert_eq!((r.rgba[3], r.rgba[7]), (0, 255));
        // alpha endpoints 255 and 0, index 1 on pixel 0 -> 0
        let mut b5 = vec![255u8, 0, 0b001, 0, 0, 0, 0, 0];
        b5.extend(colour);
        let r = decode(&dds(b"DXT5", 4, 4, &b5)).unwrap();
        assert_eq!((r.rgba[3], r.rgba[7]), (0, 255));
    }

    #[test]
    fn bc4_and_bc5_channels() {
        let r = decode(&dds(b"ATI1", 4, 4, &[200, 100, 0, 0, 0, 0, 0, 0])).unwrap();
        assert_eq!(&r.rgba[0..4], &[200, 200, 200, 255]);
        let mut d = vec![10u8, 5, 0, 0, 0, 0, 0, 0];
        d.extend([77u8, 1, 0, 0, 0, 0, 0, 0]);
        let r = decode(&dds(b"ATI2", 4, 4, &d)).unwrap();
        assert_eq!(&r.rgba[0..4], &[10, 77, 0, 255]);
    }

    #[test]
    fn etc1_individual_and_differential() {
        // individual: both halves base (15,0,0)*17 = (255,0,0), table 0, every index 0 -> +2 on each channel
        let r = decode(&pkm(0, &[0xF0, 0x00, 0x00, 0x00 | (0 << 5), 0, 0, 0, 0], 4, 4)).unwrap();
        assert_eq!(&r.rgba[0..4], &[255, 2, 2, 255]);
        // differential: base 16 (grey 132), delta 0, table 0, index 3 (-8) on pixel (0,0): msb bit16, lsb bit0
        let bits = (1u32 << 16) | 1;
        let mut blk = vec![0x80, 0x80, 0x80, 0x02];
        blk.extend(bits.to_be_bytes());
        let r = decode(&pkm(0, &blk, 4, 4)).unwrap();
        assert_eq!(&r.rgba[0..4], &[132 - 8, 132 - 8, 132 - 8, 255]);
        assert_eq!(&r.rgba[4..8], &[134, 134, 134, 255]);
    }

    fn pkm(kind: u16, data: &[u8], w: u16, h: u16) -> Vec<u8> {
        let mut k = vec![0u8; 16];
        k[..6].copy_from_slice(b"PKM 10");
        k[6..8].copy_from_slice(&kind.to_be_bytes());
        k[8..10].copy_from_slice(&w.to_be_bytes());
        k[10..12].copy_from_slice(&h.to_be_bytes());
        k[12..14].copy_from_slice(&w.to_be_bytes());
        k[14..16].copy_from_slice(&h.to_be_bytes());
        k.extend_from_slice(data);
        k
    }

    #[test]
    fn ktx1_and_pvr3_carry_the_same_blocks() {
        let blk = [0x00u8, 0xF8, 0x1F, 0x00, 0, 0, 0, 0];
        let mut k = vec![0u8; 64];
        k[..12].copy_from_slice(b"\xabKTX 11\xbb\r\n\x1a\n");
        k[12..16].copy_from_slice(&0x04030201u32.to_le_bytes());
        k[28..32].copy_from_slice(&0x83F1u32.to_le_bytes());
        k[36..40].copy_from_slice(&4u32.to_le_bytes());
        k[40..44].copy_from_slice(&4u32.to_le_bytes());
        k[60..64].copy_from_slice(&4u32.to_le_bytes()); // 4 key/value bytes
        k.extend([0u8; 4]);
        k.extend(8u32.to_le_bytes());
        k.extend(blk);
        assert_eq!(&decode(&k).unwrap().rgba[0..4], &[255, 0, 0, 255]);

        let mut p = vec![0u8; 52];
        p[..4].copy_from_slice(b"PVR\x03");
        p[8..12].copy_from_slice(&7u32.to_le_bytes());
        p[24..28].copy_from_slice(&4u32.to_le_bytes());
        p[28..32].copy_from_slice(&4u32.to_le_bytes());
        p[32..36].copy_from_slice(&1u32.to_le_bytes());
        p[36..40].copy_from_slice(&1u32.to_le_bytes());
        p[40..44].copy_from_slice(&1u32.to_le_bytes());
        p.extend(blk);
        assert_eq!(&decode(&p).unwrap().rgba[0..4], &[255, 0, 0, 255]);
    }

    #[test]
    fn short_data_and_other_formats_are_not_decoded() {
        assert!(decode(&dds(b"DXT1", 64, 64, &[0; 8])).is_none());
        assert!(decode(&dds(b"DXT1", 0, 4, &[0; 8])).is_none());
        assert!(decode(&dds(b"DXT1", 100_000, 4, &[0; 8])).is_none());
        let mut d = dds(b"DX10", 4, 4, &[0; 36]);
        d[128..132].copy_from_slice(&98u32.to_le_bytes()); // BC7
        assert!(decode(&d).is_none());
        assert!(decode(b"hello").is_none());
    }
}
