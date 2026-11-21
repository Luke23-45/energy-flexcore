// FIM-F12.06 — Integration tests (WRITTEN-UNRUN; execution is a human
//             gate): the OTA COHORT ROLLOUT battery for feature F-12
//             (ledger FIM-F12.03/FIM-F12.05/FIM-F12.06), pinned counts.
//
// THE HONESTY BOUNDARY (restated so no runner can misread it): no rollout
// has ever run, no cohort exists, no fleet exists, no device report has
// ever been received — the coordinator here is driven through the
// `OtaReportReceiver` PORT TRAIT with typed fixture reports (never HTTP;
// the transport is a deployment concern). The cohort mechanism, the
// pre-registered thresholds, the declared-order progression, and the
// terminal audited abort are the EXISTING P18.06 orchestrator (consumed,
// never re-implemented); the evidence chains are the EXISTING P18.07
// vocabulary. The evidence namespace key is FIXED, LOUDLY-LABELLED
// TEST-GRADE material (never a deployment key; the P11.15/P18.03 key
// ceremonies are HUMAN GATES and no key material ships).
//
// THE MATRIX (each test hand-computed):
// - the happy path: canary → wider cohort, reported through the port
//   trait, advancing in the declared order to COMPLETE;
// - the abort threshold with PINNED COUNTS: 1 success then 1 failure =
//   500 permille = EQUAL (the recorded semantics keep going); a second
//   failure = 666 permille > 500 → ABORT; the abort FREEZES the rollout
//   (no authorization, no report, no advance) and is recorded on BOTH
//   audit layers (FLEET_ROLLOUT_ABORTED + OTA_ROLLOUT_FROZEN);
// - the per-cohort max-failing-devices tripwire freezes independently of
//   the rate threshold;
// - a rollback report counts as a FAILURE for the threshold and lands the
//   P18.07 Rollback evidence kind;
// - unassigned devices and duplicate terminal outcomes are refused;
// - every attempt lands in the evidence chain (Attempt → Verify →
//   Install → Result, or Attempt → Rollback), exportable, never a release
//   signature.

use std::sync::Arc;

use flexcore_application::rollout_orchestrator::{
    AbortThresholds, AttestationVerdict, MemoryRolloutAuditSink, RolloutPhase,
};
use flexcore_audit::chain::ChainKey;
use flexcore_control_plane::ota_rollout::{
    build_plan, OtaCohortDefinition, OtaDeviceReport, OtaReportReceiver, OtaRolloutAuditKind,
    OtaRolloutAuditSink, OtaRolloutCoordinator, OtaRolloutError, MemoryOtaRolloutAuditSink,
    OTA_REPORT_FAILURE_STATE_NAMES, OTA_REPORT_STATE_NAMES, OTA_ROLLOUT_STATEMENT,
};
use flexcore_ports::clock::Clock;

/// The injected clock sits FAR AHEAD of the fixture report instants: the
/// P18.07 evidence rule (ingest >= event) holds for every fixture report
/// without the tests advancing the clock.
const REMOTE_CLOCK_MILLIS: i64 = 5_000_000_000_000;

// --- Fixtures ------------------------------------------------------------------------

struct FixedClock(i64);
impl Clock for FixedClock {
    fn now_event_time(&self) -> flexcore_contracts::time_model::EventTime {
        flexcore_contracts::time_model::EventTime::from_unix_millis(self.0)
            .expect("valid instant")
    }
    fn now_ingest_time(&self) -> flexcore_contracts::time_model::IngestTime {
        flexcore_contracts::time_model::IngestTime::from_unix_millis(self.0)
            .expect("valid instant")
    }
}

/// The FIXED TEST-GRADE evidence namespace key (loudly labelled; never a
/// deployment key — the P11.15 test discipline).
fn test_namespace_key() -> ChainKey {
    ChainKey::from_bytes(&[0x3Cu8; 32]).expect("valid test key")
}

fn definition(id: &str, canary: bool, ids: &[&str], max_failing: Option<u32>)
-> OtaCohortDefinition {
    OtaCohortDefinition {
        cohort_id: id.to_string(),
        canary,
        device_ids: ids.iter().map(|id| id.to_string()).collect(),
        max_failing_devices: max_failing,
    }
}

