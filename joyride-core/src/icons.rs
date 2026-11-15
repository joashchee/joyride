//! Desktop icon files from one square PNG: `.icns` (macOS) and `.ico`
//! (Windows). Both containers hold PNG-compressed images, so this is only
//! framing around the `image` crate's PNG encoder; no new dependency.
//!
//! The method follows the formats' own descriptions (an `.icns` is a `icns`
//! header plus `type, length, data` chunks, big-endian; an `.ico` is a
//! directory of entries whose image data may be a whole PNG file), not
//! anyone's code. Unit tests read the results back with the `image` crate.

use crate::{err, Error, Result};
use image::{imageops::FilterType, DynamicImage, ImageFormat};
use std::io::Cursor;
use std::path::Path;

/// The smallest source worth scaling down from: the biggest `.icns` entry is
/// 1024 px, and a smaller source would be scaled up (blurry).
pub const BEST_SOURCE: u32 = 1024;

/// Loads the icon, which must be square and at least 64 px.
pub fn load(path: &Path) -> Result<DynamicImage> {
    let img = image::open(path).map_err(|e| Error::Msg(format!("icon {}: {e}", path.display())))?;
    if img.width() != img.height() {
        return err(format!("icon {} is {}x{}: it must be square", path.display(), img.width(), img.height()));
    }
    if img.width() < 64 {
        return err(format!("icon {} is {} px: use at least 64 (1024 is best)", path.display(), img.width()));
    }
    Ok(img)
}

fn png(img: &DynamicImage, px: u32) -> Result<Vec<u8>> {
    let scaled = if img.width() == px { img.clone() } else { img.resize_exact(px, px, FilterType::Lanczos3) };
    let mut out = Cursor::new(Vec::new());
    scaled.write_to(&mut out, ImageFormat::Png).map_err(|e| Error::Msg(format!("icon: {e}")))?;
    Ok(out.into_inner())
}

/// `.icns` chunk types that hold a PNG, with the pixel size of each
/// (`ic11` to `ic14` are the 2x versions of 16, 32, 128 and 256 points).
/// There is no `icp6`: macOS's `iconutil` reads that slot as 48 px, not 64,
/// and 64 is already `ic12`.
const ICNS: [(&[u8; 4], u32); 9] = [
    (b"icp4", 16),
    (b"icp5", 32),
    (b"ic11", 32),
    (b"ic12", 64),
    (b"ic07", 128),
    (b"ic13", 256),
    (b"ic08", 256),
    (b"ic14", 512),
    (b"ic09", 512),
];

/// An `.icns` file; the 1024 px `ic10` entry is added when the source is at
/// least that big (a smaller source is never scaled up into it).
pub fn icns(img: &DynamicImage) -> Result<Vec<u8>> {
    let mut chunks: Vec<(&[u8; 4], Vec<u8>)> = Vec::new();
    for (ty, px) in ICNS {
        if px <= img.width().max(64) || px <= 128 {
            chunks.push((ty, png(img, px)?));
        }
    }
    if img.width() >= BEST_SOURCE {
        chunks.push((b"ic10", png(img, 1024)?));
    }
    let total: usize = 8 + chunks.iter().map(|(_, d)| 8 + d.len()).sum::<usize>();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"icns");
    out.extend_from_slice(&(total as u32).to_be_bytes());
    for (ty, data) in chunks {
        out.extend_from_slice(ty);
        out.extend_from_slice(&((8 + data.len()) as u32).to_be_bytes());
        out.extend_from_slice(&data);
    }
    Ok(out)
}

