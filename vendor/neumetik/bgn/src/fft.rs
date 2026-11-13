// Copyright 2026 ansiapps. Neumetik BGN: proprietary, see the LICENSE at the repository root.

//! A radix-2 complex FFT, just enough for spectral noise.

use std::f32::consts::TAU;

pub struct Fft {
    n: usize,
    cos: Vec<f32>,
    sin: Vec<f32>,
    rev: Vec<usize>,
}

impl Fft {
    /// `n` must be a power of two.
    pub fn new(n: usize) -> Fft {
        assert!(n.is_power_of_two());
        let bits = n.trailing_zeros();
        let rev = (0..n).map(|i| if bits == 0 { 0 } else { i.reverse_bits() >> (usize::BITS - bits) }).collect();
        let cos = (0..n / 2).map(|k| (TAU * k as f32 / n as f32).cos()).collect();
        let sin = (0..n / 2).map(|k| (TAU * k as f32 / n as f32).sin()).collect();
        Fft { n, cos, sin, rev }
    }

    /// The inverse transform in place, unscaled.
    pub fn inverse(&mut self, re: &mut [f32], im: &mut [f32]) {
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
                    // e^{+i 2 pi k / len} for the inverse.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_bin_comes_back_as_a_sinusoid() {
        let n = 64;
        let mut f = Fft::new(n);
        let (mut re, mut im) = (vec![0.0; n], vec![0.0; n]);
        re[3] = 1.0;
        f.inverse(&mut re, &mut im);
        for i in 0..n {
            let w = TAU * 3.0 * i as f32 / n as f32;
            assert!((re[i] - w.cos()).abs() < 1e-4 && (im[i] - w.sin()).abs() < 1e-4);
        }
    }
}
