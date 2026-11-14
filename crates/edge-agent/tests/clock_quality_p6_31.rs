// P6.31 — Integration tests: edge time synchronization and clock-quality
//         monitoring (ledger P6.31 step 4; decision record
//         `25_31_32_console_clock_decommission.md` §2).
//
// EVERY TEST BELOW IS UNRUN (the loop bans in
// `ledger/phase-06-edge-protocol/ENTRY_NOTE.md`; the human build/test pass
// owns execution). Covered cases (the ledger's fault-injection set):
//
// - **Unknown before the first observation** → the P6.15 expiry check
//   FAILS CLOSED (verified through the REAL `CommandVerifier`, whose
//   `clock_quality` parameter this module produces — the recorded seam);
// - **forced clock offset beyond tolerance** → `Degraded`, the command is
//   rejected with the `expiry` reason and AUDITED (the verifier's rejection
//   audit event), and the audited hold-on-drift gate refuses with its
//   named reason;
// - **sync-source outage** → `Unsynchronized` (explicit note and the
//   quiet-source timeout path) → same fail-closed behaviour + audited
//   transition;
// - **recovery** → a fresh in-tolerance pair restores `Synchronized` and
//   the SAME command shape verifies again;
// - **the tolerance boundary is respected** (exactly at tolerance is
//   inside it; one millisecond beyond is Degraded);
// - the P6.17 health seam renders `DEGRADED_UNVERIFIED` /
//   `DEGRADED_CLOCK_DRIFT` / `HEALTHY` (closed vocabulary, rendered
//   verbatim).

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use flexcore_ports::fakes::FixedClock;

use flexcore_edge_agent::clock_quality::{
    audited_expiry_check, ClockQualityMonitor, ClockQualityState, SyncSourceConfig,
    TimeSamplePair, TimeSyncProfile, DEFAULT_PILOT_SYNC_TIMEOUT_MILLIS,
    DEFAULT_PILOT_TOLERANCE_MILLIS,
};
use flexcore_edge_agent::command_path::{CommandReceipt, EdgeCommand};
use flexcore_edge_agent::command_verify::{
    sign_command, AssetCommandScope, CommandAuthorization, CommandSigningKey,
    CommandVerificationRejection, CommandVerifier, FileSequenceCheckpoint, IncomingCommand,
};
use flexcore_edge_agent::health::{AdapterHealthMonitor, AdapterObservation, HealthLimits};
use flexcore_edge_agent::ports::InMemoryAuditSink;

const BASE_MILLIS: i64 = 1_700_000_000_000;
const SITE: &str = "pilot-site-A"; // PLACEHOLDER site naming (D5 input; ENTRY_NOTE boundary 5)

// --- Fixtures ---------------------------------------------------------------------

/// A std-only unique scratch directory per call (the P6.15 test pattern).
fn scratch_dir(label: &str) -> PathBuf {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "flexcore-edge-p631-it-{}-{}-{}",
        std::process::id(),
        label,
        COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).expect("scratch dir creates");
    dir
}

fn signing_key() -> CommandSigningKey {
    CommandSigningKey::new(
        "lab-command-signing-key-material".to_string(),
        "deployment:lab-command-key-1".to_string(),
    )
    .expect("valid key")
}

fn authorization() -> CommandAuthorization {
    CommandAuthorization {
        site_id: SITE.to_string(),
        valid_until_unix_millis: BASE_MILLIS + 86_400_000,
        adapter_allowlist: vec!["simulated-test".to_string()],
        asset_scopes: vec![AssetCommandScope {
            asset_key: "ess-01".to_string(),
            allowed_quantities: vec!["active_power".to_string()],
            min_value: -5.0,
            max_value: 5.0,
        }],
    }
}

/// A fresh verifier over its own scratch checkpoint, auditing into the
/// SHARED `audit` allocation (one sink, two handles: the verifier's
/// `Arc<dyn AuditSink>` and the test's concrete handle).
fn verifier(scratch: &Path, audit: Arc<InMemoryAuditSink>) -> CommandVerifier {
    let checkpoint =
        FileSequenceCheckpoint::open(&scratch.join("seq.ckpt")).expect("checkpoint opens");
    CommandVerifier::new(
        signing_key(),
        authorization(),
        Box::new(checkpoint),
        audit,
        Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 1_000)),
    )
    .expect("verifier composes")
}

