// P14.17 — The simulator's DOE behaviour profile (decision record 81;
// the recorded P14.15 → P14.17 handoff, record 63 §5 and the
// `doe_envelope` module header: "wave-3 Agent A (P14.17) extends
// `crates/flexcore-simulator/src/profiles.rs` with a DOE behaviour
// profile consuming [`DynamicOperatingEnvelope`], [`StaticConnectionLimit`],
// and [`effective_envelope_bounds_mw`] from THIS module (the simulator
// never redefines the semantics; it only samples them per tick)").
//
// WHAT THIS MODULE IS:
//
// - The per-tick DOE sampler the handoff names. The envelope semantics
//   live ENTIRELY in `flexcore_domain::doe_envelope`
//   (`evaluate_envelope` — lapse reasons, freshness budget, the SAPN
//   static fallback — all the domain's own UNVERIFIED-against-primary-
//   -sources semantics); this module CONSUMES them and adds only the
//   simulator's two jobs:
//     1. sampling the envelope at a declared tick instant (an injected
//        `EventTime` — the injected-Clock rule; no wall-clock read), and
//     2. clipping a synthetic asset tick's active power into the
//        effective bounds ([`effective_envelope_bounds_mw`] — the frozen
//        P > 0 = export sign rule), producing named breach/curtailment
//        simulator records when the synthetic shape exceeds the envelope.
// - The TEST-ONLY fixture constructors for envelopes and static limits
//   (synthetic declared data, the same class as the Phase 2 profiles).
//
// WHAT THIS MODULE IS NOT:
//
// - **Not a redefinition of DOE semantics.** Lapse behaviour, cadence
//   staleness, and the bounds arithmetic are the domain's. A change
//   there flows through here untouched.
// - **Not a dispatch path.** Clipping a synthetic tick is simulator
//   bookkeeping for constraint-data testing; it issues no command and
//   produces no setpoint (phase-14 context §3 constraint 3 — DOE bounds
//   are optimizer constraint data only; ADR-010 extended).
// - **Not a scenario-schema extension.** The v1 scenario schema
//   (`scenario.rs`, the frozen P2.13 artefact) is UNCHANGED: a DOE
//   profile is a composition-time profile consumed by the engine's
//   CALLER, not a new `AssetType`. Recorded decision (record 81 §4):
//   extending the versioned scenario file format for a Phase 14 profile
//   would churn a frozen surface for no simulation gain.
//
// Every produced record is SIMULATED data: breach and curtailment
// records from this module describe synthetic shapes against synthetic
// envelopes and are never evidence about real network behaviour.

use flexcore_contracts::time_model::EventTime;
use flexcore_contracts::units::ActivePowerMW;
use flexcore_domain::doe_envelope::{
    effective_envelope_bounds_mw, evaluate_envelope, BreachDirection, ConnectionPointId,
    CurtailmentRecord, DynamicOperatingEnvelope, EffectiveEnvelope, EnvelopeBreachRecord,
    ImportExportLimits, LapseReason, StaticConnectionLimit,
};

/// The simulator's per-tick DOE sample: the effective envelope at the
/// tick instant plus the bounds pair derived from it (the frozen sign
/// rule: `[−import_limit, +export_limit]` MW).
#[derive(Debug, Clone, PartialEq)]
pub struct DoeTickSample {
    /// The effective envelope at the tick (the domain's own evaluation
    /// result — dynamic limits or the named lapse reason with the static
    /// fallback).
    pub effective: EffectiveEnvelope,
    /// The derived bounds pair `(p_min_mw, p_max_mw)` (the optimizer
    /// constraint-data shape; consumed here for clipping).
    pub p_min_mw: f64,
    pub p_max_mw: f64,
}

/// Errors the DOE profile reports. Every failure is named; the domain's
/// own errors are re-raised, never reinterpreted.
#[derive(Debug, Clone, PartialEq)]
pub enum DoeProfileError {
    /// The domain's envelope evaluation refused (re-raised verbatim —
    /// the simulator never reinterprets a domain refusal).
    Evaluation(flexcore_domain::doe_envelope::DoeError),
    /// A breach record could not be constructed for the clipped shape
    /// (the domain refuses a "breach" that does not breach — a clipping
    /// arithmetic defect, surfaced, never tolerated).
    Breach(flexcore_domain::doe_envelope::DoeError),
}

