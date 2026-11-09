// P2.14 — Deterministic seeded RNG for the synthetic telemetry simulator.
//
// Recorded algorithm and seed handling (P2.14 step 2; D-SC4): the scenario
// carries ONE master seed; every asset's stream is derived as
//
//     seed(asset_key) = master_seed XOR splitmix64(FNV-1a-64(asset_key))
//
// exactly as recorded in the scenario file's `seeds.derivation` string, so
// adding an asset in a future scenario version does not perturb the streams
// of existing assets. Both primitives are the standard public-domain
// reference implementations:
//
// - **FNV-1a-64**: offset basis `0xcbf29ce484222325`, prime
//   `0x100000001b3`, over the UTF-8 bytes of the asset key.
// - **splitmix64** (Steele et al., 2014): golden-ratio increment
//   `0x9E3779B97F4A7C15` on the state, then the finalizer
//   (`0xBF58476D1CE4E5B9`, `0x94D049BB133111EB`, final xor-shift 31). The
//   finalizer alone — applied to the hashed asset key — is the
//   `splitmix64(x)` of the derivation rule; the full step is the stream
//   generator.
//
// No wall-clock, no global state, no ambient entropy anywhere in this
// module: the same (master_seed, asset_key) pair always yields the same
// stream, which is what makes the simulator's runs reproducible (P2.13 §3
// determinism scope; P2.24 replay).
//
// std-only by choice: the simulator takes no external RNG dependency, so
// the recorded algorithm is the code below, verbatim, and a re-run with the
// same toolchain reproduces the stream bit-for-bit.

/// FNV-1a-64 over `bytes` (the standard offset basis and prime).
pub(crate) fn fnv1a64(bytes: &[u8]) -> u64 {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET_BASIS;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

/// The splitmix64 finalizer applied to an arbitrary value: the
/// `splitmix64(x)` of the recorded derivation rule.
pub(crate) fn splitmix64_mix(x: u64) -> u64 {
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Derives an asset's stream seed from the master seed and the asset key,
/// implementing the recorded rule
/// `seed(asset_key) = master_seed XOR splitmix64(FNV-1a-64(asset_key))`.
pub(crate) fn derive_asset_seed(master_seed: u64, asset_key: &str) -> u64 {
    master_seed ^ splitmix64_mix(fnv1a64(asset_key.as_bytes()))
}

/// A splitmix64 stream generator: golden-ratio increment on the state, then
/// the finalizer, per step.
pub(crate) struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    /// Creates a generator from the (already derived) seed.
    pub(crate) const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// The raw splitmix64 stream step.
    pub(crate) fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        splitmix64_mix(self.state)
    }

    /// A finite f64 uniformly spread over `[low, high)` by scaling a 53-bit
    /// draw; used for the configured per-tick jitter only.
    pub(crate) fn f64_in(&mut self, low: f64, high: f64) -> f64 {
        debug_assert!(low <= high && low.is_finite() && high.is_finite());
        let unit = (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64; // [0, 1)
        low + unit * (high - low)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_derivation_rule_is_deterministic_and_asset_separated() {
        let a1 = derive_asset_seed(20260601, "site-north-bess-1");
        let a2 = derive_asset_seed(20260601, "site-north-bess-1");
        let b = derive_asset_seed(20260601, "site-south-bess-1");
        let c = derive_asset_seed(20260602, "site-north-bess-1");
        assert_eq!(a1, a2, "same master seed + key must derive the same seed");
        assert_ne!(a1, b, "different asset keys must derive different seeds");
        assert_ne!(a1, c, "different master seeds must derive different seeds");
    }

    #[test]
    fn streams_are_reproducible_from_the_same_seed() {
        let mut a = SplitMix64::new(derive_asset_seed(20260601, "site-north-load-1"));
        let mut b = SplitMix64::new(derive_asset_seed(20260601, "site-north-load-1"));
        for _ in 0..256 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn f64_in_stays_in_range_and_finite() {
        let mut rng = SplitMix64::new(7);
        for _ in 0..1000 {
            let v = rng.f64_in(-0.02, 0.02);
            assert!(v.is_finite());
            assert!((-0.02..0.02).contains(&v));
        }
    }
}
