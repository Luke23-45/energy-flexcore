// FIM-F12.01 + FIM-F12.02 (Wave 6, final implementation milestone) — the
//         TUF-SHAPED OTA metadata repository and bundle PRODUCER, plus the
//         device-side artifact verification that drives the produced
//         bundle through the EXISTING P18.05 extended gate.
//
// IMPLEMENTED-UNDER-FIM-F12, WRITTEN-UNRUN: this module has never been
// compiled or executed; no bundle has ever been produced, signed, staged,
// or applied (the producing code EXISTS now — the P18.02 statement in
// `crate::update_bundle` carries the honest wording — but nothing has ever
// RUN it). Every exercise is a WRITTEN-UNRUN test with fixed, loudly-
// labelled TEST-GRADE key material; the P18.03 key ceremonies are HUMAN
// GATES and no key material ships.
//
// THE TUF-SHAPED LAYOUT (the honesty boundary, restated): the repository
// metadata roles are SHAPED BY the TUF role model (root / targets /
// timestamp / snapshot; [S-161] as summarized in phase-18 record 03 §1) —
// this is a FlexCore format (`flexcore-ota-repository-v1`), NOT a TUF
// encoding, and NO TUF-conformance claim is made anywhere. The role
// semantics are UNVERIFIED against the TUF specification text. The
// role-to-FlexCore mapping (record 03, consumed by citation):
//
// | Role      | Signs                | FlexCore shape                                    |
// |---|---|---|
// | root      | the trust-anchor set | `OtaRootMetadata` (signer references + Ed25519 public keys); the DEVICE's configured anchors govern verification — the artifact can never add an anchor the device does not hold (no blind trust swap; the N+1 rotation overlap of record 03 §3 is the designed-in next step, not implemented) |
// | targets   | the target digests   | the `ExtendedUpdateManifest` (the v2 manifest IS the targets content; its per-payload entries carry Ed25519 signatures under the `ed25519` algorithm) |
// | timestamp | freshness            | `OtaTimestampMetadata` (binds the targets metadata by SHA-256 + carries its own expiry) |
// | snapshot  | consistency          | `OtaSnapshotMetadata` (binds root/timestamp/targets versions, the targets digest, and the bundle digest by SHA-256) |
//
// DELIBERATE DELTA from TUF (recorded, not a conformance gap): TUF carries
// target DIGESTS inside signed metadata and signs the metadata; FlexCore
// v1 ADDITIONALLY signs each payload and the whole bundle directly
// (per-payload signatures inside the signed manifest) — strictly more
// device-side verification work, and the reason the manifest's
// `digest_algorithm` is the signature algorithm `ed25519` rather than a
// hash name. SHA-256 (the P11.15 std-only implementation, `flexcore_audit::
// chain::sha256`, consumed never duplicated) is the repository-side
// CONSISTENCY binding (timestamp/snapshot digests), not the authorization.
//
// SIGNING DISCIPLINE: every signature is over bytes that start with a
// domain string (`crate::ota_signing` — the Patch B discipline); each
// metadata role's canonical bytes start with the metadata domain + the
// role code byte. The canonical encodings below are the ONLY byte
// constructions both sides use (one builder per role, no drift; the
// verifier re-derives from the structured view and compares
// byte-for-byte BEFORE any signature work).
//
// ANTI-ROLLBACK (honoring the existing counters exactly): the producer
// keeps the repository-side counters (`OtaRepositoryCounters`) and refuses
// any manifest sequence or role version that is not STRICTLY ahead of the
// last-issued value (`OtaProduceError::SequenceNotAhead` — the SAME stable
// refusal code and the SAME strictly-greater-than semantics as the device
// counter, `crate::update_bundle::ANTI_ROLLBACK_SEMANTICS_STATEMENT`; that
// rule is UNVERIFIED against the TUF/SUIT spec texts, the implemented rule
// is the strictly-safer reading on BOTH sides). A produced bundle can
// therefore never re-present an already-issued sequence.
//
// THE DEVICE-SIDE COMPOSITION (FIM-F12.02): `OtaArtifactVerifier::verify`
// checks the metadata signatures against the DEVICE-CONFIGURED anchors,
// checks the consistency bindings, and then calls the UNTOUCHED
// `crate::update_session::ExtendedUpdateGate::verify_fleet_bundle` — the
// P9.34 refusal order, its audit, its no-write structure, the v2 checks,
// and the durable anti-rollback-counter advance are the EXISTING code,
// not reimplemented. A tampered bundle refuses through the EXISTING
// `UpdateVerificationError` taxonomy (riding `OtaArtifactError::GateRefused`
// with the stable P9.34 code — the `BaseVerificationRefused` pattern).
//
// NO WRITE PATH: the producer assembles and signs IN MEMORY and returns
// the artifact; nothing here publishes, uploads, stages, or applies
// anything. The transport and the repository service are deployment
// concerns (record 03: the repository is not stood up in this program).

use std::collections::BTreeSet;
use std::sync::Arc;

use ed25519_dalek::Signature;
use flexcore_audit::chain::{hex_encode_digest, sha256};
use flexcore_ports::clock::Clock;

use crate::ota_signing::{
    bundle_signing_bytes, signature_to_hex, verify_signature_under_anchor, OtaEd25519ReleaseKey,
    OtaTrustAnchor, OTA_ED25519_ALGORITHM, OTA_ED25519_SIGNATURE_BYTES,
};
use crate::ports::{AuditSink, EdgeAuditEvent, EdgeAuditKind};
use crate::update_bundle::{
    ANTI_ROLLBACK_SEMANTICS_STATEMENT, ExtendedUpdateManifest, PayloadDigest,
    UpdateApplicability, UpdateBundleError, UpdateManifest, MAX_UPDATE_LABEL_BYTES,
    MAX_UPDATE_PAYLOAD_ENTRIES, MAX_UPDATE_PAYLOAD_ENTRY_BYTES, MAX_UPDATE_PAYLOAD_TOTAL_BYTES,
};
use crate::update_session::{ExtendedUpdateGate, VerifiedExtendedUpdate};

// --- Constants -----------------------------------------------------------------

/// The repository metadata format id (a FlexCore format; NOT a TUF
/// encoding and no standards-conformance claim is made).
pub const OTA_REPOSITORY_FORMAT: &str = "flexcore-ota-repository-v1";
/// The metadata canonical-encoding format version byte.
pub const OTA_REPOSITORY_FORMAT_VERSION: u8 = 0x01;

/// The FIM-F12 producer/verifier honesty statement (pinned so tests and
/// reviewers can read what IS and IS NOT claimed).
pub const OTA_PRODUCE_STATEMENT: &str = "the FIM-F12 producer assembles and Ed25519-signs \
     TUF-SHAPED repository metadata (root/targets/timestamp/snapshot) and the update \
     bundle IN MEMORY; it is a FlexCore format, NOT a TUF encoding, and no \
     TUF-conformance claim is made (role semantics UNVERIFIED against the TUF spec \
     text). The producer never verifies anything against the test-grade FNV verifier in \
     any production path — produced bundles declare the ed25519 algorithm and verify \
     through the EXISTING extended gate with the Ed25519 verifier over device-configured \
     trust anchors; NoUpdateVerifier stays the fail-closed default. Repository sequences \
     and role versions are STRICTLY ahead of the last-issued values (the device \
     anti-rollback semantics, honored on the producer side). Signing keys are INJECTED \
     seeds; the P18.03 key ceremonies are HUMAN GATES; no key material ships. The code \
     is WRITTEN-UNRUN: no bundle has ever been produced, signed, staged, or applied.";

