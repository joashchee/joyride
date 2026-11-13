// Copyright 2026 ansiapps. Neumetik Visual: proprietary, see the LICENSE at the repository root.

//! Meters: VU meters with needles and peak bars, the hi-fi deck (bars,
//! scope and meters together), and the tracker's channels.

use super::spectrum::Bars;
use super::wave::Scope;
use super::{note_name, resample};
use crate::analysis::FLOOR_DB;
use crate::canvas::{Canvas, Cell, Pixels};
use crate::cp437;
use crate::palette::{DARK_GRAY, GRAY, LIGHT_RED, WHITE};
use crate::visual::{Ctx, Method};

/// Where 0 VU sits: 10 dB under full scale, so mastered music swings
/// around it rather than pinning the needle (a studio's -18 dBFS
/// alignment is for tracks with headroom; the Sensitivity moves it).
const ALIGN_DB: f32 = -10.0;

/// The VU scale: the dial runs -20 to +3 VU, spaced by voltage as a real
/// meter's is.
fn needle_t(vu_db: f32) -> f32 {
    let vu = vu_db - ALIGN_DB;
    let v = |db: f32| 10f32.powf(db / 20.0);
    ((v(vu.clamp(-25.0, 4.0)) - v(-20.0)) / (v(3.0) - v(-20.0))).clamp(-0.02, 1.04)
}

/// One needle meter in a box of cells.
fn dial(out: &mut Canvas, cx: &Ctx, x: i32, y: i32, w: i32, h: i32, side: usize, name: &str) {
    if w < 8 || h < 4 {
        return;
    }
    let bg = cx.ramp.background();
    let face = cx.ramp.ink(0.0);
    out.frame(x, y, w, h, crate::canvas::SINGLE, face, bg);
    let mut p = Pixels::for_cells(out.width, out.height);
    // The pivot under the box's bottom middle; the needle sweeps ±50°.
    let (px, py) = ((x * 2 + w) as f32 / 2.0, ((y + h) * 2) as f32);
    let r = ((w as f32 / 2.0 - 1.5) / 50f32.to_radians().sin()).min((h * 2) as f32 - 3.0);
    let angle = |t: f32| (-50.0 + 100.0 * t).to_radians();
    let marks = [-20.0, -10.0, -7.0, -5.0, -3.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0];
    for m in marks {
        let a = angle(needle_t(m + ALIGN_DB));
        let c = if m > 0.0 { LIGHT_RED } else { cx.ramp.ink(0.6) };
        for k in 0..2 {
            let rr = r - 1.0 - k as f32;
            p.set((px + a.sin() * rr) as i32, (py - a.cos() * rr) as i32, c);
        }
    }
    let tip = angle(needle_t(cx.a.vu_db[side]));
    p.line(px as i32, py as i32 - 1, (px + tip.sin() * (r - 1.0)) as i32, (py - tip.cos() * (r - 1.0)) as i32, cx.ramp.brightest());
    p.draw(out);
    if cx.settings.labels && h >= 5 {
        let label = |out: &mut Canvas, m: f32, s: &str| {
            let a = angle(needle_t(m + ALIGN_DB));
            let lx = (px + a.sin() * (r - 3.5)) as i32 - s.len() as i32 / 2;
            let ly = ((py - a.cos() * (r - 3.5)) / 2.0) as i32;
            if ly > y && ly < y + h - 1 {
                out.text_over(lx.max(x + 1), ly, s, if m > 0.0 { LIGHT_RED } else { GRAY });
            }
        };
        label(out, -20.0, "-20");
        label(out, -7.0, "-7");
        label(out, 0.0, "0");
        label(out, 3.0, "+3");
        out.text_over(x + 2, y + h - 2, name, GRAY);
        out.text_over(x + w - 4, y + h - 2, "VU", GRAY);
    }
}

/// Peak bars for both sides, on rows from `y`: each a bar to the peak
/// level, the VU's level brighter within it, the held peak, and clip.
fn bars(out: &mut Canvas, cx: &Ctx, y: i32, labels: bool) {
    let bg = cx.ramp.background();
    let w = out.width as i32;
    let x0 = if labels { 2 } else { 0 };
    let tail = if labels { 9 } else { 0 };
    let len = (w - x0 - tail).max(1);
    for side in 0..2 {
        let row = y + side as i32;
        if labels {
            out.text(0, row, ["L", "R"][side], GRAY, bg);
        }
        let ppm = (cx.a.ppm[side] * len as f32) as i32;
        let vu = (cx.a.vu[side] * len as f32) as i32;
        let hold = (cx.a.hold[side] * len as f32) as i32;
        for k in 0..len {
            let t = k as f32 / len as f32;
            let c = cx.ramp.ink(t);
            let cell = if k < vu {
                Cell::new(cp437::FULL, c, bg)
            } else if k < ppm {
                Cell::new(cp437::DARK, c, bg)
            } else if cx.settings.peaks && k == hold.min(len - 1) && hold > 0 {
                Cell::new(cp437::LEFT, WHITE, bg)
            } else {
                Cell::new(cp437::DOT, DARK_GRAY, bg)
            };
            out.put(x0 + k, row, cell);
        }
        if labels {
            let db = cx.a.ppm_db[side];
            let s = if db <= FLOOR_DB + 0.5 { "  -inf".to_string() } else { format!("{db:6.1}") };
            out.text(w - tail + 1, row, &s, GRAY, bg);
            out.put(w - 1, row, Cell::new(cp437::FULL, if cx.a.clip[side] > 0.0 { LIGHT_RED } else { DARK_GRAY }, bg));
        }
    }
    if labels && out.height as i32 > y + 2 {
        for db in [-48.0f32, -36.0, -24.0, -12.0, -6.0, 0.0] {
            let k = ((db - FLOOR_DB) / -FLOOR_DB * len as f32) as i32;
            let s = format!("{}", db as i32);
            out.text((x0 + k - s.len() as i32 / 2).clamp(0, w - s.len() as i32), y + 2, &s, DARK_GRAY, bg);
        }
    }
}

