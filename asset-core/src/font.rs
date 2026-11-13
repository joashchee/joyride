//! Font facts read from the file itself: format, family and style, version,
//! glyph count, the designer, the copyright and the licence the font carries
//! in its own `name` table, and whether it may be embedded in a game (the
//! `OS/2` embedding flags). Pure Rust over the bytes, so it runs wherever the
//! catalog does.
//!
//! What LÖVE loads was measured on the real 11.5 and 12.0 binaries
//! (`love.font.newRasterizer`, 2026-11-13): TrueType, OpenType and TrueType
//! Collection files and WOFF all load, on both. WOFF2, Type 1, BDF and PCF
//! were not measured (no sample to try), so they are reported as unknown, never
//! as working.

use crate::licence;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    TrueType,
    OpenTypeCff,
    Collection,
    Woff,
    Woff2,
}

impl Format {
    pub fn name(self) -> &'static str {
        match self {
            Format::TrueType => "TrueType",
            Format::OpenTypeCff => "OpenType (CFF)",
            Format::Collection => "TrueType Collection",
            Format::Woff => "WOFF",
            Format::Woff2 => "WOFF2",
        }
    }
}

/// Whether LÖVE's `love.graphics.newFont` loads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Love {
    /// Measured on the real LÖVE 11.5 and 12.0.
    Loads,
    /// Not measured: no sample to try.
    Unknown,
}

/// What the font's `OS/2` embedding flags allow (`fsType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Embedding {
    Installable,
    /// Bit 1: the font must not be embedded; a game ships it, so this blocks use.
    Restricted,
    PreviewAndPrint,
    Editable,
}

impl Embedding {
    pub fn describe(self) -> &'static str {
        match self {
            Embedding::Installable => "may be embedded",
            Embedding::Restricted => "embedding is restricted by the font's own flags",
            Embedding::PreviewAndPrint => "may be embedded for preview and print only",
            Embedding::Editable => "may be embedded for editing documents",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FontFacts {
    pub format: Format,
    pub love: Love,
    /// Number of fonts in a collection (1 for a single font).
    pub fonts: u32,
    pub family: String,
    pub style: String,
    pub full_name: String,
    pub version: String,
    pub copyright: String,
    pub designer: String,
    /// The licence description (name ID 13) and URL (name ID 14).
    pub licence_text: String,
    pub licence_url: String,
    pub glyphs: Option<u32>,
    pub embedding: Option<Embedding>,
}

impl FontFacts {
    /// An SPDX identifier guessed from the licence text, then its URL, then the
    /// copyright line. Only a guess: the person confirms it.
    pub fn licence_guess(&self) -> Option<String> {
        [&self.licence_text, &self.licence_url, &self.copyright].iter().filter(|t| !t.trim().is_empty()).find_map(|t| licence::guess_from_text(t))
    }
}

fn be16(b: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*b.get(o)?, *b.get(o + 1)?]))
}
fn be32(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_be_bytes([*b.get(o)?, *b.get(o + 1)?, *b.get(o + 2)?, *b.get(o + 3)?]))
}

