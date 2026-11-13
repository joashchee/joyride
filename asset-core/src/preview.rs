//! What the library shows for an asset before it is used: a thumbnail, the
//! facts and tags an image gives (size, palette, pixel art or not), and a
//! waveform for sound. Everything is derived from the file and stored beside
//! the catalog record, never in place of the file.

use crate::{Error, Result};
use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, ImageFormat};
use std::collections::HashMap;
use std::io::Cursor;
use std::path::Path;

/// Longest side of a stored thumbnail, in pixels.
pub const THUMB_EDGE: u32 = 128;

#[derive(Debug, Clone, PartialEq)]
pub struct ImageFacts {
    pub width: u32,
    pub height: u32,
    /// Any pixel is not fully opaque.
    pub has_alpha: bool,
    /// Distinct visible colours (counted up to a cap, so very rich images read as the cap).
    pub colours: usize,
    /// The most used colours, near-duplicates merged, most used first.
    pub palette: Vec<[u8; 3]>,
    /// Small, few colours and hard-edged: made pixel by pixel.
    pub pixel_art: bool,
}

const COLOUR_CAP: usize = 4096;

fn open(path: &Path) -> Result<DynamicImage> {
    image::ImageReader::open(path)?.with_guessed_format()?.decode().map_err(|e| Error::Other(format!("{}: {e}", path.display())))
}

pub fn image_facts(path: &Path) -> Result<ImageFacts> {
    Ok(facts_of(&open(path)?))
}

fn facts_of(img: &DynamicImage) -> ImageFacts {
    let (w, h) = img.dimensions();
    let rgba = img.to_rgba8();
    let mut counts: HashMap<[u8; 3], u32> = HashMap::new();
    let (mut has_alpha, mut soft_alpha) = (false, false);
    // Very large images are sampled: a palette does not need every pixel.
    let stride = ((w as u64 * h as u64) / 1_000_000).max(1) as usize;
    for (i, p) in rgba.pixels().enumerate() {
        if p[3] < 255 {
            has_alpha = true;
            if p[3] != 0 {
                soft_alpha = true;
            }
        }
        if p[3] == 0 || i % stride != 0 {
            continue;
        }
        if counts.len() < COLOUR_CAP || counts.contains_key(&[p[0], p[1], p[2]]) {
            *counts.entry([p[0], p[1], p[2]]).or_default() += 1;
        }
    }
    let colours = counts.len();
    let mut ranked: Vec<([u8; 3], u32)> = counts.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut palette: Vec<[u8; 3]> = Vec::new();
    for (c, _) in ranked {
        if palette.len() == 6 {
            break;
        }
        if palette.iter().all(|p| distance(*p, c) > 40) {
            palette.push(c);
        }
    }
    let pixel_art = w <= 256 && h <= 256 && colours <= 64 && !soft_alpha;
    ImageFacts { width: w, height: h, has_alpha, colours, palette, pixel_art }
}

fn distance(a: [u8; 3], b: [u8; 3]) -> i32 {
    (0..3).map(|i| (a[i] as i32 - b[i] as i32).abs()).sum()
}

/// The tags an image's own facts imply, in the library's tag syntax.
pub fn image_tags(f: &ImageFacts) -> Vec<String> {
    let mut t = vec![format!("size:{}x{}", f.width, f.height)];
    let long = f.width.max(f.height);
    t.push(format!("size-class:{}", match long { 0..=32 => "tiny", 33..=128 => "small", 129..=512 => "medium", _ => "large" }));
    if f.pixel_art {
        t.push("style:pixel-art".into());
    }
    t.push(format!("palette:{}", if f.colours <= 16 { "limited" } else if f.colours <= 256 { "moderate" } else { "rich" }));
    if f.has_alpha {
        t.push("alpha:yes".into());
    }
    t
}

/// A PNG thumbnail, no larger than `max_edge` on its long side. Pixel art
/// that is smaller is scaled up by a whole number with hard edges so it
/// stays crisp; anything else only ever shrinks.
pub fn thumbnail_png(path: &Path, max_edge: u32) -> Result<(Vec<u8>, u32, u32)> {
    let img = open(path)?;
    let f = facts_of(&img);
    let (w, h) = img.dimensions();
    let long = w.max(h);
    let out = if f.pixel_art && long < max_edge {
        let k = max_edge / long;
        img.resize_exact(w * k, h * k, FilterType::Nearest)
    } else if long > max_edge {
        let (nw, nh) = if w >= h { (max_edge, (h * max_edge / w).max(1)) } else { ((w * max_edge / h).max(1), max_edge) };
        img.resize_exact(nw, nh, if f.pixel_art { FilterType::Nearest } else { FilterType::Triangle })
    } else {
        img
    };
    let (ow, oh) = out.dimensions();
    let mut buf = Cursor::new(Vec::new());
    out.write_to(&mut buf, ImageFormat::Png).map_err(|e| Error::Other(e.to_string()))?;
    Ok((buf.into_inner(), ow, oh))
}

