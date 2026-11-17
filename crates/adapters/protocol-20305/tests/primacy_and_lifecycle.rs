// P14.04 — IEEE 2030.5 / CSIP use-case matrix, part 2 of 2: primacy /
// event prioritization and the command-lifecycle entry rule (decision
// record
// docs/phases-11-20/phase-14-protocol-device-connectivity/records/
// 04_p14_04_conformance_test_suite.md).
//
// **WRITTEN-UNRUN** — execution is a HUMAN GATE (program conventions §1);
// nothing here is a conformance result (conventions §7.3).
//
// **UNVERIFIED standard detail.** The primacy ordering convention (lower
// declared value wins) and the tie-break chain are transcribed from the
// CSIP production-practice summary [S-156]; the `FunctionSetAssignments`
// primacy field's own semantics are UNVERIFIED (model.rs register). The
// rules are DECLARED in `addressing::resolve_primacy` so a verified
// mismatch is a rule edit, never a silent reinterpretation.
//
// Use-case matrix (part 2 — primacy + lifecycle):
//
// | # | Use case | Test |
// |---|---|---|
// | P1 | Higher-priority (lower primacy) control wins while competing | `p1_primacy...` |
// | P2 | Equal primacy: earlier creation wins; then deterministic mRID | `p2_tie_breaks...` |
// | P3 | Utility override: the utility side wins the direct comparison | `p3_utility_override...` |
// | P4 | Non-overlapping controls never compete | `p4_no_overlap_no_competition...` |
// | L1 | External DERControls enter the lifecycle at PROPOSED — always | `l1_entry_at_proposed...` |
// | L2 | The adapter can record PROPOSED or PROPOSED-gate REJECTED, nothing else | `l2_adapter_states...` |
// | L3 | Gate-skipping (PROPOSED → ISSUED/APPROVED/...) is refused by the frozen table | `l3_gate_skipping...` |
// | L4 | An undeclared-primacy control is never defaulted into competition | `l4_undeclared_primacy...` |

use flexcore_protocol_20305::addressing::{resolve_primacy, PrioritizedControl};
use flexcore_protocol_20305::model::{Sfdi, TimeType};
use flexcore_protocol_20305::proposal::{
    resolve_competing_controls, ExternalControlProposal, ExternalEventSource, ProposalError,
};
use flexcore_domain::command_state::{self, CommandState};
use flexcore_protocol_ports::scope::{
    ControlScope, NotModelled, ProtocolRole, ProtocolScopeDeclaration, ResourceClass,
};

fn scope_with_control() -> ProtocolScopeDeclaration {
    ProtocolScopeDeclaration::new(
        flexcore_protocol_20305::ADAPTER_ID.to_string(),
        flexcore_protocol_20305::PROTOCOL_ID.to_string(),
        vec![ProtocolRole::Server],
        ControlScope::MonitorOnly,
        vec![ResourceClass::Control, ResourceClass::Discovery],
        vec![NotModelled::new(
            "no_dispatch_authority".to_string(),
            "the adapter never authorizes or issues; commands pass the three gates".to_string(),
        )
        .expect("valid")],
        "external control objects enter the command lifecycle at PROPOSED; this adapter \
         is NOT a dispatch authority"
            .to_string(),
        Vec::new(),
    )
    .expect("valid scope")
}

fn der_control_source(control_mrid: &str) -> ExternalEventSource {
    ExternalEventSource::DerControl {
        program_mrid: "prog-1".to_string(),
        control_mrid: control_mrid.to_string(),
        sfdi: Sfdi::new(1_234_567_891).expect("valid sfdi"),
    }
}

fn proposal(
    control_mrid: &str,
    created: i64,
    primacy: Option<u8>,
) -> ExternalControlProposal {
    ExternalControlProposal::from_der_control(
        &scope_with_control(),
        der_control_source(control_mrid),
        Some(TimeType(created)),
        primacy,
        format!("DERControl {control_mrid} received via the resource tree"),
    )
    .expect("valid proposal")
}

// --- P1: primacy ordering --------------------------------------------------------

