// P18.14 — Integration tests (WRITTEN-UNRUN; execution is a human gate):
//          ATTESTATION ORDERED REFUSALS + FLEET-REGISTRY VOCABULARIES
//          (plan §2 P18.14; decision record `docs/phases-11-20/phase-18-
//          edge-fleet-lifecycle-ota/records/41_p18_14_update_test_suite_
//          mock_device.md`). The wave-1 close-outs name these named
//          refusal vocabularies as the P18.14 negative-test surface.
//
// THE HONESTY BOUNDARY: no device exists; no evidence was ever collected,
// reported, or verified; no attestation decision has ever been made
// against a real device; no fleet exists; no journal row has ever been
// written to any database (the migration is WRITTEN-UNRUN). Everything
// below is the pure check functions and the in-memory projection over
// fixture data. The P18-E-2 hardware posture carries over: a claimed
// anchor is a device REPORT this platform does not verify; software-only
// evidence is consistency checking that cannot detect a compromised
// reporter.
//
// THE MATRIX (each test hand-computed):
// - the attestation gate's decision ORDER: absent evidence → revoked →
//   device mismatch → stale → counter regression → measurement check →
//   anchor requirement → AuthorizeUpdate, every refusal named;
// - the honest modes: NoMeasurementCheck records that nothing is
//   asserted; BaselineDigestEquality refuses missing, altered, and
//   EXTRA components; ReferenceRegisterMatch refuses a scheme change;
// - the P18-E-2 DEFAULT policy does not require an anchor; a deployment
//   that sets the requirement refuses software-only evidence BY NAME;
// - invalid policies and invalid evidence are ATTESTATION_* errors;
// - the fleet journal: the five kinds, the closed firmware/health/
//   attestation vocabularies with exact-case refusals, the cohort
//   IN:/OUT encoding with its parse refusals, the row-shape refusals,
//   and the latest-observation-wins projection (out-of-order arrival
//   does not displace the latest; an invalid row never enters);
// - the CROSS-LINK: a device whose journal attestation state is REVOKED
//   or FAILED never satisfies the gate (the journal vocabulary and the
//   gate agree on the closed words).

mod p18_14_support;

use std::collections::BTreeMap;

use flexcore_edge_agent::attestation::{
    check_before_update_authorization, AttestationDecision, AttestationError,
    AttestationVerificationPolicy, FleetAttestationState, MeasurementCheckMode,
    MeasurementExpectations, MeasurementRecord, TrustAnchorMode, HARDWARE_POSTURE_STATEMENT,
};
use flexcore_edge_agent::fleet_registry::{
    CohortMembership, FirmwareState, FleetRegistryError, FleetStateKind, FleetStateRecord,
    FleetStateView, ObservedHealth,
};

use p18_14_support::{
    attestation_value_parser, fixture_baseline, fixture_evidence, fixture_policy, DEVICE_ID,
    BASE_MILLIS,
};

const DAY: i64 = 24 * 60 * 60_000;

// --- 1. The attestation gate's ordered refusals ----------------------------------------

#[test]
fn the_gate_refuses_absent_evidence_first() {
    let policy = fixture_policy();
    assert_eq!(
        check_before_update_authorization(
            &policy,
            None,
            DEVICE_ID,
            false,
            None,
            &fixture_baseline(),
            BASE_MILLIS + 1_000,
        )
        .expect("pure check"),
        AttestationDecision::RefuseNoEvidence,
        "absence is never a pass"
    );
}

#[test]
fn the_gate_refuses_revoked_devices_before_any_freshness_work() {
    let policy = fixture_policy();
    let evidence = fixture_evidence(DEVICE_ID, BASE_MILLIS);
    // Stale AND revoked: the revoked refusal wins (the recorded order).
    assert_eq!(
        check_before_update_authorization(
            &policy,
            Some(&evidence),
            DEVICE_ID,
            true,
            None,
            &fixture_baseline(),
            BASE_MILLIS + 2 * DAY + 1,
        )
        .expect("pure check"),
        AttestationDecision::RefuseRevokedDevice
    );
}

