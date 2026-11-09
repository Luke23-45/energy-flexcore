// P3.12 — Portfolio flexibility envelope: acceptance tests (ledger P3.12).
//
// Covers the ledger acceptance criteria:
//
// - THE GATE: the envelope is strictly less than the nameplate sum on the
//   documented case where a shared constraint binds (the site PCC export
//   limit), with a same-portfolio no-limit control isolating the shared
//   constraint's effect (a gate, not a nice-to-have);
// - the constrained aggregation composes the frozen P3.02 unknown-narrows
//   rule (unknown never widens; empty = infeasible, never repaired);
// - per-interval forecast caps through the labeled accessor only, with the
//   synthetic label and version surviving into the envelope's input
//   snapshot (the envelope-input place of the four-place rule);
// - central estimate present; the availability/confidence descriptor is
//   the explicit "not supported" statement — no confidence number exists;
// - the correlation treatment: declared group derate narrows, ungrouped
//   assets are recorded as correlation-unmodelled, no coefficient invented;
// - the conservative BESS energy cap (time-coupled constraint) and its
//   required-for-BESS rule;
// - the `FlexibilityEnvelopeUpdated` publication: frozen subject,
//   deterministic version-carrying idempotency key, one event per site.
//
// NOTE (VERIFY-BUILD, recorded): per the authoring constraint, cargo was
// not run; the first `cargo test -p flexcore-application --test
// flexibility_envelope` run must confirm.

use flexcore_application::flexibility_envelope::{
    calculate_portfolio_envelope, envelope_updated_publications,
    AVAILABILITY_DESCRIPTOR_NOT_SUPPORTED, CORRELATION_TREATMENT_STATEMENT,
    ENVELOPE_PAYLOAD_GAP_NOTE, AssetAvailabilityInput, AssetContribution, AssetEnvelopeInput,
    EnvelopeComputationInput, EnvelopeError, EnvelopeIntervalStatus, EnvelopeUnknownKind,
    InfeasibilityCause, LossAdjustment, PortfolioFlexibilityEnvelope, ProvenancedScalar,
    SharedExposureGroup, SiteCouplingInput, EnergyCouplingInput,
};
use flexcore_application::forecast_fixture::{generate_central_fixture, ForecastFixture};
use flexcore_contracts::capability::Provenance as CapabilityProvenance;
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::{EventTime, ValidFrom};
use flexcore_domain::capability_provenance::{AssetClass, BoundInterval};
use flexcore_domain::ids::{AssetId, PortfolioId, SiteId, TenantId};
use flexcore_domain::scheduling_time::{SchedulingGrid, SchedulingGridConfig};

/// The central fixture's horizon start (2026-06-01T00:00:00Z, D-FX-1).
const HORIZON_START_MILLIS: i64 = 1_780_272_000_000;
/// One day over the scenario cadence (D-FX-1): 1440 one-minute intervals.
const INTERVAL_COUNT: usize = 1440;

fn uid(n: u128) -> FlexUuid {
    FlexUuid::from_bytes(n.to_be_bytes())
}

fn tenant() -> TenantId {
    TenantId::new(uid(0xA100))
}

fn portfolio() -> PortfolioId {
    PortfolioId::new(uid(0xA200))
}

fn north_site() -> SiteId {
    SiteId::new(uid(0xA300))
}

fn south_site() -> SiteId {
    SiteId::new(uid(0xA301))
}

fn grid() -> SchedulingGrid {
    SchedulingGrid::new(
        ValidFrom::from_unix_millis(HORIZON_START_MILLIS).expect("valid timestamp"),
        SchedulingGridConfig::new(86_400_000, 60_000).expect("valid grid"),
    )
}

fn computed_at() -> EventTime {
    EventTime::from_unix_millis(1_000).expect("valid timestamp")
}

/// The declared availability used by every test: the scenario declares
/// availability constant true, so the fraction is the constant 1.0
/// (D-FX-2); the source string is recorded per the envelope rule. The
/// slice is leaked (test-only) so the borrowed asset inputs stay valid.
fn full_availability() -> &'static [f64] {
    Box::leak(vec![1.0; INTERVAL_COUNT].into_boxed_slice())
}

fn availability_source() -> String {
    "scenario-v2: the scenario declares availability constant true (D-FX-2)".to_string()
}

fn series<'a>(fixture: &'a ForecastFixture, asset_key: &str) -> &'a flexcore_application::forecast_fixture::LabeledForecastSeries {
    fixture
        .series()
        .iter()
        .find(|s| s.asset_key() == asset_key)
        .expect("the fixture carries the scenario series")
}

