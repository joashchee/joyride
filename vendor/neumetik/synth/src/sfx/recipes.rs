// Copyright 2026 ansiapps. Neumetik Synth: proprietary, see the LICENSE at the repository root.

//! The recipes. Levels inside a recipe are relative: [`super::render`]
//! brings every sound to the same peak.

use super::kit::{add, bandpass, decay, drive, env, filter, glide, highpass, lowpass, mul, norm, reverb, scale, Kit, Mode, Wave};
use super::Recipe;
use std::f32::consts::TAU;

/// A boom: a sine dropping from `f0` to `f1` over `fall` seconds, dying in `t60`.
fn boom(k: &Kit, s: f32, f0: f32, f1: f32, fall: f32, t60: f32) -> Vec<f32> {
    let mut x = k.tone(s, Wave::Sine, glide(f0, f1, fall));
    let n = x.len();
    mul(&mut x, &decay(n, k.rate, 0.002, t60));
    x
}

/// Noise low-passed at `cut`, struck and dying in `t60`: the body of a thud.
fn thud(k: &mut Kit, s: f32, cut: f32, t60: f32) -> Vec<f32> {
    let mut x = lowpass(&k.white(s), k.rate, cut);
    let n = x.len();
    mul(&mut x, &decay(n, k.rate, 0.001, t60));
    x
}

/// A slow random level, a new target `per_sec` times a second, between `lo` and 1.
fn wobble(k: &mut Kit, s: f32, per_sec: f32, lo: f32) -> Vec<f32> {
    let steps = (s * per_sec).ceil() as usize + 1;
    let points: Vec<(f32, f32)> = (0..=steps).map(|i| (i as f32 / per_sec, k.range(lo, 1.0))).collect();
    env(k.len(s), k.rate, &points)
}

/// Noise switched on and off in 5 ms pieces, on `on` of the time: sparks.
fn sparks(k: &mut Kit, s: f32, on: f32) -> Vec<f32> {
    let mut x = k.white(s);
    let piece = k.len(0.005).max(1);
    let mut gate = 0.0;
    for (i, v) in x.iter_mut().enumerate() {
        if i % piece == 0 {
            gate = if k.rand() < on { k.range(0.3, 1.0) } else { 0.0 };
        }
        *v *= gate;
    }
    x
}

fn sum(parts: &[(&[f32], f32)]) -> Vec<f32> {
    let mut out = Vec::new();
    for (p, g) in parts {
        add(&mut out, p, 0, *g);
    }
    out
}

fn normed(mut x: Vec<f32>) -> Vec<f32> {
    norm(&mut x);
    x
}

fn shaped(mut x: Vec<f32>, e: &[f32]) -> Vec<f32> {
    mul(&mut x, e);
    x
}

// The sub-bass layer: weight under the big moments, its harmonics driven
// up so small speakers still give a sense of it.

fn sub_impact(k: &mut Kit) -> Vec<f32> {
    let b = boom(k, 1.6, 60.0, 32.0, 0.25, 1.4);
    let t = thud(k, 1.6, 180.0, 0.25);
    let mut x = sum(&[(&b, 1.0), (&t, 0.7)]);
    drive(&mut x, 1.8);
    x
}

fn sub_critical(k: &mut Kit) -> Vec<f32> {
    let b = boom(k, 0.6, 85.0, 45.0, 0.08, 0.4);
    let click = shaped(highpass(&k.white(0.6), k.rate, 3000.0), &decay(k.len(0.6), k.rate, 0.0, 0.01));
    let mut x = sum(&[(&b, 1.0), (&click, 0.3)]);
    drive(&mut x, 1.5);
    x
}

fn sub_damage(k: &mut Kit) -> Vec<f32> {
    let b = boom(k, 0.8, 70.0, 45.0, 0.12, 0.6);
    let t = thud(k, 0.8, 350.0, 0.12);
    let mut x = sum(&[(&b, 1.0), (&t, 0.6)]);
    drive(&mut x, 1.6);
    x
}

fn sub_creature_death(k: &mut Kit) -> Vec<f32> {
    let s = 1.8;
    let n = k.len(s);
    let mut x = k.tone(s, Wave::Sine, glide(110.0, 28.0, 1.4));
    let growl: Vec<f32> = (0..n).map(|i| 0.75 + 0.25 * (TAU * 28.0 * i as f32 / k.rate).sin()).collect();
    mul(&mut x, &growl);
    let rumble = lowpass(&k.brown(s), k.rate, 120.0);
    let mut x = sum(&[(&x, 1.0), (&normed(rumble), 0.3)]);
    mul(&mut x, &decay(n, k.rate, 0.01, 2.0));
    drive(&mut x, 1.8);
    x
}