#[test]
fn evidence_naming_another_device_is_refused_by_name() {
    let policy = fixture_policy();
    let evidence = fixture_evidence("some-other-device", BASE_MILLIS);
    assert_eq!(
        check_before_update_authorization(
            &policy,
            Some(&evidence),
            DEVICE_ID,
            false,
            None,
            &fixture_baseline(),
            BASE_MILLIS + 1_000,
        )
        .expect("pure check"),
        AttestationDecision::RefuseEvidenceDeviceMismatch
    );
}

#[test]
fn stale_evidence_is_refused_at_the_age_ceiling() {
    let policy = fixture_policy();
    let evidence = fixture_evidence(DEVICE_ID, BASE_MILLIS);
    // Within the 24 h default ceiling: fresh.
    assert_ne!(
        check_before_update_authorization(
            &policy,
            Some(&evidence),
            DEVICE_ID,
            false,
            None,
            &fixture_baseline(),
            BASE_MILLIS + DAY,
        )
        .expect("pure check"),
        AttestationDecision::RefuseEvidenceStale
    );
    // One millis past the ceiling: stale.
    assert_eq!(
        check_before_update_authorization(
            &policy,
            Some(&evidence),
            DEVICE_ID,
            false,
            None,
            &fixture_baseline(),
            BASE_MILLIS + DAY + 1,
        )
        .expect("pure check"),
        AttestationDecision::RefuseEvidenceStale
    );
    // Evidence reporting a FUTURE instant is an error, never a negative
    // age silently treated as fresh.
    assert!(matches!(
        check_before_update_authorization(
            &policy,
            Some(&evidence),
            DEVICE_ID,
            false,
            None,
            &fixture_baseline(),
            BASE_MILLIS - 1,
        ),
        Err(AttestationError::InvalidEvidence { .. })
    ));
}

#[test]
fn a_counter_regression_is_refused_named() {
    let policy = fixture_policy();
    let evidence = fixture_evidence(DEVICE_ID, BASE_MILLIS);
    assert_eq!(
        check_before_update_authorization(
            &policy,
            Some(&evidence),
            DEVICE_ID,
            false,
            Some(8),
            &fixture_baseline(),
            BASE_MILLIS + 1_000,
        )
        .expect("pure check"),
        AttestationDecision::RefuseCounterRegression,
        "reported 7, last seen 8 — a regression is the signature of a restored image"
    );
    // Equal and higher counters pass the check.
    assert_ne!(
        check_before_update_authorization(
            &policy,
            Some(&evidence),
            DEVICE_ID,
            false,
            Some(7),
            &fixture_baseline(),
            BASE_MILLIS + 1_000,
        )
        .expect("pure check"),
        AttestationDecision::RefuseCounterRegression
    );
    // A deployment may turn the check off — recorded, never silent: the
    // regression then does NOT refuse (the policy owns the choice).
    let permissive = AttestationVerificationPolicy::new(
        DAY,
        false,
        MeasurementCheckMode::NoMeasurementCheck,
        false,
    )
    .expect("valid policy");
    assert_ne!(
        check_before_update_authorization(
            &permissive,
            Some(&evidence),
            DEVICE_ID,
            false,
            Some(99),
            &fixture_baseline(),
            BASE_MILLIS + 1_000,
        )
        .expect("pure check"),
        AttestationDecision::RefuseCounterRegression
    );
}

