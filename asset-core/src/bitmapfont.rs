//! Bitmap fonts drawn as a sample for the preview: BDF, PSF (1 and 2) and
//! BMFont (the text `.fnt` plus the page pictures it names). Written from the
//! formats' public descriptions. PCF, Type 1 and the binary BMFont are named
//! but not drawn (`detect` still says what they are).
//!
//! LÖVE loads a BMFont `.fnt` (and image fonts) itself; BDF is refused (see
//! `kind::love_support`), PSF is a console font LÖVE does not read.

use std::collections::HashMap;
use std::path::{Component, Path};

/// Text drawn under the font's own name.
pub const SAMPLE: [&str; 3] = ["The quick brown fox jumps over", "the lazy dog. 0123456789", "ABCDEFGHIJKLM abcdefghijklm !?"];

const MAX_FILE: u64 = 4 * 1024 * 1024;
const MAX_CELL: u32 = 256;
const MAX_EDGE: u32 = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Bdf,
    Psf1,
    Psf2,
    BmfontText,
    BmfontBinary,
    Pcf,
    Type1,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Bdf => "BDF bitmap font",
            Kind::Psf1 => "PSF 1 console font",
            Kind::Psf2 => "PSF 2 console font",
            Kind::BmfontText => "BMFont (text)",
            Kind::BmfontBinary => "BMFont (binary)",
            Kind::Pcf => "PCF bitmap font",
            Kind::Type1 => "Type 1 font",
        }
    }
}

/// What a file is, by its first bytes.
pub fn detect(b: &[u8]) -> Option<Kind> {
    if b.starts_with(b"STARTFONT") {
        Some(Kind::Bdf)
    } else if b.starts_with(&[0x36, 0x04]) {
        Some(Kind::Psf1)
    } else if b.starts_with(&[0x72, 0xb5, 0x4a, 0x86]) {
        Some(Kind::Psf2)
    } else if b.starts_with(b"info ") {
        Some(Kind::BmfontText)
    } else if b.starts_with(b"BMF") && b.get(3) == Some(&3) {
        Some(Kind::BmfontBinary)
    } else if b.starts_with(b"\x01fcp") {
        Some(Kind::Pcf)
    } else if b.starts_with(b"%!PS-AdobeFont") || b.starts_with(b"%!FontType1") || (b.first() == Some(&0x80) && b.get(1) == Some(&1)) {
        Some(Kind::Type1)
    } else {
        None
    }
}

#[derive(Debug, Clone)]
pub struct Sample {
    pub kind: Kind,
    pub name: Option<String>,
    pub glyphs: usize,
    /// The font's cell (or line) size.
    pub cell: (u32, u32),
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    /// How many times each font pixel was enlarged.
    pub scale: u32,
    /// Said when something the font names is missing (a page picture).
    pub warning: Option<String>,
}

/// One glyph: a 1-bit bitmap placed on a baseline, or a piece of a picture.
struct Glyph {
    w: u32,
    h: u32,
    /// Offset of the glyph's left edge from the pen, and of its bottom edge above the baseline.
    x: i32,
    y: i32,
    advance: i32,
    /// RGBA, `w * h * 4`.
    px: Vec<u8>,
}

struct Font {
    glyphs: HashMap<u32, Glyph>,
    ascent: i32,
    descent: i32,
    cell: (u32, u32),
    name: Option<String>,
    warning: Option<String>,
    /// Draw dark on light, or keep the picture's own colours.
    own_colours: bool,
}

/// Draw `SAMPLE` in the font at `path`. `Ok(None)` for a kind not drawn.
pub fn render(path: &Path) -> Result<Option<Sample>, String> {
    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if meta.len() > MAX_FILE {
        return Err("this font file is too large to draw".into());
    }
    let b = std::fs::read(path).map_err(|e| e.to_string())?;
    let Some(kind) = detect(&b) else { return Ok(None) };
    let font = match kind {
        Kind::Bdf => bdf(&b)?,
        Kind::Psf1 | Kind::Psf2 => psf(&b, kind)?,
        Kind::BmfontText => bmfont(&b, path)?,
        _ => return Ok(None),
    };
    Ok(Some(draw(kind, font)))
}

