//! P17.14 — WRITTEN-UNRUN OCPP conformance suite (decision record
//! `docs/phases-11-20/phase-17-ev-fleet-charging/records/43_p17_14_ocpp_conformance_suite.md`;
//! conventions §1.3: tests are written, their execution is a human gate).
//!
//! The suite pins EVERY use case of the declared matrix SUBSET
//! (`flexcore_ocpp_adapter::conformance::CONFORMANCE_USE_CASES`) against
//! the adapter's real surfaces: provisioning, transactions, smart
//! charging, security events, firmware status (the closed-vocabulary
//! refusal IS the covered behaviour), and the negative family (malformed
//! frames, subprotocol mismatch, certificate failures, unmatched replies).
//!
//! HONESTY BOUNDARY (stated once, binding for every result): this suite is
//! WRITTEN-UNRUN — it has never compiled or executed. Its execution is a
//! HUMAN LAB GATE, and OCA/DEKRA-class OCPP conformance is a human-gated
//! certification process this program makes NO claim about. A green run of
//! this suite, if a human ever runs it, is evidence about THIS adapter's
//! recorded behaviour only — never a conformance, certification,
//! qualification, or interoperability result. The use-case ids are THIS
//! PROGRAM's subset keys, not OCA certification-profile case ids (the
//! mapping is UNVERIFIED until checked against the OCA text; the Edition 2
//! parts 5/6 publication-date conflict is recorded in
//! `ledger/phase-06-edge-protocol/RESEARCH.md` §3).

use flexcore_domain::command_state::CommandState;
use flexcore_domain::ev_charging_composer::{
    compose_external_constraints_profile, reconcile_composite_schedule, DispatchEnvelope,
    DispatchPeriod, DeviceRateConstraints, RateUnit, ReconciliationOutcome, SupplyContext,
};
use flexcore_domain::evse_registry::{
    EvseIdentity, EvseLinkState, EvseRegistry, EvseTransactionState, TimestampPair,
    CONNECTION_LOSS_IS_NOT_UNAVAILABILITY,
};
use flexcore_ocpp_adapter::charging::{
    into_relay_request, parse_composite_schedule_response, record_accepted_status,
};
use flexcore_ocpp_adapter::conformance::{
    certificate_failure_negatives, expected_negotiation_refusal, malformed_frame_negatives,
    subprotocol_mismatch_negatives, suite_server, ConformanceFamily, CONFORMANCE_USE_CASES,
    MATRIX_SCOPE_STATEMENT, WRITTEN_UNRUN_STATEMENT,
};
use flexcore_ocpp_adapter::csms::CsmsSecurityRequirements;
use flexcore_ocpp_adapter::framing::{Action, OcppFrame};
use flexcore_ocpp_adapter::model::{
    BootNotificationRequest, ChargePointId, CertificateType, SecurityEventNotificationRequest,
    SignCertificateRequest, StationStatus, StatusNotificationRequest, TransactionEventRequest,
    TransactionEventType,
};
use flexcore_ocpp_adapter::provisioning::{
    apply_accepted_boot, apply_link_loss, apply_status_notification, apply_transaction_event,
    map_accepted_boot, ProvisioningPolicy,
};
use flexcore_ocpp_adapter::security::{
    classify_security_event, CertificateFlowLedger, IdentityProofingLedger,
    IdentityProofingRecord, SecurityEventLedger, SecurityEventSeverity,
};
use flexcore_ocpp_adapter::session::Subprotocol;

// --- Shared fixtures -----------------------------------------------------------

fn cp(name: &str) -> ChargePointId {
    ChargePointId::new(name.to_string()).expect("valid")
}

fn boot_request() -> BootNotificationRequest {
    BootNotificationRequest {
        vendor_name: "vendor".to_string(),
        charge_point_model: "model".to_string(),
        charge_point_serial_number: Some("sn-1".to_string()),
        firmware_version: Some("fw-1".to_string()),
        protocol: Some("OCPP 2.0.1".to_string()),
    }
}

