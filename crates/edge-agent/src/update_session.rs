// P18.04 + P18.05 — The device update FLOW state machine and the extended
//                 update gate behind the P9.34 seam (decision records
//                 `docs/phases-11-20/phase-18-edge-fleet-lifecycle-ota/
//                 records/04_p18_04_device_update_flow.md` and
//                 `records/05_p18_05_edge_agent_update_extension.md`;
//                 ADR-1801 in records/01).
//
// P18.04 (the flow): the closed update-progress state machine
// ([`UpdateProgressState`]) models download → verify → install →
// health-check → active, with EXPLICIT failure states and auto-rollback
// edges (every illegal transition is a named refusal; terminal states are
// terminal). Status notifications ([`UpdateStatusNotification`]) carry the
// closed state + failure-state vocabulary. A/B slots follow
// [`crate::update_verification::UPDATE_SLOT_DECISION`] verbatim: the
// register is the source of truth and the two-slot layout is the
// RECOMMENDED deployment shape — the device-side slot mechanics
// (watchdog/boot counters, slot swap) are DEVICE concerns the platform
// SPECIFIES (interfaces, evidence contracts, thresholds) but does not
// implement (plan §4 non-goals).
//
// P18.05 (the seam): [`Sha256UpdateVerifier`] is the real SHA-256 digest
// verifier behind the P9.34 `UpdateVerifier` port — the crypto is
// CONSUMED from the P11.15 std-only SHA-256 implementation
// (`flexcore-audit/src/chain.rs`, cited, never duplicated). It is an
// OPTION, never a default: with nothing configured the gate still
// composes `NoUpdateVerifier` and refuses EVERYTHING — the fail-closed
// P9.34 semantics are preserved byte-for-byte (context constraint 1).
// [`ExtendedUpdateGate`] composes the P18.02 v2 manifest checks
// (applicability, expiry, anti-rollback sequence) AROUND the untouched
// P9.34 gate (`UpdateGate::verify_before_staging` — its refusal order,
// audit, and no-write structure are preserved verbatim as the inner step)
// and verifies per-payload digests. Wiring a production verifier under
// reviewed key management remains a DEPLOYMENT REQUIREMENT (conventions
// §6.6; P10.21 §5).
//
// THE BLOCKING RULE (the phase-plan P18.15 row, consumed by citation):
// updates are BLOCKED while any command is ISSUED/TRACKING on the device
// — the state machine's first transition takes the device's in-flight
// command count and refuses with [`UpdateSessionError::CommandsInFlight`]
// when it is non-zero. P18.15 (wave 2) formalizes the interlock; this
// parameter is the seam it lands on.
//
// HONESTY BOUNDS (carried verbatim from the P9.34 module): NO update has
// ever been built, signed, staged, or applied; no fleet exists; every
// hardware determination (watchdog/boot-counter behavior, A/B slot
// layout) is PENDING PILOT HARDWARE SPEC (the P6.34 posture); no
// certification or security claim is made. The SHA-256 implementation is
// WRITTEN-UNRUN and anchored to published test vectors in
// `flexcore-audit/src/chain.rs` — verified only at the first human
// `cargo test`.

use std::sync::Arc;

use flexcore_audit::chain::{hex_encode_digest, sha256};
use flexcore_ports::clock::Clock;

use crate::ports::{AuditSink, EdgeAuditEvent, EdgeAuditKind};
use crate::update_bundle::{
    AntiRollbackCounter, ExtendedUpdateManifest, UpdateBundleError,
    MAX_UPDATE_PAYLOAD_TOTAL_BYTES,
};
use crate::update_verification::{UpdateDigestVerdict, UpdateGate, UpdateVerifier, VerifiedUpdate};

// --- Honesty statements -------------------------------------------------------------

/// The P18.05 statement (pinned so tests and reviewers can read what IS
/// and IS NOT claimed by the extended gate and the real verifier).
pub const UPDATE_SESSION_STATEMENT: &str = "the P18.05 extension adds a real SHA-256 digest \
     verifier (the P11.15 std-only implementation, consumed not duplicated) and the v2 \
     manifest checks AROUND the untouched P9.34 gate; with nothing configured the default \
     remains NoUpdateVerifier and EVERY update is refused (fail-closed preserved). No update \
     has ever been built, signed, staged, or applied; no fleet exists; the sequence \
     semantics are UNVERIFIED against the TUF/SUIT spec texts; wiring the production \
     verifier under reviewed key management is a DEPLOYMENT REQUIREMENT. No certification \
     or security claim is made.";

/// The A/B-slot + hardware statement (the P6.34 posture carried into the
/// flow).
pub const UPDATE_FLOW_HARDWARE_STATEMENT: &str = "A/B slot mechanics, watchdogs, and boot \
     counters are DEVICE concerns the platform specifies (interfaces, evidence contracts, \
     thresholds) but cannot implement; every hardware determination is PENDING PILOT \
     HARDWARE SPEC (the P6.34 posture). The update/rollback REGISTER (P9.34) is the source \
     of truth for the active version; the two-slot A/B layout is the RECOMMENDED deployment \
     shape. No hardware behavior is claimed.";

// --- The SHA-256 verifier (P18.05) -----------------------------------------------------

/// The production SHA-256 update digest verifier behind the P9.34
/// `UpdateVerifier` port. The digest is CONSUMED from the P11.15
/// std-only SHA-256 (`flexcore_audit::chain::sha256`) — the workspace's
/// existing crypto approach, never duplicated (program conventions §6.6;
/// the loop bans forbid new crypto-crate additions). The implementation
/// is anchored to the FIPS 180-4 published vectors exercised in
/// `flexcore-audit`'s test module and verified only at the first human
/// `cargo test` (WRITTEN-UNRUN).
///
/// DEPLOYMENT REQUIREMENT (unchanged from P9.34): composing THIS verifier
/// is necessary but not sufficient — reviewed key management for the
/// trusted signer references, and the human key ceremonies (P10.21), are
/// the operator's gates. This struct is not composed by default anywhere:
/// the shipped default remains `NoUpdateVerifier` (fail-closed).
#[derive(Debug, Clone, Copy, Default)]
pub struct Sha256UpdateVerifier;

impl UpdateVerifier for Sha256UpdateVerifier {
    fn algorithm_name(&self) -> &'static str {
        // The P9.34 closed vocabulary's production name — the same string
        // the closed `UPDATE_DIGEST_ALGORITHMS` set carries, so an
        // algorithm match is possible at all.
        "sha-256"
    }

    fn verify(&self, bundle: &[u8], declared_digest_hex: &str) -> UpdateDigestVerdict {
        let digest = sha256(bundle);
        let computed = hex_encode_digest(&digest);
        if computed == declared_digest_hex {
            UpdateDigestVerdict::Matches
        } else {
            UpdateDigestVerdict::Mismatch
        }
    }
}

// --- The update-progress state machine (P18.04) -----------------------------------------

