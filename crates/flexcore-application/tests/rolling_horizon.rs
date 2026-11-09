// P3.24 — Rolling-horizon re-scheduling tests (report §12.1, §24 steps 3–4;
//         ledger P3.24).
//
// Covered, without any solver or I/O (the roll planning is a pure
// application function; the re-optimized window is the composition root's
// solve through the `Optimizer` port):
//
// - the trigger set (ledger step 1) is closed and its stored strings match
//   the storage crate's `SCHEDULE_ROLL_TRIGGERS` exactly (the string list is
//   pinned here and in flexcore-storage);
// - the mid-horizon dropout scenario (ledger acceptance): a re-solve after a
//   mid-horizon EV opt-out (P3.11 semantics) plans a new revision that
//   respects the realised SOC, keeps the past intervals unchanged (frozen
//   realised inputs, never the prior plan's values), excludes the dropped
//   asset from the re-optimized window, and keeps the supersede chain
//   intact (prior revision 1 → new revision 2, §14.2 SUPERSEDED);
// - realised SOC outside [0,1] and missing realised coverage are rejected
//   with named errors;
// - the trigger/boundary consistency (D-P3.24-2) and the horizon-advance
//   window shift;
// - the projection pastes frozen + future continuously and enforces the
//   `DispatchedAfterOptOut` discipline;
// - `ScheduleProposed` assembles per new revision through the P3.21
//   machinery (the frozen payload cannot carry the trigger or the supersede
//   link — the gap is recorded, and they travel on the revision row).

use flexcore_application::forecast_fixture::{
    ForecastQuantity, ForecastVersion, LabeledSeriesValues, Provenance,
};
use flexcore_application::flexibility_envelope::EnvelopeVersion;
use flexcore_application::optimization_result_record::{
    assemble_schedule_result_record, schedule_proposed_idempotency_key, GridModelTraceability,
    InputSnapshotRecord, PersistedResultRecord, ScheduleResultAssemblyInput,
};
use flexcore_application::rolling_horizon::{
    plan_rolling_horizon_roll, project_rolling_schedule,
    rolling_horizon_schedule_proposed_publication, AssetDropout, AssetSetpoint, CarriedAssetState,
    IntervalPlan, PriorRevision, RealisedAssetValue, RealisedInterval, RealisedState,
    RollRequest, RollTrigger, CARRY_FORWARD_STATEMENT, FROZEN_INTERVAL_STATEMENT,
    ROLL_TRIGGER_STATEMENT, SCHEDULE_PROPOSED_TRIGGER_GAP_NOTE, SCHEDULE_ROLL_TRIGGER_STRINGS,
};
use flexcore_application::scheduling_objective::ObjectiveConfig;
use flexcore_contracts::capability::CAPABILITY_SCHEMA_VERSION;
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::{EventTime, SiteTimezone};
use flexcore_domain::entities::{Portfolio, Site, Tenant};
use flexcore_domain::ids::{PortfolioId, SiteId, TenantId};
use flexcore_events::subject::subject_for;
use flexcore_optimization_api::config::SolverConfig;
use flexcore_optimization_api::problem::ProblemClass;
use flexcore_optimization_api::result::{
    Diagnostics, OptimizationResult, PrimalDiagnostics, ResidualReport, SolveStatus, VariableValue,
};
use std::time::Duration;

const T0: i64 = 1_700_000_000_000;
const HOUR: i64 = 3_600_000;

fn tenant_id() -> TenantId {
    TenantId::new(FlexUuid::parse("00000000-0000-0000-0000-00000000d001").expect("parses"))
}

fn portfolio_id() -> PortfolioId {
    PortfolioId::new(FlexUuid::parse("00000000-0000-0000-0000-00000000d002").expect("parses"))
}

fn site_id() -> SiteId {
    SiteId::new(FlexUuid::parse("00000000-0000-0000-0000-00000000d003").expect("parses"))
}

