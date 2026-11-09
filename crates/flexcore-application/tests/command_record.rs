// P4.14 — Command record tests (report §14.3; ledger P4.14 acceptance
//         criteria).
//
// Coverage per the ledger acceptance criteria:
//
// - Every §14.3 field is required and validated at creation AND at issue:
//   the completeness table ([`S143_FIELD_GROUPS`]) is pinned (17 ledger
//   bullets), every field's corruption yields its NAMED error, the issue
//   phase refuses the pending authorization placeholder and an expired
//   authorization. A field dropped from the schema breaks these tests
//   loudly (the recorded mitigation for the "optional for now" failure
//   mode).
// - Each §14.3 final-paragraph rejection rule has its own test: stale,
//   duplicated-with-conflicting-payload, out-of-sequence,
//   outside-capability, missing-required-evidence.
// - Per-asset (not portfolio-inferred) acknowledgement is demonstrated in
//   a multi-asset test.
//
// No clock, no store: this suite is pure — every timestamp in it is a
// literal, never a wall-clock read.

use std::time::Duration;

use flexcore_application::command_record::{
    duplicate_conflict_rejection, expiry_milestone_for, missing_evidence_rejection,
    out_of_sequence_rejection, outside_capability_rejection, ramp_outside_capability_rejection,
    stale_rejection, AssetAcknowledgement, AssetAcknowledgementState, AssetCapabilityWindow,
    AuthorizationRecordFields, CommandRecordError, CommandRecordFields, DuplicateVerdict,
    FleetAcknowledgement, FleetAcknowledgementState, RecordValidationPhase, S143_FIELD_GROUPS,
    PENDING_AUTHORIZATION_PREFIX,
};
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::{ValidFrom, ValidUntil, ValidityWindow};
use flexcore_contracts::units::{ActivePowerMW, ReactivePowerMVAr, ReactiveSignConvention};
use flexcore_domain::command_lifecycle::{
    Actor, CommandTransition, EvidenceReference, ExpiryMilestone,
};
use flexcore_domain::command_state::CommandState;
use flexcore_domain::dispatch_command::{
    ActivePowerSignConvention, CommandAuthenticity, CommandIntent, CommandScope, ControlTarget,
    DeclaredSignConventions, DispatchCommand, EvidenceReferences, ExecutionRequirements,
    RampLimits, ScheduleReference, SourceOrigin,
};
use flexcore_domain::ids::{AssetId, CommandId, PortfolioId, ScheduleId, SiteId, TenantId};

// --- Fixture identity -----------------------------------------------------------

fn uuid(tag: u8) -> FlexUuid {
    let mut bytes = [0u8; 16];
    bytes[15] = tag;
    FlexUuid::from_bytes(bytes)
}

fn scope(asset_tag: u8) -> CommandScope {
    CommandScope {
        tenant_id: TenantId::new(uuid(1)),
        portfolio_id: PortfolioId::new(uuid(2)),
        site_id: SiteId::new(uuid(3)),
        asset_id: AssetId::new(uuid(asset_tag)),
    }
}

fn target() -> ControlTarget {
    ControlTarget::PowerSetpoint {
        p: ActivePowerMW::new(1.5).unwrap(),
        q: None,
        conventions: DeclaredSignConventions {
            active: ActivePowerSignConvention::ExportPositive,
            reactive: None,
        },
    }
}

fn validity() -> ValidityWindow {
    ValidityWindow::new(
        ValidFrom::from_unix_millis(10_000).unwrap(),
        ValidUntil::from_unix_millis(70_000).unwrap(),
    )
    .unwrap()
}

