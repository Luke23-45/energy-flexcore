// FIM-F12.06 — Integration tests (WRITTEN-UNRUN; execution is a human
//             gate): the OTA PRODUCER → VERIFIER battery for feature F-12
//             (ledger FIM-F12.01/FIM-F12.02/FIM-F12.05/FIM-F12.06).
//
// THE HONESTY BOUNDARY (restated so no runner can misread it): no device
// exists and no fleet exists — the produced artifact here is an in-memory
// composition over fixture payloads, signed with FIXED, LOUDLY-LABELLED
// TEST-GRADE key material (never a release key; the P18.03 key ceremonies
// are HUMAN GATES and no key material ships). The metadata roles are
// TUF-SHAPED, not TUF-conformant; the role semantics are UNVERIFIED
// against the TUF spec text. Nothing in these tests claims a pass at
// authoring time; each test hand-computes the expected outcome against
// the recorded rules.
//
// THE MATRIX (each test hand-computed):
// - the round trip: a produced bundle (manifest satisfying
//   `UpdateManifest::validate` via the v2 composition) verifies through
//   the EXISTING extended gate with the REAL Ed25519 verifier over
//   device-configured trust anchors, and the anti-rollback counter
//   advances durably;
// - tamper refusals: a flipped IMAGE byte refuses through the EXISTING
//   taxonomy (`UPDATE_DIGEST_MISMATCH` riding `GateRefused`), a
//   validly-signed manifest with a wrong PAYLOAD signature refuses at the
//   v2 payload check (`UPDATE_PAYLOAD_DIGEST_MISMATCH` — the same
//   taxonomy), a flipped METADATA byte refuses the canonical
//   re-derivation, a wrong signer is refused before any trust is
//   extended, and an unconfigured root anchor is refused (no blind trust
//   swap);
// - anti-rollback preserved end to end: the produced artifact cannot be
//   re-presented (`UPDATE_SEQUENCE_NOT_AHEAD` through the gate), and the
//   PRODUCER refuses sequences and role versions that are not strictly
//   ahead of its counters (the same stable code, both sides);
// - the test-grade verifier is NEVER accepted in a production path: a
//   gate composed with `TestOnlyFnv1a64UpdateVerifier` refuses the
//   produced bundle at the algorithm step, the fail-closed default
//   (`NoUpdateVerifier`) refuses everything, and a zero-anchor Ed25519
//   verifier refuses everything;
// - the audit trail: bundle production, metadata-verification
//   pass/refusal, and the gate's own P9.34 + extended lines all land
//   through the EXISTING edge audit shapes (FIM-F12.05).

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use flexcore_audit::chain::{hex_encode_digest, sha256};
use flexcore_edge_agent::ota_bundle_produce::{
    canonical_manifest_bytes, canonical_root_bytes, canonical_snapshot_bytes,
    canonical_timestamp_bytes, OtaArtifactError, OtaArtifactVerifier, OtaBundleProducer,
    OtaDistributionArtifact, OtaMetadataRole, OtaProduceError, OtaProducePayload,
    OtaProduceRequest, OtaRootMetadata, OtaSignedRole, OtaSnapshotMetadata, OtaTimestampMetadata,
    OTA_PRODUCE_STATEMENT, OTA_REPOSITORY_FORMAT,
};
use flexcore_edge_agent::ota_signing::{
    signature_to_hex, Ed25519UpdateSignatureVerifier, OtaEd25519ReleaseKey, OtaTrustAnchor,
    OTA_SIGNING_STATEMENT,
};
use flexcore_edge_agent::ports::{AuditSink, EdgeAuditEvent, EdgeError};
use flexcore_edge_agent::update_bundle::{
    AntiRollbackCounter, ExtendedUpdateManifest, UpdateApplicability,
    ANTI_ROLLBACK_SEMANTICS_STATEMENT, UPDATE_BUNDLE_STATEMENT,
};
use flexcore_edge_agent::update_session::ExtendedUpdateGate;
use flexcore_edge_agent::update_verification::{
    NoUpdateVerifier, TestOnlyFnv1a64UpdateVerifier, UpdateDigestVerdict, UpdateVerifier,
    UPDATE_DIGEST_ALGORITHMS,
};
use flexcore_ports::clock::Clock;

const BASE_MILLIS: i64 = 1_700_000_000_000;
/// A far-future expiry for the fixtures (10 years past the base instant —
/// a fixture value, never a policy claim).
const VALID_EXPIRY: i64 = BASE_MILLIS + 10 * 365 * 24 * 60 * 60_000;
const DEVICE_VENDOR: &str = "neryva";
const DEVICE_CLASS: &str = "edge-gateway";
const DEVICE_ID: &str = "device-1";
const SIGNER_REFERENCE: &str = "key-store:flexcore-release-targets";
const FOREIGN_SIGNER_REFERENCE: &str = "key-store:flexcore-release-foreign";

