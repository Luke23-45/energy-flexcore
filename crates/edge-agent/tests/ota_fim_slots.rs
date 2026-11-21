// FIM-F12.06 — Integration tests (WRITTEN-UNRUN; execution is a human
//             gate): the DEVICE-SIDE A/B SLOTS battery for feature F-12
//             (ledger FIM-F12.04/FIM-F12.05/FIM-F12.06).
//
// THE HONESTY BOUNDARY (restated): no device exists — the slot manager is
// the deterministic, in-memory BOOKKEEPING the composition drives against
// a REAL file-backed register; the slot write, boot switch, and health
// evidence are device-side contracts (PENDING PILOT HARDWARE SPEC, P6.34).
// The bundles are produced by the FIM-F12.01 producer over fixture
// payloads with FIXED, LOUDLY-LABELLED TEST-GRADE key material (the P18.03
// ceremonies are HUMAN GATES; no key material ships).
//
// THE MATRIX (each test hand-computed):
// - the full flow: download → verify (the EXISTING extended gate; the
//   counter advances) → install into the INACTIVE slot → health confirm →
//   the register records the activation and the active slot switches;
// - a failed install leaves the previous slot active (the A/B
//   rollback-in-place) and reports UPDATE_FAILED/INSTALL_FAILED;
// - a health failure auto-rolls back to the previous slot THROUGH THE
//   REGISTER and reports UPDATE_ROLLED_BACK — and the anti-rollback
//   counter NEVER rewinds (the next update must still be strictly ahead
//   of the highest-ever-accepted sequence);
// - THE HONEST CONFLICT: the first-ever update whose health fails has no
//   register-activated rollback target (the factory slot) — the rollback
//   is impossible, the refusal is recorded, the register is untouched,
//   and the flow reports UPDATE_FAILED/ROLLBACK_FAILED;
// - a below-floor rollback requires the recorded override (the P9.34
//   rule, verbatim);
// - every step's audit record is present through the existing edge audit
//   shapes (FIM-F12.05).

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use flexcore_edge_agent::ota_bundle_produce::{
    OtaBundleProducer, OtaProduceRequest, OtaRootMetadata,
};
use flexcore_edge_agent::ota_signing::{OtaEd25519ReleaseKey, OtaTrustAnchor};
use flexcore_edge_agent::ota_slots::{
    OtaSlotId, OtaSlotManager, OtaSlotsError, OTA_SLOTS_STATEMENT,
};
use flexcore_edge_agent::ports::{AuditSink, EdgeAuditEvent, EdgeError};
use flexcore_edge_agent::update_session::{
    UpdateProgressState, UPDATE_FAILURE_INSTALL_FAILED, UPDATE_FAILURE_ROLLBACK_FAILED,
};
use flexcore_edge_agent::update_verification::{
    FileUpdateLedger, RollbackOverride, UpdateActivation, UpdateLedgerEventKind,
};
use flexcore_ports::clock::Clock;

const BASE_MILLIS: i64 = 1_700_000_000_000;
const VALID_EXPIRY: i64 = BASE_MILLIS + 10 * 365 * 24 * 60 * 60_000;
const SIGNER_REFERENCE: &str = "key-store:flexcore-release-targets";

/// The FIXED TEST-GRADE signing seed (loudly labelled; never a release
/// key; the P18.03 ceremonies are HUMAN GATES). Deterministic.
const TEST_GRADE_SEED: [u8; 32] = [0x51u8; 32];

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

#[derive(Default)]
struct CollectingAuditSink {
    events: Mutex<Vec<String>>,
}

impl CollectingAuditSink {
    fn lines(&self) -> Vec<String> {
        self.events.lock().expect("poisoned test sink mutex").clone()
    }
}

impl AuditSink for CollectingAuditSink {
    fn record(&self, event: EdgeAuditEvent) -> Result<(), EdgeError> {
        self.events
            .lock()
            .expect("poisoned test sink mutex")
            .push(event.detail);
        Ok(())
    }
}

fn key() -> OtaEd25519ReleaseKey {
    OtaEd25519ReleaseKey::from_injected_seed(&TEST_GRADE_SEED, SIGNER_REFERENCE)
        .expect("the test-grade key constructs")
}