fn pv_input<'a>(
    asset_id: AssetId,
    site_id: SiteId,
    fixture: &'a ForecastFixture,
    asset_key: &str,
    declared_upper: f64,
) -> AssetEnvelopeInput<'a> {
    AssetEnvelopeInput {
        asset_id,
        site_id,
        class: AssetClass::InverterPq,
        capability_profile_reference: "profile:pv_clear_sky".to_string(),
        declared_p_net_mw: BoundInterval::new(Some(0.0), Some(declared_upper))
            .expect("valid range"),
        p_net_provenance: CapabilityProvenance::Measured,
        availability: AssetAvailabilityInput::Declared {
            fractions: full_availability(),
            source: availability_source(),
        },
        contribution: AssetContribution::ForecastCappedPv {
            series: series(fixture, asset_key),
        },
        energy_coupling: None,
    }
}

/// The north PV (rated 1.5 MW) and south PV (rated 2.5 MW) both placed on
/// the north site: the shared-site gate portfolio.
fn gate_assets<'a>(fixture: &'a ForecastFixture) -> Vec<AssetEnvelopeInput<'a>> {
    vec![
        pv_input(AssetId::new(uid(0xB001)), north_site(), fixture, "site-north-pv-1", 1.5),
        pv_input(AssetId::new(uid(0xB002)), north_site(), fixture, "site-south-pv-1", 2.5),
    ]
}

fn north_coupling(export_limit: Option<ProvenancedScalar>) -> SiteCouplingInput {
    SiteCouplingInput {
        site_id: north_site(),
        point_of_common_coupling: "PCC-NORTH-01".to_string(),
        grid_model_reference: "grid-model:north@v1".to_string(),
        export_limit_mw: export_limit,
        import_limit_mw: None,
    }
}

fn compute(
    fixture: &ForecastFixture,
    assets: Vec<AssetEnvelopeInput<'_>>,
    sites: Vec<SiteCouplingInput>,
    groups: Vec<SharedExposureGroup>,
) -> Result<PortfolioFlexibilityEnvelope, EnvelopeError> {
    calculate_portfolio_envelope(EnvelopeComputationInput {
        envelope_version: "env-v1",
        computed_at: computed_at(),
        tenant_id: tenant(),
        portfolio_id: portfolio(),
        grid: grid(),
        fixture,
        assets,
        site_couplings: sites,
        correlation_groups: groups,
        loss_adjustment: LossAdjustment::Unmodelled,
        nameplate_sum_mw: 4.0,
    })
}

fn pv_series_value(fixture: &ForecastFixture, asset_key: &str, index: usize) -> f64 {
    series(fixture, asset_key).values().values[index]
}

// --- THE GATE ---------------------------------------------------------------

/// THE GATE (ledger P3.12 acceptance): with a binding shared site export
/// limit the envelope is (a) exactly the site cap while the unconstrained
/// aggregation of the SAME portfolio is the forecast sum, and (b) strictly
/// less than the nameplate sum. The no-limit control isolates the shared
/// constraint: the narrowing comes from the site coupling, not from the
/// forecast cap alone.
#[test]
fn envelope_is_strictly_below_the_nameplate_sum_where_the_shared_site_limit_binds() {
    let fixture = generate_central_fixture();
    let noon = 720; // 2026-06-01T12:00:00Z, deep inside the clear-sky bell

    // The shared-constraint case: both PVs behind one 1.0 MW PCC export
    // limit (the registry holds no transformer limit — the value is a
    // recorded fixture configuration, provenance Measured = declared bound
    // used as declared; the synthetic basis is recorded in the scenario).
    let limited = compute(
        &fixture,
        gate_assets(&fixture),
        vec![north_coupling(Some(ProvenancedScalar {
            value: 1.0,
            provenance: CapabilityProvenance::Measured,
        }))],
        vec![],
    )
    .expect("the gate case computes");

    let interval = &limited.intervals()[noon];
    assert_eq!(
        interval.status,
        EnvelopeIntervalStatus::Feasible,
        "the gate interval must be feasible"
    );
    assert!(
        (interval.upper_mw - 1.0).abs() < 1e-9,
        "the binding site limit caps the aggregate exactly at 1.0 MW, got {}",
        interval.upper_mw
    );

    // The control: the SAME portfolio without the shared limit aggregates
    // to the sum of the per-asset forecast-capped ranges.
    let unlimited = compute(
        &fixture,
        gate_assets(&fixture),
        vec![north_coupling(None)],
        vec![],
    )
    .expect("the control case computes");
    let control_upper = unlimited.intervals()[noon].upper_mw;
    let expected_sum = pv_series_value(&fixture, "site-north-pv-1", noon)
        + pv_series_value(&fixture, "site-south-pv-1", noon);
    assert!(
        (control_upper - expected_sum).abs() < 1e-9,
        "without the shared limit the aggregate is the forecast sum ({control_upper} vs {expected_sum})"
    );

    // The gate itself: strictly less than the nameplate sum (4.0 MW) and
    // strictly less than the unconstrained aggregate.
    assert!(
        interval.upper_mw < limited.nameplate_sum_mw(),
        "the envelope upper ({}) must be strictly below the nameplate sum ({})",
        interval.upper_mw,
        limited.nameplate_sum_mw()
    );
    assert!(
        interval.upper_mw < control_upper - 1e-9,
        "the shared constraint must strictly narrow the aggregate"
    );
}

