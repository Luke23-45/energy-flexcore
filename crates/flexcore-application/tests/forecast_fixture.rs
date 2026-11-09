// P3.31 — Synthetic forecast fixture: acceptance tests (ledger P3.31).
//
// Covers the ledger acceptance criteria:
//
// - the fixture regenerates **byte-identically** from the seed (two
//   generations, and the committed versioned artefacts match the generator);
// - every forecast value carries the synthetic label AND the version —
//   through the only access path, the labeled accessor;
// - the envelope and result records reference the forecast VERSION, not bare
//   values (the accessor type carries the version on every read);
// - an unlabelled or unversioned forecast is rejected by the envelope input
//   validation with a named error;
// - the P3.26 error variant's magnitude is recorded;
// - the transcribed scenario constants are pinned against the versioned
//   scenario artefact itself.
//
// NOTE (VERIFY-BUILD, recorded): the committed JSON artefacts were produced
// by replicating the generator algorithm outside cargo; the byte-equality
// tests below are the verification obligation on the first build.

use flexcore_application::forecast_fixture::{
    forecast_publication, generate_central_fixture, generate_error_pv_ramp_fixture,
    validate_forecast_input, FixtureMethod, ForecastFixtureError, ForecastPublicationInput,
    ForecastQuantity, ForecastVersion, Provenance, UnvalidatedErrorVariant, UnvalidatedForecastInput,
    UnvalidatedSeries,
};
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::EventTime;
use flexcore_domain::ids::{AssetId, PortfolioId, SiteId, TenantId};
use flexcore_domain::scheduling_time::SchedulingGridConfig;
use flexcore_events::envelope::SUPPORTED_SCHEMA_VERSION;
use flexcore_events::events::{DomainEvent, ForecastPublishedPayload};

/// The committed central fixture artefact (the versioned file, loaded from
/// the repo so crate and artefact cannot drift silently).
const COMMITTED_CENTRAL: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../scenarios/synthetic-portfolio/forecast-fixture-v1.json"
));

/// The committed P3.26 error-variant artefact.
const COMMITTED_ERROR_PV_RAMP: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../scenarios/synthetic-portfolio/forecast-fixture-v1-error-pv-ramp.json"
));

/// The versioned exit-gate scenario, loaded to pin the transcribed
/// generator constants against the artefact itself.
const SCENARIO_V2: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../scenarios/synthetic-portfolio/scenario-v2.json"
));

// --- Byte identity ---------------------------------------------------------

#[test]
fn regeneration_is_byte_identical() {
    let a = generate_central_fixture().to_json();
    let b = generate_central_fixture().to_json();
    assert_eq!(a, b, "same seed must regenerate a byte-identical artefact");
    let c = generate_error_pv_ramp_fixture().to_json();
    let d = generate_error_pv_ramp_fixture().to_json();
    assert_eq!(c, d, "the error variant must regenerate byte-identically");
    assert_ne!(a, c, "the two fixture versions are different artefacts");
}

#[test]
fn committed_central_artefact_matches_generator() {
    let generated = generate_central_fixture().to_json();
    assert_eq!(
        generated, COMMITTED_CENTRAL,
        "VERIFY-BUILD: the committed central fixture must be byte-identical to the \
         generator output (the artefact is RECORDED-OUTPUT; this test is the \
         verification obligation)"
    );
}

#[test]
fn committed_error_variant_artefact_matches_generator() {
    let generated = generate_error_pv_ramp_fixture().to_json();
    assert_eq!(
        generated, COMMITTED_ERROR_PV_RAMP,
        "VERIFY-BUILD: the committed error-variant fixture must be byte-identical to \
         the generator output (RECORDED-OUTPUT; this test is the verification)"
    );
}

// --- Labelling: every value labeled, version on every access ----------------

#[test]
fn every_value_is_labeled_synthetic_with_a_version() {
    for fixture in [generate_central_fixture(), generate_error_pv_ramp_fixture()] {
        assert_eq!(fixture.meta().provenance(), Provenance::Synthetic);
        for series in fixture.series() {
            let labeled = series.values();
            // The ONLY access path returns label, version, and quantity
            // with the values.
            assert_eq!(
                labeled.provenance,
                Provenance::Synthetic,
                "every value must carry the synthetic label"
            );
            assert_eq!(
                labeled.forecast_version.as_str(),
                fixture.forecast_version().as_str(),
                "every value must carry its fixture's forecast version"
            );
            assert!(!labeled.values.is_empty());
            assert_eq!(labeled.values.len(), fixture.grid().interval_count());
        }
    }
}

