// P18.14 — Integration tests (WRITTEN-UNRUN; execution is a human gate):
//          MANIFEST VALIDATION + ANTI-ROLLBACK NEGATIVE battery for the
//          phase-18 update path (plan §2 P18.14; decision record
//          `docs/phases-11-20/phase-18-edge-fleet-lifecycle-ota/records/
//          41_p18_14_update_test_suite_mock_device.md`).
//
// THE HONESTY BOUNDARY (restated so no runner can misread it): no device
// exists — the "device" in these tests is `MockUpdateDevice`, a TEST
// DOUBLE in `tests/p18_14_support/mod.rs`; no update has ever been
// produced, signed, staged, or applied — every bundle here is a fixture
// byte string and every digest is the loudly-labelled test-grade FNV-1a-64
// (NOT a security primitive; a production release digest is the release
// tooling's job under the SHA-256 DEPLOYMENT REQUIREMENT). Nothing in
// these tests claims a pass at authoring time; each test hand-computes
// the expected outcome against the recorded rules (the same discipline as
// the P9.34 integration suite).
//
// THE MATRIX (each test hand-computed):
// - the v2 manifest vocabulary: the 14 named `UpdateBundleError` variants
//   exist with stable names;
// - structural validation: the well-formed fixture validates; zero
//   sequence, duplicate payload names, overlapping ranges, zero-size
//   payloads, oversized entries/total, and base-manifest violations are
//   all refused INTO one named reason list;
// - applicability: vendor/class mandatory exact match, the closed
//   empty-vs-listed allowlist semantics, refusal BEFORE digest work;
// - expiry: refused at and after the instant against the injected clock
//   (a recomposed gate sees the advanced clock);
// - ANTI-ROLLBACK NEGATIVES: replay of an already-accepted sequence,
//   equal sequence, lower sequence, the counter's reopen-replay
//   monotonicity, the truncate-and-audit corrupt tail, a foreign file
//   refused as not-a-counter, and the durable-before-verdict advance (a
//   refused verdict records nothing);
// - the fail-closed default: with no verifier configured the extended
//   gate refuses EVERYTHING as BaseVerificationRefused carrying the
//   P9.34 stable code UPDATE_VERIFIER_REFUSED, and the counter never
//   advances.

mod p18_14_support;

use std::collections::BTreeSet;

use flexcore_edge_agent::update_bundle::{
    AntiRollbackCounter, PayloadDigest, UpdateApplicability, UpdateBundleError,
    ANTI_ROLLBACK_SEQUENCE_FLOOR, ANTI_ROLLBACK_SEMANTICS_STATEMENT,
    MAX_UPDATE_PAYLOAD_ENTRY_BYTES, MAX_UPDATE_PAYLOAD_TOTAL_BYTES, UPDATE_BUNDLE_FORMAT_V2,
    UPDATE_BUNDLE_STATEMENT,
};

use p18_14_support::{
    cleanup, fixture_bundle, fixture_digest, v2_manifest, MockUpdateDevice, MockVerifierChoice,
    BASE_MILLIS, DEVICE_CLASS, DEVICE_ID, DEVICE_VENDOR,
};

// A far-future expiry for the valid manifests (10 years past the fixture
// base instant — a fixture value, never a policy claim).
const VALID_EXPIRY: i64 = BASE_MILLIS + 10 * 365 * 24 * 60 * 60_000;

// --- 1. The named-error vocabulary (the 14 variants, pinned) -----------------------

