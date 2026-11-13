//! Previewing the selected file by what it is, not by its extension: an image
//! shows as a picture, sound plays, Theora video plays, a font shows a sample,
//! a `.love` or zip lists its members, MIDI and tracker modules play through
//! `nttplay`, text goes to the editor, and anything else shows its facts and a
//! hex dump, never a blank. Nothing is written, and every path stays inside
//! the project (`joyride_core::files::resolve`).
//!
//! The work is plain functions over a path so it is tested without a window;
//! the Tauri commands at the bottom only call them, off the UI thread.

use crate::project::Project;
use base64::Engine;
use joyride_core::files;
use serde::Serialize;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use tauri::{AppHandle, Manager, State};

/// Biggest file sent to the screen to play or draw (it travels as base64).
const MAX_PLAY: u64 = 32 * 1024 * 1024;
/// Biggest file read whole to probe or decode.
const MAX_READ: u64 = 256 * 1024 * 1024;
/// Seconds of a tracker or MIDI file rendered for the preview.
const MUSIC_SECONDS: u32 = 60;
const MUSIC_RATE: u32 = 22050;
const HEX_BYTES: usize = 1024;
const MEMBERS_SHOWN: usize = 400;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Fact {
    pub label: String,
    pub value: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Body {
    /// Text: the editor opens it.
    Text,
    #[serde(rename_all = "camelCase")]
    Image { data_url: String, width: u32, height: u32, pixel_art: bool, palette: Vec<String> },
    /// `data_url` is absent when the file is too big to play here.
    #[serde(rename_all = "camelCase")]
    Audio { data_url: Option<String>, waveform: Option<Vec<u8>>, seconds: Option<f64> },
    /// "ogv": frames come from `preview_video_frames`; "web": the system plays `data_url`.
    #[serde(rename_all = "camelCase")]
    Video { source: &'static str, data_url: Option<String>, width: u32, height: u32, fps: f64, frames: Option<u64>, seconds: Option<f64>, poster: Option<String> },
    #[serde(rename_all = "camelCase")]
    Font { data_url: String },
    #[serde(rename_all = "camelCase")]
    Members { names: Vec<Member>, more: usize },
    Hex { dump: String },
}

#[derive(Serialize, Clone, Debug)]
pub struct Member {
    pub name: String,
    pub size: u64,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub path: String,
    /// "text", "image", "audio", "video", "font", "archive", "music" or "binary".
    pub kind: &'static str,
    pub size: u64,
    pub facts: Vec<Fact>,
    /// What to tell the person when something could not be shown.
    pub note: Option<String>,
    pub body: Body,
}

fn fact(label: &str, value: impl ToString) -> Fact {
    Fact { label: label.into(), value: value.to_string() }
}

fn human(n: u64) -> String {
    match n {
        0..=1023 => format!("{n} bytes"),
        1024..=1048575 => format!("{:.1} KB", n as f64 / 1024.0),
        _ => format!("{:.1} MB", n as f64 / 1048576.0),
    }
}

fn clock(secs: f64) -> String {
    let t = secs.max(0.0);
    format!("{}:{:04.1}", (t / 60.0) as u64, t % 60.0)
}

fn data_url(mime: &str, bytes: &[u8]) -> String {
    format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes))
}

/// What a file is, from its first bytes (and its extension where bytes are not enough).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Sniff {
    Image,
    Wav,
    Flac,
    Mp3,
    Ogg,
    Font(&'static str),
    Zip,
    Mp4,
    Webm,
    Unknown,
}

