// P4.10 — Command lifecycle engine tests (report §14.1–§14.2; ledger P4.10;
//         P0.11 frozen table as the oracle).
//
// Coverage per the ledger P4.10 acceptance criteria (the exhaustive 14×14
// matrix is P4.33's — this suite exercises every LEGAL edge once plus a
// representative set of illegal ones, not the full matrix):
//
// - the full forward chain PROPOSED → VALIDATED → FEASIBLE → APPROVED →
//   ISSUED → ACKNOWLEDGED → TRACKING → COMPLETED, with a complete journal;
// - EVERY frozen edge exercised through its engine operation, derived from
//   `permitted_transitions()` (which delegates to the P0.11
//   `try_transition` — the frozen table stays the single source of truth;
//   this test adds no edge of its own);
// - gate separation is structural: the three gates apply only from their
//   own states; APPROVED without FEASIBLE evidence is impossible; every
//   REJECTED record names WHICH gate rejected and the failing check;
// - controlled-clock expiry: EXPIRED never fires before the validity window
//   ends, the milestone-not-reached must match the state, and timestamps
//   may never regress;
// - bidirectional supersede (SUPERSEDED ≠ CANCELLED), FAILED/EXPIRED →
//   FALLBACK state support, and representative illegal transitions out of
//   terminal states — all rejected with a named error.

use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::ValidUntil;
use flexcore_domain::command_lifecycle::{
    Actor, AuthorizationOutcome, CommandLifecycle, EvidenceReference, ExpiryMilestone, FailureReason,
    Gate, GateOutcome, LifecycleClock, LifecycleError,
};
use flexcore_domain::command_state::CommandState;
use flexcore_domain::ids::CommandId;

// --- Harness --------------------------------------------------------------------

const UUID_BASE: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f";

fn command_uuid(suffix: u8) -> FlexUuid {
    let mut bytes = [0u8; 16];
    bytes[..12].copy_from_slice(UUID_BASE.as_bytes());
    bytes[12] = 0xB4; // P4.10 fixture marker
    bytes[13] = suffix;
    FlexUuid::from_bytes(bytes)
}

fn command_id(suffix: u8) -> CommandId {
    CommandId::new(command_uuid(suffix))
}

/// A fixed, advanceable controlled clock (never wall-clock).
struct FixedClock {
    now_unix_millis: i64,
}

impl FixedClock {
    fn at(now_unix_millis: i64) -> Self {
        Self { now_unix_millis }
    }
    fn set(&mut self, now_unix_millis: i64) {
        self.now_unix_millis = now_unix_millis;
    }
}

impl LifecycleClock for FixedClock {
    fn now_unix_millis(&self) -> i64 {
        self.now_unix_millis
    }
}

fn actor(name: &str) -> Actor {
    Actor::new(name).expect("actor builds")
}

fn evidence(reference: &str) -> EvidenceReference {
    EvidenceReference::new(reference).expect("evidence builds")
}

/// The validity bound used across the suite: 100 000 ms. The clock starts at
/// 1 000 ms, so expiry requires an explicit clock advance (controlled-clock
/// discipline).
fn valid_until() -> ValidUntil {
    ValidUntil::from_unix_millis(100_000).expect("valid until builds")
}

fn propose(id: u8) -> CommandLifecycle {
    CommandLifecycle::propose(command_id(id), valid_until()).expect("lifecycle proposes")
}