fn thresholds() -> AbortThresholds {
    AbortThresholds {
        max_failure_rate_permille: 500,
        min_observations: 2,
    }
}

fn report(device: &str, attempt: &str, state: &str, at: i64) -> OtaDeviceReport {
    OtaDeviceReport {
        device_id: device.to_string(),
        attempt_id: attempt.to_string(),
        state: state.to_string(),
        failure_state: None,
        version: "2.0.0".to_string(),
        reported_at_unix_millis: at,
    }
}

fn failed_report(device: &str, attempt: &str, at: i64) -> OtaDeviceReport {
    OtaDeviceReport {
        device_id: device.to_string(),
        attempt_id: attempt.to_string(),
        state: "UPDATE_FAILED".to_string(),
        failure_state: Some("UPDATE_FAILURE_HEALTH_CHECK_FAILED".to_string()),
        version: "2.0.0".to_string(),
        reported_at_unix_millis: at,
    }
}

fn rolled_back_report(device: &str, attempt: &str, at: i64) -> OtaDeviceReport {
    OtaDeviceReport {
        device_id: device.to_string(),
        attempt_id: attempt.to_string(),
        state: "UPDATE_ROLLED_BACK".to_string(),
        failure_state: Some("UPDATE_FAILURE_HEALTH_CHECK_FAILED".to_string()),
        version: "2.0.0".to_string(),
        reported_at_unix_millis: at,
    }
}

fn passed_attestation() -> AttestationVerdict {
    AttestationVerdict::Passed {
        evidence_reference: "attest-evidence-f12".to_string(),
    }
}

struct Harness {
    coordinator: OtaRolloutCoordinator,
    own_sink: Arc<MemoryOtaRolloutAuditSink>,
    orchestrator_sink: Arc<MemoryRolloutAuditSink>,
}

fn harness(definitions: Vec<OtaCohortDefinition>, thresholds: AbortThresholds) -> Harness {
    let (plan, definitions) =
        build_plan("2.0.0", definitions, thresholds, 500).expect("the plan builds");
    let own_sink = Arc::new(MemoryOtaRolloutAuditSink::new());
    let orchestrator_sink = Arc::new(MemoryRolloutAuditSink::new());
    let coordinator = OtaRolloutCoordinator::new(
        plan,
        definitions,
        Box::new(orchestrator_sink.clone()),
        own_sink.clone() as Arc<dyn OtaRolloutAuditSink>,
        test_namespace_key(),
        Arc::new(FixedClock(REMOTE_CLOCK_MILLIS)),
    );
    Harness {
        coordinator,
        own_sink,
        orchestrator_sink,
    }
}

impl Harness {
    fn cleanup(&self) {
        // The harness is fully in-memory; nothing to clean up. The method
        // exists so every test reads the same lifecycle.
    }
}

// --- 1. The happy path (through the PORT TRAIT) ----------------------------------