fn sniff(head: &[u8], ext: &str) -> Sniff {
    let h = head;
    if h.starts_with(b"\x89PNG") || h.starts_with(b"\xff\xd8\xff") || h.starts_with(b"GIF8") || h.starts_with(b"II*\0") || h.starts_with(b"MM\0*") || (h.len() >= 12 && h.starts_with(b"RIFF") && &h[8..12] == b"WEBP") || (h.starts_with(b"BM") && ext == "bmp") {
        Sniff::Image
    } else if h.len() >= 12 && h.starts_with(b"RIFF") && &h[8..12] == b"WAVE" {
        Sniff::Wav
    } else if h.starts_with(b"fLaC") {
        Sniff::Flac
    } else if h.starts_with(b"OggS") {
        Sniff::Ogg
    } else if h.starts_with(b"ID3") || (h.len() >= 2 && h[0] == 0xFF && h[1] & 0xE0 == 0xE0 && ext == "mp3") {
        Sniff::Mp3
    } else if h.starts_with(b"\0\x01\0\0") || h.starts_with(b"true") {
        Sniff::Font("font/ttf")
    } else if h.starts_with(b"OTTO") {
        Sniff::Font("font/otf")
    } else if h.starts_with(b"wOFF") {
        Sniff::Font("font/woff")
    } else if h.starts_with(b"wOF2") {
        Sniff::Font("font/woff2")
    } else if h.starts_with(b"PK\x03\x04") || h.starts_with(b"PK\x05\x06") {
        Sniff::Zip
    } else if h.len() >= 12 && &h[4..8] == b"ftyp" {
        Sniff::Mp4
    } else if h.starts_with(b"\x1a\x45\xdf\xa3") {
        Sniff::Webm
    } else {
        Sniff::Unknown
    }
}

fn read_head(path: &Path, n: usize) -> std::io::Result<Vec<u8>> {
    let mut b = Vec::new();
    File::open(path)?.take(n as u64).read_to_end(&mut b)?;
    Ok(b)
}

/// UTF-8 text with no NUL, readable as a whole by the editor.
fn looks_like_text(head: &[u8], size: u64) -> bool {
    if size > joyride_core::files::MAX_TEXT_BYTES || head.contains(&0) {
        return false;
    }
    match std::str::from_utf8(head) {
        Ok(_) => true,
        // The 8 KB cut may split a character at its end.
        Err(e) => e.error_len().is_none() && e.valid_up_to() + 4 > head.len(),
    }
}

/// `nttplay`, if there is one: `JOYRIDE_NTTPLAY`, one beside the app's tools,
/// the copy built from `vendor/neumetik` in a development tree, then the PATH.
pub fn find_nttplay(tools: Option<&Path>) -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("JOYRIDE_NTTPLAY").map(PathBuf::from) {
        return p.is_file().then_some(p);
    }
    let mut c: Vec<PathBuf> = Vec::new();
    if let Some(t) = tools {
        c.push(t.join("nttplay"));
        c.push(t.join("nttplay/nttplay"));
    }
    c.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("../vendor/neumetik/nttplay/target/release/nttplay"));
    if let Some(path) = std::env::var_os("PATH") {
        c.extend(std::env::split_paths(&path).map(|d| d.join("nttplay")));
    }
    c.into_iter().find(|p| p.is_file())
}

/// Builds the preview of `rel` inside `root`.
pub fn build(root: &Path, rel: &str, nttplay: Option<&Path>) -> Result<Preview, String> {
    let path = files::resolve(root, rel).map_err(|e| e.to_string())?;
    let real = path.canonicalize().map_err(|e| e.to_string())?;
    if !real.starts_with(root.canonicalize().map_err(|e| e.to_string())?) {
        return Err(format!("`{rel}` leaves the project"));
    }
    let meta = std::fs::metadata(&real).map_err(|e| e.to_string())?;
    if !meta.is_file() {
        return Err(format!("`{rel}` is not a file"));
    }
    let size = meta.len();
    let ext = real.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let head = read_head(&real, 8192).map_err(|e| e.to_string())?;
    let mut p = Preview { path: rel.to_string(), kind: "binary", size, facts: vec![fact("Size", human(size))], note: None, body: Body::Text };

    let kind = sniff(&head, &ext);
    let shown = match kind {
        Sniff::Image => image(&mut p, &real),
        Sniff::Wav | Sniff::Flac | Sniff::Mp3 => audio(&mut p, &real, kind, size),
        Sniff::Ogg => {
            if head.windows(7).any(|w| w == b"\x80theora") {
                ogv_video(&mut p, &real, size)
            } else {
                audio(&mut p, &real, kind, size)
            }
        }
        Sniff::Font(mime) => font(&mut p, &real, mime, size),
        Sniff::Zip => members(&mut p, &real),
        Sniff::Mp4 | Sniff::Webm => web_video(&mut p, &real, kind, size),
        Sniff::Unknown => false,
    };
    if shown {
        return Ok(p);
    }
    // Not a known media type (or it would not open): text, music, then bytes.
    if kind == Sniff::Unknown && looks_like_text(&head, size) {
        p.kind = "text";
        p.body = Body::Text;
        return Ok(p);
    }
    if kind == Sniff::Unknown && size <= MAX_READ && music(&mut p, &real, nttplay) {
        return Ok(p);
    }
    hex(&mut p, &head);
    Ok(p)
}