#[test]
fn the_update_bundle_error_vocabulary_has_exactly_the_recorded_names() {
    // The 14 stable names (the named-error table; record 02 §3). A
    // vocabulary change is a format change and must be recorded, never
    // silent — this test is the pin.
    let expected: [&str; 14] = [
        "UPDATE_UNSIGNED_BUNDLE",
        "UPDATE_MANIFEST_INVALID",
        "UPDATE_NOT_APPLICABLE",
        "UPDATE_MANIFEST_EXPIRED",
        "UPDATE_SEQUENCE_NOT_AHEAD",
        "UPDATE_COUNTER_UNAVAILABLE",
        "UPDATE_COUNTER_IO",
        "UPDATE_NOT_AN_ANTI_ROLLBACK_COUNTER",
        "UPDATE_PAYLOAD_BOUNDS_OUTSIDE",
        "UPDATE_PAYLOAD_DIGEST_MISMATCH",
        "UPDATE_PAYLOAD_VERIFIER_REFUSED",
        "UPDATE_BASE_VERIFICATION_REFUSED",
        "UPDATE_BUNDLE_TOO_LARGE",
        "UPDATE_INPUT_INVALID",
    ];
    let samples: Vec<UpdateBundleError> = vec![
        UpdateBundleError::UnsignedBundle,
        UpdateBundleError::ManifestInvalid { reason: "x".to_string() },
        UpdateBundleError::ApplicabilityMismatch { reason: "x".to_string() },
        UpdateBundleError::Expired { expires_at_unix_millis: 1, now_unix_millis: 2 },
        UpdateBundleError::SequenceNotAhead { declared: 1, last_seen: 1 },
        UpdateBundleError::CounterUnavailable { reason: "x".to_string() },
        UpdateBundleError::CounterIo { reason: "x".to_string() },
        UpdateBundleError::NotAnAntiRollbackCounter { reason: "x".to_string() },
        UpdateBundleError::PayloadBoundsOutside { payload_name: "p".to_string() },
        UpdateBundleError::PayloadDigestMismatch { payload_name: "p".to_string() },
        UpdateBundleError::PayloadVerifierRefused {
            payload_name: "p".to_string(),
            reason: "x".to_string(),
        },
        UpdateBundleError::BaseVerificationRefused { code: "c".to_string(), reason: "x" },
        UpdateBundleError::BundleTooLarge { size_bytes: 2, cap: 1 },
        UpdateBundleError::InvalidInput { reason: "x".to_string() },
    ];
    assert_eq!(samples.len(), expected.len(), "14 named variants");
    for (error, name) in samples.iter().zip(expected.iter()) {
        assert_eq!(error.name(), *name, "the stable name changed");
    }
}

#[test]
fn the_format_id_and_the_honesty_statements_are_pinned() {
    assert_eq!(UPDATE_BUNDLE_FORMAT_V2, "flexcore-edge-update-manifest-v2");
    assert!(UPDATE_BUNDLE_STATEMENT.contains("NOT a SUIT CBOR encoding"));
    assert!(UPDATE_BUNDLE_STATEMENT.contains("never been produced, signed, staged"));
    assert!(ANTI_ROLLBACK_SEMANTICS_STATEMENT.contains("STRICTLY GREATER"));
    assert!(ANTI_ROLLBACK_SEMANTICS_STATEMENT.contains("UNVERIFIED"));
    assert_eq!(ANTI_ROLLBACK_SEQUENCE_FLOOR, 1, "zero is a malformed manifest");
}

// --- 2. Structural validation ---------------------------------------------------------

#[test]
fn the_well_formed_fixture_manifest_validates() {
    let bundle = fixture_bundle();
    let manifest = v2_manifest("1.2.0", "1.0.0", 1, VALID_EXPIRY, &bundle);
    manifest.validate().expect("the well-formed fixture validates");
}

#[test]
fn zero_sequence_is_refused_as_malformed() {
    let bundle = fixture_bundle();
    let manifest = v2_manifest("1.2.0", "1.0.0", 0, VALID_EXPIRY, &bundle);
    let error = manifest.validate().expect_err("zero sequence refused");
    let UpdateBundleError::ManifestInvalid { reason } = error else {
        panic!("expected ManifestInvalid, got {error:?}")
    };
    assert!(reason.contains("anti_rollback_sequence"), "{reason}");
}

#[test]
fn duplicate_payload_names_and_overlapping_ranges_are_refused() {
    let bundle = fixture_bundle();
    let mut manifest = v2_manifest("1.2.0", "1.0.0", 1, VALID_EXPIRY, &bundle);
    manifest.payloads.push(PayloadDigest {
        name: "fw.bin".to_string(),
        offset: 4,
        size_bytes: 8,
        digest_hex: fixture_digest(&bundle[4..12]),
    });
    let error = manifest.validate().expect_err("refused");
    let UpdateBundleError::ManifestInvalid { reason } = error else {
        panic!("expected ManifestInvalid, got {error:?}")
    };
    assert!(reason.contains("twice"), "duplicate name caught: {reason}");
    assert!(reason.contains("overlap"), "overlap caught: {reason}");
}