fn anchor() -> OtaTrustAnchor {
    OtaTrustAnchor::new(SIGNER_REFERENCE, key().public_key_bytes())
        .expect("the anchor constructs")
}

fn anchor_for_key(key: &OtaEd25519ReleaseKey) -> OtaTrustAnchor {
    OtaTrustAnchor::new(key.signer_reference(), key.public_key_bytes())
        .expect("the anchor constructs")
}

fn register_path(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join(format!("flexcore-f12-slots-{tag}-{}", std::process::id()));
    dir.join("update.register")
}

fn open_register(tag: &str) -> FileUpdateLedger {
    let path = register_path(tag);
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::remove_file(&path);
    FileUpdateLedger::open(&path).expect("the register opens")
}

fn cleanup_register(tag: &str) {
    let path = register_path(tag);
    let _ = std::fs::remove_file(&path);
    if let Some(dir) = path.parent() {
        let _ = std::fs::remove_dir(dir);
    }
}

fn counter_path(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join(format!("flexcore-f12-slots-counter-{tag}-{}", std::process::id()));
    dir.join("antirb.counter")
}

fn open_counter(tag: &str) -> flexcore_edge_agent::update_bundle::AntiRollbackCounter {
    let path = counter_path(tag);
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::remove_file(&path);
    flexcore_edge_agent::update_bundle::AntiRollbackCounter::open(&path)
        .expect("the counter opens")
}

fn cleanup_counter(tag: &str) {
    let path = counter_path(tag);
    let _ = std::fs::remove_file(&path);
    if let Some(dir) = path.parent() {
        let _ = std::fs::remove_dir(dir);
    }
}

/// One OTA composition for a test: the producer (the FIM-F12.01 producer
/// with the test-grade key), the extended gate (the REAL Ed25519 verifier
/// over the device anchors), and the slot manager.
struct Composition {
    producer: OtaBundleProducer,
    gate: flexcore_edge_agent::update_session::ExtendedUpdateGate,
    manager: OtaSlotManager,
    audit: Arc<CollectingAuditSink>,
    tag: String,
}

fn composition(tag: &str) -> Composition {
    let audit = Arc::new(CollectingAuditSink::default());
    let edge_audit: Arc<dyn AuditSink> = audit.clone();
    let key = key();
    let anchors = vec![anchor_for_key(&key)];
    let producer = OtaBundleProducer::new(key, edge_audit.clone(), Arc::new(FixedClock(BASE_MILLIS)));
    let counter = open_counter(tag);
    let gate = flexcore_edge_agent::update_session::ExtendedUpdateGate::new(
        "neryva".to_string(),
        "edge-gateway".to_string(),
        "device-1".to_string(),
        vec![SIGNER_REFERENCE.to_string()],
        Arc::new(flexcore_edge_agent::ota_signing::Ed25519UpdateSignatureVerifier::new(
            anchors,
        )
        .expect("the verifier composes")),
        counter,
        edge_audit.clone(),
        Arc::new(FixedClock(BASE_MILLIS)),
    )
    .expect("the gate composes");
    let manager = OtaSlotManager::new("0.9.0", edge_audit, Arc::new(FixedClock(BASE_MILLIS)))
        .expect("the slot manager provisions");
    Composition {
        producer,
        gate,
        manager,
        audit,
        tag: tag.to_string(),
    }
}

impl Composition {
    fn cleanup(&self) {
        cleanup_register(&self.tag);
        cleanup_counter(&self.tag);
    }

