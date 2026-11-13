// Copyright 2026 ansiapps. Neumetik Visual: proprietary, see the LICENSE at the repository root.

//! The song's own screens: its title in big letters over a sine
//! scroller of its words, and its cover art (or art opened from Stylus),
//! played and lit by the beat.

use super::resample;
use crate::canvas::{Canvas, Cell, Pixels};
use crate::font;
use crate::palette::bright;
use crate::visual::{Ctx, Method};

/// The title in the block font, bouncing on the beat, its letters
/// stepping through the palette; under it the song's words in a sine
/// scroller, the demoscene's way, over a dim spectrum.
#[derive(Default)]
pub struct Title {
    scroll: f32,
}

impl Method for Title {
    fn bands(&self, width: usize, _height: usize) -> usize {
        width.clamp(1, 128)
    }

    fn draw(&mut self, cx: &Ctx, out: &mut Canvas) {
        let bg = cx.ramp.background();
        out.clear(bg);
        let (w, h) = (out.width as i32, out.height as i32);
        let mut p = Pixels::for_cells(out.width, out.height);
        // A dim spectrum along the bottom.
        let bands = resample(&cx.a.bands, out.width);
        let dim = cx.ramp.colors.get(1).copied().unwrap_or(8);
        for (x, v) in bands.iter().enumerate() {
            let bh = (v * p.height as f32 * 0.45) as i32;
            p.rect(x as i32, p.height as i32 - bh, 1, bh, dim);
        }
        let title = if cx.input.title.trim().is_empty() { "NEUMETIK" } else { cx.input.title.trim() };
        let title: String = title.chars().take(40).collect();
        let mut scale = 1;
        while scale < 4 && font::width(&title, scale + 1) <= w - 2 && font::height(scale + 1) <= h * 2 / 2 {
            scale += 1;
        }
        let tw = font::width(&title, scale);
        let has_room = tw <= w && font::height(scale) + 2 <= h * 2;
        let bounce = (cx.a.beat * 2.0 * scale as f32) as i32;
        let ty = ((h * 2 - font::height(scale)) / 3 - bounce).max(0);
        if has_room {
            let mut x = (w - tw) / 2;
            for (i, ch) in title.chars().enumerate() {
                let t = ((i as f32 * 0.15 + cx.t as f32 * 0.3).fract() * 2.0 - 1.0).abs();
                let wave = ((i as f32 * 0.7 + cx.t as f32 * 4.0).sin() * cx.a.energy * 2.0) as i32;
                font::draw(&mut p, x, ty + wave, &ch.to_string(), scale, cx.ramp.ink(t), Some(0));
                x += 4 * scale;
            }
        }
        p.draw(out);
        if !has_room {
            out.center(0, &title, cx.ramp.brightest(), bg);
        }
        // The scroller.
        let words: String = cx.input.words.split_whitespace().collect::<Vec<_>>().join(" ");
        let text = if words.is_empty() { format!("{title}   ·   played by Neumetik   ·   ") } else { format!("{words}   ·   ") };
        let chars: Vec<char> = text.chars().collect();
        self.scroll += cx.dt * (10.0 + cx.a.energy * 8.0);
        let base = (ty + font::height(scale)) / 2 + 1 + (h - (ty + font::height(scale)) / 2) / 2;
        let amp = ((h - base) as f32 / 2.0 - 0.5).clamp(0.0, 1.5);
        for x in 0..w {
            let k = (x as f32 + self.scroll) as usize % chars.len().max(1);
            let y = base + ((x as f32 * 0.1 + cx.t as f32 * 2.0).sin() * amp).round() as i32;
            let c = cx.ramp.ink(((x as f32 / w as f32) + cx.t as f32 * 0.2).fract());
            let mut buf = [0u8; 4];
            out.text_over(x, y.clamp(0, h - 1), chars[k].encode_utf8(&mut buf), c);
        }
    }
}

/// Art shown as it is, centered (or made smaller to fit), its frames
/// played at the visual's speed, its colors brightened on the beat.
/// Without art, the title stands in.
#[derive(Default)]
pub struct Art {
    title: Title,
}

impl Method for Art {
    fn draw(&mut self, cx: &Ctx, out: &mut Canvas) {
        let Some(art) = cx.art.filter(|a| !a.frames.is_empty()) else {
            self.title.draw(cx, out);
            return;
        };
        let ms = (cx.t * 1000.0) as u64;
        let Some(frame) = art.frame_at(ms) else { return };
        let bg = frame.cells.first().map_or(0, |c| if c.ch == b' ' { c.bg } else { 0 });
        out.clear(bg);
        let (w, h) = (out.width, out.height);
        if frame.width <= w && frame.height <= h {
            out.blit(frame, (w - frame.width) as i32 / 2, (h - frame.height) as i32 / 2, None);
        } else {
            // Smaller, the same shape.
            let k = (w as f32 / frame.width as f32).min(h as f32 / frame.height as f32);
            let (sw, sh) = (((frame.width as f32 * k) as usize).max(1), ((frame.height as f32 * k) as usize).max(1));
            let mut small = Canvas::new(sw, sh);
            small.blit_scaled(frame);
            out.blit(&small, (w - sw) as i32 / 2, (h - sh) as i32 / 2, None);
        }
        // The beat lights it up, a sweep of light following the bass.
        let sweep = ((cx.t as f32 * 0.5).fract() * (w as f32 + 20.0)) as i32 - 10;
        for y in 0..h as i32 {
            for x in 0..w as i32 {
                let near = (x - sweep).abs() < 3 && cx.a.bass > 0.4;
                if cx.a.beat > 0.6 || near {
                    out.with(x, y, |c: &mut Cell| c.fg = bright(c.fg));
                }
            }
        }
    }
}