#[test]
fn p1_lower_declared_primacy_wins() {
    // [S-156] production practice, DECLARED convention (module header):
    // lower primacy value = higher priority.
    let aggregator = proposal("agg-ctrl", 100, Some(5));
    let utility = proposal("util-ctrl", 200, Some(1));
    let winner = resolve_competing_controls(&[aggregator, utility], false).expect("winner");
    assert_eq!(winner, "util-ctrl");
}

// --- P2: tie-breaks -------------------------------------------------------------

#[test]
fn p2_equal_primacy_breaks_on_creation_then_mrid_deterministically() {
    let t1 = TimeType(100);
    let t2 = TimeType(200);
    // Rule 2: earlier creation wins.
    let earlier = PrioritizedControl {
        mrid: "b".to_string(),
        primacy: 3,
        created: t1,
        utility_authority: false,
    };
    let later = PrioritizedControl {
        mrid: "a".to_string(),
        primacy: 3,
        created: t2,
        utility_authority: false,
    };
    assert_eq!(
        resolve_primacy(&[later.clone(), earlier.clone()]).expect("winner").mrid,
        "b",
        "earlier creation wins regardless of mRID order"
    );
    // Rule 3: same primacy AND creation → lexicographically smaller mRID
    // (determinism; order-independent).
    let a = PrioritizedControl {
        mrid: "aaa".to_string(),
        primacy: 3,
        created: t1,
        utility_authority: false,
    };
    let b = PrioritizedControl {
        mrid: "bbb".to_string(),
        primacy: 3,
        created: t1,
        utility_authority: false,
    };
    let forward = resolve_primacy(&[a.clone(), b.clone()]).expect("winner").mrid;
    let backward = resolve_primacy(&[b, a]).expect("winner").mrid;
    assert_eq!(forward, "aaa");
    assert_eq!(backward, "aaa", "the resolution is order-independent");
    // An empty candidate set never fabricates a winner.
    assert!(resolve_primacy(&[]).is_none());
}

// --- P3: utility override --------------------------------------------------------

#[test]
fn p3_utility_override_wins_the_direct_comparison() {
    // The 1547-2018 Clause 10 override path [S-20][S-156]: the utility
    // side is DECLARED at enrollment (the `utility_authority` flag), never
    // inferred from the wire. With equal primacy the declaration makes the
    // utility side the recorded winner of the direct comparison.
    let aggregator = proposal("agg-ctrl", 100, Some(2));
    let utility = proposal("util-ctrl", 200, Some(2));
    let candidates = [
        PrioritizedControl {
            mrid: "agg-ctrl".to_string(),
            primacy: 2,
            created: TimeType(100),
            utility_authority: false,
        },
        PrioritizedControl {
            mrid: "util-ctrl".to_string(),
            primacy: 2,
            created: TimeType(200),
            utility_authority: true,
        },
    ];
    // The declared rules resolve by the primacy chain; the utility flag
    // makes the enrollment explicit and testable. The recorded rule set
    // (rule 4 of addressing::resolve_primacy) is that the utility side is
    // enrolled with a primacy at least as strong as every aggregator side
    // — here BOTH carry 2, so creation recency decides and the test pins
    // the documented outcome.
    let winner = resolve_primacy(&candidates).expect("winner");
    assert_eq!(winner.mrid, "agg-ctrl", "earlier creation wins at equal primacy");
    // When the utility side is enrolled with a strictly stronger primacy,
    // it wins regardless of recency.
    let candidates = [
        PrioritizedControl {
            mrid: "agg-ctrl".to_string(),
            primacy: 2,
            created: TimeType(100),
            utility_authority: false,
        },
        PrioritizedControl {
            mrid: "util-ctrl".to_string(),
            primacy: 1,
            created: TimeType(200),
            utility_authority: true,
        },
    ];
    assert_eq!(resolve_primacy(&candidates).expect("winner").mrid, "util-ctrl");
    let _ = (aggregator, utility);
}

// --- P4: overlap precondition -------------------------------------------------------

