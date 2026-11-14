// P9.34 — The signed edge-update VERIFICATION and ROLLBACK surface: the
//         update manifest, verification BEFORE any write (fail-closed
//         default), the append-only update/rollback register, and the
//         never-silent downgrade rule (report §18.3 signed and verified
//         update bundles; §17.3 secure boot/update where supported;
//         §21.2; ledger P9.34).
//
// WHAT THIS MODULE EXTENDS: the P7.14 provenance pattern
// (`crates/flexcore-kernel/src/signing.rs`) — the same digest-and-record
// structure, the same closed vocabularies, the same fail-closed
// `NoVerifier` default — and the Phase 6 signed-update POSITION
// (`ledger/phase-06-edge-protocol/34_secure_boot_position.md` §3: "the
// P6.20 update bundle is signature-verified BEFORE any swap"; "automatic
// rollback on failed health re-activates a previously verified bundle; a
// downgrade is an explicit operator-signed action"). That position was
// DESIGN ONLY — no verification code existed. This module is the code.
//
// THE ORDERING RULE (ledger step 1's named risk: "verification that runs
// after files are already written"): `UpdateGate::verify_before_staging`
// takes NO path and performs NO filesystem write — the refusal is a return
// value plus an audit record, structurally incapable of touching disk. The
// update/rollback register ([`FileUpdateLedger`]) is appended ONLY after a
// verification verdict exists (activation) — a refused bundle leaves zero
// records and zero files behind.
//
// THE MANIFEST (ledger §18.3 fields): version, bundle digest, digest
// algorithm, signer key REFERENCE (scheme-prefixed — the closed key set
// makes inline key material structurally impossible, the P7.14 rule), the
// MINIMUM-VERSION FLOOR, and the operator approval reference (ledger step
// 4: "who approves an update" — the pilot update policy lives in
// `34_signed_update_verification.md`; the code enforces that SOME named
// approval reference exists, fail-closed).
//
// THE VERIFIER PORT (the P7.14 honesty pattern, restated): the DEFAULT is
// [`NoUpdateVerifier`] — refuses EVERYTHING (fail-closed: the absence of
// signing infrastructure can never look like success). [`TestOnlyFnv1a64UpdateVerifier`]
// is the loudly-labelled test-grade FNV-1a-64 (the repo's integrity-
// checksum pattern) so the gate/refusal/audit paths are exercisable without
// a crypto dependency. Wiring a production SHA-256 (or stronger) verifier
// is a recorded DEPLOYMENT REQUIREMENT. "Signature" here is honest about
// being a digest-and-record scheme: tamper-EVIDENCE against casual
// tampering, NOT a cryptographic guarantee.
//
// THE ROLLBACK REGISTER (ledger step 3): rollback = re-activation of a
// PREVIOUSLY-VERIFIED (previously-activated) version, with the recorded
// reason, appended to the same register. NEVER a silent downgrade:
//
// - the target must be a version this register has SEEN activated
//   ([`UpdateVerificationError::RollbackTargetNeverActivated`]);
// - the target must be BEHIND the active version
//   ([`UpdateVerificationError::RollbackTargetNotBehindActive`]);
// - a rollback to a version BELOW the deployment's minimum-version floor
//   is refused
//   ([`UpdateVerificationError::RollbackBelowFloorRefused`]) UNLESS an
//   explicit recorded override ([`RollbackOverride`]: named actor + named
//   authority reference) is presented — and even then the override rides
//   the register record AND the audit line (never silent).
//
// THE A/B SLOT DECISION (recorded, per the ledger): ledger P9.34 does NOT
// dictate a slot scheme; the recorded decision is
// [`UPDATE_SLOT_DECISION`] — the register (not a slot table) is the source
// of truth for the active version, and a two-slot A/B layout is the
// RECOMMENDED deployment shape (the current binary stays untouched until
// the new one is verified and activated; rollback is a re-activation, so
// the previous slot must be retained). The code here models exactly what
// the ledger dictates: active version + verified candidate + register.
//
// SECURE-BOOT POSITION (carried from Phase 6, unchanged): this surface
// verifies the FlexCore update bundle ABOVE the OS; it does NOT establish
// boot-chain integrity. Where the pilot hardware's secure-boot
// determination comes back "supported" (P6.34: every hardware cell is
// PENDING PILOT HARDWARE SPEC), the two controls are complementary —
// neither substitutes for the other. Where it comes back "unsupported",
// this verification is the ONLY FlexCore-layer integrity control and the
// residual risk (an attacker below the FlexCore layer) is recorded in the
// P6.34 position, not quantified here.
//
// HONESTY BOUNDS (AGENTS.md rule 1, §13.3 discipline): NO update has ever
// been built, signed, staged, or applied — the ledger's "apply a valid
// update to one non-production edge" (step 2) and "an update that fails
// mid-apply" (step 3) are the recorded UNRUN human executions; this
// module is the verification/refusal/rollback-decision surface those
// executions drive. What the WRITTEN-UNRUN tests would and would not
// establish is itemised in `34_signed_update_verification.md`.

use std::collections::BTreeSet;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use flexcore_ports::clock::Clock;

use crate::ports::{AuditSink, EdgeAuditEvent, EdgeAuditKind};

// --- Constants -----------------------------------------------------------------

/// The closed digest-algorithm vocabulary for update manifests (the P7.14
/// closed set, mirrored). `sha-256` is the name a PRODUCTION deployment
/// wires (DEPLOYMENT REQUIREMENT — acceptance is meaningless until a real
/// SHA-256 verifier is composed; `NoUpdateVerifier` refuses everything
/// regardless of the declared algorithm). `fnv1a64-test` is the labelled
/// test-grade checksum.
pub const UPDATE_DIGEST_ALGORITHMS: [&str; 2] = ["sha-256", "fnv1a64-test"];
/// The accepted signer key-reference schemes (the P7.05/P7.14 scheme rule;
/// `key-store:` is the deployment key store). A reference without one of
/// these prefixes is refused as likely inline key material — the value is
/// never echoed back.
pub const UPDATE_KEY_REFERENCE_SCHEMES: [&str; 3] = ["key-store:", "env:", "file:"];
/// The manifest format id (versioned; a manifest carrying another id is
/// refused, never parsed permissively).
pub const UPDATE_MANIFEST_FORMAT: &str = "flexcore-edge-update-manifest-v1";
/// Maximum accepted manifest source size in bytes (a manifest is metadata,
/// not a payload).
pub const MAX_UPDATE_MANIFEST_BYTES: usize = 64 * 1024;
/// Hard sanity cap on one update-register record payload.
const MAX_UPDATE_RECORD_BYTES: u32 = 64 * 1024;

