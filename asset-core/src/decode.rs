//! Decoding sound to PCM for the preview: waveforms for MP3, FLAC, Ogg
//! Vorbis and WAV, a 16-bit WAV to play in a webview that cannot play
//! Vorbis, and the Vorbis track of an Ogg Theora video. Symphonia does the
//! decoding (pure Rust, MPL-2.0); this file is the framing around it.

use crate::{Error, Result};
use std::io::Cursor;
use std::path::Path;
use symphonia::core::audio::sample::Sample;
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;

fn bad(what: impl std::fmt::Display) -> Error {
    Error::Other(format!("cannot decode this sound: {what}"))
}

/// Decoded sound: interleaved samples in -1.0..1.0.
pub struct Pcm {
    pub rate: u32,
    pub channels: u16,
    pub samples: Vec<f32>,
    /// True when decoding stopped at the frame limit.
    pub truncated: bool,
}

impl Pcm {
    pub fn frames(&self) -> u64 {
        self.samples.len() as u64 / self.channels.max(1) as u64
    }
    pub fn seconds(&self) -> f64 {
        self.frames() as f64 / self.rate.max(1) as f64
    }
}

/// Calls `each(rate, channels, interleaved)` for every decoded packet until
/// the stream ends or `each` returns false. A packet that fails to decode is
/// skipped (a damaged frame should not blank a preview); an unreadable
/// stream is an error.
fn walk(src: Box<dyn MediaSource>, ext: Option<&str>, mut each: impl FnMut(u32, u16, &[f32]) -> bool) -> Result<()> {
    let mut hint = Hint::new();
    if let Some(e) = ext {
        hint.with_extension(e);
    }
    let mss = MediaSourceStream::new(src, Default::default());
    let mut format = symphonia::default::get_probe().probe(&hint, mss, FormatOptions::default(), MetadataOptions::default()).map_err(bad)?;
    let track = format.default_track(TrackType::Audio).ok_or_else(|| bad("no sound track"))?;
    let params = track.codec_params.as_ref().and_then(|p| p.audio()).ok_or_else(|| bad("no sound parameters"))?;
    let mut decoder = symphonia::default::get_codecs().make_audio_decoder(params, &AudioDecoderOptions::default()).map_err(bad)?;
    let id = track.id;
    let mut buf: Vec<f32> = Vec::new();
    let mut any = false;
    loop {
        let packet = match format.next_packet() {
            Ok(Some(p)) => p,
            Ok(None) => break,
            Err(SymError::IoError(_)) | Err(SymError::ResetRequired) => break, // a cut-off file ends here
            Err(e) => {
                if any {
                    break;
                }
                return Err(bad(e));
            }
        };
        if packet.track_id != id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(b) => {
                let (rate, channels) = (b.spec().rate(), b.spec().channels().count() as u16);
                buf.resize(b.samples_interleaved(), f32::MID);
                b.copy_to_slice_interleaved(&mut buf);
                any = true;
                if !each(rate, channels, &buf) {
                    break;
                }
            }
            Err(SymError::DecodeError(_)) => {}
            Err(e) => {
                if any {
                    break;
                }
                return Err(bad(e));
            }
        }
    }
    if any { Ok(()) } else { Err(bad("no sound could be decoded")) }
}

fn file_source(path: &Path) -> Result<(Box<dyn MediaSource>, Option<String>)> {
    let ext = path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase());
    Ok((Box::new(std::fs::File::open(path)?), ext))
}

/// Decodes at most `max_frames` frames of a sound file.
pub fn decode_file(path: &Path, max_frames: u64) -> Result<Pcm> {
    let (src, ext) = file_source(path)?;
    collect(src, ext.as_deref(), max_frames)
}

/// Decodes at most `max_frames` frames of sound held in memory. `ext` is a
/// hint (`ogg`, `mp3`...); the bytes decide.
pub fn decode_bytes(bytes: Vec<u8>, ext: Option<&str>, max_frames: u64) -> Result<Pcm> {
    collect(Box::new(Cursor::new(bytes)), ext, max_frames)
}