fn incoming(sequence: u64) -> IncomingCommand {
    let edge = EdgeCommand::new(
        "cmd-1".to_string(),
        "ess-01".to_string(),
        "ISSUED".to_string(),
        sequence,
    )
    .expect("valid command");
    IncomingCommand::new(
        edge,
        "simulated-test".to_string(),
        String::new(),
        "edge-session-1".to_string(),
        "active_power".to_string(),
        1.5,
        BASE_MILLIS,
        BASE_MILLIS + 3_600_000,
    )
    .expect("valid incoming")
}

/// The incoming command WITH its payload-bound signature (signing an
/// unsigned-shaped command and attaching the reference — the canonical
/// binding does not include the signature field).
fn signed_incoming(sequence: u64) -> IncomingCommand {
    let unsigned = incoming(sequence);
    let signature = sign_command(&signing_key(), &unsigned);
    let mut command = incoming(sequence);
    command.signature_reference = signature;
    command
}

fn receipt() -> CommandReceipt {
    CommandReceipt {
        command_id: "cmd-1".to_string(),
        target_adapter: "simulated-test".to_string(),
        admitted_at_unix_millis: BASE_MILLIS,
        approver: "J. Doe (security lead)".to_string(),
        enablement_valid_until_unix_millis: BASE_MILLIS + 3_600_000,
        scope_note: "gate admission only (test)".to_string(),
    }
}

fn monitor_with(audit: Arc<InMemoryAuditSink>, now: i64) -> ClockQualityMonitor {
    let config =
        SyncSourceConfig::for_profile(TimeSyncProfile::Pilot, "site-time-service-1".to_string());
    ClockQualityMonitor::new(config, Arc::new(FixedClock::at_unix_millis(now)), audit)
        .expect("valid config")
}

/// A time-sample pair whose offset is `offset_millis` (edge ahead of the
/// reference) observed at `at`.
fn pair(offset_millis: i64, at: i64) -> TimeSamplePair {
    TimeSamplePair {
        reference_unix_millis: at - offset_millis,
        edge_unix_millis: at,
        observed_at_unix_millis: at,
    }
}

fn health_limits() -> HealthLimits {
    HealthLimits {
        freshness_limit_millis: 30_000,
        unresponsive_after_millis: 60_000,
        error_rate_window_millis: 300_000,
        max_errors_in_window: 3,
        credential_rotation_window_millis: 7 * 86_400_000,
    }
}

// --- Unknown before the first observation: fail-closed through the verifier -------

#[test]
fn unknown_before_first_observation_fails_the_p6_15_expiry_check_closed() {
    let scratch = scratch_dir("unknown");
    let audit = Arc::new(InMemoryAuditSink::new());
    let monitor = monitor_with(Arc::new(InMemoryAuditSink::new()), BASE_MILLIS);
    assert_eq!(monitor.quality_at(BASE_MILLIS), ClockQualityState::Unknown);

    let mut verifier = verifier(&scratch, audit.clone());
    // The seam's parameter is the contracts ClockQuality — exactly what
    // `CommandVerifier::verify` declares (conformance with the P6.15 seam).
    let quality = monitor.p6_15_clock_quality(BASE_MILLIS);
    assert!(matches!(
        quality,
        flexcore_contracts::time_model::ClockQuality::Unknown
    ));

    let command = signed_incoming(1);
    let rejection = verifier
        .verify(&receipt(), &command, quality)
        .expect_err("UNKNOWN clock quality must FAIL the expiry check CLOSED");
    assert!(
        matches!(rejection, CommandVerificationRejection::Expiry { .. }),
        "the rejection reason is `expiry`: {rejection:?}"
    );
    // The rejection is audited by the verifier (the P4-pattern record).
    assert!(audit
        .events()
        .iter()
        .any(|e| e.kind.name() == "EDGE_COMMAND_VERIFICATION_REJECTED"
            && e.detail.contains("Unknown")));
}

// --- Forced offset beyond tolerance: Degraded + rejected + audited -----------------

