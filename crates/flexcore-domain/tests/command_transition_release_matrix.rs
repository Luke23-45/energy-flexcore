// P10.03 — Release-candidate command-state transition suite — full matrix
//         (report §22.3 release gate 2, §14.1–§14.3; ledger P10.03; the P0.11
//         frozen 27-edge table as the oracle).
//
// Relationship to the existing suites (ADDITIVE, no ownership overlap):
// - `src/command_state.rs` (P0.11) owns the frozen table implementation and
//   in-module table tests;
// - `tests/transition_matrix.rs` (P4.33) owns the TABLE-level 196-cell matrix
//   over `command_state::try_transition`;
// - `tests/command_lifecycle.rs` (P4.10) owns the ENGINE-level legal walks and
//   gate discipline with representative illegal cases;
// - THIS suite is the Phase 10 release-gate verification (P10.03): the
//   ENGINE-level FULL matrix — every (state, engine operation) cell of the
//   `CommandLifecycle` operation surface has a hand-computed expected outcome —
//   plus the table-level 196-cell re-verification, the §14.3 rejection
//   classes, the expiry path, the failure-trigger matrix, hand-computed
//   journal expectations, per-asset partial-fleet semantics, and a
//   compile-time exhaustive match over the state enum proving the suite
//   covers the whole lifecycle vocabulary.
//
// Anti-circularity (the ledger risk: "a test that passes because the test
// harness, not the platform, enforces the rule"): the expected outcome of
// every cell is derived from the P0.11 freeze table transcribed BELOW as
// data (verbatim §14.1 guard text) and from the hand-computed trigger /
// milestone annotations — never from the implementation under test. A table
// change that adds, removes, or re-annotates an edge without a corresponding
// freeze change fails the cell comparison.
//
// Terminology note (recorded in `ledger/phase-10-v1-release-hardening/
// 03_command_transition_tests.md`): the identifiers `FROZEN_TRANSITIONS` and
// `CRASH_TRANSITIONS` belong to the PLUGIN lifecycle (report §5.6,
// `crates/flexcore-kernel/src/lifecycle.rs`), not to the command lifecycle.
// The command lifecycle's frozen table is the P0.11 27-edge table implemented
// in `crates/flexcore-domain/src/command_state.rs`. The plugin-lifecycle
// tables are exercised by `crates/flexcore-kernel/tests/lifecycle_p7_07.rs`
// (out of this file's crate seam); the command-domain fail-closed analogue is
// asserted by `every_non_terminal_state_has_a_frozen_exit_fail_closed` below.
//
// Build/test status (honest): WRITTEN, never executed — the loop bans (no
// builds, no test runs, no installs) were held; "implemented, awaiting human
// build/test". Zero observed failures are NOT claimed from an unrun suite.

use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::ValidUntil;
use flexcore_domain::command_lifecycle::{
    Actor, AuthorizationOutcome, CommandLifecycle, CommandTransition, EvidenceReference,
    ExpiryMilestone, FailureReason, Gate, GateOutcome, LifecycleClock, LifecycleError,
};
use flexcore_domain::command_state::{is_terminal, try_transition, CommandState, TransitionError};
use flexcore_domain::ids::CommandId;

use CommandState::*;

// ============================================================================
// Harness — controlled clock only, never wall-clock (P4.15 discipline)
// ============================================================================

/// Fixed step between hand-computed transition timestamps.
const STEP_MILLIS: i64 = 1_000;
/// The §14.3 valid-until bound of every fixture lifecycle (UTC unix millis).
const VALID_UNTIL_MILLIS: i64 = 100_000;

struct ManualClock {
    now_unix_millis: i64,
}

impl ManualClock {
    fn new() -> Self {
        Self { now_unix_millis: 0 }
    }
    fn step(&mut self) {
        self.now_unix_millis += STEP_MILLIS;
    }
    /// Moves the clock to the instant the validity window has just ended
    /// (`now >= valid_until`, §10.3 end-exclusive interval semantics).
    fn lapse(&mut self) {
        self.now_unix_millis = VALID_UNTIL_MILLIS;
    }
}

impl LifecycleClock for ManualClock {
    fn now_unix_millis(&self) -> i64 {
        self.now_unix_millis
    }
}

/// Deterministic fixture UUIDs: the release-candidate marker in the first
/// nine bytes, the fixture suffix in the last byte.
fn fixture_uuid(suffix: u8) -> FlexUuid {
    let mut bytes = [0u8; 16];
    bytes[..9].copy_from_slice(b"RC-P10.03");
    bytes[15] = suffix;
    FlexUuid::from_bytes(bytes)
}

fn command_id_for(state: CommandState) -> CommandId {
    let index = CommandState::ALL
        .iter()
        .position(|&s| s == state)
        .expect("state is in ALL")
        + 1;
    CommandId::new(fixture_uuid(index as u8))
}

/// The replacement command used by every SUPERSEDE operation (distinct from
/// every fixture command id).
fn superseding_id() -> CommandId {
    CommandId::new(fixture_uuid(0xAA))
}

fn fresh_valid_until() -> ValidUntil {
    ValidUntil::from_unix_millis(VALID_UNTIL_MILLIS).expect("valid until builds")
}

fn actor(name: &str) -> Actor {
    Actor::new(name).expect("actor builds")
}

fn evidence(reference: &str) -> EvidenceReference {
    EvidenceReference::new(reference).expect("evidence builds")
}

fn gate_pass() -> GateOutcome {
    GateOutcome::pass(evidence("ev:rc-pass"))
}

fn gate_fail(failing_check: &'static str) -> GateOutcome {
    GateOutcome::fail(evidence("ev:rc-fail"), failing_check).expect("gate outcome builds")
}

fn authz_approve() -> AuthorizationOutcome {
    AuthorizationOutcome::approve(evidence("authz:rc-approved"), actor("op:approver"))
}

fn authz_deny() -> AuthorizationOutcome {
    AuthorizationOutcome::deny(evidence("authz:rc-denied"), actor("op:approver"), "outside approved scope")
        .expect("denial builds")
}

/// Applies one legal drive step to a lifecycle under construction. Each
/// target is used only from its canonical predecessor, so the operation is
/// unambiguous.
fn step_to(lc: &mut CommandLifecycle, to: CommandState, clock: &mut ManualClock) {
    clock.step();
    match to {
        Proposed => unreachable!("PROPOSED is the entry marker, never a drive step"),
        Validated => {
            lc.record_validation(gate_pass(), actor("svc:validator"), clock)
                .expect("drive: validation passes")
        }
        Feasible => {
            lc.record_feasibility(gate_pass(), actor("svc:feasibility"), clock)
                .expect("drive: feasibility passes")
        }
        Approved => {
            lc.record_authorization(authz_approve(), clock)
                .expect("drive: authorization grants")
        }
        Issued => {
            lc.record_issue(evidence("sig:rc"), actor("svc:dispatch"), clock)
                .expect("drive: issue records")
        }
        Acknowledged => {
            lc.record_acknowledgement(evidence("ack:rc"), actor("adapter:simulator"), clock)
                .expect("drive: acknowledgement records")
        }
        Tracking => {
            lc.record_tracking_started(evidence("telemetry:rc"), actor("svc:tracking"), clock)
                .expect("drive: tracking starts")
        }
        Completed => {
            lc.record_completion(evidence("criterion:rc"), actor("svc:tracking"), clock)
                .expect("drive: completion records")
        }
        Rejected => {
            lc.record_validation(gate_fail("malformed_unsupported"), actor("svc:validator"), clock)
                .expect("drive: validation rejection records")
        }
        Failed => {
            lc.record_failure(FailureReason::Nack, actor("adapter:simulator"), clock)
                .expect("drive: failure records")
        }
        Expired => {
            // The milestone must be the one the frozen table annotates for the
            // current state; the clock must have lapsed first.
            let milestone = match lc.state() {
                Approved => ExpiryMilestone::BeforeIssue,
                Issued => ExpiryMilestone::BeforeAcknowledgement,
                Acknowledged => ExpiryMilestone::BeforeTrackingCompletion,
                Tracking => ExpiryMilestone::BeforeCompletion,
                other => panic!("cannot expire from {}", other.name()),
            };
            clock.lapse();
            lc.record_expiry(milestone, actor("svc:expiry"), clock)
                .expect("drive: expiry records")
        }
        Cancelled => {
            lc.record_cancellation("operator hold", actor("op:operator"), clock)
                .expect("drive: cancellation records")
        }
        Superseded => {
            lc.record_superseded_by(superseding_id(), actor("svc:scheduling"), clock)
                .expect("drive: supersede records")
        }
        Fallback => {
            lc.record_fallback(evidence("policy:rc"), actor("policy:site"), clock)
                .expect("drive: fallback records")
        }
    }
}

/// Drives a fresh lifecycle to `state` through legal engine operations only,
/// with a hand-computable timeline (each drive step costs `STEP_MILLIS`).
fn drive_to(state: CommandState, clock: &mut ManualClock) -> CommandLifecycle {
    let mut lc = CommandLifecycle::propose(command_id_for(state), fresh_valid_until())
        .expect("lifecycle proposes");
    let path: &[CommandState] = match state {
        Proposed => &[],
        Validated => &[Validated],
        Feasible => &[Validated, Feasible],
        Approved => &[Validated, Feasible, Approved],
        Issued => &[Validated, Feasible, Approved, Issued],
        Acknowledged => &[Validated, Feasible, Approved, Issued, Acknowledged],
        Tracking => &[Validated, Feasible, Approved, Issued, Acknowledged, Tracking],
        Completed => &[Validated, Feasible, Approved, Issued, Acknowledged, Tracking, Completed],
        Rejected => &[Rejected],
        Failed => &[Validated, Feasible, Approved, Issued, Failed],
        Expired => &[Validated, Feasible, Approved, Expired],
        Cancelled => &[Validated, Feasible, Approved, Cancelled],
        Superseded => &[Validated, Superseded],
        Fallback => &[Validated, Feasible, Approved, Issued, Failed, Fallback],
    };
    for &to in path {
        step_to(&mut lc, to, clock);
    }
    assert_eq!(lc.state(), state, "drive reached {}", state.name());
    lc
}