const SCHEDULE_ID: &str = "0192a5e0-0000-7000-8000-0000000000d1";

/// The prior revision: 8 hourly intervals over [T0, T0 + 8h), covering the
/// BESS and the EV fleet. The PRIOR PLAN's values are deliberately different
/// from the realised values used in the tests, so "past unchanged" can only
/// mean the realised values were kept.
fn prior_revision() -> PriorRevision {
    let mut intervals = Vec::new();
    for hour in 0..8 {
        let start = T0 + hour * HOUR;
        intervals.push(IntervalPlan {
            interval_start_millis: start,
            interval_end_millis: start + HOUR,
            setpoints: vec![
                AssetSetpoint { asset_key: "bess".to_string(), p_mw: 1.0 },
                AssetSetpoint { asset_key: "ev-fleet".to_string(), p_mw: 0.5 },
            ],
        });
    }
    PriorRevision {
        schedule_id: SCHEDULE_ID.to_string(),
        revision: 1,
        candidate_id: "0192a5e0-0000-7000-8000-0000000000d2".to_string(),
        horizon_start_millis: T0,
        horizon_end_millis: T0 + 8 * HOUR,
        interval_millis: HOUR,
        asset_keys: vec!["bess".to_string(), "ev-fleet".to_string()],
        intervals,
    }
}

/// The realised state at T0 + 4h: actual SOC carried forward, actual load
/// response for the four executed intervals, and the mid-horizon EV opt-out
/// at T0 + 2h (P3.11 semantics).
fn dropout_realised_state() -> RealisedState {
    let mut realised_intervals = Vec::new();
    for hour in 0..4 {
        let start = T0 + hour * HOUR;
        let values = if start >= T0 + 2 * HOUR {
            // After the EV opt-out, only the BESS has a realised value.
            vec![RealisedAssetValue { asset_key: "bess".to_string(), realised_p_mw: 0.8 }]
        } else {
            vec![
                RealisedAssetValue { asset_key: "bess".to_string(), realised_p_mw: 0.8 },
                RealisedAssetValue { asset_key: "ev-fleet".to_string(), realised_p_mw: 0.4 },
            ]
        };
        realised_intervals.push(RealisedInterval {
            interval_start_millis: start,
            interval_end_millis: start + HOUR,
            values,
        });
    }
    RealisedState {
        executed_until_millis: T0 + 4 * HOUR,
        carried_assets: vec![CarriedAssetState {
            asset_key: "bess".to_string(),
            realised_soc: Some(0.62),
        }],
        realised_intervals,
        dropouts: vec![AssetDropout {
            asset_key: "ev-fleet".to_string(),
            from_millis: T0 + 2 * HOUR,
        }],
    }
}

#[test]
fn the_trigger_set_is_closed_and_matches_the_storage_vocabulary_exactly() {
    // The typed set and the stored string set agree — the drift guard.
    let strings: Vec<&str> = RollTrigger::all().iter().map(|t| t.as_str()).collect();
    assert_eq!(strings, SCHEDULE_ROLL_TRIGGER_STRINGS.to_vec());
    assert_eq!(
        SCHEDULE_ROLL_TRIGGER_STRINGS,
        [
            "initial_solve",
            "horizon_advance",
            "envelope_invalidation",
            "asset_opt_out_dropout",
            "forecast_update",
            "operator_request",
        ]
    );
    assert!(ROLL_TRIGGER_STATEMENT.contains("horizon_advance"));
    assert!(ROLL_TRIGGER_STATEMENT.contains("asset_opt_out_dropout"));
    assert!(ROLL_TRIGGER_STATEMENT.contains("REQUIRED"));
    assert!(FROZEN_INTERVAL_STATEMENT.contains("REALISED"));
    assert!(CARRY_FORWARD_STATEMENT.contains("[0,1]"));
}

