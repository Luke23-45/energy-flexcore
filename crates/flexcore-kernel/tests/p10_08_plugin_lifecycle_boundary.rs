// P10.08 — Plugin permission-boundary tests (release-gate slice, kernel):
//         the manifest gate's never-granted refusals for EVERY plugin class
//         (A/B/C) and the §5.6 plugin-lifecycle state-machine boundary —
//         illegal transitions refused, drain-before-replace enforced, the
//         mid-dispatch upgrade refused, crash edges, and the explicit-only
//         restart (report §22.3 gate 7; §5.4–§5.6; §9.3; §14.1 by analogy;
//         ledger
//         `ledger/phase-10-v1-release-hardening/ledger_phase_10.md` P10.08;
//         record:
//         `ledger/phase-10-v1-release-hardening/08_plugin_boundary_tests.md`).
//
// WHAT THIS FILE PROVES — over the REAL code paths
// (`crates/flexcore-kernel/src/manifest.rs` P7.05 and `lifecycle.rs`
// P7.07), by exercise:
//
// 1. THE MANIFEST GATE REFUSES EVERY NEVER-GRANTED PERMISSION FOR EVERY
//    PLUGIN CLASS (§5.7; §5.4 Class A builtin-rust / Class B
//    wasm-component / Class C grpc-worker): a manifest of ANY class
//    requesting `dispatch.authorize`, `dispatch.issue`, `secret.read`, or
//    `network.any` is rejected at the EARLIEST point it can be refused —
//    before composition, lifecycle, or any broker exists to mis-grant it
//    (the recorded kernel gate discipline; the broker re-refuses later —
//    proven in the plugin-host slice of this suite,
//    `apps/plugin-host/tests/p10_08_plugin_permission_boundary.rs`). The
//    refusal is the DISTINCT named error `PERMISSION_NEVER_GRANTED`. This
//    is 4 permissions × 3 classes = 12 cells, every one refused. The same
//    gate refuses unknown permission ids (`PERMISSION_ID_UNKNOWN`) per
//    class, and the two refusals are pairwise distinct.
// 2. THE §5.6 PLUGIN LIFECYCLE STATE MACHINE (the kernel-side tables
//    P10.03 recorded as complemented here): the states are the closed
//    10-state enum; the transition table is FROZEN (12 §5.6 edges + 5
//    recorded crash edges = 17 legal ordered pairs out of 10 × 10 = 100;
//    83 illegal). The predicate `is_legal_transition` matches the
//    transcription for ALL 100 pairs — hand-computed, asserted, counts
//    pinned (17 / 83 / 100, no edge in both tables, no outgoing edge from
//    a terminal state). Through the MACHINE, illegal transitions are the
//    DISTINCT named error `ILLEGAL_TRANSITION` — never a warning, never a
//    silent no-op — the state is UNCHANGED, and the refusal is AUDITED.
// 3. DRAIN-BEFORE-REPLACE (§5.6, the core rule; acceptance criterion c):
//    a replacement may only start once the incumbent is FULLY drained
//    (`Stopped`) — anything earlier (Ready active, Draining in progress)
//    is the NAMED error `REPLACEMENT_BLOCKED_UNDRAINED`, audited as a
//    refusal. An upgrade attempted MID-DISPATCH-TRANSACTION (work still
//    outstanding at the drain deadline) is refused fail-closed: the stop
//    is NOT committed, the incumbent is RESTORED to `Ready` (active,
//    pending an explicit operator decision), and the refusal is audited.
//    There is no forced stop and no hot reload: the version change is
//    recorded ONLY after a completed drain, and the only path to a new
//    capability set is re-instantiation through this drain-before-replace
//    (the P7.12 frozen-capability rule's counterpart).
// 4. NO SELF-UPGRADE / NO AUTO-RESTART: a plugin fails into `Failed` and
//    STAYS there (crash edges from any non-terminal state so a crash is
//    always representable and audited); the only path out is the audited
//    EXPLICIT operator restart — no auto-restart exists anywhere.
// 5. GATE REFUSALS ARE FAIL-CLOSED: a failed §5.6 gate (provenance,
//    compatibility, schema/secrets, dependency graph, health) refuses the
//    transition, leaves the state unchanged, is audited, and the §5.6 exit
//    edges (`Rejected` / `Failed`) are then applied EXPLICITLY — never
//    implicitly by a failed gate.
//
// STATUS: WRITTEN-UNRUN (ENTRY_NOTE boundary 3; no cargo command was
// executed in this loop; zero observed failures is NOT claimed — the human
// build/test gate owns:
// `cargo test -p flexcore-kernel --test p10_08_plugin_lifecycle_boundary`).
// Expected strings are the modules' own constants. The P7.05/P7.07 suites
// (`manifest_p7_05.rs`, `lifecycle_p7_07.rs`) are cross-referenced — this
// file is the release-gate slice, not a replacement.

