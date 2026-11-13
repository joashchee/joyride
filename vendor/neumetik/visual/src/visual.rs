// Copyright 2026 ansiapps. Neumetik Visual: proprietary, see the LICENSE at the repository root.

//! The visualizer: the sound analyzed ([`crate::analysis`]) and drawn by
//! one of the methods ([`KINDS`]) on a canvas of any size, a frame at a
//! time, in a palette; what it's showing kept as settings in a line of
//! text (what a UMF's VISL chunk holds), and what it shows recorded as
//! an ANSImation to take into Stylus.

use std::sync::Arc;

use crate::analysis::{Analysis, Analyzer, Response};
use crate::anim::{Animation, MAX_FRAMES};
use crate::canvas::Canvas;
use crate::methods;
use crate::palette::Ramp;

/// A tracker channel as it plays, for the methods that show channels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Voice {
    /// Its level, 0 to 1.
    pub level: f32,
    /// The note it last played, when it has one: 0 is C-0, as trackers
    /// name them (middle C, C-5, is 60).
    pub note: Option<u8>,
    /// The instrument (or sample) that played it, 1 up, 0 none.
    pub instrument: u16,
    pub muted: bool,
}

/// What's playing, a frame's worth.
#[derive(Clone, Copy, Debug)]
pub struct Input<'a> {
    /// The latest samples each side, newest last ([`crate::analysis::FFT_SIZE`] is plenty).
    pub left: &'a [f32],
    pub right: &'a [f32],
    /// How many of them are new since the last frame.
    pub fresh: usize,
    pub rate: u32,
    /// Seconds since the last frame.
    pub dt: f32,
    /// The tracker's channels, when a module is playing.
    pub voices: &'a [Voice],
    /// The song's title, and words to scroll (its message, its samples' names).
    pub title: &'a str,
    pub words: &'a str,
    pub playing: bool,
}

impl Input<'_> {
    /// Silence: a frame with nothing playing.
    pub fn silent(dt: f32) -> Input<'static> {
        Input { left: &[], right: &[], fresh: 0, rate: 48_000, dt, voices: &[], title: "", words: "", playing: false }
    }
}

/// What a method draws from.
pub struct Ctx<'a> {
    pub a: &'a Analysis,
    pub input: &'a Input<'a>,
    pub ramp: &'a Ramp,
    pub settings: &'a Settings,
    /// Seconds of the visual's own time (sped up or slowed by the
    /// settings' speed), and since the last frame.
    pub t: f64,
    pub dt: f32,
    /// A picture or animation to show (the cover, or art opened).
    pub art: Option<&'a Animation>,
}

/// A way of showing the sound.
pub trait Method: Send {
    /// Draws a frame onto the canvas (already its size; what was there is
    /// the last frame, cleared or not as the method likes).
    fn draw(&mut self, cx: &Ctx, out: &mut Canvas);
    /// How many spectrum bands it wants for a canvas this size.
    fn bands(&self, _width: usize, _height: usize) -> usize {
        64
    }
}

/// The kinds of method, for grouping in a menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    /// The hi-fi's: spectrum, scope, meters.
    Classic,
    /// Other ways of drawing the spectrum and the wave.
    Analysis,
    /// The demoscene's effects, played by the sound.
    Demo,
    /// The song's own: its channels, its title, its cover.
    Song,
}

/// A method, by name.
pub struct Kind {
    pub id: &'static str,
    pub name: &'static str,
    pub group: Group,
    /// What it shows, in a sentence.
    pub about: &'static str,
    /// The palette it starts in.
    pub palette: &'static str,
    pub make: fn() -> Box<dyn Method>,
}

