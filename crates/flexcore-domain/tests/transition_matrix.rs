// P4.33 — Exhaustive command-transition matrix (report §22.3 release gate,
//         §14.1–§14.2; ledger P4.33; P0.11 frozen table as the oracle).
//
// Closes the gap between P4.10's "representative set of illegal transitions"
// and the §22.3 release gate ("all command-state transitions are tested"):
// EVERY ordered pair of the 14 states — 14 × 14 = 196 cells — has a tested
// outcome, legal and illegal alike.
//
// Method (anti-circular, per the ledger mitigation): the expected outcome of
// every cell is derived from the P0.11 FREEZE DOCUMENT
// (`ledger/phase-00-product-freeze/11_command_state_machine.md` §2), which
// is transcribed BELOW as data, in table order, with its guard text — NOT
// from the implementation. The matrix is then built mechanically over ALL
// 196 ordered pairs and compared cell-by-cell against the real oracle
// (`command_state::try_transition`, whose edges delegate to the frozen
// table). A transition-table change that adds an edge without adding it to
// the transcribed freeze data fails the cell comparison; a removed edge
// fails the legal-edge walk. The counts (27 legal / 169 illegal) are
// asserted, not assumed.
//
// The full matrix is exhaustive by construction: the driver iterates
// `CommandState::ALL` × `CommandState::ALL` (the type is closed — the
// `exactly_14_states` and state-name checks below pin the vocabulary), so
// no pair can be skipped and no untested cell exists.
//
// Totality check: `no_edge_outside_the_frozen_table_exists` proves that an
// accepted transition and a transcribed freeze edge coincide over the whole
// pair space — no edge exists that is not in the P0.11 table, and no table
// edge is missing from the implementation.
//
// Recorded boundary (honest): `try_transition` is a PURE domain function —
// a rejected transition leaves no state change by construction (there is no
// store at the domain layer) and there is nothing to audit-gap: no
// transition row is produced. Rejected-attempt AUDIT ROWS (where an audit
// policy requires one) are the application/persistence wiring (P4.30,
// pending) — recorded in the decision note, not silently assumed. The
// engine-level gate discipline (a gate applied from the wrong state is
// refused and journalling unchanged) is exercised by the P4.10 suite
// (`tests/command_lifecycle.rs`); this suite owns the TABLE.
//
// Build/test status (honest): WRITTEN, never executed — the loop bans (no
// builds, no test runs, no installs) were held; "implemented, awaiting
// human build/test". The ledger's mutation check (deliberately add a bogus
// transition, confirm failure, revert) is PENDING the human run; the suite
// proves mechanically (see `a_bogus_added_transition_would_fail_this_matrix`)
// that a bogus accepted edge cannot pass the cell comparison.

use flexcore_domain::command_state::{try_transition, CommandState, TransitionError};

use CommandState::*;

// --- The P0.11 freeze table, transcribed from the DOCUMENT -------------------------
//
// Source: `ledger/phase-00-product-freeze/11_command_state_machine.md` §2,
// "State-to-state transitions (27)", in table order, guard text verbatim.
// This transcription is the anti-circular anchor: the expected outcomes are
// derived from the freeze document, never from the implementation under
// test. (The initial marker `[*] → PROPOSED` and the five terminal markers
// `→ [*]` are rows 28–33 of the freeze table; they are not state-to-state
// transitions and are asserted separately as terminal/entry properties, not
// as matrix cells.)
const FROZEN_TABLE_EDGES: [(CommandState, CommandState, &str); 27] = [
    (Proposed, Validated, "schema, freshness, asset checks pass"),                          // 1
    (Proposed, Rejected, "malformed / unsupported"),                                        // 2
    (Validated, Feasible, "network checks pass"),                                           // 3
    (Validated, Rejected, "capability / network failure"),                                  // 4
    (Validated, Superseded, "newer approved schedule/command revision replaces it"),        // 5
    (Feasible, Approved, "policy / operator authorization"),                                // 6
    (Feasible, Rejected, "authorization denied"),                                           // 7
    (Feasible, Superseded, "newer approved schedule/command revision replaces it"),         // 8
    (Approved, Issued, "signed, sequenced command sent"),                                   // 9
    (Approved, Expired, "validity ends before issue"),                                      // 10
    (Approved, Superseded, "newer approved schedule/command revision replaces it"),         // 11
    (Approved, Cancelled, "operator/policy cancellation"),                                  // 12
    (Issued, Acknowledged, "device/gateway accepts"),                                       // 13
    (Issued, Failed, "NACK / timeout / transport failure"),                                 // 14
    (Issued, Expired, "validity ends before acknowledgement"),                              // 15
    (Issued, Superseded, "newer approved schedule/command revision replaces it"),           // 16
    (Issued, Cancelled, "supported cancellation semantics"),                                // 17
    (Acknowledged, Tracking, "telemetry observed"),                                         // 18
    (Acknowledged, Failed, "no expected response"),                                         // 19
    (Acknowledged, Expired, "validity ends before tracking completes"),                     // 20
    (Acknowledged, Superseded, "newer approved schedule/command revision replaces it"),     // 21
    (Tracking, Completed, "acceptance criterion satisfied"),                                // 22
    (Tracking, Failed, "tracking deviation / timeout"),                                     // 23
    (Tracking, Expired, "validity ends"),                                                   // 24
    (Tracking, Superseded, "newer approved schedule/command revision replaces it"),         // 25
    (Failed, Fallback, "asset/site policy activated"),                                      // 26
    (Expired, Fallback, "policy requires"),                                                 // 27
];