/// Reads a font's facts; `None` when the bytes are not a font this reads.
pub fn probe(data: &[u8]) -> Option<FontFacts> {
    let magic = data.get(..4)?;
    let (format, fonts, base) = match magic {
        b"wOFF" => (Format::Woff, 1, 0),
        b"wOF2" => (Format::Woff2, 1, 0),
        b"ttcf" => (Format::Collection, be32(data, 8)?, be32(data, 12)? as usize),
        b"OTTO" => (Format::OpenTypeCff, 1, 0),
        [0, 1, 0, 0] | b"true" => (Format::TrueType, 1, 0),
        _ => return None,
    };
    let mut f = FontFacts {
        format,
        love: if matches!(format, Format::Woff2) { Love::Unknown } else { Love::Loads },
        fonts,
        family: String::new(),
        style: String::new(),
        full_name: String::new(),
        version: String::new(),
        copyright: String::new(),
        designer: String::new(),
        licence_text: String::new(),
        licence_url: String::new(),
        glyphs: None,
        embedding: None,
    };
    // WOFF compresses its tables and WOFF2 even more so: only the format is read.
    if matches!(format, Format::Woff | Format::Woff2) {
        return Some(f);
    }
    let tables = be16(data, base + 4)? as usize;
    let table = |tag: &[u8; 4]| -> Option<&[u8]> {
        for i in 0..tables {
            let r = base + 12 + i * 16;
            if data.get(r..r + 4)? == tag {
                let (off, len) = (be32(data, r + 8)? as usize, be32(data, r + 12)? as usize);
                return data.get(off..off.checked_add(len)?);
            }
        }
        None
    };
    if let Some(t) = table(b"name") {
        read_names(t, &mut f);
    }
    f.glyphs = table(b"maxp").and_then(|t| be16(t, 4)).map(u32::from);
    f.embedding = table(b"OS/2").and_then(|t| be16(t, 8)).map(|fs| {
        // The lowest set restriction wins, as a reader of fsType takes it.
        if fs & 0x0002 != 0 {
            Embedding::Restricted
        } else if fs & 0x0004 != 0 {
            Embedding::PreviewAndPrint
        } else if fs & 0x0008 != 0 {
            Embedding::Editable
        } else {
            Embedding::Installable
        }
    });
    Some(f)
}

fn read_names(t: &[u8], f: &mut FontFacts) {
    let (Some(count), Some(strings)) = (be16(t, 2), be16(t, 4)) else { return };
    // The best record for each name ID: Windows English, any Windows, Unicode, then Mac Roman.
    let mut best: [Option<(u8, String)>; 18] = Default::default();
    for i in 0..count as usize {
        let r = 6 + i * 12;
        let (Some(plat), Some(enc), Some(lang), Some(id), Some(len), Some(off)) = (be16(t, r), be16(t, r + 2), be16(t, r + 4), be16(t, r + 6), be16(t, r + 8), be16(t, r + 10)) else { return };
        let id = id as usize;
        if id >= best.len() {
            continue;
        }
        let Some(raw) = t.get(strings as usize + off as usize..strings as usize + off as usize + len as usize) else { continue };
        let (rank, text) = match (plat, enc) {
            (3, 1) | (3, 10) => (if lang == 0x409 { 4 } else { 3 }, utf16(raw)),
            (0, _) => (2, utf16(raw)),
            (1, 0) => (1, raw.iter().map(|&b| b as char).collect()),
            _ => continue,
        };
        if text.trim().is_empty() {
            continue;
        }
        if best[id].as_ref().is_none_or(|(r0, _)| rank > *r0) {
            best[id] = Some((rank, text));
        }
    }
    let get = |id: usize| best[id].as_ref().map(|(_, s)| s.trim().to_string()).unwrap_or_default();
    let pick = |a: usize, b: usize| {
        let s = get(a);
        if s.is_empty() {
            get(b)
        } else {
            s
        }
    };
    f.copyright = get(0);
    // The typographic names (16, 17) are the family and style a person knows.
    f.family = pick(16, 1);
    f.style = pick(17, 2);
    f.full_name = get(4);
    f.version = get(5);
    f.designer = get(9);
    f.licence_text = get(13);
    f.licence_url = get(14);
}