fn draw(kind: Kind, f: Font) -> Sample {
    let line_h = (f.ascent + f.descent).max(1) as u32 + 1;
    let pad = 4u32;
    let advance_of = |c: char| f.glyphs.get(&(c as u32)).map(|g| g.advance).unwrap_or(f.cell.0 as i32).max(0) as u32;
    let width = SAMPLE.iter().map(|l| l.chars().map(advance_of).sum::<u32>()).max().unwrap_or(1).clamp(1, MAX_EDGE - 2 * pad) + 2 * pad;
    let height = (line_h * SAMPLE.len() as u32 + 2 * pad).min(MAX_EDGE);
    // Dark background, light ink for one-bit fonts; a picture font keeps its colours over the same dark.
    let (bg, ink) = ([16u8, 16, 32, 255], [230u8, 230, 230]);
    let mut rgba = bg.repeat(width as usize * height as usize);
    for (row, line) in SAMPLE.iter().enumerate() {
        let base = pad as i32 + row as i32 * line_h as i32 + f.ascent;
        let mut pen = pad as i32;
        for c in line.chars() {
            if let Some(g) = f.glyphs.get(&(c as u32)) {
                for gy in 0..g.h as i32 {
                    for gx in 0..g.w as i32 {
                        let (px, py) = (pen + g.x + gx, base - g.y - g.h as i32 + gy);
                        if px < 0 || py < 0 || px >= width as i32 || py >= height as i32 {
                            continue;
                        }
                        let s = &g.px[((gy as u32 * g.w + gx as u32) * 4) as usize..][..4];
                        if s[3] == 0 {
                            continue;
                        }
                        let d = &mut rgba[((py as u32 * width + px as u32) * 4) as usize..][..4];
                        let col = if f.own_colours { [s[0], s[1], s[2]] } else { ink };
                        for i in 0..3 {
                            d[i] = ((col[i] as u32 * s[3] as u32 + d[i] as u32 * (255 - s[3] as u32)) / 255) as u8;
                        }
                    }
                }
            }
            pen += advance_of(c) as i32;
        }
    }
    // Whole-number enlargement so a small font is readable (pixels stay square).
    let k = (640 / width).clamp(1, 4);
    if k > 1 {
        let (nw, nh) = (width * k, height * k);
        let mut big = Vec::with_capacity(nw as usize * nh as usize * 4);
        for y in 0..nh {
            for x in 0..nw {
                big.extend_from_slice(&rgba[(((y / k) * width + x / k) * 4) as usize..][..4]);
            }
        }
        return Sample { kind, name: f.name, glyphs: f.glyphs.len(), cell: f.cell, width: nw, height: nh, rgba: big, scale: k, warning: f.warning };
    }
    Sample { kind, name: f.name, glyphs: f.glyphs.len(), cell: f.cell, width, height, rgba, scale: 1, warning: f.warning }
}

fn one_bit(rows: &[Vec<u8>], w: u32) -> Vec<u8> {
    let mut px = Vec::with_capacity(w as usize * rows.len() * 4);
    for r in rows {
        for x in 0..w as usize {
            let on = r.get(x / 8).is_some_and(|b| b & (0x80 >> (x % 8)) != 0);
            px.extend([255, 255, 255, if on { 255 } else { 0 }]);
        }
    }
    px
}