/// Advances one lifecycle to `state` through legal engine operations only.
fn lifecycle_at(id: u8, state: CommandState, clock: &mut FixedClock) -> CommandLifecycle {
    let mut lc = propose(id);
    match state {
        CommandState::Proposed => {}
        CommandState::Validated => {
            lc.record_validation(gate_pass(), actor("svc:validator"), clock)
                .expect("validation passes");
        }
        CommandState::Feasible => {
            lc.record_validation(gate_pass(), actor("svc:validator"), clock)
                .expect("validation passes");
            lc.record_feasibility(gate_pass(), actor("svc:feasibility"), clock)
                .expect("feasibility passes");
        }
        CommandState::Approved => {
            lifecycle_at(id, CommandState::Feasible, clock);
            lc.record_authorization(authz_approve(), clock)
                .expect("authorization grants");
        }
        CommandState::Issued => {
            lifecycle_at(id, CommandState::Approved, clock);
            lc.record_issue(evidence("sig:sim-1"), actor("svc:dispatch"), clock)
                .expect("issue records");
        }
        CommandState::Acknowledged => {
            lifecycle_at(id, CommandState::Issued, clock);
            lc.record_acknowledgement(evidence("ack:sim-1"), actor("adapter:simulator"), clock)
                .expect("acknowledgement records");
        }
        CommandState::Tracking => {
            lifecycle_at(id, CommandState::Acknowledged, clock);
            lc.record_tracking_started(evidence("telemetry:snap-1"), actor("svc:tracking"), clock)
                .expect("tracking starts");
        }
        CommandState::Failed => {
            lifecycle_at(id, CommandState::Issued, clock);
            lc.record_failure(FailureReason::Nack, actor("svc:dispatch"), clock)
                .expect("failure records");
        }
        CommandState::Expired => {
            lifecycle_at(id, CommandState::Approved, clock);
            clock.set(lc.valid_until_millis() + 1);
            lc.record_expiry(ExpiryMilestone::BeforeIssue, actor("svc:expiry-sweep"), clock)
                .expect("expiry records");
            clock.set(1_000);
        }
        // FAILED and EXPIRED are reachable from-state handles too (frozen
        // edges 26–27 leave them). Terminal states are never pre-built —
        // the edge driver enters them directly.
        _ => unreachable!("lifecycle_at is only for non-terminal states"),
    }
    lc
}

fn gate_pass() -> GateOutcome {
    GateOutcome::pass(evidence("evidence:check-pass"))
}

fn gate_fail(check: &str) -> GateOutcome {
    GateOutcome::fail(evidence("evidence:check-fail"), check).expect("failing check named")
}

fn authz_approve() -> AuthorizationOutcome {
    AuthorizationOutcome::approve(evidence("authz:decision-1"), actor("operator:human-1"))
}

/// Performs the engine operation that produces `to` from the lifecycle's
/// current state — one dispatch table covering all 27 frozen edges. Returns
/// the recorded transition.
fn drive_edge(
    lc: &mut CommandLifecycle,
    to: CommandState,
    clock: &mut FixedClock,
    superseding_id: CommandId,
) -> flexcore_domain::command_lifecycle::CommandTransition {
    let from = lc.state();
    let recorded = match to {
        CommandState::Validated => {
            lc.record_validation(gate_pass(), actor("svc:validator"), clock)
        }
        CommandState::Feasible => {
            lc.record_feasibility(gate_pass(), actor("svc:feasibility"), clock)
        }
        CommandState::Approved => lc.record_authorization(authz_approve(), clock),
        CommandState::Rejected => match from {
            CommandState::Proposed => {
                lc.record_validation(gate_fail("malformed payload"), actor("svc:validator"), clock)
            }
            CommandState::Validated => {
                lc.record_feasibility(gate_fail("capability exceeded"), actor("svc:feasibility"), clock)
            }
            CommandState::Feasible => lc.record_authorization(
                AuthorizationOutcome::deny(
                    evidence("authz:decision-2"),
                    actor("operator:human-2"),
                    "authorization denied by policy",
                )
                .expect("denial reason named"),
                clock,
            ),
            other => panic!("no gate rejection drives from {}", other.name()),
        },
        CommandState::Issued => {
            lc.record_issue(evidence("sig:sim-1"), actor("svc:dispatch"), clock)
        }
        CommandState::Acknowledged => {
            lc.record_acknowledgement(evidence("ack:sim-1"), actor("adapter:simulator"), clock)
        }
        CommandState::Tracking => {
            lc.record_tracking_started(evidence("telemetry:snap-1"), actor("svc:tracking"), clock)
        }
        CommandState::Completed => {
            lc.record_completion(evidence("criterion:satisfied"), actor("svc:tracking"), clock)
        }
        CommandState::Failed => {
            let reason = match from {
                CommandState::Issued => FailureReason::Nack,
                CommandState::Acknowledged => FailureReason::NoExpectedResponse,
                CommandState::Tracking => FailureReason::TrackingDeviation,
                other => panic!("no failure trigger drives from {}", other.name()),
            };
            lc.record_failure(reason, actor("svc:dispatch"), clock)
        }
        CommandState::Expired => {
            // The validity window must actually have ended (controlled clock).
            clock.set(lc.valid_until_millis() + 1);
            let milestone = match from {
                CommandState::Approved => ExpiryMilestone::BeforeIssue,
                CommandState::Issued => ExpiryMilestone::BeforeAcknowledgement,
                CommandState::Acknowledged => ExpiryMilestone::BeforeTrackingCompletion,
                CommandState::Tracking => ExpiryMilestone::BeforeCompletion,
                other => panic!("no expiry milestone drives from {}", other.name()),
            };
            let result = lc.record_expiry(milestone, actor("svc:expiry-sweep"), clock);
            clock.set(1_000); // restore the suite clock
            result
        }
        CommandState::Cancelled => {
            lc.record_cancellation("operator cancelled the window", actor("operator:human-1"), clock)
        }
        CommandState::Superseded => lc.record_superseded_by(superseding_id, actor("svc:revision"), clock),
        CommandState::Fallback => {
            lc.record_fallback(evidence("fallback:policy-ref"), actor("svc:fallback"), clock)
        }
        other => panic!("no engine operation produces {}", other.name()),
    };
    recorded.expect("the edge drives")
}

