// Copyright 2026 ansiapps. Neumetik Visual: proprietary, see the LICENSE at the repository root.

//! ANSI art files, read and written so Stylus and Neumetik open each
//! other's (`docs/stylus-interchange.md`):
//!
//! - **ANSI** (`.ans`): code page 437 with SGR colors, iCE (a bright
//!   background in place of blink, said in SAUCE). A picture is written as
//!   Stylus's own converters write one: every cell, no line breaks, each
//!   row filling the width SAUCE gives.
//! - **ANSImation** (`.ans`, SAUCE file type 2): the first frame drawn
//!   whole, each one after it starting with the cursor sent home (`ESC[H`,
//!   no numbers) and writing only the cells that changed, by cursor moves
//!   that always give both numbers. The hold times are in a SAUCE comment.
//!   Read, any ANSI file's frames are what's on screen each time the cursor
//!   is sent home that way (or the screen clears) after drawing.
//! - **XBin** (`.xb`) and **binary text** (`.bin`): cells as character
//!   and attribute bytes.
//!
//! The interpreter follows ANSI.SYS and the art scene's viewers (as
//! libansilove does): 80 columns unless SAUCE says, wrapping at the edge,
//! a tab to the next eighth column; SGR's bold for a bright color, blink
//! for a bright background; aixterm's bright colors, 256-color and 24-bit
//! colors (xterm's and PabloDraw's) taken to the nearest VGA color.

use crate::anim::{Animation, Frame, DEFAULT_HOLD_MS, MAX_FRAMES, MAX_HOLD_MS, MIN_HOLD_MS};
use crate::canvas::{Canvas, Cell};
use crate::palette::nearest;
use crate::sauce::{self, Sauce};

/// The rows an ANSI file can grow to.
const MAX_ROWS: usize = 5000;
/// The first line of the SAUCE comments of an ANSImation Neumetik wrote.
pub const ANIMATION_MARK: &str = "Neumetik ANSImation 1";

/// What's said about a file in its SAUCE record.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Meta {
    pub title: String,
    pub author: String,
    pub group: String,
    pub comments: Vec<String>,
}

impl Meta {
    fn sauce(&self, data_type: u8, file_type: u8, width: usize, height: usize) -> Sauce {
        let mut s = Sauce::character(file_type, width, height);
        s.data_type = data_type;
        s.title = self.title.clone();
        s.author = self.author.clone();
        s.group = self.group.clone();
        s.comments = self.comments.clone();
        s
    }
}

/// A character that's a control code in an ANSI file, written as a space.
fn printable(ch: u8) -> u8 {
    match ch {
        0x00 | 0x07..=0x0A | 0x0D | 0x1A | 0x1B => b' ',
        c => c,
    }
}

/// SGR for a cell's colors: bold for a bright color, blink for a bright
/// background (iCE).
fn sgr(out: &mut Vec<u8>, c: Cell) {
    let mut s = String::from("\x1b[0");
    if c.fg >= 8 {
        s.push_str(";1");
    }
    if c.bg >= 8 {
        s.push_str(";5");
    }
    // ANSI's color numbers run red, green, blue: the VGA's run blue, green, red.
    const ORDER: [u8; 8] = [0, 4, 2, 6, 1, 5, 3, 7];
    s.push_str(&format!(";{};{}m", 30 + ORDER[usize::from(c.fg & 7)], 40 + ORDER[usize::from(c.bg & 7)]));
    out.extend_from_slice(s.as_bytes());
}

/// Cells from (x, y) on, colors changed only where they change.
fn cells_out(out: &mut Vec<u8>, cells: &[Cell], last: &mut Option<Cell>) {
    for &c in cells {
        if last.is_none_or(|l| l.fg != c.fg || l.bg != c.bg) {
            sgr(out, c);
        }
        *last = Some(c);
        out.push(printable(c.ch));
    }
}

