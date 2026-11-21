// FIM-F10.05 — pinned tests for the FIM-F10 vendor-ingestion surface
//             (integration level, over the worker crate's public lib API).
//
// HONESTY HEADER (FIM-F10.06): WRITTEN-UNRUN — no cargo command was
// executed in this loop; the human build/test gate owns the run. Every
// expected value below is a hand-derived property of the validation rules,
// a published SHA-256 test vector, or a float spelling confirmed with
// `python -I` (recorded in the FIM close-out). Nothing is fabricated; no
// skill, accuracy, or vendor-feed result is claimed anywhere. Vendor
// feeds and evaluation runs are HUMAN GATES (token registration;
// P15-E-1/P15-E-2 stay OPEN).
//
// The in-module tests in `src/vendor_ingestion.rs` cover the internals
// (response parsing, cache reclassification, refusal prose); this file
// pins the PUBLIC surface: the fail-closed config gate, the staleness
// classification boundaries, the declared unit conversions, provenance
// presence, the SHA-256 artifact digest chain (continuity + tamper
// refusal + replay identity), and the persistence-baseline fallback
// boundaries (fresh / max-age / max-age + 1 / no-forecast / clock-before /
// labeled synthetic escalation).

use flexcore_worker::vendor_ingestion::{
    artifact_chain_digest, artifact_payload_digest, classify_record_quality,
    render_values_canonical, select_ingested_forecast, sha256_hex, ArtifactChainError,
    ForecastArtifactChain, IngestedForecastDecision, IngestedForecastRef,
    IngestionFallbackReason, UnitConversion, VendorIngestionError, VendorToken,
    VendorWeatherConfig,
};
use flexcore_contracts::time_model::EventTime;
use flexcore_application::forecast_integration::DEFAULT_FORECAST_MAX_AGE_MILLIS;

fn t(millis: i64) -> EventTime {
    EventTime::from_unix_millis(millis).expect("fixture instant is representable")
}

// ---------------------------------------------------------------------------
// The fail-closed configuration gate (FIM-F10.01)
// ---------------------------------------------------------------------------