/// The P9.34 honesty statement (the P4.28 statement pattern — recorded so
/// tests and reviewers can pin what IS and IS NOT claimed).
pub const UPDATE_STATEMENT: &str = "the P9.34 edge-update surface is a digest-and-record \
     verification structure: manifests, a pluggable fail-closed verifier port, refusal \
     BEFORE any write, an append-only activation/rollback register, and audit on every \
     verdict. No update has ever been built, signed, staged, or applied; no cryptographic \
     signature exists (the default verifier refuses everything; the test-grade FNV-1a-64 \
     verifier is NOT a security primitive); wiring a production SHA-256 (or stronger) \
     verifier is a DEPLOYMENT REQUIREMENT. Secure boot is NOT established by this surface \
     (the P6.34 position stands: every hardware determination is PENDING PILOT HARDWARE \
     SPEC). No certification or security claim is made.";

/// The recorded A/B-slot decision (see the module header).
pub const UPDATE_SLOT_DECISION: &str = "the update/rollback REGISTER is the source of truth \
     for the active version; a two-slot A/B layout (previous version retained untouched \
     until the new one is verified and activated) is the RECOMMENDED deployment shape. \
     Ledger P9.34 dictates no slot scheme; the code models active version + verified \
     candidate + register only.";

// --- Refusal reason tags (stable; they ride the audit text; the tests pin them) ------

/// No manifest was supplied — an unsigned bundle is refused before anything
/// else (the P7.14 rule).
pub const REFUSAL_UNSIGNED: &str = "UPDATE_UNSIGNED_BUNDLE";
/// The manifest names a signer key reference outside the trusted set.
pub const REFUSAL_UNTRUSTED_SIGNER: &str = "UPDATE_UNTRUSTED_SIGNER";
/// The manifest's declared algorithm is not the configured verifier's.
pub const REFUSAL_ALGORITHM_MISMATCH: &str = "UPDATE_DIGEST_ALGORITHM_MISMATCH";
/// The bundle bytes do not match the manifest digest under the verifier.
pub const REFUSAL_DIGEST_MISMATCH: &str = "UPDATE_DIGEST_MISMATCH";
/// The configured verifier refused (e.g. the fail-closed default).
pub const REFUSAL_VERIFIER_REFUSED: &str = "UPDATE_VERIFIER_REFUSED";
/// The manifest version is below the minimum-version floor.
pub const REFUSAL_BELOW_FLOOR: &str = "UPDATE_BELOW_MINIMUM_FLOOR";

// --- Errors -----------------------------------------------------------------------

/// Named errors of the update surface. Every failure is named
/// ([`UpdateVerificationError::name`]); nothing implies a silent pass. No
/// error ever echoes a signer key-reference VALUE (it may be key
/// material).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateVerificationError {
    /// An input was unusable (empty reason text, empty actor, …).
    InvalidUpdateInput { reason: String },
    /// The manifest failed validation (missing field, bad scheme, bad
    /// digest shape, malformed version/floor).
    ManifestInvalid { reason: String },
    /// The bundle carries NO manifest — refused before any other check.
    UnsignedBundle,
    /// The manifest's signer key reference is outside the gate's trusted
    /// set (ledger step 1: "a bundle signed by an untrusted key"). The
    /// reference value is deliberately NOT echoed.
    UntrustedSigner,
    /// The manifest declares an algorithm the configured verifier does not
    /// compute.
    AlgorithmMismatch { declared: String, configured: &'static str },
    /// The bundle bytes do not match the manifest digest (ledger step 1:
    /// "a bad-signature bundle").
    DigestMismatch,
    /// The verifier could not verify at all (not configured, backend
    /// unavailable). Never treated as a pass.
    VerifierRefused { reason: String },
    /// The manifest version is below the manifest's minimum-version floor
    /// (refused at install).
    BelowMinimumFloor { version: String, floor: String },
    /// A version string is not a numeric M.m.p core.
    VersionMalformed { value: String, reason: String },
    /// Activation was attempted for the version that is ALREADY active.
    VersionAlreadyActive { version: String },
    /// The rollback target was never activated in this register — a
    /// rollback re-activates a PREVIOUSLY-VERIFIED version, never an
    /// unknown one.
    RollbackTargetNeverActivated { version: String },
    /// The rollback target is not behind the active version (that would be
    /// an upgrade wearing a rollback's clothes).
    RollbackTargetNotBehindActive { active: String, target: String },
    /// THE NEVER-SILENT-DOWNGRADE REFUSAL: a rollback below the deployment
    /// minimum-version floor without an explicit recorded override.
    RollbackBelowFloorRefused { target: String, floor: String },
    /// The presented rollback override is invalid (no named actor or no
    /// authority reference).
    RollbackOverrideInvalid { reason: String },
    /// The update register could not be opened/loaded (fail-closed).
    LedgerUnavailable { reason: String },
    /// The update register I/O failed (the record was NOT persisted).
    LedgerIo { reason: String },
    /// The file exists but is not a FlexCore edge update register (wrong
    /// magic or version) — refused, never guessed at.
    NotAnUpdateLedger { reason: String },
}

impl UpdateVerificationError {
    /// The stable machine-readable error name (the named-error table key).
    pub fn name(&self) -> &'static str {
        match self {
            UpdateVerificationError::InvalidUpdateInput { .. } => "UPDATE_INPUT_INVALID",
            UpdateVerificationError::ManifestInvalid { .. } => "UPDATE_MANIFEST_INVALID",
            UpdateVerificationError::UnsignedBundle => REFUSAL_UNSIGNED,
            UpdateVerificationError::UntrustedSigner => REFUSAL_UNTRUSTED_SIGNER,
            UpdateVerificationError::AlgorithmMismatch { .. } => REFUSAL_ALGORITHM_MISMATCH,
            UpdateVerificationError::DigestMismatch => REFUSAL_DIGEST_MISMATCH,
            UpdateVerificationError::VerifierRefused { .. } => REFUSAL_VERIFIER_REFUSED,
            UpdateVerificationError::BelowMinimumFloor { .. } => REFUSAL_BELOW_FLOOR,
            UpdateVerificationError::VersionMalformed { .. } => "UPDATE_VERSION_MALFORMED",
            UpdateVerificationError::VersionAlreadyActive { .. } => "UPDATE_VERSION_ALREADY_ACTIVE",
            UpdateVerificationError::RollbackTargetNeverActivated { .. } => {
                "UPDATE_ROLLBACK_TARGET_NEVER_ACTIVATED"
            }
            UpdateVerificationError::RollbackTargetNotBehindActive { .. } => {
                "UPDATE_ROLLBACK_TARGET_NOT_BEHIND_ACTIVE"
            }
            UpdateVerificationError::RollbackBelowFloorRefused { .. } => {
                "UPDATE_ROLLBACK_BELOW_FLOOR_REFUSED"
            }
            UpdateVerificationError::RollbackOverrideInvalid { .. } => {
                "UPDATE_ROLLBACK_OVERRIDE_INVALID"
            }
            UpdateVerificationError::LedgerUnavailable { .. } => "UPDATE_LEDGER_UNAVAILABLE",
            UpdateVerificationError::LedgerIo { .. } => "UPDATE_LEDGER_IO",
            UpdateVerificationError::NotAnUpdateLedger { .. } => "UPDATE_NOT_AN_UPDATE_LEDGER",
        }
    }
}