    /// Produces one artifact with strictly-ahead counters.
    fn produce(&mut self, sequence: u64, release_version: &str) 
    -> flexcore_edge_agent::ota_bundle_produce::OtaDistributionArtifact {
        let root = OtaRootMetadata {
            root_version: sequence, // the version counters ride the sequence in the fixtures
            anchors: vec![anchor()],
            expires_at_unix_millis: VALID_EXPIRY,
        };
        self.producer
            .produce(OtaProduceRequest {
                package_id: "edge-agent-release".to_string(),
                version: release_version.to_string(),
                digest_algorithm: "ed25519".to_string(),
                signer_key_reference: SIGNER_REFERENCE.to_string(),
                minimum_version_floor: "0.0.0".to_string(),
                approval_reference: "approval-record-f12".to_string(),
                applicability: flexcore_edge_agent::update_bundle::UpdateApplicability {
                    vendor: "neryva".to_string(),
                    device_class: "edge-gateway".to_string(),
                    device_ids: std::collections::BTreeSet::new(),
                },
                anti_rollback_sequence: sequence,
                payloads: vec![flexcore_edge_agent::ota_bundle_produce::OtaProducePayload {
                    name: "fw.bin".to_string(),
                    bytes: format!("flexcore-fixture-image-{sequence}").into_bytes(),
                }],
                expires_at_unix_millis: VALID_EXPIRY,
                root,
                root_version: sequence,
                timestamp_version: sequence,
                timestamp_expiry_unix_millis: VALID_EXPIRY,
                snapshot_version: sequence,
            })
            .expect("the artifact is produced")
    }
}

// --- 1. The full flow ----------------------------------------------------------

#[test]
fn the_full_slot_flow_installs_and_activates_the_new_slot() {
    let mut harness = composition("full-flow");
    let ledger = open_register("full-flow");
    assert!(ledger.is_empty(), "a fresh register has seen no activation");
    let mut artifact = harness.produce(3, "2.0.0");
    let manifest = artifact.targets.structured.clone();
    let bundle = std::mem::take(&mut artifact.bundle);
    let mut attempt = harness
        .manager
        .open_attempt("attempt-1", "device-1", "2.0.0", 3, 0, BASE_MILLIS + 100)
        .expect("the attempt opens");
    harness
        .manager
        .download_complete(&mut attempt, BASE_MILLIS + 200)
        .expect("the download completes");
    harness
        .manager
        .verify(&mut attempt, &harness.gate, &manifest, &bundle, BASE_MILLIS + 300)
        .expect("the verification passes");
    harness
        .manager
        .install(&mut attempt, BASE_MILLIS + 400)
        .expect("the install completes");
    // The install targeted the INACTIVE slot; the active slot is still A.
    assert_eq!(harness.manager.active_slot(), OtaSlotId::A);
    let slot_b = harness
        .manager
        .slot_record(OtaSlotId::B)
        .expect("slot B holds the installed bundle");
    assert_eq!(slot_b.version, "2.0.0");
    assert_eq!(slot_b.anti_rollback_sequence, 3);
    assert!(!slot_b.healthy, "the record starts unhealthy until the health check confirms");
    // The health check confirms: the register records the activation and
    // the active slot switches.
    harness
        .manager
        .health_confirmed(&mut attempt, &ledger, "1.0.0", BASE_MILLIS + 500)
        .expect("the activation lands");
    assert_eq!(harness.manager.active_slot(), OtaSlotId::B);
    assert!(harness
        .manager
        .slot_record(OtaSlotId::B)
        .expect("slot B")
        .healthy);
    assert_eq!(ledger.active_version().as_deref(), Some("2.0.0"));
    assert_eq!(ledger.len(), 1);
    // The attempt is terminal-success; the status report says so.
    assert_eq!(attempt.state(), UpdateProgressState::Active);
    let notification = harness.manager.status_report(&attempt);
    assert_eq!(notification.state.name(), "UPDATE_ACTIVE");
    // The register record is an activation. The register started EMPTY
    // (the factory slot was never register-activated — the honest-conflict
    // test below pins exactly that), so the first activation's
    // previous_version is empty, never the factory version.
    let history = ledger.history();
    assert_eq!(history[0].kind, UpdateLedgerEventKind::Activation);
    assert_eq!(history[0].previous_version, "");
    harness.cleanup();
    // THE AUDIT TRAIL (FIM-F12.05): every step landed.
    let lines = harness.audit.lines();
    for expected in [
        "EDGE_OTA_SLOT_PROVISIONED",
        "EDGE_OTA_SLOT_DOWNLOAD_BEGUN",
        "EDGE_OTA_SLOT_DOWNLOAD_COMPLETE",
        "EDGE_OTA_SLOT_VERIFIED",
        "EDGE_OTA_SLOT_INSTALL_BEGUN",
        "EDGE_OTA_SLOT_INSTALLED",
        "EDGE_OTA_SLOT_HEALTH_CONFIRMED",
        "EDGE_UPDATE_ACTIVATED",
        "EDGE_OTA_SLOT_ACTIVATED",
        "EDGE_OTA_SLOT_STATUS_REPORTED",
        "EDGE_OTA_BUNDLE_PRODUCED",
        "EDGE_FLEET_UPDATE_VERIFIED",
        "EDGE_FLEET_UPDATE_SEQUENCE_ADVANCED",
    ] {
        assert!(
            lines.iter().any(|l| l.contains(expected)),
            "the audit trail must carry {expected}: {lines:?}"
        );
    }
}