#[test]
fn zero_size_and_empty_payload_lists_are_refused() {
    let bundle = fixture_bundle();
    let mut manifest = v2_manifest("1.2.0", "1.0.0", 1, VALID_EXPIRY, &bundle);
    manifest.payloads[0].size_bytes = 0;
    assert!(manifest.validate().is_err(), "a zero-byte payload entry is malformed");

    let mut empty = v2_manifest("1.2.0", "1.0.0", 1, VALID_EXPIRY, &bundle);
    empty.payloads.clear();
    assert!(empty.validate().is_err(), "at least one payload is mandatory");
}

#[test]
fn entry_and_total_size_caps_are_code_maxima() {
    // Per-entry cap: one entry above it is refused.
    let mut over_entry = {
        let bundle = fixture_bundle();
        v2_manifest("1.2.0", "1.0.0", 1, VALID_EXPIRY, &bundle)
    };
    over_entry.payloads[0].size_bytes = MAX_UPDATE_PAYLOAD_ENTRY_BYTES + 1;
    assert!(over_entry.validate().is_err());

    // Total cap: eight maximal entries (non-overlapping ranges) exceed
    // 1 GiB in total and are refused even though each entry is legal.
    let mut over_total = {
        let bundle = fixture_bundle();
        v2_manifest("1.2.0", "1.0.0", 1, VALID_EXPIRY, &bundle)
    };
    over_total.payloads.clear();
    for i in 0..8u64 {
        over_total.payloads.push(PayloadDigest {
            name: format!("part-{i}"),
            offset: (i as usize) * MAX_UPDATE_PAYLOAD_ENTRY_BYTES,
            size_bytes: MAX_UPDATE_PAYLOAD_ENTRY_BYTES,
            digest_hex: "aabbccddeeff0011".to_string(),
        });
    }
    let error = over_total.validate().expect_err("total cap exceeded");
    let UpdateBundleError::ManifestInvalid { reason } = error else {
        panic!("expected ManifestInvalid, got {error:?}")
    };
    assert!(reason.contains("total declared payload size"), "{reason}");
    assert_eq!(
        MAX_UPDATE_PAYLOAD_TOTAL_BYTES,
        1024 * 1024 * 1024,
        "the total cap constant is the recorded code maximum"
    );
}

#[test]
fn base_manifest_violations_ride_the_v2_refusal() {
    let bundle = fixture_bundle();
    // A scheme-less signer reference (the P9.34 inline-key-material rule).
    let mut manifest = v2_manifest("1.2.0", "1.0.0", 1, VALID_EXPIRY, &bundle);
    manifest.base.signer_key_reference = "raw-key-material-reference".to_string();
    let error = manifest.validate().expect_err("refused");
    let UpdateBundleError::ManifestInvalid { reason } = error else {
        panic!("expected ManifestInvalid, got {error:?}")
    };
    assert!(
        reason.contains("signer_key_reference"),
        "the P9.34 reason text rides the v2 refusal: {reason}"
    );
    // A malformed version core (the P9.34 numeric M.m.p rule).
    let bad_version = v2_manifest("1.2", "1.0.0", 1, VALID_EXPIRY, &bundle);
    assert!(bad_version.validate().is_err());
}

#[test]
fn a_non_positive_expiry_is_refused_at_validation() {
    let bundle = fixture_bundle();
    let manifest = v2_manifest("1.2.0", "1.0.0", 1, 0, &bundle);
    assert!(manifest.validate().is_err(), "expiry is mandatory in v2");
}

// --- 3. Applicability ------------------------------------------------------------------

