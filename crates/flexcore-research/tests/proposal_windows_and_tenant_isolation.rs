// P8.31/P8.34 — integration tests for the research tenant-isolation and
// proposal-validity-window modules: the ledger's hostile-proposal suites,
// end to end through the P8.05 independent validation harness. Ledger
// `ledger/phase-08-advanced-coordination/ledger_phase_8.md` §P8.31/§P8.34;
// decision records:
// `ledger/phase-08-advanced-coordination/31_tenant_isolation.md` and
// `ledger/phase-08-advanced-coordination/34_proposal_windows.md`.
//
// P8.34 verification method (the ledger's "submit the three hostile
// proposals and record the rejections"): an EXPIRED proposal, a proposal
// against a SUPERSEDED snapshot, and a proposal with a MISSING window —
// each rejected by the same gate vocabulary, and the expired/superseded
// ones never reaching the physical gate.
//
// P8.31 verification method ("independent re-run of the cross-tenant
// suite"): a worker scoped to tenant A attempts to read tenant B's data —
// every attempt denied and audited; a model trained on tenant A's data is
// not servable to tenant B without a recorded approval.
//
// Honest status: WRITTEN-UNRUN — no cargo command was executed in this
// loop (the subtask's loop bans forbid build/test); the human build/test
// gate owns the run. Every expected value is hand-computed from the
// recorded rules — nothing is fabricated.
//
// Control status: research proposals are DATA (ADR-010). Nothing here
// authorizes, issues, approves, or dispatches anything.

use std::cell::Cell;

use flexcore_research::proposal_windows::{
    gate_at_time_of_use, grant_validity_window, window_from_wire_parts,
    CheckpointedSnapshotSource, MemorySnapshotSupersessionRegister, MAX_VALIDITY_WINDOW_MS,
};
use flexcore_research::tenant_isolation::{
    MemoryTenantIsolationAuditSink, ModelArtifactRegistration, ModelServingApproval,
    ResearchDataKind, ResearchDataRecord, ResearchWorkerFamily, ResearchWorkerScope,
    ScopedResearchCatalog, TenantIsolationAuditKind, WorkerScopeStamp,
};
use flexcore_research::validation::{
    GateSeamError, HarnessClock, IndependentFeasibilityGate, IndependentVerdict, ResearchProposal,
    ResearchProvenance, ResearchResultKind, ResearchValidityWindow, ValidationHarness,
};

struct FixedClock {
    now: i64,
}

impl HarnessClock for FixedClock {
    fn now_millis(&self) -> i64 {
        self.now
    }
}

/// The passing gate spy: counts how many times the physical gate was
/// entered, so the tests can pin that a stale/malformed proposal never
/// consumes a feasibility attempt.
struct GateSpy {
    calls: Cell<usize>,
}

impl GateSpy {
    fn new() -> GateSpy {
        GateSpy { calls: Cell::new(0) }
    }
}

impl IndependentFeasibilityGate for GateSpy {
    fn revalidate(
        &self,
        _proposal: &ResearchProposal,
    ) -> Result<IndependentVerdict, GateSeamError> {
        self.calls.set(self.calls.get() + 1);
        Ok(IndependentVerdict::Passed {
            attempt_id: "AT-IT".to_string(),
            model_id: "ieee13".to_string(),
            model_content_hash: "hash-it".to_string(),
            margins: "no violations".to_string(),
            coverage: "declared scope only".to_string(),
        })
    }
}