fn sub_body_fall(k: &mut Kit) -> Vec<f32> {
    let b = boom(k, 0.7, 65.0, 40.0, 0.1, 0.45);
    let t = thud(k, 0.7, 220.0, 0.15);
    let mut x = sum(&[(&b, 1.0), (&t, 0.8)]);
    let bounce = x.clone();
    add(&mut x, &bounce, k.len(0.12), 0.35);
    drive(&mut x, 1.6);
    x
}

fn sub_explosion(k: &mut Kit) -> Vec<f32> {
    let s = 2.4;
    let n = k.len(s);
    let blast = shaped(filter(&k.white(s), k.rate, Mode::Low, glide(1200.0, 90.0, 1.2), 0.7), &decay(n, k.rate, 0.002, 2.2));
    let b = boom(k, s, 55.0, 30.0, 0.4, 2.0);
    let debris = shaped(lowpass(&k.crackle(s, 30.0), k.rate, 2000.0), &decay(n, k.rate, 0.05, 1.8));
    let mut x = sum(&[(&normed(blast), 1.0), (&b, 0.9), (&normed(debris), 0.3)]);
    drive(&mut x, 2.0);
    x
}

fn sub_electric(k: &mut Kit) -> Vec<f32> {
    let s = 1.5;
    let n = k.len(s);
    let crack = shaped(highpass(&k.white(s), k.rate, 1500.0), &decay(n, k.rate, 0.0, 0.04));
    let mut hum = k.tone(s, Wave::Sine, |_| 45.0);
    let flicker = wobble(k, s, 30.0, 0.3);
    mul(&mut hum, &flicker);
    mul(&mut hum, &decay(n, k.rate, 0.005, 1.3));
    let rumble = shaped(normed(lowpass(&k.brown(s), k.rate, 90.0)), &decay(n, k.rate, 0.05, 1.4));
    let mut x = sum(&[(&crack, 0.8), (&hum, 1.0), (&rumble, 0.5)]);
    drive(&mut x, 1.7);
    x
}

fn earthquake(k: &mut Kit) -> Vec<f32> {
    let s = 4.0;
    let n = k.len(s);
    let shape = env(n, k.rate, &[(0.0, 0.0), (0.4, 1.0), (3.0, 0.8), (4.0, 0.0)]);
    let mut ground = normed(lowpass(&k.brown(s), k.rate, 70.0));
    let w = wobble(k, s, 4.0, 0.4);
    mul(&mut ground, &w);
    mul(&mut ground, &shape);
    let mut low = k.tone(s, Wave::Sine, |_| 35.0);
    mul(&mut low, &shape);
    let mut x = sum(&[(&ground, 1.0), (&low, 0.4)]);
    for _ in 0..12 {
        let t = k.range(0.2, 3.4);
        let at = k.len(t);
        let rock = thud(k, 0.3, 250.0, 0.08);
        let g = k.range(0.2, 0.6);
        add(&mut x, &rock, at, g);
    }
    drive(&mut x, 1.6);
    x
}

fn roar_foundation(k: &mut Kit) -> Vec<f32> {
    let s = 1.6;
    let n = k.len(s);
    let w = wobble(k, s, 12.0, 0.0);
    let saw = k.tone(s, Wave::Saw, |t| 42.0 * (1.0 + 0.04 * w[((t * k.rate) as usize).min(w.len() - 1)]));
    let mut x = lowpass(&saw, k.rate, 220.0);
    let am: Vec<f32> = (0..n).map(|i| 0.7 + 0.3 * (TAU * 24.0 * i as f32 / k.rate).sin()).collect();
    mul(&mut x, &am);
    let sine = k.tone(s, Wave::Sine, |_| 42.0);
    let mut x = sum(&[(&normed(x), 1.0), (&sine, 0.6)]);
    mul(&mut x, &env(n, k.rate, &[(0.0, 0.0), (0.15, 1.0), (1.0, 0.8), (1.6, 0.0)]));
    drive(&mut x, 2.0);
    x
}