#[test]
fn forced_offset_beyond_tolerance_degrades_and_holds_commands() {
    let scratch = scratch_dir("degraded");
    let monitor_audit = Arc::new(InMemoryAuditSink::new());
    let monitor = monitor_with(monitor_audit.clone(), BASE_MILLIS);
    // Fault injection: a forced 900 ms offset (pilot tolerance 100 ms).
    let state = monitor.observe_pair(pair(900, BASE_MILLIS)).expect("pair");
    assert_eq!(state, ClockQualityState::Degraded);
    // The transition is AUDITED (the dedicated event name leads the detail).
    assert!(monitor_audit
        .events()
        .iter()
        .any(|e| e.detail.starts_with("EDGE_CLOCK_QUALITY_CHANGED")
            && e.detail.contains("DEGRADED")));

    // The audited hold-on-drift gate refuses with its named reason.
    let hold_audit = Arc::new(InMemoryAuditSink::new());
    let error = audited_expiry_check(&monitor, &hold_audit, BASE_MILLIS)
        .expect_err("a Degraded clock holds commands");
    assert!(error.contains("DEGRADED"));
    assert!(error.contains("NOT trusted"));
    assert!(hold_audit
        .events()
        .iter()
        .any(|e| e.detail.starts_with("EDGE_CLOCK_HOLD_ON_DRIFT")));

    // Through the REAL P6.15 verifier: rejected with the `expiry` reason
    // and audited.
    let verify_audit = Arc::new(InMemoryAuditSink::new());
    let mut verifier = verifier(&scratch, verify_audit.clone());
    let rejection = verifier
        .verify(&receipt(), &signed_incoming(1), monitor.p6_15_clock_quality(BASE_MILLIS))
        .expect_err("a Degraded clock must fail the expiry check CLOSED");
    assert!(matches!(rejection, CommandVerificationRejection::Expiry { .. }));
    assert!(verify_audit
        .events()
        .iter()
        .any(|e| e.kind.name() == "EDGE_COMMAND_VERIFICATION_REJECTED"
            && e.detail.contains("Unsynchronized")));
}

// --- Sync-source outage: Unsynchronized → same fail-closed behaviour ---------------

#[test]
fn sync_source_outage_is_unsynchronized_and_holds_commands() {
    let scratch = scratch_dir("outage");
    let monitor_audit = Arc::new(InMemoryAuditSink::new());
    let monitor = monitor_with(monitor_audit.clone(), BASE_MILLIS);
    monitor.observe_pair(pair(5, BASE_MILLIS)).expect("pair");
    // Explicit outage note (the deployment wiring reports the allowlisted
    // source unreachable):
    let state = monitor.note_source_unreachable(BASE_MILLIS + 1_000);
    assert_eq!(state, ClockQualityState::Unsynchronized);
    // The transition is audited.
    assert!(monitor_audit
        .events()
        .iter()
        .any(|e| e.detail.contains("UNSYNCHRONIZED")));

    let mut verifier = verifier(&scratch, Arc::new(InMemoryAuditSink::new()));
    assert!(verifier
        .verify(&receipt(), &signed_incoming(1), monitor.p6_15_clock_quality(BASE_MILLIS + 2_000))
        .is_err());

    // The QUIET-source path: no explicit note, but no fresh observation
    // inside the sync timeout also ages the state into Unsynchronized.
    let quiet = monitor_with(Arc::new(InMemoryAuditSink::new()), BASE_MILLIS);
    quiet.observe_pair(pair(5, BASE_MILLIS)).expect("pair");
    assert_eq!(
        quiet.quality_at(BASE_MILLIS + DEFAULT_PILOT_SYNC_TIMEOUT_MILLIS + 1),
        ClockQualityState::Unsynchronized
    );
    assert!(quiet
        .expiry_check_allowed(BASE_MILLIS + DEFAULT_PILOT_SYNC_TIMEOUT_MILLIS + 1)
        .is_err());
}

// --- Recovery: back to Synchronized, commands verify again -------------------------

#[test]
fn recovery_restores_synchronized_and_command_trust() {
    let scratch = scratch_dir("recovery");
    let monitor = monitor_with(Arc::new(InMemoryAuditSink::new()), BASE_MILLIS);
    monitor.observe_pair(pair(900, BASE_MILLIS)).expect("drift");
    assert_eq!(monitor.quality_at(BASE_MILLIS), ClockQualityState::Degraded);
    // Recovery: a fresh in-tolerance pair (the offset was corrected).
    let state = monitor
        .observe_pair(pair(5, BASE_MILLIS + 5_000))
        .expect("pair");
    assert_eq!(state, ClockQualityState::Synchronized);
    assert!(monitor.expiry_check_allowed(BASE_MILLIS + 5_000).is_ok());

    let mut verifier = verifier(&scratch, Arc::new(InMemoryAuditSink::new()));
    let verified = verifier
        .verify(&receipt(), &signed_incoming(1), monitor.p6_15_clock_quality(BASE_MILLIS + 5_000))
        .expect("a recovered clock verifies the command (test env only)");
    assert_eq!(verified.command_id, "cmd-1");
}

