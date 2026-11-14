// P6.32 — Integration tests: edge decommissioning and unenrollment (ledger
//         P6.32 step 4; decision record
//         `25_31_32_console_clock_decommission.md` §3).
//
// EVERY TEST BELOW IS UNRUN (the loop bans in
// `ledger/phase-06-edge-protocol/ENTRY_NOTE.md`; the human build/test pass
// owns execution). Covered cases (the ledger's test set):
//
// - **reconnect-after-decommission is rejected and audited** — through a
//   test-local control-plane fake implementing the SAME fail-closed matrix
//   as the server seam (`routes_edge.rs`): a DECOMMISSIONED identity's
//   re-enrollment and session-scope resolution are refused and the
//   refusals are audited (the edge-agent fakes pattern — the real router
//   surface is the control-plane crate's, whose test files are owned by
//   another wave; recorded in the decision record);
// - **post-wipe, nothing is recoverable through the public API**: no
//   credential record (and no `.partial` sibling), no buffered telemetry,
//   no audit-journal records;
// - **un-uploaded audit records follow the RECORDED RULE**: upload-then-
//   wipe on the happy path; an upload failure wipes NOTHING (batch back in
//   order); an unreachable channel records the loss as a GAP EVENT
//   (counted, audited, LATCHED) before the wipe — never silently dropped;
// - **the confirmation record is present per decommission**, naming edge
//   identity, tenant, site, actor, and time;
// - **idempotence**: wiping twice converges (absent targets are success).

use std::path::PathBuf;
use std::sync::Arc;

use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::time_model::{EventTime, IngestTime};
use flexcore_contracts::units::ActivePowerMW;
use flexcore_ports::fakes::FixedClock;
use flexcore_ports::telemetry_source::{QuantityValue, TelemetryQuantity};

use flexcore_edge_agent::audit_buffer::{BufferedAuditKind, FileAuditBuffer};
use flexcore_edge_agent::buffer_file::FileLocalBuffer;
use flexcore_edge_agent::enrollment::{
    AuditSettlement, CredentialStore, DecommissionConfirmation, DecommissionGapLedger,
    DecommissionWipe, DeviceCredentialRecord, FilesystemCredentialStore, PemBody,
};
use flexcore_edge_agent::ports::{
    EdgeError, EdgeTelemetrySample, EdgeTelemetrySampleParts, InMemoryAuditSink, LocalBuffer,
};

const BASE_MILLIS: i64 = 1_700_000_000_000;
const TENANT_A: &str = "tenant-a";
const SITE_A: &str = "pilot-site-A"; // PLACEHOLDER site naming (D5 input; ENTRY_NOTE boundary 5)
const DEVICE: &str = "edge-01";

// --- Fixtures ---------------------------------------------------------------------

/// A std-only unique scratch directory per call (the P6.06/P6.31 pattern).
fn scratch_dir(label: &str) -> PathBuf {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "flexcore-edge-p632-it-{}-{}-{}",
        std::process::id(),
        label,
        COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).expect("scratch dir creates");
    dir
}

/// A credential store rooted in a per-test scratch directory (the
/// TempFileCredentialStore pattern, inlined here so the wipe's post-
/// condition can inspect the OWNED paths).
fn credential_store(label: &str) -> (FilesystemCredentialStore, PathBuf) {
    let dir = scratch_dir(label);
    let path = dir.join("device-identity.conf");
    (FilesystemCredentialStore::new(path.clone()), path)
}

fn credential_record() -> DeviceCredentialRecord {
    DeviceCredentialRecord {
        device_id: DEVICE.to_string(),
        tenant_id: TENANT_A.to_string(),
        site_id: SITE_A.to_string(),
        certificate_pem: Some(PemBody::new(
            "-----BEGIN CERTIFICATE-----\nP632DEVICECERT\n-----END CERTIFICATE-----\n"
                .to_string(),
        )),
        chain_pem: Vec::new(),
        root_ca_pem: Some(PemBody::new(
            "-----BEGIN CERTIFICATE-----\nP632ROOT\n-----END CERTIFICATE-----\n".to_string(),
        )),
        private_key_reference: Some(PathBuf::from("/var/lib/flexcore-edge/keys/device.key")),
        issued_at_unix_millis: BASE_MILLIS,
        expires_at_unix_millis: Some(BASE_MILLIS + 30 * 86_400_000),
    }
}