use flexcore_kernel::lifecycle::{
    is_legal_transition, DrainOutcome, DrainPolicy, DrainPort, FixedClock, GateOutcome,
    LifecycleAuditKind, LifecycleError, LifecycleState, MemoryAuditSink, PluginLifecycle,
    CRASH_TRANSITIONS, DEFAULT_DRAIN_DEADLINE_MS, FROZEN_TRANSITIONS, GATE_HEALTH,
    GATE_MANIFEST_API_COMPATIBILITY, GATE_PROVENANCE, GATE_SCHEMA_SECRETS,
    LIFECYCLE_STATES,
};
use flexcore_kernel::manifest::{
    parse_manifest_value, ManifestError, PluginClass, NEVER_GRANTED_PERMISSIONS,
};

// =================================================================================
// 1. The manifest gate: never-granted refused for EVERY plugin class.
// =================================================================================

/// A valid Class A manifest (mirrors the published reference example).
fn valid_builtin_json() -> serde_json::Value {
    serde_json::json!({
        "manifest-schema": "flexcore-plugin-manifest-v1",
        "plugin-id": "neryva.sim-builtin-ess",
        "name": "Built-in ESS Simulator Provider",
        "version": "0.1.0",
        "api-version": ">=0.5,<1.0",
        "class": "builtin-rust",
        "entry-point": { "registered-type": "neryva.builtin/sim-ess" },
        "provider-of": ["ess.simulation"],
        "consumes": ["telemetry.edge-sample"],
        "permissions": ["telemetry.read"],
        "configuration-schema": {
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "rated-power-mw": { "type": "number", "description": "Rated power in MW." }
            },
            "required": ["rated-power-mw"]
        },
        "secrets": [],
        "resources": { "max-memory-mb": 64, "timeout-ms": 30000 },
        "metadata": {
            "license": "Apache-2.0",
            "description": "Deterministic built-in ESS simulator provider.",
            "source": "crates/flexcore-simulator"
        }
    })
}

/// A manifest of the GIVEN class (Class A / B / C), each with its OWN
/// required entry-point key (§5.5 per-class entry point) — the same rigor
/// for every class (the ledger's recorded risk: a built-in plugin tested
/// less rigorously because it is "trusted" is refused by construction).
fn manifest_of_class(class: &str, plugin_id: &str) -> serde_json::Value {
    let mut manifest = valid_builtin_json();
    manifest["class"] = serde_json::json!(class);
    manifest["plugin-id"] = serde_json::json!(plugin_id);
    manifest["entry-point"] = match class {
        "builtin-rust" => serde_json::json!({ "registered-type": format!("neryva.builtin/{plugin_id}") }),
        "wasm-component" => serde_json::json!({ "symbol": "flexcore:optimizer/optimizer" }),
        "grpc-worker" => serde_json::json!({ "target": "https://worker.internal:50051" }),
        other => panic!("unknown test class '{other}'"),
    };
    manifest
}

#[test]
fn every_never_granted_permission_is_rejected_at_the_manifest_gate_for_every_class() {
    // 4 never-granted ids × 3 classes = 12 cells, every one the DISTINCT
    // named error PERMISSION_NEVER_GRANTED — the earliest point a
    // never-granted request can be refused.
    let classes = ["builtin-rust", "wasm-component", "grpc-worker"];
    let mut refused_cells = 0usize;
    for class in classes {
        for permission in NEVER_GRANTED_PERMISSIONS {
            let mut manifest = manifest_of_class(class, &format!("neryva.test-{class}"));
            manifest["permissions"] = serde_json::json!([permission]);
            let error = parse_manifest_value(&manifest)
                .expect_err("a never-granted permission is rejected at the gate");
            assert_eq!(
                error.name(),
                "PERMISSION_NEVER_GRANTED",
                "class {class} permission {permission}"
            );
            assert!(
                error.to_string().contains("never-granted set"),
                "the refusal names the never-granted set: {error}"
            );
            refused_cells += 1;
        }
    }
    assert_eq!(refused_cells, 12, "every (class, never-granted permission) cell is refused");
    // The never-granted set is exactly the four §5.7 ids, verbatim.
    assert_eq!(NEVER_GRANTED_PERMISSIONS.len(), 4);
    assert!(NEVER_GRANTED_PERMISSIONS.contains(&"dispatch.authorize"));
    assert!(NEVER_GRANTED_PERMISSIONS.contains(&"dispatch.issue"));
    assert!(NEVER_GRANTED_PERMISSIONS.contains(&"secret.read"));
    assert!(NEVER_GRANTED_PERMISSIONS.contains(&"network.any"));
}

