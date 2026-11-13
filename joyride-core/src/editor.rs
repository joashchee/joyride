//! What the editor's providers need (spec 3B): colour literals for inline
//! swatches, and asset-path strings for hover previews. Offsets are byte
//! offsets into the source; the frontend turns them into positions.

use crate::lua::{lex, Tok};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct ColorLiteral {
    pub line: u32,
    /// The numbers inside the parentheses (first number to last number).
    pub start: usize,
    pub end: usize,
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

/// `love.graphics.setColor(r, g, b[, a])` and `setBackgroundColor`, with
/// plain 0 to 1 number literals (also in one table). Anything else (names,
/// expressions, 0 to 255 values) is left alone: no swatch.
pub fn color_literals(src: &str) -> Vec<ColorLiteral> {
    let t = lex(src);
    let mut out = Vec::new();
    for i in 0..t.len() {
        if !(t[i].is_name("love") && t.get(i + 1).is_some_and(|x| x.is_punct(".")) && t.get(i + 2).is_some_and(|x| x.is_name("graphics")) && t.get(i + 3).is_some_and(|x| x.is_punct("."))) {
            continue;
        }
        if !(t.get(i + 4).is_some_and(|x| x.is_name("setColor") || x.is_name("setBackgroundColor")) && t.get(i + 5).is_some_and(|x| x.is_punct("("))) {
            continue;
        }
        let mut j = i + 6;
        if t.get(j).is_some_and(|x| x.is_punct("{")) {
            j += 1;
        }
        let mut nums = Vec::new();
        let (mut first, mut last) = (None, 0);
        while let Some(k) = t.get(j) {
            match k.tok {
                Tok::Number(n) => {
                    nums.push(n);
                    first.get_or_insert(k.start);
                    last = k.end;
                    j += 1;
                    match t.get(j) {
                        Some(c) if c.is_punct(",") => j += 1,
                        _ => break,
                    }
                }
                _ => break,
            }
        }
        // The call must end right after the numbers (a closing brace and paren).
        let rest = &t[j.min(t.len())..];
        let closed = match rest {
            [a, ..] if a.is_punct(")") => true,
            [a, b, ..] if a.is_punct("}") && b.is_punct(")") => true,
            _ => false,
        };
        if (nums.len() == 3 || nums.len() == 4) && closed && nums.iter().all(|n| (0.0..=1.0).contains(n)) {
            out.push(ColorLiteral { line: t[i].line, start: first.unwrap(), end: last, r: nums[0], g: nums[1], b: nums[2], a: nums.get(3).copied().unwrap_or(1.0) });
        }
    }
    out
}

/// The text to write back when the swatch picker changes a colour: up to
/// three decimals, trailing zeros dropped, alpha only if it is not 1.
pub fn format_color(r: f64, g: f64, b: f64, a: f64) -> String {
    let f = |x: f64| {
        let s = format!("{:.3}", x.clamp(0.0, 1.0));
        let s = s.trim_end_matches('0').trim_end_matches('.');
        if s.is_empty() { "0".to_string() } else { s.to_string() }
    };
    if (a - 1.0).abs() < 0.0005 {
        format!("{}, {}, {}", f(r), f(g), f(b))
    } else {
        format!("{}, {}, {}, {}", f(r), f(g), f(b), f(a))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetKind {
    Image,
    Audio,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetString {
    pub start: usize,
    pub end: usize,
    pub path: String,
    pub kind: AssetKind,
}

/// String literals naming an image or sound file, for hover previews.
pub fn asset_strings(src: &str) -> Vec<AssetString> {
    lex(src)
        .into_iter()
        .filter_map(|t| {
            let Tok::Str(s) = t.tok else { return None };
            let ext = s.rsplit_once('.')?.1.to_ascii_lowercase();
            let kind = match ext.as_str() {
                "png" | "jpg" | "jpeg" | "gif" | "bmp" | "tga" => AssetKind::Image,
                "ogg" | "wav" | "mp3" | "flac" => AssetKind::Audio,
                _ => return None,
            };
            Some(AssetString { start: t.start, end: t.end, path: s, kind })
        })
        .collect()
}

/// Resolves a path string against the project, only if it names an existing
/// file inside it.
pub fn resolve_asset(project: &Path, s: &str) -> Option<PathBuf> {
    let p = project.join(s.trim_start_matches('/'));
    let (canon, root) = (p.canonicalize().ok()?, project.canonicalize().ok()?);
    (canon.starts_with(&root) && canon.is_file()).then_some(canon)
}

/// Width and height of a PNG, GIF or JPEG from its header.
pub fn image_size(path: &Path) -> Option<(u32, u32)> {
    use std::io::Read;
    let mut f = std::fs::File::open(path).ok()?;
    let mut head = vec![0u8; 64 * 1024];
    let n = f.read(&mut head).ok()?;
    let b = &head[..n];
    if b.starts_with(b"\x89PNG\r\n\x1a\n") && n >= 24 {
        return Some((u32::from_be_bytes(b[16..20].try_into().ok()?), u32::from_be_bytes(b[20..24].try_into().ok()?)));
    }
    if (b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a")) && n >= 10 {
        return Some((u16::from_le_bytes([b[6], b[7]]) as u32, u16::from_le_bytes([b[8], b[9]]) as u32));
    }
    if b.starts_with(&[0xFF, 0xD8]) {
        let mut i = 2;
        while i + 9 < n {
            if b[i] != 0xFF {
                i += 1;
                continue;
            }
            let m = b[i + 1];
            if m == 0xD8 || m == 0x01 || (0xD0..=0xD7).contains(&m) || m == 0xFF {
                i += if m == 0xFF { 1 } else { 2 };
                continue;
            }
            let len = u16::from_be_bytes([b[i + 2], b[i + 3]]) as usize;
            if matches!(m, 0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF) {
                return Some((u16::from_be_bytes([b[i + 7], b[i + 8]]) as u32, u16::from_be_bytes([b[i + 5], b[i + 6]]) as u32));
            }
            i += 2 + len;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colour_literals_with_ranges() {
        let src = "love.graphics.setColor(0.2, 0.4, 0.6)\nlove.graphics.setColor({1, 0, 0, 0.5})\nlove.graphics.setBackgroundColor(0,0,0)";
        let c = color_literals(src);
        assert_eq!(c.len(), 3);
        assert_eq!((c[0].r, c[0].g, c[0].b, c[0].a), (0.2, 0.4, 0.6, 1.0));
        assert_eq!(&src[c[0].start..c[0].end], "0.2, 0.4, 0.6");
        assert_eq!((c[1].line, c[1].a), (2, 0.5));
        assert_eq!(&src[c[1].start..c[1].end], "1, 0, 0, 0.5");
    }

    #[test]
    fn no_swatch_for_expressions_names_or_255_values() {
        for s in ["love.graphics.setColor(r, g, b)", "love.graphics.setColor(255, 0, 0)", "love.graphics.setColor(0.5, 0.5)", "love.graphics.setColor(0.1, 0.2, 0.3 + x)", "-- love.graphics.setColor(0,0,0)", "love.graphics.setColor(1, 1, 1, 1, 1)"] {
            assert!(color_literals(s).is_empty(), "{s}");
        }
    }

    #[test]
    fn colour_formatting_round_trips() {
        assert_eq!(format_color(0.2, 0.4, 0.6, 1.0), "0.2, 0.4, 0.6");
        assert_eq!(format_color(1.0, 0.0, 0.0, 0.5), "1, 0, 0, 0.5");
        assert_eq!(format_color(0.33333, 0.0, 1.5, 1.0), "0.333, 0, 1");
        let c = color_literals(&format!("love.graphics.setColor({})", format_color(0.125, 0.5, 0.875, 0.25)));
        assert_eq!((c[0].r, c[0].g, c[0].b, c[0].a), (0.125, 0.5, 0.875, 0.25));
    }

    #[test]
    fn asset_strings_and_resolution() {
        let src = "local i = love.graphics.newImage('img/Hero.PNG') -- 'no.png'\nlocal s = love.audio.newSource(\"sfx/boom.ogg\", 'static') local x = 'readme.txt'";
        let a = asset_strings(src);
        assert_eq!(a.iter().map(|x| (x.path.as_str(), x.kind)).collect::<Vec<_>>(), [("img/Hero.PNG", AssetKind::Image), ("sfx/boom.ogg", AssetKind::Audio)]);
        assert_eq!(&src[a[0].start..a[0].end], "'img/Hero.PNG'");
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("game/img")).unwrap();
        std::fs::write(d.path().join("game/img/Hero.PNG"), b"x").unwrap();
        std::fs::write(d.path().join("secret.png"), b"x").unwrap();
        assert!(resolve_asset(&d.path().join("game"), "img/Hero.PNG").is_some());
        assert!(resolve_asset(&d.path().join("game"), "missing.png").is_none());
        assert!(resolve_asset(&d.path().join("game"), "../secret.png").is_none());
    }

    #[test]
    fn image_sizes() {
        let d = tempfile::tempdir().unwrap();
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        png.extend(64u32.to_be_bytes());
        png.extend(32u32.to_be_bytes());
        png.extend([8, 6, 0, 0, 0]);
        std::fs::write(d.path().join("a.png"), &png).unwrap();
        assert_eq!(image_size(&d.path().join("a.png")), Some((64, 32)));
        let mut gif = b"GIF89a".to_vec();
        gif.extend(10u16.to_le_bytes());
        gif.extend(20u16.to_le_bytes());
        gif.extend([0; 4]);
        std::fs::write(d.path().join("a.gif"), &gif).unwrap();
        assert_eq!(image_size(&d.path().join("a.gif")), Some((10, 20)));
        // JPEG: SOI, an APP0 segment to skip, then SOF0 (height 48, width 96).
        let mut jpg = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0x00, 0x00, 0xFF, 0xC0, 0x00, 0x0B, 8];
        jpg.extend(48u16.to_be_bytes());
        jpg.extend(96u16.to_be_bytes());
        jpg.extend([1, 1, 0x11, 0, 0, 0]);
        std::fs::write(d.path().join("a.jpg"), &jpg).unwrap();
        assert_eq!(image_size(&d.path().join("a.jpg")), Some((96, 48)));
        std::fs::write(d.path().join("t.txt"), b"hello").unwrap();
        assert_eq!(image_size(&d.path().join("t.txt")), None);
    }
}
