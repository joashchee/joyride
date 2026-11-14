// Copyright 2026 ansiapps. Neumetik for the R36S: proprietary, see the LICENSE at the repository root.

//! **Neumetik for the R36S**: what the handheld's host (`r36s/host/`,
//! C and SDL2) needs from Neumetik, behind a small C API
//! (`neumetik_r36s.h`): a file opened as a [`Song`] (a MIDI file, music as
//! text, a tracker module) or a BGN scene, an [`Engine`] that plays one
//! on the audio thread, and a [`Vis`], Neumetik Visual drawing what
//! plays into the host's cells.
//!
//! Threads: the host calls [`nm36_render`] from SDL's audio callback and
//! everything else that touches the engine with the audio device locked
//! (`SDL_LockAudioDevice`). Loading a song and drawing a frame take the
//! time; neither needs the lock, so the audio never waits for them:
//! [`nm36_song_load`] makes a song apart from the engine, [`nm36_play`]
//! swaps it in, and [`nm36_vis_capture`] (locked, a copy) comes before
//! [`nm36_vis_draw`] (unlocked).
//!
//! A replayer that panics on a strange file never takes the host down:
//! loading returns an error, and a panic while playing stops the song and
//! says so in [`Nm36Info`].

pub mod bench;
pub mod gen;

