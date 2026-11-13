// Copyright 2026 ansiapps. Neumetik Visual: proprietary, see the LICENSE at the repository root.

//! The VGA's 16 colors, and the palettes the visualizations shade with:
//! runs of colors from dark to bright, each step between two colors made
//! smoother by the shade characters (░▒▓) of one over the other.

use crate::canvas::Cell;
use crate::cp437;

/// The VGA's text-mode colors, as Stylus and the ANSIapps theme have them.
pub const VGA: [[u8; 3]; 16] = [
    [0x00, 0x00, 0x00],
    [0x00, 0x00, 0xAA],
    [0x00, 0xAA, 0x00],
    [0x00, 0xAA, 0xAA],
    [0xAA, 0x00, 0x00],
    [0xAA, 0x00, 0xAA],
    [0xAA, 0x55, 0x00],
    [0xAA, 0xAA, 0xAA],
    [0x55, 0x55, 0x55],
    [0x55, 0x55, 0xFF],
    [0x55, 0xFF, 0x55],
    [0x55, 0xFF, 0xFF],
    [0xFF, 0x55, 0x55],
    [0xFF, 0x55, 0xFF],
    [0xFF, 0xFF, 0x55],
    [0xFF, 0xFF, 0xFF],
];

pub const BLACK: u8 = 0;
pub const BLUE: u8 = 1;
pub const GREEN: u8 = 2;
pub const CYAN: u8 = 3;
pub const RED: u8 = 4;
pub const MAGENTA: u8 = 5;
pub const BROWN: u8 = 6;
pub const GRAY: u8 = 7;
pub const DARK_GRAY: u8 = 8;
pub const LIGHT_BLUE: u8 = 9;
pub const LIGHT_GREEN: u8 = 10;
pub const LIGHT_CYAN: u8 = 11;
pub const LIGHT_RED: u8 = 12;
pub const LIGHT_MAGENTA: u8 = 13;
pub const YELLOW: u8 = 14;
pub const WHITE: u8 = 15;

/// The VGA color nearest an RGB color, by a weighted distance (red 2,
/// green 4, blue 3: the eye's weights, near enough).
pub fn nearest(rgb: [u8; 3]) -> u8 {
    let mut best = (u32::MAX, 0u8);
    for (i, c) in VGA.iter().enumerate() {
        let d = |a: u8, b: u8| (i32::from(a) - i32::from(b)).pow(2) as u32;
        let dist = 2 * d(rgb[0], c[0]) + 4 * d(rgb[1], c[1]) + 3 * d(rgb[2], c[2]);
        if dist < best.0 {
            best = (dist, i as u8);
        }
    }
    best.1
}

/// How bright a VGA color looks, 0 to 255.
pub fn luma(c: u8) -> u32 {
    let [r, g, b] = VGA[usize::from(c & 15)];
    (u32::from(r) * 299 + u32::from(g) * 587 + u32::from(b) * 114) / 1000
}

/// A palette: colors from the darkest (the background) to the brightest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ramp {
    pub id: &'static str,
    pub name: &'static str,
    pub colors: Vec<u8>,
}

/// Every palette a visualization can take, by id.
pub const RAMPS: &[(&str, &str, &[u8])] = &[
    ("classic", "Classic (green, yellow, red)", &[0, 2, 10, 14, 12]),
    ("fire", "Fire", &[0, 4, 12, 14, 15]),
    ("ice", "Ice", &[0, 1, 9, 11, 15]),
    ("ocean", "Ocean", &[0, 1, 3, 11, 15]),
    ("acid", "Acid", &[0, 2, 10, 14, 15]),
    ("neon", "Neon", &[0, 5, 13, 11, 15]),
    ("sunset", "Sunset", &[0, 1, 5, 4, 12, 6, 14]),
    ("forest", "Forest", &[0, 2, 6, 10, 14]),
    ("matrix", "Matrix", &[0, 2, 10, 15]),
    ("amber", "Amber", &[0, 6, 14, 15]),
    ("mono", "Gray", &[0, 8, 7, 15]),
    ("rainbow", "Rainbow", &[0, 1, 9, 3, 11, 2, 10, 14, 6, 12, 4, 13, 5]),
    ("ansiapps", "ANSIapps", &[1, 3, 11, 10, 15]),
    ("blood", "Blood", &[0, 8, 4, 12]),
    ("vaporwave", "Vaporwave", &[0, 5, 13, 9, 11]),
    ("c64", "C64", &[1, 9, 11, 15]),
];

