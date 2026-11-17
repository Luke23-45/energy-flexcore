// P18.02 — The versioned update-bundle FORMAT for the edge fleet OTA
//         pipeline (ADR-1801, decision record
//         `docs/phases-11-20/phase-18-edge-fleet-lifecycle-ota/records/
//         01_p18_01_ota_architecture_adr.md`; format spec record 02):
//         the `flexcore-edge-update-manifest-v2` manifest extending the
//         P9.34 `UpdateManifest` with SUIT-shaped applicability metadata
//         and sequence-number anti-rollback [S-162], per-payload digests,
//         and size caps — plus the device-side monotonic anti-rollback
//         counter register.
//
// WHAT THIS MODULE IS (and is not): a FORMAT definition plus the
// structural validation and device-side applicability/sequence/expiry
// checks. It produces nothing, signs nothing, stages nothing, applies
// nothing — no bundle has ever been produced, signed, staged, or applied
// (the phase-18 acceptance criteria; the P9.34 honesty bounds carry over
// verbatim). The P9.34 gate (`crate::update_verification::UpdateGate`)
// remains the verification authority; this module's checks are strictly
// ADDITIONAL metadata checks that run before/after the gate's preserved
// refusal order (see `crate::update_session::ExtendedUpdateGate` for the
// composition).
//
// SUIT SHAPING (the honesty boundary): the applicability metadata and the
// monotonically-increasing sequence number are SHAPED BY the SUIT
// manifest concepts (IETF RFC 9019 [S-162] as summarized in
// `00_GAP_ANALYSIS.md` §11 and the phase-18 context). This format is a
// FlexCore format, NOT a SUIT CBOR encoding, and NO SUIT-conformance
// claim is made anywhere. The exact TUF/SUIT comparison semantics for
// sequence numbers (strictly-greater-than vs greater-or-equal) are
// UNVERIFIED against the spec texts (`ANTI_ROLLBACK_SEMANTICS_STATEMENT`);
// the implemented semantics are the strictly-safer reading and are
// recorded, not assumed to match the standards.
//
// ANTI-ROLLBACK SEMANTICS (designed, UNVERIFIED against [S-161]/[S-162]
// spec texts): a manifest is accepted only when its
// `anti_rollback_sequence` is STRICTLY GREATER than the last-seen value
// in the device's monotonic counter ([`AntiRollbackCounter`]); the
// counter advances ONLY on a full verification pass (durably, before the
// verdict is returned). Consequence (recorded, fail-safe direction): a
// manifest whose install later fails CANNOT be re-presented — a retry
// requires re-issued metadata with a higher sequence number. The
// P9.34 rollback path is unaffected: a rollback re-activates a
// PREVIOUSLY-ACTIVATED version through the append-only register and the
// recorded [`crate::update_verification::RollbackOverride`] rule — it
// never re-presents a manifest.
//
// SIZE CAPS: the caps here are CODE MAXIMA (a deployment may configure
// tighter caps; it can never configure looser ones — the constant is the
// ceiling the validation refuses to exceed).

use std::collections::BTreeSet;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::update_verification::{
    compare_versions, crc32, UpdateManifest, UpdateVerificationError,
};

// --- Constants -----------------------------------------------------------------

/// The v2 bundle-manifest format id (versioned; a manifest carrying
/// another id is refused, never parsed permissively — the P9.34 format-id
/// rule carried forward).
pub const UPDATE_BUNDLE_FORMAT_V2: &str = "flexcore-edge-update-manifest-v2";
/// Hard cap on ONE payload entry's declared size in bytes (a code maximum;
/// deployments may tighten, never loosen).
pub const MAX_UPDATE_PAYLOAD_ENTRY_BYTES: usize = 256 * 1024 * 1024;
/// Hard cap on the TOTAL declared payload size of one bundle in bytes.
pub const MAX_UPDATE_PAYLOAD_TOTAL_BYTES: usize = 1024 * 1024 * 1024;
/// Hard cap on the number of payload entries in one manifest.
pub const MAX_UPDATE_PAYLOAD_ENTRIES: usize = 64;
/// Maximum length of a payload name / vendor / device-class string.
pub const MAX_UPDATE_LABEL_BYTES: usize = 128;
/// Maximum length of one device-id string in the applicability allowlist.
pub const MAX_UPDATE_DEVICE_ID_BYTES: usize = 256;
/// Maximum number of explicit device ids in one applicability allowlist
/// (larger fleets use device-class applicability, not per-device lists).
pub const MAX_APPLICABILITY_DEVICE_IDS: usize = 4096;
/// The anti-rollback sequence floor: sequence numbers start at 1; zero is
/// refused (fail-closed — a zero sequence is a malformed manifest).
pub const ANTI_ROLLBACK_SEQUENCE_FLOOR: u64 = 1;

/// The P18.02 honesty statement (pinned so tests and reviewers can read
/// what IS and IS NOT claimed by this format module).
pub const UPDATE_BUNDLE_STATEMENT: &str = "the flexcore-edge-update-manifest-v2 format is a \
     FlexCore SIGNED-MANIFEST format shaped by the SUIT manifest concepts (RFC 9019 [S-162]) \
     and the TUF role model (ADR-1801 [S-161]); it is NOT a SUIT CBOR encoding and no \
     standards-conformance claim is made. No update bundle has ever been produced, signed, \
     staged, or applied. The sequence-comparison semantics are the strictly-safer reading \
     and are UNVERIFIED against the TUF/SUIT spec texts. The P9.34 gate remains the \
     verification authority; nothing here bypasses it.";