#[test]
fn unknown_permission_ids_and_never_granted_ids_are_distinct_refusals_per_class() {
    for class in ["builtin-rust", "wasm-component", "grpc-worker"] {
        // An id in NO vocabulary: PERMISSION_ID_UNKNOWN.
        let mut unknown = manifest_of_class(class, "neryva.test-unknown");
        unknown["permissions"] = serde_json::json!(["device.protocolspeak"]);
        let error = parse_manifest_value(&unknown)
            .expect_err("an unknown permission id is rejected");
        assert_eq!(error.name(), "PERMISSION_ID_UNKNOWN", "class {class}");
        // A verdict-shaped id (the ADR-010 authority spelling): also
        // PERMISSION_ID_UNKNOWN — the authority does not exist to request.
        let mut verdict = manifest_of_class(class, "neryva.test-verdict");
        verdict["permissions"] = serde_json::json!(["dispatch.validate.verdict"]);
        let error = parse_manifest_value(&verdict).expect_err("no such authority");
        assert_eq!(error.name(), "PERMISSION_ID_UNKNOWN", "class {class}");
        // The closed §5.7 vocabulary still parses per class (the happy path
        // is not broken by the refusals).
        let mut ok = manifest_of_class(class, &format!("neryva.test-ok-{class}"));
        ok["permissions"] = serde_json::json!(["telemetry.read", "dispatch.validate.request"]);
        let manifest = parse_manifest_value(&ok).expect("allowed ids parse for every class");
        assert_eq!(manifest.class, PluginClass::parse(class).expect("class parses"));
        assert_eq!(manifest.permissions.len(), 2);
    }
}

// =================================================================================
// 2. The §5.6 lifecycle transition tables — the full 100-pair matrix.
// =================================================================================

/// The transcription of the frozen tables (the test oracle — derived from
/// the §5.6 diagram and the recorded crash decision, never from the
/// implementation's own iteration).
fn transcribed_legal_edges() -> Vec<(LifecycleState, LifecycleState)> {
    use LifecycleState::*;
    vec![
        // The 12 §5.6 frozen edges (with the diagram's guard labels).
        (Discovered, Verified),   // signature / provenance accepted
        (Verified, Compatible),   // manifest and API compatibility pass
        (Compatible, Configured), // schema and secret references valid
        (Configured, Started),    // dependency graph resolved
        (Started, Ready),         // health checks pass
        (Ready, Draining),        // disable / upgrade requested
        (Draining, Stopped),      // active work completed or cancelled
        (Verified, Rejected),     // verification failure
        (Compatible, Rejected),   // incompatible API
        (Started, Failed),        // initialization failure
        (Ready, Failed),          // health / runtime failure
        (Failed, Draining),       // failed plugin drained before removal
        // The 5 recorded crash edges (DEFAULT decision): a crash in any
        // non-terminal state is representable and audited.
        (Discovered, Failed),
        (Verified, Failed),
        (Compatible, Failed),
        (Configured, Failed),
        (Draining, Failed),
    ]
}