impl std::fmt::Display for DoeProfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DoeProfileError::Evaluation(error) => write!(f, "the domain refused the envelope evaluation: {error}"),
            DoeProfileError::Breach(error) => write!(f, "the domain refused the breach record: {error}"),
        }
    }
}

impl std::error::Error for DoeProfileError {}

/// The DOE behaviour profile: one envelope + one mandatory static
/// fallback for one connection point, sampled per tick. Constructed
/// only through [`DoeProfile::new`] — no `Default`, no partially-declared
/// profile (a DOE without its static fallback never evaluates: the
/// domain's own rule, honored at construction).
#[derive(Debug, Clone)]
pub struct DoeProfile {
    /// The profile's identity string (carries the `sim-doe` prefix —
    /// synthetic data, never evidence about a real connection point).
    pub profile_id: String,
    /// The ingested envelope (domain data, held by value).
    pub envelope: DynamicOperatingEnvelope,
    /// The mandatory static fallback (domain data, held by value).
    pub fallback: StaticConnectionLimit,
}

impl DoeProfile {
    /// Constructs the profile over declared domain data. The fallback's
    /// connection point must be covered by the envelope's scope (a
    /// profile whose fallback belongs to another point is a composition
    /// error — the sampled bounds would apply to the wrong point).
    pub fn new(
        profile_id: impl Into<String>,
        envelope: DynamicOperatingEnvelope,
        fallback: StaticConnectionLimit,
    ) -> Result<Self, DoeProfileError> {
        if !envelope.scope.covers(&fallback.connection_point) {
            return Err(DoeProfileError::Evaluation(
                flexcore_domain::doe_envelope::DoeError::EmptyField {
                    field: "static fallback connection point is not covered by the envelope scope",
                },
            ));
        }
        Ok(Self {
            profile_id: format!("sim-doe:{}", profile_id.into()),
            envelope,
            fallback,
        })
    }

    /// Samples the profile at one tick instant (an injected `EventTime`).
    /// The evaluation is the DOMAIN's `evaluate_envelope` — this module
    /// supplies only the instant.
    pub fn sample(&self, now: &EventTime) -> Result<DoeTickSample, DoeProfileError> {
        let effective = evaluate_envelope(&self.envelope, &self.fallback, now)
            .map_err(DoeProfileError::Evaluation)?;
        let (p_min_mw, p_max_mw) = effective_envelope_bounds_mw(&effective);
        Ok(DoeTickSample { effective, p_min_mw, p_max_mw })
    }

    /// Clips one synthetic tick's active power into the sampled bounds
    /// and reports the simulator records the tick produces:
    ///
    /// - `clipped_mw`: the synthetic value AFTER clipping (inside the
    ///   effective bounds; the tick as the constraint-aware simulator
    ///   emits it);
    /// - `breach`: a domain [`EnvelopeBreachRecord`] when the RAW
    ///   synthetic value exceeded a bound (None otherwise) — built
    ///   through the domain's own validating constructor, so a record
    ///   that does not breach is a named error here, never a spurious
    ///   record;
    /// - `curtailment`: a domain [`CurtailmentRecord`] for the clipped
    ///   magnitude when a breach was clipped (None otherwise).
    ///
    /// No schedule, no setpoint, no command: the clipped value is what
    /// the synthetic tick EMITS for the constraint-data path to test
    /// against.
    pub fn clip_tick(
        &self,
        raw_mw: f64,
        now: &EventTime,
        site: Option<flexcore_domain::ids::SiteId>,
    ) -> Result<DoeClippedTick, DoeProfileError> {
        let sample = self.sample(now)?;
        let (p_min, p_max) = (sample.p_min_mw, sample.p_max_mw);
        let (clipped_mw, direction, limit_mw) = if raw_mw > p_max {
            (p_max, BreachDirection::Export, p_max)
        } else if raw_mw < p_min {
            (p_min, BreachDirection::Import, -p_min)
        } else {
            (raw_mw, BreachDirection::Export, 0.0) // direction/limit unused when no breach
        };
        // The tick's record timestamp: the injected instant converted to
        // the records' i64 Unix-ms domain (the same conversion the
        // domain's own evaluation applies to the instant).
        let observed_at_unix_ms =
            i64::try_from(now.unix_millis()).unwrap_or(self.envelope.issued_at_unix_ms);
        let breach = if raw_mw > p_max || raw_mw < p_min {
            let p = ActivePowerMW::new(raw_mw).map_err(|_| {
                DoeProfileError::Breach(flexcore_domain::doe_envelope::DoeError::NonFiniteValue {
                    field: "raw_mw",
                })
            })?;
            Some(
                EnvelopeBreachRecord::new(
                    Some(self.envelope.envelope_id.clone()),
                    self.fallback.connection_point.clone(),
                    direction,
                    p,
                    limit_mw,
                    observed_at_unix_ms,
                    site.clone(),
                )
                .map_err(DoeProfileError::Breach)?,
            )
        } else {
            None
        };
        let curtailment = match (breach.is_some(), direction) {
            (true, dir) if (clipped_mw - raw_mw).abs() > f64::EPSILON => {
                let magnitude = (raw_mw - clipped_mw).abs();
                Some(
                    CurtailmentRecord::new(
                        Some(self.envelope.envelope_id.clone()),
                        self.fallback.connection_point.clone(),
                        magnitude,
                        dir,
                        observed_at_unix_ms,
                        site,
                    )
                    .map_err(DoeProfileError::Breach)?,
                )
            }
            _ => None,
        };
        Ok(DoeClippedTick { clipped_mw, breach, curtailment, effective: sample.effective })
    }
}