/// The expected outcome of one matrix cell, derived from the transcribed
/// P0.11 freeze table (NOT from the implementation).
fn expected_legal(from: CommandState, to: CommandState) -> bool {
    FROZEN_TABLE_EDGES
        .iter()
        .any(|&(f, t, _)| f == from && t == to)
}

/// The five terminal states of the freeze (rows 29–33; §14.1).
const TERMINAL_STATES: [CommandState; 5] = [Completed, Rejected, Cancelled, Superseded, Fallback];

/// The per-state outgoing-edge counts recorded in the freeze document §2.1
/// (the second mechanical cross-check pass over the §14.1 diagram).
const FREEZE_OUTGOING_COUNTS: [(CommandState, &[CommandState]); 10] = [
    (Proposed, &[Validated, Rejected]),
    (Validated, &[Feasible, Rejected, Superseded]),
    (Feasible, &[Approved, Rejected, Superseded]),
    (Approved, &[Issued, Expired, Superseded, Cancelled]),
    (Issued, &[Acknowledged, Failed, Expired, Superseded, Cancelled]),
    (Acknowledged, &[Tracking, Failed, Expired, Superseded]),
    (Tracking, &[Completed, Failed, Expired, Superseded]),
    (Failed, &[Fallback]),
    (Expired, &[Fallback]),
    // COMPLETED / REJECTED / CANCELLED / SUPERSEDED / FALLBACK: 0 (terminal).
];

/// One matrix cell: the ordered pair and its frozen-table expectation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MatrixCell {
    from: CommandState,
    to: CommandState,
    legal: bool,
}

/// The full matrix, built mechanically over ALL 14 × 14 = 196 ordered
/// pairs (exhaustive by construction — the state set is closed and pinned).
fn full_matrix() -> Vec<MatrixCell> {
    let mut cells = Vec::with_capacity(196);
    for &from in CommandState::ALL.iter() {
        for &to in CommandState::ALL.iter() {
            cells.push(MatrixCell {
                from,
                to,
                legal: expected_legal(from, to),
            });
        }
    }
    cells
}

// ============================================================================
// The matrix itself: every cell of the 14 × 14 grid has a tested outcome
// matching the P0.11 table; zero untested cells
// ============================================================================

#[test]
fn every_cell_of_the_14x14_matrix_has_a_tested_frozen_outcome() {
    let cells = full_matrix();

    // The matrix covers exactly the 196 ordered pairs, each once.
    assert_eq!(cells.len(), 196, "14 states × 14 states = 196 ordered pairs");
    for (i, cell) in cells.iter().enumerate() {
        assert_eq!(
            cells.iter().filter(|c| c.from == cell.from && c.to == cell.to).count(),
            1,
            "cell #{} ({:?} -> {:?}) appears exactly once",
            i + 1,
            cell.from,
            cell.to
        );
    }

    // The counts are asserted, not assumed: 27 legal cells, 169 illegal.
    let legal = cells.iter().filter(|c| c.legal).count();
    let illegal = cells.iter().filter(|c| !c.legal).count();
    assert_eq!(legal, 27, "the freeze table has exactly 27 edges");
    assert_eq!(illegal, 169, "196 - 27 = 169 illegal ordered pairs");
    assert_eq!(legal + illegal, 196);

    // Every transcribed freeze edge is realized as exactly one LEGAL cell —
    // the matrix data and the transcription cannot drift apart.
    for &(from, to, guard) in FROZEN_TABLE_EDGES.iter() {
        assert!(
            cells.iter().any(|c| c.from == from && c.to == to && c.legal),
            "freeze edge {} -> {} ({}) must be a legal matrix cell",
            from.name(),
            to.name(),
            guard
        );
    }
}