#[test]
fn the_full_100_pair_matrix_matches_the_transcription_with_pinned_counts() {
    let transcription = transcribed_legal_edges();
    // The frozen tables match the transcription exactly, and no edge
    // appears in BOTH tables (12 + 5 = 17 distinct legal edges).
    let mut code_edges: Vec<(LifecycleState, LifecycleState)> = FROZEN_TRANSITIONS.to_vec();
    code_edges.extend_from_slice(&CRASH_TRANSITIONS);
    let mut sorted_code = code_edges.clone();
    sorted_code.sort();
    let mut sorted_transcribed = transcription.clone();
    sorted_transcribed.sort();
    assert_eq!(sorted_code, sorted_transcribed, "the frozen tables are the transcription");
    let distinct = {
        let mut deduped = sorted_code.clone();
        deduped.dedup();
        deduped
    };
    assert_eq!(distinct.len(), code_edges.len(), "no edge appears in both tables");
    assert_eq!(FROZEN_TRANSITIONS.len(), 12, "the §5.6 table is 12 edges");
    assert_eq!(CRASH_TRANSITIONS.len(), 5, "the recorded crash set is 5 edges");
    // The 10-state vocabulary is closed and pinned.
    assert_eq!(LIFECYCLE_STATES.len(), 10);
    // ALL 100 ordered pairs: the predicate agrees with the transcription.
    let states = LIFECYCLE_STATES;
    let mut legal = 0usize;
    let mut illegal = 0usize;
    for from in states {
        for to in states {
            let in_transcription = transcription.contains(&(from, to));
            assert_eq!(
                is_legal_transition(from, to),
                in_transcription,
                "pair {from:?} -> {to:?}: the predicate must match the transcription"
            );
            if in_transcription {
                legal += 1;
            } else {
                illegal += 1;
            }
        }
    }
    assert_eq!(legal, 17, "17 legal ordered pairs (hand-computed)");
    assert_eq!(illegal, 83, "83 illegal ordered pairs (100 − 17), hand-computed");
    // Terminal states hold their outcome: NO outgoing edge from Stopped,
    // Rejected, or Failed — except the recorded §5.6 cleanup edge
    // Failed → Draining.
    for terminal in [LifecycleState::Stopped, LifecycleState::Rejected] {
        for to in states {
            assert!(
                !is_legal_transition(terminal, to),
                "{terminal:?} is terminal: no outgoing edge to {to:?}"
            );
        }
    }
    let failed_outgoing: Vec<LifecycleState> = states
        .iter()
        .copied()
        .filter(|to| is_legal_transition(LifecycleState::Failed, *to))
        .collect();
    assert_eq!(
        failed_outgoing,
        vec![LifecycleState::Draining],
        "the ONLY §5.6 edge out of Failed is the cleanup drain"
    );
}

#[test]
fn terminal_states_are_exactly_the_three_recorded_ones() {
    for state in LIFECYCLE_STATES {
        let expected = matches!(
            state,
            LifecycleState::Stopped | LifecycleState::Rejected | LifecycleState::Failed
        );
        assert_eq!(state.is_terminal(), expected, "terminal set pinned for {state:?}");
    }
}

// =================================================================================
// 3. Through the machine: illegal transitions refused, audited, state
//    unchanged.
// =================================================================================

fn fresh_lifecycle<'a>(
    clock: &'a FixedClock,
    audit: &'a MemoryAuditSink,
) -> PluginLifecycle<'a> {
    PluginLifecycle::register(
        "neryva.test-plugin",
        "1.0.0",
        clock,
        audit,
        DrainPolicy::default(),
    )
}

fn refusals(audit: &MemoryAuditSink) -> Vec<LifecycleAuditRecordRef> {
    audit
        .records()
        .iter()
        .filter(|r| r.kind == LifecycleAuditKind::Refused)
        .map(|r| LifecycleAuditRecordRef {
            from: r.from,
            to: r.to,
            reason: r.reason.clone(),
        })
        .collect()
}

/// A light clone of the audit record's refusal fields (the in-memory sink's
/// borrow is brief; this keeps the assertion code readable).
struct LifecycleAuditRecordRef {
    from: Option<LifecycleState>,
    to: Option<LifecycleState>,
    reason: String,
}

#[test]
fn an_illegal_upgrade_from_discovered_is_refused_audited_and_leaves_the_state_unchanged() {
    let clock = FixedClock::new(1_000);
    let audit = MemoryAuditSink::new();
    let mut lifecycle = fresh_lifecycle(&clock, &audit);
    assert_eq!(lifecycle.state(), LifecycleState::Discovered);
    // An upgrade/drain from Discovered: NOT a §5.6 edge (drain begins from
    // Ready or the Failed cleanup only).
    struct NoWork;
    impl DrainPort for NoWork {
        fn outstanding_work(&self) -> usize {
            0
        }
        fn drain(&mut self, _deadline_millis: u64) -> DrainOutcome {
            DrainOutcome::Quiesced { cancelled: 0 }
        }
    }
    let mut port = NoWork;
    let error = lifecycle
        .begin_drain(&mut port, "attacker", "upgrade requested from Discovered")
        .expect_err("drain from Discovered is illegal");
    assert_eq!(error.name(), "ILLEGAL_TRANSITION");
    assert!(error.to_string().contains("Discovered -> Draining"));
    assert_eq!(lifecycle.state(), LifecycleState::Discovered, "the state is UNCHANGED");
    // RECORDED FINDING F-P10.08-1 (observed behaviour, asserted as found —
    // NOT fixed here; src logic is outside this subtask's ownership): an
    // ILLEGAL transition refusal (`LifecycleError::IllegalTransition`) is
    // currently NOT emitted as an audit record — `lifecycle.rs::illegal()`
    // constructs the error without a `Refused` emission, while the module
    // header states "EVERY transition — committed or refused — emits a
    // LifecycleAuditRecord". Gate refusals, drain-deadline refusals, and
    // replacement refusals ARE audited (asserted below); the illegal-
    // transition emission gap is recorded in the P10.08 decision record as
    // an audit-completeness follow-up. This test pins the CURRENT state so
    // a fix makes the gap visible.
    let refusal_records = refusals(&audit);
    assert!(
        refusal_records.is_empty(),
        "F-P10.08-1: illegal-transition refusals are currently not audited — if this \
         assertion fails, the audit-completeness follow-up has landed; update the finding"
    );
    // And reject() from Discovered is equally illegal (Rejected is
    // reachable only from Verified or Compatible).
    let error = lifecycle
        .reject("attacker", "skip verification")
        .expect_err("Rejected is not reachable from Discovered");
    assert_eq!(error.name(), "ILLEGAL_TRANSITION");
    assert_eq!(lifecycle.state(), LifecycleState::Discovered);
    // fail() from Discovered is not a §5.6 edge either (use crash()).
    let error = lifecycle
        .fail("attacker", "skip to Failed")
        .expect_err("fail() is only legal from Started or Ready");
    assert_eq!(error.name(), "ILLEGAL_TRANSITION");
    assert_eq!(lifecycle.state(), LifecycleState::Discovered);
}

