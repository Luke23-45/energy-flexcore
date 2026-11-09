// P4.10 — Command-record persistence contract tests (report §14.1–§14.2,
//         §16.1; ledger P4.10).
//
// These are the crate-external contract tests for
// `flexcore_storage::command_store`: the closed 14-state vocabulary mirrors
// the P0.11 domain vocabulary, the journal-row constructors enforce the
// frozen-edge rule through the DOMAIN table (`try_transition` — the oracle,
// never a second encoding), the §14.1 reason/evidence/gate rules hold for
// every journal row, the SUPERSEDED/CANCELLED distinction is structural,
// and the P4.12 pre-wiring errors are named. The PostgreSQL behaviour (RLS,
// compare-and-swap transitions, immutability trigger, cross-tenant
// isolation) is covered by the adapter's DB-gated suite
// (`crates/adapters/postgres/tests/command_store.rs`).

use flexcore_contracts::ids::FlexUuid;
use flexcore_domain::command_lifecycle::{Actor, CommandTransition, EvidenceReference, Gate};
use flexcore_domain::command_state::CommandState;
use flexcore_domain::ids::{CommandId, TenantId};
use flexcore_storage::command_store::{
    CommandRecord, CommandRecordRow, CommandStoreError, CommandTransitionRecord, COMMAND_GATES,
    COMMAND_STATE_PROPOSED, COMMAND_STATE_SUPERSEDED, COMMAND_STATES,
    COMMAND_STATES_REQUIRING_EVIDENCE, COMMAND_STATES_REQUIRING_GATE,
    COMMAND_STATES_REQUIRING_REASON,
};

const UUID_TENANT: &str = "0192a5e0-0000-7000-8000-000000000041";
const UUID_COMMAND: &str = "0192a5e0-0000-7000-8000-0000000000a1";
const UUID_COMMAND_OTHER: &str = "0192a5e0-0000-7000-8000-0000000000a2";
const UUID_PORTFOLIO: &str = "0192a5e0-0000-7000-8000-0000000000b1";
const UUID_SITE: &str = "0192a5e0-0000-7000-8000-0000000000c1";
const UUID_ASSET: &str = "0192a5e0-0000-7000-8000-0000000000d1";
const UUID_SCHEDULE: &str = "0192a5e0-0000-7000-8000-0000000000e1";

fn tenant() -> TenantId {
    TenantId::new(FlexUuid::parse(UUID_TENANT).expect("tenant uuid parses"))
}

fn command_id() -> CommandId {
    CommandId::new(FlexUuid::parse(UUID_COMMAND).expect("command uuid parses"))
}

fn base_row() -> CommandRecordRow {
    CommandRecordRow::new(
        tenant(),
        UUID_COMMAND.to_string(),
        UUID_PORTFOLIO.to_string(),
        UUID_SITE.to_string(),
        UUID_ASSET.to_string(),
        UUID_SCHEDULE.to_string(),
        2,
        1_700_000_100_000,
        None,
        "{\"target\":{\"p_mw\":1.5}}".to_string(),
    )
    .expect("base row builds")
}

fn transition(from: CommandState, to: CommandState) -> CommandTransition {
    CommandTransition {
        command_id: command_id(),
        from,
        to,
        actor: Actor::new("svc:lifecycle-test").expect("actor builds"),
        at_unix_millis: 1_700_000_000_000,
        gate: None,
        reason: None,
        evidence: None,
    }
}

#[test]
fn the_stored_state_vocabulary_is_the_frozen_14_state_vocabulary() {
    // The storage vocabulary is derived from the P0.11 freeze by construction:
    // every stored name is the canonical name of a domain state, in
    // `CommandState::ALL` order, and none is missing.
    assert_eq!(COMMAND_STATES.len(), CommandState::ALL.len());
    for (stored, state) in COMMAND_STATES.iter().zip(CommandState::ALL.iter()) {
        assert_eq!(stored, &state.name());
    }
    assert_eq!(COMMAND_GATES, ["validation", "feasibility", "authorization"]);
    assert_eq!(COMMAND_STATE_PROPOSED, "PROPOSED");
    assert_eq!(COMMAND_STATE_SUPERSEDED, "SUPERSEDED");
}