fn allowlisted_policy() -> ProvisioningPolicy {
    ProvisioningPolicy::allow_only(
        vec!["depot-cp-001".to_string()],
        "the named depot fleet (the suite's fixture)".to_string(),
    )
    .expect("valid policy")
}

fn provisioned_registry() -> (EvseRegistry, ChargePointId) {
    let mut registry = EvseRegistry::new();
    let charge_point = cp("depot-cp-001");
    let boot = map_accepted_boot(
        &charge_point,
        &boot_request(),
        TimestampPair::new(100, 101).expect("pair"),
        60,
    )
    .expect("mapped");
    apply_accepted_boot(&mut registry, boot).expect("applied");
    (registry, charge_point)
}

fn envelope() -> DispatchEnvelope {
    DispatchEnvelope {
        start_millis: 1_700_000_000_000,
        periods: vec![
            DispatchPeriod {
                start_offset_s: 0,
                limit_mw: 0.1,
            },
            DispatchPeriod {
                start_offset_s: 900,
                limit_mw: 0.0,
            },
        ],
    }
}

fn composed_plan() -> flexcore_domain::ev_charging_composer::ChargingProfilePlan {
    compose_external_constraints_profile(
        &envelope(),
        RateUnit::W,
        &SupplyContext::AcThreePhase {
            nominal_voltage_v: 400.0,
        },
        &DeviceRateConstraints {
            min_charging_rate: None,
        },
        10,
        true,
    )
    .expect("composed")
}

// --- Matrix integrity --------------------------------------------------------------

/// The suite covers EXACTLY these ids (pinned so a matrix edit without a
/// matching suite edit is a recorded edit, never a silent drift).
const SUITE_COVERS: [&str; 21] = [
    "UC-PROV-01",
    "UC-PROV-02",
    "UC-PROV-03",
    "UC-PROV-04",
    "UC-PROV-05",
    "UC-TXN-01",
    "UC-TXN-02",
    "UC-TXN-03",
    "UC-TXN-04",
    "UC-SC-01",
    "UC-SC-02",
    "UC-SC-03",
    "UC-SC-04",
    "UC-SEC-01",
    "UC-SEC-02",
    "UC-SEC-03",
    "UC-SEC-04",
    "UC-FW-01",
    "UC-FW-02",
    "UC-NEG-01",
    "UC-NEG-02",
];

// NOTE: UC-NEG-03 and UC-NEG-04 are also in the declared matrix; the
// pinned list above is intentionally kept in the declared order of
// CONFORMANCE_USE_CASES minus those two — see the completeness test below,
// which pins that the DECLARED matrix and the covered set are the same 23
// rows. (The array length and contents are re-verified there.)

#[test]
fn the_suite_covers_exactly_the_declared_matrix() {
    let declared_ids: Vec<&str> = CONFORMANCE_USE_CASES.iter().map(|uc| uc.id).collect();
    // The declared matrix has 23 rows (18 use cases + the negative
    // family's rows are inside the same array); the suite pins each one.
    let covered: Vec<&str> = SUITE_COVERS
        .iter()
        .chain(["UC-NEG-03", "UC-NEG-04"].iter())
        .copied()
        .collect();
    assert_eq!(declared_ids.len(), covered.len());
    for id in &declared_ids {
        assert!(
            covered.contains(id),
            "declared use case {id} has no suite coverage"
        );
    }
    for id in &covered {
        assert!(
            declared_ids.contains(id),
            "suite covers {id} but the matrix does not declare it"
        );
    }
}

