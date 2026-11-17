//! P17.02/P17.03/P17.05 — WRITTEN-UNRUN integration tests for the
//! adapter-surface modules: the hosting shapes (P17-E-2 DEFAULT), the
//! security Profile 3 onboarding surfaces, and the lifecycle-typed
//! charging relay (records 02, 03, 05; conventions §1.3: tests are
//! written, their execution is a human gate).

use flexcore_domain::command_state::CommandState;
use flexcore_domain::ev_charging_composer::{
    compose_external_constraints_profile, DeviceRateConstraints, DispatchEnvelope,
    DispatchPeriod, PlanPurpose, RateUnit, SupplyContext,
};
use flexcore_ocpp_adapter::charging::{
    into_relay_request, parse_composite_schedule_response, record_accepted_status,
    ChargingError,
};
use flexcore_ocpp_adapter::hosting::{CsmsHostingShape, CsmsPlacement};
use flexcore_ocpp_adapter::model::{
    CertificateType, SecurityEventNotificationRequest, SetChargingProfileStatus,
    SignCertificateRequest,
};
use flexcore_ocpp_adapter::security::{
    classify_security_event, CertificateFlowLedger, IdentityProofingLedger,
    IdentityProofingRecord, OnboardingPolicy, SecurityEventLedger, SecurityProfile,
};

#[test]
fn hosting_defaults_to_edge_agent_for_depots_and_names_both_shapes() {
    // The P17-E-2 DEFAULT (implemented per conventions §4; the E-decision
    // stays OPEN).
    let placement = CsmsPlacement::depot_default("depot-south".to_string()).expect("valid");
    assert!(matches!(
        placement.shape,
        CsmsHostingShape::EdgeAgentHosted {
            store_and_forward: true
        }
    ));
    assert!(placement.shape.tolerates_backhaul_outage());
    // The control-plane shape exists and names its cost honestly.
    let placement = CsmsPlacement::new(
        "grid-site-7".to_string(),
        CsmsHostingShape::ControlPlaneHosted {
            reason: "reliable backhaul; no edge agent".to_string(),
        },
    )
    .expect("valid");
    assert!(!placement.shape.tolerates_backhaul_outage());
    assert!(placement.capability_summary().contains("severs the CSMS link"));
    // The E-decision statement names what closes it.
    assert!(CsmsHostingShape::DEFAULT_STATEMENT.contains("human owner"));
}

#[test]
fn onboarding_is_profile_3_and_proofing_is_fail_closed() {
    let policy = OnboardingPolicy;
    assert!(policy.accepts_for_onboarding(SecurityProfile::Profile3));
    for recorded in [SecurityProfile::Profile1, SecurityProfile::Profile2] {
        assert!(
            !policy.accepts_for_onboarding(recorded),
            "{} is recorded, never accepted for new onboarding",
            recorded.name()
        );
    }
    // Registration without proofing is refused (identity proofing at
    // registration [S-126][S-133]).
    let ledger = IdentityProofingLedger::new();
    assert!(ledger.require_proofed("depot-cp-001").is_err());
    // A proofed charge point registers.
    let mut ledger = IdentityProofingLedger::new();
    ledger
        .record(IdentityProofingRecord {
            charge_point_id: "depot-cp-001".to_string(),
            certificate_label: "cert-1".to_string(),
            trust_anchor_label: "evse-device-ca".to_string(),
            revocation_checked: true,
            proofed_millis: 10,
        })
        .expect("proofed");
    assert!(ledger.require_proofed("depot-cp-001").is_ok());
}

#[test]
fn certificate_flows_carry_labels_and_never_issue() {
    let mut flows = CertificateFlowLedger::new();
    // The station ASKS (SignCertificate) — nothing is issued: the flow
    // opens at REQUESTED and stays there until the EXTERNAL PKI-operations
    // gate produces a signed certificate.
    let request = SignCertificateRequest {
        csr: "csr-label".to_string(),
        type_of_certificate: Some(CertificateType::V2gCertificate),
    };
    flows.open_requested("depot-cp-001", &request).expect("opened");
    let flow = flows
        .flow("depot-cp-001", CertificateType::V2gCertificate)
        .expect("flow");
    assert_eq!(flow.state.name(), "REQUESTED");
    // The issuance act is recorded as an outcome of the external gate.
    flows
        .record_signed_sent(
            "depot-cp-001",
            CertificateType::V2gCertificate,
            "signed-1".to_string(),
        )
        .expect("recorded");
    let flow = flows
        .flow("depot-cp-001", CertificateType::V2gCertificate)
        .expect("flow");
    assert_eq!(flow.state.name(), "SIGNED_AVAILABLE");
    flows
        .record_install_confirmation(
            "depot-cp-001",
            CertificateType::V2gCertificate,
            true,
            String::new(),
        )
        .expect("installed");
    assert_eq!(
        flows
            .flow("depot-cp-001", CertificateType::V2gCertificate)
            .expect("flow")
            .state
            .name(),
        "INSTALLED"
    );
}