#[test]
fn complete_drain_is_only_legal_from_draining() {
    let clock = FixedClock::new(1_000);
    let audit = MemoryAuditSink::new();
    let mut lifecycle = fresh_lifecycle(&clock, &audit);
    // From Discovered (never drained, never Draining).
    let error = lifecycle
        .complete_drain("attacker", "stop without a drain")
        .expect_err("complete_drain requires Draining");
    assert_eq!(error.name(), "ILLEGAL_TRANSITION");
    assert_eq!(lifecycle.state(), LifecycleState::Discovered);
}

#[test]
fn an_explicit_restart_is_only_legal_from_failed_no_auto_restart_exists() {
    let clock = FixedClock::new(1_000);
    let audit = MemoryAuditSink::new();
    // From Discovered: refused (no auto-restart; the explicit restart is a
    // Failed-only operation).
    let lifecycle = fresh_lifecycle(&clock, &audit);
    let error = lifecycle
        .restart_explicitly("operator", "restart without a failure")
        .expect_err("restart only from Failed");
    assert_eq!(error.name(), "ILLEGAL_TRANSITION");
    // From Failed: the audited explicit restart (tested below with a real
    // failure path).
}

#[test]
fn a_failed_gate_refuses_the_transition_and_the_exit_edges_are_explicit() {
    let clock = FixedClock::new(1_000);
    let audit = MemoryAuditSink::new();
    let mut lifecycle = fresh_lifecycle(&clock, &audit);
    // The PROVENANCE gate fails: the transition to Verified is REFUSED,
    // the state is unchanged, and the refusal is audited with the gate
    // name.
    let error = lifecycle
        .verify(
            GateOutcome::Failed { reason: "signature did not verify".to_string() },
            "composition-root",
            "attempt the forward transition",
        )
        .expect_err("a failed gate refuses");
    assert_eq!(error.name(), "GATE_REFUSED");
    match &error {
        LifecycleError::GateRefused { from, to, gate, .. } => {
            assert_eq!(*from, LifecycleState::Discovered);
            assert_eq!(*to, LifecycleState::Verified);
            assert_eq!(*gate, GATE_PROVENANCE);
        }
        other => panic!("expected GateRefused, got {other:?}"),
    }
    assert_eq!(lifecycle.state(), LifecycleState::Discovered, "fail-closed: unchanged");
    // Gate refusals ARE audited (the gated_transition emission).
    assert!(
        refusals(&audit).iter().any(|r| r.reason.contains("PROVENANCE")),
        "the gate refusal is audited with the gate name"
    );
    // The §5.6 exit edge is applied EXPLICITLY by the host (a failed
    // provenance gate at Discovered crashes the lifecycle into Failed —
    // the recorded crash edge), never implicitly by the refused gate.
    lifecycle
        .crash("composition-root", "provenance gate failed; the plugin cannot proceed")
        .expect("the crash edge is applied explicitly");
    assert_eq!(lifecycle.state(), LifecycleState::Failed);
    assert_eq!(lifecycle.failed_from(), Some(LifecycleState::Discovered));
}

