// P8.07/P8.08 — integration flow tests for the ADMM/DMPC provider: the
// public surface composes end-to-end — coupled problem (`admm`) → rolling-
// horizon plan (`dmpc`) → research proposal (`validation`) → the P8.05
// independent validation harness — plus the deliberately-wrong cases the
// harness must catch. Ledger `ledger/phase-08-advanced-coordination/
// ledger_phase_8.md` §P8.07/§P8.08; decision records:
// `ledger/phase-08-advanced-coordination/07_admm_dmpc.md` and
// `ledger/phase-08-advanced-coordination/08_admm_operations.md`.
//
// Honest status: WRITTEN-UNRUN — no cargo command was executed in this loop
// (the subtask's loop bans forbid build/test); the human build/test gate
// owns the run. Every expected value is hand-computed arithmetic stated in
// the test comments (the toy's optimum and fixed point are derived on
// paper) — nothing is fabricated, no benchmark figure exists.
//
// Control status: research proposals are DATA (ADR-010). Nothing here
// authorizes, issues, approves, or dispatches anything.

use std::collections::BTreeMap;

use flexcore_research::admm::{CoupledProblem, Participant};
use flexcore_research::baselines::PriorityListFallback;
use flexcore_research::dmpc::{
    plan_horizon, proposal_for_first_step, DmpcHorizonInput, TerminationCriteria,
};
use flexcore_research::validation::{
    GateSeamError, HarnessClock, IndependentFeasibilityGate, IndependentVerdict, ResearchProvenance,
    ResearchResultKind, ResearchValidityWindow, SnapshotFreshness, SnapshotFreshnessSource,
    ValidationHarness,
};

/// Demand-floor participant: a_k = −1 (its export counts toward meeting the
/// aggregate floor b = −d).
fn demand_participant(id: &str, q: f64, c: f64, p_min: f64, p_max: f64) -> Participant {
    Participant {
        participant_id: id.to_string(),
        cost_curvature: q,
        preferred_output_mw: c,
        p_min_mw: p_min,
        p_max_mw: p_max,
        energy_capacity_mwh: None,
        soc_fraction: None,
        coupling_coefficient: -1.0,
    }
}

/// The toy problem (the P8.07/P8.08 fixed-point toy): two symmetric
/// participants, q = 2, c = 0, boxes [0, 3], demand d = 4 (b = −4), ρ = 2.
/// Hand-derived optimum: minimize x_a² + x_b² s.t. x_a + x_b ≥ 4 → the
/// constraint is active, symmetry gives x_a = x_b = 2 (marginal costs
/// 2·x equal at λ = 4 > 0); the operational run converges there EXACTLY in
/// 2 iterations (hand-traced in the dmpc.rs unit tests).
fn toy_horizon_input(horizon_id: &str) -> DmpcHorizonInput {
    DmpcHorizonInput {
        horizon_id: horizon_id.to_string(),
        interval_hours: 1.0,
        interval_demand_mw: vec![4.0],
        participants: vec![
            demand_participant("a", 2.0, 0.0, 0.0, 3.0),
            demand_participant("b", 2.0, 0.0, 0.0, 3.0),
        ],
        rho: 2.0,
        criteria: TerminationCriteria::standard(),
    }
}

/// The injected feasibility gate (passing) — the harness supplies the
/// physics; the fakes mirror the in-module test fixtures.
struct PassingGate;

impl IndependentFeasibilityGate for PassingGate {
    fn revalidate(
        &self,
        _proposal: &flexcore_research::validation::ResearchProposal,
    ) -> Result<IndependentVerdict, GateSeamError> {
        Ok(IndependentVerdict::Passed {
            attempt_id: "AT-FLOW".to_string(),
            model_id: "toy".to_string(),
            model_content_hash: "hash-flow".to_string(),
            margins: "no violations".to_string(),
            coverage: "declared scope only".to_string(),
        })
    }
}

/// The injected gate that fails the proposal on a named limit (the
/// deliberately-wrong case: the provider's answer is NOT trusted).
struct LimitViolatedGate;