fn hex(p: &mut Preview, head: &[u8]) {
    p.kind = "binary";
    let mut s = String::new();
    for (i, row) in head[..head.len().min(HEX_BYTES)].chunks(16).enumerate() {
        s.push_str(&format!("{:08x}  ", i * 16));
        for j in 0..16 {
            match row.get(j) {
                Some(b) => s.push_str(&format!("{b:02x} ")),
                None => s.push_str("   "),
            }
        }
        s.push(' ');
        s.extend(row.iter().map(|&b| if (0x20..0x7f).contains(&b) { b as char } else { '.' }));
        s.push('\n');
    }
    if p.size as usize > HEX_BYTES {
        s.push_str(&format!("… {} more bytes\n", p.size as usize - HEX_BYTES));
    }
    if p.note.is_none() {
        p.note = Some("No viewer for this kind of file, so here are its first bytes.".into());
    }
    p.body = Body::Hex { dump: s };
}

fn image(p: &mut Preview, path: &Path) -> bool {
    use asset_core::preview as pv;
    if p.size > MAX_READ {
        p.note = Some("This picture is too large to show.".into());
        return false;
    }
    let Ok(f) = pv::image_facts(path) else {
        p.note = Some("This picture could not be read (it may be damaged, or a format that is not supported).".into());
        return false;
    };
    let Ok((png, w, h)) = pv::thumbnail_png(path, 1024) else { return false };
    p.kind = "image";
    p.facts.push(fact("Picture", format!("{} x {}", f.width, f.height)));
    p.facts.push(fact("Transparency", if f.has_alpha { "yes" } else { "no" }));
    p.facts.push(fact("Colours", if f.colours >= 4096 { "4096 or more".to_string() } else { f.colours.to_string() }));
    if f.pixel_art {
        p.facts.push(fact("Style", "pixel art"));
    }
    p.body = Body::Image { data_url: data_url("image/png", &png), width: w, height: h, pixel_art: f.pixel_art, palette: f.palette.iter().map(|c| format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])).collect() };
    true
}

fn audio(p: &mut Preview, path: &Path, kind: Sniff, size: u64) -> bool {
    let info = asset_core::audio::probe(path).ok();
    p.kind = "audio";
    let mut seconds = None;
    if let Some(i) = &info {
        p.facts.push(fact("Sound", format!("{} in {}", i.codec, i.container)));
        p.facts.push(fact("Sample rate", format!("{} Hz", i.sample_rate)));
        p.facts.push(fact("Channels", i.channels));
        if let Some(b) = i.bit_depth {
            p.facts.push(fact("Bit depth", b));
        }
        seconds = Some(i.duration_ms as f64 / 1000.0);
        p.facts.push(fact("Length", clock(i.duration_ms as f64 / 1000.0)));
    }
    let waveform = asset_core::preview::waveform(path, 96).ok().flatten();
    let mime = match kind {
        Sniff::Wav => "audio/wav",
        Sniff::Flac => "audio/flac",
        Sniff::Mp3 => "audio/mpeg",
        _ => "audio/ogg",
    };
    let data = if size <= MAX_PLAY {
        std::fs::read(path).ok().map(|b| data_url(mime, &b))
    } else {
        p.note = Some("This file is too large to play here.".into());
        None
    };
    p.body = Body::Audio { data_url: data, waveform, seconds };
    true
}