/// Needle VU meters for each side above peak bars; only the bars when
/// it's small.
pub struct Vu;

impl Method for Vu {
    fn draw(&mut self, cx: &Ctx, out: &mut Canvas) {
        out.clear(cx.ramp.background());
        let (w, h) = (out.width as i32, out.height as i32);
        let labels = cx.settings.labels && w >= 24;
        let bar_rows = if labels && h >= 6 { 3 } else { 2.min(h) };
        let dial_h = h - bar_rows - 1;
        if dial_h >= 5 && w >= 24 {
            let dw = (w / 2 - 1).min(dial_h * 4);
            let gap = (w - 2 * dw) / 3;
            dial(out, cx, gap, 0, dw, dial_h, 0, "LEFT");
            dial(out, cx, w - gap - dw, 0, dw, dial_h, 1, "RIGHT");
        }
        bars(out, cx, h - bar_rows, labels);
    }
}

/// The hi-fi deck: spectrum bars above, the scope and the meters below.
#[derive(Default)]
pub struct Deck {
    bars: Bars,
    scope: Scope,
}

impl Method for Deck {
    fn bands(&self, width: usize, height: usize) -> usize {
        self.bars.bands(width, height)
    }

    fn draw(&mut self, cx: &Ctx, out: &mut Canvas) {
        let bg = cx.ramp.background();
        out.clear(bg);
        let (w, h) = (out.width, out.height);
        if h < 6 || w < 20 {
            self.bars.draw(cx, out);
            return;
        }
        let top = h * 3 / 5;
        let mut spectrum = Canvas::new(w, top);
        self.bars.draw(cx, &mut spectrum);
        out.blit(&spectrum, 0, 0, None);
        let rest = h - top;
        let half = w / 2;
        let mut scope = Canvas::new(half.saturating_sub(1).max(1), rest);
        self.scope.draw(cx, &mut scope);
        out.blit(&scope, 0, top as i32, None);
        let mut meters = Canvas::new(w - half, rest);
        meters.clear(bg);
        let labels = cx.settings.labels;
        bars(&mut meters, cx, (rest as i32 - 3).max(0) / 2, labels);
        out.blit(&meters, half as i32, top as i32, None);
        for y in top..h {
            out.put(half as i32 - 1, y as i32, Cell::new(0xB3, DARK_GRAY, bg));
        }
    }
}

/// Each tracker channel as a column: its note and instrument on top and
/// its level as a bar; muted channels gray. With no tracker playing, the
/// spectrum's bars stand in.
pub struct Channels;

impl Method for Channels {
    fn draw(&mut self, cx: &Ctx, out: &mut Canvas) {
        let voices = cx.input.voices;
        if voices.is_empty() {
            Bars.draw(cx, out);
            return;
        }
        let bg = cx.ramp.background();
        out.clear(bg);
        let w = out.width;
        let shown = voices.len().min((w / 3).max(1));
        let cw = (w / shown).max(1);
        let x0 = (w - cw * shown) as i32 / 2;
        let labels = cx.settings.labels && cw >= 3 && out.height >= 6;
        let head = if labels { 2 } else { 0 };
        let rows = out.height.saturating_sub(head).max(1);
        let mut p = Pixels::for_cells(w, out.height);
        let hp = (rows * 2) as i32;
        let levels = resample(&voices.iter().map(|v| v.level).collect::<Vec<_>>(), voices.len());
        for (i, v) in voices.iter().take(shown).enumerate() {
            let x = x0 + (i * cw) as i32;
            let level = (levels[i] * 1.4).min(1.0);
            let h = (level * hp as f32) as i32;
            for k in 0..h {
                let c = if v.muted { DARK_GRAY } else { cx.ramp.ink(k as f32 / hp as f32) };
                p.rect(x, (head * 2) as i32 + hp - 1 - k, cw.saturating_sub(1).max(1) as i32, 1, c);
            }
            if labels {
                let note = v.note.map_or("---".to_string(), note_name);
                let fg = if v.muted { DARK_GRAY } else { cx.ramp.brightest() };
                out.text(x, 0, &crate::canvas::fit(&note, cw.saturating_sub(1).max(1)), fg, bg);
                let ins = if v.instrument > 0 { format!("{:02X}", v.instrument.min(255)) } else { "..".into() };
                out.text(x, 1, &crate::canvas::fit(&ins, cw.saturating_sub(1).max(1)), if v.muted { DARK_GRAY } else { GRAY }, bg);
            }
        }
        p.draw(out);
    }
}

#[cfg(test)]
mod tests {
    use super::{needle_t, ALIGN_DB};

    #[test]
    fn the_needle_sits_at_its_scale_marks() {
        assert!(needle_t(ALIGN_DB - 20.0).abs() < 1e-4);
        assert!((needle_t(ALIGN_DB + 3.0) - 1.0).abs() < 1e-4);
        let zero = needle_t(ALIGN_DB);
        assert!(zero > 0.6 && zero < 0.75, "{zero}");
    }
}