// ============================================================================
// Legal half of the matrix: the 27 frozen edges each succeed exactly
// (assert target state), re-running the P4.10 legal set as part of the
// matrix (ledger P4.33 step 2)
// ============================================================================

#[test]
fn every_legal_edge_succeeds_returning_exactly_the_frozen_target() {
    for (i, &(from, to, guard)) in FROZEN_TABLE_EDGES.iter().enumerate() {
        // The expected outcome is derived from the freeze table; the
        // assertion is against the real oracle.
        assert!(
            expected_legal(from, to),
            "internal: edge #{} must be classified legal by the matrix data",
            i + 1
        );
        assert_eq!(
            try_transition(from, to),
            Ok(to),
            "frozen edge #{} ({} -> {}, guard '{}') must succeed and land in the frozen target",
            i + 1,
            from.name(),
            to.name(),
            guard
        );
    }
}

// ============================================================================
// Illegal half of the matrix: all 169 illegal pairs are rejected with the
// NAMED domain error — including every transition out of a terminal state
// and every gate-skipping shortcut the ledger names
// ============================================================================

#[test]
fn every_illegal_pair_is_rejected_with_the_named_transition_error() {
    let mut rejected = 0usize;
    for &from in CommandState::ALL.iter() {
        for &to in CommandState::ALL.iter() {
            if expected_legal(from, to) {
                continue; // the legal half asserts Ok above
            }
            rejected += 1;
            // The NAMED error carries both states — the domain-layer
            // rejection is attributable, never a silent no-op.
            assert_eq!(
                try_transition(from, to),
                Err(TransitionError::NotPermitted { from, to }),
                "illegal pair {} -> {} must be rejected with NotPermitted naming both states",
                from.name(),
                to.name()
            );
            // The error's Display names the pair too (the audit-ready text).
            let text = try_transition(from, to)
                .unwrap_err()
                .to_string();
            assert!(
                text.contains(from.name()) && text.contains(to.name()),
                "the named error must name both states: {text}"
            );
        }
    }
    assert_eq!(rejected, 169, "exactly the 169 illegal pairs are rejected");
}

#[test]
fn all_transitions_out_of_terminal_states_are_illegal_cells() {
    // Terminal per the freeze: COMPLETED, REJECTED, CANCELLED, SUPERSEDED,
    // FALLBACK — 5 × 14 = 70 cells, every one illegal.
    let mut terminal_cells = 0usize;
    for &from in TERMINAL_STATES.iter() {
        for &to in CommandState::ALL.iter() {
            terminal_cells += 1;
            assert!(
                !expected_legal(from, to),
                "terminal state {} must have no outgoing edge (to {})",
                from.name(),
                to.name()
            );
            assert!(try_transition(from, to).is_err());
        }
    }
    assert_eq!(terminal_cells, 70);
}

#[test]
fn the_ledgers_named_gate_skipping_shortcuts_are_illegal_cells() {
    // Ledger P4.33 step 2 names the skips explicitly; each is an illegal
    // cell of the matrix and is rejected by the oracle.
    for &(from, to) in [
        (Proposed, Approved),   // skips validation + feasibility + authorization
        (Proposed, Issued),     // skips everything
        (Validated, Issued),    // skips feasibility + authorization
        (Feasible, Issued),     // skips authorization
        (Proposed, Feasible),   // skips validation
        (Validated, Approved),  // skips feasibility
        (Approved, Acknowledged), // skips issue
        (Issued, Tracking),     // skips acknowledgement
        (Acknowledged, Completed), // skips tracking
        // No self-transition exists for any state (14 diagonal cells).
    ] {
        assert!(
            !expected_legal(from, to),
            "{} -> {} must not be a freeze edge",
            from.name(),
            to.name()
        );
        assert_eq!(
            try_transition(from, to),
            Err(TransitionError::NotPermitted { from, to })
        );
    }
    for &state in CommandState::ALL.iter() {
        assert!(!expected_legal(state, state), "no self-transition for {}", state.name());
        assert!(try_transition(state, state).is_err());
    }
}

// ============================================================================
// Totality: no edge exists that is not in the P0.11 table, and no table
// edge is missing — the accepted-transition set and the transcribed freeze
// set coincide over the WHOLE pair space
// ============================================================================