/// The FIXED TEST-GRADE signing seed A (loudly labelled; never a release
/// key; the P18.03 ceremonies are HUMAN GATES). Deterministic: socket-free
/// and randomness-free.
const TEST_GRADE_SEED_A: [u8; 32] = [0x42u8; 32];
/// The FIXED TEST-GRADE signing seed B (the wrong-signer fixtures).
const TEST_GRADE_SEED_B: [u8; 32] = [0x24u8; 32];
/// The FIXED TEST-GRADE seed C (the unconfigured-anchor fixture).
const TEST_GRADE_SEED_C: [u8; 32] = [0x66u8; 32];

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

fn key_a() -> OtaEd25519ReleaseKey {
    OtaEd25519ReleaseKey::from_injected_seed(&TEST_GRADE_SEED_A, SIGNER_REFERENCE)
        .expect("the test-grade key constructs")
}

/// Seed B under the SAME reference — the same-reference-different-key
/// wrong-signer fixture.
fn key_b_same_reference() -> OtaEd25519ReleaseKey {
    OtaEd25519ReleaseKey::from_injected_seed(&TEST_GRADE_SEED_B, SIGNER_REFERENCE)
        .expect("the wrong-signer test-grade key constructs")
}

/// Seed B under its OWN reference — the distinct-reference fixture.
fn key_b_own_reference() -> OtaEd25519ReleaseKey {
    OtaEd25519ReleaseKey::from_injected_seed(
        &TEST_GRADE_SEED_B,
        "key-store:flexcore-release-secondary",
    )
    .expect("the secondary test-grade key constructs")
}

/// Seed C under the foreign reference — the unconfigured-anchor fixture.
fn key_c() -> OtaEd25519ReleaseKey {
    OtaEd25519ReleaseKey::from_injected_seed(&TEST_GRADE_SEED_C, FOREIGN_SIGNER_REFERENCE)
        .expect("the foreign test-grade key constructs")
}

fn anchor_for(key: &OtaEd25519ReleaseKey) -> OtaTrustAnchor {
    OtaTrustAnchor::new(key.signer_reference(), key.public_key_bytes())
        .expect("the anchor constructs")
}

fn root_metadata(anchors: Vec<OtaTrustAnchor>) -> OtaRootMetadata {
    OtaRootMetadata {
        root_version: 1,
        anchors,
        expires_at_unix_millis: VALID_EXPIRY,
    }
}

fn applicability() -> UpdateApplicability {
    UpdateApplicability {
        vendor: DEVICE_VENDOR.to_string(),
        device_class: DEVICE_CLASS.to_string(),
        device_ids: BTreeSet::new(),
    }
}

fn request(sequence: u64, digest_algorithm: &str, signer: &str, root: OtaRootMetadata)
-> OtaProduceRequest {
    OtaProduceRequest {
        package_id: "edge-agent-release".to_string(),
        version: "2.0.0".to_string(),
        digest_algorithm: digest_algorithm.to_string(),
        signer_key_reference: signer.to_string(),
        minimum_version_floor: "1.0.0".to_string(),
        approval_reference: "approval-record-f12".to_string(),
        applicability: applicability(),
        anti_rollback_sequence: sequence,
        payloads: vec![OtaProducePayload {
            name: "fw.bin".to_string(),
            bytes: b"flexcore-ota-fixture-image-bytes".to_vec(),
        }],
        expires_at_unix_millis: VALID_EXPIRY,
        root,
        root_version: 1,
        timestamp_version: 1,
        timestamp_expiry_unix_millis: VALID_EXPIRY,
        snapshot_version: 1,
    }
}

fn counter_path(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join(format!("flexcore-f12-bundle-{tag}-{}", std::process::id()));
    dir.join("antirb.counter")
}

fn open_counter(tag: &str) -> AntiRollbackCounter {
    let path = counter_path(tag);
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::remove_file(&path);
    AntiRollbackCounter::open(&path).expect("the counter opens")
}

fn cleanup_counter(tag: &str) {
    let path = counter_path(tag);
    let _ = std::fs::remove_file(&path);
    if let Some(dir) = path.parent() {
        let _ = std::fs::remove_dir(dir);
    }
}

fn ed25519_verifier(anchors: &[OtaTrustAnchor]) -> Arc<Ed25519UpdateSignatureVerifier> {
    Arc::new(
        Ed25519UpdateSignatureVerifier::new(anchors.to_vec()).expect("the verifier composes"),
    )
}