#[test]
fn applicability_matches_exactly_and_the_allowlist_is_closed() {
    // Empty allowlist = every device of the vendor+class.
    let any = UpdateApplicability {
        vendor: DEVICE_VENDOR.to_string(),
        device_class: DEVICE_CLASS.to_string(),
        device_ids: BTreeSet::new(),
    };
    assert!(any.matches(DEVICE_VENDOR, DEVICE_CLASS, DEVICE_ID));
    assert!(!any.matches("other-vendor", DEVICE_CLASS, DEVICE_ID));
    assert!(!any.matches(DEVICE_VENDOR, "other-class", DEVICE_ID));
    // Non-empty allowlist = only the listed ids.
    let listed = UpdateApplicability {
        vendor: DEVICE_VENDOR.to_string(),
        device_class: DEVICE_CLASS.to_string(),
        device_ids: BTreeSet::from(["mock-device-2".to_string()]),
    };
    assert!(!listed.matches(DEVICE_VENDOR, DEVICE_CLASS, DEVICE_ID));
    assert!(listed.matches(DEVICE_VENDOR, DEVICE_CLASS, "mock-device-2"));
    // There is no wildcard vocabulary: '*' is a literal string and does
    // not match an id (closed semantics, no third reading).
    let wildcard = UpdateApplicability {
        vendor: DEVICE_VENDOR.to_string(),
        device_class: DEVICE_CLASS.to_string(),
        device_ids: BTreeSet::from(["*".to_string()]),
    };
    assert!(!wildcard.matches(DEVICE_VENDOR, DEVICE_CLASS, DEVICE_ID));
}

#[test]
fn an_inapplicable_manifest_is_refused_before_any_digest_work_at_the_gate() {
    let device = MockUpdateDevice::new("applicability", MockVerifierChoice::TestOnlyFnv1a64);
    let bundle = fixture_bundle();
    // A digest that would NOT match the bundle: if the applicability
    // check did not run FIRST, the gate would report a digest mismatch
    // instead of the applicability refusal (the recorded refusal order:
    // applicability precedes all digest work).
    let mut manifest = v2_manifest("1.2.0", "1.0.0", 1, VALID_EXPIRY, &bundle);
    manifest.base.bundle_digest_hex = "0000000000000000".to_string();
    manifest.payloads[0].digest_hex = "0000000000000000".to_string();
    manifest.applicability.vendor = "other-vendor".to_string();
    let error = device
        .present_bundle(Some(&manifest), &bundle)
        .expect_err("refused on applicability");
    assert!(
        matches!(error, UpdateBundleError::ApplicabilityMismatch { .. }),
        "the applicability refusal precedes the digest checks: {error:?}"
    );
    cleanup(device.scratch());
}

#[test]
fn an_allowlisted_device_outside_the_list_is_refused_at_the_gate() {
    let device = MockUpdateDevice::new("allowlist", MockVerifierChoice::TestOnlyFnv1a64);
    let bundle = fixture_bundle();
    let mut manifest = v2_manifest("1.2.0", "1.0.0", 1, VALID_EXPIRY, &bundle);
    manifest
        .applicability
        .device_ids
        .insert("mock-device-9".to_string());
    let error = device
        .present_bundle(Some(&manifest), &bundle)
        .expect_err("refused");
    assert!(matches!(error, UpdateBundleError::ApplicabilityMismatch { .. }));
    assert_eq!(
        device.last_seen_sequence(),
        0,
        "a refused manifest never advances the counter"
    );
    cleanup(device.scratch());
}

// --- 4. Expiry (injected clock) ----------------------------------------------------------

#[test]
fn an_expired_manifest_is_refused_at_the_gate() {
    let device = MockUpdateDevice::new("expiry", MockVerifierChoice::TestOnlyFnv1a64);
    let bundle = fixture_bundle();
    // The gate's clock is frozen at BASE_MILLIS; an expiry BEFORE that
    // instant is already stale at composition.
    let before = v2_manifest("1.2.0", "1.0.0", 1, BASE_MILLIS - 1, &bundle);
    let error = device
        .present_bundle(Some(&before), &bundle)
        .expect_err("refused");
    match error {
        UpdateBundleError::Expired {
            expires_at_unix_millis,
            now_unix_millis,
        } => {
            assert_eq!(expires_at_unix_millis, BASE_MILLIS - 1);
            assert_eq!(now_unix_millis, BASE_MILLIS);
        }
        other => panic!("expected Expired, got {other:?}"),
    }
    // At the exact instant: refused (>= is the recorded boundary).
    let at_instant = v2_manifest("1.2.0", "1.0.0", 1, BASE_MILLIS, &bundle);
    assert!(matches!(
        device.present_bundle(Some(&at_instant), &bundle),
        Err(UpdateBundleError::Expired { .. })
    ));
    // One millis after: accepted through the expiry step (the full pass
    // succeeds under the test-grade verifier).
    let fresh = v2_manifest("1.2.0", "1.0.0", 1, BASE_MILLIS + 1, &bundle);
    device
        .present_bundle(Some(&fresh), &bundle)
        .expect("one millis of validity is valid");
    cleanup(device.scratch());
}

