// P3.13 — Envelope staleness and recomputation triggers: acceptance tests
// (ledger P3.13).
//
// Covers the ledger acceptance criteria:
//
// - EACH listed trigger invalidates the envelope in tests:
//   `TelemetryQualityChanged` (the ledger's named test), `AssetAvailability
//   Changed`, `AssetCapabilityUpdated`, `ForecastPublished`, horizon
//   rollover, and the configured maximum age bound;
// - every envelope carries the version, computation timestamp, and
//   input-snapshot reference (built by the P3.12 calculator and asserted
//   through the metadata);
// - the schedule-envelope reference type is exposed and a schedule
//   referencing a superseded envelope version is rejected; a stale
//   envelope is rejected for a new schedule (recompute-or-fail-loudly —
//   the enforcement wiring lands P3.22/P3.24);
// - the recorded debounce configuration defers only the recomputation
//   advice and can never weaken the staleness guarantee (documented
//   invariant, tested directly).
//
// NOTE (VERIFY-BUILD, recorded): per the authoring constraint, cargo was
// not run; the first `cargo test -p flexcore-application --test
// envelope_staleness` run must confirm.

use flexcore_application::envelope_staleness::{
    assert_usable_for_new_schedule, check_schedule_envelope_reference, evaluate_envelope_trigger,
    evaluate_max_age, DEBOUNCE_GUARANTEE_STATEMENT, DEFAULT_MAX_AGE_MILLIS, EnvelopeFreshness,
    EnvelopeReferenceError, EnvelopeTriggerEvent, RecomputationAdvice, ScheduleEnvelopeReference,
    StalenessConfig, StalenessReason, STALE_ENVELOPE_POLICY,
};
use flexcore_application::flexibility_envelope::EnvelopeMetadata;
use flexcore_application::flexibility_envelope::EnvelopeVersion;
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::time_model::EventTime;
use flexcore_domain::ids::{AssetId, PortfolioId, TenantId};
use flexcore_events::events::{
    AssetAvailabilityChangedPayload, AssetCapabilityUpdatedPayload, TelemetryQualityChangedPayload,
};

const HORIZON_START_MILLIS: i128 = 1_780_272_000_000;

fn uid(n: u128) -> FlexUuid {
    FlexUuid::from_bytes(n.to_be_bytes())
}

fn tenant() -> TenantId {
    TenantId::new(uid(0xA100))
}

fn portfolio() -> PortfolioId {
    PortfolioId::new(uid(0xA200))
}

fn asset_a() -> AssetId {
    AssetId::new(uid(0xB001))
}

fn asset_foreign() -> AssetId {
    AssetId::new(uid(0xF001))
}

/// A fresh envelope metadata record computed at T0 = the horizon start.
fn metadata(computed_at_millis: i64) -> EnvelopeMetadata {
    EnvelopeMetadata {
        envelope_version: EnvelopeVersion::new("env-v1").expect("valid version"),
        computed_at: EventTime::from_unix_millis(computed_at_millis).expect("valid timestamp"),
        tenant_id: tenant(),
        portfolio_id: portfolio(),
        input_snapshot_ref: "envelope_input_snapshot:synthetic-forecast-fixture:v1:assets=2"
            .to_string(),
        forecast_version: "v1".to_string(),
        horizon_start_millis: i64::try_from(HORIZON_START_MILLIS).expect("in range"),
        horizon_end_millis: i64::try_from(HORIZON_START_MILLIS + 86_400_000).expect("in range"),
        scoped_asset_ids: vec![asset_a(), AssetId::new(uid(0xB002))],
    }
}

fn at(millis: i64) -> EventTime {
    EventTime::from_unix_millis(millis).expect("valid timestamp")
}

fn quality_event(asset: AssetId, previous: TelemetryQuality, new: TelemetryQuality) -> EnvelopeTriggerEvent {
    EnvelopeTriggerEvent::TelemetryQualityChanged(TelemetryQualityChangedPayload {
        correlation_id: uid(0xC001),
        tenant_id: tenant(),
        site_id: flexcore_domain::ids::SiteId::new(uid(0xA300)),
        asset_id: asset,
        quality: new,
        previous_quality: Some(previous),
        event_time: at(HORIZON_START_MILLIS as i64 + 120_000),
    })
}

