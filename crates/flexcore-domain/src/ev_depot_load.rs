// P17.11 — Depot / site load management (pure rules, no I/O).
//
// Implements ledger P17.11: site import/transformer limits, simultaneity-
// factor control across chargers, the P14.15 DOE-envelope interplay, and
// the external-constraint profile composition the P17.05 composer consumes
// (Agent A's — consumed here BY CITATION; this module depends on nothing
// infrastructure). Decision record:
// `docs/phases-11-20/phase-17-ev-fleet-charging/records/
// 26_p17_11_depot_load_management.md`.
//
// # The model, decided once [S-141][S-140]
//
// 1. **The site import/transformer limit is the binding envelope.** The
//    depot's site import (fleet charging plus every other site load,
//    carried as the declared non-EV load) must stay under the transformer
//    limit per interval — the domain fleet model already reports
//    `SiteTransformerLimitExceeded` for the fleet import alone; this module
//    composes the SITE-level cap (transformer vs DOE vs static connection
//    limit) the composer shapes per-charger limits against.
// 2. **The DOE interplay (P14.15).** When a dynamic operating envelope is
//    in effect at the site's connection point, its import bound is an
//    ADDITIONAL cap: the effective site import limit is the MINIMUM of the
//    transformer limit and the effective DOE import bound. When the
//    envelope has lapsed (not-yet-in-effect / expired / stale — the
//    `doe_envelope` lapse semantics, reused verbatim), the declared static
//    connection limit governs — a DOE without a static fallback never
//    silently becomes "unconstrained" (the P14.15 mandatory-fallback rule).
// 3. **Simultaneity-factor control.** The declared simultaneity factor
//    (the fraction of connected chargers assumed to charge at full rate
//    together [S-141][S-140], UNVERIFIED transcription) shapes the
//    per-charger limits: if `factor × Σ(connected ratings)` fits under the
//    effective site headroom, chargers keep their ratings; otherwise the
//    headroom is distributed PROPORTIONALLY to the connected chargers'
//    ratings (deterministic; a charger never receives more than its
//    rating, and a zero-rating charger receives zero). The factor is
//    declared site configuration in [0,1]; there is no default.
// 4. **External-constraint profile composition.** The per-interval
//    effective site import caps are composed into an ordered profile
//    (BTreeMap-backed deterministic order) — CONSTRAINT DATA the P17.05
//    composer maps onto the `ChargingStationExternalConstraints` charging
//    profile purpose (evseId 0; the purpose name is SECONDARY-SOURCED
//    [S-124], UNVERIFIED against the OCA text). This module produces the
//    profile; it never produces a `SetChargingProfile` payload — that is
//    the composer's job, downstream of the frozen lifecycle gates.
//
// Control status: pure domain rules for the simulation-only 0.x line. No
// depot site is named, no transformer limit was measured, no DOE was
// received from any operator.

use crate::doe_envelope::{
    evaluate_envelope, ConnectionPointId, DynamicOperatingEnvelope, StaticConnectionLimit,
};
use flexcore_contracts::time_model::EventTime;
use std::collections::BTreeMap;

/// The recorded depot load-management statement (decided once).
pub const DEPOT_LOAD_MANAGEMENT_STATEMENT: &str =
    "Depot load management composes the SITE-level import cap (the minimum \
of the site transformer limit and the effective DOE import bound — the \
static connection limit governing on DOE lapse), shapes per-charger \
limits under the declared simultaneity factor, and produces the \
external-constraint profile the P17.05 composer consumes. The composer \
maps the profile onto the ChargingStationExternalConstraints purpose \
[UNVERIFIED spelling, S-124] only through the frozen 14-state command \
lifecycle; nothing here produces a charging-profile payload.";

/// The declared rating of one charger in the depot (declared data with
/// provenance; the adapter's charger id spelling is recorded verbatim).
#[derive(Debug, Clone, PartialEq)]
pub struct ChargerRating {
    /// The charger id (non-empty; verbatim spelling, never parsed here).
    pub charger_id: String,
    /// The charger's rated power in MW (strictly positive).
    pub rated_power_mw: f64,
}