impl std::fmt::Display for UpdateVerificationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UpdateVerificationError::InvalidUpdateInput { reason } => {
                write!(f, "the update input is invalid: {reason}")
            }
            UpdateVerificationError::ManifestInvalid { reason } => {
                write!(f, "the update manifest is invalid: {reason}")
            }
            UpdateVerificationError::UnsignedBundle => write!(
                f,
                "no update manifest was supplied; an unsigned bundle is refused before any \
                 other check (§18.3; the P7.14 rule)"
            ),
            UpdateVerificationError::UntrustedSigner => write!(
                f,
                "the manifest's signer key reference is outside the trusted set (ledger \
                 P9.34 step 1: a bundle signed by an untrusted key is refused; the \
                 reference value is not echoed)"
            ),
            UpdateVerificationError::AlgorithmMismatch { declared, configured } => write!(
                f,
                "the manifest declares digest algorithm '{declared}' but the configured \
                 verifier computes '{configured}' — refused, never guessed at"
            ),
            UpdateVerificationError::DigestMismatch => write!(
                f,
                "the bundle bytes do not match the manifest digest (a bad-signature bundle; \
                 refused BEFORE any file is written)"
            ),
            UpdateVerificationError::VerifierRefused { reason } => write!(
                f,
                "the configured verifier refused to verify (fail-closed): {reason}"
            ),
            UpdateVerificationError::BelowMinimumFloor { version, floor } => write!(
                f,
                "the manifest version {version} is below the minimum-version floor {floor} — \
                 refused at install"
            ),
            UpdateVerificationError::VersionMalformed { value, reason } => {
                write!(f, "version '{value}' is not a numeric major.minor.patch core: {reason}")
            }
            UpdateVerificationError::VersionAlreadyActive { version } => write!(
                f,
                "version {version} is already the active version; a duplicate activation is \
                 refused"
            ),
            UpdateVerificationError::RollbackTargetNeverActivated { version } => write!(
                f,
                "version {version} was never activated in this register — a rollback \
                 re-activates a PREVIOUSLY-VERIFIED version, never an unknown one"
            ),
            UpdateVerificationError::RollbackTargetNotBehindActive { active, target } => write!(
                f,
                "the rollback target {target} is not behind the active version {active} — \
                 that is an upgrade, not a rollback, and is refused"
            ),
            UpdateVerificationError::RollbackBelowFloorRefused { target, floor } => write!(
                f,
                "the rollback target {target} is BELOW the minimum-version floor {floor} — \
                 refused (never a silent downgrade) unless an explicit recorded override is \
                 presented"
            ),
            UpdateVerificationError::RollbackOverrideInvalid { reason } => write!(
                f,
                "the rollback override is invalid: {reason} (an override without a named \
                 actor and an authority reference is not a recorded override)"
            ),
            UpdateVerificationError::LedgerUnavailable { reason } => write!(
                f,
                "the update register is unavailable; activation/rollback are refused \
                 fail-closed: {reason}"
            ),
            UpdateVerificationError::LedgerIo { reason } => {
                write!(f, "the update register I/O failed: {reason}")
            }
            UpdateVerificationError::NotAnUpdateLedger { reason } => write!(
                f,
                "the file is not a FlexCore edge update register (refused, never guessed): \
                 {reason}"
            ),
        }
    }
}

impl std::error::Error for UpdateVerificationError {}

// --- Version ordering (numeric M.m.p core) ------------------------------------------

/// Compares two numeric `major.minor.patch` version cores
/// (hand-computable ordering: `1.2.3 < 1.10.0`; `2.0.0 > 1.9.9`; equal
/// cores order `Equal`). Non-numeric or missing components are the named
/// [`UpdateVerificationError::VersionMalformed`] — never guessed at, never
/// lexically compared ("1.10" sorts BEFORE "1.9" lexically; versions are
/// compared numerically or not at all).
pub fn compare_versions(a: &str, b: &str) -> Result<std::cmp::Ordering, UpdateVerificationError> {
    let parse = |value: &str| -> Result<[u64; 3], UpdateVerificationError> {
        let parts: Vec<&str> = value.split('.').collect();
        if parts.len() != 3 {
            return Err(UpdateVerificationError::VersionMalformed {
                value: value.to_string(),
                reason: "expected exactly three numeric components (major.minor.patch)"
                    .to_string(),
            });
        }
        let mut out = [0u64; 3];
        for (index, part) in parts.iter().enumerate() {
            if part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(UpdateVerificationError::VersionMalformed {
                    value: value.to_string(),
                    reason: "every component must be a non-empty run of ASCII digits"
                        .to_string(),
                });
            }
            out[index] = part
                .parse::<u64>()
                .map_err(|_| UpdateVerificationError::VersionMalformed {
                    value: value.to_string(),
                    reason: "a component does not fit u64".to_string(),
                })?;
        }
        Ok(out)
    };
    let left = parse(a)?;
    let right = parse(b)?;
    Ok(left.cmp(&right))
}

// --- The verifier port (the P7.14 pattern, mirrored) ----------------------------------

/// The outcome of one bundle digest verification. Fail-closed: an
/// implementation that cannot verify reports [`UpdateDigestVerdict::Refused`],
/// never a guess.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateDigestVerdict {
    /// The bundle bytes match the declared digest under this verifier's
    /// algorithm.
    Matches,
    /// The bundle bytes do NOT match the declared digest.
    Mismatch,
    /// The verifier could not verify at all (not configured, backend
    /// unavailable). Never treated as a pass.
    Refused { reason: String },
}

/// The pluggable update digest verifier (the P7.14 `DigestVerifier` port,
/// mirrored for the edge update surface). The DEFAULT is
/// [`NoUpdateVerifier`]; a production SHA-256 (or stronger) implementation
/// is a recorded DEPLOYMENT REQUIREMENT.
pub trait UpdateVerifier {
    /// The algorithm name this verifier computes (compared against the
    /// manifest's `digest_algorithm`; a mismatch is a refusal).
    fn algorithm_name(&self) -> &'static str;
    /// Verify `bundle` against the declared lowercase-hex digest.
    fn verify(&self, bundle: &[u8], declared_digest_hex: &str) -> UpdateDigestVerdict;
}