#[test]
fn the_reason_evidence_and_gate_rule_sets_match_the_frozen_annotations() {
    // Evidence-carrying entries: the three gate passes, the four evidence
    // steps past the gates, the supersede reference, the fallback policy.
    assert_eq!(
        COMMAND_STATES_REQUIRING_EVIDENCE,
        [
            "VALIDATED",
            "FEASIBLE",
            "APPROVED",
            "ISSUED",
            "ACKNOWLEDGED",
            "TRACKING",
            "COMPLETED",
            "SUPERSEDED",
            "FALLBACK",
        ]
    );
    // Reason-carrying entries: the failing check, the failure trigger, the
    // milestone not reached, the cancellation reason, the fallback policy.
    assert_eq!(
        COMMAND_STATES_REQUIRING_REASON,
        ["REJECTED", "FAILED", "EXPIRED", "CANCELLED", "FALLBACK"]
    );
    // Gate-naming entries: the three gate passes plus their rejection point.
    assert_eq!(COMMAND_STATES_REQUIRING_GATE, ["VALIDATED", "FEASIBLE", "APPROVED", "REJECTED"]);
}

#[test]
fn a_command_row_round_trips_its_identity_and_schedule_reference() {
    let row = base_row();
    assert_eq!(row.command_id, UUID_COMMAND);
    assert_eq!(row.schedule_id, UUID_SCHEDULE);
    assert_eq!(row.schedule_revision, 2);
    assert_eq!(row.tenant_id, tenant());
    assert_eq!(row.valid_until_millis, 1_700_000_100_000);
    // The forward supersede link is optional and validated.
    let replacing = CommandRecordRow::new(
        row.tenant_id,
        UUID_COMMAND_OTHER.to_string(),
        row.portfolio_id.clone(),
        row.site_id.clone(),
        row.asset_id.clone(),
        row.schedule_id.clone(),
        row.schedule_revision,
        row.valid_until_millis,
        Some(UUID_COMMAND.to_string()),
        row.command_payload_json.clone(),
    )
    .expect("replacing row builds");
    assert_eq!(replacing.supersedes_command_id.as_deref(), Some(UUID_COMMAND));
}

#[test]
fn malformed_command_rows_are_rejected_with_the_field_named() {
    let row = base_row();
    assert_eq!(
        CommandRecordRow::new(
            row.tenant_id,
            "".to_string(),
            row.portfolio_id.clone(),
            row.site_id.clone(),
            row.asset_id.clone(),
            row.schedule_id.clone(),
            row.schedule_revision,
            row.valid_until_millis,
            None,
            row.command_payload_json.clone(),
        ),
        Err(CommandStoreError::InvalidRecord { field: "command_id" })
    );
    assert_eq!(
        CommandRecordRow::new(
            row.tenant_id,
            row.command_id.clone(),
            row.portfolio_id.clone(),
            row.site_id.clone(),
            row.asset_id.clone(),
            row.schedule_id.clone(),
            0,
            row.valid_until_millis,
            None,
            row.command_payload_json.clone(),
        ),
        Err(CommandStoreError::InvalidRecord { field: "schedule_revision" })
    );
    assert_eq!(
        CommandRecordRow::new(
            row.tenant_id,
            row.command_id.clone(),
            row.portfolio_id.clone(),
            row.site_id.clone(),
            row.asset_id.clone(),
            row.schedule_id.clone(),
            row.schedule_revision,
            row.valid_until_millis,
            None,
            "  ".to_string(),
        ),
        Err(CommandStoreError::InvalidRecord { field: "command_payload_json" })
    );
}

