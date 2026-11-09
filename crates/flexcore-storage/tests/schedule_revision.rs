// P3.22 — Schedule identity/revision/supersede tests (report §12.6, §14.2;
//         ledger P3.22).
//
// The storage-layer chain semantics, without a database: the closed
// vocabularies, the exact gapless chain rule, the bidirectional supersede
// link, the record-state consistency, and the Phase 4 gate check surface.
// The DATABASE-level enforcement (immutable INSERT-only rows, the dedicated
// SUPERSEDED transition, the trigger, the partial unique index) is proven by
// the DB-gated adapter suite
// (`crates/adapters/postgres/tests/schedule_revisions.rs`).
//
// These tests document the shape Phase 4 consumes: the
// `is_revision_superseded` semantics (None = unknown, Some(true) =
// SUPERSEDED, Some(false) = active) are asserted against the record type
// here; the trait method is the persistence-backed form of the same check.

use flexcore_contracts::ids::FlexUuid;
use flexcore_domain::ids::TenantId;
use flexcore_storage::schedule_revision::{
    ScheduleRevisionError, ScheduleRevisionRecord, ScheduleRevisionRow,
    ScheduleSupersedeTarget, PHASE4_GATE_NOTE, REVISION_IDENTITY_STATEMENT,
    SCHEDULE_REVISION_STATES, SCHEDULE_REVISION_STATE_ACTIVE,
    SCHEDULE_REVISION_STATE_SUPERSEDED, SCHEDULE_ROLL_TRIGGERS,
    SUPERSEDE_TRANSITION_STATEMENT,
};

const SCHEDULE_ID: &str = "0192a5e0-0000-7000-8000-00000000000a";

fn tenant() -> TenantId {
    let mut bytes = [0u8; 16];
    bytes[15] = 3;
    TenantId::new(FlexUuid::from_bytes(bytes))
}

fn row(revision: u64, trigger: &str) -> ScheduleRevisionRow {
    let candidate = format!("0192a5e0-0000-7000-8000-0000000{revision:05x}");
    let (supersedes, supersedes_candidate) = if revision == 1 {
        (None, None)
    } else {
        (
            Some(revision - 1),
            Some(format!("0192a5e0-0000-7000-8000-0000000{:05x}", revision - 1)),
        )
    };
    ScheduleRevisionRow::new(
        tenant(),
        "0192a5e0-0000-7000-8000-000000000001".to_string(),
        "0192a5e0-0000-7000-8000-000000000002".to_string(),
        SCHEDULE_ID.to_string(),
        revision,
        1_700_000_000_000,
        1_700_008_640_000,
        trigger.to_string(),
        supersedes,
        supersedes_candidate,
        candidate,
        format!("{{\"revision\":{revision}}}"),
    )
    .expect("the revision row builds")
}

#[test]
fn the_vocabularies_are_closed_and_the_statements_name_the_scheme() {
    assert_eq!(SCHEDULE_REVISION_STATES, ["active", "superseded"]);
    // The trigger set is exactly the P3.24 trigger set + initial_solve (the
    // application layer's typed set must carry the SAME strings — pinned in
    // both crates).
    assert_eq!(
        SCHEDULE_ROLL_TRIGGERS,
        [
            "initial_solve",
            "horizon_advance",
            "envelope_invalidation",
            "asset_opt_out_dropout",
            "forecast_update",
            "operator_request",
        ]
    );
    assert!(REVISION_IDENTITY_STATEMENT.contains("monotonically increasing"));
    assert!(REVISION_IDENTITY_STATEMENT.contains("CANDIDATE"));
    assert!(SUPERSEDE_TRANSITION_STATEMENT.contains("both directions"));
    // The Phase 4 gate note names the P4.12 cross-reference and states that
    // no command path is built here.
    assert!(PHASE4_GATE_NOTE.contains("P4.12"));
    assert!(PHASE4_GATE_NOTE.contains("No command path"));
}

