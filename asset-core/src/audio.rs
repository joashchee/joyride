//! Audio assets: what a file is (codec, rate, channels, length), how loud,
//! and what its name says. Facts come from the file's own header, in pure
//! Rust, so it runs anywhere the catalog does.
//!
//! The vocabulary and file-naming convention follow Neumetik's music catalog
//! (`music-catalog-system.md`, §15 technical metadata, §17 tag and game-audio
//! naming), cut down to what a game developer browsing assets needs.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioInfo {
    pub codec: &'static str,
    pub container: &'static str,
    pub sample_rate: u32,
    pub channels: u16,
    /// Absent for lossy codecs.
    pub bit_depth: Option<u16>,
    pub duration_ms: u64,
    pub lossless: bool,
}

/// Sample levels in dBFS (0 is full scale). Not LUFS: that needs K-weighting
/// and gating, which come later; these are honest raw peak and RMS.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Levels {
    pub peak_dbfs: f64,
    pub rms_dbfs: f64,
}

fn bad(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.to_string())
}

fn u16le(b: &[u8]) -> u16 {
    u16::from_le_bytes([b[0], b[1]])
}
fn u32le(b: &[u8]) -> u32 {
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

/// Reads a file's audio facts from its header, by content (not extension).
pub fn probe(path: &Path) -> io::Result<AudioInfo> {
    let mut f = File::open(path)?;
    let len = f.metadata()?.len();
    let mut head = [0u8; 12];
    let n = f.read(&mut head)?;
    let head = &head[..n];
    f.seek(SeekFrom::Start(0))?;
    if head.starts_with(b"RIFF") && head.len() >= 12 && &head[8..12] == b"WAVE" {
        probe_wav(&mut f).map(|(i, _)| i)
    } else if head.starts_with(b"fLaC") {
        probe_flac(&mut f)
    } else if head.starts_with(b"OggS") {
        probe_ogg(&mut f, len)
    } else if head.starts_with(b"ID3") || (head.len() >= 2 && head[0] == 0xFF && head[1] & 0xE0 == 0xE0) {
        probe_mp3(&mut f, len)
    } else {
        Err(bad("not a recognised audio file"))
    }
}

struct WavFmt {
    tag: u16,
    channels: u16,
    rate: u32,
    bits: u16,
}

fn probe_wav(f: &mut File) -> io::Result<(AudioInfo, (WavFmt2, u64, u64))> {
    f.seek(SeekFrom::Start(12))?;
    let mut fmt: Option<WavFmt> = None;
    let mut hdr = [0u8; 8];
    loop {
        if f.read(&mut hdr)? < 8 {
            return Err(bad("wav has no data chunk"));
        }
        let size = u32le(&hdr[4..8]) as u64;
        match &hdr[..4] {
            b"fmt " => {
                let mut b = vec![0u8; size.min(64) as usize];
                f.read_exact(&mut b)?;
                if b.len() < 16 {
                    return Err(bad("short fmt chunk"));
                }
                let mut tag = u16le(&b[0..2]);
                if tag == 0xFFFE && b.len() >= 26 {
                    tag = u16le(&b[24..26]); // extensible: the sub-format's first two bytes
                }
                fmt = Some(WavFmt { tag, channels: u16le(&b[2..4]), rate: u32le(&b[4..8]), bits: u16le(&b[14..16]) });
                let rest = size - b.len() as u64 + (size & 1);
                f.seek(SeekFrom::Current(rest as i64))?;
            }
            b"data" => {
                let fm = fmt.ok_or_else(|| bad("wav data before fmt"))?;
                let start = f.stream_position()?;
                let avail = f.metadata()?.len().saturating_sub(start);
                let size = size.min(avail); // a streamed or truncated file lies about its size
                if fm.channels == 0 || fm.bits == 0 || fm.rate == 0 {
                    return Err(bad("wav fmt is empty"));
                }
                let frame = fm.channels as u64 * (fm.bits as u64 / 8).max(1);
                let duration_ms = size / frame * 1000 / fm.rate as u64;
                let codec = match fm.tag {
                    1 | 3 => "pcm",
                    2 => "adpcm",
                    _ => "wav-other",
                };
                let info = AudioInfo {
                    codec,
                    container: "wav",
                    sample_rate: fm.rate,
                    channels: fm.channels,
                    bit_depth: Some(fm.bits),
                    duration_ms,
                    lossless: matches!(fm.tag, 1 | 3),
                };
                return Ok((info, (WavFmt2 { tag: fm.tag, channels: fm.channels, bits: fm.bits }, start, size)));
            }
            _ => {
                f.seek(SeekFrom::Current((size + (size & 1)) as i64))?;
            }
        }
    }
}

struct WavFmt2 {
    tag: u16,
    channels: u16,
    bits: u16,
}

fn probe_flac(f: &mut File) -> io::Result<AudioInfo> {
    let mut b = [0u8; 4 + 4 + 34];
    f.read_exact(&mut b)?;
    if b[4] & 0x7F != 0 || (u32::from(b[5]) << 16 | u32::from(b[6]) << 8 | u32::from(b[7])) < 34 {
        return Err(bad("flac has no STREAMINFO first"));
    }
    let s = &b[8..];
    let rate = (u32::from(s[10]) << 12) | (u32::from(s[11]) << 4) | (u32::from(s[12]) >> 4);
    let channels = ((s[12] >> 1) & 7) as u16 + 1;
    let bits = (((s[12] & 1) << 4) | (s[13] >> 4)) as u16 + 1;
    let total = (u64::from(s[13] & 0xF) << 32) | u64::from(u32::from_be_bytes([s[14], s[15], s[16], s[17]]));
    if rate == 0 {
        return Err(bad("flac rate is zero"));
    }
    Ok(AudioInfo {
        codec: "flac",
        container: "flac",
        sample_rate: rate,
        channels,
        bit_depth: Some(bits),
        duration_ms: total * 1000 / rate as u64,
        lossless: true,
    })
}

fn probe_ogg(f: &mut File, len: u64) -> io::Result<AudioInfo> {
    let mut first = vec![0u8; 128];
    let n = f.read(&mut first)?;
    first.truncate(n);
    if first.len() < 28 {
        return Err(bad("short ogg"));
    }
    let nseg = first[26] as usize;
    let body = 27 + nseg;
    let pkt = first.get(body..).ok_or_else(|| bad("short ogg"))?;
    let (codec, channels, rate, pre_skip) = if pkt.len() >= 16 && pkt[0] == 1 && &pkt[1..7] == b"vorbis" {
        ("vorbis", pkt[11] as u16, u32le(&pkt[12..16]), 0u64)
    } else if pkt.len() >= 19 && &pkt[..8] == b"OpusHead" {
        ("opus", pkt[9] as u16, 48000, u16le(&pkt[10..12]) as u64)
    } else {
        return Err(bad("ogg holds neither vorbis nor opus"));
    };
    // The last page's granule position is the length in samples.
    let tail_len = len.min(65536 + 27);
    f.seek(SeekFrom::Start(len - tail_len))?;
    let mut tail = vec![0u8; tail_len as usize];
    f.read_exact(&mut tail)?;
    let granule = tail
        .windows(4)
        .rposition(|w| w == b"OggS")
        .and_then(|i| tail.get(i + 6..i + 14))
        .map(|g| u64::from_le_bytes(g.try_into().unwrap()))
        .unwrap_or(0);
    let duration_ms = if rate == 0 { 0 } else { granule.saturating_sub(pre_skip) * 1000 / rate as u64 };
    Ok(AudioInfo { codec, container: "ogg", sample_rate: rate, channels, bit_depth: None, duration_ms, lossless: false })
}

fn probe_mp3(f: &mut File, len: u64) -> io::Result<AudioInfo> {
    let mut head = [0u8; 10];
    f.read_exact(&mut head)?;
    let mut pos = 0u64;
    if &head[..3] == b"ID3" {
        pos = 10 + ((u64::from(head[6] & 0x7F) << 21) | (u64::from(head[7] & 0x7F) << 14) | (u64::from(head[8] & 0x7F) << 7) | u64::from(head[9] & 0x7F));
    }
    f.seek(SeekFrom::Start(pos))?;
    let mut buf = vec![0u8; 8192];
    let n = f.read(&mut buf)?;
    const RATES: [[u32; 3]; 3] = [[44100, 48000, 32000], [22050, 24000, 16000], [11025, 12000, 8000]];
    const BR_V1_L3: [u32; 16] = [0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 0];
    const BR_V2_L3: [u32; 16] = [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160, 0];
    for i in 0..n.saturating_sub(4) {
        if buf[i] != 0xFF || buf[i + 1] & 0xE0 != 0xE0 {
            continue;
        }
        let version = (buf[i + 1] >> 3) & 3; // 3 = MPEG1, 2 = MPEG2, 0 = MPEG2.5
        let layer = (buf[i + 1] >> 1) & 3; // 1 = Layer III
        let br_idx = (buf[i + 2] >> 4) as usize;
        let sr_idx = ((buf[i + 2] >> 2) & 3) as usize;
        if version == 1 || layer != 1 || sr_idx == 3 || br_idx == 0 || br_idx == 15 {
            continue;
        }
        let (row, brs) = match version {
            3 => (0, &BR_V1_L3),
            2 => (1, &BR_V2_L3),
            _ => (2, &BR_V2_L3),
        };
        let kbps = brs[br_idx] as u64;
        let channels = if (buf[i + 3] >> 6) == 3 { 1 } else { 2 };
        let audio_bytes = len.saturating_sub(pos);
        // Constant-bitrate estimate; a variable-bitrate file's real length differs.
        let duration_ms = audio_bytes * 8 / kbps;
        return Ok(AudioInfo { codec: "mp3", container: "mp3", sample_rate: RATES[row][sr_idx], channels, bit_depth: None, duration_ms, lossless: false });
    }
    Err(bad("no mp3 frame found"))
}

/// Raw peak and RMS of a PCM or float WAV, read in chunks. `None` for any
/// other file, or a WAV in an unsupported sample format.
pub fn levels(path: &Path) -> io::Result<Option<Levels>> {
    let mut f = File::open(path)?;
    let mut head = [0u8; 12];
    if f.read(&mut head)? < 12 || !head.starts_with(b"RIFF") || &head[8..12] != b"WAVE" {
        return Ok(None);
    }
    let (_, (fmt, start, size)) = probe_wav(&mut f)?;
    let (bytes, float) = match (fmt.tag, fmt.bits) {
        (1, 8) => (1, false),
        (1, 16) => (2, false),
        (1, 24) => (3, false),
        (1, 32) => (4, false),
        (3, 32) => (4, true),
        _ => return Ok(None),
    };
    f.seek(SeekFrom::Start(start))?;
    let (mut peak, mut sum_sq, mut count) = (0f64, 0f64, 0u64);
    let mut buf = vec![0u8; 1 << 16];
    let mut left = size - size % (bytes as u64 * fmt.channels as u64).max(1);
    let mut carry: Vec<u8> = Vec::new();
    while left > 0 {
        let want = (buf.len() as u64).min(left) as usize;
        let n = f.read(&mut buf[..want])?;
        if n == 0 {
            break;
        }
        left -= n as u64;
        carry.extend_from_slice(&buf[..n]);
        let whole = carry.len() / bytes * bytes;
        for s in carry[..whole].chunks_exact(bytes) {
            let v = match (bytes, float) {
                (1, _) => (s[0] as f64 - 128.0) / 128.0,
                (2, _) => i16::from_le_bytes([s[0], s[1]]) as f64 / 32768.0,
                (3, _) => (i32::from_le_bytes([0, s[0], s[1], s[2]]) >> 8) as f64 / 8388608.0,
                (4, false) => i32::from_le_bytes([s[0], s[1], s[2], s[3]]) as f64 / 2147483648.0,
                _ => f32::from_le_bytes([s[0], s[1], s[2], s[3]]) as f64,
            };
            peak = peak.max(v.abs());
            sum_sq += v * v;
            count += 1;
        }
        carry.drain(..whole);
    }
    if count == 0 {
        return Ok(None);
    }
    let db = |x: f64| if x <= 0.0 { -120.0 } else { (20.0 * x.log10()).max(-120.0) };
    Ok(Some(Levels { peak_dbfs: db(peak), rms_dbfs: db((sum_sq / count as f64).sqrt()) }))
}

// ---- the game-audio file naming convention ----

/// A name like `MUS_Forest_Explore_A_L2Perc_96_Dm_v03` taken apart.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GameName {
    pub prefix: String,
    /// The words between the prefix and the numbers, in order.
    pub words: Vec<String>,
    pub bpm: Option<u32>,
    pub key: Option<String>,
    pub version: Option<u32>,
}

