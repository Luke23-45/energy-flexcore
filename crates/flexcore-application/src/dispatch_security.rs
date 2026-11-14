// P4.28 — Dispatch security controls (§17.3–§17.4, §17.5 inv. 8, §16.2,
//         §14.3 field 15; ledger P4.28; decision record:
//         `ledger/phase-04-grid-feasibility-dispatch/28_security_controls.md`).
//
// This module closes the P4.28 gaps the P4.16/P4.14 surfaces deliberately
// left open (each gap is cited in the decision record; nothing here edits
// those surfaces):
//
// 1. **Payload-bound signing + verification.** P4.16's
//    [`SimulationKeyring`](crate::dispatch_simulator::SimulationKeyring)
//    signs `sim-sig:<key_id>:<command_id>` — a traceability reference that
//    does NOT bind the payload, so a tampered payload under the same
//    command ID is indistinguishable from the original. The ledger's
//    tamper test ("tampered payload → signature MUST fail") needs a
//    signature that covers the envelope contents. [`sign_dispatch_payload`]
//    produces `sim-sig:<key_id>:<command_id>:fnv1a64:<digest>` over the
//    canonical [`DispatchPayloadBinding`] (command, asset, sequence,
//    quantity, value, transport identity); [`verify_dispatch_payload`]
//    recomputes and compares, with the NAMED failure surface
//    ([`DispatchSecurityError`]: missing, malformed, foreign-key,
//    tampered). THE DIGEST IS `fnv1a64` — the same NON-cryptographic
//    pattern the repo already uses for model-content hashes; it is an
//    integrity CHECKSUM, not a security primitive, and the whole layer
//    inherits the P4.16 keyring's dev/SIMULATION labelling
//    ([`DISPATCH_SECURITY_STATEMENT`]). No cryptographic property is
//    claimed; a production signing identity is a Stage 6/9 qualification
//    matter.
// 2. **Sequence/replay enforcement at the receive point.** P4.14's
//    `out_of_sequence_rejection` is the pure creation-side rule. The
//    receive side (the "simulator adapter verifies before accepting"
//    half of the ledger step) gets [`SequenceGuard`]: a per-asset/channel
//    monotonic guard whose refusals DISTINGUISH a replayed sequence
//    (equal to the last accepted) from an out-of-order one (lower), and
//    never update their state on a refusal.
// 3. **The verifying transport.** [`VerifyingDispatchTransport`] wraps any
//    [`SimulatorDispatchTransport`] and is the composition-root pattern
//    for receive-side verification: verify the envelope's payload-bound
//    signature, then admit its sequence, and ONLY THEN hand it to the
//    inner transport. A refused envelope is an application-layer NACK
//    (the frozen ISSUED → FAILED trigger) and NEVER reaches the inner
//    transport. Recorded wiring note: the P4.16 adapter's issue path
//    still records the legacy keyring reference; moving its issued
//    envelopes to the payload-bound signature is composition-root/Stage-6
//    wiring (recorded in the decision record, not silently assumed).
// 4. **Tenant isolation, audited.** The tenant-scoped store seam and the
//    cross-tenant `NotFound` denials already exist (P0.16 rule 5;
//    P4.16/P4.23 tests). What the P4.23 decision record left open was the
//    AUDIT half of the denial ("its AuditSink append is the P4.28/P4.30
//    wiring"). [`TenantDenialAuditRecord`] +
//    [`record_tenant_denial`] + the [`SecurityAuditSink`] port close the
//    record side (the port is declared HERE because flexcore-application
//    cannot depend on flexcore-audit — the same seam pattern as
//    `fallback_policy.rs`'s `FallbackAuditSink`; the P4.30 persistence
//    wiring consumes these records). Existence still never leaks: the
//    audit record is built by the PLATFORM from its own records for the
//    denied CALLER's attempt, never returned to the caller.
// 5. **Secrets.** No secret material exists or may exist here
//    ([`SECRETS_DEPLOYMENT_REQUIREMENT_STATEMENT`]): the keyring carries
//    key IDENTIFIERS only; short-lived credentials and rotation are
//    recorded as later-stage deployment requirements, not implemented
//    against any real secret store.
//
// ## Control status
//
// Code-level controls in the simulation layer only (CONTEXT.md D6: no OT
// boundary crossed; P4.28–P4.29 are code-level controls). The entire 0.x
// line is simulation-only (§21.4); nothing here authorises field control
// or device writes. FlexCore performs no primary frequency response,
// synthetic inertia, relay protection, inverter inner-loop control, or
// sub-cycle control (§14.5). mTLS is NOT implemented: the recorded
// simulation stand-in for §14.3 field 15's authenticated transport
// identity remains the P4.16 `sim://` identity string; real transport
// security is a Stage 6 deployment matter.

