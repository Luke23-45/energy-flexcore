// P9.34 — Integration tests: signed edge-update VERIFICATION and ROLLBACK
//         (ledger P9.34; report §18.3, §17.3, §21.2).
//
// EVERY TEST BELOW IS UNRUN (the loop bans in
// `ledger/phase-09-pilot-qualification/ENTRY_NOTE.md` boundary 5 forbid
// building and running tests; the human build/test pass owns execution).
// Nothing here claims a pass. NO UPDATE HAS EVER BEEN BUILT, SIGNED,
// STAGED, OR APPLIED — the ledger's "apply a valid update to one
// non-production edge" (step 2) and "an update that fails mid-apply"
// (step 3) are the recorded UNRUN human executions; these tests exercise
// the verification/refusal/rollback-decision STRUCTURE those executions
// drive.
//
// The matrix (each test hand-computed against the recorded rules):
// - the happy path: a verified update activates, the register reports the
//   new version, the activation is audited;
// - LEDGER STEP 1 (the named risk "verification that runs after files are
//   already written"): a bad-signature bundle and an untrusted-key bundle
//   are refused BEFORE any file is written — the gate takes no path and
//   the scratch directory holds ZERO files after the refusals; both
//   refusals are audited with their stable tags;
// - the fail-closed default: `NoUpdateVerifier` refuses EVERYTHING (the
//   absence of signing infrastructure can never look like success);
// - the unsigned bundle is refused before any other check (the P7.14
//   rule);
// - the minimum-version floor is enforced at install (the equal-to-floor
//   boundary accepted, below refused — hand-computed);
// - LEDGER STEP 3: a rollback re-activates a PREVIOUSLY-VERIFIED version
//   with the recorded reason, audited, the register keeping both terms;
// - the never-silent downgrade: a rollback BELOW the minimum-version floor
//   is refused without an explicit recorded override; with the override,
//   the override rides the register record AND the audit line;
// - impossible rollbacks (never-activated target, not-behind target) are
//   refused by name;
// - the register round-trips a restart, refuses foreign files, and
//   truncates corrupt tails with a counted loss.

use std::sync::Arc;

use flexcore_ports::clock::Clock;
use flexcore_ports::fakes::FixedClock;

use flexcore_edge_agent::ports::{AuditSink, InMemoryAuditSink};
use flexcore_edge_agent::update_verification::{
    compare_versions, FileUpdateLedger, NoUpdateVerifier, RollbackOverride,
    TestOnlyFnv1a64UpdateVerifier, UpdateActivation, UpdateGate, UpdateLedgerEventKind,
    UpdateLedgerRecord, UpdateVerificationError, UPDATE_SLOT_DECISION, UPDATE_STATEMENT,
};

const BASE_MILLIS: i64 = 1_700_000_000_000;
const TRUSTED_RELEASE_KEY: &str = "key-store:pilot-release-key-1";

// --- Shared fixtures -------------------------------------------------------------

fn scratch_dir(label: &str) -> std::path::PathBuf {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "flexcore-edge-p934-it-{}-{}-{}",
        std::process::id(),
        label,
        COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).expect("scratch dir creates");
    dir
}

/// The fixture bundle bytes (a lab string; no update has ever been built).
fn bundle_bytes() -> Vec<u8> {
    b"flexcore-edge-update-bundle-fixture-bytes".to_vec()
}

/// The manifest for the fixture bundle: the digest is computed by the
/// SAME labelled test-grade helper the verifier computes (honest fixture
/// construction, not a fabricated digest).
fn manifest_for(version: &str, floor: &str, bundle: &[u8]) -> UpdateManifest {
    UpdateManifest {
        package_id: "flexcore-edge-update".to_string(),
        version: version.to_string(),
        bundle_digest_hex: flexcore_edge_agent::update_verification::test_grade_fnv1a64_hex(bundle),
        digest_algorithm: "fnv1a64-test".to_string(),
        signer_key_reference: TRUSTED_RELEASE_KEY.to_string(),
        minimum_version_floor: floor.to_string(),
        approval_reference: "pilot-update-approval-001".to_string(),
    }
}