/// The closed update-progress vocabulary (P18.04). Terminal states are
/// [`UpdateProgressState::Active`], [`UpdateProgressState::RolledBack`],
/// and [`UpdateProgressState::UpdateFailed`] — no transition leaves them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateProgressState {
    /// No update session is in progress on this device.
    Idle,
    /// The bundle is being downloaded (no verification has happened; no
    /// manifest has been trusted).
    Downloading,
    /// The bundle is being verified (the P9.34 gate + the extended v2
    /// checks). NOTHING is written during this state (the P9.34
    /// verify-before-any-write ordering).
    Verifying,
    /// Verification PASSED; the install is awaiting the device's install
    /// path (A/B slot write per `UPDATE_SLOT_DECISION`).
    VerifiedPendingInstall,
    /// The verified bundle is being installed into the inactive slot.
    Installing,
    /// Installed; the device is performing the post-install health check
    /// (watchdog/boot-counter evidence — a device-side contract).
    HealthCheckPending,
    /// The health check confirmed the new version — the session ends
    /// successfully (terminal).
    Active,
    /// The health check FAILED; the device is auto-rolling back to the
    /// previously-activated version (the P9.34 rollback rules: a
    /// re-activation of a previously-activated version through the
    /// append-only register, never a silent downgrade).
    RollingBack,
    /// The rollback completed — the session ends with the previous
    /// version re-activated (terminal).
    RolledBack,
    /// The session FAILED in a named, explicit failure state (terminal;
    /// the failure-state name rides the status notification).
    UpdateFailed,
}

impl UpdateProgressState {
    /// The stable machine-readable state name (status notifications,
    /// audit lines, the fleet evidence records).
    pub fn name(self) -> &'static str {
        match self {
            UpdateProgressState::Idle => "UPDATE_IDLE",
            UpdateProgressState::Downloading => "UPDATE_DOWNLOADING",
            UpdateProgressState::Verifying => "UPDATE_VERIFYING",
            UpdateProgressState::VerifiedPendingInstall => "UPDATE_VERIFIED_PENDING_INSTALL",
            UpdateProgressState::Installing => "UPDATE_INSTALLING",
            UpdateProgressState::HealthCheckPending => "UPDATE_HEALTH_CHECK_PENDING",
            UpdateProgressState::Active => "UPDATE_ACTIVE",
            UpdateProgressState::RollingBack => "UPDATE_ROLLING_BACK",
            UpdateProgressState::RolledBack => "UPDATE_ROLLED_BACK",
            UpdateProgressState::UpdateFailed => "UPDATE_FAILED",
        }
    }

    /// Whether the session is over (no further transitions exist).
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            UpdateProgressState::Active
                | UpdateProgressState::RolledBack
                | UpdateProgressState::UpdateFailed
        )
    }
}

/// The closed FAILURE-state vocabulary (P18.04: "explicit failure
/// states"). These are FlexCore names. The mapping to the OCPP 2.0.1
/// firmware-update status names ([S-121][S-122]) is UNVERIFIED until
/// enumerated from the OCA spec text at implementation time (the phase
/// plan's P18.04 row; record 04 §UNVERIFIED register) — no OCPP status
/// name is claimed anywhere here.
pub const UPDATE_FAILURE_DOWNLOAD_FAILED: &str = "UPDATE_FAILURE_DOWNLOAD_FAILED";
/// Verification refused: the P9.34 gate or the extended v2 checks
/// refused (the stable refusal code rides the audit line and the
/// evidence record, never this constant alone).
pub const UPDATE_FAILURE_VERIFICATION_REFUSED: &str = "UPDATE_FAILURE_VERIFICATION_REFUSED";
/// The install into the inactive slot failed (device-side contract).
pub const UPDATE_FAILURE_INSTALL_FAILED: &str = "UPDATE_FAILURE_INSTALL_FAILED";
/// The post-install health check failed (watchdog/boot-counter evidence
/// said the new version is unhealthy) — auto-rollback begins.
pub const UPDATE_FAILURE_HEALTH_CHECK_FAILED: &str = "UPDATE_FAILURE_HEALTH_CHECK_FAILED";
/// The rollback itself failed — the device needs operator intervention
/// (recorded, never silent; the register shows the last good state).
pub const UPDATE_FAILURE_ROLLBACK_FAILED: &str = "UPDATE_FAILURE_ROLLBACK_FAILED";
/// The session was cancelled before install (download or verified-pending
/// states only — a cancel never aborts an in-progress install).
pub const UPDATE_FAILURE_CANCELLED: &str = "UPDATE_FAILURE_CANCELLED";

/// The named errors of the update-progress state machine. Every refusal
/// is distinct; nothing is a silent pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateSessionError {
    /// The transition is illegal from the current state (terminal states
    /// refuse everything; the flow order is the recorded flow).
    IllegalTransition { from: &'static str, to: &'static str },
    /// The update is BLOCKED because commands are ISSUED/TRACKING on the
    /// device (the phase-plan P18.15 rule, consumed by citation; the
    /// drain-before-update discipline precedes any download).
    CommandsInFlight { issued_or_tracking: usize },
    /// An input was unusable (empty attempt id, non-positive instant, …).
    InvalidInput { reason: String },
}

impl UpdateSessionError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            UpdateSessionError::IllegalTransition { .. } => "UPDATE_TRANSITION_ILLEGAL",
            UpdateSessionError::CommandsInFlight { .. } => "UPDATE_BLOCKED_COMMANDS_IN_FLIGHT",
            UpdateSessionError::InvalidInput { .. } => "UPDATE_SESSION_INPUT_INVALID",
        }
    }
}

impl std::fmt::Display for UpdateSessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UpdateSessionError::IllegalTransition { from, to } => write!(
                f,
                "the transition {from} -> {to} is illegal in the recorded update flow — \
                 refused, never guessed at"
            ),
            UpdateSessionError::CommandsInFlight { issued_or_tracking } => write!(
                f,
                "the update is BLOCKED: {issued_or_tracking} command(s) are ISSUED/TRACKING \
                 on this device — drain to a terminal state first (the report §5.6 \
                 lifecycle drain discipline; P18.15 interlock)"
            ),
            UpdateSessionError::InvalidInput { reason } => {
                write!(f, "the update session input is invalid: {reason}")
            }
        }
    }
}

impl std::error::Error for UpdateSessionError {}

/// The update-progress state machine over ONE device update attempt
/// (P18.04). Pure bookkeeping: it holds NO bundle bytes, NO manifest, NO
/// paths, and NO filesystem access — the durable evidence is the P9.34
/// append-only register (the source of truth) and the P18.07 audit
/// records; this type drives the flow and the status notifications.
/// Timestamps come from the INJECTED clock at every transition (the
/// caller supplies them; no wall time is read anywhere).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateSessionStateMachine {
    state: UpdateProgressState,
    attempt_id: String,
    device_id: String,
    version: String,
    anti_rollback_sequence: u64,
    failure_state: Option<&'static str>,
    last_transition_unix_millis: i64,
}