// --- Tests -----------------------------------------------------------------------

#[test]
fn the_full_forward_chain_completes_with_a_complete_journal() {
    let mut clock = FixedClock::at(1_000);
    let mut lc = propose(0x01);
    let path = [
        CommandState::Validated,
        CommandState::Feasible,
        CommandState::Approved,
        CommandState::Issued,
        CommandState::Acknowledged,
        CommandState::Tracking,
        CommandState::Completed,
    ];
    let mut previous_at = 0;
    for &to in &path {
        clock.set(clock.now_unix_millis() + 1);
        let _ = drive_edge(&mut lc, to, &mut clock, command_id(0xFF));
        let entry = lc.journal().last().expect("journal entry recorded");
        assert_eq!(entry.to, to);
        assert!(entry.at_unix_millis >= previous_at, "timestamps never regress");
        previous_at = entry.at_unix_millis;
    }
    assert_eq!(lc.state(), CommandState::Completed);
    assert!(lc.is_terminal());
    assert_eq!(lc.journal().len(), 7);
    assert!(lc.rejected_by().is_none());
    assert!(lc.superseded_by().is_none());
    // Journal completeness: every entry carries its actor and timestamp; the
    // three gate transitions carry gate + evidence; the terminal completion
    // carries its acceptance evidence.
    for entry in lc.journal() {
        assert!(!entry.actor.identity().trim().is_empty());
        assert!(entry.at_unix_millis > 0);
    }
    let gates: Vec<Gate> = lc
        .journal()
        .iter()
        .filter_map(|entry| entry.gate)
        .collect();
    assert_eq!(
        gates,
        [Gate::Validation, Gate::Feasibility, Gate::Authorization]
    );
    let last = lc.journal().last().unwrap();
    assert_eq!(last.evidence.as_ref().unwrap().as_str(), "criterion:satisfied");
}

