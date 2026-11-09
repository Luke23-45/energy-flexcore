// P4.04 + P4.05 — Feasibility-gate integration tests (ledger P4.04 and
// P4.05 acceptance; report §12.5 steps 1–4).
//
// Deterministic throughout: the power flow runs through the PORT against
// the scripted/stub backend doubles from the P4.02 contract tests — no real
// solver, no I/O. Every test asserts EVIDENCE (rows, records, rejections,
// aborts), never a feasibility verdict: the limit check is P4.06 and the
// reject/constrain/return routing is P4.07.
//
// Covered here:
// 1. Step 1: malformed candidates rejected at a NAMED field (missing ref,
//    inverted validity window, expired timestamp, duration mismatch,
//    empty setpoints); stale envelope rejected (the P3.13 gate);
//    superseded AND unknown revisions rejected (the P3.22 gate).
// 2. Step 2: capability exceedance rejected (named rule); Q-convention
//    mismatch rejected; BAD/MISSING/STALE ramp baselines BLOCK (fail-closed,
//    the P4.08 pre-wiring), GOOD passes; ramp violation rejected; every
//    derived injection round-trips the OWNED translation layer.
// 3. Steps 3–4: health-gated application (a stale model aborts BEFORE the
//    power flow), the pre-run content-hash hook aborts a mid-gate model
//    change, the AC power flow runs through the port, non-convergence is
//    recorded INCONCLUSIVE fail-closed, the provider pin is required, the
//    scope-declared unbalanced limitation + P4.34 deferral note surface.
// 4. End-to-end: steps run in order, every intermediate evidence row
//    present, and the FeasibilityAttemptRecord is complete.

mod common;

use common::{model_ref, test_limits, EXPECTED_PIN, ScriptedBackend};
use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::units::{ActivePowerMW, ReactivePowerMVAr, ReactiveSignConvention};
use flexcore_grid_api::adapter::{PandapowerGridProvider, PROVIDER_ID};
use flexcore_grid_api::feasibility::{
    run_feasibility_gate, AttemptOutput, AttemptRecordError, CandidateSetpoint,
    CandidateValidationConfig, DeclaredBounds, EnvelopeFreshness, EvidenceOutcome,
    FeasibilityCandidate, GateAbort, GatePipelineOutcome, GateSources, GateStep, RevisionState,
    TelemeteredState, AC_POWER_FLOW_STATEMENT, UNBALANCED_DEFERRAL_NOTE,
};
use flexcore_grid_api::model_health::{
    ModelCoverage, ModelHealthConfig, ModelLifecycleState, ModelVersionRegistry, RegistryElement,
    RegistryElementKind, VersionedModel,
};
use flexcore_grid_api::port::{
    GridModelProvider, PowerFlowStatus, VersionedNetworkModel,
};
use flexcore_grid_api::scope::NotModelled;
use flexcore_grid_api::tests_support::{converged_outcome, three_bus_model};
use std::sync::Arc;

const NOW: i64 = 10_000;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn coverage() -> ModelCoverage {
    ModelCoverage {
        covered_feeder_ids: vec!["feeder-contract-test".to_string()],
        covered_element_ids: vec!["asset-1".to_string()],
        coverage_statement: "covers the contract-test feeder and asset-1 at bus_3".to_string(),
    }
}

fn fresh_model() -> VersionedNetworkModel {
    let mut reference = model_ref();
    reference.content_hash = String::new(); // the versioning layer assigns it
    three_bus_model(reference)
}

/// The registry with the active model version registered at `NOW - 1000`.
fn registry_with_model(versioned: &VersionedModel) -> ModelVersionRegistry {
    let mut registry = ModelVersionRegistry::new();
    registry.register_version(versioned.clone()).expect("registers");
    registry
}

fn versioned_model() -> VersionedModel {
    VersionedModel::register(
        fresh_model(),
        ModelLifecycleState::Active,
        coverage(),
        NOW - 1_000,
    )
    .expect("registers")
}

fn portfolio() -> Vec<RegistryElement> {
    vec![
        RegistryElement {
            element_id: "feeder-contract-test".to_string(),
            kind: RegistryElementKind::Feeder,
        },
        RegistryElement {
            element_id: "asset-1".to_string(),
            kind: RegistryElementKind::Element,
        },
    ]
}