/// Prefix to the asset type it means (Neumetik §17.2).
pub fn prefix_type(prefix: &str) -> Option<&'static str> {
    Some(match prefix {
        "MUS" => "music",
        "STG" => "stinger",
        "TRN" => "transition",
        "AMB" => "ambience",
        "UIM" => "ui",
        "DIE" => "diegetic",
        "CIN" => "cinematic",
        "OST" => "soundtrack",
        "SFX" => "sfx",
        "VO" => "voice",
        _ => return None,
    })
}

/// Parses a game-audio file stem; `None` if it doesn't start with a known
/// prefix and an underscore.
pub fn parse_game_name(stem: &str) -> Option<GameName> {
    let mut parts = stem.split('_');
    let prefix = parts.next()?.to_string();
    prefix_type(&prefix)?;
    let mut g = GameName { prefix, ..Default::default() };
    for p in parts {
        if p.is_empty() {
            continue;
        }
        if g.version.is_none() && p.len() > 1 && p.starts_with('v') && p[1..].chars().all(|c| c.is_ascii_digit()) {
            g.version = p[1..].parse().ok();
        } else if g.bpm.is_none() && p.chars().all(|c| c.is_ascii_digit()) && (30..=300).contains(&p.parse::<u32>().unwrap_or(0)) {
            g.bpm = p.parse().ok();
        } else if g.key.is_none() && key_slug(p).is_some() {
            g.key = key_slug(p);
        } else {
            g.words.push(p.to_string());
        }
    }
    Some(g)
}

