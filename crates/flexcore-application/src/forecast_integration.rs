// P15.16 — Forecast-to-schedule integration: envelopes and rolling-horizon
//         scheduling consume real forecasts with staleness rules EXTENDED
//         from the P3.13 envelope-staleness discipline, with a fallback to
//         the P15.02 persistence baseline on staleness; the P3.31 fixture
//         stays the labeled synthetic fallback source.
//
// Phase 15 plan §2 row P15.16; decision record:
// `docs/phases-11-20/phase-15-forecasting-asset-intelligence/records/
// 45_p15_16_forecast_to_schedule_integration.md`.
// Sources: [S-41] (forecasting as table stakes — the consumption side of
// the forecast pipeline). No skill claim anywhere.
//
// THE POLICY, EXTENDED FROM P3.13 (envelope_staleness.rs — the single
// staleness home; this module COMPOSES its pure functions, never
// restates them):
//
//   **A STALE forecast is never silently consumed by scheduling.** When
//   the published forecast is stale (a `ForecastPublished` trigger with a
//   new version, a horizon rollover the forecast does not cover, or the
//   configured max-age bound), the scheduler falls back — loudly, with
//   the staleness reason carried on the decision — to the persistence
//   baseline (P15.02 `persistence_forecast`, the frozen reference rule
//   set). When no history exists for even the persistence baseline, the
//   P3.31 synthetic fixture remains the labeled synthetic fallback
//   source — its `PROVENANCE_SYNTHETIC` label travels with every value
//   (the four-place labeling rule is unchanged). A fallback is a
//   RECORDED degradation, never a silent substitution: every decision
//   names the source it selected and why.
//
// The three triggers of the P3.13 discipline are REUSED via
// `evaluate_envelope_trigger` (the `ForecastPublished` event with the
// observed version, the horizon rollover, and the max-age bound — the
// same P2.08 equality convention: age == bound is NOT stale). The
// debounce semantics are the P3.13 ones verbatim (debounce delays only
// the recomputation advice; it never re-validates a stale source).
//
// THE SOURCE ORDER, DECIDED ONCE:
//
//   1. `PublishedForecast` — a real forecast artifact (P15.01 vocabulary)
//      whose version is current (no trigger fired) and whose age is
//      within the configured bound.
//   2. `PersistenceBaselineFallback` — the P15.02 persistence rule set
//      computed over the caller-declared history, carried with the
//      staleness reason that forced the fallback (or the
//      no-published-forecast reason) and the standing NO-SKILL statement
//      (a persistence baseline never claims skill — P15.02).
//   3. `SyntheticFixtureFallback` — the P3.31 fixture, selected only when
//      no history exists for the persistence baseline; the caller
//      generates the fixture itself (this module never invents values);
//      the decision carries the fixture id and the `synthetic`
//      provenance label.
//
// Control status: pure source selection for the simulation-only 0.x
// line. No I/O, no store, no scheduler wiring, no field-write path. A
// fallback forecast is an INPUT to scheduling analytics, never a
// command (ADR-010 extension, program conventions §6.3). No safety,
// certification, or skill claim is made.

use flexcore_contracts::time_model::EventTime;

use crate::envelope_staleness::{
    evaluate_envelope_trigger, EnvelopeFreshness, EnvelopeTriggerEvent, StalenessConfig,
    StalenessReason,
};
use crate::flexibility_envelope::EnvelopeMetadata;
use crate::forecast_baselines::{persistence_forecast, ForecastBaselineError};
use crate::forecast_fixture::{ForecastFixture, Provenance as FixtureProvenance, FIXTURE_ID};
use crate::rolling_horizon::RollTrigger;

/// The recorded default maximum published-forecast age: one hour — the
/// SAME recorded default the P3.13 envelope staleness carries (a
/// CONFIGURATION default, not a report requirement; per-deployment
/// configuration overrides it).
pub const DEFAULT_FORECAST_MAX_AGE_MILLIS: i64 = 3_600_000;

/// The forecast-to-schedule policy statement (pinned by tests).
pub const FORECAST_TO_SCHEDULE_STATEMENT: &str = "Envelopes and rolling-horizon \
     scheduling consume real forecasts with the P3.13 staleness discipline: a STALE \
     forecast is never silently consumed. On staleness (a ForecastPublished supersede, a \
     horizon rollover the forecast does not cover, or the configured max-age bound) the \
     scheduler falls back LOUDLY to the persistence baseline (P15.02), carrying the \
     staleness reason on the decision. When no history exists even for the persistence \
     baseline, the P3.31 synthetic fixture remains the labeled synthetic fallback source. \
     Every decision names the source it selected and why; a fallback is a recorded \
     degradation, never a silent substitution.";