/// A proposal computed against window [1_000, 2_000) and snapshot
/// "snap-2026-11-14-a" (shape-valid by construction).
fn reference_proposal() -> ResearchProposal {
    ResearchProposal::new(
        "prop-it-1",
        "plugin/research-admm",
        "0.1.0",
        1,
        ResearchResultKind::ProposedSchedule,
        serde_json::json!({ "setpoints_mw": [1.5, 2.5] }),
        ResearchProvenance {
            worker_version: "research-worker/0.1.0".to_string(),
            model_version: "grid-model/2026.11".to_string(),
            solver_version: "highspy 1.15.1".to_string(),
            input_snapshot_id: "snap-2026-11-14-a".to_string(),
            termination_status: "converged".to_string(),
            diagnostics: Vec::new(),
            seed: Some(7),
            seed_supported: true,
            deterministic: true,
            model_artifact_ref: "artifact://models/admm.v1".to_string(),
        },
        ResearchValidityWindow { valid_from_ms: 1_000, valid_until_ms: 2_000 },
    )
    .expect("the reference proposal is shape-valid")
}

// ---------------------------------------------------------------------------
// P8.34 — the three hostile proposals (the ledger's verification method)
// ---------------------------------------------------------------------------

#[test]
fn hostile_proposal_1_expired_is_rejected_and_never_reaches_the_gate() {
    let gate = GateSpy::new();
    let register = MemorySnapshotSupersessionRegister::new();
    register
        .register_current("snap-2026-11-14-a")
        .expect("the snapshot is registered");
    let source = CheckpointedSnapshotSource::new(register);
    let clock = FixedClock { now: 2_000 }; // exactly-at-expiry — REJECTED (the recorded rule)
    let harness = ValidationHarness::new(&gate, &source, &clock);
    let refusal = harness
        .validate(&reference_proposal())
        .expect_err("an expired proposal is rejected");
    assert_eq!(refusal.name(), "PROPOSAL_EXPIRED");
    assert_eq!(refusal, flexcore_research::validation::Refusal::ProposalExpired {
        valid_until_ms: 2_000
    });
    assert_eq!(gate.calls.get(), 0, "a stale proposal never consumes a feasibility attempt");
    // The consumption-point re-check agrees (same gate vocabulary).
    let gate_refusal = gate_at_time_of_use(&reference_proposal(), &clock, &source)
        .expect_err("expired at the point of use too");
    assert_eq!(gate_refusal.name(), "PROPOSAL_EXPIRED");
}

#[test]
fn hostile_proposal_2_superseded_snapshot_is_rejected_and_never_reaches_the_gate() {
    let gate = GateSpy::new();
    let register = MemorySnapshotSupersessionRegister::new();
    let clock = FixedClock { now: 1_500 };
    register
        .register_current("snap-2026-11-14-a")
        .expect("the old snapshot is registered");
    register
        .register_current("snap-2026-11-14-b")
        .expect("the new snapshot is registered");
    // The host supersedes: a→b (the restart-surviving checkpoint record).
    register
        .supersede("snap-2026-11-14-a", "snap-2026-11-14-b", "host", &clock)
        .expect("supersedes");
    let source = CheckpointedSnapshotSource::new(register);
    let harness = ValidationHarness::new(&gate, &source, &clock);
    let refusal = harness
        .validate(&reference_proposal())
        .expect_err("a proposal against a superseded snapshot is rejected");
    assert_eq!(refusal.name(), "SNAPSHOT_SUPERSEDED");
    assert_eq!(
        refusal,
        flexcore_research::validation::Refusal::SnapshotSuperseded {
            by_snapshot_id: "snap-2026-11-14-b".to_string()
        }
    );
    assert_eq!(gate.calls.get(), 0, "a stale proposal never consumes a feasibility attempt");
}

#[test]
fn hostile_proposal_3_missing_window_is_refused_never_defaulted() {
    // The wire delivered no window fields: the extraction point refuses
    // with the absent field named — a defaulted window is never built,
    // so no proposal with a missing window can even be constructed.
    let err = window_from_wire_parts(None, None).expect_err("missing window refused");
    assert_eq!(err.name(), "PROPOSAL_WINDOW_MISSING");
    assert_eq!(err, flexcore_research::proposal_windows::WindowError::WindowMissing {
        field: "valid_from_ms"
    });
    // One field present, one absent: still missing, the absent one named.
    let err = window_from_wire_parts(Some(1_000), None).expect_err("missing valid_until refused");
    assert_eq!(err.name(), "PROPOSAL_WINDOW_MISSING");
    // The grantor path cannot produce an unbounded window either (the
    // ledger's "window set from the proposal's own clock with no bound"
    // risk): the duration is capped at the recorded default.
    let clock = FixedClock { now: 1_000 };
    let err = grant_validity_window(&clock, MAX_VALIDITY_WINDOW_MS + 1)
        .expect_err("an over-cap window is refused");
    assert_eq!(err.name(), "PROPOSAL_WINDOW_EXCEEDS_CAP");
}

