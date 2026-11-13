// Copyright 2026 ansiapps. Neumetik Visual: proprietary, see the LICENSE at the repository root.

//! The demoscene's effects, played by the sound: fire, plasma, a
//! starfield, a tunnel, code rain, Conway's Life, water ripples,
//! metaballs and fireworks. Each is the well-known textbook form of the
//! effect, written here from its description.

use super::Glow;
use crate::canvas::{Canvas, Cell, Pixels};
use crate::cp437;
use crate::palette::WHITE;
use crate::rng::SplitMix64;
use crate::visual::{Ctx, Method};

/// Fire: heat seeded along the bottom by the sound (the bass most), each
/// cell above the cooled average of those under it, the wind the stereo
/// balance.
pub struct Fire {
    heat: Vec<f32>,
    w: usize,
    h: usize,
    rng: SplitMix64,
}

impl Default for Fire {
    fn default() -> Fire {
        Fire { heat: Vec::new(), w: 0, h: 0, rng: SplitMix64::new(0xF1E) }
    }
}

impl Method for Fire {
    fn bands(&self, width: usize, _height: usize) -> usize {
        width.clamp(1, 128)
    }

    fn draw(&mut self, cx: &Ctx, out: &mut Canvas) {
        let (w, h) = (out.width, out.height + 2);
        if (w, h) != (self.w, self.h) {
            (self.w, self.h) = (w, h);
            self.heat = vec![0.0; w * h];
        }
        let bands = super::resample(&cx.a.bands, w);
        let fuel = 0.2 + cx.a.energy * 0.4 + cx.a.bass * 0.35 + cx.a.beat * 0.3;
        for x in 0..w {
            let r = self.rng.unit();
            let v = if cx.a.silent { r * 0.15 } else { r * (fuel + bands[x] * 0.5) * 1.4 };
            self.heat[(h - 1) * w + x] = v.min(1.3);
        }
        let wind = (cx.a.balance * 2.0).round() as i32;
        let cool = 1.6 / h as f32 + 0.01;
        let steps = ((cx.dt * 40.0).round() as usize).clamp(1, 4);
        for _ in 0..steps {
            for y in 0..h - 1 {
                for x in 0..w {
                    let at = |dx: i32, dy: usize| {
                        let xx = (x as i32 + dx - wind).clamp(0, w as i32 - 1) as usize;
                        self.heat[(y + dy).min(h - 1) * w + xx]
                    };
                    let v = (at(-1, 1) + at(0, 1) + at(1, 1) + at(0, 2)) / 4.0;
                    self.heat[y * w + x] = (v - cool * self.rng.unit() * 2.0).max(0.0);
                }
            }
        }
        for y in 0..out.height {
            for x in 0..w {
                out.put(x as i32, y as i32, cx.ramp.shade(self.heat[y * w + x]));
            }
        }
    }
}

/// Plasma: four sines over the screen, flowing at the music's pace, the
/// colors stepping on with each beat.
#[derive(Default)]
pub struct Plasma {
    phase: f32,
    shift: f32,
}

impl Method for Plasma {
    fn draw(&mut self, cx: &Ctx, out: &mut Canvas) {
        self.phase += cx.dt * (0.6 + cx.a.energy * 2.5);
        self.shift += cx.a.beat * cx.dt * 3.0;
        let t = self.phase;
        let zoom = 0.11 + 0.05 * cx.a.bass;
        for y in 0..out.height {
            for x in 0..out.width {
                let (fx, fy) = (x as f32 * zoom, y as f32 * zoom * 2.0);
                let mut v = (fx + t).sin() + ((fy + t * 1.3) * 0.8).sin() + ((fx + fy + t * 0.7) * 0.6).sin();
                let (dx, dy) = (fx - 4.0 * (t * 0.3).sin() - out.width as f32 * zoom / 2.0, fy - 3.0 * (t * 0.4).cos() - out.height as f32 * zoom);
                v += ((dx * dx + dy * dy).sqrt() * 0.9 - t).sin();
                let level = ((v / 4.0 + 1.0) / 2.0 + self.shift).fract();
                // Up and back down the palette, so the colors never jump.
                let tri = 1.0 - (level * 2.0 - 1.0).abs();
                out.put(x as i32, y as i32, cx.ramp.shade(tri * (0.55 + 0.45 * cx.a.energy.max(0.2))));
            }
        }
    }
}