/// A lookup with EVERY variable present except the named exceptions.
// Batch 68: the elided `'_` was ambiguous (`exceptions` carries two
// lifetimes) — the named lifetime rustc suggests.
fn full_lookup_without<'a>(exceptions: &'a [&'a str]) -> impl Fn(&str) -> Option<String> + 'a {
    move |name: &str| {
        if exceptions.contains(&name) {
            return None;
        }
        match name {
            "FLEXCORE_VENDOR_WEATHER_URL" => Some("https://vendor.weather.example".to_string()),
            "FLEXCORE_VENDOR_WEATHER_TOKEN" => Some("fixture-token".to_string()),
            "FLEXCORE_VENDOR_WEATHER_SOURCE_REF" => {
                Some("weather://fixture/query".to_string())
            }
            "FLEXCORE_VENDOR_WEATHER_FIELD_MAP" => {
                Some("{\"ghi_vendor\": \"ghi\"}".to_string())
            }
            "FLEXCORE_VENDOR_WEATHER_UNIT_FACTORS" => {
                Some(
                    "{\"ghi\": {\"vendor_unit\": \"kW/m2\", \"factor\": 1000.0, \
                     \"factor_provenance\": \"fixture-unit-table\"}}"
                        .to_string(),
                )
            }
            _ => None,
        }
    }
}

#[test]
fn config_without_a_registered_token_refuses_by_name() {
    let lookup = full_lookup_without(&["FLEXCORE_VENDOR_WEATHER_TOKEN"]);
    let error = VendorWeatherConfig::from_lookup(&lookup).expect_err("no token: refused");
    assert_eq!(error.name(), "VENDOR_TOKEN_NOT_REGISTERED");
    assert!(
        matches!(error, VendorIngestionError::TokenNotRegistered { variable }
                 if variable == "FLEXCORE_VENDOR_WEATHER_TOKEN")
    );
    // The refusal names the human gate and the open E-decisions; it never
    // carries a credential value.
    let rendered = error.to_string();
    assert!(rendered.contains("HUMAN GATE"));
    assert!(rendered.contains("P15-E-1"));
}

#[test]
fn a_blank_registered_token_refuses_the_same_way() {
    let lookup = |name: &str| -> Option<String> {
        if name == "FLEXCORE_VENDOR_WEATHER_TOKEN" {
            Some("  ".to_string())
        } else {
            full_lookup_without(&[])(name)
        }
    };
    let error = VendorWeatherConfig::from_lookup(&lookup).expect_err("blank token refused");
    assert_eq!(error.name(), "VENDOR_TOKEN_NOT_REGISTERED");
}

#[test]
fn a_construction_with_a_registered_token_succeeds_and_redacts() {
    let lookup = full_lookup_without(&[]);
    let config = VendorWeatherConfig::from_lookup(&lookup).expect("token present: constructs");
    let rendered = format!("{config:?}");
    assert!(!rendered.contains("fixture-token"));
    assert!(rendered.contains("**REDACTED**"));
    // The declared conversion survived the env round trip.
    let conversion = config
        .unit_conversions
        .get("ghi")
        .expect("the declared conversion is present");
    assert_eq!(conversion.vendor_unit, "kW/m2");
    assert_eq!(conversion.factor, 1000.0);
    assert_eq!(conversion.factor_provenance, "fixture-unit-table");
}

// ---------------------------------------------------------------------------
// The staleness classification boundaries (FIM-F10.01; the P2.08 convention)
// ---------------------------------------------------------------------------

#[test]
fn staleness_classification_carries_the_p2_08_boundaries() {
    // Inside the window (including exactly AT it): GOOD.
    assert_eq!(classify_record_quality(0, 3_600_000).name(), "GOOD");
    assert_eq!(classify_record_quality(1, 3_600_000).name(), "GOOD");
    assert_eq!(classify_record_quality(3_600_000, 3_600_000).name(), "GOOD");
    // Past the window: STALE (never relabeled fresh, never dropped).
    assert_eq!(classify_record_quality(3_600_001, 3_600_000).name(), "STALE");
    // A clock read before the fetch is not silently GOOD.
    assert_eq!(classify_record_quality(-1, 3_600_000).name(), "STALE");
}

// ---------------------------------------------------------------------------
// The declared unit conversions (FIM-F10.01; §10.2 boundary)
// ---------------------------------------------------------------------------

#[test]
fn declared_factors_convert_with_the_pinned_float_spellings() {
    // 0.6125 × 1000.0 == 612.5 and 62.0 × 0.01 == 0.62 — the exact IEEE-754
    // double spellings confirmed with `python -I`.
    let kw =
        UnitConversion::new("kW/m2".to_string(), 1000.0, "fixture-unit-table".to_string())
            .expect("valid");
    assert_eq!(kw.convert(0.6125), 612.5);
    let percent =
        UnitConversion::new("percent".to_string(), 0.01, "fixture-unit-table".to_string())
            .expect("valid");
    assert_eq!(percent.convert(62.0), 0.62);
    // Identity is a DECLARED factor with provenance, never a default.
    let identity =
        UnitConversion::new("m/s".to_string(), 1.0, "fixture-unit-table".to_string())
            .expect("valid");
    assert_eq!(identity.convert(4.5), 4.5);
}

#[test]
fn degenerate_and_unprovenanced_factors_are_refused() {
    // A negative factor (e.g. the degF→degC affine temptation) is refused
    // at construction — affine conversions are not expressible as a
    // declared multiplicative factor and are never approximated.
    for factor in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let error = UnitConversion::new("kW/m2".to_string(), factor, "fixture".to_string())
            .expect_err("degenerate factor refused");
        assert_eq!(error.name(), "VENDOR_INGESTION_INVALID_CONFIG");
    }
    let error = UnitConversion::new("kW/m2".to_string(), 1000.0, " ".to_string())
        .expect_err("unprovenanced factor refused");
    assert_eq!(error.name(), "VENDOR_INGESTION_INVALID_CONFIG");
}

// ---------------------------------------------------------------------------
// The SHA-256 artifact digest chain (FIM-F10.04; P15.06 shape)
// ---------------------------------------------------------------------------