// --- Metadata roles ----------------------------------------------------------------------

/// The closed repository-metadata role vocabulary (the TUF-shaped role
/// set; record 03 §1). The role code byte rides the canonical bytes (role
/// separation inside one domain).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OtaMetadataRole {
    /// The trust-anchor set (record 03 §1: the OFFLINE root; its key
    /// ceremony is a HUMAN GATE).
    Root,
    /// The target digests — in FlexCore: the signed v2 update manifest.
    Targets,
    /// Freshness (short expiry; binds the targets metadata by SHA-256).
    Timestamp,
    /// Consistency (binds the role versions, the targets digest, and the
    /// bundle digest).
    Snapshot,
}

impl OtaMetadataRole {
    /// The role code byte (stable; part of the canonical encoding).
    pub fn code(self) -> u8 {
        match self {
            OtaMetadataRole::Root => 1,
            OtaMetadataRole::Targets => 2,
            OtaMetadataRole::Timestamp => 3,
            OtaMetadataRole::Snapshot => 4,
        }
    }

    /// The stable role name (audit lines, evidence records).
    pub fn name(self) -> &'static str {
        match self {
            OtaMetadataRole::Root => "root",
            OtaMetadataRole::Targets => "targets",
            OtaMetadataRole::Timestamp => "timestamp",
            OtaMetadataRole::Snapshot => "snapshot",
        }
    }
}

/// The root role's structured metadata: the repository's trust-anchor set
/// (the deployment's release keys — minted by the P18.03 HUMAN GATE
/// ceremonies, distributed as device configuration), the root metadata
/// version (monotonic), and the root expiry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtaRootMetadata {
    pub root_version: u64,
    /// The trust anchors (sorted by signer reference in the canonical
    /// encoding; duplicate references are refused at production).
    pub anchors: Vec<OtaTrustAnchor>,
    pub expires_at_unix_millis: i64,
}

/// The timestamp role's structured metadata: freshness (its own expiry —
/// the TUF-shaped analog riding the v2 manifest's mandatory-expiry
/// residual, record 03 §4) plus the SHA-256 binding of the targets
/// metadata bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtaTimestampMetadata {
    pub timestamp_version: u64,
    /// The lowercase-hex SHA-256 of the targets role's canonical bytes.
    pub targets_digest_hex: String,
    pub expires_at_unix_millis: i64,
}

/// The snapshot role's structured metadata: consistency. Binds WHICH root
/// / targets / timestamp versions are current, the targets digest, and
/// the bundle digest (SHA-256 — the TUF-shaped targets-file digest analog;
/// the bundle's signature remains the authorization).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtaSnapshotMetadata {
    pub snapshot_version: u64,
    pub root_version: u64,
    /// The targets version IS the manifest's anti-rollback sequence (the
    /// recorded TUF-shaped reading: one monotonic counter per release).
    pub targets_version: u64,
    pub timestamp_version: u64,
    /// The lowercase-hex SHA-256 of the targets role's canonical bytes.
    pub targets_digest_hex: String,
    /// The lowercase-hex SHA-256 of the bundle bytes.
    pub bundle_digest_hex: String,
}

/// One metadata role as it travels in the artifact: the structured view,
/// the EXACT canonical bytes the signature covers, the signer reference,
/// and the signature. The verifier re-derives the canonical bytes from the
/// structured view and refuses any drift BEFORE verifying the signature
/// (the signature always covers provably re-derivable bytes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtaSignedRole<T> {
    pub role: OtaMetadataRole,
    pub structured: T,
    pub canonical_bytes: Vec<u8>,
    pub signer_reference: String,
    pub signature: [u8; OTA_ED25519_SIGNATURE_BYTES],
}

// --- Canonical encodings (the only byte constructions; one per role) ------

/// Appends `bytes` as u32 little-endian length + bytes (the Patch B /
/// `canonical_signing_bytes` discipline).
fn push_lp32(out: &mut Vec<u8>, bytes: &[u8]) {
    let len = u32::try_from(bytes.len())
        .expect("an OTA metadata field longer than u32::MAX bytes cannot be signed");
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(bytes);
}

/// The role prelude every metadata role's canonical bytes start with:
/// metadata domain || role code || format version byte. The domain is
/// INSIDE the signed bytes (the Patch B pattern).
fn role_prelude(role: OtaMetadataRole) -> Vec<u8> {
    let mut out = Vec::with_capacity(OTA_METADATA_DOMAIN_LEN + 2);
    out.extend_from_slice(crate::ota_signing::OTA_METADATA_SIGNATURE_DOMAIN);
    out.push(role.code());
    out.push(OTA_REPOSITORY_FORMAT_VERSION);
    out
}

const OTA_METADATA_DOMAIN_LEN: usize = crate::ota_signing::OTA_METADATA_SIGNATURE_DOMAIN.len();

/// The canonical bytes of the root role.
pub fn canonical_root_bytes(root: &OtaRootMetadata) -> Vec<u8> {
    let mut out = role_prelude(OtaMetadataRole::Root);
    out.extend_from_slice(&root.root_version.to_le_bytes());
    // Deterministic anchor order: sorted by signer reference (the
    // duplicate-reference refusal is the producer's; the encoding here is
    // total regardless — first anchor wins on an equal reference, and the
    // producer refuses that case before anything is signed).
    let mut references: Vec<&str> =
        root.anchors.iter().map(|a| a.signer_reference.as_str()).collect();
    references.sort_unstable();
    references.dedup();
    out.extend_from_slice(&(references.len() as u32).to_le_bytes());
    for reference in references {
        let anchor = root
            .anchors
            .iter()
            .find(|a| a.signer_reference == reference)
            .expect("the reference came from the anchor list");
        push_lp32(&mut out, anchor.signer_reference.as_bytes());
        out.extend_from_slice(&anchor.public_key);
    }
    out.extend_from_slice(&root.expires_at_unix_millis.to_le_bytes());
    out
}

/// The canonical bytes of the TARGETS role payload: the FULL v2 manifest,
/// every field, in fixed order (deterministic; payload entries sorted by
/// name — names are unique, offsets are authoritative, the declared order
/// carries no semantics).
pub fn canonical_manifest_bytes(manifest: &ExtendedUpdateManifest) -> Vec<u8> {
    let mut out = role_prelude(OtaMetadataRole::Targets);
    // The P9.34 v1 base fields (validated verbatim by the gate).
    push_lp32(&mut out, manifest.base.package_id.as_bytes());
    push_lp32(&mut out, manifest.base.version.as_bytes());
    push_lp32(&mut out, manifest.base.bundle_digest_hex.as_bytes());
    push_lp32(&mut out, manifest.base.digest_algorithm.as_bytes());
    push_lp32(&mut out, manifest.base.signer_key_reference.as_bytes());
    push_lp32(&mut out, manifest.base.minimum_version_floor.as_bytes());
    push_lp32(&mut out, manifest.base.approval_reference.as_bytes());
    // The v2 applicability metadata (device ids in BTreeSet order).
    push_lp32(&mut out, manifest.applicability.vendor.as_bytes());
    push_lp32(&mut out, manifest.applicability.device_class.as_bytes());
    out.extend_from_slice(&(manifest.applicability.device_ids.len() as u32).to_le_bytes());
    for device_id in &manifest.applicability.device_ids {
        push_lp32(&mut out, device_id.as_bytes());
    }
    // The anti-rollback sequence.
    out.extend_from_slice(&manifest.anti_rollback_sequence.to_le_bytes());
    // The per-payload integrity entries (sorted by name; names unique).
    let mut sorted: Vec<&PayloadDigest> = manifest.payloads.iter().collect();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));
    out.extend_from_slice(&(sorted.len() as u32).to_le_bytes());
    for payload in sorted {
        push_lp32(&mut out, payload.name.as_bytes());
        out.extend_from_slice(&(payload.offset as u64).to_le_bytes());
        out.extend_from_slice(&(payload.size_bytes as u64).to_le_bytes());
        push_lp32(&mut out, payload.digest_hex.as_bytes());
    }
    // The mandatory expiry.
    out.extend_from_slice(&manifest.expires_at_unix_millis.to_le_bytes());
    out
}