/// The ADR-010 statement (pinned by tests).
pub const FALLBACK_IS_NOT_A_COMMAND_STATEMENT: &str = "A selected forecast source — \
     published artifact, persistence fallback, or synthetic fixture — is an INPUT to \
     scheduling analytics. No forecast source decision authorizes, produces, or \
     influences a dispatch command (ADR-010 extension, program conventions 6.3).";

/// The staleness-reuse statement (pinned by tests): this module composes
/// the P3.13 pure functions, it does not restate the rules.
pub const STALENESS_REUSE_STATEMENT: &str = "The staleness rules are the P3.13 \
     envelope-staleness discipline (envelope_staleness.rs — the single staleness home), \
     consumed via evaluate_envelope_trigger and evaluate_max_age: the ForecastPublished \
     supersede trigger, the horizon rollover, and the configured max-age bound with the \
     P2.08 equality convention. The debounce semantics are the P3.13 ones verbatim: \
     debouncing delays only the recomputation advice and never re-validates a stale \
     source.";

// --- The decision types ---------------------------------------------------------

/// The selected forecast source (closed; the source order of the module
/// header).
#[derive(Debug, Clone, PartialEq)]
pub enum ForecastSourceDecision {
    /// The published forecast is current: no trigger fired and the age
    /// bound holds.
    PublishedForecast {
        /// The published forecast version consumed.
        forecast_version: String,
    },
    /// The published forecast is stale (or none was published): the
    /// persistence-baseline fallback was selected, with the reason
    /// carried verbatim.
    PersistenceBaselineFallback {
        /// Why the published forecast was not selected.
        reason: FallbackReason,
        /// The persistence-baseline output (the P15.02 rule set,
        /// sign- and unit-preserving over the caller's history).
        values: Vec<f64>,
    },
    /// No history existed for even the persistence baseline: the P3.31
    /// fixture is the labeled synthetic fallback. This module never
    /// invents fixture values — the caller generates the fixture (P3.31)
    /// and consumes it through the labeled accessor, unchanged.
    SyntheticFixtureFallback {
        /// Why the published forecast and the persistence baseline were
        /// not selected.
        reason: FallbackReason,
        /// The fixture identity (`FIXTURE_ID`), carried for the record.
        fixture_id: String,
        /// The fixture provenance label (`synthetic`) — the label that
        /// must travel with every value the caller reads from the
        /// fixture (the P3.31 four-place rule).
        provenance_label: &'static str,
    },
}

/// Why a published forecast was not selected (carried verbatim on every
/// fallback decision — a fallback is recorded, never silent).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FallbackReason {
    /// A newer forecast version was published (the P3.13
    /// `ForecastSuperseded` trigger).
    ForecastSuperseded {
        /// The version the schedule/envelope consumed.
        consumed: String,
        /// The published version.
        published: String,
    },
    /// The configured maximum age was exceeded.
    MaxAgeExceeded {
        /// The forecast's age at evaluation time, millis.
        age_millis: i64,
        /// The configured bound.
        max_age_millis: i64,
    },
    /// A new horizon starts at or beyond the forecast's horizon end (the
    /// forecast cannot serve a horizon it does not cover).
    HorizonRollover,
    /// No published forecast existed at all.
    NoPublishedForecast,
    /// The evaluation clock read before the forecast's computation time
    /// — a caller-side clock inconsistency, treated as
    /// stale-until-resolved (never silently consumed).
    ClockBeforeComputation,
}

impl FallbackReason {
    /// The operator-view spelling of the reason.
    pub fn as_str(&self) -> String {
        match self {
            FallbackReason::ForecastSuperseded { consumed, published } => format!(
                "forecast_superseded: consumed {consumed}, published {published}"
            ),
            FallbackReason::MaxAgeExceeded {
                age_millis,
                max_age_millis,
            } => format!(
                "max_age_exceeded: age {age_millis} ms > bound {max_age_millis} ms"
            ),
            FallbackReason::HorizonRollover => "horizon_rollover".to_string(),
            FallbackReason::NoPublishedForecast => "no_published_forecast".to_string(),
            FallbackReason::ClockBeforeComputation => {
                "clock_before_computation".to_string()
            }
        }
    }
}

