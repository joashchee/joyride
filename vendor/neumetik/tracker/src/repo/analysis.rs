//! What a waveform is like, measured, for searching and sorting: its
//! peak and loudness, how bright it is, its pitch where it has one, and
//! a first guess at its category and tags from its names and its sound.

use crate::model::{LoopKind, Sample, Wave};

/// Measurements of a waveform.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Features {
    /// 0 to 1 of full scale.
    pub peak: f32,
    pub rms: f32,
    /// Zero crossings a frame: how bright or noisy.
    pub crossings: f32,
    /// Its period in frames, when it has a steady pitch (0 when not).
    pub period: f32,
}

pub fn measure(w: &Wave) -> Features {
    let ch = usize::from(w.channels.max(1));
    let frames = w.frames();
    if frames == 0 {
        return Features::default();
    }
    let mono: Vec<f32> = (0..frames).map(|i| w.pcm[i * ch..i * ch + ch].iter().map(|&v| f32::from(v)).sum::<f32>() / (ch as f32 * 32768.0)).collect();
    let peak = mono.iter().fold(0f32, |a, v| a.max(v.abs()));
    let rms = (mono.iter().map(|v| v * v).sum::<f32>() / frames as f32).sqrt();
    let crossings = mono.windows(2).filter(|p| (p[0] < 0.0) != (p[1] < 0.0)).count() as f32 / frames as f32;
    Features { peak, rms, crossings, period: pitch_period(&mono) }
}

/// The fundamental's period in frames by autocorrelation over a window
/// from where the sound has settled; 0 when nothing repeats clearly.
fn pitch_period(x: &[f32]) -> f32 {
    let n = x.len();
    if n < 64 {
        return 0.0;
    }
    let start = (n / 8).min(n.saturating_sub(2048));
    let len = (n - start).min(2048);
    let w = &x[start..start + len];
    let energy: f32 = w.iter().map(|v| v * v).sum();
    if energy < 1e-6 {
        return 0.0;
    }
    let most = (len / 2).min(1024);
    let corr = |lag: usize| -> f32 {
        let (mut s, mut a, mut b) = (0f32, 0f32, 0f32);
        for i in 0..len - lag {
            s += w[i] * w[i + lag];
            a += w[i] * w[i];
            b += w[i + lag] * w[i + lag];
        }
        if a <= 0.0 || b <= 0.0 {
            0.0
        } else {
            s / (a * b).sqrt()
        }
    };
    // The first lag past the first dip that correlates well.
    let mut lag = 2;
    while lag < most && corr(lag) > 0.3 {
        lag += 1;
    }
    let (mut best, mut best_lag) = (0f32, 0usize);
    while lag < most {
        let c = corr(lag);
        if c > best {
            best = c;
            best_lag = lag;
        }
        if best > 0.85 && c < best - 0.2 {
            break;
        }
        lag += 1;
    }
    if best < 0.8 || best_lag == 0 {
        return 0.0;
    }
    // A finer peak, between the lags either side.
    let (a, b, c) = (corr(best_lag - 1), best, corr(best_lag + 1));
    let d = a - 2.0 * b + c;
    let off = if d.abs() > 1e-6 { 0.5 * (a - c) / d } else { 0.0 };
    best_lag as f32 + off.clamp(-0.5, 0.5)
}

/// Bands of the fingerprint's spectrum, log-spaced over nine octaves
/// (a band is 3/16 of an octave), and points of its loudness over time.
pub const BANDS: usize = 48;
pub const ENVELOPE: usize = 16;
const FFT: usize = 1024;

/// An in-place radix-2 FFT of `re` and `im` (their length a power of two).
fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -2.0 * std::f32::consts::PI / len as f32;
        for start in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (s, c) = (ang * k as f32).sin_cos();
                let (a, b) = (start + k, start + k + len / 2);
                let (tr, ti) = (re[b] * c - im[b] * s, re[b] * s + im[b] * c);
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
            }
        }
        len <<= 1;
    }
}