#[test]
fn the_mid_horizon_dropout_respects_realised_soc_freezes_the_past_and_keeps_the_chain() {
    let prior = prior_revision();
    let request = RollRequest {
        trigger: RollTrigger::AssetOptOutDropout,
        trigger_detail: "vehicle opt-out received at T0+2h (P3.11)".to_string(),
        realised: dropout_realised_state(),
    };
    let plan = plan_rolling_horizon_roll(&prior, &request)
        .expect("the mid-horizon dropout roll plans");

    // The supersede chain is intact (P3.22): prior 1 superseded by new 2,
    // same schedule id, no gaps.
    assert_eq!(plan.superseded.schedule_id, SCHEDULE_ID);
    assert_eq!(plan.superseded.revision, 1);
    assert_eq!(plan.new_revision.schedule_id, SCHEDULE_ID);
    assert_eq!(plan.new_revision.revision, 2);
    assert_eq!(plan.trigger_storage_string(), "asset_opt_out_dropout");
    assert_eq!(plan.trigger, RollTrigger::AssetOptOutDropout);

    // Realised-state carry-forward: the actual SOC enters the new solve as
    // an initial condition (never re-solved from forecasts).
    assert_eq!(plan.initial_conditions.len(), 1);
    assert_eq!(plan.initial_conditions[0].asset_key, "bess");
    assert_eq!(plan.initial_conditions[0].realised_soc, Some(0.62));

    // Only PAST intervals are frozen (started before the boundary); they
    // carry the REALISED values, never the prior plan's values.
    assert_eq!(plan.frozen_intervals.len(), 4);
    for (index, frozen) in plan.frozen_intervals.iter().enumerate() {
        assert_eq!(frozen.interval_start_millis, T0 + (index as i64) * HOUR);
        let bess = frozen
            .setpoints
            .iter()
            .find(|s| s.asset_key == "bess")
            .expect("the BESS has a realised value");
        assert_eq!(
            bess.p_mw, 0.8,
            "the frozen interval carries the REALISED value, not the prior plan's 1.0"
        );
    }
    // The in-progress boundary: interval 3 (T0+3h) is frozen; interval 4
    // (T0+4h) is future.
    assert_eq!(plan.reoptimize_start_millis, T0 + 4 * HOUR);
    assert_eq!(plan.reoptimize_end_millis, T0 + 8 * HOUR);

    // The dropped asset left the re-optimized window from its opt-out time
    // (which precedes the boundary); the dropout stays recorded.
    assert_eq!(plan.active_asset_keys, vec!["bess".to_string()]);
    assert_eq!(plan.dropouts.len(), 1);
    assert!(plan
        .notes
        .iter()
        .any(|n| n.contains("ev-fleet") && n.contains("P3.11")));

    // The prior revision's own intervals are untouched (no in-place
    // mutation anywhere — the plan holds NEW interval values).
    assert_eq!(prior.intervals[0].setpoints[0].p_mw, 1.0);
}