fn harness() -> (UpdateGate, UpdateActivation, Arc<InMemoryAuditSink>, Arc<dyn Clock + Send + Sync>) {
    let audit = Arc::new(InMemoryAuditSink::new());
    let clock: Arc<dyn Clock + Send + Sync> = Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 1_000));
    let gate = UpdateGate::new(
        vec![TRUSTED_RELEASE_KEY.to_string()],
        Box::new(TestOnlyFnv1a64UpdateVerifier),
        Arc::clone(&audit) as Arc<dyn AuditSink>,
        Arc::clone(&clock),
    );
    let activation = UpdateActivation::new(
        Arc::clone(&audit) as Arc<dyn AuditSink>,
        Arc::clone(&clock),
    );
    (gate, activation, audit, clock)
}

fn audited(audit: &InMemoryAuditSink, event_name: &str) -> Vec<String> {
    audit
        .events()
        .iter()
        .filter(|event| event.detail.starts_with(event_name))
        .map(|event| event.detail.clone())
        .collect()
}

// --- 1. The happy path: verified → activated → version reported ------------------

#[test]
fn a_valid_update_verifies_activates_and_reports_its_new_version() {
    let (gate, activation, audit, _clock) = harness();
    let bundle = bundle_bytes();
    let manifest = manifest_for("1.2.0", "1.0.0", &bundle);

    // The definitional known answer: the test-grade digest of the EMPTY
    // input is the FNV-1a-64 offset basis (hand-derivable, pinned).
    assert_eq!(
        flexcore_edge_agent::update_verification::test_grade_fnv1a64_hex(&[]),
        "cbf29ce484222325"
    );

    let verified = gate
        .verify_before_staging(Some(&manifest), &bundle)
        .expect("the valid update verifies");
    assert_eq!(verified.manifest.version, "1.2.0");
    assert_eq!(audited(&audit, "EDGE_UPDATE_VERIFIED").len(), 1);

    let dir = scratch_dir("happy-path");
    let ledger = FileUpdateLedger::open(&dir.join("update.register")).expect("ledger opens");
    let record = activation.activate(&ledger, &verified).expect("activates");
    // THE NEW VERSION REPORT (ledger step 2): the activation record names
    // the version now active.
    assert_eq!(record.version, "1.2.0");
    assert_eq!(record.previous_version, "", "this is the first term");
    assert_eq!(ledger.active_version().as_deref(), Some("1.2.0"));
    assert_eq!(audited(&audit, "EDGE_UPDATE_ACTIVATED").len(), 1);

    // The recorded statements are pinned (the P4.28 statement pattern).
    assert!(UPDATE_STATEMENT.contains("No update has ever been built"));
    assert!(UPDATE_SLOT_DECISION.contains("RECOMMENDED deployment shape"));
    let _ = std::fs::remove_dir_all(&dir);
}

// --- 2. Ledger step 1: bad signature + untrusted key refused BEFORE any write -----

