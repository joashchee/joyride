// Copyright 2026 ansiapps. Neumetik Visual: proprietary, see the LICENSE at the repository root.

//! The cover painter: a song's cover, 80 by 25 cells of ANSI art,
//! painted from what's known of the song. Its words (title, message,
//! samples' names) choose a motif and a palette (fire for "inferno", a
//! sunset over a grid for "highway", a planet for "orbit"...); with none
//! to go on, its format and its samples' sound do (copper bars for an
//! Amiga module, plasma for a PC one; bright palettes for bright
//! samples). OctaMED songs carry their editor's screen colors, and those
//! become the palette. A message that's already ASCII art becomes the
//! cover itself. The title goes on in big block letters, the artist under
//! it, the format along the bottom.
//!
//! It follows Stylus's rule for seeded art: the same clues paint the same
//! cover forever. The seed comes from the clues (FNV-1a), the generator is
//! SplitMix64, the drawing uses integer math (an integer sine, integer
//! value noise; no floating-point functions that differ between
//! machines), and the SAUCE comments say the painter, its version, the
//! seed and what it chose.

use crate::ansi::{self, Meta};
use crate::canvas::{Canvas, Cell, Pixels, DOUBLE, SINGLE};
use crate::cp437;
use crate::font;
use crate::palette::{luma, nearest, Ramp, BLACK, DARK_GRAY, GRAY, WHITE};
use crate::rng::{fnv1a, value_noise, SplitMix64};

pub const PAINTER: &str = "Neumetik cover painter";
pub const VERSION: u32 = 1;
pub const WIDTH: usize = 80;
pub const HEIGHT: usize = 25;

/// What's known of a song, for its cover.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Clues {
    pub title: String,
    pub artist: String,
    /// The song's message (and comments).
    pub message: String,
    /// The format's short name (`mod`, `xm`...) and in words.
    pub format: String,
    pub format_name: String,
    pub tracker: String,
    pub samples: Vec<String>,
    pub channels: usize,
    /// Beats a minute, near enough.
    pub bpm: u32,
    /// Colors the song keeps (OctaMED's screen colors), as RGB.
    pub palette: Vec<[u8; 3]>,
    /// How bright its samples sound, 0 to 100 (how often they cross zero).
    pub brightness: u32,
    /// Another painting from the same clues: 0 is the song's own, 1 up
    /// the others ("paint another").
    pub variant: u32,
}

/// What a cover is made of underneath its title.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Motif {
    Flames,
    Crystals,
    Space,
    Waves,
    Night,
    Hearts,
    Hills,
    Plasma,
    Circuit,
    Outrun,
    Mountains,
    Copper,
    Automaton,
    MessageArt,
}

impl Motif {
    pub fn id(self) -> &'static str {
        match self {
            Motif::Flames => "flames",
            Motif::Crystals => "crystals",
            Motif::Space => "space",
            Motif::Waves => "waves",
            Motif::Night => "night",
            Motif::Hearts => "hearts",
            Motif::Hills => "hills",
            Motif::Plasma => "plasma",
            Motif::Circuit => "circuit",
            Motif::Outrun => "outrun",
            Motif::Mountains => "mountains",
            Motif::Copper => "copper",
            Motif::Automaton => "automaton",
            Motif::MessageArt => "message",
        }
    }
}

/// Words that suggest a motif and a palette.
const MOODS: &[(&[&str], Motif, &str)] = &[
    (&["fire", "flame", "burn", "hot", "hell", "inferno", "lava", "heat", "blaze", "volcano", "phoenix", "dragon"], Motif::Flames, "fire"),
    (&["ice", "snow", "winter", "frost", "cold", "freez", "frozen", "crystal", "arctic", "polar", "glacier", "diamond"], Motif::Crystals, "ice"),
    (&["space", "star", "galax", "cosmic", "cosmos", "orbit", "planet", "nebula", "moon", "astro", "universe", "mars", "saturn", "comet", "alien", "ufo", "rocket", "voyage"], Motif::Space, "c64"),
    (&["sea", "ocean", "water", "wave", "rain", "aqua", "river", "dive", "island", "beach", "tide", "storm", "sail", "surf"], Motif::Waves, "ocean"),
    (&["night", "dark", "shadow", "black", "death", "doom", "evil", "blood", "grave", "ghost", "horror", "demon", "fear", "vampire", "witch"], Motif::Night, "blood"),
    (&["love", "heart", "sweet", "kiss", "dream", "girl", "baby", "angel", "romance", "valentine", "honey"], Motif::Hearts, "vaporwave"),
    (&["forest", "tree", "green", "nature", "garden", "jungle", "spring", "leaf", "flower", "wood", "meadow"], Motif::Hills, "forest"),
    (&["techno", "acid", "rave", "trance", "house", "dance", "party", "funk", "disco", "groove", "club", "jam", "boogie"], Motif::Plasma, "neon"),
    (&["machine", "robot", "cyber", "digital", "computer", "chip", "matrix", "code", "data", "virus", "hack", "tech", "electr", "binary", "byte"], Motif::Circuit, "matrix"),
    (&["city", "street", "urban", "neon", "drive", "road", "car", "outrun", "speed", "race", "highway", "miami", "sunset", "turbo"], Motif::Outrun, "sunset"),
    (&["mountain", "hill", "land", "travel", "journey", "fly", "sky", "cloud", "wind", "air", "valley", "horizon"], Motif::Mountains, "c64"),
];

