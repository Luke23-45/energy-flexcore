// P18.14 — SHARED TEST SUPPORT: the mock device test double and the
//          fixtures for the phase-18 update test suite (plan §2 P18.14;
//          decision record `docs/phases-11-20/phase-18-edge-fleet-
//          lifecycle-ota/records/41_p18_14_update_test_suite_mock_device.md`).
//
// WHAT THIS IS: a TEST DOUBLE and fixture helpers — written-unrun like
// every other phase-18 artifact (the loop bans forbid running tests;
// execution is the human gate). It composes the REAL update surfaces the
// phase built (`update_verification` P9.34, `update_bundle` P18.02,
// `update_session` P18.04/P18.05, `attestation` P18.13,
// `fleet_registry` P18.12) into one in-memory + temp-dir-backed object so
// the scenario suites can drive whole flows.
//
// WHAT THIS IS NOT (the honesty boundary, restated so no reader can
// mistake it): **NO DEVICE EXISTS.** `MockUpdateDevice` is not a device,
// not a simulator of hardware, and not a field anything — it is a Rust
// struct in test code that calls the same library functions a composed
// edge agent would call, with no network, no transport, no EMS, no
// hardware, and no fleet. NO UPDATE HAS EVER BEEN PRODUCED, SIGNED,
// STAGED, OR APPLIED by it or by anything else in this program — the
// bundles here are fixture byte strings, the manifests are fixture
// structures, the signatures are the loudly-labelled test-grade FNV-1a-64
// (`TestOnlyFnv1a64UpdateVerifier` — NOT a security primitive) or the
// SHA-256 option wired explicitly per test (a DEPLOYMENT REQUIREMENT,
// exercised here only as code, never as a release). Every disk touch is
// the crate's own temp-dir scratch pattern (the P9.34 integration-test
// convention); nothing writes outside `std::env::temp_dir()`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use flexcore_ports::clock::Clock;
use flexcore_ports::fakes::FixedClock;

use flexcore_edge_agent::attestation::{
    AttestationVerificationPolicy, MeasurementExpectations, TrustAnchorMode,
};
use flexcore_edge_agent::fleet_registry::FleetRegistryError;
use flexcore_edge_agent::ports::{AuditSink, InMemoryAuditSink};
use flexcore_edge_agent::update_bundle::{
    AntiRollbackCounter, ExtendedUpdateManifest, PayloadDigest, UpdateApplicability,
    UpdateBundleError,
};
use flexcore_edge_agent::update_session::{ExtendedUpdateGate, Sha256UpdateVerifier};
use flexcore_edge_agent::update_verification::{
    test_grade_fnv1a64_hex, FileUpdateLedger, NoUpdateVerifier, TestOnlyFnv1a64UpdateVerifier,
    UpdateActivation, UpdateDigestVerdict, UpdateManifest, UpdateVerifier,
};

// --- The fixture constants -------------------------------------------------------------

/// The base instant every fixture clock starts at (the P9.34 suite
/// convention).
pub const BASE_MILLIS: i64 = 1_700_000_000_000;

/// The device identity the mock reports (fixture data, not a real device).
pub const DEVICE_VENDOR: &str = "neryva";
pub const DEVICE_CLASS: &str = "edge-gateway";
pub const DEVICE_ID: &str = "mock-device-1";

/// The trusted signer key REFERENCE the fixture gates hold (a reference
/// only — no key material exists anywhere in this program).
pub const TRUSTED_KEY_REFERENCE: &str = "key-store:fixture-release-targets";

/// The fixture bundle bytes (a lab string; no update has ever been built
/// from these bytes and none ever will be inside this program).
pub fn fixture_bundle() -> Vec<u8> {
    b"flexcore-p18-14-fixture-bundle-bytes-0123456789abcdef".to_vec()
}

/// The digest helper the fixtures declare (the SAME labelled test-grade
/// computation the `TestOnlyFnv1a64UpdateVerifier` performs — honest
/// fixture construction, not a fabricated digest).
pub fn fixture_digest(bytes: &[u8]) -> String {
    test_grade_fnv1a64_hex(bytes)
}