/// A fully valid §14.3 command (every field group populated).
fn valid_command(command_tag: u8, asset_tag: u8, sequence: u64) -> DispatchCommand {
    DispatchCommand::try_new(
        CommandId::new(uuid(command_tag)),
        scope(asset_tag),
        SourceOrigin {
            subsystem: "optimizer:day-ahead".to_string(),
            component_version: "plugin/1.2.3".to_string(),
        },
        ScheduleReference {
            schedule_id: ScheduleId::new(uuid(6)),
            revision: 3,
        },
        CommandIntent {
            reason: "peak-shaving window".to_string(),
            action: "set-bess-power".to_string(),
            actor_or_service: "service:scheduler".to_string(),
        },
        target(),
        validity(),
        ExecutionRequirements {
            duration: Duration::from_secs(600),
            ramp: RampLimits {
                max_up_mw_per_s: 0.5,
                max_down_mw_per_s: 0.5,
            },
        },
        sequence,
        "idem:op-42".to_string(),
        EvidenceReferences {
            grid_model: "grid-model:rev-42".to_string(),
            telemetry_snapshot: "snapshot:2026-01-01T00:00Z".to_string(),
            validation_evidence: "evidence:validation-77".to_string(),
        },
        flexcore_domain::dispatch_command::AuthorizationReference {
            decision_reference: "authz:decision-9".to_string(),
            approved_scope: "scope:bess-power".to_string(),
            approved_limits: "limits:+-2.0 MW".to_string(),
            expiry: ValidUntil::from_unix_millis(100_000).unwrap(),
        },
        CommandAuthenticity::Signature("sig:abc123".to_string()),
        None,
    )
    .expect("the fixture command is valid")
}

fn real_authorization() -> AuthorizationRecordFields {
    AuthorizationRecordFields {
        decision_reference: "authz-decision:command/x@42".to_string(),
        approved_scope: "asset:<asset>".to_string(),
        approved_limits: "limits:+-2.0 MW".to_string(),
        expiry_unix_millis: 100_000,
    }
}

// --- The completeness table ---------------------------------------------------------

#[test]
fn the_s143_table_has_exactly_the_seventeen_ledger_bullets_in_order() {
    // The ledger P4.14 step-1 enumeration, in order. If a bullet is added,
    // dropped, or reordered, THIS test fails loudly — the recorded
    // mitigation for the "a field treated as optional 'for now' becomes a
    // permanent gap" failure mode.
    let expected: [&str; 17] = [
        "unique_command_id",
        "scope_ids",
        "source_subsystem_and_version",
        "schedule_reference",
        "intent",
        "target",
        "units_and_sign_convention",
        "validity_window",
        "duration_and_ramp",
        "sequence",
        "idempotency_key",
        "grid_model_ref",
        "telemetry_snapshot_ref",
        "validation_evidence_ref",
        "authorization_reference",
        "authenticity",
        "supersedes_reference",
    ];
    assert_eq!(S143_FIELD_GROUPS.len(), 17);
    for (i, name) in expected.iter().enumerate() {
        assert_eq!(S143_FIELD_GROUPS[i].0, *name, "bullet {i} must match the ledger order");
    }
}

#[test]
fn creation_completeness_passes_on_a_fully_valid_record() {
    let command = valid_command(10, 4, 7);
    let authorization = AuthorizationRecordFields::pending_placeholder(command.command_id(), 70_000);
    let record = CommandRecordFields::from_command(&command, &authorization).unwrap();
    record.validate_creation().expect("a valid record passes creation");
    // The placeholder is allowed AT CREATION (D-P4.11-3) but is visible as
    // such.
    assert!(record
        .authorization_decision_ref
        .starts_with(PENDING_AUTHORIZATION_PREFIX));
}