#[test]
fn baseline_equality_refuses_missing_altered_and_extra_components() {
    let policy = fixture_policy();
    let evidence = fixture_evidence(DEVICE_ID, BASE_MILLIS);

    // Missing component.
    let mut missing = fixture_evidence(DEVICE_ID, BASE_MILLIS);
    missing.measurements.retain(|m| m.component != "agent");
    assert_eq!(
        check_before_update_authorization(
            &policy, Some(&missing), DEVICE_ID, false, None, &fixture_baseline(),
            BASE_MILLIS + 1_000,
        )
        .expect("pure check"),
        AttestationDecision::RefuseMeasurementMismatch
    );

    // Altered digest.
    let mut altered = fixture_evidence(DEVICE_ID, BASE_MILLIS);
    altered.measurements[0].digest_hex = "deadbeefdeadbeef".to_string();
    assert_eq!(
        check_before_update_authorization(
            &policy, Some(&altered), DEVICE_ID, false, None, &fixture_baseline(),
            BASE_MILLIS + 1_000,
        )
        .expect("pure check"),
        AttestationDecision::RefuseMeasurementMismatch
    );

    // EXTRA component (the measurement set must be exactly the baseline
    // set — an unexpected component is a mismatch, not a bonus).
    let mut extra = fixture_evidence(DEVICE_ID, BASE_MILLIS);
    extra.measurements.push(MeasurementRecord {
        component: "unexpected-stage".to_string(),
        digest_hex: "eeee4444ffff5555".to_string(),
        digest_scheme: "test-fnv1a64".to_string(),
    });
    assert_eq!(
        check_before_update_authorization(
            &policy, Some(&extra), DEVICE_ID, false, None, &fixture_baseline(),
            BASE_MILLIS + 1_000,
        )
        .expect("pure check"),
        AttestationDecision::RefuseMeasurementMismatch
    );
}

#[test]
fn the_reference_register_mode_refuses_a_scheme_change() {
    let policy = AttestationVerificationPolicy::new(
        DAY,
        true,
        MeasurementCheckMode::ReferenceRegisterMatch,
        false,
    )
    .expect("valid policy");
    let mut register = BTreeMap::new();
    register.insert("bootloader".to_string(), "test-fnv1a64:aaaa0000bbbb1111".to_string());
    register.insert("agent".to_string(), "test-fnv1a64:cccc2222dddd3333".to_string());
    let expectations = MeasurementExpectations {
        expected_digests: register,
    };
    let evidence = fixture_evidence(DEVICE_ID, BASE_MILLIS);
    assert_eq!(
        check_before_update_authorization(
            &policy, Some(&evidence), DEVICE_ID, false, None, &expectations,
            BASE_MILLIS + 1_000,
        )
        .expect("pure check"),
        AttestationDecision::AuthorizeUpdate
    );
    // A digest-scheme change (same digest text) is a detectable format
    // change, never a silent pass.
    let mut rescheme = fixture_evidence(DEVICE_ID, BASE_MILLIS);
    rescheme.measurements[0].digest_scheme = "sha256-256bit".to_string();
    assert_eq!(
        check_before_update_authorization(
            &policy, Some(&rescheme), DEVICE_ID, false, None, &expectations,
            BASE_MILLIS + 1_000,
        )
        .expect("pure check"),
        AttestationDecision::RefuseMeasurementMismatch
    );
}

#[test]
fn no_measurement_check_records_that_nothing_is_asserted() {
    let policy = AttestationVerificationPolicy::new(
        DAY,
        true,
        MeasurementCheckMode::NoMeasurementCheck,
        false,
    )
    .expect("valid policy");
    let mut empty = fixture_evidence(DEVICE_ID, BASE_MILLIS);
    empty.measurements.clear();
    assert_eq!(
        check_before_update_authorization(
            &policy, Some(&empty), DEVICE_ID, false, None, &fixture_baseline(),
            BASE_MILLIS + 1_000,
        )
        .expect("pure check"),
        AttestationDecision::AuthorizeUpdate,
        "the mode makes no measurement assertion — honestly, not as a pass of one"
    );
}