/// Formats from the Amiga (and its kin), whose covers lean to copper bars.
const AMIGA: &[&str] = &["mod", "med", "mmd", "okt", "ml", "puma", "sfx", "dss", "ftm", "symmod", "dbm", "digi", "emod", "ims", "stp", "kris", "gmc", "ice", "mtn"];

/// A cover painted, with what was chosen.
#[derive(Clone, Debug)]
pub struct Painting {
    pub canvas: Canvas,
    pub motif: Motif,
    pub ramp: Ramp,
    pub seed: u64,
    pub meta: Meta,
}

/// Sine in integers: `a` in 1024ths of a turn, the result -256 to 256
/// (Bhaskara I's approximation, within a percent).
pub fn isin(a: i32) -> i32 {
    let a = a.rem_euclid(1024);
    let (x, sign) = if a < 512 { (a, 1) } else { (a - 512, -1) };
    let p = x * (512 - x);
    sign * (16 * p * 256) / (5 * 512 * 512 - 4 * p)
}

pub fn icos(a: i32) -> i32 {
    isin(a + 256)
}

/// The words in the clues, lower case.
fn words(c: &Clues) -> String {
    let mut s = format!("{} {} {}", c.title, c.message, c.samples.join(" "));
    s.make_ascii_lowercase();
    s
}

/// The message's lines when they're drawn rather than written: at least
/// four lines, a third of their characters or more not letters, digits
/// or spaces.
fn message_art(message: &str) -> Option<Vec<String>> {
    let lines: Vec<String> = message.lines().map(|l| l.trim_end().to_string()).collect();
    let start = lines.iter().position(|l| !l.trim().is_empty())?;
    let end = lines.iter().rposition(|l| !l.trim().is_empty())? + 1;
    let lines = lines[start..end].to_vec();
    if lines.len() < 4 {
        return None;
    }
    let (mut drawn, mut all) = (0, 0);
    for ch in lines.iter().flat_map(|l| l.chars()) {
        if ch != ' ' {
            all += 1;
            if !ch.is_alphanumeric() {
                drawn += 1;
            }
        }
    }
    (all >= 20 && drawn * 3 >= all).then_some(lines)
}

/// The palette: the song's own colors when it has enough, else the mood's,
/// else one its samples' brightness and the seed choose.
fn choose_ramp(c: &Clues, mood: Option<&str>, rng: &mut SplitMix64) -> Ramp {
    let mut own: Vec<u8> = c.palette.iter().map(|&rgb| nearest(rgb)).collect();
    own.sort_by_key(|&k| luma(k));
    own.dedup();
    if own.len() >= 3 {
        if own[0] != BLACK && luma(own[0]) > 60 {
            own.insert(0, BLACK);
        }
        return Ramp::custom(own);
    }
    if let Some(m) = mood {
        return Ramp::by_id(m);
    }
    let pool: &[&str] = if c.brightness >= 60 {
        &["neon", "acid", "rainbow", "amber", "vaporwave"]
    } else if c.brightness > 0 && c.brightness <= 25 {
        &["blood", "ocean", "forest", "ice", "mono"]
    } else {
        &["fire", "ice", "ocean", "acid", "neon", "sunset", "forest", "amber", "vaporwave", "c64", "ansiapps"]
    };
    Ramp::by_id(rng.pick(pool))
}