/// The fail-closed DEFAULT verifier: refuses EVERY bundle. A deployment
/// that wires nothing verifies nothing — and nothing passes the gate. This
/// is the recorded safety posture: the absence of signing infrastructure
/// can never look like success (the kernel `NoVerifier` pattern, mirrored).
#[derive(Debug, Clone, Copy, Default)]
pub struct NoUpdateVerifier;

impl UpdateVerifier for NoUpdateVerifier {
    fn algorithm_name(&self) -> &'static str {
        "none"
    }

    fn verify(&self, _bundle: &[u8], _declared_digest_hex: &str) -> UpdateDigestVerdict {
        UpdateDigestVerdict::Refused {
            reason: "no update digest verifier is configured; verification is refused \
                     (fail-closed default; production SHA-256 wiring is a DEPLOYMENT \
                     REQUIREMENT)"
                .to_string(),
        }
    }
}

/// The FNV-1a 64-bit hash (the repo's existing non-cryptographic pattern —
/// P4.28/P6.14/P7.14; deterministic, dependency-free, explicitly NOT a
/// security primitive).
fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// The TEST-GRADE verifier: FNV-1a 64 over the bundle bytes. UNSAFE for
/// update-integrity by construction (a 64-bit non-cryptographic checksum;
/// forged collisions are practical). Exists ONLY so the gate, tamper,
/// refusal, and audit paths are testable without a crypto dependency. A
/// deployment that wires this in production has wired nothing — and should
/// be treated as [`NoUpdateVerifier`]. PRODUCTION-HASH DEPLOYMENT
/// REQUIREMENT: SHA-256 or stronger under reviewed key management (the
/// P6.20/P7.14 standing requirement; unchanged here).
#[derive(Debug, Clone, Copy, Default)]
pub struct TestOnlyFnv1a64UpdateVerifier;

impl UpdateVerifier for TestOnlyFnv1a64UpdateVerifier {
    fn algorithm_name(&self) -> &'static str {
        "fnv1a64-test"
    }

    fn verify(&self, bundle: &[u8], declared_digest_hex: &str) -> UpdateDigestVerdict {
        let computed = test_grade_fnv1a64_hex(bundle);
        if computed == declared_digest_hex {
            UpdateDigestVerdict::Matches
        } else {
            UpdateDigestVerdict::Mismatch
        }
    }
}

/// The TEST-GRADE digest helper (the exact computation
/// [`TestOnlyFnv1a64UpdateVerifier`] performs), exposed under its honest
/// name for test fixtures and lab manifests ONLY. The FNV-1a 64 of an EMPTY
/// input is the offset basis `cbf29ce484222325` — the definitional
/// known answer the tests pin. NEVER used to sign anything real: a
/// production release digest is computed by the release tooling under the
/// deployment's SHA-256 (or stronger) verifier — the standing DEPLOYMENT
/// REQUIREMENT.
pub fn test_grade_fnv1a64_hex(bytes: &[u8]) -> String {
    format!("{:016x}", fnv1a64(bytes))
}

// --- The manifest ---------------------------------------------------------------------

/// The signed-update manifest (§18.3; ledger P9.34): version, bundle
/// digest, digest algorithm, signer key REFERENCE, minimum-version floor,
/// and the operator approval reference. NO key material is structurally
/// possible (the closed scheme set; a scheme-less reference is refused as
/// likely inline key material and its value never echoed).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateManifest {
    /// The update package id (e.g. the release/artifact reference).
    pub package_id: String,
    /// The version this manifest installs (numeric M.m.p core).
    pub version: String,
    /// The bundle digest as lowercase hex (the verifier compares its own
    /// computation over the bundle bytes).
    pub bundle_digest_hex: String,
    /// The digest algorithm, from [`UPDATE_DIGEST_ALGORITHMS`] (closed).
    pub digest_algorithm: String,
    /// The signer key REFERENCE (scheme-prefixed). Never key material.
    pub signer_key_reference: String,
    /// The MINIMUM-VERSION FLOOR this manifest enforces at install
    /// (refuses installing onto an edge below the floor).
    pub minimum_version_floor: String,
    /// The operator-approval reference (ledger step 4: who approves — the
    /// named approval record on the platform side). Empty = refused
    /// (fail-closed: an unapproved update is not an update).
    pub approval_reference: String,
}

impl UpdateManifest {
    /// Validates the manifest (fail-closed; every violation is a named
    /// [`UpdateVerificationError::ManifestInvalid`]).
    pub fn validate(&self) -> Result<(), UpdateVerificationError> {
        let mut problems: Vec<String> = Vec::new();
        if self.package_id.trim().is_empty() || self.package_id.len() > 256 {
            problems.push("package_id must be a non-empty string of at most 256 characters"
                .to_string());
        }
        if let Err(error) = compare_versions(&self.version, "0.0.0") {
            problems.push(format!("version: {error}"));
        }
        if let Err(error) = compare_versions(&self.minimum_version_floor, "0.0.0") {
            problems.push(format!("minimum_version_floor: {error}"));
        }
        if !UPDATE_DIGEST_ALGORITHMS.contains(&self.digest_algorithm.as_str()) {
            problems.push(format!(
                "digest_algorithm '{}' is not in the closed vocabulary ({})",
                self.digest_algorithm,
                UPDATE_DIGEST_ALGORITHMS.join(", ")
            ));
        }
        let hex_ok = self.bundle_digest_hex.len() >= 8
            && self.bundle_digest_hex.len() <= 512
            && self.bundle_digest_hex.len() % 2 == 0
            && self
                .bundle_digest_hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b.is_ascii_lowercase() && b.is_ascii_hexdigit()));
        if !hex_ok {
            problems.push(
                "bundle_digest_hex must be an even-length lowercase hex string of 8..=512 \
                 digits"
                    .to_string(),
            );
        }
        let scheme_ok = UPDATE_KEY_REFERENCE_SCHEMES.iter().any(|scheme| {
            self.signer_key_reference
                .strip_prefix(*scheme)
                .map(|rest| !rest.is_empty())
                .unwrap_or(false)
        });
        if !scheme_ok {
            problems.push(
                "signer_key_reference must use the key-store:, env:, or file: scheme and \
                 name a location; update manifests carry key REFERENCES only (a scheme-less \
                 reference is refused as likely inline key material; the value is not \
                 echoed)"
                    .to_string(),
            );
        }
        if self.approval_reference.trim().is_empty() {
            problems.push("approval_reference is empty (an unapproved update is refused, \
                           fail-closed)"
                .to_string());
        }
        if problems.is_empty() {
            Ok(())
        } else {
            Err(UpdateVerificationError::ManifestInvalid {
                reason: problems.join("; "),
            })
        }
    }
}