#[test]
fn the_projection_pastes_frozen_and_future_continuously_with_the_optout_enforced() {
    let prior = prior_revision();
    let request = RollRequest {
        trigger: RollTrigger::AssetOptOutDropout,
        trigger_detail: "mid-horizon EV opt-out".to_string(),
        realised: dropout_realised_state(),
    };
    let plan = plan_rolling_horizon_roll(&prior, &request).expect("plans");

    // The re-solve of the future window (composition root / `Optimizer`
    // port) produced only-BESS intervals respecting the carried SOC.
    let mut future = Vec::new();
    for hour in 4..8 {
        let start = T0 + hour * HOUR;
        future.push(IntervalPlan {
            interval_start_millis: start,
            interval_end_millis: start + HOUR,
            setpoints: vec![AssetSetpoint { asset_key: "bess".to_string(), p_mw: -0.7 }],
        });
    }
    let projection = project_rolling_schedule(&plan, future).expect("the projection pastes");

    // 8 intervals: 4 frozen (realised) + 4 re-optimized (future).
    assert_eq!(projection.intervals.len(), 8);
    assert_eq!(projection.revision, plan.new_revision);

    // The past is unchanged: the frozen half equals the realised inputs
    // verbatim; the future half is the re-solve.
    for index in 0..4 {
        let interval = &projection.intervals[index];
        assert_eq!(interval.interval_start_millis, T0 + (index as i64) * HOUR);
        assert_eq!(interval.setpoints.len(), 1);
        assert_eq!(interval.setpoints[0].p_mw, 0.8, "frozen realised value");
    }
    for index in 4..8 {
        let interval = &projection.intervals[index];
        assert_eq!(interval.setpoints[0].asset_key, "bess");
        assert_eq!(interval.setpoints[0].p_mw, -0.7, "re-optimized future value");
    }
    // The carried SOC is the initial condition of the re-optimized half.
    assert_eq!(projection.initial_conditions[0].realised_soc, Some(0.62));

    // Dispatching the dropped asset at or after its opt-out instant is
    // unrepresentable (P3.11).
    let mut future_with_ev = Vec::new();
    for hour in 4..8 {
        let start = T0 + hour * HOUR;
        future_with_ev.push(IntervalPlan {
            interval_start_millis: start,
            interval_end_millis: start + HOUR,
            setpoints: vec![
                AssetSetpoint { asset_key: "bess".to_string(), p_mw: -0.7 },
                AssetSetpoint { asset_key: "ev-fleet".to_string(), p_mw: 0.5 },
            ],
        });
    }
    let error =
        project_rolling_schedule(&plan, future_with_ev).expect_err("an opted-out asset is never dispatched");
    assert!(matches!(error, flexcore_application::rolling_horizon::RollingHorizonError::DispatchedAfterOptOut { .. }));

    // A future interval starting before the boundary is a projection gap —
    // the past is never re-solved.
    let re_solve_the_past = vec![IntervalPlan {
        interval_start_millis: T0 + 3 * HOUR,
        interval_end_millis: T0 + 4 * HOUR,
        setpoints: vec![AssetSetpoint { asset_key: "bess".to_string(), p_mw: 9.9 }],
    }];
    assert!(matches!(
        project_rolling_schedule(&plan, re_solve_the_past),
        Err(flexcore_application::rolling_horizon::RollingHorizonError::ProjectionGap { .. })
    ));
}

#[test]
fn realised_state_violations_are_rejected_with_named_errors() {
    // SOC outside [0,1] (the canonical fraction unit, §10.2).
    let mut realised = dropout_realised_state();
    realised.carried_assets[0].realised_soc = Some(1.5);
    assert!(matches!(
        plan_rolling_horizon_roll(
            &prior_revision(),
            &RollRequest {
                trigger: RollTrigger::AssetOptOutDropout,
                trigger_detail: "x".to_string(),
                realised,
            }
        ),
        Err(flexcore_application::rolling_horizon::RollingHorizonError::SocOutOfRange { .. })
    ));

    // A frozen interval without a realised record.
    let mut missing_interval = dropout_realised_state();
    missing_interval.realised_intervals.remove(2);
    assert!(matches!(
        plan_rolling_horizon_roll(
            &prior_revision(),
            &RollRequest {
                trigger: RollTrigger::AssetOptOutDropout,
                trigger_detail: "x".to_string(),
                realised: missing_interval,
            }
        ),
        Err(flexcore_application::rolling_horizon::RollingHorizonError::MissingRealisedInterval { .. })
    ));

    // A frozen (interval, asset) pair without a realised value and without
    // a covering dropout.
    let mut missing_value = dropout_realised_state();
    missing_value.realised_intervals[0]
        .values
        .retain(|v| v.asset_key != "ev-fleet");
    assert!(matches!(
        plan_rolling_horizon_roll(
            &prior_revision(),
            &RollRequest {
                trigger: RollTrigger::AssetOptOutDropout,
                trigger_detail: "x".to_string(),
                realised: missing_value,
            }
        ),
        Err(flexcore_application::rolling_horizon::RollingHorizonError::MissingRealisedValue { .. })
    ));

    // A non-grid-aligned realised interval.
    let mut misaligned = dropout_realised_state();
    misaligned.realised_intervals[0].interval_start_millis = T0 + 1;
    assert!(matches!(
        plan_rolling_horizon_roll(
            &prior_revision(),
            &RollRequest {
                trigger: RollTrigger::AssetOptOutDropout,
                trigger_detail: "x".to_string(),
                realised: misaligned,
            }
        ),
        Err(flexcore_application::rolling_horizon::RollingHorizonError::MisalignedInterval { .. })
    ));
}