/// Paints a cover from its clues.
pub fn paint(c: &Clues) -> Painting {
    let key = format!("{}\u{1}{}\u{1}{}\u{1}{}\u{1}{}", c.title, c.artist, c.format, c.samples.join("\u{2}"), c.message);
    let key = if c.variant > 0 { format!("{key}\u{1}{}", c.variant) } else { key };
    let seed = fnv1a(key.as_bytes());
    let mut rng = SplitMix64::new(seed);
    let text = words(c);
    let art = message_art(&c.message);
    let mood = MOODS.iter().find(|(ws, _, _)| ws.iter().any(|w| text.split(|ch: char| !ch.is_alphanumeric()).any(|t| t.starts_with(w))));
    let motif = if art.is_some() {
        Motif::MessageArt
    } else if let Some(m) = mood {
        m.1
    } else if AMIGA.contains(&c.format.as_str()) {
        *rng.pick(&[Motif::Copper, Motif::Copper, Motif::Automaton, Motif::Mountains, Motif::Waves])
    } else {
        *rng.pick(&[Motif::Plasma, Motif::Automaton, Motif::Copper, Motif::Crystals, Motif::Space, Motif::Outrun, Motif::Circuit])
    };
    let ramp = choose_ramp(c, mood.map(|m| m.2), &mut rng);
    let mut canvas = Canvas::new(WIDTH, HEIGHT);
    let fast = c.bpm >= 140;
    match motif {
        Motif::Flames => flames(&mut canvas, &ramp, &mut rng),
        Motif::Crystals => crystals(&mut canvas, &ramp, &mut rng),
        Motif::Space => space(&mut canvas, &ramp, &mut rng),
        Motif::Waves => waves(&mut canvas, &ramp, &mut rng, fast),
        Motif::Night => night(&mut canvas, &ramp, &mut rng),
        Motif::Hearts => hearts(&mut canvas, &ramp, &mut rng),
        Motif::Hills => hills(&mut canvas, &ramp, &mut rng, true),
        Motif::Mountains => hills(&mut canvas, &ramp, &mut rng, false),
        Motif::Plasma => plasma(&mut canvas, &ramp, &mut rng, fast),
        Motif::Circuit => circuit(&mut canvas, &ramp, &mut rng),
        Motif::Outrun => outrun(&mut canvas, &ramp),
        Motif::Copper => copper(&mut canvas, &ramp, &mut rng, fast),
        Motif::Automaton => automaton(&mut canvas, &ramp, &mut rng),
        Motif::MessageArt => message(&mut canvas, &ramp, art.as_deref().unwrap_or(&[])),
    }
    let title_at = if motif == Motif::MessageArt { 0 } else { 3 };
    title(&mut canvas, &ramp, c, title_at, motif == Motif::MessageArt, &mut rng);
    footer(&mut canvas, c);
    let mut clue_words = Vec::new();
    if !c.title.is_empty() {
        clue_words.push("title".to_string());
    }
    if !c.samples.is_empty() {
        clue_words.push(format!("{} samples", c.samples.len()));
    }
    if c.bpm > 0 {
        clue_words.push(format!("{} bpm", c.bpm));
    }
    if !c.palette.is_empty() {
        clue_words.push("its colors".into());
    }
    let meta = Meta {
        title: c.title.clone(),
        author: c.artist.clone(),
        group: "Neumetik".into(),
        comments: vec![
            format!("Painted by {PAINTER} {VERSION}"),
            if c.variant > 0 { format!("seed {seed:016X}, variant {}", c.variant) } else { format!("seed {seed:016X}") },
            format!("motif {}, palette {}", motif.id(), ramp.id),
            format!("from {}", if clue_words.is_empty() { "nothing but the format".into() } else { clue_words.join(", ") }),
        ],
    };
    Painting { canvas, motif, ramp, seed, meta }
}

/// A cover as an ANSI file, SAUCE and all: what a UMF keeps.
pub fn cover_file(c: &Clues) -> Vec<u8> {
    let p = paint(c);
    ansi::write_ans(&p.canvas, &p.meta)
}

/// A sky from dark at the top to lighter at `to` rows, in shade steps.
fn sky(out: &mut Canvas, ramp: &Ramp, to: usize, top: u32, bottom: u32) {
    for y in 0..to.min(out.height) {
        let t = (top + (bottom - top) * y as u32 / to.max(1) as u32) as f32 / 100.0;
        for x in 0..out.width {
            out.put(x as i32, y as i32, ramp.shade(t));
        }
    }
}