#[test]
fn bad_signature_and_untrusted_key_bundles_are_refused_before_any_file_is_written() {
    let (gate, _activation, audit, _clock) = harness();
    let dir = scratch_dir("refused-before-write");

    // BAD SIGNATURE: the bundle bytes were TAMPERED after signing — the
    // digest does not match. Refusal tag UPDATE_DIGEST_MISMATCH.
    let manifest = manifest_for("1.2.0", "1.0.0", &bundle_bytes());
    let tampered = {
        let mut bytes = bundle_bytes();
        bytes[0] ^= 0x01; // one flipped byte: a different payload
        bytes
    };
    let error = gate
        .verify_before_staging(Some(&manifest), &tampered)
        .expect_err("the bad-signature bundle is refused");
    assert_eq!(error.name(), "UPDATE_DIGEST_MISMATCH");

    // UNTRUSTED KEY: a well-formed manifest whose signer reference is
    // outside the trusted set — refused at the trust check, BEFORE the
    // digest check (the recorded refusal order).
    let mut rogue = manifest_for("1.2.0", "1.0.0", &bundle_bytes());
    rogue.signer_key_reference = "key-store:rogue-signing-key".to_string();
    let error = gate
        .verify_before_staging(Some(&rogue), &bundle_bytes())
        .expect_err("the untrusted-key bundle is refused");
    assert_eq!(error.name(), "UPDATE_UNTRUSTED_SIGNER");

    // BOTH refusals are audited with their stable tags.
    let refusals = audited(&audit, "EDGE_UPDATE_VERIFICATION_REFUSED");
    assert_eq!(refusals.len(), 2, "each refusal audited once: {refusals:?}");
    assert!(refusals.iter().any(|d| d.contains("UPDATE_DIGEST_MISMATCH")));
    assert!(refusals.iter().any(|d| d.contains("UPDATE_UNTRUSTED_SIGNER")));
    assert!(
        refusals.iter().all(|d| d.contains("refused BEFORE any file was written")),
        "the audit line records the BEFORE-any-write fact: {refusals:?}"
    );

    // THE STRUCTURAL CHECK: the gate holds no path and performed no write —
    // the scratch directory contains ZERO files after both refusals.
    let created: Vec<_> = std::fs::read_dir(&dir)
        .expect("reads the scratch dir")
        .filter_map(|entry| entry.ok())
        .collect();
    assert!(
        created.is_empty(),
        "a refused bundle leaves NO file behind (ledger P9.34 step 1): {created:?}"
    );

    // The register, when the composition later opens it, holds zero
    // records: no refused bundle ever reached it.
    let ledger = FileUpdateLedger::open(&dir.join("update.register")).expect("opens");
    assert!(ledger.is_empty(), "no refused bundle produced a register record");
    let _ = std::fs::remove_dir_all(&dir);
}

// --- 3. The fail-closed default verifier refuses everything -----------------------

#[test]
fn the_fail_closed_default_verifier_refuses_every_bundle() {
    let audit = Arc::new(InMemoryAuditSink::new());
    let clock: Arc<dyn Clock + Send + Sync> = Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 1_000));
    // THE DEFAULT: no verifier wired — the deployment that wires nothing
    // verifies nothing.
    let gate = UpdateGate::new(
        vec![TRUSTED_RELEASE_KEY.to_string()],
        Box::new(NoUpdateVerifier),
        Arc::clone(&audit) as Arc<dyn AuditSink>,
        Arc::clone(&clock),
    );
    let bundle = bundle_bytes();

    // A manifest declaring the PRODUCTION algorithm name (sha-256): refused
    // at the algorithm check — the configured verifier computes none.
    let mut production = manifest_for("1.2.0", "1.0.0", &bundle);
    production.digest_algorithm = "sha-256".to_string();
    let error = gate
        .verify_before_staging(Some(&production), &bundle)
        .expect_err("an algorithm the verifier does not compute is refused");
    assert_eq!(error.name(), "UPDATE_DIGEST_ALGORITHM_MISMATCH");

    // A manifest declaring the TEST algorithm under the NoVerifier gate:
    // refused at the same check ('none' != 'fnv1a64-test').
    let test_algo = manifest_for("1.2.0", "1.0.0", &bundle);
    let error = gate
        .verify_before_staging(Some(&test_algo), &bundle)
        .expect_err("the no-verifier gate refuses");
    assert_eq!(error.name(), "UPDATE_DIGEST_ALGORITHM_MISMATCH");
    assert_eq!(audited(&audit, "EDGE_UPDATE_VERIFICATION_REFUSED").len(), 2);
}

// --- 4. The unsigned bundle is refused before anything else -----------------------

#[test]
fn an_unsigned_bundle_is_refused_before_any_other_check() {
    let (gate, _activation, audit, _clock) = harness();
    let error = gate
        .verify_before_staging(None, &bundle_bytes())
        .expect_err("an unsigned bundle is refused");
    assert_eq!(error.name(), "UPDATE_UNSIGNED_BUNDLE");
    assert_eq!(audited(&audit, "EDGE_UPDATE_VERIFICATION_REFUSED").len(), 1);
}