#[test]
fn the_trigger_must_match_the_execution_boundary_and_horizon_advance_shifts_the_window() {
    // horizon_advance BEFORE the horizon is consumed is a mismatch.
    assert!(matches!(
        plan_rolling_horizon_roll(
            &prior_revision(),
            &RollRequest {
                trigger: RollTrigger::HorizonAdvance,
                trigger_detail: "x".to_string(),
                realised: dropout_realised_state(),
            }
        ),
        Err(flexcore_application::rolling_horizon::RollingHorizonError::TriggerHorizonMismatch { .. })
    ));

    // A mid-horizon trigger AT the horizon end is a mismatch: the caller
    // should use horizon_advance.
    let mut at_end = dropout_realised_state();
    at_end.executed_until_millis = T0 + 8 * HOUR;
    // All 8 intervals are frozen at the horizon end; realised coverage is
    // required for all of them.
    for hour in 4..8 {
        let start = T0 + hour * HOUR;
        at_end.realised_intervals.push(RealisedInterval {
            interval_start_millis: start,
            interval_end_millis: start + HOUR,
            values: vec![
                RealisedAssetValue { asset_key: "bess".to_string(), realised_p_mw: 0.8 },
                RealisedAssetValue { asset_key: "ev-fleet".to_string(), realised_p_mw: 0.4 },
            ],
        });
    }
    at_end.dropouts.clear();
    assert!(matches!(
        plan_rolling_horizon_roll(
            &prior_revision(),
            &RollRequest {
                trigger: RollTrigger::ForecastUpdate,
                trigger_detail: "x".to_string(),
                realised: at_end,
            }
        ),
        Err(flexcore_application::rolling_horizon::RollingHorizonError::TriggerHorizonMismatch { .. })
    ));

    // horizon_advance AT the horizon end shifts the same-length window and
    // supersedes the prior revision (the P3.03 rollover rule — never a
    // silent overwrite).
    let mut rollover = dropout_realised_state();
    rollover.executed_until_millis = T0 + 8 * HOUR;
    for hour in 4..8 {
        let start = T0 + hour * HOUR;
        rollover.realised_intervals.push(RealisedInterval {
            interval_start_millis: start,
            interval_end_millis: start + HOUR,
            values: vec![
                RealisedAssetValue { asset_key: "bess".to_string(), realised_p_mw: 0.8 },
                RealisedAssetValue { asset_key: "ev-fleet".to_string(), realised_p_mw: 0.4 },
            ],
        });
    }
    rollover.dropouts.clear();
    rollover.carried_assets.push(CarriedAssetState {
        asset_key: "ev-fleet".to_string(),
        realised_soc: Some(0.9),
    });
    let plan = plan_rolling_horizon_roll(
        &prior_revision(),
        &RollRequest {
            trigger: RollTrigger::HorizonAdvance,
            trigger_detail: "24h rollover".to_string(),
            realised: rollover,
        },
    )
    .expect("the rollover plans");
    assert_eq!(plan.new_horizon_start_millis, T0 + 8 * HOUR);
    assert_eq!(plan.new_horizon_end_millis, T0 + 16 * HOUR);
    assert_eq!(plan.reoptimize_start_millis, T0 + 8 * HOUR);
    assert!(plan.frozen_intervals.is_empty(), "a new horizon has no frozen intervals of its own");
    assert_eq!(plan.new_revision.revision, 2);
    assert_eq!(plan.trigger_storage_string(), "horizon_advance");
}

