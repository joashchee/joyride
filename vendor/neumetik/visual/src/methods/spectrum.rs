// Copyright 2026 ansiapps. Neumetik Visual: proprietary, see the LICENSE at the repository root.

//! The spectrum, drawn: bars, an LED analyzer, a scrolling spectrogram,
//! ridgelines, a radial spectrum, a chromagram with the key it suggests,
//! and a kaleidoscope.

use super::resample;
use crate::canvas::{Canvas, Cell, Pixels};
use crate::cp437;
use crate::palette::{dim, GRAY};
use crate::visual::{Ctx, Method};

/// Frequency labels along the bottom, under the bands' edges.
fn freq_labels(out: &mut Canvas, cx: &Ctx, y: i32, x0: i32, w: i32) {
    let edges = &cx.a.edges;
    if edges.len() < 2 || w < 12 {
        return;
    }
    let (lo, hi) = (edges[0].ln(), edges[edges.len() - 1].ln());
    let mut last_end = -1;
    for (f, label) in [(50.0, "50"), (100.0, "100"), (200.0, "200"), (500.0, "500"), (1000.0, "1k"), (2000.0, "2k"), (5000.0, "5k"), (10_000.0, "10k")] {
        let t = (f32::ln(f) - lo) / (hi - lo);
        if !(0.0..1.0).contains(&t) {
            continue;
        }
        let x = x0 + (t * w as f32) as i32;
        if x > last_end {
            out.text(x, y, label, GRAY, cx.ramp.background());
            last_end = x + label.len() as i32;
        }
    }
}

/// Spectrum bars: a column (or two) a band, half a cell at a time, with
/// a falling peak above each.
#[derive(Default)]
pub struct Bars;

impl Bars {
    fn layout(width: usize) -> (usize, usize) {
        // Bars two columns wide and a gap, as many as fit (at most 96).
        let w = width.max(1);
        let n = if w >= 48 {
            (w / 3).min(96)
        } else if w >= 16 {
            w / 2
        } else {
            w
        };
        (n.max(1), w / n.max(1))
    }
}

impl Method for Bars {
    fn bands(&self, width: usize, _height: usize) -> usize {
        Bars::layout(width).0
    }

    fn draw(&mut self, cx: &Ctx, out: &mut Canvas) {
        out.clear(cx.ramp.background());
        let labels = cx.settings.labels && out.height >= 8 && out.width >= 24;
        let rows = out.height - usize::from(labels);
        let (n, step) = Bars::layout(out.width);
        let bar_w = if step >= 3 { step - 1 } else { 1 };
        let x0 = (out.width - n * step) as i32 / 2;
        let bands = resample(&cx.a.bands, n);
        let peaks = resample(&cx.a.peaks, n);
        let mut p = Pixels::for_cells(out.width, rows);
        let hp = p.height as i32;
        for b in 0..n {
            let x = x0 + (b * step) as i32;
            let (v, pk) = (bands[b], peaks[b]);
            if cx.settings.mirror {
                let half = hp / 2;
                let h = (v * half as f32).round() as i32;
                for k in 0..h {
                    let c = cx.ramp.ink(k as f32 / half as f32);
                    p.rect(x, half - 1 - k, bar_w as i32, 1, c);
                    p.rect(x, half + k, bar_w as i32, 1, dim(c));
                }
                continue;
            }
            let h = (v * hp as f32).round() as i32;
            for k in 0..h {
                p.rect(x, hp - 1 - k, bar_w as i32, 1, cx.ramp.ink(k as f32 / hp as f32));
            }
            if cx.settings.peaks && pk > 0.02 {
                let y = hp - 1 - ((pk * hp as f32).round() as i32).min(hp - 1);
                p.rect(x, y, bar_w as i32, 1, cx.ramp.brightest());
            }
        }
        p.draw(out);
        if labels {
            freq_labels(out, cx, out.height as i32 - 1, x0, (n * step) as i32);
        }
    }
}

/// A hi-fi's LED analyzer: each bar a column of segments (■), lit to the
/// level and dark above it, the top segment held as the peak.
pub struct Led;

impl Method for Led {
    fn bands(&self, width: usize, _height: usize) -> usize {
        (width / 2).clamp(1, 64)
    }

    fn draw(&mut self, cx: &Ctx, out: &mut Canvas) {
        let bg = cx.ramp.background();
        out.clear(bg);
        let n = (out.width / 2).clamp(1, 64);
        let step = out.width / n;
        let x0 = (out.width - n * step) as i32 / 2;
        let bands = resample(&cx.a.bands, n);
        let peaks = resample(&cx.a.peaks, n);
        let rows = out.height;
        for b in 0..n {
            let lit = (bands[b] * rows as f32).round() as usize;
            let peak = (peaks[b] * rows as f32).round() as usize;
            for k in 0..rows {
                let y = (rows - 1 - k) as i32;
                let c = cx.ramp.ink(k as f32 / rows as f32);
                let on = k < lit || (cx.settings.peaks && k + 1 == peak);
                let cell = if on { Cell::new(cp437::SQUARE, c, bg) } else { Cell::new(cp437::SQUARE, if bg == 8 { 0 } else { 8 }, bg) };
                for dx in 0..step.saturating_sub(1).max(1) {
                    out.put(x0 + (b * step + dx) as i32, y, cell);
                }
            }
        }
    }
}

