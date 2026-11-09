// P3.21 — Integration tests for the optimization result contract
//         (report §12.6; ledger P3.21 acceptance criteria).
//
// Covered here (the ledger's completeness discipline, per PROVIDER PATH,
// not just the happy path):
//
// - every §12.6 field is present on every assembled result (per provider
//   path: Tier 1 optimal / Tier 1 infeasible / Tier 1 timeout / Tier 2
//   optimal — the P3.19 risk note: "diagnostics dropped for one solver
//   path");
// - a result missing ANY required field fails validation (the port-level
//   `validate_complete` gate, exercised through the assembly);
// - residual/violation reports are populated OR explicitly not available
//   with a non-empty per-solver reason;
// - the input-snapshot reference RESOLVES: the result's `input_snapshot_id`
//   equals the snapshot's own deterministic reference, and every required
//   snapshot constituent is visible in it;
// - the grid-model version is traceable to the Phase 2 Site grid-model
//   reference (P2.02), and the record names WHICH site supplied it;
// - the SYNTHETIC labels survive into the record (objective provenance,
//   forecast provenance, assumptions);
// - the `ScheduleProposed` publication assembles ONLY over the matching
//   persistence proof (persist-before-use, §16.1).
//
// Control status: candidate-schedule bookkeeping for the simulation-only
// 0.x line; a solved schedule is a CANDIDATE, never "feasible".

use std::time::Duration;

use flexcore_application::forecast_fixture::{ForecastQuantity, ForecastVersion, Provenance};
use flexcore_application::flexibility_envelope::EnvelopeVersion;
use flexcore_application::optimization_result_record::{
    assemble_schedule_result_record, schedule_proposed_idempotency_key,
    schedule_proposed_publication, CANDIDATE_STATUS_STATEMENT, GRID_MODEL_PROVENANCE_STATEMENT,
    PERSIST_BEFORE_USE_STATEMENT, SCHEDULE_PROPOSED_PAYLOAD_GAP_NOTE, CapabilityVersionRef,
    GridModelTraceability, InputSnapshotRecord, PersistedResultRecord,
    ScheduleResultAssemblyInput, ScheduleResultError,
};
use flexcore_contracts::capability::CAPABILITY_SCHEMA_VERSION;
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::{EventTime, SiteTimezone};
use flexcore_domain::entities::{Portfolio, Site, Tenant};
use flexcore_domain::ids::{PortfolioId, SiteId, TenantId};
use flexcore_optimization_api::config::SolverConfig;
use flexcore_optimization_api::problem::ProblemClass;
use flexcore_optimization_api::result::{
    Diagnostics, DualDiagnostics, InfeasibilityEvidence, OptimizationResult,
    PrimalDiagnostics, ResidualReport, ResultCompletenessError, SolveStatus, VariableValue,
};
use flexcore_optimization_api::clarabel_provider::SOCP_EXACTNESS_CAVEAT;

// --- Fixtures ------------------------------------------------------------------

const TENANT_TEXT: &str = "0192a5e0-0000-7000-8000-00000000c021";
const PORTFOLIO_TEXT: &str = "0192a5e0-0000-7000-8000-00000000c022";
const SITE_TEXT: &str = "0192a5e0-0000-7000-8000-00000000c023";
const CANDIDATE_TEXT: &str = "0192a5e0-0000-7000-8000-00000000c024";

fn event_time(millis: i64) -> EventTime {
    EventTime::from_unix_millis(millis).expect("valid event time")
}

fn uuid(text: &str) -> FlexUuid {
    FlexUuid::parse(text).expect("valid fixture uuid")
}