#[test]
fn a_manifest_that_expires_while_the_device_is_down_is_refused_after_reboot() {
    let mut device = MockUpdateDevice::new("expiry-reboot", MockVerifierChoice::TestOnlyFnv1a64);
    let bundle = fixture_bundle();
    // Valid NOW (at BASE_MILLIS), expiring one hour later.
    let manifest = v2_manifest("1.2.0", "1.0.0", 1, BASE_MILLIS + 60 * 60_000, &bundle);
    // The device is down for two hours, then reboots (recomposition re-
    // reads the injected clock — the mock never reads wall time).
    device.advance_clock_to(BASE_MILLIS + 2 * 60 * 60_000);
    device.power_cycle();
    let error = device
        .present_bundle(Some(&manifest), &bundle)
        .expect_err("stale after the outage");
    assert!(matches!(error, UpdateBundleError::Expired { .. }));
    assert_eq!(
        device.last_seen_sequence(),
        0,
        "the stale manifest never advanced the counter"
    );
    cleanup(device.scratch());
}

// --- 5. ANTI-ROLLBACK NEGATIVES ------------------------------------------------------------

#[test]
fn a_replayed_manifest_is_refused_after_a_full_pass() {
    let device = MockUpdateDevice::new("replay", MockVerifierChoice::TestOnlyFnv1a64);
    let bundle = fixture_bundle();
    let manifest = v2_manifest("1.2.0", "1.0.0", 5, VALID_EXPIRY, &bundle);
    device
        .present_bundle(Some(&manifest), &bundle)
        .expect("first presentation passes");
    assert_eq!(device.last_seen_sequence(), 5);
    // Replay of the SAME manifest: refused (a full pass recorded the
    // sequence durably; re-presenting is never silent).
    assert!(matches!(
        device.present_bundle(Some(&manifest), &bundle),
        Err(UpdateBundleError::SequenceNotAhead { declared: 5, last_seen: 5 })
    ));
    cleanup(device.scratch());
}

#[test]
fn equal_and_lower_sequences_are_refused() {
    let device = MockUpdateDevice::new("equal-lower", MockVerifierChoice::TestOnlyFnv1a64);
    let bundle = fixture_bundle();
    let first = v2_manifest("1.2.0", "1.0.0", 9, VALID_EXPIRY, &bundle);
    device
        .present_bundle(Some(&first), &bundle)
        .expect("sequence 9 passes");
    // Equal sequence, different content: refused.
    let equal = v2_manifest("1.3.0", "1.0.0", 9, VALID_EXPIRY, &bundle);
    assert!(matches!(
        device.present_bundle(Some(&equal), &bundle),
        Err(UpdateBundleError::SequenceNotAhead { declared: 9, last_seen: 9 })
    ));
    // Lower sequence (the classic rollback attack: an old, validly
    // formed manifest re-presented): refused.
    let lower = v2_manifest("1.1.0", "1.0.0", 3, VALID_EXPIRY, &bundle);
    assert!(matches!(
        device.present_bundle(Some(&lower), &bundle),
        Err(UpdateBundleError::SequenceNotAhead { declared: 3, last_seen: 9 })
    ));
    cleanup(device.scratch());
}

#[test]
fn a_tampered_replay_is_refused_as_a_sequence_refusal_never_a_pass() {
    let device = MockUpdateDevice::new("tampered-replay", MockVerifierChoice::TestOnlyFnv1a64);
    let bundle = fixture_bundle();
    let manifest = v2_manifest("1.2.0", "1.0.0", 4, VALID_EXPIRY, &bundle);
    device
        .present_bundle(Some(&manifest), &bundle)
        .expect("passes once");
    // An attacker re-presents the SAME sequence with different bytes: the
    // sequence check fires BEFORE the base gate (step 6 precedes step 7),
    // so the refusal is the anti-rollback one — either way, never a pass.
    let error = device
        .present_bundle(Some(&manifest), b"tampered bytes")
        .expect_err("refused");
    assert!(
        matches!(error, UpdateBundleError::SequenceNotAhead { .. }),
        "the sequence check precedes digest work: {error:?}"
    );
    cleanup(device.scratch());
}