fn sources() -> GateSources {
    GateSources {
        capability_bounds: Arc::new(|_asset, _at| {
            Ok(DeclaredBounds {
                p_min_mw: -5.0,
                p_max_mw: 5.0,
                q_min_mvar: -2.0,
                q_max_mvar: 2.0,
                q_convention: ReactiveSignConvention::InjectionPositive,
                ramp_mw_per_min: 1.0,
            })
        }),
        envelope_freshness: Arc::new(|_| Ok(EnvelopeFreshness::Fresh)),
        revision_superseded: Arc::new(|_, _| Ok(RevisionState::Active)),
        ramp_baseline: Arc::new(|_| {
            Ok(TelemeteredState {
                active_power_mw: 0.0,
                quality: TelemetryQuality::Good,
                observed_at_millis: NOW - 100,
            })
        }),
    }
}

fn setpoint() -> CandidateSetpoint {
    CandidateSetpoint {
        setpoint_id: "sp-1".to_string(),
        asset_id: "asset-1".to_string(),
        bus_id: "bus_3".to_string(),
        active_power: ActivePowerMW::new(2.5).expect("valid"),
        reactive_power: ReactivePowerMVAr::new(0.5).expect("valid"),
        reactive_sign_convention: ReactiveSignConvention::InjectionPositive,
        interval_start_millis: NOW,
    }
}

fn candidate() -> FeasibilityCandidate {
    FeasibilityCandidate {
        candidate_id: "cand-gate-1".to_string(),
        schedule_id: "0192a5e0-8c3d-7e2f-9a1b-3c4d5e6f7a8b".to_string(),
        revision: 2,
        envelope_version: "env-v1".to_string(),
        result_record_snapshot_ref: "schedule_input_snapshot:env-v1:...".to_string(),
        created_at_millis: NOW - 100,
        valid_from_millis: NOW,
        valid_until_millis: NOW + 3_000,
        stated_duration_millis: 3_000,
        setpoints: vec![setpoint()],
    }
}

/// The provider over its counted backend, wired together (call-count
/// assertions read the same backend the provider uses).
fn provider_with_backend(backend: ScriptedBackend) -> PandapowerGridProvider {
    PandapowerGridProvider::new(Arc::new(backend), flexcore_grid_api::adapter::balanced_worker_scope())
        .expect("balanced scope accepted")
}

fn run_gate(
    candidate: &FeasibilityCandidate,
    sources: &GateSources,
    versioned: &VersionedModel,
    registry: &ModelVersionRegistry,
    provider: &dyn GridModelProvider,
) -> GatePipelineOutcome {
    run_feasibility_gate(flexcore_grid_api::feasibility::GateRunRequest {
        candidate,
        sources,
        config: &CandidateValidationConfig::default(),
        versioned_model: versioned,
        registry,
        portfolio_elements: &portfolio(),
        health_config: &ModelHealthConfig::default(),
        provider,
        limits: &test_limits(),
        attempt_id: "attempt-1".to_string(),
        now_millis: NOW,
    })
}

/// The canonical step order for the row-order assertion.
fn step_rank(step: GateStep) -> u8 {
    match step {
        GateStep::Step1CandidateValidation => 1,
        GateStep::Step2InjectionDerivation => 2,
        GateStep::Step3ModelApplication => 3,
        GateStep::Step4PowerFlow => 4,
    }
}

// ---------------------------------------------------------------------------
// Step 1 — malformed candidates rejected at a NAMED field
// ---------------------------------------------------------------------------

#[test]
fn a_missing_reference_is_rejected_naming_the_field() {
    let mut candidate = candidate();
    candidate.schedule_id = "  ".to_string();
    let (registry, versioned) = {
        let v = versioned_model();
        (registry_with_model(&v), v)
    };
    let provider = provider_with_backend(ScriptedBackend::new(converged_outcome(EXPECTED_PIN)));
    let outcome = run_gate(&candidate, &sources(), &versioned, &registry, &provider);
    match outcome {
        GatePipelineOutcome::Rejected { step, rejection, evidence } => {
            assert_eq!(step, GateStep::Step1CandidateValidation);
            assert_eq!(rejection.subject, "schedule_id");
            assert_eq!(rejection.rule_code, "REF_MISSING");
            assert!(evidence.last().expect("row").outcome == EvidenceOutcome::Rejected);
        }
        other => panic!("expected a step-1 rejection, got {other:?}"),
    }
}

#[test]
fn an_inverted_validity_window_is_rejected_naming_the_rule() {
    let mut candidate = candidate();
    candidate.valid_until_millis = candidate.valid_from_millis - 1;
    candidate.stated_duration_millis = candidate.valid_until_millis - candidate.valid_from_millis;
    let v = versioned_model();
    let registry = registry_with_model(&v);
    let provider = provider_with_backend(ScriptedBackend::new(converged_outcome(EXPECTED_PIN)));
    match run_gate(&candidate, &sources(), &v, &registry, &provider) {
        GatePipelineOutcome::Rejected { rejection, .. } => {
            assert_eq!(rejection.subject, "validity window");
            assert_eq!(rejection.rule_code, "VALIDITY_WINDOW_INVERTED");
        }
        other => panic!("expected a rejection, got {other:?}"),
    }
}

