// FIM-F12.04 (Wave 6, final implementation milestone) — the DEVICE-SIDE
//         A/B slot mechanics for the OTA flow: the download → verify →
//         install → status-report state machine per slot, the failed-
//         install/health-failure rollback to the previous slot, and the
//         status notifications — COMPOSED, never forked, from the existing
//         surfaces:
//
// - the flow machine is the EXISTING P18.04
//   [`crate::update_session::UpdateSessionStateMachine`] (every illegal
//   transition a named refusal; terminal states terminal) — this module
//   drives it and adds the SLOT bookkeeping on top;
// - the source of truth for the active version is the EXISTING P9.34
//   append-only register ([`crate::update_verification::FileUpdateLedger`])
//   per [`crate::update_verification::UPDATE_SLOT_DECISION`] ("the
//   register is the source of truth; a two-slot A/B layout is the
//   RECOMMENDED deployment shape") — the slot table here is the
//   device-side view the register governs, never a second truth;
// - activation/rollback are the EXISTING
//   [`crate::update_verification::UpdateActivation`] decision rules
//   (previously-activated + behind-active + the never-silent-downgrade
//   floor rule with its recorded override);
// - verification is the EXISTING
//   [`crate::update_session::ExtendedUpdateGate::verify_fleet_bundle`]
//   (the P9.34 gate + the v2 checks + the durable anti-rollback-counter
//   advance); its refusal rides this module's `VerifyRefused` with the
//   stable P9.34/v2 code carried verbatim (the `BaseVerificationRefused`
//   pattern — the existing taxonomy is never re-coded).
//
// IMPLEMENTED-UNDER-FIM-F12, WRITTEN-UNRUN: this module has never been
// compiled or run; no device has ever installed anything; no slot exists.
// The slot WRITE, the boot switch, and the watchdog/boot-counter health
// evidence are DEVICE concerns the platform SPECIFIES (interfaces,
// evidence contracts, thresholds) and cannot implement — every hardware
// determination is PENDING PILOT HARDWARE SPEC (the P6.34 posture; the
// `UPDATE_FLOW_HARDWARE_STATEMENT` carries over verbatim). What this
// module adds is the deterministic, in-memory SLOT BOOKKEEPING the
// composition drives against a real register.
//
// THE ANTI-ROLLBACK × A/B ROLLBACK RESOLUTION (recorded honestly — this
// is THE interaction the FIM-F12.04 task asks to resolve against
// `update_bundle.rs`'s strictly-greater-than semantics, line 125's
// `SequenceNotAhead` rule):
//
// 1. A rollback NEVER re-presents a manifest. The strictly-ahead rule
//    (`ANTI_ROLLBACK_SEMANTICS_STATEMENT`) governs MANIFEST
//    re-presentation at the gate; rollback is a REGISTER event
//    (re-activation of a previously-activated version). The two rules
//    compose without exception: the gate is never called on a rollback
//    path here, so the counter's strictly-ahead semantics are untouched —
//    not weakened, not bypassed.
// 2. The anti-rollback counter NEVER rewinds: it is monotonic by
//    construction (append-only, max-by-replay), and nothing on this
//    module or the register touches it on a rollback. Consequence
//    (recorded): after any attempt — successful or rolled back — the NEXT
//    update must still carry a sequence strictly ahead of the
//    highest-ever-accepted sequence, even though the RUNNING version may
//    be the older one.
// 3. The previous slot's own sequence always satisfies the policy by
//    construction: it was accepted by the gate (which advanced the
//    counter durably to exactly that value) when IT was installed. There
//    is therefore no case where a register-activated rollback target
//    violates the last-seen sequence policy.
// 4. THE COMMON A/B CASE NEEDS NO REGISTER RECORD (recorded): activation
//    happens at health CONFIRMATION, so a health FAILURE means the failed
//    version was never activated and the register's active version is
//    ALREADY the rollback target. A re-activation of the RUNNING version
//    is refused by the crate's own rule (`UPDATE_ROLLBACK_TARGET_
//    NOT_BEHIND_ACTIVE`) — no register rollback record exists or is
//    possible; the slot-level boot rollback completes, is REPORTED
//    (`EDGE_OTA_SLOT_ROLLED_BACK`), and the register stays untouched (it
//    already carries the truth). The register rules run VERBATIM when the
//    failed version HAD been activated (a post-activation rollback: floor
//    rule and recorded override included).
// 5. THE HONEST CONFLICT (recorded, never silently weakened): the strict
//    reading DOES make a rollback impossible in exactly one case — a
//    rollback target that was never REGISTER-activated. Concretely: the
//    FIRST-ever update on a device whose register is empty fails its
//    health check → the factory slot was never gate-verified or
//    register-activated → `UpdateActivation::rollback` refuses
//    (`UPDATE_ROLLBACK_TARGET_NEVER_ACTIVATED`). This module records the
//    refusal (`OTA_SLOTS_ROLLBACK_IMPOSSIBLE`, audited), reports
//    `UPDATE_FAILED` with the explicit `UPDATE_FAILURE_ROLLBACK_FAILED`
//    failure state, and leaves the register untouched — the recorded
//    P18.04 semantic ("the rollback itself failed — the device needs
//    operator intervention; the register shows the last good state").
//    Operator recovery below the floor still goes through the recorded
//    [`crate::update_verification::RollbackOverride`].
//
// AUDIT (FIM-F12.05): every slot-level step is audited through the
// EXISTING edge audit shapes (`AuditSink` + `EdgeAuditKind::UpdateLifecycle`,
// dedicated event name leading the detail): download begun/complete,
// verification pass/refusal, install begun/completed/failed, health
// confirmed/failed, activation (and its refusal), rollback
// begun/completed/refused, and every status report. The register's own
// audit lines (`EDGE_UPDATE_ACTIVATED`, `EDGE_UPDATE_ROLLBACK`) ride
// inside the activation/rollback calls — one event, two honest layers,
// the P18.05 pattern.