/// The Phase 2 entity chain (P2.02): tenant -> portfolio -> site with a
/// non-empty `grid_model_reference` — the site whose reference supplies the
/// §12.6 grid-model version.
fn phase2_site() -> Site {
    let created = event_time(1_700_000_000_000);
    let tenant = Tenant::try_new(
        TenantId::new(uuid(TENANT_TEXT)),
        "fixture-tenant".to_string(),
        None,
        "idp-ref-fixture".to_string(),
        "default".to_string(),
        created,
    )
    .expect("tenant builds");
    let portfolio = Portfolio::try_new(
        &tenant,
        PortfolioId::new(uuid(PORTFOLIO_TEXT)),
        "fixture-portfolio".to_string(),
        None,
        "fixture-operator".to_string(),
        "none (D1 OPEN)".to_string(),
        "{}".to_string(),
        created,
    )
    .expect("portfolio builds");
    Site::try_new(
        &portfolio,
        SiteId::new(uuid(SITE_TEXT)),
        "fixture-site".to_string(),
        None,
        "synthetic location".to_string(),
        SiteTimezone::new("UTC".to_string()).expect("UTC is a valid site timezone"),
        "fixture-pcc".to_string(),
        "site-grid-model-reference-v1".to_string(),
        "fixture-edge-identity".to_string(),
        created,
    )
    .expect("site builds")
}

fn grid_model(site: &Site) -> GridModelTraceability {
    GridModelTraceability::from_site(site, "grid-model-v1").expect("traceability builds")
}

fn input_snapshot() -> InputSnapshotRecord {
    let capabilities = [CapabilityVersionRef {
        asset_key: "asset-bess-1".to_string(),
        capability_profile_reference: "bess-profile-ref-v1".to_string(),
        capability_schema_version: CAPABILITY_SCHEMA_VERSION,
    }];
    let envelope_version = EnvelopeVersion::new("envelope-v1").expect("valid version");
    let forecast_version = ForecastVersion::new("forecast-v1").expect("valid version");
    let forecast_values = flexcore_application::forecast_fixture::LabeledSeriesValues {
        provenance: Provenance::Synthetic,
        forecast_version: &forecast_version,
        quantity: ForecastQuantity::PvOutputMw,
        values: &[0.5, 1.5],
    };
    let objective = flexcore_application::scheduling_objective::ObjectiveConfig::default_synthetic();
    let solver_config = SolverConfig::default();
    InputSnapshotRecord::assemble(
        &capabilities,
        &envelope_version,
        forecast_values,
        &objective,
        &solver_config,
        "telemetry-snapshot-ref-2026-11-07",
    )
    .expect("snapshot assembles")
}

/// A §12.6-optimal Tier 1 result as the HiGHS provider path returns it
/// (pinned solver identity, populated residuals, duals not exposed).
fn tier1_optimal() -> OptimizationResult {
    OptimizationResult {
        status: SolveStatus::Optimal,
        problem_class: ProblemClass::LinearProgram,
        solver_name: "highs".to_string(),
        solver_version: "1.15.1".to_string(),
        provider_id: "tier1-python-worker".to_string(),
        provider_version: "0.1.0".to_string(),
        objective_value: Some(-42.0),
        objective_units: Some("currency-units".to_string()),
        primal: Diagnostics::Available(PrimalDiagnostics {
            values: vec![
                VariableValue {
                    name: "P_export_t0".to_string(),
                    unit: "MW".to_string(),
                    value: 1.25,
                },
                VariableValue {
                    name: "E_bess_t1".to_string(),
                    unit: "MWh".to_string(),
                    value: 0.75,
                },
            ],
        }),
        dual: Diagnostics::NotAvailable {
            reason: "the pinned HiGHS binding build does not expose row duals (VERIFY-BUILD \
                     item, decision note 17_optimizer_port.md)"
                .to_string(),
        },
        residuals: Diagnostics::Available(ResidualReport {
            max_primal_violation: Some(0.0),
            max_dual_violation: Some(0.0),
            iteration_count: Some(23),
        }),
        infeasibility_evidence: Diagnostics::NotAvailable {
            reason: "not an infeasible result".to_string(),
        },
        runtime: Duration::from_millis(120),
        resources: Some("peak RSS 48 MB (worker-reported)".to_string()),
        candidate_id: Some(uuid(CANDIDATE_TEXT)),
        input_snapshot_id: None,
        data_timestamp: None,
        grid_model_version: None,
        uncertainty_and_assumptions: vec![
            "SYNTHETIC objective inputs: price curve synthetic-price-curve-v1 and reference \
             trajectory synthetic-reference-trajectory-v1 (D1/D2 OPEN; not market data)"
                .to_string(),
        ],
        warnings: vec![],
        determinism: Some(flexcore_optimization_api::result::DeterminismRecord {
            solver_version: "1.15.1".to_string(),
            thread_count: 1,
            time_limit_ms: Some(30_000),
            config_digest: "fnv1a64:00000000000000a1".to_string(),
        }),
    }
}