fn ogv_video(p: &mut Preview, path: &Path, size: u64) -> bool {
    if size > MAX_READ {
        return false;
    }
    let Ok(bytes) = std::fs::read(path) else { return false };
    let Ok(info) = ogv::probe(&bytes) else {
        p.note = Some("This Ogg file has no Theora video; it may be damaged.".into());
        return false;
    };
    let v = &info.video;
    p.kind = "video";
    p.facts.push(fact("Video", "Theora in Ogg"));
    p.facts.push(fact("Picture", format!("{} x {}", v.width, v.height)));
    p.facts.push(fact("Frame rate", format!("{:.2} per second", v.fps)));
    if let Some(n) = v.frames {
        p.facts.push(fact("Frames", n));
    }
    if let Some(d) = v.duration {
        p.facts.push(fact("Length", clock(d)));
    }
    if let Some(a) = &info.audio {
        p.facts.push(fact("Sound track", format!("Vorbis, {} channel(s), {} Hz (not played in the preview)", a.channels, a.rate)));
    }
    let poster = ogv::Decoder::new(&bytes[..]).ok().and_then(|mut d| d.next_frame().ok().flatten()).and_then(|f| asset_core::preview::jpeg_from_rgba(f.width, f.height, &f.rgba, 85).ok()).map(|j| data_url("image/jpeg", &j));
    p.body = Body::Video { source: "ogv", data_url: None, width: v.width, height: v.height, fps: v.fps, frames: v.frames, seconds: v.duration, poster };
    true
}

fn web_video(p: &mut Preview, path: &Path, kind: Sniff, size: u64) -> bool {
    p.kind = "video";
    p.facts.push(fact("Video", if kind == Sniff::Mp4 { "MP4 (played by the system)" } else { "WebM (played by the system)" }));
    let data = if size <= MAX_PLAY {
        std::fs::read(path).ok().map(|b| data_url(if kind == Sniff::Mp4 { "video/mp4" } else { "video/webm" }, &b))
    } else {
        p.note = Some("This video is too large to play here.".into());
        None
    };
    p.body = Body::Video { source: "web", data_url: data, width: 0, height: 0, fps: 0.0, frames: None, seconds: None, poster: None };
    true
}

fn font(p: &mut Preview, path: &Path, mime: &str, size: u64) -> bool {
    if size > MAX_PLAY {
        return false;
    }
    let Ok(b) = std::fs::read(path) else { return false };
    p.kind = "font";
    match asset_core::font::probe(&b) {
        Some(f) => {
            p.facts.extend(crate::fonts::facts_of(&f));
            if let Some(g) = f.licence_guess() {
                p.facts.push(fact("Licence note", g));
            }
        }
        None => p.facts.push(fact("Font", mime.trim_start_matches("font/").to_uppercase())),
    }
    p.body = Body::Font { data_url: data_url(mime, &b) };
    true
}

fn u16le(b: &[u8], o: usize) -> usize {
    u16::from_le_bytes([b[o], b[o + 1]]) as usize
}
fn u32le(b: &[u8], o: usize) -> usize {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]) as usize
}