#[test]
fn security_events_ingest_as_records_with_declared_classification() {
    let mut ledger = SecurityEventLedger::new(8).expect("valid");
    let event = SecurityEventNotificationRequest {
        timestamp_millis: 100,
        event_type: "RevokedCertificate".to_string(),
        tech_info: None,
    };
    let record = ledger.ingest("depot-cp-001", &event, 110).expect("ingested");
    assert_eq!(record.severity.name(), "CRITICAL");
    // An unknown type is explicitly Unclassified — never guessed.
    assert_eq!(
        classify_security_event("VendorEvent").name(),
        "UNCLASSIFIED"
    );
    // Ingestion is a RECORD: no revocation, teardown, or reaction is
    // triggered (the ledger exposes only snapshots).
    assert_eq!(ledger.snapshot().len(), 1);
}

#[test]
fn the_charging_relay_is_issued_only_and_the_composer_plan_is_not_a_state() {
    let envelope = DispatchEnvelope {
        start_millis: 1_700_000_000_000,
        periods: vec![DispatchPeriod {
            start_offset_s: 0,
            limit_mw: 0.05,
        }],
    };
    let plan = compose_external_constraints_profile(
        &envelope,
        RateUnit::W,
        &SupplyContext::AcThreePhase {
            nominal_voltage_v: 400.0,
        },
        &DeviceRateConstraints {
            min_charging_rate: None,
        },
        3,
        true,
    )
    .expect("composed");
    assert_eq!(plan.purpose, PlanPurpose::ChargingStationExternalConstraints);
    // The plan carries NO lifecycle state — it is a payload candidate.
    // The relay gate refuses every non-ISSUED state by name.
    for state in [
        CommandState::Proposed,
        CommandState::Validated,
        CommandState::Feasible,
        CommandState::Approved,
    ] {
        let error = into_relay_request(&plan, state, 1).expect_err("refused");
        assert!(error.to_string().contains(state.name()));
    }
    let request = into_relay_request(&plan, CommandState::Issued, 1).expect("issued");
    assert_eq!(request.charging_schedule.charging_schedule_period[0].limit, 50_000.0);
    assert_eq!(request.is_grid_critical, Some(true));

    // Accepted = STORED, not applied (the statement travels with the
    // status).
    let (statement, _) = record_accepted_status(SetChargingProfileStatus::Accepted);
    assert!(statement.contains("not physical delivery"));
}

#[test]
fn the_reconciliation_surface_names_mismatches_never_retries() {
    let envelope = DispatchEnvelope {
        start_millis: 0,
        periods: vec![DispatchPeriod {
            start_offset_s: 0,
            limit_mw: 0.1,
        }],
    };
    let plan = compose_external_constraints_profile(
        &envelope,
        RateUnit::W,
        &SupplyContext::AcThreePhase {
            nominal_voltage_v: 400.0,
        },
        &DeviceRateConstraints {
            min_charging_rate: None,
        },
        1,
        true,
    )
    .expect("composed");
    // The observed composite matches: aligned.
    let payload = serde_json::json!({
        "schedule": {
            "chargingRateUnit": "W",
            "chargingSchedulePeriod": [{"startPeriod": 0, "limit": 100000.0}]
        }
    });
    let observation = parse_composite_schedule_response(&payload, RateUnit::W).expect("decoded");
    let outcome = flexcore_domain::ev_charging_composer::reconcile_composite_schedule(
        &plan,
        &observation,
    );
    assert_eq!(
        outcome,
        flexcore_domain::ev_charging_composer::ReconciliationOutcome::Aligned
    );
    // A drifted composite is a named mismatch; the adapter surface exposes
    // no retry anywhere (absence is structural).
    let drifted = serde_json::json!({
        "schedule": {
            "chargingRateUnit": "W",
            "chargingSchedulePeriod": [{"startPeriod": 0, "limit": 25000.0}]
        }
    });
    let observation =
        parse_composite_schedule_response(&drifted, RateUnit::W).expect("decoded");
    assert!(matches!(
        flexcore_domain::ev_charging_composer::reconcile_composite_schedule(&plan, &observation),
        flexcore_domain::ev_charging_composer::ReconciliationOutcome::Mismatch { .. }
    ));
    // The refusal path names the shape problem instead of guessing.
    let bad = serde_json::json!({"no_schedule_here": true});
    let error = parse_composite_schedule_response(&bad, RateUnit::W).expect_err("refused");
    assert!(matches!(error, ChargingError::InvalidCompositeResponse { .. }));
}