/// A Tier 2 (Clarabel conic) result: the provider pre-records the SOCP
/// exactness caveat and reports the per-cone relaxation residual machinery.
fn tier2_optimal() -> OptimizationResult {
    let mut result = tier1_optimal();
    result.problem_class = ProblemClass::ContinuousSecondOrderConeProgram;
    result.solver_name = "clarabel".to_string();
    result.solver_version = "0.11.1".to_string();
    result.provider_id = "tier2-rust-conic".to_string();
    result.objective_value = Some(-17.5);
    result.dual = Diagnostics::Available(DualDiagnostics {
        reduced_costs: vec![0.0, -0.5],
        row_duals: vec![1.5, 2.5],
    });
    result.warnings = vec![SOCP_EXACTNESS_CAVEAT.to_string()];
    result
}

/// A Tier 1 infeasible result: conflict evidence present, NO schedule, no
/// relaxed output (P3.19 step 3).
fn tier1_infeasible() -> OptimizationResult {
    let mut result = tier1_optimal();
    result.status = SolveStatus::Infeasible;
    result.objective_value = None;
    result.objective_units = None;
    result.infeasibility_evidence = Diagnostics::Available(InfeasibilityEvidence {
        conflict_variables: vec![],
        conflict_rows: vec!["ev_departure_energy_floor".to_string()],
        backend_report: "IIS: rows 12, 13 conflict with bounds".to_string(),
    });
    result
}

/// A Tier 1 timeout result: no solution, residuals explicitly not available
/// WITH a reason (never silence).
fn tier1_timeout() -> OptimizationResult {
    let mut result = tier1_optimal();
    result.status = SolveStatus::Timeout;
    result.objective_value = None;
    result.objective_units = None;
    result.primal = Diagnostics::NotAvailable {
        reason: "the time limit stopped the solve before a feasible point was found".to_string(),
    };
    result.residuals = Diagnostics::NotAvailable {
        reason: "the pinned solver reported no residual metrics for a limit-stopped run"
            .to_string(),
    };
    result
}

fn assemble_with(result: OptimizationResult, site: &Site) -> Result<
    flexcore_application::optimization_result_record::ScheduleResultRecord,
    ScheduleResultError,
> {
    assemble_schedule_result_record(ScheduleResultAssemblyInput {
        tenant_id: TenantId::new(uuid(TENANT_TEXT)),
        portfolio_id: PortfolioId::new(uuid(PORTFOLIO_TEXT)),
        site_id: SiteId::new(uuid(SITE_TEXT)),
        result,
        grid_model: grid_model(site),
        input_snapshot: input_snapshot(),
        objective_scaling_warnings: vec![],
    })
}

// --- The §12.6 completeness checklist, per provider path -----------------------