/// The anti-rollback semantics statement (recorded design + UNVERIFIED
/// label; see the module header).
pub const ANTI_ROLLBACK_SEMANTICS_STATEMENT: &str = "a manifest is accepted only when its \
     anti_rollback_sequence is STRICTLY GREATER than the device's last-seen sequence; the \
     counter advances only on a FULL verification pass, durably before the verdict is \
     returned. Consequence: a manifest whose install fails cannot be re-presented — a retry \
     requires re-issued metadata with a higher sequence. Rollback is unaffected: it \
     re-activates a previously-activated version through the P9.34 append-only register, \
     never by re-presenting a manifest. The strictly-greater-than rule is UNVERIFIED against \
     the TUF/SUIT spec texts (the implemented rule is the strictly-safer reading).";

// --- Errors -----------------------------------------------------------------------

/// Named errors of the bundle format and the device-side metadata checks.
/// Every failure is named ([`UpdateBundleError::name`]); nothing is a
/// silent pass. Where a P9.34 check refuses, the P9.34 stable refusal
/// code is carried verbatim in [`UpdateBundleError::BaseVerificationRefused`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateBundleError {
    /// No manifest was supplied — an unsigned bundle is refused before
    /// anything else (the P7.14/P9.34 rule, mirrored).
    UnsignedBundle,
    /// The v2 manifest failed validation (base-manifest violations are
    /// folded into the reason with the P9.34 error text).
    ManifestInvalid { reason: String },
    /// The manifest is not applicable to THIS device (vendor, device
    /// class, or device-id allowlist).
    ApplicabilityMismatch { reason: String },
    /// The manifest's mandatory expiry has passed — a stale manifest is
    /// refused (expiry discipline, ADR-1801 §3.3 residual).
    Expired { expires_at_unix_millis: i64, now_unix_millis: i64 },
    /// The manifest's anti-rollback sequence is not strictly ahead of the
    /// device's last-seen sequence — refused (never a silent reinstall).
    SequenceNotAhead { declared: u64, last_seen: u64 },
    /// The anti-rollback counter could not be read — fail-closed: a check
    /// that cannot read the counter NEVER reports "ahead".
    CounterUnavailable { reason: String },
    /// The anti-rollback counter I/O failed; the sequence was NOT
    /// recorded and the verdict is refused (fail-closed).
    CounterIo { reason: String },
    /// The file exists but is not a FlexCore edge anti-rollback counter
    /// (wrong magic or version) — refused, never guessed at.
    NotAnAntiRollbackCounter { reason: String },
    /// The bundle bytes do not cover a declared payload range.
    PayloadBoundsOutside { payload_name: String },
    /// One payload's bytes do not match its declared digest.
    PayloadDigestMismatch { payload_name: String },
    /// The configured verifier could not verify a payload at all (not
    /// configured, backend unavailable). Never treated as a pass.
    PayloadVerifierRefused { payload_name: String, reason: String },
    /// The P9.34 base gate refused — the stable P9.34 refusal code is
    /// carried verbatim (`UpdateVerificationError::name`).
    BaseVerificationRefused { code: String, reason: String },
    /// The bundle itself exceeds the total-size cap.
    BundleTooLarge { size_bytes: usize, cap: usize },
    /// An input was unusable (empty path, empty device identity, …).
    InvalidInput { reason: String },
}

impl UpdateBundleError {
    /// The stable machine-readable error name (the named-error table key).
    pub fn name(&self) -> &'static str {
        match self {
            UpdateBundleError::UnsignedBundle => "UPDATE_UNSIGNED_BUNDLE",
            UpdateBundleError::ManifestInvalid { .. } => "UPDATE_MANIFEST_INVALID",
            UpdateBundleError::ApplicabilityMismatch { .. } => "UPDATE_NOT_APPLICABLE",
            UpdateBundleError::Expired { .. } => "UPDATE_MANIFEST_EXPIRED",
            UpdateBundleError::SequenceNotAhead { .. } => "UPDATE_SEQUENCE_NOT_AHEAD",
            UpdateBundleError::CounterUnavailable { .. } => "UPDATE_COUNTER_UNAVAILABLE",
            UpdateBundleError::CounterIo { .. } => "UPDATE_COUNTER_IO",
            UpdateBundleError::NotAnAntiRollbackCounter { .. } => {
                "UPDATE_NOT_AN_ANTI_ROLLBACK_COUNTER"
            }
            UpdateBundleError::PayloadBoundsOutside { .. } => "UPDATE_PAYLOAD_BOUNDS_OUTSIDE",
            UpdateBundleError::PayloadDigestMismatch { .. } => "UPDATE_PAYLOAD_DIGEST_MISMATCH",
            UpdateBundleError::PayloadVerifierRefused { .. } => "UPDATE_PAYLOAD_VERIFIER_REFUSED",
            UpdateBundleError::BaseVerificationRefused { .. } => "UPDATE_BASE_VERIFICATION_REFUSED",
            UpdateBundleError::BundleTooLarge { .. } => "UPDATE_BUNDLE_TOO_LARGE",
            UpdateBundleError::InvalidInput { .. } => "UPDATE_INPUT_INVALID",
        }
    }
}