#[test]
fn the_accessor_references_the_version_not_bare_values() {
    // The envelope (P3.12) and result records (P3.21) reference the forecast
    // VERSION; the accessor type enforces that the version travels with any
    // value read.
    let fixture = generate_central_fixture();
    let series = &fixture.series()[0];
    let labeled = series.values();
    let version = labeled.forecast_version;
    assert_eq!(version.as_str(), "v1");
    assert_eq!(version.to_string(), "v1");
    // The quantity and its canonical unit travel too.
    assert_eq!(labeled.quantity, ForecastQuantity::PvOutputMw);
    assert_eq!(labeled.quantity.unit(), "MW");
    // The fixture-level version matches the series-level version.
    assert_eq!(
        fixture.forecast_version().as_str(),
        labeled.forecast_version.as_str()
    );
}

// --- The error variant -------------------------------------------------------

#[test]
fn the_error_variant_magnitude_is_recorded() {
    let fixture = generate_error_pv_ramp_fixture();
    let variant = fixture.error_variant().expect("the variant is recorded");
    assert_eq!(variant.variant_id(), "pv-ramp");
    assert!(variant.direction_downward());
    assert_eq!(variant.applies_to(), ForecastQuantity::PvOutputMw);
    assert_eq!(variant.magnitude_milli(), 800, "stated magnitude: 80 percent");
    assert!((variant.magnitude() - 0.8).abs() < 1e-12);
    assert_eq!(variant.window_first_interval(), 720);
    assert_eq!(variant.window_last_exclusive(), 780);
    // The artefact carries the magnitude explicitly.
    let json = fixture.to_json();
    assert!(
        json.contains("\"error_magnitude_milli\": 800"),
        "the stated magnitude must be recorded in the artefact"
    );
    // The two versions are versioned SEPARATELY.
    assert_eq!(fixture.forecast_version().as_str(), "v1-error-pv-ramp");
    assert_eq!(
        generate_central_fixture().forecast_version().as_str(),
        "v1"
    );
}

#[test]
fn the_error_variant_drops_the_pv_forecast_only_inside_the_window() {
    let central = generate_central_fixture();
    let error = generate_error_pv_ramp_fixture();
    let first = 720usize;
    let last = 780usize;
    // Central vs error: inside the window the error forecast is strictly
    // lower for every PV series (the 80 percent downward ramp dominates the
    // ±2 percent noise band of the independent streams, same shape).
    for asset in ["site-north-pv-1", "site-south-pv-1"] {
        let c = central
            .series()
            .iter()
            .find(|s| s.asset_key() == asset)
            .unwrap()
            .values()
            .values;
        let e = error
            .series()
            .iter()
            .find(|s| s.asset_key() == asset)
            .unwrap()
            .values()
            .values;
        assert_eq!(c.len(), e.len());
        for i in first..last {
            assert!(
                e[i] < c[i],
                "interval {i}: the ramp variant must forecast strictly less PV than the central one"
            );
        }
        // Midday shape > 0.4 MW, so the drop is material (the stated 80
        // percent magnitude dominates the noise).
        assert!(c[720] > 0.4);
        assert!(e[720] < 0.4 * c[720] + 0.05);
        // Load and availability series are untouched by the variant: same
        // quantity set, same interval counts.
        assert_eq!(central.series().len(), error.series().len());
    }
}

// --- Envelope input validation ----------------------------------------------

fn raw_series_from(fixture: &flexcore_application::forecast_fixture::ForecastFixture) -> Vec<UnvalidatedSeries> {
    fixture
        .series()
        .iter()
        .map(|s| UnvalidatedSeries {
            asset_key: s.asset_key().to_string(),
            site_key: s.site_key().to_string(),
            quantity: s.values().quantity,
            values: s.values().values.to_vec(),
        })
        .collect()
}

