// P10.05 — Optimizer independent validation (release-gate harness slice):
//         the physical re-validation consumption path — the validator that
//         does NOT trust the optimizer/researcher (report §22.3 gate 4;
//         §12.5; §17.5 invariant 9; ledger P10.05 steps 2 and 4; record:
//         `ledger/phase-10-v1-release-hardening/05_optimizer_validation.md`).
//
// WHAT THIS FILE PROVES (over the P8.05 independent validation harness,
// `crates/flexcore-research/src/validation.rs` — the §12.5 feasibility-gate
// CONSUMPTION path; the harness never re-implements physics, the gate is
// the platform's existing independent feasibility pipeline injected as a
// port):
//
// 1. DELIBERATELY-WRONG OPTIMIZER OUTPUTS ARE CAUGHT. A proposal whose
//    payload violates a network limit is refused with the named refusal
//    `PHYSICAL_REVALIDATION_LIMIT_VIOLATED` and the violated constraint
//    named — even though the proposal's own provenance diagnostics claim
//    "feasible/optimal/safe". The optimizer never validates itself: the
//    proposal type has NO field a producer could use to assert a verdict,
//    and the injected gate's verdict is the only decision input.
// 2. A FAILED / NON-CONVERGED / UNAVAILABLE VALIDATION IS FAIL-CLOSED.
//    A gate verdict of FailClosed ("power-flow did not converge") and a
//    gate seam error are refusals (`PHYSICAL_REVALIDATION_FAIL_CLOSED` /
//    `PHYSICAL_REVALIDATION_SEAM_FAILED`) — never an inferred pass
//    (§12.5 closing rule; §17.5 invariant 9: a stale or failed model is
//    never interpreted as a passing feasibility check).
// 3. A FULL PASS PRODUCES A SEALED VALIDATED PROPOSAL carrying the model
//    hash and the recorded `feasibility:attempt/<id>:model/<mid>:hash/<h>`
//    evidence reference (hand-computed known answer), stamped with the
//    harness version. No other API returns a validated proposal (the
//    constructor is crate-private; the seal field makes external
//    construction a compile error).
// 4. THE GATE IS PHYSICALLY THE LAST STAGE. Every cheaper refusal —
//    expired validity window, unknown/superseded input snapshot, missing
//    provenance, malformed envelope — happens with ZERO gate attempts
//    (a stale or malformed candidate never consumes a feasibility run).
// 5. THE VALIDATOR AND THE OPTIMIZER ARE DIFFERENT CODE PATHS. The gate
//    is an injected port implemented by the composition root over the
//    platform's feasibility pipeline; the harness consumes it without
//    re-implementing it (the alignment seam is recorded and pinned).
//
// EMPTY BENCHMARK CELL (honest status, ENTRY_NOTE boundary 4): NO optimizer
// has been built or run, and no §19.3–§19.4 benchmark or validation-run
// number is recorded here. What is validated is the CONTRACT — the refusal
// classes, the fail-closed semantics, the seal, and the stage order — with
// the gate as a scripted injected port (a test double, not a solver and not
// a benchmark). The P9.19 shadow-run cross-reference and the pilot grid
// model (P9.03) are cited in the record; no shadow case is re-run here.
//
// STATUS: WRITTEN-UNRUN (no cargo command was executed in this loop; the
// human build/test gate owns:
// `cargo test -p flexcore-research --test p10_05_independent_validation`).

use std::cell::Cell;

use flexcore_research::validation::{
    GateSeamError, HarnessClock, IndependentFeasibilityGate, IndependentVerdict, Refusal,
    ResearchProposal, ResearchProvenance, ResearchResultKind, ResearchValidityWindow,
    SnapshotFreshness, SnapshotFreshnessSource, ValidationHarness, HARNESS_VERSION, STAGE_ORDER,
    VALIDATION_STATEMENT,
};

// --- Injected collaborators ------------------------------------------------------------

struct FixedClock {
    now: i64,
}

impl HarnessClock for FixedClock {
    fn now_millis(&self) -> i64 {
        self.now
    }
}

struct FixedSnapshots {
    fresh: SnapshotFreshness,
}

impl SnapshotFreshnessSource for FixedSnapshots {
    fn freshness(&self, _snapshot_id: &str) -> SnapshotFreshness {
        self.fresh.clone()
    }
}

/// The gate spy: returns the programmed verdict AND counts entries — the
/// instrument that proves the gate is last and exactly-once.
struct GateSpy {
    verdict: Result<IndependentVerdict, GateSeamError>,
    calls: Cell<usize>,
}