/// A picture as an ANSI file, with SAUCE.
pub fn write_ans(canvas: &Canvas, meta: &Meta) -> Vec<u8> {
    let mut out = Vec::new();
    let mut last = None;
    cells_out(&mut out, &canvas.cells, &mut last);
    out.extend_from_slice(b"\x1b[0m");
    finish(out, meta.sauce(sauce::CHARACTER, sauce::ANSI, canvas.width, canvas.height))
}

fn finish(mut out: Vec<u8>, sauce: Sauce) -> Vec<u8> {
    let size = out.len();
    out.push(0x1A);
    out.extend(sauce.to_bytes(size));
    out
}

/// The SAUCE comment lines that give each frame's hold time.
fn hold_lines(holds: &[u32]) -> Vec<String> {
    let mut lines = vec![ANIMATION_MARK.to_string()];
    if holds.iter().all(|&h| h == holds[0]) {
        lines.push(format!("hold {}", holds[0]));
        return lines;
    }
    let mut line = String::from("holds");
    for h in holds {
        let word = format!(" {h}");
        if line.len() + word.len() > 64 {
            lines.push(std::mem::replace(&mut line, String::from("holds")));
        }
        line.push_str(&word);
    }
    lines.push(line);
    lines
}

/// An animation as an ANSImation: the first frame whole, then each frame
/// from the cursor's home, writing only what changed.
pub fn write_animation(anim: &Animation, meta: &Meta) -> Vec<u8> {
    let mut anim = anim.clone();
    anim.even_up();
    let (w, h) = (anim.width(), anim.height());
    let mut out = b"\x1b[0m\x1b[2J".to_vec();
    let mut prev: Option<&Canvas> = None;
    let mut last = None;
    for f in &anim.frames {
        out.extend_from_slice(b"\x1b[H");
        for y in 0..h {
            let row = &f.canvas.cells[y * w..(y + 1) * w];
            let Some(p) = prev else {
                out.extend_from_slice(format!("\x1b[{};1H", y + 1).as_bytes());
                cells_out(&mut out, row, &mut last);
                continue;
            };
            let old = &p.cells[y * w..(y + 1) * w];
            let mut x = 0;
            while x < w {
                if row[x] == old[x] {
                    x += 1;
                    continue;
                }
                // A run of changes, bridging gaps of up to four cells the
                // same (a cursor move costs about that).
                let mut end = x + 1;
                let mut same = 0;
                let mut j = x + 1;
                while j < w && same <= 4 {
                    if row[j] == old[j] {
                        same += 1;
                    } else {
                        same = 0;
                        end = j + 1;
                    }
                    j += 1;
                }
                out.extend_from_slice(format!("\x1b[{};{}H", y + 1, x + 1).as_bytes());
                cells_out(&mut out, &row[x..end], &mut last);
                x = end;
            }
        }
        prev = Some(&f.canvas);
    }
    out.extend_from_slice(b"\x1b[0m");
    let mut meta = meta.clone();
    let holds: Vec<u32> = anim.frames.iter().map(|f| f.hold_ms).collect();
    let mut comments = hold_lines(&holds);
    comments.append(&mut meta.comments);
    comments.truncate(255);
    meta.comments = comments;
    let file_type = if anim.frames.len() > 1 { sauce::ANSIMATION } else { sauce::ANSI };
    finish(out, meta.sauce(sauce::CHARACTER, file_type, w, h))
}

/// A picture as XBin: no palette or font of its own (the VGA's), not
/// compressed, iCE colors.
pub fn write_xbin(canvas: &Canvas, meta: &Meta) -> Vec<u8> {
    let mut out = b"XBIN\x1a".to_vec();
    out.extend_from_slice(&(canvas.width as u16).to_le_bytes());
    out.extend_from_slice(&(canvas.height as u16).to_le_bytes());
    out.push(16);
    out.push(0x08);
    for c in &canvas.cells {
        out.push(c.ch);
        out.push(c.attr());
    }
    finish(out, meta.sauce(sauce::XBIN, 0, canvas.width, canvas.height))
}