/// Asserts EVERY §12.6 field is present (populated, or explicitly not
/// available with a reason) on one assembled record.
fn assert_s126_complete(
    record: &flexcore_application::optimization_result_record::ScheduleResultRecord,
) {
    let result = record.result();
    // status: inside the closed six-status vocabulary.
    assert!(SolveStatus::parse(result.status.as_str()).is_some());
    // problem class and solver/provider version.
    assert!(!result.solver_name.trim().is_empty());
    assert!(!result.solver_version.trim().is_empty());
    assert!(!result.provider_id.trim().is_empty());
    assert!(!result.provider_version.trim().is_empty());
    // objective value and units: a solving status carries BOTH; a
    // non-solving status carries the defined absence.
    match result.status {
        SolveStatus::Optimal | SolveStatus::Feasible => {
            assert!(result.objective_value.is_some(), "objective value missing");
            let units = result.objective_units.as_deref().expect("objective units");
            assert!(!units.trim().is_empty());
        }
        _ => assert!(result.objective_value.is_none()),
    }
    // primal and dual diagnostics: available, or explicitly not available
    // with a reason.
    for (name, diagnostic) in [("primal", &result.primal), ("dual", &result.dual)] {
        if let Some(reason) = diagnostic.not_available_reason() {
            assert!(!reason.trim().is_empty(), "{name} not available WITHOUT a reason");
        }
    }
    // constraint residuals and violations: populated, or explicitly not
    // available with a reason (per solver).
    match &result.residuals {
        Diagnostics::Available(report) => {
            // At least the primal violation side is reported for a solved
            // result; iteration count is per-solver.
            assert!(
                report.max_primal_violation.is_some()
                    || report.max_dual_violation.is_some()
                    || report.iteration_count.is_some(),
                "a residual report with no recorded metric is not a report"
            );
        }
        Diagnostics::NotAvailable { reason } => {
            assert!(!reason.trim().is_empty(), "residuals not available WITHOUT a reason");
        }
    }
    // runtime and resource use.
    assert!(result.runtime > Duration::ZERO, "runtime must be recorded");
    assert!(result.resources.is_some(), "the resource-use note must be recorded");
    // input snapshot and grid model version: the P3.21 wiring fields.
    assert!(result.input_snapshot_id.is_some(), "input snapshot missing");
    assert!(result.grid_model_version.is_some(), "grid model version missing");
    // uncertainty and assumptions: the synthetic labels survive.
    assert!(
        !result.uncertainty_and_assumptions.is_empty(),
        "uncertainty/assumptions must be declared"
    );
    assert!(result
        .uncertainty_and_assumptions
        .iter()
        .any(|a| a.contains("SYNTHETIC")));
    // warnings: every degraded path records them; the Tier 2 path carries
    // the SOCP caveat.
    if result.problem_class.tier() == 2 {
        assert!(result.warnings.iter().any(|w| w == SOCP_EXACTNESS_CAVEAT));
    }
    // output candidate ID.
    assert!(result.candidate_id.is_some());
    // determinism record (P3.19): pinned version + thread count + digest.
    let determinism = result.determinism.as_ref().expect("determinism record");
    assert_eq!(determinism.thread_count, 1);
    assert!(determinism.config_digest.starts_with("fnv1a64:"));
    // The assembled record passes its own completeness check.
    record.validate_record_complete().expect("record complete");
}

#[test]
fn every_s126_field_is_present_on_every_provider_path() {
    let site = phase2_site();
    for result in [tier1_optimal(), tier1_infeasible(), tier1_timeout(), tier2_optimal()] {
        let record = assemble_with(result, &site).expect("assembles");
        assert_s126_complete(&record);
    }
}

