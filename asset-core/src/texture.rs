//! Compressed-texture containers: what is inside (size, format, levels,
//! faces), read from the header alone. LÖVE reads DDS, KTX, PKM, PVR and ASTC
//! through `love.image.newCompressedData` and the GPU decodes them, so there
//! is no pixel decoder here: the preview shows these facts, not a picture.
//! KTX 2 and the legacy PVR 1 and 2 headers are named but LÖVE is not known to
//! load them.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextureFacts {
    pub container: &'static str,
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub levels: u32,
    /// Faces (a cube map has 6) times array layers.
    pub faces: u32,
    pub format: String,
    pub compressed: bool,
    /// LÖVE is not known to load this container version.
    pub love_unknown: bool,
}

fn le32(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(o..o + 4)?.try_into().ok()?))
}
fn be16(b: &[u8], o: usize) -> Option<u32> {
    Some(u16::from_be_bytes(b.get(o..o + 2)?.try_into().ok()?) as u32)
}

pub fn probe(b: &[u8]) -> Option<TextureFacts> {
    if b.starts_with(b"DDS ") {
        dds(b)
    } else if b.starts_with(b"\xabKTX 11\xbb\r\n\x1a\n") {
        ktx1(b)
    } else if b.starts_with(b"\xabKTX 20\xbb\r\n\x1a\n") {
        ktx2(b)
    } else if b.starts_with(b"PVR\x03") {
        pvr3(b)
    } else if b.starts_with(b"PKM ") {
        pkm(b)
    } else if b.starts_with(b"\x13\xab\xa1\x5c") {
        astc(b)
    } else {
        None
    }
}

fn dds(b: &[u8]) -> Option<TextureFacts> {
    let (h, w, depth, mips) = (le32(b, 12)?, le32(b, 16)?, le32(b, 24)?, le32(b, 28)?);
    let four = b.get(84..88)?;
    let (format, compressed) = match four {
        b"DXT1" => ("BC1 (DXT1)".to_string(), true),
        b"DXT2" | b"DXT3" => ("BC2 (DXT3)".to_string(), true),
        b"DXT4" | b"DXT5" => ("BC3 (DXT5)".to_string(), true),
        b"ATI1" | b"BC4U" => ("BC4".to_string(), true),
        b"ATI2" | b"BC5U" => ("BC5".to_string(), true),
        b"DX10" => {
            let f = le32(b, 128)?;
            let name = match f {
                28 | 29 => "R8G8B8A8".into(),
                87 | 91 => "B8G8R8A8".into(),
                70..=72 => "BC1 (DXT1)".to_string(),
                73..=75 => "BC2 (DXT3)".to_string(),
                76..=78 => "BC3 (DXT5)".to_string(),
                79..=81 => "BC4".to_string(),
                82..=84 => "BC5".to_string(),
                94..=96 => "BC6H".to_string(),
                97..=99 => "BC7".to_string(),
                n => format!("DXGI format {n}"),
            };
            let c = name.starts_with("BC");
            (name, c)
        }
        [0, 0, 0, 0] => (format!("uncompressed, {} bits per pixel", le32(b, 88)?), false),
        other => (format!("FourCC {}", String::from_utf8_lossy(other)), true),
    };
    let caps2 = le32(b, 112).unwrap_or(0);
    Some(TextureFacts { container: "DDS", width: le32(b, 16).map(|_| w)?, height: h, depth: depth.max(1), levels: mips.max(1), faces: if caps2 & 0x200 != 0 { 6 } else { 1 }, format, compressed, love_unknown: false })
}

fn gl_name(f: u32) -> String {
    match f {
        0x83F0 => "BC1 RGB (DXT1)".into(),
        0x83F1 => "BC1 RGBA (DXT1)".into(),
        0x83F2 => "BC2 (DXT3)".into(),
        0x83F3 => "BC3 (DXT5)".into(),
        0x8DBB => "BC4".into(),
        0x8DBD => "BC5".into(),
        0x8E8C | 0x8E8D => "BC7".into(),
        0x8E8E | 0x8E8F => "BC6H".into(),
        0x8D64 => "ETC1".into(),
        0x9274 | 0x9275 => "ETC2 RGB".into(),
        0x9276 | 0x9277 => "ETC2 RGB + 1-bit alpha".into(),
        0x9278 | 0x9279 => "ETC2 RGBA".into(),
        0x9270 | 0x9271 => "EAC R11".into(),
        0x9272 | 0x9273 => "EAC RG11".into(),
        0x8C00 => "PVRTC 4 bits RGB".into(),
        0x8C01 => "PVRTC 2 bits RGB".into(),
        0x8C02 => "PVRTC 4 bits RGBA".into(),
        0x8C03 => "PVRTC 2 bits RGBA".into(),
        0x93B0..=0x93BD => format!("ASTC {}", ["4x4", "5x4", "5x5", "6x5", "6x6", "8x5", "8x6", "8x8", "10x5", "10x6", "10x8", "10x10", "12x10", "12x12"][(f - 0x93B0) as usize]),
        0x93D0..=0x93DD => format!("ASTC {} (sRGB)", ["4x4", "5x4", "5x5", "6x5", "6x6", "8x5", "8x6", "8x8", "10x5", "10x6", "10x8", "10x10", "12x10", "12x12"][(f - 0x93D0) as usize]),
        0x8058 | 0x8C43 => "RGBA8".into(),
        0x8051 | 0x8C41 => "RGB8".into(),
        n => format!("OpenGL format 0x{n:04X}"),
    }
}