#[test]
fn the_cohort_rollout_walks_canary_to_completion_through_the_report_port() {
    let mut h = harness(
        vec![
            definition("canary", true, &["d1", "d2"], None),
            definition("block-1", false, &["d3", "d4"], None),
        ],
        thresholds(),
    );
    h.coordinator.start().expect("starts");
    for device in ["d1", "d2"] {
        h.coordinator
            .authorize_device(device, &passed_attestation())
            .expect("authorized");
    }
    // The reports flow through the PORT TRAIT (never HTTP): the receiver
    // is taken as a trait object — the pinned FIM-F12.03 seam.
    fn drive(receiver: &mut dyn OtaReportReceiver, device: &str, attempt: &str) {
        receiver
            .receive(&report(device, attempt, "UPDATE_DOWNLOADING", 1_100))
            .expect("progress accepted");
        receiver
            .receive(&report(device, attempt, "UPDATE_VERIFIED_PENDING_INSTALL", 1_200))
            .expect("verify evidence accepted");
        receiver
            .receive(&report(device, attempt, "UPDATE_HEALTH_CHECK_PENDING", 1_300))
            .expect("install evidence accepted");
        receiver
            .receive(&report(device, attempt, "UPDATE_ACTIVE", 1_400))
            .expect("terminal accepted");
    }
    {
        let receiver: &mut dyn OtaReportReceiver = &mut h.coordinator;
        drive(receiver, "d1", "attempt-d1");
        drive(receiver, "d2", "attempt-d2");
    }
    assert_eq!(h.coordinator.phase(), RolloutPhase::InProgress);
    let next = h
        .coordinator
        .advance_cohort("J. Doe (ops)", "canary clean: 2/2 success")
        .expect("advances to the wider cohort");
    assert_eq!(next, "block-1");
    for device in ["d3", "d4"] {
        h.coordinator
            .authorize_device(device, &passed_attestation())
            .expect("authorized");
        h.coordinator
            .receive(&report(device, &format!("attempt-{device}"), "UPDATE_ACTIVE", 1_500))
            .expect("terminal accepted");
    }
    assert!(matches!(
        h.coordinator.advance_cohort("J. Doe (ops)", "block-1 clean"),
        Err(OtaRolloutError::OrchestratorRefused { code }) if code == "ROLLOUT_COMPLETE"
    ));
    assert_eq!(h.coordinator.phase(), RolloutPhase::Complete);
    assert!(!h.coordinator.is_frozen());
    h.cleanup();
    // THE AUDIT TRAIL (FIM-F12.05): cohort assignments, per-device
    // assignments, reports, progress, evidence — coordinator side.
    let own = h.own_sink.records();
    assert!(own.iter().any(|r| r.kind == OtaRolloutAuditKind::CohortAssigned
        && r.subject == "canary"
        && r.detail.contains("d1,d2")));
    assert!(own.iter().any(|r| r.kind == OtaRolloutAuditKind::DeviceAssigned
        && r.subject == "block-1/d4"));
    assert!(own.iter().any(|r| r.kind == OtaRolloutAuditKind::ReportReceived));
    assert!(own.iter().any(|r| r.kind == OtaRolloutAuditKind::ProgressRecorded));
    assert!(own.iter().any(|r| r.kind == OtaRolloutAuditKind::EvidenceRecorded));
    // The orchestrator's own records rode its EXISTING shape.
    assert!(h
        .orchestrator_sink
        .records()
        .iter()
        .any(|r| r.kind.name() == "FLEET_ROLLOUT_DEVICE_AUTHORIZED"));
    // The per-attempt evidence chains: 4 records each (Attempt, Verify,
    // Install, Result).
    let (count, head) = h
        .coordinator
        .attempt_evidence("attempt-d1")
        .expect("the attempt's evidence exists");
    assert_eq!(count, 4);
    assert!(head.len() == 64, "the head digest is a SHA-256 hex string");
}

// --- 2. The abort threshold (PINNED COUNTS) and the freeze -------------------------

#[test]
fn a_failing_canary_aborts_at_the_threshold_and_the_abort_freezes_the_rollout() {
    let mut h = harness(
        vec![
            definition("canary", true, &["d1", "d2", "d3"], None),
            definition("block-1", false, &["d4", "d5"], None),
        ],
        thresholds(),
    );
    h.coordinator.start().expect("starts");
    for device in ["d1", "d2", "d3"] {
        h.coordinator
            .authorize_device(device, &passed_attestation())
            .expect("authorized");
    }
    // d1 succeeds. d2 fails: 1/2 = 500 permille = EQUAL — the recorded
    // semantics keep going. d3 fails: 2/3 = 666 permille > 500 → ABORT.
    h.coordinator
        .receive(&report("d1", "a1", "UPDATE_ACTIVE", 1_100))
        .expect("success recorded");
    assert!(!h.coordinator.is_frozen(), "an equal rate does not abort");
    h.coordinator
        .receive(&failed_report("d2", "a2", 1_200))
        .expect("failure recorded");
    assert!(!h.coordinator.is_frozen(), "1/2 = 500 permille is not > 500");
    h.coordinator
        .receive(&failed_report("d3", "a3", 1_300))
        .expect("failure recorded");
    assert_eq!(h.coordinator.phase(), RolloutPhase::Aborted);
    assert!(h.coordinator.is_frozen());
    assert!(h
        .coordinator
        .freeze_reason()
        .expect("the freeze reason is recorded")
        .contains("ABORTED"));
    // THE FREEZE: no further authorization, report, or advance.
    assert!(matches!(
        h.coordinator.authorize_device("d4", &passed_attestation()),
        Err(OtaRolloutError::FrozenRollout { .. })
    ));
    assert!(matches!(
        h.coordinator.receive(&report("d4", "a4", "UPDATE_DOWNLOADING", 1_400)),
        Err(OtaRolloutError::FrozenRollout { .. })
    ));
    assert!(matches!(
        h.coordinator.advance_cohort("J. Doe (ops)", "ignored — frozen"),
        Err(OtaRolloutError::FrozenRollout { .. })
    ));
    h.cleanup();
    // The orchestrator's abort record AND the coordinator's freeze record
    // are both present (one event, two honest layers).
    assert!(h
        .orchestrator_sink
        .records()
        .iter()
        .any(|r| r.kind.name() == "FLEET_ROLLOUT_ABORTED"));
    assert!(h
        .own_sink
        .records()
        .iter()
        .any(|r| r.kind == OtaRolloutAuditKind::Frozen));
}

