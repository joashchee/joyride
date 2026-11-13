// Copyright 2026 ansiapps. Neumetik Visual: proprietary, see the LICENSE at the repository root.

//! A small block font, three pixels by five, drawn in half-block pixels:
//! titles in big letters, four cells a letter and three rows high (or
//! twice that, scaled). Drawn for Neumetik.

use crate::canvas::Pixels;

/// Each glyph's five rows, `#` a pixel.
const GLYPHS: &[(char, [&str; 5])] = &[
    ('A', [".#.", "#.#", "###", "#.#", "#.#"]),
    ('B', ["##.", "#.#", "##.", "#.#", "##."]),
    ('C', [".##", "#..", "#..", "#..", ".##"]),
    ('D', ["##.", "#.#", "#.#", "#.#", "##."]),
    ('E', ["###", "#..", "##.", "#..", "###"]),
    ('F', ["###", "#..", "##.", "#..", "#.."]),
    ('G', [".##", "#..", "#.#", "#.#", ".##"]),
    ('H', ["#.#", "#.#", "###", "#.#", "#.#"]),
    ('I', ["###", ".#.", ".#.", ".#.", "###"]),
    ('J', ["..#", "..#", "..#", "#.#", ".#."]),
    ('K', ["#.#", "#.#", "##.", "#.#", "#.#"]),
    ('L', ["#..", "#..", "#..", "#..", "###"]),
    ('M', ["#.#", "###", "###", "#.#", "#.#"]),
    ('N', ["##.", "#.#", "#.#", "#.#", "#.#"]),
    ('O', [".#.", "#.#", "#.#", "#.#", ".#."]),
    ('P', ["##.", "#.#", "##.", "#..", "#.."]),
    ('Q', [".#.", "#.#", "#.#", "##.", ".##"]),
    ('R', ["##.", "#.#", "##.", "#.#", "#.#"]),
    ('S', [".##", "#..", ".#.", "..#", "##."]),
    ('T', ["###", ".#.", ".#.", ".#.", ".#."]),
    ('U', ["#.#", "#.#", "#.#", "#.#", "###"]),
    ('V', ["#.#", "#.#", "#.#", "#.#", ".#."]),
    ('W', ["#.#", "#.#", "###", "###", "#.#"]),
    ('X', ["#.#", "#.#", ".#.", "#.#", "#.#"]),
    ('Y', ["#.#", "#.#", ".#.", ".#.", ".#."]),
    ('Z', ["###", "..#", ".#.", "#..", "###"]),
    ('0', ["###", "#.#", "#.#", "#.#", "###"]),
    ('1', [".#.", "##.", ".#.", ".#.", "###"]),
    ('2', ["##.", "..#", ".#.", "#..", "###"]),
    ('3', ["##.", "..#", ".#.", "..#", "##."]),
    ('4', ["#.#", "#.#", "###", "..#", "..#"]),
    ('5', ["###", "#..", "##.", "..#", "##."]),
    ('6', [".##", "#..", "###", "#.#", "###"]),
    ('7', ["###", "..#", ".#.", ".#.", ".#."]),
    ('8', ["###", "#.#", "###", "#.#", "###"]),
    ('9', ["###", "#.#", "###", "..#", "##."]),
    (' ', ["...", "...", "...", "...", "..."]),
    ('.', ["...", "...", "...", "...", ".#."]),
    (',', ["...", "...", "...", ".#.", "#.."]),
    ('!', [".#.", ".#.", ".#.", "...", ".#."]),
    ('?', ["##.", "..#", ".#.", "...", ".#."]),
    ('-', ["...", "...", "###", "...", "..."]),
    ('_', ["...", "...", "...", "...", "###"]),
    ('\'', [".#.", ".#.", "...", "...", "..."]),
    ('"', ["#.#", "#.#", "...", "...", "..."]),
    (':', ["...", ".#.", "...", ".#.", "..."]),
    ('/', ["..#", "..#", ".#.", "#..", "#.."]),
    ('(', [".#.", "#..", "#..", "#..", ".#."]),
    (')', [".#.", "..#", "..#", "..#", ".#."]),
    ('+', ["...", ".#.", "###", ".#.", "..."]),
    ('=', ["...", "###", "...", "###", "..."]),
    ('#', ["#.#", "###", "#.#", "###", "#.#"]),
    ('&', [".#.", "#.#", ".#.", "#.#", ".##"]),
    ('*', ["#.#", ".#.", "#.#", "...", "..."]),
];

fn glyph(c: char) -> &'static [&'static str; 5] {
    let c = fold(c);
    GLYPHS.iter().find(|g| g.0 == c).map_or(&GLYPHS[40].1, |g| &g.1)
}

/// A letter as the font has it: upper case, accents taken off the common
/// ones, anything else a question mark.
fn fold(c: char) -> char {
    let c = c.to_ascii_uppercase();
    match c {
        'À'..='Å' | 'à'..='å' => 'A',
        'Ç' | 'ç' => 'C',
        'È'..='Ë' | 'è'..='ë' => 'E',
        'Ì'..='Ï' | 'ì'..='ï' => 'I',
        'Ñ' | 'ñ' => 'N',
        'Ò'..='Ö' | 'ò'..='ö' | 'Ø' | 'ø' => 'O',
        'Ù'..='Ü' | 'ù'..='ü' => 'U',
        'ß' => 'S',
        c => c,
    }
}

/// How many pixels wide text is at a scale (a pixel's gap between letters).
pub fn width(text: &str, scale: i32) -> i32 {
    let n = text.chars().count() as i32;
    if n == 0 {
        0
    } else {
        n * 4 * scale - scale
    }
}

/// Pixels tall at a scale.
pub fn height(scale: i32) -> i32 {
    5 * scale
}

/// Draws text with its top left at (x, y) pixels, each font pixel
/// `scale` × `scale`, in `color`; `shadow`, when some, a font pixel down
/// and right.
pub fn draw(p: &mut Pixels, x: i32, y: i32, text: &str, scale: i32, color: u8, shadow: Option<u8>) {
    let scale = scale.max(1);
    for pass in 0..2 {
        let (dx, c) = match (pass, shadow) {
            (0, Some(s)) => (scale, s),
            (0, None) => continue,
            _ => (0, color),
        };
        let mut cx = x;
        for ch in text.chars() {
            for (row, bits) in glyph(ch).iter().enumerate() {
                for (col, b) in bits.bytes().enumerate() {
                    if b == b'#' {
                        p.rect(cx + col as i32 * scale + dx, y + row as i32 * scale + dx, scale, scale, c);
                    }
                }
            }
            cx += 4 * scale;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_glyph_is_three_by_five_and_text_measures_right() {
        for (c, rows) in GLYPHS {
            assert!(rows.iter().all(|r| r.len() == 3), "{c}");
        }
        assert_eq!(GLYPHS[40].0, '?');
        assert_eq!(width("AB", 1), 7);
        let mut p = Pixels::for_cells(8, 3);
        draw(&mut p, 0, 0, "é", 1, 15, None);
        assert_eq!(p.get(0, 0), 15);
        assert_eq!(p.get(1, 0), 15);
    }
}