/// The extended gate composed with the REAL Ed25519 verifier over the
/// device anchors (the gate owns the file-backed anti-rollback counter;
/// cleanup is by tag path).
fn gate(
    tag: &str,
    audit: Arc<dyn AuditSink>,
    anchors: &[OtaTrustAnchor],
) -> ExtendedUpdateGate {
    let counter = open_counter(tag);
    ExtendedUpdateGate::new(
        DEVICE_VENDOR.to_string(),
        DEVICE_CLASS.to_string(),
        DEVICE_ID.to_string(),
        vec![SIGNER_REFERENCE.to_string()],
        ed25519_verifier(anchors),
        counter,
        audit,
        Arc::new(FixedClock(BASE_MILLIS)),
    )
    .expect("the gate composes")
}

fn artifact_verifier(anchors: Vec<OtaTrustAnchor>, audit: Arc<dyn AuditSink>) -> OtaArtifactVerifier {
    OtaArtifactVerifier::new(anchors, audit, Arc::new(FixedClock(BASE_MILLIS)))
        .expect("the device verifier composes")
}

/// Produces one artifact with the default fixture request (sequence 3,
/// key A, anchor set = [A]).
fn produce_default(audit: Arc<dyn AuditSink>) -> OtaDistributionArtifact {
    let mut producer = OtaBundleProducer::new(key_a(), audit.clone(), Arc::new(FixedClock(BASE_MILLIS)));
    producer
        .produce(request(3, "ed25519", SIGNER_REFERENCE, root_metadata(vec![anchor_for(&key_a())])))
        .expect("the artifact is produced")
}

// --- 1. The round trip -----------------------------------------------------------

#[test]
fn a_produced_bundle_verifies_through_the_existing_gate() {
    let sink = Arc::new(CollectingAuditSink::default());
    let audit: Arc<dyn AuditSink> = sink.clone();
    let key = key_a();
    let anchors = vec![anchor_for(&key)];
    let mut producer =
        OtaBundleProducer::new(key_a(), audit.clone(), Arc::new(FixedClock(BASE_MILLIS)));
    let artifact =
        producer.produce(request(3, "ed25519", SIGNER_REFERENCE, root_metadata(anchors.clone())))
            .expect("the artifact is produced");
    // The manifest satisfies the closed vocabulary: the SIGNATURE
    // algorithm, a full Ed25519 signature in the bundle hex field.
    let manifest: &ExtendedUpdateManifest = &artifact.targets.structured;
    assert_eq!(manifest.base.digest_algorithm, "ed25519");
    assert_eq!(manifest.base.bundle_digest_hex.len(), 128);
    assert_eq!(manifest.anti_rollback_sequence, 3);
    assert_eq!(manifest.payloads.len(), 1);
    // The producer's counters advanced.
    assert_eq!(producer.counters().last_manifest_sequence, 3);
    // THE EXISTING GATE (FIM-F12.02): the produced bundle verifies.
    let gate = gate("round-trip", audit.clone(), &anchors);
    artifact_verifier(anchors, audit)
        .verify(&gate, &artifact, BASE_MILLIS)
        .expect("the produced bundle verifies through the existing gate");
    assert_eq!(gate.last_seen_sequence(), 3, "the counter advanced durably");
    cleanup_counter("round-trip");
    // THE AUDIT TRAIL (FIM-F12.05): production, the four metadata roles,
    // the P9.34 verdict, the extended verdict, and the artifact verdict
    // all landed through the existing edge audit shapes.
    let lines = sink.lines();
    assert!(
        lines.iter().any(|l| l.contains("EDGE_OTA_BUNDLE_PRODUCED")),
        "the production is audited: {lines:?}"
    );
    assert_eq!(
        lines.iter().filter(|l| l.contains("EDGE_OTA_METADATA_VERIFIED")).count(),
        4,
        "all four metadata roles verified"
    );
    assert!(lines.iter().any(|l| l.contains("EDGE_UPDATE_VERIFIED")), "the P9.34 verdict");
    assert!(lines.iter().any(|l| l.contains("EDGE_FLEET_UPDATE_VERIFIED")), "the extended verdict");
    assert!(lines.iter().any(|l| l.contains("EDGE_OTA_ARTIFACT_VERIFIED")));
    assert!(lines
        .iter()
        .any(|l| l.contains("EDGE_FLEET_UPDATE_SEQUENCE_ADVANCED")), "the counter advance");
}

// --- 2. Tamper refusals -----------------------------------------------------------