/// A picture as binary text: its width made even (a blank column added
/// when it's odd), as the format needs.
pub fn write_bin(canvas: &Canvas, meta: &Meta) -> Vec<u8> {
    let w = canvas.width + canvas.width % 2;
    let mut out = Vec::with_capacity(w * canvas.height * 2);
    for row in canvas.cells.chunks(canvas.width) {
        for x in 0..w {
            let c = row.get(x).copied().unwrap_or(Cell::BLANK);
            out.push(c.ch);
            out.push(c.attr());
        }
    }
    let mut s = meta.sauce(sauce::BINARY_TEXT, (w / 2).min(255) as u8, w, canvas.height);
    s.info = [0; 4];
    finish(out, s)
}

/// Writes a picture or an animation by its extension (`ans`, `xb`,
/// `bin`); the binary formats take the first frame.
pub fn write(extension: &str, anim: &Animation, meta: &Meta) -> Result<Vec<u8>, String> {
    let first = &anim.frames.first().ok_or("There's nothing to save.")?.canvas;
    match extension.to_ascii_lowercase().as_str() {
        "ans" if anim.is_animated() => Ok(write_animation(anim, meta)),
        "ans" => Ok(write_ans(first, meta)),
        "xb" => Ok(write_xbin(first, meta)),
        "bin" => Ok(write_bin(first, meta)),
        other => Err(format!("Neumetik can't save .{other} art.")),
    }
}

/// Reads ANSI art by its extension: `xb` and `bin` as binary cells,
/// anything else (`ans`, `asc`, `txt`, `nfo`, `diz`...) through the ANSI
/// interpreter.
pub fn read(extension: &str, data: &[u8]) -> Result<Animation, String> {
    let (sauce, len) = Sauce::read(data);
    let body = &data[..len];
    let mut anim = match extension.to_ascii_lowercase().as_str() {
        "xb" | "xbin" => read_xbin(data)?,
        "bin" => read_bin(body, sauce.as_ref()),
        _ => read_ansi(&body[..sauce::text_end(body, body.len())], sauce.as_ref()),
    };
    if anim.frames.is_empty() {
        return Err("The file has no art in it.".into());
    }
    anim.even_up();
    anim.sauce = sauce;
    Ok(anim)
}

fn read_bin(body: &[u8], sauce: Option<&Sauce>) -> Animation {
    // The width: binary text's own way (half of it as the file type), or
    // in type info 1, where icy_engine (and so Stylus) writes it.
    let w = match sauce {
        Some(s) if s.data_type == sauce::BINARY_TEXT && s.file_type > 0 => usize::from(s.file_type) * 2,
        Some(s) if s.info[0] > 0 => usize::from(s.info[0]).min(1000),
        _ => 160,
    };
    let h = (body.len() / 2).div_ceil(w).clamp(1, MAX_ROWS);
    let mut c = Canvas::new(w, h);
    for (i, p) in body.chunks_exact(2).take(w * h).enumerate() {
        c.cells[i] = Cell::from_attr(p[0], p[1]);
    }
    Animation::still(c)
}