fn is_compressed_gl(f: u32) -> bool {
    matches!(f, 0x83F0..=0x83F3 | 0x8DBB..=0x8DBD | 0x8E8C..=0x8E8F | 0x8D64 | 0x9270..=0x9279 | 0x8C00..=0x8C03 | 0x93B0..=0x93BD | 0x93D0..=0x93DD)
}

fn ktx1(b: &[u8]) -> Option<TextureFacts> {
    let f = le32(b, 28)?;
    let (w, h, d, arr, faces, mips) = (le32(b, 36)?, le32(b, 40)?, le32(b, 44)?, le32(b, 48)?, le32(b, 52)?, le32(b, 56)?);
    Some(TextureFacts { container: "KTX 1", width: w, height: h, depth: d.max(1), levels: mips.max(1), faces: faces.max(1) * arr.max(1), format: gl_name(f), compressed: is_compressed_gl(f), love_unknown: false })
}

fn ktx2(b: &[u8]) -> Option<TextureFacts> {
    let vk = le32(b, 12)?;
    let format = match vk {
        0 => "supercompressed (Basis Universal)".to_string(),
        37 | 43 => "R8G8B8A8".into(),
        131..=134 => "BC1 (DXT1)".into(),
        135 | 136 => "BC2 (DXT3)".into(),
        137 | 138 => "BC3 (DXT5)".into(),
        139 | 140 => "BC4".into(),
        141 | 142 => "BC5".into(),
        143 | 144 => "BC6H".into(),
        145 | 146 => "BC7".into(),
        147..=152 => "ETC2".into(),
        153..=156 => "EAC".into(),
        157..=184 => "ASTC".into(),
        n => format!("Vulkan format {n}"),
    };
    Some(TextureFacts { container: "KTX 2", width: le32(b, 20)?, height: le32(b, 24)?, depth: le32(b, 28)?.max(1), levels: le32(b, 40)?.max(1), faces: le32(b, 36)?.max(1) * le32(b, 32)?.max(1), compressed: vk == 0 || vk >= 131, format, love_unknown: true })
}

fn pvr3(b: &[u8]) -> Option<TextureFacts> {
    let (lo, hi) = (le32(b, 8)?, le32(b, 12)?);
    let (format, compressed) = if hi == 0 {
        let name = match lo {
            0 => "PVRTC 2 bits RGB",
            1 => "PVRTC 2 bits RGBA",
            2 => "PVRTC 4 bits RGB",
            3 => "PVRTC 4 bits RGBA",
            4 => "PVRTC II 2 bits",
            5 => "PVRTC II 4 bits",
            6 => "ETC1",
            7 => "BC1 (DXT1)",
            9 => "BC2 (DXT3)",
            11 => "BC3 (DXT5)",
            12 => "BC4",
            13 => "BC5",
            14 => "BC6",
            15 => "BC7",
            22 => "ETC2 RGB",
            23 => "ETC2 RGBA",
            24 => "ETC2 RGB + 1-bit alpha",
            25 => "EAC R11",
            26 => "EAC RG11",
            27..=40 => return pvr_astc(b, lo),
            n => return Some(pvr_with(b, format!("PVR format {n}"), true)),
        };
        (name.to_string(), true)
    } else {
        // Uncompressed: the channel letters and their bit counts are the eight bytes.
        let ch: String = b.get(8..12)?.iter().filter(|c| c.is_ascii_alphabetic()).map(|&c| c as char).collect::<String>().to_uppercase();
        (format!("uncompressed {ch}"), false)
    };
    Some(pvr_with(b, format, compressed))
}

fn pvr_astc(b: &[u8], n: u32) -> Option<TextureFacts> {
    let dims = ["4x4", "5x4", "5x5", "6x5", "6x6", "8x5", "8x6", "8x8", "10x5", "10x6", "10x8", "10x10", "12x10", "12x12"];
    Some(pvr_with(b, format!("ASTC {}", dims.get((n - 27) as usize)?), true))
}

fn pvr_with(b: &[u8], format: String, compressed: bool) -> TextureFacts {
    let g = |o| le32(b, o).unwrap_or(1);
    TextureFacts { container: "PVR 3", width: g(28), height: g(24), depth: g(32).max(1), levels: g(44).max(1), faces: g(40).max(1) * g(36).max(1), format, compressed, love_unknown: false }
}

fn pkm(b: &[u8]) -> Option<TextureFacts> {
    let t = be16(b, 6)?;
    let name = match (t, b.get(4..6)) {
        (0, _) => "ETC1 RGB",
        (1, _) => "ETC2 RGB",
        (2 | 3, _) => "ETC2 RGBA",
        (4, _) => "ETC2 RGB + 1-bit alpha",
        (5 | 7, _) => "EAC R11",
        (6 | 8, _) => "EAC RG11",
        _ => "ETC",
    };
    Some(TextureFacts { container: "PKM", width: be16(b, 12)?, height: be16(b, 14)?, depth: 1, levels: 1, faces: 1, format: name.into(), compressed: true, love_unknown: false })
}