fn default_config() -> StalenessConfig {
    StalenessConfig::default()
}

// --- Every listed trigger invalidates (ledger acceptance) ---------------------

/// The ledger's named test: a `TelemetryQualityChanged` event invalidates
/// the current envelope. DECIDED: ANY quality transition on a scoped asset
/// invalidates, in either direction (the non-silencing rule, §10.4).
#[test]
fn a_telemetry_quality_change_invalidates_the_current_envelope() {
    let md = metadata(HORIZON_START_MILLIS as i64);
    let now = at(HORIZON_START_MILLIS as i64 + 120_000);

    // GOOD -> SUSPECT on a scoped asset: stale.
    let verdict = evaluate_envelope_trigger(
        &md,
        &quality_event(asset_a(), TelemetryQuality::Good, TelemetryQuality::Suspect),
        &default_config(),
        now,
        None,
    );
    assert_eq!(
        verdict.freshness,
        EnvelopeFreshness::Stale {
            reason: StalenessReason::TelemetryQualityChanged {
                asset_id: asset_a().to_string()
            }
        }
    );
    assert_eq!(verdict.recomputation, RecomputationAdvice::Immediate);

    // A quality IMPROVEMENT (SUSPECT -> GOOD) invalidates too: the recorded
    // computation's input basis changed; a flag is never silently
    // converted (§10.4).
    let verdict = evaluate_envelope_trigger(
        &md,
        &quality_event(asset_a(), TelemetryQuality::Suspect, TelemetryQuality::Good),
        &default_config(),
        now,
        None,
    );
    assert!(matches!(verdict.freshness, EnvelopeFreshness::Stale { .. }));

    // BAD and MISSING poison the envelope as well.
    for new in [TelemetryQuality::Bad, TelemetryQuality::Missing, TelemetryQuality::Stale] {
        let verdict = evaluate_envelope_trigger(
            &md,
            &quality_event(asset_a(), TelemetryQuality::Good, new),
            &default_config(),
            now,
            None,
        );
        assert!(matches!(verdict.freshness, EnvelopeFreshness::Stale { .. }));
    }
}

#[test]
fn a_quality_change_on_a_foreign_asset_or_tenant_does_not_invalidate() {
    let md = metadata(HORIZON_START_MILLIS as i64);
    let now = at(HORIZON_START_MILLIS as i64 + 120_000);

    // Out-of-scope asset.
    let verdict = evaluate_envelope_trigger(
        &md,
        &quality_event(asset_foreign(), TelemetryQuality::Good, TelemetryQuality::Bad),
        &default_config(),
        now,
        None,
    );
    assert_eq!(verdict.freshness, EnvelopeFreshness::Fresh);

    // Foreign tenant: the same asset, another tenant — never invalidates
    // (P0.16 rule 5).
    let foreign = TelemetryQualityChangedPayload {
        correlation_id: uid(0xC001),
        tenant_id: TenantId::new(uid(0xDEAD)),
        site_id: flexcore_domain::ids::SiteId::new(uid(0xA300)),
        asset_id: asset_a(),
        quality: TelemetryQuality::Bad,
        previous_quality: Some(TelemetryQuality::Good),
        event_time: at(HORIZON_START_MILLIS as i64 + 120_000),
    };
    let verdict = evaluate_envelope_trigger(
        &md,
        &EnvelopeTriggerEvent::TelemetryQualityChanged(foreign),
        &default_config(),
        now,
        None,
    );
    assert_eq!(verdict.freshness, EnvelopeFreshness::Fresh);
}

#[test]
fn an_availability_change_invalidates() {
    let md = metadata(HORIZON_START_MILLIS as i64);
    let now = at(HORIZON_START_MILLIS as i64 + 120_000);
    let verdict = evaluate_envelope_trigger(
        &md,
        &EnvelopeTriggerEvent::AssetAvailabilityChanged(AssetAvailabilityChangedPayload {
            correlation_id: uid(0xC001),
            tenant_id: tenant(),
            site_id: flexcore_domain::ids::SiteId::new(uid(0xA300)),
            asset_id: asset_a(),
            event_time: at(HORIZON_START_MILLIS as i64 + 120_000),
        }),
        &default_config(),
        now,
        None,
    );
    assert_eq!(
        verdict.freshness,
        EnvelopeFreshness::Stale {
            reason: StalenessReason::AssetAvailabilityChanged {
                asset_id: asset_a().to_string()
            }
        }
    );
}