/// `Dm` becomes `d-minor`, `Bb` `b-flat-major`, `F#m` `f-sharp-minor`.
pub fn key_slug(k: &str) -> Option<String> {
    let mut c = k.chars();
    let note = c.next()?.to_ascii_lowercase();
    if !('a'..='g').contains(&note) {
        return None;
    }
    let rest: String = c.collect();
    let (acc, mode) = match rest.as_str() {
        "" => ("", "major"),
        "m" => ("", "minor"),
        "b" => ("-flat", "major"),
        "bm" => ("-flat", "minor"),
        "#" => ("-sharp", "major"),
        "#m" => ("-sharp", "minor"),
        _ => return None,
    };
    // A lone 'A'..'G' word is far more likely a variant letter than a key.
    if acc.is_empty() && mode == "major" {
        return None;
    }
    Some(format!("{note}{acc}-{mode}"))
}

/// The tags a game-audio name implies, in Neumetik's tag syntax.
pub fn tags_from_name(g: &GameName) -> Vec<String> {
    let mut t = Vec::new();
    if let Some(ty) = prefix_type(&g.prefix) {
        t.push(format!("type:{ty}"));
    }
    let slug = |s: &str| crate::catalog::normalise_tag(s);
    if let Some(area) = g.words.first() {
        t.push(format!("game:area/{}", slug(area)));
    }
    if let Some(state) = g.words.get(1) {
        t.push(format!("game:state/{}", slug(state)));
    }
    if let Some(b) = g.bpm {
        t.push(format!("bpm:{b}"));
    }
    if let Some(k) = &g.key {
        t.push(format!("key:{k}"));
    }
    t
}