#[test]
fn a_chain_of_revisions_is_gapless_and_every_new_revision_supersedes_exactly_the_prior() {
    // revision 1: initial solve, no link.
    let first = row(1, "initial_solve");
    assert_eq!(first.revision, 1);
    assert_eq!(first.supersedes_revision, None);
    assert_eq!(first.supersedes_candidate_id, None);

    // revision 2: a re-solve (new trigger, new candidate) supersedes exactly 1.
    let second = row(2, "forecast_update");
    assert_eq!(second.supersedes_revision, Some(1));
    assert_eq!(
        second.supersedes_candidate_id.as_deref(),
        Some(first.candidate_id.as_str()),
        "the link carries the superseded candidate's identity"
    );

    // revision 3 supersedes exactly 2.
    let third = row(3, "asset_opt_out_dropout");
    assert_eq!(third.supersedes_revision, Some(2));
    assert_eq!(
        third.supersedes_candidate_id.as_deref(),
        Some(second.candidate_id.as_str())
    );
}

#[test]
fn the_chain_rule_rejects_gaps_and_linkless_or_orphaned_links() {
    // A revision 3 that skips its predecessor.
    let mut parts_row = row(3, "operator_request");
    parts_row.supersedes_revision = Some(1);
    let broken = ScheduleRevisionRow::new(
        parts_row.tenant_id,
        parts_row.portfolio_id.clone(),
        parts_row.site_id.clone(),
        parts_row.schedule_id.clone(),
        parts_row.revision,
        parts_row.horizon_start_millis,
        parts_row.horizon_end_millis,
        parts_row.roll_trigger.clone(),
        parts_row.supersedes_revision,
        parts_row.supersedes_candidate_id.clone(),
        parts_row.candidate_id.clone(),
        parts_row.schedule_payload_json.clone(),
    );
    assert_eq!(
        broken,
        Err(ScheduleRevisionError::InvalidRevision { field: "supersedes_revision" })
    );

    // Revision 1 must NOT carry a supersedes link.
    let first = row(1, "initial_solve");
    assert_eq!(
        ScheduleRevisionRow::new(
            first.tenant_id,
            first.portfolio_id.clone(),
            first.site_id.clone(),
            first.schedule_id.clone(),
            1,
            first.horizon_start_millis,
            first.horizon_end_millis,
            first.roll_trigger.clone(),
            Some(0),
            Some(first.candidate_id.clone()),
            first.candidate_id.clone(),
            first.schedule_payload_json.clone(),
        ),
        Err(ScheduleRevisionError::InvalidRevision { field: "supersedes_revision" })
    );

    // Revision 2 without ANY link is rejected.
    let second = row(2, "forecast_update");
    assert!(matches!(
        ScheduleRevisionRow::new(
            second.tenant_id,
            second.portfolio_id.clone(),
            second.site_id.clone(),
            second.schedule_id.clone(),
            2,
            second.horizon_start_millis,
            second.horizon_end_millis,
            second.roll_trigger.clone(),
            None,
            None,
            second.candidate_id.clone(),
            second.schedule_payload_json.clone(),
        ),
        Err(ScheduleRevisionError::InvalidRevision { field: "supersedes_revision" })
    ));
}