fn flames(out: &mut Canvas, ramp: &Ramp, rng: &mut SplitMix64) {
    let (w, h) = (out.width, out.height + 3);
    let mut heat = vec![0u32; w * h];
    for _ in 0..90 {
        for x in 0..w {
            heat[(h - 1) * w + x] = 600 + rng.below(424) as u32;
        }
        for y in 0..h - 1 {
            for x in 0..w {
                let at = |dx: i32, dy: usize| heat[(y + dy).min(h - 1) * w + (x as i32 + dx).clamp(0, w as i32 - 1) as usize];
                let v = (at(-1, 1) + at(0, 1) + at(1, 1) + at(0, 2)) / 4;
                heat[y * w + x] = v.saturating_sub(rng.below(70) as u32);
            }
        }
    }
    for y in 0..out.height {
        for x in 0..w {
            out.put(x as i32, y as i32, ramp.shade(heat[y * w + x] as f32 / 900.0));
        }
    }
}

/// Mirrored noise in six-fold-ish symmetry (left-right and top-bottom
/// mirrors over diagonals of noise), with sparkles: a snowflake's order.
fn crystals(out: &mut Canvas, ramp: &Ramp, rng: &mut SplitMix64) {
    let s = rng.next_u64();
    let (cx, cy) = (out.width as i32 / 2, out.height as i32);
    for y in 0..out.height as i32 {
        for x in 0..out.width as i32 {
            let (dx, dy) = ((x - cx).abs(), (y * 2 - cy).abs());
            let (a, b) = if dx > dy { (dx, dy) } else { (dy, dx) };
            let n = value_noise(s, a, b, 6) as i32 + value_noise(s ^ 1, a + b, a - b, 3) as i32 / 2;
            let ring = isin((a * a + b * b) / 4) / 4 + 64;
            let v = ((n + ring) as f32 / 512.0).clamp(0.0, 1.0);
            out.put(x, y, ramp.shade(v * v));
        }
    }
    sparkle(out, ramp, rng, 40);
}

/// Stars sprinkled over dark cells.
fn sparkle(out: &mut Canvas, ramp: &Ramp, rng: &mut SplitMix64, n: usize) {
    for _ in 0..n {
        let (x, y) = (rng.below(out.width as u64) as i32, rng.below(out.height as u64) as i32);
        let ch = *rng.pick(&[cp437::DOT, cp437::DOT, cp437::BULLET, b'*', 0x0F, b'+']);
        out.with(x, y, |c| {
            if c.ch == b' ' && luma(c.bg) < 120 {
                *c = Cell::new(ch, if ch == 0x0F { WHITE } else { ramp.ink(0.7 + rng_part(ch)) }, c.bg);
            }
        });
    }
}

fn rng_part(ch: u8) -> f32 {
    if ch == b'*' {
        0.3
    } else {
        0.0
    }
}

fn space(out: &mut Canvas, ramp: &Ramp, rng: &mut SplitMix64) {
    out.clear(ramp.background());
    sparkle(out, ramp, rng, 140);
    // A planet, lit from the top left, with a ring.
    let mut p = Pixels::for_cells(out.width, out.height);
    let r = 9 + rng.range(0, 5);
    let (px, py) = (rng.range(r + 2, out.width as i32 - r - 2), rng.range(r + 8, out.height as i32 * 2 - 2));
    for y in -r..=r {
        for x in -r..=r {
            let d = x * x + y * y;
            if d <= r * r {
                // Light by how far toward the top left, with bands across.
                let light = (r * r - d) + (-x - y) * r / 2;
                let band = isin(y * 1024 / (r / 2).max(1)) / 64;
                let t = ((light + band * r) as f32 / (r * r) as f32 * 0.8).clamp(0.08, 1.0);
                p.set(px + x, py + y, ramp.color(t));
            }
        }
    }
    for k in 0..256 {
        let (rx, ry) = (r * 17 / 10, r / 3);
        let (x, y) = (icos(k * 4) * rx / 256, isin(k * 4) * ry / 256);
        // The back of the ring is behind the planet.
        if y < 0 && x * x + y * y < r * r {
            continue;
        }
        p.set(px + x, py + y, ramp.brightest());
    }
    p.draw(out);
}