use std::ffi::{c_char, CStr, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::sync::{Arc, OnceLock};

use neumetik_tracker::Module;
#[cfg(feature = "visual")]
use neumetik_visual::visual::{Input, Settings, Visualizer, Voice, KINDS};

/// Samples a side kept for Visual: its analysis's window.
#[cfg(feature = "visual")]
pub const RING: usize = neumetik_visual::analysis::FFT_SIZE;
/// Without Visual nothing reads the ring; the same size keeps the engine one shape.
#[cfg(not(feature = "visual"))]
pub const RING: usize = 2048;
/// Frames rendered at a time.
const BLOCK: usize = 512;

pub const NM36_NONE: i32 = 0;
pub const NM36_MIDI: i32 = 1;
pub const NM36_MODULE: i32 = 2;
pub const NM36_BGN: i32 = 3;

enum Source {
    Midi(Box<neumetik_midi::Player>),
    Module(Box<neumetik_tracker::Player>, Arc<Module>),
    #[cfg(feature = "bgn")]
    Bgn(Box<neumetik_bgn::Engine>),
}

/// Something to play, made apart from the engine (it can take a while).
pub struct Song {
    source: Source,
    title: String,
    /// What Visual's title scroller shows: the song's message, its samples' names.
    words: String,
    format: String,
    /// Seconds once through; 0 when it plays for ever (BGN).
    duration: f64,
    channels: usize,
}

impl Song {
    pub fn kind(&self) -> i32 {
        match self.source {
            Source::Midi(_) => NM36_MIDI,
            Source::Module(..) => NM36_MODULE,
            #[cfg(feature = "bgn")]
            Source::Bgn(_) => NM36_BGN,
        }
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn format(&self) -> &str {
        &self.format
    }

    pub fn duration(&self) -> f64 {
        self.duration
    }

    /// Channels it plays on (a MIDI file's 16, a module's own count).
    pub fn channels(&self) -> usize {
        self.channels
    }

    /// Orders in a module's song (0 for anything else).
    pub fn orders(&self) -> usize {
        match &self.source {
            Source::Module(_, m) => m.orders.len(),
            _ => 0,
        }
    }

    /// What a module says about itself (its message), or a scene's note; empty for a MIDI file.
    pub fn words(&self) -> &str {
        &self.words
    }

    /// A MIDI file's bytes.
    pub fn midi(bytes: &[u8], title: &str, format: &str, rate: u32) -> Result<Song, String> {
        let song = neumetik_midi::Song::from_midi(bytes).map_err(|e| e.to_string())?;
        let duration = song.duration();
        let mut p = neumetik_midi::Player::new(rate);
        p.load_song(song);
        p.play();
        Ok(Song { source: Source::Midi(Box::new(p)), title: title.to_string(), words: String::new(), format: format.to_string(), duration, channels: 16 })
    }

    /// A tracker module, read already.
    pub fn module(module: Module, title: &str, rate: u32) -> Song {
        let module = Arc::new(module);
        let duration = neumetik_tracker::player::length(module.clone(), 3600.0);
        let mut p = neumetik_tracker::Player::new(module.clone(), rate);
        p.repeat = false;
        let name = if module.title.trim().is_empty() { title.to_string() } else { module.title.trim().to_string() };
        let words = module.message.clone();
        let channels = module.channels.len();
        let format = format!("{} {}ch", if module.format_name.is_empty() { module.format.to_uppercase() } else { module.format_name.clone() }, channels);
        Song { source: Source::Module(Box::new(p), module), title: name, words, format, duration, channels }
    }

    /// A scene from BGN's bank, by its place in `neumetik_bgn::BANK`.
    #[cfg(feature = "bgn")]
    pub fn bgn(preset: usize, rate: u32) -> Option<Song> {
        let p = neumetik_bgn::BANK.get(preset)?;
        let mut e = neumetik_bgn::Engine::new(rate);
        e.set_scene(&p.scene());
        e.set_playing(true);
        Some(Song { source: Source::Bgn(Box::new(e)), title: p.name.to_string(), words: p.note.to_string(), format: format!("BGN {}", p.group), duration: 0.0, channels: 4 })
    }

    /// A file on disk, whatever it is: a MIDI file (by its header), a
    /// tracker module in any format the Tracker reads (its companion
    /// files from the same folder), or music as text (MusicXML, ABC,
    /// ChordPro, MML...), played as MIDI.
    pub fn open(path: &Path, rate: u32) -> Result<Song, String> {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let title = path.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let ext = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase());
        if bytes.starts_with(b"MThd") || (bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"RMID")) {
            return Song::midi(&bytes, &title, "MIDI", rate);
        }
        // Extensions that are only ever text; anything else is the Tracker's first.
        let text_only = matches!(
            ext.as_deref(),
            Some("musicxml" | "mxl" | "xml" | "abc" | "cho" | "chordpro" | "chopro" | "crd" | "pro" | "mini" | "strudel" | "tidal" | "rtttl" | "rtx" | "krn" | "alda" | "tmf" | "ms2mml" | "flmml" | "sion")
        );
        let as_text = |why: String| -> Result<Song, String> {
            if neumetik_midi::text::detect(&bytes, ext.as_deref()).is_none() {
                return Err(why);
            }
            let midi = neumetik_midi::text::to_midi(&bytes, ext.as_deref()).map_err(|e| e.to_string())?;
            Song::midi(&midi, &title, "Text", rate)
        };
        if text_only {
            return as_text("not a file Neumetik reads".to_string());
        }
        match neumetik_tracker::load_with(&bytes, &name, &neumetik_tracker::beside(path)) {
            Ok(m) => Ok(Song::module(m, &title, rate)),
            Err(e) => as_text(e.to_string()),
        }
    }

    fn render(&mut self, l: &mut [f32], r: &mut [f32]) {
        l.fill(0.0);
        r.fill(0.0);
        match &mut self.source {
            Source::Midi(p) => p.render(l, r),
            Source::Module(p, _) => p.render(l, r),
            #[cfg(feature = "bgn")]
            Source::Bgn(e) => e.render(l, r),
        }
    }

    fn finished(&self) -> bool {
        match &self.source {
            Source::Midi(p) => p.is_finished(),
            Source::Module(p, _) => p.ended(),
            #[cfg(feature = "bgn")]
            Source::Bgn(_) => false,
        }
    }

    fn set_looping(&mut self, on: bool) {
        match &mut self.source {
            Source::Midi(p) => p.set_looping(on),
            Source::Module(p, _) => p.repeat = on,
            #[cfg(feature = "bgn")]
            Source::Bgn(_) => {}
        }
    }
}