#[test]
fn a_capability_update_invalidates() {
    let md = metadata(HORIZON_START_MILLIS as i64);
    let now = at(HORIZON_START_MILLIS as i64 + 120_000);
    let verdict = evaluate_envelope_trigger(
        &md,
        &EnvelopeTriggerEvent::AssetCapabilityUpdated(AssetCapabilityUpdatedPayload {
            correlation_id: uid(0xC001),
            tenant_id: tenant(),
            site_id: flexcore_domain::ids::SiteId::new(uid(0xA300)),
            asset_id: asset_a(),
            event_time: at(HORIZON_START_MILLIS as i64 + 120_000),
        }),
        &default_config(),
        now,
        None,
    );
    assert_eq!(
        verdict.freshness,
        EnvelopeFreshness::Stale {
            reason: StalenessReason::AssetCapabilityUpdated {
                asset_id: asset_a().to_string()
            }
        }
    );
}

#[test]
fn a_new_forecast_version_invalidates_and_the_same_version_does_not() {
    let md = metadata(HORIZON_START_MILLIS as i64);
    let now = at(HORIZON_START_MILLIS as i64 + 120_000);

    // A different forecast version: stale (the envelope consumed v1).
    let verdict = evaluate_envelope_trigger(
        &md,
        &EnvelopeTriggerEvent::ForecastPublished {
            forecast_version: "v1-error-pv-ramp".to_string(),
        },
        &default_config(),
        now,
        None,
    );
    assert_eq!(
        verdict.freshness,
        EnvelopeFreshness::Stale {
            reason: StalenessReason::ForecastSuperseded {
                envelope_forecast_version: "v1".to_string(),
                published_forecast_version: "v1-error-pv-ramp".to_string(),
            }
        }
    );

    // The SAME version republished: fresh (durable dedupe on the outbox
    // key; the D-FX-9 gap note records where the version comes from).
    let verdict = evaluate_envelope_trigger(
        &md,
        &EnvelopeTriggerEvent::ForecastPublished {
            forecast_version: "v1".to_string(),
        },
        &default_config(),
        now,
        None,
    );
    assert_eq!(verdict.freshness, EnvelopeFreshness::Fresh);
}

#[test]
fn a_horizon_rollover_beyond_the_envelope_invalidates() {
    let md = metadata(HORIZON_START_MILLIS as i64);

    // A new horizon starting at or after the envelope's end: stale.
    let verdict = evaluate_envelope_trigger(
        &md,
        &EnvelopeTriggerEvent::HorizonRollover {
            new_horizon_start_millis: i64::try_from(HORIZON_START_MILLIS + 86_400_000)
                .expect("in range"),
        },
        &default_config(),
        at(HORIZON_START_MILLIS as i64 + 120_000),
        None,
    );
    assert_eq!(
        verdict.freshness,
        EnvelopeFreshness::Stale { reason: StalenessReason::HorizonRollover }
    );

    // A new horizon still inside the envelope's horizon: fresh.
    let verdict = evaluate_envelope_trigger(
        &md,
        &EnvelopeTriggerEvent::HorizonRollover {
            new_horizon_start_millis: i64::try_from(HORIZON_START_MILLIS + 3_600_000)
                .expect("in range"),
        },
        &default_config(),
        at(HORIZON_START_MILLIS as i64 + 120_000),
        None,
    );
    assert_eq!(verdict.freshness, EnvelopeFreshness::Fresh);
}

