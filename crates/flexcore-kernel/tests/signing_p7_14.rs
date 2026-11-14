// P7.14 — Plugin signing, provenance, and SBOM: gate and refusal tests
// (ledger P7.14; test file written, NOT run — the loop bans hold; the
// human build/test pass executes these).
//
// Coverage (ledger P7.14 acceptance criteria, insofar as they are code
// properties; the tamper-the-package install path and SBOM generation are
// P7.27/deployment work — see the decision record):
//
// 1. An UNSIGNED artifact is refused at the `Discovered → Verified` gate
//    with the named refusal (`UNSIGNED_ARTIFACT`), before any
//    compatibility check, and the refusal is audited.
// 2. A TAMPERED artifact (digest mismatch — the test verifier flips a
//    bit of the artifact) is refused with the named refusal
//    (`DIGEST_MISMATCH`), and the refusal is audited.
// 3. A valid provenance record over an intact artifact passes the gate
//    and the lifecycle commits `Discovered → Verified`.
// 4. The fail-closed default ([`NoVerifier`]) refuses EVERYTHING — an
//    absent verifier can never look like success.
// 5. Key-reference-only structure: a record is structurally incapable of
//    carrying key material — a non-scheme key reference is a named
//    rejection (value never echoed) and a field named for key material is
//    an unknown-field rejection.
// 6. Audit records exist on verify AND refuse, with injected-clock
//    timestamps.
// 7. Record↔manifest binding: a record naming a different manifest
//    id/version is refused (`MANIFEST_MISMATCH`); a record declaring a
//    different algorithm than the verifier computes is refused
//    (`DIGEST_ALGORITHM_MISMATCH`).

use flexcore_kernel::lifecycle::{
    FixedClock, GateOutcome, LifecycleAuditKind, LifecycleState, MemoryAuditSink, PluginLifecycle,
};
use flexcore_kernel::manifest::SemVer;
use flexcore_kernel::signing::{
    parse_provenance, provenance_gate, verify_artifact, DigestVerdict, DigestVerifier,
    NoVerifier, ProvenanceRecord, ProvenanceVerdict, SigningError, UnsafeTestOnlyDigest,
    DIGEST_ALGORITHM_SHA256, REFUSAL_ALGORITHM_MISMATCH, REFUSAL_DIGEST_MISMATCH,
    REFUSAL_MANIFEST_MISMATCH, REFUSAL_UNSIGNED,
};

const MANIFEST_ID: &str = "neryva.optimizer-wasm-lp";
const MANIFEST_VERSION: SemVer = SemVer::new(0, 2, 0);

const ARTISTITE_BYTES: &[u8] = b"plugin-artifact-bytes-for-the-p7-14-tests";

fn artifact_bytes() -> &'static [u8] {
    ARTISTITE_BYTES
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// A provenance record for the intact test artifact, as JSON (the
/// signer-side shape a deployment would emit).
fn provenance_json() -> String {
    format!(
        r#"{{
  "provenance-schema": "flexcore-plugin-provenance-v1",
  "artifact-id": "plugins/artifacts/neryva.optimizer-wasm-lp-0.2.0.wasm",
  "digest-algorithm": "fnv1a64-test",
  "digest": "{:016x}",
  "signer-key-reference": "key-store:release/plugin-signing-2026",
  "signed-at-millis": 1762348800000,
  "manifest-id": "{mid}",
  "manifest-version": "0.2.0",
  "builder-id": "ci-builder/flexcore-release-2026-11",
  "sbom-reference": "sbom/neryva.optimizer-wasm-lp-0.2.0.cdx.json"
}}"#,
        fnv1a64(artifact_bytes()),
        mid = MANIFEST_ID
    )
}

fn valid_record() -> ProvenanceRecord {
    parse_provenance(&provenance_json()).expect("the fixture record must parse")
}

/// Walk the gate with the given record/verifier and collect the audit
/// sink.
fn run_gate(
    record: Option<&ProvenanceRecord>,
    artifact: &[u8],
    verifier: &dyn DigestVerifier,
) -> (GateOutcome, MemoryAuditSink, FixedClock) {
    let audit = MemoryAuditSink::new();
    let clock = FixedClock::new(1_000);
    let outcome = provenance_gate(
        record,
        artifact,
        MANIFEST_ID,
        MANIFEST_VERSION,
        verifier,
        &clock,
        &audit,
    );
    (outcome, audit, clock)
}

// --- 1. Unsigned artifact -------------------------------------------------------