// --- The gate (verification BEFORE any write) -------------------------------------------

/// A verified-update verdict: the manifest plus when verification passed
/// (the INJECTED clock). The ONLY thing that may proceed to activation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedUpdate {
    pub manifest: UpdateManifest,
    pub verified_at_unix_millis: i64,
}

/// The update verification gate over ONE edge. Holds the trusted signer
/// key-reference set and the configured verifier; performs NO filesystem
/// write ever (the ordering rule, module header).
pub struct UpdateGate {
    /// The trusted signer key REFERENCES (the deployment's release-key
    /// references; the P9.09 certificate inventory feeds this). A bundle
    /// signed by anything outside the set is refused — ledger step 1's
    /// "untrusted key" case.
    trusted_signer_references: BTreeSet<String>,
    verifier: Box<dyn UpdateVerifier + Send>,
    audit: Arc<dyn AuditSink>,
    clock: Arc<dyn Clock + Send + Sync>,
}

impl UpdateGate {
    /// Composes the gate. An EMPTY trusted set is allowed and refuses
    /// every bundle (`UntrustedSigner`) — fail-closed by construction.
    pub fn new(
        trusted_signer_references: Vec<String>,
        verifier: Box<dyn UpdateVerifier + Send>,
        audit: Arc<dyn AuditSink>,
        clock: Arc<dyn Clock + Send + Sync>,
    ) -> Self {
        Self {
            trusted_signer_references: trusted_signer_references.into_iter().collect(),
            verifier,
            audit,
            clock,
        }
    }

    /// The trusted signer references (diagnostics; references only).
    pub fn trusted_signer_references(&self) -> Vec<&str> {
        self.trusted_signer_references.iter().map(|s| s.as_str()).collect()
    }

    /// Verifies ONE update bundle against its manifest, BEFORE anything is
    /// written (structural: this function takes no path and touches no
    /// file). Refusal order (recorded, the P7.14 order mirrored):
    ///
    /// 1. manifest present — else [`UpdateVerificationError::UnsignedBundle`];
    /// 2. manifest valid — else [`UpdateVerificationError::ManifestInvalid`];
    /// 3. declared algorithm == configured verifier — else `AlgorithmMismatch`;
    /// 4. signer reference trusted — else `UntrustedSigner`;
    /// 5. version >= the manifest's minimum-version floor — else `BelowMinimumFloor`;
    /// 6. verifier verdict `Matches` — else `DigestMismatch`/`VerifierRefused`.
    ///
    /// Every refusal is audited (`EDGE_UPDATE_VERIFICATION_REFUSED`, with
    /// the stable refusal tag); every pass is audited
    /// (`EDGE_UPDATE_VERIFIED`). No "warn-and-continue" exists.
    pub fn verify_before_staging(
        &self,
        manifest: Option<&UpdateManifest>,
        bundle: &[u8],
    ) -> Result<VerifiedUpdate, UpdateVerificationError> {
        let verdict = self.verify_inner(manifest, bundle);
        match &verdict {
            Ok(verified) => {
                self.audit_event(
                    "EDGE_UPDATE_VERIFIED",
                    format!(
                        "package='{}' version={} algorithm='{}' signer_ref_present=true \
                         approval_ref='{}' — verified BEFORE any write (ledger P9.34)",
                        verified.manifest.package_id,
                        verified.manifest.version,
                        verified.manifest.digest_algorithm,
                        verified.manifest.approval_reference,
                    ),
                );
            }
            Err(error) => {
                self.audit_event(
                    "EDGE_UPDATE_VERIFICATION_REFUSED",
                    format!(
                        "refusal code={} :: {error} — refused BEFORE any file was written \
                         (ledger P9.34 step 1)",
                        error.name(),
                    ),
                );
            }
        }
        verdict
    }

    fn verify_inner(
        &self,
        manifest: Option<&UpdateManifest>,
        bundle: &[u8],
    ) -> Result<VerifiedUpdate, UpdateVerificationError> {
        // 1. Present (unsigned refused before everything).
        let manifest = manifest.ok_or(UpdateVerificationError::UnsignedBundle)?;
        // 2. Valid.
        manifest.validate()?;
        // 3. Algorithm matches the configured verifier.
        if manifest.digest_algorithm != self.verifier.algorithm_name() {
            return Err(UpdateVerificationError::AlgorithmMismatch {
                declared: manifest.digest_algorithm.clone(),
                configured: self.verifier.algorithm_name(),
            });
        }
        // 4. Signer reference is trusted.
        if !self.trusted_signer_references.contains(&manifest.signer_key_reference) {
            return Err(UpdateVerificationError::UntrustedSigner);
        }
        // 5. Version respects the manifest's minimum-version floor.
        if compare_versions(&manifest.version, &manifest.minimum_version_floor)?
            == std::cmp::Ordering::Less
        {
            return Err(UpdateVerificationError::BelowMinimumFloor {
                version: manifest.version.clone(),
                floor: manifest.minimum_version_floor.clone(),
            });
        }
        // 6. The bundle bytes match the declared digest.
        match self.verifier.verify(bundle, &manifest.bundle_digest_hex) {
            UpdateDigestVerdict::Matches => {}
            UpdateDigestVerdict::Mismatch => return Err(UpdateVerificationError::DigestMismatch),
            UpdateDigestVerdict::Refused { reason } => {
                return Err(UpdateVerificationError::VerifierRefused { reason })
            }
        }
        Ok(VerifiedUpdate {
            manifest: manifest.clone(),
            verified_at_unix_millis: self.now_unix_millis(),
        })
    }

    fn now_unix_millis(&self) -> i64 {
        let stamp = self.clock.now_ingest_time();
        i64::try_from(stamp.unix_millis()).unwrap_or(i64::MAX)
    }

    /// Records one audit event through the edge `AuditSink` (the
    /// dedicated-event-name-leads-detail pattern on the closed
    /// `EdgeAuditKind` vocabulary, as in P6.14/P9.31/P9.32/P9.33).
    fn audit_event(&self, name: &'static str, detail: String) {
        let event = EdgeAuditEvent {
            recorded_at: self.clock.now_ingest_time(),
            kind: EdgeAuditKind::Startup,
            detail: format!("{name}: {detail}"),
        };
        if let Err(error) = self.audit.record(event) {
            tracing::error!(%error, audit_event = name, "update-gate audit write FAILED");
        }
    }
}

// --- The append-only update/rollback register -------------------------------------------