#[test]
fn a_flipped_image_byte_refuses_through_the_existing_taxonomy() {
    let sink = Arc::new(CollectingAuditSink::default());
    let audit: Arc<dyn AuditSink> = sink.clone();
    let anchors = vec![anchor_for(&key_a())];
    let mut artifact = produce_default(audit.clone());
    // ONE flipped byte in the image.
    artifact.bundle[0] ^= 0x01;
    let gate = gate("tamper-image", audit.clone(), &anchors);
    let error = artifact_verifier(anchors, audit)
        .verify(&gate, &artifact, BASE_MILLIS)
        .expect_err("the tampered bundle is refused");
    let OtaArtifactError::GateRefused { code, reason } = error else {
        panic!("expected GateRefused, got {error:?}")
    };
    // THE PIN (FIM-F12.02): the refusal rides the EXISTING taxonomy — the
    // P9.34 digest/signature mismatch code, verbatim.
    assert_eq!(code, "UPDATE_DIGEST_MISMATCH");
    assert!(
        reason.contains("the bundle bytes do not match the manifest digest"),
        "{reason}"
    );
    assert_eq!(gate.last_seen_sequence(), 0, "a refused verdict records nothing");
    cleanup_counter("tamper-image");
    // The gate's own refusal audit and the artifact-level refusal both
    // fired.
    let lines = sink.lines();
    assert!(lines.iter().any(|l| l.contains("EDGE_UPDATE_VERIFICATION_REFUSED")));
    assert!(lines.iter().any(|l| l.contains("EDGE_OTA_ARTIFACT_REFUSED")));
}

#[test]
fn a_validly_signed_manifest_with_a_wrong_payload_signature_refuses() {
    // A compromised/buggy producer can re-sign a manifest whose PAYLOAD
    // signature is wrong (the bundle itself is intact and correctly
    // signed). The v2 payload check is the layer that catches exactly
    // this — through the EXISTING taxonomy.
    let sink = Arc::new(CollectingAuditSink::default());
    let audit: Arc<dyn AuditSink> = sink.clone();
    let key = key_a();
    let anchors = vec![anchor_for(&key)];
    let mut producer =
        OtaBundleProducer::new(key_a(), audit.clone(), Arc::new(FixedClock(BASE_MILLIS)));
    let artifact =
        producer.produce(request(3, "ed25519", SIGNER_REFERENCE, root_metadata(anchors.clone())))
            .expect("produced");
    // Splice: corrupt the payload's signature hex, then re-sign ALL FOUR
    // roles over the tampered content (the roles remain internally
    // consistent — every signature valid — and the payload check still
    // refuses).
    let mut tampered_manifest = artifact.targets.structured.clone();
    let mut hex = tampered_manifest.payloads[0].digest_hex.clone();
    let first = hex.remove(0);
    hex.insert(0, if first == '0' { '1' } else { '0' });
    tampered_manifest.payloads[0].digest_hex = hex;
    let targets_canonical = canonical_manifest_bytes(&tampered_manifest);
    let targets_digest_hex = hex_encode_digest(&sha256(&targets_canonical));
    let targets_signature = key.sign_exact(&targets_canonical).to_bytes();
    let timestamp = OtaTimestampMetadata {
        timestamp_version: 1,
        targets_digest_hex: targets_digest_hex.clone(),
        expires_at_unix_millis: VALID_EXPIRY,
    };
    let timestamp_canonical = canonical_timestamp_bytes(&timestamp);
    let timestamp_signature = key.sign_exact(&timestamp_canonical).to_bytes();
    let snapshot = OtaSnapshotMetadata {
        snapshot_version: 1,
        root_version: 1,
        targets_version: 3,
        timestamp_version: 1,
        targets_digest_hex,
        bundle_digest_hex: hex_encode_digest(&sha256(&artifact.bundle)),
    };
    let snapshot_canonical = canonical_snapshot_bytes(&snapshot);
    let snapshot_signature = key.sign_exact(&snapshot_canonical).to_bytes();
    let root_canonical = canonical_root_bytes(&artifact.root.structured);
    let root_signature = key.sign_exact(&root_canonical).to_bytes();
    let signer = SIGNER_REFERENCE.to_string();
    let spliced = OtaDistributionArtifact {
        root: OtaSignedRole {
            role: OtaMetadataRole::Root,
            structured: artifact.root.structured.clone(),
            canonical_bytes: root_canonical,
            signer_reference: signer.clone(),
            signature: root_signature,
        },
        targets: OtaSignedRole {
            role: OtaMetadataRole::Targets,
            structured: tampered_manifest,
            canonical_bytes: targets_canonical,
            signer_reference: signer.clone(),
            signature: targets_signature,
        },
        timestamp: OtaSignedRole {
            role: OtaMetadataRole::Timestamp,
            structured: timestamp,
            canonical_bytes: timestamp_canonical,
            signer_reference: signer.clone(),
            signature: timestamp_signature,
        },
        snapshot: OtaSignedRole {
            role: OtaMetadataRole::Snapshot,
            structured: snapshot,
            canonical_bytes: snapshot_canonical,
            signer_reference: signer,
            signature: snapshot_signature,
        },
        bundle: artifact.bundle,
    };
    let gate = gate("tamper-payload", audit.clone(), &anchors);
    let error = artifact_verifier(anchors, audit)
        .verify(&gate, &spliced, BASE_MILLIS)
        .expect_err("the wrong payload signature is refused");
    let OtaArtifactError::GateRefused { code, .. } = error else {
        panic!("expected GateRefused, got {error:?}")
    };
    // THE EXISTING TAXONOMY at the v2 payload level.
    assert_eq!(code, "UPDATE_PAYLOAD_DIGEST_MISMATCH");
    assert_eq!(gate.last_seen_sequence(), 0);
    cleanup_counter("tamper-payload");
}

