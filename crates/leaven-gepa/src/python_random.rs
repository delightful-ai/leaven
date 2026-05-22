use serde::{Deserialize, Serialize};

const N: usize = 624;
const M: usize = 397;
const MATRIX_A: u32 = 0x9908_b0df;
const UPPER_MASK: u32 = 0x8000_0000;
const LOWER_MASK: u32 = 0x7fff_ffff;

#[derive(Clone, Debug, Serialize, Deserialize)]
// `pub` despite being crate-internal: the module declaration `mod python_random;`
// is private (no `pub mod`), so external code cannot reach this name; the inner
// `pub` is needed because clippy's `redundant_pub_crate` lint refuses
// `pub(crate)` inside a private module.
pub struct PythonRandom {
    mt: Vec<u32>,
    index: usize,
}

impl Default for PythonRandom {
    fn default() -> Self {
        Self::seeded(0)
    }
}

impl PythonRandom {
    pub(super) fn seeded(seed: u64) -> Self {
        let key = seed_key(seed);
        let mut rng = Self {
            mt: vec![0; N],
            index: N,
        };
        rng.init_by_array(&key);
        rng
    }

    pub(super) fn randbelow(&mut self, upper: usize) -> usize {
        assert!(upper > 0, "upper bound must be nonzero");
        let bits = usize::BITS - upper.leading_zeros();
        loop {
            let value = self.getrandbits(bits);
            if value < upper as u64 {
                return usize::try_from(value).expect("bounded value fits usize");
            }
        }
    }

    pub(super) fn shuffle<T>(&mut self, values: &mut [T]) {
        for i in (1..values.len()).rev() {
            let j = self.randbelow(i + 1);
            values.swap(i, j);
        }
    }

    fn init_by_array(&mut self, key: &[u32]) {
        self.init_genrand(19_650_218);
        let mut i = 1usize;
        let mut j = 0usize;
        let mut k = N.max(key.len());
        while k > 0 {
            let previous = self.mt[i - 1] ^ (self.mt[i - 1] >> 30);
            self.mt[i] = (self.mt[i] ^ previous.wrapping_mul(1_664_525))
                .wrapping_add(key[j])
                .wrapping_add(u32::try_from(j).expect("seed key index fits u32"));
            i += 1;
            j += 1;
            if i >= N {
                self.mt[0] = self.mt[N - 1];
                i = 1;
            }
            if j >= key.len() {
                j = 0;
            }
            k -= 1;
        }
        k = N - 1;
        while k > 0 {
            let previous = self.mt[i - 1] ^ (self.mt[i - 1] >> 30);
            self.mt[i] = (self.mt[i] ^ previous.wrapping_mul(1_566_083_941))
                .wrapping_sub(u32::try_from(i).expect("MT index fits u32"));
            i += 1;
            if i >= N {
                self.mt[0] = self.mt[N - 1];
                i = 1;
            }
            k -= 1;
        }
        self.mt[0] = UPPER_MASK;
        self.index = N;
    }

    fn init_genrand(&mut self, seed: u32) {
        self.mt[0] = seed;
        for i in 1..N {
            self.mt[i] = 1_812_433_253u32
                .wrapping_mul(self.mt[i - 1] ^ (self.mt[i - 1] >> 30))
                .wrapping_add(u32::try_from(i).expect("MT index fits u32"));
        }
        self.index = N;
    }

    fn getrandbits(&mut self, bits: u32) -> u64 {
        if bits == 0 {
            return 0;
        }
        assert!(bits <= 32, "GEPA only needs small Python randbelow draws");
        u64::from(self.gen_u32() >> (32 - bits))
    }

    fn gen_u32(&mut self) -> u32 {
        if self.index >= N {
            self.twist();
        }
        let mut y = self.mt[self.index];
        self.index += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c_5680;
        y ^= (y << 15) & 0xefc6_0000;
        y ^ (y >> 18)
    }

    fn twist(&mut self) {
        for kk in 0..(N - M) {
            let y = (self.mt[kk] & UPPER_MASK) | (self.mt[kk + 1] & LOWER_MASK);
            self.mt[kk] = self.mt[kk + M] ^ (y >> 1) ^ mag01(y);
        }
        for kk in (N - M)..(N - 1) {
            let y = (self.mt[kk] & UPPER_MASK) | (self.mt[kk + 1] & LOWER_MASK);
            self.mt[kk] = self.mt[kk + M - N] ^ (y >> 1) ^ mag01(y);
        }
        let y = (self.mt[N - 1] & UPPER_MASK) | (self.mt[0] & LOWER_MASK);
        self.mt[N - 1] = self.mt[M - 1] ^ (y >> 1) ^ mag01(y);
        self.index = 0;
    }
}

fn seed_key(seed: u64) -> Vec<u32> {
    if seed == 0 {
        return vec![0];
    }
    let mut remaining = seed;
    let mut key = Vec::new();
    while remaining > 0 {
        key.push(
            u32::try_from(remaining & u64::from(u32::MAX)).expect("masked seed word fits u32"),
        );
        remaining >>= 32;
    }
    key
}

const fn mag01(y: u32) -> u32 {
    if y & 1 == 0 { 0 } else { MATRIX_A }
}

#[cfg(test)]
mod tests {
    use super::PythonRandom;

    #[test]
    fn randbelow_matches_python_random_for_seed_zero() {
        let mut rng = PythonRandom::seeded(0);
        let draws = (0..10).map(|_| rng.randbelow(3)).collect::<Vec<_>>();
        assert_eq!(draws, [1, 1, 0, 1, 2, 1, 1, 1, 1, 1]);
    }

    #[test]
    fn shuffle_matches_python_random_for_known_seeds() {
        let mut seed_zero = PythonRandom::seeded(0);
        let mut values = vec![0, 1, 2, 3, 4];
        seed_zero.shuffle(&mut values);
        assert_eq!(values, [2, 1, 0, 4, 3]);

        let mut seed_forty_one = PythonRandom::seeded(41);
        let mut values = vec![0, 1, 2, 3, 4];
        seed_forty_one.shuffle(&mut values);
        assert_eq!(values, [1, 4, 0, 2, 3]);
    }
}