// --- The constrained aggregation ---------------------------------------------

#[test]
fn the_forecast_caps_the_pv_export_per_interval_through_the_labeled_accessor() {
    let fixture = generate_central_fixture();
    let envelope = compute(
        &fixture,
        vec![pv_input(AssetId::new(uid(0xB001)), north_site(), &fixture, "site-north-pv-1", 1.5)],
        vec![north_coupling(None)],
        vec![],
    )
    .unwrap();

    // At night (interval 0) the forecast is zero: no export capability.
    let night = &envelope.intervals()[0];
    assert_eq!(night.upper_mw, 0.0);
    assert_eq!(night.lower_mw, 0.0);
    // At noon the export is capped by the forecast value, below the 1.5 MW
    // declared range.
    let noon = &envelope.intervals()[720];
    let forecast = pv_series_value(&fixture, "site-north-pv-1", 720);
    assert!((noon.upper_mw - forecast).abs() < 1e-9);
    assert!(noon.upper_mw < 1.5);
    // The central estimate is the midpoint (the recorded convention).
    assert!((noon.central_estimate_mw - (noon.lower_mw + noon.upper_mw) / 2.0).abs() < 1e-12);
}

#[test]
fn unknown_provenance_narrows_an_asset_to_zero_credit_via_the_frozen_p3_02_rule() {
    let fixture = generate_central_fixture();
    let mut asset = pv_input(AssetId::new(uid(0xB001)), north_site(), &fixture, "site-north-pv-1", 1.5);
    asset.p_net_provenance = CapabilityProvenance::Unknown;
    let envelope = compute(&fixture, vec![asset], vec![north_coupling(None)], vec![]).unwrap();

    let noon = &envelope.intervals()[720];
    assert_eq!(noon.upper_mw, 0.0, "unknown never widens: no credit");
    assert_eq!(noon.lower_mw, 0.0);
    assert_eq!(envelope.nameplate_sum_mw(), 4.0);
}

#[test]
fn an_empty_narrowed_intersection_makes_the_interval_infeasible_never_widened() {
    let fixture = generate_central_fixture();
    let mut asset = pv_input(AssetId::new(uid(0xB001)), north_site(), &fixture, "site-north-pv-1", 3.0);
    // A declared range [2.0, 3.0] with UNKNOWN provenance intersects the
    // recorded collapse-to-zero default in the empty set: the dimension is
    // infeasible (P3.02), never repaired by widening.
    asset.declared_p_net_mw = BoundInterval::new(Some(2.0), Some(3.0)).unwrap();
    asset.p_net_provenance = CapabilityProvenance::Unknown;
    let envelope = compute(&fixture, vec![asset], vec![north_coupling(None)], vec![]).unwrap();

    for interval in envelope.intervals() {
        assert_eq!(
            interval.status,
            EnvelopeIntervalStatus::Infeasible {
                cause: InfeasibilityCause::AssetDimensionEmpty {
                    asset_id: AssetId::new(uid(0xB001)),
                }
            },
            "every interval is infeasible for the infeasible asset dimension"
        );
        assert_eq!((interval.lower_mw, interval.upper_mw), (0.0, 0.0));
    }
}

#[test]
fn unknown_availability_credits_nothing_and_is_recorded() {
    let fixture = generate_central_fixture();
    let mut asset = pv_input(AssetId::new(uid(0xB001)), north_site(), &fixture, "site-north-pv-1", 1.5);
    asset.availability = AssetAvailabilityInput::Unknown;
    let envelope = compute(&fixture, vec![asset], vec![north_coupling(None)], vec![]).unwrap();

    let noon = &envelope.intervals()[720];
    assert_eq!(noon.upper_mw, 0.0, "no evidenced availability credits nothing");
    assert!(envelope.unknown_dimensions().iter().any(|d| d.kind
        == EnvelopeUnknownKind::AvailabilityUnknownNarrowedToZero
        && d.asset_id == Some(AssetId::new(uid(0xB001)))));
}