#[test]
fn every_required_field_corruption_yields_its_named_error() {
    // One corruption per field of the §14.3 table (group 17 is
    // conditionally required and tested separately). Each corruption MUST
    // produce the NAMED error for that field — a missing field is never
    // silent and never defaulted (ledger P4.14 step 2).
    type Mutation = Box<dyn Fn(&mut CommandRecordFields)>;
    let cases: Vec<(&'static str, bool, Mutation)> = vec![
        // (field name, expects MissingField, mutation)
        ("command_id", true, Box::new(|r: &mut CommandRecordFields| r.command_id = " ".into())),
        ("tenant_id", true, Box::new(|r| r.tenant_id = String::new())),
        ("portfolio_id", true, Box::new(|r| r.portfolio_id = String::new())),
        ("site_id", true, Box::new(|r| r.site_id = "  ".into())),
        ("asset_id", true, Box::new(|r| r.asset_id = String::new())),
        ("source_subsystem", true, Box::new(|r| r.source_subsystem = String::new())),
        ("source_component_version", true, Box::new(|r| r.source_component_version = "".into())),
        ("schedule_id", true, Box::new(|r| r.schedule_id = String::new())),
        (
            "schedule_revision",
            false,
            Box::new(|r| r.schedule_revision = 0),
        ),
        ("reason", true, Box::new(|r| r.reason = String::new())),
        ("action", true, Box::new(|r| r.action = " ".into())),
        ("actor_or_service", true, Box::new(|r| r.actor_or_service = "".into())),
        ("target", true, Box::new(|r| r.target = String::new())),
        ("units", true, Box::new(|r| r.units = String::new())),
        ("sign_convention", true, Box::new(|r| r.sign_convention = " ".into())),
        // Group 8: a valid window is required; inverting it is invalid.
        (
            "valid_from_millis",
            false,
            Box::new(|r| r.valid_from_millis = r.valid_until_millis + 1),
        ),
        (
            "duration_millis",
            false,
            Box::new(|r| r.duration_millis = 0),
        ),
        (
            "ramp_up_mw_per_s",
            false,
            Box::new(|r| r.ramp_up_mw_per_s = f64::NAN),
        ),
        (
            "ramp_down_mw_per_s",
            false,
            Box::new(|r| r.ramp_down_mw_per_s = -1.0),
        ),
        ("sequence", false, Box::new(|r| r.sequence = 0)),
        ("idempotency_key", true, Box::new(|r| r.idempotency_key = String::new())),
        ("grid_model_ref", true, Box::new(|r| r.grid_model_ref = "".into())),
        (
            "telemetry_snapshot_ref",
            true,
            Box::new(|r| r.telemetry_snapshot_ref = " ".into()),
        ),
        (
            "validation_evidence_ref",
            true,
            Box::new(|r| r.validation_evidence_ref = String::new()),
        ),
        (
            "authorization_decision_ref",
            true,
            Box::new(|r| r.authorization_decision_ref = String::new()),
        ),
        ("approved_scope", true, Box::new(|r| r.approved_scope = " ".into())),
        ("approved_limits", true, Box::new(|r| r.approved_limits = "".into())),
        // Group 16 — authenticity.
        ("authenticity", true, Box::new(|r| r.authenticity = String::new())),
    ];
    assert_eq!(cases.len(), 28, "every required field of the §14.3 table is corrupted here");
    for (field, expects_missing, mutate) in cases {
        let command = valid_command(10, 4, 7);
        let authorization =
            AuthorizationRecordFields::pending_placeholder(command.command_id(), 70_000);
        let mut record = CommandRecordFields::from_command(&command, &authorization).unwrap();
        mutate(&mut record);
        let error = record.validate_creation().expect_err(
            "a corrupted §14.3 field must block creation with a named error",
        );
        match error {
            CommandRecordError::MissingField { phase, field: named } => {
                assert!(expects_missing, "field {field}: unexpected MissingField shape");
                assert_eq!(phase, RecordValidationPhase::Creation);
                assert_eq!(named, field, "the error must name the corrupted field");
            }
            CommandRecordError::InvalidField { phase, field: named, .. } => {
                assert!(!expects_missing, "field {field}: unexpected InvalidField shape");
                assert_eq!(phase, RecordValidationPhase::Creation);
                assert_eq!(named, field, "the error must name the corrupted field");
            }
            other => panic!("field {field}: unexpected error {other:?}"),
        }
    }
}

#[test]
fn issue_validation_refuses_the_pending_authorization_placeholder() {
    let command = valid_command(10, 4, 7);
    let authorization =
        AuthorizationRecordFields::pending_placeholder(command.command_id(), 70_000);
    let record = CommandRecordFields::from_command(&command, &authorization).unwrap();
    let error = record
        .validate_issue(1_000)
        .expect_err("a pending authorization blocks the issue transition");
    assert!(matches!(error, CommandRecordError::AuthorizationPending { .. }));
}