fn doom_riser(k: &mut Kit) -> Vec<f32> {
    let s = 3.9;
    let n = k.len(s);
    let a = k.tone(s, Wave::Saw, glide(38.0, 76.0, 2.8));
    let b = k.tone(s, Wave::Saw, glide(38.4, 77.0, 2.8));
    let saws = filter(&sum(&[(&a, 1.0), (&b, 1.0)]), k.rate, Mode::Low, glide(150.0, 900.0, 2.8), 1.2);
    let air = filter(&k.white(s), k.rate, Mode::Band, glide(300.0, 3000.0, 2.8), 1.5);
    let mut rise = sum(&[(&normed(saws), 1.0), (&normed(air), 0.3)]);
    mul(&mut rise, &env(n, k.rate, &[(0.0, 0.0), (1.5, 0.25), (2.8, 1.0), (2.85, 0.0)]));
    let b = boom(k, 1.1, 60.0, 32.0, 0.2, 1.0);
    add(&mut rise, &b, k.len(2.8), 1.0);
    drive(&mut rise, 1.6);
    rise
}

fn implosion(k: &mut Kit) -> Vec<f32> {
    let s = 1.5;
    let n = k.len(s);
    let b = boom(k, s, 120.0, 30.0, 1.2, 1.4);
    let rush = shaped(filter(&k.white(s), k.rate, Mode::Low, glide(3000.0, 200.0, 1.2), 0.9), &decay(n, k.rate, 0.002, 1.4));
    let mut x = sum(&[(&b, 1.0), (&normed(rush), 0.6)]);
    x.reverse();
    let tail = thud(k, 0.3, 150.0, 0.12);
    let end = x.len();
    add(&mut x, &tail, end - k.len(0.01), 0.8);
    drive(&mut x, 1.5);
    x
}

fn elemental(k: &mut Kit) -> Vec<f32> {
    let s = 1.6;
    let n = k.len(s);
    let swirl = filter(&k.white(s), k.rate, Mode::Band, |t| 700.0 + 500.0 * (TAU * 0.8 * t).sin(), 2.0);
    let a = k.tone(s, Wave::Sine, |t| 220.0 * (1.0 + 0.01 * (TAU * 3.0 * t).sin()));
    let b = k.tone(s, Wave::Sine, |_| 331.0);
    let c = k.tone(s, Wave::Triangle, |_| 445.0);
    let burst = shaped(bandpass(&k.white(s), k.rate, 1200.0, 0.8), &env(n, k.rate, &[(0.0, 0.0), (0.45, 0.0), (0.5, 1.0), (0.9, 0.0)]));
    let mut x = sum(&[(&normed(swirl), 0.8), (&a, 0.4), (&b, 0.3), (&c, 0.2), (&normed(burst), 0.6)]);
    mul(&mut x, &env(n, k.rate, &[(0.0, 0.0), (0.3, 1.0), (1.2, 0.7), (1.6, 0.0)]));
    reverb(&x, k.rate, 0.8, 0.25)
}

// Magic: twelve families, each a texture played three ways: struck (the
// impact), swelling (the cast) and high-passed and short (the accent).

#[derive(Clone, Copy)]
enum El {
    Arcane,
    Fire,
    Cold,
    Water,
    Acid,
    Gas,
    Electric,
    Mind,
    Poison,
    Paralysis,
    Holy,
    Necro,
}

fn sines(k: &Kit, s: f32, freqs: &[f32]) -> Vec<f32> {
    let mut out = Vec::new();
    for &f in freqs {
        add(&mut out, &k.tone(s, Wave::Sine, |_| f), 0, 1.0);
    }
    out
}