fn bdf(b: &[u8]) -> Result<Font, String> {
    let text = String::from_utf8_lossy(b);
    let mut f = Font { glyphs: HashMap::new(), ascent: 0, descent: 0, cell: (0, 0), name: None, warning: None, own_colours: false };
    let (mut ascent, mut descent) = (None, None);
    let (mut enc, mut bbx, mut dw, mut rows): (Option<i64>, (i32, i32, i32, i32), Option<i32>, Option<Vec<Vec<u8>>>) = (None, (0, 0, 0, 0), None, None);
    let n = |s: &str| s.parse::<i32>().ok();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        let Some(key) = it.next() else { continue };
        let args: Vec<&str> = it.collect();
        if let Some(r) = rows.as_mut() {
            if key == "ENDCHAR" {
                let (w, h) = (bbx.0.clamp(0, MAX_CELL as i32) as u32, bbx.1.clamp(0, MAX_CELL as i32) as u32);
                if let Some(e) = enc.filter(|e| *e >= 0) {
                    let mut rr = std::mem::take(r);
                    rr.resize(h as usize, Vec::new());
                    f.glyphs.insert(e as u32, Glyph { w, h, x: bbx.2, y: bbx.3, advance: dw.unwrap_or(bbx.0), px: one_bit(&rr, w) });
                }
                rows = None;
            } else {
                let hex: Vec<u8> = (0..key.len() / 2).filter_map(|i| u8::from_str_radix(key.get(i * 2..i * 2 + 2)?, 16).ok()).collect();
                r.push(hex);
            }
            continue;
        }
        match key {
            "FONTBOUNDINGBOX" if args.len() >= 4 => {
                f.cell = (n(args[0]).unwrap_or(0).clamp(0, MAX_CELL as i32) as u32, n(args[1]).unwrap_or(0).clamp(0, MAX_CELL as i32) as u32);
                descent = descent.or(n(args[3]).map(|y| -y));
            }
            "FONT_ASCENT" => ascent = args.first().and_then(|a| n(a)),
            "FONT_DESCENT" => descent = args.first().and_then(|a| n(a)),
            "FAMILY_NAME" | "FONT" if f.name.is_none() || key == "FAMILY_NAME" => f.name = Some(args.join(" ").trim_matches('"').to_string()),
            "STARTCHAR" => (enc, bbx, dw) = (None, (0, 0, 0, 0), None),
            "ENCODING" => enc = args.first().and_then(|a| a.parse().ok()),
            "BBX" if args.len() >= 4 => bbx = (n(args[0]).unwrap_or(0), n(args[1]).unwrap_or(0), n(args[2]).unwrap_or(0), n(args[3]).unwrap_or(0)),
            "DWIDTH" => dw = args.first().and_then(|a| n(a)),
            "BITMAP" => rows = Some(Vec::new()),
            _ => {}
        }
    }
    if f.glyphs.is_empty() {
        return Err("no glyphs were found in this BDF".into());
    }
    f.ascent = ascent.unwrap_or(f.cell.1 as i32);
    f.descent = descent.unwrap_or(0).max(0);
    if f.cell.0 == 0 || f.cell.1 == 0 {
        f.cell = (f.glyphs.values().map(|g| g.w).max().unwrap_or(0), f.glyphs.values().map(|g| g.h).max().unwrap_or(0));
    }
    Ok(f)
}

fn psf(b: &[u8], kind: Kind) -> Result<Font, String> {
    let u32at = |o: usize| b.get(o..o + 4).map(|s| u32::from_le_bytes(s.try_into().unwrap()));
    let (head, count, size, h, w) = if kind == Kind::Psf1 {
        let (mode, h) = (*b.get(2).ok_or("cut short")?, *b.get(3).ok_or("cut short")? as u32);
        (4usize, if mode & 1 != 0 { 512u32 } else { 256 }, h, h, 8u32)
    } else {
        (u32at(8).ok_or("cut short")? as usize, u32at(16).ok_or("cut short")?, u32at(20).ok_or("cut short")?, u32at(24).ok_or("cut short")?, u32at(28).ok_or("cut short")?)
    };
    if w == 0 || h == 0 || w > MAX_CELL || h > MAX_CELL || count == 0 || count > 65536 {
        return Err("the font's header is not valid".into());
    }
    let bpr = w.div_ceil(8) as usize;
    if size as usize != bpr * h as usize {
        return Err("the font's glyph size does not match its header".into());
    }
    let mut f = Font { glyphs: HashMap::new(), ascent: h as i32, descent: 0, cell: (w, h), name: None, warning: Some("PSF glyphs are shown in order (glyph 65 is drawn as `A`); the font's own Unicode table is not read.".into()), own_colours: false };
    for i in 0..count.min(512) {
        let o = head + i as usize * size as usize;
        let Some(data) = b.get(o..o + size as usize) else { break };
        let rows: Vec<Vec<u8>> = data.chunks(bpr).map(|r| r.to_vec()).collect();
        f.glyphs.insert(i, Glyph { w, h, x: 0, y: 0, advance: w as i32, px: one_bit(&rows, w) });
    }
    if f.glyphs.is_empty() {
        return Err("the font's glyphs are cut short".into());
    }
    if (f.glyphs.len() as u32) < count.min(512) {
        f.warning = Some(format!("The file ends early: only {} of {} glyphs are there.", f.glyphs.len(), count.min(512)));
    }
    Ok(f)
}

fn kv(line: &str) -> HashMap<&str, &str> {
    let mut m = HashMap::new();
    let mut rest = line;
    while let Some(eq) = rest.find('=') {
        let key = rest[..eq].rsplit(char::is_whitespace).next().unwrap_or("");
        let after = &rest[eq + 1..];
        let (val, next) = if let Some(q) = after.strip_prefix('"') {
            match q.find('"') {
                Some(e) => (&q[..e], &q[e + 1..]),
                None => (q, ""),
            }
        } else {
            match after.find(char::is_whitespace) {
                Some(e) => (&after[..e], &after[e..]),
                None => (after, ""),
            }
        };
        m.insert(key, val);
        rest = next;
    }
    m
}