#[test]
fn the_configured_maximum_age_bound_is_enforced() {
    let computed = HORIZON_START_MILLIS as i64;
    let md = metadata(computed);

    // Age == limit: NOT stale (the P2.08 staleness convention).
    let config = StalenessConfig { max_age_millis: 1_000, min_recompute_interval_millis: 0 };
    assert_eq!(
        evaluate_max_age(&md, &config, at(computed + 1_000)),
        EnvelopeFreshness::Fresh
    );
    // Limit + 1 ms: stale.
    assert_eq!(
        evaluate_max_age(&md, &config, at(computed + 1_001)),
        EnvelopeFreshness::Stale {
            reason: StalenessReason::MaxAgeExceeded { age_millis: 1_001, max_age_millis: 1_000 }
        }
    );
    // A clock read before the computation timestamp is not stale.
    assert_eq!(evaluate_max_age(&md, &config, at(computed - 1)), EnvelopeFreshness::Fresh);

    // The recorded default bound is the configuration default.
    assert_eq!(StalenessConfig::default().max_age_millis, DEFAULT_MAX_AGE_MILLIS);

    // The trigger evaluation applies the age bound as well: an old,
    // event-less envelope is stale through the same function.
    let verdict = evaluate_envelope_trigger(
        &md,
        &EnvelopeTriggerEvent::ForecastPublished { forecast_version: "v1".to_string() },
        &StalenessConfig { max_age_millis: 60_000, min_recompute_interval_millis: 0 },
        at(computed + 60_001),
        None,
    );
    assert_eq!(
        verdict.freshness,
        EnvelopeFreshness::Stale {
            reason: StalenessReason::MaxAgeExceeded { age_millis: 60_001, max_age_millis: 60_000 }
        }
    );
}

// --- The schedule gates (P3.13 step 4) ----------------------------------------

#[test]
fn a_stale_envelope_is_never_usable_for_a_new_schedule() {
    let computed = HORIZON_START_MILLIS as i64;
    let md = metadata(computed);
    let config = StalenessConfig { max_age_millis: 1_000, min_recompute_interval_millis: 0 };

    // Fresh: usable.
    assert_eq!(assert_usable_for_new_schedule(&md, &config, at(computed + 1_000)), Ok(()));

    // Stale (max age): rejected with the reason — recompute or fail
    // loudly, never silently proceed.
    assert_eq!(
        assert_usable_for_new_schedule(&md, &config, at(computed + 1_001)),
        Err(EnvelopeReferenceError::StaleEnvelopeRejected {
            reason: StalenessReason::MaxAgeExceeded { age_millis: 1_001, max_age_millis: 1_000 }
        })
    );

    // The policy statement is a visible constant.
    assert!(STALE_ENVELOPE_POLICY.contains("never used for a new schedule"));
}

#[test]
fn a_schedule_referencing_a_superseded_envelope_version_is_rejected() {
    let md = metadata(HORIZON_START_MILLIS as i64);

    // The current version: accepted.
    let current = ScheduleEnvelopeReference::new(EnvelopeVersion::new("env-v1").unwrap());
    assert_eq!(check_schedule_envelope_reference(&current, &md), Ok(()));

    // An older version: superseded — rejected (or re-derived by the
    // caller); never silently used.
    let superseded = ScheduleEnvelopeReference::new(EnvelopeVersion::new("env-v0").unwrap());
    assert_eq!(
        check_schedule_envelope_reference(&superseded, &md),
        Err(EnvelopeReferenceError::EnvelopeSuperseded {
            referenced: "env-v0".to_string(),
            current: "env-v1".to_string(),
        })
    );
}

// --- Debouncing (the recorded risk mitigation) ---------------------------------