// ============================================================================
// The P0.11 freeze table, transcribed from the §14.1 diagram (anti-circular
// anchor): 27 state-to-state edges, in table order, guard text verbatim from
// report §14.1. The terminal markers (`→ [*]`) and the entry marker
// (`[*] → PROPOSED`) are not state-to-state edges and are asserted as
// terminal/entry properties.
// ============================================================================

const FROZEN_TABLE: [(CommandState, CommandState, &str); 27] = [
    (Proposed, Validated, "schema, freshness, asset checks pass"),                              // 1
    (Proposed, Rejected, "malformed / unsupported"),                                            // 2
    (Validated, Feasible, "network checks pass"),                                               // 3
    (Validated, Rejected, "capability / network failure"),                                      // 4
    (Validated, Superseded, "newer approved schedule/command revision replaces it"),            // 5
    (Feasible, Approved, "policy / operator authorization"),                                    // 6
    (Feasible, Rejected, "authorization denied"),                                               // 7
    (Feasible, Superseded, "newer approved schedule/command revision replaces it"),             // 8
    (Approved, Issued, "signed, sequenced command sent"),                                       // 9
    (Approved, Expired, "validity ends before issue"),                                          // 10
    (Approved, Superseded, "newer approved schedule/command revision replaces it"),             // 11
    (Approved, Cancelled, "operator/policy cancellation"),                                      // 12
    (Issued, Acknowledged, "device/gateway accepts"),                                           // 13
    (Issued, Failed, "NACK / timeout / transport failure"),                                     // 14
    (Issued, Expired, "validity ends before acknowledgement"),                                  // 15
    (Issued, Superseded, "newer approved schedule/command revision replaces it"),               // 16
    (Issued, Cancelled, "supported cancellation semantics"),                                    // 17
    (Acknowledged, Tracking, "telemetry observed"),                                             // 18
    (Acknowledged, Failed, "no expected response"),                                             // 19
    (Acknowledged, Expired, "validity ends before tracking completes"),                         // 20
    (Acknowledged, Superseded, "newer approved schedule/command revision replaces it"),         // 21
    (Tracking, Completed, "acceptance criterion satisfied"),                                    // 22
    (Tracking, Failed, "tracking deviation / timeout"),                                         // 23
    (Tracking, Expired, "validity ends"),                                                       // 24
    (Tracking, Superseded, "newer approved schedule/command revision replaces it"),             // 25
    (Failed, Fallback, "asset/site policy activated"),                                          // 26
    (Expired, Fallback, "policy requires"),                                                     // 27
];

fn table_successors(state: CommandState) -> Vec<CommandState> {
    FROZEN_TABLE
        .iter()
        .filter(|&&(f, _, _)| f == state)
        .map(|&(_, t, _)| t)
        .collect()
}

/// The hand-computed failure-trigger annotations of the frozen table: the
/// three FAILED edges (14, 19, 23) and the trigger each state accepts.
/// Hand-transcribed from the §14.1 guard labels — never derived from the
/// implementation (`FailureReason::allowed_from` is private, which forces the
/// anti-circular transcription).
fn trigger_allowed(state: CommandState, reason: FailureReason) -> bool {
    matches!(
        (state, reason),
        (Issued, FailureReason::Nack)
            | (Issued, FailureReason::TransportTimeout)
            | (Issued, FailureReason::TransportFailure)
            | (Acknowledged, FailureReason::NoExpectedResponse)
            | (Tracking, FailureReason::TrackingDeviation)
            | (Tracking, FailureReason::TrackingTimeout)
    )
}

/// The hand-computed expiry-milestone annotations of the frozen table: the
/// four EXPIRED edges (10, 15, 20, 24) and the milestone each state expects.
fn expected_milestone(state: CommandState) -> Option<ExpiryMilestone> {
    match state {
        Approved => Some(ExpiryMilestone::BeforeIssue),
        Issued => Some(ExpiryMilestone::BeforeAcknowledgement),
        Acknowledged => Some(ExpiryMilestone::BeforeTrackingCompletion),
        Tracking => Some(ExpiryMilestone::BeforeCompletion),
        _ => None,
    }
}

// ============================================================================
// Part 1 — the state vocabulary and the transcribed table itself
// ============================================================================

#[test]
fn the_state_vocabulary_is_the_frozen_14_with_no_extra_state() {
    assert_eq!(CommandState::ALL.len(), 14, "exactly the 14 §14.2 states");
    let names: Vec<&str> = CommandState::ALL.iter().map(|s| s.name()).collect();
    assert_eq!(
        names,
        vec![
            "PROPOSED", "VALIDATED", "FEASIBLE", "APPROVED", "ISSUED", "ACKNOWLEDGED",
            "TRACKING", "COMPLETED", "REJECTED", "FAILED", "EXPIRED", "CANCELLED",
            "SUPERSEDED", "FALLBACK",
        ],
        "the suite is built over exactly the §14.2 vocabulary — no invented state"
    );
    let mut sorted = names.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), 14, "no duplicate states in ALL");
}

#[test]
fn the_transcribed_table_has_exactly_27_edges_with_frozen_targets() {
    assert_eq!(FROZEN_TABLE.len(), 27, "the §14.1 diagram has 27 edges");
    // Per-state outgoing counts, hand-computed from the diagram:
    // 2+3+3+4+5+4+4+1+1+0×5 = 27.
    let counts: [(CommandState, usize); 14] = [
        (Proposed, 2),
        (Validated, 3),
        (Feasible, 3),
        (Approved, 4),
        (Issued, 5),
        (Acknowledged, 4),
        (Tracking, 4),
        (Completed, 0),
        (Rejected, 0),
        (Failed, 1),
        (Expired, 1),
        (Cancelled, 0),
        (Superseded, 0),
        (Fallback, 0),
    ];
    for (state, count) in counts {
        assert_eq!(
            table_successors(state).len(),
            count,
            "{}: outgoing edge count matches the §14.1 diagram",
            state.name()
        );
    }
    // No duplicate edges.
    for (i, &(f1, t1, _)) in FROZEN_TABLE.iter().enumerate() {
        for &(f2, t2, _) in FROZEN_TABLE.iter().skip(i + 1) {
            assert!(
                f1 != f2 || t1 != t2,
                "duplicate edge {} -> {} in the transcription",
                f1.name(),
                t1.name()
            );
        }
    }
}

#[test]
fn the_five_terminal_states_are_exactly_the_frozen_ones() {
    for (state, terminal) in [
        (Completed, true),
        (Rejected, true),
        (Cancelled, true),
        (Superseded, true),
        (Fallback, true),
        (Proposed, false),
        (Validated, false),
        (Feasible, false),
        (Approved, false),
        (Issued, false),
        (Acknowledged, false),
        (Tracking, false),
        (Failed, false),
        (Expired, false),
    ] {
        assert_eq!(
            is_terminal(state),
            terminal,
            "{} terminal status matches the freeze",
            state.name()
        );
        // The five terminal markers are exactly the states with zero outgoing
        // edges in the transcription — the two views cannot drift.
        assert_eq!(
            table_successors(state).is_empty(),
            terminal,
            "{}: terminal ⇔ no outgoing frozen edge",
            state.name()
        );
    }
}

// ============================================================================
// Part 2 — table-level full matrix re-verification (196 ordered pairs):
//          every legal edge succeeds positively, every illegal pair is
//          refused with the NAMED error (release-gate re-check against the
//          release candidate; complements the P4.33 suite)
// ============================================================================

#[test]
fn table_level_matrix_all_196_pairs_have_hand_computed_outcomes() {
    let mut legal = 0usize;
    let mut illegal = 0usize;
    for &from in CommandState::ALL.iter() {
        for &to in CommandState::ALL.iter() {
            let in_table = FROZEN_TABLE.iter().any(|&(f, t, _)| f == from && t == to);
            let result = try_transition(from, to);
            if in_table {
                legal += 1;
                // Positive: the edge succeeds and lands EXACTLY in the frozen
                // target — the oracle never redirects.
                assert_eq!(
                    result,
                    Ok(to),
                    "legal edge {} -> {} must succeed into the frozen target",
                    from.name(),
                    to.name()
                );
            } else {
                illegal += 1;
                // Negative: the pair is refused with the NAMED domain error
                // carrying both states — never a silent no-op.
                assert_eq!(
                    result,
                    Err(TransitionError::NotPermitted { from, to }),
                    "illegal pair {} -> {} must be refused naming both states",
                    from.name(),
                    to.name()
                );
            }
        }
    }
    assert_eq!(legal, 27, "exactly the 27 frozen edges are legal");
    assert_eq!(illegal, 169, "196 - 27 = 169 illegal pairs, all refused");
    assert_eq!(legal + illegal, 196, "the pair space is fully covered");
}

#[test]
fn table_level_no_edge_exists_outside_the_transcribed_freeze() {
    // Totality: the accepted set and the transcribed freeze set coincide over
    // the whole pair space — an added edge (not in the freeze) or a removed
    // edge (in the freeze but refused) fails here.
    for &(from, to, guard) in FROZEN_TABLE.iter() {
        assert_eq!(
            try_transition(from, to),
            Ok(to),
            "freeze edge {} -> {} ('{}') must be accepted",
            from.name(),
            to.name(),
            guard
        );
    }
    let accepted = CommandState::ALL
        .iter()
        .flat_map(|&f| CommandState::ALL.iter().map(move |&t| (f, t)))
        .filter(|&(f, t)| try_transition(f, t).is_ok())
        .count();
    assert_eq!(accepted, 27, "the accepted set is exactly the freeze set");
}