#[test]
fn a_flipped_metadata_byte_refuses_the_canonical_rederivation() {
    let sink = Arc::new(CollectingAuditSink::default());
    let audit: Arc<dyn AuditSink> = sink.clone();
    let anchors = vec![anchor_for(&key_a())];
    let mut artifact = produce_default(audit.clone());
    // ONE flipped byte in the carried TARGETS canonical bytes.
    artifact.targets.canonical_bytes[30] ^= 0x01;
    let gate = gate("tamper-metadata", audit.clone(), &anchors);
    let error = artifact_verifier(anchors, audit)
        .verify(&gate, &artifact, BASE_MILLIS)
        .expect_err("the tampered metadata is refused");
    let OtaArtifactError::CanonicalMismatch { role } = error else {
        panic!("expected CanonicalMismatch, got {error:?}")
    };
    assert_eq!(role, "targets");
    assert_eq!(gate.last_seen_sequence(), 0, "a refused verdict records nothing");
    cleanup_counter("tamper-metadata");
    // The metadata refusal is audited.
    assert!(sink
        .lines()
        .iter()
        .any(|l| l.contains("EDGE_OTA_METADATA_REFUSED") && l.contains("targets")));
    // The same for the ROOT role.
    let mut artifact = produce_default(audit.clone());
    artifact.root.canonical_bytes[27] ^= 0x02;
    let gate = gate("tamper-root", audit.clone(), &anchors);
    let error = artifact_verifier(anchors, audit)
        .verify(&gate, &artifact, BASE_MILLIS)
        .expect_err("the tampered root metadata is refused");
    let OtaArtifactError::CanonicalMismatch { role } = error else {
        panic!("expected CanonicalMismatch, got {error:?}")
    };
    assert_eq!(role, "root");
    cleanup_counter("tamper-root");
}

#[test]
fn a_wrong_signer_is_refused() {
    let sink = Arc::new(CollectingAuditSink::default());
    let audit: Arc<dyn AuditSink> = sink.clone();
    let device_anchors = vec![anchor_for(&key_a())];
    // (a) Same reference, different key: the artifact declares the device
    //     anchor's reference but the signature is from another key — the
    //     signature check catches it.
    let wrong_key = key_b_same_reference();
    let mut producer = OtaBundleProducer::new(
        key_b_same_reference(),
        audit.clone(),
        Arc::new(FixedClock(BASE_MILLIS)),
    );
    let artifact = producer
        .produce(request(
            3,
            "ed25519",
            wrong_key.signer_reference(),
            root_metadata(vec![anchor_for(&key_b_same_reference())]),
        ))
        .expect("the wrong-key artifact is produced");
    let gate = gate("wrong-signer-key", audit.clone(), &device_anchors);
    let error = artifact_verifier(device_anchors.clone(), audit.clone())
        .verify(&gate, &artifact, BASE_MILLIS)
        .expect_err("the wrong-key artifact is refused");
    let OtaArtifactError::SignatureRefused { role, reason } = error else {
        panic!("expected SignatureRefused, got {error:?}")
    };
    assert_eq!(role, "root");
    assert!(reason.contains("does not verify"), "{reason}");
    assert_eq!(gate.last_seen_sequence(), 0);
    cleanup_counter("wrong-signer-key");
    // (b) A foreign REFERENCE: the signature names a signer that matches
    //     NO device anchor — refused BEFORE any cryptographic work.
    let foreign_key = key_c();
    let mut producer = OtaBundleProducer::new(
        key_c(),
        audit.clone(),
        Arc::new(FixedClock(BASE_MILLIS)),
    );
    let artifact = producer
        .produce(request(
            3,
            "ed25519",
            foreign_key.signer_reference(),
            root_metadata(vec![anchor_for(&key_c())]),
        ))
        .expect("the foreign-reference artifact is produced");
    let gate = gate("wrong-signer-ref", audit.clone(), &device_anchors);
    let error = artifact_verifier(device_anchors.clone(), audit.clone())
        .verify(&gate, &artifact, BASE_MILLIS)
        .expect_err("the foreign-reference artifact is refused");
    let OtaArtifactError::SignatureRefused { role, reason } = error else {
        panic!("expected SignatureRefused, got {error:?}")
    };
    assert_eq!(role, "root");
    assert!(reason.contains("matches NO configured trust anchor"), "{reason}");
    cleanup_counter("wrong-signer-ref");
    assert!(sink
        .lines()
        .iter()
        .any(|l| l.contains("EDGE_OTA_METADATA_REFUSED")));
}