#[test]
fn the_p18_e_2_default_does_not_require_an_anchor_and_a_requirement_refuses_by_name() {
    // DEFAULT (a): hardware-optional. Software-only evidence authorizes
    // (the honest bound stands: consistency checks only).
    let default_policy = fixture_policy();
    assert!(!default_policy.require_hardware_anchor);
    let software = fixture_evidence(DEVICE_ID, BASE_MILLIS);
    assert_eq!(
        check_before_update_authorization(
            &default_policy, Some(&software), DEVICE_ID, false, None, &fixture_baseline(),
            BASE_MILLIS + 1_000,
        )
        .expect("pure check"),
        AttestationDecision::AuthorizeUpdate
    );
    // A hardware-required deployment refuses software-only evidence BY
    // NAME (a recorded policy change, never silent).
    let anchor_required = AttestationVerificationPolicy::new(
        DAY,
        true,
        MeasurementCheckMode::BaselineDigestEquality,
        true,
    )
    .expect("valid policy");
    assert_eq!(
        check_before_update_authorization(
            &anchor_required, Some(&software), DEVICE_ID, false, None, &fixture_baseline(),
            BASE_MILLIS + 1_000,
        )
        .expect("pure check"),
        AttestationDecision::RefuseHardwareAnchorRequired
    );
    // A CLAIMED anchor satisfies the requirement — a claim, never a
    // platform verification (the P6.34 posture; P18-E-2 stays OPEN).
    let mut claimed = fixture_evidence(DEVICE_ID, BASE_MILLIS);
    claimed.anchor_mode = TrustAnchorMode::HardwareAnchorClaimed;
    assert_eq!(
        check_before_update_authorization(
            &anchor_required, Some(&claimed), DEVICE_ID, false, None, &fixture_baseline(),
            BASE_MILLIS + 1_000,
        )
        .expect("pure check"),
        AttestationDecision::AuthorizeUpdate
    );
    // The statement stays pinned.
    assert!(HARDWARE_POSTURE_STATEMENT.contains("PENDING PILOT HARDWARE SPEC"));
    assert!(HARDWARE_POSTURE_STATEMENT.contains("cannot detect a compromised reporter"));
    assert!(HARDWARE_POSTURE_STATEMENT.contains("No root of trust is assumed"));
}

#[test]
fn invalid_policies_and_invalid_evidence_are_named_errors() {
    // A non-positive age ceiling is refused (fail-closed construction).
    assert_eq!(
        AttestationVerificationPolicy::new(0, true, MeasurementCheckMode::NoMeasurementCheck, false)
            .unwrap_err()
            .name(),
        "ATTESTATION_POLICY_INVALID"
    );
    // Duplicate components, missing digests, missing scheme, empty
    // identity, empty report reference — all ATTESTATION_EVIDENCE_INVALID.
    let mut duplicate = fixture_evidence(DEVICE_ID, BASE_MILLIS);
    duplicate.measurements.push(MeasurementRecord {
        component: "bootloader".to_string(),
        digest_hex: "aaaa0000bbbb1111".to_string(),
        digest_scheme: "test-fnv1a64".to_string(),
    });
    assert!(matches!(
        duplicate.validate(),
        Err(AttestationError::InvalidEvidence { .. })
    ));
    let mut anonymous = fixture_evidence(DEVICE_ID, BASE_MILLIS);
    anonymous.report_reference = " ".to_string();
    assert!(matches!(
        anonymous.validate(),
        Err(AttestationError::InvalidEvidence { .. })
    ));
    // An unattributable evidence never reaches the policy checks (the
    // gate validates before deciding).
    let mut no_digest = fixture_evidence(DEVICE_ID, BASE_MILLIS);
    no_digest.measurements[0].digest_hex = "  ".to_string();
    assert!(matches!(
        check_before_update_authorization(
            &fixture_policy(), Some(&no_digest), DEVICE_ID, false, None, &fixture_baseline(),
            BASE_MILLIS + 1_000,
        ),
        Err(AttestationError::InvalidEvidence { .. })
    ));
}