impl IndependentFeasibilityGate for LimitViolatedGate {
    fn revalidate(
        &self,
        _proposal: &flexcore_research::validation::ResearchProposal,
    ) -> Result<IndependentVerdict, GateSeamError> {
        Ok(IndependentVerdict::LimitViolated {
            attempt_id: "AT-FLOW-2".to_string(),
            model_id: "toy".to_string(),
            model_content_hash: "hash-flow".to_string(),
            constraint: "substation headroom exceeded".to_string(),
        })
    }
}

struct CurrentSnapshots;

impl SnapshotFreshnessSource for CurrentSnapshots {
    fn freshness(&self, _snapshot_id: &str) -> SnapshotFreshness {
        SnapshotFreshness::Current
    }
}

struct FixedClock {
    now: i64,
}

impl HarnessClock for FixedClock {
    fn now_millis(&self) -> i64 {
        self.now
    }
}

#[test]
fn the_provider_flows_end_to_end_into_a_validated_proposal() {
    // Coupled problem → rolling-horizon plan → proposal → INDEPENDENT
    // harness. The plan's first step is the converged toy solution (2, 2)
    // — hand-derived and hand-traced (see above).
    let input = toy_horizon_input("flow-1");
    let plan = plan_horizon(&input, &PriorityListFallback).expect("the plan plans");
    assert_eq!(plan.intervals.len(), 1);
    assert_eq!(plan.intervals[0].run.status.name(), "CONVERGED");
    assert_eq!(plan.intervals[0].run.iterations_run, 2);
    assert_eq!(plan.intervals[0].run.distributed_x_mw["a"], 2.0);
    assert_eq!(plan.intervals[0].run.distributed_x_mw["b"], 2.0);

    let proposal = proposal_for_first_step(
        &plan,
        "plugin/research-admm",
        "0.1.0",
        1,
        "grid-model/2026.11",
        "snapshot-2026-11-13-a",
        ResearchValidityWindow {
            valid_from_ms: 1_000,
            valid_until_ms: 2_000,
        },
    )
    .expect("the builder answers")
    .expect("the converged plan proposes");
    assert_eq!(proposal.result_kind, ResearchResultKind::ProposedSchedule);
    assert_eq!(proposal.provenance.termination_status, "CONVERGED");

    let gate = PassingGate;
    let snapshots = CurrentSnapshots;
    let clock = FixedClock { now: 1_500 };
    let harness = ValidationHarness::new(&gate, &snapshots, &clock);
    let report = harness.validate(&proposal).expect("full pass");
    assert!(report.is_validated());
    let validated = report.validated.as_ref().expect("validated");
    // Hand-computed from the recorded evidence-reference format.
    assert_eq!(
        validated.evidence_reference,
        "feasibility:attempt/AT-FLOW:model/toy:hash/hash-flow"
    );
}

#[test]
fn the_harness_catches_a_physically_infeasible_provider_answer() {
    // The deliberately-wrong case: the same proposal, but the independent
    // gate finds a violated limit → named refusal, never forwarded. The
    // provider does not validate its own output.
    let input = toy_horizon_input("flow-2");
    let plan = plan_horizon(&input, &PriorityListFallback).expect("the plan plans");
    let proposal = proposal_for_first_step(
        &plan,
        "plugin/research-admm",
        "0.1.0",
        1,
        "grid-model/2026.11",
        "snapshot-2026-11-13-a",
        ResearchValidityWindow {
            valid_from_ms: 1_000,
            valid_until_ms: 2_000,
        },
    )
    .expect("the builder answers")
    .expect("the converged plan proposes");
    let gate = LimitViolatedGate;
    let snapshots = CurrentSnapshots;
    let clock = FixedClock { now: 1_500 };
    let harness = ValidationHarness::new(&gate, &snapshots, &clock);
    let refusal = harness
        .validate(&proposal)
        .expect_err("a limit-violated proposal is refused");
    assert_eq!(refusal.name(), "PHYSICAL_REVALIDATION_LIMIT_VIOLATED");
}