// ---- tag vocabulary ----

const MOODS: [&str; 18] = [
    "happy", "sad", "melancholic", "tense", "epic", "calm", "dark", "playful", "mysterious", "aggressive",
    "triumphant", "romantic", "eerie", "hopeful", "nostalgic", "peaceful", "energetic", "somber",
];
const TYPES: [&str; 20] = [
    "music", "loop", "one-shot", "stinger", "transition", "ambience", "ui", "diegetic", "cinematic", "soundtrack",
    "sfx", "voice", "stem", "sample", "midi", "sprite", "tileset", "font", "shader", "library",
];
const VOCALS: [&str; 5] = ["none", "wordless", "lead", "choir", "spoken"];

/// Checks one tag against the syntax and the controlled namespaces:
/// lowercase kebab-case, `:` namespace, `/` hierarchy. Returns what is wrong.
pub fn lint_tag(tag: &str) -> Option<String> {
    if tag.is_empty() {
        return Some("empty tag".into());
    }
    if tag.chars().any(|c| c.is_uppercase() || c.is_whitespace()) {
        return Some(format!("`{tag}` must be lowercase with no spaces"));
    }
    let (ns, value) = tag.split_once(':')?;
    if value.is_empty() {
        return Some(format!("`{tag}` has a namespace but no value"));
    }
    let in_list = |list: &[&str]| list.contains(&value);
    match ns {
        "mood" if !in_list(&MOODS) => Some(format!("unknown mood `{value}`")),
        "type" if !in_list(&TYPES) => Some(format!("unknown type `{value}`")),
        "vocals" if !in_list(&VOCALS) => Some(format!("unknown vocals value `{value}`")),
        "energy" if !value.parse::<u8>().is_ok_and(|n| (1..=10).contains(&n)) => Some("energy is 1 to 10".into()),
        "bpm" if !value.parse::<u16>().is_ok_and(|n| (30..=300).contains(&n)) => Some("bpm is 30 to 300".into()),
        "camelot" if !is_camelot(value) => Some(format!("`{value}` is not a Camelot code (1a to 12b)")),
        _ => None,
    }
}