/// The members of a zip (a `.love` is one) from its central directory.
pub fn zip_members(path: &Path) -> Option<(Vec<Member>, usize)> {
    let mut f = File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    let tail = len.min(65557);
    f.seek(SeekFrom::Start(len - tail)).ok()?;
    let mut t = vec![0u8; tail as usize];
    f.read_exact(&mut t).ok()?;
    let e = t.windows(4).rposition(|w| w == b"PK\x05\x06")?;
    if t.len() < e + 22 {
        return None;
    }
    let (count, cd_size, cd_at) = (u16le(&t, e + 10), u32le(&t, e + 12), u32le(&t, e + 16));
    if cd_size > 8 * 1024 * 1024 || cd_at as u64 + cd_size as u64 > len {
        return None;
    }
    f.seek(SeekFrom::Start(cd_at as u64)).ok()?;
    let mut cd = vec![0u8; cd_size];
    f.read_exact(&mut cd).ok()?;
    let (mut at, mut out) = (0usize, Vec::new());
    for _ in 0..count {
        if at + 46 > cd.len() || &cd[at..at + 4] != b"PK\x01\x02" {
            break;
        }
        let (size, n, x, c) = (u32le(&cd, at + 24), u16le(&cd, at + 28), u16le(&cd, at + 30), u16le(&cd, at + 32));
        let name = cd.get(at + 46..at + 46 + n)?;
        out.push(Member { name: String::from_utf8_lossy(name).into_owned(), size: size as u64 });
        at += 46 + n + x + c;
    }
    Some((out, count))
}

fn members(p: &mut Preview, path: &Path) -> bool {
    let Some((mut names, total)) = zip_members(path) else {
        p.note = Some("This archive could not be listed.".into());
        return false;
    };
    p.kind = "archive";
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    p.facts.push(fact("Archive", if ext == "love" { "LÖVE game (zip)" } else { "zip" }));
    p.facts.push(fact("Members", total));
    if names.iter().any(|m| m.name == "main.lua") {
        p.facts.push(fact("Starts with", "main.lua"));
    }
    let more = names.len().saturating_sub(MEMBERS_SHOWN);
    names.truncate(MEMBERS_SHOWN);
    p.body = Body::Members { names, more };
    true
}

/// Wraps 16-bit stereo PCM in a WAV header.
fn wav16(pcm: &[u8], rate: u32) -> Vec<u8> {
    let mut w = Vec::with_capacity(44 + pcm.len());
    w.extend(b"RIFF");
    w.extend((36 + pcm.len() as u32).to_le_bytes());
    w.extend(b"WAVEfmt ");
    w.extend(16u32.to_le_bytes());
    w.extend(1u16.to_le_bytes());
    w.extend(2u16.to_le_bytes());
    w.extend(rate.to_le_bytes());
    w.extend((rate * 4).to_le_bytes());
    w.extend(4u16.to_le_bytes());
    w.extend(16u16.to_le_bytes());
    w.extend(b"data");
    w.extend((pcm.len() as u32).to_le_bytes());
    w.extend(pcm);
    w
}