#[test]
fn every_frozen_edge_is_exercised_through_its_engine_operation() {
    // The from-states the frozen edges leave (the 9 non-terminal states with
    // outgoing edges, per the P0.11 freeze §2.1 outgoing-edge table).
    let from_states = [
        CommandState::Proposed,
        CommandState::Validated,
        CommandState::Feasible,
        CommandState::Approved,
        CommandState::Issued,
        CommandState::Acknowledged,
        CommandState::Tracking,
        CommandState::Failed,
        CommandState::Expired,
    ];
    let mut exercised = 0usize;
    for (i, &from) in from_states.iter().enumerate() {
        for &to in CommandState::ALL.iter() {
            // The permitted set comes from the frozen table itself.
            let probe = lifecycle_at(i as u8 + 1, from, &mut FixedClock::at(1_000));
            if !probe.permitted_transitions().contains(&to) {
                continue;
            }
            let mut clock = FixedClock::at(1_000);
            let mut lc = lifecycle_at(i as u8 + 1, from, &mut clock);
            let recorded = drive_edge(&mut lc, to, &mut clock, command_id(0xFF));
            assert_eq!(recorded.from, from, "edge {from:?} -> {to:?}");
            assert_eq!(recorded.to, to, "edge {from:?} -> {to:?}");
            assert_eq!(lc.state(), to, "edge {from:?} -> {to:?} landed");
            // Every journalled edge names its actor and time; reason and
            // evidence presence follow the report's rules (the SQL journal
            // mirrors the same rules — see the P4.10 migration).
            assert!(!recorded.actor.identity().trim().is_empty());
            assert!(recorded.at_unix_millis >= 1_000);
            if matches!(
                to,
                CommandState::Validated
                    | CommandState::Feasible
                    | CommandState::Approved
                    | CommandState::Issued
                    | CommandState::Acknowledged
                    | CommandState::Tracking
                    | CommandState::Completed
                    | CommandState::Superseded
                    | CommandState::Fallback
            ) {
                assert!(
                    recorded.evidence.is_some(),
                    "edge {from:?} -> {to:?} must carry an evidence reference"
                );
            }
            if matches!(
                to,
                CommandState::Rejected
                    | CommandState::Failed
                    | CommandState::Expired
                    | CommandState::Cancelled
                    | CommandState::Fallback
            ) {
                assert!(
                    recorded.reason.is_some(),
                    "edge {from:?} -> {to:?} must carry a reason"
                );
            }
            exercised += 1;
        }
    }
    // Exactly the 27 frozen state-to-state edges were exercised — no more,
    // no fewer (P0.11 §2). The exhaustive 14×14 matrix (legal AND illegal)
    // is P4.33's.
    assert_eq!(exercised, 27);
}

#[test]
fn each_rejection_names_its_gate_and_failing_check() {
    // The three rejection points of the frozen table (§14.1): one per gate.
    let mut clock = FixedClock::at(1_000);
    let mut proposed = propose(0x10);
    proposed
        .record_validation(gate_fail("malformed payload"), actor("svc:validator"), &mut clock)
        .expect("validation rejection records");
    assert_eq!(proposed.state(), CommandState::Rejected);
    assert_eq!(proposed.rejected_by(), Some(Gate::Validation));
    assert_eq!(
        proposed.journal().last().unwrap().reason.as_deref(),
        Some("malformed payload")
    );
    assert_eq!(
        proposed.journal().last().unwrap().gate,
        Some(Gate::Validation)
    );

    let mut validated = lifecycle_at(0x11, CommandState::Validated, &mut clock);
    validated
        .record_feasibility(gate_fail("capability exceeded"), actor("svc:feasibility"), &mut clock)
        .expect("feasibility rejection records");
    assert_eq!(validated.state(), CommandState::Rejected);
    assert_eq!(validated.rejected_by(), Some(Gate::Feasibility));

    let mut feasible = lifecycle_at(0x12, CommandState::Feasible, &mut clock);
    feasible
        .record_authorization(
            AuthorizationOutcome::deny(
                evidence("authz:decision-2"),
                actor("operator:human-2"),
                "authorization denied by policy",
            )
            .expect("denial reason named"),
            &mut clock,
        )
        .expect("authorization denial records");
    assert_eq!(feasible.state(), CommandState::Rejected);
    assert_eq!(feasible.rejected_by(), Some(Gate::Authorization));
    assert_eq!(
        feasible.journal().last().unwrap().actor.identity(),
        "operator:human-2"
    );
}

#[test]
fn approved_without_feasible_evidence_is_impossible_by_construction() {
    let mut clock = FixedClock::at(1_000);
    // The authorization gate applies ONLY from FEASIBLE; FEASIBLE is
    // reachable ONLY through a passed feasibility outcome. Both illegal
    // entry points are named errors.
    let mut proposed = propose(0x20);
    assert_eq!(
        proposed.record_authorization(authz_approve(), &mut clock),
        Err(LifecycleError::WrongState {
            current: CommandState::Proposed,
            operation: "record_authorization",
        })
    );
    let mut validated = lifecycle_at(0x21, CommandState::Validated, &mut clock);
    assert_eq!(
        validated.record_authorization(authz_approve(), &mut clock),
        Err(LifecycleError::WrongState {
            current: CommandState::Validated,
            operation: "record_authorization",
        })
    );
    assert_ne!(validated.state(), CommandState::Approved);
    // And the same structural separation holds for the other gates.
    assert_eq!(
        proposed.record_feasibility(gate_pass(), actor("svc:feasibility"), &mut clock),
        Err(LifecycleError::WrongState {
            current: CommandState::Proposed,
            operation: "record_feasibility",
        })
    );
    assert_eq!(
        validated.record_validation(gate_pass(), actor("svc:validator"), &mut clock),
        Err(LifecycleError::WrongState {
            current: CommandState::Validated,
            operation: "record_validation",
        })
    );
}