#[test]
fn an_unsigned_artifact_is_refused_at_the_verified_gate_before_anything_else() {
    let (outcome, audit, _clock) = run_gate(None, artifact_bytes(), &UnsafeTestOnlyDigest);
    let reason = match &outcome {
        GateOutcome::Failed { reason } => reason.as_str(),
        GateOutcome::Passed => panic!("an unsigned artifact must not pass the provenance gate"),
    };
    assert!(reason.starts_with(REFUSAL_UNSIGNED), "named refusal expected: {reason}");
    // The refusal is audited with the attempted Discovered -> Verified edge.
    let records = audit.records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].kind, LifecycleAuditKind::Refused);
    assert_eq!(records[0].from, Some(LifecycleState::Discovered));
    assert_eq!(records[0].to, Some(LifecycleState::Verified));
    assert_eq!(records[0].timestamp_millis, 1_000);

    // And the lifecycle itself refuses the transition (fail-closed, state
    // unchanged) when handed the failed gate outcome.
    let lifecycle_audit = MemoryAuditSink::new();
    let clock = FixedClock::new(2_000);
    let mut lifecycle = PluginLifecycle::register(
        MANIFEST_ID,
        "0.2.0",
        &clock,
        &lifecycle_audit,
        Default::default(),
    );
    let err = lifecycle
        .verify(outcome, "composition-root", "install attempt")
        .expect_err("the gate failed; the transition must be refused");
    assert_eq!(err.name(), "GATE_REFUSED");
    assert_eq!(lifecycle.state(), LifecycleState::Discovered);
}

// --- 2. Tampered artifact (the test verifier "flips a bit") -----------------------

#[test]
fn a_tampered_artifact_fails_the_digest_check_with_a_named_refusal() {
    let record = valid_record();
    // Tamper: flip one byte of the artifact (the ledger's tamper test).
    let mut tampered = artifact_bytes().to_vec();
    tampered[0] ^= 0x01;
    let (outcome, audit, _clock) = run_gate(Some(&record), &tampered, &UnsafeTestOnlyDigest);
    let reason = match &outcome {
        GateOutcome::Failed { reason } => reason.as_str(),
        GateOutcome::Passed => panic!("a tampered artifact must not pass the provenance gate"),
    };
    assert!(reason.starts_with(REFUSAL_DIGEST_MISMATCH), "named refusal expected: {reason}");
    let records = audit.records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].kind, LifecycleAuditKind::Refused);
}

#[test]
fn the_verifier_port_reports_the_mismatch_itself() {
    let record = valid_record();
    let mut tampered = artifact_bytes().to_vec();
    tampered[1] ^= 0x80;
    let verdict = UnsafeTestOnlyDigest.verify(&tampered, &record.digest_hex);
    assert_eq!(verdict, DigestVerdict::Mismatch);
    // And the pure verification function wraps it in the named refusal.
    let refused = verify_artifact(
        &record,
        &tampered,
        MANIFEST_ID,
        MANIFEST_VERSION,
        &UnsafeTestOnlyDigest,
    );
    match refused {
        ProvenanceVerdict::Refused { reason } => {
            assert!(reason.starts_with(REFUSAL_DIGEST_MISMATCH));
        }
        ProvenanceVerdict::Accepted => panic!("tampered bytes must not verify"),
    }
}

// --- 3. Valid provenance -> Verified ----------------------------------------------

#[test]
fn a_valid_provenance_record_passes_the_gate_and_the_lifecycle_commits_verified() {
    let record = valid_record();
    let (outcome, gate_audit, _clock) = run_gate(Some(&record), artifact_bytes(), &UnsafeTestOnlyDigest);
    assert_eq!(outcome, GateOutcome::Passed);
    // The gate's acceptance is audited.
    let records = gate_audit.records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].kind, LifecycleAuditKind::Transition);
    assert!(records[0].reason.contains("provenance verified"));
    // The SBOM reference rides the record (metadata only; generation is
    // P7.27 CI tooling).
    assert_eq!(record.sbom_reference, "sbom/neryva.optimizer-wasm-lp-0.2.0.cdx.json");

    // The lifecycle commits Discovered -> Verified with the passed outcome.
    let lifecycle_audit = MemoryAuditSink::new();
    let clock = FixedClock::new(3_000);
    let mut lifecycle = PluginLifecycle::register(
        MANIFEST_ID,
        "0.2.0",
        &clock,
        &lifecycle_audit,
        Default::default(),
    );
    lifecycle
        .verify(outcome, "composition-root", "install with provenance")
        .expect("a passed gate commits the transition");
    assert_eq!(lifecycle.state(), LifecycleState::Verified);
}