/// The canonical bytes of the timestamp role.
pub fn canonical_timestamp_bytes(timestamp: &OtaTimestampMetadata) -> Vec<u8> {
    let mut out = role_prelude(OtaMetadataRole::Timestamp);
    out.extend_from_slice(&timestamp.timestamp_version.to_le_bytes());
    push_lp32(&mut out, timestamp.targets_digest_hex.as_bytes());
    out.extend_from_slice(&timestamp.expires_at_unix_millis.to_le_bytes());
    out
}

/// The canonical bytes of the snapshot role.
pub fn canonical_snapshot_bytes(snapshot: &OtaSnapshotMetadata) -> Vec<u8> {
    let mut out = role_prelude(OtaMetadataRole::Snapshot);
    out.extend_from_slice(&snapshot.snapshot_version.to_le_bytes());
    out.extend_from_slice(&snapshot.root_version.to_le_bytes());
    out.extend_from_slice(&snapshot.targets_version.to_le_bytes());
    out.extend_from_slice(&snapshot.timestamp_version.to_le_bytes());
    push_lp32(&mut out, snapshot.targets_digest_hex.as_bytes());
    push_lp32(&mut out, snapshot.bundle_digest_hex.as_bytes());
    out
}

// --- Producer errors -----------------------------------------------------------------

/// Named errors of the PRODUCER side. Every failure is named
/// ([`OtaProduceError::name`]); nothing is a silent pass. The anti-rollback
/// refusal reuses the EXISTING stable device-side code
/// (`UPDATE_SEQUENCE_NOT_AHEAD`) — one vocabulary on both sides of the
/// seam.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OtaProduceError {
    /// An input was unusable (empty reference, duplicate payload name,
    /// anchorless root, non-positive expiry, …).
    InvalidInput { reason: String },
    /// The request declared a digest algorithm the producer does not
    /// produce: the producer SIGNS, so the only producible algorithm is
    /// `ed25519` (sha-256 manifests are other verifiers' business).
    AlgorithmUnsupported { declared: String },
    /// The manifest's signer reference does not name THIS producer's key
    /// (the reference value is not echoed — the P9.34 discipline).
    SignerMismatch,
    /// The producer's signing key is not among the supplied root role's
    /// anchors — the produced artifact could never verify against its own
    /// root (refused, never produced).
    SignerNotAnchored,
    /// The manifest sequence is not STRICTLY ahead of the repository's
    /// last-issued sequence (the EXISTING stable refusal code; the
    /// strictly-greater-than semantics, honored on the producer side).
    SequenceNotAhead { declared: u64, last_seen: u64 },
    /// A repository role version is not STRICTLY ahead of its counter.
    VersionNotAhead { role: &'static str, declared: u64, last_seen: u64 },
    /// The assembled manifest failed the v2 validation (the P9.34/v2
    /// problem text rides verbatim).
    ManifestInvalid { reason: String },
    /// The assembled bundle exceeds the total-size cap (the EXISTING code).
    BundleTooLarge { size_bytes: usize, cap: usize },
}

impl OtaProduceError {
    /// The stable machine-readable error name (the named-error table key).
    pub fn name(&self) -> &'static str {
        match self {
            OtaProduceError::InvalidInput { .. } => "OTA_PRODUCE_INPUT_INVALID",
            OtaProduceError::AlgorithmUnsupported { .. } => "OTA_PRODUCE_ALGORITHM_UNSUPPORTED",
            OtaProduceError::SignerMismatch => "OTA_PRODUCE_SIGNER_MISMATCH",
            OtaProduceError::SignerNotAnchored => "OTA_PRODUCE_SIGNER_NOT_ANCHORED",
            OtaProduceError::SequenceNotAhead { .. } => "UPDATE_SEQUENCE_NOT_AHEAD",
            OtaProduceError::VersionNotAhead { .. } => "OTA_REPOSITORY_VERSION_NOT_AHEAD",
            OtaProduceError::ManifestInvalid { .. } => "UPDATE_MANIFEST_INVALID",
            OtaProduceError::BundleTooLarge { .. } => "UPDATE_BUNDLE_TOO_LARGE",
        }
    }
}

impl std::fmt::Display for OtaProduceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OtaProduceError::InvalidInput { reason } => {
                write!(f, "the OTA produce input is invalid: {reason}")
            }
            OtaProduceError::AlgorithmUnsupported { declared } => write!(
                f,
                "the producer signs bundles, so the only producible digest algorithm is \
                 '{OTA_ED25519_ALGORITHM}' — the request declared '{declared}' and is \
                 refused, never produced with a weaker scheme"
            ),
            OtaProduceError::SignerMismatch => write!(
                f,
                "the manifest's signer reference does not name this producer's release key \
                 — refused (the reference value is not echoed)"
            ),
            OtaProduceError::SignerNotAnchored => write!(
                f,
                "the producer's signing key is not among the supplied root role's trust \
                 anchors — the artifact could never verify against its own root; refused, \
                 never produced"
            ),
            OtaProduceError::SequenceNotAhead { declared, last_seen } => write!(
                f,
                "the manifest sequence {declared} is not strictly ahead of the last-issued \
                 sequence {last_seen} — refused (the device anti-rollback semantics, \
                 honored on the producer side; {ANTI_ROLLBACK_SEMANTICS_STATEMENT})"
            ),
            OtaProduceError::VersionNotAhead { role, declared, last_seen } => write!(
                f,
                "the {role} role version {declared} is not strictly ahead of the last-issued \
                 version {last_seen} — refused (repository counters are monotonic by the \
                 same strictly-ahead reading)"
            ),
            OtaProduceError::ManifestInvalid { reason } => {
                write!(f, "the assembled manifest is invalid: {reason}")
            }
            OtaProduceError::BundleTooLarge { size_bytes, cap } => write!(
                f,
                "the assembled bundle is {size_bytes} bytes; the total-size cap is {cap} \
                 bytes — refused (a code maximum; deployments may tighten, never loosen)"
            ),
        }
    }
}

impl std::error::Error for OtaProduceError {}

// --- The producer --------------------------------------------------------------------------

/// The repository-side counters (the producer-side mirror of the device's
/// monotonic anti-rollback counter: every issued sequence / role version
/// is STRICTLY ahead of the last-issued value; a fresh repository starts
/// at 0, the same "nothing issued yet" reading as
/// `crate::update_bundle::AntiRollbackCounter::last_seen`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OtaRepositoryCounters {
    pub last_manifest_sequence: u64,
    pub last_root_version: u64,
    pub last_timestamp_version: u64,
    pub last_snapshot_version: u64,
}

/// One payload to assemble into the bundle: a name and the image bytes.
/// The offset, size, and signature are computed BY the producer (the
/// caller never declares a byte layout the producer did not assemble).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtaProducePayload {
    pub name: String,
    pub bytes: Vec<u8>,
}