fn read_xbin(data: &[u8]) -> Result<Animation, String> {
    if data.len() < 11 || &data[..5] != b"XBIN\x1a" {
        return Err("This isn't an XBin file.".into());
    }
    let w = usize::from(u16::from_le_bytes([data[5], data[6]])).max(1);
    let h = usize::from(u16::from_le_bytes([data[7], data[8]])).clamp(1, MAX_ROWS);
    let font_height = usize::from(data[9]);
    let flags = data[10];
    let mut at = 11;
    // Its own palette: each index shown as the VGA color nearest it.
    let mut map: [u8; 16] = std::array::from_fn(|i| i as u8);
    if flags & 0x01 != 0 {
        let pal = data.get(at..at + 48).ok_or("The XBin file ends in its palette.")?;
        for (i, m) in map.iter_mut().enumerate() {
            let six = |v: u8| ((u16::from(v & 63) * 255 + 31) / 63) as u8;
            *m = nearest([six(pal[i * 3]), six(pal[i * 3 + 1]), six(pal[i * 3 + 2])]);
        }
        at += 48;
    }
    if flags & 0x02 != 0 {
        at += font_height * if flags & 0x10 != 0 { 512 } else { 256 };
    }
    let mut cells = Vec::with_capacity(w * h);
    let total = w * h;
    let src = data.get(at..).unwrap_or(&[]);
    if flags & 0x04 != 0 {
        let mut i = 0;
        while cells.len() < total && i < src.len() {
            let head = src[i];
            i += 1;
            let n = usize::from(head & 63) + 1;
            match head >> 6 {
                0 => {
                    for _ in 0..n {
                        let (Some(&ch), Some(&a)) = (src.get(i), src.get(i + 1)) else { break };
                        cells.push((ch, a));
                        i += 2;
                    }
                }
                1 => {
                    let Some(&ch) = src.get(i) else { break };
                    i += 1;
                    for _ in 0..n {
                        let Some(&a) = src.get(i) else { break };
                        cells.push((ch, a));
                        i += 1;
                    }
                }
                2 => {
                    let Some(&a) = src.get(i) else { break };
                    i += 1;
                    for _ in 0..n {
                        let Some(&ch) = src.get(i) else { break };
                        cells.push((ch, a));
                        i += 1;
                    }
                }
                _ => {
                    let (Some(&ch), Some(&a)) = (src.get(i), src.get(i + 1)) else { break };
                    i += 2;
                    cells.extend(std::iter::repeat_n((ch, a), n));
                }
            }
        }
    } else {
        cells.extend(src.chunks_exact(2).take(total).map(|p| (p[0], p[1])));
    }
    let mut c = Canvas::new(w, h);
    for (i, (ch, a)) in cells.into_iter().take(total).enumerate() {
        let a = map[usize::from(a & 15)] | map[usize::from(a >> 4)] << 4;
        c.cells[i] = Cell::from_attr(ch, a);
    }
    Ok(Animation::still(c))
}

/// The ANSI interpreter's state.
struct Term {
    width: usize,
    rows: Vec<Cell>,
    height: usize,
    x: usize,
    y: usize,
    saved: (usize, usize),
    fg: u8,
    bg: u8,
    bold: bool,
    blink: bool,
    reverse: bool,
    /// Something was drawn since the last frame was kept.
    drawn: bool,
    frames: Vec<(Canvas, usize)>,
    /// Bytes read since the last frame, for its hold at a baud rate.
    bytes: usize,
}

impl Term {
    fn new(width: usize) -> Term {
        Term { width, rows: Vec::new(), height: 0, x: 0, y: 0, saved: (0, 0), fg: 7, bg: 0, bold: false, blink: false, reverse: false, drawn: false, frames: Vec::new(), bytes: 0 }
    }

    fn cell_attr(&self) -> (u8, u8) {
        let mut fg = self.fg | if self.bold { 8 } else { 0 };
        let mut bg = self.bg | if self.blink { 8 } else { 0 };
        if self.reverse {
            std::mem::swap(&mut fg, &mut bg);
        }
        (fg, bg)
    }

    fn grow(&mut self, y: usize) {
        if y >= self.height && y < MAX_ROWS {
            self.height = y + 1;
            self.rows.resize(self.height * self.width, Cell::BLANK);
        }
    }

    fn print(&mut self, ch: u8) {
        if self.y >= MAX_ROWS {
            return;
        }
        self.grow(self.y);
        let (fg, bg) = self.cell_attr();
        self.rows[self.y * self.width + self.x] = Cell::new(ch, fg, bg);
        self.drawn = true;
        self.x += 1;
        if self.x >= self.width {
            self.x = 0;
            self.y += 1;
        }
    }

    fn keep_frame(&mut self) {
        if self.drawn && self.frames.len() < MAX_FRAMES {
            let mut c = Canvas::new(self.width, self.height.max(1));
            c.cells[..self.rows.len()].copy_from_slice(&self.rows);
            self.frames.push((c, self.bytes));
            self.bytes = 0;
        }
        self.drawn = false;
    }