// --- 5. The minimum-version floor at install (hand-computed boundary) --------------

#[test]
fn below_the_minimum_version_floor_is_refused_and_equal_to_it_passes() {
    let (gate, _activation, audit, _clock) = harness();
    let bundle = bundle_bytes();

    // BELOW: 1.9.9 < floor 2.0.0 → refused at install.
    let below = manifest_for("1.9.9", "2.0.0", &bundle);
    let error = gate
        .verify_before_staging(Some(&below), &bundle)
        .expect_err("a below-floor version is refused");
    assert_eq!(error.name(), "UPDATE_BELOW_MINIMUM_FLOOR");

    // EQUAL: 2.0.0 == floor 2.0.0 → accepted (the hand-computed boundary).
    let equal = manifest_for("2.0.0", "2.0.0", &bundle);
    gate.verify_before_staging(Some(&equal), &bundle)
        .expect("the equal-to-floor version installs");
    assert_eq!(audited(&audit, "EDGE_UPDATE_VERIFICATION_REFUSED").len(), 1);
}

// --- 6. Ledger step 3: rollback to a previously verified version -------------------

#[test]
fn a_rollback_to_a_previously_verified_version_records_the_reason() {
    let (gate, activation, audit, _clock) = harness();
    let bundle = bundle_bytes();
    let dir = scratch_dir("rollback");
    let ledger = FileUpdateLedger::open(&dir.join("update.register")).expect("ledger opens");

    // Two valid updates activate in order: 1.0.1, then 1.2.0.
    for version in ["1.0.1", "1.2.0"] {
        let manifest = manifest_for(version, "1.0.0", &bundle);
        let verified = gate
            .verify_before_staging(Some(&manifest), &bundle)
            .expect("the update verifies");
        activation.activate(&ledger, &verified).expect("activates");
    }
    assert_eq!(ledger.active_version().as_deref(), Some("1.2.0"));

    // THE ROLLBACK (ledger step 3): re-activate the PREVIOUSLY-VERIFIED
    // 1.0.1 with the recorded reason (the mid-apply failure case's
    // decision surface; the actual mid-apply execution is the UNRUN human
    // gate).
    let record = activation
        .rollback(
            &ledger,
            "1.0.0", // the deployment's minimum-version floor
            "1.0.1",
            "post-update health check failed; rolled back to the last verified version",
            None,
        )
        .expect("the rollback records");
    // The edge reports its RE-ACTIVATED version.
    assert_eq!(record.version, "1.0.1");
    assert_eq!(record.previous_version, "1.2.0");
    assert_eq!(ledger.active_version().as_deref(), Some("1.0.1"));
    assert_eq!(record.override_reference, "", "an above-floor rollback needs no override");

    // The register keeps ALL THREE events (append-only, never overwritten).
    assert_eq!(ledger.len(), 3);
    let versions: Vec<&str> = ledger.history().iter().map(|r| r.version.as_str()).collect();
    assert_eq!(versions, vec!["1.0.1", "1.2.0", "1.0.1"]);

    // The rollback is audited, and the re-activation was of a version the
    // register had SEEN (the re-activation rule).
    let rollbacks = audited(&audit, "EDGE_UPDATE_ROLLBACK");
    assert_eq!(rollbacks.len(), 1);
    assert!(rollbacks[0].contains("rolled back 1.2.0 -> 1.0.1"));
    assert!(ledger.previously_activated().contains("1.2.0"));
    let _ = std::fs::remove_dir_all(&dir);
}

// --- 7. The never-silent downgrade rule (floor + recorded override) ----------------