/// A starfield: stars coming at the viewer, faster with the music, drawn
/// as streaks on the beat (warp).
pub struct Starfield {
    stars: Vec<[f32; 3]>,
    rng: SplitMix64,
}

impl Default for Starfield {
    fn default() -> Starfield {
        Starfield { stars: Vec::new(), rng: SplitMix64::new(0x57A2) }
    }
}

impl Method for Starfield {
    fn draw(&mut self, cx: &Ctx, out: &mut Canvas) {
        let bg = cx.ramp.background();
        out.clear(bg);
        let want = (out.width * out.height / 10).clamp(8, 600);
        while self.stars.len() < want {
            let s = [self.rng.unit() * 2.0 - 1.0, self.rng.unit() * 2.0 - 1.0, self.rng.unit()];
            self.stars.push(s);
        }
        self.stars.truncate(want);
        let speed = 0.15 + cx.a.energy * 0.9 + cx.a.beat * 1.2;
        let (mx, my) = (out.width as f32 / 2.0, out.height as f32 / 2.0);
        let mut p = Pixels::for_cells(out.width, out.height);
        let turn = cx.a.balance * cx.dt;
        for s in &mut self.stars {
            let z0 = s[2];
            s[2] -= speed * cx.dt;
            let (sn, cs) = turn.sin_cos();
            (s[0], s[1]) = (s[0] * cs - s[1] * sn, s[0] * sn + s[1] * cs);
            if s[2] <= 0.02 {
                *s = [self.rng.unit() * 2.0 - 1.0, self.rng.unit() * 2.0 - 1.0, 1.0];
                continue;
            }
            let proj = |z: f32| ((mx + s[0] / z * mx * 0.5) as i32, (my * 2.0 + s[1] / z * my) as i32);
            let (x, y) = proj(s[2]);
            let c = cx.ramp.color(1.0 - s[2] * 0.8);
            if cx.a.beat > 0.3 {
                let (x0, y0) = proj(z0.min(s[2] + 0.12));
                p.line(x0, y0, x, y, c);
            } else {
                p.set(x, y, c);
            }
        }
        p.draw(out);
    }
}

/// A tunnel: each cell's angle and distance from the middle looking up a
/// checkered texture that moves toward the viewer and turns.
#[derive(Default)]
pub struct Tunnel {
    depth: f32,
    turn: f32,
}

impl Method for Tunnel {
    fn draw(&mut self, cx: &Ctx, out: &mut Canvas) {
        self.depth += cx.dt * (0.4 + cx.a.energy * 2.0 + cx.a.beat);
        self.turn += cx.dt * (0.1 + cx.a.balance * 1.5 + cx.a.treble * 0.3);
        let (mx, my) = (out.width as f32 / 2.0 + (cx.t as f32 * 0.7).sin() * out.width as f32 * 0.1, out.height as f32);
        for y in 0..out.height {
            for x in 0..out.width {
                let (dx, dy) = (x as f32 + 0.5 - mx, (y as f32 + 0.5) * 2.0 - my);
                let dist = (dx * dx + dy * dy).sqrt().max(0.5);
                let u = dy.atan2(dx) / std::f32::consts::PI * 4.0 + self.turn;
                let v = 24.0 / dist + self.depth * 2.0;
                let check = ((u.floor() as i32 + v.floor() as i32) & 1) as f32;
                let fog = (dist / (mx.max(my) * 0.9)).min(1.0);
                let lit = (0.25 + 0.75 * check) * fog * (0.6 + 0.4 * cx.a.energy.max(0.3));
                out.put(x as i32, y as i32, cx.ramp.shade(lit));
            }
        }
    }
}

