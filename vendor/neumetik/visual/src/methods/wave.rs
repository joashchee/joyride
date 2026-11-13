// Copyright 2026 ansiapps. Neumetik Visual: proprietary, see the LICENSE at the repository root.

//! The waveform, drawn on a glowing phosphor screen: an oscilloscope, a
//! vectorscope, a polar scope and a phase-space plot.

use super::Glow;
use crate::canvas::{Canvas, Cell};
use crate::cp437;
use crate::palette::{DARK_GRAY, GRAY, LIGHT_GREEN, LIGHT_RED, YELLOW};
use crate::visual::{Ctx, Method};

/// Where to start a trace so it stands still: the first rise through
/// zero in the window's first half, else its start.
fn trigger(wave: &[f32], span: usize) -> usize {
    let limit = wave.len().saturating_sub(span);
    (1..limit).find(|&i| wave[i - 1] < 0.0 && wave[i] >= 0.0).unwrap_or(limit)
}

/// The dots of a scope's graticule.
fn graticule(out: &mut Canvas, bg: u8) {
    let (w, h) = (out.width as i32, out.height as i32);
    for y in 0..h {
        for x in 0..w {
            let on_x = w >= 10 && x % (w / 10).max(1) == 0;
            let on_y = h >= 4 && y % (h / 4).max(1) == 0;
            if on_x && on_y {
                out.put(x, y, Cell::new(b'+', DARK_GRAY, bg));
            } else if y == h / 2 || on_x && on_y {
                out.put(x, y, Cell::new(cp437::DOT, DARK_GRAY, bg));
            }
        }
    }
}

/// An oscilloscope: the wave, triggered, its trace glowing and fading.
/// Mirrored, the two sides are drawn apart, left above right.
#[derive(Default)]
pub struct Scope {
    glow: Glow,
}

impl Scope {
    fn trace(&mut self, wave: &[f32], top: i32, height: i32, span: usize) {
        let w = self.glow.width as i32;
        if wave.is_empty() || w == 0 || height <= 0 {
            return;
        }
        let start = trigger(wave, span);
        let mid = top as f32 + height as f32 / 2.0;
        let amp = height as f32 / 2.0 - 1.0;
        let mut prev = None;
        for x in 0..w {
            let i = (start + x as usize * span / w as usize).min(wave.len() - 1);
            let y = (mid - wave[i].clamp(-1.0, 1.0) * amp).round() as i32;
            if let Some((px, py)) = prev {
                self.glow.line(px, py, x, y, 1.0);
            }
            prev = Some((x, y));
        }
    }
}

impl Method for Scope {
    fn draw(&mut self, cx: &Ctx, out: &mut Canvas) {
        self.glow.fit(out);
        self.glow.fade(cx.dt, 14.0);
        // About 25 ms across, at most half the window.
        let span = ((cx.input.rate as f32 * 0.025) as usize).clamp(64, cx.a.wave.len() / 2);
        let h = self.glow.height as i32;
        if cx.settings.mirror {
            self.trace(&cx.a.left, 0, h / 2, span);
            self.trace(&cx.a.right, h / 2, h - h / 2, span);
        } else {
            self.trace(&cx.a.wave, 0, h, span);
        }
        let bg = cx.ramp.background();
        out.clear(bg);
        if cx.settings.labels {
            graticule(out, bg);
        }
        self.glow.draw(out, cx.ramp);
    }
}

/// Left against right, turned 45° so a mono sound is a vertical line and
/// out-of-phase sound a horizontal one (a goniometer), glowing and fading;
/// with the correlation meter under it.
#[derive(Default)]
pub struct Vectorscope {
    glow: Glow,
}