/// A waveform's fingerprint, for finding samples that sound alike: its
/// average spectrum in `BANDS` log-spaced bands (each in decibels, the
/// loudest 255, 60 dB down 0), then its loudness at `ENVELOPE` points
/// through it. Level doesn't change it, and resampling only slides the
/// spectrum along (which `similarity` allows for).
pub fn fingerprint(w: &Wave) -> Vec<u8> {
    let ch = usize::from(w.channels.max(1));
    let frames = w.frames();
    if frames == 0 {
        return Vec::new();
    }
    let mono: Vec<f32> = (0..frames).map(|i| w.pcm[i * ch..i * ch + ch].iter().map(|&v| f32::from(v)).sum::<f32>() / (ch as f32 * 32768.0)).collect();
    // Up to 32 windows spread through it.
    let windows = (frames / FFT).clamp(1, 32);
    let step = if frames > FFT { (frames - FFT) / windows.max(1) } else { 0 };
    let mut power = vec![0f32; FFT / 2];
    let (mut re, mut im) = (vec![0f32; FFT], vec![0f32; FFT]);
    for k in 0..windows {
        let at = k * step;
        for i in 0..FFT {
            let hann = 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / FFT as f32).cos();
            re[i] = mono.get(at + i).copied().unwrap_or(0.0) * hann;
            im[i] = 0.0;
        }
        fft(&mut re, &mut im);
        for (b, p) in power.iter_mut().enumerate() {
            *p += re[b] * re[b] + im[b] * im[b];
        }
    }
    let mut bands = [0f32; BANDS];
    for (b, band) in bands.iter_mut().enumerate() {
        let lo = 2f32.powf(b as f32 * 9.0 / BANDS as f32);
        let hi = 2f32.powf((b + 1) as f32 * 9.0 / BANDS as f32);
        let (a, z) = (lo.floor() as usize, (hi.ceil() as usize).min(FFT / 2));
        *band = power[a.max(1)..z.max(a.max(1) + 1).min(FFT / 2)].iter().sum::<f32>() / (z.saturating_sub(a)).max(1) as f32;
    }
    let top = bands.iter().fold(0f32, |m, v| m.max(*v)).max(1e-20);
    let mut out: Vec<u8> = bands.iter().map(|v| ((10.0 * (v / top).max(1e-6).log10() + 60.0) / 60.0 * 255.0).clamp(0.0, 255.0) as u8).collect();
    // Loudness through it.
    let env: Vec<f32> = (0..ENVELOPE)
        .map(|k| {
            let (a, b) = (k * frames / ENVELOPE, ((k + 1) * frames / ENVELOPE).max(k * frames / ENVELOPE + 1).min(frames));
            (mono[a..b].iter().map(|v| v * v).sum::<f32>() / (b - a) as f32).sqrt()
        })
        .collect();
    let loudest = env.iter().fold(0f32, |m, v| m.max(*v)).max(1e-9);
    out.extend(env.iter().map(|v| (v / loudest * 255.0) as u8));
    out
}

/// How alike two fingerprints are, 0 to 1: their spectra (the best of
/// them slid up to three octaves either way, for a resampled copy) and
/// their loudness over time.
pub fn similarity(a: &[u8], b: &[u8]) -> f32 {
    if a.len() < BANDS + ENVELOPE || b.len() < BANDS + ENVELOPE {
        return 0.0;
    }
    let cos = |x: &[f32], y: &[f32]| -> f32 {
        let (mut s, mut p, mut q) = (0f32, 0f32, 0f32);
        for (u, v) in x.iter().zip(y) {
            s += u * v;
            p += u * u;
            q += v * v;
        }
        if p <= 0.0 || q <= 0.0 {
            0.0
        } else {
            s / (p * q).sqrt()
        }
    };
    let spec = |f: &[u8]| -> Vec<f32> { f[..BANDS].iter().map(|&v| f32::from(v) / 255.0).collect() };
    let (sa, sb) = (spec(a), spec(b));
    let mut best = 0f32;
    for shift in -16i32..=16 {
        let (x, y): (Vec<f32>, Vec<f32>) = (0..BANDS as i32).filter_map(|i| {
            let j = i + shift;
            (0..BANDS as i32).contains(&j).then(|| (sa[i as usize], sb[j as usize]))
        }).unzip();
        if x.len() < BANDS / 2 {
            continue;
        }
        // A little less for a farther slide, so the same pitch wins a tie.
        best = best.max(cos(&x, &y) - shift.unsigned_abs() as f32 * 0.002);
    }
    let env = |f: &[u8]| -> Vec<f32> { f[BANDS..BANDS + ENVELOPE].iter().map(|&v| f32::from(v) / 255.0).collect() };
    (best * 0.75 + cos(&env(a), &env(b)) * 0.25).clamp(0.0, 1.0)
}