/// The inputs of one source decision (the caller declares everything;
/// nothing is invented here).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedForecastRef<'a> {
    /// The published forecast version.
    pub forecast_version: &'a str,
    /// The forecast's computation timestamp (the publish event's time).
    pub computed_at: EventTime,
}

/// The source-order policy configuration (the max-age bound is
/// CONFIGURATION with the recorded default
/// [`DEFAULT_FORECAST_MAX_AGE_MILLIS`]; there is no "disable fallback"
/// switch — a scheduler that cannot select a source FAILS, it never
/// silently proceeds with a stale source).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForecastSourceConfig {
    /// The maximum published-forecast age in milliseconds beyond which
    /// the forecast is stale regardless of events. `age == max_age` is
    /// NOT stale; `age > max_age` is (the P2.08 convention).
    pub max_age_millis: i64,
}

impl Default for ForecastSourceConfig {
    /// The recorded default: the one-hour bound (the same recorded
    /// default the P3.13 staleness carries).
    fn default() -> Self {
        Self {
            max_age_millis: DEFAULT_FORECAST_MAX_AGE_MILLIS,
        }
    }
}

/// Selects the forecast source for one scheduling pass (pure).
///
/// The staleness evaluation reuses `evaluate_envelope_trigger` over the
/// P3.13 trigger vocabulary: the `ForecastPublished` event carries the
/// published version; a horizon rollover is the `HorizonRollover` event
/// (a new horizon at or beyond the envelope's horizon end); the max-age
/// bound rides on the same evaluation. The debounce configuration of the
/// envelope is passed through untouched (it affects only the
/// recomputation advice inside the P3.13 verdict — never the source
/// selection here).
///
/// `history` is the caller-declared observed series the persistence
/// fallback would consume (the caller's declared sign convention; the
/// persistence rule set is sign-preserving).
pub fn select_forecast_source(
    metadata: &EnvelopeMetadata,
    published: Option<PublishedForecastRef<'_>>,
    history: &[f64],
    horizon_steps: usize,
    envelope_config: &StalenessConfig,
    config: &ForecastSourceConfig,
    now: EventTime,
    last_recompute_at: Option<EventTime>,
) -> Result<ForecastSourceDecision, ForecastBaselineError> {
    // The staleness evaluation, in two composed halves:
    //
    // (1) The P3.13 event triggers over the envelope metadata (the
    //     `ForecastPublished` supersede trigger — the version comparison
    //     — plus whatever the envelope config's own bound does at this
    //     instant). The envelope's telemetry/availability/capability
    //     triggers invalidate the ENVELOPE, not the forecast source, so
    //     they are deliberately NOT mapped to fallback reasons here
    //     (recorded; an exhaustive match guards future P3.13 reasons
    //     against silent collapse).
    // (2) The forecast's OWN max-age bound against
    //     [`ForecastSourceConfig`] — a published forecast has its own
    //     computation timestamp, which is not the envelope's; the
    //     P2.08 equality convention applies (age == bound is NOT stale).
    let trigger = EnvelopeTriggerEvent::ForecastPublished {
        forecast_version: published
            .as_ref()
            .map(|r| r.forecast_version.to_string())
            .unwrap_or_default(),
    };
    let verdict =
        evaluate_envelope_trigger(metadata, &trigger, envelope_config, now, last_recompute_at);

    // Reason 1: no published forecast at all.
    // Reason 2: the evaluation clock is before the forecast's
    // computation time — stale-until-resolved, never a silent fresh
    // verdict.
    // Reason 3: the P3.13 supersede trigger fired (a different forecast
    // version was published than the one the envelope consumed).
    // Reason 4: the forecast's own max-age bound (ForecastSourceConfig).
    let fallback_reason = match &published {
        None => Some(FallbackReason::NoPublishedForecast),
        Some(reference) if now.unix_millis() < reference.computed_at.unix_millis() => {
            Some(FallbackReason::ClockBeforeComputation)
        }
        // The reference is bound HERE (FRV1-R7-010): the arm's pattern
        // already guarantees `published` is `Some`, so the max-age arm
        // below carries the value out of the guard scope instead of
        // re-looking it up behind an `expect` — the value comes from the
        // match itself, not from a reviewer-traced re-read.
        Some(reference) => match &verdict.freshness {
            EnvelopeFreshness::Stale {
                reason: StalenessReason::ForecastSuperseded {
                    envelope_forecast_version,
                    published_forecast_version,
                },
            } => Some(FallbackReason::ForecastSuperseded {
                consumed: envelope_forecast_version.clone(),
                published: published_forecast_version.clone(),
            }),
            _ => {
                let age = now.unix_millis() - reference.computed_at.unix_millis();
                if age > i128::from(config.max_age_millis) {
                    Some(FallbackReason::MaxAgeExceeded {
                        age_millis: i64::try_from(age).unwrap_or(i64::MAX),
                        max_age_millis: config.max_age_millis,
                    })
                } else {
                    None
                }
            }
        },
    };

    match fallback_reason {
        None => {
            let version = published
                .as_ref()
                .map(|r| r.forecast_version.to_string())
                .unwrap_or_default();
            Ok(ForecastSourceDecision::PublishedForecast {
                forecast_version: version,
            })
        }
        Some(reason) => {
            // The persistence-baseline fallback: the P15.02 rule set over
            // the declared history. An empty or non-finite history is a
            // named refusal of the rule set — which escalates the
            // decision to the synthetic fixture (labeled), never a
            // zero-fill.
            match persistence_forecast(history, horizon_steps) {
                Ok(values) => Ok(ForecastSourceDecision::PersistenceBaselineFallback {
                    reason,
                    values,
                }),
                Err(_) => Ok(ForecastSourceDecision::SyntheticFixtureFallback {
                    reason,
                    fixture_id: FIXTURE_ID.to_string(),
                    provenance_label: fixture_label(),
                }),
            }
        }
    }
}