#[test]
fn the_load_baseline_shifts_the_declared_offset_range() {
    let fixture = generate_central_fixture();
    let asset = AssetEnvelopeInput {
        asset_id: AssetId::new(uid(0xB003)),
        site_id: north_site(),
        class: AssetClass::FlexibleLoad,
        capability_profile_reference: "profile:load_baseline_reducible".to_string(),
        declared_p_net_mw: BoundInterval::new(Some(-0.3), Some(0.2)).expect("valid range"),
        p_net_provenance: CapabilityProvenance::Measured,
        availability: AssetAvailabilityInput::Declared {
            fractions: full_availability(),
            source: availability_source(),
        },
        contribution: AssetContribution::ForecastBaselineLoad {
            series: series(&fixture, "site-north-load-1"),
            reducible_mw: ProvenancedScalar {
                value: 0.3,
                provenance: CapabilityProvenance::Measured,
            },
            increasable_mw: ProvenancedScalar {
                value: 0.2,
                provenance: CapabilityProvenance::Measured,
            },
        },
        energy_coupling: None,
    };
    let envelope = compute(&fixture, vec![asset], vec![north_coupling(None)], vec![]).unwrap();

    let index = 0; // night: baseline = -0.25 * 0.8 = -0.2 MW (plus noise)
    let baseline = pv_series_value(&fixture, "site-north-load-1", index);
    let interval = &envelope.intervals()[index];
    assert!(
        (interval.lower_mw - (baseline - 0.3)).abs() < 1e-9,
        "the reduction widens the import side below the baseline"
    );
    assert!(
        (interval.upper_mw - (baseline + 0.2)).abs() < 1e-9,
        "the increase narrows the import above the baseline"
    );
    assert!(interval.upper_mw < 0.0, "the load imports at night even fully increased");
}

#[test]
fn an_unknown_provenance_offset_credits_no_load_flexibility() {
    let fixture = generate_central_fixture();
    let asset = AssetEnvelopeInput {
        asset_id: AssetId::new(uid(0xB003)),
        site_id: north_site(),
        class: AssetClass::FlexibleLoad,
        capability_profile_reference: "profile:load_baseline_reducible".to_string(),
        declared_p_net_mw: BoundInterval::new(Some(-0.3), Some(0.2)).expect("valid range"),
        p_net_provenance: CapabilityProvenance::Measured,
        availability: AssetAvailabilityInput::Declared {
            fractions: full_availability(),
            source: availability_source(),
        },
        contribution: AssetContribution::ForecastBaselineLoad {
            series: series(&fixture, "site-north-load-1"),
            reducible_mw: ProvenancedScalar {
                value: 0.3,
                provenance: CapabilityProvenance::Unknown,
            },
            increasable_mw: ProvenancedScalar {
                value: 0.2,
                provenance: CapabilityProvenance::Unknown,
            },
        },
        energy_coupling: None,
    };
    let envelope = compute(&fixture, vec![asset], vec![north_coupling(None)], vec![]).unwrap();

    let baseline = pv_series_value(&fixture, "site-north-load-1", 720);
    let interval = &envelope.intervals()[720];
    assert!((interval.lower_mw - baseline).abs() < 1e-9);
    assert!((interval.upper_mw - baseline).abs() < 1e-9);
}

// --- Time coupling and site coupling -----------------------------------------

#[test]
fn the_conservative_energy_cap_constrains_sustained_bess_export() {
    let fixture = generate_central_fixture();
    let asset = AssetEnvelopeInput {
        asset_id: AssetId::new(uid(0xB004)),
        site_id: north_site(),
        class: AssetClass::Bess,
        capability_profile_reference: "profile:bess_daily_cycle".to_string(),
        declared_p_net_mw: BoundInterval::new(Some(-1.0), Some(1.0)).expect("valid range"),
        p_net_provenance: CapabilityProvenance::Measured,
        availability: AssetAvailabilityInput::Declared {
            fractions: full_availability(),
            source: availability_source(),
        },
        contribution: AssetContribution::Dispatchable,
        // 0.01 MWh above the floor at the horizon start: the conservative
        // sustained-export cap over a 60 s interval is
        // 0.01 * 0.9 / (1/60) = 0.54 MW — below the declared 1.0 MW.
        energy_coupling: Some(EnergyCouplingInput {
            e_min_mwh: 0.0,
            e_max_mwh: 8.0,
            e_initial_mwh: 0.01,
            eta_ch: 0.9,
            eta_dis: 0.9,
        }),
    };
    let envelope = compute(&fixture, vec![asset], vec![north_coupling(None)], vec![]).unwrap();

    let interval = &envelope.intervals()[720];
    assert!((interval.upper_mw - 0.54).abs() < 1e-9, "got {}", interval.upper_mw);
    // The import side keeps the full declared range (headroom to the
    // ceiling is large; the conservative cap does not bind there).
    assert!((interval.lower_mw - (-1.0)).abs() < 1e-9);
}