use std::sync::Arc;

use flexcore_ports::clock::Clock;

use crate::ports::{AuditSink, EdgeAuditEvent, EdgeAuditKind};
use crate::update_bundle::ExtendedUpdateManifest;
use crate::update_session::{
    UpdateProgressState, UpdateSessionError, UpdateSessionStateMachine, UpdateStatusNotification,
    UPDATE_FAILURE_INSTALL_FAILED, UPDATE_FAILURE_ROLLBACK_FAILED, VerifiedExtendedUpdate,
};
use crate::update_verification::{
    FileUpdateLedger, RollbackOverride, UpdateActivation, UpdateLedgerRecord,
};

// --- Honesty statement -------------------------------------------------------------

/// The FIM-F12.04 A/B-slot statement (pinned so tests and reviewers can
/// read what IS and IS NOT claimed).
pub const OTA_SLOTS_STATEMENT: &str = "the FIM-F12 A/B slots drive the EXISTING P18.04 flow \
     machine and the EXISTING P9.34 register/activation rules; the slot write, the boot \
     switch, and the health evidence are DEVICE concerns the platform specifies but \
     cannot implement (PENDING PILOT HARDWARE SPEC, P6.34). A rollback NEVER re-presents \
     a manifest and the anti-rollback counter NEVER rewinds (monotonic by construction): \
     the strictly-ahead semantics are untouched, and the NEXT update after a rollback \
     must still be strictly ahead of the highest-ever-accepted sequence. The rollback \
     resolution is the REGISTER'S ACTUAL POLICY, three-way: when the failed version was \
     never activated (the common A/B case — activation happens at health confirmation) \
     the register already carries the previous version and NO register rollback record \
     exists or is possible (a re-activation of the running version is refused as \
     not-behind-active) — the slot-level boot rollback completes and is reported; when \
     the failed version HAD been activated, the register rollback rules run verbatim \
     (floor rule and recorded override included); and a rollback target that was never \
     REGISTER-activated (the factory slot on a first-ever update) makes the rollback \
     impossible — the refusal is recorded and reported as \
     UPDATE_FAILURE_ROLLBACK_FAILED, never weakened. WRITTEN-UNRUN; no device has ever \
     installed anything; no slot exists.";

// --- Errors -----------------------------------------------------------------------

/// Named errors of the slot mechanics. Every failure is named
/// ([`OtaSlotsError::name`]); nothing is a silent pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OtaSlotsError {
    /// An input was unusable (empty ids, non-positive instants, …).
    InvalidInput { reason: String },
    /// The flow machine refused the transition (the P18.04 recorded flow;
    /// the session error's own text rides verbatim).
    FlowRefused { reason: String },
    /// The extended gate REFUSED the verification — the stable P9.34/v2
    /// refusal code rides verbatim (the existing taxonomy, never re-coded).
    VerifyRefused { code: String, reason: String },
    /// The previous slot carries no record — there is nothing to roll
    /// back TO at the slot level.
    NoPreviousSlot { reason: String },
    /// The register-level rollback was REFUSED (never-activated target /
    /// below-floor without override / not-behind-active) — THE honest
    /// anti-rollback conflict; the register's stable refusal code rides.
    RollbackImpossible { reason: String },
    /// The register-level activation was refused (e.g. the version is
    /// already active) — the attempt cannot complete as an activation;
    /// the auto-rollback path runs and the refusal is recorded.
    ActivationRefused { reason: String },
}

impl OtaSlotsError {
    /// The stable machine-readable error name (the named-error table key).
    pub fn name(&self) -> &'static str {
        match self {
            OtaSlotsError::InvalidInput { .. } => "OTA_SLOTS_INPUT_INVALID",
            OtaSlotsError::FlowRefused { .. } => "OTA_SLOTS_FLOW_REFUSED",
            OtaSlotsError::VerifyRefused { .. } => "OTA_SLOTS_VERIFY_REFUSED",
            OtaSlotsError::NoPreviousSlot { .. } => "OTA_SLOTS_NO_PREVIOUS_SLOT",
            OtaSlotsError::RollbackImpossible { .. } => "OTA_SLOTS_ROLLBACK_IMPOSSIBLE",
            OtaSlotsError::ActivationRefused { .. } => "OTA_SLOTS_ACTIVATION_REFUSED",
        }
    }
}

impl std::fmt::Display for OtaSlotsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OtaSlotsError::InvalidInput { reason } => {
                write!(f, "the OTA slot input is invalid: {reason}")
            }
            OtaSlotsError::FlowRefused { reason } => {
                write!(f, "the P18.04 flow machine refused the transition: {reason}")
            }
            OtaSlotsError::VerifyRefused { code, reason } => write!(
                f,
                "the existing extended gate refused the verification ({code}): {reason} — \
                 the preserved P9.34/v2 refusal order, verbatim"
            ),
            OtaSlotsError::NoPreviousSlot { reason } => {
                write!(f, "the previous slot carries no record to roll back to: {reason}")
            }
            OtaSlotsError::RollbackImpossible { reason } => write!(
                f,
                "the rollback is IMPOSSIBLE under the recorded rules — refused, never \
                 weakened: {reason}"
            ),
            OtaSlotsError::ActivationRefused { reason } => {
                write!(f, "the register-level activation was refused: {reason}")
            }
        }
    }
}