/// The spectrum through time: a column a step, scrolling left, low notes
/// at the bottom, shaded by level.
#[derive(Default)]
pub struct Spectrogram {
    history: Vec<Vec<f32>>,
    carry: f32,
}

impl Method for Spectrogram {
    fn bands(&self, _width: usize, height: usize) -> usize {
        height.max(1)
    }

    fn draw(&mut self, cx: &Ctx, out: &mut Canvas) {
        let (w, h) = (out.width, out.height);
        // A column every 1/30 s (at speed 1), however often it's drawn.
        self.carry += cx.dt * 30.0;
        let new = (self.carry as usize).min(w);
        self.carry -= new as f32;
        for _ in 0..new {
            self.history.push(resample(&cx.a.bands, h));
        }
        if self.history.len() > w {
            let cut = self.history.len() - w;
            self.history.drain(..cut);
        }
        out.clear(cx.ramp.background());
        let x0 = w - self.history.len();
        for (i, col) in self.history.iter().enumerate() {
            for y in 0..h.min(col.len()) {
                let v = col[y];
                out.put((x0 + i) as i32, (h - 1 - y) as i32, cx.ramp.shade(v * v * 1.2));
            }
        }
    }
}

/// Spectra stacked as ridges, oldest at the back (the top), each hiding
/// what's behind it: the cover of a famous record, from the sound.
#[derive(Default)]
pub struct Ridges {
    history: Vec<Vec<f32>>,
    carry: f32,
}

impl Method for Ridges {
    fn bands(&self, width: usize, _height: usize) -> usize {
        width.clamp(1, 256)
    }

    fn draw(&mut self, cx: &Ctx, out: &mut Canvas) {
        let w = out.width;
        let mut p = Pixels::for_cells(w, out.height);
        let hp = p.height as i32;
        let lines = ((hp - 4) / 5).clamp(1, 24) as usize;
        self.carry += cx.dt * 12.0;
        if self.carry >= 1.0 || self.history.is_empty() {
            self.carry = self.carry.fract();
            // Strongest in the middle, as on the record.
            let bands = resample(&cx.a.bands, w);
            let row: Vec<f32> = bands
                .iter()
                .enumerate()
                .map(|(x, v)| {
                    let t = (x as f32 / w.max(2) as f32 - 0.5) * 2.0;
                    v * (1.0 - t * t).max(0.0).powf(0.7)
                })
                .collect();
            self.history.push(row);
        }
        if self.history.len() > lines {
            let cut = self.history.len() - lines;
            self.history.drain(..cut);
        }
        let bg = cx.ramp.background();
        p.clear(bg);
        let gap = (hp - 2) as f32 / (lines + 3) as f32;
        let amp = gap * 3.0;
        let n = self.history.len();
        for (k, row) in self.history.iter().enumerate() {
            let base = (gap * (lines - n + k) as f32 + gap * 4.0) as i32;
            let c = cx.ramp.ink((k + 1) as f32 / n as f32);
            let mut prev = None;
            for (x, v) in row.iter().enumerate() {
                let y = base - (v * amp) as i32;
                // What's behind this ridge, below its line, is hidden.
                for yy in y + 1..=base.min(hp - 1) {
                    p.set(x as i32, yy, bg);
                }
                if let Some((px, py)) = prev {
                    p.line(px, py, x as i32, y, c);
                }
                p.set(x as i32, y, c);
                prev = Some((x as i32, y));
            }
        }
        p.draw(out);
    }
}

/// The spectrum around a circle: a spoke a band, mirrored left and right,
/// the circle swelling with the bass.
pub struct Radial;

impl Method for Radial {
    fn bands(&self, _width: usize, _height: usize) -> usize {
        48
    }

    fn draw(&mut self, cx: &Ctx, out: &mut Canvas) {
        let bg = cx.ramp.background();
        out.clear(bg);
        let mut p = Pixels::for_cells(out.width, out.height);
        let (mx, my) = (out.width as f32 / 2.0, out.height as f32);
        let size = mx.min(my);
        let r0 = size * (0.25 + 0.12 * cx.a.bass + 0.06 * cx.a.beat);
        let n = cx.a.bands.len().max(1);
        let turn = cx.t as f32 * 0.2;
        for side in [-1.0f32, 1.0] {
            for (b, v) in cx.a.bands.iter().enumerate() {
                let ang = turn + side * (std::f32::consts::PI * (b as f32 + 0.5) / n as f32);
                let (s, c) = ang.sin_cos();
                let r1 = r0 + v * size * 0.7;
                let col = cx.ramp.ink(b as f32 / n as f32);
                p.line((mx + s * r0) as i32, (my - c * r0) as i32, (mx + s * r1) as i32, (my - c * r1) as i32, col);
            }
        }
        // The ring.
        let steps = (r0 * 8.0) as i32 + 16;
        for i in 0..steps {
            let ang = i as f32 / steps as f32 * std::f32::consts::TAU;
            p.set((mx + ang.sin() * r0 * 0.92) as i32, (my - ang.cos() * r0 * 0.92) as i32, cx.ramp.brightest());
        }
        p.draw(out);
    }
}