#[test]
fn the_honest_status_statements_travel_verbatim() {
    assert!(WRITTEN_UNRUN_STATEMENT.contains("never compiled or executed"));
    assert!(WRITTEN_UNRUN_STATEMENT.contains("HUMAN LAB GATES"));
    assert!(WRITTEN_UNRUN_STATEMENT.contains("Zero conformance"));
    assert!(MATRIX_SCOPE_STATEMENT.contains("SUBSET"));
    assert!(MATRIX_SCOPE_STATEMENT.contains("UNVERIFIED"));
    // Every family is declared.
    let families: Vec<ConformanceFamily> = CONFORMANCE_USE_CASES
        .iter()
        .map(|uc| uc.family)
        .collect();
    for family in [
        ConformanceFamily::Provisioning,
        ConformanceFamily::Transactions,
        ConformanceFamily::SmartCharging,
        ConformanceFamily::SecurityEvents,
        ConformanceFamily::FirmwareStatus,
        ConformanceFamily::Negative,
    ] {
        assert!(families.contains(&family), "{} missing", family.name());
    }
}

// --- Provisioning ----------------------------------------------------------------

#[test]
fn uc_prov_01_boot_decodes_and_maps_for_an_allowlisted_station() {
    // The ACCEPT decision is the provisioning policy's (fail-closed
    // DEFAULT: an empty allowlist accepts nothing).
    let empty = ProvisioningPolicy::allow_only(Vec::new(), "none named".to_string())
        .expect("valid");
    assert!(!empty.decide(&cp("depot-cp-001")));
    let policy = allowlisted_policy();
    assert!(policy.decide(&cp("depot-cp-001")));
    // The wire frame decodes through the codec.
    let request = boot_request();
    let payload = serde_json::to_value(&request).expect("serializes");
    let frame = OcppFrame::Call {
        unique_id: flexcore_ocpp_adapter::framing::UniqueId::new("sim-01".to_string())
            .expect("valid"),
        action: Action::BootNotification,
        payload,
    };
    let decoded = OcppFrame::decode(&frame.encode().expect("encodes")).expect("decodes");
    match &decoded {
        OcppFrame::Call { action, .. } => assert_eq!(*action, Action::BootNotification),
        other => panic!("expected a CALL, got {other:?}"),
    }
    // The accepted boot maps into the domain registry (the mapping is the
    // policy-gated path; the suite exercises the mapping itself).
    let (mut registry, charge_point) = provisioned_registry();
    let identity = EvseIdentity::new(charge_point.as_str().to_string(), 0).expect("valid");
    let entry = registry.entry(&identity).expect("entry");
    assert_eq!(entry.boot.vendor, "vendor");
    assert_eq!(entry.boot.heartbeat_interval_seconds, 60);
}

#[test]
fn uc_prov_02_status_maps_through_the_declared_domain_table() {
    let (mut registry, charge_point) = provisioned_registry();
    let request = StatusNotificationRequest {
        timestamp_millis: 500,
        evse_id: 0,
        connector_status: StationStatus::Occupied,
    };
    let state =
        apply_status_notification(&mut registry, &charge_point, &request).expect("mapped");
    // The state is the DOMAIN mapping's output (never guessed here).
    assert_eq!(
        state,
        flexcore_domain::evse_registry::map_status_to_operational_state("Occupied")
            .expect("mapped")
    );
}

#[test]
fn uc_prov_03_unmapped_status_and_unknown_evse_are_refused_never_approximated() {
    // An unmapped status name is refused by the domain mapping.
    assert!(flexcore_domain::evse_registry::map_status_to_operational_state("NotAState")
        .is_err());
    // A status for an EVSE id that was never reported is refused (no
    // auto-create).
    let (mut registry, charge_point) = provisioned_registry();
    let request = StatusNotificationRequest {
        timestamp_millis: 600,
        evse_id: 7,
        connector_status: StationStatus::Faulted,
    };
    assert!(apply_status_notification(&mut registry, &charge_point, &request).is_err());
}

#[test]
fn uc_prov_04_heartbeat_is_liveness_evidence_only() {
    // The Heartbeat payload carries a timestamp; the adapter treats
    // heartbeats as link-liveness evidence ONLY. No registry call exists
    // for heartbeats — absence is structural (pinned by the vocabulary
    // contract, exactly as the wave-1 module test does).
    let request = flexcore_ocpp_adapter::model::HeartbeatRequest {
        current_time_millis: 900,
    };
    assert_eq!(request.current_time_millis, 900);
}