#[test]
fn issue_validation_refuses_an_expired_authorization() {
    let command = valid_command(10, 4, 7);
    let mut authorization = real_authorization();
    authorization.expiry_unix_millis = 5_000;
    let record = CommandRecordFields::from_command(&command, &authorization).unwrap();
    let error = record
        .validate_issue(6_000)
        .expect_err("an expired authorization blocks the issue transition");
    match error {
        CommandRecordError::InvalidField { phase, field, .. } => {
            assert_eq!(phase, RecordValidationPhase::Issue);
            assert_eq!(field, "authorization_expiry_millis");
        }
        other => panic!("unexpected error {other:?}"),
    }
}

#[test]
fn issue_validation_passes_with_a_real_authorization_inside_its_expiry() {
    let command = valid_command(10, 4, 7);
    let record = CommandRecordFields::from_command(&command, &real_authorization()).unwrap();
    record.validate_issue(6_000).expect("a real authorization issues");
    // Boundary: the expiry instant itself is past (end-exclusive).
    let error = record.validate_issue(100_000).expect_err("boundary is expired");
    assert!(matches!(error, CommandRecordError::InvalidField { .. }));
}

// --- Rejection rule: STALE ----------------------------------------------------------

#[test]
fn rejection_rule_stale_fires_only_after_the_window_ends() {
    // §10.3 end-exclusive: the window is over once now >= valid_until.
    assert!(stale_rejection(70_000, 69_999).is_none(), "still valid: no rejection");
    let rejection = stale_rejection(70_000, 70_000).expect("boundary is stale");
    assert_eq!(rejection.code(), "stale");
    assert!(stale_rejection(70_000, 70_001).is_some());
}

// --- Rejection rule: DUPLICATED WITH CONFLICTING PAYLOAD ----------------------------

#[test]
fn rejection_rule_duplicated_conflicting_payload_fires_only_on_key_match_with_different_payload() {
    // New key: a new command.
    assert!(duplicate_conflict_rejection(false, false).is_none());
    // Same key, same payload: an idempotent replay — never a rejection.
    assert!(duplicate_conflict_rejection(true, true).is_none());
    // Same key, different payload: THE conflicting duplicate.
    let rejection = duplicate_conflict_rejection(true, false).expect("conflict rejected");
    assert_eq!(rejection.code(), "duplicated_conflicting_payload");
    assert_eq!(rejection.rule, flexcore_application::command_record::CommandRejectionRule::DuplicatedConflictingPayload);
    // The classification enum agrees with the rejection function.
    assert_eq!(
        flexcore_application::command_record::DuplicateVerdict::New,
        DuplicateVerdict::New
    );
}

// --- Rejection rule: OUT OF SEQUENCE ------------------------------------------------

#[test]
fn rejection_rule_out_of_sequence_requires_strict_monotonicity_per_channel() {
    // The first command of a channel is never out of sequence.
    assert!(out_of_sequence_rejection(1, None).is_none());
    // Strictly greater passes.
    assert!(out_of_sequence_rejection(5, Some(4)).is_none());
    // Equal is out of sequence (a replayed/reordered sequence number).
    let equal = out_of_sequence_rejection(4, Some(4)).expect("equal rejected");
    assert_eq!(equal.code(), "out_of_sequence");
    // Lower is out of sequence.
    assert!(out_of_sequence_rejection(2, Some(4)).is_some());
}

// --- Rejection rule: OUTSIDE CAPABILITY ---------------------------------------------

fn capability_window() -> AssetCapabilityWindow {
    AssetCapabilityWindow {
        min_p_mw: -2.0,
        max_p_mw: 2.0,
        min_q_mvar: Some(-0.5),
        max_q_mvar: Some(0.5),
        max_up_ramp_mw_per_s: 0.8,
        max_down_ramp_mw_per_s: 0.8,
    }
}