impl UpdateSessionStateMachine {
    /// Opens a session in [`UpdateProgressState::Idle`].
    pub fn new(
        attempt_id: &str,
        device_id: &str,
        version: &str,
        anti_rollback_sequence: u64,
        at_unix_millis: i64,
    ) -> Result<Self, UpdateSessionError> {
        if attempt_id.trim().is_empty() || device_id.trim().is_empty() || version.trim().is_empty()
        {
            return Err(UpdateSessionError::InvalidInput {
                reason: "attempt_id, device_id, and version are mandatory".to_string(),
            });
        }
        if at_unix_millis <= 0 {
            return Err(UpdateSessionError::InvalidInput {
                reason: "the session instant must be a positive unix-millis value".to_string(),
            });
        }
        Ok(Self {
            state: UpdateProgressState::Idle,
            attempt_id: attempt_id.to_string(),
            device_id: device_id.to_string(),
            version: version.to_string(),
            anti_rollback_sequence,
            failure_state: None,
            last_transition_unix_millis: at_unix_millis,
        })
    }

    /// The current state.
    pub fn state(&self) -> UpdateProgressState {
        self.state
    }

    /// The attempt id (diagnostics; the key the P18.07 evidence chain
    /// commits to).
    pub fn attempt_id(&self) -> &str {
        &self.attempt_id
    }

    /// The device id (diagnostics).
    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    /// The target version (diagnostics).
    pub fn version(&self) -> &str {
        &self.version
    }

    /// The manifest's anti-rollback sequence (diagnostics).
    pub fn anti_rollback_sequence(&self) -> u64 {
        self.anti_rollback_sequence
    }

    /// The explicit failure state, when the session failed
    /// ([`UpdateProgressState::UpdateFailed`] carries one).
    pub fn failure_state(&self) -> Option<&'static str> {
        self.failure_state
    }

    /// The last transition instant (diagnostics).
    pub fn last_transition_unix_millis(&self) -> i64 {
        self.last_transition_unix_millis
    }

    /// The status notification for the CURRENT state (P18.04 status
    /// notifications; built on demand — nothing is emitted internally).
    pub fn status_notification(&self) -> UpdateStatusNotification {
        UpdateStatusNotification {
            device_id: self.device_id.clone(),
            attempt_id: self.attempt_id.clone(),
            state: self.state,
            failure_state: self.failure_state,
            version: self.version.clone(),
            at_unix_millis: self.last_transition_unix_millis,
        }
    }

    fn enter(
        &mut self,
        expected: UpdateProgressState,
        next: UpdateProgressState,
        at_unix_millis: i64,
    ) -> Result<UpdateProgressState, UpdateSessionError> {
        if at_unix_millis <= 0 {
            return Err(UpdateSessionError::InvalidInput {
                reason: "the transition instant must be a positive unix-millis value"
                    .to_string(),
            });
        }
        if self.state != expected {
            return Err(UpdateSessionError::IllegalTransition {
                from: self.state.name(),
                to: next.name(),
            });
        }
        self.state = next;
        self.last_transition_unix_millis = at_unix_millis;
        Ok(next)
    }

    /// `Idle → Downloading`. THE BLOCKING RULE: `issued_or_tracking` is
    /// the device's count of commands in the ISSUED/TRACKING command
    /// states — a non-zero count refuses the session start (the
    /// phase-plan P18.15 rule, consumed by citation; P18.15 formalizes
    /// the interlock in wave 2).
    pub fn begin_download(
        &mut self,
        issued_or_tracking_commands: usize,
        at_unix_millis: i64,
    ) -> Result<UpdateProgressState, UpdateSessionError> {
        if issued_or_tracking_commands > 0 {
            return Err(UpdateSessionError::CommandsInFlight {
                issued_or_tracking: issued_or_tracking_commands,
            });
        }
        self.enter(UpdateProgressState::Idle, UpdateProgressState::Downloading, at_unix_millis)
    }

    /// `Downloading → Verifying` (the bundle bytes are complete; nothing
    /// has been trusted yet).
    pub fn download_complete(&mut self, at_unix_millis: i64)
    -> Result<UpdateProgressState, UpdateSessionError> {
        self.enter(UpdateProgressState::Downloading, UpdateProgressState::Verifying, at_unix_millis)
    }

    /// `Downloading → UpdateFailed` with the explicit download failure
    /// state.
    pub fn download_failed(
        &mut self,
        at_unix_millis: i64,
    ) -> Result<UpdateProgressState, UpdateSessionError> {
        let next = self.enter(
            UpdateProgressState::Downloading,
            UpdateProgressState::UpdateFailed,
            at_unix_millis,
        )?;
        self.failure_state = Some(UPDATE_FAILURE_DOWNLOAD_FAILED);
        Ok(next)
    }

    /// `Verifying → VerifiedPendingInstall` — the full verification pass
    /// succeeded (P9.34 gate + the extended v2 checks; the anti-rollback
    /// counter has advanced durably).
    pub fn verify_passed(&mut self, at_unix_millis: i64)
    -> Result<UpdateProgressState, UpdateSessionError> {
        self.enter(
            UpdateProgressState::Verifying,
            UpdateProgressState::VerifiedPendingInstall,
            at_unix_millis,
        )
    }

    /// `Verifying → UpdateFailed` with the verification-refused failure
    /// state (the stable refusal code travels in the evidence record and
    /// the audit line, not in this vocabulary — see P18.07).
    pub fn verify_refused(
        &mut self,
        at_unix_millis: i64,
    ) -> Result<UpdateProgressState, UpdateSessionError> {
        let next = self.enter(
            UpdateProgressState::Verifying,
            UpdateProgressState::UpdateFailed,
            at_unix_millis,
        )?;
        self.failure_state = Some(UPDATE_FAILURE_VERIFICATION_REFUSED);
        Ok(next)
    }

    /// `VerifiedPendingInstall → Installing` (the verified bundle is
    /// written into the INACTIVE slot per `UPDATE_SLOT_DECISION`).
    pub fn begin_install(&mut self, at_unix_millis: i64)
    -> Result<UpdateProgressState, UpdateSessionError> {
        self.enter(
            UpdateProgressState::VerifiedPendingInstall,
            UpdateProgressState::Installing,
            at_unix_millis,
        )
    }

    /// `Installing → HealthCheckPending` (the slot write completed; the
    /// device reboots into the new slot and runs the health check).
    pub fn install_complete(&mut self, at_unix_millis: i64)
    -> Result<UpdateProgressState, UpdateSessionError> {
        self.enter(
            UpdateProgressState::Installing,
            UpdateProgressState::HealthCheckPending,
            at_unix_millis,
        )
    }

    /// `Installing → UpdateFailed` with the install failure state.
    pub fn install_failed(
        &mut self,
        at_unix_millis: i64,
    ) -> Result<UpdateProgressState, UpdateSessionError> {
        let next = self.enter(
            UpdateProgressState::Installing,
            UpdateProgressState::UpdateFailed,
            at_unix_millis,
        )?;
        self.failure_state = Some(UPDATE_FAILURE_INSTALL_FAILED);
        Ok(next)
    }

    /// `HealthCheckPending → Active` — the health evidence confirmed the
    /// new version; the P9.34 register records the activation and the
    /// session ends (terminal).
    pub fn health_confirmed(&mut self, at_unix_millis: i64)
    -> Result<UpdateProgressState, UpdateSessionError> {
        self.enter(
            UpdateProgressState::HealthCheckPending,
            UpdateProgressState::Active,
            at_unix_millis,
        )
    }

    /// `HealthCheckPending → RollingBack` — the health evidence says the
    /// new version is unhealthy; the AUTO-ROLLBACK begins (a P9.34
    /// rollback: re-activation of a previously-activated version, never a
    /// silent downgrade).
    pub fn health_failed(&mut self, at_unix_millis: i64)
    -> Result<UpdateProgressState, UpdateSessionError> {
        self.enter(
            UpdateProgressState::HealthCheckPending,
            UpdateProgressState::RollingBack,
            at_unix_millis,
        )
    }

    /// `RollingBack → RolledBack` — the previous version is re-activated;
    /// the session ends (terminal).
    pub fn rollback_complete(&mut self, at_unix_millis: i64)
    -> Result<UpdateProgressState, UpdateSessionError> {
        self.enter(
            UpdateProgressState::RollingBack,
            UpdateProgressState::RolledBack,
            at_unix_millis,
        )
    }

    /// `RollingBack → UpdateFailed` with the rollback-failed failure
    /// state — the device needs operator intervention (recorded, never
    /// silent).
    pub fn rollback_failed(
        &mut self,
        at_unix_millis: i64,
    ) -> Result<UpdateProgressState, UpdateSessionError> {
        let next = self.enter(
            UpdateProgressState::RollingBack,
            UpdateProgressState::UpdateFailed,
            at_unix_millis,
        )?;
        self.failure_state = Some(UPDATE_FAILURE_ROLLBACK_FAILED);
        Ok(next)
    }

    /// `Downloading → UpdateFailed` / `VerifiedPendingInstall →
    /// UpdateFailed` with the cancelled failure state. A cancel is NEVER
    /// legal from `Installing` or later — an in-progress install cannot
    /// be cancelled, only failed or rolled back.
    pub fn cancel(&mut self, at_unix_millis: i64)
    -> Result<UpdateProgressState, UpdateSessionError> {
        let next = match self.state {
            UpdateProgressState::Downloading | UpdateProgressState::VerifiedPendingInstall => {
                self.enter(
                    self.state,
                    UpdateProgressState::UpdateFailed,
                    at_unix_millis,
                )?
            }
            _ => {
                return Err(UpdateSessionError::IllegalTransition {
                    from: self.state.name(),
                    to: UpdateProgressState::UpdateFailed.name(),
                })
            }
        };
        self.failure_state = Some(UPDATE_FAILURE_CANCELLED);
        Ok(next)
    }
}