impl Ramp {
    pub fn by_id(id: &str) -> Ramp {
        let (id, name, colors) = RAMPS.iter().find(|r| r.0 == id).copied().unwrap_or(RAMPS[0]);
        Ramp { id, name, colors: colors.to_vec() }
    }

    /// A palette of chosen colors, darkest first.
    pub fn custom(mut colors: Vec<u8>) -> Ramp {
        colors.retain(|&c| c < 16);
        colors.dedup();
        if colors.is_empty() {
            colors = vec![0, 7, 15];
        }
        Ramp { id: "custom", name: "Custom", colors }
    }

    pub fn background(&self) -> u8 {
        self.colors[0]
    }

    pub fn brightest(&self) -> u8 {
        *self.colors.last().expect("a ramp has colors")
    }

    /// The color at `t`, 0 (the background) to 1, without shading.
    pub fn color(&self, t: f32) -> u8 {
        let n = self.colors.len();
        let i = (t.clamp(0.0, 1.0) * (n - 1) as f32).round() as usize;
        self.colors[i.min(n - 1)]
    }

    /// The color at `t` among the colors past the background, for lines
    /// and text that must show on it.
    pub fn ink(&self, t: f32) -> u8 {
        let n = self.colors.len();
        if n < 2 {
            return 15;
        }
        let i = 1 + (t.clamp(0.0, 1.0) * (n - 2) as f32).round() as usize;
        self.colors[i.min(n - 1)]
    }

    /// Levels of shade the palette has: four between each two colors.
    pub fn levels(&self) -> usize {
        (self.colors.len() - 1) * 4 + 1
    }

    /// A cell shaded to `t`, 0 to 1: a color, or the next one over it in
    /// ░, ▒ or ▓.
    pub fn shade(&self, t: f32) -> Cell {
        let steps = self.levels() - 1;
        let k = (t.clamp(0.0, 1.0) * steps as f32).round() as usize;
        let (i, part) = (k / 4, k % 4);
        let lo = self.colors[i.min(self.colors.len() - 1)];
        if part == 0 {
            return Cell::new(b' ', lo, lo);
        }
        let hi = self.colors[(i + 1).min(self.colors.len() - 1)];
        let ch = [b' ', cp437::LIGHT, cp437::MEDIUM, cp437::DARK][part];
        Cell::new(ch, hi, lo)
    }
}

/// The two VGA colors in a shade cell, and how much of the brighter,
/// for blending a cell's look (0 to 4 quarters).
pub fn quarters(c: Cell) -> u32 {
    match c.ch {
        cp437::LIGHT => 1,
        cp437::MEDIUM => 2,
        cp437::DARK => 3,
        cp437::FULL => 4,
        _ => 0,
    }
}

/// The color 8 brighter (the bright half of the 16), or itself.
pub fn bright(c: u8) -> u8 {
    if c < 8 && c != 0 {
        c + 8
    } else {
        c
    }
}

/// The color 8 darker, or itself.
pub fn dim(c: u8) -> u8 {
    if c >= 8 {
        c - 8
    } else {
        c
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_vga_colors_are_their_own_nearest() {
        for (i, c) in VGA.iter().enumerate() {
            assert_eq!(usize::from(nearest(*c)), i);
        }
        assert_eq!(nearest([250, 90, 80]), LIGHT_RED);
        assert_eq!(nearest([200, 10, 10]), RED);
    }

    #[test]
    fn a_ramp_shades_from_its_background_to_its_brightest() {
        let r = Ramp::by_id("fire");
        assert_eq!(r.shade(0.0), Cell::new(b' ', 0, 0));
        assert_eq!(r.shade(1.0), Cell::new(b' ', 15, 15));
        assert_eq!(r.shade(1.0 / 16.0 * 2.0), Cell::new(cp437::MEDIUM, 4, 0));
        assert_eq!(r.levels(), 17);
        assert_eq!(Ramp::by_id("nonesuch").id, "classic");
    }
}