impl std::error::Error for OtaSlotsError {}

// --- Slots -----------------------------------------------------------------------------

/// The two A/B slots (the RECOMMENDED deployment shape per
/// `UPDATE_SLOT_DECISION`; the register stays the source of truth).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum OtaSlotId {
    /// Slot A (provisioned at construction with the factory image).
    A,
    /// Slot B (empty until the first install targets it).
    B,
}

impl OtaSlotId {
    /// The stable slot name (audit lines, status reports).
    pub fn name(self) -> &'static str {
        match self {
            OtaSlotId::A => "slot-a",
            OtaSlotId::B => "slot-b",
        }
    }

    /// The OTHER slot (the install target of the active slot's next
    /// attempt).
    pub fn other(self) -> OtaSlotId {
        match self {
            OtaSlotId::A => OtaSlotId::B,
            OtaSlotId::B => OtaSlotId::A,
        }
    }
}

/// One slot's device-side record. The `anti_rollback_sequence` is the
/// sequence the slot's manifest carried when the gate accepted it — 0 for
/// the factory slot (never gate-verified; the counter floor is 0, so the
/// FIRST manifest must carry a sequence >= 1, the
/// `ANTI_ROLLBACK_SEQUENCE_FLOOR`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtaSlotRecord {
    pub slot: OtaSlotId,
    pub version: String,
    pub anti_rollback_sequence: u64,
    pub healthy: bool,
}

/// ONE update attempt in flight against the slots: the EXISTING P18.04
/// flow machine plus the attempt's slot bookkeeping (the target and the
/// slot that was active before the install — the rollback target). Holds
/// no bundle bytes and no paths (the gate owns verification; the register
/// owns the durable truth).
pub struct OtaSlotAttempt {
    session: UpdateSessionStateMachine,
    target_version: String,
    target_sequence: u64,
    /// The slot that was ACTIVE when the install began — the rollback
    /// target (the register decides whether that target is legal).
    previous_active_slot: Option<OtaSlotId>,
    /// The gate's verdict, set by a passing `verify` (the ONLY thing that
    /// may proceed to activation).
    verified: Option<VerifiedExtendedUpdate>,
}

impl OtaSlotAttempt {
    /// The current flow state.
    pub fn state(&self) -> UpdateProgressState {
        self.session.state()
    }

    /// The attempt id (the P18.07 evidence-chain key).
    pub fn attempt_id(&self) -> &str {
        self.session.attempt_id()
    }

    /// The target version (diagnostics).
    pub fn target_version(&self) -> &str {
        &self.target_version
    }

    /// The target's anti-rollback sequence (diagnostics).
    pub fn target_sequence(&self) -> u64 {
        self.target_sequence
    }

    /// The status notification for the CURRENT state (built on demand —
    /// nothing is emitted internally).
    pub fn status_notification(&self) -> UpdateStatusNotification {
        self.session.status_notification()
    }
}

// --- The slot manager ---------------------------------------------------------------------

/// The device-side A/B slot manager for ONE edge: the slot table, the
/// active-slot pointer, and the attempt lifecycle, all governed by the
/// EXISTING register/activation rules. Deterministic and socket-free: the
/// injected clock stamps every audit line and every status report, and
/// nothing here reads wall time, opens a socket, or writes a file (the
/// register itself is opened by the composition and passed in).
pub struct OtaSlotManager {
    active_slot: OtaSlotId,
    slot_a: Option<OtaSlotRecord>,
    slot_b: Option<OtaSlotRecord>,
    activation: UpdateActivation,
    audit: Arc<dyn AuditSink>,
    clock: Arc<dyn Clock + Send + Sync>,
}

impl OtaSlotManager {
    /// Provisions the manager: slot A carries the factory image (active,
    /// healthy, sequence 0 — never gate-verified), slot B is empty. The
    /// provisioning is audited (`EDGE_OTA_SLOT_PROVISIONED`).
    pub fn new(
        initial_version: &str,
        audit: Arc<dyn AuditSink>,
        clock: Arc<dyn Clock + Send + Sync>,
    ) -> Result<Self, OtaSlotsError> {
        if initial_version.trim().is_empty() {
            return Err(OtaSlotsError::InvalidInput {
                reason: "the factory image version is mandatory".to_string(),
            });
        }
        let manager = Self {
            active_slot: OtaSlotId::A,
            slot_a: Some(OtaSlotRecord {
                slot: OtaSlotId::A,
                version: initial_version.to_string(),
                anti_rollback_sequence: 0,
                healthy: true,
            }),
            slot_b: None,
            activation: UpdateActivation::new(audit.clone(), clock.clone()),
            audit,
            clock,
        };
        manager.audit_event(
            "EDGE_OTA_SLOT_PROVISIONED",
            format!(
                "slot-a carries the factory image version={initial_version} sequence=0 \
                 (never gate-verified — the factory slot's rollback legality is the \
                 register's decision, see OTA_SLOTS_STATEMENT); slot-b empty; the register \
                 is the source of truth for the active version"
            ),
        );
        Ok(manager)
    }

    /// The ACTIVE slot id.
    pub fn active_slot(&self) -> OtaSlotId {
        self.active_slot
    }

    /// The INACTIVE slot id (the install target).
    pub fn inactive_slot(&self) -> OtaSlotId {
        self.active_slot.other()
    }

