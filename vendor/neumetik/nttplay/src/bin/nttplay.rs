// Copyright 2026 ansiapps. Neumetik for the R36S: proprietary, see the LICENSE at the repository root.

//! **nttplay**: Neumetik's players as a command-line tool, for r3i6nos's Commander (which runs it the way it runs `ffmpeg`) and for a shell.
//!
//! ```text
//! nttplay probe FILE
//! nttplay decode FILE [-ss SECONDS] [-t SECONDS] [-ar RATE] [-vol 0..1] [-loop] [-f s16le|wav] [-o FILE]
//! nttplay --version | --help
//! ```
//!
//! `probe` prints what the file is, one `key=value` line each (single-line values): `kind` (`midi` or `module`), `format` (`MIDI`, `Text`, `XM 8ch`, ...),
//! `title`, `duration` (seconds once through; `0.000` when it is not known), `channels`, `orders` (a module's), `message` (a module's, up to 200 characters).
//! `decode` renders it to 16-bit little-endian stereo at `-ar` (44100 by default) on stdout, or to `-o FILE`: the same PCM `ffmpeg -f s16le -ar 44100 -ac 2`
//! gives, so a caller that reads ffmpeg's pipe reads this one. `-ss` starts at a time, `-t` stops after that many seconds (a file that never ends, or `-loop`,
//! needs it), `-vol` is the player's volume control (the sound is heard as the square of it; 1 by default is full scale), `-f wav` puts a WAV header first (its
//! sizes are patched at the end with `-o`; on a pipe they say "unknown").
//!
//! What it reads is what `Song::open` reads: a MIDI file, a tracker module in any of the Tracker's formats (companion files from the file's folder) and music as
//! text (MusicXML, ABC, ChordPro, MML, ...), played as MIDI. Exit status: 0 done (a reader that closes the pipe early is not an error), 1 the file cannot be read,
//! 2 usage, 3 the replayer failed part-way (what played is already written). Messages go to stderr, one line, starting `nttplay:`.

use std::fs::File;
use std::io::{self, BufWriter, Seek, SeekFrom, Write};
use std::path::Path;
use std::process::ExitCode;

use neumetik_r36s::{Engine, Song};

const USAGE: &str = "usage: nttplay probe FILE\n       nttplay decode FILE [-ss SECONDS] [-t SECONDS] [-ar RATE] [-vol 0..1] [-loop] [-f s16le|wav] [-o FILE]\n       nttplay --version";
/// Frames rendered and written at a time.
const CHUNK: usize = 4096;
/// A file with no `-t` stops here, whatever it does: a safety against a replayer that never ends (four hours).
const HARD_CAP_SECONDS: f64 = 4.0 * 3600.0;

fn fail(code: u8, msg: impl AsRef<str>) -> ExitCode {
    eprintln!("nttplay: {}", msg.as_ref());
    ExitCode::from(code)
}

fn one_line(s: &str, max: usize) -> String {
    let t: String = s.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
    t.chars().take(max).collect()
}

fn probe(path: &str) -> ExitCode {
    let song = match Song::open(Path::new(path), 44_100) {
        Ok(s) => s,
        Err(e) => return fail(1, format!("{path}: {e}")),
    };
    let out = io::stdout();
    let mut o = out.lock();
    let kind = match song.kind() {
        neumetik_r36s::NM36_MIDI => "midi",
        neumetik_r36s::NM36_MODULE => "module",
        _ => "other",
    };
    let _ = writeln!(o, "kind={kind}");
    let _ = writeln!(o, "format={}", one_line(song.format(), 64));
    let _ = writeln!(o, "title={}", one_line(song.title(), 128));
    let _ = writeln!(o, "duration={:.3}", song.duration());
    let _ = writeln!(o, "channels={}", song.channels());
    if song.orders() > 0 {
        let _ = writeln!(o, "orders={}", song.orders());
    }
    let words = one_line(song.words(), 200);
    if !words.is_empty() {
        let _ = writeln!(o, "message={words}");
    }
    ExitCode::SUCCESS
}

struct Decode {
    path: String,
    seek: f64,
    limit: Option<f64>,
    rate: u32,
    volume: f32,
    looping: bool,
    wav: bool,
    out: Option<String>,
}

fn number(args: &[String], i: usize, what: &str) -> Result<f64, String> {
    let v = args.get(i + 1).ok_or_else(|| format!("{what} needs a value"))?;
    let n: f64 = v.parse().map_err(|_| format!("{what}: \"{v}\" is not a number"))?;
    if !n.is_finite() {
        return Err(format!("{what}: \"{v}\" is not a number"));
    }
    Ok(n)
}