#[test]
fn a_rollback_below_the_floor_is_refused_unless_a_recorded_override_exists() {
    let (gate, activation, audit, _clock) = harness();
    let bundle = bundle_bytes();
    let dir = scratch_dir("below-floor");
    let ledger = FileUpdateLedger::open(&dir.join("update.register")).expect("ledger opens");

    // History: 1.9.0 activated (under the older manifest floor 1.0.0),
    // then 2.0.1 activated (same floor). The DEPLOYMENT floor has since
    // RISEN to 2.0.0 (the parameter the composition passes).
    for version in ["1.9.0", "2.0.1"] {
        let manifest = manifest_for(version, "1.0.0", &bundle);
        let verified = gate
            .verify_before_staging(Some(&manifest), &bundle)
            .expect("the update verifies");
        activation.activate(&ledger, &verified).expect("activates");
    }
    assert_eq!(ledger.active_version().as_deref(), Some("2.0.1"));

    // THE REFUSAL: rolling back to 1.9.0 (hand-computed: 1.9.0 < floor
    // 2.0.0) without an override is the named
    // RollbackBelowFloorRefused — never a silent downgrade.
    let error = activation
        .rollback(&ledger, "2.0.0", "1.9.0", "operator prefers the old build", None)
        .expect_err("a below-floor rollback is refused");
    assert_eq!(error.name(), "UPDATE_ROLLBACK_BELOW_FLOOR_REFUSED");
    assert_eq!(ledger.active_version().as_deref(), Some("2.0.1"), "nothing moved");

    // An override WITHOUT a named actor/reference is not a recorded
    // override — still refused.
    let empty_override = RollbackOverride {
        actor: String::new(),
        authority_reference: String::new(),
    };
    let error = activation
        .rollback(&ledger, "2.0.0", "1.9.0", "operator prefers the old build", Some(&empty_override))
        .expect_err("an empty override is invalid");
    assert_eq!(error.name(), "UPDATE_ROLLBACK_OVERRIDE_INVALID");

    // WITH the explicit recorded override: the rollback proceeds, and the
    // override RIDES THE REGISTER RECORD AND THE AUDIT LINE.
    let recorded = RollbackOverride {
        actor: "J. Doe (platform security lead)".to_string(),
        authority_reference: "pilot-rollback-override-001".to_string(),
    };
    let record = activation
        .rollback(&ledger, "2.0.0", "1.9.0", "authorized exception, ticket recorded", Some(&recorded))
        .expect("the override rollback records");
    assert_eq!(record.version, "1.9.0");
    assert!(
        record.override_reference.contains("pilot-rollback-override-001"),
        "the override reference is IN the register record: {}",
        record.override_reference
    );
    let rollbacks = audited(&audit, "EDGE_UPDATE_ROLLBACK");
    assert_eq!(rollbacks.len(), 1);
    assert!(
        rollbacks[0].contains("RECORDED — never a silent downgrade"),
        "the audit line names the override explicitly: {}", rollbacks[0]
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// --- 8. Impossible rollbacks are refused by name ------------------------------------

#[test]
fn rollback_targets_that_were_never_activated_or_are_not_behind_are_refused() {
    let (gate, activation, _audit, _clock) = harness();
    let bundle = bundle_bytes();
    let dir = scratch_dir("impossible-rollback");
    let ledger = FileUpdateLedger::open(&dir.join("update.register")).expect("ledger opens");

    let manifest = manifest_for("2.0.0", "1.0.0", &bundle);
    let verified = gate
        .verify_before_staging(Some(&manifest), &bundle)
        .expect("verifies");
    activation.activate(&ledger, &verified).expect("activates");

    // NEVER ACTIVATED: 9.9.9 is not a version this register has seen — a
    // rollback re-activates a PREVIOUSLY-VERIFIED version, never an
    // unknown one.
    let error = activation
        .rollback(&ledger, "1.0.0", "9.9.9", "unknown target", None)
        .expect_err("a never-activated target is refused");
    assert_eq!(error.name(), "UPDATE_ROLLBACK_TARGET_NEVER_ACTIVATED");

    // NOT BEHIND: the target IS the active version.
    let error = activation
        .rollback(&ledger, "1.0.0", "2.0.0", "same version", None)
        .expect_err("a rollback to the active version is refused");
    assert_eq!(error.name(), "UPDATE_ROLLBACK_TARGET_NOT_BEHIND_ACTIVE");

    // NOT BEHIND, previously activated: 1.0.0 activates, then a later
    // "rollback" to the HIGHER 2.0.0 is an upgrade wearing a rollback's
    // clothes — refused.
    let older = manifest_for("1.0.0", "1.0.0", &bundle);
    let verified_older = gate
        .verify_before_staging(Some(&older), &bundle)
        .expect("verifies");
    activation.activate(&ledger, &verified_older).expect("activates");
    let error = activation
        .rollback(&ledger, "1.0.0", "2.0.0", "this is an upgrade, not a rollback", None)
        .expect_err("a forward 'rollback' is refused");
    assert_eq!(error.name(), "UPDATE_ROLLBACK_TARGET_NOT_BEHIND_ACTIVE");

    // A duplicate activation of the ACTIVE version is refused by name.
    let error = activation
        .activate(&ledger, &verified_older)
        .expect_err("a duplicate activation is refused");
    assert_eq!(error.name(), "UPDATE_VERSION_ALREADY_ACTIVE");
    let _ = std::fs::remove_dir_all(&dir);
}

// --- 9. The register: restart survival, foreign files, corrupt tails -----------------

#[test]
fn the_update_register_round_trips_refuses_foreign_files_and_truncates_corrupt_tails() {
    let dir = scratch_dir("register");
    let path = dir.join("update.register");
    {
        let ledger = FileUpdateLedger::open(&path).expect("opens");
        ledger
            .append(UpdateLedgerRecord {
                kind: UpdateLedgerEventKind::Activation,
                at_unix_millis: BASE_MILLIS + 1,
                version: "1.0.1".to_string(),
                previous_version: String::new(),
                manifest_digest_hex: "0011223344556677".to_string(),
                approval_or_reason: "pilot-update-approval-001".to_string(),
                override_reference: String::new(),
            })
            .expect("appends");
        ledger
            .append(UpdateLedgerRecord {
                kind: UpdateLedgerEventKind::Rollback,
                at_unix_millis: BASE_MILLIS + 2,
                version: "1.0.1".to_string(),
                previous_version: "1.2.0".to_string(),
                manifest_digest_hex: String::new(),
                approval_or_reason: "health check failed".to_string(),
                override_reference: String::new(),
            })
            .expect("appends");
    }
    // THE RESTART: the active version is DERIVED by replay (the register
    // is the source of truth — the recorded slot decision).
    let reopened = FileUpdateLedger::open(&path).expect("reopens");
    assert_eq!(reopened.len(), 2);
    assert_eq!(reopened.active_version().as_deref(), Some("1.0.1"));
    assert_eq!(reopened.corrupt_tail_truncated(), 0);

    // A FOREIGN FILE is refused, never guessed at.
    let foreign = dir.join("not-a-register");
    std::fs::write(&foreign, b"something else entirely").expect("writes");
    assert!(matches!(
        FileUpdateLedger::open(&foreign),
        Err(UpdateVerificationError::NotAnUpdateLedger { .. })
    ));

    // A CORRUPT TAIL is truncated with a counted loss, never silent.
    let corrupted = dir.join("corrupted.register");
    {
        let ledger = FileUpdateLedger::open(&corrupted).expect("opens");
        ledger
            .append(UpdateLedgerRecord {
                kind: UpdateLedgerEventKind::Activation,
                at_unix_millis: BASE_MILLIS + 3,
                version: "1.4.0".to_string(),
                previous_version: String::new(),
                manifest_digest_hex: "aabbccddeeff0011".to_string(),
                approval_or_reason: "pilot-update-approval-002".to_string(),
                override_reference: String::new(),
            })
            .expect("appends");
    }
    let mut raw = std::fs::read(&corrupted).expect("reads");
    raw.extend_from_slice(&[0xFF, 0x00, 0x00, 0x00, 0x99]); // a torn frame
    std::fs::write(&corrupted, &raw).expect("corrupts the tail");
    let recovered = FileUpdateLedger::open(&corrupted).expect("recovers");
    assert_eq!(recovered.len(), 1, "only the good record survives");
    assert_eq!(recovered.corrupt_tail_truncated(), 1, "the loss is counted");
    assert_eq!(recovered.active_version().as_deref(), Some("1.4.0"));
    let _ = std::fs::remove_dir_all(&dir);
}

// --- 10. Manifest validation is fail-closed ------------------------------------------

#[test]
fn manifest_validation_is_fail_closed_with_named_reasons() {
    let (gate, _activation, audit, _clock) = harness();
    let bundle = bundle_bytes();

    // A SCHEME-LESS signer reference is refused as likely inline key
    // material (the value is never echoed back — the P7.05/P7.14 rule).
    let mut inline_key = manifest_for("1.2.0", "1.0.0", &bundle);
    inline_key.signer_key_reference = "raw-key-material-lookalike".to_string();
    let error = gate
        .verify_before_staging(Some(&inline_key), &bundle)
        .expect_err("a scheme-less key reference is refused");
    assert_eq!(error.name(), "UPDATE_MANIFEST_INVALID");
    assert!(
        !error.to_string().contains("raw-key-material-lookalike"),
        "the rejected key-reference VALUE is never echoed: {error}"
    );

    // An UPPERCASE digest is refused (lowercase hex only).
    let mut uppercase = manifest_for("1.2.0", "1.0.0", &bundle);
    uppercase.bundle_digest_hex = uppercase.bundle_digest_hex.to_uppercase();
    let error = gate
        .verify_before_staging(Some(&uppercase), &bundle)
        .expect_err("an uppercase digest is refused");
    assert_eq!(error.name(), "UPDATE_MANIFEST_INVALID");

    // A MALFORMED floor and a MALFORMED version are refused.
    let mut malformed = manifest_for("1.2", "1.0.0", &bundle);
    let error = gate
        .verify_before_staging(Some(&malformed), &bundle)
        .expect_err("a two-component version is refused");
    assert_eq!(error.name(), "UPDATE_MANIFEST_INVALID");
    malformed.version = "1.2.0".to_string();
    malformed.minimum_version_floor = "one-point-oh".to_string();
    let error = gate
        .verify_before_staging(Some(&malformed), &bundle)
        .expect_err("a malformed floor is refused");
    assert_eq!(error.name(), "UPDATE_MANIFEST_INVALID");

    // An UNAPPROVED manifest (no approval reference) is refused fail-closed
    // (ledger step 4: the update policy names who approves).
    let mut unapproved = manifest_for("1.2.0", "1.0.0", &bundle);
    unapproved.approval_reference = String::new();
    let error = gate
        .verify_before_staging(Some(&unapproved), &bundle)
        .expect_err("an unapproved manifest is refused");
    assert_eq!(error.name(), "UPDATE_MANIFEST_INVALID");

    // A digest algorithm outside the CLOSED vocabulary is refused at
    // validation.
    let mut foreign_algo = manifest_for("1.2.0", "1.0.0", &bundle);
    foreign_algo.digest_algorithm = "md5".to_string();
    let error = gate
        .verify_before_staging(Some(&foreign_algo), &bundle)
        .expect_err("an unknown algorithm is refused");
    assert_eq!(error.name(), "UPDATE_MANIFEST_INVALID");

    assert_eq!(audited(&audit, "EDGE_UPDATE_VERIFICATION_REFUSED").len(), 6);

    // The hand-computed ordering known answers once more, at the boundary
    // the floor rule rides on.
    assert_eq!(compare_versions("2.0.0", "2.0.0"), Ok(std::cmp::Ordering::Equal));
    assert_eq!(
        compare_versions("1.9.9", "2.0.0"),
        Ok(std::cmp::Ordering::Less)
    );
}