/// Every method.
pub const KINDS: &[Kind] = &[
    Kind {
        id: "bars",
        name: "Spectrum Bars",
        group: Group::Classic,
        about: "The spectrum in bars, low notes left, with falling peaks.",
        palette: "classic",
        make: || Box::new(methods::spectrum::Bars::default()),
    },
    Kind { id: "led", name: "LED Analyzer", group: Group::Classic, about: "A hi-fi's LED spectrum analyzer: lit and unlit segments.", palette: "classic", make: || Box::new(methods::spectrum::Led) },
    Kind {
        id: "scope",
        name: "Oscilloscope",
        group: Group::Classic,
        about: "The waveform, held still by a trigger, on a glowing screen.",
        palette: "matrix",
        make: || Box::new(methods::wave::Scope::default()),
    },
    Kind { id: "vu", name: "VU Meters", group: Group::Classic, about: "Needle VU meters and peak bars for each side, in dB.", palette: "amber", make: || Box::new(methods::meters::Vu) },
    Kind { id: "deck", name: "Hi-Fi Deck", group: Group::Classic, about: "Spectrum, scope and meters on one screen.", palette: "classic", make: || Box::new(methods::meters::Deck::default()) },
    Kind {
        id: "spectrogram",
        name: "Spectrogram",
        group: Group::Analysis,
        about: "The spectrum through time, scrolling: a waterfall.",
        palette: "fire",
        make: || Box::new(methods::spectrum::Spectrogram::default()),
    },
    Kind {
        id: "ridges",
        name: "Ridgelines",
        group: Group::Analysis,
        about: "Spectra stacked as ridges, the newest in front.",
        palette: "mono",
        make: || Box::new(methods::spectrum::Ridges::default()),
    },
    Kind {
        id: "radial",
        name: "Radial Spectrum",
        group: Group::Analysis,
        about: "The spectrum around a circle that swells with the bass.",
        palette: "neon",
        make: || Box::new(methods::spectrum::Radial),
    },
    Kind {
        id: "vectorscope",
        name: "Vectorscope",
        group: Group::Analysis,
        about: "Left against right: the stereo picture, and the correlation.",
        palette: "ice",
        make: || Box::new(methods::wave::Vectorscope::default()),
    },
    Kind { id: "polar", name: "Polar Scope", group: Group::Analysis, about: "The waveform wrapped around a circle.", palette: "vaporwave", make: || Box::new(methods::wave::Polar::default()) },
    Kind {
        id: "phase",
        name: "Phase Space",
        group: Group::Analysis,
        about: "The wave against itself a moment before: its attractor.",
        palette: "ocean",
        make: || Box::new(methods::wave::Phase::default()),
    },
    Kind { id: "chroma", name: "Chromagram", group: Group::Analysis, about: "The twelve notes' shares, and the key they suggest.", palette: "rainbow", make: || Box::new(methods::spectrum::Chroma) },
    Kind { id: "kaleidoscope", name: "Kaleidoscope", group: Group::Analysis, about: "The spectrum turned and mirrored six ways.", palette: "neon", make: || Box::new(methods::spectrum::Kaleidoscope) },
    Kind { id: "fire", name: "Fire", group: Group::Demo, about: "Flames fed by the sound, fanned by its balance.", palette: "fire", make: || Box::new(methods::demo::Fire::default()) },
    Kind { id: "plasma", name: "Plasma", group: Group::Demo, about: "Sine plasma, flowing faster as the music gets louder.", palette: "sunset", make: || Box::new(methods::demo::Plasma::default()) },
    Kind {
        id: "starfield",
        name: "Starfield",
        group: Group::Demo,
        about: "Flying through stars, faster with the music; warp on the beat.",
        palette: "mono",
        make: || Box::new(methods::demo::Starfield::default()),
    },
    Kind {
        id: "tunnel",
        name: "Tunnel",
        group: Group::Demo,
        about: "Down a tunnel at the music's speed, turning with its balance.",
        palette: "ice",
        make: || Box::new(methods::demo::Tunnel::default()),
    },
    Kind { id: "rain", name: "Code Rain", group: Group::Demo, about: "Falling characters, a burst at every beat.", palette: "matrix", make: || Box::new(methods::demo::Rain::default()) },
    Kind {
        id: "life",
        name: "Game of Life",
        group: Group::Demo,
        about: "Conway's Life, seeded by the beats, a generation a step.",
        palette: "acid",
        make: || Box::new(methods::demo::Life::default()),
    },
    Kind { id: "ripple", name: "Ripples", group: Group::Demo, about: "Water rippling where the notes fall.", palette: "ocean", make: || Box::new(methods::demo::Ripple::default()) },
    Kind {
        id: "metaballs",
        name: "Metaballs",
        group: Group::Demo,
        about: "Blobs swelling with their part of the spectrum, melting together.",
        palette: "vaporwave",
        make: || Box::new(methods::demo::Metaballs::default()),
    },
    Kind {
        id: "fireworks",
        name: "Fireworks",
        group: Group::Demo,
        about: "A rocket at every beat, bursting in the music's brightness.",
        palette: "rainbow",
        make: || Box::new(methods::demo::Fireworks::default()),
    },
    Kind {
        id: "channels",
        name: "Tracker Channels",
        group: Group::Song,
        about: "Each channel's level, note and instrument as the module plays.",
        palette: "classic",
        make: || Box::new(methods::meters::Channels),
    },
    Kind {
        id: "title",
        name: "Title Scroller",
        group: Group::Song,
        about: "The title in big letters and the song's words in a sine scroller.",
        palette: "rainbow",
        make: || Box::new(methods::show::Title::default()),
    },
    Kind {
        id: "art",
        name: "Cover Art",
        group: Group::Song,
        about: "The song's cover (or art opened from Stylus), lit up by the beat.",
        palette: "classic",
        make: || Box::new(methods::show::Art::default()),
    },
];