#[test]
fn the_decision_vocabulary_names_are_stable() {
    let samples = [
        (AttestationDecision::AuthorizeUpdate, "ATTESTATION_AUTHORIZE_UPDATE"),
        (AttestationDecision::RefuseNoEvidence, "ATTESTATION_REFUSE_NO_EVIDENCE"),
        (AttestationDecision::RefuseRevokedDevice, "ATTESTATION_REFUSE_REVOKED_DEVICE"),
        (AttestationDecision::RefuseEvidenceStale, "ATTESTATION_REFUSE_EVIDENCE_STALE"),
        (
            AttestationDecision::RefuseCounterRegression,
            "ATTESTATION_REFUSE_COUNTER_REGRESSION",
        ),
        (
            AttestationDecision::RefuseEvidenceDeviceMismatch,
            "ATTESTATION_REFUSE_EVIDENCE_DEVICE_MISMATCH",
        ),
        (
            AttestationDecision::RefuseMeasurementMismatch,
            "ATTESTATION_REFUSE_MEASUREMENT_MISMATCH",
        ),
        (
            AttestationDecision::RefuseHardwareAnchorRequired,
            "ATTESTATION_REFUSE_HARDWARE_ANCHOR_REQUIRED",
        ),
    ];
    for (decision, name) in samples {
        assert_eq!(decision.name(), name, "the stable name changed");
    }
}

// --- 2. The fleet-registry vocabularies (P18.12 schema) ----------------------------------

#[test]
fn the_fleet_state_kinds_are_exactly_the_migration_0023_check_set() {
    assert_eq!(FleetStateKind::FirmwareState.sql_kind(), "FIRMWARE_STATE");
    assert_eq!(FleetStateKind::CohortMembership.sql_kind(), "COHORT_MEMBERSHIP");
    assert_eq!(FleetStateKind::AttestationState.sql_kind(), "ATTESTATION_STATE");
    assert_eq!(FleetStateKind::LastSeenHealth.sql_kind(), "LAST_SEEN_HEALTH");
    assert_eq!(FleetStateKind::EsimProfileState.sql_kind(), "ESIM_PROFILE_STATE");
    // Unknown kinds are refused, never coerced.
    assert_eq!(
        FleetStateKind::from_sql_kind("PLUGIN_ROLLOUT").unwrap_err().name(),
        "FLEET_STATE_KIND_UNKNOWN"
    );
}

#[test]
fn the_firmware_vocabulary_is_closed_and_never_healthy_by_default() {
    for state in [
        FirmwareState::InSync,
        FirmwareState::UpdatePending,
        FirmwareState::Updating,
        FirmwareState::UpdateFailed,
        FirmwareState::RolledBack,
        FirmwareState::Unknown,
    ] {
        assert_eq!(FirmwareState::from_sql_value(state.sql_value()).expect("round trip"), state);
    }
    // Exact-case closed: lowercase and made-up values are refused.
    assert_eq!(
        FirmwareState::from_sql_value("in_sync").unwrap_err().name(),
        "FLEET_STATE_VALUE_UNKNOWN"
    );
    assert!(matches!(
        FirmwareState::from_sql_value("healthy"),
        Err(FleetRegistryError::UnknownStateValue { .. })
    ));
}

#[test]
fn the_health_and_attestation_vocabularies_are_closed() {
    for health in [
        ObservedHealth::SeenHealthy,
        ObservedHealth::SeenDegraded,
        ObservedHealth::NotSeen,
    ] {
        assert_eq!(ObservedHealth::from_sql_value(health.sql_value()).expect("round trip"), health);
    }
    assert!(matches!(
        ObservedHealth::from_sql_value("FINE"),
        Err(FleetRegistryError::UnknownStateValue { .. })
    ));
    for state in [
        FleetAttestationState::NotReported,
        FleetAttestationState::Current,
        FleetAttestationState::Stale,
        FleetAttestationState::Failed,
        FleetAttestationState::Revoked,
    ] {
        assert_eq!(
            FleetAttestationState::from_sql_value(state.sql_value()).expect("round trip"),
            state
        );
    }
    assert!(matches!(
        FleetAttestationState::from_sql_value("current"),
        Err(FleetRegistryError::UnknownStateValue { .. })
    ));
}