#[test]
fn an_unconfigured_root_anchor_is_refused_no_blind_trust_swap() {
    let sink = Arc::new(CollectingAuditSink::default());
    let audit: Arc<dyn AuditSink> = sink.clone();
    // The artifact's root lists anchors A + C; the device anchors A + B.
    // The root signature verifies under A (the producer is A), but the
    // subset check refuses C — the artifact can never add an anchor the
    // device does not hold.
    let key_a = key_a();
    let key_c = key_c();
    let device_anchors = vec![anchor_for(&key_a), anchor_for(&key_b_own_reference())];
    let mut producer =
        OtaBundleProducer::new(key_a, audit.clone(), Arc::new(FixedClock(BASE_MILLIS)));
    let artifact = producer
        .produce(request(
            3,
            "ed25519",
            SIGNER_REFERENCE,
            root_metadata(vec![anchor_for(&key_a()), anchor_for(&key_c)]),
        ))
        .expect("produced");
    let gate = gate("unconfigured-anchor", audit.clone(), &device_anchors);
    let error = artifact_verifier(device_anchors, audit)
        .verify(&gate, &artifact, BASE_MILLIS)
        .expect_err("the unconfigured anchor is refused");
    assert!(matches!(error, OtaArtifactError::RootAnchorNotConfigured), "{error:?}");
    cleanup_counter("unconfigured-anchor");
    assert!(sink
        .lines()
        .iter()
        .any(|l| l.contains("OTA_ARTIFACT_ROOT_ANCHOR_NOT_CONFIGURED")));
}

// --- 3. Anti-rollback preserved end to end -----------------------------------------

#[test]
fn the_produced_artifact_cannot_be_represented_and_the_producer_counters_hold() {
    let sink = Arc::new(CollectingAuditSink::default());
    let audit: Arc<dyn AuditSink> = sink.clone();
    let key = key_a();
    let anchors = vec![anchor_for(&key)];
    let mut producer =
        OtaBundleProducer::new(key_a(), audit.clone(), Arc::new(FixedClock(BASE_MILLIS)));
    let artifact =
        producer.produce(request(3, "ed25519", SIGNER_REFERENCE, root_metadata(anchors.clone())))
            .expect("produced");
    let gate = gate("anti-rollback", audit.clone(), &anchors);
    artifact_verifier(anchors.clone(), audit.clone())
        .verify(&gate, &artifact, BASE_MILLIS)
        .expect("first presentation verifies");
    assert_eq!(gate.last_seen_sequence(), 3);
    // THE STRICTLY-AHEAD RULE (update_bundle.rs:125 semantics, honored
    // exactly): the SAME artifact cannot be re-presented — a retry
    // requires re-issued metadata with a higher sequence.
    let error = artifact_verifier(anchors.clone(), audit.clone())
        .verify(&gate, &artifact, BASE_MILLIS)
        .expect_err("the re-presentation is refused");
    let OtaArtifactError::GateRefused { code, .. } = error else {
        panic!("expected GateRefused, got {error:?}")
    };
    assert_eq!(code, "UPDATE_SEQUENCE_NOT_AHEAD");
    // The PRODUCER side honors the same semantics: the repository refuses
    // a sequence that is not strictly ahead of its last-issued sequence.
    let refused = producer
        .produce(request(3, "ed25519", SIGNER_REFERENCE, root_metadata(anchors.clone())))
        .expect_err("the re-issued sequence is refused");
    assert!(matches!(
        refused,
        OtaProduceError::SequenceNotAhead { declared: 3, last_seen: 3 }
    ));
    assert_eq!(refused.name(), "UPDATE_SEQUENCE_NOT_AHEAD", "one stable code, both sides");
    // Role versions are strictly ahead too.
    let refused = producer
        .produce(request(5, "ed25519", SIGNER_REFERENCE, root_metadata(anchors.clone())))
        .expect_err("the reused root version is refused");
    assert!(matches!(
        refused,
        OtaProduceError::VersionNotAhead { role: "root", declared: 1, last_seen: 1 }
    ));
    // A strictly-ahead issue passes and advances the counters.
    let next = producer
        .produce(request(4, "ed25519", SIGNER_REFERENCE, root_metadata(anchors.clone())))
        .expect("the strictly-ahead issue is produced");
    assert_eq!(next.targets.structured.anti_rollback_sequence, 4);
    assert_eq!(producer.counters().last_manifest_sequence, 4);
    // The pinned statement.
    assert!(ANTI_ROLLBACK_SEMANTICS_STATEMENT.contains("STRICTLY GREATER"));
    assert!(ANTI_ROLLBACK_SEMANTICS_STATEMENT.contains("UNVERIFIED"));
    cleanup_counter("anti-rollback");
}