/// The closed update-register event vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateLedgerEventKind {
    /// A verified version was activated (installed) on this edge.
    Activation,
    /// A previously-activated version was re-activated (a rollback), with
    /// the recorded reason and (when below the floor) the recorded
    /// override.
    Rollback,
}

impl UpdateLedgerEventKind {
    pub fn code(self) -> u8 {
        match self {
            UpdateLedgerEventKind::Activation => 1,
            UpdateLedgerEventKind::Rollback => 2,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            UpdateLedgerEventKind::Activation => "UPDATE_ACTIVATED",
            UpdateLedgerEventKind::Rollback => "UPDATE_ROLLBACK",
        }
    }

    fn from_code(code: u8) -> Option<Self> {
        match code {
            1 => Some(UpdateLedgerEventKind::Activation),
            2 => Some(UpdateLedgerEventKind::Rollback),
            _ => None,
        }
    }
}

/// One append-only update-register record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateLedgerRecord {
    pub kind: UpdateLedgerEventKind,
    /// When the activation/rollback happened (the INJECTED clock).
    pub at_unix_millis: i64,
    /// The version activated (for a rollback: the version RE-ACTIVATED).
    pub version: String,
    /// The version that was active BEFORE this event (empty for the first
    /// activation).
    pub previous_version: String,
    /// The manifest digest of the activated bundle (the verified manifest's
    /// digest; empty for a rollback re-activation — the ORIGINAL manifest's
    /// digest is on the original activation record).
    pub manifest_digest_hex: String,
    /// The approval reference (activation) or the rollback reason
    /// (rollback).
    pub approval_or_reason: String,
    /// The recorded override reference for a below-floor rollback (empty
    /// otherwise). NEVER empty-and-silent: a below-floor rollback carries
    /// its override here AND in the audit line.
    pub override_reference: String,
}

/// The file-backed append-only update/rollback register. Same framing
/// discipline as the P9.31 binding journal / P9.33 identity register /
/// P6.18 audit journal:
///
/// ```text
/// header:  b"FLEXCORE-EDGE-UPDATE-REGISTER" + format version byte (0x01)
/// record:  [u32 LE payload_len][u32 LE crc32(payload)][payload]
/// payload: [u8 kind][i64 LE at][u32 ver_len][version][u32 prev_len][previous]
///          [u32 dig_len][digest][u32 appr_len][approval_or_reason]
///          [u32 ovr_len][override_reference]
/// ```
///
/// Append-only (nothing edited or removed in place); truncate-and-audit
/// recovery; durable BEFORE the in-memory view moves; the ACTIVE version
/// and the previously-activated set are DERIVED by replay (the register is
/// the source of truth — the recorded slot decision).
pub struct FileUpdateLedger {
    path: PathBuf,
    file: Mutex<std::fs::File>,
    records: Mutex<Vec<UpdateLedgerRecord>>,
    corrupt_tail_truncated: std::sync::atomic::AtomicU64,
    records_written: std::sync::atomic::AtomicU64,
}

impl FileUpdateLedger {
    /// The register magic header (28-byte label) + format version.
    fn magic() -> Vec<u8> {
        let mut magic = b"FLEXCORE-EDGE-UPDATE-REGISTER".to_vec();
        magic.push(0x01);
        magic
    }

    /// Opens (or creates) the register at `path` and replays any existing
    /// records.
    pub fn open(path: &Path) -> Result<Self, UpdateVerificationError> {
        if path.as_os_str().is_empty() {
            return Err(UpdateVerificationError::LedgerUnavailable {
                reason: "the update register path is empty".to_string(),
            });
        }
        let magic = Self::magic();
        let (file, replayed) = match std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
        {
            Ok(mut existing) => {
                let mut contents = Vec::new();
                existing
                    .read_to_end(&mut contents)
                    .map_err(|e| UpdateVerificationError::LedgerIo {
                        reason: format!("reading the existing update register failed: {e}"),
                    })?;
                if contents.is_empty() {
                    (existing, Vec::new())
                } else {
                    if contents.len() < magic.len() || contents[..magic.len()] != magic[..] {
                        return Err(UpdateVerificationError::NotAnUpdateLedger {
                            reason: format!(
                                "the file at {} does not carry the FlexCore edge update \
                                 register magic",
                                path.display()
                            ),
                        });
                    }
                    let mut records = Vec::new();
                    let mut offset = magic.len();
                    let mut corrupt_tail_truncated: u64 = 0;
                    while offset + 8 <= contents.len() {
                        let len = u32::from_le_bytes(
                            contents[offset..offset + 4].try_into().expect("4 bytes"),
                        );
                        if len > MAX_UPDATE_RECORD_BYTES
                            || offset + 8 + len as usize > contents.len()
                        {
                            corrupt_tail_truncated += 1;
                            break;
                        }
                        let payload = &contents[offset + 8..offset + 8 + len as usize];
                        let stored_crc = u32::from_le_bytes(
                            contents[offset + 4..offset + 8].try_into().expect("4 bytes"),
                        );
                        if crc32(payload) != stored_crc {
                            corrupt_tail_truncated += 1;
                            break;
                        }
                        match decode_update_payload(payload) {
                            Some(record) => {
                                records.push(record);
                                offset += 8 + len as usize;
                            }
                            None => {
                                corrupt_tail_truncated += 1;
                                break;
                            }
                        }
                    }
                    if offset < contents.len() && corrupt_tail_truncated == 0 {
                        corrupt_tail_truncated += 1;
                    }
                    if corrupt_tail_truncated > 0 {
                        tracing::warn!(
                            truncated_records = corrupt_tail_truncated,
                            path = %path.display(),
                            "update register: corrupt tail truncated; the affected events \
                             reverted to their last good record"
                        );
                        existing
                            .set_len(offset as u64)
                            .map_err(|e| UpdateVerificationError::LedgerIo {
                                reason: format!(
                                    "truncating the corrupt update-register tail failed: {e}"
                                ),
                            })?;
                    }
                    (existing, records)
                }
            }
            Err(_) => {
                let mut created = std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create_new(true)
                    .open(path)
                    .map_err(|e| UpdateVerificationError::LedgerIo {
                        reason: format!("creating the update register file failed: {e}"),
                    })?;
                created
                    .write_all(&magic)
                    .and_then(|_| created.flush())
                    .map_err(|e| UpdateVerificationError::LedgerIo {
                        reason: format!("writing the update register header failed: {e}"),
                    })?;
                (created, Vec::new())
            }
        };
        Ok(Self {
            path: path.to_path_buf(),
            file: Mutex::new(file),
            records: Mutex::new(replayed),
            corrupt_tail_truncated: std::sync::atomic::AtomicU64::new(0),
            records_written: std::sync::atomic::AtomicU64::new(0),
        })
    }