    fn sgr(&mut self, params: &[u32]) {
        let mut i = 0;
        if params.is_empty() {
            self.reset();
        }
        while i < params.len() {
            const ORDER: [u8; 8] = [0, 4, 2, 6, 1, 5, 3, 7];
            match params[i] {
                0 => self.reset(),
                1 => self.bold = true,
                5 | 6 => self.blink = true,
                7 => self.reverse = true,
                22 => self.bold = false,
                25 => self.blink = false,
                27 => self.reverse = false,
                n @ 30..=37 => self.fg = ORDER[(n - 30) as usize],
                39 => self.fg = 7,
                n @ 40..=47 => self.bg = ORDER[(n - 40) as usize],
                49 => self.bg = 0,
                n @ 90..=97 => {
                    self.fg = ORDER[(n - 90) as usize];
                    self.bold = true;
                }
                n @ 100..=107 => {
                    self.bg = ORDER[(n - 100) as usize];
                    self.blink = true;
                }
                n @ (38 | 48) => {
                    let color = match params.get(i + 1) {
                        Some(5) => {
                            let c = params.get(i + 2).copied().unwrap_or(0);
                            i += 2;
                            Some(xterm(c))
                        }
                        Some(2) => {
                            let v = |k: usize| params.get(i + k).copied().unwrap_or(0).min(255) as u8;
                            let c = nearest([v(2), v(3), v(4)]);
                            i += 4;
                            Some(c)
                        }
                        _ => None,
                    };
                    if let Some(c) = color {
                        if n == 38 {
                            self.fg = c & 7;
                            self.bold = c >= 8;
                        } else {
                            self.bg = c & 7;
                            self.blink = c >= 8;
                        }
                    }
                }
                _ => {}
            }
            i += 1;
        }
    }

    fn reset(&mut self) {
        self.fg = 7;
        self.bg = 0;
        self.bold = false;
        self.blink = false;
        self.reverse = false;
    }

    fn csi(&mut self, params: &[u32], cmd: u8) {
        let p = |i: usize, d: u32| params.get(i).copied().filter(|&v| v > 0).unwrap_or(d) as usize;
        match cmd {
            b'm' => self.sgr(params),
            b'H' | b'f' => {
                // The cursor sent home with no numbers starts a frame; a
                // move to (1, 1) with them is only a move.
                if params.is_empty() {
                    self.keep_frame();
                }
                let (row, col) = (p(0, 1), p(1, 1));
                self.y = (row - 1).min(MAX_ROWS - 1);
                self.x = (col - 1).min(self.width - 1);
            }
            b'A' => self.y = self.y.saturating_sub(p(0, 1)),
            b'B' => self.y = (self.y + p(0, 1)).min(MAX_ROWS - 1),
            b'C' => self.x = (self.x + p(0, 1)).min(self.width - 1),
            b'D' => self.x = self.x.saturating_sub(p(0, 1)),
            b'E' => {
                self.y = (self.y + p(0, 1)).min(MAX_ROWS - 1);
                self.x = 0;
            }
            b'F' => {
                self.y = self.y.saturating_sub(p(0, 1));
                self.x = 0;
            }
            b'G' => self.x = (p(0, 1) - 1).min(self.width - 1),
            b'J' => {
                let n = params.first().copied().unwrap_or(0);
                if n == 2 {
                    self.keep_frame();
                    self.rows.fill(Cell::BLANK);
                    self.x = 0;
                    self.y = 0;
                } else if n == 0 {
                    let from = (self.y * self.width + self.x).min(self.rows.len());
                    self.rows[from..].fill(Cell::BLANK);
                } else if n == 1 {
                    let to = (self.y * self.width + self.x + 1).min(self.rows.len());
                    self.rows[..to].fill(Cell::BLANK);
                }
            }
            b'K' => {
                if self.y < self.height {
                    let n = params.first().copied().unwrap_or(0);
                    let row = self.y * self.width;
                    let (a, b) = match n {
                        1 => (0, self.x + 1),
                        2 => (0, self.width),
                        _ => (self.x, self.width),
                    };
                    self.rows[row + a..row + b.min(self.width)].fill(Cell::BLANK);
                }
            }
            b's' => self.saved = (self.x, self.y),
            b'u' => (self.x, self.y) = self.saved,
            b't' => {
                // PabloDraw's 24-bit colors: 0 the background, 1 the color.
                if params.len() >= 4 {
                    let v = |k: usize| params[k].min(255) as u8;
                    let c = nearest([v(1), v(2), v(3)]);
                    if params[0] == 0 {
                        self.bg = c & 7;
                        self.blink = c >= 8;
                    } else {
                        self.fg = c & 7;
                        self.bold = c >= 8;
                    }
                }
            }
            _ => {}
        }
    }
}