// --- 4. NoVerifier default (fail-closed) ------------------------------------------

#[test]
fn the_default_no_verifier_refuses_everything() {
    let record = valid_record();
    // Even the INTACT artifact is refused: an absent verifier is never a
    // pass (fail-closed default; production SHA-256 wiring is a recorded
    // deployment requirement).
    let (outcome, _audit, _clock) = run_gate(Some(&record), artifact_bytes(), &NoVerifier);
    let reason = match &outcome {
        GateOutcome::Failed { reason } => reason.as_str(),
        GateOutcome::Passed => panic!("NoVerifier must refuse everything"),
    };
    assert!(reason.starts_with("VERIFIER_REFUSED"), "named refusal expected: {reason}");
    // The port itself refuses too.
    assert!(matches!(
        NoVerifier.verify(artifact_bytes(), &record.digest_hex),
        DigestVerdict::Refused { .. }
    ));
}

// --- 5. Key-reference-only structure (no key material possible) --------------------

#[test]
fn a_key_reference_without_a_scheme_is_a_named_rejection_and_the_value_is_never_echoed() {
    let json = provenance_json().replace(
        "key-store:release/plugin-signing-2026",
        "MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8A-inline-key-material-attempt",
    );
    let err = parse_provenance(&json).expect_err("a non-scheme key reference must be rejected");
    assert_eq!(err.name(), "PROVENANCE_KEY_REFERENCE_INVALID");
    // The rejected VALUE (likely key material) is never echoed back.
    assert!(!err.to_string().contains("MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8A"));
}

#[test]
fn the_closed_key_set_makes_key_material_structurally_impossible() {
    // A field named for key material is an unknown-field rejection: there
    // is no field anywhere in the record a key could hide in.
    let json = provenance_json().replace(
        "\"builder-id\": \"",
        "\"public-key\": \"spki-placeholder\",\n  \"builder-id\": \"",
    );
    let err = parse_provenance(&json).expect_err("a public-key field must be rejected");
    assert_eq!(err.name(), "PROVENANCE_UNKNOWN_FIELD");
    let json2 = provenance_json().replace(
        "\"builder-id\": \"",
        "\"signing-key\": \"material-attempt\",\n  \"builder-id\": \"",
    );
    let err2 = parse_provenance(&json2).expect_err("a signing-key field must be rejected");
    assert_eq!(err2.name(), "PROVENANCE_UNKNOWN_FIELD");
}

// --- 6. Audit on verify and refuse; clock-injected timestamps ----------------------

#[test]
fn gate_audit_records_carry_injected_clock_timestamps_and_distinct_kinds() {
    let record = valid_record();
    let (pass_outcome, pass_audit, _c1) = run_gate(Some(&record), artifact_bytes(), &UnsafeTestOnlyDigest);
    assert_eq!(pass_outcome, GateOutcome::Passed);
    let (fail_outcome, fail_audit, _c2) = run_gate(None, artifact_bytes(), &UnsafeTestOnlyDigest);
    assert!(!fail_outcome.passed());
    let pass_records = pass_audit.records();
    let fail_records = fail_audit.records();
    assert_eq!(pass_records.len(), 1);
    assert_eq!(fail_records.len(), 1);
    assert_eq!(pass_records[0].kind, LifecycleAuditKind::Transition);
    assert_eq!(fail_records[0].kind, LifecycleAuditKind::Refused);
    for record in pass_records.iter().chain(fail_records.iter()) {
        assert_eq!(record.timestamp_millis, 1_000);
        assert_eq!(record.plugin_id, MANIFEST_ID);
    }
}

// --- 7. Record↔manifest binding and algorithm binding ------------------------------

#[test]
fn a_record_binding_a_different_manifest_is_refused() {
    let record = valid_record();
    let verdict = verify_artifact(
        &record,
        artifact_bytes(),
        "builtin.highs-optimizer",
        MANIFEST_VERSION,
        &UnsafeTestOnlyDigest,
    );
    match verdict {
        ProvenanceVerdict::Refused { reason } => {
            assert!(reason.starts_with(REFUSAL_MANIFEST_MISMATCH));
        }
        ProvenanceVerdict::Accepted => panic!("a foreign record must not verify"),
    }
}