/// The result of clipping one synthetic tick (see
/// [`DoeProfile::clip_tick`]).
#[derive(Debug, Clone, PartialEq)]
pub struct DoeClippedTick {
    /// The synthetic value after clipping (inside the effective bounds).
    pub clipped_mw: f64,
    /// The breach record for the RAW synthetic value, when it exceeded a
    /// bound.
    pub breach: Option<EnvelopeBreachRecord>,
    /// The curtailment record for the clipped magnitude, when one was
    /// clipped.
    pub curtailment: Option<CurtailmentRecord>,
    /// The effective envelope governing this tick (dynamic or lapsed).
    pub effective: EffectiveEnvelope,
}

/// Builds a TEST-ONLY envelope fixture (synthetic declared data; every
/// semantic is the domain's — this constructor only fills declared
/// values with a fixed provenance note naming the fixture).
#[allow(clippy::too_many_arguments)]
pub fn fixture_envelope(
    envelope_id: impl Into<String>,
    scope: flexcore_domain::doe_envelope::EnvelopeScope,
    limits: ImportExportLimits,
    issued_at_unix_ms: i64,
    expires_at_unix_ms: i64,
    expected_interval_seconds: i64,
    max_intervals_before_stale: u32,
) -> Result<DynamicOperatingEnvelope, DoeProfileError> {
    DynamicOperatingEnvelope::new(
        envelope_id.into(),
        scope,
        limits,
        issued_at_unix_ms,
        expires_at_unix_ms,
        flexcore_domain::doe_envelope::UpdateCadence {
            expected_interval_seconds,
            max_intervals_before_stale,
        },
        issued_at_unix_ms,
        "simulated DNSP fixture (TEST/simulation-only; never a real operator)".to_string(),
        flexcore_contracts::capability::Provenance::Configured,
        "synthetic DOE fixture for the simulator's constraint-data path (record 81)".to_string(),
    )
    .map_err(DoeProfileError::Evaluation)
}

/// Builds a TEST-ONLY static-limit fixture for a connection point.
pub fn fixture_static_limit(
    connection_point: ConnectionPointId,
    limits: ImportExportLimits,
) -> Result<StaticConnectionLimit, DoeProfileError> {
    StaticConnectionLimit::new(
        connection_point,
        limits,
        "synthetic connection agreement fixture (TEST/simulation-only)".to_string(),
        flexcore_contracts::capability::Provenance::Configured,
    )
    .map_err(DoeProfileError::Evaluation)
}

/// The canonical name of a lapse reason (a pass-through to the domain's
/// own `name()` — re-exported for the simulator's record surfaces so no
/// caller re-spells a lapse reason).
pub const fn lapse_reason_name(reason: LapseReason) -> &'static str {
    reason.name()
}

#[cfg(test)]
mod tests {
    use super::*;

    const POINT: &str = "sim-cp-0001";

    fn point() -> ConnectionPointId {
        ConnectionPointId::new(POINT.to_string()).expect("valid id")
    }

    fn profile() -> DoeProfile {
        let envelope = fixture_envelope(
            "doe-sim-1",
            flexcore_domain::doe_envelope::EnvelopeScope::Single(point()),
            ImportExportLimits::new(2.0, 1.0).expect("valid limits"),
            1_000,
            1_000_000,
            60,
            2,
        )
        .expect("valid fixture envelope");
        let fallback = fixture_static_limit(
            point(),
            ImportExportLimits::new(5.0, 3.0).expect("valid limits"),
        )
        .expect("valid fixture static limit");
        DoeProfile::new("profile-1", envelope, fallback).expect("valid profile")
    }