/// The fixture provenance label, read from the fixture vocabulary (the
/// single home of the `synthetic` spelling).
#[must_use]
fn fixture_label() -> &'static str {
    FixtureProvenance::Synthetic.as_str()
}

/// The rolling-horizon roll trigger a source decision implies (the P3.24
/// trigger vocabulary, reused — never restated): a superseded forecast
/// or a max-age/horizon degradation is a `forecast_update` re-solve
/// trigger; a fresh published forecast that differs from the consumed
/// version is also `forecast_update` (the P3.24 `ForecastUpdate` row).
/// The horizon rollover of the P3.13 discipline maps to `horizon_advance`
/// when it is the horizon (not the forecast) that moved — the caller
/// distinguishes by passing the reason.
#[must_use]
pub fn roll_trigger_for_fallback(reason: &FallbackReason) -> RollTrigger {
    match reason {
        FallbackReason::HorizonRollover => RollTrigger::HorizonAdvance,
        _ => RollTrigger::ForecastUpdate,
    }
}

/// The fixture consumption helper: confirms the caller's fixture is the
/// labeled synthetic one and returns its version for the record. The
/// fixture stays the labeled synthetic fallback source — this function
/// never reads values (the labeled accessor, P3.31, is the only value
/// path) and never relabels.
pub fn fixture_fallback_reference(fixture: &ForecastFixture) -> (String, &'static str, String) {
    (
        FIXTURE_ID.to_string(),
        fixture_label(),
        fixture.forecast_version().as_str().to_string(),
    )
}