/// What plays, on the audio thread.
pub struct Engine {
    rate: u32,
    song: Option<Box<Song>>,
    paused: bool,
    looping: bool,
    volume: f32,
    /// Frames played since the song started (a module's time played; a MIDI file's own position is its player's).
    frames: u64,
    ended: bool,
    error: String,
    ring_l: Vec<f32>,
    ring_r: Vec<f32>,
    ring_at: usize,
    written: u64,
    l: Vec<f32>,
    r: Vec<f32>,
}

impl Engine {
    pub fn new(rate: u32) -> Engine {
        Engine {
            rate: rate.clamp(8000, 96_000),
            song: None,
            paused: false,
            looping: false,
            volume: 0.8,
            frames: 0,
            ended: false,
            error: String::new(),
            ring_l: vec![0.0; RING],
            ring_r: vec![0.0; RING],
            ring_at: 0,
            written: 0,
            l: vec![0.0; BLOCK],
            r: vec![0.0; BLOCK],
        }
    }

    pub fn rate(&self) -> u32 {
        self.rate
    }

    /// Plays `song` from its start; the one it replaces comes back, to be dropped outside the audio lock.
    pub fn play(&mut self, song: Option<Box<Song>>) -> Option<Box<Song>> {
        let old = std::mem::replace(&mut self.song, song);
        if let Some(s) = self.song.as_mut() {
            s.set_looping(self.looping);
        }
        self.paused = false;
        self.frames = 0;
        self.ended = false;
        self.error.clear();
        old
    }

    pub fn set_looping(&mut self, on: bool) {
        self.looping = on;
        if let Some(s) = self.song.as_mut() {
            s.set_looping(on);
        }
    }

    pub fn set_volume(&mut self, v: f32) {
        self.volume = v.clamp(0.0, 1.0);
    }

    pub fn set_paused(&mut self, on: bool) {
        self.paused = on;
    }

    /// `step` places on: ten seconds of a MIDI file, an order of a module. BGN has no place.
    pub fn seek(&mut self, step: i32) {
        let rate = f64::from(self.rate);
        let Some(s) = self.song.as_mut() else { return };
        match &mut s.source {
            Source::Midi(p) => {
                let to = (p.position() + f64::from(step) * 10.0).clamp(0.0, p.duration().max(0.0));
                p.seek(to);
                self.frames = (to * rate) as u64;
            }
            Source::Module(p, m) => {
                let last = m.orders.len().saturating_sub(1) as i64;
                let to = (p.position().order as i64 + i64::from(step)).clamp(0, last) as usize;
                p.seek(to, 0);
            }
            #[cfg(feature = "bgn")]
            Source::Bgn(_) => {}
        }
        self.ended = false;
    }

    /// Jumps to `seconds` from the start (nttplay's `-ss`). A MIDI file jumps; a module or a scene has no place to jump to
    /// by time, so it is rendered and the sound thrown away (14 to 35 times faster than real time on the unit).
    pub fn seek_to(&mut self, seconds: f64) {
        let rate = f64::from(self.rate);
        let target = (seconds.max(0.0) * rate) as u64;
        if let Some(Source::Midi(p)) = self.song.as_mut().map(|s| &mut s.source) {
            let to = seconds.clamp(0.0, p.duration().max(0.0));
            p.seek(to);
            self.frames = (to * rate) as u64;
            self.ended = false;
            return;
        }
        let mut sink = vec![0.0f32; 4096 * 2];
        while self.frames < target && !self.ended && self.song.is_some() {
            let n = ((target - self.frames) as usize).min(4096);
            self.render(&mut sink[..n * 2]);
        }
    }

    /// It came to its end (not looping) or its replayer failed (then [`Engine::error`] says why).
    pub fn has_ended(&self) -> bool {
        self.ended
    }

    /// Why a song stopped, empty when it did not fail.
    pub fn error(&self) -> &str {
        &self.error
    }