fn bmfont(b: &[u8], path: &Path) -> Result<Font, String> {
    let text = String::from_utf8_lossy(b);
    let (mut name, mut base, mut line_h) = (None, 0i32, 0i32);
    let mut pages: HashMap<u32, String> = HashMap::new();
    let mut chars: Vec<HashMap<&str, &str>> = Vec::new();
    for l in text.lines() {
        let tag = l.split_whitespace().next().unwrap_or("");
        let m = kv(l);
        match tag {
            "info" => name = m.get("face").map(|s| s.to_string()),
            "common" => {
                base = m.get("base").and_then(|v| v.parse().ok()).unwrap_or(0);
                line_h = m.get("lineHeight").and_then(|v| v.parse().ok()).unwrap_or(0);
            }
            "page" => {
                if let (Some(id), Some(file)) = (m.get("id").and_then(|v| v.parse().ok()), m.get("file")) {
                    pages.insert(id, file.to_string());
                }
            }
            "char" => chars.push(m),
            _ => {}
        }
    }
    let dir = path.parent().unwrap_or(Path::new("."));
    let mut images: HashMap<u32, image::RgbaImage> = HashMap::new();
    let mut missing = Vec::new();
    for (id, file) in &pages {
        let rel = Path::new(file);
        // A page names a picture beside the .fnt (or below it): never above it, never absolute.
        let safe = rel.components().all(|c| matches!(c, Component::Normal(_) | Component::CurDir));
        match safe.then(|| image::open(dir.join(rel)).ok()).flatten() {
            Some(i) => {
                images.insert(*id, i.to_rgba8());
            }
            None => missing.push(file.clone()),
        }
    }
    let int = |m: &HashMap<&str, &str>, k: &str| m.get(k).and_then(|v| v.parse::<i32>().ok()).unwrap_or(0);
    let mut f = Font { glyphs: HashMap::new(), ascent: base, descent: (line_h - base).max(0), cell: (0, line_h.max(0) as u32), name, warning: None, own_colours: true };
    for c in &chars {
        let (id, x, y, w, h) = (int(c, "id"), int(c, "x"), int(c, "y"), int(c, "width"), int(c, "height"));
        let Some(img) = images.get(&(int(c, "page") as u32)) else { continue };
        if id < 0 || w < 0 || h < 0 || w as u32 > MAX_CELL || h as u32 > MAX_CELL {
            continue;
        }
        let mut px = Vec::with_capacity((w * h * 4) as usize);
        for gy in 0..h {
            for gx in 0..w {
                let (sx, sy) = ((x + gx) as u32, (y + gy) as u32);
                px.extend(if sx < img.width() && sy < img.height() { img.get_pixel(sx, sy).0 } else { [0; 4] });
            }
        }
        // Glyph placed `yoffset` below the line's top: its bottom is `base - (yoffset + h)` above the baseline.
        f.glyphs.insert(id as u32, Glyph { w: w as u32, h: h as u32, x: int(c, "xoffset"), y: base - (int(c, "yoffset") + h), advance: int(c, "xadvance"), px });
        f.cell.0 = f.cell.0.max(int(c, "xadvance").max(0) as u32);
    }
    if !missing.is_empty() {
        missing.sort();
        f.warning = Some(format!("The page picture{} {} could not be read, so the glyphs on {} are not drawn.", if missing.len() > 1 { "s" } else { "" }, missing.join(", "), if missing.len() > 1 { "them" } else { "it" }));
    }
    if f.glyphs.is_empty() {
        return Err(f.warning.unwrap_or_else(|| "no glyphs were found in this BMFont".into()));
    }
    Ok(f)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BDF: &str = "STARTFONT 2.1\nFONT -test-Tiny-medium-r-normal--8-80-75-75-c-80-iso10646-1\nFONTBOUNDINGBOX 8 8 0 0\nSTARTPROPERTIES 2\nFONT_ASCENT 8\nFONT_DESCENT 0\nENDPROPERTIES\nCHARS 1\nSTARTCHAR T\nENCODING 84\nSWIDTH 600 0\nDWIDTH 8 0\nBBX 8 2 0 6\nBITMAP\nFF\n18\nENDCHAR\nENDFONT\n";

    #[test]
    fn bdf_draws_its_glyph_on_a_baseline() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("t.bdf"), BDF).unwrap();
        let s = render(&d.path().join("t.bdf")).unwrap().unwrap();
        assert_eq!((s.kind, s.glyphs, s.cell), (Kind::Bdf, 1, (8, 8)));
        // The first `T` in "The" is drawn at the pad (4, 4): a full top row of ink.
        let at = |x: u32, y: u32| &s.rgba[(((y * s.scale) * s.width + x * s.scale) * 4) as usize..][..3];
        assert_eq!(at(4, 4), &[230, 230, 230]);
        assert_eq!(at(11, 4), &[230, 230, 230]);
        assert_eq!(at(4, 5), &[16, 16, 32]); // second row 0x18 is two middle pixels
        assert_eq!(at(7, 5), &[230, 230, 230]);
    }

    #[test]
    fn psf1_and_psf2_glyphs() {
        let d = tempfile::tempdir().unwrap();
        let mut p1 = vec![0x36, 0x04, 0, 2];
        for g in 0..256u32 {
            p1.extend(if g == b'T' as u32 { [0xFF, 0x18] } else { [0, 0] });
        }
        std::fs::write(d.path().join("a.psf"), &p1).unwrap();
        let s = render(&d.path().join("a.psf")).unwrap().unwrap();
        assert_eq!((s.kind, s.cell, s.glyphs), (Kind::Psf1, (8, 2), 256));
        assert!(s.warning.is_some());
        let mut p2 = vec![0x72, 0xb5, 0x4a, 0x86];
        for v in [0u32, 32, 0, 256, 2, 2, 8] {
            p2.extend(v.to_le_bytes());
        }
        for g in 0..256u32 {
            p2.extend(if g == b'T' as u32 { [0xFF, 0x18] } else { [0, 0] });
        }
        std::fs::write(d.path().join("b.psf"), &p2).unwrap();
        assert_eq!(render(&d.path().join("b.psf")).unwrap().unwrap().kind, Kind::Psf2);
        std::fs::write(d.path().join("c.psf"), &p1[..10]).unwrap();
        assert!(render(&d.path().join("c.psf")).unwrap().unwrap().warning.unwrap().contains("ends early"));
        std::fs::write(d.path().join("e.psf"), &p1[..4]).unwrap();
        assert!(render(&d.path().join("e.psf")).is_err(), "no glyphs at all is an error, not a blank");
    }

    #[test]
    fn bmfont_text_uses_its_page_picture_and_names_a_missing_one() {
        let d = tempfile::tempdir().unwrap();
        let mut img = image::RgbaImage::new(8, 8);
        for x in 0..4 {
            for y in 0..4 {
                img.put_pixel(x, y, image::Rgba([255, 0, 0, 255]));
            }
        }
        img.save(d.path().join("page.png")).unwrap();
        let fnt = "info face=\"Tiny Face\" size=8\ncommon lineHeight=8 base=6 scaleW=8 scaleH=8 pages=1\npage id=0 file=\"page.png\"\nchars count=1\nchar id=84 x=0 y=0 width=4 height=4 xoffset=0 yoffset=2 xadvance=5 page=0\n";
        std::fs::write(d.path().join("t.fnt"), fnt).unwrap();
        let s = render(&d.path().join("t.fnt")).unwrap().unwrap();
        assert_eq!((s.kind, s.name.as_deref(), s.glyphs), (Kind::BmfontText, Some("Tiny Face"), 1));
        assert!(s.rgba.chunks(4).any(|p| p[..3] == [255, 0, 0]), "the page's red shows");
        // A page that is missing, or climbs out of the folder, is named and not read.
        std::fs::write(d.path().join("m.fnt"), fnt.replace("page.png", "../page.png")).unwrap();
        let e = render(&d.path().join("m.fnt")).unwrap_err();
        assert!(e.contains("../page.png"), "{e}");
    }

    #[test]
    fn other_kinds_are_named_not_drawn() {
        assert_eq!(detect(b"\x01fcp\0\0\0\0"), Some(Kind::Pcf));
        assert_eq!(detect(b"%!PS-AdobeFont-1.0: X"), Some(Kind::Type1));
        assert_eq!(detect(b"BMF\x03"), Some(Kind::BmfontBinary));
        assert_eq!(detect(b"hello"), None);
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("x.pcf"), b"\x01fcp\0\0\0\0").unwrap();
        assert!(render(&d.path().join("x.pcf")).unwrap().is_none());
    }
}