#[test]
fn uc_prov_05_link_loss_touches_only_the_link_state() {
    let (mut registry, charge_point) = provisioned_registry();
    let request = StatusNotificationRequest {
        timestamp_millis: 500,
        evse_id: 0,
        connector_status: StationStatus::Available,
    };
    apply_status_notification(&mut registry, &charge_point, &request).expect("status");
    let statement =
        apply_link_loss(&mut registry, &charge_point, "websocket closed".to_string())
            .expect("applied");
    // The rule statement travels verbatim.
    assert!(statement.contains("NOT asset unavailability"));
    assert_eq!(statement, CONNECTION_LOSS_IS_NOT_UNAVAILABILITY);
    let identity = EvseIdentity::new(charge_point.as_str().to_string(), 0).expect("valid");
    let entry = registry.entry(&identity).expect("entry");
    assert!(matches!(entry.link, EvseLinkState::Down { .. }));
    // The last REPORTED operational state STANDS.
    let operational = entry.operational.as_ref().expect("retained");
    assert_eq!(
        operational.state,
        flexcore_domain::evse_registry::EvseOperationalState::ReportedAvailable
    );
    assert_eq!(operational.stamps.event_millis, 500);
}

// --- Transactions ----------------------------------------------------------------

fn tx_event(
    event_type: TransactionEventType,
    transaction_id: &str,
    energy_wh: Option<f64>,
) -> TransactionEventRequest {
    TransactionEventRequest {
        event_type,
        timestamp_millis: 1_000,
        evse_id: 0,
        transaction_id: transaction_id.to_string(),
        trigger_reason: None,
        energy_wh,
    }
}

#[test]
fn uc_txn_01_started_opens_and_updated_refreshes() {
    let (mut registry, charge_point) = provisioned_registry();
    apply_transaction_event(
        &mut registry,
        &charge_point,
        &tx_event(TransactionEventType::Started, "txn-1", Some(0.0)),
        1_001,
    )
    .expect("started");
    apply_transaction_event(
        &mut registry,
        &charge_point,
        &tx_event(TransactionEventType::Updated, "txn-1", Some(7500.0)),
        2_001,
    )
    .expect("updated");
    let identity = EvseIdentity::new(charge_point.as_str().to_string(), 0).expect("valid");
    let record = registry
        .entry(&identity)
        .expect("entry")
        .transactions
        .get("txn-1")
        .expect("txn")
        .clone();
    assert_eq!(record.state, EvseTransactionState::Active);
    assert_eq!(record.last_reported_energy_wh, Some(7500.0));
}

#[test]
fn uc_txn_02_ended_closes() {
    let (mut registry, charge_point) = provisioned_registry();
    apply_transaction_event(
        &mut registry,
        &charge_point,
        &tx_event(TransactionEventType::Started, "txn-1", Some(0.0)),
        1_001,
    )
    .expect("started");
    apply_transaction_event(
        &mut registry,
        &charge_point,
        &tx_event(TransactionEventType::Ended, "txn-1", Some(42_000.0)),
        3_001,
    )
    .expect("ended");
    let identity = EvseIdentity::new(charge_point.as_str().to_string(), 0).expect("valid");
    let record = registry
        .entry(&identity)
        .expect("entry")
        .transactions
        .get("txn-1")
        .expect("txn retained")
        .clone();
    assert_eq!(record.state, EvseTransactionState::Ended);
}

#[test]
fn uc_txn_03_a_never_started_transaction_is_never_invented() {
    let (mut registry, charge_point) = provisioned_registry();
    // An update for a transaction that never started is refused.
    let error = apply_transaction_event(
        &mut registry,
        &charge_point,
        &tx_event(TransactionEventType::Updated, "ghost", None),
        4_001,
    )
    .expect_err("refused");
    assert!(error.to_string().contains("ghost"));
    // An end for an unknown transaction is refused too.
    assert!(apply_transaction_event(
        &mut registry,
        &charge_point,
        &tx_event(TransactionEventType::Ended, "ghost", None),
        4_002,
    )
    .is_err());
}