    /// The next `out.len() / 2` frames, left and right interleaved.
    pub fn render(&mut self, out: &mut [f32]) {
        let mut done = 0;
        let frames = out.len() / 2;
        while done < frames {
            let n = (frames - done).min(BLOCK);
            let playing = !self.paused && !self.ended && self.song.is_some();
            if playing {
                let song = self.song.as_mut().unwrap();
                let (l, r) = (&mut self.l[..n], &mut self.r[..n]);
                if catch_unwind(AssertUnwindSafe(|| song.render(l, r))).is_err() {
                    self.error = format!("{} stopped: its replayer failed", song.title);
                    self.ended = true;
                    self.l[..n].fill(0.0);
                    self.r[..n].fill(0.0);
                } else {
                    self.frames += n as u64;
                    if !self.looping && song.finished() {
                        self.ended = true;
                    }
                }
            } else {
                self.l[..n].fill(0.0);
                self.r[..n].fill(0.0);
            }
            let v = self.volume * self.volume; // the control is heard as loudness
            for i in 0..n {
                let (a, b) = ((self.l[i] * v).clamp(-1.0, 1.0), (self.r[i] * v).clamp(-1.0, 1.0));
                out[(done + i) * 2] = a;
                out[(done + i) * 2 + 1] = b;
                self.ring_l[self.ring_at] = a;
                self.ring_r[self.ring_at] = b;
                self.ring_at = (self.ring_at + 1) % RING;
            }
            self.written += n as u64;
            done += n;
        }
    }

    pub fn info(&self) -> Nm36Info {
        let mut i = Nm36Info::default();
        i.paused = self.paused as i32;
        i.ended = self.ended as i32;
        i.looping = self.looping as i32;
        i.volume = self.volume;
        copy_c(&mut i.error, &self.error);
        let Some(s) = self.song.as_ref() else { return i };
        i.kind = s.kind();
        i.playing = (!self.paused && !self.ended) as i32;
        i.duration = s.duration;
        i.channels = s.channels as i32;
        i.position = self.frames as f64 / f64::from(self.rate);
        match &s.source {
            Source::Midi(p) => i.position = p.position(),
            Source::Module(p, m) => {
                let pos = p.position();
                i.order = pos.order as i32;
                i.orders = m.orders.len() as i32;
                i.row = pos.row as i32;
            }
            #[cfg(feature = "bgn")]
            Source::Bgn(_) => {}
        }
        copy_c(&mut i.title, &s.title);
        copy_c(&mut i.format, &s.format);
        i
    }
}

/// What the host shows of what plays.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Nm36Info {
    pub kind: i32,
    pub playing: i32,
    pub paused: i32,
    /// It came to its end (and is not looping), or its replayer failed (`error`).
    pub ended: i32,
    pub looping: i32,
    pub volume: f32,
    /// Seconds: a MIDI file's place in it; for a module, the time it has played.
    pub position: f64,
    /// Seconds once through; 0 unknown or for ever.
    pub duration: f64,
    pub order: i32,
    pub orders: i32,
    pub row: i32,
    pub channels: i32,
    pub title: [c_char; 128],
    pub format: [c_char; 64],
    pub error: [c_char; 128],
}

impl Default for Nm36Info {
    fn default() -> Nm36Info {
        Nm36Info {
            kind: NM36_NONE,
            playing: 0,
            paused: 0,
            ended: 0,
            looping: 0,
            volume: 0.0,
            position: 0.0,
            duration: 0.0,
            order: 0,
            orders: 0,
            row: 0,
            channels: 0,
            title: [0; 128],
            format: [0; 64],
            error: [0; 128],
        }
    }
}

/// `s` into a C buffer, cut at a character boundary, always ended with a 0.
fn copy_c(out: &mut [c_char], s: &str) {
    let Some(room) = out.len().checked_sub(1) else { return };
    let mut n = s.len().min(room);
    while !s.is_char_boundary(n) {
        n -= 1;
    }
    for (o, b) in out.iter_mut().zip(s.as_bytes()[..n].iter()) {
        *o = *b as c_char;
    }
    out[n] = 0;
}