fn collect(src: Box<dyn MediaSource>, ext: Option<&str>, max_frames: u64) -> Result<Pcm> {
    let mut pcm = Pcm { rate: 0, channels: 0, samples: Vec::new(), truncated: false };
    walk(src, ext, |rate, channels, s| {
        if pcm.rate == 0 {
            (pcm.rate, pcm.channels) = (rate, channels);
        } else if (rate, channels) != (pcm.rate, pcm.channels) {
            return false; // a format change mid-stream (chained Ogg): the first part is the preview
        }
        let left = max_frames.saturating_sub(pcm.frames()).saturating_mul(channels.max(1) as u64).min(usize::MAX as u64) as usize;
        if s.len() > left {
            pcm.samples.extend_from_slice(&s[..left]);
            pcm.truncated = true;
            return false;
        }
        pcm.samples.extend_from_slice(s);
        true
    })?;
    Ok(pcm)
}

/// A waveform of `bins` peaks (0 to 255) over the whole sound, decoded in a
/// stream so a long file is never held in memory.
pub fn waveform(path: &Path, bins: usize) -> Result<Option<Vec<u8>>> {
    if bins == 0 {
        return Ok(None);
    }
    let (src, ext) = file_source(path)?;
    // Peaks of 512-frame slices, then binned once the length is known.
    let mut slices: Vec<(u64, f32)> = Vec::new();
    let mut at = 0u64;
    walk(src, ext.as_deref(), |_, channels, s| {
        let ch = channels.max(1) as usize;
        for chunk in s.chunks(512 * ch) {
            let peak = chunk.iter().fold(0f32, |m, v| m.max(v.abs()));
            slices.push((at, peak));
            at += (chunk.len() / ch) as u64;
        }
        true
    })?;
    if at == 0 {
        return Ok(None);
    }
    let mut peaks = vec![0f32; bins];
    for (start, p) in slices {
        let b = ((start * bins as u64) / at) as usize;
        peaks[b] = peaks[b].max(p);
    }
    Ok(Some(peaks.iter().map(|p| (p.min(1.0) * 255.0).round() as u8).collect()))
}

/// 16-bit PCM WAV bytes of a decoded sound.
pub fn wav16(pcm: &Pcm) -> Vec<u8> {
    let data_len = pcm.samples.len() * 2;
    let mut w = Vec::with_capacity(44 + data_len);
    w.extend(b"RIFF");
    w.extend((36 + data_len as u32).to_le_bytes());
    w.extend(b"WAVEfmt ");
    w.extend(16u32.to_le_bytes());
    w.extend(1u16.to_le_bytes());
    w.extend(pcm.channels.to_le_bytes());
    w.extend(pcm.rate.to_le_bytes());
    w.extend((pcm.rate * pcm.channels as u32 * 2).to_le_bytes());
    w.extend((pcm.channels * 2).to_le_bytes());
    w.extend(16u16.to_le_bytes());
    w.extend(b"data");
    w.extend((data_len as u32).to_le_bytes());
    for s in &pcm.samples {
        w.extend(((s.clamp(-1.0, 1.0) * 32767.0).round() as i16).to_le_bytes());
    }
    w
}