#[test]
fn a_result_missing_any_required_field_fails_validation() {
    let site = phase2_site();

    // Missing candidate ID.
    let mut result = tier1_optimal();
    result.candidate_id = None;
    assert_eq!(
        assemble_with(result, &site),
        Err(ScheduleResultError::IncompleteResult(ResultCompletenessError::MissingCandidateId))
    );

    // Missing determinism record (P3.19).
    let mut result = tier1_optimal();
    result.determinism = None;
    assert_eq!(
        assemble_with(result, &site),
        Err(ScheduleResultError::IncompleteResult(
            ResultCompletenessError::MissingDeterminismRecord
        ))
    );

    // Empty solver identification.
    let mut result = tier1_optimal();
    result.solver_name = "  ".to_string();
    assert!(matches!(
        assemble_with(result, &site),
        Err(ScheduleResultError::IncompleteResult(
            ResultCompletenessError::EmptyIdentification { .. }
        ))
    ));

    // Objective value without units on a solving status.
    let mut result = tier1_optimal();
    result.objective_units = None;
    assert!(matches!(
        assemble_with(result, &site),
        Err(ScheduleResultError::IncompleteResult(
            ResultCompletenessError::ObjectiveIncomplete { .. }
        ))
    ));

    // A solving status without primal diagnostics.
    let mut result = tier1_optimal();
    result.primal = Diagnostics::NotAvailable { reason: "lost".to_string() };
    assert!(matches!(
        assemble_with(result, &site),
        Err(ScheduleResultError::IncompleteResult(
            ResultCompletenessError::MissingPrimalDiagnostics { .. }
        ))
    ));

    // A not-available diagnostic with an EMPTY reason (silence is never legal).
    let mut result = tier1_optimal();
    result.dual = Diagnostics::NotAvailable { reason: "   ".to_string() };
    assert!(matches!(
        assemble_with(result, &site),
        Err(ScheduleResultError::IncompleteResult(
            ResultCompletenessError::EmptyNotAvailableReason { .. }
        ))
    ));

    // The assembly never papers over a provider-set snapshot reference that
    // disagrees with the snapshot it is given.
    let mut result = tier1_optimal();
    result.input_snapshot_id = Some("schedule_input_snapshot:from-another-solve".to_string());
    assert!(matches!(
        assemble_with(result, &site),
        Err(ScheduleResultError::SnapshotReferenceMismatch { .. })
    ));

    // ... nor a provider-set grid-model version that disagrees with the
    // traceability.
    let mut result = tier1_optimal();
    result.grid_model_version = Some("some-other-model".to_string());
    assert!(matches!(
        assemble_with(result, &site),
        Err(ScheduleResultError::GridModelVersionMismatch { .. })
    ));
}

#[test]
fn the_snapshot_reference_resolves_and_names_every_constituent() {
    let site = phase2_site();
    let record = assemble_with(tier1_optimal(), &site).expect("assembles");
    let reference = record.result().input_snapshot_id.as_deref().expect("wired");
    // The result's reference IS the snapshot's own reference.
    assert_eq!(reference, record.input_snapshot().reference());
    // Every required constituent (ledger P3.21 step 2) is visible.
    for constituent in [
        "schedule_input_snapshot:",
        "envelope-v1",                            // envelope version (P3.12/13)
        "forecast-v1",                            // forecast version (P3.31)
        "synthetic",                              // the provenance label
        "telemetry=telemetry-snapshot-ref-2026-11-07", // telemetry snapshot reference
        "assets=1",                               // capability versions present
    ] {
        assert!(reference.contains(constituent), "reference '{reference}' lacks '{constituent}'");
    }
    // The solver configuration digest (P3.19) is part of the reference.
    assert!(reference.contains(record.input_snapshot().solver_config_digest.as_str()));
    // The objective configuration (P3.14) is recorded verbatim, with the
    // synthetic labels intact.
    let objective_record = &record.input_snapshot().objective_config_record;
    assert!(objective_record
        .iter()
        .any(|(k, v)| k == "objective.provenance" && v == "SYNTHETIC"));
    assert!(objective_record
        .iter()
        .any(|(k, v)| k == "objective.not_a_market_product" && v.starts_with("true")));
    assert!(objective_record
        .iter()
        .any(|(k, _)| k == "objective.name"));
}