impl std::fmt::Display for UpdateBundleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UpdateBundleError::UnsignedBundle => write!(
                f,
                "no update manifest was supplied; an unsigned bundle is refused before any \
                 other check (the P9.34 rule, mirrored)"
            ),
            UpdateBundleError::ManifestInvalid { reason } => {
                write!(f, "the v2 update manifest is invalid: {reason}")
            }
            UpdateBundleError::ApplicabilityMismatch { reason } => write!(
                f,
                "the update is not applicable to this device: {reason} (SUIT-shaped \
                 applicability metadata, [S-162]-shaped; refused BEFORE any digest work)"
            ),
            UpdateBundleError::Expired { expires_at_unix_millis, now_unix_millis } => write!(
                f,
                "the manifest expired at {expires_at_unix_millis} (now {now_unix_millis}) — \
                 a stale manifest is refused (expiry discipline; freshness is carried by the \
                 mandatory expiry field until a full TUF-style timestamp role is operated)"
            ),
            UpdateBundleError::SequenceNotAhead { declared, last_seen } => write!(
                f,
                "the manifest sequence {declared} is not strictly ahead of the last-seen \
                 sequence {last_seen} — refused (anti-rollback: reinstalling an old, \
                 potentially vulnerable manifest is never silent; {ANTI_ROLLBACK_SEMANTICS_STATEMENT})"
            ),
            UpdateBundleError::CounterUnavailable { reason } => write!(
                f,
                "the anti-rollback counter could not be read: {reason} (fail-closed: a check \
                 that cannot read the counter never reports 'ahead')"
            ),
            UpdateBundleError::CounterIo { reason } => write!(
                f,
                "the anti-rollback counter append failed: {reason} — the sequence was NOT \
                 recorded and the verdict is refused (fail-closed)"
            ),
            UpdateBundleError::NotAnAntiRollbackCounter { reason } => write!(
                f,
                "the file is not a FlexCore edge anti-rollback counter (refused, never \
                 guessed): {reason}"
            ),
            UpdateBundleError::PayloadBoundsOutside { payload_name } => write!(
                f,
                "payload '{payload_name}' declares a byte range outside the bundle — refused, \
                 never truncated or guessed at"
            ),
            UpdateBundleError::PayloadDigestMismatch { payload_name } => write!(
                f,
                "payload '{payload_name}' does not match its declared digest — refused BEFORE \
                 any file is written"
            ),
            UpdateBundleError::PayloadVerifierRefused { payload_name, reason } => write!(
                f,
                "the configured verifier refused to verify payload '{payload_name}' \
                 (fail-closed): {reason}"
            ),
            UpdateBundleError::BaseVerificationRefused { code, reason } => write!(
                f,
                "the P9.34 base gate refused ({code}): {reason} — the preserved refusal \
                 order, verbatim"
            ),
            UpdateBundleError::BundleTooLarge { size_bytes, cap } => write!(
                f,
                "the bundle is {size_bytes} bytes; the total-size cap is {cap} bytes — \
                 refused (a code maximum; deployments may tighten, never loosen)"
            ),
            UpdateBundleError::InvalidInput { reason } => {
                write!(f, "the update input is invalid: {reason}")
            }
        }
    }
}

impl std::error::Error for UpdateBundleError {}

// --- Applicability metadata (SUIT-shaped) ---------------------------------------------

/// The applicability metadata: WHICH devices may install the manifest
/// ([S-162]-shaped concept; a FlexCore field, not a SUIT encoding).
///
/// Semantics (fail-closed, recorded):
/// - `vendor` and `device_class` are BOTH mandatory and must BOTH match
///   the device exactly (case-sensitive; no wildcard vocabulary exists);
/// - `device_ids` EMPTY means "every device of this vendor+device class";
///   NON-EMPTY means "only the listed device ids" — the two readings are
///   the closed semantics, there is no third reading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateApplicability {
    /// The device vendor the manifest applies to (mandatory match).
    pub vendor: String,
    /// The device class the manifest applies to (mandatory match; the
    /// class vocabulary is the fleet registry's — the P18.12 schema,
    /// consumed by citation, never redefined here).
    pub device_class: String,
    /// The explicit device-id allowlist (empty = all devices of the
    /// vendor+class; see the type doc for the closed semantics).
    pub device_ids: BTreeSet<String>,
}

impl UpdateApplicability {
    /// Validates the applicability metadata (fail-closed; every violation
    /// is reported into the caller's problem list).
    fn validate_into(&self, problems: &mut Vec<String>) {
        if self.vendor.trim().is_empty() || self.vendor.len() > MAX_UPDATE_LABEL_BYTES {
            problems.push(format!(
                "applicability.vendor must be a non-empty string of at most \
                 {MAX_UPDATE_LABEL_BYTES} characters"
            ));
        }
        if self.device_class.trim().is_empty()
            || self.device_class.len() > MAX_UPDATE_LABEL_BYTES
        {
            problems.push(format!(
                "applicability.device_class must be a non-empty string of at most \
                 {MAX_UPDATE_LABEL_BYTES} characters"
            ));
        }
        if self.device_ids.len() > MAX_APPLICABILITY_DEVICE_IDS {
            problems.push(format!(
                "applicability.device_ids exceeds {MAX_APPLICABILITY_DEVICE_IDS} entries — \
                 use device-class applicability, not per-device lists"
            ));
        }
        for id in &self.device_ids {
            if id.trim().is_empty() || id.len() > MAX_UPDATE_DEVICE_ID_BYTES {
                problems.push(
                    "every applicability device id must be a non-empty string of at most 256 \
                     characters"
                        .to_string(),
                );
                break;
            }
        }
    }