#[test]
fn the_counter_reopens_monotonic_from_disk_replay() {
    let mut device = MockUpdateDevice::new("counter-reopen", MockVerifierChoice::TestOnlyFnv1a64);
    let bundle = fixture_bundle();
    let first = v2_manifest("1.2.0", "1.0.0", 6, VALID_EXPIRY, &bundle);
    device
        .present_bundle(Some(&first), &bundle)
        .expect("sequence 6 accepted");
    // Power loss and reboot: the last-seen value is DERIVED by replay
    // from the durable file — the register is the source of truth.
    device.power_cycle();
    assert_eq!(device.last_seen_sequence(), 6, "replay-derived, not remembered");
    // The pre-loss manifest is now a replay: refused.
    assert!(matches!(
        device.present_bundle(Some(&first), &bundle),
        Err(UpdateBundleError::SequenceNotAhead { declared: 6, last_seen: 6 })
    ));
    cleanup(device.scratch());
}

#[test]
fn a_torn_counter_tail_is_truncated_and_counted_never_silent() {
    use std::io::Write;
    let mut device = MockUpdateDevice::new("torn-tail", MockVerifierChoice::TestOnlyFnv1a64);
    let bundle = fixture_bundle();
    let first = v2_manifest("1.2.0", "1.0.0", 12, VALID_EXPIRY, &bundle);
    device
        .present_bundle(Some(&first), &bundle)
        .expect("sequence 12 accepted");
    // SIMULATED POWER LOSS MID-WRITE: a torn append leaves a partial
    // record frame on disk. Drop the gate's handle first (the counter
    // file is held open by the gate), then append the torn frame.
    device.power_cycle();
    {
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(device.counter_path())
            .expect("counter file opens for the torn-write simulation");
        // A partial frame: a length prefix claiming 8 payload bytes,
        // followed by only 3 — a CRC-mismatch tail exactly like a torn
        // durable write.
        let mut torn = Vec::new();
        torn.extend_from_slice(&8u32.to_le_bytes());
        torn.extend_from_slice(&[0xde, 0xad, 0xbe]);
        file.write_all(&torn).expect("torn tail written");
        file.sync_all().expect("torn tail durable");
    }
    // Reboot: the corrupt tail is truncated and COUNTED, and the
    // last-seen value is the maximum of the GOOD records.
    device.power_cycle();
    // The gate recomposition opened the counter internally; assert the
    // semantics through a direct open (the same file, the same rules).
    let counter = AntiRollbackCounter::open(device.counter_path()).expect("reopens");
    assert_eq!(
        counter.corrupt_tail_truncated(),
        1,
        "the torn tail is counted, never silent"
    );
    assert_eq!(counter.last_seen(), 12, "the good record survives the torn tail");
    // And the monotonicity survives: the pre-loss sequence is still a
    // replay refusal; a fresh higher sequence still advances.
    assert!(matches!(
        counter.accept(12),
        Err(UpdateBundleError::SequenceNotAhead { declared: 12, last_seen: 12 })
    ));
    counter.accept(13).expect("13 advances");
    cleanup(device.scratch());
}

#[test]
fn a_foreign_file_is_refused_as_not_a_counter() {
    let dir = p18_14_support::scratch_dir("foreign-counter");
    let path = dir.join("not-a-counter.bin");
    std::fs::write(&path, b"attacker-planted bytes with no magic").expect("writes");
    let error = AntiRollbackCounter::open(&path).expect_err("refused");
    assert!(matches!(
        error,
        UpdateBundleError::NotAnAntiRollbackCounter { .. }
    ));
    cleanup(&dir);
}