/// Errors of the depot load-management model. Every variant names the
/// offending field or charger (the P3.04 named-error style).
#[derive(Debug, Clone, PartialEq)]
pub enum DepotLoadError {
    /// The site transformer limit was non-finite or non-positive.
    InvalidTransformerLimit {
        /// The offending value in MW.
        value: f64,
    },
    /// The declared non-EV site load was negative or non-finite.
    InvalidNonEvLoadMw {
        /// The offending value in MW.
        value: f64,
    },
    /// The simultaneity factor was outside [0,1] (boundary-inclusive) or
    /// non-finite. There is no default factor — it is declared site
    /// configuration.
    SimultaneityFactorOutOfRange {
        /// The offending value.
        value: f64,
    },
    /// A charger rating was invalid (empty id, or a non-positive /
    /// non-finite rating).
    InvalidChargerRating {
        /// The offending charger id (as declared).
        charger_id: String,
    },
    /// A charger id appeared twice in the depot's declared charger list.
    DuplicateChargerId {
        /// The duplicated id.
        charger_id: String,
    },
    /// The declared non-EV load plus the minimum import floor left no
    /// feasible headroom (headroom would be negative — the composition is
    /// refused rather than producing negative charger limits).
    NoFeasibleHeadroom {
        /// The effective site import limit in MW.
        effective_limit_mw: f64,
        /// The declared non-EV load in MW.
        non_ev_load_mw: f64,
    },
    /// The DOE evaluation failed (the P14.15 error is wrapped with its
    /// human-safe text).
    DoeEvaluation(String),
    /// The declared interval length was zero or negative — the composition
    /// grid must be positive.
    NonPositiveIntervalMillis {
        /// The offending interval length in ms.
        value: i64,
    },
}

impl std::fmt::Display for DepotLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DepotLoadError::InvalidTransformerLimit { value } => {
                write!(f, "the site transformer limit must be finite and positive in MW (found {value})")
            }
            DepotLoadError::InvalidNonEvLoadMw { value } => {
                write!(f, "the declared non-EV site load must be finite and non-negative in MW (found {value})")
            }
            DepotLoadError::SimultaneityFactorOutOfRange { value } => {
                write!(f, "the simultaneity factor must be a fraction in [0,1] (found {value})")
            }
            DepotLoadError::InvalidChargerRating { charger_id } => {
                write!(f, "charger '{charger_id}': the id must be non-empty and the rating a finite positive MW value")
            }
            DepotLoadError::DuplicateChargerId { charger_id } => {
                write!(f, "charger id '{charger_id}' is declared twice in the depot's charger list")
            }
            DepotLoadError::NoFeasibleHeadroom { effective_limit_mw, non_ev_load_mw } => {
                write!(
                    f,
                    "no feasible headroom: the effective site import limit {effective_limit_mw} MW \
                     is at or below the declared non-EV load {non_ev_load_mw} MW"
                )
            }
            DepotLoadError::DoeEvaluation(text) => write!(f, "the DOE evaluation failed: {text}"),
            DepotLoadError::NonPositiveIntervalMillis { value } => {
                write!(f, "the composition interval length must be positive in ms (found {value})")
            }
        }
    }
}

impl std::error::Error for DepotLoadError {}

/// The depot site load-management configuration (declared data; no
/// default simultaneity factor exists).
#[derive(Debug, Clone, PartialEq)]
pub struct DepotSiteConfig {
    /// The site transformer limit in MW (strictly positive).
    pub site_transformer_limit_mw: f64,
    /// The declared non-EV site load in MW (non-negative; the load the
    /// chargers must leave room for — declared, never inferred from
    /// telemetry here).
    pub non_ev_site_load_mw: f64,
    /// The declared simultaneity factor in [0,1] (the fraction of
    /// connected chargers assumed to charge at full rate together;
    /// [S-141][S-140], UNVERIFIED transcription). No default.
    pub simultaneity_factor: f64,
    /// The depot's chargers (non-empty, unique ids, positive ratings).
    pub chargers: Vec<ChargerRating>,
}