// --- 3. The per-cohort count tripwire ------------------------------------------------

#[test]
fn the_per_cohort_max_failing_devices_tripwire_freezes_independently() {
    // A rate threshold that never fires at these counts (1000 permille)
    // + a strict per-cohort cap of ZERO failing devices.
    let mut h = harness(
        vec![
            definition("canary", true, &["d1", "d2"], Some(0)),
            definition("block-1", false, &["d3"], None),
        ],
        AbortThresholds { max_failure_rate_permille: 1000, min_observations: 1 },
    );
    h.coordinator.start().expect("starts");
    for device in ["d1", "d2"] {
        h.coordinator
            .authorize_device(device, &passed_attestation())
            .expect("authorized");
    }
    h.coordinator
        .receive(&report("d1", "a1", "UPDATE_ACTIVE", 1_100))
        .expect("success recorded");
    h.coordinator
        .receive(&failed_report("d2", "a2", 1_200))
        .expect("failure recorded");
    // The rate is 1/2 = 500 permille <= 1000 — the orchestrator did NOT
    // abort; the COUNT tripwire did (1 > 0).
    assert_eq!(h.coordinator.phase(), RolloutPhase::InProgress);
    assert!(h.coordinator.is_frozen());
    assert!(h
        .coordinator
        .freeze_reason()
        .expect("the freeze reason")
        .contains("max_failing_devices=0"));
    assert!(matches!(
        h.coordinator.receive(&report("d1", "a3", "UPDATE_ACTIVE", 1_300)),
        Err(OtaRolloutError::FrozenRollout { .. })
    ));
    h.cleanup();
}

// --- 4. The rollback report maps to a threshold failure ------------------------------

#[test]
fn a_rollback_report_counts_as_a_failure_and_lands_rollback_evidence() {
    let mut h = harness(vec![definition("canary", true, &["d1"], None)], thresholds());
    h.coordinator.start().expect("starts");
    h.coordinator
        .authorize_device("d1", &passed_attestation())
        .expect("authorized");
    let receipt = h
        .coordinator
        .receive(&rolled_back_report("d1", "a1", 1_100))
        .expect("accepted");
    assert!(receipt.outcome_recorded);
    assert_eq!(receipt.outcome_name, "DEVICE_UPDATE_ROLLED_BACK");
    // The snapshot counts the rolled-back device.
    let snapshot = h.coordinator.fleet_snapshot();
    assert_eq!(snapshot.cohorts[0].rolled_back, 1);
    assert_eq!(snapshot.cohorts[0].failed, 0);
    // The rollback evidence record is the P18.07 Rollback kind (Attempt +
    // Rollback = 2 records).
    let (count, _) = h.coordinator.attempt_evidence("a1").expect("evidence");
    assert_eq!(count, 2);
    h.cleanup();
    assert!(h
        .own_sink
        .records()
        .iter()
        .any(|r| r.kind == OtaRolloutAuditKind::EvidenceRecorded && r.detail.contains("records=2")));
}

// --- 5. The refusals -----------------------------------------------------------------