#[test]
fn a_refused_verdict_never_advances_the_counter() {
    let device = MockUpdateDevice::new("no-advance", MockVerifierChoice::TestOnlyFnv1a64);
    let bundle = fixture_bundle();
    // A digest mismatch (bytes swapped under a valid manifest shape).
    let manifest = v2_manifest("1.2.0", "1.0.0", 3, VALID_EXPIRY, &bundle);
    let wrong_bytes = b"not the bundle the manifest commits to".to_vec();
    assert!(matches!(
        device.present_bundle(Some(&manifest), &wrong_bytes),
        Err(UpdateBundleError::BaseVerificationRefused { code, .. }) if code == "UPDATE_DIGEST_MISMATCH"
    ));
    assert_eq!(
        device.last_seen_sequence(),
        0,
        "a refused verdict records nothing (durable-before-verdict, never before-pass)"
    );
    cleanup(device.scratch());
}

#[test]
fn the_fail_closed_default_refuses_everything_and_advances_nothing() {
    // NO VERIFIER CONFIGURED — the shipped composition. The P9.34
    // fail-closed semantics preserved verbatim: the extended gate's pass
    // is impossible because the base gate's inner step refuses
    // everything.
    let device = MockUpdateDevice::new("fail-closed", MockVerifierChoice::NoneConfigured);
    let bundle = fixture_bundle();
    let manifest = v2_manifest("1.2.0", "1.0.0", 1, VALID_EXPIRY, &bundle);
    let error = device
        .present_bundle(Some(&manifest), &bundle)
        .expect_err("refused");
    match error {
        UpdateBundleError::BaseVerificationRefused { code, reason } => {
            assert_eq!(
                code, "UPDATE_VERIFIER_REFUSED",
                "the P9.34 stable code rides through"
            );
            assert!(reason.contains("fail-closed"), "{reason}");
        }
        other => panic!("expected BaseVerificationRefused, got {other:?}"),
    }
    assert_eq!(device.last_seen_sequence(), 0);
    // The audit shows both honest layers.
    let lines: Vec<String> = device
        .audit
        .events()
        .iter()
        .map(|event| event.detail.clone())
        .collect();
    assert!(
        lines.iter().any(|l| l.contains("EDGE_FLEET_UPDATE_REFUSED")),
        "the extended refusal is audited: {lines:?}"
    );
    assert!(
        lines
            .iter()
            .any(|l| l.contains("EDGE_UPDATE_VERIFICATION_REFUSED")),
        "the P9.34 refusal rides through the base gate: {lines:?}"
    );
    cleanup(device.scratch());
}

#[test]
fn the_sha256_option_round_trips_and_refuses_a_payload_mismatch() {
    // The P18.05 SHA-256 verifier (the P11.15 std-only implementation,
    // consumed not duplicated), exercised as CODE — wiring it under
    // reviewed key management remains a DEPLOYMENT REQUIREMENT, and
    // nothing here is a release signature.
    use flexcore_edge_agent::update_verification::{UpdateDigestVerdict, UpdateVerifier};
    let device = MockUpdateDevice::new("sha256-option", MockVerifierChoice::Sha256);
    let bundle = fixture_bundle();
    // The published FIPS 180-4 "abc" vector pins the algorithm selection
    // and the hex comparison (the same anchor update_session's unit test
    // pins).
    assert_eq!(
        flexcore_edge_agent::update_session::Sha256UpdateVerifier.verify(
            b"abc",
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        ),
        UpdateDigestVerdict::Matches
    );
    // A full pass with a SHA-256 manifest: the declared digest is the one
    // the SAME verifier computes over the fixture bytes (honest fixture
    // construction — the expectation is shared with the implementation,
    // not fabricated).
    let digest = flexcore_audit_sha256_of(&bundle);
    let mut manifest = v2_manifest("1.2.0", "1.0.0", 1, VALID_EXPIRY, &bundle);
    manifest.base.digest_algorithm = "sha-256".to_string();
    manifest.base.bundle_digest_hex = digest.clone();
    manifest.payloads[0].digest_hex = digest;
    device
        .present_bundle(Some(&manifest), &bundle)
        .expect("the sha-256 manifest passes the extended gate");
    assert_eq!(device.last_seen_sequence(), 1);
    // A corrupted payload digest is refused under sha-256 too.
    let mut bad = v2_manifest("1.3.0", "1.0.0", 2, VALID_EXPIRY, &bundle);
    bad.base.digest_algorithm = "sha-256".to_string();
    bad.base.bundle_digest_hex = flexcore_audit_sha256_of(&bundle);
    bad.payloads[0].digest_hex =
        "0000000000000000000000000000000000000000000000000000000000000000".to_string();
    assert!(matches!(
        device.present_bundle(Some(&bad), &bundle),
        Err(UpdateBundleError::PayloadDigestMismatch { .. })
    ));
    cleanup(device.scratch());
}