// --- The tolerance boundary is respected -------------------------------------------

#[test]
fn the_tolerance_boundary_is_respected_exactly() {
    let monitor = monitor_with(Arc::new(InMemoryAuditSink::new()), BASE_MILLIS);
    // Exactly AT the pilot tolerance (100 ms) is INSIDE it:
    let at_tolerance = monitor
        .observe_pair(pair(DEFAULT_PILOT_TOLERANCE_MILLIS, BASE_MILLIS))
        .expect("pair");
    assert_eq!(at_tolerance, ClockQualityState::Synchronized);
    // One millisecond beyond it is Degraded:
    let beyond = monitor
        .observe_pair(pair(DEFAULT_PILOT_TOLERANCE_MILLIS + 1, BASE_MILLIS + 1_000))
        .expect("pair");
    assert_eq!(beyond, ClockQualityState::Degraded);
    // The bounded window's median is now beyond tolerance: hold.
    assert!(monitor.expiry_check_allowed(BASE_MILLIS + 1_000).is_err());
}

// --- The health seam (P6.17, additive) ---------------------------------------------

#[test]
fn clock_quality_surfaces_through_the_closed_health_vocabulary() {
    let monitor = monitor_with(Arc::new(InMemoryAuditSink::new()), BASE_MILLIS);
    // Unknown → DEGRADED_UNVERIFIED (never healthy-by-default).
    assert_eq!(
        monitor.health_record(BASE_MILLIS).status_name(),
        "DEGRADED_UNVERIFIED"
    );
    // Degraded → DEGRADED_CLOCK_DRIFT.
    monitor.observe_pair(pair(900, BASE_MILLIS)).expect("pair");
    let record = monitor.health_record(BASE_MILLIS);
    assert_eq!(record.status_name(), "DEGRADED_CLOCK_DRIFT");
    assert!(record.detail.contains("BEYOND"));
    // Synchronized → HEALTHY (reached only through the positive check).
    monitor.observe_pair(pair(5, BASE_MILLIS + 1_000)).expect("pair");
    assert_eq!(
        monitor.health_record(BASE_MILLIS + 1_000).status_name(),
        "HEALTHY"
    );

    // The clock health record composes alongside adapter health in the
    // same closed vocabulary (P6.17 monitor untouched; the record shape is
    // the shared one).
    let mut adapters = AdapterHealthMonitor::new(health_limits()).expect("limits");
    adapters.observe(
        "simulated-test",
        BASE_MILLIS,
        AdapterObservation::Responsive { event_age_millis: 10 },
    );
    let adapter_record = adapters
        .assess("simulated-test", BASE_MILLIS + 100)
        .expect("assessed");
    assert_eq!(adapter_record.status_name(), "HEALTHY");
    assert_eq!(
        monitor.health_record(BASE_MILLIS + 100).status_name(),
        "HEALTHY"
    );
}

// --- Configuration honesty ----------------------------------------------------------

#[test]
fn the_sync_source_is_a_named_allowlisted_reference_per_profile() {
    // The DEFAULT tolerances/timeout are recorded per profile (decision
    // record §2); the source reference is mandatory and non-empty.
    let lab = SyncSourceConfig::for_profile(TimeSyncProfile::Lab, "lab-time-service-1".to_string());
    assert_eq!(lab.profile.as_str(), "lab");
    assert!(!lab.source_reference.trim().is_empty());
    assert!(lab.tolerance_millis > 0);
    assert!(lab.sync_timeout_millis > 0);
    let pilot =
        SyncSourceConfig::for_profile(TimeSyncProfile::Pilot, "site-time-service-1".to_string());
    assert!(pilot.validate().is_ok());
    assert!(lab.validate().is_ok());
    // An unnamed source refuses at construction (never an assumption that
    // the host clock is correct).
    let unnamed = SyncSourceConfig::for_profile(TimeSyncProfile::Pilot, "  ".to_string());
    assert!(unnamed.validate().is_err());
}