    /// One slot's record (None = the slot is empty).
    pub fn slot_record(&self, slot: OtaSlotId) -> Option<&OtaSlotRecord> {
        match slot {
            OtaSlotId::A => self.slot_a.as_ref(),
            OtaSlotId::B => self.slot_b.as_ref(),
        }
    }

    /// The ACTIVE slot's record (a provisioned manager always has one).
    pub fn active_record(&self) -> Result<&OtaSlotRecord, OtaSlotsError> {
        self.slot_record(self.active_slot).ok_or_else(|| OtaSlotsError::NoPreviousSlot {
            reason: format!("the active slot {} carries no record", self.active_slot.name()),
        })
    }

    /// Opens ONE update attempt: the P18.04 flow machine in Idle, moved to
    /// Downloading with THE BLOCKING RULE (`in_flight` = the device's
    /// ISSUED/TRACKING command count — a non-zero count refuses, the
    /// P18.15 discipline consumed through the existing session rule).
    /// Audited (`EDGE_OTA_SLOT_DOWNLOAD_BEGUN`).
    pub fn open_attempt(
        &self,
        attempt_id: &str,
        device_id: &str,
        target_version: &str,
        target_sequence: u64,
        in_flight_commands: usize,
        at_unix_millis: i64,
    ) -> Result<OtaSlotAttempt, OtaSlotsError> {
        let mut session = UpdateSessionStateMachine::new(
            attempt_id,
            device_id,
            target_version,
            target_sequence,
            at_unix_millis,
        )
        .map_err(flow_refused)?;
        session
            .begin_download(in_flight_commands, at_unix_millis)
            .map_err(flow_refused)?;
        self.audit_event(
            "EDGE_OTA_SLOT_DOWNLOAD_BEGUN",
            format!(
                "attempt='{}' device='{}' target_version={} target_sequence={} slot={} \
                 in_flight={} — the download path began (nothing has been verified; no \
                 manifest has been trusted)",
                attempt_id,
                device_id,
                target_version,
                target_sequence,
                self.inactive_slot().name(),
                in_flight_commands,
            ),
        );
        Ok(OtaSlotAttempt {
            session,
            target_version: target_version.to_string(),
            target_sequence,
            previous_active_slot: None,
            verified: None,
        })
    }

    /// `Downloading → Verifying` (the bundle bytes are complete; nothing
    /// has been trusted yet). Audited.
    pub fn download_complete(
        &self,
        attempt: &mut OtaSlotAttempt,
        at_unix_millis: i64,
    ) -> Result<(), OtaSlotsError> {
        attempt.session.download_complete(at_unix_millis).map_err(flow_refused)?;
        self.audit_event(
            "EDGE_OTA_SLOT_DOWNLOAD_COMPLETE",
            format!(
                "attempt='{}' — the bundle bytes are complete; verification begins (the \
                 P9.34 verify-before-any-write ordering)",
                attempt.attempt_id(),
            ),
        );
        Ok(())
    }

    /// `Verifying → VerifiedPendingInstall` on a FULL verification pass,
    /// or `Verifying → UpdateFailed` on a refusal. The verdict is the
    /// EXISTING extended gate's (its own audit lines ride inside — the
    /// P9.34 layer AND the extended layer, verbatim); the counter has
    /// advanced DURABLY on a pass. Audited at the slot level
    /// (`EDGE_OTA_SLOT_VERIFIED` / `EDGE_OTA_SLOT_VERIFY_REFUSED`); the
    /// refusal carries the gate's stable code verbatim.
    pub fn verify(
        &self,
        attempt: &mut OtaSlotAttempt,
        gate: &ExtendedUpdateGate,
        manifest: &ExtendedUpdateManifest,
        bundle: &[u8],
        at_unix_millis: i64,
    ) -> Result<(), OtaSlotsError> {
        if attempt.session.state() != UpdateProgressState::Verifying {
            return Err(flow_error(attempt.session.state(), UpdateProgressState::Verifying));
        }
        match gate.verify_fleet_bundle(Some(manifest), bundle) {
            Ok(verified) => {
                attempt.session.verify_passed(at_unix_millis).map_err(flow_refused)?;
                attempt.verified = Some(verified);
                self.audit_event(
                    "EDGE_OTA_SLOT_VERIFIED",
                    format!(
                        "attempt='{}' — the extended gate PASSED (the anti-rollback counter \
                         advanced durably to {}; the install path may proceed)",
                        attempt.attempt_id(),
                        attempt.target_sequence,
                    ),
                );
                Ok(())
            }
            Err(error) => {
                // The flow moves to the explicit verification-refused
                // failure state; the gate's refusal audit already fired.
                let _ = attempt.session.verify_refused(at_unix_millis);
                let refused = OtaSlotsError::VerifyRefused {
                    code: error.name().to_string(),
                    reason: error.to_string(),
                };
                self.audit_event(
                    "EDGE_OTA_SLOT_VERIFY_REFUSED",
                    format!(
                        "attempt='{}' refusal code={} :: {refused} — the flow reports \
                         UPDATE_FAILED with the verification-refused failure state",
                        attempt.attempt_id(),
                        error.name(),
                    ),
                );
                Err(refused)
            }
        }
    }

