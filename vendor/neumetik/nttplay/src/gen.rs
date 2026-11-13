// Copyright 2026 ansiapps. Neumetik for the R36S: proprietary, see the LICENSE at the repository root.

//! Music made here, for the speed test and the self-test: a busy General
//! MIDI song and ProTracker modules of 4 and 8 channels with their own
//! drawn samples. Nothing is anyone else's (CLAUDE.md rule 2 of r3i6nos:
//! no third-party music in a package).

/// I, vi, IV, V in C, a bar each: the chord's notes from its root.
const CHORDS: [[u8; 4]; 4] = [[60, 64, 67, 72], [57, 60, 64, 69], [53, 57, 60, 65], [55, 59, 62, 67]];
const MELODY: [u8; 16] = [72, 74, 76, 79, 76, 74, 72, 69, 71, 72, 74, 76, 74, 71, 67, 69];

fn vlq(mut v: u32, out: &mut Vec<u8>) {
    let mut bytes = vec![(v & 0x7F) as u8];
    v >>= 7;
    while v > 0 {
        bytes.push(0x80 | (v & 0x7F) as u8);
        v >>= 7;
    }
    out.extend(bytes.iter().rev());
}

/// A General MIDI song of `seconds` at 120 beats a minute: piano, strings,
/// bass, a square lead, a guitar's arpeggio, an organ and drums, about
/// 30 notes sounding at once: harder work than most songs.
pub fn midi_song(seconds: u32) -> Vec<u8> {
    const Q: u32 = 480; // ticks a beat
    let bars = seconds.div_ceil(2); // 4 beats at 120 a minute: 2 s a bar
    // (tick, order at that tick: offs first, the event's bytes)
    let mut ev: Vec<(u32, u8, Vec<u8>)> = Vec::new();
    let note = |ev: &mut Vec<(u32, u8, Vec<u8>)>, ch: u8, key: u8, vel: u8, at: u32, len: u32| {
        ev.push((at, 1, vec![0x90 | ch, key, vel]));
        ev.push((at + len, 0, vec![0x80 | ch, key, 0]));
    };
    let programs: [(u8, u8); 6] = [(0, 0), (1, 48), (2, 33), (3, 80), (4, 25), (5, 16)];
    for (ch, prog) in programs {
        ev.push((0, 0, vec![0xC0 | ch, prog]));
        ev.push((0, 0, vec![0xB0 | ch, 91, 50])); // reverb
        ev.push((0, 0, vec![0xB0 | ch, 93, 30])); // chorus
    }
    for bar in 0..bars {
        let t = bar * 4 * Q;
        let c = CHORDS[(bar % 4) as usize];
        for &k in &c {
            note(&mut ev, 1, k - 12, 70, t, 4 * Q - 10); // strings, the whole bar
            note(&mut ev, 5, k, 50, t, 4 * Q - 10); // organ
        }
        for beat in 0..4 {
            for &k in &c {
                note(&mut ev, 0, k, 80, t + beat * Q, Q / 2); // piano
            }
        }
        for e in 0..8 {
            note(&mut ev, 2, c[0] - 24 + if e % 2 == 1 { 12 } else { 0 }, 100, t + e * Q / 2, Q / 2 - 20); // bass
            note(&mut ev, 3, MELODY[((bar * 8 + e) % 16) as usize], 90, t + e * Q / 2, Q / 2 - 30); // lead
        }
        for s in 0..16 {
            note(&mut ev, 4, c[(s % 4) as usize] + 12, 70, t + s * Q / 4, Q / 4); // arpeggio
            note(&mut ev, 9, 42, 70, t + s * Q / 4, Q / 8); // closed hi-hat
            if s % 4 == 0 {
                note(&mut ev, 9, 36, 110, t + s * Q / 4, Q / 4); // kick
            }
            if s % 8 == 4 {
                note(&mut ev, 9, 38, 100, t + s * Q / 4, Q / 4); // snare
            }
        }
    }
    ev.sort_by_key(|e| (e.0, e.1));
    let mut track = Vec::new();
    track.extend([0x00, 0xFF, 0x51, 0x03, 0x07, 0xA1, 0x20]); // 500000 us a beat
    let mut last = 0;
    for (at, _, bytes) in &ev {
        vlq(at - last, &mut track);
        track.extend(bytes);
        last = *at;
    }
    track.extend([0x00, 0xFF, 0x2F, 0x00]);
    let mut out = b"MThd".to_vec();
    out.extend([0, 0, 0, 6, 0, 0, 0, 1]);
    out.extend((Q as u16).to_be_bytes());
    out.extend(b"MTrk");
    out.extend((track.len() as u32).to_be_bytes());
    out.extend(track);
    out
}

/// ProTracker's periods, C-1 to B-3.
const PERIODS: [u16; 36] = [
    856, 808, 762, 720, 678, 640, 604, 570, 538, 508, 480, 453, 428, 404, 381, 360, 339, 320, 302, 285, 269, 254, 240, 226, 214, 202, 190, 180, 170, 160, 151, 143, 135, 127, 120, 113,
];