#[test]
fn the_cohort_encoding_parses_exactly_and_refuses_its_negatives() {
    let membership = CohortMembership {
        cohort_id: "cohort-2026-11".to_string(),
        block_index: 2,
        observed_at_unix_millis: BASE_MILLIS,
    };
    assert_eq!(membership.sql_value(), "IN:cohort-2026-11:2");
    let parsed = CohortMembership::from_sql_value("IN:cohort-2026-11:2").expect("parses");
    assert_eq!(parsed.cohort_id, "cohort-2026-11");
    assert_eq!(parsed.block_index, 2);
    let out = CohortMembership::from_sql_value("OUT").expect("parses");
    assert!(out.cohort_id.is_empty(), "OUT is recorded data, not an absence");
    // Refusals: empty cohort, non-numeric block, unknown encoding.
    assert!(CohortMembership::from_sql_value("IN::1").is_err());
    assert!(CohortMembership::from_sql_value("IN:cohort-1:x").is_err());
    assert!(CohortMembership::from_sql_value("MEMBER").is_err());
    assert!(CohortMembership::from_sql_value("IN:cohort-1:").is_err());
}

#[test]
fn a_journal_row_cohort_leaving_is_recorded_not_deleted() {
    let mut view = FleetStateView::new();
    let joined = fixture_cohort_row("IN:cohort-a:0", BASE_MILLIS);
    let left = fixture_cohort_row("OUT", BASE_MILLIS + 1_000);
    view.append(joined, &attestation_value_parser).expect("joined");
    view.append(left, &attestation_value_parser).expect("left");
    let current = view.current_cohort(DEVICE_ID).expect("a cohort row exists");
    let membership = current.expect("parses");
    assert!(membership.cohort_id.is_empty(), "the latest row wins: OUT");
    assert_eq!(view.history_len(), 2, "append-only: the join is retained in history");
}

#[test]
fn the_projection_keeps_the_latest_observation_and_refuses_invalid_rows() {
    let mut view = FleetStateView::new();
    let early = fixture_firmware_row("UPDATING", BASE_MILLIS);
    let late = fixture_firmware_row("IN_SYNC", BASE_MILLIS + 50_000);
    let stale = fixture_firmware_row("ROLLED_BACK", BASE_MILLIS - 1_000_000);
    view.append(early, &attestation_value_parser).expect("valid");
    view.append(late, &attestation_value_parser).expect("valid");
    view.append(stale, &attestation_value_parser).expect("valid (stale arrival)");
    assert_eq!(view.history_len(), 3);
    let current = view
        .current_firmware_state(DEVICE_ID)
        .expect("a firmware row exists");
    assert_eq!(current.expect("parses"), FirmwareState::InSync, "the LATEST wins");
    // An invalid row NEVER enters the view.
    let bad = fixture_firmware_row("made-up-state", BASE_MILLIS + 60_000);
    assert_eq!(
        view.append(bad, &attestation_value_parser).unwrap_err().name(),
        "FLEET_STATE_VALUE_UNKNOWN"
    );
    assert_eq!(view.history_len(), 3);
}

