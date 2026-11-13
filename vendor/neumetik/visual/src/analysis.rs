// Copyright 2026 ansiapps. Neumetik Visual: proprietary, see the LICENSE at the repository root.

//! What the visualizations know of the sound: its spectrum (a radix-2 FFT
//! under a Hann window) in log-spaced bands with falling peaks; levels as
//! a VU meter (300 ms to rise, IEC 60268-17) and a peak meter (instant
//! rise, 20 dB in 1.7 s to fall, as a PPM's) show them; onsets by spectral
//! flux over an adaptive threshold (Bello et al., "A Tutorial on Onset
//! Detection in Music Signals", 2005) and the tempo between them; the
//! spectral centroid (brightness); a chromagram (each pitch class's share,
//! Fujishima 1999); and the stereo picture (correlation and balance).

use std::f32::consts::PI;

/// The FFT's size: 2048 points, 43 ms at 48 kHz.
pub const FFT_SIZE: usize = 2048;
/// The quietest level the meters show, in dB below full scale.
pub const FLOOR_DB: f32 = -60.0;

/// An in-place radix-2 FFT of a fixed size.
pub struct Fft {
    n: usize,
    cos: Vec<f32>,
    sin: Vec<f32>,
    rev: Vec<usize>,
}

impl Fft {
    pub fn new(n: usize) -> Fft {
        assert!(n.is_power_of_two() && n >= 2);
        let bits = n.trailing_zeros();
        let rev = (0..n).map(|i| i.reverse_bits() >> (usize::BITS - bits)).collect();
        let cos = (0..n / 2).map(|k| (2.0 * PI * k as f32 / n as f32).cos()).collect();
        let sin = (0..n / 2).map(|k| -(2.0 * PI * k as f32 / n as f32).sin()).collect();
        Fft { n, cos, sin, rev }
    }

    pub fn size(&self) -> usize {
        self.n
    }

    pub fn forward(&self, re: &mut [f32], im: &mut [f32]) {
        let n = self.n;
        for i in 0..n {
            let j = self.rev[i];
            if j > i {
                re.swap(i, j);
                im.swap(i, j);
            }
        }
        let mut len = 2;
        while len <= n {
            let step = n / len;
            for start in (0..n).step_by(len) {
                for k in 0..len / 2 {
                    let (c, s) = (self.cos[k * step], self.sin[k * step]);
                    let (a, b) = (start + k, start + k + len / 2);
                    let tr = re[b] * c - im[b] * s;
                    let ti = re[b] * s + im[b] * c;
                    re[b] = re[a] - tr;
                    im[b] = im[a] - ti;
                    re[a] += tr;
                    im[a] += ti;
                }
            }
            len <<= 1;
        }
    }
}

/// Everything known of the sound at a frame, for the visualizations.
#[derive(Clone, Debug)]
pub struct Analysis {
    /// Each FFT bin's level, 0 (the floor) to 1 (full scale), unsmoothed.
    pub spectrum: Vec<f32>,
    /// The Hz each bin is apart.
    pub bin_hz: f32,
    /// The bands, log-spaced from 30 Hz up, 0 to 1, falling smoothly.
    pub bands: Vec<f32>,
    /// Each band's peak, held and falling.
    pub peaks: Vec<f32>,
    /// The bands' edges in Hz (one more than the bands).
    pub edges: Vec<f32>,
    /// The latest samples, mono and each side, oldest first (the FFT's
    /// window's worth).
    pub wave: Vec<f32>,
    pub left: Vec<f32>,
    pub right: Vec<f32>,
    /// VU (average) and peak levels each side, 0 to 1 on the meters' dB
    /// scale ([`FLOOR_DB`] to 0), and their dB.
    pub vu: [f32; 2],
    pub ppm: [f32; 2],
    pub vu_db: [f32; 2],
    pub ppm_db: [f32; 2],
    /// The highest peak in the last two seconds, each side.
    pub hold: [f32; 2],
    /// The sound clipped (reached full scale) lately, each side: 1 then falling.
    pub clip: [f32; 2],
    /// Low, middle and high's energy, 0 to 1, smoothed.
    pub bass: f32,
    pub mid: f32,
    pub treble: f32,
    /// The whole sound's level, 0 to 1, smoothed.
    pub energy: f32,
    /// An onset just now.
    pub onset: bool,
    /// 1 at an onset, falling to 0 within a third of a second.
    pub beat: f32,
    /// Beats counted since it started.
    pub beats: u64,
    /// The tempo the onsets keep, when they keep one, in beats a minute.
    pub bpm: Option<f32>,
    /// The spectral centroid, 0 (dull) to 1 (bright).
    pub brightness: f32,
    /// Each pitch class's share (C first), the largest 1.
    pub chroma: [f32; 12],
    /// -1 (out of phase) to 1 (mono).
    pub correlation: f32,
    /// -1 (all left) to 1 (all right).
    pub balance: f32,
    /// Seconds since it started, and since the frame before.
    pub time: f64,
    pub dt: f32,
    /// Whether there's sound at all.
    pub silent: bool,
}