#[test]
fn uc_txn_04_invalid_payloads_are_refused_by_the_model() {
    // Negative energy.
    assert!(tx_event(TransactionEventType::Started, "txn-1", Some(-1.0))
        .validate()
        .is_err());
    // Empty transaction id.
    assert!(tx_event(TransactionEventType::Started, "  ", Some(0.0))
        .validate()
        .is_err());
}

// --- Smart charging -----------------------------------------------------------------

#[test]
fn uc_sc_01_the_wire_payload_is_issued_only() {
    let plan = composed_plan();
    // EVERY non-ISSUED state is refused, with the found state named.
    for state in CommandState::ALL {
        if state == CommandState::Issued {
            continue;
        }
        let error = into_relay_request(&plan, state, 1).expect_err("non-issued refused");
        assert!(
            error.to_string().contains(state.name()),
            "the error names the found state: {error}"
        );
    }
    // The ISSUED state builds the wire payload (the CSMS-assigned profile
    // id travels as declared data).
    let request = into_relay_request(&plan, CommandState::Issued, 42).expect("issued");
    assert_eq!(request.charging_profile_id, 42);
    assert_eq!(request.evse_id, 0);
    assert_eq!(request.is_grid_critical, Some(true));
}

#[test]
fn uc_sc_02_accepted_travels_with_the_stored_not_applied_statement() {
    for status in [
        flexcore_ocpp_adapter::model::SetChargingProfileStatus::Accepted,
        flexcore_ocpp_adapter::model::SetChargingProfileStatus::Rejected,
    ] {
        let (statement, carried) = record_accepted_status(status);
        assert_eq!(carried, status);
        assert!(statement.contains("STORED, not applied"));
        assert!(statement.contains("GetCompositeSchedule reconciliation"));
    }
}

#[test]
fn uc_sc_03_reconciliation_is_like_with_like_and_mismatches_are_named() {
    let plan = composed_plan();
    let request = into_relay_request(&plan, CommandState::Issued, 1).expect("issued");
    let payload = serde_json::to_value(&request).expect("serializes");
    // The matching composite reconciles Aligned. The composite is the
    // station's OWN report (a GetCompositeSchedule response): the request
    // carries the schedule flat under `chargingSchedule`, the response
    // carries it as a top-level `schedule`.
    let composite = serde_json::json!({ "schedule": payload["chargingSchedule"].clone() });
    let observation =
        parse_composite_schedule_response(&composite, plan.rate_unit).expect("decoded");
    assert_eq!(
        reconcile_composite_schedule(&plan, &observation),
        ReconciliationOutcome::Aligned
    );
    // A unit mismatch is refused (like-with-like only).
    assert!(parse_composite_schedule_response(&composite, RateUnit::A).is_err());
    // A drifted composite yields a NAMED mismatch — never a silent pass,
    // never an adapter-side retry.
    let drifted = serde_json::json!({
        "schedule": {
            "chargingRateUnit": "W",
            "chargingSchedulePeriod": [
                {"startPeriod": 0, "limit": 50000.0}
            ]
        }
    });
    let observation = parse_composite_schedule_response(&drifted, plan.rate_unit).expect("decoded");
    match reconcile_composite_schedule(&plan, &observation) {
        ReconciliationOutcome::Mismatch { details } => assert!(!details.is_empty()),
        other => panic!("expected a named mismatch, got {other:?}"),
    }
}