/// What a family sounds like, `s` seconds of it, peaking at 1.
fn texture(k: &mut Kit, el: El, s: f32) -> Vec<f32> {
    let n = k.len(s);
    let rate = k.rate;
    let x = match el {
        El::Arcane => {
            let mut out = Vec::new();
            for _ in 0..6 {
                let (f, r, ph) = (k.range(700.0, 2800.0), k.range(5.0, 11.0), k.range(0.0, TAU));
                let mut t = k.tone(s, Wave::Sine, |_| f);
                let trem: Vec<f32> = (0..n).map(|i| 0.5 + 0.5 * (TAU * r * i as f32 / rate + ph).sin()).collect();
                mul(&mut t, &trem);
                add(&mut out, &t, 0, 1.0);
            }
            let air = bandpass(&k.white(s), rate, 2000.0, 3.0);
            sum(&[(&normed(out), 1.0), (&normed(air), 0.3)])
        }
        El::Fire => {
            let mut roar = lowpass(&k.white(s), rate, 1800.0);
            let w = wobble(k, s, 8.0, 0.3);
            mul(&mut roar, &w);
            let crack = bandpass(&k.crackle(s, 60.0), rate, 2500.0, 0.7);
            let low = lowpass(&k.brown(s), rate, 250.0);
            sum(&[(&normed(roar), 1.0), (&normed(crack), 0.8), (&normed(low), 0.6)])
        }
        El::Cold => {
            let p = k.pings(s, (s * 40.0) as usize, 2500.0, 9000.0, 0.12, 0.9);
            let air = highpass(&k.white(s), rate, 6000.0);
            sum(&[(&normed(p), 1.0), (&normed(air), 0.1)])
        }
        El::Water => {
            let b = k.bubbles(s, (s * 30.0) as usize, 300.0, 1400.0, 0.9);
            let swirl = filter(&k.white(s), rate, Mode::Band, |t| 1000.0 + 700.0 * (TAU * 3.0 * t).sin(), 1.5);
            sum(&[(&normed(b), 1.0), (&normed(swirl), 0.4)])
        }
        El::Acid => {
            let fizz = highpass(&k.crackle(s, 400.0), rate, 2500.0);
            let b = k.bubbles(s, (s * 50.0) as usize, 1200.0, 3500.0, 0.95);
            let hiss = highpass(&k.white(s), rate, 5000.0);
            sum(&[(&normed(fizz), 1.0), (&normed(b), 0.5), (&normed(hiss), 0.1)])
        }
        El::Gas => {
            let mut v = filter(&k.pink(s), rate, Mode::Band, glide(1200.0, 3200.0, s), 0.8);
            let w = wobble(k, s, 3.0, 0.5);
            mul(&mut v, &w);
            v
        }
        El::Electric => {
            let w = wobble(k, s, 37.0, 0.0);
            let buzz = bandpass(&k.tone(s, Wave::Saw, |t| 110.0 * (1.0 + 0.05 * w[((t * rate) as usize).min(w.len() - 1)])), rate, 1800.0, 1.0);
            let sp = highpass(&sparks(k, s, 0.3), rate, 1200.0);
            sum(&[(&normed(buzz), 0.6), (&normed(sp), 1.0)])
        }
        El::Mind => {
            let mut v = sines(k, s, &[440.0, 443.5, 660.0, 666.0]);
            let ring: Vec<f32> = (0..n).map(|i| (TAU * 7.0 * i as f32 / rate).sin()).collect();
            let rung = shaped(v.clone(), &ring);
            add(&mut v, &rung, 0, 0.8);
            v
        }
        El::Poison => {
            let a = k.tone(s, Wave::Saw, |_| 180.0);
            let b = k.tone(s, Wave::Saw, |_| 186.0);
            let sick = filter(&sum(&[(&a, 1.0), (&b, 1.0)]), rate, Mode::Band, |t| 600.0 + 400.0 * (TAU * 2.5 * t).sin(), 4.0);
            let bub = k.bubbles(s, (s * 15.0) as usize, 150.0, 500.0, 0.9);
            sum(&[(&normed(sick), 1.0), (&normed(bub), 0.6)])
        }
        El::Paralysis => {
            let trem: Vec<f32> = (0..n).map(|i| 0.5 + 0.5 * (TAU * 14.0 * i as f32 / rate).sin()).collect();
            let whine = k.tone(s, Wave::Sine, |t| 520.0 * (1.0 + 0.02 * (TAU * 5.0 * t).sin()));
            let sq = lowpass(&k.tone(s, Wave::Square, |_| 260.0), rate, 900.0);
            shaped(sum(&[(&whine, 1.0), (&sq, 0.3)]), &trem)
        }
        El::Holy => {
            let mut out = Vec::new();
            for (i, f) in [1046.5f32, 1318.5, 1568.0].iter().enumerate() {
                let b = k.bell(s, *f, 2.0);
                add(&mut out, &b, k.len(0.06 * i as f32), 1.0);
            }
            out.truncate(n);
            let mut choir = Vec::new();
            for f in [523.25f32, 659.25, 784.0] {
                add(&mut choir, &k.tone(s, Wave::Saw, |t| f * (1.0 + 0.003 * (TAU * 5.0 * t).sin())), 0, 1.0);
            }
            let choir = shaped(lowpass(&choir, rate, 1800.0), &env(n, rate, &[(0.0, 0.0), (0.3, 1.0)]));
            sum(&[(&normed(out), 1.0), (&normed(choir), 0.35)])
        }
        El::Necro => {
            let mut v = Vec::new();
            for f in [98.0f32, 98.8, 147.0] {
                add(&mut v, &k.tone(s, Wave::Saw, glide(f, f * 0.85, s)), 0, 1.0);
            }
            let ooh = sum(&[(&bandpass(&v, rate, 450.0, 3.0), 1.0), (&bandpass(&v, rate, 900.0, 4.0), 0.6)]);
            let mut whisper = bandpass(&k.white(s), rate, 2500.0, 2.0);
            let w = wobble(k, s, 9.0, 0.0);
            mul(&mut whisper, &w);
            sum(&[(&normed(ooh), 1.0), (&normed(whisper), 0.3)])
        }
    };
    normed(x)
}