#[test]
fn a_fresh_window_from_the_grantor_flows_through_the_full_harness() {
    // The happy path: the HOST grants the window from the injected clock
    // ([1_000, 1_500) for duration 500), the proposal carries it, and the
    // full harness accepts it with the gate LAST.
    let gate = GateSpy::new();
    let register = MemorySnapshotSupersessionRegister::new();
    register
        .register_current("snap-2026-11-14-a")
        .expect("the snapshot is registered");
    let source = CheckpointedSnapshotSource::new(register);
    let clock = FixedClock { now: 1_000 };
    let window = grant_validity_window(&clock, 500).expect("grants");
    assert_eq!(window.valid_from_ms, 1_000);
    assert_eq!(window.valid_until_ms, 1_500);
    let proposal = ResearchProposal::new(
        "prop-it-2",
        "plugin/research-admm",
        "0.1.0",
        1,
        ResearchResultKind::ProposedSchedule,
        serde_json::json!({ "setpoints_mw": [1.5, 2.5] }),
        ResearchProvenance {
            worker_version: "research-worker/0.1.0".to_string(),
            model_version: "grid-model/2026.11".to_string(),
            solver_version: "highspy 1.15.1".to_string(),
            input_snapshot_id: "snap-2026-11-14-a".to_string(),
            termination_status: "converged".to_string(),
            diagnostics: Vec::new(),
            seed: Some(7),
            seed_supported: true,
            deterministic: true,
            model_artifact_ref: "artifact://models/admm.v1".to_string(),
        },
        window,
    )
    .expect("shape-valid");
    let harness = ValidationHarness::new(&gate, &source, &clock);
    let report = harness.validate(&proposal).expect("full pass");
    assert!(report.is_validated());
    assert_eq!(gate.calls.get(), 1, "exactly one gate attempt on a full pass");
    // And the consumption-point check passes for the SAME window.
    gate_at_time_of_use(&proposal, &clock, &source).expect("fresh at the point of use");
}

// ---------------------------------------------------------------------------
// P8.31 — the cross-tenant negative suite (the ledger's verification
// method: an independent re-run of the cross-tenant suite)
// ---------------------------------------------------------------------------

fn catalog() -> ScopedResearchCatalog {
    let mut catalog = ScopedResearchCatalog::new();
    catalog
        .register(ResearchDataRecord {
            kind: ResearchDataKind::Telemetry,
            tenant_id: "tenant-a".to_string(),
            site_id: Some("s1".to_string()),
            record_id: "ckt-1".to_string(),
            descriptor: "tenant-a telemetry".to_string(),
        })
        .expect("registers");
    catalog
        .register(ResearchDataRecord {
            kind: ResearchDataKind::TrainingData,
            tenant_id: "tenant-a".to_string(),
            site_id: None,
            record_id: "ds-1".to_string(),
            descriptor: "tenant-a training data".to_string(),
        })
        .expect("registers");
    catalog
        .register(ResearchDataRecord {
            kind: ResearchDataKind::Topology,
            tenant_id: "tenant-b".to_string(),
            site_id: None,
            record_id: "topo-9".to_string(),
            descriptor: "tenant-b topology".to_string(),
        })
        .expect("registers");
    catalog
}