/// The VGA color nearest one of xterm's 256.
fn xterm(c: u32) -> u8 {
    const ORDER: [u8; 8] = [0, 4, 2, 6, 1, 5, 3, 7];
    match c {
        0..=7 => ORDER[c as usize],
        8..=15 => ORDER[c as usize - 8] + 8,
        16..=231 => {
            let n = c - 16;
            let level = |v: u32| if v == 0 { 0 } else { (55 + v * 40) as u8 };
            nearest([level(n / 36), level(n / 6 % 6), level(n % 6)])
        }
        _ => {
            let g = (8 + (c.min(255) - 232) * 10) as u8;
            nearest([g, g, g])
        }
    }
}

/// The hold times an ANSImation's SAUCE comments give: one for all
/// (`hold 80`), or one a frame (`holds 80 80 120`, over lines).
fn holds_from(sauce: Option<&Sauce>) -> (Option<u32>, Vec<u32>, Option<u32>) {
    let (mut one, mut each, mut baud) = (None, Vec::new(), None);
    for line in sauce.map(|s| s.comments.as_slice()).unwrap_or(&[]) {
        let mut words = line.split_whitespace();
        match words.next() {
            Some("hold") => one = words.next().and_then(|w| w.parse().ok()),
            Some("holds") => each.extend(words.filter_map(|w| w.parse::<u32>().ok())),
            Some("baud") => baud = words.next().and_then(|w| w.parse().ok()),
            _ => {}
        }
    }
    (one, each, baud)
}