fn impact(k: &mut Kit, el: El) -> Vec<f32> {
    let s = 1.6;
    let n = k.len(s);
    let tex = shaped(texture(k, el, s), &decay(n, k.rate, 0.005, 1.2));
    let punch = boom(k, 0.4, 90.0, 45.0, 0.15, 0.3);
    let hit = shaped(highpass(&k.white(0.1), k.rate, 1000.0), &decay(k.len(0.1), k.rate, 0.0, 0.03));
    let gentle = matches!(el, El::Holy | El::Mind | El::Paralysis);
    let x = sum(&[(&tex, 1.0), (&punch, if gentle { 0.2 } else { 0.5 }), (&hit, if gentle { 0.0 } else { 0.4 })]);
    reverb(&x, k.rate, 0.8, 0.25)
}

fn cast(k: &mut Kit, el: El) -> Vec<f32> {
    let s = 1.0;
    let n = k.len(s);
    let swell = env(n, k.rate, &[(0.0, 0.0), (0.75, 1.0), (0.95, 0.6), (1.0, 0.0)]);
    let tex = shaped(texture(k, el, s), &swell);
    let tone = shaped(k.tone(s, Wave::Sine, glide(200.0, 800.0, 0.95)), &swell);
    let air = shaped(filter(&k.white(s), k.rate, Mode::Band, glide(500.0, 4000.0, 0.95), 1.5), &swell);
    let x = sum(&[(&tex, 1.0), (&tone, 0.15), (&normed(air), 0.2)]);
    reverb(&x, k.rate, 0.6, 0.3)
}

/// The high detail over an impact or cast: 2 to 8 kHz only, so it adds
/// air without muddying what it's laid over.
fn accent(k: &mut Kit, el: El) -> Vec<f32> {
    let s = 0.6;
    let n = k.len(s);
    let tex = match el {
        // Their own textures live low; up here they're a ring and a buzz.
        El::Mind => sines(k, 1.0, &[2500.0, 2531.0, 3750.0]),
        El::Poison => {
            let am: Vec<f32> = (0..n).map(|i| 0.6 + 0.4 * (TAU * 31.0 * i as f32 / k.rate).sin()).collect();
            shaped(bandpass(&k.tone(s, Wave::Saw, |_| 220.0), k.rate, 3000.0, 2.0), &am)
        }
        _ => texture(k, el, s),
    };
    let t60 = if matches!(el, El::Mind) { 0.9 } else { 0.5 };
    let n = tex.len();
    // Twice over: 24 dB an octave below 2 kHz.
    let high = highpass(&highpass(&tex, k.rate, 2000.0), k.rate, 2000.0);
    shaped(high, &decay(n, k.rate, 0.002, t60))
}

fn portal(k: &mut Kit) -> Vec<f32> {
    let s = 1.5;
    let n = k.len(s);
    let sweep = filter(&k.white(s), k.rate, Mode::Band, glide(300.0, 6000.0, 1.2), 1.2);
    let rise = k.tone(s, Wave::Sine, glide(200.0, 1600.0, 1.2));
    let shimmer = texture(k, El::Arcane, s);
    let mut x = sum(&[(&normed(sweep), 1.0), (&rise, 0.3), (&shimmer, 0.4)]);
    mul(&mut x, &env(n, k.rate, &[(0.0, 0.0), (1.0, 1.0), (1.5, 0.0)]));
    reverb(&x, k.rate, 1.0, 0.3)
}