/// The production request: the manifest's identity/applicability fields,
/// the payloads, the expiries, and the root role to publish under. The
/// bundle digest and per-payload hex entries are computed by the producer
/// (they are the Ed25519 signatures over the assembled bytes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtaProduceRequest {
    /// The P9.34 v1 identity fields.
    pub package_id: String,
    pub version: String,
    /// The digest algorithm the caller asks the manifest to declare —
    /// MUST be `ed25519` (the producer signs; anything else is a named
    /// refusal).
    pub digest_algorithm: String,
    /// The signer reference the manifest declares — MUST name THIS
    /// producer's key.
    pub signer_key_reference: String,
    pub minimum_version_floor: String,
    pub approval_reference: String,
    /// The v2 applicability metadata (SUIT-shaped; consumed verbatim).
    pub applicability: UpdateApplicability,
    /// The anti-rollback sequence (STRICTLY ahead of the last-issued).
    pub anti_rollback_sequence: u64,
    /// The payloads to assemble (offsets computed by the producer).
    pub payloads: Vec<OtaProducePayload>,
    /// The mandatory manifest expiry.
    pub expires_at_unix_millis: i64,
    /// The root role to publish under (the deployment's anchor set).
    pub root: OtaRootMetadata,
    /// The repository role versions (each STRICTLY ahead of its counter).
    pub root_version: u64,
    pub timestamp_version: u64,
    pub timestamp_expiry_unix_millis: i64,
    pub snapshot_version: u64,
}

/// The produced distribution artifact: the four signed metadata roles plus
/// the assembled bundle bytes. Plain data — the transport/publishing is a
/// deployment concern; nothing here sends anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtaDistributionArtifact {
    pub root: OtaSignedRole<OtaRootMetadata>,
    pub targets: OtaSignedRole<ExtendedUpdateManifest>,
    pub timestamp: OtaSignedRole<OtaTimestampMetadata>,
    pub snapshot: OtaSignedRole<OtaSnapshotMetadata>,
    /// The assembled bundle: the payloads at their declared offsets (what
    /// the existing extended gate verifies).
    pub bundle: Vec<u8>,
}

/// The FIM-F12.01 bundle producer: assembles image bytes + metadata into a
/// bundle whose manifest satisfies `UpdateManifest::validate` /
/// `ExtendedUpdateManifest::validate`, signs everything with the INJECTED
/// Ed25519 release key, and records the production in the audit trail.
/// Holds the repository counters (anti-rollback honored on the producer
/// side). Produces IN MEMORY; publishes nothing.
pub struct OtaBundleProducer {
    key: OtaEd25519ReleaseKey,
    counters: OtaRepositoryCounters,
    audit: Arc<dyn AuditSink>,
    clock: Arc<dyn Clock + Send + Sync>,
}

impl OtaBundleProducer {
    /// Composes the producer over the INJECTED release key (never
    /// generated here), the audit sink, and the injected clock.
    pub fn new(
        key: OtaEd25519ReleaseKey,
        audit: Arc<dyn AuditSink>,
        clock: Arc<dyn Clock + Send + Sync>,
    ) -> Self {
        Self {
            key,
            counters: OtaRepositoryCounters::default(),
            audit,
            clock,
        }
    }

    /// The repository counters (diagnostics).
    pub fn counters(&self) -> &OtaRepositoryCounters {
        &self.counters
    }

    /// Produces ONE distribution artifact (the recorded order):
    ///
    /// 1. request sanity (algorithm, signer reference, payload names,
    ///    sizes, expiries, anchored root);
    /// 2. the anti-rollback sequence and every role version STRICTLY ahead
    ///    of their counters (the device semantics, producer side);
    /// 3. the producer key anchored in the supplied root role;
    /// 4. bundle assembly (contiguous declared-order offsets);
    /// 5. signatures (bundle level + per-payload, domain-separated);
    /// 6. the manifest built and validated (the v2 rules, verbatim);
    /// 7. the root/timestamp/snapshot roles built, canonically encoded,
    ///    and signed (the consistency bindings computed over the real
    ///    bytes);
    /// 8. counters advance and the production is audited
    ///    (`EDGE_OTA_BUNDLE_PRODUCED`; refusals are audited
    ///    `EDGE_OTA_PRODUCE_REFUSED` with the stable code).
    pub fn produce(
        &mut self,
        request: OtaProduceRequest,
    ) -> Result<OtaDistributionArtifact, OtaProduceError> {
        let verdict = self.produce_inner(request);
        match &verdict {
            Ok(artifact) => {
                self.audit_event(
                    "EDGE_OTA_BUNDLE_PRODUCED",
                    format!(
                        "package='{}' version={} sequence={} payloads={} bundle_bytes={} \
                         format='{}' — assembled and signed IN MEMORY (written-unrun code; \
                         the P18.03 key ceremonies are HUMAN GATES)",
                        artifact.targets.structured.base.package_id,
                        artifact.targets.structured.base.version,
                        artifact.targets.structured.anti_rollback_sequence,
                        artifact.targets.structured.payloads.len(),
                        artifact.bundle.len(),
                        OTA_REPOSITORY_FORMAT,
                    ),
                );
            }
            Err(error) => {
                self.audit_event(
                    "EDGE_OTA_PRODUCE_REFUSED",
                    format!("refusal code={} :: {error}", error.name()),
                );
            }
        }
        verdict
    }