#[test]
fn a_bess_asset_without_the_energy_cap_is_rejected() {
    let fixture = generate_central_fixture();
    let mut asset = pv_input(AssetId::new(uid(0xB004)), north_site(), &fixture, "site-north-pv-1", 1.5);
    asset.class = AssetClass::Bess;
    asset.contribution = AssetContribution::Dispatchable;
    asset.declared_p_net_mw = BoundInterval::new(Some(-1.0), Some(1.0)).unwrap();
    asset.energy_coupling = None;
    assert_eq!(
        compute(&fixture, vec![asset], vec![north_coupling(None)], vec![]),
        Err(EnvelopeError::EnergyCouplingRequiredForBess {
            asset_id: AssetId::new(uid(0xB004))
        })
    );
}

#[test]
fn an_unknown_provenance_site_limit_collapses_to_zero_headroom() {
    let fixture = generate_central_fixture();
    let envelope = compute(
        &fixture,
        gate_assets(&fixture),
        vec![north_coupling(Some(ProvenancedScalar {
            value: 5.0,
            provenance: CapabilityProvenance::Unknown,
        }))],
        vec![],
    )
    .unwrap();

    // The frozen P3.02 fallback for SITE_NETWORK collapses an
    // unknown-provenance limit to zero: the aggregate export is zero even
    // though the forecast is high (never widened, never "unconstrained").
    let noon = &envelope.intervals()[720];
    assert_eq!(noon.upper_mw, 0.0);
}

#[test]
fn a_site_import_limit_that_crosses_the_aggregate_makes_the_interval_infeasible() {
    let fixture = generate_central_fixture();
    let asset = AssetEnvelopeInput {
        asset_id: AssetId::new(uid(0xB003)),
        site_id: north_site(),
        class: AssetClass::FlexibleLoad,
        capability_profile_reference: "profile:load_baseline_reducible".to_string(),
        declared_p_net_mw: BoundInterval::new(Some(-0.3), Some(0.2)).expect("valid range"),
        p_net_provenance: CapabilityProvenance::Measured,
        availability: AssetAvailabilityInput::Declared {
            fractions: full_availability(),
            source: availability_source(),
        },
        contribution: AssetContribution::ForecastBaselineLoad {
            series: series(&fixture, "site-north-load-1"),
            reducible_mw: ProvenancedScalar {
                value: 0.3,
                provenance: CapabilityProvenance::Measured,
            },
            increasable_mw: ProvenancedScalar {
                value: 0.2,
                provenance: CapabilityProvenance::Measured,
            },
        },
        energy_coupling: None,
    };
    // Zero import headroom against a load that always imports: the site
    // range is empty at night — infeasible, never repaired.
    let coupling = SiteCouplingInput {
        site_id: north_site(),
        point_of_common_coupling: "PCC-NORTH-01".to_string(),
        grid_model_reference: "grid-model:north@v1".to_string(),
        export_limit_mw: None,
        import_limit_mw: Some(ProvenancedScalar {
            value: 0.0,
            provenance: CapabilityProvenance::Measured,
        }),
    };
    let envelope = compute(&fixture, vec![asset], vec![coupling], vec![]).unwrap();
    let night = &envelope.intervals()[0];
    assert_eq!(
        night.status,
        EnvelopeIntervalStatus::Infeasible {
            cause: InfeasibilityCause::SiteRangeEmpty { site_id: north_site() }
        }
    );
    assert_eq!((night.lower_mw, night.upper_mw), (0.0, 0.0));
}

// --- Loss/adjustment term ------------------------------------------------------

#[test]
fn the_loss_adjustment_term_is_subtracted_from_the_aggregate() {
    let fixture = generate_central_fixture();
    let without = compute(
        &fixture,
        vec![pv_input(AssetId::new(uid(0xB001)), north_site(), &fixture, "site-north-pv-1", 1.5)],
        vec![north_coupling(None)],
        vec![],
    )
    .unwrap();
    let with = calculate_portfolio_envelope(EnvelopeComputationInput {
        envelope_version: "env-v1",
        computed_at: computed_at(),
        tenant_id: tenant(),
        portfolio_id: portfolio(),
        grid: grid(),
        fixture: &fixture,
        assets: vec![pv_input(
            AssetId::new(uid(0xB001)),
            north_site(),
            &fixture,
            "site-north-pv-1",
            1.5,
        )],
        site_couplings: vec![north_coupling(None)],
        correlation_groups: vec![],
        loss_adjustment: LossAdjustment::Constant {
            mw: 0.1,
            provenance: CapabilityProvenance::Measured,
        },
        nameplate_sum_mw: 4.0,
    })
    .unwrap();
    let noon_w = &with.intervals()[720];
    let noon_wo = &without.intervals()[720];
    assert!((noon_w.upper_mw - (noon_wo.upper_mw - 0.1)).abs() < 1e-9);
    assert!((noon_w.lower_mw - (noon_wo.lower_mw - 0.1)).abs() < 1e-9);
}