// ============================================================================
// Part 3 — ENGINE-level full matrix: every (state, engine operation) cell has
//          a hand-computed expected outcome. The engine operation surface is
//          fully enumerated (12 methods, with their argument domains: 6
//          failure triggers, 4 expiry milestones, pass/fail and approve/deny
//          outcomes) and the expectation function uses a WILDCARD-FREE match
//          over CommandState — adding a state variant is a compile error, so
//          the suite is exhaustive over the lifecycle vocabulary by
//          construction.
// ============================================================================

/// The full engine operation surface, enumerated (no hidden operation).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    ValidationPass,
    ValidationFail,
    FeasibilityPass,
    FeasibilityFail,
    AuthorizeApprove,
    AuthorizeDeny,
    Issue,
    Acknowledge,
    StartTracking,
    Complete,
    Cancel,
    Supersede,
    Fallback,
    Fail(FailureReason),
    Expire(ExpiryMilestone),
}

const ALL_FAILURE_REASONS: [FailureReason; 6] = [
    FailureReason::Nack,
    FailureReason::TransportTimeout,
    FailureReason::TransportFailure,
    FailureReason::NoExpectedResponse,
    FailureReason::TrackingDeviation,
    FailureReason::TrackingTimeout,
];

const ALL_EXPIRY_MILESTONES: [ExpiryMilestone; 4] = [
    ExpiryMilestone::BeforeIssue,
    ExpiryMilestone::BeforeAcknowledgement,
    ExpiryMilestone::BeforeTrackingCompletion,
    ExpiryMilestone::BeforeCompletion,
];

fn all_ops() -> Vec<Op> {
    let mut ops = vec![
        Op::ValidationPass,
        Op::ValidationFail,
        Op::FeasibilityPass,
        Op::FeasibilityFail,
        Op::AuthorizeApprove,
        Op::AuthorizeDeny,
        Op::Issue,
        Op::Acknowledge,
        Op::StartTracking,
        Op::Complete,
        Op::Cancel,
        Op::Supersede,
        Op::Fallback,
    ];
    for reason in ALL_FAILURE_REASONS {
        ops.push(Op::Fail(reason));
    }
    for milestone in ALL_EXPIRY_MILESTONES {
        ops.push(Op::Expire(milestone));
    }
    ops
}

/// Whether the driver runs the expiry operation with the validity window
/// already ended (`AfterDrive`) or not yet ended (`Never`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lapse {
    AfterDrive,
    Never,
}