fn waves(out: &mut Canvas, ramp: &Ramp, rng: &mut SplitMix64, fast: bool) {
    let horizon = out.height * 2 / 5;
    sky(out, ramp, horizon, 15, 45);
    let mut p = Pixels::for_cells(out.width, out.height);
    let sun = (rng.range(10, out.width as i32 - 10), horizon as i32 * 2 - 3);
    p.disc(sun.0, sun.1, 5, ramp.brightest());
    p.draw(out);
    let freq = if fast { 3 } else { 2 };
    let phase = rng.range(0, 1023);
    for y in horizon..out.height {
        let depth = (y - horizon) as i32;
        for x in 0..out.width as i32 {
            let v = isin(x * freq * (6 + depth) + phase + depth * 90) + isin(x * 7 - depth * 50) / 2;
            let t = 0.15 + (v + 384) as f32 / 768.0 * 0.5 - depth as f32 * 0.012;
            let crest = v > 300 - depth * 8;
            let cell = if crest { Cell::new(b'~', ramp.brightest(), ramp.shade(t).bg) } else { ramp.shade(t) };
            out.put(x, y as i32, cell);
        }
    }
}

fn night(out: &mut Canvas, ramp: &Ramp, rng: &mut SplitMix64) {
    sky(out, ramp, out.height, 0, 50);
    sparkle(out, ramp, rng, 60);
    let mut p = Pixels::for_cells(out.width, out.height);
    let (mx, my) = (rng.range(12, out.width as i32 - 12), rng.range(9, 16));
    p.disc(mx, my, 7, GRAY);
    p.disc(mx + 3, my - 2, 6, ramp.background());
    // Hills in black along the bottom.
    let s = rng.next_u64();
    for x in 0..p.width as i32 {
        let top = p.height as i32 - 6 - value_noise(s, x, 0, 12) as i32 / 24;
        for y in top..p.height as i32 {
            p.set(x, y, BLACK);
        }
    }
    p.draw(out);
}

fn hearts(out: &mut Canvas, ramp: &Ramp, rng: &mut SplitMix64) {
    sky(out, ramp, out.height, 10, 35);
    for _ in 0..50 {
        let (x, y) = (rng.below(out.width as u64) as i32, rng.below(out.height as u64) as i32);
        out.with(x, y, |c| *c = Cell::new(0x03, ramp.ink(0.5), c.bg));
    }
    // A big heart: (x² + y² - 1)³ - x²y³ ≤ 0, in sixteenths.
    let mut p = Pixels::for_cells(out.width, out.height);
    let (cx, cy, s) = (out.width as i32 / 2, out.height as i32 + 2, 14);
    for y in -s * 2..s * 2 {
        for x in -s * 2..s * 2 {
            let (fx, fy) = (x as f32 / s as f32, -y as f32 / s as f32);
            let a = fx * fx + fy * fy - 1.0;
            if a * a * a - fx * fx * fy * fy * fy <= 0.0 {
                let t = 0.55 + (-x - y) as f32 / (s * 8) as f32;
                p.set(cx + x, cy + y, ramp.color(t));
            }
        }
    }
    p.draw(out);
}

/// Layered hills (or mountains, snow-capped), nearest darkest, under a sky.
fn hills(out: &mut Canvas, ramp: &Ramp, rng: &mut SplitMix64, trees: bool) {
    sky(out, ramp, out.height, 25, 55);
    let mut p = Pixels::for_cells(out.width, out.height);
    let h = p.height as i32;
    let layers = 3;
    for k in 0..layers {
        let s = rng.next_u64();
        let base = h / 3 + k * h / 6;
        let rough = if trees { 3 } else { 1 } + k;
        let shade = ramp.colors[(ramp.colors.len() - 2 - k as usize).max(1).min(ramp.colors.len() - 1)];
        for x in 0..p.width as i32 {
            let top = base + value_noise(s, x, k, 14 / rough + 4) as i32 * (if trees { 10 } else { 24 }) / 256 - if trees { 0 } else { 8 };
            for y in top..h {
                let snow = !trees && k == 0 && y < top + 2;
                p.set(x, y, if snow { WHITE } else { shade });
            }
        }
    }
    p.draw(out);
    if trees {
        let bottom = out.height as i32 - 1;
        for x in (0..out.width as i32).step_by(3) {
            if rng.chance(0.6) {
                let y = bottom - rng.range(0, 3);
                out.with(x, y, |c| *c = Cell::new(*[0x05, 0x06, 0x18].get(rng_index(x)).unwrap_or(&0x06), ramp.ink(0.8), c.bg));
            }
        }
    }
}

fn rng_index(x: i32) -> usize {
    (x.rem_euclid(3)) as usize
}