fn read_ansi(body: &[u8], sauce: Option<&Sauce>) -> Animation {
    let width = match sauce {
        Some(s) if s.data_type == sauce::CHARACTER && s.info[0] > 0 => usize::from(s.info[0]).min(1000),
        _ => 80,
    };
    let mut t = Term::new(width);
    let mut i = 0;
    while i < body.len() {
        let b = body[i];
        i += 1;
        t.bytes += 1;
        match b {
            0x1B if body.get(i) == Some(&b'[') => {
                i += 1;
                let mut params = Vec::new();
                let mut cur: Option<u32> = None;
                while i < body.len() {
                    let c = body[i];
                    i += 1;
                    t.bytes += 1;
                    match c {
                        b'0'..=b'9' => cur = Some(cur.unwrap_or(0).saturating_mul(10).saturating_add(u32::from(c - b'0'))),
                        b';' => params.push(cur.take().unwrap_or(0)),
                        b'?' | b'=' | b'>' | b' ' => {}
                        0x40..=0x7E => {
                            if let Some(v) = cur.take() {
                                params.push(v);
                            }
                            t.csi(&params, c);
                            break;
                        }
                        _ => break,
                    }
                }
            }
            0x1B => {}
            b'\r' => t.x = 0,
            b'\n' => {
                t.y += 1;
                t.x = 0;
                t.grow(t.y.saturating_sub(1));
            }
            b'\t' => {
                for _ in 0..(8 - t.x % 8) {
                    t.print(b' ');
                }
            }
            0x08 => t.x = t.x.saturating_sub(1),
            0x07 | 0x00 => {}
            0x0C => {
                t.keep_frame();
                t.rows.fill(Cell::BLANK);
                t.x = 0;
                t.y = 0;
            }
            c => t.print(c),
        }
    }
    if t.height == 0 {
        return Animation::default();
    }
    if t.drawn || t.frames.is_empty() {
        t.drawn = true;
        t.keep_frame();
    }
    let (one, each, baud) = holds_from(sauce);
    let frames = t
        .frames
        .into_iter()
        .enumerate()
        .map(|(k, (canvas, bytes))| {
            let hold = each.get(k).copied().or(one).unwrap_or_else(|| match baud {
                // At a baud rate, a frame takes as long as its bytes took to arrive (ten bits each).
                Some(b) if b > 0 => (bytes as u64 * 10_000 / u64::from(b)).min(u64::from(MAX_HOLD_MS)) as u32,
                _ => DEFAULT_HOLD_MS,
            });
            Frame { canvas, hold_ms: hold.clamp(MIN_HOLD_MS, MAX_HOLD_MS) }
        })
        .collect();
    Animation { frames, sauce: None }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cp437;

    fn picture() -> Canvas {
        let mut c = Canvas::new(20, 4);
        c.text(1, 1, "Neumetik", 14, 9);
        c.put(19, 3, Cell::new(cp437::FULL, 12, 0));
        c.put(0, 0, Cell::new(0x1A, 15, 4));
        c
    }

    #[test]
    fn a_picture_reads_back_from_ansi_xbin_and_bin() {
        let c = picture();
        let meta = Meta { title: "Test".into(), ..Meta::default() };
        for ext in ["ans", "xb", "bin"] {
            let file = write(ext, &Animation::still(c.clone()), &meta).unwrap();
            let back = read(ext, &file).unwrap();
            assert_eq!(back.frames.len(), 1, "{ext}");
            assert_eq!(back.sauce.as_ref().unwrap().title, "Test");
            let mut expect = c.clone();
            // ANSI can't hold the EOF character: it comes back a space.
            if ext == "ans" {
                expect.cells[0].ch = b' ';
            }
            assert_eq!(back.frames[0].canvas, expect, "{ext}");
        }
    }

    #[test]
    fn an_animation_reads_back_frame_for_frame_with_its_holds() {
        let mut a = Animation::default();
        let mut c = picture();
        for k in 0..5 {
            c.put(k, 2, Cell::new(b'*', 10, 1));
            a.push(c.clone(), 40 + k as u32 * 10);
        }
        let file = write_animation(&a, &Meta::default());
        let back = read("ans", &file).unwrap();
        assert_eq!(back.frames.len(), 5);
        for (x, y) in a.frames.iter().zip(&back.frames) {
            assert_eq!(x.hold_ms, y.hold_ms);
            let mut expect = x.canvas.clone();
            expect.cells[0].ch = b' ';
            assert_eq!(expect, y.canvas);
        }
        assert_eq!(back.sauce.unwrap().file_type, sauce::ANSIMATION);
    }

    #[test]
    fn binary_text_takes_its_width_from_either_place_sauce_keeps_it() {
        // icy_engine (and so Stylus) writes it in type info 1, under data type 6.
        let mut s = Sauce::character(0, 4, 1);
        s.data_type = sauce::XBIN;
        let mut file = b"A\x07B\x07C\x07D\x07E\x07F\x07G\x07H\x07".to_vec();
        file.push(0x1A);
        file.extend(s.to_bytes(16));
        let c = &read("bin", &file).unwrap().frames[0].canvas;
        assert_eq!((c.width, c.height), (4, 2));
    }

    #[test]
    fn plain_ansi_is_interpreted_as_the_viewers_do() {
        let file = b"\x1b[1;33mHi\x1b[0m\r\n\x1b[44m \x1b[5;31mX\x1b[2CY\tZ";
        let a = read("ans", file).unwrap();
        let c = &a.frames[0].canvas;
        assert_eq!(c.width, 80);
        assert_eq!(c.get(0, 0), Some(Cell::new(b'H', 14, 0)));
        assert_eq!(c.get(0, 1), Some(Cell::new(b' ', 7, 1)));
        assert_eq!(c.get(1, 1), Some(Cell::new(b'X', 4, 9)));
        assert_eq!(c.get(4, 1).unwrap().ch, b'Y');
        assert_eq!(c.get(8, 1).unwrap().ch, b'Z');
    }
}
