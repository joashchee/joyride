// Copyright 2026 ansiapps. Neumetik Visual: proprietary, see the LICENSE at the repository root.

//! Code page 437, the IBM PC's characters, which ANSI art is drawn in:
//! each byte's Unicode character as the VGA shows it (the control codes'
//! glyphs too), and back.

/// Every byte's character, 0x00 to 0xFF.
pub const TABLE: [char; 256] = [
    ' ', '☺', '☻', '♥', '♦', '♣', '♠', '•', '◘', '○', '◙', '♂', '♀', '♪', '♫', '☼', //
    '►', '◄', '↕', '‼', '¶', '§', '▬', '↨', '↑', '↓', '→', '←', '∟', '↔', '▲', '▼', //
    ' ', '!', '"', '#', '$', '%', '&', '\'', '(', ')', '*', '+', ',', '-', '.', '/', //
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', ':', ';', '<', '=', '>', '?', //
    '@', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', //
    'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', '[', '\\', ']', '^', '_', //
    '`', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', //
    'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z', '{', '|', '}', '~', '⌂', //
    'Ç', 'ü', 'é', 'â', 'ä', 'à', 'å', 'ç', 'ê', 'ë', 'è', 'ï', 'î', 'ì', 'Ä', 'Å', //
    'É', 'æ', 'Æ', 'ô', 'ö', 'ò', 'û', 'ù', 'ÿ', 'Ö', 'Ü', '¢', '£', '¥', '₧', 'ƒ', //
    'á', 'í', 'ó', 'ú', 'ñ', 'Ñ', 'ª', 'º', '¿', '⌐', '¬', '½', '¼', '¡', '«', '»', //
    '░', '▒', '▓', '│', '┤', '╡', '╢', '╖', '╕', '╣', '║', '╗', '╝', '╜', '╛', '┐', //
    '└', '┴', '┬', '├', '─', '┼', '╞', '╟', '╚', '╔', '╩', '╦', '╠', '═', '╬', '╧', //
    '╨', '╤', '╥', '╙', '╘', '╒', '╓', '╫', '╪', '┘', '┌', '█', '▄', '▌', '▐', '▀', //
    'α', 'ß', 'Γ', 'π', 'Σ', 'σ', 'µ', 'τ', 'Φ', 'Θ', 'Ω', 'δ', '∞', 'φ', 'ε', '∩', //
    '≡', '±', '≥', '≤', '⌠', '⌡', '÷', '≈', '°', '∙', '·', '√', 'ⁿ', '²', '■', '\u{a0}', //
];

/// The byte for a character, if code page 437 has it. Printable ASCII is
/// itself; a space is 0x20, never 0x00 or 0xFF.
pub fn from_char(c: char) -> Option<u8> {
    if (' '..='~').contains(&c) {
        return Some(c as u8);
    }
    // Some characters have two common spellings.
    let c = match c {
        'β' => 'ß',
        '\u{2126}' => '\u{3a9}',
        'μ' => 'µ',
        '∑' => 'Σ',
        '∈' | '€' => 'ε',
        '⎮' => '│',
        _ => c,
    };
    TABLE.iter().skip(1).position(|&t| t == c).map(|i| (i + 1) as u8).filter(|&b| b != 0x20)
}

/// Text as code page 437 bytes; a character it hasn't becomes `?`.
pub fn encode(s: &str) -> Vec<u8> {
    s.chars().map(|c| from_char(c).unwrap_or(b'?')).collect()
}

/// Code page 437 bytes as text.
pub fn decode(b: &[u8]) -> String {
    b.iter().map(|&b| TABLE[usize::from(b)]).collect()
}

/// The block and shade characters the visualizations draw with.
pub const LIGHT: u8 = 0xB0;
pub const MEDIUM: u8 = 0xB1;
pub const DARK: u8 = 0xB2;
pub const FULL: u8 = 0xDB;
pub const LOWER: u8 = 0xDC;
pub const LEFT: u8 = 0xDD;
pub const RIGHT: u8 = 0xDE;
pub const UPPER: u8 = 0xDF;
pub const SQUARE: u8 = 0xFE;
pub const DOT: u8 = 0xFA;
pub const BULLET: u8 = 0xF9;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_byte_but_the_spaces_comes_back_from_its_character() {
        for b in 1..=255u8 {
            if b == 0x20 || b == 0xFF {
                continue;
            }
            assert_eq!(from_char(TABLE[usize::from(b)]), Some(b), "{b:#04x}");
        }
        assert_eq!(decode(&encode("Hello ▀▄█░")), "Hello ▀▄█░");
    }
}