/// Code rain: columns of falling characters, their heads bright, a burst
/// of new ones at every beat.
pub struct Rain {
    drops: Vec<(f32, f32, f32)>,
    glyphs: Vec<u8>,
    w: usize,
    h: usize,
    rng: SplitMix64,
}

impl Default for Rain {
    fn default() -> Rain {
        Rain { drops: Vec::new(), glyphs: Vec::new(), w: 0, h: 0, rng: SplitMix64::new(0x2A1) }
    }
}

/// The characters it rains: Greek, math, and box pieces from code page 437.
const RAIN: &[u8] = b"\xe0\xe1\xe2\xe3\xe4\xe5\xe6\xe7\xe8\xe9\xea\xeb\xec\xed\xee\xef\xf0\xf1\xf2\xf3\xf7\xfb\x9b\x9c\x9d\x9f0123456789ABCDEF<>=*+";

impl Method for Rain {
    fn draw(&mut self, cx: &Ctx, out: &mut Canvas) {
        let (w, h) = (out.width, out.height);
        if (w, h) != (self.w, self.h) {
            (self.w, self.h) = (w, h);
            self.glyphs = (0..w * h).map(|_| *self.rng.pick(RAIN)).collect();
            self.drops.clear();
        }
        let rate = 0.3 + cx.a.energy * 2.0;
        let mut spawn = rate * cx.dt * w as f32 / 8.0;
        if cx.a.onset {
            spawn += w as f32 / 14.0;
        }
        while spawn > 0.0 {
            if spawn >= 1.0 || self.rng.unit() < spawn {
                let x = self.rng.below(w as u64) as f32;
                let speed = 6.0 + self.rng.unit() * 14.0;
                self.drops.push((x, -1.0, speed));
            }
            spawn -= 1.0;
        }
        self.drops.truncate(w * 3 / 2);
        // Characters change now and then.
        for _ in 0..(w * h / 40).max(1) {
            let i = self.rng.below((w * h) as u64) as usize;
            self.glyphs[i] = *self.rng.pick(RAIN);
        }
        let bg = cx.ramp.background();
        // The old trail fades: each frame, cells step down the palette.
        let fade = (cx.dt * 6.0).min(1.0);
        for c in &mut out.cells {
            if self.rng.unit() < fade {
                let i = cx.ramp.colors.iter().position(|&k| k == c.fg).unwrap_or(0);
                if i <= 1 {
                    *c = Cell::new(b' ', 7, bg);
                } else {
                    c.fg = cx.ramp.colors[i - 1];
                }
            }
            c.bg = bg;
        }
        let tail = cx.ramp.ink(0.6);
        for d in &mut self.drops {
            let y0 = d.1 as i32;
            d.1 += d.2 * cx.dt * (0.7 + cx.a.energy);
            let y1 = d.1 as i32;
            for y in y0.max(0)..=y1.min(h as i32 - 1) {
                let x = d.0 as i32;
                let ch = self.glyphs[y as usize * w + x as usize];
                out.put(x, y, Cell::new(ch, if y == y1 { WHITE } else { tail }, bg));
            }
            if y1 >= 0 && y1 < h as i32 {
                let ch = self.glyphs[y1 as usize * w + d.0 as usize];
                out.put(d.0 as i32, y1, Cell::new(ch, WHITE, bg));
            }
        }
        self.drops.retain(|d| d.1 < h as f32 + 1.0);
    }
}

/// Conway's Game of Life in half-block pixels: a generation a step (more
/// steps a second when it's loud), new life seeded where the spectrum is
/// strong at every beat, cells colored by age and fading when they die.
pub struct Life {
    cells: Vec<u8>,
    trail: Vec<f32>,
    w: usize,
    h: usize,
    carry: f32,
    rng: SplitMix64,
}

impl Default for Life {
    fn default() -> Life {
        Life { cells: Vec::new(), trail: Vec::new(), w: 0, h: 0, carry: 0.0, rng: SplitMix64::new(0x11FE) }
    }
}

