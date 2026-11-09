// P3.17 — Provider-owned solver configuration (report §7.4, §12.3, §12.6;
// consumed by P3.19).
//
// Solver options pass through this provider-owned struct only — never as raw
// solver-specific options in domain or application code (P3.17 risk
// mitigation: "leaky abstraction"). The struct is solver-neutral; the
// provider/worker adapter translates it into the pinned solver's option set
// at build time (VERIFY-BUILD markers in `backend.rs`).
//
// Determinism: a fixed thread count is part of the configuration; the
// recorded default is 1 and is asserted in tests (P3.19 step 1). The
// configuration is digest-able so every result can record exactly which
// configuration produced it.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Scaling policy for the solve. The provider default is the pinned solver's
/// automatic scaling; disabling it is an explicit, recorded choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScalingPolicy {
    /// The provider's default scaling approach (the pinned solver's
    /// automatic scaling).
    #[default]
    ProviderDefault,
    /// Scaling explicitly disabled (an auditable choice, recorded in the
    /// config digest and therefore in every result).
    Disabled,
}

/// Invalid solver configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// Tolerances and gaps must be finite and non-negative.
    NonFiniteValue { field: &'static str },
    /// A tolerance must be strictly positive.
    NonPositiveTolerance { field: &'static str },
    /// The MIP relative gap must be in [0, 1).
    MipGapOutOfRange { value: f64 },
    /// The thread count must be at least 1 (fixed for determinism).
    ThreadCountBelowOne,
    /// A configured time limit must be non-zero.
    ZeroTimeLimit,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::NonFiniteValue { field } => {
                write!(f, "config field '{field}' must be finite")
            }
            ConfigError::NonPositiveTolerance { field } => {
                write!(f, "config field '{field}' must be > 0")
            }
            ConfigError::MipGapOutOfRange { value } => {
                write!(f, "mip_relative_gap {value} outside [0, 1)")
            }
            ConfigError::ThreadCountBelowOne => {
                write!(f, "thread_count must be >= 1 (fixed for determinism)")
            }
            ConfigError::ZeroTimeLimit => write!(f, "time_limit must be non-zero when set"),
        }
    }
}

impl std::error::Error for ConfigError {}

// P3.19 — Solver configuration, tolerances, determinism, and failure handling
// (ledger P3.19 step 1; report §12.6, §19.2 layer 5). ADDITIVE extension of
// the P3.17 configuration surface: every pre-existing type, field, default,
// and digest value is unchanged.

/// The recorded determinism thread count (P3.17 decision note §5): ONE thread
/// unless a run explicitly pins another count. The value is part of the
/// configuration, flows into every provider's solver options, and is recorded
/// on every §12.6 result (`DeterminismRecord::thread_count` — the recording
/// the ledger P3.19 step 1 requires; asserted per provider in the tests).
pub const DETERMINISM_THREAD_COUNT_DEFAULT: u32 = 1;

/// The recording requirement, stated once so providers and reviewers share
/// it (ledger P3.19 step 1: "record the value used").
pub const THREAD_COUNT_RECORDING_NOTE: &str = "the fixed thread count used for \
     a solve is recorded on every §12.6 result (DeterminismRecord::thread_count); \
     the recorded default is 1 (DETERMINISM_THREAD_COUNT_DEFAULT)";

/// Which configuration layer a contract entry describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderTier {
    /// The Tier 1 (LP / convex QP / MILP) path (report §12.3).
    Tier1,
    /// The Tier 2 (continuous SOCP) path (report §12.3).
    Tier2,
}