fn plasma(out: &mut Canvas, ramp: &Ramp, rng: &mut SplitMix64, fast: bool) {
    let (a, b, c) = (rng.range(5, 14), rng.range(6, 16), rng.range(3, 9));
    let k = if fast { 2 } else { 1 };
    let phase = rng.range(0, 1023);
    for y in 0..out.height as i32 {
        for x in 0..out.width as i32 {
            let (dx, dy) = (x - 40, y * 2 - 25);
            let r = (dx * dx + dy * dy) / 8;
            let v = isin(x * a * k + phase) + isin(y * b * 2) + isin((x + y) * c * 3) + isin(r * 2 + phase);
            let t = (v + 1024) as f32 / 2048.0;
            let tri = 1.0 - (t * 4.0 % 2.0 - 1.0).abs();
            out.put(x, y, ramp.shade(tri));
        }
    }
}

fn circuit(out: &mut Canvas, ramp: &Ramp, rng: &mut SplitMix64) {
    out.clear(ramp.background());
    let dim = ramp.ink(0.2);
    // A faint grid of dots, then traces wandering across it.
    for y in (1..out.height as i32).step_by(2) {
        for x in (1..out.width as i32).step_by(4) {
            out.put(x, y, Cell::new(cp437::DOT, DARK_GRAY, ramp.background()));
        }
    }
    for _ in 0..22 {
        let (mut x, mut y) = (rng.below(out.width as u64) as i32, rng.below(out.height as u64) as i32);
        let mut dir = rng.below(4) as i32;
        let c = if rng.chance(0.3) { ramp.brightest() } else { dim };
        out.put(x, y, Cell::new(cp437::SQUARE, ramp.brightest(), ramp.background()));
        for _ in 0..rng.range(6, 30) {
            let turn = rng.chance(0.2);
            let old = dir;
            if turn {
                dir = (dir + if rng.chance(0.5) { 1 } else { 3 }) % 4;
            }
            let (dx, dy) = [(1, 0), (0, 1), (-1, 0), (0, -1)][dir as usize];
            // The corner piece joining the old direction to the new.
            let ch = if old == dir {
                if dx != 0 {
                    0xC4
                } else {
                    0xB3
                }
            } else {
                match (old, dir) {
                    (0, 1) | (3, 2) => 0xBF,
                    (0, 3) | (1, 2) => 0xD9,
                    (2, 1) | (3, 0) => 0xDA,
                    _ => 0xC0,
                }
            };
            x += dx;
            y += dy;
            if x < 0 || y < 0 || x >= out.width as i32 || y >= out.height as i32 {
                break;
            }
            out.put(x, y, Cell::new(ch, c, ramp.background()));
        }
        out.put(x, y, Cell::new(b'o', ramp.brightest(), ramp.background()));
    }
}

/// A sunset over a grid: the sun striped, the floor's lines running to it.
fn outrun(out: &mut Canvas, ramp: &Ramp) {
    let horizon = out.height as i32 * 3 / 5;
    sky(out, ramp, horizon as usize, 5, 70);
    let mut p = Pixels::for_cells(out.width, out.height);
    let (sx, sy, r): (i32, i32, i32) = (out.width as i32 / 2, horizon * 2 - 1, 13);
    for y in -r..=0 {
        // Stripes cut out of the sun's lower half, wider toward the horizon.
        if y > -r / 2 && (y.rem_euclid(4) == 0 || y > -3 && y.rem_euclid(2) == 0) {
            continue;
        }
        for x in -r..=r {
            if x * x + y * y <= r * r {
                // Yellow at the top to red at the horizon.
                p.set(sx + x, sy + y, ramp.color(1.0 - 0.45 * (y + r) as f32 / r as f32));
            }
        }
    }
    let floor = ramp.colors[1.min(ramp.colors.len() - 1)];
    let line = ramp.ink(0.35);
    for y in horizon * 2..p.height as i32 {
        for x in 0..p.width as i32 {
            p.set(x, y, BLACK);
        }
    }
    // Lines across, closer together toward the horizon.
    let mut z = 1;
    while horizon * 2 + z * z / 3 < p.height as i32 {
        let y = horizon * 2 + z * z / 3;
        for x in 0..p.width as i32 {
            p.set(x, y, line);
        }
        z += 2;
    }
    for k in -12..=12 {
        let (x0, y0) = (sx, horizon * 2);
        let (x1, y1) = (sx + k * 14, p.height as i32 - 1);
        p.line(x0 + k, y0, x1, y1, floor.max(line));
    }
    p.draw(out);
}