impl Life {
    fn seed(&mut self, x: usize, y: usize) {
        // An R-pentomino or a glider, turned at random.
        const SHAPES: [&[(i32, i32)]; 2] = [&[(1, 0), (2, 0), (0, 1), (1, 1), (1, 2)], &[(1, 0), (2, 1), (0, 2), (1, 2), (2, 2)]];
        let shape = SHAPES[self.rng.below(2) as usize];
        let (fx, fy) = (if self.rng.chance(0.5) { -1 } else { 1 }, if self.rng.chance(0.5) { -1 } else { 1 });
        for &(dx, dy) in shape {
            let xx = (x as i32 + dx * fx).rem_euclid(self.w as i32) as usize;
            let yy = (y as i32 + dy * fy).rem_euclid(self.h as i32) as usize;
            self.cells[yy * self.w + xx] = 1;
        }
    }

    fn step(&mut self) {
        let (w, h) = (self.w, self.h);
        let mut next = vec![0u8; w * h];
        for y in 0..h {
            for x in 0..w {
                let mut n = 0;
                for dy in [h - 1, 0, 1] {
                    for dx in [w - 1, 0, 1] {
                        if (dx, dy) != (0, 0) && self.cells[(y + dy) % h * w + (x + dx) % w] > 0 {
                            n += 1;
                        }
                    }
                }
                let age = self.cells[y * w + x];
                next[y * w + x] = match (age > 0, n) {
                    (true, 2 | 3) => age.saturating_add(1),
                    (false, 3) => 1,
                    _ => 0,
                };
            }
        }
        self.cells = next;
    }
}

impl Method for Life {
    fn draw(&mut self, cx: &Ctx, out: &mut Canvas) {
        let (w, h) = (out.width, out.height * 2);
        if (w, h) != (self.w, self.h) {
            (self.w, self.h) = (w, h);
            self.cells = vec![0; w * h];
            self.trail = vec![0.0; w * h];
            for _ in 0..(w * h / 60).max(1) {
                let (x, y) = (self.rng.below(w as u64) as usize, self.rng.below(h as u64) as usize);
                self.seed(x, y);
            }
        }
        if cx.a.onset || self.cells.iter().all(|&c| c == 0) {
            let n = cx.a.bands.len().max(1);
            for _ in 0..(2 + (cx.a.energy * 6.0) as usize) {
                // Where the spectrum is strongest, across; anywhere, down.
                let mut best = 0;
                for _ in 0..4 {
                    let b = self.rng.below(n as u64) as usize;
                    if cx.a.bands.get(b) > cx.a.bands.get(best) {
                        best = b;
                    }
                }
                let x = best * w / n;
                let y = self.rng.below(h as u64) as usize;
                self.seed(x, y);
            }
        }
        self.carry += cx.dt * (6.0 + cx.a.energy * 18.0);
        while self.carry >= 1.0 {
            self.carry -= 1.0;
            self.step();
        }
        let k = (-cx.dt * 3.0).exp();
        let mut p = Pixels::for_cells(out.width, out.height);
        for i in 0..w * h {
            let age = self.cells[i];
            self.trail[i] = if age > 0 { 1.0 } else { self.trail[i] * k };
            p.px[i] = if age > 0 {
                cx.ramp.ink(1.0 - (f32::from(age.min(40)) / 40.0) * 0.7)
            } else if self.trail[i] > 0.1 {
                cx.ramp.color(self.trail[i] * 0.35)
            } else {
                cx.ramp.background()
            };
        }
        p.draw(out);
    }
}

/// Water: the 2D wave equation on two height maps (the effect as Hugo
/// Elias described it), a drop where the notes are at every onset, rain
/// with the treble; shaded by the slope, as light through water.
pub struct Ripple {
    a: Vec<f32>,
    b: Vec<f32>,
    w: usize,
    h: usize,
    carry: f32,
    rng: SplitMix64,
}

impl Default for Ripple {
    fn default() -> Ripple {
        Ripple { a: Vec::new(), b: Vec::new(), w: 0, h: 0, carry: 0.0, rng: SplitMix64::new(0xAA7E2) }
    }
}