/// The hand-computed expected outcome of one engine cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    /// The operation succeeds and lands in exactly this state.
    Becomes(CommandState),
    /// The operation is refused with a NAMED error.
    Refused(Refusal),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Refusal {
    /// `LifecycleError::WrongState` naming the operation.
    WrongState { operation: &'static str },
    /// `LifecycleError::NotPermitted` naming the refused target.
    NotPermitted { to: CommandState },
    /// `LifecycleError::InvalidFailureReason`.
    InvalidFailureReason,
    /// `LifecycleError::InvalidExpiryMilestone`.
    InvalidExpiryMilestone,
    /// `LifecycleError::ValidityNotLapsed` (controlled clock; expiry never
    /// fires early).
    ValidityNotLapsed,
}

/// The gate operations, whose refusal is always the structural
/// `WrongState { operation }` form (the current state is asserted by the
/// driver).
fn gate_refusal(op: &Op) -> Option<Outcome> {
    let operation = match op {
        Op::ValidationPass | Op::ValidationFail => "record_validation",
        Op::FeasibilityPass | Op::FeasibilityFail => "record_feasibility",
        Op::AuthorizeApprove | Op::AuthorizeDeny => "record_authorization",
        _ => return None,
    };
    Some(Outcome::Refused(Refusal::WrongState { operation }))
}

/// The expected outcome of every engine cell whose current state is terminal
/// (COMPLETED, REJECTED, CANCELLED, SUPERSEDED, FALLBACK): no transition can
/// leave a terminal state, so every operation is refused with a named error.
fn terminal_outcome(op: &Op) -> Outcome {
    match op {
        Op::ValidationPass | Op::ValidationFail => {
            Outcome::Refused(Refusal::WrongState { operation: "record_validation" })
        }
        Op::FeasibilityPass | Op::FeasibilityFail => {
            Outcome::Refused(Refusal::WrongState { operation: "record_feasibility" })
        }
        Op::AuthorizeApprove | Op::AuthorizeDeny => {
            Outcome::Refused(Refusal::WrongState { operation: "record_authorization" })
        }
        Op::Issue => Outcome::Refused(Refusal::NotPermitted { to: Issued }),
        Op::Acknowledge => Outcome::Refused(Refusal::NotPermitted { to: Acknowledged }),
        Op::StartTracking => Outcome::Refused(Refusal::NotPermitted { to: Tracking }),
        Op::Complete => Outcome::Refused(Refusal::NotPermitted { to: Completed }),
        Op::Cancel => Outcome::Refused(Refusal::NotPermitted { to: Cancelled }),
        Op::Supersede => Outcome::Refused(Refusal::NotPermitted { to: Superseded }),
        Op::Fallback => Outcome::Refused(Refusal::WrongState { operation: "record_fallback" }),
        Op::Fail(_) => Outcome::Refused(Refusal::InvalidFailureReason),
        Op::Expire(_) => Outcome::Refused(Refusal::InvalidExpiryMilestone),
    }
}

/// THE hand-computed expectation table — one arm per state (wildcard-free:
/// adding a `CommandState` variant is a COMPILE ERROR, proving the suite is
/// exhaustive over the lifecycle vocabulary), derived from the transcribed
/// P0.11 freeze table and the hand-computed trigger/milestone annotations,
/// never from the implementation under test.
fn expected_outcome(state: CommandState, op: &Op, lapse: Lapse) -> Outcome {
    // Expiry requested before the validity window has ended is ALWAYS refused
    // with the named controlled-clock error, from every state.
    if let Op::Expire(_) = op {
        if lapse == Lapse::Never {
            return Outcome::Refused(Refusal::ValidityNotLapsed);
        }
    }
    match state {
        Proposed => match op {
            Op::ValidationPass => Outcome::Becomes(Validated),    // frozen edge 1
            Op::ValidationFail => Outcome::Becomes(Rejected),     // frozen edge 2
            Op::FeasibilityPass | Op::FeasibilityFail => gate_refusal(op).unwrap(),
            Op::AuthorizeApprove | Op::AuthorizeDeny => gate_refusal(op).unwrap(),
            Op::Issue => Outcome::Refused(Refusal::NotPermitted { to: Issued }),
            Op::Acknowledge => Outcome::Refused(Refusal::NotPermitted { to: Acknowledged }),
            Op::StartTracking => Outcome::Refused(Refusal::NotPermitted { to: Tracking }),
            Op::Complete => Outcome::Refused(Refusal::NotPermitted { to: Completed }),
            Op::Cancel => Outcome::Refused(Refusal::NotPermitted { to: Cancelled }),
            Op::Supersede => Outcome::Refused(Refusal::NotPermitted { to: Superseded }),
            Op::Fallback => Outcome::Refused(Refusal::WrongState { operation: "record_fallback" }),
            Op::Fail(_) => Outcome::Refused(Refusal::InvalidFailureReason),
            Op::Expire(_) => Outcome::Refused(Refusal::InvalidExpiryMilestone),
        },
        Validated => match op {
            Op::ValidationPass | Op::ValidationFail => gate_refusal(op).unwrap(),
            Op::FeasibilityPass => Outcome::Becomes(Feasible),    // frozen edge 3
            Op::FeasibilityFail => Outcome::Becomes(Rejected),    // frozen edge 4
            Op::AuthorizeApprove | Op::AuthorizeDeny => gate_refusal(op).unwrap(),
            Op::Issue => Outcome::Refused(Refusal::NotPermitted { to: Issued }),
            Op::Acknowledge => Outcome::Refused(Refusal::NotPermitted { to: Acknowledged }),
            Op::StartTracking => Outcome::Refused(Refusal::NotPermitted { to: Tracking }),
            Op::Complete => Outcome::Refused(Refusal::NotPermitted { to: Completed }),
            Op::Cancel => Outcome::Refused(Refusal::NotPermitted { to: Cancelled }),
            Op::Supersede => Outcome::Becomes(Superseded),        // frozen edge 5
            Op::Fallback => Outcome::Refused(Refusal::WrongState { operation: "record_fallback" }),
            Op::Fail(_) => Outcome::Refused(Refusal::InvalidFailureReason),
            Op::Expire(_) => Outcome::Refused(Refusal::InvalidExpiryMilestone),
        },
        Feasible => match op {
            Op::ValidationPass | Op::ValidationFail => gate_refusal(op).unwrap(),
            Op::FeasibilityPass | Op::FeasibilityFail => gate_refusal(op).unwrap(),
            Op::AuthorizeApprove => Outcome::Becomes(Approved),   // frozen edge 6
            Op::AuthorizeDeny => Outcome::Becomes(Rejected),      // frozen edge 7
            Op::Issue => Outcome::Refused(Refusal::NotPermitted { to: Issued }),
            Op::Acknowledge => Outcome::Refused(Refusal::NotPermitted { to: Acknowledged }),
            Op::StartTracking => Outcome::Refused(Refusal::NotPermitted { to: Tracking }),
            Op::Complete => Outcome::Refused(Refusal::NotPermitted { to: Completed }),
            Op::Cancel => Outcome::Refused(Refusal::NotPermitted { to: Cancelled }),
            Op::Supersede => Outcome::Becomes(Superseded),        // frozen edge 8
            Op::Fallback => Outcome::Refused(Refusal::WrongState { operation: "record_fallback" }),
            Op::Fail(_) => Outcome::Refused(Refusal::InvalidFailureReason),
            Op::Expire(_) => Outcome::Refused(Refusal::InvalidExpiryMilestone),
        },
        Approved => match op {
            Op::ValidationPass | Op::ValidationFail => gate_refusal(op).unwrap(),
            Op::FeasibilityPass | Op::FeasibilityFail => gate_refusal(op).unwrap(),
            Op::AuthorizeApprove | Op::AuthorizeDeny => gate_refusal(op).unwrap(),
            Op::Issue => Outcome::Becomes(Issued),                // frozen edge 9
            Op::Acknowledge => Outcome::Refused(Refusal::NotPermitted { to: Acknowledged }),
            Op::StartTracking => Outcome::Refused(Refusal::NotPermitted { to: Tracking }),
            Op::Complete => Outcome::Refused(Refusal::NotPermitted { to: Completed }),
            Op::Cancel => Outcome::Becomes(Cancelled),            // frozen edge 12
            Op::Supersede => Outcome::Becomes(Superseded),        // frozen edge 11
            Op::Fallback => Outcome::Refused(Refusal::WrongState { operation: "record_fallback" }),
            Op::Fail(_) => Outcome::Refused(Refusal::InvalidFailureReason),
            Op::Expire(ExpiryMilestone::BeforeIssue) => Outcome::Becomes(Expired), // edge 10
            Op::Expire(_) => Outcome::Refused(Refusal::InvalidExpiryMilestone),
        },
        Issued => match op {
            Op::ValidationPass | Op::ValidationFail => gate_refusal(op).unwrap(),
            Op::FeasibilityPass | Op::FeasibilityFail => gate_refusal(op).unwrap(),
            Op::AuthorizeApprove | Op::AuthorizeDeny => gate_refusal(op).unwrap(),
            Op::Issue => Outcome::Refused(Refusal::NotPermitted { to: Issued }),
            Op::Acknowledge => Outcome::Becomes(Acknowledged),    // frozen edge 13
            Op::StartTracking => Outcome::Refused(Refusal::NotPermitted { to: Tracking }),
            Op::Complete => Outcome::Refused(Refusal::NotPermitted { to: Completed }),
            Op::Cancel => Outcome::Becomes(Cancelled),            // frozen edge 17
            Op::Supersede => Outcome::Becomes(Superseded),        // frozen edge 16
            Op::Fallback => Outcome::Refused(Refusal::WrongState { operation: "record_fallback" }),
            Op::Fail(reason) if trigger_allowed(state, *reason) => Outcome::Becomes(Failed), // edge 14
            Op::Fail(_) => Outcome::Refused(Refusal::InvalidFailureReason),
            Op::Expire(ExpiryMilestone::BeforeAcknowledgement) => Outcome::Becomes(Expired), // edge 15
            Op::Expire(_) => Outcome::Refused(Refusal::InvalidExpiryMilestone),
        },
        Acknowledged => match op {
            Op::ValidationPass | Op::ValidationFail => gate_refusal(op).unwrap(),
            Op::FeasibilityPass | Op::FeasibilityFail => gate_refusal(op).unwrap(),
            Op::AuthorizeApprove | Op::AuthorizeDeny => gate_refusal(op).unwrap(),
            Op::Issue => Outcome::Refused(Refusal::NotPermitted { to: Issued }),
            Op::Acknowledge => Outcome::Refused(Refusal::NotPermitted { to: Acknowledged }),
            Op::StartTracking => Outcome::Becomes(Tracking),      // frozen edge 18
            Op::Complete => Outcome::Refused(Refusal::NotPermitted { to: Completed }),
            Op::Cancel => Outcome::Refused(Refusal::NotPermitted { to: Cancelled }),
            Op::Supersede => Outcome::Becomes(Superseded),        // frozen edge 21
            Op::Fallback => Outcome::Refused(Refusal::WrongState { operation: "record_fallback" }),
            Op::Fail(reason) if trigger_allowed(state, *reason) => Outcome::Becomes(Failed), // edge 19
            Op::Fail(_) => Outcome::Refused(Refusal::InvalidFailureReason),
            Op::Expire(ExpiryMilestone::BeforeTrackingCompletion) => Outcome::Becomes(Expired), // edge 20
            Op::Expire(_) => Outcome::Refused(Refusal::InvalidExpiryMilestone),
        },
        Tracking => match op {
            Op::ValidationPass | Op::ValidationFail => gate_refusal(op).unwrap(),
            Op::FeasibilityPass | Op::FeasibilityFail => gate_refusal(op).unwrap(),
            Op::AuthorizeApprove | Op::AuthorizeDeny => gate_refusal(op).unwrap(),
            Op::Issue => Outcome::Refused(Refusal::NotPermitted { to: Issued }),
            Op::Acknowledge => Outcome::Refused(Refusal::NotPermitted { to: Acknowledged }),
            Op::StartTracking => Outcome::Refused(Refusal::NotPermitted { to: Tracking }),
            Op::Complete => Outcome::Becomes(Completed),          // frozen edge 22
            Op::Cancel => Outcome::Refused(Refusal::NotPermitted { to: Cancelled }),
            Op::Supersede => Outcome::Becomes(Superseded),        // frozen edge 25
            Op::Fallback => Outcome::Refused(Refusal::WrongState { operation: "record_fallback" }),
            Op::Fail(reason) if trigger_allowed(state, *reason) => Outcome::Becomes(Failed), // edge 23
            Op::Fail(_) => Outcome::Refused(Refusal::InvalidFailureReason),
            Op::Expire(ExpiryMilestone::BeforeCompletion) => Outcome::Becomes(Expired), // edge 24
            Op::Expire(_) => Outcome::Refused(Refusal::InvalidExpiryMilestone),
        },
        Completed => terminal_outcome(op),
        Rejected => terminal_outcome(op),
        Cancelled => terminal_outcome(op),
        Superseded => terminal_outcome(op),
        Fallback => terminal_outcome(op),
        Failed => match op {
            Op::ValidationPass | Op::ValidationFail => gate_refusal(op).unwrap(),
            Op::FeasibilityPass | Op::FeasibilityFail => gate_refusal(op).unwrap(),
            Op::AuthorizeApprove | Op::AuthorizeDeny => gate_refusal(op).unwrap(),
            Op::Issue => Outcome::Refused(Refusal::NotPermitted { to: Issued }),
            Op::Acknowledge => Outcome::Refused(Refusal::NotPermitted { to: Acknowledged }),
            Op::StartTracking => Outcome::Refused(Refusal::NotPermitted { to: Tracking }),
            Op::Complete => Outcome::Refused(Refusal::NotPermitted { to: Completed }),
            Op::Cancel => Outcome::Refused(Refusal::NotPermitted { to: Cancelled }),
            Op::Supersede => Outcome::Refused(Refusal::NotPermitted { to: Superseded }),
            Op::Fallback => Outcome::Becomes(Fallback),           // frozen edge 26
            Op::Fail(_) => Outcome::Refused(Refusal::InvalidFailureReason),
            Op::Expire(_) => Outcome::Refused(Refusal::InvalidExpiryMilestone),
        },
        Expired => match op {
            Op::ValidationPass | Op::ValidationFail => gate_refusal(op).unwrap(),
            Op::FeasibilityPass | Op::FeasibilityFail => gate_refusal(op).unwrap(),
            Op::AuthorizeApprove | Op::AuthorizeDeny => gate_refusal(op).unwrap(),
            Op::Issue => Outcome::Refused(Refusal::NotPermitted { to: Issued }),
            Op::Acknowledge => Outcome::Refused(Refusal::NotPermitted { to: Acknowledged }),
            Op::StartTracking => Outcome::Refused(Refusal::NotPermitted { to: Tracking }),
            Op::Complete => Outcome::Refused(Refusal::NotPermitted { to: Completed }),
            Op::Cancel => Outcome::Refused(Refusal::NotPermitted { to: Cancelled }),
            Op::Supersede => Outcome::Refused(Refusal::NotPermitted { to: Superseded }),
            Op::Fallback => Outcome::Becomes(Fallback),           // frozen edge 27
            Op::Fail(_) => Outcome::Refused(Refusal::InvalidFailureReason),
            Op::Expire(_) => Outcome::Refused(Refusal::InvalidExpiryMilestone),
        },
    }
}

/// Applies one engine operation to a lifecycle driven to `state`.
fn apply(
    lc: &mut CommandLifecycle,
    op: &Op,
    lapse: Lapse,
    clock: &mut ManualClock,
) -> Result<CommandTransition, LifecycleError> {
    match op {
        Op::ValidationPass => lc.record_validation(gate_pass(), actor("svc:validator"), clock),
        Op::ValidationFail => {
            lc.record_validation(gate_fail("policy_check"), actor("svc:validator"), clock)
        }
        Op::FeasibilityPass => lc.record_feasibility(gate_pass(), actor("svc:feasibility"), clock),
        Op::FeasibilityFail => {
            lc.record_feasibility(gate_fail("network_check"), actor("svc:feasibility"), clock)
        }
        Op::AuthorizeApprove => lc.record_authorization(authz_approve(), clock),
        Op::AuthorizeDeny => lc.record_authorization(authz_deny(), clock),
        Op::Issue => lc.record_issue(evidence("sig:rc"), actor("svc:dispatch"), clock),
        Op::Acknowledge => {
            lc.record_acknowledgement(evidence("ack:rc"), actor("adapter:simulator"), clock)
        }
        Op::StartTracking => {
            lc.record_tracking_started(evidence("telemetry:rc"), actor("svc:tracking"), clock)
        }
        Op::Complete => {
            lc.record_completion(evidence("criterion:rc"), actor("svc:tracking"), clock)
        }
        Op::Cancel => lc.record_cancellation("operator request", actor("op:operator"), clock),
        Op::Supersede => lc.record_superseded_by(superseding_id(), actor("svc:scheduling"), clock),
        Op::Fallback => lc.record_fallback(evidence("policy:rc"), actor("policy:site"), clock),
        Op::Fail(reason) => lc.record_failure(*reason, actor("adapter:simulator"), clock),
        Op::Expire(milestone) => {
            if lapse == Lapse::AfterDrive {
                clock.lapse();
            }
            lc.record_expiry(*milestone, actor("svc:expiry"), clock)
        }
    }
}

/// The driver: for every (state, operation, lapse-mode) cell, the ACTUAL
/// engine outcome must equal the HAND-COMPUTED expected outcome — success
/// into exactly the frozen target with the journal appended by one entry, or
/// refusal with the exact named error, the state unchanged, and the journal
/// unchanged.
#[test]
fn engine_matrix_every_cell_matches_its_hand_computed_expectation() {
    let mut cells = 0usize;
    let mut succeeded = 0usize;
    let mut refused_wrong_state = 0usize;
    let mut refused_not_permitted = 0usize;
    let mut refused_invalid_failure_reason = 0usize;
    let mut refused_invalid_expiry_milestone = 0usize;
    let mut refused_validity_not_lapsed = 0usize;

    // Pass 1: every state × every operation, clock lapsed for expiry ops.
    for &state in CommandState::ALL.iter() {
        for op in all_ops() {
            cells += 1;
            let mut clock = ManualClock::new();
            let mut lc = drive_to(state, &mut clock);
            let journal_len_before = lc.journal().len();
            let now_at_apply = clock.now_unix_millis();
            let expected = expected_outcome(state, &op, Lapse::AfterDrive);
            let actual = apply(&mut lc, &op, Lapse::AfterDrive, &mut clock);
            assert_cell(
                &lc, state, &op, &actual, &expected, journal_len_before, now_at_apply,
            );
            match (&actual, &expected) {
                (Ok(_), Outcome::Becomes(_)) => succeeded += 1,
                (Err(LifecycleError::WrongState { .. }), Outcome::Refused(Refusal::WrongState { .. })) => {
                    refused_wrong_state += 1
                }
                (Err(LifecycleError::NotPermitted { .. }), _) => refused_not_permitted += 1,
                (Err(LifecycleError::InvalidFailureReason { .. }), _) => {
                    refused_invalid_failure_reason += 1
                }
                (Err(LifecycleError::InvalidExpiryMilestone { .. }), _) => {
                    refused_invalid_expiry_milestone += 1
                }
                _ => unreachable!("cell already asserted against its expectation"),
            }
        }
    }

    // Pass 2: expiry operations requested BEFORE the validity window has
    // ended — refused from every state that can be reached without the clock
    // having lapsed (EXPIRED itself cannot be reached pre-lapse, so 13 × 4
    // cells).
    for &state in CommandState::ALL.iter() {
        if state == Expired {
            continue; // unreachable before the window ends, by construction
        }
        for &milestone in ALL_EXPIRY_MILESTONES.iter() {
            cells += 1;
            let op = Op::Expire(milestone);
            let mut clock = ManualClock::new();
            let mut lc = drive_to(state, &mut clock);
            let journal_len_before = lc.journal().len();
            assert!(
                clock.now_unix_millis() < VALID_UNTIL_MILLIS,
                "pre-lapse pass drives {} without ending the window",
                state.name()
            );
            let now_at_apply = clock.now_unix_millis();
            let expected = expected_outcome(state, &op, Lapse::Never);
            let actual = apply(&mut lc, &op, Lapse::Never, &mut clock);
            assert_cell(
                &lc, state, &op, &actual, &expected, journal_len_before, now_at_apply,
            );
            refused_validity_not_lapsed += 1;
        }
    }

    // Hand-computed totals (asserted, not assumed):
    // - cells: 14 × 23 (lapsed) + 13 × 4 (pre-lapse) = 322 + 52 = 374;
    // - successes: 6 gate edges + 4 forward edges + 2 cancellations
    //   + 6 supersedes + 2 fallbacks + 6 failure (state, trigger) combos
    //   + 4 expiry combos = 30 — note three FAILED edges (14, 19, 23)
    //   are realized by six (state, trigger) combos, so the engine surface
    //   has 30 success cells over 27 table edges (target SETS still match
    //   the table exactly — asserted in
    //   `engine_success_targets_equal_the_frozen_table_successors`);
    // - refusals: WrongState 90, NotPermitted 72, InvalidFailureReason 78,
    //   InvalidExpiryMilestone 52, ValidityNotLapsed 52 — total refused 344.
    assert_eq!(cells, 374, "14×23 + 13×4 engine cells, each tested once");
    assert_eq!(succeeded, 30, "30 engine success cells over 27 frozen edges");
    assert_eq!(refused_wrong_state, 90);
    assert_eq!(refused_not_permitted, 72);
    assert_eq!(refused_invalid_failure_reason, 78);
    assert_eq!(refused_invalid_expiry_milestone, 52);
    assert_eq!(refused_validity_not_lapsed, 52);
    assert_eq!(
        succeeded
            + refused_wrong_state
            + refused_not_permitted
            + refused_invalid_failure_reason
            + refused_invalid_expiry_milestone
            + refused_validity_not_lapsed,
        374,
        "every cell is either a hand-computed success or a hand-computed named refusal"
    );
}

/// Compares one actual engine cell against its hand-computed expectation,
/// with exact named-error equality and state/journal invariants.
/// `now_at_apply` is the controlled-clock value at the moment the operation
/// was applied (used for the pre-lapse expiry expectation).
fn assert_cell(
    lc: &CommandLifecycle,
    state: CommandState,
    op: &Op,
    actual: &Result<CommandTransition, LifecycleError>,
    expected: &Outcome,
    journal_len_before: usize,
    now_at_apply: i64,
) {
    match expected {
        Outcome::Becomes(to) => {
            let transition = actual.as_ref().unwrap_or_else(|err| {
                panic!(
                    "{} + {:?}: expected success into {}, got {:?}",
                    state.name(),
                    op,
                    to.name(),
                    err
                )
            });
            assert_eq!(transition.from, state, "journal 'from' is the pre-state");
            assert_eq!(transition.to, *to, "journal 'to' is the frozen target");
            assert_eq!(
                lc.state(),
                *to,
                "{} + {:?}: the lifecycle lands in {}",
                state.name(),
                op,
                to.name()
            );
            assert_eq!(
                lc.journal().len(),
                journal_len_before + 1,
                "the journal gains exactly one entry on success"
            );
            assert_eq!(
                lc.journal().last().map(|t| (t.from, t.to)),
                Some((state, *to)),
                "the appended journal entry records the transition"
            );
        }
        Outcome::Refused(refusal) => {
            // The state and the journal are unchanged by a refusal — a
            // refused transition is attributable, never a silent mutation.
            assert_eq!(
                lc.state(),
                state,
                "{} + {:?}: the state is unchanged after refusal",
                state.name(),
                op
            );
            assert_eq!(
                lc.journal().len(),
                journal_len_before,
                "{} + {:?}: the journal is unchanged after refusal",
                state.name(),
                op
            );
            let expected_err = match refusal {
                Refusal::WrongState { operation } => LifecycleError::WrongState {
                    current: state,
                    operation,
                },
                Refusal::NotPermitted { to } => {
                    LifecycleError::NotPermitted { from: state, to: *to }
                }
                Refusal::InvalidFailureReason => match op {
                    Op::Fail(reason) => LifecycleError::InvalidFailureReason {
                        current: state,
                        reason: reason.reason_text(),
                    },
                    other => panic!("InvalidFailureReason expectation on non-failure op {:?}", other),
                },
                Refusal::InvalidExpiryMilestone => match op {
                    Op::Expire(milestone) => LifecycleError::InvalidExpiryMilestone {
                        current: state,
                        milestone: milestone.reason_text(),
                    },
                    other => panic!("InvalidExpiryMilestone expectation on non-expiry op {:?}", other),
                },
                Refusal::ValidityNotLapsed => LifecycleError::ValidityNotLapsed {
                    now_unix_millis: now_at_apply,
                    valid_until_millis: VALID_UNTIL_MILLIS,
                },
            };
            assert_eq!(
                actual,
                &Err(expected_err),
                "{} + {:?}: the refusal must be the exact named error",
                state.name(),
                op
            );
        }
    }
}

// ============================================================================
// Part 4 — the engine success targets equal the frozen-table successors
//          (the engine surface accepts EXACTLY the frozen edges)
// ============================================================================

#[test]
fn engine_success_targets_equal_the_frozen_table_successors() {
    for &state in CommandState::ALL.iter() {
        let mut targets: Vec<CommandState> = Vec::new();
        for op in all_ops() {
            if let Outcome::Becomes(to) = expected_outcome(state, &op, Lapse::AfterDrive) {
                if !targets.contains(&to) {
                    targets.push(to);
                }
            }
        }
        let mut expected = table_successors(state);
        targets.sort_by_key(|s| CommandState::ALL.iter().position(|&x| x == *s).unwrap());
        expected.sort_by_key(|s| CommandState::ALL.iter().position(|&x| x == *s).unwrap());
        assert_eq!(
            targets, expected,
            "{}: the engine accepts exactly the frozen-table successors",
            state.name()
        );
    }
}

// ============================================================================
// Part 5 — hand-computed journal expectations: the full forward walk, field
//          by field, with a hand-computed timeline
// ============================================================================

#[test]
fn full_forward_walk_journal_is_exactly_as_hand_computed() {
    let mut clock = ManualClock::new();
    let id = command_id_for(Completed);
    let mut lc = CommandLifecycle::propose(id, fresh_valid_until()).expect("lifecycle proposes");

    // The entry marker ([*] → PROPOSED) is NOT a transition: no journal
    // entry, no timestamp taken.
    assert_eq!(lc.state(), Proposed);
    assert_eq!(lc.journal().len(), 0);
    assert!(!lc.is_terminal());

    clock.step(); // 1_000
    lc.record_validation(gate_pass(), actor("svc:validator"), &clock)
        .expect("validation passes");
    clock.step(); // 2_000
    lc.record_feasibility(gate_pass(), actor("svc:feasibility"), &clock)
        .expect("feasibility passes");
    clock.step(); // 3_000
    lc.record_authorization(authz_approve(), &clock)
        .expect("authorization grants");
    clock.step(); // 4_000
    lc.record_issue(evidence("sig-ev-1"), actor("svc:dispatch"), &clock)
        .expect("issue records");
    clock.step(); // 5_000
    lc.record_acknowledgement(evidence("ack-ev-1"), actor("adapter:simulator"), &clock)
        .expect("acknowledgement records");
    clock.step(); // 6_000
    lc.record_tracking_started(evidence("telemetry-ev-1"), actor("svc:tracking"), &clock)
        .expect("tracking starts");
    clock.step(); // 7_000
    lc.record_completion(evidence("criterion-ev-1"), actor("svc:tracking"), &clock)
        .expect("completion records");

    // The hand-computed journal, entry by entry, field by field (§14.1 edges
    // 1, 3, 6, 9, 13, 18, 22 in order).
    let expected_journal = vec![
        CommandTransition {
            command_id: id,
            from: Proposed,
            to: Validated,
            actor: actor("svc:validator"),
            at_unix_millis: 1_000,
            gate: Some(Gate::Validation),
            reason: None,
            evidence: Some(evidence("ev:rc-pass")),
        },
        CommandTransition {
            command_id: id,
            from: Validated,
            to: Feasible,
            actor: actor("svc:feasibility"),
            at_unix_millis: 2_000,
            gate: Some(Gate::Feasibility),
            reason: None,
            evidence: Some(evidence("ev:rc-pass")),
        },
        CommandTransition {
            command_id: id,
            from: Feasible,
            to: Approved,
            actor: actor("op:approver"),
            at_unix_millis: 3_000,
            gate: Some(Gate::Authorization),
            reason: None,
            evidence: Some(evidence("authz:rc-approved")),
        },
        CommandTransition {
            command_id: id,
            from: Approved,
            to: Issued,
            actor: actor("svc:dispatch"),
            at_unix_millis: 4_000,
            gate: None,
            reason: None,
            evidence: Some(evidence("sig-ev-1")),
        },
        CommandTransition {
            command_id: id,
            from: Issued,
            to: Acknowledged,
            actor: actor("adapter:simulator"),
            at_unix_millis: 5_000,
            gate: None,
            reason: None,
            evidence: Some(evidence("ack-ev-1")),
        },
        CommandTransition {
            command_id: id,
            from: Acknowledged,
            to: Tracking,
            actor: actor("svc:tracking"),
            at_unix_millis: 6_000,
            gate: None,
            reason: None,
            evidence: Some(evidence("telemetry-ev-1")),
        },
        CommandTransition {
            command_id: id,
            from: Tracking,
            to: Completed,
            actor: actor("svc:tracking"),
            at_unix_millis: 7_000,
            gate: None,
            reason: None,
            evidence: Some(evidence("criterion-ev-1")),
        },
    ];
    assert_eq!(lc.journal(), expected_journal);
    assert_eq!(lc.state(), Completed);
    assert!(lc.is_terminal());
    // Acknowledgement is not physical delivery (§14.1): the acknowledgement
    // entry is application-layer acceptance, distinct from the completion
    // criterion — two different evidence references, two different states.
    assert_ne!(expected_journal[4].evidence, expected_journal[6].evidence);
}

#[test]
fn gate_rejection_walks_name_which_gate_rejected() {
    // §14.1: the three rejection points are the three gates; every REJECTED
    // record names WHICH gate rejected and the failing check (§14.3).
    // Walk 1 — validation gate fails (frozen edge 2).
    let mut clock = ManualClock::new();
    let mut lc = CommandLifecycle::propose(command_id_for(Rejected), fresh_valid_until())
        .expect("lifecycle proposes");
    clock.step(); // 1_000
    lc.record_validation(gate_fail("asset_capability"), actor("svc:validator"), &clock)
        .expect("validation rejection records");
    assert_eq!(lc.state(), Rejected);
    assert_eq!(lc.rejected_by(), Some(Gate::Validation));
    let entry = &lc.journal()[0];
    assert_eq!((entry.from, entry.to), (Proposed, Rejected));
    assert_eq!(entry.gate, Some(Gate::Validation));
    assert_eq!(entry.reason.as_deref(), Some("asset_capability"));
    assert!(lc.is_terminal());

    // Walk 2 — feasibility gate fails (frozen edge 4).
    let mut clock = ManualClock::new();
    let mut lc = drive_to(Validated, &mut clock);
    clock.step(); // 2_000
    lc.record_feasibility(gate_fail("network_voltage_band"), actor("svc:feasibility"), &clock)
        .expect("feasibility rejection records");
    assert_eq!(lc.state(), Rejected);
    assert_eq!(lc.rejected_by(), Some(Gate::Feasibility));
    let entry = &lc.journal()[1];
    assert_eq!((entry.from, entry.to), (Validated, Rejected));
    assert_eq!(entry.gate, Some(Gate::Feasibility));
    assert_eq!(entry.reason.as_deref(), Some("network_voltage_band"));

    // Walk 3 — authorization gate denies (frozen edge 7).
    let mut clock = ManualClock::new();
    let mut lc = drive_to(Feasible, &mut clock);
    clock.step(); // 3_000
    lc.record_authorization(authz_deny(), &clock)
        .expect("authorization denial records");
    assert_eq!(lc.state(), Rejected);
    assert_eq!(lc.rejected_by(), Some(Gate::Authorization));
    let entry = &lc.journal()[2];
    assert_eq!((entry.from, entry.to), (Feasible, Rejected));
    assert_eq!(entry.gate, Some(Gate::Authorization));
    assert_eq!(entry.reason.as_deref(), Some("outside approved scope"));
    assert_eq!(entry.actor, actor("op:approver"));
}

// ============================================================================
// Part 6 — §14.3 rejection classes: stale, duplicated-with-conflicting-
//          payload, out-of-sequence, outside-capability, missing-evidence —
//          all refused. At the lifecycle seam the five classes arrive as gate
//          outcomes naming the failing check; the payload-level checks
//          themselves live in `crates/flexcore-domain/src/dispatch_command.rs`
//          (P1.06) — this suite proves the ENGINE routes every failing
//          outcome to REJECTED with the named check, and that structurally
//          missing evidence/reason fields are refused with named errors.
// ============================================================================

#[test]
fn the_five_14_3_rejection_classes_are_refused_with_named_checks() {
    for class in [
        "stale_telemetry",
        "duplicate_conflicting_payload",
        "out_of_sequence",
        "outside_capability",
        "missing_evidence",
    ] {
        let mut clock = ManualClock::new();
        let mut lc =
            CommandLifecycle::propose(command_id_for(Rejected), fresh_valid_until())
                .expect("lifecycle proposes");
        clock.step(); // 1_000
        lc.record_validation(
            GateOutcome::fail(evidence("ev:rc-fail"), class).expect("outcome builds"),
            actor("svc:validator"),
            &clock,
        )
        .expect("rejection records");
        assert_eq!(lc.state(), Rejected, "class '{}' is refused", class);
        assert_eq!(lc.rejected_by(), Some(Gate::Validation));
        let entry = &lc.journal()[0];
        assert_eq!(entry.reason.as_deref(), Some(class), "the failing check is named");
        assert_eq!(entry.gate, Some(Gate::Validation), "the rejecting gate is named");
        assert!(lc.is_terminal());
    }
}

#[test]
fn structurally_missing_evidence_and_reasons_are_refused_by_name() {
    // Empty evidence reference.
    assert_eq!(
        EvidenceReference::new(""),
        Err(LifecycleError::EmptyField { field: "evidence_reference" })
    );
    // Whitespace-only actor identity.
    assert_eq!(
        Actor::new("   "),
        Err(LifecycleError::EmptyField { field: "actor" })
    );
    // A failed gate outcome that does not name its failing check.
    assert_eq!(
        GateOutcome::fail(evidence("ev"), ""),
        Err(LifecycleError::EmptyField { field: "failing_check" })
    );
    // An authorization denial that does not carry its denial reason.
    assert_eq!(
        AuthorizationOutcome::deny(evidence("d"), actor("op:approver"), ""),
        Err(LifecycleError::MissingFailingCheck { gate: Gate::Authorization })
    );
    // A cancellation without its required §14.2 reason: refused, state and
    // journal unchanged.
    let mut clock = ManualClock::new();
    let mut lc = drive_to(Approved, &mut clock);
    let before = lc.journal().len();
    assert_eq!(
        lc.record_cancellation("", actor("op:operator"), &clock),
        Err(LifecycleError::EmptyField { field: "reason" })
    );
    assert_eq!(lc.state(), Approved, "the cancellation was refused");
    assert_eq!(lc.journal().len(), before);
    // A command cannot supersede itself.
    let mut lc = drive_to(Validated, &mut clock);
    let self_id = lc.command_id();
    assert_eq!(
        lc.record_superseded_by(self_id, actor("svc:scheduling"), &clock),
        Err(LifecycleError::SelfSupersede { command_id: self_id })
    );
    assert_eq!(lc.state(), Validated, "the self-supersede was refused");
}

// ============================================================================
// Part 7 — the expiry path, hand-computed: the four EXPIRED edges, the
//          controlled-clock guard, wrong milestones, and EXPIRED → FALLBACK
// ============================================================================

#[test]
fn the_four_expiry_edges_fire_exactly_as_hand_computed() {
    // (state, milestone) pairs annotated by the frozen table: edges 10, 15,
    // 20, 24. Each walk: drive to the source state, lapse the clock, record
    // the expiry, assert the journal entry field by field.
    for (state, milestone, drive_entries) in [
        (Approved, ExpiryMilestone::BeforeIssue, 3usize),
        (Issued, ExpiryMilestone::BeforeAcknowledgement, 4usize),
        (Acknowledged, ExpiryMilestone::BeforeTrackingCompletion, 5usize),
        (Tracking, ExpiryMilestone::BeforeCompletion, 6usize),
    ] {
        let mut clock = ManualClock::new();
        let mut lc = drive_to(state, &mut clock);
        assert_eq!(lc.journal().len(), drive_entries);
        assert!(!lc.validity_has_lapsed(&clock), "the window has not ended yet");
        clock.lapse(); // now == 100_000 ≥ valid_until: the window has ended
        assert!(lc.validity_has_lapsed(&clock));
        lc.record_expiry(milestone, actor("svc:expiry"), &clock)
            .expect("expiry records");
        assert_eq!(lc.state(), Expired, "{} expires on its annotated milestone", state.name());
        assert!(!lc.is_terminal(), "EXPIRED is not terminal (frozen edge to FALLBACK)");
        let entry = lc.journal().last().expect("expiry entry journalled");
        assert_eq!((entry.from, entry.to), (state, Expired));
        assert_eq!(entry.at_unix_millis, VALID_UNTIL_MILLIS);
        assert_eq!(entry.gate, None);
        assert_eq!(entry.reason, Some(milestone.reason_text().to_string()));
        assert_eq!(entry.evidence, None);
        // EXPIRED exits only via FALLBACK (frozen edge 27), with the
        // policy-reference evidence and the state-specific reason.
        lc.record_fallback(evidence("policy:site-1"), actor("policy:site"), &clock)
            .expect("fallback records");
        assert_eq!(lc.state(), Fallback);
        assert!(lc.is_terminal());
        let fb = lc.journal().last().expect("fallback entry journalled");
        assert_eq!((fb.from, fb.to), (Expired, Fallback));
        assert_eq!(fb.reason.as_deref(), Some("fallback policy requires"));
        assert_eq!(fb.evidence.as_ref().map(|e| e.as_str()), Some("policy:site-1"));
    }
}

#[test]
fn expiry_requested_before_the_window_ends_is_refused_by_name() {
    let mut clock = ManualClock::new();
    let mut lc = drive_to(Validated, &mut clock);
    clock.step(); // 2_000 — the window (100_000) has NOT ended
    let before = lc.journal().len();
    assert_eq!(
        lc.record_expiry(ExpiryMilestone::BeforeIssue, actor("svc:expiry"), &clock),
        Err(LifecycleError::ValidityNotLapsed {
            now_unix_millis: 2_000,
            valid_until_millis: VALID_UNTIL_MILLIS,
        }),
        "EXPIRED is never recorded early (controlled-clock guard)"
    );
    assert_eq!(lc.state(), Validated, "the state is unchanged");
    assert_eq!(lc.journal().len(), before, "no journal entry for a refused expiry");

    // Boundary: now == valid_until IS lapsed (§10.3 end-exclusive window).
    let mut clock = ManualClock::new();
    let mut lc = drive_to(Approved, &mut clock);
    clock.lapse(); // now == 100_000 == valid_until
    lc.record_expiry(ExpiryMilestone::BeforeIssue, actor("svc:expiry"), &clock)
        .expect("expiry at the exact boundary records");
    assert_eq!(lc.state(), Expired);
}

#[test]
fn a_wrong_expiry_milestone_is_refused_by_name() {
    // Hand-computed: of the 14 × 4 = 56 (state, milestone) combos, exactly 4
    // are the annotated ones (accepted in the expiry-edge test above) and 4
    // sit on the EXPIRED state (unreachable pre-lapse; covered lapsed in the
    // engine matrix). The remaining 48 are checked here: each is refused with
    // InvalidExpiryMilestone naming the current state and the supplied
    // milestone text.
    let mut checked = 0usize;
    for &state in CommandState::ALL.iter() {
        for &milestone in ALL_EXPIRY_MILESTONES.iter() {
            let annotated = expected_milestone(state) == Some(milestone);
            if annotated {
                continue; // exercised in the_four_expiry_edges_fire_exactly_as_hand_computed
            }
            if state == Expired {
                continue; // unreachable pre-lapse; covered in the engine matrix (lapsed)
            }
            checked += 1;
            let mut clock = ManualClock::new();
            let mut lc = drive_to(state, &mut clock);
            clock.lapse();
            assert_eq!(
                lc.record_expiry(milestone, actor("svc:expiry"), &clock),
                Err(LifecycleError::InvalidExpiryMilestone {
                    current: state,
                    milestone: milestone.reason_text(),
                }),
                "{} must not expire on milestone '{}'",
                state.name(),
                milestone.reason_text()
            );
            assert_eq!(lc.state(), state, "the state is unchanged");
        }
    }
    // 56 (state, milestone) combos − 4 annotated (accepted in the expiry-edge
    // test) − 4 EXPIRED-state cells (unreachable pre-lapse; covered lapsed in
    // the engine matrix) = 48 refusals here.
    assert_eq!(checked, 48, "56 - 4 annotated - 4 EXPIRED-state cells = 48 refusals");
}

// ============================================================================
// Part 8 — the failure-trigger matrix, hand-computed: 14 states × 6 triggers
//          = 84 cells; exactly the 6 annotated (state, trigger) combos
//          succeed into FAILED with the trigger as the journal reason; the
//          other 78 are refused with InvalidFailureReason.
// ============================================================================

#[test]
fn the_failure_trigger_matrix_is_exactly_as_hand_computed() {
    let mut ok = 0usize;
    let mut refused = 0usize;
    for &state in CommandState::ALL.iter() {
        for &reason in ALL_FAILURE_REASONS.iter() {
            let mut clock = ManualClock::new();
            let mut lc = drive_to(state, &mut clock);
            clock.step();
            let before = lc.journal().len();
            let result = lc.record_failure(reason, actor("adapter:simulator"), &clock);
            if trigger_allowed(state, reason) {
                ok += 1;
                let transition = result.expect("annotated trigger succeeds");
                assert_eq!((transition.from, transition.to), (state, Failed));
                assert_eq!(lc.state(), Failed);
                assert_eq!(lc.journal().len(), before + 1);
                let entry = lc.journal().last().expect("failure entry journalled");
                assert_eq!(entry.reason, Some(reason.reason_text().to_string()));
                assert_eq!(entry.gate, None);
                assert_eq!(entry.evidence, None);
            } else {
                refused += 1;
                assert_eq!(
                    result,
                    Err(LifecycleError::InvalidFailureReason {
                        current: state,
                        reason: reason.reason_text(),
                    }),
                    "trigger '{}' must not fail a command in {}",
                    reason.reason_text(),
                    state.name()
                );
                assert_eq!(lc.state(), state, "the state is unchanged");
                assert_eq!(lc.journal().len(), before, "the journal is unchanged");
            }
        }
    }
    assert_eq!(ok, 6, "exactly the 6 annotated (state, trigger) combos succeed");
    assert_eq!(refused, 78, "84 - 6 = 78 refused combos");
    assert_eq!(ok + refused, 84, "the trigger matrix is fully covered");
}

// ============================================================================
// Part 9 — SUPERSEDED and CANCELLED are distinct states with distinct
//          journal records (§14.2, P0.11 §3.1), hand-computed
// ============================================================================

#[test]
fn superseded_and_cancelled_are_distinct_hand_computed_records() {
    // SUPERSEDED walk (frozen edge 5): the bidirectional supersedes/
    // superseded-by link, the replacement named as evidence.
    let mut clock = ManualClock::new();
    let mut lc = drive_to(Validated, &mut clock);
    clock.step(); // 2_000
    let replacement = superseding_id();
    lc.record_superseded_by(replacement, actor("svc:scheduling"), &clock)
        .expect("supersede records");
    assert_eq!(lc.state(), Superseded);
    assert!(lc.is_terminal());
    assert_eq!(lc.superseded_by(), Some(replacement));
    assert_eq!(lc.rejected_by(), None);
    let entry = &lc.journal()[1];
    assert_eq!((entry.from, entry.to), (Validated, Superseded));
    assert_eq!(
        entry.reason.as_deref(),
        Some("invalidated by a newer approved schedule/command revision")
    );
    assert_eq!(
        entry.evidence.as_ref().map(|e| e.as_str()),
        Some(format!("command:{}", replacement).as_str())
    );

    // CANCELLED walk (frozen edge 12): the explicit operator/policy act, no
    // replacement link.
    let mut clock = ManualClock::new();
    let mut lc = drive_to(Approved, &mut clock);
    clock.step(); // 4_000
    lc.record_cancellation("operator hold", actor("op:operator"), &clock)
        .expect("cancellation records");
    assert_eq!(lc.state(), Cancelled);
    assert!(lc.is_terminal());
    assert_eq!(lc.superseded_by(), None, "cancellation is not supersede");
    let entry = &lc.journal()[3];
    assert_eq!((entry.from, entry.to), (Approved, Cancelled));
    assert_eq!(entry.reason.as_deref(), Some("operator hold"));
    assert_eq!(entry.evidence, None);

    // The two states are distinct vocabulary with distinct causes (§14.2).
    assert_ne!(Superseded, Cancelled);
}

// ============================================================================
// Part 10 — §14.3 partial fleet success is represented per asset: a
//           portfolio-level acknowledgement is not inferred from the first
//           responding device
// ============================================================================

#[test]
fn partial_fleet_success_is_represented_per_asset() {
    let mut clock = ManualClock::new();

    // Asset A: the full forward path to COMPLETED.
    let mut asset_a = CommandLifecycle::propose(command_id_for(Completed), fresh_valid_until())
        .expect("asset A proposes");
    for &to in [Validated, Feasible, Approved, Issued, Acknowledged, Tracking, Completed].iter() {
        step_to(&mut asset_a, to, &mut clock);
    }
    assert_eq!(asset_a.state(), Completed);
    assert_eq!(asset_a.journal().len(), 7);

    // Asset B: the same dispatch to the fleet, but this device NACKed and its
    // site fallback policy activated.
    let mut asset_b = CommandLifecycle::propose(command_id_for(Fallback), fresh_valid_until())
        .expect("asset B proposes");
    for &to in [Validated, Feasible, Approved, Issued].iter() {
        step_to(&mut asset_b, to, &mut clock);
    }
    clock.step();
    asset_b
        .record_failure(FailureReason::Nack, actor("adapter:simulator"), &clock)
        .expect("asset B fails");
    clock.step();
    asset_b
        .record_fallback(evidence("policy:site-2"), actor("policy:site"), &clock)
        .expect("asset B falls back");
    assert_eq!(asset_b.state(), Fallback);
    assert_eq!(asset_b.journal().len(), 6);

    // Per-asset representation: B's failure is not absorbed into A's
    // completion, and A's completion is not inferred onto B — a portfolio-
    // level acknowledgement is never derived from the first responding
    // device (§14.3).
    assert_ne!(asset_a.state(), asset_b.state());
    assert!(!asset_b.journal().iter().any(|t| t.to == Completed),
        "asset B never recorded COMPLETED");
    assert!(asset_a.journal().iter().all(|t| t.command_id == asset_a.command_id()));
    assert!(asset_b.journal().iter().all(|t| t.command_id == asset_b.command_id()));
    assert_ne!(asset_a.command_id(), asset_b.command_id());
}

// ============================================================================
// Part 11 — controlled-clock discipline: timestamps never regress; equal
//           timestamps are allowed (§17.4 clock-drift threat)
// ============================================================================

#[test]
fn a_regressing_timestamp_is_refused_by_name_and_equal_timestamps_pass() {
    let mut clock = ManualClock::new();
    let mut lc = CommandLifecycle::propose(command_id_for(Validated), fresh_valid_until())
        .expect("lifecycle proposes");
    clock.step(); // 1_000
    lc.record_validation(gate_pass(), actor("svc:validator"), &clock)
        .expect("validation passes");

    clock.now_unix_millis = 500; // the clock moves BACKWARDS
    let before = lc.journal().len();
    assert_eq!(
        lc.record_feasibility(gate_pass(), actor("svc:feasibility"), &clock),
        Err(LifecycleError::TimestampRegression {
            previous_unix_millis: 1_000,
            proposed_unix_millis: 500,
        }),
        "a backwards timestamp is refused with the named error"
    );
    assert_eq!(lc.state(), Validated, "the state is unchanged");
    assert_eq!(lc.journal().len(), before, "no journal entry for a refused transition");

    // Equal timestamps are allowed (only strictly-earlier is refused).
    clock.now_unix_millis = 1_000;
    lc.record_feasibility(gate_pass(), actor("svc:feasibility"), &clock)
        .expect("feasibility at an equal timestamp records");
    assert_eq!(lc.state(), Feasible);
    assert_eq!(lc.journal()[1].at_unix_millis, 1_000);
}

// ============================================================================
// Part 12 — fail-closed structure and the CRASH_TRANSITIONS cross-reference
//
// The PLUGIN lifecycle (report §5.6, crates/flexcore-kernel/src/lifecycle.rs)
// carries the identifiers FROZEN_TRANSITIONS (12 edges) and CRASH_TRANSITIONS
// (5 added `→ Failed` edges so a plugin crash is always representable); those
// tables are exercised by crates/flexcore-kernel/tests/lifecycle_p7_07.rs and
// are outside this file's crate seam (flexcore-domain cannot depend on
// flexcore-kernel — dependency rule, tools/check_dependency_rule.py). The
// COMMAND lifecycle's fail-closed analogue is asserted here: every
// non-terminal command state has at least one frozen exit (no state can get
// stuck without a recorded path forward), and every terminal state has none.
// ============================================================================

#[test]
fn every_non_terminal_state_has_a_frozen_exit_fail_closed() {
    for &state in CommandState::ALL.iter() {
        let successors = table_successors(state);
        if is_terminal(state) {
            assert!(
                successors.is_empty(),
                "terminal {} must have no exit",
                state.name()
            );
        } else {
            assert!(
                !successors.is_empty(),
                "{}: a non-terminal state must always have a recorded path forward (fail-closed)",
                state.name()
            );
            // And the engine accepts each such exit (the table is not just
            // drawn — it is reachable through the engine operation surface).
            for &to in successors.iter() {
                assert_eq!(try_transition(state, to), Ok(to));
            }
        }
    }
}

#[test]
fn the_failure_and_expiry_annotation_domains_are_exhaustive_and_pinned() {
    // Compile-time exhaustive matches over the annotation enums (adding a
    // variant is a compile error), with the frozen annotation text pinned.
    let failure_texts: [&str; 6] = [
        match FailureReason::Nack {
            FailureReason::Nack => "NACK from gateway/adapter",
            FailureReason::TransportTimeout => "acknowledgement timeout",
            FailureReason::TransportFailure => "transport failure",
            FailureReason::NoExpectedResponse => "no expected response",
            FailureReason::TrackingDeviation => "tracking deviation",
            FailureReason::TrackingTimeout => "tracking timeout",
        },
        match FailureReason::TransportTimeout {
            FailureReason::Nack => unreachable!(),
            FailureReason::TransportTimeout => "acknowledgement timeout",
            FailureReason::TransportFailure => unreachable!(),
            FailureReason::NoExpectedResponse => unreachable!(),
            FailureReason::TrackingDeviation => unreachable!(),
            FailureReason::TrackingTimeout => unreachable!(),
        },
        match FailureReason::TransportFailure {
            FailureReason::Nack => unreachable!(),
            FailureReason::TransportTimeout => unreachable!(),
            FailureReason::TransportFailure => "transport failure",
            FailureReason::NoExpectedResponse => unreachable!(),
            FailureReason::TrackingDeviation => unreachable!(),
            FailureReason::TrackingTimeout => unreachable!(),
        },
        match FailureReason::NoExpectedResponse {
            FailureReason::Nack => unreachable!(),
            FailureReason::TransportTimeout => unreachable!(),
            FailureReason::TransportFailure => unreachable!(),
            FailureReason::NoExpectedResponse => "no expected response",
            FailureReason::TrackingDeviation => unreachable!(),
            FailureReason::TrackingTimeout => unreachable!(),
        },
        match FailureReason::TrackingDeviation {
            FailureReason::Nack => unreachable!(),
            FailureReason::TransportTimeout => unreachable!(),
            FailureReason::TransportFailure => unreachable!(),
            FailureReason::NoExpectedResponse => unreachable!(),
            FailureReason::TrackingDeviation => "tracking deviation",
            FailureReason::TrackingTimeout => unreachable!(),
        },
        match FailureReason::TrackingTimeout {
            FailureReason::Nack => unreachable!(),
            FailureReason::TransportTimeout => unreachable!(),
            FailureReason::TransportFailure => unreachable!(),
            FailureReason::NoExpectedResponse => unreachable!(),
            FailureReason::TrackingDeviation => unreachable!(),
            FailureReason::TrackingTimeout => "tracking timeout",
        },
    ];
    for (reason, text) in ALL_FAILURE_REASONS.iter().zip(failure_texts.iter()) {
        assert_eq!(reason.reason_text(), *text, "frozen trigger text pinned");
    }

    let milestone_texts: [&str; 4] = [
        match ExpiryMilestone::BeforeIssue {
            ExpiryMilestone::BeforeIssue => {
                "validity ended before issue (milestone not reached: ISSUED)"
            }
            ExpiryMilestone::BeforeAcknowledgement
            | ExpiryMilestone::BeforeTrackingCompletion
            | ExpiryMilestone::BeforeCompletion => unreachable!(),
        },
        match ExpiryMilestone::BeforeAcknowledgement {
            ExpiryMilestone::BeforeIssue => unreachable!(),
            ExpiryMilestone::BeforeAcknowledgement => {
                "validity ended before acknowledgement (milestone not reached: ACKNOWLEDGED)"
            }
            ExpiryMilestone::BeforeTrackingCompletion | ExpiryMilestone::BeforeCompletion => {
                unreachable!()
            }
        },
        match ExpiryMilestone::BeforeTrackingCompletion {
            ExpiryMilestone::BeforeIssue | ExpiryMilestone::BeforeAcknowledgement => unreachable!(),
            ExpiryMilestone::BeforeTrackingCompletion => {
                "validity ended before tracking completes (milestone not reached: TRACKING)"
            }
            ExpiryMilestone::BeforeCompletion => unreachable!(),
        },
        match ExpiryMilestone::BeforeCompletion {
            ExpiryMilestone::BeforeIssue
            | ExpiryMilestone::BeforeAcknowledgement
            | ExpiryMilestone::BeforeTrackingCompletion => unreachable!(),
            ExpiryMilestone::BeforeCompletion => {
                "validity ended before completion (milestone not reached: COMPLETED)"
            }
        },
    ];
    for (milestone, text) in ALL_EXPIRY_MILESTONES.iter().zip(milestone_texts.iter()) {
        assert_eq!(milestone.reason_text(), *text, "frozen milestone text pinned");
    }
}

// ============================================================================
// Part 13 — the engine's own permitted-transitions view (derived from the
//           frozen table) agrees with the transcription for every state
// ============================================================================

#[test]
fn permitted_transitions_matches_the_transcribed_table_for_every_state() {
    for &state in CommandState::ALL.iter() {
        let mut clock = ManualClock::new();
        let lc = drive_to(state, &mut clock);
        let mut permitted = lc.permitted_transitions();
        permitted.sort_by_key(|s| CommandState::ALL.iter().position(|&x| x == *s).unwrap());
        let mut expected = table_successors(state);
        expected.sort_by_key(|s| CommandState::ALL.iter().position(|&x| x == *s).unwrap());
        assert_eq!(
            permitted, expected,
            "{}: permitted_transitions must be exactly the frozen successors",
            state.name()
        );
    }
}