/// Visual, drawing what the engine played into cells.
#[cfg(feature = "visual")]
pub struct Vis {
    vis: Visualizer,
    l: Vec<f32>,
    r: Vec<f32>,
    fresh: usize,
    seen: u64,
    rate: u32,
    levels: Vec<f32>,
    voices: Vec<Voice>,
    title: String,
    words: String,
    playing: bool,
}

#[cfg(feature = "visual")]
impl Vis {
    pub fn new(kind: usize) -> Vis {
        let id = KINDS.get(kind).map_or("bars", |k| k.id);
        Vis {
            vis: Visualizer::new(Settings::for_method(id)),
            l: vec![0.0; RING],
            r: vec![0.0; RING],
            fresh: 0,
            seen: 0,
            rate: 44_100,
            levels: Vec::new(),
            voices: Vec::new(),
            title: String::new(),
            words: String::new(),
            playing: false,
        }
    }

    pub fn set_kind(&mut self, kind: usize) {
        if let Some(k) = KINDS.get(kind) {
            self.vis.set_settings(Settings::for_method(k.id));
        }
    }

    /// Copies what's new from the engine (quick: call it with the audio locked).
    pub fn capture(&mut self, e: &Engine) {
        let at = e.ring_at;
        self.l[..RING - at].copy_from_slice(&e.ring_l[at..]);
        self.l[RING - at..].copy_from_slice(&e.ring_l[..at]);
        self.r[..RING - at].copy_from_slice(&e.ring_r[at..]);
        self.r[RING - at..].copy_from_slice(&e.ring_r[..at]);
        self.fresh = (e.written - self.seen.min(e.written)).min(RING as u64) as usize;
        self.seen = e.written;
        self.rate = e.rate;
        self.playing = !e.paused && !e.ended && e.song.is_some();
        self.voices.clear();
        match e.song.as_deref() {
            Some(s) => {
                if self.title != s.title {
                    self.title.clone_from(&s.title);
                    self.words.clone_from(&s.words);
                }
                if let Source::Module(p, m) = &s.source {
                    self.levels.resize(m.channels.len(), 0.0);
                    p.levels(&mut self.levels);
                    self.voices.extend(self.levels.iter().enumerate().map(|(i, &level)| Voice { level, note: None, instrument: 0, muted: m.channels[i].muted }));
                }
            }
            None => {
                self.title.clear();
                self.words.clear();
            }
        }
    }

    /// The next frame, `w` by `h` cells, as code page 437 characters and VGA colours.
    pub fn draw(&mut self, dt: f32, w: usize, h: usize) -> &neumetik_visual::canvas::Canvas {
        let input = Input { left: &self.l, right: &self.r, fresh: self.fresh, rate: self.rate, dt, voices: &self.voices, title: &self.title, words: &self.words, playing: self.playing };
        self.fresh = 0;
        self.vis.frame(&input, w, h)
    }
}

// ---- the C API (neumetik_r36s.h) -------------------------------------------------------------------------------

fn c_str<'a>(p: *const c_char) -> Option<&'a str> {
    if p.is_null() {
        return None;
    }
    // SAFETY: the host passes a 0-ended string that outlives the call.
    unsafe { CStr::from_ptr(p) }.to_str().ok()
}

fn put_err(err: *mut c_char, cap: usize, msg: &str) {
    if err.is_null() || cap == 0 {
        return;
    }
    // SAFETY: the host gives a buffer of `cap` bytes.
    let out = unsafe { std::slice::from_raw_parts_mut(err, cap) };
    copy_c(out, msg);
}