#[test]
fn a_command_record_enforces_the_supersede_state_link() {
    let record =
        CommandRecord::new(base_row(), COMMAND_STATE_PROPOSED.to_string(), None)
            .expect("proposed record builds");
    assert!(!record.is_superseded());
    let superseded = CommandRecord::new(
        base_row(),
        COMMAND_STATE_SUPERSEDED.to_string(),
        Some(UUID_COMMAND_OTHER.to_string()),
    )
    .expect("superseded record builds");
    assert!(superseded.is_superseded());
    assert_eq!(superseded.superseded_by_command_id.as_deref(), Some(UUID_COMMAND_OTHER));
    // The link is present EXACTLY when the state is SUPERSEDED, and never
    // self-referential; unknown states are rejected.
    assert_eq!(
        CommandRecord::new(base_row(), COMMAND_STATE_SUPERSEDED.to_string(), None),
        Err(CommandStoreError::InvalidRecord { field: "superseded_by_command_id" })
    );
    assert_eq!(
        CommandRecord::new(
            base_row(),
            COMMAND_STATE_PROPOSED.to_string(),
            Some(UUID_COMMAND_OTHER.to_string()),
        ),
        Err(CommandStoreError::InvalidRecord { field: "superseded_by_command_id" })
    );
    assert_eq!(
        CommandRecord::new(
            base_row(),
            COMMAND_STATE_SUPERSEDED.to_string(),
            Some(UUID_COMMAND.to_string()),
        ),
        Err(CommandStoreError::InvalidRecord { field: "superseded_by_command_id" })
    );
    assert_eq!(
        CommandRecord::new(base_row(), "FALLBAK".to_string(), None),
        Err(CommandStoreError::InvalidRecord { field: "state" })
    );
}

/// Builds a fully-populated journal record (gate pass shape) for `from`→`to`.
fn populated(from: CommandState, to: CommandState) -> CommandTransition {
    CommandTransition {
        command_id: command_id(),
        from,
        to,
        actor: Actor::new("svc:lifecycle-test").expect("actor builds"),
        at_unix_millis: 1_700_000_000_000,
        gate: None,
        reason: Some("recorded reason".to_string()),
        evidence: Some(EvidenceReference::new("evidence:ref-1").expect("evidence builds")),
    }
}

#[test]
fn journal_records_validate_the_frozen_edge_through_the_domain_table() {
    // A frozen edge builds; a shortcut edge is rejected with the named
    // error BEFORE anything is written (the P0.11 table is the oracle —
    // this contract never re-encodes it).
    let record = CommandTransitionRecord::from_transition(
        tenant(),
        &populated(CommandState::Proposed, CommandState::Validated),
    )
    .expect("frozen edge builds");
    assert_eq!(record.from_state, "PROPOSED");
    assert_eq!(record.to_state, "VALIDATED");
    assert_eq!(record.command_id, UUID_COMMAND);
    assert!(matches!(
        CommandTransitionRecord::from_transition(
            tenant(),
            &populated(CommandState::Proposed, CommandState::Approved)
        ),
        Err(CommandStoreError::IllegalTransition { .. })
    ));
    assert!(matches!(
        CommandTransitionRecord::from_transition(
            tenant(),
            &populated(CommandState::Completed, CommandState::Tracking)
        ),
        Err(CommandStoreError::IllegalTransition { .. })
    ));
}