use std::collections::HashMap;
use std::sync::Mutex;

use flexcore_domain::dispatch_command::DispatchCommand;
use flexcore_domain::ids::{AssetId, CommandId, TenantId};
use flexcore_domain::tenant_context::TenantContext;

use crate::dispatch_simulator::{SimulatorDispatchEnvelope, SimulatorDispatchTransport};

/// The P4.28 control statement (see the module header). Recorded so tests
/// and reviewers can pin what IS and IS NOT claimed.
pub const DISPATCH_SECURITY_STATEMENT: &str = "the P4.28 dispatch security controls are \
     code-level controls in the SIMULATION layer (§17.3–§17.4): payload-bound simulation \
     signing references (fnv1a64, non-cryptographic), monotonic per-asset/channel sequence \
     enforcement, cross-tenant denial records, and the verifying-transport pattern; the \
     signing identity is the dev/SIMULATION keyring and MUST be replaced by a Stage 6/9-\
     qualified signing identity before any non-simulation use; mTLS is NOT implemented — \
     the §14.3 transport identity stays the recorded sim:// stand-in; no certification, \
     no safety claim, and no transport-security claim is made";

/// The recorded secrets/deployment-requirements statement (ledger P4.28
/// step 4): what is a requirement for later stages versus what exists here.
pub const SECRETS_DEPLOYMENT_REQUIREMENT_STATEMENT: &str = "no secrets in command payloads, \
     logs, or fixtures (§17.3); short-lived credentials and signing-key/certificate rotation \
     are DEPLOYMENT REQUIREMENTS for Stage 6+ — recorded here, NOT implemented: no real \
     secret store is consulted in the simulation line (no Phase 1 secret-store port exists \
     in this slice — recorded, not assumed); every key in this layer is a non-production \
     simulation key IDENTIFIER; no key material exists in code, tests, or fixtures";

/// The signature-reference scheme prefix (the P4.28 payload-bound form of
/// the P4.16 keyring reference).
pub const PAYLOAD_SIGNATURE_PREFIX: &str = "sim-sig";

/// The digest algorithm label inside a payload-bound signature reference.
/// `fnv1a64` is the repo's existing non-cryptographic hash pattern (model
/// content hashes): an integrity checksum, NOT a security primitive.
pub const SIMULATION_DIGEST_ALG: &str = "fnv1a64";

/// The default control-channel name of the per-asset sequence guard
/// (§14.3 field 10: "per asset or control channel"; a deployment may name
/// more channels — the guard's key is asset + channel).
pub const DEFAULT_DISPATCH_CHANNEL: &str = "dispatch";

// --- The payload binding and its canonical form ---------------------------------------

/// The canonical dispatch-payload fields a P4.28 signature covers. Built
/// from a [`SimulatorDispatchEnvelope`] (the receive-side shape) or
/// directly (the test/attack surface). The value travels as its exact
/// `f64` bit pattern — two bindings differ if and only if any field
/// differs, byte-for-byte.
#[derive(Debug, Clone, PartialEq)]
pub struct DispatchPayloadBinding {
    /// The command the envelope carries.
    pub command_id: CommandId,
    /// The target asset.
    pub asset_id: AssetId,
    /// The monotonic per-asset/channel sequence (§14.3 field 10).
    pub sequence: u64,
    /// The requested quantity label (the frozen `SimulatorTargetValue`
    /// quantity vocabulary).
    pub quantity: &'static str,
    /// The requested value (canonical units; compared by bit pattern).
    pub value: f64,
    /// The authenticated transport identity of the channel (§14.3 field
    /// 15; the recorded `sim://` stand-in).
    pub transport_identity: String,
}