/// How the analysis responds.
#[derive(Clone, Copy, Debug)]
pub struct Response {
    /// Gain before analysis (the visual's sensitivity), 1 as it is.
    pub gain: f32,
    /// How slowly the bands fall, 0 (at once) to 1 (very slowly).
    pub smoothing: f32,
}

impl Default for Response {
    fn default() -> Response {
        Response { gain: 1.0, smoothing: 0.5 }
    }
}

pub struct Analyzer {
    fft: Fft,
    window: Vec<f32>,
    re: Vec<f32>,
    im: Vec<f32>,
    prev: Vec<f32>,
    flux: Vec<f32>,
    since_onset: f32,
    intervals: Vec<f32>,
    hold_age: [f32; 2],
    pub out: Analysis,
}

fn db(v: f32) -> f32 {
    20.0 * v.max(1e-9).log10()
}

fn scale(db: f32) -> f32 {
    ((db - FLOOR_DB) / -FLOOR_DB).clamp(0.0, 1.0)
}

impl Default for Analyzer {
    fn default() -> Analyzer {
        Analyzer::new()
    }
}

impl Analyzer {
    pub fn new() -> Analyzer {
        let n = FFT_SIZE;
        let window = (0..n).map(|i| 0.5 - 0.5 * (2.0 * PI * i as f32 / (n - 1) as f32).cos()).collect();
        Analyzer {
            fft: Fft::new(n),
            window,
            re: vec![0.0; n],
            im: vec![0.0; n],
            prev: vec![0.0; n / 2],
            flux: Vec::new(),
            since_onset: 10.0,
            intervals: Vec::new(),
            hold_age: [0.0; 2],
            out: Analysis {
                spectrum: vec![0.0; n / 2],
                bin_hz: 0.0,
                bands: Vec::new(),
                peaks: Vec::new(),
                edges: Vec::new(),
                wave: vec![0.0; n],
                left: vec![0.0; n],
                right: vec![0.0; n],
                vu: [0.0; 2],
                ppm: [0.0; 2],
                vu_db: [FLOOR_DB; 2],
                ppm_db: [FLOOR_DB; 2],
                hold: [0.0; 2],
                clip: [0.0; 2],
                bass: 0.0,
                mid: 0.0,
                treble: 0.0,
                energy: 0.0,
                onset: false,
                beat: 0.0,
                beats: 0,
                bpm: None,
                brightness: 0.0,
                chroma: [0.0; 12],
                correlation: 1.0,
                balance: 0.0,
                time: 0.0,
                dt: 0.0,
                silent: true,
            },
        }
    }

