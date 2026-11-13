// Copyright 2026 ansiapps. Neumetik Visual: proprietary, see the LICENSE at the repository root.

//! SplitMix64 (Steele, Lea and Flood, 2014), the generator Stylus's rule
//! for seeded art names: the same seed makes the same art forever, so
//! nothing here may use another generator. And FNV-1a, for a seed from
//! words.

#[derive(Clone, Debug)]
pub struct SplitMix64(pub u64);

impl SplitMix64 {
    pub fn new(seed: u64) -> SplitMix64 {
        SplitMix64(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// 0 to n - 1 (0 when n is 0).
    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 {
            return 0;
        }
        ((u128::from(self.next_u64()) * u128::from(n)) >> 64) as u64
    }

    pub fn range(&mut self, lo: i32, hi: i32) -> i32 {
        lo + self.below((hi - lo).max(0) as u64 + 1) as i32
    }

    /// 0 to 1.
    pub fn unit(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    pub fn chance(&mut self, p: f32) -> bool {
        self.unit() < p
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len() as u64) as usize]
    }
}

/// FNV-1a over bytes, 64 bits.
pub fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xCBF2_9CE4_8422_2325u64, |h, &b| (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01B3))
}

/// Integer value noise in 2D, 0 to 255, smooth between lattice points
/// `cell` apart: what the cover painter's landscapes and clouds are made
/// of, in integers so it's the same everywhere.
pub fn value_noise(seed: u64, x: i32, y: i32, cell: i32) -> u32 {
    let cell = cell.max(1);
    let (gx, gy) = (x.div_euclid(cell), y.div_euclid(cell));
    let (fx, fy) = (x.rem_euclid(cell), y.rem_euclid(cell));
    let at = |ix: i32, iy: i32| {
        let h = fnv1a(&[seed.to_le_bytes().as_slice(), &ix.to_le_bytes(), &iy.to_le_bytes()].concat());
        (h >> 56) as i64
    };
    // Smoothstep weights in 1/256ths.
    let w = |f: i32| {
        let t = i64::from(f) * 256 / i64::from(cell);
        t * t * (768 - 2 * t) / 65_536
    };
    let (wx, wy) = (w(fx), w(fy));
    let top = at(gx, gy) * (256 - wx) + at(gx + 1, gy) * wx;
    let bottom = at(gx, gy + 1) * (256 - wx) + at(gx + 1, gy + 1) * wx;
    ((top * (256 - wy) + bottom * wy) / 65_536) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitmix_gives_its_published_first_values() {
        // The reference implementation's first output for seed 0.
        let mut r = SplitMix64::new(0);
        assert_eq!(r.next_u64(), 0xE220_A839_7B1D_CDAF);
        assert!((0..1000).all(|_| r.below(7) < 7));
    }

    #[test]
    fn value_noise_stays_in_range_and_repeats() {
        for y in -20..20 {
            for x in -20..20 {
                let v = value_noise(5, x, y, 8);
                assert!(v <= 255);
                assert_eq!(v, value_noise(5, x, y, 8));
            }
        }
    }
}