macro_rules! family {
    ($($f:ident = $how:ident($el:ident);)*) => {
        $(fn $f(k: &mut Kit) -> Vec<f32> { $how(k, El::$el) })*
    };
}

family! {
    impact_arcane = impact(Arcane); impact_fire = impact(Fire); impact_cold = impact(Cold); impact_water = impact(Water);
    impact_acid = impact(Acid); impact_gas = impact(Gas); impact_electric = impact(Electric); impact_mind = impact(Mind);
    impact_poison = impact(Poison); impact_paralysis = impact(Paralysis); impact_holy = impact(Holy); impact_necro = impact(Necro);
    cast_arcane = cast(Arcane); cast_fire = cast(Fire); cast_cold = cast(Cold); cast_water = cast(Water);
    cast_acid = cast(Acid); cast_gas = cast(Gas); cast_electric = cast(Electric); cast_mind = cast(Mind);
    cast_poison = cast(Poison); cast_paralysis = cast(Paralysis); cast_holy = cast(Holy); cast_necro = cast(Necro);
    accent_arcane = accent(Arcane); accent_fire = accent(Fire); accent_cold = accent(Cold); accent_water = accent(Water);
    accent_acid = accent(Acid); accent_gas = accent(Gas); accent_electric = accent(Electric); accent_mind = accent(Mind);
    accent_poison = accent(Poison); accent_holy = accent(Holy); accent_necro = accent(Necro);
}

// The interface: short, clean and quiet in character, so they mark a
// moment without drowning the text.

/// Bells struck one after another: (Hz, seconds in).
fn chimes(k: &Kit, s: f32, notes: &[(f32, f32)], t60: f32) -> Vec<f32> {
    let mut out = vec![0.0; k.len(s)];
    for &(f, at) in notes {
        add(&mut out, &k.bell(t60, f, t60), k.len(at), 1.0);
    }
    out
}

fn blip(k: &Kit, s: f32, f: f32, t60: f32) -> Vec<f32> {
    let x = sum(&[(&k.tone(s, Wave::Sine, |_| f), 1.0), (&k.tone(s, Wave::Sine, |_| f * 2.0), 0.2)]);
    let n = x.len();
    shaped(x, &decay(n, k.rate, 0.002, t60))
}

fn ui_accept(k: &mut Kit) -> Vec<f32> {
    let click = shaped(highpass(&k.white(0.15), k.rate, 2000.0), &decay(k.len(0.15), k.rate, 0.0, 0.004));
    sum(&[(&click, 0.5), (&blip(k, 0.15, 1760.0, 0.08), 1.0)])
}

fn ui_reject(k: &mut Kit) -> Vec<f32> {
    let s = 0.22;
    let buzz = lowpass(&k.tone(s, Wave::Square, |_| 98.0), k.rate, 900.0);
    let n = buzz.len();
    shaped(buzz, &env(n, k.rate, &[(0.0, 0.0), (0.005, 1.0), (0.18, 1.0), (0.22, 0.0)]))
}

fn ui_level_up(k: &mut Kit) -> Vec<f32> {
    let x = chimes(k, 1.6, &[(523.25, 0.0), (659.25, 0.09), (784.0, 0.18), (1046.5, 0.27)], 1.2);
    let sparkle = shaped(k.pings(1.6, 12, 3000.0, 8000.0, 0.15, 0.4), &env(k.len(1.6), k.rate, &[(0.0, 0.0), (0.27, 0.0), (0.3, 1.0)]));
    reverb(&sum(&[(&x, 1.0), (&sparkle, 0.15)]), k.rate, 0.8, 0.25)
}

fn ui_quest_update(k: &mut Kit) -> Vec<f32> {
    reverb(&chimes(k, 1.0, &[(784.0, 0.0), (1174.7, 0.12)], 0.8), k.rate, 0.6, 0.2)
}