/// One status notification (P18.04: the "status notifications" leg of the
/// flow). A plain data type — the transport is the composition's uplink
/// path; nothing here sends anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateStatusNotification {
    pub device_id: String,
    /// The update attempt this notification belongs to (the P18.07
    /// evidence-chain key).
    pub attempt_id: String,
    pub state: UpdateProgressState,
    /// The explicit failure state, when `state == UpdateFailed`.
    pub failure_state: Option<&'static str>,
    pub version: String,
    pub at_unix_millis: i64,
}

impl UpdateStatusNotification {
    /// The human-readable summary line (no secrets; version and ids only).
    pub fn summary(&self) -> String {
        match self.failure_state {
            Some(failure) => format!(
                "device='{}' attempt='{}' state={} failure={} version={}",
                self.device_id, self.attempt_id, self.state.name(), failure, self.version
            ),
            None => format!(
                "device='{}' attempt='{}' state={} version={}",
                self.device_id, self.attempt_id, self.state.name(), self.version
            ),
        }
    }
}

// --- The extended gate (P18.05) -----------------------------------------------------------

/// Adapter that lets one `Arc`-held verifier feed BOTH the composed P9.34
/// gate (which takes a boxed verifier) and the extended gate's payload
/// checks — one verifier instance, one algorithm, no divergence.
struct SharedVerifierHandle(Arc<dyn UpdateVerifier + Send + Sync>);

impl UpdateVerifier for SharedVerifierHandle {
    fn algorithm_name(&self) -> &'static str {
        self.0.algorithm_name()
    }

    fn verify(&self, bundle: &[u8], declared_digest_hex: &str) -> UpdateDigestVerdict {
        self.0.verify(bundle, declared_digest_hex)
    }
}

/// A full extended-gate verdict: the v2 manifest, the projected P9.34
/// verdict, and the instant of the pass. The ONLY thing that may proceed
/// to the install path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedExtendedUpdate {
    pub manifest: ExtendedUpdateManifest,
    /// The P9.34 gate's own verdict (carries the base manifest and the
    /// gate-side verification instant).
    pub base: VerifiedUpdate,
    /// The extended-gate instant (== `base.verified_at_unix_millis`; kept
    /// explicit for evidence records).
    pub verified_at_unix_millis: i64,
}

/// The P18.05 extended update gate for ONE device: the P18.02 v2 manifest
/// checks (structural validation, applicability, expiry, anti-rollback
/// sequence) composed AROUND the UNTOUCHED P9.34 gate, then per-payload
/// digest verification, then the durable anti-rollback-counter advance.
///
/// REFUSAL ORDER (recorded):
/// 1. manifest present — else [`UpdateBundleError::UnsignedBundle`];
/// 2. bundle within the total-size cap — else `BundleTooLarge`;
/// 3. v2 manifest valid — else `ManifestInvalid` (the base manifest's
///    P9.34 validation runs inside);
/// 4. applicability matches THIS device (identity from the edge's own
///    configuration, never from the manifest) — else `ApplicabilityMismatch`;
/// 5. manifest not expired (injected clock) — else `Expired`;
/// 6. sequence strictly ahead of the anti-rollback counter — else
///    `SequenceNotAhead` (fail-closed on an unavailable counter);
/// 7. **the P9.34 gate, verbatim** — its six-step refusal order
///    (unsigned → manifest valid → algorithm → signer → floor → digest),
///    its audit on every verdict, and its no-write structure are
///    preserved unchanged; a refusal surfaces as
///    `BaseVerificationRefused` carrying the stable P9.34 refusal code;
/// 8. every payload digest matches (same verifier instance) — else
///    `PayloadDigestMismatch`/`PayloadVerifierRefused`/`PayloadBoundsOutside`;
/// 9. the anti-rollback counter advances DURABLY — an I/O failure here
///    refuses the verdict (fail-closed: a pass without the recorded
///    sequence is never issued).
///
/// Every extended-level refusal is audited (`EDGE_FLEET_UPDATE_REFUSED`,
/// with the stable error name); every pass is audited
/// (`EDGE_FLEET_UPDATE_VERIFIED`) plus the counter advance
/// (`EDGE_FLEET_UPDATE_SEQUENCE_ADVANCED`). The P9.34 gate emits its own
/// `EDGE_UPDATE_VERIFIED`/`EDGE_UPDATE_VERIFICATION_REFUSED` lines inside
/// step 7 (one verification, two honest audit layers).
pub struct ExtendedUpdateGate {
    device_vendor: String,
    device_class: String,
    device_id: String,
    base_gate: UpdateGate,
    verifier: Arc<dyn UpdateVerifier + Send + Sync>,
    counter: AntiRollbackCounter,
    audit: Arc<dyn AuditSink>,
    clock: Arc<dyn Clock + Send + Sync>,
}