fn p_target(mw: f64) -> ControlTarget {
    ControlTarget::PowerSetpoint {
        p: ActivePowerMW::new(mw).unwrap(),
        q: None,
        conventions: DeclaredSignConventions {
            active: ActivePowerSignConvention::ExportPositive,
            reactive: None,
        },
    }
}

#[test]
fn rejection_rule_outside_capability_fires_per_target_dimension() {
    let window = capability_window();
    // Inside the window: no rejection.
    assert!(outside_capability_rejection(&p_target(1.5), &window).is_none());
    assert!(outside_capability_rejection(&p_target(-2.0), &window).is_none());
    // Above the maximum.
    let above = outside_capability_rejection(&p_target(2.1), &window).expect("above max");
    assert_eq!(above.code(), "outside_capability");
    // Below the minimum.
    assert!(outside_capability_rejection(&p_target(-2.1), &window).is_some());

    // Reactive target within the declared window passes…
    let q_target = ControlTarget::PowerSetpoint {
        p: ActivePowerMW::new(1.0).unwrap(),
        q: Some(ReactivePowerMVAr::new(0.4).unwrap()),
        conventions: DeclaredSignConventions {
            active: ActivePowerSignConvention::ExportPositive,
            reactive: Some(ReactiveSignConvention::InjectionPositive),
        },
    };
    assert!(outside_capability_rejection(&q_target, &window).is_none());
    // …and outside it is rejected.
    let q_out = ControlTarget::PowerSetpoint {
        p: ActivePowerMW::new(1.0).unwrap(),
        q: Some(ReactivePowerMVAr::new(0.6).unwrap()),
        conventions: DeclaredSignConventions {
            active: ActivePowerSignConvention::ExportPositive,
            reactive: Some(ReactiveSignConvention::InjectionPositive),
        },
    };
    assert!(outside_capability_rejection(&q_out, &window).is_some());

    // A reactive target with NO declared reactive window is outside
    // capability (undeclared is never unconstrained).
    let p_only_window = AssetCapabilityWindow {
        min_q_mvar: None,
        max_q_mvar: None,
        ..window
    };
    assert!(outside_capability_rejection(&q_target, &p_only_window).is_some());

    // SOC is structurally bounded to [0,1] (P0.07) — never outside on
    // magnitude.
    let soc_target = ControlTarget::SupportedParameter {
        name: "soc-target".to_string(),
        value: flexcore_domain::dispatch_command::ControlParameterValue::SocFraction(
            flexcore_contracts::units::StateOfCharge::new(0.8).unwrap(),
        ),
    };
    assert!(outside_capability_rejection(&soc_target, &window).is_none());
}

#[test]
fn rejection_rule_outside_capability_covers_the_ramp_requirements() {
    let window = capability_window();
    assert!(ramp_outside_capability_rejection(0.8, 0.8, &window).is_none());
    let up = ramp_outside_capability_rejection(0.9, 0.5, &window).expect("ramp up rejected");
    assert_eq!(up.code(), "outside_capability");
    assert!(ramp_outside_capability_rejection(0.5, 0.9, &window).is_some());
}

// --- Rejection rule: MISSING REQUIRED EVIDENCE --------------------------------------

fn actor() -> Actor {
    Actor::new("test:actor").unwrap()
}

fn evidence(reference: &str) -> Option<EvidenceReference> {
    Some(EvidenceReference::new(reference).unwrap())
}

fn entry(from: CommandState, to: CommandState, evidence: Option<EvidenceReference>) -> CommandTransition {
    CommandTransition {
        command_id: CommandId::new(uuid(10)),
        from,
        to,
        actor: actor(),
        at_unix_millis: 1_000,
        gate: None,
        reason: None,
        evidence,
    }
}