/// The note name nearest a frequency (C-5 is middle C), and how many
/// cents off.
pub fn note_of(hz: f32) -> Option<(String, i32)> {
    if !(hz.is_finite() && hz > 8.0) {
        return None;
    }
    let semis = 12.0 * (hz / 261.6256).log2();
    let n = semis.round();
    let cents = ((semis - n) * 100.0).round() as i32;
    let note = (60 + n as i32).clamp(0, 119) as u8 + 1;
    Some((crate::model::note_name(note), cents))
}

/// The categories, in the order they're offered.
pub const CATEGORIES: &[&str] = &[
    "Kick",
    "Snare",
    "Clap",
    "Hi-hat",
    "Cymbal",
    "Tom",
    "Percussion",
    "Drum loop",
    "Bass",
    "Piano",
    "Electric piano",
    "Organ",
    "Mallets & bells",
    "Strings",
    "Brass",
    "Woodwind",
    "Guitar",
    "Plucked",
    "Choir & voice",
    "Pad",
    "Lead",
    "Synth",
    "Chip waveform",
    "Orchestral hit",
    "Sound effect",
    "Other",
];

/// Words in names that point to a category: whole words, or the start
/// of one for the longer ones.
const WORDS: &[(&str, &[&str])] = &[
    ("Kick", &["kick", "bd", "bassdrum", "kik", "bdrum", "kck", "909bd", "808bd"]),
    ("Snare", &["snare", "sd", "snr", "snar", "rimshot"]),
    ("Clap", &["clap", "clp", "handclap"]),
    ("Hi-hat", &["hihat", "hat", "hh", "ohh", "chh", "ohat", "chat", "openhat", "closedhat", "hhat"]),
    ("Cymbal", &["crash", "ride", "cymbal", "cym", "splash", "china", "crsh"]),
    ("Tom", &["tom", "toms", "tomtom", "floortom"]),
    ("Percussion", &["perc", "conga", "bongo", "cowbell", "shaker", "tamb", "tambourine", "clave", "claves", "timbale", "agogo", "triangle", "guiro", "cabasa", "woodblock", "block", "rim", "snap", "drum", "drums", "timpani", "tabla"]),
    ("Drum loop", &["break", "breakbeat", "beat", "amen", "drumloop", "groove"]),
    ("Bass", &["bass", "bas", "sub", "subbass", "303", "fretless", "slap", "basse"]),
    ("Piano", &["piano", "pno", "grand", "pian"]),
    ("Electric piano", &["rhodes", "ep", "wurli", "epiano", "wurlitzer", "dx7"]),
    ("Organ", &["organ", "org", "hammond", "orgel", "accordion", "harmonium"]),
    ("Mallets & bells", &["marimba", "xylo", "xylophone", "vibes", "vibraphone", "glock", "glockenspiel", "bell", "bells", "celesta", "kalimba", "tubular", "chime", "chimes", "musicbox"]),
    ("Strings", &["string", "strings", "str", "violin", "viola", "cello", "contrabass", "orch", "orchestra", "ensemble", "strng", "pizz", "pizzicato"]),
    ("Brass", &["brass", "trumpet", "trp", "trombone", "horn", "horns", "tuba", "fanfare"]),
    ("Woodwind", &["flute", "flt", "sax", "saxophone", "clarinet", "oboe", "bassoon", "pipe", "panflute", "piccolo", "recorder", "whistle", "ocarina"]),
    ("Guitar", &["guitar", "gtr", "git", "guit", "dist", "distortion", "strat", "nylon", "acoustic", "chord", "powerchord", "riff"]),
    ("Plucked", &["harp", "pluck", "koto", "sitar", "banjo", "mandolin", "harpsichord", "clav", "clavinet", "lute", "shamisen"]),
    ("Choir & voice", &["choir", "vox", "voice", "aah", "ooh", "vocal", "vocals", "sing", "voc", "speech", "yeah", "hey", "chant"]),
    ("Pad", &["pad", "warm", "atmos", "atmosphere", "ambient", "sweep", "drone", "strpad"]),
    ("Lead", &["lead", "ld", "solo"]),
    ("Synth", &["synth", "syn", "saw", "sawtooth", "square", "sqr", "pulse", "sine", "triangle", "analog", "moog", "juno", "acid", "stab", "arp", "fm"]),
    ("Orchestral hit", &["hit", "orchhit", "orchit", "stab"]),
    ("Sound effect", &["fx", "sfx", "noise", "explosion", "laser", "zap", "wind", "rain", "scratch", "riser", "boom", "crash", "siren", "sweep", "blip", "bleep", "beep", "shot", "gun", "thunder", "water", "reverse", "rev"]),
];

