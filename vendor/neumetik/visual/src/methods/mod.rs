// Copyright 2026 ansiapps. Neumetik Visual: proprietary, see the LICENSE at the repository root.

//! The visualization methods, and what they share: a phosphor screen
//! that glows and fades, and resampling.

pub mod demo;
pub mod meters;
pub mod show;
pub mod spectrum;
pub mod wave;

use crate::canvas::{Canvas, Pixels};
use crate::palette::Ramp;

/// Values resampled to `n`, linearly.
pub fn resample(v: &[f32], n: usize) -> Vec<f32> {
    if v.is_empty() || n == 0 {
        return vec![0.0; n];
    }
    if v.len() == n {
        return v.to_vec();
    }
    (0..n)
        .map(|i| {
            let x = if n == 1 { 0.0 } else { i as f32 * (v.len() - 1) as f32 / (n - 1) as f32 };
            let k = x.floor() as usize;
            let t = x - k as f32;
            v[k] * (1.0 - t) + v[(k + 1).min(v.len() - 1)] * t
        })
        .collect()
}

/// A note's name, tracker style (0 is C-0): `C-5`, `F#2`.
pub fn note_name(n: u8) -> String {
    const NAMES: [&str; 12] = ["C-", "C#", "D-", "D#", "E-", "F-", "F#", "G-", "G#", "A-", "A#", "B-"];
    format!("{}{}", NAMES[usize::from(n % 12)], n / 12)
}

/// A phosphor screen in half-block pixels: what's drawn glows at 1 and
/// fades, shown through a palette.
#[derive(Default)]
pub struct Glow {
    pub width: usize,
    pub height: usize,
    pub v: Vec<f32>,
}

impl Glow {
    /// Sized for a canvas, cleared when the size changed.
    pub fn fit(&mut self, canvas: &Canvas) {
        let (w, h) = (canvas.width, canvas.height * 2);
        if w != self.width || h != self.height {
            self.width = w;
            self.height = h;
            self.v = vec![0.0; w * h];
        }
    }

    /// Fades: `per_second` of the glow gone each second.
    pub fn fade(&mut self, dt: f32, per_second: f32) {
        let k = (-per_second * dt).exp();
        self.v.iter_mut().for_each(|v| *v *= k);
    }

    pub fn hit(&mut self, x: i32, y: i32, v: f32) {
        if x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height {
            let p = &mut self.v[y as usize * self.width + x as usize];
            *p = p.max(v);
        }
    }

    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, v: f32) {
        let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
        let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
        let (mut x, mut y, mut err) = (x0, y0, dx + dy);
        for _ in 0..4096 {
            self.hit(x, y, v);
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

    /// Onto the canvas through the palette (the dimmest glow left as the
    /// background).
    pub fn draw(&self, out: &mut Canvas, ramp: &Ramp) {
        let mut p = Pixels::for_cells(out.width, out.height);
        for (i, &v) in self.v.iter().enumerate() {
            if v > 0.04 {
                p.px[i] = ramp.color(v);
            }
        }
        p.draw(out);
    }
}

/// A canvas's middle, in pixels.
pub fn middle(out: &Canvas) -> (f32, f32) {
    (out.width as f32 / 2.0, out.height as f32)
}