    fn produce_inner(
        &mut self,
        request: OtaProduceRequest,
    ) -> Result<OtaDistributionArtifact, OtaProduceError> {
        // 1. Request sanity.
        if request.package_id.trim().is_empty() || request.version.trim().is_empty() {
            return Err(OtaProduceError::InvalidInput {
                reason: "package_id and version are mandatory".to_string(),
            });
        }
        if request.signer_key_reference.trim().is_empty() {
            return Err(OtaProduceError::InvalidInput {
                reason: "signer_key_reference is mandatory".to_string(),
            });
        }
        // The producer SIGNS: the only producible algorithm is ed25519
        // (refused before any assembly — never produced with a weaker
        // scheme, never with the test-grade checksum).
        if request.digest_algorithm != OTA_ED25519_ALGORITHM {
            return Err(OtaProduceError::AlgorithmUnsupported {
                declared: request.digest_algorithm,
            });
        }
        if request
            .payloads
            .iter()
            .any(|payload| payload.name.trim().is_empty() || payload.bytes.is_empty())
        {
            return Err(OtaProduceError::InvalidInput {
                reason: "every payload needs a non-empty name and at least one byte"
                    .to_string(),
            });
        }
        if request.payloads.is_empty() || request.payloads.len() > MAX_UPDATE_PAYLOAD_ENTRIES {
            return Err(OtaProduceError::InvalidInput {
                reason: format!("payloads must number 1..={MAX_UPDATE_PAYLOAD_ENTRIES}"),
            });
        }
        let mut names: BTreeSet<&str> = BTreeSet::new();
        for payload in &request.payloads {
            if payload.name.len() > MAX_UPDATE_LABEL_BYTES {
                return Err(OtaProduceError::InvalidInput {
                    reason: format!("payload name '{}' exceeds the label cap", payload.name),
                });
            }
            if !names.insert(payload.name.as_str()) {
                return Err(OtaProduceError::InvalidInput {
                    reason: format!("payload name '{}' appears twice", payload.name),
                });
            }
            if payload.bytes.len() > MAX_UPDATE_PAYLOAD_ENTRY_BYTES {
                return Err(OtaProduceError::InvalidInput {
                    reason: format!("payload '{}' exceeds the per-entry cap", payload.name),
                });
            }
        }
        if request.expires_at_unix_millis <= 0 || request.timestamp_expiry_unix_millis <= 0 {
            return Err(OtaProduceError::InvalidInput {
                reason: "the manifest and timestamp expiries are mandatory positive \
                         unix-millis instants"
                    .to_string(),
            });
        }
        if request.root.expires_at_unix_millis <= 0 {
            return Err(OtaProduceError::InvalidInput {
                reason: "the root role's expiry is a mandatory positive unix-millis instant"
                    .to_string(),
            });
        }
        if request.root.anchors.is_empty() {
            return Err(OtaProduceError::InvalidInput {
                reason: "the root role carries no trust anchors — an anchorless repository \
                         verifies nothing and is refused"
                    .to_string(),
            });
        }
        let mut anchor_references: BTreeSet<&str> = BTreeSet::new();
        for anchor in &request.root.anchors {
            if !anchor_references.insert(anchor.signer_reference.as_str()) {
                return Err(OtaProduceError::InvalidInput {
                    reason: "the root role lists a trust-anchor reference twice".to_string(),
                });
            }
        }

        // 2. Anti-rollback: the sequence and every role version strictly
        //    ahead of their counters.
        if request.anti_rollback_sequence <= self.counters.last_manifest_sequence {
            return Err(OtaProduceError::SequenceNotAhead {
                declared: request.anti_rollback_sequence,
                last_seen: self.counters.last_manifest_sequence,
            });
        }
        if request.root_version <= self.counters.last_root_version {
            return Err(OtaProduceError::VersionNotAhead {
                role: OtaMetadataRole::Root.name(),
                declared: request.root_version,
                last_seen: self.counters.last_root_version,
            });
        }
        if request.timestamp_version <= self.counters.last_timestamp_version {
            return Err(OtaProduceError::VersionNotAhead {
                role: OtaMetadataRole::Timestamp.name(),
                declared: request.timestamp_version,
                last_seen: self.counters.last_timestamp_version,
            });
        }
        if request.snapshot_version <= self.counters.last_snapshot_version {
            return Err(OtaProduceError::VersionNotAhead {
                role: OtaMetadataRole::Snapshot.name(),
                declared: request.snapshot_version,
                last_seen: self.counters.last_snapshot_version,
            });
        }

        // 3. The producer key must be anchored in the supplied root (the
        //    artifact could never verify against its own root otherwise).
        if request.signer_key_reference != self.key.signer_reference() {
            return Err(OtaProduceError::SignerMismatch);
        }
        let anchored = request.root.anchors.iter().any(|anchor| {
            anchor.signer_reference == self.key.signer_reference()
                && anchor.public_key == self.key.public_key_bytes()
        });
        if !anchored {
            return Err(OtaProduceError::SignerNotAnchored);
        }

        // 4. Bundle assembly: contiguous offsets in DECLARED order.
        let mut bundle: Vec<u8> = Vec::new();
        let mut payload_digests: Vec<PayloadDigest> =
            Vec::with_capacity(request.payloads.len());
        for payload in &request.payloads {
            let offset = bundle.len();
            bundle.extend_from_slice(&payload.bytes);
            payload_digests.push(PayloadDigest {
                name: payload.name.clone(),
                offset,
                size_bytes: payload.bytes.len(),
                // Placeholder — replaced by the payload's signature below.
                digest_hex: String::new(),
            });
        }
        if bundle.len() > MAX_UPDATE_PAYLOAD_TOTAL_BYTES {
            return Err(OtaProduceError::BundleTooLarge {
                size_bytes: bundle.len(),
                cap: MAX_UPDATE_PAYLOAD_TOTAL_BYTES,
            });
        }

        // 5. Signatures: the bundle level and each payload level, over the
        //    domain-separated exact bytes (one construction —
        //    `bundle_signing_bytes`; no drift).
        let bundle_signature = self.key.sign_exact(&bundle_signing_bytes(&bundle));
        for (index, payload) in request.payloads.iter().enumerate() {
            let start = payload_digests[index].offset;
            let end = start + payload.bytes.len();
            let slice_signature =
                self.key.sign_exact(&bundle_signing_bytes(&bundle[start..end]));
            payload_digests[index].digest_hex = signature_to_hex(&slice_signature);
        }

        // 6. The manifest, built and validated (the v2 rules verbatim —
        //    the base P9.34 validation runs inside).
        let manifest = ExtendedUpdateManifest {
            base: UpdateManifest {
                package_id: request.package_id,
                version: request.version,
                bundle_digest_hex: signature_to_hex(&bundle_signature),
                digest_algorithm: OTA_ED25519_ALGORITHM.to_string(),
                signer_key_reference: request.signer_key_reference,
                minimum_version_floor: request.minimum_version_floor,
                approval_reference: request.approval_reference,
            },
            applicability: request.applicability,
            anti_rollback_sequence: request.anti_rollback_sequence,
            payloads: payload_digests,
            expires_at_unix_millis: request.expires_at_unix_millis,
        };
        manifest.validate().map_err(|error| match error {
            UpdateBundleError::ManifestInvalid { reason } => {
                OtaProduceError::ManifestInvalid { reason }
            }
            other => OtaProduceError::ManifestInvalid {
                reason: other.to_string(),
            },
        })?;

        // 7. The metadata roles: canonically encoded, consistency-bound,
        //    and signed. The canonical bytes are computed from the
        //    structured values FIRST, then signed, then moved into the
        //    roles (one encoding per role, computed once).
        let root_metadata = OtaRootMetadata {
            root_version: request.root_version,
            anchors: request.root.anchors,
            expires_at_unix_millis: request.root.expires_at_unix_millis,
        };
        let targets_canonical = canonical_manifest_bytes(&manifest);
        let targets_digest_hex = hex_encode_digest(&sha256(&targets_canonical));
        let bundle_digest_hex = hex_encode_digest(&sha256(&bundle));
        let timestamp_metadata = OtaTimestampMetadata {
            timestamp_version: request.timestamp_version,
            targets_digest_hex: targets_digest_hex.clone(),
            expires_at_unix_millis: request.timestamp_expiry_unix_millis,
        };
        let snapshot_metadata = OtaSnapshotMetadata {
            snapshot_version: request.snapshot_version,
            root_version: request.root_version,
            targets_version: request.anti_rollback_sequence,
            timestamp_version: request.timestamp_version,
            targets_digest_hex,
            bundle_digest_hex,
        };
        let root_canonical = canonical_root_bytes(&root_metadata);
        let timestamp_canonical = canonical_timestamp_bytes(&timestamp_metadata);
        let snapshot_canonical = canonical_snapshot_bytes(&snapshot_metadata);
        let root_signature = self.key.sign_exact(&root_canonical).to_bytes();
        let timestamp_signature = self.key.sign_exact(&timestamp_canonical).to_bytes();
        let snapshot_signature = self.key.sign_exact(&snapshot_canonical).to_bytes();
        let targets_signature = self.key.sign_exact(&targets_canonical).to_bytes();
        let signer_reference = self.key.signer_reference().to_string();

        // 8. Counters advance ONLY on success; the production is audited
        //    by the caller-visible wrapper.
        self.counters = OtaRepositoryCounters {
            last_manifest_sequence: request.anti_rollback_sequence,
            last_root_version: request.root_version,
            last_timestamp_version: request.timestamp_version,
            last_snapshot_version: request.snapshot_version,
        };
        Ok(OtaDistributionArtifact {
            root: OtaSignedRole {
                role: OtaMetadataRole::Root,
                structured: root_metadata,
                canonical_bytes: root_canonical,
                signer_reference: signer_reference.clone(),
                signature: root_signature,
            },
            targets: OtaSignedRole {
                role: OtaMetadataRole::Targets,
                structured: manifest,
                canonical_bytes: targets_canonical,
                signer_reference: signer_reference.clone(),
                signature: targets_signature,
            },
            timestamp: OtaSignedRole {
                role: OtaMetadataRole::Timestamp,
                structured: timestamp_metadata,
                canonical_bytes: timestamp_canonical,
                signer_reference: signer_reference.clone(),
                signature: timestamp_signature,
            },
            snapshot: OtaSignedRole {
                role: OtaMetadataRole::Snapshot,
                structured: snapshot_metadata,
                canonical_bytes: snapshot_canonical,
                signer_reference,
                signature: snapshot_signature,
            },
            bundle,
        })
    }