/// The Vorbis stream of an Ogg file as an Ogg file of its own: every page
/// of the one logical stream whose first packet is a Vorbis header, in
/// order. An Ogg Theora video interleaves its picture and sound pages; this
/// leaves the sound. `None` when there is no Vorbis stream.
pub fn vorbis_only(ogg: &[u8]) -> Option<Vec<u8>> {
    // (offset, length, serial, begins-stream, starts with a Vorbis id header)
    let mut pages = Vec::new();
    let mut at = 0usize;
    while at + 27 <= ogg.len() && &ogg[at..at + 4] == b"OggS" {
        let nseg = ogg[at + 26] as usize;
        let table_end = at + 27 + nseg;
        if table_end > ogg.len() {
            break;
        }
        let body: usize = ogg[at + 27..table_end].iter().map(|&b| b as usize).sum();
        let end = table_end + body;
        if end > ogg.len() {
            break; // a cut-off last page is dropped
        }
        let serial = u32::from_le_bytes(ogg[at + 14..at + 18].try_into().unwrap());
        let bos = ogg[at + 5] & 0x02 != 0;
        let vorbis = bos && ogg[table_end..end].starts_with(b"\x01vorbis");
        pages.push((at, end - at, serial, vorbis));
        at = end;
    }
    let serial = pages.iter().find(|p| p.3)?.2;
    let mut out = Vec::new();
    for (off, len, s, _) in pages {
        if s == serial {
            out.extend_from_slice(&ogg[off..off + len]);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn io(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../samples/io-checker/assets/audio").join(name)
    }

    #[test]
    fn the_four_encodings_of_the_jingle_agree() {
        let wav = decode_file(&io("jingle.wav"), u64::MAX).unwrap();
        assert!(wav.frames() > 1000 && !wav.truncated);
        for name in ["jingle.flac", "jingle.mp3", "jingle.ogg"] {
            let p = decode_file(&io(name), u64::MAX).unwrap();
            // The sample's files are not all one format (the .ogg is 44.1 kHz stereo):
            // each is checked against its own header, read by the independent probe.
            let header = crate::audio::probe(&io(name)).unwrap();
            assert_eq!((p.rate, p.channels), (header.sample_rate, header.channels as u16), "{name}");
            // Lossy codecs pad and trim a little; the length must still be close.
            assert!((p.seconds() - wav.seconds()).abs() < 0.15, "{name}: {} s against {} s", p.seconds(), wav.seconds());
        }
        // FLAC is lossless: the samples are the WAV's within 16-bit rounding.
        let flac = decode_file(&io("jingle.flac"), u64::MAX).unwrap();
        let n = wav.samples.len().min(flac.samples.len());
        let worst = wav.samples[..n].iter().zip(&flac.samples[..n]).map(|(a, b)| (a - b).abs()).fold(0f32, f32::max);
        assert!(worst < 1.0 / 16384.0, "lossless differs by {worst}");
    }

    #[test]
    fn waveforms_of_every_format_have_the_same_shape() {
        let reference = waveform(&io("jingle.wav"), 32).unwrap().unwrap();
        assert!(reference.iter().any(|&v| v > 40), "the jingle is not silent: {reference:?}");
        for name in ["jingle.flac", "jingle.mp3", "jingle.ogg"] {
            let w = waveform(&io(name), 32).unwrap().unwrap();
            assert_eq!(w.len(), 32);
            // Mean difference per bin, in 0..255: lossy coding moves peaks, not the outline.
            let diff: f32 = reference.iter().zip(&w).map(|(a, b)| (*a as f32 - *b as f32).abs()).sum::<f32>() / 32.0;
            assert!(diff < 40.0, "{name}: mean difference {diff}\n{reference:?}\n{w:?}");
        }
    }

    #[test]
    fn the_frame_limit_truncates() {
        let p = decode_file(&io("jingle.flac"), 1000).unwrap();
        assert_eq!(p.frames(), 1000);
        assert!(p.truncated);
    }

    #[test]
    fn wav16_round_trips_through_the_decoder() {
        let p = decode_file(&io("jingle.flac"), u64::MAX).unwrap();
        let w = wav16(&p);
        assert_eq!(&w[..4], b"RIFF");
        let back = decode_bytes(w, Some("wav"), u64::MAX).unwrap();
        assert_eq!((back.rate, back.channels, back.frames()), (p.rate, p.channels, p.frames()));
    }

    #[test]
    fn the_vorbis_track_is_pulled_out_of_a_theora_video() {
        let ogv = std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("../ogv/tests/bars.ogv")).unwrap();
        let only = vorbis_only(&ogv).expect("bars.ogv has a Vorbis track");
        assert!(only.len() < ogv.len() && only.starts_with(b"OggS"));
        let p = decode_bytes(only, Some("ogg"), u64::MAX).unwrap();
        assert!(p.frames() > 0 && p.rate > 0, "decoded {} frames", p.frames());
        // A plain Ogg Vorbis is returned whole, and a non-Ogg file has none.
        let plain = std::fs::read(io("jingle.ogg")).unwrap();
        assert_eq!(vorbis_only(&plain).unwrap().len(), plain.len());
        assert!(vorbis_only(b"RIFF....WAVE").is_none());
    }

    #[test]
    fn garbage_is_an_error_not_a_panic() {
        assert!(decode_bytes(vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10], None, 100).is_err());
        assert!(decode_bytes(vec![], None, 100).is_err());
    }
}