/// The SHA-256 digest of `bytes` as lowercase hex, computed through the
/// P11.15 std-only implementation that `Sha256UpdateVerifier` consumes —
/// the fixture shares the implementation instead of fabricating an
/// expectation.
fn flexcore_audit_sha256_of(bytes: &[u8]) -> String {
    flexcore_audit::chain::hex_encode_digest(&flexcore_audit::chain::sha256(bytes))
}

#[test]
fn payload_bounds_outside_the_bundle_are_refused() {
    let device = MockUpdateDevice::new("bounds", MockVerifierChoice::TestOnlyFnv1a64);
    let bundle = fixture_bundle();
    let mut manifest = v2_manifest("1.2.0", "1.0.0", 1, VALID_EXPIRY, &bundle);
    manifest.payloads[0].offset = bundle.len() + 1;
    assert!(matches!(
        device.present_bundle(Some(&manifest), &bundle),
        Err(UpdateBundleError::PayloadBoundsOutside { payload_name }) if payload_name == "fw.bin"
    ));
    cleanup(device.scratch());
}

#[test]
fn a_manifest_whose_declared_size_exceeds_the_total_cap_is_refused() {
    // The byte-level cap at the gate is checked against `bundle.len()`
    // against the same constant pinned above; the structural route here
    // proves the ordering without allocating a GiB of test bytes.
    let device = MockUpdateDevice::new("too-large", MockVerifierChoice::TestOnlyFnv1a64);
    let bundle = fixture_bundle();
    let mut manifest = v2_manifest("1.2.0", "1.0.0", 1, VALID_EXPIRY, &bundle);
    manifest.payloads[0].size_bytes = MAX_UPDATE_PAYLOAD_TOTAL_BYTES + 1;
    assert!(matches!(
        device.present_bundle(Some(&manifest), &bundle),
        Err(UpdateBundleError::ManifestInvalid { .. })
    ));
    cleanup(device.scratch());
}

#[test]
fn an_unsigned_presentation_is_refused_first() {
    let device = MockUpdateDevice::new("unsigned", MockVerifierChoice::TestOnlyFnv1a64);
    let bundle = fixture_bundle();
    let error = device.present_bundle(None, &bundle).expect_err("refused");
    assert!(matches!(error, UpdateBundleError::UnsignedBundle));
    assert_eq!(device.last_seen_sequence(), 0);
    cleanup(device.scratch());
}

// --- 6. Cross-links -----------------------------------------------------------------------

#[test]
fn the_p9_34_base_validation_remains_the_inner_authority() {
    // The v2 layer never weakens the base rules: a manifest that violates
    // a P9.34 rule (below its own floor) is refused through the v2
    // validation with the P9.34 text riding the reason.
    let bundle = fixture_bundle();
    let manifest = v2_manifest("0.9.0", "1.0.0", 1, VALID_EXPIRY, &bundle);
    let error = manifest.validate().expect_err("below own floor");
    let UpdateBundleError::ManifestInvalid { reason } = error else {
        panic!("expected ManifestInvalid, got {error:?}")
    };
    assert!(reason.contains("below the minimum-version floor"), "{reason}");
}

#[test]
fn the_counter_semantics_statement_matches_the_implementation() {
    // The strictly-ahead rule implemented in `sequence_refuses` and
    // `AntiRollbackCounter::accept` is the one the statement records
    // (UNVERIFIED against the TUF/SUIT spec texts — the statement says
    // so, and this test only pins that the code and the statement agree
    // with each other).
    let bundle = fixture_bundle();
    let manifest = v2_manifest("1.2.0", "1.0.0", 2, VALID_EXPIRY, &bundle);
    assert!(manifest.sequence_refuses(1).is_ok());
    assert!(manifest.sequence_refuses(2).is_err());
    assert!(manifest.sequence_refuses(3).is_err());
}