#[test]
fn expiry_never_fires_before_the_validity_window_ends() {
    let mut clock = FixedClock::at(1_000);
    let mut lc = lifecycle_at(0x30, CommandState::Approved, &mut clock);
    // Before valid-until: EXPIRED is refused with the named error.
    assert_eq!(
        lc.record_expiry(ExpiryMilestone::BeforeIssue, actor("svc:expiry-sweep"), &mut clock),
        Err(LifecycleError::ValidityNotLapsed {
            now_unix_millis: 1_000,
            valid_until_millis: 100_000,
        })
    );
    assert_eq!(lc.state(), CommandState::Approved);
    // validity_has_lapsed is the state support the sweep (P4.15) will call.
    assert!(!lc.validity_has_lapsed(&clock));
    clock.set(100_000); // end-exclusive window: over at exactly valid-until
    assert!(lc.validity_has_lapsed(&clock));
    lc.record_expiry(ExpiryMilestone::BeforeIssue, actor("svc:expiry-sweep"), &mut clock)
        .expect("expiry records once the window has ended");
    assert_eq!(lc.state(), CommandState::Expired);
    assert_eq!(
        lc.journal().last().unwrap().reason.as_deref(),
        Some(ExpiryMilestone::BeforeIssue.reason_text())
    );
    // The milestone must match the state the expiry leaves: an APPROVED
    // command cannot expire "before acknowledgement".
    let mut lc2 = lifecycle_at(0x31, CommandState::Approved, &mut clock);
    assert_eq!(
        lc2.record_expiry(
            ExpiryMilestone::BeforeAcknowledgement,
            actor("svc:expiry-sweep"),
            &mut clock
        ),
        Err(LifecycleError::InvalidExpiryMilestone {
            current: CommandState::Approved,
            milestone: ExpiryMilestone::BeforeAcknowledgement.reason_text(),
        })
    );
    // EXPIRED exits only via FALLBACK (frozen edge 27) — nothing else.
    assert_eq!(lc2.permitted_transitions(), vec![CommandState::Fallback]);
}

#[test]
fn supersede_links_bidirectionally_and_stays_distinct_from_cancellation() {
    let mut clock = FixedClock::at(1_000);
    let older_id = command_id(0x40);
    let newer_id = command_id(0x41);
    // A VALIDATED command is supersedeable (frozen edge 5).
    let mut older = lifecycle_at(0x40, CommandState::Validated, &mut clock);
    assert_eq!(older.command_id(), older_id);
    // A command cannot supersede itself.
    assert_eq!(
        older.record_superseded_by(older_id, actor("svc:revision"), &mut clock),
        Err(LifecycleError::SelfSupersede { command_id: older_id })
    );
    // The replacement invalidates the older command before completion; the
    // older record is retained for audit with the backward link.
    let recorded = older
        .record_superseded_by(newer_id, actor("svc:revision"), &mut clock)
        .expect("supersede records");
    assert_eq!(older.state(), CommandState::Superseded);
    assert!(older.is_terminal());
    assert_eq!(older.superseded_by(), Some(newer_id));
    assert_eq!(
        recorded.evidence.as_ref().unwrap().as_str(),
        format!("command:{newer_id}")
    );
    // The forward link lives on the replacement (§14.3 field 16); the
    // persistence layer stores both directions (D-P4.10-3).
    assert_ne!(older.state(), CommandState::Cancelled);
    // The two acts produce DISTINCT audit records.
    let mut approved = lifecycle_at(0x42, CommandState::Approved, &mut clock);
    approved
        .record_cancellation("operator cancelled the window", actor("operator:human-1"), &mut clock)
        .expect("cancellation records");
    assert_eq!(approved.state(), CommandState::Cancelled);
    assert_eq!(
        approved.journal().last().unwrap().reason.as_deref(),
        Some("operator cancelled the window")
    );
    assert_eq!(
        older.journal().last().unwrap().reason.as_deref(),
        Some("invalidated by a newer approved schedule/command revision")
    );
    // SUPERSEDED is terminal (frozen table: no outgoing edge).
    assert_eq!(older.permitted_transitions(), Vec::<CommandState>::new());
}