fn parse_decode(args: &[String]) -> Result<Decode, String> {
    let mut d = Decode { path: String::new(), seek: 0.0, limit: None, rate: 44_100, volume: 1.0, looping: false, wav: false, out: None };
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-ss" => {
                d.seek = number(args, i, "-ss")?;
                if d.seek < 0.0 {
                    return Err("-ss must not be negative".into());
                }
                i += 1;
            }
            "-t" => {
                let t = number(args, i, "-t")?;
                if t <= 0.0 {
                    return Err("-t must be more than 0".into());
                }
                d.limit = Some(t);
                i += 1;
            }
            "-ar" => {
                let r = number(args, i, "-ar")?;
                if !(8000.0..=96_000.0).contains(&r) || r.fract() != 0.0 {
                    return Err("-ar must be a whole rate from 8000 to 96000".into());
                }
                d.rate = r as u32;
                i += 1;
            }
            "-vol" => {
                let v = number(args, i, "-vol")?;
                if !(0.0..=1.0).contains(&v) {
                    return Err("-vol must be from 0 to 1".into());
                }
                d.volume = v as f32;
                i += 1;
            }
            "-loop" => d.looping = true,
            "-f" => {
                match args.get(i + 1).map(String::as_str) {
                    Some("s16le") => d.wav = false,
                    Some("wav") => d.wav = true,
                    Some(o) => return Err(format!("-f: \"{o}\" is not s16le or wav")),
                    None => return Err("-f needs s16le or wav".into()),
                }
                i += 1;
            }
            "-o" => {
                d.out = Some(args.get(i + 1).ok_or("-o needs a file name")?.clone());
                i += 1;
            }
            a if a.starts_with('-') && a.len() > 1 => return Err(format!("unknown option {a}")),
            a => {
                if !d.path.is_empty() {
                    return Err(format!("one file at a time (\"{}\" and \"{a}\")", d.path));
                }
                d.path = a.to_string();
            }
        }
        i += 1;
    }
    if d.path.is_empty() {
        return Err("no file given".into());
    }
    if d.looping && d.limit.is_none() {
        return Err("-loop never ends: give -t".into());
    }
    Ok(d)
}

fn wav_header(rate: u32, data: u32) -> [u8; 44] {
    let mut h = [0u8; 44];
    h[0..4].copy_from_slice(b"RIFF");
    h[4..8].copy_from_slice(&data.saturating_add(36).to_le_bytes());
    h[8..16].copy_from_slice(b"WAVEfmt ");
    h[16..20].copy_from_slice(&16u32.to_le_bytes());
    h[20..22].copy_from_slice(&1u16.to_le_bytes()); // PCM
    h[22..24].copy_from_slice(&2u16.to_le_bytes()); // stereo
    h[24..28].copy_from_slice(&rate.to_le_bytes());
    h[28..32].copy_from_slice(&(rate * 4).to_le_bytes());
    h[32..34].copy_from_slice(&4u16.to_le_bytes());
    h[34..36].copy_from_slice(&16u16.to_le_bytes());
    h[36..40].copy_from_slice(b"data");
    h[40..44].copy_from_slice(&data.to_le_bytes());
    h
}

fn decode(d: Decode) -> ExitCode {
    let song = match Song::open(Path::new(&d.path), d.rate) {
        Ok(s) => s,
        Err(e) => return fail(1, format!("{}: {e}", d.path)),
    };
    let mut engine = Engine::new(d.rate);
    engine.set_volume(d.volume);
    engine.set_looping(d.looping);
    drop(engine.play(Some(Box::new(song))));
    if d.seek > 0.0 {
        engine.seek_to(d.seek);
    }
    // where the sound goes: a file (its WAV sizes can be patched) or stdout
    let mut file: Option<File> = None;
    let sink: Box<dyn Write> = match &d.out {
        Some(p) => match File::create(p) {
            Ok(f) => {
                file = f.try_clone().ok();
                Box::new(BufWriter::new(f))
            }
            Err(e) => return fail(1, format!("{p}: {e}")),
        },
        None => Box::new(BufWriter::with_capacity(1 << 16, io::stdout().lock())),
    };
    let mut sink = sink;
    let result = (|| -> io::Result<u64> {
        if d.wav {
            sink.write_all(&wav_header(d.rate, if d.out.is_some() { 0 } else { u32::MAX - 36 }))?;
        }
        let rate = f64::from(d.rate);
        let cap = (d.limit.unwrap_or(HARD_CAP_SECONDS) * rate) as u64;
        let mut written = 0u64;
        let mut pcm = vec![0.0f32; CHUNK * 2];
        let mut bytes = Vec::with_capacity(CHUNK * 4);
        while written < cap {
            let n = ((cap - written) as usize).min(CHUNK);
            engine.render(&mut pcm[..n * 2]);
            bytes.clear();
            for &x in &pcm[..n * 2] {
                bytes.extend_from_slice(&((x.clamp(-1.0, 1.0) * 32767.0).round() as i16).to_le_bytes());
            }
            sink.write_all(&bytes)?;
            written += n as u64;
            if engine.has_ended() {
                break;
            }
        }
        sink.flush()?;
        Ok(written)
    })();
    let frames = match result {
        Ok(n) => n,
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => return ExitCode::SUCCESS, // the reader has what it wants
        Err(e) => return fail(1, format!("writing: {e}")),
    };
    if d.wav {
        if let Some(f) = file.as_mut() {
            let data = (frames * 4).min(u64::from(u32::MAX) - 36) as u32;
            if f.seek(SeekFrom::Start(0)).and_then(|_| f.write_all(&wav_header(d.rate, data))).is_err() {
                return fail(1, "could not finish the WAV header");
            }
        }
    }
    if !engine.error().is_empty() {
        return fail(3, engine.error());
    }
    ExitCode::SUCCESS
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--version") | Some("-version") => {
            println!("nttplay {} (neumetik-r36s {})", env!("CARGO_PKG_VERSION"), env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some("--help") | Some("-h") | Some("help") => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some("probe") => match args.get(1) {
            Some(p) if args.len() == 2 && !p.starts_with('-') => probe(p),
            _ => fail(2, format!("probe takes one file\n{USAGE}")),
        },
        Some("decode") => match parse_decode(&args[1..]) {
            Ok(d) => decode(d),
            Err(e) => fail(2, format!("{e}\n{USAGE}")),
        },
        Some(other) => fail(2, format!("unknown command \"{other}\"\n{USAGE}")),
        None => fail(2, USAGE),
    }
}