#[test]
fn a_record_declaring_another_algorithm_than_the_verifier_is_refused() {
    let json = provenance_json().replace("fnv1a64-test", DIGEST_ALGORITHM_SHA256);
    let record = parse_provenance(&json).expect("the algorithm is in the closed vocabulary");
    let verdict = verify_artifact(
        &record,
        artifact_bytes(),
        MANIFEST_ID,
        MANIFEST_VERSION,
        &UnsafeTestOnlyDigest,
    );
    match verdict {
        ProvenanceVerdict::Refused { reason } => {
            assert!(reason.starts_with(REFUSAL_ALGORITHM_MISMATCH));
        }
        ProvenanceVerdict::Accepted => panic!("an algorithm mismatch must not verify"),
    }
}

// --- Record parse refusals (closed schema) ------------------------------------------

#[test]
fn record_schema_and_field_rules_are_fail_closed() {
    // Wrong envelope id.
    let json = provenance_json().replace("flexcore-plugin-provenance-v1", "flexcore-plugin-provenance-v2");
    assert_eq!(parse_provenance(&json).unwrap_err().name(), "PROVENANCE_SCHEMA_UNSUPPORTED");
    // Unknown top-level field (a signature blob field is a rejection).
    let json = provenance_json().replace(
        "\"builder-id\": \"",
        "\"signature\": \"blob-attempt\",\n  \"builder-id\": \"",
    );
    assert_eq!(parse_provenance(&json).unwrap_err().name(), "PROVENANCE_UNKNOWN_FIELD");
    // Missing SBOM reference.
    let json = provenance_json().replace(",\n  \"sbom-reference\": \"sbom/neryva.optimizer-wasm-lp-0.2.0.cdx.json\"", "");
    assert_eq!(parse_provenance(&json).unwrap_err().name(), "PROVENANCE_MISSING_FIELD");
    // Non-hex digest.
    let json = provenance_json().replace(&format!("{:016x}", fnv1a64(artifact_bytes())), "zzzz-not-hex!!");
    assert_eq!(parse_provenance(&json).unwrap_err().name(), "PROVENANCE_DIGEST_INVALID");
    // Unknown digest algorithm (closed vocabulary).
    let json = provenance_json().replace("fnv1a64-test", "md5");
    assert_eq!(parse_provenance(&json).unwrap_err().name(), "PROVENANCE_DIGEST_ALGORITHM_UNKNOWN");
    // Not an object.
    let err = parse_provenance("[1,2]").unwrap_err();
    assert_eq!(err.name(), "PROVENANCE_RECORD_NOT_AN_OBJECT");
    // Oversize source is refused before parsing.
    let big = " ".repeat(70_000);
    let err = parse_provenance(&big).unwrap_err();
    assert_eq!(err.name(), "PROVENANCE_RECORD_JSON_INVALID");
}

#[test]
fn every_signing_error_name_is_distinct() {
    let names = [
        SigningError::RecordJsonInvalid { reason: "x".to_string() }.name(),
        SigningError::RecordNotAnObject.name(),
        SigningError::RecordSchemaUnsupported { found: "v2".to_string() }.name(),
        SigningError::RecordUnknownField { location: "l".to_string(), field: "f".to_string() }.name(),
        SigningError::RecordMissingField { field: "f".to_string(), location: "l".to_string() }.name(),
        SigningError::RecordWrongType {
            location: "l".to_string(),
            field: "f".to_string(),
            expected: "string",
            found: "number",
        }
        .name(),
        SigningError::RecordManifestIdMalformed { value: "v".to_string(), reason: "r".to_string() }.name(),
        SigningError::RecordManifestVersionInvalid { value: "v".to_string(), reason: "r".to_string() }.name(),
        SigningError::RecordDigestInvalid { reason: "r".to_string() }.name(),
        SigningError::RecordDigestAlgorithmUnknown { value: "md5".to_string() }.name(),
        SigningError::RecordKeyReferenceInvalid { reason: "r".to_string() }.name(),
        SigningError::RecordSbomReferenceInvalid { reason: "r".to_string() }.name(),
        SigningError::RecordFieldInvalid { field: "f".to_string(), reason: "r".to_string() }.name(),
    ];
    for i in 0..names.len() {
        for j in (i + 1)..names.len() {
            assert_ne!(names[i], names[j], "error names must be distinct");
        }
    }
}

#[test]
fn the_signing_statement_pins_what_is_and_is_not_claimed() {
    // The honesty statement exists and names the deployment requirement
    // (the P4.28 statement pattern). Kept as a live assertion so the
    // statement cannot silently rot.
    let statement = flexcore_kernel::signing::SIGNING_STATEMENT;
    assert!(statement.contains("DEPLOYMENT REQUIREMENT"));
    assert!(statement.contains("refuses everything"));
}