#[test]
fn failed_and_expired_exit_only_via_fallback_with_a_policy_reference() {
    let mut clock = FixedClock::at(1_000);
    // FAILED → FALLBACK (frozen edge 26).
    let mut failed = lifecycle_at(0x50, CommandState::Issued, &mut clock);
    failed
        .record_failure(FailureReason::Nack, actor("svc:dispatch"), &mut clock)
        .expect("failure records");
    assert_eq!(failed.state(), CommandState::Failed);
    assert_eq!(
        failed.permitted_transitions(),
        vec![CommandState::Fallback],
        "FAILED exits only via FALLBACK"
    );
    failed
        .record_fallback(evidence("fallback:hold-last-setpoint"), actor("svc:fallback"), &mut clock)
        .expect("fallback records");
    assert_eq!(failed.state(), CommandState::Fallback);
    assert_eq!(failed.journal().last().unwrap().reason.as_deref(), Some("asset/site fallback policy activated"));
    // EXPIRED → FALLBACK (frozen edge 27).
    let mut expired = lifecycle_at(0x51, CommandState::Tracking, &mut clock);
    clock.set(expired.valid_until_millis() + 1);
    expired
        .record_expiry(ExpiryMilestone::BeforeCompletion, actor("svc:expiry-sweep"), &mut clock)
        .expect("expiry records");
    expired
        .record_fallback(evidence("fallback:require-operator"), actor("svc:fallback"), &mut clock)
        .expect("fallback records");
    assert_eq!(expired.state(), CommandState::Fallback);
    assert_eq!(expired.journal().last().unwrap().reason.as_deref(), Some("fallback policy requires"));
    // The failure trigger must be the frozen annotation for the state.
    let mut issued = lifecycle_at(0x52, CommandState::Issued, &mut clock);
    assert_eq!(
        issued.record_failure(FailureReason::TrackingDeviation, actor("svc:dispatch"), &mut clock),
        Err(LifecycleError::InvalidFailureReason {
            current: CommandState::Issued,
            reason: FailureReason::TrackingDeviation.reason_text(),
        })
    );
}

#[test]
fn terminal_states_have_no_outgoing_engine_operation() {
    let mut clock = FixedClock::at(1_000);
    // COMPLETED
    let mut completed = lifecycle_at(0x60, CommandState::Tracking, &mut clock);
    completed
        .record_completion(evidence("criterion:satisfied"), actor("svc:tracking"), &mut clock)
        .expect("completes");
    assert_eq!(completed.permitted_transitions(), Vec::<CommandState>::new());
    assert!(completed.record_issue(evidence("sig:x"), actor("svc:dispatch"), &mut clock).is_err());
    assert!(completed.record_failure(FailureReason::TrackingTimeout, actor("svc:dispatch"), &mut clock).is_err());
    assert!(completed.record_cancellation("reason", actor("operator:human-1"), &mut clock).is_err());
    // REJECTED
    let mut rejected = propose(0x61);
    rejected
        .record_validation(gate_fail("malformed payload"), actor("svc:validator"), &mut clock)
        .expect("rejects");
    assert!(rejected.record_validation(gate_pass(), actor("svc:validator"), &mut clock).is_err());
    assert!(rejected.record_superseded_by(command_id(0xFF), actor("svc:revision"), &mut clock).is_err());
    assert_eq!(rejected.permitted_transitions(), Vec::<CommandState>::new());
    // CANCELLED, SUPERSEDED, FALLBACK
    let mut cancelled = lifecycle_at(0x62, CommandState::Approved, &mut clock);
    cancelled
        .record_cancellation("reason", actor("operator:human-1"), &mut clock)
        .expect("cancels");
    assert!(cancelled.record_issue(evidence("sig:x"), actor("svc:dispatch"), &mut clock).is_err());
    let mut superseded = lifecycle_at(0x63, CommandState::Feasible, &mut clock);
    superseded
        .record_superseded_by(command_id(0x64), actor("svc:revision"), &mut clock)
        .expect("supersedes");
    assert!(superseded.record_authorization(authz_approve(), &mut clock).is_err());
    // Build an APPROVED lifecycle, then expire it (controlled clock).
    let mut fallback = lifecycle_at(0x65, CommandState::Approved, &mut clock);
    clock.set(fallback.valid_until_millis() + 1);
    fallback
        .record_expiry(ExpiryMilestone::BeforeIssue, actor("svc:expiry-sweep"), &mut clock)
        .expect("expires");
    clock.set(1_000);
    fallback
        .record_fallback(evidence("fallback:policy-ref"), actor("svc:fallback"), &mut clock)
        .expect("falls back");
    assert!(fallback.record_failure(FailureReason::TransportTimeout, actor("svc:dispatch"), &mut clock).is_err());
    // All five terminal states still hold their state — nothing moved.
    assert_eq!(completed.state(), CommandState::Completed);
    assert_eq!(rejected.state(), CommandState::Rejected);
    assert_eq!(cancelled.state(), CommandState::Cancelled);
    assert_eq!(superseded.state(), CommandState::Superseded);
    assert_eq!(fallback.state(), CommandState::Fallback);
}