    /// `VerifiedPendingInstall → Installing → HealthCheckPending`: the
    /// verified bundle is written into the INACTIVE slot (the A/B layout;
    /// the active slot is untouched until the health check confirms).
    /// Audited (`EDGE_OTA_SLOT_INSTALL_BEGUN`, `EDGE_OTA_SLOT_INSTALLED`).
    pub fn install(
        &mut self,
        attempt: &mut OtaSlotAttempt,
        at_unix_millis: i64,
    ) -> Result<(), OtaSlotsError> {
        if attempt.session.state() != UpdateProgressState::VerifiedPendingInstall {
            return Err(flow_error(
                attempt.session.state(),
                UpdateProgressState::Installing,
            ));
        }
        let target_slot = self.inactive_slot();
        attempt.session.begin_install(at_unix_millis).map_err(flow_refused)?;
        self.audit_event(
            "EDGE_OTA_SLOT_INSTALL_BEGUN",
            format!(
                "attempt='{}' target_slot={} version={} sequence={} — the verified bundle \
                 is written into the INACTIVE slot (the active slot {} stays untouched; \
                 the slot WRITE itself is a device-side contract, P6.34)",
                attempt.attempt_id(),
                target_slot.name(),
                attempt.target_version,
                attempt.target_sequence,
                self.active_slot.name(),
            ),
        );
        // The slot write completed (the device-side contract acknowledged;
        // the record starts unhealthy until the health check confirms).
        let record = OtaSlotRecord {
            slot: target_slot,
            version: attempt.target_version.clone(),
            anti_rollback_sequence: attempt.target_sequence,
            healthy: false,
        };
        match target_slot {
            OtaSlotId::A => self.slot_a = Some(record),
            OtaSlotId::B => self.slot_b = Some(record),
        }
        // The rollback target is the slot that was active at install time.
        attempt.previous_active_slot = Some(self.active_slot);
        attempt.session.install_complete(at_unix_millis).map_err(flow_refused)?;
        self.audit_event(
            "EDGE_OTA_SLOT_INSTALLED",
            format!(
                "attempt='{}' target_slot={} — the slot write completed; the device \
                 reboots into the new slot and runs the health check (watchdog/boot-\
                 counter evidence is a device-side contract, P6.34)",
                attempt.attempt_id(),
                target_slot.name(),
            ),
        );
        Ok(())
    }

    /// `VerifiedPendingInstall → UpdateFailed` with the explicit
    /// install-failure state: the slot WRITE failed. THE A/B INVARIANT
    /// (audited): the active slot is UNTOUCHED — the previous version was
    /// never left, which IS the install-failure rollback-in-place. The
    /// install attempt itself is audited with the failure (FIM-F12.05).
    pub fn install_failed(
        &mut self,
        attempt: &mut OtaSlotAttempt,
        at_unix_millis: i64,
    ) -> Result<(), OtaSlotsError> {
        if attempt.session.state() != UpdateProgressState::VerifiedPendingInstall {
            return Err(flow_error(
                attempt.session.state(),
                UpdateProgressState::UpdateFailed,
            ));
        }
        attempt.session.install_failed(at_unix_millis).map_err(flow_refused)?;
        self.audit_event(
            "EDGE_OTA_SLOT_INSTALL_FAILED",
            format!(
                "attempt='{}' — the slot write FAILED; the active slot {} is UNTOUCHED \
                 (the previous version was never left — the A/B rollback-in-place) and \
                 the flow reports UPDATE_FAILED/{UPDATE_FAILURE_INSTALL_FAILED}",
                attempt.attempt_id(),
                self.active_slot.name(),
            ),
        );
        Ok(())
    }

    /// `HealthCheckPending → Active`: the health evidence confirmed the
    /// new version — the register records the activation (the EXISTING
    /// `UpdateActivation::activate`; its `EDGE_UPDATE_ACTIVATED` line rides
    /// inside) and the active slot switches to the new slot. Audited at
    /// the slot level (`EDGE_OTA_SLOT_HEALTH_CONFIRMED`,
    /// `EDGE_OTA_SLOT_ACTIVATED`). An activation REFUSAL (e.g. the version
    /// is already active) is recorded and the auto-rollback path runs
    /// before the error surfaces (never swallowed).
    pub fn health_confirmed(
        &mut self,
        attempt: &mut OtaSlotAttempt,
        ledger: &FileUpdateLedger,
        minimum_version_floor: &str,
        at_unix_millis: i64,
    ) -> Result<UpdateLedgerRecord, OtaSlotsError> {
        if attempt.session.state() != UpdateProgressState::HealthCheckPending {
            return Err(flow_error(
                attempt.session.state(),
                UpdateProgressState::Active,
            ));
        }
        let verified = attempt.verified.clone().ok_or_else(|| OtaSlotsError::FlowRefused {
            reason: "the attempt carries no gate verdict — health confirmation without a \
                     verification pass is a composition defect"
                .to_string(),
        })?;
        self.audit_event(
            "EDGE_OTA_SLOT_HEALTH_CONFIRMED",
            format!(
                "attempt='{}' — the health evidence confirmed version {} (the evidence \
                 contract is device-side; the platform records the verdict)",
                attempt.attempt_id(),
                attempt.target_version,
            ),
        );
        // The register-level activation FIRST (the source of truth moves
        // here); the flow machine only moves to Active AFTER the register
        // accepted — a refused activation must never leave the flow
        // claiming a success the register does not carry.
        let record = match self.activation.activate(ledger, &verified.base) {
            Ok(record) => record,
            Err(error) => {
                self.audit_event(
                    "EDGE_OTA_SLOT_ACTIVATION_REFUSED",
                    format!(
                        "attempt='{}' refusal code={} :: {error} — the register refused \
                         the activation; the auto-rollback path begins",
                        attempt.attempt_id(),
                        error.name(),
                    ),
                );
                // The session moves into RollingBack and the rollback
                // runs (the previous slot is still the register's active
                // version in the already-active case — the rollback
                // refusal then lands the attempt in the explicit
                // rollback-failed terminal state). The rollback reason is
                // mandatory (the register rule) — it names the refusal.
                let _ = attempt.session.health_failed(at_unix_millis);
                let rollback_reason = format!("the register refused the activation: {error}");
                let _ = self.execute_rollback(
                    attempt,
                    ledger,
                    minimum_version_floor,
                    &rollback_reason,
                    None,
                    at_unix_millis,
                );
                return Err(OtaSlotsError::ActivationRefused {
                    reason: error.to_string(),
                });
            }
        };
        attempt
            .session
            .health_confirmed(at_unix_millis)
            .map_err(flow_refused)?;
        // The active slot switches to the installed (now healthy) slot.
        let new_active = self.inactive_slot();
        if let Some(record) = self.slot_record_mut(new_active) {
            record.healthy = true;
        }
        self.active_slot = new_active;
        self.audit_event(
            "EDGE_OTA_SLOT_ACTIVATED",
            format!(
                "attempt='{}' — the active slot is now {} (version {}; the register's \
                 activation record is the source of truth, the slot table is the \
                 device-side view)",
                attempt.attempt_id(),
                new_active.name(),
                attempt.target_version,
            ),
        );
        Ok(record)
    }