// --- 4. The test-grade verifier is never accepted in production paths --------------

#[test]
fn the_test_grade_verifier_is_never_accepted_for_a_produced_bundle() {
    let sink = Arc::new(CollectingAuditSink::default());
    let audit: Arc<dyn AuditSink> = sink.clone();
    let anchors = vec![anchor_for(&key_a())];
    let artifact = produce_default(audit.clone());
    // A gate composed with the loudly-labelled TEST-GRADE verifier: the
    // algorithm step refuses before the verifier is ever consulted (the
    // manifest declares ed25519; the test verifier computes fnv1a64-test).
    let test_grade_gate = ExtendedUpdateGate::new(
        DEVICE_VENDOR.to_string(),
        DEVICE_CLASS.to_string(),
        DEVICE_ID.to_string(),
        vec![SIGNER_REFERENCE.to_string()],
        Arc::new(TestOnlyFnv1a64UpdateVerifier),
        open_counter("test-grade"),
        audit.clone(),
        Arc::new(FixedClock(BASE_MILLIS)),
    )
    .expect("the gate composes");
    let error = artifact_verifier(anchors.clone(), audit.clone())
        .verify(&test_grade_gate, &artifact, BASE_MILLIS)
        .expect_err("the test-grade verifier can never validate a produced bundle");
    let OtaArtifactError::GateRefused { code, .. } = error else {
        panic!("expected GateRefused, got {error:?}")
    };
    assert_eq!(code, "UPDATE_DIGEST_ALGORITHM_MISMATCH");
    cleanup_counter("test-grade");
    // Direct: the test-grade checksum can never match a signature hex.
    assert_eq!(
        TestOnlyFnv1a64UpdateVerifier.verify(
            &artifact.bundle,
            &artifact.targets.structured.base.bundle_digest_hex,
        ),
        UpdateDigestVerdict::Mismatch
    );
    // The fail-closed DEFAULT: NoUpdateVerifier refuses everything (the
    // algorithm mismatch fires at the gate's step 3 — 'none' vs 'ed25519').
    let default_gate = ExtendedUpdateGate::new(
        DEVICE_VENDOR.to_string(),
        DEVICE_CLASS.to_string(),
        DEVICE_ID.to_string(),
        vec![SIGNER_REFERENCE.to_string()],
        Arc::new(NoUpdateVerifier),
        open_counter("fail-closed"),
        audit.clone(),
        Arc::new(FixedClock(BASE_MILLIS)),
    )
    .expect("the gate composes");
    let error = artifact_verifier(anchors.clone(), audit.clone())
        .verify(&default_gate, &artifact, BASE_MILLIS)
        .expect_err("the fail-closed default refuses everything");
    let OtaArtifactError::GateRefused { code, .. } = error else {
        panic!("expected GateRefused, got {error:?}")
    };
    assert_eq!(code, "UPDATE_DIGEST_ALGORITHM_MISMATCH");
    cleanup_counter("fail-closed");
    // A ZERO-ANCHOR Ed25519 verifier refuses everything (the absence of
    // trust anchors can never look like success).
    let anchorless_gate = ExtendedUpdateGate::new(
        DEVICE_VENDOR.to_string(),
        DEVICE_CLASS.to_string(),
        DEVICE_ID.to_string(),
        vec![SIGNER_REFERENCE.to_string()],
        Arc::new(Ed25519UpdateSignatureVerifier::new(Vec::new()).expect("composes")),
        open_counter("zero-anchors"),
        audit.clone(),
        Arc::new(FixedClock(BASE_MILLIS)),
    )
    .expect("the gate composes");
    let error = artifact_verifier(anchors, audit)
        .verify(&anchorless_gate, &artifact, BASE_MILLIS)
        .expect_err("the zero-anchor verifier refuses everything");
    let OtaArtifactError::GateRefused { code, reason } = error else {
        panic!("expected GateRefused, got {error:?}")
    };
    assert_eq!(code, "UPDATE_VERIFIER_REFUSED");
    assert!(reason.contains("no OTA trust anchors are configured"), "{reason}");
    cleanup_counter("zero-anchors");
}

// --- 5. The producer's named refusals ------------------------------------------------