#[test]
fn gate_skipping_shortcuts_are_named_errors() {
    let mut clock = FixedClock::at(1_000);
    // PROPOSED cannot skip to ISSUED; VALIDATED cannot skip to APPROVED or
    // ISSUED; FEASIBLE cannot skip to ISSUED (no convenience edges, §14.1).
    let mut proposed = propose(0x70);
    assert_eq!(
        proposed.record_issue(evidence("sig:x"), actor("svc:dispatch"), &mut clock),
        Err(LifecycleError::NotPermitted {
            from: CommandState::Proposed,
            to: CommandState::Issued,
        })
    );
    let mut validated = lifecycle_at(0x71, CommandState::Validated, &mut clock);
    assert!(validated.record_issue(evidence("sig:x"), actor("svc:dispatch"), &mut clock).is_err());
    assert!(validated.record_completion(evidence("criterion:x"), actor("svc:tracking"), &mut clock).is_err());
    let mut feasible = lifecycle_at(0x72, CommandState::Feasible, &mut clock);
    assert!(feasible.record_acknowledgement(evidence("ack:x"), actor("adapter:simulator"), &mut clock).is_err());
    // Backwards transitions are rejected too.
    let mut approved = lifecycle_at(0x73, CommandState::Approved, &mut clock);
    assert!(approved.record_feasibility(gate_pass(), actor("svc:feasibility"), &mut clock).is_err());
    // Nothing moved.
    assert_eq!(proposed.state(), CommandState::Proposed);
    assert_eq!(validated.state(), CommandState::Validated);
    assert_eq!(feasible.state(), CommandState::Feasible);
    assert_eq!(approved.state(), CommandState::Approved);
}

#[test]
fn timestamps_come_from_the_controlled_clock_and_never_regress() {
    let mut clock = FixedClock::at(5_000);
    let mut lc = propose(0x80);
    let validated = lc
        .record_validation(gate_pass(), actor("svc:validator"), &mut clock)
        .expect("validates");
    assert_eq!(validated.at_unix_millis, 5_000);
    clock.set(4_000); // the controlled clock moved backwards
    assert_eq!(
        lc.record_feasibility(gate_pass(), actor("svc:feasibility"), &mut clock),
        Err(LifecycleError::TimestampRegression {
            previous_unix_millis: 5_000,
            proposed_unix_millis: 4_000,
        })
    );
    assert_eq!(lc.state(), CommandState::Validated, "nothing moved on a refused transition");
    assert_eq!(lc.journal().len(), 1, "the refused transition left no journal entry");
}

#[test]
fn empty_actors_reasons_and_references_are_named_errors() {
    let mut clock = FixedClock::at(1_000);
    assert!(Actor::new("   ").is_err());
    assert!(EvidenceReference::new("").is_err());
    assert!(GateOutcome::fail(evidence("evidence:x"), "  ").is_err());
    assert!(
        AuthorizationOutcome::deny(evidence("authz:x"), actor("operator:human-1"), "")
            .is_err()
    );
    let mut lc = propose(0x90);
    assert_eq!(
        lc.record_cancellation("", actor("operator:human-1"), &mut clock),
        Err(LifecycleError::EmptyField { field: "reason" })
    );
}