fn capital(k: &str) -> String {
    let mut c = k.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

/// MIDI, tracker modules and music written as text, through `nttplay`: a file
/// it can read plays, the first minute of it.
fn music(p: &mut Preview, path: &Path, nttplay: Option<&Path>) -> bool {
    let Some(np) = nttplay else { return false };
    let probe = Command::new(np).arg("probe").arg(path).stdin(Stdio::null()).stderr(Stdio::null()).output();
    let Ok(out) = probe else { return false };
    if !out.status.success() {
        return false;
    }
    p.kind = "music";
    let mut seconds = None;
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let Some((k, v)) = line.split_once('=') else { continue };
        match k {
            "kind" => {}
            "duration" => {
                seconds = v.parse::<f64>().ok();
                p.facts.push(fact("Length", seconds.map(clock).unwrap_or_else(|| v.to_string())));
            }
            "format" => p.facts.push(fact("Music", v)),
            "title" => p.facts.push(fact("Title", v)),
            "channels" => p.facts.push(fact("Channels", v)),
            _ => p.facts.push(fact(&capital(k), v)),
        }
    }
    let dec = Command::new(np)
        .arg("decode")
        .arg(path)
        .args(["-ar", &MUSIC_RATE.to_string(), "-t", &MUSIC_SECONDS.to_string(), "-f", "s16le"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output();
    match dec {
        Ok(o) if o.status.success() && !o.stdout.is_empty() => {
            if seconds.is_some_and(|s| s > MUSIC_SECONDS as f64) {
                p.note = Some(format!("Plays the first {MUSIC_SECONDS} seconds."));
            }
            let wav = wav16(&o.stdout, MUSIC_RATE);
            p.body = Body::Audio { data_url: Some(data_url("audio/wav", &wav)), waveform: None, seconds: Some(o.stdout.len() as f64 / (MUSIC_RATE as f64 * 4.0)) };
        }
        _ => {
            p.note = Some("This music file could not be turned into sound.".into());
            p.body = Body::Audio { data_url: None, waveform: None, seconds };
        }
    }
    true
}

// ---- Theora video, frame by frame ----

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VideoFrame {
    pub index: u64,
    pub time: f64,
    pub jpeg: String,
}

#[derive(Serialize, Clone)]
pub struct VideoFrames {
    pub frames: Vec<VideoFrame>,
    /// No frames follow these.
    pub end: bool,
}

struct Session {
    path: PathBuf,
    dec: ogv::Decoder<std::io::BufReader<File>>,
    /// The index of the next frame the decoder will give.
    next: u64,
}

#[derive(Default)]
pub struct VideoSession(Mutex<Option<Session>>);

/// Up to `count` frames from frame `from`. Playing goes forward, so the
/// decoder is kept between calls; asking for an earlier frame starts again.
pub fn video_frames(session: &VideoSession, root: &Path, rel: &str, from: u64, count: u32) -> Result<VideoFrames, String> {
    let path = files::resolve(root, rel).map_err(|e| e.to_string())?;
    let real = path.canonicalize().map_err(|e| e.to_string())?;
    if !real.starts_with(root.canonicalize().map_err(|e| e.to_string())?) {
        return Err(format!("`{rel}` leaves the project"));
    }
    let mut guard = session.0.lock().unwrap();
    let fresh = match guard.as_ref() {
        Some(s) => s.path != real || from < s.next,
        None => true,
    };
    if fresh {
        let f = File::open(&real).map_err(|e| e.to_string())?;
        let dec = ogv::Decoder::new(std::io::BufReader::new(f)).map_err(|e| e.to_string())?;
        *guard = Some(Session { path: real, dec, next: 0 });
    }
    let s = guard.as_mut().unwrap();
    let mut out = Vec::new();
    let mut end = false;
    while out.len() < count.min(64) as usize {
        match s.dec.next_frame().map_err(|e| e.to_string())? {
            Some(f) => {
                s.next = f.index + 1;
                if f.index >= from {
                    let j = asset_core::preview::jpeg_from_rgba(f.width, f.height, &f.rgba, 80).map_err(|e| e.to_string())?;
                    out.push(VideoFrame { index: f.index, time: f.time, jpeg: data_url("image/jpeg", &j) });
                }
            }
            None => {
                end = true;
                break;
            }
        }
    }
    Ok(VideoFrames { frames: out, end })
}

// ---- Commands ----

/// What the selected file is, with what is needed to show it.
#[tauri::command]
pub async fn preview_file(app: AppHandle, project: State<'_, Project>, path: String) -> Result<Preview, String> {
    let root = project.root()?;
    let tools = crate::bundled_toolchains(&app);
    let np = find_nttplay(tools.as_deref());
    tauri::async_runtime::spawn_blocking(move || build(&root, &path, np.as_deref())).await.map_err(|e| e.to_string())?
}

/// The next frames of an Ogg Theora video.
#[tauri::command]
pub async fn preview_video_frames(app: AppHandle, project: State<'_, Project>, path: String, from: u64, count: u32) -> Result<VideoFrames, String> {
    let root = project.root()?;
    tauri::async_runtime::spawn_blocking(move || {
        let session = app.state::<VideoSession>();
        video_frames(&session, &root, &path, from, count)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proj() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn text_goes_to_the_editor() {
        let d = proj();
        std::fs::write(d.path().join("main.lua"), "print('héllo')\n").unwrap();
        let p = build(d.path(), "main.lua", None).unwrap();
        assert_eq!(p.kind, "text");
        assert!(matches!(p.body, Body::Text));
    }

    #[test]
    fn an_image_shows_its_facts_and_a_picture() {
        let d = proj();
        let mut img = image::RgbaImage::new(8, 8);
        for (x, _, px) in img.enumerate_pixels_mut() {
            *px = if x < 4 { image::Rgba([255, 0, 0, 255]) } else { image::Rgba([0, 0, 255, 255]) };
        }
        img.put_pixel(7, 7, image::Rgba([0, 0, 0, 0]));
        img.save(d.path().join("a.png")).unwrap();
        // The name says nothing: the bytes do.
        std::fs::rename(d.path().join("a.png"), d.path().join("sprite.dat")).unwrap();
        let p = build(d.path(), "sprite.dat", None).unwrap();
        assert_eq!(p.kind, "image");
        assert!(p.facts.contains(&fact("Picture", "8 x 8")) && p.facts.contains(&fact("Transparency", "yes")));
        match p.body {
            Body::Image { data_url, pixel_art, palette, .. } => {
                assert!(data_url.starts_with("data:image/png;base64,"));
                assert!(pixel_art && palette.len() == 2);
            }
            b => panic!("{b:?}"),
        }
    }

    #[test]
    fn a_wav_plays_and_shows_a_waveform() {
        let d = proj();
        let samples: Vec<u8> = (0..4410).flat_map(|i| (((i as f32 / 20.0).sin() * 12000.0) as i16).to_le_bytes()).collect();
        std::fs::write(d.path().join("tone.wav"), {
            let mut w = wav16(&[], 44100);
            // A mono 16-bit file: patch the header the helper wrote for stereo.
            w[22..24].copy_from_slice(&1u16.to_le_bytes());
            w[28..32].copy_from_slice(&(44100u32 * 2).to_le_bytes());
            w[32..34].copy_from_slice(&2u16.to_le_bytes());
            w[4..8].copy_from_slice(&(36 + samples.len() as u32).to_le_bytes());
            w[40..44].copy_from_slice(&(samples.len() as u32).to_le_bytes());
            w.extend(&samples);
            w
        })
        .unwrap();
        let p = build(d.path(), "tone.wav", None).unwrap();
        assert_eq!(p.kind, "audio");
        match p.body {
            Body::Audio { data_url, waveform, seconds } => {
                assert!(data_url.unwrap().starts_with("data:audio/wav;base64,"));
                assert_eq!(waveform.unwrap().len(), 96);
                assert!((seconds.unwrap() - 0.1).abs() < 0.01);
            }
            b => panic!("{b:?}"),
        }
    }

    const BARS: &[u8] = include_bytes!("../../ogv/tests/bars.ogv");

    #[test]
    fn theora_video_has_facts_a_poster_and_frames_in_order() {
        let d = proj();
        std::fs::write(d.path().join("intro.ogv"), BARS).unwrap();
        let p = build(d.path(), "intro.ogv", None).unwrap();
        assert_eq!(p.kind, "video");
        assert!(p.facts.contains(&fact("Frames", 15)) && p.facts.contains(&fact("Picture", "64 x 48")));
        match &p.body {
            Body::Video { source, poster, fps, .. } => {
                assert_eq!(*source, "ogv");
                assert!(poster.as_ref().unwrap().starts_with("data:image/jpeg;base64,"));
                assert_eq!(*fps, 10.0);
            }
            b => panic!("{b:?}"),
        }
        let s = VideoSession::default();
        let a = video_frames(&s, d.path(), "intro.ogv", 0, 6).unwrap();
        assert_eq!(a.frames.iter().map(|f| f.index).collect::<Vec<_>>(), [0, 1, 2, 3, 4, 5]);
        assert!(!a.end);
        // Playing on continues without starting again; the time follows the index.
        let b = video_frames(&s, d.path(), "intro.ogv", 6, 20).unwrap();
        assert_eq!(b.frames.len(), 9);
        assert!(b.end && (b.frames[0].time - 0.6).abs() < 1e-6);
        // Going back starts again.
        let c = video_frames(&s, d.path(), "intro.ogv", 2, 2).unwrap();
        assert_eq!(c.frames.iter().map(|f| f.index).collect::<Vec<_>>(), [2, 3]);
    }

    #[test]
    fn a_love_file_lists_its_members() {
        let d = proj();
        let game = d.path().join("g");
        std::fs::create_dir_all(game.join("gfx")).unwrap();
        std::fs::write(game.join("main.lua"), "function love.draw() end\n").unwrap();
        std::fs::write(game.join("gfx/a.txt"), "x").unwrap();
        joyride_core::export::love_archive(&game, &d.path().join("g.love")).unwrap();
        let p = build(d.path(), "g.love", None).unwrap();
        assert_eq!(p.kind, "archive");
        assert!(p.facts.contains(&fact("Starts with", "main.lua")));
        match p.body {
            Body::Members { names, more } => {
                assert!(names.iter().any(|m| m.name == "main.lua") && names.iter().any(|m| m.name == "gfx/a.txt"));
                assert_eq!(more, 0);
            }
            b => panic!("{b:?}"),
        }
    }

    #[test]
    fn midi_plays_through_nttplay_when_there_is_one() {
        let Some(np) = find_nttplay(None) else { return eprintln!("no nttplay: skipped") };
        let d = proj();
        let t = [0u8, 0x90, 60, 100, 96, 0x80, 60, 0, 0, 0xFF, 0x2F, 0];
        let mut m = b"MThd".to_vec();
        m.extend([0, 0, 0, 6, 0, 0, 0, 1, 0, 96, b'M', b'T', b'r', b'k', 0, 0, 0, t.len() as u8]);
        m.extend(t);
        std::fs::write(d.path().join("song.mid"), m).unwrap();
        let p = build(d.path(), "song.mid", Some(&np)).unwrap();
        assert_eq!(p.kind, "music", "{:?}", p.facts);
        match p.body {
            Body::Audio { data_url, .. } => assert!(data_url.unwrap().starts_with("data:audio/wav;base64,")),
            b => panic!("{b:?}"),
        }
        // Without it the same file is shown as bytes, with a reason.
        let q = build(d.path(), "song.mid", None).unwrap();
        assert_eq!(q.kind, "binary");
        assert!(q.note.is_some());
    }

    #[test]
    fn anything_else_is_bytes_never_blank() {
        let d = proj();
        std::fs::write(d.path().join("blob.bin"), (0..=255u8).cycle().take(3000).collect::<Vec<_>>()).unwrap();
        let p = build(d.path(), "blob.bin", None).unwrap();
        assert_eq!(p.kind, "binary");
        match p.body {
            Body::Hex { dump } => {
                assert!(dump.starts_with("00000000  00 01 02 03"));
                assert!(dump.contains("… 1976 more bytes"));
            }
            b => panic!("{b:?}"),
        }
    }

    #[test]
    fn a_damaged_picture_falls_back_to_bytes_with_a_reason() {
        let d = proj();
        std::fs::write(d.path().join("bad.png"), b"\x89PNG\r\n\x1a\nnot really").unwrap();
        let p = build(d.path(), "bad.png", None).unwrap();
        assert_eq!(p.kind, "binary");
        assert!(p.note.unwrap().contains("could not be read"));
    }

    #[test]
    fn paths_cannot_leave_the_project() {
        let d = proj();
        assert!(build(d.path(), "../x", None).is_err());
        assert!(video_frames(&VideoSession::default(), d.path(), "../x", 0, 1).is_err());
    }
}