/// Copper bars: the Amiga's color bars, each a run up and down the palette.
fn copper(out: &mut Canvas, ramp: &Ramp, rng: &mut SplitMix64, fast: bool) {
    out.clear(ramp.background());
    let bars = if fast { 5 } else { 4 };
    let span = out.height as i32;
    let phase = rng.range(0, 1023);
    for k in 0..bars {
        let centre = span * (2 * k + 1) / (2 * bars) + isin(k * 200 + phase) * 2 / 256;
        for dy in -2i32..=2 {
            let t = 1.0 - dy.abs() as f32 / 2.5;
            for x in 0..out.width as i32 {
                out.put(x, centre + dy, ramp.shade(t));
            }
        }
    }
}

/// An elementary cellular automaton (Wolfram's rules) from a seeded row,
/// a generation a pixel row down.
fn automaton(out: &mut Canvas, ramp: &Ramp, rng: &mut SplitMix64) {
    const RULES: [u8; 8] = [30, 45, 73, 90, 105, 110, 150, 182];
    let rule = RULES[rng.below(8) as usize];
    let mut p = Pixels::for_cells(out.width, out.height);
    let w = p.width;
    let mut row: Vec<bool> = (0..w).map(|_| rng.chance(0.3)).collect();
    for y in 0..p.height {
        for (x, &on) in row.iter().enumerate() {
            p.set(x as i32, y as i32, if on { ramp.ink(y as f32 / p.height as f32) } else { ramp.background() });
        }
        row = (0..w)
            .map(|x| {
                let l = row[(x + w - 1) % w] as u8;
                let c = row[x] as u8;
                let r = row[(x + 1) % w] as u8;
                rule >> (l << 2 | c << 1 | r) & 1 == 1
            })
            .collect();
    }
    p.draw(out);
}

/// The message's own art, centered, on the palette's background.
fn message(out: &mut Canvas, ramp: &Ramp, lines: &[String]) {
    out.clear(ramp.background());
    let top = 3 + (out.height as i32 - 5 - lines.len() as i32).max(0) / 2;
    let widest = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) as i32;
    let x = (out.width as i32 - widest).max(0) / 2;
    for (i, l) in lines.iter().enumerate().take(out.height - 5) {
        let s: String = l.chars().take(out.width).collect();
        out.text(x, top + i as i32, &s, ramp.ink(0.8), ramp.background());
    }
}

/// The title in big letters on a plate across the cover, the artist
/// under it.
fn title(out: &mut Canvas, ramp: &Ramp, c: &Clues, at: i32, small: bool, rng: &mut SplitMix64) {
    let name = if c.title.trim().is_empty() { "Untitled".to_string() } else { c.title.trim().to_string() };
    let bg = ramp.background();
    let ink = ramp.brightest();
    let scale = if !small && font::width(&name, 2) <= out.width as i32 - 4 { 2 } else { 1 };
    let big = !small && font::width(&name, 1) <= out.width as i32 - 2;
    let mut y = at;
    if big {
        let rows = (font::height(scale) + 1) / 2 + 1;
        // The plate: a band of the background, edged with half blocks.
        let edge = ramp.ink(0.3);
        for x in 0..out.width as i32 {
            out.put(x, y - 1, Cell::new(cp437::LOWER, bg, out.get(x, y - 1).map_or(bg, |c| c.looks())));
            out.fill(x, y, 1, rows, Cell::new(b' ', 7, bg));
            out.put(x, y + rows, Cell::new(cp437::UPPER, bg, out.get(x, y + rows).map_or(bg, |c| c.looks())));
        }
        let mut p = Pixels::for_cells(out.width, out.height);
        let tw = font::width(&name, scale);
        font::draw(&mut p, (out.width as i32 - tw) / 2, y * 2 + 1, &name, scale, ink, Some(edge));
        p.draw(out);
        y += rows;
    } else {
        let boxed: String = name.chars().take(out.width - 6).collect();
        let w = boxed.chars().count() as i32 + 4;
        let x = (out.width as i32 - w) / 2;
        out.fill(x, y, w, 3, Cell::new(b' ', 7, bg));
        out.frame(x, y, w, 3, if rng.chance(0.5) { DOUBLE } else { SINGLE }, ramp.ink(0.5), bg);
        out.text(x + 2, y + 1, &boxed, ink, bg);
        y += 3;
    }
    if !c.artist.trim().is_empty() {
        let by = format!(" by {} ", c.artist.trim());
        let by: String = by.chars().take(out.width).collect();
        let x = (out.width as i32 - by.chars().count() as i32) / 2;
        out.text(x, y + 1, &by, ramp.ink(0.7), bg);
    }
}