#[test]
fn an_expired_timestamp_is_rejected_naming_the_rule() {
    let mut candidate = candidate();
    candidate.created_at_millis = NOW - 400_000; // beyond the 5-minute default tolerance
    let v = versioned_model();
    let registry = registry_with_model(&v);
    let provider = provider_with_backend(ScriptedBackend::new(converged_outcome(EXPECTED_PIN)));
    match run_gate(&candidate, &sources(), &v, &registry, &provider) {
        GatePipelineOutcome::Rejected { rejection, .. } => {
            assert_eq!(rejection.subject, "created_at");
            assert_eq!(rejection.rule_code, "TIMESTAMP_EXPIRED");
            assert!(rejection.detail.contains("beyond the configured tolerance"));
        }
        other => panic!("expected a rejection, got {other:?}"),
    }
}

#[test]
fn a_duration_mismatch_is_rejected_naming_the_field() {
    let mut candidate = candidate();
    candidate.stated_duration_millis = 999;
    let v = versioned_model();
    let registry = registry_with_model(&v);
    let provider = provider_with_backend(ScriptedBackend::new(converged_outcome(EXPECTED_PIN)));
    match run_gate(&candidate, &sources(), &v, &registry, &provider) {
        GatePipelineOutcome::Rejected { rejection, .. } => {
            assert_eq!(rejection.subject, "stated_duration");
            assert_eq!(rejection.rule_code, "DURATION_MISMATCH");
        }
        other => panic!("expected a rejection, got {other:?}"),
    }
}