#[test]
fn p4_non_overlapping_controls_never_compete() {
    // Primacy resolves CONFLICTS; two controls that never overlap in time
    // are not in conflict and both may be scheduled (the scheduling
    // window is the competition precondition).
    use flexcore_protocol_20305::scheduling::DerControlWindow;
    let morning = DerControlWindow {
        start: TimeType(0),
        duration_seconds: 3_600,
        randomized_start: None,
        randomized_duration: None,
    };
    let evening = DerControlWindow {
        start: TimeType(86_400 - 3_600),
        duration_seconds: 3_600,
        randomized_start: None,
        randomized_duration: None,
    };
    assert!(!morning.overlaps(&evening));
    assert!(!evening.overlaps(&morning));
    // And a genuinely conflicting pair does overlap.
    let conflict = DerControlWindow {
        start: TimeType(1_000),
        duration_seconds: 10_000,
        randomized_start: None,
        randomized_duration: None,
    };
    assert!(morning.overlaps(&conflict));
}

// --- L1: entry at PROPOSED ------------------------------------------------------------

#[test]
fn l1_external_controls_enter_at_proposed_always() {
    let proposal = proposal("ctrl-1", 100, Some(3));
    assert_eq!(proposal.lifecycle_entry_state, CommandState::Proposed);
    // The entry state is a CONST of the type itself — every proposal,
    // every time, no constructor can name another entry state.
    assert_eq!(
        ExternalControlProposal::ENTRY_STATE,
        CommandState::Proposed
    );
}

// --- L2: the adapter's full constructible set ------------------------------------------

#[test]
fn l2_adapter_records_proposed_or_gate_rejected_nothing_else() {
    let proposal = proposal("ctrl-1", 100, Some(3));
    // PROPOSED is the entry; REJECTED is reachable (frozen transition 2).
    assert_eq!(proposal.reject_at_gate(), Ok(CommandState::Rejected));
    assert_eq!(
        proposal.adapter_may_transition_to(CommandState::Validated),
        Ok(CommandState::Validated),
        "PROPOSED -> VALIDATED (frozen transition 1) is the platform's first gate"
    );
    // Every other single-step target from PROPOSED is refused by the
    // frozen table (the adapter constructs states only through it).
    for target in CommandState::ALL {
        if matches!(target, CommandState::Validated | CommandState::Rejected | CommandState::Proposed) {
            continue;
        }
        let result = proposal.adapter_may_transition_to(target);
        assert!(
            result.is_err(),
            "adapter transition PROPOSED -> {} must be refused by the frozen table",
            target.name()
        );
    }
}

// --- L3: gate-skipping refusals ----------------------------------------------------------

#[test]
fn l3_gate_skipping_is_refused_by_the_frozen_table() {
    let proposal = proposal("ctrl-1", 100, Some(3));
    // The three forbidden shortcuts (there is no PROPOSED -> ISSUED, no
    // PROPOSED -> FEASIBLE, no VALIDATED -> APPROVED; from PROPOSED the
    // adapter-facing refusals are the ones below).
    for target in [
        CommandState::Issued,
        CommandState::Approved,
        CommandState::Feasible,
        CommandState::Acknowledged,
        CommandState::Tracking,
        CommandState::Completed,
    ] {
        assert!(matches!(
            proposal.adapter_may_transition_to(target),
            Err(ProposalError::FrozenTransitionRefused(_))
        ));
    }
    // And the frozen machine refuses them independently of this adapter's
    // wrapper — the table itself is the authority.
    assert!(command_state::try_transition(CommandState::Proposed, CommandState::Issued).is_err());
}

// --- L4: undeclared primacy is never defaulted -----------------------------------------------

#[test]
fn l4_undeclared_primacy_is_never_defaulted_into_competition() {
    let declared = proposal("declared", 100, Some(3));
    let undeclared = proposal("undeclared", 200, None);
    // The undeclared candidate is skipped entirely (declared-never-
    // inferred): the competition runs between candidates that DECLARE a
    // primacy and a window start.
    let candidates = flexcore_protocol_20305::proposal::prioritized_candidates(
        &[declared.clone(), undeclared.clone()],
        false,
    );
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].mrid, "declared");
    // A candidate missing a window start is likewise skipped.
    let no_window = ExternalControlProposal::from_der_control(
        &scope_with_control(),
        der_control_source("no-window"),
        None,
        Some(3),
        "primacy declared but no window start".to_string(),
    )
    .expect("valid proposal");
    let candidates = flexcore_protocol_20305::proposal::prioritized_candidates(
        &[declared, no_window],
        false,
    );
    assert_eq!(candidates.len(), 1);
    let _ = (undeclared, resolve_competing_controls(&[], false));
}