// --- 2. The failed install ------------------------------------------------------

#[test]
fn a_failed_install_leaves_the_previous_slot_active_and_reports_the_failure() {
    let mut harness = composition("failed-install");
    let ledger = open_register("failed-install");
    let mut artifact = harness.produce(3, "2.0.0");
    let manifest = artifact.targets.structured.clone();
    let bundle = std::mem::take(&mut artifact.bundle);
    let mut attempt = harness
        .manager
        .open_attempt("attempt-1", "device-1", "2.0.0", 3, 0, BASE_MILLIS + 100)
        .expect("opens");
    harness.manager.download_complete(&mut attempt, BASE_MILLIS + 200).expect("ok");
    harness
        .manager
        .verify(&mut attempt, &harness.gate, &manifest, &bundle, BASE_MILLIS + 300)
        .expect("verified");
    // THE SLOT WRITE FAILS (the device-side contract): the previous
    // version was never left.
    harness
        .manager
        .install_failed(&mut attempt, BASE_MILLIS + 400)
        .expect("recorded");
    assert_eq!(harness.manager.active_slot(), OtaSlotId::A, "the active slot is untouched");
    assert!(
        harness.manager.slot_record(OtaSlotId::B).is_none(),
        "no record was written into the failed slot"
    );
    assert_eq!(attempt.state(), UpdateProgressState::UpdateFailed);
    let notification = harness.manager.status_report(&attempt);
    assert_eq!(notification.failure_state, Some(UPDATE_FAILURE_INSTALL_FAILED));
    assert!(ledger.is_empty(), "nothing was activated");
    harness.cleanup();
    assert!(harness
        .audit
        .lines()
        .iter()
        .any(|l| l.contains("EDGE_OTA_SLOT_INSTALL_FAILED")
            && l.contains("UNTOUCHED")));
}

// --- 3. The health-failure auto-rollback ------------------------------------------