impl GateSpy {
    fn passing() -> GateSpy {
        GateSpy {
            verdict: Ok(IndependentVerdict::Passed {
                attempt_id: "AT-P10-05-1".to_string(),
                model_id: "ieee13".to_string(),
                model_content_hash: "gridmodel-abc123".to_string(),
                margins: "min v_pu 0.97, max loading 0.71".to_string(),
                coverage: "balanced radial, declared scope only".to_string(),
            }),
            calls: Cell::new(0),
        }
    }

    fn with(verdict: Result<IndependentVerdict, GateSeamError>) -> GateSpy {
        GateSpy {
            verdict,
            calls: Cell::new(0),
        }
    }
}

impl IndependentFeasibilityGate for GateSpy {
    fn revalidate(&self, _proposal: &ResearchProposal) -> Result<IndependentVerdict, GateSeamError> {
        self.calls.set(self.calls.get() + 1);
        self.verdict.clone()
    }
}

/// Reference provenance: every field complete and honest (non-bare
/// termination status; seed recorded where seed support is claimed).
fn reference_provenance() -> ResearchProvenance {
    ResearchProvenance {
        worker_version: "optimization-worker/0.1.0".to_string(),
        model_version: "grid-model/2026.11".to_string(),
        solver_version: "highspy 1.15.1 (Tier 1 LP/QP/MIP — not SOCP)".to_string(),
        input_snapshot_id: "snapshot-p10-05".to_string(),
        termination_status: "converged".to_string(),
        diagnostics: Vec::new(),
        seed: Some(7),
        seed_supported: true,
        deterministic: true,
        model_artifact_ref: "artifact://models/scheduler.v1".to_string(),
    }
}

fn reference_window() -> ResearchValidityWindow {
    ResearchValidityWindow {
        valid_from_ms: 1_000,
        valid_until_ms: 2_000,
    }
}

/// The optimizer-output candidate fixture: a proposed schedule the
/// PRODUCER considers feasible. The payload carries a setpoint vector the
/// gate will re-validate physically.
fn optimizer_proposal(payload: serde_json::Value, diagnostics: Vec<String>) -> ResearchProposal {
    ResearchProposal::new(
        "cand-p10-05-1",
        "builtin.optimizer-schedule",
        "0.1.0",
        1,
        ResearchResultKind::ProposedSchedule,
        payload,
        ResearchProvenance {
            diagnostics,
            ..reference_provenance()
        },
        reference_window(),
    )
    .expect("the reference proposal is shape-valid")
}

fn harness<'a>(gate: &'a GateSpy, snapshots: &'a FixedSnapshots, clock: &'a FixedClock) -> ValidationHarness<'a> {
    ValidationHarness::new(gate, snapshots, clock)
}

// --- 1. Deliberately-wrong optimizer outputs are caught ---------------------------------

#[test]
fn a_limit_violating_optimizer_output_is_refused_with_the_named_constraint() {
    // The optimizer's payload asks for a setpoint that violates a branch
    // rating. The gate says LimitViolated; the harness refuses with the
    // constraint NAMED — the optimizer's own diagnostics (claiming
    // feasible) are never consulted.
    let proposal = optimizer_proposal(
        serde_json::json!({ "setpoints_mw": [12.5] }), // beyond the feeder rating
        vec!["feasible: yes, optimal: yes (producer's own claim)".to_string()],
    );
    let gate = GateSpy::with(Ok(IndependentVerdict::LimitViolated {
        attempt_id: "AT-P10-05-2".to_string(),
        model_id: "ieee13".to_string(),
        model_content_hash: "gridmodel-abc123".to_string(),
        constraint: "branch-7 current above rating".to_string(),
    }));
    let snapshots = FixedSnapshots {
        fresh: SnapshotFreshness::Current,
    };
    let clock = FixedClock { now: 1_500 };
    let h = harness(&gate, &snapshots, &clock);
    let refusal = h.validate(&proposal).expect_err("a limit-violating output is refused");
    assert_eq!(
        refusal,
        Refusal::PhysicalRevalidationLimitViolated {
            constraint: "branch-7 current above rating".to_string()
        }
    );
    assert_eq!(refusal.name(), "PHYSICAL_REVALIDATION_LIMIT_VIOLATED");
    assert_eq!(gate.calls.get(), 1, "exactly one gate attempt — the gate decided, not the producer");
}