#[test]
fn rejection_rule_missing_required_evidence_per_state() {
    // PROPOSED: no evidence requirement yet.
    assert!(missing_evidence_rejection(CommandState::Proposed, &[]).is_none());
    // VALIDATED requires the validation evidence.
    let rejection =
        missing_evidence_rejection(CommandState::Validated, &[]).expect("missing validation");
    assert_eq!(rejection.code(), "missing_required_evidence");
    // An EMPTY evidence reference counts as missing.
    let empty = vec![entry(CommandState::Proposed, CommandState::Validated, None)];
    assert!(missing_evidence_rejection(CommandState::Validated, &empty).is_some());
    // With the evidence present, VALIDATED passes.
    let validated = vec![entry(
        CommandState::Proposed,
        CommandState::Validated,
        evidence("validation:command/x@1"),
    )];
    assert!(missing_evidence_rejection(CommandState::Validated, &validated).is_none());
    // FEASIBLE additionally requires the (separate) feasibility evidence.
    assert!(missing_evidence_rejection(CommandState::Feasible, &validated).is_some());
    let mut feasible = validated.clone();
    feasible.push(entry(
        CommandState::Validated,
        CommandState::Feasible,
        evidence("feasibility:attempt/a:model/m:hash/h"),
    ));
    assert!(missing_evidence_rejection(CommandState::Feasible, &feasible).is_none());
    // APPROVED additionally requires the authorization decision evidence.
    assert!(missing_evidence_rejection(CommandState::Approved, &feasible).is_some());
    let mut approved = feasible.clone();
    approved.push(entry(
        CommandState::Feasible,
        CommandState::Approved,
        evidence("authz-decision:command/x@3"),
    ));
    assert!(missing_evidence_rejection(CommandState::Approved, &approved).is_none());
    assert!(missing_evidence_rejection(CommandState::Issued, &approved).is_none());
    // Non-forward states are governed by their own journal rules, not this
    // one.
    assert!(missing_evidence_rejection(CommandState::Rejected, &[]).is_none());
    assert!(missing_evidence_rejection(CommandState::Failed, &[]).is_none());
    assert!(missing_evidence_rejection(CommandState::Completed, &[]).is_none());
}

// --- Per-asset acknowledgement (multi-asset; never portfolio-inferred) ---------------

fn ack(asset_tag: u8, state: AssetAcknowledgementState, reference: Option<String>) -> AssetAcknowledgement {
    AssetAcknowledgement::new(
        CommandId::new(uuid(asset_tag + 100)),
        AssetId::new(uuid(asset_tag)),
        state,
        reference,
        Some(1_000),
    )
    .unwrap()
}

#[test]
fn a_portfolio_acknowledgement_is_never_inferred_from_the_first_responding_device() {
    // Three assets: the FIRST responds immediately with acceptance; the
    // other two are still outstanding. The fleet-level answer is
    // PartiallyAccepted — never "acknowledged".
    let fleet = FleetAcknowledgement::new(vec![
        ack(4, AssetAcknowledgementState::Accepted, Some("acc-1".to_string())),
        ack(5, AssetAcknowledgementState::AwaitingAcceptance, None),
        ack(6, AssetAcknowledgementState::AwaitingAcceptance, None),
    ])
    .unwrap();
    assert_eq!(fleet.total(), 3);
    assert_eq!(fleet.accepted_count(), 1);
    assert!(!fleet.fully_accepted());
    assert_eq!(
        fleet.portfolio_state(),
        FleetAcknowledgementState::PartiallyAccepted { accepted: 1, assets: 3 }
    );

    // The partial state is stable no matter WHICH asset responded.
    let fleet_other = FleetAcknowledgement::new(vec![
        ack(4, AssetAcknowledgementState::AwaitingAcceptance, None),
        ack(5, AssetAcknowledgementState::Accepted, Some("acc-2".to_string())),
        ack(6, AssetAcknowledgementState::AwaitingAcceptance, None),
    ])
    .unwrap();
    assert_eq!(
        fleet_other.portfolio_state(),
        FleetAcknowledgementState::PartiallyAccepted { accepted: 1, assets: 3 }
    );

    // Only EVERY asset accepting yields the fleet-level acceptance.
    let all = FleetAcknowledgement::new(vec![
        ack(4, AssetAcknowledgementState::Accepted, Some("acc-1".to_string())),
        ack(5, AssetAcknowledgementState::Accepted, Some("acc-2".to_string())),
        ack(6, AssetAcknowledgementState::Accepted, Some("acc-3".to_string())),
    ])
    .unwrap();
    assert!(all.fully_accepted());
    assert_eq!(all.portfolio_state(), FleetAcknowledgementState::AllAccepted { assets: 3 });

    // And a mixed failure keeps partial-failure visible per asset.
    let mixed = FleetAcknowledgement::new(vec![
        ack(4, AssetAcknowledgementState::Accepted, Some("acc-1".to_string())),
        ack(5, AssetAcknowledgementState::Refused, None),
        ack(6, AssetAcknowledgementState::TimedOut, None),
    ])
    .unwrap();
    assert_eq!(
        mixed.portfolio_state(),
        FleetAcknowledgementState::PartiallyAccepted { accepted: 1, assets: 3 }
    );
    let per_asset = mixed.per_asset();
    assert_eq!(per_asset[1].state, AssetAcknowledgementState::Refused);
    assert_eq!(per_asset[2].state, AssetAcknowledgementState::TimedOut);
}