fn is_camelot(v: &str) -> bool {
    let (num, letter) = v.split_at(v.len().saturating_sub(1));
    matches!(letter, "a" | "b") && num.parse::<u8>().is_ok_and(|n| (1..=12).contains(&n))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn wav(samples: &[i16], rate: u32, channels: u16) -> Vec<u8> {
        let data: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
        let mut v = Vec::new();
        v.extend(b"RIFF");
        v.extend((36 + data.len() as u32).to_le_bytes());
        v.extend(b"WAVEfmt ");
        v.extend(16u32.to_le_bytes());
        v.extend(1u16.to_le_bytes());
        v.extend(channels.to_le_bytes());
        v.extend(rate.to_le_bytes());
        v.extend((rate * channels as u32 * 2).to_le_bytes());
        v.extend((channels * 2).to_le_bytes());
        v.extend(16u16.to_le_bytes());
        v.extend(b"data");
        v.extend((data.len() as u32).to_le_bytes());
        v.extend(data);
        v
    }

    fn write(d: &Path, name: &str, b: &[u8]) -> std::path::PathBuf {
        let p = d.join(name);
        File::create(&p).unwrap().write_all(b).unwrap();
        p
    }

    #[test]
    fn wav_facts_and_levels() {
        let d = tempfile::tempdir().unwrap();
        // 0.5 s of mono at 8 kHz: half the samples at +16384 (half scale), half silent.
        let mut s = vec![16384i16; 2000];
        s.extend(vec![0i16; 2000]);
        let p = write(d.path(), "a.wav", &wav(&s, 8000, 1));
        let i = probe(&p).unwrap();
        assert_eq!((i.codec, i.container, i.sample_rate, i.channels, i.bit_depth, i.duration_ms, i.lossless), ("pcm", "wav", 8000, 1, Some(16), 500, true));
        let l = levels(&p).unwrap().unwrap();
        assert!((l.peak_dbfs - -6.02).abs() < 0.01, "{}", l.peak_dbfs);
        assert!((l.rms_dbfs - -9.03).abs() < 0.01, "{}", l.rms_dbfs); // half the samples: -6.02 - 3.01
    }

    #[test]
    fn silence_is_floored_and_other_files_have_no_levels() {
        let d = tempfile::tempdir().unwrap();
        let p = write(d.path(), "s.wav", &wav(&[0; 100], 44100, 2));
        assert_eq!(levels(&p).unwrap().unwrap().peak_dbfs, -120.0);
        let t = write(d.path(), "t.txt", b"hello there, not audio");
        assert!(probe(&t).is_err());
        assert!(levels(&t).unwrap().is_none());
    }

    #[test]
    fn flac_streaminfo() {
        // 44100 Hz, stereo, 16-bit, 88200 samples (2 s).
        let mut b = b"fLaC".to_vec();
        b.extend([0x80, 0, 0, 34]);
        let mut si = vec![0u8; 34];
        let packed: u64 = (44100u64 << 44) | (1u64 << 41) | (15u64 << 36) | 88200u64;
        si[10..18].copy_from_slice(&packed.to_be_bytes());
        b.extend(si);
        let d = tempfile::tempdir().unwrap();
        let i = probe(&write(d.path(), "a.flac", &b)).unwrap();
        assert_eq!((i.codec, i.sample_rate, i.channels, i.bit_depth, i.duration_ms), ("flac", 44100, 2, Some(16), 2000));
    }

    #[test]
    fn ogg_vorbis_header_and_length() {
        let mut page1 = b"OggS".to_vec();
        page1.extend([0, 2]);
        page1.extend([0u8; 8]);
        page1.extend([0u8; 12]);
        page1.push(1);
        page1.push(30);
        let mut id = vec![1u8];
        id.extend(b"vorbis");
        id.extend(0u32.to_le_bytes());
        id.push(2);
        id.extend(44100u32.to_le_bytes());
        id.extend([0u8; 13]);
        page1.extend(&id);
        let mut last = b"OggS".to_vec();
        last.extend([0, 4]);
        last.extend(88200u64.to_le_bytes());
        last.extend([0u8; 12]);
        last.push(0);
        let mut all = page1;
        all.extend(last);
        let d = tempfile::tempdir().unwrap();
        let i = probe(&write(d.path(), "a.ogg", &all)).unwrap();
        assert_eq!((i.codec, i.sample_rate, i.channels, i.duration_ms, i.lossless), ("vorbis", 44100, 2, 2000, false));
    }

    #[test]
    fn mp3_header_after_id3() {
        let mut b = b"ID3\x03\0\0\0\0\0\x00".to_vec(); // empty ID3v2 tag
        b.extend([0xFF, 0xFB, 0x90, 0x00]); // MPEG1 L3, 128 kbps, 44100, stereo
        b.extend(vec![0u8; 16000 - 4]);
        let d = tempfile::tempdir().unwrap();
        let i = probe(&write(d.path(), "a.mp3", &b)).unwrap();
        assert_eq!((i.codec, i.sample_rate, i.channels), ("mp3", 44100, 2));
        assert_eq!(i.duration_ms, 1000); // 16000 bytes at 128 kbps
    }

    #[test]
    fn game_names_parse_into_tags() {
        let g = parse_game_name("MUS_Forest_Explore_A_L2Perc_96_Dm_v03").unwrap();
        assert_eq!((g.bpm, g.version, g.key.as_deref()), (Some(96), Some(3), Some("d-minor")));
        assert_eq!(g.words, ["Forest", "Explore", "A", "L2Perc"]);
        assert_eq!(
            tags_from_name(&g),
            ["type:music", "game:area/forest", "game:state/explore", "bpm:96", "key:d-minor"]
        );
        let s = parse_game_name("STG_Victory_Fanfare_01_v02").unwrap();
        assert_eq!(s.words, ["Victory", "Fanfare", "01"]);
        assert!(tags_from_name(&s).contains(&"type:stinger".to_string()));
        assert!(parse_game_name("explosion_01").is_none());
        assert!(parse_game_name("MUS").is_some());
    }

    #[test]
    fn keys() {
        assert_eq!(key_slug("F#m").unwrap(), "f-sharp-minor");
        assert_eq!(key_slug("Bb").unwrap(), "b-flat-major");
        assert_eq!(key_slug("Cm").unwrap(), "c-minor");
        assert_eq!(key_slug("A"), None); // a bare letter is a variant, not a key
        assert_eq!(key_slug("Hm"), None);
    }

    #[test]
    fn tag_lint() {
        assert_eq!(lint_tag("genre:electronic/drum-and-bass"), None);
        assert!(lint_tag("Mood:Sad").is_some());
        assert!(lint_tag("mood:grumpy").is_some());
        assert_eq!(lint_tag("mood:tense"), None);
        assert!(lint_tag("energy:11").is_some());
        assert_eq!(lint_tag("energy:7"), None);
        assert!(lint_tag("bpm:12").is_some());
        assert_eq!(lint_tag("camelot:8a"), None);
        assert!(lint_tag("camelot:13a").is_some());
        assert!(lint_tag("type:").is_some());
        assert_eq!(lint_tag("free-form-tag"), None);
    }
}