    /// Does this applicability match the given device identity? The
    /// device identity comes from the edge's OWN configured/enrolled
    /// identity — never from the manifest or the network.
    pub fn matches(&self, vendor: &str, device_class: &str, device_id: &str) -> bool {
        if self.vendor != vendor || self.device_class != device_class {
            return false;
        }
        if self.device_ids.is_empty() {
            true
        } else {
            self.device_ids.contains(device_id)
        }
    }
}

// --- Per-payload digests ------------------------------------------------------------

/// One payload entry: a named byte range inside the bundle with its own
/// declared digest and size (the manifest-enclosed-payload-digest concept
/// shaped by [S-162]; the bundle-level digest in the P9.34 base manifest
/// commits to the WHOLE bundle, these entries commit to the individual
/// payloads inside it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayloadDigest {
    /// The payload name (unique within the manifest).
    pub name: String,
    /// The byte offset of the payload inside the bundle.
    pub offset: usize,
    /// The payload size in bytes (>= 1: a zero-byte payload entry is
    /// refused as malformed).
    pub size_bytes: usize,
    /// The payload digest as lowercase hex (same shape rule as the P9.34
    /// `bundle_digest_hex`).
    pub digest_hex: String,
}

impl PayloadDigest {
    fn validate_into(&self, problems: &mut Vec<String>) {
        if self.name.trim().is_empty() || self.name.len() > MAX_UPDATE_LABEL_BYTES {
            problems.push(
                "every payload name must be a non-empty string of at most 128 characters"
                    .to_string(),
            );
        }
        if self.size_bytes == 0 {
            problems.push(format!("payload '{}' size_bytes must be >= 1", self.name));
        }
        if self.size_bytes > MAX_UPDATE_PAYLOAD_ENTRY_BYTES {
            problems.push(format!(
                "payload '{}' declares {size} bytes; the per-entry cap is {cap} bytes",
                self.name,
                size = self.size_bytes,
                cap = MAX_UPDATE_PAYLOAD_ENTRY_BYTES,
            ));
        }
        let hex_ok = self.digest_hex.len() >= 8
            && self.digest_hex.len() <= 512
            && self.digest_hex.len() % 2 == 0
            && self
                .digest_hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b.is_ascii_lowercase() && b.is_ascii_hexdigit()));
        if !hex_ok {
            problems.push(format!(
                "payload '{}' digest_hex must be an even-length lowercase hex string of \
                 8..=512 digits",
                self.name
            ));
        }
    }
}

// --- The v2 manifest ------------------------------------------------------------------

/// The `flexcore-edge-update-manifest-v2` manifest: the P9.34
/// `UpdateManifest` (v1) fields COMPOSED (not forked) with the v2
/// additions — applicability, anti-rollback sequence, per-payload
/// digests, mandatory expiry. The base manifest is validated by the
/// P9.34 rules exactly as shipped; the v2 fields add strictly more
/// refusals, never fewer.
///
/// NO key material is structurally possible (the base manifest's closed
/// scheme set carries over; a scheme-less signer reference is refused by
/// the P9.34 validation as likely inline key material).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtendedUpdateManifest {
    /// The P9.34 v1 manifest (package id, version, bundle digest,
    /// algorithm, signer key REFERENCE, minimum-version floor, approval
    /// reference) — validated by the P9.34 rules verbatim.
    pub base: UpdateManifest,
    /// SUIT-shaped applicability metadata ([S-162]-shaped).
    pub applicability: UpdateApplicability,
    /// The anti-rollback sequence number (monotonic; see
    /// [`ANTI_ROLLBACK_SEMANTICS_STATEMENT`]).
    pub anti_rollback_sequence: u64,
    /// The per-payload digests (at least one; no duplicate names; no
    /// overlapping byte ranges).
    pub payloads: Vec<PayloadDigest>,
    /// The mandatory expiry instant (unix millis, injected-clock
    /// comparable): a manifest that never expires cannot be refused as
    /// stale, so expiry is REQUIRED in v2 (the freshness residual of
    /// ADR-1801 §4 until a full timestamp role is operated).
    pub expires_at_unix_millis: i64,
}