impl DepotSiteConfig {
    /// Validates the configuration: positive transformer limit,
    /// non-negative non-EV load, factor in [0,1], valid unique charger
    /// ratings.
    pub fn validate(&self) -> Result<(), DepotLoadError> {
        if !self.site_transformer_limit_mw.is_finite() || self.site_transformer_limit_mw <= 0.0 {
            return Err(DepotLoadError::InvalidTransformerLimit {
                value: self.site_transformer_limit_mw,
            });
        }
        if !self.non_ev_site_load_mw.is_finite() || self.non_ev_site_load_mw < 0.0 {
            return Err(DepotLoadError::InvalidNonEvLoadMw {
                value: self.non_ev_site_load_mw,
            });
        }
        if !self.simultaneity_factor.is_finite()
            || self.simultaneity_factor < 0.0
            || self.simultaneity_factor > 1.0
        {
            return Err(DepotLoadError::SimultaneityFactorOutOfRange {
                value: self.simultaneity_factor,
            });
        }
        let mut seen: Vec<&str> = Vec::new();
        for charger in &self.chargers {
            if charger.charger_id.trim().is_empty()
                || !charger.rated_power_mw.is_finite()
                || charger.rated_power_mw <= 0.0
            {
                return Err(DepotLoadError::InvalidChargerRating {
                    charger_id: charger.charger_id.clone(),
                });
            }
            if seen.contains(&charger.charger_id.as_str()) {
                return Err(DepotLoadError::DuplicateChargerId {
                    charger_id: charger.charger_id.clone(),
                });
            }
            seen.push(&charger.charger_id);
        }
        Ok(())
    }
}

/// The DOE interplay: the effective SITE import limit at one evaluation
/// instant — the MINIMUM of the transformer limit and the effective DOE
/// import bound. When no DOE applies (lapsed, per the P14.15 lapse
/// semantics reused verbatim), the declared static connection limit
/// governs (the mandatory-fallback rule — never silently unconstrained).
pub fn effective_site_import_limit_mw(
    config: &DepotSiteConfig,
    doe: Option<&DynamicOperatingEnvelope>,
    doe_fallback: Option<&StaticConnectionLimit>,
    now: &EventTime,
) -> Result<f64, DepotLoadError> {
    config.validate()?;
    let mut limit = config.site_transformer_limit_mw;
    if let Some(envelope) = doe {
        // The P14.15 rule: a DOE is only evaluated WITH its static
        // fallback; evaluating without one is refused (never "unconstrained").
        let fallback = doe_fallback.ok_or_else(|| {
            DepotLoadError::DoeEvaluation(
                "a DOE was declared without a static connection-limit fallback (the \
                 P14.15 mandatory-fallback rule)"
                    .to_string(),
            )
        })?;
        let effective = evaluate_envelope(envelope, fallback, now)
            .map_err(|e| DepotLoadError::DoeEvaluation(e.to_string()))?;
        let (p_min_mw, _p_max_mw) = crate::doe_envelope::effective_envelope_bounds_mw(&effective);
        // p_min = −import_limit (the frozen P > 0 = export rule), so the
        // import bound is −p_min. The lapse semantics are already folded
        // into the bounds by `effective_envelope_bounds_mw`: a lapsed
        // envelope yields its static fallback limits (the P14.15
        // mandatory-fallback rule — never silently unconstrained).
        let doe_import_bound = -p_min_mw;
        limit = limit.min(doe_import_bound);
    }
    Ok(limit)
}

/// One per-charger limit the shaping produced (constraint data for the
/// composer).
#[derive(Debug, Clone, PartialEq)]
pub struct ShapedChargerLimit {
    /// The charger id (verbatim spelling).
    pub charger_id: String,
    /// The shaped per-charger limit in MW (non-negative; never above the
    /// charger's rating).
    pub limit_mw: f64,
    /// Whether the shaping had to curtail the charger below its rating
    /// (false = the charger keeps its full rating).
    pub curtailed_by_shaping: bool,
}