/// The period of MIDI key `key`, folded into ProTracker's three octaves.
fn period(key: u8) -> u16 {
    let mut i = i32::from(key) - 48; // C-1 is key 48 here
    while i < 0 {
        i += 12;
    }
    while i > 35 {
        i -= 12;
    }
    PERIODS[i as usize]
}

/// A sample: its name, data (signed 8-bit, an even length), and whether it loops whole.
fn samples() -> Vec<(&'static str, Vec<i8>, bool)> {
    let mut seed = 0x2545_F491u32;
    let mut noise = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed >> 24) as i8
    };
    let saw: Vec<i8> = (0..64).map(|i| (i * 4 - 128) as i8).collect();
    let square: Vec<i8> = (0..64).map(|i| if i < 32 { 90 } else { -90 }).collect();
    let tri: Vec<i8> = (0..128).map(|i| if i < 64 { (i * 4 - 128) as i8 } else { (127 - (i - 64) * 4) as i8 }).collect();
    let kick: Vec<i8> = (0..3000)
        .map(|i| {
            let t = i as f32 / 8363.0;
            let f = 150.0 * (-t * 18.0).exp() + 45.0;
            ((t * f * std::f32::consts::TAU).sin() * 120.0 * (-t * 7.0).exp()) as i8
        })
        .collect();
    let hat: Vec<i8> = (0..800).map(|i| (f32::from(noise()) * (1.0 - i as f32 / 800.0)) as i8).collect();
    vec![("saw lead", saw, true), ("square bass", square, true), ("triangle pad", tri, true), ("kick", kick, false), ("hat", hat, false)]
}

/// A ProTracker module (`M.K.`, or FastTracker's `8CHN`) of `orders`
/// orders of four patterns: lead, bass, kick and hat on the first four
/// channels, a pad's chord and an echo of the lead on the other four.
pub fn module(channels: usize, orders: usize) -> Vec<u8> {
    assert!(channels == 4 || channels == 8);
    let mut out = Vec::new();
    let mut title = b"neumetik r36s test".to_vec();
    title.resize(20, 0);
    out.extend(&title);
    let smp = samples();
    for i in 0..31 {
        let mut h = [0u8; 30];
        if let Some((name, data, looped)) = smp.get(i) {
            h[..name.len()].copy_from_slice(name.as_bytes());
            let words = (data.len() / 2) as u16;
            h[22..24].copy_from_slice(&words.to_be_bytes());
            h[25] = 64; // volume
            h[28..30].copy_from_slice(&(if *looped { words } else { 1 }).to_be_bytes());
        }
        out.extend(h);
    }
    out.push(orders.min(128) as u8);
    out.push(127);
    let mut table = [0u8; 128];
    for (i, t) in table.iter_mut().enumerate().take(orders.min(128)) {
        *t = (i % 4) as u8;
    }
    out.extend(table);
    out.extend(if channels == 8 { b"8CHN" } else { b"M.K." });
    for pat in 0..4u8 {
        let chord = CHORDS[pat as usize];
        for row in 0..64usize {
            for ch in 0..channels {
                // (sample 1-based, key) or nothing
                let cell: Option<(u8, u8, u8, u8)> = match ch {
                    0 if row % 2 == 0 => Some((1, MELODY[(row / 2 + pat as usize * 4) % 16], 0, 0)),
                    1 if row % 4 == 0 => Some((2, chord[0] - 24, 0, 0)),
                    2 if row % 8 == 0 => Some((4, 60, 0, 0)),
                    3 if row % 2 == 0 => Some((5, 72, 0x0C, if row % 4 == 0 { 40 } else { 20 })),
                    4..=6 if row % 16 == 0 => Some((3, chord[ch - 3], 0x0C, 24)),
                    7 if row % 2 == 1 => Some((1, MELODY[(row / 2 + pat as usize * 4) % 16] - 12, 0x0C, 16)),
                    _ => None,
                };
                let mut b = [0u8; 4];
                if let Some((s, key, fx, param)) = cell {
                    let p = period(key);
                    b[0] = (s & 0xF0) | (p >> 8) as u8;
                    b[1] = (p & 0xFF) as u8;
                    b[2] = ((s & 0x0F) << 4) | fx;
                    b[3] = param;
                }
                out.extend(b);
            }
        }
    }
    for (_, data, _) in &smp {
        out.extend(data.iter().map(|&s| s as u8));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_midi_song_reads_and_lasts_as_asked() {
        let song = neumetik_midi::Song::from_midi(&midi_song(30)).unwrap();
        assert!((song.duration() - 30.0).abs() < 1.0, "{}", song.duration());
    }

    #[test]
    fn the_modules_read_as_protracker() {
        for ch in [4, 8] {
            let m = neumetik_tracker::load(&module(ch, 16), "test.mod").unwrap();
            assert_eq!(m.channels.len(), ch);
            assert_eq!(m.orders.len(), 16);
            assert_eq!(m.samples.iter().filter(|s| s.frames() > 0).count(), 5);
        }
    }
}