    /// `HealthCheckPending → RollingBack → RolledBack`: the health
    /// evidence FAILED the new version — the AUTO-ROLLBACK follows the
    /// REGISTER'S ACTUAL POLICY (the three-way resolution in
    /// [`Self::execute_rollback`]): when the failed version was never
    /// activated (the common A/B case) the register already carries the
    /// previous version and the slot-level boot rollback completes with
    /// NO register record; when the failed version HAD been activated the
    /// EXISTING register rules run verbatim; and a never-activated target
    /// (the factory slot on a first-ever update) is the recorded honest
    /// conflict (`UPDATE_FAILURE_ROLLBACK_FAILED`). A rollback NEVER
    /// re-presents a manifest and the counter NEVER rewinds (see
    /// [`OTA_SLOTS_STATEMENT`]). The register's `EDGE_UPDATE_ROLLBACK`
    /// line rides inside on the register-record path; the slot level
    /// audits the beginning, the completion (and its register-record
    /// status), and — the honest conflict — the refusal
    /// (`OTA_SLOTS_ROLLBACK_IMPOSSIBLE`; the flow reports
    /// UPDATE_FAILED/UPDATE_FAILURE_ROLLBACK_FAILED and the register is
    /// untouched).
    pub fn health_failed_rollback(
        &mut self,
        attempt: &mut OtaSlotAttempt,
        ledger: &FileUpdateLedger,
        minimum_version_floor: &str,
        reason: &str,
        override_record: Option<&RollbackOverride>,
        at_unix_millis: i64,
    ) -> Result<Option<UpdateLedgerRecord>, OtaSlotsError> {
        if attempt.session.state() != UpdateProgressState::HealthCheckPending {
            return Err(flow_error(
                attempt.session.state(),
                UpdateProgressState::RollingBack,
            ));
        }
        attempt.session.health_failed(at_unix_millis).map_err(flow_refused)?;
        self.audit_event(
            "EDGE_OTA_SLOT_HEALTH_FAILED_AUTO_ROLLBACK",
            format!(
                "attempt='{}' — the health evidence FAILED version {}; the auto-rollback \
                 to the previous slot begins (reason='{reason}')",
                attempt.attempt_id(),
                attempt.target_version,
            ),
        );
        self.execute_rollback(
            attempt,
            ledger,
            minimum_version_floor,
            reason,
            override_record,
            at_unix_millis,
        )
    }