/// The format along the bottom row.
fn footer(out: &mut Canvas, c: &Clues) {
    let mut parts = Vec::new();
    if !c.format_name.is_empty() {
        parts.push(c.format_name.clone());
    }
    if c.channels > 0 {
        parts.push(format!("{} channels", c.channels));
    }
    if !c.tracker.is_empty() && c.tracker != c.format_name {
        parts.push(c.tracker.clone());
    }
    if parts.is_empty() {
        return;
    }
    let s = format!(" {} ", parts.join(" · "));
    let s: String = s.chars().take(out.width).collect();
    let y = out.height as i32 - 1;
    for x in 0..out.width as i32 {
        out.put(x, y, Cell::new(b' ', 7, BLACK));
    }
    out.center(y, &s, GRAY, BLACK);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clues(title: &str) -> Clues {
        Clues {
            title: title.into(),
            artist: "Someone".into(),
            format: "xm".into(),
            format_name: "FastTracker 2".into(),
            samples: vec!["bassdrum".into(), "snare".into()],
            channels: 8,
            bpm: 125,
            ..Clues::default()
        }
    }

    #[test]
    fn the_integer_sine_is_close() {
        for a in 0..1024 {
            let exact = (a as f64 / 1024.0 * std::f64::consts::TAU).sin() * 256.0;
            assert!((f64::from(isin(a)) - exact).abs() < 2.5, "{a}");
        }
    }

    #[test]
    fn the_same_clues_paint_the_same_cover() {
        let a = paint(&clues("Inferno"));
        let b = paint(&clues("Inferno"));
        assert_eq!(a.canvas, b.canvas);
        assert_eq!(a.motif, Motif::Flames);
        assert_eq!(a.ramp.id, "fire");
        assert_ne!(paint(&clues("Inferno 2")).canvas, a.canvas);
        let other = paint(&Clues { variant: 1, ..clues("Inferno") });
        assert_ne!(other.seed, a.seed);
        assert_eq!(other.motif, Motif::Flames);
        assert!(a.meta.comments[1].starts_with("seed "));
    }

    #[test]
    fn words_choose_motifs_and_every_motif_paints() {
        for (t, m) in [("Turbo Highway", Motif::Outrun), ("Lost In Space", Motif::Space), ("Ocean Drive", Motif::Waves), ("Robot Love", Motif::Hearts)] {
            let p = paint(&clues(t));
            assert_eq!(p.motif, m, "{t}");
        }
        // The first mood in the list that any word matches wins: love before robot.
        let mut seen = std::collections::HashSet::new();
        for i in 0..400 {
            let mut c = clues(&format!("song {i}"));
            c.format = if i % 2 == 0 { "mod".into() } else { "it".into() };
            let p = paint(&c);
            seen.insert(p.motif);
            assert_eq!((p.canvas.width, p.canvas.height), (WIDTH, HEIGHT));
            assert!(p.canvas.cells.iter().all(|c| c.fg < 16 && c.bg < 16));
            let file = ansi::write_ans(&p.canvas, &p.meta);
            let back = ansi::read("ans", &file).unwrap();
            let mut expect = p.canvas.clone();
            for c in &mut expect.cells {
                if matches!(c.ch, 0x00 | 0x07..=0x0A | 0x0D | 0x1A | 0x1B) {
                    c.ch = b' ';
                }
            }
            assert_eq!(back.frames[0].canvas, expect);
        }
        assert!(seen.len() >= 6, "{seen:?}");
    }

    #[test]
    fn a_message_drawn_in_characters_is_the_cover() {
        let mut c = clues("Art");
        c.message = "   /\\_/\\  \n  ( o.o ) \n   > ^ <  \n  /|___|\\ \n ~~~~~~~~~ \n".into();
        let p = paint(&c);
        assert_eq!(p.motif, Motif::MessageArt);
        assert!(p.canvas.to_text().contains("( o.o )"));
    }

    #[test]
    fn a_songs_own_colors_become_its_palette() {
        let mut c = clues("Plain");
        c.palette = vec![[0, 0, 0], [0xFF, 0xFF, 0xFF], [0x00, 0x00, 0xAA], [0xAA, 0, 0]];
        let p = paint(&c);
        assert_eq!(p.ramp.id, "custom");
        assert_eq!(p.ramp.colors, vec![0, 1, 4, 15]);
    }
}