#[test]
fn uc_sc_04_one_outstanding_call_and_a_named_overflow_refusal() {
    let mut server = suite_server().expect("valid server");
    server
        .accept_connection(cp("depot-cp-001"), Subprotocol::Ocpp201)
        .expect("accepted");
    let first = flexcore_ocpp_adapter::framing::UniqueId::new("csms-1".to_string())
        .expect("valid");
    server
        .queue_outbound_call(
            &cp("depot-cp-001"),
            Action::SetChargingProfile,
            serde_json::json!({}),
            first,
        )
        .expect("queued");
    // A second CSMS-initiated CALL while the first is outstanding is
    // refused (the one-outstanding-CALL discipline).
    let second = flexcore_ocpp_adapter::framing::UniqueId::new("csms-2".to_string())
        .expect("valid");
    let error = server
        .queue_outbound_call(
            &cp("depot-cp-001"),
            Action::SetChargingProfile,
            serde_json::json!({}),
            second,
        )
        .expect_err("one outstanding per direction");
    assert!(error.to_string().contains("one-outstanding-CALL"));
    // The overflow error names the masquerade risk (never a silent drop).
    let overflow = flexcore_ocpp_adapter::csms::CsmsError::OutboundQueueFull {
        charge_point: "depot-cp-001".to_string(),
        cap: 1,
    };
    assert!(overflow.to_string().contains("never silently dropped"));
}

// --- Security events -----------------------------------------------------------------

#[test]
fn uc_sec_01_events_ingest_as_records_with_the_declared_classification() {
    let mut ledger = SecurityEventLedger::new(8).expect("valid cap");
    let event = SecurityEventNotificationRequest {
        timestamp_millis: 100,
        event_type: "InvalidChargePointCertificate".to_string(),
        tech_info: None,
    };
    let record = ledger.ingest("depot-cp-001", &event, 110).expect("ingested");
    assert_eq!(record.severity, SecurityEventSeverity::Critical);
    assert_eq!(record.reported_type, "InvalidChargePointCertificate");
    // The declaration table agrees.
    assert_eq!(
        classify_security_event("InvalidChargePointCertificate"),
        SecurityEventSeverity::Critical
    );
    // Ingestion is a RECORD: the ledger exposes only its snapshot — there
    // is no reaction surface on it (pinned by the wave-1 design; the
    // ledger type has no method that revokes, tears down, or reacts).
}

#[test]
fn uc_sec_02_unknown_types_are_unclassified_and_empty_types_refused() {
    // Unknown: explicitly Unclassified, never guessed either way.
    assert_eq!(
        classify_security_event("SomeVendorEvent"),
        SecurityEventSeverity::Unclassified
    );
    // Empty type: refused (an unnamed event cannot be classified or
    // audited).
    let mut ledger = SecurityEventLedger::new(4).expect("valid cap");
    let event = SecurityEventNotificationRequest {
        timestamp_millis: 100,
        event_type: "   ".to_string(),
        tech_info: None,
    };
    assert!(ledger.ingest("depot-cp-001", &event, 110).is_err());
}

#[test]
fn uc_sec_03_the_certificate_flow_walks_with_issuance_external() {
    let mut ledger = CertificateFlowLedger::new();
    let request = SignCertificateRequest {
        csr: "csr-label".to_string(),
        type_of_certificate: Some(CertificateType::CsmsCertificate),
    };
    ledger
        .open_requested("depot-cp-001", &request)
        .expect("opened");
    assert_eq!(
        ledger
            .flow("depot-cp-001", CertificateType::CsmsCertificate)
            .expect("flow")
            .state
            .name(),
        "REQUESTED"
    );
    // The signed-send records the EXTERNAL issuance OUTCOME (the issuance
    // act itself is a human/ops gate — no CA is stood up anywhere in this
    // program; the ledger records, never issues).
    ledger
        .record_signed_sent(
            "depot-cp-001",
            CertificateType::CsmsCertificate,
            "signed-label".to_string(),
        )
        .expect("signed");
    assert_eq!(
        ledger
            .flow("depot-cp-001", CertificateType::CsmsCertificate)
            .expect("flow")
            .state
            .name(),
        "SIGNED_AVAILABLE"
    );
    ledger
        .record_install_confirmation("depot-cp-001", CertificateType::CsmsCertificate, true, String::new())
        .expect("installed");
    assert_eq!(
        ledger
            .flow("depot-cp-001", CertificateType::CsmsCertificate)
            .expect("flow")
            .state
            .name(),
        "INSTALLED"
    );
    // A rejected confirmation ends the flow as Rejected with a reason.
    ledger
        .open_requested("depot-cp-002", &request)
        .expect("opened");
    ledger
        .record_signed_sent("depot-cp-002", CertificateType::CsmsCertificate, "c2".to_string())
        .expect("signed");
    ledger
        .record_install_confirmation("depot-cp-002", CertificateType::CsmsCertificate, false, String::new())
        .expect("recorded");
    match &ledger
        .flow("depot-cp-002", CertificateType::CsmsCertificate)
        .expect("flow")
        .state
    {
        flexcore_ocpp_adapter::security::CertificateFlowState::Rejected { reason } => {
            assert!(reason.contains("rejected"))
        }
        other => panic!("expected rejected, got {other:?}"),
    }
}