impl Ripple {
    fn drop(&mut self, x: usize, y: usize, r: i32, depth: f32) {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy <= r * r {
                    let (xx, yy) = (x as i32 + dx, y as i32 + dy);
                    if xx > 0 && yy > 0 && (xx as usize) < self.w - 1 && (yy as usize) < self.h - 1 {
                        self.a[yy as usize * self.w + xx as usize] += depth;
                    }
                }
            }
        }
    }
}

impl Method for Ripple {
    fn draw(&mut self, cx: &Ctx, out: &mut Canvas) {
        let (w, h) = (out.width.max(3), (out.height * 2).max(3));
        if (w, h) != (self.w, self.h) {
            (self.w, self.h) = (w, h);
            self.a = vec![0.0; w * h];
            self.b = vec![0.0; w * h];
        }
        if cx.a.onset {
            let n = cx.a.bands.len().max(1);
            let loud = (0..n).max_by(|&i, &j| cx.a.bands[i].total_cmp(&cx.a.bands[j])).unwrap_or(0);
            let x = 1 + (loud * (w - 2) / n).min(w - 3);
            let y = 1 + self.rng.below((h - 2) as u64) as usize;
            self.drop(x, y, 1 + (cx.a.bass * 3.0) as i32, 3.0 + cx.a.energy * 4.0);
        }
        if self.rng.unit() < cx.a.treble * cx.dt * 6.0 {
            let (x, y) = (1 + self.rng.below((w - 2) as u64) as usize, 1 + self.rng.below((h - 2) as u64) as usize);
            self.drop(x, y, 1, 1.5);
        }
        if cx.a.silent && self.rng.unit() < cx.dt * 0.5 {
            let (x, y) = (1 + self.rng.below((w - 2) as u64) as usize, 1 + self.rng.below((h - 2) as u64) as usize);
            self.drop(x, y, 1, 2.0);
        }
        self.carry += cx.dt * 40.0;
        while self.carry >= 1.0 {
            self.carry -= 1.0;
            for y in 1..h - 1 {
                for x in 1..w - 1 {
                    let i = y * w + x;
                    let v = (self.a[i - 1] + self.a[i + 1] + self.a[i - w] + self.a[i + w]) / 2.0 - self.b[i];
                    self.b[i] = v * 0.95;
                }
            }
            std::mem::swap(&mut self.a, &mut self.b);
        }
        let mut p = Pixels::for_cells(out.width, out.height);
        for y in 0..p.height.min(h) {
            for x in 0..p.width.min(w) {
                let i = y * w + x;
                let slope = if x + 1 < w && y + 1 < h { self.a[i] - self.a[i + 1] + self.a[i] - self.a[i + w] } else { 0.0 };
                p.px[y * p.width + x] = cx.ramp.color(0.35 + slope * 0.1);
            }
        }
        p.draw(out);
    }
}

/// Metaballs: a ball for each part of the spectrum on its own Lissajous
/// path, swelling with its level; where their fields add up past a
/// threshold, they're drawn, melting into each other.
#[derive(Default)]
pub struct Metaballs;

impl Method for Metaballs {
    fn bands(&self, _width: usize, _height: usize) -> usize {
        6
    }

    fn draw(&mut self, cx: &Ctx, out: &mut Canvas) {
        let (w, h) = (out.width as f32, out.height as f32 * 2.0);
        let size = w.min(h).max(2.0);
        let t = cx.t as f32;
        let balls: Vec<(f32, f32, f32)> =
            cx.a.bands
                .iter()
                .enumerate()
                .map(|(i, &v)| {
                    let k = i as f32 + 1.0;
                    let x = w / 2.0 + (t * 0.31 * k + k).sin() * w * 0.35;
                    let y = h / 2.0 + (t * 0.23 * (7.0 - k) + k * 2.0).cos() * h * 0.33;
                    (x, y, size * (0.06 + 0.16 * v + 0.04 * cx.a.beat))
                })
                .collect();
        for yy in 0..out.height {
            for xx in 0..out.width {
                let (px, py) = (xx as f32 + 0.5, (yy as f32 + 0.5) * 2.0);
                let field: f32 = balls.iter().map(|&(x, y, r)| r * r / ((px - x).powi(2) + (py - y).powi(2) + 1.0)).sum();
                let v = if field > 1.0 { 0.6 + (field - 1.0).min(1.0) * 0.4 } else { field * 0.45 };
                out.put(xx as i32, yy as i32, cx.ramp.shade(v));
            }
        }
    }
}