impl DispatchPayloadBinding {
    /// Builds the binding from the envelope's target value.
    pub fn from_envelope(envelope: &SimulatorDispatchEnvelope) -> Self {
        Self {
            command_id: envelope.command_id,
            asset_id: envelope.asset_id,
            sequence: envelope.sequence,
            quantity: envelope.target.quantity(),
            value: envelope.target.value(),
            transport_identity: envelope.transport_identity.clone(),
        }
    }

    /// The canonical string the digest is taken over. Stable field order,
    /// `|` separators; the value renders as its 16-hex-digit bit pattern so
    /// the encoding is total and exact (no float formatting ambiguity).
    pub fn canonical_string(&self) -> String {
        format!(
            "cmd:{}|asset:{}|seq:{}|qty:{}|val:{:016x}|via:{}",
            self.command_id,
            self.asset_id,
            self.sequence,
            self.quantity,
            self.value.to_bits(),
            self.transport_identity,
        )
    }

    /// The `fnv1a64` digest of the canonical string (the recorded
    /// non-cryptographic integrity checksum; NOT a security primitive).
    pub fn digest(&self) -> u64 {
        fnv1a64(self.canonical_string().as_bytes())
    }
}

/// The FNV-1a 64-bit hash (the repo's existing non-cryptographic pattern;
/// deterministic, dependency-free, explicitly NOT a security primitive).
fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

// --- Signing and verification (over the P4.16 keyring) --------------------------------

/// Signs the payload binding with the P4.16 simulation keyring: the
/// payload-bound signature reference
/// `sim-sig:<key_id>:<command_id>:fnv1a64:<16 hex digits>`. Dev/SIMULATION
/// only — no cryptographic property is claimed ([`DISPATCH_SECURITY_STATEMENT`]).
pub fn sign_dispatch_payload(
    keyring: &crate::dispatch_simulator::SimulationKeyring,
    binding: &DispatchPayloadBinding,
) -> String {
    format!(
        "{PAYLOAD_SIGNATURE_PREFIX}:{}:{}:{SIMULATION_DIGEST_ALG}:{:016x}",
        keyring.active_key_id(),
        binding.command_id,
        binding.digest(),
    )
}

/// The named P4.28 verification failures. Every variant is a REFUSAL —
/// nothing is coerced, guessed around, or auto-corrected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchSecurityError {
    /// The presented reference is empty or whitespace-only — no signature
    /// at all (the ledger's "missing signature" case).
    MissingSignature {
        /// The command the unsigned material claimed to carry.
        command_id: String,
    },
    /// The presented reference does not parse as the recorded
    /// payload-bound scheme (`sim-sig:<key>:<command>:fnv1a64:<digest>`).
    MalformedSignature {
        /// The command.
        command_id: String,
        /// The recorded detail (audit text).
        detail: String,
    },
    /// The reference was produced under a DIFFERENT key id than the
    /// verifying keyring's active key — the wrong-key case.
    ForeignKey {
        /// The command.
        command_id: String,
        /// The verifying keyring's active key id.
        expected_key_id: String,
        /// The key id presented in the reference.
        presented_key_id: String,
    },
    /// The signature's integrity digest does not match the binding — the
    /// TAMPERED-PAYLOAD case (any changed field: value, sequence, asset,
    /// transport identity, or the bound command id itself).
    TamperedPayload {
        /// The command.
        command_id: String,
        /// The digest the binding hashes to.
        expected_digest: String,
        /// The digest presented in the reference.
        presented_digest: String,
    },
}