    /// The rollback execution (state: RollingBack). THE REGISTER'S ACTUAL
    /// POLICY, three-way (recorded):
    ///
    /// 1. the register's active version ALREADY IS the previous slot's
    ///    version (the common A/B case: the failed version was never
    ///    activated, because activation happens at health confirmation) —
    ///    NO register record exists or is possible under the crate's rules
    ///    (a re-activation of the RUNNING version is refused as
    ///    not-behind-active, `UPDATE_ROLLBACK_TARGET_NOT_BEHIND_ACTIVE`);
    ///    the slot-level boot rollback completes, is REPORTED, and the
    ///    register stays untouched (it already carries the truth);
    /// 2. the register's active version is the FAILED version (a
    ///    post-activation rollback — a composition that activates at
    ///    install time): the EXISTING register rules run VERBATIM
    ///    (previously-activated + behind-active + the never-silent-
    ///    downgrade floor rule with its recorded override);
    /// 3. NEITHER (the factory slot on a first-ever update — the target
    ///    was never register-activated): the register refuses, THE honest
    ///    conflict — recorded, reported as UPDATE_FAILURE_ROLLBACK_FAILED,
    ///    the register untouched, never weakened.
    fn execute_rollback(
        &mut self,
        attempt: &mut OtaSlotAttempt,
        ledger: &FileUpdateLedger,
        minimum_version_floor: &str,
        reason: &str,
        override_record: Option<&RollbackOverride>,
        at_unix_millis: i64,
    ) -> Result<Option<UpdateLedgerRecord>, OtaSlotsError> {
        let previous_slot = attempt
            .previous_active_slot
            .ok_or_else(|| OtaSlotsError::NoPreviousSlot {
                reason: "the attempt never began an install — there is no previous slot to \
                         roll back to"
                    .to_string(),
            })?;
        let previous_version = self
            .slot_record(previous_slot)
            .map(|record| record.version.clone())
            .ok_or_else(|| OtaSlotsError::NoPreviousSlot {
                reason: format!(
                    "the previous slot {} carries no record to roll back to",
                    previous_slot.name()
                ),
            })?;
        // The FAILED slot is the install target (the slot that was
        // inactive when the attempt began) — it keeps its unhealthy record
        // either way (the slot history is the device-side evidence).
        let failed_slot = previous_slot.other();
        // THE REGISTER'S ACTUAL POLICY (three-way — see the method doc).
        let register_active = ledger.active_version();
        if register_active.as_deref() == Some(previous_version.as_str()) {
            // Case 1: the register already carries the rollback target —
            // the failed version was never activated. No register record
            // exists or is possible under the crate's rules; the
            // slot-level boot rollback completes and is REPORTED.
            if let Some(record) = self.slot_record_mut(failed_slot) {
                record.healthy = false;
            }
            attempt.session.rollback_complete(at_unix_millis).map_err(flow_refused)?;
            self.audit_event(
                "EDGE_OTA_SLOT_ROLLED_BACK",
                format!(
                    "attempt='{}' — version {} is re-activated at the slot level (boot the \
                     previous slot); the register's active version was NEVER the failed \
                     version, so no register rollback record exists or is possible under \
                     the recorded rules (a re-activation of the running version is refused \
                     as not-behind-active); the failed slot {} keeps its unhealthy record; \
                     the status report carries UPDATE_ROLLED_BACK (the rollback is \
                     reported, never silent); the anti-rollback counter never rewinds",
                    attempt.attempt_id(),
                    previous_version,
                    failed_slot.name(),
                ),
            );
            return Ok(None);
        }
        // Cases 2 and 3: the register's rules decide VERBATIM.
        let record = match self.activation.rollback(
            ledger,
            minimum_version_floor,
            &previous_version,
            reason,
            override_record,
        ) {
            Ok(record) => record,
            Err(error) => {
                let _ = attempt.session.rollback_failed(at_unix_millis);
                self.audit_event(
                    "EDGE_OTA_SLOT_ROLLBACK_REFUSED",
                    format!(
                        "attempt='{}' refusal code={} :: {error} — the rollback is \
                         IMPOSSIBLE under the recorded rules; the flow reports \
                         UPDATE_FAILED/{UPDATE_FAILURE_ROLLBACK_FAILED} and the register \
                         is untouched (operator intervention; the register shows the \
                         last good state)",
                        attempt.attempt_id(),
                        error.name(),
                    ),
                );
                return Err(OtaSlotsError::RollbackImpossible {
                    reason: error.to_string(),
                });
            }
        };
        // The register accepted the rollback (the post-activation case):
        // the manager's active-slot pointer follows the register (the
        // source of truth) — the failed slot's pointer slot re-activates.
        self.active_slot = previous_slot;
        if let Some(record) = self.slot_record_mut(failed_slot) {
            record.healthy = false;
        }
        attempt.session.rollback_complete(at_unix_millis).map_err(flow_refused)?;
        self.audit_event(
            "EDGE_OTA_SLOT_ROLLED_BACK",
            format!(
                "attempt='{}' — version {} is re-activated THROUGH THE REGISTER (the \
                 failed version had been activated; the register's rollback record is the \
                 source of truth); the failed slot {} keeps its unhealthy record; the \
                 status report carries UPDATE_ROLLED_BACK (the rollback is reported, \
                 never silent); the anti-rollback counter never rewinds",
                attempt.attempt_id(),
                previous_version,
                failed_slot.name(),
            ),
        );
        Ok(Some(record))
    }

    /// The status report for the attempt's CURRENT state (the P18.04
    /// status-notification leg). Audited (`EDGE_OTA_SLOT_STATUS_REPORTED`)
    /// with the notification's summary line — every attempt's every
    /// report lands in the audit trail (FIM-F12.05).
    pub fn status_report(&self, attempt: &OtaSlotAttempt) -> UpdateStatusNotification {
        let notification = attempt.status_notification();
        self.audit_event(
            "EDGE_OTA_SLOT_STATUS_REPORTED",
            format!("{} (attempt='{}')", notification.summary(), attempt.attempt_id()),
        );
        notification
    }

    fn slot_record_mut(&mut self, slot: OtaSlotId) -> Option<&mut OtaSlotRecord> {
        match slot {
            OtaSlotId::A => self.slot_a.as_mut(),
            OtaSlotId::B => self.slot_b.as_mut(),
        }
    }

    fn audit_event(&self, name: &'static str, detail: String) {
        let event = EdgeAuditEvent {
            recorded_at: self.clock.now_ingest_time(),
            kind: EdgeAuditKind::UpdateLifecycle,
            detail: format!("{name}: {detail}"),
        };
        if let Err(error) = self.audit.record(event) {
            tracing::error!(%error, audit_event = name, "ota-slot-manager audit write FAILED");
        }
    }
}

/// Maps a flow-machine refusal into the slot error (the session error's
/// own text rides verbatim — one flow vocabulary).
fn flow_refused(error: UpdateSessionError) -> OtaSlotsError {
    OtaSlotsError::FlowRefused {
        reason: error.to_string(),
    }
}

/// The out-of-order-guard error for the slot methods (the same named
/// refusal the flow machine itself would emit).
fn flow_error(from: UpdateProgressState, to: UpdateProgressState) -> OtaSlotsError {
    OtaSlotsError::FlowRefused {
        reason: UpdateSessionError::IllegalTransition {
            from: from.name(),
            to: to.name(),
        }
        .to_string(),
    }
}