impl ExtendedUpdateGate {
    /// Composes the extended gate. The device identity is the edge's OWN
    /// configured/enrolled identity (all three fields mandatory — a
    /// device that cannot state its identity cannot prove applicability
    /// and refuses everything, fail-closed). The verifier is injected:
    /// the composition root decides between `NoUpdateVerifier` (the
    /// default; everything refused) and [`Sha256UpdateVerifier`] (the
    /// DEPLOYMENT REQUIREMENT wiring) — the fail-closed default semantics
    /// are untouched.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        device_vendor: String,
        device_class: String,
        device_id: String,
        trusted_signer_references: Vec<String>,
        verifier: Arc<dyn UpdateVerifier + Send + Sync>,
        counter: AntiRollbackCounter,
        audit: Arc<dyn AuditSink>,
        clock: Arc<dyn Clock + Send + Sync>,
    ) -> Result<Self, UpdateBundleError> {
        if device_vendor.trim().is_empty()
            || device_class.trim().is_empty()
            || device_id.trim().is_empty()
        {
            return Err(UpdateBundleError::InvalidInput {
                reason: "device_vendor, device_class, and device_id are mandatory — a device \
                         that cannot state its identity refuses every update (fail-closed)"
                    .to_string(),
            });
        }
        let base_gate = UpdateGate::new(
            trusted_signer_references,
            Box::new(SharedVerifierHandle(verifier.clone())),
            audit.clone(),
            clock.clone(),
        );
        Ok(Self {
            device_vendor,
            device_class,
            device_id,
            base_gate,
            verifier,
            counter,
            audit,
            clock,
        })
    }

    /// The anti-rollback counter (diagnostics: last-seen value).
    pub fn last_seen_sequence(&self) -> u64 {
        self.counter.last_seen()
    }

    /// The anti-rollback counter (diagnostics: how many corrupt tail
    /// records the counter's open at composition truncated — the loss
    /// counted at boot, never silent; the count belongs to the instance
    /// that opened the file, so a later direct open sees a clean file).
    pub fn corrupt_tail_truncated(&self) -> u64 {
        self.counter.corrupt_tail_truncated()
    }

    /// Verifies ONE fleet-update bundle against its v2 manifest, BEFORE
    /// anything is written (the P9.34 ordering rule carried through:
    /// this function takes no path and touches no device file). See the
    /// type doc for the recorded refusal order.
    pub fn verify_fleet_bundle(
        &self,
        manifest: Option<&ExtendedUpdateManifest>,
        bundle: &[u8],
    ) -> Result<VerifiedExtendedUpdate, UpdateBundleError> {
        let verdict = self.verify_inner(manifest, bundle);
        match &verdict {
            Ok(verified) => {
                self.audit_event(
                    "EDGE_FLEET_UPDATE_VERIFIED",
                    format!(
                        "package='{}' version={} sequence={} payloads={} — extended verification \
                         PASSED (the P9.34 gate passed inside; verification BEFORE any write)",
                        verified.manifest.base.package_id,
                        verified.manifest.base.version,
                        verified.manifest.anti_rollback_sequence,
                        verified.manifest.payloads.len(),
                    ),
                );
            }
            Err(error) => {
                self.audit_event(
                    "EDGE_FLEET_UPDATE_REFUSED",
                    format!(
                        "refusal code={} :: {error} — refused BEFORE any file was written",
                        error.name(),
                    ),
                );
            }
        }
        verdict
    }

    fn verify_inner(
        &self,
        manifest: Option<&ExtendedUpdateManifest>,
        bundle: &[u8],
    ) -> Result<VerifiedExtendedUpdate, UpdateBundleError> {
        // 1. Present.
        let manifest = manifest.ok_or(UpdateBundleError::UnsignedBundle)?;
        // 2. Bundle within the total-size cap (a code maximum).
        if bundle.len() > MAX_UPDATE_PAYLOAD_TOTAL_BYTES {
            return Err(UpdateBundleError::BundleTooLarge {
                size_bytes: bundle.len(),
                cap: MAX_UPDATE_PAYLOAD_TOTAL_BYTES,
            });
        }
        // 3. v2 structural validation (the base P9.34 validation runs
        //    inside; its text rides the refusal reason).
        manifest.validate()?;
        // 4. Applicability to THIS device (identity is the edge's own).
        manifest
            .applicability_refuses(&self.device_vendor, &self.device_class, &self.device_id)?;
        // 5. Expiry (injected clock).
        manifest.expiry_refuses(self.now_unix_millis())?;
        // 6. Anti-rollback sequence (fail-closed on the counter).
        manifest.sequence_refuses(self.counter.last_seen())?;
        // 7. THE P9.34 GATE, VERBATIM: unsigned → manifest valid →
        //    algorithm → signer → floor → digest; audited inside; no
        //    filesystem write anywhere in it.
        let verified = self
            .base_gate
            .verify_before_staging(Some(manifest.base_manifest()), bundle)
            .map_err(|error| UpdateBundleError::BaseVerificationRefused {
                code: error.name().to_string(),
                reason: error.to_string(),
            })?;
        // 8. Payload digests (the SAME verifier instance; one algorithm).
        for payload in &manifest.payloads {
            let end = payload.offset.saturating_add(payload.size_bytes);
            if end > bundle.len() {
                return Err(UpdateBundleError::PayloadBoundsOutside {
                    payload_name: payload.name.clone(),
                });
            }
            let slice = &bundle[payload.offset..end];
            match self.verifier.verify(slice, &payload.digest_hex) {
                UpdateDigestVerdict::Matches => {}
                UpdateDigestVerdict::Mismatch => {
                    return Err(UpdateBundleError::PayloadDigestMismatch {
                        payload_name: payload.name.clone(),
                    })
                }
                UpdateDigestVerdict::Refused { reason } => {
                    return Err(UpdateBundleError::PayloadVerifierRefused {
                        payload_name: payload.name.clone(),
                        reason,
                    })
                }
            }
        }
        // 9. The counter advances DURABLY before the verdict is issued.
        self.counter.accept(manifest.anti_rollback_sequence)?;
        self.audit_event(
            "EDGE_FLEET_UPDATE_SEQUENCE_ADVANCED",
            format!(
                "anti-rollback sequence {} recorded (durable BEFORE the verdict is returned; \
                 {})",
                manifest.anti_rollback_sequence,
                ANTI_ROLLBACK_SEMANTICS_NOTE,
            ),
        );
        Ok(VerifiedExtendedUpdate {
            manifest: manifest.clone(),
            verified_at_unix_millis: verified.verified_at_unix_millis,
            base: verified,
        })
    }

    fn now_unix_millis(&self) -> i64 {
        let stamp = self.clock.now_ingest_time();
        i64::try_from(stamp.unix_millis()).unwrap_or(i64::MAX)
    }

    /// Records one audit event through the edge `AuditSink` (the same
    /// dedicated-event-name-leads-detail pattern as the P9.34 module;
    /// classified `EdgeAuditKind::UpdateLifecycle` — FRV1-R6-005).
    fn audit_event(&self, name: &'static str, detail: String) {
        let event = EdgeAuditEvent {
            recorded_at: self.clock.now_ingest_time(),
            kind: EdgeAuditKind::UpdateLifecycle,
            detail: format!("{name}: {detail}"),
        };
        if let Err(error) = self.audit.record(event) {
            tracing::error!(%error, audit_event = name, "extended-update-gate audit write FAILED");
        }
    }
}