fn utf16(b: &[u8]) -> String {
    let units: Vec<u16> = b.as_chunks::<2>().0.iter().map(|c| u16::from_be_bytes(*c)).collect();
    String::from_utf16_lossy(&units)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A TrueType file that is only a table directory with `name`, `maxp` and `OS/2`.
    pub fn tiny_ttf(names: &[(u16, &str)], glyphs: u16, fs_type: u16) -> Vec<u8> {
        let count = names.len() as u16;
        let mut head = vec![0u8, 0];
        head.extend(count.to_be_bytes());
        head.extend((6 + 12 * count).to_be_bytes());
        let (mut recs, mut pool) = (Vec::new(), Vec::new());
        for (id, s) in names {
            let u: Vec<u8> = s.encode_utf16().flat_map(|c| c.to_be_bytes()).collect();
            for v in [3u16, 1, 0x409, *id, u.len() as u16, pool.len() as u16] {
                recs.extend(v.to_be_bytes());
            }
            pool.extend(u);
        }
        let name_tbl = [head, recs, pool].concat();
        let mut maxp = vec![0, 0, 0x50, 0, 0, 0];
        maxp[4..6].copy_from_slice(&glyphs.to_be_bytes());
        let mut os2 = vec![0u8; 10];
        os2[8..10].copy_from_slice(&fs_type.to_be_bytes());
        let tables: [(&[u8; 4], &Vec<u8>); 3] = [(b"OS/2", &os2), (b"maxp", &maxp), (b"name", &name_tbl)];
        let mut out = vec![0, 1, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0];
        let mut off = 12 + 16 * 3;
        let mut body: Vec<u8> = Vec::new();
        for (tag, t) in tables {
            out.extend(tag.iter());
            out.extend([0u8; 4]);
            out.extend((off as u32).to_be_bytes());
            out.extend((t.len() as u32).to_be_bytes());
            body.extend(t.iter());
            off += t.len();
        }
        out.extend(body);
        out
    }

    #[test]
    fn reads_names_glyphs_and_embedding() {
        let ttf = tiny_ttf(
            &[
                (0, "Copyright 2020 The Example Authors"),
                (1, "Example Sans"),
                (2, "Bold"),
                (4, "Example Sans Bold"),
                (5, "Version 1.002"),
                (9, "Ann Designer"),
                (13, "This Font Software is licensed under the SIL Open Font License, Version 1.1."),
                (14, "https://openfontlicense.org"),
            ],
            512,
            0,
        );
        let f = probe(&ttf).unwrap();
        assert_eq!((f.format, f.love, f.glyphs, f.embedding), (Format::TrueType, Love::Loads, Some(512), Some(Embedding::Installable)));
        assert_eq!((f.family.as_str(), f.style.as_str(), f.version.as_str(), f.designer.as_str()), ("Example Sans", "Bold", "Version 1.002", "Ann Designer"));
        assert_eq!(f.licence_url, "https://openfontlicense.org");
        assert_eq!(f.licence_guess().as_deref(), Some("OFL-1.1"));
    }

    #[test]
    fn a_restricted_font_says_so() {
        let f = probe(&tiny_ttf(&[(1, "Locked")], 10, 0x0002)).unwrap();
        assert_eq!(f.embedding, Some(Embedding::Restricted));
        assert!(f.licence_guess().is_none());
    }

    #[test]
    fn typographic_names_win_over_the_legacy_ones() {
        let f = probe(&tiny_ttf(&[(1, "Example Sans Bold"), (2, "Regular"), (16, "Example Sans"), (17, "Bold")], 1, 0)).unwrap();
        assert_eq!((f.family.as_str(), f.style.as_str()), ("Example Sans", "Bold"));
    }

    #[test]
    fn woff_and_woff2_report_only_their_format() {
        let f = probe(b"wOFF\0\x01\0\0rest").unwrap();
        assert_eq!((f.format, f.love, f.glyphs), (Format::Woff, Love::Loads, None));
        let g = probe(b"wOF2\0\x01\0\0rest").unwrap();
        assert_eq!((g.format, g.love), (Format::Woff2, Love::Unknown));
    }

    #[test]
    fn not_a_font_and_truncated_files_are_refused_without_panicking() {
        assert!(probe(b"hello there").is_none());
        assert!(probe(b"").is_none());
        let ttf = tiny_ttf(&[(1, "X")], 1, 0);
        for n in [4, 12, 30, 60, ttf.len() - 3] {
            let _ = probe(&ttf[..n]); // must not panic
        }
    }

    #[test]
    fn reads_a_real_system_font_when_there_is_one() {
        let Ok(b) = std::fs::read("/System/Library/Fonts/Supplemental/Arial.ttf") else { return };
        let f = probe(&b).unwrap();
        assert_eq!(f.family, "Arial");
        assert!(f.glyphs.unwrap() > 1000);
        assert!(f.fonts == 1 && !f.copyright.is_empty());
    }
}