impl std::fmt::Display for DispatchSecurityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DispatchSecurityError::MissingSignature { command_id } => {
                write!(f, "command {command_id} carries no signature — refused")
            }
            DispatchSecurityError::MalformedSignature { command_id, detail } => {
                write!(f, "command {command_id}: malformed signature reference: {detail}")
            }
            DispatchSecurityError::ForeignKey {
                command_id,
                expected_key_id,
                presented_key_id,
            } => write!(
                f,
                "command {command_id}: signature key '{presented_key_id}' is not the \
                 verifying key's '{expected_key_id}' — refused"
            ),
            DispatchSecurityError::TamperedPayload {
                command_id,
                expected_digest,
                presented_digest,
            } => write!(
                f,
                "command {command_id}: payload integrity check failed (presented digest \
                 {presented_digest}, expected {expected_digest}) — the payload does not \
                 match what was signed; refused"
            ),
        }
    }
}

impl std::error::Error for DispatchSecurityError {}

/// Verifies a payload-bound signature reference against the binding and
/// the verifying keyring. `Ok(())` only when the reference parses, names
/// the verifying key, names the SAME command, and its digest matches the
/// binding exactly — i.e. the payload is bit-for-bit what was signed.
pub fn verify_dispatch_payload(
    keyring: &crate::dispatch_simulator::SimulationKeyring,
    binding: &DispatchPayloadBinding,
    presented: &str,
) -> Result<(), DispatchSecurityError> {
    let command_id = binding.command_id.to_string();
    let presented = presented.trim();
    if presented.is_empty() {
        return Err(DispatchSecurityError::MissingSignature { command_id });
    }
    let parts: Vec<&str> = presented.split(':').collect();
    if parts.len() != 5
        || parts[0] != PAYLOAD_SIGNATURE_PREFIX
        || parts[3] != SIMULATION_DIGEST_ALG
    {
        return Err(DispatchSecurityError::MalformedSignature {
            command_id,
            detail: format!(
                "expected {PAYLOAD_SIGNATURE_PREFIX}:<key_id>:<command_id>:\
                 {SIMULATION_DIGEST_ALG}:<digest>"
            ),
        });
    }
    let presented_key = parts[1];
    if presented_key != keyring.active_key_id() {
        return Err(DispatchSecurityError::ForeignKey {
            command_id,
            expected_key_id: keyring.active_key_id().to_string(),
            presented_key_id: presented_key.to_string(),
        });
    }
    if parts[2] != command_id {
        // A reference for a different command id is a payload binding
        // mismatch by definition (recorded under the tampered case; the
        // detail names the field).
        return Err(DispatchSecurityError::TamperedPayload {
            command_id,
            expected_digest: format!("{:016x}", binding.digest()),
            presented_digest: format!("command-id={}", parts[2]),
        });
    }
    let expected = format!("{:016x}", binding.digest());
    if parts[4] != expected {
        return Err(DispatchSecurityError::TamperedPayload {
            command_id,
            expected_digest: expected,
            presented_digest: parts[4].to_string(),
        });
    }
    Ok(())
}

/// The envelope-verifier seam of the verifying transport. Declared so a
/// composition root can verify envelopes with a future (cryptographic)
/// verifier without changing the transport decorator.
pub trait DispatchEnvelopeVerifier {
    /// Verifies one envelope's authenticity. `Ok(())` means the envelope
    /// is accepted for sequence admission and delivery.
    fn verify(&self, envelope: &SimulatorDispatchEnvelope) -> Result<(), DispatchSecurityError>;
}

/// The P4.28 default verifier: the envelope's recorded signature must be a
/// payload-bound reference over the envelope's own fields, verifiable with
/// the supplied keyring. A legacy P4.16 keyring reference
/// (`sim-sig:<key>:<command>` — no digest) is REFUSED: it does not cover
/// the payload, so it cannot prove the envelope untampered (recorded
/// wiring note in the module header).
pub struct PayloadBoundVerifier<'a> {
    keyring: &'a crate::dispatch_simulator::SimulationKeyring,
}