fn ui_quest_complete(k: &mut Kit) -> Vec<f32> {
    let s = 1.6;
    let chord = |k: &Kit, len: f32| {
        let mut out = Vec::new();
        for f in [261.6f32, 329.6, 392.0, 523.25] {
            add(&mut out, &k.tone(len, Wave::Saw, |t| f * (1.0 + 0.002 * (TAU * 5.5 * t).sin())), 0, 1.0);
        }
        out
    };
    let short = chord(k, 0.14);
    let short = shaped(lowpass(&short, k.rate, 2500.0), &env(k.len(0.14), k.rate, &[(0.0, 0.0), (0.01, 1.0), (0.11, 0.8), (0.14, 0.0)]));
    let long = chord(k, 1.1);
    let long = filter(&long, k.rate, Mode::Low, |t| if t < 0.08 { 400.0 + 2600.0 * t / 0.08 } else { 3000.0 - 1800.0 * ((t - 0.08) / 1.0).min(1.0) }, 0.9);
    let long = shaped(long, &env(k.len(1.1), k.rate, &[(0.0, 0.0), (0.02, 1.0), (0.8, 0.7), (1.1, 0.0)]));
    let mut x = vec![0.0; k.len(s)];
    add(&mut x, &short, 0, 1.0);
    add(&mut x, &long, k.len(0.18), 1.0);
    reverb(&x, k.rate, 1.0, 0.25)
}

fn ui_channel(k: &mut Kit) -> Vec<f32> {
    blip(k, 0.2, 880.0, 0.12)
}

fn ui_tell(k: &mut Kit) -> Vec<f32> {
    let mut x = blip(k, 0.1, 1318.5, 0.07);
    add(&mut x, &blip(k, 0.15, 1760.0, 0.09), k.len(0.09), 1.0);
    x
}

fn ui_login(k: &mut Kit) -> Vec<f32> {
    reverb(&chimes(k, 1.2, &[(659.25, 0.0), (880.0, 0.08), (1318.5, 0.16)], 0.9), k.rate, 0.6, 0.2)
}

fn ui_say(k: &mut Kit) -> Vec<f32> {
    lowpass(&blip(k, 0.12, 440.0, 0.06), k.rate, 1500.0)
}

fn ui_defeat(k: &mut Kit) -> Vec<f32> {
    let bells = chimes(k, 2.4, &[(440.0, 0.0), (349.2, 0.3), (261.6, 0.6)], 1.6);
    let low = boom(k, 2.4, 110.0, 110.0, 1.0, 2.0);
    reverb(&sum(&[(&bells, 1.0), (&low, 0.4)]), k.rate, 1.2, 0.3)
}

fn sting_threat(k: &mut Kit) -> Vec<f32> {
    let s = 1.0;
    let n = k.len(s);
    let a = k.tone(s, Wave::Saw, glide(73.4, 66.0, 0.6));
    let b = k.tone(s, Wave::Saw, glide(77.8, 70.0, 0.6));
    let stab = shaped(lowpass(&sum(&[(&a, 1.0), (&b, 1.0)]), k.rate, 1200.0), &decay(n, k.rate, 0.005, 0.9));
    let b = boom(k, s, 70.0, 40.0, 0.2, 0.6);
    reverb(&sum(&[(&normed(stab), 1.0), (&b, 0.6)]), k.rate, 0.7, 0.2)
}

fn whistle(k: &mut Kit) -> Vec<f32> {
    let s = 0.8;
    let n = k.len(s);
    let f = |t: f32| {
        let base = if t < 0.25 {
            1300.0 * (2400.0f32 / 1300.0).powf(t / 0.25)
        } else if t < 0.3 {
            2400.0
        } else {
            2400.0 * (1400.0f32 / 2400.0).powf(((t - 0.3) / 0.4).min(1.0))
        };
        base * (1.0 + 0.01 * (TAU * 6.0 * t).sin())
    };
    let tone = k.tone(s, Wave::Sine, f);
    let breath = highpass(&k.white(s), k.rate, 3000.0);
    shaped(sum(&[(&tone, 1.0), (&breath, 0.05)]), &env(n, k.rate, &[(0.0, 0.0), (0.03, 1.0), (0.24, 0.9), (0.27, 0.2), (0.32, 0.9), (0.7, 0.6), (0.8, 0.0)]))
}

fn bleep(k: &mut Kit) -> Vec<f32> {
    let s = 0.5;
    let n = k.len(s);
    shaped(k.tone(s, Wave::Sine, |_| 1000.0), &env(n, k.rate, &[(0.0, 0.0), (0.01, 1.0), (0.45, 1.0), (0.46, 0.0)]))
}