#[test]
fn cross_tenant_reads_are_denied_and_audited_for_every_data_kind() {
    let catalog = catalog();
    // A worker stamped for tenant-a (site s1): telemetry and topology reads
    // against tenant-b are denied; every denial is audited.
    let worker_a = WorkerScopeStamp::stamp_by_host(
        "worker/admm-it-1",
        ResearchWorkerFamily::AdmmDmpc,
        ResearchWorkerScope::from_authenticated_context("tenant-a", Some("s1"))
            .expect("valid scope"),
    )
    .expect("stamps");
    let audit = MemoryTenantIsolationAuditSink::new();
    let clock = FixedClock { now: 10_000 };

    // Topology of tenant-b (record exists, key resolves): DENIED with
    // TENANT_SCOPE_MISMATCH and audited.
    let err = catalog
        .scoped_read_audited(
            &worker_a,
            ResearchDataKind::Topology,
            "tenant-b",
            None,
            "topo-9",
            &clock,
            &audit,
        )
        .expect_err("cross-tenant topology denied");
    assert_eq!(err.name(), "TENANT_SCOPE_MISMATCH");
    // A cross-tenant read of a record that does NOT exist at that kind
    // (telemetry/topo-9 is not a registered key) fails closed with the
    // unknown-record refusal — unknown is never in-scope, also audited.
    let err = catalog
        .scoped_read_audited(
            &worker_a,
            ResearchDataKind::Telemetry,
            "tenant-b",
            None,
            "topo-9",
            &clock,
            &audit,
        )
        .expect_err("unknown cross-tenant record denied");
    assert_eq!(err.name(), "RESEARCH_DATA_RECORD_UNKNOWN");
    // Training data of tenant-a read by a worker NARROWED to s1 while the
    // dataset is tenant-wide: denied (a narrowed stamp does not cover a
    // tenant-wide record).
    let err = catalog
        .scoped_read_audited(
            &worker_a,
            ResearchDataKind::TrainingData,
            "tenant-a",
            None,
            "ds-1",
            &clock,
            &audit,
        )
        .expect_err("narrowed stamp does not cover tenant-wide data");
    assert_eq!(err.name(), "TENANT_SCOPE_MISMATCH");

    // The audit trail: three denials, every one audited (§16.2) with its
    // named refusal class in the detail.
    let records = audit.records();
    assert_eq!(records.len(), 3);
    assert!(records
        .iter()
        .all(|r| r.kind == TenantIsolationAuditKind::ScopeReadRefused));
    assert_eq!(
        records
            .iter()
            .filter(|r| r.detail.contains("TENANT_SCOPE_MISMATCH"))
            .count(),
        2
    );
    assert_eq!(
        records
            .iter()
            .filter(|r| r.detail.contains("RESEARCH_DATA_RECORD_UNKNOWN"))
            .count(),
        1
    );
}

#[test]
fn in_scope_reads_are_admitted_and_audited() {
    let catalog = catalog();
    let worker_a = WorkerScopeStamp::stamp_by_host(
        "worker/gnn-it-1",
        ResearchWorkerFamily::Gnn,
        ResearchWorkerScope::from_authenticated_context("tenant-a", None)
            .expect("valid scope"),
    )
    .expect("stamps");
    let audit = MemoryTenantIsolationAuditSink::new();
    let clock = FixedClock { now: 11_000 };
    // Tenant-wide stamp: tenant-wide training data admitted; the site
    // telemetry admitted (tenant-wide covers any site in the tenant).
    catalog
        .scoped_read_audited(
            &worker_a,
            ResearchDataKind::TrainingData,
            "tenant-a",
            None,
            "ds-1",
            &clock,
            &audit,
        )
        .expect("admitted");
    catalog
        .scoped_read_audited(
            &worker_a,
            ResearchDataKind::Telemetry,
            "tenant-a",
            Some("s1"),
            "ckt-1",
            &clock,
            &audit,
        )
        .expect("admitted");
    let records = audit.records();
    assert_eq!(records.len(), 2);
    assert!(records
        .iter()
        .all(|r| r.kind == TenantIsolationAuditKind::ScopeReadAdmitted));
}