fn sample(sequence: u64) -> EdgeTelemetrySample {
    EdgeTelemetrySample::new(EdgeTelemetrySampleParts {
        asset_key: "ess-01".to_string(),
        quantity: TelemetryQuantity::ActivePower,
        value: QuantityValue::Power(ActivePowerMW::new(1.5).expect("finite")),
        quality: TelemetryQuality::Good,
        source_quality_code: None,
        event_time: EventTime::from_unix_millis(BASE_MILLIS + sequence).expect("valid"),
        ingest_time: IngestTime::from_unix_millis(BASE_MILLIS + sequence + 1).expect("valid"),
        source_sequence: sequence,
        source_id: "edge:pilot-site-A:openems-rest:ess-01:active_power".to_string(),
    })
    .expect("valid sample")
}

fn confirmation() -> DecommissionConfirmation {
    DecommissionConfirmation {
        device_id: DEVICE.to_string(),
        tenant_id: TENANT_A.to_string(),
        site_id: SITE_A.to_string(),
        actor: "J. Doe (site operator)".to_string(),
        confirmed_at_unix_millis: BASE_MILLIS + 1_000,
    }
}

fn audit_buffer(label: &str) -> FileAuditBuffer {
    let dir = scratch_dir(label);
    FileAuditBuffer::open(
        dir.join("audit.journal"),
        16,
        Arc::new(FixedClock::at_unix_millis(BASE_MILLIS)),
    )
    .expect("audit buffer opens")
}

fn telemetry_buffer(label: &str) -> FileLocalBuffer {
    let dir = scratch_dir(label);
    FileLocalBuffer::open(
        dir.join("telemetry.journal"),
        16,
        None,
        Arc::new(FixedClock::at_unix_millis(BASE_MILLIS)),
    )
    .expect("telemetry buffer opens")
}

// --- The reconnect-after-decommission surface (test-local server fake) -------------
//
// The server-side fail-closed matrix of `routes_edge.rs`, mirrored here as
// a test-local fake (the fakes pattern of the edge-agent tests; the real
// router lives in apps/control-plane, whose test files are owned by
// another wave). The matrix: a DECOMMISSIONED identity's term is REVOKED —
// session-scope resolution refuses any non-ACTIVE term, re-enrollment of a
// REVOKED identity is refused, and EVERY refusal is audited.

#[derive(Default)]
struct FakeControlPlaneRegistry {
    device_status: Option<String>,
    audited_rejections: Vec<String>,
}

impl FakeControlPlaneRegistry {
    fn decommission(&mut self) {
        self.device_status = Some("DECOMMISSIONED".to_string());
    }

    /// The `resolve_edge_session_scope` rule: only an ACTIVE term resolves.
    fn resolve_session_scope(&mut self, device_id: &str) -> Result<(), String> {
        match self.device_status.as_deref() {
            Some("ACTIVE") => Ok(()),
            Some(status) => {
                let reason =
                    format!("device {device_id} has no ACTIVE identity term (current status: \
                             {status}); the session has no scope");
                self.audited_rejections.push(format!(
                    "SESSION_SCOPE_REFUSED: device='{device_id}' {reason}"
                ));
                Err(reason)
            }
            None => {
                let reason = format!(
                    "device {device_id} does not resolve to an identity in this tenant scope"
                );
                self.audited_rejections.push(reason.clone());
                Err(reason)
            }
        }
    }

    /// The enrollment-route rule: a REVOKED identity's re-enrollment is
    /// refused (403-style) and audited.
    fn attempt_reenroll(&mut self, device_id: &str) -> Result<(), String> {
        match self.device_status.as_deref() {
            Some("ACTIVE") | None => Ok(()),
            Some("REVOKED") | Some("DECOMMISSIONED") => {
                let reason = format!(
                    "revoked identity: re-enrollment is refused for device {device_id} \
                     (current status: {})",
                    self.device_status.clone().unwrap_or_default()
                );
                self.audited_rejections.push(reason.clone());
                Err(reason)
            }
            Some(status) => Err(format!("registry status {status} is not enrollable")),
        }
    }
}

#[test]
fn reconnect_after_decommission_is_rejected_and_audited() {
    let mut registry = FakeControlPlaneRegistry::default();
    // Pre-decommission: the edge has a live identity and a session scope.
    registry.device_status = Some("ACTIVE".to_string());
    registry.resolve_session_scope(DEVICE).expect("resolves");

    // Operator decommissions the identity (server-side revocation marks
    // the ACTIVE term DECOMMISSIONED/REVOKED).
    registry.decommission();

    // ANY later session/enrollment attempt from that identity is REJECTED:
    let session = registry
        .resolve_session_scope(DEVICE)
        .expect_err("a decommissioned identity has no session scope");
    assert!(session.contains("DECOMMISSIONED"));
    let reenroll = registry
        .attempt_reenroll(DEVICE)
        .expect_err("re-enrollment of a decommissioned identity is refused");
    assert!(reenroll.contains("revoked identity") || reenroll.contains("DECOMMISSIONED"));

    // EVERY refusal is AUDITED (two rejections, two audit records).
    assert_eq!(registry.audited_rejections.len(), 2);

    // Edge-side: the wiped credential store loads None — the edge holds no
    // identity it could reconnect WITH (no identity → no channel, P6.06).
    let (mut store, _path) = credential_store("reconnect-wiped");
    store.store(&credential_record()).expect("stored");
    DecommissionWipe::wipe_credential_store(&mut store).expect("wiped");
    assert_eq!(store.load().expect("loads"), None, "no identity to reconnect with");
}