/// The per-provider configuration contract (P3.19 step 1, solver-neutral):
/// which [`SolverConfig`] options each provider tier consumes. A provider
/// must consume every option marked applicable and must RECORD (as a
/// per-result warning, never silence) every configured option marked
/// inapplicable — the Tier 2 path already records an ignored MIP gap this
/// way. This is documentation made checkable: the tables are unit-tested
/// against the recorded provider behaviour.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigContract {
    /// The tier this contract describes.
    pub tier: ProviderTier,
    /// Whether the provider consumes [`SolverConfig::time_limit`].
    pub consumes_time_limit: bool,
    /// Whether the provider consumes [`SolverConfig::mip_relative_gap`]
    /// (`false` = a configured gap is recorded as ignored, never applied).
    pub consumes_mip_relative_gap: bool,
    /// Whether the provider consumes the feasibility tolerance
    /// ([`SolverConfig::primal_feasibility_tolerance`]).
    pub consumes_feasibility_tolerance: bool,
    /// Whether the provider consumes the optimality tolerance
    /// ([`SolverConfig::dual_feasibility_tolerance`]; the Tier 2 conic solver
    /// carries it on its gap tolerances — recorded in
    /// `ledger/phase-03-flexibility-scheduling/18_clarabel_tier2.md` §5).
    pub consumes_optimality_tolerance: bool,
    /// Whether the provider consumes [`SolverConfig::scaling`].
    pub consumes_scaling: bool,
    /// Whether the thread count is fixed for determinism (always `true` —
    /// the ledger risk mitigation requires it).
    pub thread_count_fixed_for_determinism: bool,
    /// Recorded notes (translation specifics, VERIFY-BUILD pointers).
    pub notes: Vec<&'static str>,
}

/// The per-tier configuration contract (P3.19 step 1). Both tiers consume
/// the time limit, tolerances, scaling, and the fixed thread count; only
/// Tier 1 (a MIP-capable class path) consumes the MIP relative gap.
pub fn config_contract(tier: ProviderTier) -> ConfigContract {
    match tier {
        ProviderTier::Tier1 => ConfigContract {
            tier,
            consumes_time_limit: true,
            consumes_mip_relative_gap: true,
            consumes_feasibility_tolerance: true,
            consumes_optimality_tolerance: true,
            consumes_scaling: true,
            thread_count_fixed_for_determinism: true,
            notes: vec![
                "options translate 1:1 into BackendOptions and are mapped onto \
                 the pinned solver's option names by the worker adapter \
                 (VERIFY-BUILD item 5, decision note 17_optimizer_port.md §6)",
            ],
        },
        ProviderTier::Tier2 => ConfigContract {
            tier,
            consumes_time_limit: true,
            consumes_mip_relative_gap: false,
            consumes_feasibility_tolerance: true,
            consumes_optimality_tolerance: true,
            consumes_scaling: true,
            thread_count_fixed_for_determinism: true,
            notes: vec![
                "a continuous conic solve has no MIP gap: a configured \
                 mip_relative_gap is recorded as ignored (decision note \
                 18_clarabel_tier2.md §5)",
                "the optimality tolerance is carried by the solver's absolute/\
                 relative gap tolerances; there is no separate dual-feasibility \
                 knob (decision note 18_clarabel_tier2.md §5)",
            ],
        },
    }
}

/// Provider-owned solver configuration (P3.19 consumes this; P3.17 records
/// the deterministic defaults).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SolverConfig {
    /// Wall-clock time limit for the solve; `None` = no explicit limit.
    pub time_limit: Option<Duration>,
    /// Relative MIP optimality gap; `None` = the solver's default gap
    /// handling. Must be in [0, 1) when set.
    pub mip_relative_gap: Option<f64>,
    /// Primal feasibility tolerance. Default 1e-7 (the pinned solver's
    /// documented default — VERIFY-BUILD at adapter build time).
    pub primal_feasibility_tolerance: f64,
    /// Dual feasibility / optimality tolerance. Default 1e-7 (same
    /// VERIFY-BUILD marker).
    pub dual_feasibility_tolerance: f64,
    /// Scaling approach.
    pub scaling: ScalingPolicy,
    /// Fixed thread count for determinism; default 1 (recorded). Must be
    /// >= 1.
    pub thread_count: u32,
}

impl Default for SolverConfig {
    fn default() -> Self {
        Self {
            time_limit: None,
            mip_relative_gap: None,
            primal_feasibility_tolerance: 1e-7,
            dual_feasibility_tolerance: 1e-7,
            scaling: ScalingPolicy::ProviderDefault,
            // Determinism default recorded in the decision note §5 and
            // asserted in tests: one thread unless a run explicitly pins
            // another count.
            thread_count: DETERMINISM_THREAD_COUNT_DEFAULT,
        }
    }
}