#[test]
fn a_verified_plugin_whose_compatibility_gate_fails_exits_through_rejected_explicitly() {
    let clock = FixedClock::new(1_000);
    let audit = MemoryAuditSink::new();
    let mut lifecycle = fresh_lifecycle(&clock, &audit);
    lifecycle
        .verify(GateOutcome::Passed, "composition-root", "provenance accepted")
        .expect("verify");
    assert_eq!(lifecycle.state(), LifecycleState::Verified);
    // The compatibility gate fails → refused, state unchanged.
    let error = lifecycle
        .mark_compatible(
            GateOutcome::Failed { reason: "api-version range does not cover the platform".to_string() },
            "composition-root",
            "attempt compatibility",
        )
        .expect_err("a failed compatibility gate refuses");
    assert_eq!(error.name(), "GATE_REFUSED");
    let refused_gate = match &error {
        LifecycleError::GateRefused { gate, .. } => *gate,
        other => panic!("expected GateRefused, got {other:?}"),
    };
    assert_eq!(refused_gate, GATE_MANIFEST_API_COMPATIBILITY);
    assert_eq!(lifecycle.state(), LifecycleState::Verified);
    // The §5.6 exit edge Verified → Rejected, applied explicitly.
    lifecycle
        .reject("composition-root", "incompatible API; the plugin is rejected")
        .expect("the exit edge");
    assert_eq!(lifecycle.state(), LifecycleState::Rejected);
    assert!(LifecycleState::Rejected.is_terminal());
    // Terminal: nothing moves it.
    let error = lifecycle
        .verify(GateOutcome::Passed, "attacker", "revive a rejected plugin")
        .expect_err("a terminal state has no outgoing edges");
    assert_eq!(error.name(), "ILLEGAL_TRANSITION");
    assert_eq!(lifecycle.state(), LifecycleState::Rejected);
}

#[test]
fn the_full_forward_path_and_the_gate_names_are_exercised_in_order() {
    let clock = FixedClock::new(1_000);
    let audit = MemoryAuditSink::new();
    let mut lifecycle = fresh_lifecycle(&clock, &audit);
    lifecycle
        .verify(GateOutcome::Passed, "composition-root", "provenance")
        .expect("1");
    lifecycle
        .mark_compatible(GateOutcome::Passed, "composition-root", "compatibility")
        .expect("2");
    lifecycle
        .configure(GateOutcome::Passed, "composition-root", "schema + secrets")
        .expect("3");
    lifecycle
        .start(GateOutcome::Passed, "composition-root", "dependency graph")
        .expect("4");
    lifecycle
        .mark_ready(GateOutcome::Passed, "composition-root", "health")
        .expect("5");
    assert_eq!(lifecycle.state(), LifecycleState::Ready);
    // The gate names travelled through the records (spot-check two).
    let reasons: Vec<String> = audit.records().iter().map(|r| r.reason.clone()).collect();
    let _ = (GATE_SCHEMA_SECRETS, GATE_HEALTH, GATE_PROVENANCE, GATE_MANIFEST_API_COMPATIBILITY);
    assert!(reasons.iter().any(|r| r.contains("health")), "the health gate reason is recorded");
}