#[test]
fn the_grid_model_version_is_traceable_to_the_phase2_site_reference() {
    let site = phase2_site();
    let record = assemble_with(tier1_optimal(), &site).expect("assembles");
    let traceability = record.grid_model();
    // The version on the §12.6 result IS the traceability's version.
    assert_eq!(
        record.result().grid_model_version.as_deref(),
        Some(traceability.grid_model_version())
    );
    // The record names WHICH site's reference supplied it (P2.02).
    assert_eq!(traceability.site_id(), site.site_id());
    assert_eq!(traceability.site_grid_model_reference(), "site-grid-model-reference-v1");
    let statement = traceability.traceability_statement();
    assert!(statement.contains("grid-model-v1"));
    assert!(statement.contains("site-grid-model-reference-v1"));
    assert!(statement.contains(&site.site_id().to_string()));
    // A traceability naming another site never assembles.
    let other = Site::try_new(
        &Portfolio::try_new(
            &Tenant::try_new(
                TenantId::new(uuid("0192a5e0-0000-7000-8000-00000000c091")),
                "other-tenant".to_string(),
                None,
                "idp-ref-other".to_string(),
                "default".to_string(),
                event_time(1_700_000_000_000),
            )
            .expect("tenant builds"),
            PortfolioId::new(uuid("0192a5e0-0000-7000-8000-00000000c092")),
            "other-portfolio".to_string(),
            None,
            "operator".to_string(),
            "none (D1 OPEN)".to_string(),
            "{}".to_string(),
            event_time(1_700_000_000_000),
        )
        .expect("portfolio builds"),
        SiteId::new(uuid("0192a5e0-0000-7000-8000-00000000c093")),
        "other-site".to_string(),
        None,
        "location".to_string(),
        SiteTimezone::new("UTC".to_string()).expect("UTC is a valid site timezone"),
        "pcc".to_string(),
        "other-site-grid-reference".to_string(),
        "edge".to_string(),
        event_time(1_700_000_000_000),
    )
    .expect("site builds");
    let mut result = tier1_optimal();
    let foreign = GridModelTraceability::from_site(&other, "grid-model-v1").expect("builds");
    let result = tier1_optimal();
    assert!(matches!(
        assemble_schedule_result_record(ScheduleResultAssemblyInput {
            tenant_id: TenantId::new(uuid(TENANT_TEXT)),
            portfolio_id: PortfolioId::new(uuid(PORTFOLIO_TEXT)),
            site_id: SiteId::new(uuid(SITE_TEXT)),
            grid_model: foreign,
            input_snapshot: input_snapshot(),
            objective_scaling_warnings: vec![],
            result,
        }),
        Err(ScheduleResultError::GridModelSiteMismatch { .. })
    ));
}

#[test]
fn residuals_are_populated_or_explicitly_not_available_per_path() {
    let site = phase2_site();
    // Tier 1 optimal: populated.
    let record = assemble_with(tier1_optimal(), &site).expect("assembles");
    match &record.result().residuals {
        Diagnostics::Available(report) => {
            assert_eq!(report.max_primal_violation, Some(0.0));
            assert_eq!(report.iteration_count, Some(23));
        }
        Diagnostics::NotAvailable { .. } => panic!("tier 1 optimal must report residuals"),
    }
    // Timeout: explicitly not available, reason recorded.
    let record = assemble_with(tier1_timeout(), &site).expect("assembles");
    match &record.result().residuals {
        Diagnostics::NotAvailable { reason } => {
            assert!(!reason.trim().is_empty());
        }
        Diagnostics::Available(_) => panic!("unexpected residuals on a timeout path"),
    }
    // Infeasible: evidence is available with the conflict rows.
    let record = assemble_with(tier1_infeasible(), &site).expect("assembles");
    match &record.result().infeasibility_evidence {
        Diagnostics::Available(evidence) => {
            assert!(evidence.conflict_rows.contains(&"ev_departure_energy_floor".to_string()));
            assert!(!evidence.backend_report.is_empty());
        }
        Diagnostics::NotAvailable { .. } => panic!("infeasible evidence lost"),
    }
}