/// The category a sample seems to be, from its names (any of them) and
/// how it sounds: "Other" when nothing says.
pub fn guess_category(names: &[&str], s: &Sample, f: &Features) -> &'static str {
    let mut words: Vec<String> = Vec::new();
    for n in names {
        let lower = n.to_lowercase();
        for w in lower.split(|c: char| !c.is_ascii_alphanumeric()) {
            if !w.is_empty() {
                // Digits stuck to a word ("kick2", "bd01") come off.
                let core = w.trim_end_matches(|c: char| c.is_ascii_digit());
                words.push(if core.is_empty() { w.to_string() } else { core.to_string() });
            }
        }
    }
    for (cat, keys) in WORDS {
        if words.iter().any(|w| keys.iter().any(|k| w == k || (k.len() >= 5 && w.starts_with(k)))) {
            return cat;
        }
    }
    let frames = s.wave.frames();
    if s.looped.on() && frames <= 1024 && s.looped.start as usize <= frames / 8 {
        return "Chip waveform";
    }
    if f.crossings > 0.25 && f.period == 0.0 {
        return "Sound effect";
    }
    "Other"
}

/// Tags a sample gets from what it is.
pub fn auto_tags(s: &Sample, frames: usize, rate: u32) -> Vec<String> {
    let mut t = vec![if s.wave.bits <= 8 { "8-bit" } else { "16-bit" }.to_string()];
    if s.wave.channels == 2 {
        t.push("stereo".into());
    }
    match s.looped.kind {
        LoopKind::Forward if s.looped.on() => t.push("looped".into()),
        LoopKind::PingPong if s.looped.on() => {
            t.push("looped".into());
            t.push("ping-pong".into());
        }
        _ if !s.sustain.on() => t.push("one-shot".into()),
        _ => {}
    }
    if s.sustain.on() {
        t.push("sustain loop".into());
    }
    if s.looped.on() && frames <= 1024 {
        t.push("single-cycle".into());
    }
    let secs = frames as f32 / rate.max(1) as f32;
    if secs < 0.1 {
        t.push("short".into());
    } else if secs > 5.0 {
        t.push("long".into());
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Loop;
    use std::sync::Arc;

    #[test]
    fn a_sine_has_its_period() {
        let pcm: Vec<i16> = (0..8000).map(|i| ((i as f32 * std::f32::consts::TAU / 50.0).sin() * 20000.0) as i16).collect();
        let f = measure(&Wave::new(16, 1, pcm));
        assert!((f.period - 50.0).abs() < 0.5, "{}", f.period);
        assert!(f.peak > 0.6 && f.rms > 0.4);
        assert_eq!(note_of(261.6).unwrap().0, "C-5");
        assert_eq!(note_of(440.0).unwrap(), ("A-5".to_string(), 0));
    }

    #[test]
    fn names_say_what_it_is() {
        let s = Sample { wave: Arc::new(Wave::new(8, 1, vec![0; 3000])), ..Sample::empty() };
        let f = Features::default();
        assert_eq!(guess_category(&["Kick2"], &s, &f), "Kick");
        assert_eq!(guess_category(&["st-01:strings"], &s, &f), "Strings");
        assert_eq!(guess_category(&["HiHat closed"], &s, &f), "Hi-hat");
        assert_eq!(guess_category(&["xyz"], &s, &f), "Other");
        let chip = Sample { wave: Arc::new(Wave::new(8, 1, vec![0; 32])), looped: Loop::forward(0, 32), ..Sample::empty() };
        assert_eq!(guess_category(&["x"], &chip, &f), "Chip waveform");
    }
}