// --- Post-wipe: nothing recoverable through the public API -------------------------

#[test]
fn post_wipe_no_credential_telemetry_or_audit_record_is_recoverable() {
    // Credentials: stored, then wiped — nothing loadable, files gone
    // (record + .partial sibling).
    let (mut store, store_path) = credential_store("post-wipe-credentials");
    let partial = store_path.with_extension("partial");
    std::fs::write(&partial, b"leftover atomic-write sibling").expect("sibling written");
    store.store(&credential_record()).expect("stored");
    DecommissionWipe::wipe_credential_store(&mut store).expect("wiped");
    assert_eq!(store.load().expect("loads"), None);
    assert!(!store_path.exists(), "the credential record file is gone");
    assert!(!partial.exists(), "the .partial sibling is gone");
    // The bytes are NOT recoverable through the store's public API: a
    // fresh read (the only public surface) returns nothing.
    let reread = FilesystemCredentialStore::new(store_path.clone());
    assert_eq!(reread.load().expect("loads"), None);

    // Buffered telemetry: drained, committed, journal wiped.
    let mut buffer = telemetry_buffer("post-wipe-telemetry");
    buffer.append(sample(1)).expect("appended");
    buffer.append(sample(2)).expect("appended");
    let journal = buffer.journal_path().to_path_buf();
    let wiped = DecommissionWipe::wipe_telemetry_buffer(&mut buffer).expect("wiped");
    assert_eq!(wiped, 2, "both buffered samples were wiped");
    assert_eq!(buffer.len(), 0, "nothing recoverable through the port");
    DecommissionWipe::wipe_journal_file(&journal).expect("journal wiped");
    assert!(!journal.exists(), "the telemetry journal file is gone");

    // Audit buffer: uploaded-then-wiped, journal gone.
    let audit_buf = audit_buffer("post-wipe-audit");
    audit_buf
        .record_event(BufferedAuditKind::EnrollmentCompleted, "edge", DEVICE, "enrolled".to_string())
        .expect("recorded");
    audit_buf
        .record_event(BufferedAuditKind::ForwardBatch, "edge", "uplink", "batch".to_string())
        .expect("recorded");
    let audit_journal = audit_buf.journal_path();
    let ledger = DecommissionGapLedger::new();
    let audit_sink = InMemoryAuditSink::new();
    let settlement = DecommissionWipe::settle_audit_buffer(
        &audit_buf,
        true,
        &|_records| Ok(()),
        &audit_sink,
        &ledger,
        DEVICE,
        BASE_MILLIS + 2_000,
    )
    .expect("settled");
    assert_eq!(settlement, AuditSettlement::UploadedThenWiped { uploaded: 2 });
    assert_eq!(audit_buf.pending_len(), 0);
    assert!(!audit_journal.exists(), "the audit journal file is gone");
    // No gap was recorded on the happy path.
    assert!(!ledger.is_latched());
    assert_eq!(ledger.gap_records_total(), 0);
}

// --- The un-uploaded audit rule: upload-then-wipe / gap-event -----------------------