/// An `.ico` file with 16, 24, 32, 48, 64, 128 and 256 px PNG entries.
/// (Windows has read PNG entries since Vista; `0` in the width and height
/// bytes means 256.)
pub fn ico(img: &DynamicImage) -> Result<Vec<u8>> {
    let sizes = [16u32, 24, 32, 48, 64, 128, 256];
    let images = sizes.iter().map(|&s| png(img, s).map(|d| (s, d))).collect::<Result<Vec<_>>>()?;
    let mut out = Vec::new();
    out.extend_from_slice(&[0, 0, 1, 0]); // reserved, type 1 = icon
    out.extend_from_slice(&(images.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * images.len() as u32;
    for (s, d) in &images {
        let dim = if *s >= 256 { 0 } else { *s as u8 };
        out.extend_from_slice(&[dim, dim, 0, 0]); // width, height, palette, reserved
        out.extend_from_slice(&1u16.to_le_bytes()); // colour planes
        out.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
        out.extend_from_slice(&(d.len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += d.len() as u32;
    }
    for (_, d) in images {
        out.extend_from_slice(&d);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    fn source(px: u32) -> DynamicImage {
        let mut i = RgbaImage::new(px, px);
        for (x, y, p) in i.enumerate_pixels_mut() {
            *p = Rgba([(x * 255 / px) as u8, (y * 255 / px) as u8, 90, 255]);
        }
        DynamicImage::ImageRgba8(i)
    }

    /// Splits an `.icns` back into `(type, bytes)`.
    fn chunks(b: &[u8]) -> Vec<(String, Vec<u8>)> {
        assert_eq!(&b[..4], b"icns");
        assert_eq!(u32::from_be_bytes(b[4..8].try_into().unwrap()) as usize, b.len(), "the header length is the file's");
        let mut i = 8;
        let mut v = vec![];
        while i < b.len() {
            let len = u32::from_be_bytes(b[i + 4..i + 8].try_into().unwrap()) as usize;
            v.push((String::from_utf8(b[i..i + 4].to_vec()).unwrap(), b[i + 8..i + len].to_vec()));
            i += len;
        }
        assert_eq!(i, b.len());
        v
    }

    #[test]
    fn icns_chunks_decode_to_their_sizes() {
        let b = icns(&source(1024)).unwrap();
        let c = chunks(&b);
        assert_eq!(c.len(), 10, "nine and the 1024");
        for (ty, data) in &c {
            let want = match ty.as_str() {
                "icp4" => 16, "icp5" | "ic11" => 32, "ic12" => 64, "ic07" => 128, "ic13" | "ic08" => 256, "ic14" | "ic09" => 512, "ic10" => 1024,
                t => panic!("unexpected chunk {t}"),
            };
            let img = image::load_from_memory_with_format(data, ImageFormat::Png).unwrap();
            assert_eq!((img.width(), img.height()), (want, want), "{ty}");
        }
    }

    #[test]
    fn a_small_source_is_not_scaled_up_into_big_chunks() {
        let c = chunks(&icns(&source(256)).unwrap());
        let names: Vec<&str> = c.iter().map(|(t, _)| t.as_str()).collect();
        assert!(names.contains(&"ic08") && !names.contains(&"ic09") && !names.contains(&"ic10"), "{names:?}");
    }

    #[test]
    fn ico_directory_points_at_whole_pngs() {
        let b = ico(&source(512)).unwrap();
        assert_eq!(&b[..4], &[0, 0, 1, 0]);
        let n = u16::from_le_bytes([b[4], b[5]]) as usize;
        assert_eq!(n, 7);
        let mut sizes = vec![];
        for k in 0..n {
            let e = &b[6 + 16 * k..6 + 16 * (k + 1)];
            let len = u32::from_le_bytes(e[8..12].try_into().unwrap()) as usize;
            let off = u32::from_le_bytes(e[12..16].try_into().unwrap()) as usize;
            let img = image::load_from_memory_with_format(&b[off..off + len], ImageFormat::Png).unwrap();
            let dim = if e[0] == 0 { 256 } else { e[0] as u32 };
            assert_eq!((img.width(), img.height()), (dim, dim));
            sizes.push(dim);
        }
        assert_eq!(sizes, vec![16, 24, 32, 48, 64, 128, 256]);
    }

    #[test]
    fn a_bad_icon_is_refused_with_a_reason() {
        let d = tempfile::tempdir().unwrap();
        RgbaImage::new(64, 32).save(d.path().join("wide.png")).unwrap();
        RgbaImage::new(32, 32).save(d.path().join("tiny.png")).unwrap();
        assert!(load(&d.path().join("wide.png")).unwrap_err().to_string().contains("square"));
        assert!(load(&d.path().join("tiny.png")).unwrap_err().to_string().contains("at least 64"));
        assert!(load(&d.path().join("missing.png")).is_err());
    }
}