/// Fireworks: a rocket at every beat, bursting at its top into sparks
/// that fall and fade, their colors the music's brightness.
pub struct Fireworks {
    sparks: Vec<Spark>,
    glow: Glow,
    rng: SplitMix64,
}

struct Spark {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: f32,
    rocket: bool,
    color: u8,
}

impl Default for Fireworks {
    fn default() -> Fireworks {
        Fireworks { sparks: Vec::new(), glow: Glow::default(), rng: SplitMix64::new(0xF12E) }
    }
}

impl Method for Fireworks {
    fn draw(&mut self, cx: &Ctx, out: &mut Canvas) {
        self.glow.fit(out);
        self.glow.fade(cx.dt, 7.0);
        let (w, h) = (self.glow.width as f32, self.glow.height as f32);
        let quiet_launch = cx.a.silent && self.rng.unit() < cx.dt * 0.7;
        let rockets = self.sparks.iter().filter(|s| s.rocket).count();
        if (cx.a.onset || quiet_launch || self.sparks.is_empty() && self.rng.unit() < cx.dt * 2.0) && rockets < 3 && self.sparks.len() < 600 {
            let x = w * (0.15 + 0.7 * self.rng.unit());
            let color = cx.ramp.ink(0.4 + 0.6 * self.rng.unit());
            self.sparks.push(Spark { x, y: h - 1.0, vx: (self.rng.unit() - 0.5) * 6.0, vy: -h * (0.9 + 0.4 * self.rng.unit()), life: 0.6 + cx.a.energy * 0.3, rocket: true, color });
        }
        let g = h * 1.2;
        let mut born = Vec::new();
        for s in &mut self.sparks {
            let (x0, y0) = (s.x, s.y);
            s.vy += g * cx.dt;
            s.x += s.vx * cx.dt;
            s.y += s.vy * cx.dt;
            s.life -= cx.dt;
            if s.rocket {
                self.glow.line(x0 as i32, y0 as i32, s.x as i32, s.y as i32, 0.6);
            } else {
                self.glow.hit(s.x as i32, s.y as i32, s.life.clamp(0.2, 1.0));
            }
            if s.rocket && (s.vy >= 0.0 || s.life <= 0.0) {
                s.life = 0.0;
                let n = 14 + (cx.a.brightness * 18.0) as usize;
                for k in 0..n {
                    let a = k as f32 / n as f32 * std::f32::consts::TAU + self.rng.unit() * 0.2;
                    let sp = h * (0.25 + 0.35 * self.rng.unit());
                    born.push(Spark { x: s.x, y: s.y, vx: a.cos() * sp * 1.6, vy: a.sin() * sp, life: 0.8 + self.rng.unit() * 0.8, rocket: false, color: s.color });
                }
            }
        }
        self.sparks.extend(born);
        self.sparks.retain(|s| s.life > 0.0 && s.y < h + 2.0);
        out.clear(cx.ramp.background());
        self.glow.draw(out, cx.ramp);
        for s in &self.sparks {
            if !s.rocket && s.life > 0.5 {
                let (x, y) = (s.x as i32, s.y as i32 / 2);
                out.with(x, y, |c| {
                    if c.ch == cp437::UPPER || c.ch == b' ' {
                        *c = Cell::new(if s.life > 1.1 { b'*' } else { cp437::BULLET }, s.color, c.bg);
                    }
                });
            }
        }
    }
}