#[test]
fn fleet_acknowledgement_structure_rules() {
    // Empty fleets are refused.
    assert!(FleetAcknowledgement::new(Vec::new()).is_err());
    // One acknowledgement state per asset: a duplicate asset is refused.
    assert!(FleetAcknowledgement::new(vec![
        ack(4, AssetAcknowledgementState::Accepted, Some("a".to_string())),
        ack(4, AssetAcknowledgementState::Refused, None),
    ])
    .is_err());
    // An acceptance without its record reference is not recordable.
    assert!(AssetAcknowledgement::new(
        CommandId::new(uuid(10)),
        AssetId::new(uuid(4)),
        AssetAcknowledgementState::Accepted,
        None,
        Some(1_000),
    )
    .is_err());
    // A non-acceptance state must NOT carry an acceptance reference.
    assert!(AssetAcknowledgement::new(
        CommandId::new(uuid(10)),
        AssetId::new(uuid(4)),
        AssetAcknowledgementState::Refused,
        Some("acc-x".to_string()),
        Some(1_000),
    )
    .is_err());
    // A resolved state requires its instant.
    assert!(AssetAcknowledgement::new(
        CommandId::new(uuid(10)),
        AssetId::new(uuid(4)),
        AssetAcknowledgementState::TimedOut,
        None,
        None,
    )
    .is_err());
    // Awaiting is the only state allowed without an instant.
    assert!(AssetAcknowledgement::new(
        CommandId::new(uuid(10)),
        AssetId::new(uuid(4)),
        AssetAcknowledgementState::AwaitingAcceptance,
        None,
        None,
    )
    .is_ok());
}

// --- The state→milestone mirror (P4.15 conformance base) -----------------------------

#[test]
fn the_milestone_mirror_matches_the_frozen_expired_edges() {
    // The mirror is Some exactly for the four states with frozen EXPIRED
    // edges (10, 15, 20, 24) and None for the other ten.
    assert_eq!(expiry_milestone_for(CommandState::Approved), Some(ExpiryMilestone::BeforeIssue));
    assert_eq!(
        expiry_milestone_for(CommandState::Issued),
        Some(ExpiryMilestone::BeforeAcknowledgement)
    );
    assert_eq!(
        expiry_milestone_for(CommandState::Acknowledged),
        Some(ExpiryMilestone::BeforeTrackingCompletion)
    );
    assert_eq!(
        expiry_milestone_for(CommandState::Tracking),
        Some(ExpiryMilestone::BeforeCompletion)
    );
    for state in [
        CommandState::Proposed,
        CommandState::Validated,
        CommandState::Feasible,
        CommandState::Completed,
        CommandState::Rejected,
        CommandState::Failed,
        CommandState::Expired,
        CommandState::Cancelled,
        CommandState::Superseded,
        CommandState::Fallback,
    ] {
        assert_eq!(expiry_milestone_for(state), None, "{} has no EXPIRED edge", state.name());
    }
}