#[test]
fn unassigned_devices_and_duplicate_outcomes_are_refused() {
    let mut h = harness(vec![definition("canary", true, &["d1"], None)], thresholds());
    h.coordinator.start().expect("starts");
    // An unassigned device (never authorized) reports — refused.
    assert!(matches!(
        h.coordinator.receive(&report("dX", "aX", "UPDATE_ACTIVE", 1_100)),
        Err(OtaRolloutError::DeviceNotAssigned { .. })
    ));
    h.coordinator
        .authorize_device("d1", &passed_attestation())
        .expect("authorized");
    h.coordinator
        .receive(&report("d1", "a1", "UPDATE_ACTIVE", 1_100))
        .expect("first terminal accepted");
    // A duplicate terminal outcome — refused (the append-once rule).
    assert!(matches!(
        h.coordinator.receive(&report("d1", "a1", "UPDATE_ACTIVE", 1_200)),
        Err(OtaRolloutError::OrchestratorRefused { code }) if code == "ROLLOUT_DEVICE_ALREADY_OUTCOME"
    ));
    h.cleanup();
    assert!(h
        .own_sink
        .records()
        .iter()
        .any(|r| r.kind == OtaRolloutAuditKind::ReportRefused));
}

// --- 6. The evidence chain is exportable (the P18.07 pack, consumed) ------------------

#[test]
fn every_attempt_lands_in_an_exportable_evidence_chain() {
    let mut h = harness(vec![definition("canary", true, &["d1", "d2"], None)], thresholds());
    h.coordinator.start().expect("starts");
    for device in ["d1", "d2"] {
        h.coordinator
            .authorize_device(device, &passed_attestation())
            .expect("authorized");
    }
    h.coordinator
        .receive(&report("d1", "a1", "UPDATE_DOWNLOADING", 1_100))
        .expect("accepted");
    h.coordinator
        .receive(&report("d1", "a1", "UPDATE_VERIFIED_PENDING_INSTALL", 1_200))
        .expect("accepted");
    h.coordinator
        .receive(&report("d1", "a1", "UPDATE_HEALTH_CHECK_PENDING", 1_300))
        .expect("accepted");
    h.coordinator
        .receive(&report("d1", "a1", "UPDATE_ACTIVE", 1_400))
        .expect("accepted");
    // The failing attempt goes straight to a refusal (verify refused →
    // UPDATE_FAILED): Attempt + Result/FAILED.
    h.coordinator
        .receive(&failed_report("d2", "a2", 1_500))
        .expect("accepted");
    let (count_a1, _) = h.coordinator.attempt_evidence("a1").expect("evidence");
    assert_eq!(count_a1, 4, "Attempt, Verify, Install, Result");
    let (count_a2, _) = h.coordinator.attempt_evidence("a2").expect("evidence");
    assert_eq!(count_a2, 2, "Attempt, Result/FAILED");
    h.cleanup();
    // The audit line names the chain per attempt (the P18.07 shape).
    assert!(h
        .own_sink
        .records()
        .iter()
        .any(|r| r.kind == OtaRolloutAuditKind::EvidenceRecorded
            && r.subject == "d2/a2"));
}

// --- 7. The statement and vocabulary pins ----------------------------------------------

#[test]
fn the_statement_and_the_report_vocabularies_are_pinned() {
    assert!(OTA_ROLLOUT_STATEMENT.contains("EXISTING P18.06 orchestrator"));
    assert!(OTA_ROLLOUT_STATEMENT.contains("never HTTP"));
    assert!(OTA_ROLLOUT_STATEMENT.contains("FREEZES the rollout"));
    assert!(OTA_ROLLOUT_STATEMENT.contains("no rollout has ever run"));
    // The closed vocabularies are the P18.04 names, consumed by citation.
    assert_eq!(OTA_REPORT_STATE_NAMES.len(), 10);
    assert!(OTA_REPORT_STATE_NAMES.contains(&"UPDATE_ACTIVE"));
    assert!(OTA_REPORT_STATE_NAMES.contains(&"UPDATE_ROLLED_BACK"));
    assert!(OTA_REPORT_FAILURE_STATE_NAMES.contains(&"UPDATE_FAILURE_ROLLBACK_FAILED"));
    assert!(OTA_REPORT_FAILURE_STATE_NAMES.contains(&"UPDATE_FAILURE_HEALTH_CHECK_FAILED"));
}