fn scry(k: &mut Kit) -> Vec<f32> {
    let s = 1.2;
    let n = k.len(s);
    let mist = shaped(texture(k, El::Arcane, s), &env(n, k.rate, &[(0.0, 0.0), (0.4, 1.0), (1.2, 0.0)]));
    let mut x = scale_copy(&mist, 0.6);
    add(&mut x, &k.bell(1.2, 2093.0, 1.2), k.len(0.1), 1.0);
    reverb(&x, k.rate, 0.9, 0.4)
}

fn scale_copy(x: &[f32], g: f32) -> Vec<f32> {
    let mut y = x.to_vec();
    scale(&mut y, g);
    y
}

macro_rules! recipes {
    ($($id:literal $name:literal $f:ident,)*) => {
        pub static RECIPES: &[Recipe] = &[$(Recipe { id: $id, name: $name, make: $f },)*];
    };
}

recipes! {
    "sub-impact" "Heavy impact, sub-bass" sub_impact,
    "sub-critical" "Critical thump, sub-bass" sub_critical,
    "sub-damage" "Heavy damage thud, sub-bass" sub_damage,
    "sub-creature-death" "Creature death drop, sub-bass" sub_creature_death,
    "sub-body-fall" "Body fall, sub-bass" sub_body_fall,
    "sub-explosion" "Explosion, sub-bass" sub_explosion,
    "sub-electric" "Electric crack and rumble, sub-bass" sub_electric,
    "earthquake" "Earthquake rumble" earthquake,
    "roar-foundation" "Roar foundation" roar_foundation,
    "doom-riser" "Doom riser" doom_riser,
    "implosion" "Implosion" implosion,
    "elemental" "Elemental hum and burst" elemental,
    "impact-arcane" "Arcane impact" impact_arcane,
    "impact-fire" "Fire impact" impact_fire,
    "impact-cold" "Cold impact" impact_cold,
    "impact-water" "Water impact" impact_water,
    "impact-acid" "Acid impact" impact_acid,
    "impact-gas" "Gas impact" impact_gas,
    "impact-electric" "Electric impact" impact_electric,
    "impact-mind" "Mind impact" impact_mind,
    "impact-poison" "Poison impact" impact_poison,
    "impact-paralysis" "Paralysis impact" impact_paralysis,
    "impact-holy" "Holy impact" impact_holy,
    "impact-necro" "Necromantic impact" impact_necro,
    "cast-arcane" "Arcane cast" cast_arcane,
    "cast-fire" "Fire cast" cast_fire,
    "cast-cold" "Cold cast" cast_cold,
    "cast-water" "Water cast" cast_water,
    "cast-acid" "Acid cast" cast_acid,
    "cast-gas" "Gas cast" cast_gas,
    "cast-electric" "Electric cast" cast_electric,
    "cast-mind" "Mind cast" cast_mind,
    "cast-poison" "Poison cast" cast_poison,
    "cast-paralysis" "Paralysis cast" cast_paralysis,
    "cast-holy" "Holy cast" cast_holy,
    "cast-necro" "Necromantic cast" cast_necro,
    "accent-arcane" "Arcane shimmer" accent_arcane,
    "accent-fire" "Flame crackle" accent_fire,
    "accent-cold" "Frost sparkle" accent_cold,
    "accent-water" "Droplets" accent_water,
    "accent-acid" "Acid fizz" accent_acid,
    "accent-gas" "Gas hiss" accent_gas,
    "accent-electric" "Voltage crackle" accent_electric,
    "accent-mind" "Eerie ring" accent_mind,
    "accent-poison" "Venom buzz" accent_poison,
    "accent-holy" "Bell air" accent_holy,
    "accent-necro" "Whispering air" accent_necro,
    "portal" "Portal whoosh" portal,
    "ui-accept" "Command accepted" ui_accept,
    "ui-reject" "Command rejected" ui_reject,
    "ui-level-up" "Level up" ui_level_up,
    "ui-quest-update" "Quest update" ui_quest_update,
    "ui-quest-complete" "Quest complete" ui_quest_complete,
    "ui-channel" "Channel message" ui_channel,
    "ui-tell" "Tell" ui_tell,
    "ui-login" "Login" ui_login,
    "ui-say" "Say" ui_say,
    "ui-defeat" "Defeat" ui_defeat,
    "sting-threat" "Threat sting" sting_threat,
    "whistle" "Whistle" whistle,
    "bleep" "Bleep" bleep,
    "scry" "Scrying shimmer" scry,
}
