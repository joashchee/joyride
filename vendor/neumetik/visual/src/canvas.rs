// Copyright 2026 ansiapps. Neumetik Visual: proprietary, see the LICENSE at the repository root.

//! The screen a visualization draws on: a grid of cells, each a code page
//! 437 character in one of the VGA's 16 colors on another, as ANSI art is.
//! Backgrounds take all 16 colors (iCE colors, as Stylus shows them), so
//! a cell's two halves can be any two colors: [`Pixels`] draws in those
//! halves, two to a cell.

use crate::cp437;

/// One character cell: a code page 437 byte, its color and its
/// background's (VGA colors 0 to 15).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Cell {
    pub ch: u8,
    pub fg: u8,
    pub bg: u8,
}

impl Cell {
    /// A space, light gray on black: what an ANSI screen starts as.
    pub const BLANK: Cell = Cell { ch: b' ', fg: 7, bg: 0 };

    pub const fn new(ch: u8, fg: u8, bg: u8) -> Cell {
        Cell { ch, fg: fg & 15, bg: bg & 15 }
    }

    /// The PC's attribute byte: background in the high nibble (its top bit
    /// a bright background, iCE), color in the low.
    pub fn attr(self) -> u8 {
        (self.bg & 15) << 4 | (self.fg & 15)
    }

    pub fn from_attr(ch: u8, attr: u8) -> Cell {
        Cell { ch, fg: attr & 15, bg: attr >> 4 }
    }

    /// The color seen most: the background for a space, the character's
    /// color for a full block.
    pub fn looks(self) -> u8 {
        match self.ch {
            b' ' | 0 | 0xFF => self.bg,
            cp437::FULL => self.fg,
            _ => self.fg,
        }
    }
}

impl Default for Cell {
    fn default() -> Cell {
        Cell::BLANK
    }
}

/// Characters by cells, row after row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Canvas {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<Cell>,
}

/// The box-drawing sets: corners (top left, top right, bottom left,
/// bottom right), then the horizontal and vertical lines.
pub const SINGLE: [u8; 6] = [0xDA, 0xBF, 0xC0, 0xD9, 0xC4, 0xB3];
pub const DOUBLE: [u8; 6] = [0xC9, 0xBB, 0xC8, 0xBC, 0xCD, 0xBA];

impl Canvas {
    pub fn new(width: usize, height: usize) -> Canvas {
        let (width, height) = (width.max(1), height.max(1));
        Canvas { width, height, cells: vec![Cell::BLANK; width * height] }
    }

    /// A new size, cleared.
    pub fn resize(&mut self, width: usize, height: usize) {
        let (width, height) = (width.max(1), height.max(1));
        if width != self.width || height != self.height {
            *self = Canvas::new(width, height);
        }
    }

    pub fn clear(&mut self, bg: u8) {
        self.cells.fill(Cell::new(b' ', 7, bg));
    }

    pub fn get(&self, x: i32, y: i32) -> Option<Cell> {
        self.index(x, y).map(|i| self.cells[i])
    }

    fn index(&self, x: i32, y: i32) -> Option<usize> {
        (x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height).then(|| y as usize * self.width + x as usize)
    }

    /// Sets a cell; off the canvas, nothing.
    pub fn put(&mut self, x: i32, y: i32, cell: Cell) {
        if let Some(i) = self.index(x, y) {
            self.cells[i] = cell;
        }
    }

    /// Changes a cell in place.
    pub fn with(&mut self, x: i32, y: i32, f: impl FnOnce(&mut Cell)) {
        if let Some(i) = self.index(x, y) {
            f(&mut self.cells[i]);
        }
    }

    /// Text from `x`, in code page 437 (what it hasn't is `?`); returns
    /// the column after it.
    pub fn text(&mut self, x: i32, y: i32, s: &str, fg: u8, bg: u8) -> i32 {
        let mut cx = x;
        for b in cp437::encode(s) {
            self.put(cx, y, Cell::new(b, fg, bg));
            cx += 1;
        }
        cx
    }