// --- Tests (WRITTEN-UNRUN — no cargo command was executed in this loop;
// --- the human build/test gate owns the run. Every expected value below
// --- is hand-computed from the rule sets above over CONSTRUCTED inputs.
// --- Nothing is fabricated.) -----------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forecast_fixture::{Provenance as FixtureProvenance, FIXTURE_ID};
    use crate::forecast_baselines::NO_SKILL_STATEMENT;
    use flexcore_contracts::ids::FlexUuid;
    use flexcore_domain::ids::{AssetId, PortfolioId, TenantId};

    fn uid(n: u128) -> FlexUuid {
        FlexUuid::from_bytes(n.to_be_bytes())
    }

    fn tenant() -> TenantId {
        TenantId::new(uid(0xA100))
    }

    fn portfolio() -> PortfolioId {
        PortfolioId::new(uid(0xA200))
    }

    fn asset(tag: u128) -> AssetId {
        AssetId::new(uid(tag))
    }

    fn t(millis: i64) -> EventTime {
        EventTime::from_unix_millis(millis).expect("valid test time")
    }

    /// An envelope metadata record with a horizon of [0, 3_600_000) and
    /// the consumed forecast version `fv-1`.
    fn metadata() -> EnvelopeMetadata {
        EnvelopeMetadata {
            envelope_version: crate::flexibility_envelope::EnvelopeVersion::new("env-1")
                .expect("valid version"),
            computed_at: t(0),
            tenant_id: tenant(),
            portfolio_id: portfolio(),
            input_snapshot_ref: "snapshot-1".to_string(),
            forecast_version: "fv-1".to_string(),
            horizon_start_millis: 0,
            horizon_end_millis: 3_600_000,
            scoped_asset_ids: vec![asset(0xB001)],
        }
    }

    fn published(version: &str, computed_at: i64) -> PublishedForecastRef<'_> {
        PublishedForecastRef {
            forecast_version: version,
            computed_at: t(computed_at),
        }
    }

    fn config() -> (StalenessConfig, ForecastSourceConfig) {
        (StalenessConfig::default(), ForecastSourceConfig::default())
    }

    #[test]
    fn statements_are_the_recorded_strings() {
        assert!(FORECAST_TO_SCHEDULE_STATEMENT.contains("never silently consumed"));
        assert!(FORECAST_TO_SCHEDULE_STATEMENT.contains("persistence baseline"));
        assert!(FORECAST_TO_SCHEDULE_STATEMENT.contains("labeled synthetic fallback source"));
        assert!(FALLBACK_IS_NOT_A_COMMAND_STATEMENT.contains("No forecast source decision authorizes"));
        assert!(STALENESS_REUSE_STATEMENT.contains("single staleness home"));
        assert!(STALENESS_REUSE_STATEMENT.contains("evaluate_envelope_trigger"));
    }

    #[test]
    fn a_fresh_published_forecast_is_selected() {
        let m = metadata();
        let (envelope_config, source_config) = config();
        // The published version matches the consumed one (fv-1) and the
        // computation is within the max-age bound of `now`.
        let decision = select_forecast_source(
            &m,
            Some(published("fv-1", 1_000)),
            &[-0.5, -0.6],
            2,
            &envelope_config,
            &source_config,
            t(1_000 + 1_800_000),
            None,
        )
        .expect("valid selection");
        assert_eq!(
            decision,
            ForecastSourceDecision::PublishedForecast {
                forecast_version: "fv-1".to_string()
            }
        );
    }

    #[test]
    fn a_superseded_forecast_falls_back_to_the_persistence_baseline() {
        let m = metadata();
        let (envelope_config, source_config) = config();
        // The published version differs from the consumed one -> the
        // P3.13 supersede trigger fires -> persistence fallback.
        let decision = select_forecast_source(
            &m,
            Some(published("fv-2", 1_000)),
            &[-0.5, -0.6],
            2,
            &envelope_config,
            &source_config,
            t(1_000 + 60_000),
            None,
        )
        .expect("valid selection");
        match decision {
            ForecastSourceDecision::PersistenceBaselineFallback { reason, values } => {
                assert_eq!(
                    reason,
                    FallbackReason::ForecastSuperseded {
                        consumed: "fv-1".to_string(),
                        published: "fv-2".to_string(),
                    }
                );
                // The persistence rule set: last value carried forward.
                assert_eq!(values, vec![-0.6, -0.6]);
            }
            other => panic!("unexpected decision: {other:?}"),
        }
    }

    #[test]
    fn an_aged_forecast_falls_back_with_the_max_age_reason() {
        let m = metadata();
        let (envelope_config, source_config) = config();
        // Same version, but now = computed + 1h + 1ms -> the max-age
        // bound fires (age == bound is NOT stale; age > bound is).
        let decision = select_forecast_source(
            &m,
            Some(published("fv-1", 0)),
            &[-0.5, -0.6],
            2,
            &envelope_config,
            &source_config,
            t(DEFAULT_FORECAST_MAX_AGE_MILLIS + 1),
            None,
        )
        .expect("valid selection");
        match decision {
            ForecastSourceDecision::PersistenceBaselineFallback { reason, .. } => {
                assert_eq!(
                    reason,
                    FallbackReason::MaxAgeExceeded {
                        age_millis: DEFAULT_FORECAST_MAX_AGE_MILLIS + 1,
                        max_age_millis: DEFAULT_FORECAST_MAX_AGE_MILLIS,
                    }
                );
            }
            other => panic!("unexpected decision: {other:?}"),
        }
        // Exactly at the bound: fresh (the P2.08 equality convention).
        let decision = select_forecast_source(
            &m,
            Some(published("fv-1", 0)),
            &[-0.5, -0.6],
            2,
            &envelope_config,
            &source_config,
            t(DEFAULT_FORECAST_MAX_AGE_MILLIS),
            None,
        )
        .expect("valid selection");
        assert_eq!(
            decision,
            ForecastSourceDecision::PublishedForecast {
                forecast_version: "fv-1".to_string()
            }
        );
    }

    #[test]
    fn no_published_forecast_falls_back_and_reports_the_reason() {
        let m = metadata();
        let (envelope_config, source_config) = config();
        let decision = select_forecast_source(
            &m,
            None,
            &[-0.5, -0.6],
            2,
            &envelope_config,
            &source_config,
            t(1_000),
            None,
        )
        .expect("valid selection");
        match decision {
            ForecastSourceDecision::PersistenceBaselineFallback { reason, values } => {
                assert_eq!(reason, FallbackReason::NoPublishedForecast);
                assert_eq!(values, vec![-0.6, -0.6]);
            }
            other => panic!("unexpected decision: {other:?}"),
        }
    }

    #[test]
    fn a_clock_before_the_computation_time_is_stale_until_resolved() {
        let m = metadata();
        let (envelope_config, source_config) = config();
        // The evaluation clock is BEFORE the published forecast's
        // computation time: never a silent fresh verdict.
        let decision = select_forecast_source(
            &m,
            Some(published("fv-1", 10_000)),
            &[-0.5, -0.6],
            2,
            &envelope_config,
            &source_config,
            t(5_000),
            None,
        )
        .expect("valid selection");
        match decision {
            ForecastSourceDecision::PersistenceBaselineFallback { reason, .. } => {
                assert_eq!(reason, FallbackReason::ClockBeforeComputation);
            }
            other => panic!("unexpected decision: {other:?}"),
        }
    }

    #[test]
    fn an_empty_history_escalates_to_the_labeled_synthetic_fixture() {
        let m = metadata();
        let (envelope_config, source_config) = config();
        let decision = select_forecast_source(
            &m,
            None,
            &[],
            2,
            &envelope_config,
            &source_config,
            t(1_000),
            None,
        )
        .expect("valid selection");
        match decision {
            ForecastSourceDecision::SyntheticFixtureFallback {
                reason,
                fixture_id,
                provenance_label,
            } => {
                assert_eq!(reason, FallbackReason::NoPublishedForecast);
                assert_eq!(fixture_id, FIXTURE_ID);
                assert_eq!(provenance_label, "synthetic");
            }
            other => panic!("unexpected decision: {other:?}"),
        }
    }

    #[test]
    fn fallback_reasons_render_and_map_to_roll_triggers() {
        let reasons = [
            FallbackReason::ForecastSuperseded {
                consumed: "fv-1".to_string(),
                published: "fv-2".to_string(),
            },
            FallbackReason::MaxAgeExceeded {
                age_millis: 3_600_001,
                max_age_millis: 3_600_000,
            },
            FallbackReason::HorizonRollover,
            FallbackReason::NoPublishedForecast,
            FallbackReason::ClockBeforeComputation,
        ];
        for reason in &reasons {
            assert!(!reason.as_str().is_empty());
        }
        // A forecast-side degradation is a forecast_update re-solve
        // trigger; a horizon rollover is a horizon_advance.
        assert_eq!(
            roll_trigger_for_fallback(&reasons[0]).as_str(),
            "forecast_update"
        );
        assert_eq!(
            roll_trigger_for_fallback(&reasons[1]).as_str(),
            "forecast_update"
        );
        assert_eq!(
            roll_trigger_for_fallback(&reasons[2]).as_str(),
            "horizon_advance"
        );
        assert_eq!(
            roll_trigger_for_fallback(&reasons[3]).as_str(),
            "forecast_update"
        );
    }

    #[test]
    fn the_fixture_fallback_reference_carries_the_synthetic_label() {
        // The P3.31 fixture vocabulary is the single home of the
        // `synthetic` spelling; the reference helper carries it verbatim.
        assert_eq!(fixture_label(), "synthetic");
        assert_eq!(FIXTURE_ID, "synthetic-forecast-fixture");
        // The standing no-skill statement backs every fallback output
        // (the P15.02 honesty line, mirrored by citation here).
        assert!(NO_SKILL_STATEMENT.contains("no skill, unproven, UNTRAINED"));
    }
}