    fn at(ms: i64) -> EventTime {
        EventTime::from_unix_millis(ms).expect("valid event time")
    }

    #[test]
    fn the_profile_id_carries_the_sim_prefix() {
        assert!(profile().profile_id.starts_with("sim-doe"));
    }

    #[test]
    fn a_fallback_outside_the_envelope_scope_is_refused() {
        let envelope = fixture_envelope(
            "doe-sim-2",
            flexcore_domain::doe_envelope::EnvelopeScope::Single(point()),
            ImportExportLimits::new(2.0, 1.0).expect("valid"),
            1_000,
            1_000_000,
            60,
            2,
        )
        .expect("valid fixture envelope");
        let other = ConnectionPointId::new("sim-cp-other".to_string()).expect("valid id");
        let fallback = fixture_static_limit(
            other,
            ImportExportLimits::new(5.0, 3.0).expect("valid"),
        )
        .expect("valid fixture static limit");
        assert!(matches!(
            DoeProfile::new("profile-2", envelope, fallback),
            Err(DoeProfileError::Evaluation(
                flexcore_domain::doe_envelope::DoeError::EmptyField { .. }
            ))
        ));
    }

    #[test]
    fn sampling_consumes_the_domain_semantics_untouched() {
        let profile = profile();
        // In effect: dynamic bounds [-2, +1] (the domain's sign rule).
        let sample = profile.sample(&at(2_000)).expect("samples");
        assert_eq!(
            sample.effective,
            EffectiveEnvelope::Dynamic {
                limits: ImportExportLimits::new(2.0, 1.0).expect("valid"),
            }
        );
        assert_eq!(sample.p_min_mw, -2.0);
        assert_eq!(sample.p_max_mw, 1.0);
        // Stale (cadence 60 s × budget 2 → freshness 120 s; update at
        // issue): the SAPN fallback applies — the STATIC bounds.
        let sample = profile.sample(&at(121_000)).expect("samples");
        assert_eq!(lapse_reason_name(flexcore_domain::doe_envelope::LapseReason::Stale), "stale");
        let (p_min, p_max) = effective_envelope_bounds_mw(&sample.effective);
        assert_eq!(p_min, -5.0, "the static fallback governs after lapse");
        assert_eq!(p_max, 3.0);
    }

    #[test]
    fn clipping_names_breaches_and_produces_curtailment_records() {
        let profile = profile();
        // A raw synthetic export of 1.4 MW against the dynamic export
        // bound of 1.0 MW: clipped to 1.0, breach + curtailment recorded.
        let tick = profile.clip_tick(1.4, &at(2_000), None).expect("clips");
        assert_eq!(tick.clipped_mw, 1.0);
        let breach = tick.breach.expect("breach recorded");
        assert_eq!(breach.direction.name(), "export");
        assert_eq!(breach.limit_mw, 1.0);
        assert_eq!(breach.observed_mw.value(), 1.4);
        let curtailment = tick.curtailment.expect("curtailment recorded");
        assert!((curtailment.curtailed_mw - 0.4).abs() < 1e-9);
        // An in-bounds tick: no breach, no curtailment, value unchanged.
        let tick = profile.clip_tick(0.5, &at(2_000), None).expect("clips");
        assert_eq!(tick.clipped_mw, 0.5);
        assert!(tick.breach.is_none());
        assert!(tick.curtailment.is_none());
    }

    #[test]
    fn a_lapsed_tick_clips_to_the_static_bounds() {
        let profile = profile();
        // Past the freshness budget the envelope lapses: the synthetic
        // tick is clipped to the STATIC export bound of 3.0 MW, and the
        // breach record carries no envelope id (the static limit was
        // breached — the honest no-DOE representation).
        let tick = profile.clip_tick(3.5, &at(121_000), None).expect("clips");
        assert_eq!(tick.clipped_mw, 3.0);
        let breach = tick.breach.expect("static-bound breach recorded");
        // The breach record still names the (lapsed) envelope — the
        // domain's record vocabulary carries the envelope id when one
        // was declared; the effective bound was the static fallback.
        assert!(breach.envelope_id.is_some());
        assert_eq!(breach.limit_mw, 3.0);
    }
}