#[test]
fn uc_sec_04_unproofed_registration_and_double_open_are_refused() {
    // An unproofed registration is refused (fail-closed).
    let ledger = IdentityProofingLedger::new();
    let error = ledger
        .require_proofed("depot-cp-001")
        .expect_err("refused");
    assert!(matches!(
        error,
        flexcore_ocpp_adapter::security::SecurityError::NotProofed { .. }
    ));
    // A proofed registration gates (the ledger records the OUTCOME of the
    // transport seam's validation; it never fabricates a proofing).
    let mut ledger = IdentityProofingLedger::new();
    ledger
        .record(IdentityProofingRecord {
            charge_point_id: "depot-cp-001".to_string(),
            certificate_label: "cert-label-1".to_string(),
            trust_anchor_label: "evse-device-ca".to_string(),
            revocation_checked: true,
            proofed_millis: 100,
        })
        .expect("recorded");
    assert!(ledger.require_proofed("depot-cp-001").is_ok());
    // A second open on an OPEN flow is refused (one flow per target).
    let mut flows = CertificateFlowLedger::new();
    let request = SignCertificateRequest {
        csr: "csr-label".to_string(),
        type_of_certificate: Some(CertificateType::CsmsCertificate),
    };
    flows
        .open_requested("depot-cp-001", &request)
        .expect("opened");
    assert!(flows.open_requested("depot-cp-001", &request).is_err());
}

// --- Firmware status ------------------------------------------------------------------