#[test]
fn the_tier2_path_carries_the_socp_caveat_exactly_once() {
    let site = phase2_site();
    let record = assemble_with(tier2_optimal(), &site).expect("assembles");
    assert_eq!(
        record
            .result()
            .warnings
            .iter()
            .filter(|w| *w == SOCP_EXACTNESS_CAVEAT)
            .count(),
        1
    );
    // A Tier 2 result without the provider caveat gets it from the assembly.
    let mut bare = tier2_optimal();
    bare.warnings.clear();
    let record = assemble_with(bare, &site).expect("assembles");
    assert!(record.result().warnings.iter().any(|w| w == SOCP_EXACTNESS_CAVEAT));
}

#[test]
fn the_publication_requires_the_matching_persistence_proof() {
    let site = phase2_site();
    let record = assemble_with(tier1_optimal(), &site).expect("assembles");
    let key = schedule_proposed_idempotency_key(
        record.portfolio_id(),
        record.site_id(),
        record.candidate_id(),
    );
    assert!(key.contains(CANDIDATE_TEXT));

    // The matching proof publishes the frozen event.
    let persisted = PersistedResultRecord::new(record.record_reference(), key.clone()).expect("proof");
    let publication = schedule_proposed_publication(
        &record,
        &persisted,
        FlexUuid::nil(),
        event_time(1_700_000_000_500),
    )
    .expect("publishes");
    assert_eq!(publication.idempotency_key, key);
    assert_eq!(
        publication.destination_subject.as_str(),
        "flexcore.v1.scheduling.schedule_proposed"
    );
    // The frozen payload shape (unchanged): correlation/tenant/portfolio/
    // site/asset + event_time, with the nil asset id as the recorded
    // schedule-scope convention (the gap note).
    match &publication.envelope.event {
        flexcore_events::events::DomainEvent::ScheduleProposed(payload) => {
            assert_eq!(payload.tenant_id, TenantId::new(uuid(TENANT_TEXT)));
            assert_eq!(payload.portfolio_id, PortfolioId::new(uuid(PORTFOLIO_TEXT)));
            assert_eq!(payload.site_id, SiteId::new(uuid(SITE_TEXT)));
            assert_eq!(payload.asset_id.uuid(), FlexUuid::nil());
        }
        other => panic!("unexpected event: {other:?}"),
    }

    // A proof for another record cannot back the publication.
    let foreign = PersistedResultRecord::new("another-candidate", &key).expect("proof");
    assert!(matches!(
        schedule_proposed_publication(
            &record,
            &foreign,
            FlexUuid::nil(),
            event_time(1_700_000_000_500),
        ),
        Err(ScheduleResultError::PersistenceReferenceMismatch { .. })
    ));
    // A proof with a foreign key cannot either.
    let foreign_key = PersistedResultRecord::new(record.record_reference(), "other:key").expect("proof");
    assert!(matches!(
        schedule_proposed_publication(
            &record,
            &foreign_key,
            FlexUuid::nil(),
            event_time(1_700_000_000_500),
        ),
        Err(ScheduleResultError::PersistenceKeyMismatch { .. })
    ));
}

#[test]
fn the_recorded_statements_are_carried_verbatim() {
    // A ScheduleProposed is a CANDIDATE statement.
    assert!(CANDIDATE_STATUS_STATEMENT.contains("CANDIDATE"));
    // The grid-model provenance statement names P2.02.
    assert!(GRID_MODEL_PROVENANCE_STATEMENT.contains("P2.02"));
    // The persist-before-use statement names the one-transaction rule.
    assert!(PERSIST_BEFORE_USE_STATEMENT.contains("one native transaction"));
    // The payload gap note is recorded (the D-FX-9 / D-P3.12-7 precedent).
    assert!(SCHEDULE_PROPOSED_PAYLOAD_GAP_NOTE.contains("idempotency key"));
    assert!(SCHEDULE_PROPOSED_PAYLOAD_GAP_NOTE.contains("P2.16"));
}