impl ExtendedUpdateManifest {
    /// Validates the v2 manifest (fail-closed): the base manifest's P9.34
    /// validation runs first, then the v2-specific rules. Every violation
    /// is a named [`UpdateBundleError::ManifestInvalid`] with every
    /// problem listed.
    pub fn validate(&self) -> Result<(), UpdateBundleError> {
        let mut problems: Vec<String> = Vec::new();
        // Base manifest: the P9.34 validation text is carried verbatim.
        if let Err(error) = self.base.validate() {
            problems.push(format!("base manifest ({error})"));
        }
        self.applicability.validate_into(&mut problems);
        if self.anti_rollback_sequence < ANTI_ROLLBACK_SEQUENCE_FLOOR {
            problems.push(format!(
                "anti_rollback_sequence must be >= {ANTI_ROLLBACK_SEQUENCE_FLOOR} (zero is a \
                 malformed manifest, never a first version)"
            ));
        }
        if self.payloads.is_empty() {
            problems.push("payloads must contain at least one payload entry".to_string());
        }
        if self.payloads.len() > MAX_UPDATE_PAYLOAD_ENTRIES {
            problems.push(format!(
                "payloads exceeds {MAX_UPDATE_PAYLOAD_ENTRIES} entries"
            ));
        }
        let mut total: usize = 0;
        let mut names: BTreeSet<&str> = BTreeSet::new();
        for payload in &self.payloads {
            payload.validate_into(&mut problems);
            if !names.insert(payload.name.as_str()) {
                problems.push(format!("payload name '{}' appears twice", payload.name));
            }
            total = total.saturating_add(payload.size_bytes);
        }
        if total > MAX_UPDATE_PAYLOAD_TOTAL_BYTES {
            problems.push(format!(
                "the total declared payload size {total} exceeds the cap \
                 {MAX_UPDATE_PAYLOAD_TOTAL_BYTES} bytes"
            ));
        }
        // No overlapping byte ranges: sort by offset, adjacent is fine,
        // overlap is a malformed manifest.
        let mut ranges: Vec<(usize, usize)> =
            self.payloads.iter().map(|p| (p.offset, p.size_bytes)).collect();
        ranges.sort_by_key(|(offset, _)| *offset);
        for pair in ranges.windows(2) {
            let (prev_offset, prev_size) = pair[0];
            let (next_offset, _) = pair[1];
            if prev_offset.saturating_add(prev_size) > next_offset {
                problems.push(format!(
                    "payload byte ranges overlap (offset {prev_offset} + {prev_size} crosses \
                     {next_offset})"
                ));
                break;
            }
        }
        if self.expires_at_unix_millis <= 0 {
            problems.push("expires_at_unix_millis must be a positive unix-millis instant \
                           (expiry is mandatory in v2)"
                .to_string());
        }
        if problems.is_empty() {
            Ok(())
        } else {
            Err(UpdateBundleError::ManifestInvalid {
                reason: problems.join("; "),
            })
        }
    }

    /// Projects the v2 manifest onto the P9.34 v1 shape so the EXISTING
    /// `UpdateGate` verifies it behind the unchanged seam (the base fields
    /// ARE the v1 fields; nothing is restated or reinterpreted).
    pub fn base_manifest(&self) -> &UpdateManifest {
        &self.base
    }

    /// Applicability check against the device's OWN identity (fail-closed
    /// on mismatch: an update that is not demonstrably applicable is not
    /// installed "anyway").
    pub fn applicability_refuses(
        &self,
        vendor: &str,
        device_class: &str,
        device_id: &str,
    ) -> Result<(), UpdateBundleError> {
        if self.applicability.matches(vendor, device_class, device_id) {
            Ok(())
        } else {
            Err(UpdateBundleError::ApplicabilityMismatch {
                reason: format!(
                    "manifest applicability vendor='{}' device_class='{}' device_ids={} does \
                     not match this device (vendor/class mandatory match; the allowlist is \
                     {})",
                    self.applicability.vendor,
                    self.applicability.device_class,
                    if self.applicability.device_ids.is_empty() {
                        "empty (= all devices of the vendor+class)".to_string()
                    } else {
                        format!("{} explicit id(s)", self.applicability.device_ids.len())
                    },
                ),
            })
        }
    }

    /// Expiry check against the INJECTED clock's current instant (the
    /// caller supplies `now`; this module never reads wall time).
    pub fn expiry_refuses(&self, now_unix_millis: i64) -> Result<(), UpdateBundleError> {
        if now_unix_millis >= self.expires_at_unix_millis {
            Err(UpdateBundleError::Expired {
                expires_at_unix_millis: self.expires_at_unix_millis,
                now_unix_millis,
            })
        } else {
            Ok(())
        }
    }

    /// Sequence check against the last-seen value: STRICTLY greater than
    /// `last_seen` passes; anything else refuses
    /// ([`ANTI_ROLLBACK_SEMANTICS_STATEMENT`]).
    pub fn sequence_refuses(&self, last_seen: u64) -> Result<(), UpdateBundleError> {
        if self.anti_rollback_sequence > last_seen {
            Ok(())
        } else {
            Err(UpdateBundleError::SequenceNotAhead {
                declared: self.anti_rollback_sequence,
                last_seen,
            })
        }
    }

    /// The version-ordering helper is re-exported through this module's
    /// use of the P9.34 numeric comparison (used by callers validating
    /// floor relationships; the comparison itself is the P9.34 function,
    /// cited not reimplemented).
    pub fn version_ordering(
        a: &str,
        b: &str,
    ) -> Result<std::cmp::Ordering, UpdateVerificationError> {
        compare_versions(a, b)
    }
}

// --- The device-side anti-rollback counter ---------------------------------------------

/// The file-backed MONOTONIC anti-rollback counter (the device-side
/// last-seen sequence register). Same framing discipline as the P9.34
/// update register / P9.31 binding journal / P9.33 identity register:
///
/// ```text
/// header:  b"FLEXCORE-EDGE-ANTIROLLBACK" + format version byte (0x01)
/// record:  [u32 LE payload_len][u32 LE crc32(payload)][payload]
/// payload: [u64 LE sequence]
/// ```
///
/// Append-only (nothing edited or removed in place); truncate-and-audit
/// recovery; durable BEFORE the in-memory view moves; the LAST-SEEN
/// sequence is DERIVED by replay as the MAXIMUM accepted value (the
/// register is the source of truth; monotonic by construction —
/// [`AntiRollbackCounter::accept`] refuses any value not strictly ahead).
/// The CRC-32 is the repo's integrity-checksum family (corruption
/// detection, not a security primitive).
pub struct AntiRollbackCounter {
    path: PathBuf,
    file: Mutex<std::fs::File>,
    last_seen: Mutex<u64>,
    records_accepted: std::sync::atomic::AtomicU64,
    corrupt_tail_truncated: std::sync::atomic::AtomicU64,
}