impl<'a> PayloadBoundVerifier<'a> {
    /// Builds the verifier over the verifying (receiving) keyring.
    pub fn new(keyring: &'a crate::dispatch_simulator::SimulationKeyring) -> Self {
        Self { keyring }
    }
}

impl DispatchEnvelopeVerifier for PayloadBoundVerifier<'_> {
    fn verify(&self, envelope: &SimulatorDispatchEnvelope) -> Result<(), DispatchSecurityError> {
        verify_dispatch_payload(
            self.keyring,
            &DispatchPayloadBinding::from_envelope(envelope),
            &envelope.signature_reference,
        )
    }
}

// --- Sequence / replay (ledger P4.28 step 2) -------------------------------------------

/// The per-asset/channel sequence guard (§14.3 field 10; §17.4 replayed-
/// command threat). STRICTLY monotonic: the first admitted sequence is any
/// ≥ 1; every later one must be greater than the last ACCEPTED. A refusal
/// NEVER updates the guard's state — a rejected sequence cannot move the
/// window (fail-closed, no attacker-driven window advance).
#[derive(Debug, Default)]
pub struct SequenceGuard {
    last_accepted: HashMap<String, u64>,
}

/// The guard's refusals (distinguish a replay from an out-of-order
/// sequence — the ledger names both cases).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SequenceRefusal {
    /// The sequence is zero — unsequenced (the P4.16 refusal, at the
    /// receive point).
    InvalidSequence,
    /// The sequence equals the last accepted one — a REPLAY of an already
    /// accepted command.
    Replayed {
        /// The last accepted sequence for the channel.
        last_accepted: u64,
    },
    /// The sequence is lower than the last accepted one — out of order
    /// (never a "late but fine" delivery).
    OutOfOrder {
        /// The last accepted sequence for the channel.
        last_accepted: u64,
    },
}

impl SequenceRefusal {
    /// The stable recorded code (audit/NACK text).
    pub fn code(&self) -> &'static str {
        match self {
            SequenceRefusal::InvalidSequence => "unsequenced",
            SequenceRefusal::Replayed { .. } => "replayed_sequence",
            SequenceRefusal::OutOfOrder { .. } => "out_of_order_sequence",
        }
    }
}

/// The per-channel key of the guard (asset + control channel).
pub fn channel_key(asset_id: &AssetId, channel: &str) -> String {
    format!("{asset_id}/{channel}")
}

impl SequenceGuard {
    /// An empty guard (no channel has accepted anything).
    pub fn new() -> Self {
        Self::default()
    }

    /// Admits (or refuses) one sequence on the channel. On `Ok(())` the
    /// channel's last-accepted advances to `sequence`; on any refusal the
    /// guard state is UNCHANGED.
    pub fn admit(&mut self, channel: &str, sequence: u64) -> Result<(), SequenceRefusal> {
        if sequence == 0 {
            return Err(SequenceRefusal::InvalidSequence);
        }
        match self.last_accepted.get(channel) {
            None => {
                self.last_accepted.insert(channel.to_string(), sequence);
                Ok(())
            }
            Some(&last) if sequence == last => Err(SequenceRefusal::Replayed { last_accepted: last }),
            Some(&last) if sequence < last => {
                Err(SequenceRefusal::OutOfOrder { last_accepted: last })
            }
            Some(&last) => {
                debug_assert!(sequence > last);
                self.last_accepted.insert(channel.to_string(), sequence);
                Ok(())
            }
        }
    }

    /// The last accepted sequence for the channel, if any.
    pub fn last_accepted(&self, channel: &str) -> Option<u64> {
        self.last_accepted.get(channel).copied()
    }
}

// --- The verifying transport (the receive-point pattern) --------------------------------