#[test]
fn the_producer_supplies_no_verdict_and_self_declared_feasibility_changes_nothing() {
    // THE OPTIMIZER NEVER VALIDATES ITSELF: the same violating payload
    // with maximal self-praise in diagnostics is refused identically —
    // there is no field a producer could use to assert its own verdict
    // (ADR-010; the P8.05 no-self-validation rule).
    let praising = vec![
        "feasible: yes".to_string(),
        "optimal: yes".to_string(),
        "safe: yes".to_string(),
        "no violations: yes".to_string(),
    ];
    let proposal = optimizer_proposal(serde_json::json!({ "setpoints_mw": [12.5] }), praising);
    let gate = GateSpy::with(Ok(IndependentVerdict::LimitViolated {
        attempt_id: "AT-P10-05-3".to_string(),
        model_id: "ieee13".to_string(),
        model_content_hash: "gridmodel-abc123".to_string(),
        constraint: "bus-675 voltage below 0.95 pu".to_string(),
    }));
    let snapshots = FixedSnapshots {
        fresh: SnapshotFreshness::Current,
    };
    let clock = FixedClock { now: 1_500 };
    let refusal = harness(&gate, &snapshots, &clock)
        .validate(&proposal)
        .expect_err("self-declared feasibility is not a verdict");
    assert_eq!(
        refusal,
        Refusal::PhysicalRevalidationLimitViolated {
            constraint: "bus-675 voltage below 0.95 pu".to_string()
        }
    );
    assert!(VALIDATION_STATEMENT.contains("never accepts a researcher-supplied verdict"));
}

// --- 2. Failed / non-converged validation is fail-closed, never an inferred pass ---------

#[test]
fn a_non_converged_or_failed_validation_is_fail_closed_never_a_pass() {
    let snapshots = FixedSnapshots {
        fresh: SnapshotFreshness::Current,
    };
    let clock = FixedClock { now: 1_500 };
    // The feasibility run did not converge: refusal, not a pass
    // (§12.5 closing rule; §17.5 invariant 9).
    let gate = GateSpy::with(Ok(IndependentVerdict::FailClosed {
        attempt_id: Some("AT-P10-05-4".to_string()),
        reason: "AC power flow did not converge on the versioned network model".to_string(),
    }));
    let refusal = harness(&gate, &snapshots, &clock)
        .validate(&optimizer_proposal(serde_json::json!({ "setpoints_mw": [1.0] }), Vec::new()))
        .expect_err("a failed validation is a refusal");
    assert_eq!(
        refusal,
        Refusal::PhysicalRevalidationFailClosed {
            reason: "AC power flow did not converge on the versioned network model".to_string()
        }
    );
    assert_eq!(refusal.name(), "PHYSICAL_REVALIDATION_FAIL_CLOSED");
    // The gate itself failed (seam error): fail-closed, never a pass.
    let gate = GateSpy::with(Err(GateSeamError {
        seam: "feasibility_gate",
        detail: "gate port not wired".to_string(),
    }));
    let refusal = harness(&gate, &snapshots, &clock)
        .validate(&optimizer_proposal(serde_json::json!({ "setpoints_mw": [1.0] }), Vec::new()))
        .expect_err("a seam error is a refusal");
    assert_eq!(
        refusal,
        Refusal::PhysicalRevalidationSeamFailed {
            seam: "feasibility_gate",
            detail: "gate port not wired".to_string()
        }
    );
    assert_eq!(refusal.name(), "PHYSICAL_REVALIDATION_SEAM_FAILED");
}

// --- 3. A full pass produces a sealed proposal with the known-answer evidence ------------

#[test]
fn a_passing_output_is_sealed_with_the_hand_computed_evidence_reference() {
    let gate = GateSpy::passing();
    let snapshots = FixedSnapshots {
        fresh: SnapshotFreshness::Current,
    };
    let clock = FixedClock { now: 1_500 };
    let report = harness(&gate, &snapshots, &clock)
        .validate(&optimizer_proposal(
            serde_json::json!({ "setpoints_mw": [1.5, 2.5] }),
            Vec::new(),
        ))
        .expect("full pass");
    assert!(report.is_validated());
    assert_eq!(report.stages_passed, STAGE_ORDER.to_vec());
    assert_eq!(report.harness_version, HARNESS_VERSION);
    let validated = report.validated.as_ref().expect("validated on a pass");
    assert_eq!(validated.proposal_id, "cand-p10-05-1");
    assert_eq!(validated.model_content_hash, "gridmodel-abc123");
    // Hand-computed from the recorded evidence-reference format.
    assert_eq!(
        validated.evidence_reference,
        "feasibility:attempt/AT-P10-05-1:model/ieee13:hash/gridmodel-abc123"
    );
    assert_eq!(validated.harness_version, HARNESS_VERSION);
    assert!(validated.producer_deterministic);
    assert_eq!(gate.calls.get(), 1, "exactly one gate attempt on the pass");
}