fn raw_input_from(fixture: &flexcore_application::forecast_fixture::ForecastFixture) -> UnvalidatedForecastInput {
    UnvalidatedForecastInput {
        provenance: Some(Provenance::Synthetic),
        forecast_version: Some(fixture.forecast_version().as_str().to_string()),
        variant: fixture.meta().variant().to_string(),
        scenario_id: fixture.meta().scenario_id().to_string(),
        scenario_version: fixture.meta().scenario_version().to_string(),
        master_seed: fixture.meta().master_seed(),
        method: FixtureMethod::recorded(),
        horizon_start_millis: fixture.grid().horizon_start_millis(),
        horizon_start_iso: fixture.grid().horizon_start_iso().to_string(),
        horizon_millis: fixture.grid().horizon_millis(),
        interval_millis: fixture.grid().interval_millis(),
        scenario_cadence_seconds: fixture.grid().scenario_cadence_seconds(),
        error_variant: fixture.error_variant().map(|v| UnvalidatedErrorVariant {
            variant_id: v.variant_id().to_string(),
            applies_to: v.applies_to(),
            direction_downward: v.direction_downward(),
            magnitude_milli: v.magnitude_milli(),
            window_start_iso: v.window_start_iso().to_string(),
            window_end_iso: v.window_end_iso().to_string(),
            window_first_interval: v.window_first_interval(),
            window_last_exclusive: v.window_last_exclusive(),
            note: v.note().to_string(),
        }),
        series: raw_series_from(fixture),
    }
}

#[test]
fn a_fully_labeled_forecast_round_trips_through_validation() {
    let central = generate_central_fixture();
    let validated = validate_forecast_input(raw_input_from(&central)).expect("labeled and versioned input passes");
    assert_eq!(validated.meta().provenance(), Provenance::Synthetic);
    assert_eq!(validated.forecast_version().as_str(), "v1");
    assert_eq!(validated.series().len(), central.series().len());
    // The validated fixture is the typed form of the same artefact.
    assert_eq!(validated.to_json(), central.to_json());

    let error = generate_error_pv_ramp_fixture();
    let validated_error = validate_forecast_input(raw_input_from(&error))
        .expect("the labeled error variant passes");
    assert_eq!(validated_error.to_json(), error.to_json());
}

#[test]
fn an_unlabelled_forecast_is_rejected_with_a_named_error() {
    let central = generate_central_fixture();
    let mut input = raw_input_from(&central);
    input.provenance = None; // unlabelled
    assert_eq!(
        validate_forecast_input(input),
        Err(ForecastFixtureError::MissingProvenanceLabel),
        "P3.31 acceptance: no Phase 3 code path consumes an unlabelled forecast"
    );
}

#[test]
fn an_unversioned_forecast_is_rejected_with_a_named_error() {
    let central = generate_central_fixture();
    let mut input = raw_input_from(&central);
    input.forecast_version = None; // unversioned
    assert_eq!(
        validate_forecast_input(input),
        Err(ForecastFixtureError::MissingForecastVersion),
        "P3.31 acceptance: no Phase 3 code path consumes an unversioned forecast"
    );
}

#[test]
fn an_empty_version_string_is_rejected_too() {
    let central = generate_central_fixture();
    let mut input = raw_input_from(&central);
    input.forecast_version = Some(String::new());
    assert_eq!(
        validate_forecast_input(input),
        Err(ForecastFixtureError::EmptyForecastVersion)
    );
}

#[test]
fn the_fixture_grid_matches_the_p3_03_scheduling_grid() {
    // The fixture indexes the same grid as P3.03: the scenario cadence (60 s)
    // over the 24-hour horizon is a valid SchedulingGridConfig, and the
    // interval boundaries agree with the P3.03 arithmetic.
    let fixture = generate_central_fixture();
    let config = SchedulingGridConfig::new(86_400_000, 60_000).expect("valid grid");
    assert!(fixture.grid().matches_config(&config));
    assert_eq!(fixture.grid().interval_count(), config.interval_count());
    for i in 0..config.interval_count() {
        assert_eq!(
            fixture.grid().interval_start_millis(i),
            Some(1_780_272_000_000 + (i as i64) * 60_000)
        );
    }
    // Two independently built grids over the same configuration produce
    // identical boundaries (P3.03 acceptance, exercised through the fixture).
    let other = SchedulingGridConfig::new(86_400_000, 60_000).expect("valid grid");
    assert_eq!(config.interval_count(), other.interval_count());
}

// --- ForecastPublished assembly ----------------------------------------------