#[test]
fn cross_tenant_model_transfer_is_refused_without_a_recorded_approval() {
    let mut register = flexcore_research::tenant_isolation::ModelServingRegister::new();
    register
        .register_artifact(ModelArtifactRegistration {
            artifact_ref: "artifact://models/gnn-warmstart.v1".to_string(),
            owner_family: ResearchWorkerFamily::Gnn,
            owner_scope: ResearchWorkerScope::from_authenticated_context("tenant-a", None)
                .expect("valid scope"),
            training_data_ref: "training-data://tenant-a/ds-1".to_string(),
        })
        .expect("registers");
    let worker_a = WorkerScopeStamp::stamp_by_host(
        "worker/gnn-it-2",
        ResearchWorkerFamily::Gnn,
        ResearchWorkerScope::from_authenticated_context("tenant-a", None)
            .expect("valid scope"),
    )
    .expect("stamps");
    let audit = MemoryTenantIsolationAuditSink::new();
    let clock = FixedClock { now: 12_000 };

    // Tenant-b asks for the model trained on tenant-a's data: REFUSED.
    let serving_b = ResearchWorkerScope::from_authenticated_context("tenant-b", None)
        .expect("valid scope");
    let err = register
        .model_serving_audited(&worker_a, "artifact://models/gnn-warmstart.v1", &serving_b, &clock, &audit)
        .expect_err("unapproved cross-tenant serving refused");
    assert_eq!(err.name(), "RESEARCH_MODEL_SERVING_NOT_APPROVED");

    // The host records the approval; the same request is then ADMITTED.
    register
        .record_approval(
            ModelServingApproval {
                artifact_ref: "artifact://models/gnn-warmstart.v1".to_string(),
                serving_scope: serving_b.clone(),
                reason: "decision/2026-11-14 cross-tenant benchmark exchange".to_string(),
                actor: "product".to_string(),
                approved_at_millis: 0,
            },
            &clock,
            &audit,
        )
        .expect("the approval is recorded");
    register
        .model_serving_audited(&worker_a, "artifact://models/gnn-warmstart.v1", &serving_b, &clock, &audit)
        .expect("recorded approval admits the serving");

    // The audit trail: refusal, approval record, admission — in order.
    let records = audit.records();
    assert_eq!(records.len(), 3);
    assert_eq!(records[0].kind, TenantIsolationAuditKind::ModelServingRefused);
    assert_eq!(records[1].kind, TenantIsolationAuditKind::ServingApprovalRecorded);
    assert_eq!(records[2].kind, TenantIsolationAuditKind::ModelServingAdmitted);
}

#[test]
fn the_worker_stamp_cannot_be_forged_or_widened() {
    // Echo mismatch: a worker stamped for tenant-a/s1 echoing tenant-wide
    // (or tenant-b) is refused.
    let stamp = WorkerScopeStamp::stamp_by_host(
        "worker/marl-it-1",
        ResearchWorkerFamily::Marl,
        ResearchWorkerScope::from_authenticated_context("tenant-a", Some("s1"))
            .expect("valid scope"),
    )
    .expect("stamps");
    stamp
        .verify_worker_echo(
            &ResearchWorkerScope::from_authenticated_context("tenant-a", Some("s1"))
                .expect("valid scope"),
        )
        .expect("the exact echo passes");
    let err = stamp
        .verify_worker_echo(
            &ResearchWorkerScope::from_authenticated_context("tenant-a", None)
                .expect("valid scope"),
        )
        .expect_err("a widened echo is refused");
    assert_eq!(err.name(), "TENANT_SCOPE_ECHO_MISMATCH");
    let err = stamp
        .verify_worker_echo(
            &ResearchWorkerScope::from_authenticated_context("tenant-b", None)
                .expect("valid scope"),
        )
        .expect_err("a cross-tenant echo is refused");
    assert_eq!(err.name(), "TENANT_SCOPE_ECHO_MISMATCH");
}