// --- 4. The gate is physically the last stage ---------------------------------------------

#[test]
fn stale_and_malformed_candidates_never_consume_a_gate_attempt() {
    let snapshots_unknown = FixedSnapshots {
        fresh: SnapshotFreshness::Unknown,
    };
    let clock = FixedClock { now: 1_500 };

    // Unknown input snapshot (the stale/unknown-model rule): refused with
    // ZERO gate attempts — an unknown model is never fresh, never a pass.
    let gate = GateSpy::passing();
    let refusal = harness(&gate, &snapshots_unknown, &clock)
        .validate(&optimizer_proposal(serde_json::json!({ "setpoints_mw": [1.0] }), Vec::new()))
        .expect_err("an unknown snapshot is refused");
    assert_eq!(refusal.name(), "SNAPSHOT_UNKNOWN");
    assert_eq!(gate.calls.get(), 0, "no gate attempt on a stale/unknown model");

    // Superseded input snapshot: the replacing id travels.
    let gate = GateSpy::passing();
    let snapshots_superseded = FixedSnapshots {
        fresh: SnapshotFreshness::Superseded {
            by_snapshot_id: "snapshot-p10-05-b".to_string(),
        },
    };
    let refusal = harness(&gate, &snapshots_superseded, &clock)
        .validate(&optimizer_proposal(serde_json::json!({ "setpoints_mw": [1.0] }), Vec::new()))
        .expect_err("a superseded snapshot is refused");
    assert_eq!(
        refusal,
        Refusal::SnapshotSuperseded {
            by_snapshot_id: "snapshot-p10-05-b".to_string()
        }
    );
    assert_eq!(gate.calls.get(), 0);

    // Expired candidate window (the injected clock decides): refused, never
    // forwarded.
    let gate = GateSpy::passing();
    let late_clock = FixedClock { now: 2_500 };
    let refusal = harness(&gate, &snapshots_unknown, &late_clock)
        .validate(&optimizer_proposal(serde_json::json!({ "setpoints_mw": [1.0] }), Vec::new()))
        .expect_err("an expired candidate is refused");
    assert_eq!(refusal.name(), "PROPOSAL_EXPIRED");
    assert_eq!(gate.calls.get(), 0);

    // Missing provenance: refused before the gate.
    let gate = GateSpy::passing();
    let incomplete = ResearchProvenance {
        input_snapshot_id: String::new(),
        ..reference_provenance()
    };
    let proposal = ResearchProposal::new(
        "cand-p10-05-2",
        "builtin.optimizer-schedule",
        "0.1.0",
        1,
        ResearchResultKind::ProposedSchedule,
        serde_json::json!({}),
        incomplete,
        reference_window(),
    )
    .expect("shape-valid");
    let refusal = harness(&gate, &snapshots_unknown, &clock)
        .validate(&proposal)
        .expect_err("incomplete provenance is refused");
    assert_eq!(
        refusal,
        Refusal::ProvenanceIncomplete {
            field: "input_snapshot_id"
        }
    );
    assert_eq!(gate.calls.get(), 0);
}

// --- 5. The validator and the optimizer are different code paths --------------------------

#[test]
fn the_gate_is_an_injected_port_and_the_harness_never_reimplements_physics() {
    // The alignment seam is recorded and pinned: the harness consumes the
    // platform's feasibility gate as a port (the composition root
    // implements it); it contains no power flow of its own.
    assert!(flexcore_research::validation::FEASIBILITY_VERDICT_ALIGNMENT_SEAM
        .contains("command_gates.rs"));
    assert!(flexcore_research::validation::FEASIBILITY_VERDICT_ALIGNMENT_SEAM
        .contains("FeasibilityGateVerdict"));
    assert!(VALIDATION_STATEMENT.contains("never re-implements physics"));
    // The stage order is the fixed contract, with the gate LAST.
    assert_eq!(
        STAGE_ORDER,
        [
            "envelope_shape",
            "result_vocabulary",
            "provenance_completeness",
            "validity_window",
            "snapshot_freshness",
            "physical_revalidation",
        ]
    );
    // A validated proposal gains NO authority from validation (ADR-010):
    // it is a candidate for the protected path.
    assert!(VALIDATION_STATEMENT.contains("NO authority"));
}