/// The twelve pitch classes' shares, as bars named by their notes, and
/// the key they suggest by Krumhansl and Kessler's key profiles (1982).
pub struct Chroma;

const NOTE_NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
const MAJOR: [f32; 12] = [6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88];
const MINOR: [f32; 12] = [6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17];

/// The key a chromagram suggests, as a name, and how sure (the
/// correlation, -1 to 1).
pub fn key_of(chroma: &[f32; 12]) -> (String, f32) {
    let corr = |profile: &[f32; 12], tonic: usize| {
        let mx = chroma.iter().sum::<f32>() / 12.0;
        let my = profile.iter().sum::<f32>() / 12.0;
        let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
        for i in 0..12 {
            let (x, y) = (chroma[(i + tonic) % 12] - mx, profile[i] - my);
            sxy += x * y;
            sxx += x * x;
            syy += y * y;
        }
        if sxx < 1e-9 {
            0.0
        } else {
            sxy / (sxx * syy).sqrt()
        }
    };
    let mut best = (String::new(), -2.0f32);
    for tonic in 0..12 {
        for (profile, mode) in [(&MAJOR, "major"), (&MINOR, "minor")] {
            let r = corr(profile, tonic);
            if r > best.1 {
                best = (format!("{} {mode}", NOTE_NAMES[tonic]), r);
            }
        }
    }
    best
}

impl Method for Chroma {
    fn draw(&mut self, cx: &Ctx, out: &mut Canvas) {
        let bg = cx.ramp.background();
        out.clear(bg);
        let labels = cx.settings.labels && out.height >= 6;
        let rows = out.height.saturating_sub(if labels { 2 } else { 0 }).max(1);
        let step = (out.width / 12).max(1);
        let x0 = (out.width as i32 - (12 * step) as i32) / 2;
        let mut p = Pixels::for_cells(out.width, rows);
        let hp = p.height as i32;
        for (i, &v) in cx.a.chroma.iter().enumerate() {
            let h = (v * hp as f32) as i32;
            let col = cx.ramp.ink(i as f32 / 11.0);
            p.rect(x0 + (i * step) as i32, hp - h, step.saturating_sub(1).max(1) as i32, h, col);
        }
        p.draw(out);
        if labels {
            for (i, name) in NOTE_NAMES.iter().enumerate() {
                out.text(x0 + (i * step) as i32, rows as i32, name, GRAY, bg);
            }
            if !cx.a.silent {
                let (key, sure) = key_of(&cx.a.chroma);
                if sure > 0.5 {
                    out.center(out.height as i32 - 1, &format!("sounds like {key}"), cx.ramp.brightest(), bg);
                }
            }
        }
    }
}

/// The spectrum in polar form, folded into six mirrored wedges and
/// turning: rings by frequency, lit by level.
pub struct Kaleidoscope;

impl Method for Kaleidoscope {
    fn bands(&self, _width: usize, _height: usize) -> usize {
        32
    }

    fn draw(&mut self, cx: &Ctx, out: &mut Canvas) {
        let (w, h) = (out.width as f32, out.height as f32 * 2.0);
        let (mx, my) = (w / 2.0, h / 2.0);
        let size = mx.min(my).max(1.0);
        let n = cx.a.bands.len().max(1);
        let wedge = std::f32::consts::TAU / 6.0;
        let t = cx.t as f32;
        for y in 0..out.height {
            for x in 0..out.width {
                // The cell's centre, in square pixels.
                let (dx, dy) = (x as f32 + 0.5 - mx, (y as f32 + 0.5) * 2.0 - my);
                let r = (dx * dx + dy * dy).sqrt() / size;
                let mut a = dy.atan2(dx) + t * 0.3;
                a = a.rem_euclid(wedge);
                if a > wedge / 2.0 {
                    a = wedge - a;
                }
                let band = ((r * 0.8 + a * 0.6 + (t * 0.5).sin() * 0.1).abs() * n as f32) as usize % n;
                let v = cx.a.bands[band] * (1.2 - r * 0.5).clamp(0.0, 1.0);
                let ripple = ((r * 8.0 - t * 2.0 + a * 4.0).sin() * 0.5 + 0.5) * 0.25;
                out.put(x as i32, y as i32, cx.ramp.shade((v + ripple * v + cx.a.beat * 0.15 * (1.0 - r)).min(1.0)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_c_major_chord_sounds_like_c_major() {
        let mut c = [0.1f32; 12];
        c[0] = 1.0;
        c[4] = 0.8;
        c[7] = 0.9;
        assert_eq!(key_of(&c).0, "C major");
        let mut a = [0.1f32; 12];
        a[9] = 1.0;
        a[0] = 0.8;
        a[4] = 0.9;
        assert_eq!(key_of(&a).0, "A minor");
    }
}