// --- ScheduleProposed per new revision (P3.21 machinery reused) ---------------

/// The minimal domain chain the P3.21 grid-model traceability is constructed
/// from (the traceability is built EXCLUSIVELY from a domain `Site`, whose
/// grid-model reference supplies the version).
fn domain_site() -> Site {
    let created = EventTime::from_unix_millis(T0).expect("valid");
    let tenant = Tenant::try_new(
        tenant_id(),
        "p3.24 tenant".to_string(),
        None,
        format!("idp-{}", tenant_id()),
        "default".to_string(),
        created,
    )
    .expect("tenant builds");
    let portfolio = Portfolio::try_new(
        &tenant,
        portfolio_id(),
        "p3.24 portfolio".to_string(),
        None,
        "operator".to_string(),
        "none (D1 OPEN)".to_string(),
        "{}".to_string(),
        created,
    )
    .expect("portfolio builds");
    Site::try_new(
        &portfolio,
        site_id(),
        "p3.24 site".to_string(),
        None,
        "synthetic".to_string(),
        SiteTimezone::new("UTC".to_string()).expect("UTC is valid"),
        "pcc-fixture".to_string(),
        "site-grid-model-reference-v1".to_string(),
        "edge-fixture".to_string(),
        created,
    )
    .expect("site builds")
}

fn sample_optimal_result(candidate: FlexUuid, snapshot_reference: &str) -> OptimizationResult {
    OptimizationResult {
        status: SolveStatus::Optimal,
        problem_class: ProblemClass::LinearProgram,
        solver_name: "highs".to_string(),
        solver_version: "1.15.1".to_string(),
        provider_id: "tier1-python-worker".to_string(),
        provider_version: "0.1.0".to_string(),
        objective_value: Some(-12.5),
        objective_units: Some("currency-units".to_string()),
        primal: Diagnostics::Available(PrimalDiagnostics {
            values: vec![VariableValue {
                name: "P_export_t0".to_string(),
                unit: "MW".to_string(),
                value: 0.8,
            }],
        }),
        dual: Diagnostics::NotAvailable {
            reason: "the test fixture does not expose duals".to_string(),
        },
        residuals: Diagnostics::Available(ResidualReport {
            max_primal_violation: Some(0.0),
            max_dual_violation: None,
            iteration_count: Some(7),
        }),
        infeasibility_evidence: Diagnostics::NotAvailable {
            reason: "not an infeasible result".to_string(),
        },
        runtime: Duration::from_millis(12),
        resources: None,
        candidate_id: Some(candidate),
        input_snapshot_id: Some(snapshot_reference.to_string()),
        data_timestamp: None,
        grid_model_version: None,
        uncertainty_and_assumptions: vec![
            "SYNTHETIC objective inputs (D1/D2 OPEN)".to_string()
        ],
        warnings: vec![],
        determinism: Some(flexcore_optimization_api::result::DeterminismRecord {
            solver_version: "1.15.1".to_string(),
            thread_count: 1,
            time_limit_ms: None,
            config_digest: "fnv1a64:0000000000000000".to_string(),
        }),
    }
}