#[test]
fn sha256_matches_the_published_fips_vectors() {
    assert_eq!(
        sha256_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn the_chain_is_continuous_and_pins_the_computed_digests() {
    let mut chain = ForecastArtifactChain::new();
    assert_eq!(chain.head(), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    let a = chain
        .append(
            "artifact:load:v1".to_string(),
            "load".to_string(),
            "v1".to_string(),
            1_000,
            "vendor://fixture/feed".to_string(),
            render_values_canonical(&[-0.6, -0.6]),
        )
        .expect("append a");
    let b = chain
        .append(
            "artifact:pv_generation:v2".to_string(),
            "pv_generation".to_string(),
            "v2".to_string(),
            2_000,
            "vendor://fixture/feed".to_string(),
            render_values_canonical(&[8.0, 1.0]),
        )
        .expect("append b");
    // Head-link continuity: the second record names the first record's
    // digest as its previous head (the head-proof rule).
    assert_eq!(b.previous_head, a.digest);
    assert_eq!(chain.head(), b.digest);
    // The pinned digests (computed with `python -I` over the SAME canonical
    // rendering and digest input — the cross-language identity).
    assert_eq!(
        a.payload_digest,
        "668507de8a729fdcc4e208a8f1642fb64aa89dd62136abaa38e910986a4d17ff"
    );
    assert_eq!(
        a.digest,
        "5fd7adb79b9dbe196a448878c97a504bcf55c15652cc623d6b6c983f34d4cf13"
    );
    assert_eq!(
        b.digest,
        "dc5ffe60523f532c8ae44908cc6cf4b9675df54fa3e5bd513bc20977ccef9e86"
    );
    assert_eq!(chain.verify(), Ok(()));
}

#[test]
fn a_tampered_payload_is_named_never_absorbed() {
    let mut chain = ForecastArtifactChain::new();
    let a = chain
        .append(
            "artifact:load:v1".to_string(),
            "load".to_string(),
            "v1".to_string(),
            1_000,
            "vendor://fixture/feed".to_string(),
            render_values_canonical(&[-0.6, -0.6]),
        )
        .expect("append a");
    // Tamper with the payload: the recomputed payload digest diverges from
    // the committed one — the named tamper.
    let mut forged = a.clone();
    forged.payload_canonical = render_values_canonical(&[-0.7, -0.6]);
    match ForecastArtifactChain::verify_record(&forged, chain.records()[0].previous_head.as_str())
    {
        Err(ArtifactChainError::Tampered { artifact_ref, .. }) => {
            assert_eq!(artifact_ref, "artifact:load:v1");
        }
        other => panic!("unexpected verification: {other:?}"),
    }
}

#[test]
fn a_stale_head_proof_refuses_the_append_path() {
    let mut chain = ForecastArtifactChain::new();
    let a = chain
        .append(
            "artifact:load:v1".to_string(),
            "load".to_string(),
            "v1".to_string(),
            1_000,
            "vendor://fixture/feed".to_string(),
            render_values_canonical(&[-0.6, -0.6]),
        )
        .expect("append a");
    let b = chain
        .append(
            "artifact:load:v2".to_string(),
            "load".to_string(),
            "v2".to_string(),
            2_000,
            "vendor://fixture/feed".to_string(),
            render_values_canonical(&[-0.6]),
        )
        .expect("append b");
    // A record claiming the GENESIS head while the chain head has moved is
    // an out-of-order replay attempt: verification names the mismatch.
    let mut forked = b.clone();
    forked.previous_head = a.previous_head.clone(); // = genesis
    match ForecastArtifactChain::verify_record(&forked, chain.head()) {
        Err(ArtifactChainError::ChainHeadMismatch { expected, got }) => {
            assert_eq!(expected, chain.head());
            assert_eq!(got, "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        }
        other => panic!("unexpected verification: {other:?}"),
    }
}

#[test]
fn replay_requires_artifact_plus_inputs_identity() {
    // Replay: re-derive BOTH digests from the declared inputs and require
    // byte identity. The same inputs → the same digests (the replay
    // holds); one changed input (the version) → identity breaks.
    let payload = render_values_canonical(&[42.5]);
    let canonical = format!(
        "kind:{}|version:{}|produced_at:{}|source_ref:{}|payload:{}",
        "price", "v1", 3_000, "vendor://fixture/feed", payload
    );
    let record = flexcore_worker::vendor_ingestion::ForecastArtifactRecord {
        artifact_ref: "artifact:price:v1".to_string(),
        artifact_kind: "price".to_string(),
        version: "v1".to_string(),
        produced_at_millis: 3_000,
        source_ref: "vendor://fixture/feed".to_string(),
        payload_canonical: payload.clone(),
        payload_digest: artifact_payload_digest(&payload),
        previous_head: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
            .to_string(),
        digest: artifact_chain_digest(
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            &canonical,
        ),
        digest_algorithm: "sha256-chain-v1".to_string(),
    };
    assert_eq!(
        ForecastArtifactChain::verify_record(
            &record,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        ),
        Ok(())
    );
    // A drifted input (produced_at) breaks identity.
    let mut drifted = record.clone();
    drifted.produced_at_millis = 3_001;
    assert!(ForecastArtifactChain::verify_record(
        &drifted,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    )
    .is_err());
}

// ---------------------------------------------------------------------------
// The persistence-baseline fallback boundaries (FIM-F10.02)
// ---------------------------------------------------------------------------

#[test]
fn a_fresh_ingested_forecast_is_selected_at_and_inside_the_bound() {
    let values = [612.5, 610.0];
    // age == max_age is FRESH (the P2.08 equality convention).
    let decision = select_ingested_forecast(
        Some(IngestedForecastRef {
            values: &values,
            fetched_at: t(0),
        }),
        &[-0.5, -0.6],
        2,
        DEFAULT_FORECAST_MAX_AGE_MILLIS,
        t(DEFAULT_FORECAST_MAX_AGE_MILLIS),
    )
    .expect("valid selection");
    assert_eq!(
        decision,
        IngestedForecastDecision::LiveIngestedForecast {
            values: vec![612.5, 610.0],
            fetched_at_millis: 0,
        }
    );
    // Inside the bound: fresh as well.
    let decision = select_ingested_forecast(
        Some(IngestedForecastRef {
            values: &values,
            fetched_at: t(0),
        }),
        &[-0.5, -0.6],
        2,
        DEFAULT_FORECAST_MAX_AGE_MILLIS,
        t(1),
    )
    .expect("valid selection");
    assert!(matches!(
        decision,
        IngestedForecastDecision::LiveIngestedForecast { .. }
    ));
}

#[test]
fn max_age_plus_one_falls_back_to_the_persistence_baseline() {
    let values = [612.5];
    let decision = select_ingested_forecast(
        Some(IngestedForecastRef {
            values: &values,
            fetched_at: t(0),
        }),
        &[-0.5, -0.6],
        2,
        DEFAULT_FORECAST_MAX_AGE_MILLIS,
        t(DEFAULT_FORECAST_MAX_AGE_MILLIS + 1),
    )
    .expect("valid selection");
    match decision {
        IngestedForecastDecision::PersistenceBaselineFallback { reason, values } => {
            assert_eq!(
                reason,
                IngestionFallbackReason::MaxAgeExceeded {
                    age_millis: DEFAULT_FORECAST_MAX_AGE_MILLIS + 1,
                    max_age_millis: DEFAULT_FORECAST_MAX_AGE_MILLIS,
                }
            );
            // The persistence rule (the P15.02 single home): the last
            // history value carried forward.
            assert_eq!(values, vec![-0.6, -0.6]);
        }
        other => panic!("unexpected decision: {other:?}"),
    }
}

#[test]
fn no_ingested_forecast_and_a_clock_before_the_fetch_fall_back() {
    let decision =
        select_ingested_forecast(None, &[-0.5, -0.6], 2, 3_600_000, t(1_000)).expect("valid");
    match decision {
        IngestedForecastDecision::PersistenceBaselineFallback { reason, values } => {
            assert_eq!(reason, IngestionFallbackReason::NoIngestedForecast);
            assert_eq!(values, vec![-0.6, -0.6]);
        }
        other => panic!("unexpected decision: {other:?}"),
    }
    let values = [612.5];
    let decision = select_ingested_forecast(
        Some(IngestedForecastRef {
            values: &values,
            fetched_at: t(10_000),
        }),
        &[-0.5, -0.6],
        2,
        3_600_000,
        t(5_000),
    )
    .expect("valid");
    match decision {
        IngestedForecastDecision::PersistenceBaselineFallback { reason, .. } => {
            assert_eq!(reason, IngestionFallbackReason::ClockBeforeFetch);
        }
        other => panic!("unexpected decision: {other:?}"),
    }
}

#[test]
fn an_empty_history_escalates_to_the_labeled_synthetic_fixture() {
    // P15.16: the fixture stays the LABELED synthetic fallback source —
    // the decision carries the fixture id and the `synthetic` label; this
    // module never invents fixture values.
    let decision =
        select_ingested_forecast(None, &[], 2, 3_600_000, t(1_000)).expect("valid selection");
    match decision {
        IngestedForecastDecision::SyntheticFixtureFallback {
            reason,
            fixture_id,
            provenance_label,
        } => {
            assert_eq!(reason, IngestionFallbackReason::NoIngestedForecast);
            assert_eq!(fixture_id, "synthetic-forecast-fixture");
            assert_eq!(provenance_label, "synthetic");
        }
        other => panic!("unexpected decision: {other:?}"),
    }
}

#[test]
fn the_fallback_composes_the_recorded_statements_never_restates_them() {
    // ADR-010 (frozen): the fallback decision is an INPUT to scheduling
    // analytics, never a command — the composed statement, not a copy.
    assert!(flexcore_application::forecast_integration::FALLBACK_IS_NOT_A_COMMAND_STATEMENT
        .contains("INPUT"));
    // The persistence fallback carries the standing no-skill posture.
    assert!(flexcore_application::forecast_baselines::NO_SKILL_STATEMENT
        .contains("no skill, unproven, UNTRAINED"));
    // The reasons render for the operator record.
    assert_eq!(
        IngestionFallbackReason::NoIngestedForecast.as_str(),
        "no_ingested_forecast"
    );
}