/// A P9.34 v1 base manifest matching `bundle` under the test-grade
/// verifier (labelled test-grade, loudly, everywhere).
pub fn base_manifest(version: &str, floor: &str, bundle: &[u8]) -> UpdateManifest {
    UpdateManifest {
        package_id: "flexcore-edge-fixture-release".to_string(),
        version: version.to_string(),
        bundle_digest_hex: fixture_digest(bundle),
        digest_algorithm: "fnv1a64-test".to_string(),
        signer_key_reference: TRUSTED_KEY_REFERENCE.to_string(),
        minimum_version_floor: floor.to_string(),
        approval_reference: "fixture-approval-001".to_string(),
    }
}

/// A v2 manifest over the WHOLE fixture bundle with ONE payload entry.
pub fn v2_manifest(
    version: &str,
    floor: &str,
    sequence: u64,
    expires_at_unix_millis: i64,
    bundle: &[u8],
) -> ExtendedUpdateManifest {
    ExtendedUpdateManifest {
        base: base_manifest(version, floor, bundle),
        applicability: UpdateApplicability {
            vendor: DEVICE_VENDOR.to_string(),
            device_class: DEVICE_CLASS.to_string(),
            device_ids: BTreeSet::new(),
        },
        anti_rollback_sequence: sequence,
        payloads: vec![PayloadDigest {
            name: "fw.bin".to_string(),
            offset: 0,
            size_bytes: bundle.len(),
            digest_hex: fixture_digest(bundle),
        }],
        expires_at_unix_millis,
    }
}

/// The scratch directory for one test (unique per call — the P9.34
/// integration-test pattern; nothing is written outside `temp_dir`).
pub fn scratch_dir(label: &str) -> PathBuf {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "flexcore-edge-p18-14-{}-{}-{}",
        std::process::id(),
        label,
        COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).expect("scratch dir creates");
    dir
}

// --- The mock device (TEST DOUBLE — no device exists) ------------------------------------

/// Which verifier the mock's gate is composed with (the composition
/// root's P9.34 choice, mirrored here per test).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MockVerifierChoice {
    /// The fail-closed DEFAULT shipped by the code: everything refused.
    NoneConfigured,
    /// The labelled test-grade FNV-1a-64 (NOT a security primitive —
    /// exists so the pass/refusal paths are exercisable without crypto).
    TestOnlyFnv1a64,
    /// The P18.05 SHA-256 option (consumes the P11.15 std-only
    /// implementation; wiring it under reviewed key management remains a
    /// DEPLOYMENT REQUIREMENT — this exercise is code-level only).
    Sha256,
}

/// THE MOCK DEVICE (TEST DOUBLE). One in-process object standing in for
/// an edge device's update path: it holds the composed extended gate
/// (P18.05), the anti-rollback counter (P18.02), the append-only
/// update/rollback register (P9.34), and the activation/rollback surface
/// — all backed by the same temp-dir files a composed agent would use, so
/// the power-loss scenarios reopen REAL persisted state.
///
/// It cannot: talk to a network, hold key material, produce or sign a
/// bundle, touch hardware, or represent a real device. It exists so the
/// recorded flows can be exercised as CODE at the human test gate.
pub struct MockUpdateDevice {
    vendor: String,
    device_class: String,
    device_id: String,
    verifier_choice: MockVerifierChoice,
    /// The audit sink the gate and the activation surface record into
    /// (in-memory; the test reads the lines).
    pub audit: Arc<InMemoryAuditSink>,
    clock_millis: std::cell::Cell<i64>,
    scratch: PathBuf,
    ledger_path: PathBuf,
    counter_path: PathBuf,
    /// The composed extended gate (holds the counter — see `reboot`).
    gate: Option<ExtendedUpdateGate>,
}