#[test]
fn schedule_proposed_is_emitted_per_new_revision_and_the_trigger_gap_is_recorded() {
    let site = domain_site();
    let candidate = FlexUuid::parse("0192a5e0-0000-7000-8000-0000000000d3").expect("parses");

    let forecast_version = ForecastVersion::new("v1").expect("valid");
    let forecast_values = LabeledSeriesValues {
        provenance: Provenance::Synthetic,
        forecast_version: &forecast_version,
        quantity: ForecastQuantity::PvOutputMw,
        values: &[1.0],
    };
    let snapshot = InputSnapshotRecord::assemble(
        &[flexcore_application::optimization_result_record::CapabilityVersionRef {
            asset_key: "bess".to_string(),
            capability_profile_reference: "bess-profile-v1".to_string(),
            capability_schema_version: CAPABILITY_SCHEMA_VERSION,
        }],
        &EnvelopeVersion::new("env-v1").expect("valid"),
        forecast_values,
        &ObjectiveConfig::default_synthetic(),
        &SolverConfig::default(),
        "telemetry-snapshot-2026-11-07",
    )
    .expect("the snapshot assembles");
    let snapshot_reference = snapshot.reference();

    let record = assemble_schedule_result_record(ScheduleResultAssemblyInput {
        tenant_id: tenant_id(),
        portfolio_id: portfolio_id(),
        site_id: site_id(),
        result: sample_optimal_result(candidate, &snapshot_reference),
        grid_model: GridModelTraceability::from_site(&site, "grid-model-v1")
            .expect("the traceability builds from the site"),
        input_snapshot: snapshot,
        objective_scaling_warnings: vec![],
    })
    .expect("the record assembles");
    record
        .validate_record_complete()
        .expect("the record is complete");

    let key = schedule_proposed_idempotency_key(record.portfolio_id(), record.site_id(), record.candidate_id());
    let persisted = PersistedResultRecord::new(record.record_reference(), key.clone())
        .expect("the persistence proof builds");

    let plan = plan_rolling_horizon_roll(
        &prior_revision(),
        &RollRequest {
            trigger: RollTrigger::AssetOptOutDropout,
            trigger_detail: "mid-horizon EV opt-out".to_string(),
            realised: dropout_realised_state(),
        },
    )
    .expect("the roll plans");

    // The publication verifies the candidate against the record and the
    // persistence proof (the P3.21 machinery, reused) and the plan.
    let publication = rolling_horizon_schedule_proposed_publication(
        flexcore_application::rolling_horizon::RollingHorizonPublicationInput {
            record: &record,
            persisted: &persisted,
            plan: &plan,
            new_candidate_id: record.record_reference().to_string(),
            correlation_id: FlexUuid::nil(),
            event_time: EventTime::from_unix_millis(T0 + 4 * HOUR).expect("valid"),
        },
    )
    .expect("the publication assembles");
    assert_eq!(publication.idempotency_key, key);
    assert_eq!(
        subject_for(&publication.envelope.event).as_str(),
        "flexcore.v1.scheduling.schedule_proposed"
    );

    // A publication for a foreign candidate is rejected by name.
    let error = rolling_horizon_schedule_proposed_publication(
        flexcore_application::rolling_horizon::RollingHorizonPublicationInput {
            record: &record,
            persisted: &persisted,
            plan: &plan,
            new_candidate_id: "0192a5e0-0000-7000-8000-0000000000ff".to_string(),
            correlation_id: FlexUuid::nil(),
            event_time: EventTime::from_unix_millis(T0 + 4 * HOUR).expect("valid"),
        },
    )
    .expect_err("a foreign candidate is rejected");
    assert!(matches!(
        error,
        flexcore_application::rolling_horizon::RollingHorizonError::PublicationMismatch { .. }
    ));

    // The payload gap is recorded honestly: the trigger and the supersede
    // link travel on the revision row (migration 0014), not in the frozen
    // event payload.
    assert!(SCHEDULE_PROPOSED_TRIGGER_GAP_NOTE.contains("roll_trigger"));
    assert!(SCHEDULE_PROPOSED_TRIGGER_GAP_NOTE.contains("supersedes_revision"));
    assert!(SCHEDULE_PROPOSED_TRIGGER_GAP_NOTE.contains("serde(default)"));
}