    /// The register path (diagnostics).
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// How many corrupt tail records were truncated at open.
    pub fn corrupt_tail_truncated(&self) -> u64 {
        self.corrupt_tail_truncated
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    /// How many records this instance has written (diagnostics).
    pub fn records_written(&self) -> u64 {
        self.records_written
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    /// The number of records in the replayed history.
    pub fn len(&self) -> usize {
        self.records.lock().expect("poisoned update register mutex").len()
    }

    /// Whether the history is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The full append-only history (evidence reading).
    pub fn history(&self) -> Vec<UpdateLedgerRecord> {
        self.records
            .lock()
            .expect("poisoned update register mutex")
            .clone()
    }

    /// The currently ACTIVE version, derived by replay: the last event's
    /// version (an activation activates its version; a rollback
    /// re-activates its target).
    pub fn active_version(&self) -> Option<String> {
        self.records
            .lock()
            .expect("poisoned update register mutex")
            .last()
            .map(|record| record.version.clone())
    }

    /// The set of versions this register has SEEN activated (the only
    /// legal rollback targets).
    pub fn previously_activated(&self) -> BTreeSet<String> {
        self.records
            .lock()
            .expect("poisoned update register mutex")
            .iter()
            .fold(BTreeSet::new(), |mut set, record| {
                set.insert(record.version.clone());
                set
            })
    }

    /// Appends one record. Durable BEFORE the in-memory history moves.
    /// Append-only: no overwrite, no delete.
    pub fn append(&self, record: UpdateLedgerRecord) -> Result<(), UpdateVerificationError> {
        if record.at_unix_millis <= 0 {
            return Err(UpdateVerificationError::InvalidUpdateInput {
                reason: "the register event instant must be a positive unix-millis value"
                    .to_string(),
            });
        }
        if record.version.trim().is_empty() {
            return Err(UpdateVerificationError::InvalidUpdateInput {
                reason: "the register event version is mandatory".to_string(),
            });
        }
        let payload = encode_update_payload(&record);
        let record_len = payload.len() as u32;
        let crc = crc32(&payload);
        let mut framed = Vec::with_capacity(8 + payload.len());
        framed.extend_from_slice(&record_len.to_le_bytes());
        framed.extend_from_slice(&crc.to_le_bytes());
        framed.extend_from_slice(&payload);
        {
            let mut file = self.file.lock().expect("poisoned update register file mutex");
            file.seek(SeekFrom::End(0))
                .and_then(|_| file.write_all(&framed))
                .and_then(|_| file.flush())
                .and_then(|_| file.sync_data())
                .map_err(|e| UpdateVerificationError::LedgerIo {
                    reason: format!("appending the update register record failed: {e}"),
                })?;
        }
        self.records
            .lock()
            .expect("poisoned update register mutex")
            .push(record);
        self.records_written
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
}

fn encode_update_payload(record: &UpdateLedgerRecord) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.push(record.kind.code());
    payload.extend_from_slice(&record.at_unix_millis.to_le_bytes());
    for field in [
        record.version.as_str(),
        record.previous_version.as_str(),
        record.manifest_digest_hex.as_str(),
        record.approval_or_reason.as_str(),
        record.override_reference.as_str(),
    ] {
        payload.extend_from_slice(&(field.len() as u32).to_le_bytes());
        payload.extend_from_slice(field.as_bytes());
    }
    payload
}

fn decode_update_payload(payload: &[u8]) -> Option<UpdateLedgerRecord> {
    if payload.len() < 1 + 8 {
        return None;
    }
    let kind = UpdateLedgerEventKind::from_code(payload[0])?;
    let at_unix_millis = i64::from_le_bytes(payload[1..9].try_into().expect("8 bytes"));
    let mut cursor = 9usize;
    let (version, cursor) = decode_len_string(payload, cursor)?;
    let (previous_version, cursor) = decode_len_string(payload, cursor)?;
    let (manifest_digest_hex, cursor) = decode_len_string(payload, cursor)?;
    let (approval_or_reason, cursor) = decode_len_string(payload, cursor)?;
    let (override_reference, _) = decode_len_string(payload, cursor)?;
    Some(UpdateLedgerRecord {
        kind,
        at_unix_millis,
        version,
        previous_version,
        manifest_digest_hex,
        approval_or_reason,
        override_reference,
    })
}

fn decode_len_string(payload: &[u8], cursor: usize) -> Option<(String, usize)> {
    if payload.len() < cursor + 4 {
        return None;
    }
    let len =
        u32::from_le_bytes(payload[cursor..cursor + 4].try_into().expect("4 bytes")) as usize;
    if payload.len() < cursor + 4 + len {
        return None;
    }
    let value = std::str::from_utf8(&payload[cursor + 4..cursor + 4 + len])
        .ok()?
        .to_string();
    Some((value, cursor + 4 + len))
}

/// CRC-32 (IEEE, reflected) — the same checksum family as the checkpoint,
/// the audit journal, the P9.31 binding journal, and the P9.33 identity
/// register (an integrity checksum for corruption detection, not a
/// security primitive).
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

// --- Activation and rollback (the recorded decisions, audited) ---------------------------

/// The explicit recorded override for a below-floor rollback (the ledger's
/// "unless an explicit recorded override exists"). BOTH fields are
/// mandatory: an override without a named actor and an authority reference
/// is not a recorded override.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollbackOverride {
    /// The operator/authority that signed the override.
    pub actor: String,
    /// The authority record/ticket the override is written under.
    pub authority_reference: String,
}

impl RollbackOverride {
    fn validate(&self) -> Result<(), UpdateVerificationError> {
        let mut problems: Vec<String> = Vec::new();
        if self.actor.trim().is_empty() {
            problems.push("actor is empty".to_string());
        }
        if self.authority_reference.trim().is_empty() {
            problems.push("authority_reference is empty".to_string());
        }
        if problems.is_empty() {
            Ok(())
        } else {
            Err(UpdateVerificationError::RollbackOverrideInvalid {
                reason: problems.join("; "),
            })
        }
    }
}

/// The activation/rollback surface: drives the verified update into the
/// append-only register and executes the recorded rollback decision rules.
/// Holds no file paths (the composition passes the opened register) and no
/// trust material (the gate owns verification).
pub struct UpdateActivation {
    audit: Arc<dyn AuditSink>,
    clock: Arc<dyn Clock + Send + Sync>,
}

impl UpdateActivation {
    pub fn new(audit: Arc<dyn AuditSink>, clock: Arc<dyn Clock + Send + Sync>) -> Self {
        Self { audit, clock }
    }