#[test]
fn structural_row_validation_refuses_incomplete_rows() {
    let mut no_tenant = fixture_firmware_row("IN_SYNC", BASE_MILLIS);
    no_tenant.tenant_id = " ".to_string();
    assert_eq!(
        no_tenant.validate(&attestation_value_parser).unwrap_err().name(),
        "FLEET_STATE_RECORD_INVALID"
    );
    let mut no_corr = fixture_firmware_row("IN_SYNC", BASE_MILLIS);
    no_corr.correlation_id = String::new();
    assert!(no_corr.validate(&attestation_value_parser).is_err());
    let mut no_time = fixture_firmware_row("IN_SYNC", 0);
    assert!(no_time.validate(&attestation_value_parser).is_err());
    let no_value = fixture_firmware_row(" ", BASE_MILLIS);
    assert!(no_value.validate(&attestation_value_parser).is_err());
}

// --- 3. The cross-link: journal vocabulary and gate vocabulary agree ----------------------

#[test]
fn a_revoked_or_failed_journal_state_never_satisfies_the_attestation_gate() {
    // The journal's FAILED/REVOKED rows mean the gate refused at the last
    // check — the platform does not authorize updates for such a device.
    let policy = fixture_policy();
    let evidence = fixture_evidence(DEVICE_ID, BASE_MILLIS);
    // A REVOKED journal row pairs with the revoked-device refusal.
    let revoked_row = fixture_attestation_row("REVOKED", BASE_MILLIS);
    revoked_row.validate(&attestation_value_parser).expect("REVOKED parses");
    assert_eq!(
        check_before_update_authorization(
            &policy,
            Some(&evidence),
            DEVICE_ID,
            true,
            None,
            &fixture_baseline(),
            BASE_MILLIS + 1_000,
        )
        .expect("pure check"),
        AttestationDecision::RefuseRevokedDevice
    );
    // A FAILED journal row means the last evidence FAILED a check: the
    // gate below re-runs with tampered measurements and refuses again —
    // the journal records the outcome, the gate re-derives it.
    let failed_row = fixture_attestation_row("FAILED", BASE_MILLIS);
    failed_row.validate(&attestation_value_parser).expect("FAILED parses");
    let mut tampered = fixture_evidence(DEVICE_ID, BASE_MILLIS);
    tampered.measurements[1].digest_hex = "0bad0bad0bad0bad".to_string();
    assert_eq!(
        check_before_update_authorization(
            &policy,
            Some(&tampered),
            DEVICE_ID,
            false,
            None,
            &fixture_baseline(),
            BASE_MILLIS + 1_000,
        )
        .expect("pure check"),
        AttestationDecision::RefuseMeasurementMismatch
    );
}

// --- Fixtures ------------------------------------------------------------------------------

fn fixture_firmware_row(value: &str, at: i64) -> FleetStateRecord {
    FleetStateRecord {
        event_id: format!("evt-{value}-{at}"),
        tenant_id: "tenant-fixture".to_string(),
        device_id: DEVICE_ID.to_string(),
        kind: FleetStateKind::FirmwareState,
        state_value: value.to_string(),
        detail: None,
        correlation_id: "corr-fixture".to_string(),
        observed_at_unix_millis: at,
    }
}

fn fixture_cohort_row(value: &str, at: i64) -> FleetStateRecord {
    FleetStateRecord {
        event_id: format!("evt-cohort-{at}"),
        tenant_id: "tenant-fixture".to_string(),
        device_id: DEVICE_ID.to_string(),
        kind: FleetStateKind::CohortMembership,
        state_value: value.to_string(),
        detail: None,
        correlation_id: "corr-fixture".to_string(),
        observed_at_unix_millis: at,
    }
}

fn fixture_attestation_row(value: &str, at: i64) -> FleetStateRecord {
    FleetStateRecord {
        event_id: format!("evt-att-{at}"),
        tenant_id: "tenant-fixture".to_string(),
        device_id: DEVICE_ID.to_string(),
        kind: FleetStateKind::AttestationState,
        state_value: value.to_string(),
        detail: None,
        correlation_id: "corr-fixture".to_string(),
        observed_at_unix_millis: at,
    }
}