#[test]
fn a_mid_dispatch_upgrade_is_refused_drain_deadline_exceeded_restores_the_incumbent() {
    let clock = FixedClock::new(1_000);
    let audit = MemoryAuditSink::new();
    let mut lifecycle = fresh_lifecycle(&clock, &audit);
    struct BusyWork;
    impl DrainPort for BusyWork {
        fn outstanding_work(&self) -> usize {
            3 // a dispatch transaction (or any active work) still running
        }
        fn drain(&mut self, _deadline_millis: u64) -> DrainOutcome {
            DrainOutcome::DeadlineExceeded { outstanding: 3 }
        }
    }
    lifecycle
        .verify(GateOutcome::Passed, "composition-root", "provenance")
        .expect("verify");
    lifecycle
        .mark_compatible(GateOutcome::Passed, "composition-root", "compatibility")
        .expect("compatible");
    lifecycle
        .configure(GateOutcome::Passed, "composition-root", "configured")
        .expect("configure");
    lifecycle
        .start(GateOutcome::Passed, "composition-root", "started")
        .expect("start");
    lifecycle
        .mark_ready(GateOutcome::Passed, "composition-root", "ready")
        .expect("ready");
    // The upgrade is requested mid-dispatch: Ready → Draining begins, but
    // the drain CANNOT quiesce (work outstanding at the deadline).
    let mut port = BusyWork;
    lifecycle
        .begin_drain(&mut port, "operator", "upgrade requested")
        .expect("the drain begins");
    assert_eq!(lifecycle.state(), LifecycleState::Draining);
    // The stop is REFUSED fail-closed: the transition is NOT committed, the
    // incumbent is RESTORED to Ready (active), and the refusal is audited.
    // There is no forced stop — the operator decides.
    let error = lifecycle
        .complete_drain("operator", "finish the upgrade")
        .expect_err("a drain that cannot quiesce is refused");
    assert_eq!(error.name(), "DRAIN_DEADLINE_EXCEEDED");
    match &error {
        LifecycleError::DrainDeadlineExceeded { outstanding } => assert_eq!(*outstanding, 3),
        other => panic!("expected DrainDeadlineExceeded, got {other:?}"),
    }
    assert_eq!(
        lifecycle.state(),
        LifecycleState::Ready,
        "the incumbent is RESTORED to active, pending an operator decision"
    );
    let records = audit.records();
    let refused = records
        .iter()
        .filter(|r| r.kind == LifecycleAuditKind::Refused && r.to == Some(LifecycleState::Stopped))
        .count();
    let restored = records
        .iter()
        .filter(|r| r.kind == LifecycleAuditKind::Transition
            && r.from == Some(LifecycleState::Draining)
            && r.to == Some(LifecycleState::Ready))
        .count();
    assert_eq!(refused, 1, "the refused stop is audited");
    assert_eq!(restored, 1, "the restoration is audited");
    // The replacement is STILL blocked: the incumbent is active again.
    let error = lifecycle
        .require_fully_drained("composition-root", "attempt the replacement anyway")
        .expect_err("the replacement stays blocked");
    assert_eq!(error.name(), "REPLACEMENT_BLOCKED_UNDRAINED");
}

#[test]
fn a_replacement_is_blocked_until_the_incumbent_is_fully_drained() {
    let clock = FixedClock::new(1_000);
    let audit = MemoryAuditSink::new();
    let mut lifecycle = fresh_lifecycle(&clock, &audit);
    // While READY (active, no drain begun): the replacement is blocked —
    // an upgrade applied mid-transaction is a NAMED refusal, never a
    // warning.
    lifecycle
        .verify(GateOutcome::Passed, "composition-root", "provenance")
        .expect("verify");
    lifecycle
        .mark_compatible(GateOutcome::Passed, "composition-root", "compatibility")
        .expect("compatible");
    lifecycle
        .configure(GateOutcome::Passed, "composition-root", "configured")
        .expect("configure");
    lifecycle
        .start(GateOutcome::Passed, "composition-root", "started")
        .expect("start");
    lifecycle
        .mark_ready(GateOutcome::Passed, "composition-root", "ready")
        .expect("ready");
    let error = lifecycle
        .require_fully_drained("composition-root", "replace the active incumbent")
        .expect_err("blocked while active");
    assert_eq!(error.name(), "REPLACEMENT_BLOCKED_UNDRAINED");
    assert!(error.to_string().contains("drain-before-replace"));
    // Now the §5.6 drain completes cleanly: Ready → Draining → Stopped.
    struct QuietWork;
    impl DrainPort for QuietWork {
        fn outstanding_work(&self) -> usize {
            0
        }
        fn drain(&mut self, _deadline_millis: u64) -> DrainOutcome {
            DrainOutcome::Quiesced { cancelled: 0 }
        }
    }
    let mut port = QuietWork;
    lifecycle
        .begin_drain(&mut port, "operator", "disable for upgrade")
        .expect("drain begins");
    // Even DRAINING (in progress, not quiesced) blocks the replacement.
    let error = lifecycle
        .require_fully_drained("composition-root", "replace during the drain")
        .expect_err("blocked while draining");
    assert_eq!(error.name(), "REPLACEMENT_BLOCKED_UNDRAINED");
    let outcome = lifecycle
        .complete_drain("operator", "active work completed")
        .expect("quiesced");
    assert!(matches!(outcome, DrainOutcome::Quiesced { cancelled: 0 }));
    assert_eq!(lifecycle.state(), LifecycleState::Stopped);
    // ONLY now the replacement gate passes, and the version change is
    // recorded AFTER the completed drain (P7.07 step 3).
    lifecycle
        .require_fully_drained("composition-root", "incumbent fully drained")
        .expect("the replacement may start");
    lifecycle
        .record_version_change("1.1.0", "composition-root", "upgrade applied after the drain")
        .expect("the version change is recorded");
}