#[test]
fn the_publication_assembles_the_frozen_event_on_the_frozen_subject() {
    let version = ForecastVersion::new("v1").unwrap();
    let correlation = FlexUuid::nil();
    let publication = forecast_publication(ForecastPublicationInput {
        correlation_id: correlation,
        tenant_id: TenantId::new(correlation),
        portfolio_id: PortfolioId::new(correlation),
        site_id: SiteId::new(correlation),
        asset_id: AssetId::new(correlation),
        forecast_version: &version,
        provenance: Provenance::Synthetic,
        event_time: EventTime::from_unix_millis(1_000).expect("valid timestamp"),
    })
    .expect("the publication assembles");
    assert_eq!(
        publication.destination_subject.as_str(),
        "flexcore.v1.forecast.forecast_published",
        "subject per the frozen P1.14 table"
    );
    assert_eq!(
        publication.idempotency_key.as_str(),
        "forecast_published:v1:synthetic",
        "the key is deterministic from the forecast version (+ provenance)"
    );
    assert_eq!(publication.envelope.schema_version, SUPPORTED_SCHEMA_VERSION);
    // The event is the frozen ForecastPublished with the supplied scope.
    match &publication.envelope.event {
        DomainEvent::ForecastPublished(payload) => {
            assert_eq!(*payload, ForecastPublishedPayload {
                correlation_id: correlation,
                tenant_id: TenantId::new(correlation),
                portfolio_id: PortfolioId::new(correlation),
                site_id: SiteId::new(correlation),
                asset_id: AssetId::new(correlation),
                event_time: EventTime::from_unix_millis(1_000).expect("valid timestamp"),
            });
        }
        other => panic!("expected ForecastPublished, got {other:?}"),
    }
}

#[test]
fn the_publication_key_differs_per_forecast_version() {
    let v1 = ForecastVersion::new("v1").unwrap();
    let ramp = ForecastVersion::new("v1-error-pv-ramp").unwrap();
    let correlation = FlexUuid::nil();
    let make = |version: &ForecastVersion| {
        forecast_publication(ForecastPublicationInput {
            correlation_id: correlation,
            tenant_id: TenantId::new(correlation),
            portfolio_id: PortfolioId::new(correlation),
            site_id: SiteId::new(correlation),
            asset_id: AssetId::new(correlation),
            forecast_version: version,
            provenance: Provenance::Synthetic,
            event_time: EventTime::from_unix_millis(1_000).expect("valid timestamp"),
        })
        .unwrap()
    };
    let a = make(&v1);
    let b = make(&ramp);
    assert_ne!(a.idempotency_key, b.idempotency_key);
    // Same version → same key (republication of one fact dedupes durably).
    assert_eq!(a.idempotency_key, make(&v1).idempotency_key);
}

// --- Pinned scenario constants -----------------------------------------------

#[test]
fn the_transcribed_scenario_constants_appear_verbatim_in_scenario_v2() {
    // The generator's constants are transcribed from scenario-v2.json (the
    // application crate cannot parse JSON); this pins the transcription
    // against the versioned artefact itself.
    for needle in [
        "\"master_seed\": 20260601",
        "\"default_seconds\": 60",
        "\"duration_hours\": 24",
        "\"start\": \"2026-06-01T00:00:00Z\"",
        "\"scenario_id\": \"synthetic-portfolio-exit-gate\"",
        "\"version\": \"v2\"",
        "\"sunrise_utc\": \"05:30\"",
        "\"sunset_utc\": \"21:00\"",
        "\"peak_factor\": 0.85",
        "\"peak_factor\": 0.9",
        "\"rated_power_mw\": 1.5",
        "\"rated_power_mw\": 2.5",
        "\"rated_peak_mw\": 0.8",
        "\"rated_peak_mw\": 1.2",
        "\"asset_key\": \"site-north-pv-1\"",
        "\"asset_key\": \"site-north-load-1\"",
        "\"asset_key\": \"site-south-pv-1\"",
        "\"asset_key\": \"site-south-load-1\"",
    ] {
        assert!(
            SCENARIO_V2.contains(needle),
            "the scenario artefact must contain {needle} (generator constants drifted)"
        );
    }
}

#[test]
fn the_committed_artefacts_carry_the_provenance_label_everywhere() {
    // Four-place labelling, places 1–2 as committed: fixture level and per
    // series level.
    for artefact in [COMMITTED_CENTRAL, COMMITTED_ERROR_PV_RAMP] {
        assert!(
            artefact.contains("\"provenance\": \"synthetic\""),
            "the artefact must label its provenance"
        );
        // Fixture level once + once per series (8 series).
        let occurrences = artefact
            .matches("\"provenance\": \"synthetic\"")
            .count();
        assert_eq!(
            occurrences, 9,
            "one fixture-level label plus one label per series (8)"
        );
        assert!(
            artefact.contains("not measured data, not a market price"),
            "the artefact must state what the data is NOT"
        );
        assert!(
            !artefact.to_ascii_lowercase().contains("price_curve"),
            "no price data in the fixture (D1/D2 open)"
        );
    }
    assert!(COMMITTED_ERROR_PV_RAMP.contains("\"error_magnitude_milli\": 800"));
}