#[test]
fn an_unmodelled_loss_term_is_a_recorded_unknown_dimension() {
    let fixture = generate_central_fixture();
    let envelope = compute(
        &fixture,
        vec![pv_input(AssetId::new(uid(0xB001)), north_site(), &fixture, "site-north-pv-1", 1.5)],
        vec![north_coupling(None)],
        vec![],
    )
    .unwrap();
    assert!(
        envelope
            .unknown_dimensions()
            .iter()
            .any(|d| d.kind == EnvelopeUnknownKind::LossAdjustmentUnmodelled),
        "the loss omission is recorded, never silent"
    );
    assert!(
        envelope
            .unknown_dimensions()
            .iter()
            .any(|d| d.kind == EnvelopeUnknownKind::PortfolioLevelLimitUnknown),
        "the absent portfolio-level coupling limit is recorded"
    );
    assert!(
        envelope
            .unknown_dimensions()
            .iter()
            .any(|d| d.kind == EnvelopeUnknownKind::ReactiveEnvelopeNotComposed),
        "the active-power-only scope is recorded"
    );

    // An UNKNOWN-provenance loss constant is not credited (unknown never
    // widens) and the non-credit is recorded.
    let with_unknown_loss = calculate_portfolio_envelope(EnvelopeComputationInput {
        envelope_version: "env-v1",
        computed_at: computed_at(),
        tenant_id: tenant(),
        portfolio_id: portfolio(),
        grid: grid(),
        fixture: &fixture,
        assets: vec![pv_input(AssetId::new(uid(0xB001)), north_site(), &fixture, "site-north-pv-1", 1.5)],
        site_couplings: vec![north_coupling(None)],
        correlation_groups: vec![],
        loss_adjustment: LossAdjustment::Constant {
            mw: 0.5,
            provenance: CapabilityProvenance::Unknown,
        },
        nameplate_sum_mw: 4.0,
    })
    .unwrap();
    assert!(
        with_unknown_loss
            .unknown_dimensions()
            .iter()
            .any(|d| d.kind == EnvelopeUnknownKind::LossAdjustmentUnknownNotCredited)
    );
    let noon = &with_unknown_loss.intervals()[720];
    let forecast = pv_series_value(&fixture, "site-north-pv-1", 720);
    assert!((noon.upper_mw - forecast).abs() < 1e-9, "no loss term was credited");
}

// --- Correlation treatment -----------------------------------------------------

#[test]
fn the_declared_shared_exposure_derate_narrows_the_aggregate() {
    let fixture = generate_central_fixture();
    let group = SharedExposureGroup {
        group_id: "north-weather".to_string(),
        asset_ids: vec![AssetId::new(uid(0xB001)), AssetId::new(uid(0xB002))],
        derate_fraction: 0.2,
        basis: "ASSUMPTION: shared clear-sky exposure behind one PCC; conservative coincident derate; no correlation coefficient on record".to_string(),
    };
    let derated = compute(
        &fixture,
        gate_assets(&fixture),
        vec![north_coupling(None)],
        vec![group],
    )
    .unwrap();
    let plain = compute(&fixture, gate_assets(&fixture), vec![north_coupling(None)], vec![]).unwrap();

    let noon_d = &derated.intervals()[720];
    let noon_p = &plain.intervals()[720];
    assert!(
        noon_d.upper_mw < noon_p.upper_mw - 1e-9,
        "the derate must strictly narrow the aggregate"
    );
    // Both bounds scaled by (1 - 0.2): upper = 0.8 * plain upper.
    assert!((noon_d.upper_mw - 0.8 * noon_p.upper_mw).abs() < 1e-9);
    assert_eq!(
        derated.correlation_treatment().applied_groups,
        vec!["north-weather".to_string()]
    );
    assert!(derated.correlation_treatment().unmodelled_assets.is_empty());
}

#[test]
fn ungrouped_assets_are_recorded_as_correlation_unmodelled() {
    let fixture = generate_central_fixture();
    let envelope = compute(
        &fixture,
        gate_assets(&fixture),
        vec![north_coupling(None)],
        vec![],
    )
    .unwrap();
    let record = envelope.correlation_treatment();
    assert_eq!(record.statement, CORRELATION_TREATMENT_STATEMENT);
    assert!(record.applied_groups.is_empty());
    assert_eq!(
        record.unmodelled_assets,
        vec![AssetId::new(uid(0xB001)), AssetId::new(uid(0xB002))],
        "assets of no group have NO correlation model on record and none is assumed"
    );
}

