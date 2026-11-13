// Copyright 2026 ansiapps. Neumetik Visual: proprietary, see the LICENSE at the repository root.

//! ANSI art in time: frames, each held a while, as Stylus's document
//! model has them (`stylus-core/src/frames.rs`: a hold time a frame, 10 ms
//! to a minute, at most 1000 frames). A picture is an animation of one
//! frame.

use crate::canvas::Canvas;
use crate::sauce::Sauce;

/// A new frame's hold time, Stylus's default.
pub const DEFAULT_HOLD_MS: u32 = 100;
pub const MIN_HOLD_MS: u32 = 10;
pub const MAX_HOLD_MS: u32 = 60_000;
/// Most frames an animation has, as in Stylus.
pub const MAX_FRAMES: usize = 1000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub canvas: Canvas,
    pub hold_ms: u32,
}

/// Frames, all one size, and the file's SAUCE record when it had one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Animation {
    pub frames: Vec<Frame>,
    pub sauce: Option<Sauce>,
}

impl Animation {
    /// A picture: one frame.
    pub fn still(canvas: Canvas) -> Animation {
        Animation { frames: vec![Frame { canvas, hold_ms: DEFAULT_HOLD_MS }], sauce: None }
    }

    pub fn width(&self) -> usize {
        self.frames.first().map_or(0, |f| f.canvas.width)
    }

    pub fn height(&self) -> usize {
        self.frames.first().map_or(0, |f| f.canvas.height)
    }

    pub fn is_animated(&self) -> bool {
        self.frames.len() > 1
    }

    /// All the frames' hold times.
    pub fn duration_ms(&self) -> u64 {
        self.frames.iter().map(|f| u64::from(f.hold_ms)).sum()
    }

    /// Adds a frame; one the same as the last only holds the last longer.
    /// Past [`MAX_FRAMES`], nothing, and false.
    pub fn push(&mut self, canvas: Canvas, hold_ms: u32) -> bool {
        let hold_ms = hold_ms.clamp(MIN_HOLD_MS, MAX_HOLD_MS);
        if let Some(last) = self.frames.last_mut() {
            if last.canvas == canvas {
                last.hold_ms = (last.hold_ms + hold_ms).min(MAX_HOLD_MS);
                return true;
            }
        }
        if self.frames.len() >= MAX_FRAMES {
            return false;
        }
        self.frames.push(Frame { canvas, hold_ms });
        true
    }

    /// The frame showing `ms` in, played over and over.
    pub fn frame_at(&self, ms: u64) -> Option<&Canvas> {
        let total = self.duration_ms();
        if total == 0 {
            return self.frames.first().map(|f| &f.canvas);
        }
        let mut t = ms % total;
        for f in &self.frames {
            if t < u64::from(f.hold_ms) {
                return Some(&f.canvas);
            }
            t -= u64::from(f.hold_ms);
        }
        self.frames.last().map(|f| &f.canvas)
    }

    /// Every frame made one size (the largest), padded with blanks.
    pub fn even_up(&mut self) {
        let w = self.frames.iter().map(|f| f.canvas.width).max().unwrap_or(1);
        let h = self.frames.iter().map(|f| f.canvas.height).max().unwrap_or(1);
        for f in &mut self.frames {
            if f.canvas.width != w || f.canvas.height != h {
                let mut c = Canvas::new(w, h);
                c.blit(&f.canvas, 0, 0, None);
                f.canvas = c;
            }
        }
    }
}