/// Peak level per bin, 0 to 255, for a WAV (8, 16, 24, 32-bit PCM or 32-bit
/// float, any channel count). `None` for any other file.
pub fn waveform(path: &Path, bins: usize) -> Result<Option<Vec<u8>>> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path)?;
    let mut head = [0u8; 12];
    if f.read(&mut head)? < 12 || &head[..4] != b"RIFF" || &head[8..12] != b"WAVE" || bins == 0 {
        return Ok(None);
    }
    let (mut tag, mut channels, mut bits) = (0u16, 0u16, 0u16);
    let mut hdr = [0u8; 8];
    loop {
        if f.read(&mut hdr)? < 8 {
            return Ok(None);
        }
        let size = u32::from_le_bytes(hdr[4..8].try_into().unwrap()) as u64;
        match &hdr[..4] {
            b"fmt " => {
                let mut b = vec![0u8; size.min(40) as usize];
                f.read_exact(&mut b)?;
                tag = u16::from_le_bytes([b[0], b[1]]);
                channels = u16::from_le_bytes([b[2], b[3]]);
                bits = u16::from_le_bytes([b[14], b[15]]);
                f.seek(SeekFrom::Current((size - b.len() as u64 + (size & 1)) as i64))?;
            }
            b"data" => {
                let avail = f.metadata()?.len().saturating_sub(f.stream_position()?);
                let size = size.min(avail);
                let bytes = (bits / 8) as u64;
                if channels == 0 || bytes == 0 || !matches!((tag, bits), (1, 8) | (1, 16) | (1, 24) | (1, 32) | (3, 32)) {
                    return Ok(None);
                }
                let frames = size / (bytes * channels as u64);
                if frames == 0 {
                    return Ok(None);
                }
                let mut data = vec![0u8; (frames * bytes * channels as u64) as usize];
                f.read_exact(&mut data)?;
                let mut peaks = vec![0f32; bins];
                for (fi, frame) in data.chunks_exact((bytes * channels as u64) as usize).enumerate() {
                    let bin = ((fi as u64 * bins as u64) / frames) as usize;
                    for s in frame.chunks_exact(bytes as usize) {
                        let v = match (tag, bits) {
                            (1, 8) => (s[0] as f32 - 128.0) / 128.0,
                            (1, 16) => i16::from_le_bytes([s[0], s[1]]) as f32 / 32768.0,
                            (1, 24) => (i32::from_le_bytes([0, s[0], s[1], s[2]]) >> 8) as f32 / 8388608.0,
                            (1, _) => i32::from_le_bytes([s[0], s[1], s[2], s[3]]) as f32 / 2147483648.0,
                            _ => f32::from_le_bytes([s[0], s[1], s[2], s[3]]),
                        };
                        peaks[bin] = peaks[bin].max(v.abs());
                    }
                }
                return Ok(Some(peaks.iter().map(|p| (p.min(1.0) * 255.0).round() as u8).collect()));
            }
            _ => {
                f.seek(SeekFrom::Current((size + (size & 1)) as i64))?;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    fn save(d: &Path, name: &str, img: &RgbaImage) -> std::path::PathBuf {
        let p = d.join(name);
        img.save(&p).unwrap();
        p
    }

    fn pixel_art(n: u32) -> RgbaImage {
        RgbaImage::from_fn(n, n, |x, y| match (x * 2 / n, y * 2 / n) {
            (0, 0) => Rgba([255, 0, 0, 255]),
            (1, 0) => Rgba([0, 255, 0, 255]),
            (0, 1) => Rgba([0, 0, 255, 255]),
            _ => Rgba([0, 0, 0, 0]),
        })
    }

    #[test]
    fn pixel_art_facts_tags_and_a_crisp_upscaled_thumbnail() {
        let d = tempfile::tempdir().unwrap();
        let p = save(d.path(), "hero.png", &pixel_art(16));
        let f = image_facts(&p).unwrap();
        assert_eq!((f.width, f.height, f.colours, f.has_alpha, f.pixel_art), (16, 16, 3, true, true));
        assert_eq!(f.palette.len(), 3);
        let tags = image_tags(&f);
        for t in ["size:16x16", "size-class:tiny", "style:pixel-art", "palette:limited", "alpha:yes"] {
            assert!(tags.contains(&t.to_string()), "{t} in {tags:?}");
        }
        let (png, w, h) = thumbnail_png(&p, 128).unwrap();
        assert_eq!((w, h), (128, 128), "16 scales by a whole 8");
        let t = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!(t.get_pixel(0, 0).0, [255, 0, 0, 255]);
        assert_eq!(t.get_pixel(7, 7).0, [255, 0, 0, 255], "hard edges: still red 7 px in");
        assert_eq!(t.get_pixel(64, 0).0, [0, 255, 0, 255]);
        assert_eq!(t.get_pixel(100, 100).0[3], 0, "transparency survives");
    }

    #[test]
    fn a_photo_like_image_shrinks_and_is_not_pixel_art() {
        let d = tempfile::tempdir().unwrap();
        let img = RgbaImage::from_fn(512, 256, |x, y| Rgba([(x / 2) as u8, (y) as u8, ((x + y) / 3) as u8, 255]));
        let p = save(d.path(), "bg.png", &img);
        let f = image_facts(&p).unwrap();
        assert!(!f.pixel_art && f.colours > 256 && !f.has_alpha);
        let tags = image_tags(&f);
        assert!(tags.contains(&"palette:rich".to_string()) && tags.contains(&"size-class:medium".to_string()) && !tags.contains(&"alpha:yes".to_string()));
        let (png, w, h) = thumbnail_png(&p, 128).unwrap();
        assert_eq!((w, h), (128, 64));
        assert_eq!(image::load_from_memory(&png).unwrap().dimensions(), (128, 64));
    }

    #[test]
    fn near_duplicate_colours_merge_in_the_palette() {
        let d = tempfile::tempdir().unwrap();
        let img = RgbaImage::from_fn(40, 40, |x, _| if x < 20 { Rgba([200, 10, 10, 255]) } else if x < 30 { Rgba([205, 12, 8, 255]) } else { Rgba([10, 10, 200, 255]) });
        let f = image_facts(&save(d.path(), "a.png", &img)).unwrap();
        assert_eq!(f.colours, 3);
        assert_eq!(f.palette.len(), 2, "the two reds count as one: {:?}", f.palette);
    }

    #[test]
    fn jpeg_and_bad_files() {
        let d = tempfile::tempdir().unwrap();
        let rgb = image::RgbImage::from_fn(64, 32, |x, y| image::Rgb([x as u8 * 3, y as u8 * 6, 90]));
        let p = d.path().join("a.jpg");
        rgb.save(&p).unwrap();
        assert_eq!(image_facts(&p).unwrap().width, 64);
        std::fs::write(d.path().join("n.png"), b"not an image").unwrap();
        assert!(image_facts(&d.path().join("n.png")).is_err());
        assert!(thumbnail_png(&d.path().join("missing.png"), 64).is_err());
    }

    fn wav(samples: &[i16], channels: u16) -> Vec<u8> {
        let data: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
        let mut v = b"RIFF".to_vec();
        v.extend((36 + data.len() as u32).to_le_bytes());
        v.extend(b"WAVEfmt ");
        v.extend(16u32.to_le_bytes());
        v.extend([1, 0]);
        v.extend(channels.to_le_bytes());
        v.extend(8000u32.to_le_bytes());
        v.extend((8000 * channels as u32 * 2).to_le_bytes());
        v.extend((channels * 2).to_le_bytes());
        v.extend(16u16.to_le_bytes());
        v.extend(b"data");
        v.extend((data.len() as u32).to_le_bytes());
        v.extend(data);
        v
    }

    #[test]
    fn waveform_peaks_per_bin() {
        let d = tempfile::tempdir().unwrap();
        // Mono: silence, then half scale, then full scale, then quarter.
        let mut s = vec![0i16; 100];
        s.extend(vec![16384i16; 100]);
        s.extend(vec![-32768i16; 100]);
        s.extend(vec![8192i16; 100]);
        let p = d.path().join("a.wav");
        std::fs::write(&p, wav(&s, 1)).unwrap();
        assert_eq!(waveform(&p, 4).unwrap().unwrap(), [0, 128, 255, 64]);
        // Stereo: the louder channel wins in each frame.
        let st: Vec<i16> = (0..200).flat_map(|i| if i < 100 { [0, 8192] } else { [-16384, 0] }).collect();
        let q = d.path().join("s.wav");
        std::fs::write(&q, wav(&st, 2)).unwrap();
        assert_eq!(waveform(&q, 2).unwrap().unwrap(), [64, 128]);
        // Not a WAV, or no bins: nothing.
        std::fs::write(d.path().join("t.txt"), b"hello").unwrap();
        assert!(waveform(&d.path().join("t.txt"), 4).unwrap().is_none());
        assert!(waveform(&p, 0).unwrap().is_none());
    }
}