#[test]
fn a_group_without_a_recorded_basis_is_rejected() {
    let fixture = generate_central_fixture();
    let group = SharedExposureGroup {
        group_id: "north-weather".to_string(),
        asset_ids: vec![AssetId::new(uid(0xB001))],
        derate_fraction: 0.2,
        basis: "   ".to_string(),
    };
    assert!(
        matches!(
            compute(&fixture, gate_assets(&fixture), vec![north_coupling(None)], vec![group]),
            Err(EnvelopeError::InvalidSharedExposureGroup { .. })
        ),
        "the derate basis (labelled ASSUMPTION) is mandatory; no silent correlation assumption"
    );
}

// --- Descriptor, snapshot, metadata --------------------------------------------

#[test]
fn the_availability_descriptor_explicitly_denies_confidence_support() {
    let fixture = generate_central_fixture();
    let envelope = compute(
        &fixture,
        vec![pv_input(AssetId::new(uid(0xB001)), north_site(), &fixture, "site-north-pv-1", 1.5)],
        vec![north_coupling(None)],
        vec![],
    )
    .unwrap();
    assert_eq!(envelope.availability_descriptor(), AVAILABILITY_DESCRIPTOR_NOT_SUPPORTED);
    assert!(envelope.availability_descriptor().starts_with("NOT SUPPORTED"));
}

#[test]
fn the_input_snapshot_carries_the_synthetic_label_and_forecast_version() {
    let fixture = generate_central_fixture();
    let envelope = compute(
        &fixture,
        gate_assets(&fixture),
        vec![north_coupling(None)],
        vec![],
    )
    .unwrap();
    let snapshot = envelope.input_snapshot();
    assert_eq!(snapshot.forecast_provenance, flexcore_application::forecast_fixture::Provenance::Synthetic);
    assert_eq!(snapshot.forecast_version.as_str(), "v1");
    assert_eq!(snapshot.fixture_id, "synthetic-forecast-fixture");
    assert_eq!(
        snapshot.capability_profile_references,
        vec![
            "profile:pv_clear_sky".to_string(),
            "profile:pv_clear_sky".to_string()
        ]
    );
    assert_eq!(snapshot.site_pcc_references, vec!["PCC-NORTH-01".to_string()]);

    // The P3.13 metadata: version, timestamp, snapshot reference, horizon,
    // sorted scoped assets.
    let metadata = envelope.metadata();
    assert_eq!(metadata.envelope_version.as_str(), "env-v1");
    assert_eq!(metadata.computed_at, computed_at());
    assert!(metadata.input_snapshot_ref.contains("synthetic-forecast-fixture"));
    assert!(metadata.input_snapshot_ref.contains("v1"));
    assert_eq!(metadata.forecast_version, "v1");
    assert_eq!(metadata.horizon_start_millis, HORIZON_START_MILLIS);
    assert_eq!(metadata.horizon_end_millis, HORIZON_START_MILLIS + 86_400_000);
    assert_eq!(
        metadata.scoped_asset_ids,
        vec![AssetId::new(uid(0xB001)), AssetId::new(uid(0xB002))]
    );
}

#[test]
fn a_fixture_on_a_foreign_grid_is_rejected() {
    let fixture = generate_central_fixture();
    let foreign_grid = SchedulingGrid::new(
        ValidFrom::from_unix_millis(HORIZON_START_MILLIS + 60_000).expect("valid timestamp"),
        SchedulingGridConfig::new(86_400_000, 60_000).expect("valid grid"),
    );
    let result = calculate_portfolio_envelope(EnvelopeComputationInput {
        envelope_version: "env-v1",
        computed_at: computed_at(),
        tenant_id: tenant(),
        portfolio_id: portfolio(),
        grid: foreign_grid,
        fixture: &fixture,
        assets: gate_assets(&fixture),
        site_couplings: vec![north_coupling(None)],
        correlation_groups: vec![],
        loss_adjustment: LossAdjustment::Unmodelled,
        nameplate_sum_mw: 4.0,
    });
    assert!(
        matches!(result, Err(EnvelopeError::ForecastGridMismatch { .. })),
        "the envelope consumes the fixture on the fixture's own grid (D-FX-1)"
    );
}