#[test]
fn the_producer_refusals_are_named_and_audited() {
    let sink = Arc::new(CollectingAuditSink::default());
    let audit: Arc<dyn AuditSink> = sink.clone();
    let key = key_a();
    let anchors = vec![anchor_for(&key)];
    let mut producer =
        OtaBundleProducer::new(key_a(), audit.clone(), Arc::new(FixedClock(BASE_MILLIS)));
    // A non-signing algorithm is refused (the producer signs).
    let error = producer
        .produce(request(1, "sha-256", SIGNER_REFERENCE, root_metadata(anchors.clone())))
        .expect_err("refused");
    assert_eq!(error.name(), "OTA_PRODUCE_ALGORITHM_UNSUPPORTED");
    // A signer mismatch is refused (the reference value is not echoed).
    let error = producer
        .produce(request(1, "ed25519", "key-store:somewhere-else", root_metadata(anchors.clone())))
        .expect_err("refused");
    assert_eq!(error.name(), "OTA_PRODUCE_SIGNER_MISMATCH");
    assert!(!error.to_string().contains("somewhere-else"), "the reference is not echoed");
    // An unanchored signer is refused.
    let foreign_root = OtaRootMetadata {
        root_version: 1,
        anchors: vec![anchor_for(&key_b_own_reference())],
        expires_at_unix_millis: VALID_EXPIRY,
    };
    let error = producer
        .produce(request(1, "ed25519", SIGNER_REFERENCE, foreign_root))
        .expect_err("refused");
    assert_eq!(error.name(), "OTA_PRODUCE_SIGNER_NOT_ANCHORED");
    // Duplicate payload names are refused.
    let mut duplicated = request(1, "ed25519", SIGNER_REFERENCE, root_metadata(anchors.clone()));
    duplicated.payloads.push(OtaProducePayload {
        name: "fw.bin".to_string(),
        bytes: b"second".to_vec(),
    });
    let error = producer.produce(duplicated).expect_err("refused");
    assert_eq!(error.name(), "OTA_PRODUCE_INPUT_INVALID");
    // An anchorless root is refused.
    let anchorless = OtaRootMetadata {
        root_version: 1,
        anchors: Vec::new(),
        expires_at_unix_millis: VALID_EXPIRY,
    };
    let error = producer
        .produce(request(1, "ed25519", SIGNER_REFERENCE, anchorless))
        .expect_err("refused");
    assert_eq!(error.name(), "OTA_PRODUCE_INPUT_INVALID");
    // Every refusal is audited (FIM-F12.05: the production attempts land).
    let refusals = sink
        .lines()
        .iter()
        .filter(|l| l.contains("EDGE_OTA_PRODUCE_REFUSED"))
        .count();
    assert_eq!(refusals, 5, "each refusal is audited: {:?}", sink.lines());
}

// --- 6. The statement pins -------------------------------------------------------------

#[test]
fn the_statements_pin_the_honesty_bounds() {
    // The FIM-F12.07 honesty update: the producing code exists
    // (WRITTEN-UNRUN) while the fact stands — and the pre-existing pins
    // (the P18.14 suite) are preserved verbatim.
    assert!(UPDATE_BUNDLE_STATEMENT.contains("NOT a SUIT CBOR encoding"));
    assert!(UPDATE_BUNDLE_STATEMENT.contains("never been produced, signed, staged"));
    assert!(UPDATE_BUNDLE_STATEMENT.contains("FIM-F12.01 producer"));
    assert!(UPDATE_BUNDLE_STATEMENT.contains("HUMAN GATES"));
    assert!(UPDATE_BUNDLE_STATEMENT.contains("no key material ships"));
    assert!(OTA_PRODUCE_STATEMENT.contains("NOT a TUF encoding"));
    assert!(OTA_PRODUCE_STATEMENT.contains("UNVERIFIED against the TUF spec"));
    assert!(OTA_PRODUCE_STATEMENT.contains("NoUpdateVerifier stays the fail-closed default"));
    assert!(OTA_PRODUCE_STATEMENT.contains("STRICTLY ahead"));
    assert!(OTA_SIGNING_STATEMENT.contains("HUMAN GATES"));
    assert!(OTA_SIGNING_STATEMENT.contains("No bundle has ever been signed"));
    assert_eq!(OTA_REPOSITORY_FORMAT, "flexcore-ota-repository-v1");
    // The closed digest-algorithm vocabulary grew ADDITIVELY (FIM-F12):
    // the two original entries are untouched, ed25519 joined.
    assert_eq!(UPDATE_DIGEST_ALGORITHMS.len(), 3);
    assert_eq!(UPDATE_DIGEST_ALGORITHMS[0], "sha-256");
    assert_eq!(UPDATE_DIGEST_ALGORITHMS[1], "fnv1a64-test");
    assert_eq!(UPDATE_DIGEST_ALGORITHMS[2], "ed25519");
    // The role names are the TUF-shaped set.
    assert_eq!(OtaMetadataRole::Root.name(), "root");
    assert_eq!(OtaMetadataRole::Targets.name(), "targets");
    assert_eq!(OtaMetadataRole::Timestamp.name(), "timestamp");
    assert_eq!(OtaMetadataRole::Snapshot.name(), "snapshot");
    // The per-payload signatures are domain-separated full signatures.
    assert_eq!(signature_to_hex(&key_a().sign_exact(b"x")).len(), 128);
}