#[test]
fn the_debounce_window_defers_only_the_recomputation_advice() {
    let computed = HORIZON_START_MILLIS as i64;
    let md = metadata(computed);
    let trigger_at = computed + 120_000;
    let config = StalenessConfig { max_age_millis: 3_600_000, min_recompute_interval_millis: 60_000 };

    // Triggered with an open debounce window (last recompute 30 s ago):
    // STALE regardless, but the advice is deferred to
    // last_recompute + window.
    let verdict = evaluate_envelope_trigger(
        &md,
        &quality_event(asset_a(), TelemetryQuality::Good, TelemetryQuality::Bad),
        &config,
        at(trigger_at),
        Some(at(trigger_at - 30_000)),
    );
    assert!(matches!(verdict.freshness, EnvelopeFreshness::Stale { .. }));
    assert_eq!(
        verdict.recomputation,
        RecomputationAdvice::Deferred { not_before_millis: trigger_at + 30_000 }
    );

    // The window elapsed: immediate.
    let verdict = evaluate_envelope_trigger(
        &md,
        &quality_event(asset_a(), TelemetryQuality::Good, TelemetryQuality::Bad),
        &config,
        at(trigger_at),
        Some(at(trigger_at - 60_000)),
    );
    assert_eq!(verdict.recomputation, RecomputationAdvice::Immediate);

    // No last recompute recorded: immediate.
    let verdict = evaluate_envelope_trigger(
        &md,
        &quality_event(asset_a(), TelemetryQuality::Good, TelemetryQuality::Bad),
        &config,
        at(trigger_at),
        None,
    );
    assert_eq!(verdict.recomputation, RecomputationAdvice::Immediate);

    // No debounce configured (the recorded default): immediate.
    let verdict = evaluate_envelope_trigger(
        &md,
        &quality_event(asset_a(), TelemetryQuality::Good, TelemetryQuality::Bad),
        &StalenessConfig { max_age_millis: 3_600_000, min_recompute_interval_millis: 0 },
        at(trigger_at),
        Some(at(trigger_at - 1)),
    );
    assert_eq!(verdict.recomputation, RecomputationAdvice::Immediate);
}

/// THE RECORDED DEBOUNCE GUARANTEE, tested directly: debounce delays only
/// the recomputation ADVICE. The hard gates — a stale envelope is never
/// used for a new schedule, and a superseded version is rejected — ignore
/// the debounce configuration entirely.
#[test]
fn the_debounce_cannot_weaken_the_staleness_guarantee() {
    let computed = HORIZON_START_MILLIS as i64;
    let md = metadata(computed);
    // An aggressive debounce window: recomputation is deferred for an hour.
    let config = StalenessConfig { max_age_millis: 60_000, min_recompute_interval_millis: 3_600_000 };
    let now = at(computed + 60_001);

    // The verdict is stale with a deferred advice...
    let verdict = evaluate_envelope_trigger(
        &md,
        &quality_event(asset_a(), TelemetryQuality::Good, TelemetryQuality::Bad),
        &config,
        now,
        Some(at(now.unix_millis() as i64 - 1)),
    );
    assert!(matches!(verdict.freshness, EnvelopeFreshness::Stale { .. }));
    assert!(matches!(verdict.recomputation, RecomputationAdvice::Deferred { .. }));

    // ...and the new-schedule gate STILL rejects: the deferred recompute
    // never re-validates the stale envelope. (The gate does not even take
    // the debounce configuration or the advice as arguments.)
    assert!(assert_usable_for_new_schedule(&md, &config, now).is_err());

    // The superseded-version gate is likewise debounce-independent.
    let older = ScheduleEnvelopeReference::new(EnvelopeVersion::new("env-v0").unwrap());
    assert!(check_schedule_envelope_reference(&older, &md).is_err());

    // The recorded statement names exactly the two guarantees.
    assert!(DEBOUNCE_GUARANTEE_STATEMENT.contains("never used for a new schedule"));
    assert!(DEBOUNCE_GUARANTEE_STATEMENT.contains("superseded envelope version is rejected"));
}

// --- Metadata completeness (ledger step 2) -------------------------------------

#[test]
fn every_envelope_metadata_carries_version_timestamp_and_snapshot_reference() {
    let computed = HORIZON_START_MILLIS as i64;
    let md = metadata(computed);
    assert_eq!(md.envelope_version.as_str(), "env-v1");
    assert_eq!(md.computed_at.unix_millis(), i128::from(computed));
    assert!(md.input_snapshot_ref.starts_with("envelope_input_snapshot:"));
    assert_eq!(md.forecast_version, "v1");
    // The schedule reference type wraps the validated version (P3.21
    // consumes this).
    let reference = ScheduleEnvelopeReference::new(md.envelope_version.clone());
    assert_eq!(reference.envelope_version, md.envelope_version);
}