#[test]
fn structural_input_errors_are_named() {
    let fixture = generate_central_fixture();

    // No assets.
    assert_eq!(
        compute(&fixture, vec![], vec![north_coupling(None)], vec![]),
        Err(EnvelopeError::NoAssets)
    );
    // No site couplings.
    assert_eq!(
        compute(&fixture, gate_assets(&fixture), vec![], vec![]),
        Err(EnvelopeError::NoSiteCouplings)
    );
    // An asset whose site has no coupling.
    let orphan = pv_input(AssetId::new(uid(0xB001)), south_site(), &fixture, "site-north-pv-1", 1.5);
    assert_eq!(
        compute(&fixture, vec![orphan], vec![north_coupling(None)], vec![]),
        Err(EnvelopeError::MissingSiteCoupling { site_id: south_site() })
    );
    // A wrong-series quantity (a load series behind a PV cap).
    let wrong = AssetEnvelopeInput {
        contribution: AssetContribution::ForecastCappedPv {
            series: series(&fixture, "site-north-load-1"),
        },
        ..pv_input(AssetId::new(uid(0xB001)), north_site(), &fixture, "site-north-pv-1", 1.5)
    };
    assert!(matches!(
        compute(&fixture, vec![wrong], vec![north_coupling(None)], vec![]),
        Err(EnvelopeError::ForecastQuantityMismatch { .. })
    ));
    // An inverted declared range.
    let inverted = pv_input(AssetId::new(uid(0xB001)), north_site(), &fixture, "site-north-pv-1", 1.5);
    let mut inverted = inverted;
    inverted.declared_p_net_mw = BoundInterval::new(Some(2.0), Some(1.0)).unwrap();
    assert_eq!(
        compute(&fixture, vec![inverted], vec![north_coupling(None)], vec![]),
        Err(EnvelopeError::InvertedDeclaredRange { asset_id: AssetId::new(uid(0xB001)) })
    );
    // A duplicate asset.
    let dup = gate_assets(&fixture);
    let duplicate = {
        let mut v = dup.clone();
        v.push(pv_input(AssetId::new(uid(0xB001)), north_site(), &fixture, "site-north-pv-1", 1.5));
        v
    };
    assert_eq!(
        compute(&fixture, duplicate, vec![north_coupling(None)], vec![]),
        Err(EnvelopeError::DuplicateAsset { asset_id: AssetId::new(uid(0xB001)) })
    );
}

// --- FlexibilityEnvelopeUpdated publication ------------------------------------

#[test]
fn the_publication_uses_the_frozen_subject_and_carries_the_version_in_the_key() {
    let version = flexcore_application::flexibility_envelope::EnvelopeVersion::new("env-v1")
        .expect("valid version");
    let publications = envelope_updated_publications(
        flexcore_application::flexibility_envelope::EnvelopePublicationInput {
            correlation_id: uid(0xC001),
            tenant_id: tenant(),
            portfolio_id: portfolio(),
            site_ids: &[north_site(), south_site()],
            envelope_version: &version,
            event_time: computed_at(),
        },
    )
    .unwrap();

    assert_eq!(publications.len(), 2, "one event per covered site");
    for (index, publication) in publications.iter().enumerate() {
        assert_eq!(
            publication.destination_subject.as_str(),
            "flexcore.v1.flexibility.flexibility_envelope_updated"
        );
        let expected_site = if index == 0 { north_site() } else { south_site() };
        assert_eq!(
            publication.idempotency_key,
            format!(
                "flexibility_envelope_updated:{}:{}:env-v1",
                portfolio(),
                expected_site
            )
        );
        // The frozen payload shape: the nil asset id denotes the envelope
        // scope (the recorded payload-fit gap).
        match &publication.envelope.event {
            flexcore_events::events::DomainEvent::FlexibilityEnvelopeUpdated(payload) => {
                assert_eq!(payload.tenant_id, tenant());
                assert_eq!(payload.portfolio_id, portfolio());
                assert_eq!(payload.site_id, expected_site);
                assert_eq!(payload.asset_id.uuid(), FlexUuid::nil());
                assert_eq!(payload.event_time, computed_at());
            }
            other => panic!("expected FlexibilityEnvelopeUpdated, got {other:?}"),
        }
    }
    // The recorded payload-fit gap note is a visible constant.
    assert!(ENVELOPE_PAYLOAD_GAP_NOTE.contains("serde(default)"));
}

#[test]
fn the_publication_rejects_an_empty_or_duplicated_site_list() {
    let version = flexcore_application::flexibility_envelope::EnvelopeVersion::new("env-v1").unwrap();
    assert_eq!(
        envelope_updated_publications(
            flexcore_application::flexibility_envelope::EnvelopePublicationInput {
                correlation_id: uid(0xC001),
                tenant_id: tenant(),
                portfolio_id: portfolio(),
                site_ids: &[],
                envelope_version: &version,
                event_time: computed_at(),
            }
        ),
        Err(EnvelopeError::PublicationWithoutSite)
    );
    assert_eq!(
        envelope_updated_publications(
            flexcore_application::flexibility_envelope::EnvelopePublicationInput {
                correlation_id: uid(0xC001),
                tenant_id: tenant(),
                portfolio_id: portfolio(),
                site_ids: &[north_site(), north_site()],
                envelope_version: &version,
                event_time: computed_at(),
            }
        ),
        Err(EnvelopeError::DuplicatePublicationSite { site_id: north_site() })
    );
}
