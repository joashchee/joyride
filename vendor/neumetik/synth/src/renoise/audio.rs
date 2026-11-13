// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! A Renoise song's sample files decoded: FLAC (what Renoise saves), and
//! the WAV and Ogg Vorbis files it keeps as they were loaded.

use super::song::Audio;

/// A sample file decoded, by what it starts with.
pub fn decode(d: &[u8]) -> Option<Audio> {
    if crate::flac::probe(d) {
        let a = crate::flac::decode(d)?;
        let channels = (0..a.channels.len()).map(|c| a.to_f32(c)).collect();
        return Some(Audio { rate: a.rate, channels });
    }
    if d.starts_with(b"OggS") {
        let v = crate::vorbis::decode_ogg(d)?;
        let nch = usize::from(v.channels.max(1));
        let mut channels = vec![Vec::with_capacity(v.pcm.len() / nch); nch];
        for (i, &s) in v.pcm.iter().enumerate() {
            channels[i % nch].push(f32::from(s) / 32768.0);
        }
        return Some(Audio { rate: v.rate, channels });
    }
    if d.starts_with(b"RIFF") && d.get(8..12) == Some(b"WAVE") {
        return wav(d);
    }
    None
}

/// A WAV file's PCM (8 to 32 bits) or float (32 or 64 bits) points.
fn wav(d: &[u8]) -> Option<Audio> {
    let (mut fmt, mut data) = (None, None);
    let mut at = 12;
    while at + 8 <= d.len() {
        let id = &d[at..at + 4];
        let len = u32::from_le_bytes([d[at + 4], d[at + 5], d[at + 6], d[at + 7]]) as usize;
        let body = &d[at + 8..(at + 8).saturating_add(len).min(d.len())];
        match id {
            b"fmt " if body.len() >= 16 => fmt = Some(body),
            b"data" => data = Some(body),
            _ => {}
        }
        at = at + 8 + len + (len & 1);
    }
    let (fmt, data) = (fmt?, data?);
    let u16le = |i: usize| u16::from_le_bytes([fmt[i], fmt[i + 1]]);
    let mut tag = u16le(0);
    let nch = usize::from(u16le(2)).max(1);
    let rate = u32::from_le_bytes([fmt[4], fmt[5], fmt[6], fmt[7]]);
    let bits = usize::from(u16le(14));
    if tag == 0xfffe && fmt.len() >= 26 {
        tag = u16le(24);
    }
    let bytes = bits.div_ceil(8);
    if bytes == 0 {
        return None;
    }
    let mut channels = vec![Vec::with_capacity(data.len() / bytes / nch); nch];
    for (i, c) in data.chunks_exact(bytes).enumerate() {
        let v = match (tag, bytes) {
            (3, 4) => f32::from_le_bytes([c[0], c[1], c[2], c[3]]),
            (3, 8) => f64::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]]) as f32,
            (_, 1) => (f32::from(c[0]) - 128.0) / 128.0,
            (_, 2) => f32::from(i16::from_le_bytes([c[0], c[1]])) / 32768.0,
            (_, 3) => (i32::from(c[0]) | i32::from(c[1]) << 8 | i32::from(c[2] as i8) << 16) as f32 / 8_388_608.0,
            (_, 4) => i32::from_le_bytes([c[0], c[1], c[2], c[3]]) as f32 / 2_147_483_648.0,
            _ => return None,
        };
        channels[i % nch].push(v);
    }
    Some(Audio { rate, channels })
}