    /// Text over what's there, keeping each cell's background.
    pub fn text_over(&mut self, x: i32, y: i32, s: &str, fg: u8) {
        for (i, b) in cp437::encode(s).into_iter().enumerate() {
            self.with(x + i as i32, y, |c| {
                c.ch = b;
                c.fg = fg;
            });
        }
    }

    /// Text centered on the row, cut to fit.
    pub fn center(&mut self, y: i32, s: &str, fg: u8, bg: u8) {
        let s = fit(s, self.width);
        let x = (self.width as i32 - s.chars().count() as i32) / 2;
        self.text(x, y, &s, fg, bg);
    }

    pub fn fill(&mut self, x: i32, y: i32, w: i32, h: i32, cell: Cell) {
        for yy in y.max(0)..(y + h).min(self.height as i32) {
            for xx in x.max(0)..(x + w).min(self.width as i32) {
                self.put(xx, yy, cell);
            }
        }
    }

    /// A box's outline in a box-drawing set ([`SINGLE`], [`DOUBLE`]).
    pub fn frame(&mut self, x: i32, y: i32, w: i32, h: i32, set: [u8; 6], fg: u8, bg: u8) {
        if w < 2 || h < 2 {
            return;
        }
        for xx in x + 1..x + w - 1 {
            self.put(xx, y, Cell::new(set[4], fg, bg));
            self.put(xx, y + h - 1, Cell::new(set[4], fg, bg));
        }
        for yy in y + 1..y + h - 1 {
            self.put(x, yy, Cell::new(set[5], fg, bg));
            self.put(x + w - 1, yy, Cell::new(set[5], fg, bg));
        }
        self.put(x, y, Cell::new(set[0], fg, bg));
        self.put(x + w - 1, y, Cell::new(set[1], fg, bg));
        self.put(x, y + h - 1, Cell::new(set[2], fg, bg));
        self.put(x + w - 1, y + h - 1, Cell::new(set[3], fg, bg));
    }

    /// Another canvas drawn at (x, y); `clear` cells (a space on black
    /// when it's Some(0)) let what's under them show.
    pub fn blit(&mut self, src: &Canvas, x: i32, y: i32, clear: Option<u8>) {
        for sy in 0..src.height {
            for sx in 0..src.width {
                let c = src.cells[sy * src.width + sx];
                if clear.is_some_and(|bg| c.ch == b' ' && c.bg == bg) {
                    continue;
                }
                self.put(x + sx as i32, y + sy as i32, c);
            }
        }
    }

    /// Another canvas scaled to fill this one, cell for nearest cell.
    pub fn blit_scaled(&mut self, src: &Canvas) {
        for y in 0..self.height {
            let sy = y * src.height / self.height;
            for x in 0..self.width {
                let sx = x * src.width / self.width;
                self.cells[y * self.width + x] = src.cells[sy * src.width + sx];
            }
        }
    }

    /// Each cell as two bytes, its character and its attribute
    /// ([`Cell::attr`]): how the app's screen gets a frame.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(4 + self.cells.len() * 2);
        out.extend_from_slice(&(self.width as u16).to_le_bytes());
        out.extend_from_slice(&(self.height as u16).to_le_bytes());
        for c in &self.cells {
            out.push(c.ch);
            out.push(c.attr());
        }
        out
    }

    /// The canvas as lines of text, for tests and the terminal.
    pub fn to_text(&self) -> String {
        let mut s = String::new();
        for row in self.cells.chunks(self.width) {
            s.extend(row.iter().map(|c| cp437::TABLE[usize::from(c.ch)]));
            s.push('\n');
        }
        s
    }

    /// Whether every cell is the same as the first: nothing drawn.
    pub fn is_plain(&self) -> bool {
        let first = self.cells[0];
        self.cells.iter().all(|&c| c == first)
    }
}