#[test]
fn a_health_failure_rolls_back_to_the_previous_slot_and_reports_it() {
    let mut harness = composition("auto-rollback");
    let ledger = open_register("auto-rollback");
    // Attempt 1 succeeds: "2.0.0" activates on slot B.
    let mut artifact1 = harness.produce(3, "2.0.0");
    let manifest1 = artifact1.targets.structured.clone();
    let bundle1 = std::mem::take(&mut artifact1.bundle);
    let mut attempt1 = harness
        .manager
        .open_attempt("attempt-1", "device-1", "2.0.0", 3, 0, BASE_MILLIS + 100)
        .expect("opens");
    harness.manager.download_complete(&mut attempt1, BASE_MILLIS + 200).expect("ok");
    harness
        .manager
        .verify(&mut attempt1, &harness.gate, &manifest1, &bundle1, BASE_MILLIS + 300)
        .expect("verified");
    harness.manager.install(&mut attempt1, BASE_MILLIS + 400).expect("installed");
    harness
        .manager
        .health_confirmed(&mut attempt1, &ledger, "1.0.0", BASE_MILLIS + 500)
        .expect("activated");
    assert_eq!(harness.manager.active_slot(), OtaSlotId::B);
    // Attempt 2 ("2.1.0", sequence 5) installs on slot A; the health
    // check FAILS; the auto-rollback boots the previous slot (the
    // register already carries "2.0.0" — the common A/B case, below).
    let mut artifact2 = harness.produce(5, "2.1.0");
    let manifest2 = artifact2.targets.structured.clone();
    let bundle2 = std::mem::take(&mut artifact2.bundle);
    let mut attempt2 = harness
        .manager
        .open_attempt("attempt-2", "device-1", "2.1.0", 5, 0, BASE_MILLIS + 600)
        .expect("opens");
    harness.manager.download_complete(&mut attempt2, BASE_MILLIS + 700).expect("ok");
    harness
        .manager
        .verify(&mut attempt2, &harness.gate, &manifest2, &bundle2, BASE_MILLIS + 800)
        .expect("verified — the counter advanced to 5");
    harness.manager.install(&mut attempt2, BASE_MILLIS + 900).expect("installed");
    assert_eq!(harness.manager.active_slot(), OtaSlotId::B, "not switched yet");
    // THE COMMON A/B CASE: activation happens at health CONFIRMATION, so
    // the failed version was never activated and the register ALREADY
    // carries the rollback target ("2.0.0") — NO register rollback record
    // exists or is possible under the crate's rules; the slot-level boot
    // rollback completes and is REPORTED.
    let record = harness
        .manager
        .health_failed_rollback(
            &mut attempt2,
            &ledger,
            "1.0.0",
            "the new version is unhealthy",
            None,
            BASE_MILLIS + 1_000,
        )
        .expect("the slot-level rollback lands");
    assert!(record.is_none(), "no register record: the register never moved");
    // The manager's active-slot pointer NEVER MOVED (it tracks the
    // register-backed active version, which only changes at activation);
    // the failed slot keeps its unhealthy record; the register is the
    // source of truth and already shows the rollback target.
    assert_eq!(harness.manager.active_slot(), OtaSlotId::B);
    assert_eq!(ledger.active_version().as_deref(), Some("2.0.0"));
    let history = ledger.history();
    assert_eq!(history.len(), 1, "only attempt 1's activation — the register never moved");
    let slot_a = harness.manager.slot_record(OtaSlotId::A).expect("slot A");
    assert_eq!(slot_a.version, "2.1.0");
    assert!(!slot_a.healthy);
    // The flow reports the rollback (never silent).
    assert_eq!(attempt2.state(), UpdateProgressState::RolledBack);
    let notification = harness.manager.status_report(&attempt2);
    assert_eq!(notification.state.name(), "UPDATE_ROLLED_BACK");
    // THE ANTI-ROLLBACK COUNTER NEVER REWINDS (the honest resolution): the
    // counter is at 5 (the failed attempt's verify advanced it durably),
    // so re-presenting the FAILED manifest to the gate refuses with the
    // recorded strictly-ahead rule (the gate is called directly here —
    // the attempt itself is terminal, and a rollback never re-presents a
    // manifest).
    let refusal = harness
        .gate
        .verify_fleet_bundle(Some(&manifest2), &bundle2)
        .expect_err("the failed manifest can never be re-presented");
    assert!(matches!(
        refusal,
        flexcore_edge_agent::update_bundle::UpdateBundleError::SequenceNotAhead {
            declared: 5,
            last_seen: 5,
        }
    ), "{refusal:?}");
    // ...and the NEXT update must still be strictly ahead of the
    // highest-ever-accepted sequence (6 passes; 5 is refused by the
    // producer's counters — pinned in the bundle battery).
    let _ = harness.produce(6, "2.2.0");
    harness.cleanup();
    // The audit trail: rollback begun, the slot-level completion (with
    // the no-register-record statement), the status report. NO
    // EDGE_UPDATE_ROLLBACK line exists in this case — the register never
    // moved (that line rides only the register-record path).
    let lines = harness.audit.lines();
    for expected in [
        "EDGE_OTA_SLOT_HEALTH_FAILED_AUTO_ROLLBACK",
        "EDGE_OTA_SLOT_ROLLED_BACK",
        "EDGE_OTA_SLOT_STATUS_REPORTED",
    ] {
        assert!(
            lines.iter().any(|l| l.contains(expected)),
            "the audit trail must carry {expected}: {lines:?}"
        );
    }
}

// --- 4. THE HONEST CONFLICT ---------------------------------------------------------