#[test]
fn un_uploaded_audit_records_follow_the_recorded_rule() {
    // GAP-EVENT path: channel unreachable. The loss is counted, audited,
    // latched — and only then wiped.
    let buffer = audit_buffer("gap-event");
    for index in 0..3u64 {
        buffer
            .record_event(
                BufferedAuditKind::ForwardBatch,
                "edge",
                "uplink",
                format!("record {index}"),
            )
            .expect("recorded");
    }
    let audit_journal = buffer.journal_path();
    let ledger = DecommissionGapLedger::new();
    let audit_sink = Arc::new(InMemoryAuditSink::new());
    let settlement = DecommissionWipe::settle_audit_buffer(
        &buffer,
        false,
        &|_records| Ok(()), // never called on the gap path
        &audit_sink,
        &ledger,
        DEVICE,
        BASE_MILLIS + 3_000,
    )
    .expect("settled");
    assert_eq!(settlement, AuditSettlement::GapEvent { gap_records: 3 });
    assert_eq!(ledger.gap_records_total(), 3, "the loss is COUNTED");
    assert!(ledger.is_latched(), "the gap LATCHES");
    assert!(!audit_journal.exists(), "the journal is wiped after the gap record");
    // The gap event IS the audit of the loss (never a silent drop).
    let events = audit_sink.events();
    assert!(events
        .iter()
        .any(|e| e.detail.starts_with("EDGE_DECOMMISSION_AUDIT_GAP")
            && e.detail.contains("lost 3")
            && e.detail.contains(DEVICE)));

    // UPLOAD-FAILURE path: the batch goes back in order and NOTHING is
    // wiped (never a partial loss).
    let buffer = audit_buffer("upload-failure");
    buffer
        .record_event(BufferedAuditKind::EnrollmentCompleted, "edge", DEVICE, "enrolled".to_string())
        .expect("recorded");
    let audit_journal = buffer.journal_path();
    let ledger = DecommissionGapLedger::new();
    let error = DecommissionWipe::settle_audit_buffer(
        &buffer,
        true,
        &|_records| {
            Err(EdgeError::UplinkForwardFailed {
                reason: "the uplink refused the upload".to_string(),
            })
        },
        &InMemoryAuditSink::new(),
        &ledger,
        DEVICE,
        BASE_MILLIS + 4_000,
    )
    .expect_err("a failed upload refuses the wipe");
    assert!(error.to_string().contains("uplink refused"));
    assert_eq!(buffer.pending_len(), 1, "the record was put back IN ORDER");
    assert!(audit_journal.exists(), "nothing was wiped on a failed upload");
    assert!(!ledger.is_latched(), "a failed upload is not a gap event");
}

// --- The confirmation record --------------------------------------------------------

#[test]
fn the_confirmation_record_names_identity_tenant_site_actor_and_time() {
    let audit = Arc::new(InMemoryAuditSink::new());
    DecommissionWipe::audit_confirmation(&confirmation(), &audit, BASE_MILLIS + 2_000)
        .expect("confirmed");
    let events = audit.events();
    assert_eq!(events.len(), 1);
    let detail = &events[0].detail;
    assert!(detail.starts_with("EDGE_DECOMMISSION_CONFIRMED"));
    assert!(detail.contains(DEVICE));
    assert!(detail.contains(TENANT_A));
    assert!(detail.contains(SITE_A));
    assert!(detail.contains("J. Doe (site operator)"));
    assert!(detail.contains(&(BASE_MILLIS + 1_000).to_string()));
    // An incomplete confirmation is refused — a confirmation that cannot
    // name what it confirms is not a confirmation.
    let mut empty_actor = confirmation();
    empty_actor.actor = "  ".to_string();
    assert!(DecommissionWipe::audit_confirmation(&empty_actor, &audit, BASE_MILLIS).is_err());
}

// --- Idempotence ---------------------------------------------------------------------

#[test]
fn the_wipe_is_idempotent() {
    let (mut store, _path) = credential_store("idempotent-credentials");
    store.store(&credential_record()).expect("stored");
    DecommissionWipe::wipe_credential_store(&mut store).expect("first wipe");
    DecommissionWipe::wipe_credential_store(&mut store).expect("second wipe (absent = success)");

    let mut buffer = telemetry_buffer("idempotent-telemetry");
    buffer.append(sample(1)).expect("appended");
    let journal = buffer.journal_path().to_path_buf();
    DecommissionWipe::wipe_telemetry_buffer(&mut buffer).expect("first wipe");
    DecommissionWipe::wipe_journal_file(&journal).expect("journal wiped");
    DecommissionWipe::wipe_telemetry_buffer(&mut buffer).expect("second wipe");
    DecommissionWipe::wipe_journal_file(&journal).expect("second journal wipe (absent = success)");

    let buffer = audit_buffer("idempotent-audit");
    buffer
        .record_event(BufferedAuditKind::ForwardBatch, "edge", "uplink", "batch".to_string())
        .expect("recorded");
    let ledger = DecommissionGapLedger::new();
    let audit_sink = InMemoryAuditSink::new();
    DecommissionWipe::settle_audit_buffer(
        &buffer,
        false,
        &|_records| Ok(()),
        &audit_sink,
        &ledger,
        DEVICE,
        BASE_MILLIS + 5_000,
    )
    .expect("first settle");
    // A second settle over the wiped journal: the gap path re-records a
    // (zero) gap — counted and audited, never hidden — and still succeeds.
    DecommissionWipe::settle_audit_buffer(
        &buffer,
        false,
        &|_records| Ok(()),
        &audit_sink,
        &ledger,
        DEVICE,
        BASE_MILLIS + 6_000,
    )
    .expect("second settle (absent = success)");
}