/// Wraps a [`SimulatorDispatchTransport`] and verifies every envelope
/// BEFORE it reaches the inner (simulator) transport — the composition-
/// root realisation of "the adapter verifies before accepting" on the
/// receive side. Order of refusal (each is an application-layer NACK with
/// a `dispatch-security:`-prefixed reason; the inner transport is NEVER
/// called for a refused envelope, and a NACK maps to the frozen
/// ISSUED → FAILED trigger upstream):
///
/// 1. sequence 0 → NACK `unsequenced`;
/// 2. signature verification fails → NACK `signature_refused` (missing,
///    malformed, wrong key, or tampered payload — the error text names
///    which);
/// 3. the sequence guard refuses (replayed / out of order) → NACK with the
///    refusal code;
/// 4. otherwise the envelope is delivered to the inner transport.
pub struct VerifyingDispatchTransport<'a> {
    inner: &'a dyn SimulatorDispatchTransport,
    verifier: &'a dyn DispatchEnvelopeVerifier,
    guard: Mutex<SequenceGuard>,
}

impl<'a> VerifyingDispatchTransport<'a> {
    /// Wraps `inner`; envelopes are verified with `verifier` and sequence-
    /// admitted per asset on [`DEFAULT_DISPATCH_CHANNEL`].
    pub fn new(
        inner: &'a dyn SimulatorDispatchTransport,
        verifier: &'a dyn DispatchEnvelopeVerifier,
    ) -> Self {
        Self {
            inner,
            verifier,
            guard: Mutex::new(SequenceGuard::new()),
        }
    }

    /// The last accepted sequence for an asset's default dispatch channel
    /// (test/inspection surface).
    pub fn last_accepted_sequence(&self, asset_id: &AssetId) -> Option<u64> {
        self.guard
            .lock()
            .expect("sequence guard")
            .last_accepted(&channel_key(asset_id, DEFAULT_DISPATCH_CHANNEL))
    }
}

impl SimulatorDispatchTransport for VerifyingDispatchTransport<'_> {
    fn deliver(
        &self,
        envelope: &SimulatorDispatchEnvelope,
    ) -> Result<crate::dispatch_simulator::SimulatedAcceptance, crate::dispatch_simulator::SimulatorTransportError>
    {
        use crate::dispatch_simulator::{SimulatedAcceptance, SimulatorTransportFailure};
        let nack = |code: &str, detail: String| {
            Ok(SimulatedAcceptance::Nack {
                reason: format!("dispatch-security: {code}: {detail}"),
            })
        };

        // 1. Unsequenced.
        if envelope.sequence == 0 {
            return nack(
                SequenceRefusal::InvalidSequence.code(),
                format!("command {} carries sequence 0", envelope.command_id),
            );
        }

        // 2. Authenticity (missing / malformed / foreign key / tampered).
        if let Err(error) = self.verifier.verify(envelope) {
            return nack("signature_refused", error.to_string());
        }

        // 3. Sequence admission (replay / out of order). The guard state
        //    advances only on acceptance.
        let key = channel_key(&envelope.asset_id, DEFAULT_DISPATCH_CHANNEL);
        let admission = {
            let mut guard = self.guard.lock().expect("sequence guard");
            guard.admit(&key, envelope.sequence)
        };
        if let Err(refusal) = admission {
            return nack(
                refusal.code(),
                format!(
                    "command {} sequence {} refused on channel {key}",
                    envelope.command_id, envelope.sequence
                ),
            );
        }

        // 4. Verified + sequenced → the inner transport.
        self.inner.deliver(envelope)
    }

    fn observe(
        &self,
        asset_id: AssetId,
        at_unix_millis: i64,
    ) -> Result<Option<crate::dispatch_simulator::SimulatedObservation>, crate::dispatch_simulator::SimulatorTransportError>
    {
        // Telemetry pull is read-only; authenticity applies to the command
        // direction. Tenant scoping of the observe call is the store/telemetry
        // seam's obligation (P0.16 rule 5), not re-decided here.
        self.inner.observe(asset_id, at_unix_millis)
    }
}

// --- Tenant isolation, audited (ledger P4.28 step 3; §16.2) -----------------------------