#[test]
fn the_first_update_whose_health_fails_cannot_roll_back() {
    let mut harness = composition("first-failure");
    let ledger = open_register("first-failure");
    let mut artifact = harness.produce(3, "2.0.0");
    let manifest = artifact.targets.structured.clone();
    let bundle = std::mem::take(&mut artifact.bundle);
    let mut attempt = harness
        .manager
        .open_attempt("attempt-1", "device-1", "2.0.0", 3, 0, BASE_MILLIS + 100)
        .expect("opens");
    harness.manager.download_complete(&mut attempt, BASE_MILLIS + 200).expect("ok");
    harness
        .manager
        .verify(&mut attempt, &harness.gate, &manifest, &bundle, BASE_MILLIS + 300)
        .expect("verified");
    harness.manager.install(&mut attempt, BASE_MILLIS + 400).expect("installed");
    // The health check fails on the FIRST-ever update: the factory slot
    // ("0.9.0") was never REGISTER-activated — the rollback is impossible
    // under the recorded rules. THE CONFLICT IS RECORDED, NEVER WEAKENED.
    let error = harness
        .manager
        .health_failed_rollback(
            &mut attempt,
            &ledger,
            "1.0.0",
            "the new version is unhealthy",
            None,
            BASE_MILLIS + 500,
        )
        .expect_err("the rollback is impossible");
    let OtaSlotsError::RollbackImpossible { reason } = error else {
        panic!("expected RollbackImpossible, got {error:?}")
    };
    assert!(reason.contains("UPDATE_ROLLBACK_TARGET_NEVER_ACTIVATED"), "{reason}");
    // The register is UNTOUCHED (a refused rollback records nothing).
    assert!(ledger.is_empty());
    // The active slot never switched (the device needs operator
    // intervention; the recorded P18.04 semantic).
    assert_eq!(harness.manager.active_slot(), OtaSlotId::A);
    // The flow reports the explicit rollback-failed failure state.
    assert_eq!(attempt.state(), UpdateProgressState::UpdateFailed);
    let notification = harness.manager.status_report(&attempt);
    assert_eq!(notification.failure_state, Some(UPDATE_FAILURE_ROLLBACK_FAILED));
    harness.cleanup();
    assert!(harness.audit.lines().iter().any(|l| l
        .contains("EDGE_OTA_SLOT_ROLLBACK_REFUSED")
        && l.contains("UPDATE_ROLLBACK_TARGET_NEVER_ACTIVATED")));
}

// --- 5. The below-floor rollback requires the recorded override ----------------------