#[test]
fn journal_records_enforce_the_gate_evidence_and_reason_rules() {
    // Gate rule: a gate pass names its gate; a non-gate transition carries
    // none.
    let mut gate_pass = populated(CommandState::Proposed, CommandState::Validated);
    gate_pass.gate = Some(Gate::Validation);
    let record = CommandTransitionRecord::from_transition(tenant(), &gate_pass)
        .expect("gate pass builds");
    assert_eq!(record.gate.as_deref(), Some("validation"));
    assert_eq!(
        CommandTransitionRecord::from_transition(
            tenant(),
            &populated(CommandState::Proposed, CommandState::Validated)
        ),
        Err(CommandStoreError::InvalidRecord { field: "gate" })
    );
    let mut ungated = populated(CommandState::Approved, CommandState::Issued);
    ungated.gate = Some(Gate::Validation);
    assert_eq!(
        CommandTransitionRecord::from_transition(tenant(), &ungated),
        Err(CommandStoreError::InvalidRecord { field: "gate" })
    );
    // Evidence rule: evidence-bearing entries require it; others forbid it.
    let issue = populated(CommandState::Approved, CommandState::Issued);
    let record = CommandTransitionRecord::from_transition(tenant(), &issue)
        .expect("issue builds");
    assert_eq!(record.evidence_reference.as_deref(), Some("evidence:ref-1"));
    assert!(record.reason.is_none(), "ISSUED carries no reason");
    let mut failure = populated(CommandState::Issued, CommandState::Failed);
    failure.evidence = None;
    let record = CommandTransitionRecord::from_transition(tenant(), &failure)
        .expect("failure builds");
    assert_eq!(record.reason.as_deref(), Some("recorded reason"));
    assert!(record.evidence_reference.is_none(), "FAILED carries no evidence");
    // Missing required fields are named errors.
    assert_eq!(
        CommandTransitionRecord::from_transition(
            tenant(),
            &transition(CommandState::Approved, CommandState::Expired)
        ),
        Err(CommandStoreError::InvalidRecord { field: "reason" })
    );
    assert_eq!(
        CommandTransitionRecord::from_transition(
            tenant(),
            &transition(CommandState::Tracking, CommandState::Completed)
        ),
        Err(CommandStoreError::InvalidRecord { field: "evidence_reference" })
    );
    assert_eq!(
        CommandTransitionRecord::from_transition(
            tenant(),
            &transition(CommandState::Feasible, CommandState::Rejected)
        ),
        Err(CommandStoreError::InvalidRecord { field: "gate" })
    );
    assert_eq!(
        CommandTransitionRecord::from_transition(
            tenant(),
            &transition(CommandState::Approved, CommandState::Cancelled)
        ),
        Err(CommandStoreError::InvalidRecord { field: "reason" })
    );
    assert_eq!(
        CommandTransitionRecord::from_transition(
            tenant(),
            &transition(CommandState::Validated, CommandState::Superseded)
        ),
        Err(CommandStoreError::InvalidRecord { field: "evidence_reference" })
    );
}

#[test]
fn empty_actor_identities_are_named_errors_at_the_value_level() {
    // The Actor value type rejects empty identities at construction, so an
    // empty-actor journal row cannot be built through the public API — the
    // store's `actor` field check is defence in depth against adapters that
    // bypass the constructor.
    assert!(Actor::new("").is_err());
    assert!(Actor::new("   ").is_err());
    assert!(EvidenceReference::new("").is_err());
}

#[test]
fn the_error_messages_are_named_and_stable() {
    let error = CommandStoreError::ScheduleRevisionSuperseded {
        schedule_id: UUID_SCHEDULE.to_string(),
        revision: 2,
    };
    assert!(error.to_string().contains("SUPERSEDED"));
    assert!(error.to_string().contains(UUID_SCHEDULE));
    let unknown = CommandStoreError::ScheduleRevisionUnknown {
        schedule_id: UUID_SCHEDULE.to_string(),
        revision: 9,
    };
    assert!(unknown.to_string().contains("never usable"));
    let conflict = CommandStoreError::TransitionConflict {
        command_id: UUID_COMMAND.to_string(),
        expected_from: "APPROVED".to_string(),
        actual_state: "ISSUED".to_string(),
    };
    assert!(conflict.to_string().contains("APPROVED"));
    assert!(conflict.to_string().contains("ISSUED"));
}