    fn audit_event(&self, name: &'static str, detail: String) {
        let event = EdgeAuditEvent {
            recorded_at: self.clock.now_ingest_time(),
            kind: EdgeAuditKind::UpdateLifecycle,
            detail: format!("{name}: {detail}"),
        };
        if let Err(error) = self.audit.record(event) {
            tracing::error!(%error, audit_event = name, "ota-producer audit write FAILED");
        }
    }
}

// --- Device-side artifact verification (FIM-F12.02) -------------------------------

/// Named errors of the device-side artifact verification. Every failure is
/// named ([`OtaArtifactError::name`]); nothing is a silent pass. The
/// existing `UpdateVerificationError` taxonomy is PRESERVED verbatim: the
/// extended gate's refusal rides `GateRefused` with its stable code (the
/// `BaseVerificationRefused` pattern) — no gate refusal is ever re-coded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OtaArtifactError {
    /// An input was unusable.
    InvalidInput { reason: String },
    /// A metadata role's canonical bytes do not re-derive from its
    /// structured view — the carried bytes and the carried structure
    /// disagree (refused before any signature work).
    CanonicalMismatch { role: &'static str },
    /// A metadata role's signature does not verify under the
    /// DEVICE-CONFIGURED anchors (the reason carries the named
    /// `OtaSigningError`).
    SignatureRefused { role: &'static str, reason: String },
    /// A metadata role's expiry has passed (the injected clock).
    RoleExpired {
        role: &'static str,
        expires_at_unix_millis: i64,
        now_unix_millis: i64,
    },
    /// The artifact's root role lists a trust anchor the DEVICE does not
    /// hold (same reference AND same key bytes required) — the artifact
    /// can never add an anchor the device does not already trust (no
    /// blind trust swap).
    RootAnchorNotConfigured,
    /// The timestamp role's SHA-256 binding of the targets metadata does
    /// not match the actual targets bytes.
    TimestampTargetsDigestMismatch,
    /// The snapshot's targets version does not bind the manifest's
    /// anti-rollback sequence.
    SnapshotTargetsVersionMismatch { snapshot_version: u64, manifest_sequence: u64 },
    /// The snapshot's SHA-256 binding of the targets metadata does not
    /// match.
    SnapshotTargetsDigestMismatch,
    /// The snapshot's SHA-256 binding of the bundle does not match the
    /// actual bundle bytes.
    SnapshotBundleDigestMismatch,
    /// The snapshot's root/timestamp version binding does not match the
    /// artifact's own roles.
    SnapshotRoleVersionMismatch { role: &'static str, snapshot_version: u64, role_version: u64 },
    /// THE EXISTING GATE REFUSED — the stable P9.34/v2 refusal code rides
    /// verbatim (`UPDATE_*`; the refusal order, audit, and no-write
    /// structure are the untouched extended gate's).
    GateRefused { code: String, reason: String },
}

impl OtaArtifactError {
    /// The stable machine-readable error name (the named-error table key).
    pub fn name(&self) -> &'static str {
        match self {
            OtaArtifactError::InvalidInput { .. } => "OTA_ARTIFACT_INPUT_INVALID",
            OtaArtifactError::CanonicalMismatch { .. } => "OTA_ARTIFACT_CANONICAL_MISMATCH",
            OtaArtifactError::SignatureRefused { .. } => "OTA_ARTIFACT_SIGNATURE_REFUSED",
            OtaArtifactError::RoleExpired { .. } => "OTA_ARTIFACT_METADATA_EXPIRED",
            OtaArtifactError::RootAnchorNotConfigured => "OTA_ARTIFACT_ROOT_ANCHOR_NOT_CONFIGURED",
            OtaArtifactError::TimestampTargetsDigestMismatch => {
                "OTA_ARTIFACT_TIMESTAMP_TARGETS_DIGEST_MISMATCH"
            }
            OtaArtifactError::SnapshotTargetsVersionMismatch { .. } => {
                "OTA_ARTIFACT_SNAPSHOT_TARGETS_VERSION_MISMATCH"
            }
            OtaArtifactError::SnapshotTargetsDigestMismatch => {
                "OTA_ARTIFACT_SNAPSHOT_TARGETS_DIGEST_MISMATCH"
            }
            OtaArtifactError::SnapshotBundleDigestMismatch => {
                "OTA_ARTIFACT_SNAPSHOT_BUNDLE_DIGEST_MISMATCH"
            }
            OtaArtifactError::SnapshotRoleVersionMismatch { .. } => {
                "OTA_ARTIFACT_SNAPSHOT_ROLE_VERSION_MISMATCH"
            }
            OtaArtifactError::GateRefused { .. } => "OTA_ARTIFACT_GATE_REFUSED",
        }
    }
}

impl std::fmt::Display for OtaArtifactError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OtaArtifactError::InvalidInput { reason } => {
                write!(f, "the OTA artifact input is invalid: {reason}")
            }
            OtaArtifactError::CanonicalMismatch { role } => write!(
                f,
                "the {role} role's canonical bytes do not re-derive from its structured \
                 view — the carried bytes and the carried structure disagree; refused \
                 BEFORE any signature work"
            ),
            OtaArtifactError::SignatureRefused { role, reason } => write!(
                f,
                "the {role} role's signature does not verify under the device-configured \
                 trust anchors: {reason}"
            ),
            OtaArtifactError::RoleExpired {
                role,
                expires_at_unix_millis,
                now_unix_millis,
            } => write!(
                f,
                "the {role} role expired at {expires_at_unix_millis} (now {now_unix_millis}) \
                 — a stale repository metadata set is refused (freshness)"
            ),
            OtaArtifactError::RootAnchorNotConfigured => write!(
                f,
                "the artifact's root role lists a trust anchor this device does not hold — \
                 refused (the artifact can never add an anchor the device does not \
                 already trust; no blind trust swap)"
            ),
            OtaArtifactError::TimestampTargetsDigestMismatch => write!(
                f,
                "the timestamp role's SHA-256 binding of the targets metadata does not \
                 match the actual targets bytes — refused"
            ),
            OtaArtifactError::SnapshotTargetsVersionMismatch {
                snapshot_version,
                manifest_sequence,
            } => write!(
                f,
                "the snapshot binds targets version {snapshot_version} but the manifest's \
                 anti-rollback sequence is {manifest_sequence} — the consistency binding \
                 disagrees; refused"
            ),
            OtaArtifactError::SnapshotTargetsDigestMismatch => write!(
                f,
                "the snapshot's SHA-256 binding of the targets metadata does not match the \
                 actual targets bytes — refused"
            ),
            OtaArtifactError::SnapshotBundleDigestMismatch => write!(
                f,
                "the snapshot's SHA-256 binding of the bundle does not match the actual \
                 bundle bytes — refused (the consistency check fires BEFORE the gate)"
            ),
            OtaArtifactError::SnapshotRoleVersionMismatch {
                role,
                snapshot_version,
                role_version,
            } => write!(
                f,
                "the snapshot binds {role} version {snapshot_version} but the artifact's \
                 {role} role carries version {role_version} — refused"
            ),
            OtaArtifactError::GateRefused { code, reason } => write!(
                f,
                "the existing extended gate refused ({code}): {reason} — the preserved \
                 P9.34/v2 refusal order, verbatim"
            ),
        }
    }
}