fn astc(b: &[u8]) -> Option<TextureFacts> {
    let (bx, by, bz) = (*b.get(4)?, *b.get(5)?, *b.get(6)?);
    let dim = |o: usize| Some(u32::from_le_bytes([*b.get(o)?, *b.get(o + 1)?, *b.get(o + 2)?, 0]));
    let block = if bz > 1 { format!("{bx}x{by}x{bz}") } else { format!("{bx}x{by}") };
    Some(TextureFacts { container: "ASTC", width: dim(7)?, height: dim(10)?, depth: dim(13)?.max(1), levels: 1, faces: 1, format: format!("ASTC {block}"), compressed: true, love_unknown: false })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(b: &mut [u8], o: usize, v: u32) {
        b[o..o + 4].copy_from_slice(&v.to_le_bytes());
    }

    #[test]
    fn dds_dxt5_with_mips_and_a_dx10_bc7() {
        let mut d = vec![0u8; 128];
        d[..4].copy_from_slice(b"DDS ");
        put(&mut d, 12, 64);
        put(&mut d, 16, 128);
        put(&mut d, 28, 8);
        d[84..88].copy_from_slice(b"DXT5");
        let f = probe(&d).unwrap();
        assert_eq!((f.container, f.width, f.height, f.levels, f.format.as_str(), f.compressed), ("DDS", 128, 64, 8, "BC3 (DXT5)", true));
        d[84..88].copy_from_slice(b"DX10");
        d.extend([0u8; 20]);
        put(&mut d, 128, 98);
        assert_eq!(probe(&d).unwrap().format, "BC7");
        d[84..88].copy_from_slice(&[0; 4]);
        put(&mut d, 88, 32);
        assert!(!probe(&d).unwrap().compressed);
    }

    #[test]
    fn ktx1_etc2_and_astc() {
        let mut k = vec![0u8; 64];
        k[..12].copy_from_slice(b"\xabKTX 11\xbb\r\n\x1a\n");
        put(&mut k, 28, 0x9278);
        put(&mut k, 36, 256);
        put(&mut k, 40, 128);
        put(&mut k, 52, 1);
        put(&mut k, 56, 9);
        let f = probe(&k).unwrap();
        assert_eq!((f.container, f.width, f.height, f.levels, f.format.as_str()), ("KTX 1", 256, 128, 9, "ETC2 RGBA"));
        put(&mut k, 28, 0x93B7);
        assert_eq!(probe(&k).unwrap().format, "ASTC 8x8");
    }

    #[test]
    fn ktx2_is_named_but_not_known_to_love() {
        let mut k = vec![0u8; 80];
        k[..12].copy_from_slice(b"\xabKTX 20\xbb\r\n\x1a\n");
        put(&mut k, 12, 145);
        put(&mut k, 20, 32);
        put(&mut k, 24, 32);
        let f = probe(&k).unwrap();
        assert_eq!((f.container, f.format.as_str(), f.love_unknown), ("KTX 2", "BC7", true));
    }

    #[test]
    fn pvr3_pkm_and_astc_files() {
        let mut p = vec![0u8; 52];
        p[..4].copy_from_slice(b"PVR\x03");
        put(&mut p, 8, 3);
        put(&mut p, 24, 64);
        put(&mut p, 28, 128);
        put(&mut p, 44, 7);
        let f = probe(&p).unwrap();
        assert_eq!((f.container, f.width, f.height, f.levels, f.format.as_str()), ("PVR 3", 128, 64, 7, "PVRTC 4 bits RGBA"));
        put(&mut p, 8, 30);
        assert_eq!(probe(&p).unwrap().format, "ASTC 6x5");

        let mut k = vec![0u8; 16];
        k[..6].copy_from_slice(b"PKM 20");
        k[6..8].copy_from_slice(&3u16.to_be_bytes());
        k[12..14].copy_from_slice(&100u16.to_be_bytes());
        k[14..16].copy_from_slice(&50u16.to_be_bytes());
        let f = probe(&k).unwrap();
        assert_eq!((f.container, f.width, f.height, f.format.as_str()), ("PKM", 100, 50, "ETC2 RGBA"));

        let a = [0x13, 0xab, 0xa1, 0x5c, 6, 6, 1, 64, 0, 0, 32, 0, 0, 1, 0, 0];
        let f = probe(&a).unwrap();
        assert_eq!((f.container, f.width, f.height, f.format.as_str()), ("ASTC", 64, 32, "ASTC 6x6"));
    }

    #[test]
    fn other_files_and_cut_headers_are_not_textures() {
        assert!(probe(b"hello").is_none());
        assert!(probe(b"DDS ").is_none());
        assert!(probe(b"PVR\x03").is_none());
        assert!(probe(b"PKM 10").is_none());
    }
}