impl Method for Vectorscope {
    fn draw(&mut self, cx: &Ctx, out: &mut Canvas) {
        let bg = cx.ramp.background();
        out.clear(bg);
        let meter = cx.settings.labels && out.height >= 6 && out.width >= 20;
        let mut area = Canvas::new(out.width, out.height - usize::from(meter));
        self.glow.fit(&area);
        self.glow.fade(cx.dt, 10.0);
        let (w, h) = (self.glow.width as f32, self.glow.height as f32);
        let size = (w.min(h) / 2.0 - 1.0).max(1.0);
        let (l, r) = (&cx.a.left, &cx.a.right);
        let n = l.len().min(r.len());
        let take = n.min(1024);
        let mut prev = None;
        for i in n - take..n {
            let side = (r[i] - l[i]) * std::f32::consts::FRAC_1_SQRT_2;
            let midv = (l[i] + r[i]) * std::f32::consts::FRAC_1_SQRT_2;
            let x = (w / 2.0 + side.clamp(-1.4, 1.4) * size) as i32;
            let y = (h / 2.0 - midv.clamp(-1.4, 1.4) * size) as i32;
            if let Some((px, py)) = prev {
                self.glow.line(px, py, x, y, 0.8);
            }
            prev = Some((x, y));
        }
        area.clear(bg);
        if cx.settings.labels {
            // The axes: L and R on the diagonals.
            let c = area.width as i32 / 2;
            area.put(c, 0, Cell::new(b'M', DARK_GRAY, bg));
            area.put(c - (area.height as i32 - 1), 0, Cell::new(b'L', DARK_GRAY, bg));
            area.put(c + (area.height as i32 - 1), 0, Cell::new(b'R', DARK_GRAY, bg));
        }
        self.glow.draw(&mut area, cx.ramp);
        out.blit(&area, 0, 0, None);
        if meter {
            let y = out.height as i32 - 1;
            let x0 = 4;
            let w = out.width as i32 - 8;
            out.text(0, y, "-1", GRAY, bg);
            out.text(out.width as i32 - 2, y, "+1", GRAY, bg);
            for x in 0..w {
                out.put(x0 + x, y, Cell::new(0xC4, DARK_GRAY, bg));
            }
            let at = x0 + ((cx.a.correlation.clamp(-1.0, 1.0) + 1.0) / 2.0 * (w - 1) as f32) as i32;
            let c = if cx.a.correlation < 0.0 {
                LIGHT_RED
            } else if cx.a.correlation < 0.5 {
                YELLOW
            } else {
                LIGHT_GREEN
            };
            out.put(x0 + w / 2, y, Cell::new(0xC5, GRAY, bg));
            out.put(at, y, Cell::new(cp437::FULL, c, bg));
        }
    }
}

/// The waveform around a circle, its radius the wave, turning slowly.
#[derive(Default)]
pub struct Polar {
    glow: Glow,
}

impl Method for Polar {
    fn draw(&mut self, cx: &Ctx, out: &mut Canvas) {
        self.glow.fit(out);
        self.glow.fade(cx.dt, 8.0);
        let (w, h) = (self.glow.width as f32, self.glow.height as f32);
        let size = (w.min(h) / 2.0 - 1.0).max(1.0);
        let wave = &cx.a.wave;
        let span = ((cx.input.rate as f32 * 0.04) as usize).clamp(64, wave.len() / 2);
        let start = trigger(wave, span);
        let turn = cx.t as f32 * 0.4;
        let mut prev = None;
        let steps = 256;
        for k in 0..=steps {
            let i = (start + k % steps * span / steps).min(wave.len() - 1);
            let ang = turn + k as f32 / steps as f32 * std::f32::consts::TAU;
            let r = size * (0.55 + 0.45 * wave[i].clamp(-1.0, 1.0)) * (0.9 + 0.1 * cx.a.beat);
            let p = ((w / 2.0 + ang.cos() * r) as i32, (h / 2.0 + ang.sin() * r) as i32);
            if let Some((px, py)) = prev {
                self.glow.line(px, py, p.0, p.1, 1.0);
            }
            prev = Some(p);
        }
        out.clear(cx.ramp.background());
        self.glow.draw(out, cx.ramp);
    }
}

/// The wave against itself a moment before (a delay embedding, after
/// Takens): a pure tone draws an ellipse, a rich sound a tangle, noise a
/// cloud. The delay is a quarter of the strongest band's period.
#[derive(Default)]
pub struct Phase {
    glow: Glow,
}

impl Method for Phase {
    fn draw(&mut self, cx: &Ctx, out: &mut Canvas) {
        self.glow.fit(out);
        self.glow.fade(cx.dt, 6.0);
        let (w, h) = (self.glow.width as f32, self.glow.height as f32);
        let size = (w.min(h) / 2.0 - 1.0).max(1.0);
        let (mut best, mut at) = (0.0, 1);
        for (k, &v) in cx.a.spectrum.iter().enumerate().skip(2) {
            if v > best {
                best = v;
                at = k;
            }
        }
        let freq = at as f32 * cx.a.bin_hz.max(1.0);
        let tau = ((cx.input.rate as f32 / freq / 4.0) as usize).clamp(1, 400);
        let wave = &cx.a.wave;
        let n = wave.len();
        let take = 768.min(n.saturating_sub(tau));
        let rot = cx.t as f32 * 0.25;
        let (s, c) = rot.sin_cos();
        let mut prev = None;
        for i in n - take..n {
            let (a, b) = (wave[i].clamp(-1.2, 1.2), wave[i - tau].clamp(-1.2, 1.2));
            let (x, y) = (a * c - b * s, a * s + b * c);
            let p = ((w / 2.0 + x * size) as i32, (h / 2.0 - y * size) as i32);
            if let Some((px, py)) = prev {
                self.glow.line(px, py, p.0, p.1, 0.7);
            }
            prev = Some(p);
        }
        out.clear(cx.ramp.background());
        self.glow.draw(out, cx.ramp);
    }
}