// --- Tests (WRITTEN-UNRUN; execution is a human gate) -----------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::update_verification::{
        UpdateLedgerEventKind, UpdateVerificationError, UPDATE_SLOT_DECISION,
    };
    use std::sync::Mutex;

    // The fixed clock and collecting sink are the in-module test
    // discipline (the injected-clock rule; no wall time anywhere).
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
        fn record(&self, event: EdgeAuditEvent) -> Result<(), crate::ports::EdgeError> {
            self.events
                .lock()
                .expect("poisoned test sink mutex")
                .push(event.detail);
            Ok(())
        }
    }

    fn temp_register(tag: &str) -> FileUpdateLedger {
        let dir = std::env::temp_dir()
            .join(format!("flexcore-f12-slots-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("update.register");
        let _ = std::fs::remove_file(&path);
        FileUpdateLedger::open(&path).expect("register opens")
    }

    #[test]
    fn provisioning_is_the_factory_slot_and_the_register_stays_the_truth() {
        let sink = Arc::new(CollectingAuditSink::default());
        let manager = OtaSlotManager::new(
            "0.9.0",
            sink.clone() as Arc<dyn AuditSink>,
            Arc::new(FixedClock(1_000)),
        )
        .expect("provisions");
        assert_eq!(manager.active_slot(), OtaSlotId::A);
        assert_eq!(manager.inactive_slot(), OtaSlotId::B);
        let record = manager.active_record().expect("the factory slot is provisioned");
        assert_eq!(record.version, "0.9.0");
        assert_eq!(record.anti_rollback_sequence, 0, "the factory slot was never gate-verified");
        assert!(record.healthy);
        assert!(manager.slot_record(OtaSlotId::B).is_none(), "slot B is empty");
        assert!(sink.lines().iter().any(|l| l.contains("EDGE_OTA_SLOT_PROVISIONED")));
        // The slot decision statement is the consumed truth (cited, never
        // forked).
        assert!(UPDATE_SLOT_DECISION.contains("the register is the source of truth"));
    }

    #[test]
    fn the_commands_in_flight_rule_blocks_the_attempt() {
        let sink = Arc::new(CollectingAuditSink::default());
        let manager = OtaSlotManager::new(
            "0.9.0",
            sink.clone() as Arc<dyn AuditSink>,
            Arc::new(FixedClock(1_000)),
        )
        .expect("provisions");
        // The P18.15 blocking rule rides through the existing session rule.
        assert!(matches!(
            manager.open_attempt("attempt-1", "device-1", "1.2.0", 5, 2, 1_100),
            Err(OtaSlotsError::FlowRefused { .. })
        ));
        assert!(
            manager
                .open_attempt("attempt-1", "device-1", "1.2.0", 5, 0, 1_100)
                .is_ok(),
            "a drained device opens the attempt"
        );
    }

    #[test]
    fn out_of_order_slot_methods_are_named_refusals() {
        let sink = Arc::new(CollectingAuditSink::default());
        let manager = OtaSlotManager::new(
            "0.9.0",
            sink.clone() as Arc<dyn AuditSink>,
            Arc::new(FixedClock(1_000)),
        )
        .expect("provisions");
        let mut attempt = manager
            .open_attempt("attempt-1", "device-1", "1.2.0", 5, 0, 1_100)
            .expect("opens");
        // Install before download/verify is a named refusal.
        assert!(matches!(
            manager.install(&mut attempt, 1_200),
            Err(OtaSlotsError::FlowRefused { .. })
        ));
        let ledger = temp_register("out-of-order");
        // Health confirmation without verification is a named refusal.
        assert!(matches!(
            manager.health_confirmed(&mut attempt, &ledger, 1_300),
            Err(OtaSlotsError::FlowRefused { .. })
        ));
    }

    #[test]
    fn the_statement_and_the_failure_state_names_are_pinned() {
        assert_eq!(UPDATE_FAILURE_ROLLBACK_FAILED, "UPDATE_FAILURE_ROLLBACK_FAILED");
        assert!(OTA_SLOTS_STATEMENT.contains("NEVER re-presents"));
        assert!(OTA_SLOTS_STATEMENT.contains("NEVER rewinds"));
        assert!(OTA_SLOTS_STATEMENT.contains("PENDING PILOT HARDWARE SPEC"));
        assert!(OTA_SLOTS_STATEMENT.contains("no device has ever installed anything"));
        // The honest-conflict error name is stable.
        assert_eq!(
            OtaSlotsError::RollbackImpossible { reason: String::new() }.name(),
            "OTA_SLOTS_ROLLBACK_IMPOSSIBLE"
        );
    }

    #[test]
    fn the_activation_record_kinds_are_the_register_ones() {
        // The slot manager only ever appends through the register's own
        // activation/rollback paths — the closed register vocabulary is
        // consumed, never extended.
        assert_eq!(UpdateLedgerEventKind::Activation.name(), "UPDATE_ACTIVATED");
        assert_eq!(UpdateLedgerEventKind::Rollback.name(), "UPDATE_ROLLBACK");
    }

    // A pinned reference so the verification-error import stays honest:
    // the impossible-rollback case carries the register's stable name.
    #[test]
    fn the_rollback_refusal_code_is_the_register_one() {
        let error = UpdateVerificationError::RollbackTargetNeverActivated {
            version: "0.9.0".to_string(),
        };
        assert_eq!(error.name(), "UPDATE_ROLLBACK_TARGET_NEVER_ACTIVATED");
    }
}