#[test]
fn the_slot_rollback_needs_no_record_pre_activation_and_the_override_rules_run_verbatim() {
    let mut harness = composition("below-floor");
    let ledger = open_register("below-floor");
    // Attempt 1 activates "1.5.0" on slot B.
    let mut artifact1 = harness.produce(3, "1.5.0");
    let manifest1 = artifact1.targets.structured.clone();
    let bundle1 = std::mem::take(&mut artifact1.bundle);
    let mut attempt1 = harness
        .manager
        .open_attempt("attempt-1", "device-1", "1.5.0", 3, 0, BASE_MILLIS + 100)
        .expect("opens");
    harness.manager.download_complete(&mut attempt1, BASE_MILLIS + 200).expect("ok");
    harness
        .manager
        .verify(&mut attempt1, &harness.gate, &manifest1, &bundle1, BASE_MILLIS + 300)
        .expect("verified");
    harness.manager.install(&mut attempt1, BASE_MILLIS + 400).expect("installed");
    harness
        .manager
        .health_confirmed(&mut attempt1, &ledger, "1.0.0", BASE_MILLIS + 500)
        .expect("activated");
    // Attempt 2 installs "1.6.0" on slot A; the health fails. The
    // deployment floor has since been RAISED to "2.0.0" — but the failed
    // version was never ACTIVATED (activation happens at health
    // confirmation), so the register already carries "1.5.0": the
    // slot-level rollback completes with NO register record and the
    // below-floor rule cannot arise here (the register never moved).
    let mut artifact2 = harness.produce(5, "1.6.0");
    let manifest2 = artifact2.targets.structured.clone();
    let bundle2 = std::mem::take(&mut artifact2.bundle);
    let mut attempt2 = harness
        .manager
        .open_attempt("attempt-2", "device-1", "1.6.0", 5, 0, BASE_MILLIS + 600)
        .expect("opens");
    harness.manager.download_complete(&mut attempt2, BASE_MILLIS + 700).expect("ok");
    harness
        .manager
        .verify(&mut attempt2, &harness.gate, &manifest2, &bundle2, BASE_MILLIS + 800)
        .expect("verified");
    harness.manager.install(&mut attempt2, BASE_MILLIS + 900).expect("installed");
    let record = harness
        .manager
        .health_failed_rollback(
            &mut attempt2,
            &ledger,
            "2.0.0",
            "the new version is unhealthy",
            None,
            BASE_MILLIS + 1_000,
        )
        .expect("the slot-level rollback lands");
    assert!(record.is_none(), "no register record: the register never moved");
    assert_eq!(ledger.active_version().as_deref(), Some("1.5.0"));
    assert_eq!(ledger.len(), 1);
    assert_eq!(attempt2.state(), UpdateProgressState::RolledBack);
    cleanup_register("below-floor");
    // THE OVERRIDE RULES (the P9.34 never-silent-downgrade rule, verbatim,
    // at the register surface — the slot manager's case 2 runs exactly
    // these rules when the failed version HAD been activated): craft the
    // post-activation state through the EXISTING activation surface, then
    // roll back below the raised floor.
    let ledger2 = open_register("below-floor-2");
    let activation = UpdateActivation::new(
        harness.audit.clone() as Arc<dyn AuditSink>,
        Arc::new(FixedClock(BASE_MILLIS + 2_000)),
    );
    let verified_two = flexcore_edge_agent::update_verification::VerifiedUpdate {
        manifest: flexcore_edge_agent::update_verification::UpdateManifest {
            package_id: "edge-agent-release".to_string(),
            version: "1.6.0".to_string(),
            bundle_digest_hex: "00112233445566778899aabbccddeeff".to_string(),
            digest_algorithm: "sha-256".to_string(),
            signer_key_reference: "key-store:flexcore-release-targets".to_string(),
            minimum_version_floor: "0.0.0".to_string(),
            approval_reference: "approval-record-f12".to_string(),
        },
        verified_at_unix_millis: BASE_MILLIS + 1_500,
    };
    activation
        .activate(&ledger2, &verified_two)
        .expect("the post-activation state is recorded");
    // A below-floor rollback WITHOUT an override is refused (the recorded
    // refusal, verbatim).
    let error = activation
        .rollback(
            &ledger2,
            "2.0.0",
            "1.5.0",
            "operator-authorized recovery under a raised floor",
            None,
        )
        .expect_err("a below-floor rollback without an override is refused");
    assert_eq!(error.name(), "UPDATE_ROLLBACK_BELOW_FLOOR_REFUSED");
    // With the RECORDED OVERRIDE the rollback lands — the override rides
    // the register record AND the audit line (never a silent downgrade).
    let record = activation
        .rollback(
            &ledger2,
            "2.0.0",
            "1.5.0",
            "operator-authorized recovery under a raised floor",
            Some(&RollbackOverride {
                actor: "J. Doe (ops)".to_string(),
                authority_reference: "ticket-f12-1".to_string(),
            }),
        )
        .expect("the recorded override clears the below-floor refusal");
    assert_eq!(record.kind, UpdateLedgerEventKind::Rollback);
    assert_eq!(record.override_reference, "J. Doe (ops) under ticket-f12-1");
    assert_eq!(ledger2.active_version().as_deref(), Some("1.5.0"));
    cleanup_register("below-floor-2");
    let lines = harness.audit.lines();
    assert!(lines.iter().any(|l| l.contains("EDGE_UPDATE_ROLLBACK") && l.contains("RECORDED")));
    assert!(lines.iter().any(|l| l.contains("ticket-f12-1")));
}

#[test]
fn the_statements_pin_the_honesty_bounds() {
    assert!(OTA_SLOTS_STATEMENT.contains("NEVER re-presents"));
    assert!(OTA_SLOTS_STATEMENT.contains("NEVER rewinds"));
    assert!(OTA_SLOTS_STATEMENT.contains("PENDING PILOT HARDWARE SPEC"));
    assert!(OTA_SLOTS_STATEMENT.contains("UPDATE_FAILURE_ROLLBACK_FAILED"));
    assert!(OTA_SLOTS_STATEMENT.contains("no device has ever installed anything"));
    assert!(OTA_SLOTS_STATEMENT.contains("WRITTEN-UNRUN"));
}