impl std::error::Error for OtaArtifactError {}

/// The device-side artifact verifier (FIM-F12.02): composes the
/// DEVICE-CONFIGURED trust anchors (deployment configuration — the root
/// role's anchor set as the device received it), the audit sink, and the
/// injected clock; `verify` drives one artifact through the metadata
/// checks and then the UNTOUCHED extended gate.
///
/// THE RECORDED VERIFICATION ORDER (all refusals audited
/// `EDGE_OTA_METADATA_REFUSED` / `EDGE_OTA_ARTIFACT_REFUSED` with the
/// stable code; every passed role audited `EDGE_OTA_METADATA_VERIFIED`):
///
/// 1. root: canonical re-derivation → expiry → signature (device
///    anchors) → the no-blind-trust-swap anchor check;
/// 2. targets: canonical re-derivation → signature (device anchors);
/// 3. timestamp: canonical re-derivation → signature → SHA-256 binding of
///    the targets bytes → expiry;
/// 4. snapshot: canonical re-derivation → signature → the
///    targets/root/timestamp VERSION bindings;
/// 5. **the extended gate, verbatim** (`verify_fleet_bundle`) — the P9.34
///    six-step refusal order, the v2 checks, the payload signatures, the
///    durable anti-rollback-counter advance, and the gate's own audit
///    lines are the EXISTING code;
/// 6. the snapshot's SHA-256 consistency bindings (targets metadata
///    digest, bundle digest) — deliberately AFTER the gate: a tampered or
///    spliced BUNDLE is refused by the gate's signature check THROUGH THE
///    EXISTING TAXONOMY (the pinned FIM-F12.02 semantics — the gate
///    remains the verification authority), while a stale/spliced
///    snapshot (targets or bundle swapped without re-issuing the
///    snapshot) is refused by the consistency layer. Both named; neither
///    bypasses the other.
pub struct OtaArtifactVerifier {
    device_anchors: Vec<OtaTrustAnchor>,
    audit: Arc<dyn AuditSink>,
    clock: Arc<dyn Clock + Send + Sync>,
}

impl OtaArtifactVerifier {
    /// Composes the verifier over the DEVICE-CONFIGURED anchors (never
    /// anchors learned from an artifact), the audit sink, and the injected
    /// clock.
    pub fn new(
        device_anchors: Vec<OtaTrustAnchor>,
        audit: Arc<dyn AuditSink>,
        clock: Arc<dyn Clock + Send + Sync>,
    ) -> Result<Self, crate::ota_signing::OtaSigningError> {
        if device_anchors.is_empty() {
            return Err(crate::ota_signing::OtaSigningError::InvalidInput {
                reason: "a device with no configured trust anchors refuses every artifact \
                         (fail-closed — compose the anchors the deployment's root-key \
                         ceremony produced)"
                    .to_string(),
            });
        }
        for anchor in &device_anchors {
            crate::ota_signing::OtaTrustAnchor::new(
                &anchor.signer_reference,
                anchor.public_key,
            )?;
        }
        Ok(Self {
            device_anchors,
            audit,
            clock,
        })
    }

    /// Verifies ONE distribution artifact and returns the extended gate's
    /// verdict on its bundle (the ONLY thing that may proceed to the
    /// install path). See the type doc for the recorded order.
    pub fn verify(
        &self,
        gate: &ExtendedUpdateGate,
        artifact: &OtaDistributionArtifact,
        now_unix_millis: i64,
    ) -> Result<VerifiedExtendedUpdate, OtaArtifactError> {
        let verdict = self.verify_inner(gate, artifact, now_unix_millis);
        match &verdict {
            Ok(_) => {
                self.audit_event(
                    "EDGE_OTA_ARTIFACT_VERIFIED",
                    format!(
                        "package='{}' version={} sequence={} — all four metadata roles \
                         verified against the device-configured anchors and the extended \
                         gate PASSED (verification BEFORE any write)",
                        artifact.targets.structured.base.package_id,
                        artifact.targets.structured.base.version,
                        artifact.targets.structured.anti_rollback_sequence,
                    ),
                );
            }
            Err(error) => {
                self.audit_event(
                    "EDGE_OTA_ARTIFACT_REFUSED",
                    format!("refusal code={} :: {error}", error.name()),
                );
            }
        }
        verdict
    }

    fn verify_inner(
        &self,
        gate: &ExtendedUpdateGate,
        artifact: &OtaDistributionArtifact,
        now_unix_millis: i64,
    ) -> Result<VerifiedExtendedUpdate, OtaArtifactError> {
        // 1. ROOT: canonical → expiry → signature → no-blind-trust-swap.
        self.verify_role(
            artifact.root.role,
            &artifact.root.canonical_bytes,
            &canonical_root_bytes(&artifact.root.structured),
            &artifact.root.signer_reference,
            &artifact.root.signature,
        )?;
        if now_unix_millis >= artifact.root.structured.expires_at_unix_millis {
            return Err(OtaArtifactError::RoleExpired {
                role: OtaMetadataRole::Root.name(),
                expires_at_unix_millis: artifact.root.structured.expires_at_unix_millis,
                now_unix_millis,
            });
        }
        for anchor in &artifact.root.structured.anchors {
            let configured = self.device_anchors.iter().any(|device_anchor| {
                device_anchor.signer_reference == anchor.signer_reference
                    && device_anchor.public_key == anchor.public_key
            });
            if !configured {
                return Err(OtaArtifactError::RootAnchorNotConfigured);
            }
        }
        // 2. TARGETS: canonical → signature.
        self.verify_role(
            artifact.targets.role,
            &artifact.targets.canonical_bytes,
            &canonical_manifest_bytes(&artifact.targets.structured),
            &artifact.targets.signer_reference,
            &artifact.targets.signature,
        )?;
        // 3. TIMESTAMP: canonical → signature → targets binding → expiry.
        self.verify_role(
            artifact.timestamp.role,
            &artifact.timestamp.canonical_bytes,
            &canonical_timestamp_bytes(&artifact.timestamp.structured),
            &artifact.timestamp.signer_reference,
            &artifact.timestamp.signature,
        )?;
        let timestamp_targets_digest =
            hex_encode_digest(&sha256(&artifact.targets.canonical_bytes));
        if artifact.timestamp.structured.targets_digest_hex != timestamp_targets_digest {
            return Err(OtaArtifactError::TimestampTargetsDigestMismatch);
        }
        if now_unix_millis >= artifact.timestamp.structured.expires_at_unix_millis {
            return Err(OtaArtifactError::RoleExpired {
                role: OtaMetadataRole::Timestamp.name(),
                expires_at_unix_millis: artifact.timestamp.structured.expires_at_unix_millis,
                now_unix_millis,
            });
        }
        // 4. SNAPSHOT: canonical → signature → the version bindings (the
        //    SHA-256 consistency digests deliberately run AFTER the gate —
        //    see the type doc).
        self.verify_role(
            artifact.snapshot.role,
            &artifact.snapshot.canonical_bytes,
            &canonical_snapshot_bytes(&artifact.snapshot.structured),
            &artifact.snapshot.signer_reference,
            &artifact.snapshot.signature,
        )?;
        let snapshot = &artifact.snapshot.structured;
        if snapshot.targets_version != artifact.targets.structured.anti_rollback_sequence {
            return Err(OtaArtifactError::SnapshotTargetsVersionMismatch {
                snapshot_version: snapshot.targets_version,
                manifest_sequence: artifact.targets.structured.anti_rollback_sequence,
            });
        }
        if snapshot.root_version != artifact.root.structured.root_version {
            return Err(OtaArtifactError::SnapshotRoleVersionMismatch {
                role: OtaMetadataRole::Root.name(),
                snapshot_version: snapshot.root_version,
                role_version: artifact.root.structured.root_version,
            });
        }
        if snapshot.timestamp_version != artifact.timestamp.structured.timestamp_version {
            return Err(OtaArtifactError::SnapshotRoleVersionMismatch {
                role: OtaMetadataRole::Timestamp.name(),
                snapshot_version: snapshot.timestamp_version,
                role_version: artifact.timestamp.structured.timestamp_version,
            });
        }
        // 5. THE EXTENDED GATE, VERBATIM (the existing P9.34 + v2 path; a
        //    refusal rides through with its stable code — never re-coded).
        //    A tampered or spliced BUNDLE is refused HERE, through the
        //    existing taxonomy.
        let verified = gate
            .verify_fleet_bundle(Some(&artifact.targets.structured), &artifact.bundle)
            .map_err(|error| OtaArtifactError::GateRefused {
                code: error.name().to_string(),
                reason: error.to_string(),
            })?;
        // 6. The snapshot's SHA-256 consistency bindings (AFTER the gate —
        //    the stale/spliced-snapshot refusal; the gate's verdict stands
        //    and the consistency layer refuses the artifact as a whole).
        if snapshot.targets_digest_hex != timestamp_targets_digest {
            return Err(OtaArtifactError::SnapshotTargetsDigestMismatch);
        }
        let bundle_digest = hex_encode_digest(&sha256(&artifact.bundle));
        if snapshot.bundle_digest_hex != bundle_digest {
            return Err(OtaArtifactError::SnapshotBundleDigestMismatch);
        }
        Ok(verified)
    }