#[cfg(any(feature = "bgn", feature = "visual"))]
fn static_names(cell: &'static OnceLock<Vec<CString>>, make: impl FnOnce() -> Vec<String>) -> &'static [CString] {
    cell.get_or_init(|| make().into_iter().map(|s| CString::new(s.replace('\0', " ")).unwrap_or_default()).collect())
}

#[cfg(any(feature = "bgn", feature = "visual"))]
fn name_at(list: &'static [CString], i: i32) -> *const c_char {
    usize::try_from(i).ok().and_then(|i| list.get(i)).map_or(std::ptr::null(), |c| c.as_ptr())
}

#[no_mangle]
pub extern "C" fn nm36_version() -> *const c_char {
    static V: OnceLock<CString> = OnceLock::new();
    V.get_or_init(|| CString::new(env!("CARGO_PKG_VERSION")).unwrap()).as_ptr()
}

#[no_mangle]
pub extern "C" fn nm36_new(rate: u32) -> *mut Engine {
    Box::into_raw(Box::new(Engine::new(rate)))
}

/// # Safety
/// `e` is from `nm36_new` (or null), and not used after.
#[no_mangle]
pub unsafe extern "C" fn nm36_free(e: *mut Engine) {
    if !e.is_null() {
        drop(Box::from_raw(e));
    }
}

/// Opens a file to play at `rate`; null and a message in `err` when it can't.
#[no_mangle]
pub extern "C" fn nm36_song_load(path: *const c_char, rate: u32, err: *mut c_char, cap: usize) -> *mut Song {
    let Some(path) = c_str(path) else {
        put_err(err, cap, "no file name");
        return std::ptr::null_mut();
    };
    match catch_unwind(|| Song::open(Path::new(path), rate)) {
        Ok(Ok(s)) => Box::into_raw(Box::new(s)),
        Ok(Err(m)) => {
            put_err(err, cap, &m);
            std::ptr::null_mut()
        }
        Err(_) => {
            put_err(err, cap, "this file made the reader fail");
            std::ptr::null_mut()
        }
    }
}

/// BGN's scene `preset` (0 up, `nm36_bgn_count` of them), to play at `rate`.
#[no_mangle]
#[cfg(feature = "bgn")]
pub extern "C" fn nm36_bgn_song(preset: i32, rate: u32) -> *mut Song {
    usize::try_from(preset).ok().and_then(|p| catch_unwind(|| Song::bgn(p, rate)).ok().flatten()).map_or(std::ptr::null_mut(), |s| Box::into_raw(Box::new(s)))
}

/// # Safety
/// `s` is from `nm36_song_load`, `nm36_bgn_song`, `nm36_play` or `nm36_stop` (or null), and not used after.
#[no_mangle]
pub unsafe extern "C" fn nm36_song_free(s: *mut Song) {
    if !s.is_null() {
        drop(Box::from_raw(s));
    }
}

/// # Safety
/// `s` is a live song (or null).
#[no_mangle]
pub unsafe extern "C" fn nm36_song_kind(s: *const Song) -> i32 {
    s.as_ref().map_or(NM36_NONE, Song::kind)
}

/// Plays `s` (the engine takes it); the song it replaces comes back for `nm36_song_free`, outside the audio lock.
///
/// # Safety
/// `e` is a live engine; `s` a live song not given before (or null to play nothing).
#[no_mangle]
pub unsafe extern "C" fn nm36_play(e: *mut Engine, s: *mut Song) -> *mut Song {
    let Some(e) = e.as_mut() else { return s };
    let song = (!s.is_null()).then(|| Box::from_raw(s));
    e.play(song).map_or(std::ptr::null_mut(), Box::into_raw)
}

/// # Safety
/// `e` is a live engine.
#[no_mangle]
pub unsafe extern "C" fn nm36_stop(e: *mut Engine) -> *mut Song {
    nm36_play(e, std::ptr::null_mut())
}

/// # Safety
/// `e` is a live engine.
#[no_mangle]
pub unsafe extern "C" fn nm36_pause(e: *mut Engine, paused: i32) {
    if let Some(e) = e.as_mut() {
        e.set_paused(paused != 0);
    }
}

/// # Safety
/// `e` is a live engine.
#[no_mangle]
pub unsafe extern "C" fn nm36_seek(e: *mut Engine, step: i32) {
    if let Some(e) = e.as_mut() {
        e.seek(step);
    }
}

/// # Safety
/// `e` is a live engine.
#[no_mangle]
pub unsafe extern "C" fn nm36_set_looping(e: *mut Engine, on: i32) {
    if let Some(e) = e.as_mut() {
        e.set_looping(on != 0);
    }
}

/// 0 to 1.
///
/// # Safety
/// `e` is a live engine.
#[no_mangle]
pub unsafe extern "C" fn nm36_set_volume(e: *mut Engine, v: f32) {
    if let Some(e) = e.as_mut() {
        e.set_volume(v);
    }
}

/// `frames` stereo frames into `out` (2 x `frames` floats, left then right).
///
/// # Safety
/// `e` is a live engine; `out` holds 2 x `frames` floats.
#[no_mangle]
pub unsafe extern "C" fn nm36_render(e: *mut Engine, out: *mut f32, frames: usize) {
    if out.is_null() {
        return;
    }
    let out = std::slice::from_raw_parts_mut(out, frames * 2);
    match e.as_mut() {
        Some(e) => e.render(out),
        None => out.fill(0.0),
    }
}

/// # Safety
/// `e` is a live engine; `out` a writable `Nm36Info`.
#[no_mangle]
pub unsafe extern "C" fn nm36_info(e: *const Engine, out: *mut Nm36Info) {
    if let (Some(e), Some(out)) = (e.as_ref(), out.as_mut()) {
        *out = e.info();
    }
}

#[no_mangle]
#[cfg(feature = "bgn")]
pub extern "C" fn nm36_bgn_count() -> i32 {
    neumetik_bgn::BANK.len() as i32
}

/// A scene's name, its section ("Nature") and its subsection ("Weather"); null past the end.
#[no_mangle]
#[cfg(feature = "bgn")]
pub extern "C" fn nm36_bgn_name(i: i32) -> *const c_char {
    static N: OnceLock<Vec<CString>> = OnceLock::new();
    name_at(static_names(&N, || neumetik_bgn::BANK.iter().map(|p| p.name.to_string()).collect()), i)
}

#[no_mangle]
#[cfg(feature = "bgn")]
pub extern "C" fn nm36_bgn_category(i: i32) -> *const c_char {
    static N: OnceLock<Vec<CString>> = OnceLock::new();
    name_at(static_names(&N, || neumetik_bgn::BANK.iter().map(|p| p.category.to_string()).collect()), i)
}

#[no_mangle]
#[cfg(feature = "bgn")]
pub extern "C" fn nm36_bgn_group(i: i32) -> *const c_char {
    static N: OnceLock<Vec<CString>> = OnceLock::new();
    name_at(static_names(&N, || neumetik_bgn::BANK.iter().map(|p| p.group.to_string()).collect()), i)
}

#[no_mangle]
#[cfg(feature = "visual")]
pub extern "C" fn nm36_visual_count() -> i32 {
    KINDS.len() as i32
}

/// A Visual kind's name ("Spectrum Bars"); null past the end.
#[no_mangle]
#[cfg(feature = "visual")]
pub extern "C" fn nm36_visual_name(i: i32) -> *const c_char {
    static N: OnceLock<Vec<CString>> = OnceLock::new();
    name_at(static_names(&N, || KINDS.iter().map(|k| k.name.to_string()).collect()), i)
}

/// Its id ("bars"), as settings keep it.
#[no_mangle]
#[cfg(feature = "visual")]
pub extern "C" fn nm36_visual_id(i: i32) -> *const c_char {
    static N: OnceLock<Vec<CString>> = OnceLock::new();
    name_at(static_names(&N, || KINDS.iter().map(|k| k.id.to_string()).collect()), i)
}

/// What it shows, in a sentence.
#[no_mangle]
#[cfg(feature = "visual")]
pub extern "C" fn nm36_visual_about(i: i32) -> *const c_char {
    static N: OnceLock<Vec<CString>> = OnceLock::new();
    name_at(static_names(&N, || KINDS.iter().map(|k| k.about.to_string()).collect()), i)
}

#[no_mangle]
#[cfg(feature = "visual")]
pub extern "C" fn nm36_vis_new(kind: i32) -> *mut Vis {
    Box::into_raw(Box::new(Vis::new(usize::try_from(kind).unwrap_or(0))))
}

/// # Safety
/// `v` is from `nm36_vis_new` (or null), and not used after.
#[no_mangle]
#[cfg(feature = "visual")]
pub unsafe extern "C" fn nm36_vis_free(v: *mut Vis) {
    if !v.is_null() {
        drop(Box::from_raw(v));
    }
}

/// # Safety
/// `v` is a live visual.
#[no_mangle]
#[cfg(feature = "visual")]
pub unsafe extern "C" fn nm36_vis_set(v: *mut Vis, kind: i32) {
    if let (Some(v), Ok(k)) = (v.as_mut(), usize::try_from(kind)) {
        v.set_kind(k);
    }
}

/// With the audio locked: copies the latest samples and what plays.
///
/// # Safety
/// `v` a live visual, `e` a live engine.
#[no_mangle]
#[cfg(feature = "visual")]
pub unsafe extern "C" fn nm36_vis_capture(v: *mut Vis, e: *const Engine) {
    if let (Some(v), Some(e)) = (v.as_mut(), e.as_ref()) {
        v.capture(e);
    }
}

/// The next frame into `cells`: `w` x `h` cells, three bytes each (code page 437 character, foreground, background:
/// 0-15). Returns 0, or -1 when Visual failed (the cells are then left blank).
///
/// # Safety
/// `v` a live visual; `cells` holds 3 x `w` x `h` bytes.
#[no_mangle]
#[cfg(feature = "visual")]
pub unsafe extern "C" fn nm36_vis_draw(v: *mut Vis, dt: f32, cells: *mut u8, w: i32, h: i32) -> i32 {
    let (Some(v), Ok(w), Ok(h)) = (v.as_mut(), usize::try_from(w), usize::try_from(h)) else { return -1 };
    if cells.is_null() || w == 0 || h == 0 {
        return -1;
    }
    let out = std::slice::from_raw_parts_mut(cells, w * h * 3);
    let drawn = catch_unwind(AssertUnwindSafe(|| {
        let c = v.draw(dt, w, h);
        for (o, cell) in out.chunks_exact_mut(3).zip(c.cells.iter()) {
            o[0] = cell.ch;
            o[1] = cell.fg & 15;
            o[2] = cell.bg & 15;
        }
    }));
    if drawn.is_err() {
        for o in out.chunks_exact_mut(3) {
            o.copy_from_slice(&[b' ', 7, 0]);
        }
        return -1;
    }
    0
}

/// The speed test (`bench`), for the device report: each line, in its grammar (`STATUS key: value`), it writes goes to `line`. `dir` (or null) is a folder of the user's own files to time too.
///
/// # Safety
/// `line` is a function the host gives; `dir` a 0-ended path or null.
#[no_mangle]
pub unsafe extern "C" fn nm36_bench(dir: *const c_char, rate: u32, seconds: f64, line: extern "C" fn(*const c_char, *mut std::ffi::c_void), user: *mut std::ffi::c_void) {
    let dir = c_str(dir).map(Path::new);
    bench::run(dir, rate, seconds, &mut |s: &str| {
        if let Ok(c) = CString::new(s.replace('\0', " ")) {
            line(c.as_ptr(), user);
        }
    });
}

/// Writes a song made here to `path`, for the self-test: 0 a MIDI file of `seconds`, 1 a 4-channel module, 2 an
/// 8-channel one. 0 when written.
#[no_mangle]
pub extern "C" fn nm36_write_test_song(path: *const c_char, which: i32, seconds: u32) -> i32 {
    let Some(path) = c_str(path) else { return -1 };
    let bytes = match which {
        0 => gen::midi_song(seconds.max(1)),
        1 => gen::module(4, 4),
        2 => gen::module(8, 4),
        _ => return -1,
    };
    if std::fs::write(path, bytes).is_ok() {
        0
    } else {
        -1
    }
}