impl MockUpdateDevice {
    /// Composes the mock (fail-closed default verifier unless the test
    /// chooses otherwise).
    pub fn new(label: &str, verifier_choice: MockVerifierChoice) -> Self {
        let scratch = scratch_dir(label);
        let audit = Arc::new(InMemoryAuditSink::new());
        let mut device = Self {
            vendor: DEVICE_VENDOR.to_string(),
            device_class: DEVICE_CLASS.to_string(),
            device_id: DEVICE_ID.to_string(),
            verifier_choice,
            audit,
            clock_millis: std::cell::Cell::new(BASE_MILLIS),
            scratch,
            ledger_path: PathBuf::new(),
            counter_path: PathBuf::new(),
            gate: None,
        };
        device.ledger_path = device.scratch.join("update-register.ledger");
        device.counter_path = device.scratch.join("anti-rollback.counter");
        device.compose_gate();
        device
    }

    fn verifier_for(&self) -> Arc<dyn UpdateVerifier + Send + Sync> {
        match self.verifier_choice {
            MockVerifierChoice::NoneConfigured => Arc::new(NoUpdateVerifier),
            MockVerifierChoice::TestOnlyFnv1a64 => Arc::new(TestOnlyFnv1a64UpdateVerifier),
            // The verifier is a stateless struct; the digest work is the
            // P11.15 implementation's.
            MockVerifierChoice::Sha256 => Arc::new(Sha256UpdateVerifier),
        }
    }

    /// Composes (or recomposes, after a simulated reboot) the extended
    /// gate over a FRESHLY-OPENED counter at the persistent path.
    fn compose_gate(&mut self) {
        let counter = AntiRollbackCounter::open(&self.counter_path)
            .expect("mock device: the anti-rollback counter opens");
        let audit: Arc<dyn AuditSink> = self.audit.clone();
        let clock: Arc<dyn Clock + Send + Sync> =
            Arc::new(FixedClock::at_unix_millis(self.clock_millis.get()));
        let gate = ExtendedUpdateGate::new(
            self.vendor.clone(),
            self.device_class.clone(),
            self.device_id.clone(),
            vec![TRUSTED_KEY_REFERENCE.to_string()],
            self.verifier_for(),
            counter,
            audit,
            clock,
        )
        .expect("mock device: the extended gate composes");
        self.gate = Some(gate);
    }

    /// The device id the mock reports (fixture identity).
    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    /// The device class the mock reports (fixture identity).
    pub fn device_class(&self) -> &str {
        &self.device_class
    }

    /// Advances the fixture clock (the injected-clock discipline — the
    /// mock never reads wall time; this only changes what the NEXT
    /// composition sees, and gates composed earlier keep their frozen
    /// fixture clock, which is exactly the behavior the expiry tests
    /// rely on).
    pub fn advance_clock_to(&mut self, unix_millis: i64) {
        self.clock_millis.set(unix_millis);
    }

    /// The instant the mock's clock currently reports.
    pub fn now_unix_millis(&self) -> i64 {
        self.clock_millis.get()
    }

    /// The composed extended gate.
    pub fn gate(&self) -> &ExtendedUpdateGate {
        self.gate.as_ref().expect("the mock's gate is composed")
    }

    /// The last-seen anti-rollback sequence (through the gate's counter).
    pub fn last_seen_sequence(&self) -> u64 {
        self.gate().last_seen_sequence()
    }

    /// THE UPDATE PRESENTATION PATH (verify BEFORE any write): presents a
    /// bundle + optional manifest to the extended gate and returns the
    /// verdict. Nothing is staged, written, or applied by this call — the
    /// gate's contract is verification-only.
    pub fn present_bundle(
        &self,
        manifest: Option<&ExtendedUpdateManifest>,
        bundle: &[u8],
    ) -> Result<
        flexcore_edge_agent::update_session::VerifiedExtendedUpdate,
        UpdateBundleError,
    > {
        self.gate().verify_fleet_bundle(manifest, bundle)
    }

    /// Opens the append-only update/rollback register at the mock's
    /// persistent path (a fresh open per call mirrors the composed agent's
    /// per-boot open; the file is the source of truth).
    pub fn open_ledger(&self) -> FileUpdateLedger {
        FileUpdateLedger::open(&self.ledger_path).expect("mock device: the register opens")
    }