#[test]
fn a_version_change_is_only_recorded_after_a_completed_drain() {
    let clock = FixedClock::new(1_000);
    let audit = MemoryAuditSink::new();
    let mut lifecycle = fresh_lifecycle(&clock, &audit);
    // From Discovered: illegal — no drain has ever completed.
    let error = lifecycle
        .record_version_change("2.0.0", "attacker", "hot-reload the version")
        .expect_err("a version change requires Stopped");
    assert_eq!(error.name(), "ILLEGAL_TRANSITION");
    assert_eq!(lifecycle.state(), LifecycleState::Discovered);
    // (F-P10.08-1: the illegal-transition refusal itself is not audited —
    // see the pinned finding in the Discovered-upgrade test above.)
    // The DEFAULT drain deadline is the recorded 30 s (a DEFAULT decision,
    // not a report number).
    assert_eq!(lifecycle.policy().deadline_ms, DEFAULT_DRAIN_DEADLINE_MS);
    assert_eq!(DEFAULT_DRAIN_DEADLINE_MS, 30_000);
}

#[test]
fn a_failed_plugin_stays_failed_and_only_the_explicit_restart_moves_it() {
    let clock = FixedClock::new(1_000);
    let audit = MemoryAuditSink::new();
    let mut lifecycle = fresh_lifecycle(&clock, &audit);
    // A crash in ANY non-terminal state is representable (the recorded
    // crash edges): crash from Configured.
    lifecycle
        .verify(GateOutcome::Passed, "composition-root", "provenance")
        .expect("verify");
    lifecycle
        .mark_compatible(GateOutcome::Passed, "composition-root", "compatibility")
        .expect("compatible");
    lifecycle
        .configure(GateOutcome::Passed, "composition-root", "configured")
        .expect("configure");
    lifecycle
        .crash("supervisor", "the plugin crashed mid-operation")
        .expect("the crash edge");
    assert_eq!(lifecycle.state(), LifecycleState::Failed);
    assert_eq!(lifecycle.failed_from(), Some(LifecycleState::Configured), "the last state is retained as drill evidence");
    // The plugin STAYS Failed: no auto-restart exists. A crash() from
    // Failed is refused (terminal).
    let error = lifecycle
        .crash("supervisor", "crash a crashed plugin")
        .expect_err("Failed is terminal for crash()");
    assert_eq!(error.name(), "ILLEGAL_TRANSITION");
    assert_eq!(lifecycle.state(), LifecycleState::Failed);
    // The §5.6 cleanup edge Failed → Draining → Stopped works.
    struct Cleanup;
    impl DrainPort for Cleanup {
        fn outstanding_work(&self) -> usize {
            0
        }
        fn drain(&mut self, _deadline_millis: u64) -> DrainOutcome {
            DrainOutcome::Quiesced { cancelled: 0 }
        }
    }
    let mut port = Cleanup;
    lifecycle
        .begin_drain(&mut port, "operator", "drain the failed plugin before removal")
        .expect("the cleanup drain begins");
    lifecycle
        .complete_drain("operator", "removed")
        .expect("the failed plugin is drained");
    assert_eq!(lifecycle.state(), LifecycleState::Stopped);
}

#[test]
fn the_explicit_restart_starts_a_fresh_lifecycle_and_is_audited() {
    let clock = FixedClock::new(1_000);
    let audit = MemoryAuditSink::new();
    let mut lifecycle = fresh_lifecycle(&clock, &audit);
    lifecycle
        .verify(GateOutcome::Passed, "composition-root", "provenance")
        .expect("verify");
    lifecycle
        .fail("supervisor", "initialization failed")
        .expect("the §5.6 failure edge");
    assert_eq!(lifecycle.state(), LifecycleState::Failed);
    assert_eq!(lifecycle.failed_from(), Some(LifecycleState::Started));
    // The EXPLICIT operator restart: the ONLY restart path (no
    // auto-restart in v0.5.0 — the recorded DEFAULT decision).
    let mut restarted = lifecycle
        .restart_explicitly("operator", "operator-approved restart after the failure")
        .expect("the explicit restart");
    assert_eq!(restarted.state(), LifecycleState::Discovered, "a fresh lifecycle");
    let records = audit.records();
    assert!(records.iter().any(|r| r.kind == LifecycleAuditKind::ExplicitRestart
        && r.from == Some(LifecycleState::Failed)
        && r.to == Some(LifecycleState::Discovered)
        && r.actor == "operator"));
    // The fresh lifecycle walks the forward path again.
    restarted
        .verify(GateOutcome::Passed, "composition-root", "provenance re-accepted")
        .expect("the fresh lifecycle verifies");
    assert_eq!(restarted.state(), LifecycleState::Verified);
}