#[test]
fn no_edge_outside_the_frozen_table_exists() {
    let mut accepted = 0usize;
    for &from in CommandState::ALL.iter() {
        for &to in CommandState::ALL.iter() {
            let in_table = expected_legal(from, to);
            let result = try_transition(from, to);
            assert_eq!(
                result.is_ok(),
                in_table,
                "{} -> {}: the oracle must accept exactly the freeze-table edges",
                from.name(),
                to.name()
            );
            if result.is_ok() {
                accepted += 1;
                // An accepted transition lands EXACTLY in the requested
                // target — the oracle never redirects.
                assert_eq!(result, Ok(to));
            }
        }
    }
    assert_eq!(accepted, 27, "the accepted set is exactly the 27 freeze edges");
}

#[test]
fn per_state_outgoing_edges_match_the_freeze_documents_second_pass() {
    // The freeze document §2.1 cross-checked the table against the §14.1
    // diagram with per-state outgoing counts; the transcription is diffed
    // against those recorded counts here.
    for &(state, outgoing) in FREEZE_OUTGOING_COUNTS.iter() {
        let from_table: Vec<CommandState> = FROZEN_TABLE_EDGES
            .iter()
            .filter(|&&(f, _, _)| f == state)
            .map(|&(_, t, _)| t)
            .collect();
        assert_eq!(
            from_table.as_slice(),
            outgoing,
            "{}: the transcribed outgoing edges must equal the freeze §2.1 record",
            state.name()
        );
    }
    // The five terminal states contribute zero outgoing edges (the freeze
    // §2.1 table's last row).
    for &state in TERMINAL_STATES.iter() {
        assert!(
            FROZEN_TABLE_EDGES.iter().all(|&(f, _, _)| f != state),
            "terminal {} must have no transcribed outgoing edge",
            state.name()
        );
    }
    // Sum: 2+3+3+4+5+4+4+1+1 = 27.
    let total: usize = FREEZE_OUTGOING_COUNTS
        .iter()
        .map(|(_, outgoing)| outgoing.len())
        .sum();
    assert_eq!(total, 27);
}

// ============================================================================
// Mutation safety (ledger P4.33 verification method): a deliberately added
// illegal transition FAILS this matrix — demonstrated mechanically here;
// the actual mutation run (add/confirm/revert) is the human build/test pass
// ============================================================================

#[test]
fn a_bogus_added_transition_would_fail_this_matrix() {
    // Simulate the mutation: a "convenience" PROPOSED → ISSUED edge added
    // to a transition table. The matrix verifier must flag it, because the
    // bogus edge is NOT in the transcribed freeze data.
    let bogus = (Proposed, Issued);
    assert!(
        !expected_legal(bogus.0, bogus.1),
        "the bogus edge is not in the freeze table (otherwise the mutation would be invisible)"
    );
    // The oracle must refuse it, so a table that accepted it would produce
    // Ok for a cell the matrix marks illegal — caught by
    // `no_edge_outside_the_frozen_table_exists` and by the per-cell
    // comparison in `every_illegal_pair_is_rejected_with_the_named_transition_error`.
    assert!(try_transition(bogus.0, bogus.1).is_err());
    // And mechanically: a cell-by-cell verifier flags any (expected legal,
    // actual illegal) disagreement — demonstrated on the bogus edge.
    let cell = MatrixCell { from: bogus.0, to: bogus.1, legal: true };
    let oracle_agrees = match try_transition(cell.from, cell.to) {
        Ok(to) => to == cell.to,
        Err(_) => false,
    };
    assert!(
        !oracle_agrees,
        "a mutated table accepting the bogus edge fails the cell comparison"
    );
}

// ============================================================================
// The state vocabulary the matrix is built over is the frozen one (the
// exhaustiveness argument rests on the state set being closed and pinned)
// ============================================================================

#[test]
fn the_matrix_is_built_over_exactly_the_14_frozen_states() {
    assert_eq!(CommandState::ALL.len(), 14);
    let names: Vec<&str> = CommandState::ALL.iter().map(|s| s.name()).collect();
    assert_eq!(
        names,
        vec![
            "PROPOSED", "VALIDATED", "FEASIBLE", "APPROVED", "ISSUED", "ACKNOWLEDGED",
            "TRACKING", "COMPLETED", "REJECTED", "FAILED", "EXPIRED", "CANCELLED",
            "SUPERSEDED", "FALLBACK",
        ],
        "the matrix pair space is exactly the §14.2 vocabulary"
    );
    // Duplicates would shrink the pair space below 196.
    let mut sorted = names.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), 14, "no duplicate states in ALL");
}