/// The simultaneity-factor shaping across the connected chargers
/// ([S-141][S-140]; the algorithm is the recorded one — see the module
/// header):
///
/// 1. headroom = effective site import limit − declared non-EV load;
///    headroom ≤ 0 is refused (no feasible depot charging — the
///    composition says so, it never produces negative limits);
/// 2. if `factor × Σ(connected ratings)` ≤ headroom, every charger keeps
///    its rating (the coincidence assumption holds);
/// 3. otherwise the headroom is distributed proportionally to the
///    connected chargers' ratings — each charger's limit is
///    `headroom × rating_i / Σ(ratings)`, capped at its rating (a cap can
///    only bind if the headroom exceeds the sum, which step 2 excluded,
///    so the cap is defense-in-depth).
///
/// An empty connected set is valid (no charging → no limits; the depot is
/// simply idle) — it is not an error.
pub fn shape_charger_limits(
    config: &DepotSiteConfig,
    connected_charger_ids: &[&str],
    effective_site_import_limit_mw: f64,
) -> Result<Vec<ShapedChargerLimit>, DepotLoadError> {
    config.validate()?;
    if !effective_site_import_limit_mw.is_finite() || effective_site_import_limit_mw <= 0.0 {
        return Err(DepotLoadError::InvalidTransformerLimit {
            value: effective_site_import_limit_mw,
        });
    }
    let headroom = effective_site_import_limit_mw - config.non_ev_site_load_mw;
    if headroom <= 0.0 {
        return Err(DepotLoadError::NoFeasibleHeadroom {
            effective_limit_mw: effective_site_import_limit_mw,
            non_ev_load_mw: config.non_ev_site_load_mw,
        });
    }
    let connected: Vec<&ChargerRating> = connected_charger_ids
        .iter()
        .map(|id| {
            config
                .chargers
                .iter()
                .find(|c| c.charger_id == *id)
                .ok_or_else(|| DepotLoadError::InvalidChargerRating {
                    charger_id: (*id).to_string(),
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let total_rating: f64 = connected.iter().map(|c| c.rated_power_mw).sum();
    if total_rating <= 0.0 {
        // Empty connected set (or a rounding-zero sum): no charging.
        return Ok(Vec::new());
    }
    let coincident = config.simultaneity_factor * total_rating;
    let shaped = if coincident <= headroom {
        // The coincidence assumption holds: full ratings.
        connected
            .iter()
            .map(|c| ShapedChargerLimit {
                charger_id: c.charger_id.clone(),
                limit_mw: c.rated_power_mw,
                curtailed_by_shaping: false,
            })
            .collect()
    } else {
        // Proportional distribution of the headroom (deterministic; the
        // ratings cap is defense-in-depth).
        connected
            .iter()
            .map(|c| {
                let limit = (headroom * c.rated_power_mw / total_rating)
                    .min(c.rated_power_mw)
                    .max(0.0);
                ShapedChargerLimit {
                    charger_id: c.charger_id.clone(),
                    limit_mw: limit,
                    curtailed_by_shaping: limit < c.rated_power_mw,
                }
            })
            .collect()
    };
    Ok(shaped)
}

/// The external-constraint profile: per-interval effective site import
/// caps over a declared interval grid, in deterministic interval order —
/// the CONSTRAINT DATA the P17.05 composer maps onto the
/// `ChargingStationExternalConstraints` purpose (UNVERIFIED spelling,
/// [S-124]) downstream of the frozen lifecycle gates.
#[derive(Debug, Clone, PartialEq)]
pub struct ExternalConstraintProfile {
    /// The site (connection point) the profile is composed for.
    pub connection_point: ConnectionPointId,
    /// The per-interval caps keyed by interval-start (UTC ms) — BTreeMap
    /// (deterministic order).
    caps_by_interval_start: BTreeMap<i64, f64>,
    /// The interval length in milliseconds (the declared grid).
    interval_millis: i64,
}

impl ExternalConstraintProfile {
    /// Composes the profile over the declared interval grid: for every
    /// interval start in `[horizon_start, horizon_start + interval_millis
    /// × intervals)`, the effective site import limit is evaluated at that
    /// instant (DOE interplay included) and recorded. Every evaluated
    /// instant produces a cap — an unknown or unevaluable instant is a
    /// refusal, never a silent gap.
    #[allow(clippy::too_many_arguments)]
    pub fn compose(
        connection_point: ConnectionPointId,
        config: &DepotSiteConfig,
        doe: Option<&DynamicOperatingEnvelope>,
        doe_fallback: Option<&StaticConnectionLimit>,
        horizon_start: &EventTime,
        interval_millis: i64,
        intervals: usize,
    ) -> Result<Self, DepotLoadError> {
        if interval_millis <= 0 {
            return Err(DepotLoadError::NonPositiveIntervalMillis {
                value: interval_millis,
            });
        }
        let mut caps: BTreeMap<i64, f64> = BTreeMap::new();
        for index in 0..intervals {
            // `EventTime::unix_millis()` is i128 (the contracts time-model
            // contract); the grid arithmetic is checked on conversion back
            // to i64 — an out-of-range instant is a refusal, never a wrap.
            let instant_ms_i128 =
                horizon_start.unix_millis() + (index as i128) * (interval_millis as i128);
            let instant_ms = i64::try_from(instant_ms_i128).map_err(|_| {
                DepotLoadError::DoeEvaluation(format!(
                    "the interval start {instant_ms_i128} ms is outside the representable \
                     UTC range"
                ))
            })?;
            let instant = EventTime::from_unix_millis(instant_ms).map_err(|_| {
                DepotLoadError::DoeEvaluation(format!(
                    "the interval start {instant_ms} ms is outside the representable UTC range"
                ))
            })?;
            let cap =
                effective_site_import_limit_mw(config, doe, doe_fallback, &instant)?;
            caps.insert(instant_ms, cap);
        }
        Ok(Self {
            connection_point,
            caps_by_interval_start: caps,
            interval_millis,
        })
    }

    /// The per-interval caps, in deterministic interval order.
    pub fn caps(&self) -> Vec<(i64, f64)> {
        self.caps_by_interval_start.iter().map(|(k, v)| (*k, *v)).collect()
    }

    /// The cap at one interval start, when the profile covers it.
    pub fn cap_at(&self, interval_start_millis: i64) -> Option<f64> {
        self.caps_by_interval_start
            .get(&interval_start_millis)
            .copied()
    }

    /// The declared interval length in milliseconds.
    pub const fn interval_millis(&self) -> i64 {
        self.interval_millis
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doe_envelope::{ImportExportLimits, UpdateCadence};
    // `Provenance` lives in flexcore-contracts (doe_envelope imports it
    // privately — it is not re-exported from there).
    use flexcore_contracts::capability::Provenance;

    const CP: &str = "depot-site-connection-01";

    fn connection_point() -> ConnectionPointId {
        ConnectionPointId::new(CP.to_string()).unwrap()
    }

    fn config() -> DepotSiteConfig {
        DepotSiteConfig {
            site_transformer_limit_mw: 0.5,
            non_ev_site_load_mw: 0.05,
            simultaneity_factor: 1.0,
            chargers: vec![
                ChargerRating {
                    charger_id: "cs-depot-01".to_string(),
                    rated_power_mw: 0.22,
                },
                ChargerRating {
                    charger_id: "cs-depot-02".to_string(),
                    rated_power_mw: 0.22,
                },
                ChargerRating {
                    charger_id: "cs-depot-03".to_string(),
                    rated_power_mw: 0.11,
                },
            ],
        }
    }

    fn now() -> EventTime {
        EventTime::from_unix_millis(1_700_000_000_000).unwrap()
    }

    #[test]
    fn the_statement_carries_the_composer_and_lifecycle_rules() {
        assert!(DEPOT_LOAD_MANAGEMENT_STATEMENT.contains("external-constraint profile"));
        assert!(DEPOT_LOAD_MANAGEMENT_STATEMENT.contains("ChargingStationExternalConstraints"));
        assert!(DEPOT_LOAD_MANAGEMENT_STATEMENT.contains("frozen 14-state command lifecycle"));
    }

    #[test]
    fn config_validation_refuses_bad_limits_factor_and_chargers() {
        let mut bad = config();
        bad.site_transformer_limit_mw = 0.0;
        assert_eq!(
            bad.validate(),
            Err(DepotLoadError::InvalidTransformerLimit { value: 0.0 })
        );
        let mut bad = config();
        bad.non_ev_site_load_mw = -0.1;
        assert_eq!(
            bad.validate(),
            Err(DepotLoadError::InvalidNonEvLoadMw { value: -0.1 })
        );
        // No default factor: 1.5 and NaN are both refused.
        let mut bad = config();
        bad.simultaneity_factor = 1.5;
        assert_eq!(
            bad.validate(),
            Err(DepotLoadError::SimultaneityFactorOutOfRange { value: 1.5 })
        );
        let mut bad = config();
        bad.simultaneity_factor = f64::NAN;
        assert!(bad.validate().is_err());
        let mut bad = config();
        bad.chargers[1].rated_power_mw = 0.0;
        assert_eq!(
            bad.validate(),
            Err(DepotLoadError::InvalidChargerRating {
                charger_id: "cs-depot-02".to_string()
            })
        );
        let mut bad = config();
        bad.chargers.push(ChargerRating {
            charger_id: "cs-depot-01".to_string(),
            rated_power_mw: 0.11,
        });
        assert_eq!(
            bad.validate(),
            Err(DepotLoadError::DuplicateChargerId {
                charger_id: "cs-depot-01".to_string()
            })
        );
    }

    #[test]
    fn the_effective_import_limit_is_the_transformer_limit_without_a_doe() {
        let limit = effective_site_import_limit_mw(&config(), None, None, &now()).unwrap();
        assert_eq!(limit, 0.5);
    }

    #[test]
    fn a_doe_in_effect_caps_the_site_import_at_the_envelope_bound() {
        // DOE import bound 0.3 < transformer limit 0.5: the minimum
        // governs. The latest update is at `now()` (age 0, inside the
        // 300 s × 2 = 600 s freshness budget), so the envelope is in
        // effect at `now()` rather than stale.
        let envelope = DynamicOperatingEnvelope::new(
            "doe-17".to_string(),
            crate::doe_envelope::EnvelopeScope::Single(connection_point()),
            ImportExportLimits::new(0.3, 0.0).unwrap(),
            1_699_999_000_000,
            1_700_001_000_000,
            UpdateCadence {
                expected_interval_seconds: 300,
                max_intervals_before_stale: 2,
            },
            1_700_000_000_000,
            "test DNSP".to_string(),
            Provenance::Configured,
            "synthetic test envelope; no operator issued it".to_string(),
        )
        .unwrap();
        let fallback = StaticConnectionLimit::new(
            connection_point(),
            ImportExportLimits::new(0.42, 0.0).unwrap(),
            "connection agreement (declared test data)".to_string(),
            Provenance::Configured,
        )
        .unwrap();
        let limit =
            effective_site_import_limit_mw(&config(), Some(&envelope), Some(&fallback), &now())
                .unwrap();
        assert_eq!(limit, 0.3);
    }

    #[test]
    fn a_lapsed_doe_falls_back_to_the_static_connection_limit() {
        // Expiry before `now`: the envelope lapses and the static limit
        // (0.42) governs.
        let envelope = DynamicOperatingEnvelope::new(
            "doe-17".to_string(),
            crate::doe_envelope::EnvelopeScope::Single(connection_point()),
            ImportExportLimits::new(0.3, 0.0).unwrap(),
            1_699_999_000_000,
            1_699_999_500_000,
            UpdateCadence {
                expected_interval_seconds: 300,
                max_intervals_before_stale: 2,
            },
            1_699_999_000_000,
            "test DNSP".to_string(),
            Provenance::Configured,
            "synthetic test envelope; no operator issued it".to_string(),
        )
        .unwrap();
        let fallback = StaticConnectionLimit::new(
            connection_point(),
            ImportExportLimits::new(0.42, 0.0).unwrap(),
            "connection agreement (declared test data)".to_string(),
            Provenance::Configured,
        )
        .unwrap();
        let limit =
            effective_site_import_limit_mw(&config(), Some(&envelope), Some(&fallback), &now())
                .unwrap();
        assert_eq!(limit, 0.42);
    }

    #[test]
    fn a_doe_without_a_static_fallback_is_refused_never_unconstrained() {
        let envelope = DynamicOperatingEnvelope::new(
            "doe-17".to_string(),
            crate::doe_envelope::EnvelopeScope::Single(connection_point()),
            ImportExportLimits::new(0.3, 0.0).unwrap(),
            1_699_999_000_000,
            1_700_001_000_000,
            UpdateCadence {
                expected_interval_seconds: 300,
                max_intervals_before_stale: 2,
            },
            1_699_999_000_000,
            "test DNSP".to_string(),
            Provenance::Configured,
            "synthetic test envelope; no operator issued it".to_string(),
        )
        .unwrap();
        let err =
            effective_site_import_limit_mw(&config(), Some(&envelope), None, &now()).unwrap_err();
        assert!(matches!(err, DepotLoadError::DoeEvaluation(_)));
        assert!(err.to_string().contains("mandatory-fallback"));
    }

    #[test]
    fn the_coincidence_assumption_holding_keeps_full_ratings() {
        // Factor 1.0, ratings sum 0.55, headroom 0.45: 0.55 > 0.45 →
        // proportional. But factor 0.8: 0.44 ≤ 0.45 → full ratings.
        let mut cfg = config();
        cfg.simultaneity_factor = 0.8;
        let shaped = shape_charger_limits(&cfg, &["cs-depot-01", "cs-depot-02", "cs-depot-03"], 0.5)
            .unwrap();
        assert_eq!(shaped.len(), 3);
        assert!(shaped.iter().all(|s| !s.curtailed_by_shaping));
        assert_eq!(shaped[0].limit_mw, 0.22);
        assert_eq!(shaped[2].limit_mw, 0.11);
    }

    #[test]
    fn the_coincidence_assumption_failing_distributes_headroom_proportionally() {
        // Factor 1.0, ratings sum 0.55 > headroom 0.45: proportional.
        // cs-depot-01: 0.45 × 0.22/0.55 = 0.18; cs-depot-03:
        // 0.45 × 0.11/0.55 = 0.09.
        let shaped = shape_charger_limits(&config(), &["cs-depot-01", "cs-depot-02", "cs-depot-03"], 0.5)
            .unwrap();
        assert!(shaped[0].curtailed_by_shaping);
        assert!((shaped[0].limit_mw - 0.18).abs() < 1e-12);
        assert!((shaped[2].limit_mw - 0.09).abs() < 1e-12);
        // The shaped sum must not exceed the headroom.
        let sum: f64 = shaped.iter().map(|s| s.limit_mw).sum();
        assert!(sum <= 0.45 + 1e-12);
    }

    #[test]
    fn an_empty_connected_set_is_an_idle_depot_not_an_error() {
        let shaped = shape_charger_limits(&config(), &[], 0.5).unwrap();
        assert!(shaped.is_empty());
    }

    #[test]
    fn an_unknown_connected_charger_is_refused() {
        let err =
            shape_charger_limits(&config(), &["cs-ghost"], 0.5).unwrap_err();
        assert_eq!(
            err,
            DepotLoadError::InvalidChargerRating {
                charger_id: "cs-ghost".to_string()
            }
        );
    }

    #[test]
    fn no_feasible_headroom_is_refused_never_negative_limits() {
        // Non-EV load equals the limit: zero headroom is refused (the
        // composition says so rather than producing zero-or-negative
        // charger limits).
        let err = shape_charger_limits(
            &config(),
            &["cs-depot-01"],
            0.05, // effective limit == non-EV load → no headroom
        )
        .unwrap_err();
        assert!(matches!(err, DepotLoadError::NoFeasibleHeadroom { .. }));
    }

    #[test]
    fn the_profile_composes_deterministic_per_interval_caps() {
        let profile = ExternalConstraintProfile::compose(
            connection_point(),
            &config(),
            None,
            None,
            &now(),
            900_000, // 15-minute intervals
            4,
        )
        .unwrap();
        let caps = profile.caps();
        assert_eq!(caps.len(), 4);
        // Deterministic ascending interval order (BTreeMap-backed).
        let starts: Vec<i64> = caps.iter().map(|(k, _)| *k).collect();
        assert_eq!(starts[0], 1_700_000_000_000);
        assert!(starts.windows(2).all(|w| w[0] < w[1]));
        assert!(caps.iter().all(|(_, cap)| (*cap - 0.5).abs() < 1e-12));
        assert_eq!(profile.cap_at(1_700_000_000_000), Some(0.5));
        assert_eq!(profile.cap_at(1_699_999_999_999), None);
        assert_eq!(profile.interval_millis(), 900_000);
    }

    #[test]
    fn the_profile_refuses_a_non_positive_interval_grid() {
        assert_eq!(
            ExternalConstraintProfile::compose(
                connection_point(),
                &config(),
                None,
                None,
                &now(),
                0,
                4
            ),
            Err(DepotLoadError::NonPositiveIntervalMillis { value: 0 })
        );
    }
}