impl AntiRollbackCounter {
    /// The counter magic header + format version.
    fn magic() -> Vec<u8> {
        let mut magic = b"FLEXCORE-EDGE-ANTIROLLBACK".to_vec();
        magic.push(0x01);
        magic
    }

    /// Opens (or creates) the counter at `path` and replays any existing
    /// records. A corrupt tail is truncated and COUNTED, never silent; a
    /// wrong magic/version is a hard open error (fail-closed — an
    /// unreadable counter refuses every manifest
    /// [`UpdateBundleError::CounterUnavailable`]).
    pub fn open(path: &Path) -> Result<Self, UpdateBundleError> {
        if path.as_os_str().is_empty() {
            return Err(UpdateBundleError::CounterUnavailable {
                reason: "the anti-rollback counter path is empty".to_string(),
            });
        }
        let magic = Self::magic();
        let (file, replayed_last_seen, replay_corrupt_truncated) = match std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
        {
            Ok(mut existing) => {
                let mut contents = Vec::new();
                existing
                    .read_to_end(&mut contents)
                    .map_err(|e| UpdateBundleError::CounterUnavailable {
                        reason: format!("reading the existing anti-rollback counter failed: {e}"),
                    })?;
                if contents.is_empty() {
                    (existing, 0u64, 0u64)
                } else {
                    if contents.len() < magic.len() || contents[..magic.len()] != magic[..] {
                        return Err(UpdateBundleError::NotAnAntiRollbackCounter {
                            reason: format!(
                                "the file at {} does not carry the FlexCore edge \
                                 anti-rollback counter magic",
                                path.display()
                            ),
                        });
                    }
                    let mut last_seen: u64 = 0;
                    let mut offset = magic.len();
                    let mut corrupt_tail_truncated: u64 = 0;
                    while offset + 8 <= contents.len() {
                        let len = u32::from_le_bytes(
                            contents[offset..offset + 4].try_into().expect("4 bytes"),
                        );
                        if len != 8 || offset + 8 + len as usize > contents.len() {
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
                        let sequence =
                            u64::from_le_bytes(payload.try_into().expect("8 bytes"));
                        last_seen = last_seen.max(sequence);
                        offset += 8 + len as usize;
                    }
                    if offset < contents.len() && corrupt_tail_truncated == 0 {
                        corrupt_tail_truncated += 1;
                    }
                    if corrupt_tail_truncated > 0 {
                        tracing::warn!(
                            truncated_records = corrupt_tail_truncated,
                            path = %path.display(),
                            "anti-rollback counter: corrupt tail truncated; the last-seen \
                             sequence is the maximum of the GOOD records (never lower)"
                        );
                        existing
                            .set_len(offset as u64)
                            .map_err(|e| UpdateBundleError::CounterIo {
                                reason: format!(
                                    "truncating the corrupt anti-rollback-counter tail \
                                     failed: {e}"
                                ),
                            })?;
                    }
                    (existing, last_seen, corrupt_tail_truncated)
                }
            }
            Err(_) => {
                let mut created = std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create_new(true)
                    .open(path)
                    .map_err(|e| UpdateBundleError::CounterIo {
                        reason: format!("creating the anti-rollback counter file failed: {e}"),
                    })?;
                created
                    .write_all(&magic)
                    .and_then(|_| created.flush())
                    .map_err(|e| UpdateBundleError::CounterIo {
                        reason: format!("writing the anti-rollback counter header failed: {e}"),
                    })?;
                (created, 0u64, 0u64)
            }
        };
        Ok(Self {
            path: path.to_path_buf(),
            file: Mutex::new(file),
            last_seen: Mutex::new(replayed_last_seen),
            records_accepted: std::sync::atomic::AtomicU64::new(0),
            corrupt_tail_truncated: std::sync::atomic::AtomicU64::new(replay_corrupt_truncated),
        })
    }

    /// The counter path (diagnostics).
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The device's LAST-SEEN sequence (0 = nothing accepted yet; the
    /// first manifest must carry a sequence >= 1).
    pub fn last_seen(&self) -> u64 {
        *self
            .last_seen
            .lock()
            .expect("poisoned anti-rollback counter mutex")
    }

    /// How many corrupt tail records were truncated at open.
    pub fn corrupt_tail_truncated(&self) -> u64 {
        self.corrupt_tail_truncated
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    /// How many sequences this instance has accepted (diagnostics).
    pub fn records_accepted(&self) -> u64 {
        self.records_accepted
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Accepts a sequence: refuses zero, refuses anything not STRICTLY
    /// ahead of the last-seen value, appends durably (flush + sync_data
    /// BEFORE the in-memory view moves), and only then updates the
    /// in-memory last-seen value. The caller is the extended gate's
    /// full-pass path — this method is public so the composition is
    /// testable, but the ONLY production caller is
    /// `crate::update_session::ExtendedUpdateGate`.
    pub fn accept(&self, sequence: u64) -> Result<u64, UpdateBundleError> {
        if sequence < ANTI_ROLLBACK_SEQUENCE_FLOOR {
            return Err(UpdateBundleError::InvalidInput {
                reason: format!(
                    "the sequence {sequence} is below the floor {ANTI_ROLLBACK_SEQUENCE_FLOOR}"
                ),
            });
        }
        let payload = sequence.to_le_bytes();
        let record_len = payload.len() as u32;
        let crc = crc32(&payload);
        let mut framed = Vec::with_capacity(8 + payload.len());
        framed.extend_from_slice(&record_len.to_le_bytes());
        framed.extend_from_slice(&crc.to_le_bytes());
        framed.extend_from_slice(&payload);
        {
            let mut file = self
                .file
                .lock()
                .expect("poisoned anti-rollback counter file mutex");
            file.seek(SeekFrom::End(0))
                .and_then(|_| file.write_all(&framed))
                .and_then(|_| file.flush())
                .and_then(|_| file.sync_data())
                .map_err(|e| UpdateBundleError::CounterIo {
                    reason: format!("appending the anti-rollback counter record failed: {e}"),
                })?;
        }
        let mut last_seen = self
            .last_seen
            .lock()
            .expect("poisoned anti-rollback counter mutex");
        // Re-check under the lock (two gates sharing one counter must not
        // race a value backwards).
        if sequence <= *last_seen {
            return Err(UpdateBundleError::SequenceNotAhead {
                declared: sequence,
                last_seen: *last_seen,
            });
        }
        *last_seen = sequence;
        self.records_accepted
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(sequence)
    }
}

// --- Tests (WRITTEN-UNRUN; execution is a human gate) -----------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::update_verification::TestOnlyFnv1a64UpdateVerifier;

    /// A v1 base manifest that passes the P9.34 validation (test-grade
    /// FNV-1a-64 algorithm, the loudly-labelled test verifier).
    fn base_manifest(version: &str) -> UpdateManifest {
        UpdateManifest {
            package_id: "edge-agent-release".to_string(),
            version: version.to_string(),
            bundle_digest_hex: "0011223344556677".to_string(),
            digest_algorithm: "fnv1a64-test".to_string(),
            signer_key_reference: "key-store:flexcore-release-targets".to_string(),
            minimum_version_floor: "0.0.0".to_string(),
            approval_reference: "approval-record-1".to_string(),
        }
    }

    fn applicability() -> UpdateApplicability {
        UpdateApplicability {
            vendor: "neryva".to_string(),
            device_class: "edge-gateway".to_string(),
            device_ids: BTreeSet::new(),
        }
    }

    fn payload(name: &str, offset: usize, size: usize, digest: &str) -> PayloadDigest {
        PayloadDigest {
            name: name.to_string(),
            offset,
            size_bytes: size,
            digest_hex: digest.to_string(),
        }
    }

    fn v2_manifest(sequence: u64) -> ExtendedUpdateManifest {
        ExtendedUpdateManifest {
            base: base_manifest("1.2.0"),
            applicability: applicability(),
            anti_rollback_sequence: sequence,
            payloads: vec![payload("fw.bin", 0, 16, "aabbccddeeff0011")],
            expires_at_unix_millis: 1_800_000_000_000,
        }
    }

    #[test]
    fn a_well_formed_v2_manifest_validates() {
        v2_manifest(1).validate().expect("validates");
    }

    #[test]
    fn a_zero_sequence_is_refused() {
        let error = v2_manifest(0).validate().expect_err("refused");
        assert!(matches!(error, UpdateBundleError::ManifestInvalid { .. }));
        assert_eq!(error.name(), "UPDATE_MANIFEST_INVALID");
    }

    #[test]
    fn a_base_manifest_violation_is_carried_into_the_v2_reason() {
        let mut manifest = v2_manifest(1);
        manifest.base.approval_reference = String::new();
        let error = manifest.validate().expect_err("refused");
        let UpdateBundleError::ManifestInvalid { reason } = error else {
            panic!("expected ManifestInvalid, got {error:?}")
        };
        assert!(
            reason.contains("approval_reference"),
            "the P9.34 reason text rides the v2 refusal: {reason}"
        );
    }

    #[test]
    fn duplicate_payload_names_and_overlapping_ranges_are_refused() {
        let mut manifest = v2_manifest(1);
        manifest.payloads.push(payload("fw.bin", 8, 4, "1122334455667788"));
        let error = manifest.validate().expect_err("refused");
        let UpdateBundleError::ManifestInvalid { reason } = error else {
            panic!("expected ManifestInvalid, got {error:?}")
        };
        assert!(reason.contains("twice"), "duplicate name caught: {reason}");
        assert!(reason.contains("overlap"), "overlap caught: {reason}");
    }

    #[test]
    fn an_empty_payload_list_and_zero_size_entries_are_refused() {
        let mut manifest = v2_manifest(1);
        manifest.payloads.clear();
        assert!(matches!(
            manifest.validate(),
            Err(UpdateBundleError::ManifestInvalid { .. })
        ));
        let mut manifest = v2_manifest(1);
        manifest.payloads[0].size_bytes = 0;
        assert!(matches!(
            manifest.validate(),
            Err(UpdateBundleError::ManifestInvalid { .. })
        ));
    }

    #[test]
    fn total_and_entry_size_caps_are_enforced() {
        let mut manifest = v2_manifest(1);
        manifest.payloads[0].size_bytes = MAX_UPDATE_PAYLOAD_ENTRY_BYTES + 1;
        assert!(matches!(
            manifest.validate(),
            Err(UpdateBundleError::ManifestInvalid { .. })
        ));
        // Total cap: many maximal entries push the sum over the total cap.
        let mut manifest = v2_manifest(1);
        manifest.payloads.clear();
        for i in 0..8 {
            manifest.payloads.push(payload(
                &format!("part-{i}"),
                i * MAX_UPDATE_PAYLOAD_ENTRY_BYTES,
                MAX_UPDATE_PAYLOAD_ENTRY_BYTES,
                "aabbccddeeff0011",
            ));
        }
        let error = manifest.validate().expect_err("refused");
        let UpdateBundleError::ManifestInvalid { reason } = error else {
            panic!("expected ManifestInvalid, got {error:?}")
        };
        assert!(reason.contains("total declared payload size"), "{reason}");
    }

    #[test]
    fn applicability_matching_is_exact_and_the_allowlist_is_closed() {
        let manifest = v2_manifest(1);
        // Empty allowlist: any device of the vendor+class matches.
        assert!(manifest
            .applicability
            .matches("neryva", "edge-gateway", "device-7"));
        assert!(!manifest
            .applicability
            .matches("other", "edge-gateway", "device-7"));
        assert!(!manifest
            .applicability
            .matches("neryva", "other-class", "device-7"));
        // Non-empty allowlist: only listed ids match.
        let mut manifest = v2_manifest(1);
        manifest.applicability.device_ids =
            BTreeSet::from(["device-3".to_string(), "device-9".to_string()]);
        assert!(manifest
            .applicability
            .matches("neryva", "edge-gateway", "device-9"));
        assert!(!manifest
            .applicability
            .matches("neryva", "edge-gateway", "device-7"));
        // The gate-level check refuses with the named error.
        assert!(matches!(
            manifest.applicability_refuses("neryva", "edge-gateway", "device-7"),
            Err(UpdateBundleError::ApplicabilityMismatch { .. })
        ));
    }

    #[test]
    fn expiry_is_refused_at_and_after_the_instant() {
        let manifest = v2_manifest(1);
        manifest
            .expiry_refuses(1_799_999_999_999)
            .expect("before expiry passes");
        assert!(matches!(
            manifest.expiry_refuses(1_800_000_000_000),
            Err(UpdateBundleError::Expired { .. })
        ));
        assert!(matches!(
            manifest.expiry_refuses(1_800_000_000_001),
            Err(UpdateBundleError::Expired { .. })
        ));
    }

    #[test]
    fn sequence_must_be_strictly_ahead() {
        let manifest = v2_manifest(5);
        manifest.sequence_refuses(4).expect("4 -> 5 passes");
        assert!(matches!(
            manifest.sequence_refuses(5),
            Err(UpdateBundleError::SequenceNotAhead { declared: 5, last_seen: 5 })
        ));
        assert!(matches!(
            manifest.sequence_refuses(6),
            Err(UpdateBundleError::SequenceNotAhead { .. })
        ));
        // The statement is pinned so the semantics stay reviewable.
        assert!(ANTI_ROLLBACK_SEMANTICS_STATEMENT.contains("STRICTLY GREATER"));
        assert!(ANTI_ROLLBACK_SEMANTICS_STATEMENT.contains("UNVERIFIED"));
    }

    #[test]
    fn the_counter_is_monotonic_and_durable_by_construction() {
        let dir = std::env::temp_dir().join(format!(
            "flexcore-p18-antirb-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("antirb.counter");
        let _ = std::fs::remove_file(&path);
        let counter = AntiRollbackCounter::open(&path).expect("opens");
        assert_eq!(counter.last_seen(), 0, "fresh counter has seen nothing");
        counter.accept(3).expect("3 accepted");
        assert_eq!(counter.last_seen(), 3);
        assert!(matches!(
            counter.accept(3),
            Err(UpdateBundleError::SequenceNotAhead { declared: 3, last_seen: 3 })
        ));
        assert!(matches!(
            counter.accept(2),
            Err(UpdateBundleError::SequenceNotAhead { declared: 2, last_seen: 3 })
        ));
        counter.accept(4).expect("4 accepted");
        // Reopen: the last-seen value is derived by replay (the register
        // is the source of truth).
        let reopened = AntiRollbackCounter::open(&path).expect("reopens");
        assert_eq!(reopened.last_seen(), 4);
        // Zero is refused.
        assert!(matches!(
            reopened.accept(0),
            Err(UpdateBundleError::InvalidInput { .. })
        ));
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn a_foreign_file_is_refused_as_not_a_counter() {
        let dir = std::env::temp_dir().join(format!(
            "flexcore-p18-antirb-foreign-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("foreign.counter");
        std::fs::write(&path, b"definitely not a counter").expect("writes");
        assert!(matches!(
            AntiRollbackCounter::open(&path),
            Err(UpdateBundleError::NotAnAntiRollbackCounter { .. })
        ));
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn the_version_ordering_helper_is_the_p9_34_function() {
        assert_eq!(
            ExtendedUpdateManifest::version_ordering("1.2.0", "1.10.0")
                .expect("parses"),
            std::cmp::Ordering::Less
        );
    }

    #[test]
    fn the_statements_pin_the_honesty_bounds() {
        assert!(UPDATE_BUNDLE_STATEMENT.contains("NOT a SUIT CBOR encoding"));
        assert!(UPDATE_BUNDLE_STATEMENT.contains("never been produced, signed, staged"));
        assert_eq!(UPDATE_BUNDLE_FORMAT_V2, "flexcore-edge-update-manifest-v2");
        // The test-grade verifier label carries over: the format tests use
        // the loudly-labelled test verifier, never a production claim.
        assert_eq!(TestOnlyFnv1a64UpdateVerifier.algorithm_name(), "fnv1a64-test");
    }
}