/// `s` cut to `n` characters, with an ellipsis when it was longer.
pub fn fit(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        return s.to_string();
    }
    let mut out: String = s.chars().take(n.saturating_sub(1)).collect();
    // An ellipsis isn't code page 437, and three dots are too long: a
    // middle dot stands for it.
    if n > 0 {
        out.push('·');
    }
    out
}

/// Pixels two to a cell, top and bottom halves (`▀` in one color on
/// another): twice the rows, each pixel a VGA color or none.
#[derive(Clone, Debug)]
pub struct Pixels {
    pub width: usize,
    pub height: usize,
    pub px: Vec<u8>,
}

/// A pixel with nothing in it: the cell under it shows.
pub const NONE: u8 = 255;

impl Pixels {
    /// Pixels for a canvas `cols` by `rows` cells: `rows × 2` tall.
    pub fn for_cells(cols: usize, rows: usize) -> Pixels {
        Pixels { width: cols.max(1), height: rows.max(1) * 2, px: vec![NONE; cols.max(1) * rows.max(1) * 2] }
    }

    pub fn resize(&mut self, cols: usize, rows: usize) {
        if self.width != cols.max(1) || self.height != rows.max(1) * 2 {
            *self = Pixels::for_cells(cols, rows);
        }
    }

    pub fn clear(&mut self, c: u8) {
        self.px.fill(c);
    }

    pub fn get(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return NONE;
        }
        self.px[y as usize * self.width + x as usize]
    }

    pub fn set(&mut self, x: i32, y: i32, c: u8) {
        if x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height {
            self.px[y as usize * self.width + x as usize] = c;
        }
    }

    /// A line, Bresenham's way.
    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: u8) {
        let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
        let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
        let (mut x, mut y, mut err) = (x0, y0, dx + dy);
        loop {
            self.set(x, y, c);
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: u8) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.set(xx, yy, c);
            }
        }
    }

    /// A filled circle.
    pub fn disc(&mut self, cx: i32, cy: i32, r: i32, c: u8) {
        for yy in -r..=r {
            for xx in -r..=r {
                if xx * xx + yy * yy <= r * r + r / 2 {
                    self.set(cx + xx, cy + yy, c);
                }
            }
        }
    }

    /// Onto the canvas: a cell whose halves are both empty keeps what it
    /// had; one empty half takes the cell's background.
    pub fn draw(&self, canvas: &mut Canvas) {
        let rows = (self.height / 2).min(canvas.height);
        let cols = self.width.min(canvas.width);
        for y in 0..rows {
            for x in 0..cols {
                let top = self.px[(2 * y) * self.width + x];
                let bottom = self.px[(2 * y + 1) * self.width + x];
                if top == NONE && bottom == NONE {
                    continue;
                }
                let i = y * canvas.width + x;
                let under = canvas.cells[i].bg;
                let (t, b) = (if top == NONE { under } else { top }, if bottom == NONE { under } else { bottom });
                canvas.cells[i] = halves(t, b);
            }
        }
    }
}

/// A cell showing `top` over `bottom`.
pub fn halves(top: u8, bottom: u8) -> Cell {
    if top == bottom {
        Cell::new(b' ', 7, top)
    } else {
        Cell::new(cp437::UPPER, top, bottom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixels_land_two_to_a_cell() {
        let mut c = Canvas::new(4, 2);
        let mut p = Pixels::for_cells(4, 2);
        p.set(1, 0, 12);
        p.set(1, 1, 1);
        p.set(2, 3, 14);
        p.draw(&mut c);
        assert_eq!(c.get(1, 0), Some(Cell::new(cp437::UPPER, 12, 1)));
        assert_eq!(c.get(2, 1), Some(Cell::new(cp437::UPPER, 0, 14)));
        assert_eq!(c.get(0, 0), Some(Cell::BLANK));
    }

    #[test]
    fn text_is_put_in_code_page_437_and_cut_to_fit() {
        let mut c = Canvas::new(8, 1);
        c.center(0, "a long title", 15, 1);
        assert_eq!(c.to_text(), "a long ·\n");
        assert_eq!(c.cells[0].attr(), 0x1F);
    }
}