    /// Activates a VERIFIED update (the gate's `VerifiedUpdate` — an
    /// unverified manifest cannot reach this function with a verdict; the
    /// caller passing an unverified manifest is a composition defect the
    /// register's evidence would expose). Refuses a duplicate activation
    /// of the already-active version. Appends the activation record
    /// (durable before memory) and audits `EDGE_UPDATE_ACTIVATED`.
    pub fn activate(
        &self,
        ledger: &FileUpdateLedger,
        verified: &VerifiedUpdate,
    ) -> Result<UpdateLedgerRecord, UpdateVerificationError> {
        if let Some(active) = ledger.active_version() {
            if active == verified.manifest.version {
                return Err(UpdateVerificationError::VersionAlreadyActive {
                    version: verified.manifest.version.clone(),
                });
            }
        }
        let record = UpdateLedgerRecord {
            kind: UpdateLedgerEventKind::Activation,
            at_unix_millis: self.now_unix_millis(),
            version: verified.manifest.version.clone(),
            previous_version: ledger.active_version().unwrap_or_default(),
            manifest_digest_hex: verified.manifest.bundle_digest_hex.clone(),
            approval_or_reason: verified.manifest.approval_reference.clone(),
            override_reference: String::new(),
        };
        ledger.append(record.clone())?;
        self.audit_event(
            "EDGE_UPDATE_ACTIVATED",
            format!(
                "package='{}' version={} previous={} digest={} approval_ref='{}' — the NEW \
                 VERSION REPORT is this record (ledger P9.34 step 2)",
                verified.manifest.package_id,
                record.version,
                if record.previous_version.is_empty() { "none".to_string() } else { record.previous_version.clone() },
                record.manifest_digest_hex,
                record.approval_or_reason,
            ),
        );
        Ok(record)
    }

    /// Executes the ROLLBACK decision (ledger step 3): re-activate a
    /// PREVIOUSLY-VERIFIED (previously-activated) version with the recorded
    /// reason. The rules (module header):
    ///
    /// 1. the target must have been activated in this register;
    /// 2. the target must be BEHIND the active version;
    /// 3. if the target is BELOW the deployment's `minimum_version_floor`,
    ///    an explicit [`RollbackOverride`] is REQUIRED — without one the
    ///    rollback is refused ([`UpdateVerificationError::RollbackBelowFloorRefused`]);
    ///    with one, the override rides the register record AND the audit
    ///    line (never a silent downgrade).
    ///
    /// Appends the rollback record and audits `EDGE_UPDATE_ROLLBACK`.
    pub fn rollback(
        &self,
        ledger: &FileUpdateLedger,
        minimum_version_floor: &str,
        target_version: &str,
        reason: &str,
        override_record: Option<&RollbackOverride>,
    ) -> Result<UpdateLedgerRecord, UpdateVerificationError> {
        if reason.trim().is_empty() {
            return Err(UpdateVerificationError::InvalidUpdateInput {
                reason: "the rollback reason is mandatory (a rollback is never silent)"
                    .to_string(),
            });
        }
        let active = ledger
            .active_version()
            .ok_or_else(|| UpdateVerificationError::RollbackTargetNeverActivated {
                version: target_version.to_string(),
            })?;
        // 1. Previously activated.
        if !ledger.previously_activated().contains(target_version) {
            return Err(UpdateVerificationError::RollbackTargetNeverActivated {
                version: target_version.to_string(),
            });
        }
        // 2. Behind the active version.
        if compare_versions(target_version, &active)? != std::cmp::Ordering::Less {
            return Err(UpdateVerificationError::RollbackTargetNotBehindActive {
                active,
                target: target_version.to_string(),
            });
        }
        // 3. The never-silent-downgrade rule.
        let mut override_reference = String::new();
        if compare_versions(target_version, minimum_version_floor)?
            == std::cmp::Ordering::Less
        {
            let Some(recorded) = override_record else {
                return Err(UpdateVerificationError::RollbackBelowFloorRefused {
                    target: target_version.to_string(),
                    floor: minimum_version_floor.to_string(),
                });
            };
            recorded.validate()?;
            override_reference = format!(
                "{} under {}",
                recorded.actor, recorded.authority_reference
            );
        }
        let record = UpdateLedgerRecord {
            kind: UpdateLedgerEventKind::Rollback,
            at_unix_millis: self.now_unix_millis(),
            version: target_version.to_string(),
            previous_version: active.clone(),
            manifest_digest_hex: String::new(),
            approval_or_reason: reason.to_string(),
            override_reference: override_reference.clone(),
        };
        ledger.append(record.clone())?;
        self.audit_event(
            "EDGE_UPDATE_ROLLBACK",
            format!(
                "rolled back {} -> {} reason='{}'{} (append-only register event; the edge \
                 reports its re-activated version, ledger P9.34 step 3)",
                record.previous_version,
                record.version,
                reason,
                if override_reference.is_empty() {
                    String::new()
                } else {
                    format!(" override={override_reference} (RECORDED — never a silent \
                             downgrade)")
                },
            ),
        );
        Ok(record)
    }

    fn now_unix_millis(&self) -> i64 {
        let stamp = self.clock.now_ingest_time();
        i64::try_from(stamp.unix_millis()).unwrap_or(i64::MAX)
    }

    fn audit_event(&self, name: &'static str, detail: String) {
        let event = EdgeAuditEvent {
            recorded_at: self.clock.now_ingest_time(),
            kind: EdgeAuditKind::Startup,
            detail: format!("{name}: {detail}"),
        };
        if let Err(error) = self.audit.record(event) {
            tracing::error!(%error, audit_event = name, "update-activation audit write FAILED");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_ordering_is_numeric_and_hand_computed() {
        // Hand-computed known answers: numeric (not lexical) ordering.
        assert_eq!(compare_versions("1.2.3", "1.10.0").expect("parses"), std::cmp::Ordering::Less, "1.2.3 < 1.10.0 (lexically '1.10' < '1.2' — versions are NEVER lexical)");
        assert_eq!(compare_versions("2.0.0", "1.9.9").expect("parses"), std::cmp::Ordering::Greater);
        assert_eq!(compare_versions("1.2.3", "1.2.3").expect("parses"), std::cmp::Ordering::Equal);
        assert_eq!(compare_versions("1.2.4", "1.2.40").expect("parses"), std::cmp::Ordering::Less);
        assert!(matches!(
            compare_versions("1.2", "1.2.3"),
            Err(UpdateVerificationError::VersionMalformed { .. })
        ));
        assert!(matches!(
            compare_versions("1.x.3", "1.2.3"),
            Err(UpdateVerificationError::VersionMalformed { .. })
        ));
    }
}