#[test]
fn the_harness_refuses_a_proposal_with_incomplete_provenance() {
    // The second deliberately-wrong case: a hand-built proposal with a
    // MISSING artifact reference (the completeness stage cannot know which
    // module produced it and must not guess — fail-closed). This proves the
    // harness gates THIS crate's own outputs too: no self-trust.
    let provenance = ResearchProvenance {
        worker_version: "flexcore-research/dmpc.v1".to_string(),
        model_version: "grid-model/2026.11".to_string(),
        solver_version: "flexcore-research/admm-operations.v1".to_string(),
        input_snapshot_id: "snapshot-2026-11-13-a".to_string(),
        termination_status: "CONVERGED".to_string(),
        diagnostics: Vec::new(),
        seed: None,
        seed_supported: false,
        deterministic: true,
        model_artifact_ref: String::new(), // MISSING — refused
    };
    let proposal = flexcore_research::validation::ResearchProposal::new(
        "prop-flow-3",
        "plugin/research-admm",
        "0.1.0",
        1,
        ResearchResultKind::ProposedSchedule,
        serde_json::json!({ "setpoints_mw": { "a": 2.0, "b": 2.0 } }),
        provenance,
        ResearchValidityWindow {
            valid_from_ms: 1_000,
            valid_until_ms: 2_000,
        },
    )
    .expect("shape-valid");
    let gate = PassingGate;
    let snapshots = CurrentSnapshots;
    let clock = FixedClock { now: 1_500 };
    let harness = ValidationHarness::new(&gate, &snapshots, &clock);
    let refusal = harness.validate(&proposal).expect_err("refused");
    assert_eq!(refusal.name(), "PROVENANCE_INCOMPLETE");
}

#[test]
fn two_plans_of_the_same_input_are_identical() {
    // Determinism: same input → identical plan (the serialized forms are
    // byte-equal; the BTreeMap ordering is part of the contract).
    let first = plan_horizon(&toy_horizon_input("flow-det"), &PriorityListFallback)
        .expect("the plan plans");
    let second = plan_horizon(&toy_horizon_input("flow-det"), &PriorityListFallback)
        .expect("the plan plans");
    assert_eq!(first, second);
    let first_json = serde_json::to_string(&first).expect("serializes");
    let second_json = serde_json::to_string(&second).expect("serializes");
    assert_eq!(first_json, second_json);
}

#[test]
fn the_refusal_vocabulary_covers_the_full_path() {
    // A refused fallback leaves NO schedule: an export-cap problem (a = +1)
    // with an exhausted budget — the in-crate frozen fallback refuses the
    // class, and the result carries the recorded failure. The DMPC driver
    // fixes the demand-floor class (a = −1), so the export-cap problem is
    // built directly through the coordination layer, exactly as a
    // composition root would. (Hand-trace: iteration 1 gives x = (2.5, 2.5)
    // — s = 0 → x = q·c/(q + ρa²) = 10/4 — with primal residual
    // ‖(0.5, 0.5)‖ > 0, so a budget of 1 exhausts the run.)
    let mut participants = Vec::new();
    for id in ["a", "b"] {
        participants.push(Participant {
            participant_id: id.to_string(),
            cost_curvature: 2.0,
            preferred_output_mw: 5.0,
            p_min_mw: 0.0,
            p_max_mw: 5.0,
            energy_capacity_mwh: None,
            soc_fraction: None,
            coupling_coefficient: 1.0,
        });
    }
    let mut criteria = TerminationCriteria::standard();
    criteria.max_iterations = 1;
    let problem =
        CoupledProblem::new("flow-4-cap", 4.0, 1.0, participants.clone()).expect("valid problem");
    let worker_a = flexcore_research::dmpc::ClosedFormLocalWorker {
        participant: participants[0].clone(),
    };
    let worker_b = flexcore_research::dmpc::ClosedFormLocalWorker {
        participant: participants[1].clone(),
    };
    let mut workers: BTreeMap<String, &dyn flexcore_research::dmpc::AdmmLocalWorker> =
        BTreeMap::new();
    workers.insert("a".to_string(), &worker_a);
    workers.insert("b".to_string(), &worker_b);
    let coordination = flexcore_research::dmpc::AdmmCoordination::new(
        &problem,
        workers,
        &PriorityListFallback,
        criteria,
    )
    .expect("coverage ok");
    let result = coordination.run(2.0).expect("penalty valid");
    assert_eq!(result.status.name(), "NON_CONVERGED_BUDGET_EXHAUSTED");
    let error = result.fallback_error.as_ref().expect("the refusal is recorded");
    assert!(error.contains("does not support this coupling class"), "{error}");
    assert!(
        result.final_schedule().is_none(),
        "a refused fallback leaves NO schedule — never a silently partial result"
    );
}