impl SolverConfig {
    /// Validates the configuration. Providers must reject an invalid config
    /// before any solver interaction.
    pub fn validate(&self) -> Result<(), ConfigError> {
        for (field, value) in [
            ("primal_feasibility_tolerance", self.primal_feasibility_tolerance),
            ("dual_feasibility_tolerance", self.dual_feasibility_tolerance),
        ] {
            if !value.is_finite() {
                return Err(ConfigError::NonFiniteValue { field });
            }
            if value <= 0.0 {
                return Err(ConfigError::NonPositiveTolerance { field });
            }
        }
        if let Some(gap) = self.mip_relative_gap {
            if !gap.is_finite() {
                return Err(ConfigError::NonFiniteValue {
                    field: "mip_relative_gap",
                });
            }
            if !(0.0..1.0).contains(&gap) {
                return Err(ConfigError::MipGapOutOfRange { value: gap });
            }
        }
        if self.thread_count < 1 {
            return Err(ConfigError::ThreadCountBelowOne);
        }
        if let Some(limit) = self.time_limit {
            if limit.is_zero() {
                return Err(ConfigError::ZeroTimeLimit);
            }
        }
        Ok(())
    }

    /// A stable digest of the configuration (FNV-1a 64 over the canonical
    /// serde serialization). Recorded on every result so a replay (P3.19)
    /// can verify that the same configuration produced the same schedule.
    pub fn config_digest(&self) -> String {
        // serde_json serialization of a derived-Serialize struct is
        // field-order stable, so the digest is deterministic for identical
        // configurations.
        match serde_json::to_vec(self) {
            Ok(bytes) => format!("fnv1a64:{:016x}", fnv1a64(&bytes)),
            // Serialization of this struct cannot fail (no maps, no
            // non-string keys); the guard keeps the function total.
            Err(_) => "fnv1a64:serialization_unavailable".to_string(),
        }
    }

    /// The canonical JSON serialization of the configuration: the verifiable,
    /// human-readable config record (ledger P3.19: the recorded config must
    /// be verifiable, not a blob). The [`SolverConfig::config_digest`] is
    /// the FNV-1a 64 digest of exactly these bytes, so a recorded digest and
    /// a recorded canonical JSON cross-check each other. Field order is the
    /// struct's declaration order (derived serde serialization), so the
    /// serialization is stable for identical configurations.
    pub fn canonical_json(&self) -> String {
        // Serialization of this struct cannot fail (no maps, no non-string
        // keys); the fallback keeps the function total and is itself a
        // detectable sentinel.
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }

    /// `true` when `digest` is this configuration's digest. Replay
    /// verification helper (P3.19 step 4): a replay must prove the recorded
    /// configuration produced the recorded result.
    pub fn matches_digest(&self, digest: &str) -> bool {
        self.config_digest() == digest
    }

    /// The configured options that are NOT applicable to the given provider
    /// tier and must be recorded as ignored by the provider (P3.19 step 1,
    /// per-provider configuration completion; mirrors
    /// [`config_contract`]). Empty for a fully applicable configuration.
    pub fn inapplicable_options(&self, tier: ProviderTier) -> Vec<&'static str> {
        let contract = config_contract(tier);
        let mut inapplicable = Vec::new();
        if !contract.consumes_mip_relative_gap && self.mip_relative_gap.is_some() {
            inapplicable.push("mip_relative_gap");
        }
        inapplicable
    }
}

