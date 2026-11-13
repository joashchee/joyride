// Copyright 2026 ansiapps. Neumetik Visual: proprietary, see the LICENSE at the repository root.

//! SAUCE (ACiD Productions, 1994), the record at an ANSI art file's end:
//! its title, artist, group and date, its size in characters, whether it
//! has iCE colors, its font, and comment lines. Read and written as
//! Stylus does (one record, written fresh, never a second one appended).
//!
//! ```text
//! data, then 1A (EOF)
//! COMNT, then 64 bytes a comment line    (when there are comments)
//! SAUCE00                                7 bytes
//! title 35, author 20, group 20          CP437, space-padded
//! date 8                                 CCYYMMDD
//! file size                              u32, the data's
//! data type, file type                   u8 ×2
//! type info 1 to 4                       u16 ×4 (character art: width, height)
//! comment lines, flags                   u8 ×2
//! type info string 22                    the font's name, zero-padded
//! ```

use crate::cp437;

const RECORD: usize = 128;
const LINE: usize = 64;

/// Character art (ANSI, ASCII, ANSImation).
pub const CHARACTER: u8 = 1;
/// Binary text (.BIN), its width in the file type.
pub const BINARY_TEXT: u8 = 5;
/// XBin.
pub const XBIN: u8 = 6;
/// Character art's file types.
pub const ASCII: u8 = 0;
pub const ANSI: u8 = 1;
pub const ANSIMATION: u8 = 2;

/// Flags: iCE colors (a bright background, not blinking).
pub const ICE: u8 = 0x01;
/// Letter spacing 8 pixels (the 9th column off).
pub const SPACING_8: u8 = 0x02;

/// A SAUCE record.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Sauce {
    pub title: String,
    pub author: String,
    pub group: String,
    /// `CCYYMMDD`.
    pub date: String,
    pub data_type: u8,
    pub file_type: u8,
    pub info: [u16; 4],
    pub flags: u8,
    /// The font, as SAUCE names them ("IBM VGA").
    pub font: String,
    pub comments: Vec<String>,
}

impl Sauce {
    /// A record for character art `width` by `height`, dated today, in
    /// iCE colors and the IBM VGA font.
    pub fn character(file_type: u8, width: usize, height: usize) -> Sauce {
        Sauce { date: today(), data_type: CHARACTER, file_type, info: [width.min(65535) as u16, height.min(65535) as u16, 0, 0], flags: ICE | SPACING_8, font: "IBM VGA".into(), ..Sauce::default() }
    }

    pub fn ice(&self) -> bool {
        self.flags & ICE != 0
    }

    /// The record and its comments, for after a file's data (`size`
    /// bytes) and its 1A.
    pub fn to_bytes(&self, size: usize) -> Vec<u8> {
        let mut out = Vec::new();
        let comments: Vec<&String> = self.comments.iter().take(255).collect();
        if !comments.is_empty() {
            out.extend_from_slice(b"COMNT");
            for c in &comments {
                out.extend_from_slice(&padded(c, LINE, b' '));
            }
        }
        out.extend_from_slice(b"SAUCE00");
        out.extend_from_slice(&padded(&self.title, 35, b' '));
        out.extend_from_slice(&padded(&self.author, 20, b' '));
        out.extend_from_slice(&padded(&self.group, 20, b' '));
        let date = if self.date.len() == 8 && self.date.bytes().all(|b| b.is_ascii_digit()) { self.date.clone() } else { today() };
        out.extend_from_slice(date.as_bytes());
        out.extend_from_slice(&(size.min(u32::MAX as usize) as u32).to_le_bytes());
        out.push(self.data_type);
        out.push(self.file_type);
        for v in self.info {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.push(comments.len() as u8);
        out.push(self.flags);
        let mut font = cp437::encode(&self.font);
        font.resize(22, 0);
        out.extend_from_slice(&font);
        out
    }

    /// The record at the end of `data`, and how long the data before it
    /// is: the size the record gives, or up to the 1A before it. Text
    /// formats end at their first 1A ([`text_end`]); binary ones may hold
    /// the byte.
    pub fn read(data: &[u8]) -> (Option<Sauce>, usize) {
        let end = data.len();
        if end < RECORD || &data[end - RECORD..end - RECORD + 5] != b"SAUCE" {
            return (None, end);
        }
        let r = &data[end - RECORD..];
        let field = |a: usize, n: usize| text(&r[a..a + n]);
        let u16at = |a: usize| u16::from_le_bytes([r[a], r[a + 1]]);
        let lines = usize::from(r[104]);
        let mut sauce = Sauce {
            title: field(7, 35),
            author: field(42, 20),
            group: field(62, 20),
            date: field(82, 8),
            data_type: r[94],
            file_type: r[95],
            info: [u16at(96), u16at(98), u16at(100), u16at(102)],
            flags: r[105],
            font: text(&r[106..128]),
            comments: Vec::new(),
        };
        let mut before = end - RECORD;
        let block = 5 + lines * LINE;
        if lines > 0 && before >= block && &data[before - block..before - block + 5] == b"COMNT" {
            let at = before - block + 5;
            sauce.comments = (0..lines).map(|i| text(&data[at + i * LINE..at + (i + 1) * LINE])).collect();
            before -= block;
        }
        let size = u32::from_le_bytes([r[90], r[91], r[92], r[93]]) as usize;
        let len = if size > 0 && size < before {
            size
        } else if before > 0 && data[before - 1] == 0x1A {
            before - 1
        } else {
            before
        };
        (Some(sauce), len)
    }
}

/// Where a text format's data ends: at its first 1A, or `end`.
pub fn text_end(data: &[u8], end: usize) -> usize {
    data[..end].iter().position(|&b| b == 0x1A).unwrap_or(end)
}

fn text(b: &[u8]) -> String {
    let b: Vec<u8> = b.iter().copied().take_while(|&c| c != 0).collect();
    cp437::decode(&b).trim_end().to_string()
}

fn padded(s: &str, n: usize, pad: u8) -> Vec<u8> {
    let mut b = cp437::encode(s);
    b.truncate(n);
    b.resize(n, pad);
    b
}

/// Today as `CCYYMMDD` (UTC), by the days-from-civil arithmetic Howard
/// Hinnant published (public domain).
pub fn today() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let z = (secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}{m:02}{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_record_with_comments_reads_back() {
        let mut s = Sauce::character(ANSI, 80, 25);
        s.title = "Cover".into();
        s.author = "someone".into();
        s.comments = vec!["one".into(), "two".into()];
        let mut file = b"art".to_vec();
        file.push(0x1A);
        file.extend(s.to_bytes(3));
        let (back, len) = Sauce::read(&file);
        assert_eq!(len, 3);
        assert_eq!(back, Some(s));
        assert_eq!(today().len(), 8);
    }
}