#[test]
fn an_empty_candidate_is_rejected_not_treated_as_an_empty_check() {
    let mut candidate = candidate();
    candidate.setpoints.clear();
    let v = versioned_model();
    let registry = registry_with_model(&v);
    let provider = provider_with_backend(ScriptedBackend::new(converged_outcome(EXPECTED_PIN)));
    match run_gate(&candidate, &sources(), &v, &registry, &provider) {
        GatePipelineOutcome::Rejected { rejection, .. } => {
            assert_eq!(rejection.rule_code, "EMPTY_SETPOINTS");
        }
        other => panic!("expected a rejection, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Step 1 — the injected Phase 3 gates
// ---------------------------------------------------------------------------

#[test]
fn a_stale_envelope_is_rejected_via_the_p3_13_gate() {
    let candidate = candidate();
    let v = versioned_model();
    let registry = registry_with_model(&v);
    let provider = provider_with_backend(ScriptedBackend::new(converged_outcome(EXPECTED_PIN)));
    let mut custom = sources();
    custom.envelope_freshness = Arc::new(|_| {
        Ok(EnvelopeFreshness::Stale {
            reason: "MaxAgeExceeded { age_millis: 7200000, max_age_millis: 3600000 }".to_string(),
        })
    });
    match run_gate(&candidate, &custom, &v, &registry, &provider) {
        GatePipelineOutcome::Rejected { rejection, evidence } => {
            assert_eq!(rejection.rule_code, "ENVELOPE_STALE");
            assert!(rejection.detail.contains("stale"));
            // The evidence records the failing check, in step 1.
            let last = evidence.last().expect("row");
            assert_eq!(last.rule_code, "ENVELOPE_STALE");
            assert_eq!(last.step, GateStep::Step1CandidateValidation);
        }
        other => panic!("expected a rejection, got {other:?}"),
    }
}

#[test]
fn a_superseded_revision_is_rejected_via_the_p3_22_gate() {
    let candidate = candidate();
    let v = versioned_model();
    let registry = registry_with_model(&v);
    let provider = provider_with_backend(ScriptedBackend::new(converged_outcome(EXPECTED_PIN)));
    let mut custom = sources();
    custom.revision_superseded = Arc::new(|_, _| Ok(RevisionState::Superseded));
    match run_gate(&candidate, &custom, &v, &registry, &provider) {
        GatePipelineOutcome::Rejected { rejection, .. } => {
            assert_eq!(rejection.rule_code, "REVISION_SUPERSEDED");
            assert!(rejection.detail.contains("SUPERSEDED"));
        }
        other => panic!("expected a rejection, got {other:?}"),
    }
}

/// `None` from `is_revision_superseded` (the revision does not exist in the
/// caller's tenant scope) is NEVER usable: rejected with its own rule.
#[test]
fn an_unknown_revision_is_rejected_never_treated_as_usable() {
    let candidate = candidate();
    let v = versioned_model();
    let registry = registry_with_model(&v);
    let provider = provider_with_backend(ScriptedBackend::new(converged_outcome(EXPECTED_PIN)));
    let mut custom = sources();
    custom.revision_superseded = Arc::new(|_, _| Ok(RevisionState::Unknown));
    match run_gate(&candidate, &custom, &v, &registry, &provider) {
        GatePipelineOutcome::Rejected { rejection, .. } => {
            assert_eq!(rejection.rule_code, "REVISION_UNKNOWN");
            assert!(rejection.detail.contains("never treated as usable"));
        }
        other => panic!("expected a rejection, got {other:?}"),
    }
}

/// A failing seam is fail-closed: an error from the revision check rejects,
/// it never degrades to a pass or a skip.
#[test]
fn a_failing_seam_rejects_fail_closed() {
    let candidate = candidate();
    let v = versioned_model();
    let registry = registry_with_model(&v);
    let provider = provider_with_backend(ScriptedBackend::new(converged_outcome(EXPECTED_PIN)));
    let mut custom = sources();
    custom.revision_superseded = Arc::new(|_, _| {
        Err(flexcore_grid_api::feasibility::SeamError {
            seam: "revision_superseded",
            detail: "schedule store unavailable".to_string(),
        })
    });
    match run_gate(&candidate, &custom, &v, &registry, &provider) {
        GatePipelineOutcome::Rejected { rejection, .. } => {
            assert_eq!(rejection.rule_code, "REVISION_CHECK_FAILED");
            assert!(rejection.detail.contains("fail-closed"));
        }
        other => panic!("expected a rejection, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Step 2 — capability and ramp
// ---------------------------------------------------------------------------

#[test]
fn a_capability_exceedance_is_rejected_naming_the_asset() {
    let candidate = candidate(); // P = 2.5 MW
    let v = versioned_model();
    let registry = registry_with_model(&v);
    let provider = provider_with_backend(ScriptedBackend::new(converged_outcome(EXPECTED_PIN)));
    let mut custom = sources();
    custom.capability_bounds = Arc::new(|_asset, _at| {
        Ok(DeclaredBounds {
            p_min_mw: -2.0,
            p_max_mw: 2.0,
            q_min_mvar: -1.0,
            q_max_mvar: 1.0,
            q_convention: ReactiveSignConvention::InjectionPositive,
            ramp_mw_per_min: 10.0,
        })
    });
    match run_gate(&candidate, &custom, &v, &registry, &provider) {
        GatePipelineOutcome::Rejected { step, rejection, .. } => {
            assert_eq!(step, GateStep::Step2InjectionDerivation);
            assert_eq!(rejection.rule_code, "CAPABILITY_EXCEEDED");
            assert!(rejection.subject.contains("sp-1"));
            assert!(rejection.detail.contains("2.5"));
        }
        other => panic!("expected a step-2 rejection, got {other:?}"),
    }
}

#[test]
fn a_q_convention_mismatch_is_rejected_the_sign_is_never_inferred() {
    let candidate = candidate(); // declares InjectionPositive
    let v = versioned_model();
    let registry = registry_with_model(&v);
    let provider = provider_with_backend(ScriptedBackend::new(converged_outcome(EXPECTED_PIN)));
    let mut custom = sources();
    custom.capability_bounds = Arc::new(|_asset, _at| {
        Ok(DeclaredBounds {
            p_min_mw: -5.0,
            p_max_mw: 5.0,
            q_min_mvar: -2.0,
            q_max_mvar: 2.0,
            q_convention: ReactiveSignConvention::ConsumptionPositive,
            ramp_mw_per_min: 1.0,
        })
    });
    match run_gate(&candidate, &custom, &v, &registry, &provider) {
        GatePipelineOutcome::Rejected { rejection, .. } => {
            assert_eq!(rejection.rule_code, "Q_CONVENTION_MISMATCH");
            assert!(rejection.detail.contains("never inferred"));
        }
        other => panic!("expected a rejection, got {other:?}"),
    }
}

/// No declared capability for the asset is a rejection: unknown is never
/// unconstrained (the P3.02 rule carried forward).
#[test]
fn an_undeclared_capability_rejects_fail_closed() {
    let candidate = candidate();
    let v = versioned_model();
    let registry = registry_with_model(&v);
    let provider = provider_with_backend(ScriptedBackend::new(converged_outcome(EXPECTED_PIN)));
    let mut custom = sources();
    custom.capability_bounds = Arc::new(|_asset, _at| {
        Err(flexcore_grid_api::feasibility::SeamError {
            seam: "capability_bounds",
            detail: "no capability model on record for the interval".to_string(),
        })
    });
    match run_gate(&candidate, &custom, &v, &registry, &provider) {
        GatePipelineOutcome::Rejected { rejection, .. } => {
            assert_eq!(rejection.rule_code, "CAPABILITY_UNAVAILABLE");
        }
        other => panic!("expected a rejection, got {other:?}"),
    }
}

/// BAD, MISSING, and STALE ramp baselines BLOCK the ramp check (fail-closed,
/// the P4.08 pre-wiring) — the power flow is never reached.
#[test]
fn bad_missing_and_stale_baselines_block_the_ramp_check() {
    for quality in [TelemetryQuality::Bad, TelemetryQuality::Missing, TelemetryQuality::Stale] {
        let candidate = candidate();
        let v = versioned_model();
        let registry = registry_with_model(&v);
        let backend = ScriptedBackend::new(converged_outcome(EXPECTED_PIN));
        let provider = provider_with_backend(backend);
        let mut custom = sources();
        custom.ramp_baseline = Arc::new(move |_| {
            Ok(TelemeteredState {
                active_power_mw: 0.0,
                quality,
                observed_at_millis: NOW - 100,
            })
        });
        match run_gate(&candidate, &custom, &v, &registry, &provider) {
            GatePipelineOutcome::Rejected { rejection, .. } => {
                assert_eq!(rejection.rule_code, "RAMP_BASELINE_BLOCKED");
                assert!(rejection.detail.contains("fail-closed"));
            }
            other => panic!("quality {quality:?} must block, got {other:?}"),
        }
    }
}

#[test]
fn a_ramp_violation_is_rejected_with_the_delta_and_limit() {
    let candidate = candidate(); // ΔP = 2.5 MW
    let v = versioned_model();
    let registry = registry_with_model(&v);
    let provider = provider_with_backend(ScriptedBackend::new(converged_outcome(EXPECTED_PIN)));
    let mut custom = sources();
    // 0.1 MW/min × 3 min = 0.3 MW allowed < 2.5 MW requested.
    custom.capability_bounds = Arc::new(|_asset, _at| {
        Ok(DeclaredBounds {
            p_min_mw: -5.0,
            p_max_mw: 5.0,
            q_min_mvar: -2.0,
            q_max_mvar: 2.0,
            q_convention: ReactiveSignConvention::InjectionPositive,
            ramp_mw_per_min: 0.1,
        })
    });
    match run_gate(&candidate, &custom, &v, &registry, &provider) {
        GatePipelineOutcome::Rejected { rejection, .. } => {
            assert_eq!(rejection.rule_code, "RAMP_EXCEEDED");
            assert!(rejection.detail.contains("2.500000"));
            assert!(rejection.detail.contains("0.300000"));
        }
        other => panic!("expected a ramp rejection, got {other:?}"),
    }
}

/// GOOD, SUSPECT, and ESTIMATED baselines pass the quality gate (the
/// recorded policy); the recorded policy statement matches.
#[test]
fn good_suspect_and_estimated_baselines_pass_the_quality_gate() {
    for quality in [TelemetryQuality::Good, TelemetryQuality::Suspect, TelemetryQuality::Estimated] {
        let candidate = candidate();
        let v = versioned_model();
        let registry = registry_with_model(&v);
        let provider = provider_with_backend(ScriptedBackend::new(converged_outcome(EXPECTED_PIN)));
        let mut custom = sources();
        custom.ramp_baseline = Arc::new(move |_| {
            Ok(TelemeteredState {
                active_power_mw: 0.0,
                quality,
                observed_at_millis: NOW - 100,
            })
        });
        let outcome = run_gate(&candidate, &custom, &v, &registry, &provider);
        match outcome {
            GatePipelineOutcome::Completed { attempt, .. } => {
                // The quality travels on the derivation reference (recorded
                // alongside every derived injection, §10.4).
                assert!(attempt.injection_derivation_refs[0]
                    .contains(&format!("quality={}", quality.name())));
            }
            other => panic!("quality {quality:?} must pass, got {other:?}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Steps 1–4 end-to-end through the port
// ---------------------------------------------------------------------------

/// The full happy path: steps run IN ORDER, every intermediate evidence row
/// is present, and the attempt record carries the model version + hash, the
/// candidate refs, the derivation refs, the provider pin, and the applied
/// injections.
#[test]
fn the_pipeline_completes_in_order_with_every_intermediate_evidence_row() {
    let candidate = candidate();
    let v = versioned_model();
    let registry = registry_with_model(&v);
    let backend = ScriptedBackend::new(converged_outcome(EXPECTED_PIN));
    let provider = provider_with_backend(backend);
    let outcome = run_gate(&candidate, &sources(), &v, &registry, &provider);

    let (attempt, evidence) = match outcome {
        GatePipelineOutcome::Completed { attempt, evidence } => (attempt, evidence),
        other => panic!("expected completion, got {other:?}"),
    };

    // Step order: the row steps are non-decreasing and all four steps appear.
    let mut last_rank = 0;
    for row in &evidence {
        let rank = step_rank(row.step);
        assert!(
            rank >= last_rank,
            "evidence rows must follow the step order (found {:?} after a later step)",
            row.step
        );
        last_rank = rank;
    }
    for step in [
        GateStep::Step1CandidateValidation,
        GateStep::Step2InjectionDerivation,
        GateStep::Step3ModelApplication,
        GateStep::Step4PowerFlow,
    ] {
        assert!(
            evidence.iter().any(|row| row.step == step),
            "step {step:?} is missing from the evidence"
        );
    }

    // Every intermediate check row is present, in its step.
    let has = |code: &str, outcome: EvidenceOutcome| {
        evidence.iter().any(|row| row.rule_code == code && row.outcome == outcome)
    };
    // Step 1.
    assert!(has("REF_MISSING", EvidenceOutcome::Passed));
    assert!(has("UNITS_CANONICAL", EvidenceOutcome::Passed));
    assert!(has("TIMESTAMP_EXPIRED", EvidenceOutcome::Passed));
    assert!(has("VALIDITY_WINDOW_INVERTED", EvidenceOutcome::Passed));
    assert!(has("INTERVAL_OUTSIDE_WINDOW", EvidenceOutcome::Passed));
    assert!(has("ENVELOPE_STALE", EvidenceOutcome::Passed));
    assert!(has("REVISION_SUPERSEDED", EvidenceOutcome::Passed));
    // Step 2.
    assert!(has("BUS_UNRESOLVED", EvidenceOutcome::Passed));
    assert!(has("CAPABILITY_UNAVAILABLE", EvidenceOutcome::Passed));
    assert!(has("CAPABILITY_EXCEEDED", EvidenceOutcome::Passed));
    assert!(has("Q_CONVENTION_MISMATCH", EvidenceOutcome::Passed));
    assert!(has("RAMP_BASELINE_BLOCKED", EvidenceOutcome::Passed));
    assert!(has("RAMP_EXCEEDED", EvidenceOutcome::Passed));
    assert!(has("DERIVATION_MISMATCH", EvidenceOutcome::Passed));
    assert!(has("DERIVATION_RECORDED", EvidenceOutcome::Recorded));
    // Step 3.
    assert!(has("MODEL_NOT_HEALTHY", EvidenceOutcome::Passed));
    assert!(has("MODEL_HASH_MISMATCH", EvidenceOutcome::Passed));
    // Step 4.
    assert!(has("POWER_FLOW_EVALUATED", EvidenceOutcome::Recorded));

    // The attempt record is complete and carries the full provenance.
    attempt.validate_complete().expect("complete attempt record");
    assert_eq!(attempt.attempt_id, "attempt-1");
    assert_eq!(attempt.candidate_refs.candidate_id, "cand-gate-1");
    assert_eq!(attempt.candidate_refs.schedule_id, candidate.schedule_id);
    assert_eq!(attempt.candidate_refs.revision, 2);
    assert_eq!(attempt.candidate_refs.envelope_version, "env-v1");
    // Model version + hash: THE HASH the evidence references.
    assert_eq!(attempt.model_ref.version, "1");
    assert!(attempt.model_ref.content_hash.starts_with("fnv1a64:"));
    assert_eq!(attempt.model_ref.content_hash, v.model_ref().content_hash);
    // Provider pin recorded (REQUIRED, the P4.01 discipline).
    assert_eq!(attempt.provider_id, PROVIDER_ID);
    assert_eq!(attempt.provider_version, EXPECTED_PIN);
    // Derivation refs (baseline quality encoded).
    assert_eq!(attempt.injection_derivation_refs.len(), 1);
    assert!(attempt.injection_derivation_refs[0].contains("quality=GOOD"));
    // The applied injections are recorded per element on the result.
    assert_eq!(attempt.result().applied_injections.len(), 1);
    assert_eq!(attempt.result().applied_injections[0].injection_id, "sp-1");
    assert_eq!(attempt.result().applied_injections[0].bus_id, "bus_3");
    assert!(attempt.validate_complete().is_ok());
}

/// The derived injection round-trips the OWNED translation layer: the
/// canonical value and the declared convention are preserved exactly
/// (the fixtures are hand-computed; the mapping is the P4.02 layer).
#[test]
fn the_derivation_round_trips_the_translation_layer() {
    use flexcore_grid_api::translation::{candidate_to_backend, injection_from_backend};

    let setpoint = setpoint();
    let injection = flexcore_grid_api::port::CandidateInjection {
        injection_id: setpoint.setpoint_id.clone(),
        bus_id: setpoint.bus_id.clone(),
        active_power: setpoint.active_power,
        reactive_power: setpoint.reactive_power,
        reactive_sign_convention: setpoint.reactive_sign_convention,
        interval_start: None,
    };
    let backend = candidate_to_backend(&injection).expect("maps");
    // The declared convention is APPLIED in the worker mapping: an
    // injection-positive export stays a positive injection.
    assert!((backend.p_injection_mw - 2.5).abs() < 1.0e-12);
    assert!((backend.q_injection_mvar - 0.5).abs() < 1.0e-12);
    let back = injection_from_backend(&backend, setpoint.reactive_sign_convention)
        .expect("maps back");
    assert_eq!(back, injection, "the round trip is exact");
}

// ---------------------------------------------------------------------------
// Step 3 — model health and the mid-gate hash hook (fail-closed)
// ---------------------------------------------------------------------------

/// A stale model aborts at step 3 BEFORE any power flow runs (the health
/// check the gate calls; fail-closed).
#[test]
fn a_stale_model_aborts_before_the_power_flow() {
    let candidate = candidate();
    let v = versioned_model();
    let mut registry = registry_with_model(&v);
    registry
        .record_source_change("chg-1".to_string(), NOW - 500, "topology change".to_string())
        .expect("records");
    // The counted backend is shared between the test and the provider, so
    // the call count is readable after the run.
    let backend = Arc::new(ScriptedBackend::new(converged_outcome(EXPECTED_PIN)));
    let provider = PandapowerGridProvider::new(
        backend.clone(),
        flexcore_grid_api::adapter::balanced_worker_scope(),
    )
    .expect("balanced scope accepted");
    let outcome = run_gate(&candidate, &sources(), &v, &registry, &provider);
    match outcome {
        GatePipelineOutcome::Aborted { step, abort, evidence } => {
            assert_eq!(step, GateStep::Step3ModelApplication);
            match abort {
                GateAbort::ModelNotHealthy { findings } => {
                    assert!(findings
                        .iter()
                        .any(|f| matches!(f, flexcore_grid_api::model_health::HealthFinding::Stale { .. })));
                }
                other => panic!("expected ModelNotHealthy, got {other:?}"),
            }
            let last = evidence.last().expect("row");
            assert_eq!(last.rule_code, "MODEL_NOT_HEALTHY");
            assert_eq!(last.outcome, EvidenceOutcome::Aborted);
        }
        other => panic!("expected an abort, got {other:?}"),
    }
    // The backend was never consulted: the abort precedes the run.
    assert_eq!(backend.call_count(), 0, "a stale model must abort before the run");
}

/// A mid-gate model change (the live artefact no longer hashes to the
/// recorded hash) aborts fail-closed at the pre-run hook, AFTER the health
/// check passes.
#[test]
fn a_mid_gate_model_change_aborts_at_the_hash_hook() {
    let candidate = candidate();
    let mut v = versioned_model();
    let registry = registry_with_model(&v);
    // The silent edit after registration: the health check's structural
    // axes still pass (the edit keeps the model structurally valid), but
    // the recorded hash no longer matches the live content.
    v.model.branches[0].r_ohm += 0.05;
    assert!(!v.verify_content_hash());

    let backend = ScriptedBackend::new(converged_outcome(EXPECTED_PIN));
    let provider = provider_with_backend(backend);
    let outcome = run_gate(&candidate, &sources(), &v, &registry, &provider);
    match outcome {
        GatePipelineOutcome::Aborted { step, abort, evidence } => {
            assert_eq!(step, GateStep::Step3ModelApplication);
            match abort {
                GateAbort::ModelHashMismatch { recorded, computed } => {
                    assert!(recorded.starts_with("fnv1a64:"));
                    assert!(computed.starts_with("fnv1a64:"));
                    assert_ne!(recorded, computed);
                }
                other => panic!("expected ModelHashMismatch, got {other:?}"),
            }
            // Health passed (the MODEL_NOT_HEALTHY row is a Passed row);
            // the abort row is the hash mismatch.
            assert!(evidence.iter().any(|row| row.rule_code == "MODEL_NOT_HEALTHY"
                && row.outcome == EvidenceOutcome::Passed));
            let last = evidence.last().expect("row");
            assert_eq!(last.rule_code, "MODEL_HASH_MISMATCH");
            assert_eq!(last.outcome, EvidenceOutcome::Aborted);
        }
        other => panic!("expected an abort, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Step 4 — in-band inconclusive, pin requirement, scope limitations
// ---------------------------------------------------------------------------

/// Non-convergence is an in-band INCONCLUSIVE: recorded as evidence, the
/// pipeline still completes (the attempt record exists), and nothing in it
/// is a pass or a rejection-by-limit (P4.07 decides the verdict category).
#[test]
fn non_convergence_is_recorded_inconclusive_fail_closed() {
    let candidate = candidate();
    let v = versioned_model();
    let registry = registry_with_model(&v);
    let mut non_converged = converged_outcome(EXPECTED_PIN);
    non_converged.converged = false;
    let provider = provider_with_backend(ScriptedBackend::new(non_converged));
    let outcome = run_gate(&candidate, &sources(), &v, &registry, &provider);
    let attempt = match outcome {
        GatePipelineOutcome::Completed { attempt, evidence } => {
            let last = evidence.last().expect("row");
            assert_eq!(last.rule_code, "POWER_FLOW_INCONCLUSIVE");
            assert_eq!(last.outcome, EvidenceOutcome::Inconclusive);
            attempt
        }
        other => panic!("non-convergence is in-band, got {other:?}"),
    };
    match &attempt.output {
        AttemptOutput::Inconclusive { result, reason } => {
            assert_eq!(result.status, PowerFlowStatus::DidNotConverge);
            assert!(reason.contains("never a pass"));
            assert!(reason.contains("retry policy: NONE"));
        }
        other => panic!("expected Inconclusive, got {other:?}"),
    }
    attempt.validate_complete().expect("the inconclusive record is complete");
    // No converged diagnostics were fabricated.
    assert!(attempt.result().bus_voltages.is_empty());
}

/// The provider version is REQUIRED on the attempt record: an empty pin
/// fails the record completeness gate (the P4.01 pin discipline).
#[test]
fn the_provider_pin_is_required_on_the_attempt_record() {
    let candidate = candidate();
    let v = versioned_model();
    let registry = registry_with_model(&v);
    let provider = provider_with_backend(ScriptedBackend::new(converged_outcome(EXPECTED_PIN)));
    let outcome = run_gate(&candidate, &sources(), &v, &registry, &provider);
    let mut attempt = match outcome {
        GatePipelineOutcome::Completed { attempt, .. } => attempt,
        other => panic!("expected completion, got {other:?}"),
    };
    attempt.provider_version = "  ".to_string();
    assert_eq!(
        attempt.validate_complete(),
        Err(AttemptRecordError::EmptyField { field: "provider_version" })
    );
}

/// The scope-declared unbalanced/asymmetric limitation surfaces on every
/// attempt: the result carries the coverage limitations from the declared
/// scope, and the record carries the P4.34 deferral pointer (balanced
/// default per the recommendation; D3 OPEN).
#[test]
fn the_unbalanced_scope_limitation_and_deferral_pointer_surface() {
    let candidate = candidate();
    let v = versioned_model();
    let registry = registry_with_model(&v);
    let provider = provider_with_backend(ScriptedBackend::new(converged_outcome(EXPECTED_PIN)));
    let outcome = run_gate(&candidate, &sources(), &v, &registry, &provider);
    let attempt = match outcome {
        GatePipelineOutcome::Completed { attempt, .. } => attempt,
        other => panic!("expected completion, got {other:?}"),
    };
    assert!(!attempt.result().coverage_limitations.is_empty());
    assert!(attempt
        .result()
        .coverage_limitations
        .iter()
        .any(|limitation| limitation.label == "unbalanced_three_phase"));
    assert!(attempt.unbalanced_deferral_note.contains("P4.34"));
    assert_eq!(attempt.unbalanced_deferral_note, UNBALANCED_DEFERRAL_NOTE);
    assert!(AC_POWER_FLOW_STATEMENT.contains("NOT an AC-OPF"));
}

/// The scope declaration of the adapter the tests use really declares the
/// unbalanced limitation (a guard on the fixture chain: the assertion above
/// is only meaningful if the scope is the balanced one).
#[test]
fn the_balanced_scope_declares_the_unbalanced_limitation() {
    let scope = flexcore_grid_api::adapter::balanced_worker_scope();
    assert!(scope
        .not_modelled
        .iter()
        .any(|item: &NotModelled| item.label == "unbalanced_three_phase"));
}