    /// The analysis of the latest sound: `left` and `right` end with the
    /// newest sample (any length; the last [`FFT_SIZE`] are used, fewer
    /// padded with silence), `fresh` of them new since the last call, at
    /// `rate`, `dt` seconds after it; in `bands` bands.
    pub fn update(&mut self, left: &[f32], right: &[f32], fresh: usize, rate: u32, dt: f32, bands: usize, response: Response) -> &Analysis {
        let n = FFT_SIZE;
        let dt = dt.clamp(0.0, 0.5);
        let rate = rate.max(8000) as f32;
        let gain = response.gain.clamp(0.05, 20.0);
        let a = &mut self.out;
        a.time += f64::from(dt);
        a.dt = dt;
        // The window's worth, newest last.
        let take = |src: &[f32], dst: &mut [f32]| {
            let k = src.len().min(n);
            dst[..n - k].fill(0.0);
            for (d, s) in dst[n - k..].iter_mut().zip(&src[src.len() - k..]) {
                *d = s * gain;
            }
        };
        take(left, &mut a.left);
        take(right, &mut a.right);
        for i in 0..n {
            a.wave[i] = 0.5 * (a.left[i] + a.right[i]);
        }

        // Levels over the fresh samples (the latest dt's).
        let fresh = fresh.clamp(1, n);
        let mut peak = [0f32; 2];
        let mut sum = [0f32; 2];
        let (mut lr, mut ll, mut rr) = (0f32, 0f32, 0f32);
        for i in n - fresh..n {
            let (l, r) = (a.left[i], a.right[i]);
            peak[0] = peak[0].max(l.abs());
            peak[1] = peak[1].max(r.abs());
            sum[0] += l * l;
            sum[1] += r * r;
            lr += l * r;
            ll += l * l;
            rr += r * r;
        }
        a.silent = peak[0].max(peak[1]) < 1e-4;
        let vu_k = 1.0 - (-dt / 0.065).exp(); // reaches 99% in 300 ms
        let ppm_fall = 20.0 / 1.7 * dt;
        for s in 0..2 {
            let rms = (sum[s] / fresh as f32).sqrt();
            // A sine's VU reads its RMS +3 dB (the meter's sine calibration).
            let target = db(rms * std::f32::consts::SQRT_2);
            a.vu_db[s] += (target.max(FLOOR_DB) - a.vu_db[s]) * vu_k;
            let p = db(peak[s]);
            a.ppm_db[s] = if p > a.ppm_db[s] { p } else { (a.ppm_db[s] - ppm_fall).max(p).max(FLOOR_DB) };
            a.vu[s] = scale(a.vu_db[s]);
            a.ppm[s] = scale(a.ppm_db[s]);
            self.hold_age[s] += dt;
            if a.ppm[s] >= a.hold[s] || self.hold_age[s] > 2.0 {
                a.hold[s] = a.ppm[s];
                self.hold_age[s] = 0.0;
            }
            a.clip[s] = if peak[s] >= 0.999 { 1.0 } else { (a.clip[s] - dt).max(0.0) };
        }
        let corr = if ll > 1e-9 && rr > 1e-9 { lr / (ll * rr).sqrt() } else { 1.0 };
        a.correlation += (corr - a.correlation) * (1.0 - (-dt / 0.2).exp());
        let bal = if ll + rr > 1e-9 { (rr.sqrt() - ll.sqrt()) / (rr.sqrt() + ll.sqrt()) } else { 0.0 };
        a.balance += (bal - a.balance) * (1.0 - (-dt / 0.2).exp());

        // The spectrum.
        for i in 0..n {
            self.re[i] = a.wave[i] * self.window[i];
            self.im[i] = 0.0;
        }
        self.fft.forward(&mut self.re, &mut self.im);
        let bin_hz = rate / n as f32;
        a.bin_hz = bin_hz;
        // Full scale: a sine at 0 dBFS peaks at n/4 under the Hann window.
        let norm = 4.0 / n as f32;
        let mut flux = 0.0;
        let (mut weighted, mut total) = (0f32, 0f32);
        let mut chroma = [0f32; 12];
        for k in 1..n / 2 {
            let mag = (self.re[k] * self.re[k] + self.im[k] * self.im[k]).sqrt() * norm;
            let level = scale(db(mag) + 6.0);
            a.spectrum[k] = level;
            let rise = mag - self.prev[k];
            // The low end counts most toward an onset: that's where the beat is.
            let f = k as f32 * bin_hz;
            if rise > 0.0 {
                flux += rise * if f < 200.0 { 2.0 } else { 1.0 };
            }
            self.prev[k] = mag;
            weighted += mag * f;
            total += mag;
            if (55.0..2000.0).contains(&f) {
                let pc = ((12.0 * (f / 440.0).log2()).round() as i32 + 69).rem_euclid(12) as usize;
                chroma[pc] += mag * mag;
            }
        }
        a.spectrum[0] = 0.0;
        let centroid = if total > 1e-6 { weighted / total } else { 0.0 };
        let bright = ((centroid.max(50.0) / 50.0).log2() / (8000.0f32 / 50.0).log2()).clamp(0.0, 1.0);
        a.brightness += (bright - a.brightness) * (1.0 - (-dt / 0.25).exp());
        let top = chroma.iter().cloned().fold(0.0, f32::max);
        for (c, v) in a.chroma.iter_mut().zip(chroma) {
            let t = if top > 1e-9 { v / top } else { 0.0 };
            *c += (t - *c) * (1.0 - (-dt / 0.15).exp());
        }

        // The bands, log-spaced from 30 Hz to 16 kHz (or the top).
        let bands = bands.clamp(1, 512);
        let hi = (rate / 2.0).min(16_000.0);
        if a.bands.len() != bands || a.edges.last().is_none_or(|&e| (e - hi).abs() > 1.0) {
            a.bands = vec![0.0; bands];
            a.peaks = vec![0.0; bands];
            a.edges = (0..=bands).map(|i| 30.0 * (hi / 30.0).powf(i as f32 / bands as f32)).collect();
        }
        let fall = (0.25 + 2.5 * (1.0 - response.smoothing.clamp(0.0, 1.0))) * dt;
        for b in 0..bands {
            let lo_k = (a.edges[b] / bin_hz).floor() as usize;
            let hi_k = ((a.edges[b + 1] / bin_hz).ceil() as usize).max(lo_k + 1).min(n / 2 - 1);
            let lo_k = lo_k.clamp(1, hi_k);
            let mut v = 0f32;
            // Narrow bands at the bottom fall between bins: read between them.
            if hi_k - lo_k <= 1 {
                let centre = (a.edges[b] * a.edges[b + 1]).sqrt() / bin_hz;
                let k = centre.floor() as usize;
                let t = centre - k as f32;
                let at = |k: usize| a.spectrum.get(k.clamp(1, n / 2 - 1)).copied().unwrap_or(0.0);
                v = at(k) * (1.0 - t) + at(k + 1) * t;
            } else {
                for k in lo_k..hi_k {
                    v = v.max(a.spectrum[k]);
                }
            }
            // Higher bands are wider and hold less energy a bin: tilt them up a little.
            let v = (v + 0.08 * b as f32 / bands as f32).min(1.0) * if a.silent { 0.0 } else { 1.0 };
            a.bands[b] = if v > a.bands[b] { v } else { (a.bands[b] - fall).max(v) };
            a.peaks[b] = if a.bands[b] >= a.peaks[b] { a.bands[b] } else { (a.peaks[b] - 0.35 * dt).max(a.bands[b]) };
        }
        let range = |lo: f32, hi: f32| {
            let (lo, hi) = ((lo / bin_hz) as usize, ((hi / bin_hz) as usize).min(n / 2 - 1));
            let s: f32 = a.spectrum[lo.max(1)..hi.max(lo + 2)].iter().sum();
            s / (hi.max(lo + 2) - lo.max(1)) as f32
        };
        let smooth = |old: f32, new: f32| if new > old { old + (new - old) * 0.6 } else { old + (new - old) * (1.0 - (-dt / 0.2).exp()) };
        a.bass = smooth(a.bass, (range(30.0, 250.0) * 1.3).min(1.0));
        a.mid = smooth(a.mid, (range(250.0, 2500.0) * 1.4).min(1.0));
        a.treble = smooth(a.treble, (range(2500.0, 12_000.0) * 1.6).min(1.0));
        a.energy = smooth(a.energy, (a.vu[0] + a.vu[1]) * 0.5);

        // Onsets: the flux over its recent median and spread.
        self.flux.push(flux);
        if self.flux.len() > 43 {
            self.flux.remove(0);
        }
        let mut sorted = self.flux.clone();
        sorted.sort_by(|x, y| x.total_cmp(y));
        let median = sorted[sorted.len() / 2];
        let mean = self.flux.iter().sum::<f32>() / self.flux.len() as f32;
        let threshold = median + 1.5 * (mean - median).abs() + 0.02 + mean * 0.4;
        self.since_onset += dt;
        a.onset = fresh > 0 && !a.silent && flux > threshold && self.since_onset > 0.18 && self.flux.len() > 4;
        if a.onset {
            if (0.25..2.0).contains(&self.since_onset) {
                self.intervals.push(self.since_onset);
                if self.intervals.len() > 16 {
                    self.intervals.remove(0);
                }
            }
            self.since_onset = 0.0;
            a.beat = 1.0;
            a.beats += 1;
        } else {
            a.beat = (a.beat - dt * 3.0).max(0.0);
        }
        a.bpm = if self.intervals.len() >= 4 {
            let mut iv = self.intervals.clone();
            iv.sort_by(|x, y| x.total_cmp(y));
            let mut bpm = 60.0 / iv[iv.len() / 2];
            while bpm < 70.0 {
                bpm *= 2.0;
            }
            while bpm > 180.0 {
                bpm /= 2.0;
            }
            Some(bpm)
        } else {
            None
        };
        &self.out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(f: f32, amp: f32, n: usize) -> Vec<f32> {
        (0..n).map(|i| amp * (2.0 * PI * f * i as f32 / 48_000.0).sin()).collect()
    }

    #[test]
    fn the_fft_finds_a_sine_in_its_bin() {
        let fft = Fft::new(256);
        let mut re: Vec<f32> = (0..256).map(|i| (2.0 * PI * 8.0 * i as f32 / 256.0).cos()).collect();
        let mut im = vec![0.0; 256];
        fft.forward(&mut re, &mut im);
        assert!((re[8] - 128.0).abs() < 0.01, "{}", re[8]);
        assert!(re[9].abs() < 0.01 && im[3].abs() < 0.01);
    }

    #[test]
    fn a_full_scale_sine_reads_near_zero_db_where_it_is() {
        let mut an = Analyzer::new();
        let s = sine(1000.0, 1.0, FFT_SIZE);
        for _ in 0..60 {
            an.update(&s, &s, 800, 48_000, 1.0 / 60.0, 32, Response::default());
        }
        let a = &an.out;
        assert!(a.ppm_db[0] > -0.5, "{}", a.ppm_db[0]);
        assert!((a.vu_db[0] - 0.0).abs() < 0.5, "{}", a.vu_db[0]);
        let band = a.edges.iter().position(|&e| e > 1000.0).unwrap() - 1;
        let top = a.bands.iter().cloned().fold(0.0, f32::max);
        assert_eq!(a.bands[band], top);
        assert!(a.bands[band - 3] < 0.2 && a.bands[band + 3] < 0.2);
        assert!(a.correlation > 0.99);
        // 1 kHz is nearest a B (987.8 Hz).
        assert_eq!(a.chroma.iter().enumerate().max_by(|x, y| x.1.total_cmp(y.1)).unwrap().0, 11, "{:?}", a.chroma);
    }

    #[test]
    fn clicks_a_half_second_apart_are_onsets_at_120_bpm() {
        let mut an = Analyzer::new();
        let mut buf = vec![0f32; FFT_SIZE];
        let mut t = 0usize;
        let step = 800; // 60 frames a second
        for _ in 0..600 {
            for _ in 0..step {
                buf.remove(0);
                let k = t % 24_000;
                buf.push(if k < 600 { (1.0 - k as f32 / 600.0) * if (t / 7) % 2 == 0 { 0.9 } else { -0.9 } } else { 0.0 });
                t += 1;
            }
            an.update(&buf, &buf, step, 48_000, 1.0 / 60.0, 16, Response::default());
        }
        let bpm = an.out.bpm.expect("a tempo");
        assert!((bpm - 120.0).abs() < 4.0, "{bpm}");
    }
}