/// FNV-1a 64-bit (public-domain parameters). Crate-visible so the P3.19
/// determinism machinery digests its canonical serializations with the SAME
/// hash function the config digest uses (one digest convention, one place).
pub(crate) fn fnv1a64(bytes: &[u8]) -> u64 {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET_BASIS;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_pins_one_thread_and_documented_tolerances() {
        let config = SolverConfig::default();
        assert_eq!(config.thread_count, 1, "recorded determinism default");
        assert_eq!(config.scaling, ScalingPolicy::ProviderDefault);
        assert_eq!(config.primal_feasibility_tolerance, 1e-7);
        assert_eq!(config.dual_feasibility_tolerance, 1e-7);
        assert!(config.time_limit.is_none());
        assert!(config.mip_relative_gap.is_none());
        config.validate().expect("default config validates");
    }

    #[test]
    fn invalid_configs_are_rejected() {
        let mut config = SolverConfig::default();
        config.thread_count = 0;
        assert_eq!(
            config.validate(),
            Err(ConfigError::ThreadCountBelowOne)
        );

        let mut config = SolverConfig::default();
        config.primal_feasibility_tolerance = 0.0;
        assert_eq!(
            config.validate(),
            Err(ConfigError::NonPositiveTolerance {
                field: "primal_feasibility_tolerance"
            })
        );

        let mut config = SolverConfig::default();
        config.dual_feasibility_tolerance = f64::NAN;
        assert_eq!(
            config.validate(),
            Err(ConfigError::NonFiniteValue {
                field: "dual_feasibility_tolerance"
            })
        );

        let mut config = SolverConfig::default();
        config.mip_relative_gap = Some(1.0);
        assert_eq!(
            config.validate(),
            Err(ConfigError::MipGapOutOfRange { value: 1.0 })
        );

        let mut config = SolverConfig::default();
        config.mip_relative_gap = Some(-0.5);
        assert!(config.validate().is_err());

        let mut config = SolverConfig::default();
        config.time_limit = Some(Duration::ZERO);
        assert_eq!(config.validate(), Err(ConfigError::ZeroTimeLimit));
    }

    #[test]
    fn config_digest_is_stable_and_discriminating() {
        let a = SolverConfig::default();
        let a_again = SolverConfig::default();
        assert_eq!(a.config_digest(), a_again.config_digest());

        let mut b = SolverConfig::default();
        b.thread_count = 4;
        assert_ne!(a.config_digest(), b.config_digest());

        let mut c = SolverConfig::default();
        c.time_limit = Some(Duration::from_secs(30));
        assert_ne!(a.config_digest(), c.config_digest());

        let mut d = SolverConfig::default();
        d.scaling = ScalingPolicy::Disabled;
        assert_ne!(a.config_digest(), d.config_digest());
    }

    #[test]
    fn config_round_trips_through_json() {
        let config = SolverConfig::default();
        let text = serde_json::to_string(&config).expect("serializes");
        let back: SolverConfig = serde_json::from_str(&text).expect("deserializes");
        assert_eq!(back, config);
    }

    // --- P3.19 additive tests ------------------------------------------------

    #[test]
    fn determinism_thread_count_default_is_the_recorded_one() {
        assert_eq!(DETERMINISM_THREAD_COUNT_DEFAULT, 1);
        assert_eq!(SolverConfig::default().thread_count, 1);
    }

    #[test]
    fn canonical_json_is_stable_and_cross_checks_the_digest() {
        let config = SolverConfig::default();
        let json_a = config.canonical_json();
        let json_b = SolverConfig::default().canonical_json();
        assert_eq!(json_a, json_b, "canonical JSON is stable");
        // Every configured value is visible in the record (verifiable, not a
        // blob).
        assert!(json_a.contains("\"thread_count\":1"));
        assert!(json_a.contains("\"scaling\":\"provider_default\""));
        assert!(json_a.contains("\"primal_feasibility_tolerance\""));
        // The digest is computed over exactly the canonical bytes.
        assert!(config.matches_digest(&config.config_digest()));
        let mut other = SolverConfig::default();
        other.time_limit = Some(Duration::from_secs(60));
        assert!(!other.matches_digest(&config.config_digest()));
    }

    #[test]
    fn per_provider_config_contracts_match_the_recorded_behaviour() {
        let tier1 = config_contract(ProviderTier::Tier1);
        assert!(tier1.consumes_time_limit);
        assert!(tier1.consumes_mip_relative_gap, "Tier 1 covers MILP");
        assert!(tier1.consumes_feasibility_tolerance);
        assert!(tier1.consumes_optimality_tolerance);
        assert!(tier1.consumes_scaling);
        assert!(tier1.thread_count_fixed_for_determinism);

        let tier2 = config_contract(ProviderTier::Tier2);
        assert!(tier2.consumes_time_limit);
        assert!(
            !tier2.consumes_mip_relative_gap,
            "a continuous conic solve has no MIP gap"
        );
        assert!(tier2.consumes_feasibility_tolerance);
        assert!(tier2.consumes_optimality_tolerance);
        assert!(tier2.consumes_scaling);
        assert!(tier2.thread_count_fixed_for_determinism);
    }

    #[test]
    fn inapplicable_options_are_detected_for_recording() {
        let mut config = SolverConfig::default();
        assert!(config.inapplicable_options(ProviderTier::Tier2).is_empty());
        assert!(config.inapplicable_options(ProviderTier::Tier1).is_empty());
        config.mip_relative_gap = Some(0.01);
        assert_eq!(
            config.inapplicable_options(ProviderTier::Tier2),
            vec!["mip_relative_gap"],
            "a configured gap is not applicable to a continuous conic solve"
        );
        assert!(
            config.inapplicable_options(ProviderTier::Tier1).is_empty(),
            "Tier 1 (MILP-capable) consumes the gap"
        );
    }
}