    /// One role's shared checks: canonical re-derivation (byte-for-byte)
    /// and the signature under the DEVICE-CONFIGURED anchors. Audited per
    /// role (FIM-F12.05: every signature verification success/failure
    /// lands in the audit trail).
    fn verify_role(
        &self,
        role: OtaMetadataRole,
        carried_canonical: &[u8],
        rederived_canonical: &[u8],
        signer_reference: &str,
        signature: &[u8; OTA_ED25519_SIGNATURE_BYTES],
    ) -> Result<(), OtaArtifactError> {
        if carried_canonical != rederived_canonical {
            let error = OtaArtifactError::CanonicalMismatch { role: role.name() };
            self.audit_refused(&error);
            return Err(error);
        }
        let signature = Signature::from_bytes(signature);
        if let Err(signing_error) = verify_signature_under_anchor(
            carried_canonical,
            &signature,
            signer_reference,
            &self.device_anchors,
        ) {
            let error = OtaArtifactError::SignatureRefused {
                role: role.name(),
                reason: signing_error.to_string(),
            };
            self.audit_refused(&error);
            return Err(error);
        }
        self.audit_event(
            "EDGE_OTA_METADATA_VERIFIED",
            format!(
                "role={} verified under the device-configured anchors (signature over the \
                 domain-separated canonical bytes)",
                role.name(),
            ),
        );
        Ok(())
    }

    fn audit_refused(&self, error: &OtaArtifactError) {
        self.audit_event(
            "EDGE_OTA_METADATA_REFUSED",
            format!("refusal code={} :: {error}", error.name()),
        );
    }

    fn audit_event(&self, name: &'static str, detail: String) {
        let event = EdgeAuditEvent {
            recorded_at: self.clock.now_ingest_time(),
            kind: EdgeAuditKind::UpdateLifecycle,
            detail: format!("{name}: {detail}"),
        };
        if let Err(error) = self.audit.record(event) {
            tracing::error!(%error, audit_event = name, "ota-artifact-verifier audit write FAILED");
        }
    }
}

// --- Tests (WRITTEN-UNRUN; execution is a human gate) -----------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::update_bundle::UPDATE_BUNDLE_FORMAT_V2;

    /// The FIXED TEST-GRADE signing seed (loudly labelled; never a release
    /// key; the P18.03 ceremonies are HUMAN GATES).
    const TEST_GRADE_SEED: [u8; 32] = [0xC3u8; 32];
    const TEST_SIGNER_REFERENCE: &str = "key-store:flexcore-release-targets";

    fn test_key() -> OtaEd25519ReleaseKey {
        OtaEd25519ReleaseKey::from_injected_seed(&TEST_GRADE_SEED, TEST_SIGNER_REFERENCE)
            .expect("test-grade key constructs")
    }

    fn test_anchor(key: &OtaEd25519ReleaseKey) -> OtaTrustAnchor {
        OtaTrustAnchor::new(key.signer_reference(), key.public_key_bytes())
            .expect("anchor constructs")
    }

    #[test]
    fn canonical_encodings_are_deterministic_and_role_separated() {
        let key = test_key();
        let root = OtaRootMetadata {
            root_version: 2,
            anchors: vec![test_anchor(&key)],
            expires_at_unix_millis: 1_800_000_000_000,
        };
        let first = canonical_root_bytes(&root);
        let second = canonical_root_bytes(&root);
        assert_eq!(first, second, "the same structure encodes identically");
        // Anchor order in the structured list does not change the encoding
        // (sorted by reference).
        let mut reordered = root.clone();
        reordered.anchors.reverse();
        assert_eq!(first, canonical_root_bytes(&reordered));
        // The role prelude separates the roles inside one domain.
        assert_eq!(&first[..3], crate::ota_signing::OTA_METADATA_SIGNATURE_DOMAIN);
        assert_eq!(first[3], OtaMetadataRole::Root.code());
        let timestamp = canonical_timestamp_bytes(&OtaTimestampMetadata {
            timestamp_version: 1,
            targets_digest_hex: "ab".repeat(32),
            expires_at_unix_millis: 1_800_000_000_000,
        });
        assert_eq!(timestamp[3], OtaMetadataRole::Timestamp.code());
        assert_ne!(first, timestamp);
    }

    #[test]
    fn the_format_id_and_the_statement_pin_the_honesty_bounds() {
        assert_eq!(OTA_REPOSITORY_FORMAT, "flexcore-ota-repository-v1");
        assert_eq!(OTA_REPOSITORY_FORMAT_VERSION, 0x01);
        // The bundle format id policy carries over verbatim (the P9.34
        // rule: the manifest format id is unchanged; the repository format
        // is a separate id).
        assert_eq!(UPDATE_BUNDLE_FORMAT_V2, "flexcore-edge-update-manifest-v2");
        assert!(OTA_PRODUCE_STATEMENT.contains("NOT a TUF encoding"));
        assert!(OTA_PRODUCE_STATEMENT.contains("HUMAN GATES"));
        assert!(OTA_PRODUCE_STATEMENT.contains("STRICTLY ahead"));
        assert!(OTA_PRODUCE_STATEMENT.contains("no bundle has ever been produced"));
        assert!(OTA_PRODUCE_STATEMENT.contains("NoUpdateVerifier stays the fail-closed default"));
    }
}