pub fn kind(id: &str) -> &'static Kind {
    KINDS.iter().find(|k| k.id == id).unwrap_or(&KINDS[0])
}

/// What the visual shows and how, as a line of text:
/// `bars;palette=fire;gain=1.5;smooth=0.5;speed=1;peaks=1;mirror=0;labels=1`.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub method: String,
    pub palette: String,
    /// Sensitivity: the sound's gain before it's analyzed, 0.25 to 4.
    pub gain: f32,
    /// How slowly levels fall, 0 to 1.
    pub smoothing: f32,
    /// How fast the effects move, 0.25 to 4.
    pub speed: f32,
    pub peaks: bool,
    /// Mirrored (or, for scopes, each side apart).
    pub mirror: bool,
    /// Scales, names and numbers drawn.
    pub labels: bool,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings::for_method("bars")
    }
}

impl Settings {
    /// A method's settings, in its own palette.
    pub fn for_method(id: &str) -> Settings {
        let k = kind(id);
        Settings { method: k.id.into(), palette: k.palette.into(), gain: 1.0, smoothing: 0.5, speed: 1.0, peaks: true, mirror: false, labels: true }
    }

    /// Reads the line [`Settings::to_line`] writes; what it doesn't know
    /// (or can't read) is left as the method's own.
    pub fn parse(line: &str) -> Settings {
        let mut parts = line.split(';').map(str::trim);
        let mut s = Settings::for_method(parts.next().unwrap_or("bars"));
        for p in parts {
            let Some((k, v)) = p.split_once('=') else { continue };
            let num = |lo: f32, hi: f32, d: f32| v.parse::<f32>().ok().filter(|x| x.is_finite()).map_or(d, |x| x.clamp(lo, hi));
            let flag = |d: bool| match v {
                "1" | "true" | "on" => true,
                "0" | "false" | "off" => false,
                _ => d,
            };
            match k {
                "palette" if crate::palette::RAMPS.iter().any(|r| r.0 == v) => s.palette = v.into(),
                "gain" => s.gain = num(0.25, 4.0, s.gain),
                "smooth" => s.smoothing = num(0.0, 1.0, s.smoothing),
                "speed" => s.speed = num(0.25, 4.0, s.speed),
                "peaks" => s.peaks = flag(s.peaks),
                "mirror" => s.mirror = flag(s.mirror),
                "labels" => s.labels = flag(s.labels),
                _ => {}
            }
        }
        s
    }

    pub fn to_line(&self) -> String {
        let b = |v: bool| u8::from(v);
        format!(
            "{};palette={};gain={};smooth={};speed={};peaks={};mirror={};labels={}",
            self.method,
            self.palette,
            round(self.gain),
            round(self.smoothing),
            round(self.speed),
            b(self.peaks),
            b(self.mirror),
            b(self.labels)
        )
    }
}

fn round(v: f32) -> f32 {
    (v * 100.0).round() / 100.0
}

/// A recording under way.
struct Recording {
    anim: Animation,
    /// Milliseconds not yet given to a frame.
    carry: f64,
}

/// Draws the sound, a frame at a time.
pub struct Visualizer {
    settings: Settings,
    ramp: Ramp,
    analyzer: Analyzer,
    method: Box<dyn Method>,
    art: Option<Arc<Animation>>,
    t: f64,
    canvas: Canvas,
    recording: Option<Recording>,
}

impl Default for Visualizer {
    fn default() -> Visualizer {
        Visualizer::new(Settings::default())
    }
}