#[test]
fn every_new_revision_is_a_candidate_and_the_supersede_link_is_bidirectional() {
    // A row is inserted as the CURRENT candidate revision; the SUPERSEDED
    // state is applied only through the dedicated transition (the row type
    // has no state field to set — the write shape cannot express a
    // self-superseding insert).
    let prior = row(1, "initial_solve");
    let active = ScheduleRevisionRecord::new(
        prior.clone(),
        SCHEDULE_REVISION_STATE_ACTIVE.to_string(),
        None,
        None,
    )
    .expect("active record");
    assert!(!active.is_superseded());

    // After the transition, the superseded record carries the forward link —
    // and the new revision's row carries the backward link: BOTH directions.
    let next = row(2, "envelope_invalidation");
    let superseded = ScheduleRevisionRecord::new(
        prior,
        SCHEDULE_REVISION_STATE_SUPERSEDED.to_string(),
        Some(next.revision),
        Some(next.candidate_id.clone()),
    )
    .expect("superseded record");
    assert!(superseded.is_superseded());
    assert_eq!(
        superseded.superseded_by_revision,
        Some(next.revision),
        "forward link: the superseded revision names its superseding revision"
    );
    assert_eq!(
        superseded.superseded_by_candidate_id.as_deref(),
        Some(next.candidate_id.as_str()),
        "forward link: the superseded revision names its superseding candidate"
    );
    assert_eq!(
        next.supersedes_revision,
        Some(superseded.row.revision),
        "backward link: the new revision names the superseded revision"
    );
    assert_eq!(
        next.supersedes_candidate_id.as_deref(),
        Some(superseded.row.candidate_id.as_str()),
        "backward link: the new revision names the superseded candidate"
    );

    // Superseded revisions remain retrievable (retained for audit, §14.2) —
    // the record type carries the full immutable row content.
    assert_eq!(superseded.row.schedule_payload_json, "{\"revision\":1}");
}

#[test]
fn the_phase4_gate_check_semantics_hold_on_the_record_type() {
    // None = unknown, Some(true) = SUPERSEDED, Some(false) = active: the
    // semantics `ScheduleRevisionChainStore::is_revision_superseded` returns
    // and the P4.12 gate will consume.
    let prior = row(1, "initial_solve");
    let active = ScheduleRevisionRecord::new(
        prior.clone(),
        SCHEDULE_REVISION_STATE_ACTIVE.to_string(),
        None,
        None,
    )
    .expect("active record");
    assert_eq!(
        Some(active.is_superseded()),
        Some(false),
        "an active revision is usable-in-principle (the gate decides)"
    );
    let superseded = ScheduleRevisionRecord::new(
        prior,
        SCHEDULE_REVISION_STATE_SUPERSEDED.to_string(),
        Some(2),
        Some("0192a5e0-0000-7000-8000-000000000004".to_string()),
    )
    .expect("superseded record");
    assert_eq!(
        Some(superseded.is_superseded()),
        Some(true),
        "a superseded revision is rejected by the Phase 4 gate (P4.12)"
    );
}

#[test]
fn a_supersede_transition_target_must_point_one_step_ahead() {
    assert!(ScheduleSupersedeTarget::new(2, "0192a5e0-0000-7000-8000-000000000004".to_string())
        .is_ok());
    // The adapter enforces target == prior + 1 (the chain has no gaps in
    // both directions); the target type itself only pins validity.
    assert_eq!(
        ScheduleSupersedeTarget::new(0, "0192a5e0-0000-7000-8000-000000000004".to_string()),
        Err(ScheduleRevisionError::InvalidRevision { field: "revision" })
    );
    assert_eq!(
        ScheduleSupersedeTarget::new(2, "  ".to_string()),
        Err(ScheduleRevisionError::InvalidRevision { field: "candidate_id" })
    );
}

#[test]
fn the_revision_horizon_is_part_of_the_recorded_identity() {
    // The identity scope records portfolio + horizon: a revision of another
    // horizon is never confused with this one (D-P3.22-1).
    let first = row(1, "initial_solve");
    assert_eq!(first.horizon_start_millis, 1_700_000_000_000);
    assert_eq!(first.horizon_end_millis, 1_700_008_640_000);
    assert_eq!(
        ScheduleRevisionRow::new(
            first.tenant_id,
            first.portfolio_id.clone(),
            first.site_id.clone(),
            first.schedule_id.clone(),
            1,
            first.horizon_end_millis,
            first.horizon_start_millis, // inverted horizon
            first.roll_trigger.clone(),
            None,
            None,
            first.candidate_id.clone(),
            first.schedule_payload_json.clone(),
        ),
        Err(ScheduleRevisionError::InvalidRevision { field: "horizon_end_millis" })
    );
}