    /// The activation/rollback surface over the mock's audit + clock.
    pub fn activation(&self) -> UpdateActivation {
        let audit: Arc<dyn AuditSink> = self.audit.clone();
        let clock: Arc<dyn Clock + Send + Sync> =
            Arc::new(FixedClock::at_unix_millis(self.clock_millis.get()));
        UpdateActivation::new(audit, clock)
    }

    /// SIMULATED POWER LOSS / REBOOT: drops every in-memory handle (the
    /// gate, and with it the open counter file) and recomposes from the
    /// DISK state only — the register and the counter are reopened and
    /// replayed, which is exactly the recovery path the recorded
    /// semantics prescribe (replay derives truth; nothing is remembered
    /// in memory).
    pub fn power_cycle(&mut self) {
        self.gate = None; // drops the gate and its open counter file handle
        self.compose_gate();
    }

    /// The path of the anti-rollback counter file (the torn-tail tests
    /// append garbage bytes here to simulate a torn durable write —
    /// read-back happens through `AntiRollbackCounter::open`).
    pub fn counter_path(&self) -> &Path {
        &self.counter_path
    }

    /// The path of the update register file (the torn-tail tests corrupt
    /// the tail here).
    pub fn ledger_path(&self) -> &Path {
        &self.ledger_path
    }

    /// The scratch directory (cleanup helper for the suites).
    pub fn scratch(&self) -> &Path {
        &self.scratch
    }
}

// --- Shared attestation fixtures (P18.13's consumed policy surface) ----------------------

/// A valid software-only evidence report for the mock device (fixture
/// data; no evidence was ever collected from any device — this is test
/// code constructing the evidence SHAPE).
pub fn fixture_evidence(device_id: &str, reported_at_unix_millis: i64) ->
flexcore_edge_agent::attestation::AttestationEvidence
{
    flexcore_edge_agent::attestation::AttestationEvidence {
        device_id: device_id.to_string(),
        reported_at_unix_millis,
        anchor_mode: TrustAnchorMode::SoftwareOnlyEvidence,
        measurements: vec![
            flexcore_edge_agent::attestation::MeasurementRecord {
                component: "bootloader".to_string(),
                digest_hex: "aaaa0000bbbb1111".to_string(),
                digest_scheme: "test-fnv1a64".to_string(),
            },
            flexcore_edge_agent::attestation::MeasurementRecord {
                component: "agent".to_string(),
                digest_hex: "cccc2222dddd3333".to_string(),
                digest_scheme: "test-fnv1a64".to_string(),
            },
        ],
        boot_counter: 7,
        report_reference: "fixture-report-001".to_string(),
    }
}

/// The baseline expectations matching [`fixture_evidence`].
pub fn fixture_baseline() -> MeasurementExpectations {
    let mut expected = std::collections::BTreeMap::new();
    expected.insert("bootloader".to_string(), "aaaa0000bbbb1111".to_string());
    expected.insert("agent".to_string(), "cccc2222dddd3333".to_string());
    MeasurementExpectations {
        expected_digests: expected,
    }
}

/// The default P18.13 policy (P18-E-2 DEFAULT (a): hardware-optional).
pub fn fixture_policy() -> AttestationVerificationPolicy {
    AttestationVerificationPolicy::default_policy()
}

/// The extra-value parser the fleet-state view needs for ATTESTATION_STATE
/// rows (delegates to the owning vocabulary — the P18.12 contract).
pub fn attestation_value_parser(
    value: &str,
) -> Result<(), FleetRegistryError> {
    flexcore_edge_agent::attestation::FleetAttestationState::from_sql_value(value).map(|_| ())
}

/// The digest-verdict helper the tamper tests use to state intent
/// explicitly (reads better than a raw enum match at the call site).
pub fn verdict_is(verdict: UpdateDigestVerdict, expected: UpdateDigestVerdict) -> bool {
    verdict == expected
}

/// Removes a scratch directory best-effort (test cleanup; never a
/// production path).
pub fn cleanup(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}