/// Which denied operation the record covers (the cross-tenant surface the
/// ledger names: command read / approve / issue).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TenantDenialKind {
    /// A cross-tenant command read/load attempt.
    Read,
    /// A cross-tenant approval attempt.
    Approve,
    /// A cross-tenant issue attempt.
    Issue,
}

impl TenantDenialKind {
    /// The stable recorded code.
    pub fn code(self) -> &'static str {
        match self {
            TenantDenialKind::Read => "read",
            TenantDenialKind::Approve => "approve",
            TenantDenialKind::Issue => "issue",
        }
    }
}

/// The audit record of ONE cross-tenant denial (§16.2: audit cross-tenant
/// access denials). Built by the PLATFORM from its own records for the
/// denied caller's attempt — the denied caller receives only the named
/// `NotFound`-shaped refusal (existence never leaks, P0.16 rule 5); this
/// record exists for the audit trail, never as caller-visible output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TenantDenialAuditRecord {
    /// The denied caller's tenant.
    pub caller_tenant_id: TenantId,
    /// The tenant that owns the command record.
    pub owner_tenant_id: TenantId,
    /// The command that was attempted.
    pub command_id: CommandId,
    /// The command's target asset.
    pub asset_id: AssetId,
    /// Which operation was denied.
    pub kind: TenantDenialKind,
    /// The stable denial reference (`tenant-denial:command/<id>@<at>/<kind>`).
    pub denial_reference: String,
    /// The instant of the denial (controlled clock, UTC millis).
    pub at_unix_millis: i64,
}

impl TenantDenialAuditRecord {
    /// The stable audit-text rendering (what the P4.30 audit wiring
    /// persists). Contains identifiers only — no secret material may ever
    /// enter this text (the structural no-secrets test pins it).
    pub fn audit_text(&self) -> String {
        format!(
            "tenant_denial kind={} caller_tenant={} owner_tenant={} command={} asset={} at={} \
             reference={}",
            self.kind.code(),
            self.caller_tenant_id,
            self.owner_tenant_id,
            self.command_id,
            self.asset_id,
            self.at_unix_millis,
            self.denial_reference,
        )
    }
}

/// Builds the denial record for one cross-tenant attempt. `caller` is the
/// DENIED caller's context; `command` is the platform's own record of the
/// command that was attempted.
pub fn record_tenant_denial(
    caller: &TenantContext,
    command: &DispatchCommand,
    kind: TenantDenialKind,
    at_unix_millis: i64,
) -> TenantDenialAuditRecord {
    TenantDenialAuditRecord {
        caller_tenant_id: caller.tenant_id(),
        owner_tenant_id: command.scope().tenant_id,
        command_id: command.command_id(),
        asset_id: command.scope().asset_id,
        kind,
        denial_reference: format!(
            "tenant-denial:command/{}@{at_unix_millis}/{}",
            command.command_id(),
            kind.code(),
        ),
        at_unix_millis,
    }
}

/// An opaque audit-backend failure (code + human-safe message — the same
/// convention as `fallback_policy.rs`'s audit seam error; no internals).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecurityAuditError {
    /// The opaque backend code.
    pub code: String,
    /// The human-safe message.
    pub message: String,
}

impl std::fmt::Display for SecurityAuditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "security audit backend failure ({}): {}", self.code, self.message)
    }
}

impl std::error::Error for SecurityAuditError {}

/// The audit-trail seam for cross-tenant denial records (P4.28 step 3).
/// Declared HERE (flexcore-application cannot depend on flexcore-audit —
/// the recorded seam pattern of `fallback_policy.rs`); the composition
/// root implements it over the P4.30 audit persistence. Fail-closed: an
/// audit failure is an error, never a silently dropped denial record.
pub trait SecurityAuditSink {
    /// Appends one cross-tenant denial record.
    fn append_tenant_denial(
        &self,
        record: &TenantDenialAuditRecord,
    ) -> Result<(), SecurityAuditError>;
}