impl Visualizer {
    pub fn new(settings: Settings) -> Visualizer {
        let method = (kind(&settings.method).make)();
        Visualizer { ramp: Ramp::by_id(&settings.palette), settings, analyzer: Analyzer::new(), method, art: None, t: 0.0, canvas: Canvas::new(80, 25), recording: None }
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// New settings; a new method starts fresh.
    pub fn set_settings(&mut self, settings: Settings) {
        if settings.method != self.settings.method {
            self.method = (kind(&settings.method).make)();
        }
        self.ramp = Ramp::by_id(&settings.palette);
        self.settings = settings;
    }

    /// The picture or animation the art method shows (and the others may).
    pub fn set_art(&mut self, art: Option<Arc<Animation>>) {
        self.art = art;
    }

    pub fn art(&self) -> Option<&Arc<Animation>> {
        self.art.as_ref()
    }

    pub fn analysis(&self) -> &Analysis {
        &self.analyzer.out
    }

    /// The next frame, `width` by `height` cells.
    pub fn frame(&mut self, input: &Input, width: usize, height: usize) -> &Canvas {
        self.canvas.resize(width, height);
        let bands = self.method.bands(self.canvas.width, self.canvas.height);
        let response = Response { gain: self.settings.gain, smoothing: self.settings.smoothing };
        self.analyzer.update(input.left, input.right, input.fresh, input.rate, input.dt, bands, response);
        let dt = input.dt.clamp(0.0, 0.25) * self.settings.speed;
        self.t += f64::from(dt);
        let cx = Ctx { a: &self.analyzer.out, input, ramp: &self.ramp, settings: &self.settings, t: self.t, dt, art: self.art.as_deref() };
        self.method.draw(&cx, &mut self.canvas);
        if let Some(r) = self.recording.as_mut() {
            r.carry += f64::from(input.dt) * 1000.0;
            let hold = r.carry.round().max(10.0) as u32;
            if r.anim.push(self.canvas.clone(), hold) {
                r.carry -= f64::from(hold);
            }
        }
        &self.canvas
    }

    pub fn canvas(&self) -> &Canvas {
        &self.canvas
    }

    /// Starts keeping every frame drawn, as an animation.
    pub fn start_recording(&mut self) {
        self.recording = Some(Recording { anim: Animation::default(), carry: 0.0 });
    }

    /// Frames kept so far, and whether it's full ([`MAX_FRAMES`]).
    pub fn recorded(&self) -> Option<(usize, bool)> {
        self.recording.as_ref().map(|r| (r.anim.frames.len(), r.anim.frames.len() >= MAX_FRAMES))
    }

    /// The frames kept, and the recording stopped.
    pub fn stop_recording(&mut self) -> Option<Animation> {
        self.recording.take().map(|r| r.anim)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_and_forgive_what_they_dont_know() {
        let mut s = Settings::for_method("fire");
        s.gain = 2.5;
        s.mirror = true;
        assert_eq!(Settings::parse(&s.to_line()), s);
        let odd = Settings::parse("nonesuch;palette=nope;gain=99;smooth=x;what=1");
        assert_eq!(odd.method, "bars");
        assert_eq!(odd.palette, "classic");
        assert_eq!(odd.gain, 4.0);
        assert_eq!(odd.smoothing, 0.5);
    }

    fn tone(n: usize, f: f32) -> Vec<f32> {
        (0..n).map(|i| 0.5 * (std::f32::consts::TAU * f * i as f32 / 48_000.0).sin() * if (i / 6000) % 2 == 0 { 1.0 } else { 0.2 }).collect()
    }

    #[test]
    fn every_method_draws_at_every_size_with_sound_and_without() {
        let wave = tone(48_000, 220.0);
        let voices = [Voice { level: 0.7, note: Some(60), instrument: 3, muted: false }, Voice::default()];
        let mut art = Canvas::new(10, 4);
        art.text(0, 0, "ART", 14, 1);
        let art = Arc::new(Animation::still(art));
        for k in KINDS {
            for (w, h) in [(1, 1), (8, 3), (40, 12), (80, 25), (160, 45)] {
                let mut v = Visualizer::new(Settings::for_method(k.id));
                v.set_art(Some(art.clone()));
                for f in 0..40 {
                    let end = 2048 + f * 800;
                    let input = Input {
                        left: &wave[end - 2048..end],
                        right: &wave[end - 2048..end],
                        fresh: 800,
                        rate: 48_000,
                        dt: 1.0 / 60.0,
                        voices: &voices,
                        title: "Test Song",
                        words: "hello world",
                        playing: true,
                    };
                    let c = v.frame(&input, w, h);
                    assert_eq!((c.width, c.height), (w, h), "{}", k.id);
                    assert!(c.cells.iter().all(|c| c.fg < 16 && c.bg < 16), "{}", k.id);
                }
                if w >= 40 {
                    assert!(!v.canvas().is_plain(), "{} drew nothing at {w}x{h}", k.id);
                }
                for _ in 0..3 {
                    v.frame(&Input::silent(1.0 / 60.0), w, h);
                }
            }
        }
    }

    #[test]
    fn a_recording_keeps_the_frames_with_their_times() {
        let mut v = Visualizer::new(Settings::for_method("plasma"));
        v.start_recording();
        for _ in 0..30 {
            v.frame(&Input::silent(0.05), 20, 6);
        }
        let anim = v.stop_recording().unwrap();
        assert!(anim.frames.len() > 10);
        let total = anim.duration_ms();
        assert!((1450..=1550).contains(&total), "{total}");
    }
}