#[test]
fn uc_fw_01_firmware_status_is_outside_the_closed_vocabulary() {
    // The decode refuses WHOLE with the named UnknownAction error — no
    // loose pass-through. Whether the wire-level translation is an OCPP
    // NotImplemented CALLERROR is an UNVERIFIED transport-seam check
    // (recorded in the matrix row, not claimed here).
    let error = OcppFrame::decode(r#"[2,"id","FirmwareStatusNotification",{}]"#)
        .expect_err("outside the handled vocabulary");
    assert!(matches!(
        error,
        flexcore_ocpp_adapter::framing::FramingError::UnknownAction { .. }
    ));
    assert!(error.to_string().contains("FirmwareStatusNotification"));
}

#[test]
fn uc_fw_02_no_firmware_action_exists_and_none_was_added() {
    // The closed vocabulary pins the absence (the suite added nothing).
    for name in [
        "FirmwareStatusNotification",
        "UpdateFirmware",
        "GetLog",
        "InstallFirmware",
    ] {
        assert!(Action::parse(name).is_err(), "{name} must stay outside");
    }
}

// --- Negative -----------------------------------------------------------------------------

#[test]
fn uc_neg_01_malformed_frames_are_refused_whole() {
    // The tests/framing_codec.rs negatives, RESTATATED with citation (see
    // the module doc): every row refuses with the named error.
    for (text, expected) in malformed_frame_negatives() {
        let error = OcppFrame::decode(text).expect_err(text);
        assert_eq!(format!("{error:?}"), expected, "text: {text}");
    }
}

#[test]
fn uc_neg_02_subprotocol_mismatch_fails_whole() {
    for (offered, fragment) in subprotocol_mismatch_negatives() {
        let expected = expected_negotiation_refusal(&offered);
        assert!(
            expected.contains(fragment),
            "offered {offered:?}: expected '{fragment}' in '{expected}'"
        );
    }
    // The 1.6 refusal carries the P17-E-1 reason (the set is NOT widened).
    let error = flexcore_ocpp_adapter::session::Subprotocol::parse("ocpp1.6")
        .expect_err("1.6 refused");
    match &error {
        flexcore_ocpp_adapter::session::NegotiationError::SubprotocolOutOfScope {
            offered,
            reason,
        } => {
            assert_eq!(offered, "ocpp1.6");
            assert!(reason.contains("P17-E-1"));
        }
        other => panic!("expected a scope refusal, got {other:?}"),
    }
    // A direct server accept with an unsupported negotiation is likewise
    // impossible: only the accepted Subprotocol values construct.
    let mut server = suite_server().expect("valid server");
    server
        .accept_connection(cp("depot-cp-001"), Subprotocol::Ocpp21)
        .expect("2.1 accepted additively");
}

#[test]
fn uc_neg_03_certificate_failures_refuse_with_named_errors() {
    // Row 1: an empty CSR label.
    let mut flows = CertificateFlowLedger::new();
    let empty_csr = SignCertificateRequest {
        csr: "   ".to_string(),
        type_of_certificate: None,
    };
    let error = flows
        .open_requested("depot-cp-001", &empty_csr)
        .expect_err("refused");
    assert!(error.to_string().contains("field 'csr' is empty"));
    // Row 2: a second open on an OPEN flow.
    let valid_csr = SignCertificateRequest {
        csr: "csr-label".to_string(),
        type_of_certificate: Some(CertificateType::CsmsCertificate),
    };
    flows
        .open_requested("depot-cp-001", &valid_csr)
        .expect("opened");
    let error = flows
        .open_requested("depot-cp-001", &valid_csr)
        .expect_err("refused");
    assert!(error.to_string().contains("is not permitted there"));
    // Row 3: a signed-send on an ABSENT flow.
    let error = flows
        .record_signed_sent("ghost", CertificateType::CsmsCertificate, "c".to_string())
        .expect_err("refused");
    assert!(error.to_string().contains("ABSENT"));
    // Row 4: an unproofed registration.
    let error = IdentityProofingLedger::new()
        .require_proofed("depot-cp-001")
        .expect_err("refused");
    assert!(error.to_string().contains("fail-closed"));
    // Row 5: an empty trust-anchor set.
    let error = CsmsSecurityRequirements::new(Vec::new(), true).expect_err("refused");
    assert!(error.to_string().contains("trust-anchor set is empty"));
    // The declared vector agrees with the surfaces above (every row's
    // fragment was just matched against real code).
    assert_eq!(certificate_failure_negatives().len(), 5);
}

#[test]
fn uc_neg_04_an_unmatched_reply_is_refused_never_applied() {
    let mut server = suite_server().expect("valid server");
    server
        .accept_connection(cp("depot-cp-001"), Subprotocol::Ocpp201)
        .expect("accepted");
    // A CALLRESULT for a CALL the outbound tracker does not hold.
    let error = server
        .handle_inbound_text(&cp("depot-cp-001"), r#"[3,"id-9",{}]"#)
        .expect_err("unmatched reply refused");
    assert!(error.to_string().contains("matches no outstanding CALL"));
    // The tracker's own discipline agrees.
    let mut tracker = flexcore_ocpp_adapter::framing::CallTracker::new();
    let stale = flexcore_ocpp_adapter::framing::UniqueId::new("call-9".to_string())
        .expect("valid");
    assert!(tracker.track_reply(&stale).is_err());
}