/// The sequence-advance audit note (a short pointer; the full statement
/// is [`crate::update_bundle::ANTI_ROLLBACK_SEMANTICS_STATEMENT`]).
const ANTI_ROLLBACK_SEMANTICS_NOTE: &str =
    "the strictly-ahead rule is UNVERIFIED against the TUF/SUIT spec texts";

// --- Tests (WRITTEN-UNRUN; execution is a human gate) -----------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::AuditSink as EdgeAuditSinkTrait;
    use crate::update_bundle::{
        PayloadDigest, UpdateApplicability, UPDATE_BUNDLE_FORMAT_V2,
    };
    use crate::update_verification::{
        test_grade_fnv1a64_hex, NoUpdateVerifier, TestOnlyFnv1a64UpdateVerifier, UpdateManifest,
    };
    use std::collections::BTreeSet;
    use std::sync::Mutex;

    // --- A fixed clock (the injected-clock discipline) ------------------

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

    // --- A collecting audit sink (in-memory; the ports-crate pattern) ---

    #[derive(Default)]
    struct CollectingAuditSink {
        events: Mutex<Vec<String>>,
    }

    impl CollectingAuditSink {
        fn lines(&self) -> Vec<String> {
            self.events
                .lock()
                .expect("poisoned test sink mutex")
                .clone()
        }
    }

    impl EdgeAuditSinkTrait for CollectingAuditSink {
        fn record(&self, event: EdgeAuditEvent) -> Result<(), crate::ports::EdgeError> {
            self.events
                .lock()
                .expect("poisoned test sink mutex")
                .push(event.detail);
            Ok(())
        }
    }

    // --- Fixtures ---------------------------------------------------------

    const BUNDLE: &[u8] = b"0123456789abcdef";

    fn base_manifest() -> UpdateManifest {
        UpdateManifest {
            package_id: "edge-agent-release".to_string(),
            version: "1.2.0".to_string(),
            bundle_digest_hex: test_grade_fnv1a64_hex(BUNDLE),
            digest_algorithm: "fnv1a64-test".to_string(),
            signer_key_reference: "key-store:flexcore-release-targets".to_string(),
            minimum_version_floor: "0.0.0".to_string(),
            approval_reference: "approval-record-1".to_string(),
        }
    }

    fn v2_manifest(sequence: u64) -> ExtendedUpdateManifest {
        ExtendedUpdateManifest {
            base: base_manifest(),
            applicability: UpdateApplicability {
                vendor: "neryva".to_string(),
                device_class: "edge-gateway".to_string(),
                device_ids: BTreeSet::new(),
            },
            anti_rollback_sequence: sequence,
            payloads: vec![PayloadDigest {
                name: "fw.bin".to_string(),
                offset: 0,
                size_bytes: BUNDLE.len(),
                digest_hex: test_grade_fnv1a64_hex(BUNDLE),
            }],
            expires_at_unix_millis: 1_800_000_000_000,
        }
    }

    fn open_counter(tag: &str) -> AntiRollbackCounter {
        let dir = std::env::temp_dir().join(format!(
            "flexcore-p18-session-{tag}-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("antirb.counter");
        let _ = std::fs::remove_file(&path);
        AntiRollbackCounter::open(&path).expect("counter opens")
    }

    fn test_gate(tag: &str) -> ExtendedUpdateGate {
        let audit: Arc<dyn AuditSink> = Arc::new(CollectingAuditSink::default());
        ExtendedUpdateGate::new(
            "neryva".to_string(),
            "edge-gateway".to_string(),
            "device-1".to_string(),
            vec!["key-store:flexcore-release-targets".to_string()],
            Arc::new(TestOnlyFnv1a64UpdateVerifier),
            open_counter(tag),
            audit,
            Arc::new(FixedClock(1_700_000_000_000)),
        )
        .expect("gate composes")
    }

    // --- The SHA-256 verifier (published FIPS 180-4 vector) --------------

    #[test]
    fn sha256_verifier_matches_the_fips_180_4_abc_vector() {
        // The published FIPS 180-4 example digest of "abc" — pinned here
        // so the verifier's algorithm selection and hex comparison are
        // anchored to the standard (verified at the first human cargo
        // test; WRITTEN-UNRUN until then).
        let verifier = Sha256UpdateVerifier;
        assert_eq!(verifier.algorithm_name(), "sha-256");
        assert_eq!(
            verifier.verify(b"abc", "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"),
            UpdateDigestVerdict::Matches
        );
        assert_eq!(
            verifier.verify(b"abc", "0000000000000000000000000000000000000000000000000000000000000000"),
            UpdateDigestVerdict::Mismatch
        );
        // The FIPS 180-4 empty-string vector.
        assert_eq!(
            verifier.verify(b"", "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"),
            UpdateDigestVerdict::Matches
        );
    }

    // --- The state machine (P18.04) ----------------------------------------

    fn session() -> UpdateSessionStateMachine {
        UpdateSessionStateMachine::new("attempt-1", "device-1", "1.2.0", 3, 1_000)
            .expect("session opens")
    }

    #[test]
    fn the_happy_path_walks_the_recorded_flow() {
        let mut session = session();
        assert_eq!(session.state(), UpdateProgressState::Idle);
        session.begin_download(0, 1_100).expect("download begins");
        assert_eq!(session.state(), UpdateProgressState::Downloading);
        session.download_complete(1_200).expect("download completes");
        session.verify_passed(1_300).expect("verify passes");
        assert_eq!(session.state(), UpdateProgressState::VerifiedPendingInstall);
        session.begin_install(1_400).expect("install begins");
        session.install_complete(1_500).expect("install completes");
        assert_eq!(session.state(), UpdateProgressState::HealthCheckPending);
        session.health_confirmed(1_600).expect("health confirms");
        assert_eq!(session.state(), UpdateProgressState::Active);
        assert!(session.state().is_terminal());
        // Terminal: nothing further.
        assert!(matches!(
            session.begin_download(0, 1_700),
            Err(UpdateSessionError::IllegalTransition { .. })
        ));
    }

    #[test]
    fn the_auto_rollback_path_walks_health_failure_to_rolled_back() {
        let mut session = session();
        session.begin_download(0, 1_100).expect("ok");
        session.download_complete(1_200).expect("ok");
        session.verify_passed(1_300).expect("ok");
        session.begin_install(1_400).expect("ok");
        session.install_complete(1_500).expect("ok");
        session.health_failed(1_600).expect("rollback begins");
        assert_eq!(session.state(), UpdateProgressState::RollingBack);
        session.rollback_complete(1_700).expect("rollback completes");
        assert_eq!(session.state(), UpdateProgressState::RolledBack);
        assert!(session.state().is_terminal());
    }

    #[test]
    fn failure_states_are_explicit_and_named() {
        let mut session_download_failed = session();
        session_download_failed.begin_download(0, 1_100).expect("ok");
        session_download_failed.download_failed(1_150).expect("failed");
        assert_eq!(session_download_failed.failure_state(), Some(UPDATE_FAILURE_DOWNLOAD_FAILED));
        let notification = session_download_failed.status_notification();
        assert!(notification.summary().contains(UPDATE_FAILURE_DOWNLOAD_FAILED));

        let mut session_verify_refused = session();
        session_verify_refused.begin_download(0, 1_100).expect("ok");
        session_verify_refused.download_complete(1_200).expect("ok");
        session_verify_refused.verify_refused(1_300).expect("failed");
        assert_eq!(session_verify_refused.failure_state(), Some(UPDATE_FAILURE_VERIFICATION_REFUSED));

        let mut session_install_failed = session();
        session_install_failed.begin_download(0, 1_100).expect("ok");
        session_install_failed.download_complete(1_200).expect("ok");
        session_install_failed.verify_passed(1_300).expect("ok");
        session_install_failed.begin_install(1_400).expect("ok");
        session_install_failed.install_failed(1_450).expect("failed");
        assert_eq!(session_install_failed.failure_state(), Some(UPDATE_FAILURE_INSTALL_FAILED));

        let mut session = session();
        session.begin_download(0, 1_100).expect("ok");
        session.download_complete(1_200).expect("ok");
        session.verify_passed(1_300).expect("ok");
        session.begin_install(1_400).expect("ok");
        session.install_complete(1_500).expect("ok");
        session.health_failed(1_600).expect("ok");
        session.rollback_failed(1_650).expect("failed");
        assert_eq!(session.failure_state(), Some(UPDATE_FAILURE_ROLLBACK_FAILED));
    }

    #[test]
    fn commands_in_flight_block_the_session_start() {
        let mut session = session();
        // The P18.15 blocking rule (consumed by citation): updates are
        // BLOCKED while commands are ISSUED/TRACKING.
        assert!(matches!(
            session.begin_download(2, 1_100),
            Err(UpdateSessionError::CommandsInFlight { issued_or_tracking: 2 })
        ));
        // The session is still Idle — the refusal changed nothing.
        assert_eq!(session.state(), UpdateProgressState::Idle);
        session.begin_download(0, 1_100).expect("drained: download begins");
    }

    #[test]
    fn cancel_is_only_legal_before_install() {
        let mut session_cancel_downloading = session();
        session_cancel_downloading.begin_download(0, 1_100).expect("ok");
        session_cancel_downloading.cancel(1_150).expect("cancel from Downloading");
        assert_eq!(session_cancel_downloading.failure_state(), Some(UPDATE_FAILURE_CANCELLED));

        let mut session_cancel_verified = session();
        session_cancel_verified.begin_download(0, 1_100).expect("ok");
        session_cancel_verified.download_complete(1_200).expect("ok");
        session_cancel_verified.verify_passed(1_300).expect("ok");
        session_cancel_verified.cancel(1_350).expect("cancel from VerifiedPendingInstall");

        let mut session = session();
        session.begin_download(0, 1_100).expect("ok");
        session.download_complete(1_200).expect("ok");
        session.verify_passed(1_300).expect("ok");
        session.begin_install(1_400).expect("ok");
        // An in-progress install cannot be cancelled — only failed.
        assert!(matches!(
            session.cancel(1_450),
            Err(UpdateSessionError::IllegalTransition { .. })
        ));
    }

    #[test]
    fn out_of_order_transitions_are_named_refusals() {
        let mut session = session();
        // Cannot install before downloading/verifying.
        assert!(matches!(
            session.begin_install(1_100),
            Err(UpdateSessionError::IllegalTransition { .. })
        ));
        // Cannot roll back before a health check.
        assert!(matches!(
            session.rollback_complete(1_100),
            Err(UpdateSessionError::IllegalTransition { .. })
        ));
    }

    #[test]
    fn status_notifications_carry_the_closed_vocabulary() {
        let mut session = session();
        session.begin_download(0, 1_100).expect("ok");
        let notification = session.status_notification();
        assert_eq!(notification.state, UpdateProgressState::Downloading);
        assert_eq!(notification.state.name(), "UPDATE_DOWNLOADING");
        assert!(notification.failure_state.is_none());
        assert!(notification.summary().contains("device-1"));
    }

    // --- The extended gate (P18.05) -----------------------------------------

    #[test]
    fn the_extended_gate_full_pass_advances_the_counter_durably() {
        let gate = test_gate("full-pass");
        let manifest = v2_manifest(7);
        gate.verify_fleet_bundle(Some(&manifest), BUNDLE)
            .expect("full pass");
        assert_eq!(gate.last_seen_sequence(), 7, "the counter advanced");
        // The SAME manifest cannot be re-presented (strictly-ahead rule):
        // a retry after a later failure requires re-issued metadata.
        assert!(matches!(
            gate.verify_fleet_bundle(Some(&manifest), BUNDLE),
            Err(UpdateBundleError::SequenceNotAhead { declared: 7, last_seen: 7 })
        ));
        // A higher sequence with the same content passes.
        let manifest_8 = v2_manifest(8);
        gate.verify_fleet_bundle(Some(&manifest_8), BUNDLE)
            .expect("sequence 8 passes");
        assert_eq!(gate.last_seen_sequence(), 8);
    }

    #[test]
    fn the_extended_gate_refuses_before_the_base_gate_on_metadata_failures() {
        let gate = test_gate("metadata");
        // Unsigned.
        assert!(matches!(
            gate.verify_fleet_bundle(None, BUNDLE),
            Err(UpdateBundleError::UnsignedBundle)
        ));
        // Not applicable (wrong vendor) — refused before any digest work.
        let mut manifest = v2_manifest(1);
        manifest.applicability.vendor = "other".to_string();
        assert!(matches!(
            gate.verify_fleet_bundle(Some(&manifest), BUNDLE),
            Err(UpdateBundleError::ApplicabilityMismatch { .. })
        ));
        // Expired.
        let mut manifest = v2_manifest(1);
        manifest.expires_at_unix_millis = 1_699_999_999_999;
        assert!(matches!(
            gate.verify_fleet_bundle(Some(&manifest), BUNDLE),
            Err(UpdateBundleError::Expired { .. })
        ));
        // Sequence-zero manifest: refused by v2 validation (the floor),
        // never by a special case in the gate.
        let manifest_zero = v2_manifest(0);
        assert!(matches!(
            gate.verify_fleet_bundle(Some(&manifest_zero), BUNDLE),
            Err(UpdateBundleError::ManifestInvalid { .. })
        ));
        // Payload digest mismatch (declared digest does not match bytes).
        let mut manifest = v2_manifest(1);
        manifest.payloads[0].digest_hex = "0011223344556677".to_string();
        assert!(matches!(
            gate.verify_fleet_bundle(Some(&manifest), BUNDLE),
            Err(UpdateBundleError::PayloadDigestMismatch { payload_name }) if payload_name == "fw.bin"
        ));
        // Payload bounds outside the bundle.
        let mut manifest = v2_manifest(1);
        manifest.payloads[0].offset = BUNDLE.len() + 1;
        assert!(matches!(
            gate.verify_fleet_bundle(Some(&manifest), BUNDLE),
            Err(UpdateBundleError::PayloadBoundsOutside { .. })
        ));
    }

    #[test]
    fn the_fail_closed_default_is_preserved_verbatim() {
        // The same composition with NO verifier configured: the base gate
        // holds NoUpdateVerifier and refuses EVERYTHING — the P9.34
        // fail-closed semantics, unchanged (context constraint 1).
        let sink = Arc::new(CollectingAuditSink::default());
        let audit: Arc<dyn AuditSink> = sink.clone();
        let gate = ExtendedUpdateGate::new(
            "neryva".to_string(),
            "edge-gateway".to_string(),
            "device-1".to_string(),
            vec!["key-store:flexcore-release-targets".to_string()],
            Arc::new(NoUpdateVerifier),
            open_counter("fail-closed"),
            audit,
            Arc::new(FixedClock(1_700_000_000_000)),
        )
        .expect("gate composes");
        let manifest = v2_manifest(1);
        let error = gate
            .verify_fleet_bundle(Some(&manifest), BUNDLE)
            .expect_err("refused");
        match error {
            UpdateBundleError::BaseVerificationRefused { code, reason } => {
                // The P9.34 gate order is present -> valid -> ALGORITHM
                // (step 3) -> signer -> floor -> verifier verdict (step 6):
                // NoUpdateVerifier computes "none" while the manifest
                // declares "fnv1a64-test", so the algorithm mismatch fires
                // before the verifier is ever consulted -- the refusal is
                // still the fail-closed default, at the earlier step.
                assert_eq!(code, "UPDATE_DIGEST_ALGORITHM_MISMATCH");
                assert!(reason.contains("fnv1a64-test"), "{reason}");
                assert!(reason.contains("'none'"), "{reason}");
            }
            other => panic!("expected BaseVerificationRefused, got {other:?}"),
        }
        // The verifier-level refusal itself is pinned by a direct call
        // (the branch the gate's algorithm step shadows): the default
        // refuses every bundle, with the fail-closed reason.
        match NoUpdateVerifier.verify(BUNDLE, "deadbeef") {
            UpdateDigestVerdict::Refused { reason } => {
                assert!(reason.contains("fail-closed"), "{reason}");
            }
            other => panic!("expected the default verifier to refuse, got {other:?}"),
        }
        // The counter did NOT advance (a refused verdict records nothing).
        assert_eq!(gate.last_seen_sequence(), 0);
        // The audit lines: the extended envelope refusal AND the P9.34
        // refusal both fired (two honest audit layers).
        let lines = sink.lines();
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
    }

    #[test]
    fn the_device_identity_is_mandatory() {
        let audit: Arc<dyn AuditSink> = Arc::new(CollectingAuditSink::default());
        assert!(matches!(
            ExtendedUpdateGate::new(
                String::new(),
                "edge-gateway".to_string(),
                "device-1".to_string(),
                vec![],
                Arc::new(NoUpdateVerifier),
                open_counter("identity"),
                audit,
                Arc::new(FixedClock(1_700_000_000_000)),
            ),
            Err(UpdateBundleError::InvalidInput { .. })
        ));
    }

    #[test]
    fn a_device_id_allowlist_outside_the_list_refuses() {
        let audit: Arc<dyn AuditSink> = Arc::new(CollectingAuditSink::default());
        let gate = ExtendedUpdateGate::new(
            "neryva".to_string(),
            "edge-gateway".to_string(),
            "device-2".to_string(),
            vec!["key-store:flexcore-release-targets".to_string()],
            Arc::new(TestOnlyFnv1a64UpdateVerifier),
            open_counter("allowlist"),
            audit,
            Arc::new(FixedClock(1_700_000_000_000)),
        )
        .expect("gate composes");
        let mut manifest = v2_manifest(1);
        manifest.applicability.device_ids = BTreeSet::from(["device-9".to_string()]);
        assert!(matches!(
            gate.verify_fleet_bundle(Some(&manifest), BUNDLE),
            Err(UpdateBundleError::ApplicabilityMismatch { .. })
        ));
    }

    #[test]
    fn the_statements_pin_the_honesty_bounds() {
        assert!(UPDATE_SESSION_STATEMENT.contains("fail-closed preserved"));
        assert!(UPDATE_SESSION_STATEMENT.contains("No update has ever been built"));
        assert!(UPDATE_FLOW_HARDWARE_STATEMENT.contains("PENDING PILOT HARDWARE SPEC"));
        assert!(UPDATE_FLOW_HARDWARE_STATEMENT.contains("REGISTER"));
        // The closed failure-state names exist and are stable.
        assert_eq!(UPDATE_FAILURE_DOWNLOAD_FAILED, "UPDATE_FAILURE_DOWNLOAD_FAILED");
        assert_eq!(UPDATE_FAILURE_VERIFICATION_REFUSED, "UPDATE_FAILURE_VERIFICATION_REFUSED");
        assert_eq!(UPDATE_FAILURE_INSTALL_FAILED, "UPDATE_FAILURE_INSTALL_FAILED");
        assert_eq!(UPDATE_FAILURE_HEALTH_CHECK_FAILED, "UPDATE_FAILURE_HEALTH_CHECK_FAILED");
        assert_eq!(UPDATE_FAILURE_ROLLBACK_FAILED, "UPDATE_FAILURE_ROLLBACK_FAILED");
        assert_eq!(UPDATE_FAILURE_CANCELLED, "UPDATE_FAILURE_CANCELLED");
        // The format id carried through.
        assert_eq!(UPDATE_BUNDLE_FORMAT_V2, "flexcore-edge-update-manifest-v2");
    }
}
